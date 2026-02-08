# telegram-relay

A lightweight Rust HTTP server that relays messages to Telegram via a bot. Useful as a simple push notification service — point any webhook, CI pipeline, or script at it and get messages straight to your phone.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/` | Send a Telegram message |
| GET | `/health` | Returns `{"status": "ok"}` |

If `path_prefix` is set in the config (e.g. `"path_prefix": "secret"`), all endpoints move under that prefix: `POST /secret`, `GET /secret/health`.

## Setup

1. Create a bot with [@BotFather](https://t.me/BotFather) and grab the token.
2. Copy the sample config and fill it in:
   ```bash
   cp config.sample.json config.json
   ```
3. Build and run:
   ```bash
   cargo build --release
   ./target/release/telegram-relay
   # or: cargo run --release -- /path/to/config.json
   ```
4. On first run the app waits for you to send `/start` to your bot in Telegram. Once it sees a message from the configured username it resolves and caches the `telegram_chat_id` in your config file.

## Usage

The body is sent directly as the message text. JSON is also supported.

```bash
# plain text (shortest form)
curl -d 'deploy finished' http://127.0.0.1:3000

# JSON
curl -H 'Content-Type: application/json' \
  -d '{"message": "deploy finished"}' http://127.0.0.1:3000
```

### Formatting

Set the `telegram-parse-mode` header to enable Telegram formatting:

```bash
# MarkdownV2
curl -H 'telegram-parse-mode: markdown' \
  -d '*bold* _italic_' http://127.0.0.1:3000

# HTML
curl -H 'telegram-parse-mode: html' \
  -d '<b>bold</b> <i>italic</i>' http://127.0.0.1:3000
```

## Config

| Field | Required | Description |
|-------|----------|-------------|
| `listen_addr` | yes | Address to bind (e.g. `127.0.0.1:3000`) |
| `telegram_bot_token` | yes | Bot token from BotFather |
| `telegram_username` | yes | Telegram username to resolve chat ID for |
| `telegram_chat_id` | no | Resolved automatically on first run |
| `path_prefix` | no | Prefix for all routes (e.g. `"secret"` → `/secret`) |
