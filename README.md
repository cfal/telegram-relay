# telegram-relay

A lightweight Rust HTTP server that relays messages to Telegram via a bot.

## Endpoints

| Method | Path | Body | Description |
|--------|------|------|-------------|
| POST | `/send` | `{"message": "..."}` | Send a Telegram message |
| GET | `/health` | — | Returns `{"status": "ok"}` |

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

```bash
curl -X POST http://127.0.0.1:3000/send \
  -H "Content-Type: application/json" \
  -d '{"message": "deploy finished"}'
```
