use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Clone, Deserialize, Serialize)]
struct Config {
    listen_addr: String,
    telegram_bot_token: String,
    telegram_username: String,
    #[serde(default)]
    telegram_chat_id: Option<i64>,
}

#[derive(Clone)]
struct AppState {
    telegram_token: String,
    chat_id: i64,
    http_client: reqwest::Client,
}

#[derive(Deserialize)]
struct SendRequest {
    message: String,
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

async fn send_telegram_message(
    state: &AppState,
    text: &str,
    parse_mode: Option<&str>,
) -> Result<(), BoxError> {
    let url = format!(
        "https://api.telegram.org/bot{}/sendMessage",
        state.telegram_token
    );

    let mut body = serde_json::json!({
        "chat_id": state.chat_id,
        "text": text,
    });
    if let Some(mode) = parse_mode {
        body["parse_mode"] = serde_json::json!(mode);
    }

    let resp = state.http_client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Telegram API error {}: {}", status, body).into());
    }

    Ok(())
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: Arc<AppState>,
) -> Result<Response<Full<Bytes>>, BoxError> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/") => {
            let is_json = req
                .headers()
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.starts_with("application/json"))
                .unwrap_or(false);

            let parse_mode = req
                .headers()
                .get("telegram-parse-mode")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| match v.to_lowercase().as_str() {
                    "markdown" => Some("MarkdownV2"),
                    "html" => Some("HTML"),
                    _ => None,
                })
                .map(String::from);

            let body_bytes = req.collect().await?.to_bytes();

            let message = if is_json {
                let payload: SendRequest = match serde_json::from_slice(&body_bytes) {
                    Ok(p) => p,
                    Err(e) => {
                        return Ok(Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Full::new(Bytes::from(format!(
                                "{{\"error\": \"invalid JSON: {}\"}}",
                                e
                            ))))?);
                    }
                };
                payload.message
            } else {
                let text = String::from_utf8(body_bytes.to_vec()).map_err(|e| {
                    format!("invalid UTF-8 in request body: {}", e)
                })?;
                if text.is_empty() {
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Full::new(Bytes::from(
                            "{\"error\": \"empty body\"}",
                        )))?);
                }
                text
            };

            match send_telegram_message(&state, &message, parse_mode.as_deref()).await {
                Ok(()) => Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(Full::new(Bytes::from("{\"status\": \"sent\"}")))?),
                Err(e) => Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Full::new(Bytes::from(format!(
                        "{{\"error\": \"telegram send failed: {}\"}}",
                        e
                    ))))?),
            }
        }

        (&Method::GET, "/health") => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::new(Bytes::from("{\"status\": \"ok\"}")))?),

        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("{\"error\": \"not found\"}")))?),
    }
}

/// Poll Telegram's getUpdates endpoint until we find a message from the
/// configured username. Returns the chat_id for that user.
async fn resolve_chat_id(
    client: &reqwest::Client,
    token: &str,
    username: &str,
) -> Result<i64, BoxError> {
    let url = format!("https://api.telegram.org/bot{}/getUpdates", token);

    // Normalize: strip leading @ if present
    let username = username.strip_prefix('@').unwrap_or(username);

    eprintln!(
        "waiting for @{} to send a message to the bot...",
        username
    );
    eprintln!("tell them to open the bot and send /start");

    let mut offset: Option<i64> = None;

    loop {
        let mut params = serde_json::json!({"timeout": 30});
        if let Some(off) = offset {
            params["offset"] = serde_json::json!(off);
        }

        let resp = client.post(&url).json(&params).send().await?;
        let body: serde_json::Value = resp.json().await?;

        if let Some(updates) = body["result"].as_array() {
            for update in updates {
                // Track offset so we don't re-process old updates
                if let Some(id) = update["update_id"].as_i64() {
                    offset = Some(id + 1);
                }

                let msg = &update["message"];
                if let Some(from_username) = msg["from"]["username"].as_str() {
                    if from_username.eq_ignore_ascii_case(username) {
                        if let Some(chat_id) = msg["chat"]["id"].as_i64() {
                            eprintln!("resolved @{} -> chat_id {}", username, chat_id);
                            return Ok(chat_id);
                        }
                    }
                }
            }
        }
    }
}

fn load_config(path: &PathBuf) -> Result<Config, BoxError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config file {}: {}", path.display(), e))?;
    let config: Config = serde_json::from_str(&contents)
        .map_err(|e| format!("failed to parse config file {}: {}", path.display(), e))?;
    Ok(config)
}

fn save_config(path: &PathBuf, config: &Config) -> Result<(), BoxError> {
    let contents = serde_json::to_string_pretty(config)?;
    std::fs::write(path, contents.as_bytes())
        .map_err(|e| format!("failed to write config file {}: {}", path.display(), e))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let config_path = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "config.json".to_string()),
    );

    let mut config = load_config(&config_path)?;
    let client = reqwest::Client::new();

    let chat_id = match config.telegram_chat_id {
        Some(id) => {
            eprintln!("using cached chat_id {} for @{}", id, config.telegram_username);
            id
        }
        None => {
            let id = resolve_chat_id(
                &client,
                &config.telegram_bot_token,
                &config.telegram_username,
            )
            .await?;

            // Persist resolved chat_id back to config
            config.telegram_chat_id = Some(id);
            save_config(&config_path, &config)?;
            eprintln!("saved chat_id to {}", config_path.display());

            id
        }
    };

    let state = Arc::new(AppState {
        telegram_token: config.telegram_bot_token,
        chat_id,
        http_client: client,
    });

    let addr: SocketAddr = config.listen_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;
    eprintln!("listening on {}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let state = state.clone();

        tokio::task::spawn(async move {
            let service = service_fn(move |req| {
                let state = state.clone();
                handle_request(req, state)
            });

            if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                eprintln!("connection error: {:?}", e);
            }
        });
    }
}
