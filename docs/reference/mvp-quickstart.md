# MVP Quickstart

This guide covers the current MVP foundation flow for local usage.

## 0) One-Command Install

macOS/Linux:

```bash
./scripts/install.sh --setup
```

PowerShell:

```powershell
pwsh ./scripts/install.ps1 -Setup
```

## 1) Bootstrap

Generate config and local memory database:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- setup --force
```

Custom config path:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- setup --output ~/.loongclaw/custom.toml --force
```

## 2) Start CLI Chat Channel

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- chat
```

Custom session:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- chat --session demo
```

Custom config:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- chat --config ~/.loongclaw/custom.toml
```

## 3) Start Telegram Channel

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- telegram-serve --config ~/.loongclaw/custom.toml
```

Run only one polling cycle (smoke check):

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- telegram-serve --once
```

## 4) Start Feishu Channel (Webhook)

Run webhook server (default bind `127.0.0.1:8080`, path `/feishu/events`):

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- feishu-serve --config ~/.loongclaw/custom.toml
```

Override bind/path at runtime:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- feishu-serve --bind 0.0.0.0:18080 --path /bot/events
```

Send proactive Feishu message:

```bash
cargo run -p loongclaw-daemon --bin loongclawd -- feishu-send --receive-id <chat_id> --text "hello"
```

## 5) Config Schema (TOML)

```toml
[provider]
kind = "openai_compatible" # or "volcengine_custom"
model = "gpt-4o-mini"
base_url = "https://api.openai.com"
chat_completions_path = "/v1/chat/completions"
api_key_env = "OPENAI_API_KEY"
temperature = 0.2
request_timeout_ms = 30000
retry_max_attempts = 3
retry_initial_backoff_ms = 300
retry_max_backoff_ms = 3000

[provider.headers]
# Optional custom headers
# "X-Request-Source" = "loongclaw"

[cli]
enabled = true
system_prompt = "You are LoongClaw, a practical assistant."
exit_commands = ["/exit", "/quit"]

[telegram]
enabled = false
bot_token_env = "TELEGRAM_BOT_TOKEN"
base_url = "https://api.telegram.org"
polling_timeout_s = 15
allowed_chat_ids = []

[feishu]
enabled = false
app_id_env = "FEISHU_APP_ID"
app_secret_env = "FEISHU_APP_SECRET"
verification_token_env = "FEISHU_VERIFICATION_TOKEN"
encrypt_key_env = "FEISHU_ENCRYPT_KEY"
base_url = "https://open.feishu.cn"
receive_id_type = "chat_id"
webhook_bind = "127.0.0.1:8080"
webhook_path = "/feishu/events"
allowed_chat_ids = []
ignore_bot_messages = true

[tools]
shell_allowlist = ["echo", "cat", "ls", "pwd"]
file_root = "."

[memory]
sqlite_path = "~/.loongclaw/memory.sqlite3"
sliding_window = 12
```

If you configure Feishu event encryption (`encrypt_key_env` or `encrypt_key`),
the webhook handler enforces request signature verification using
`X-Lark-Request-Timestamp`, `X-Lark-Request-Nonce`, and `X-Lark-Signature`.

## 6) Tool Runtime (Core)

The core tool adapter currently supports:

- `shell.exec`
- `file.read`
- `file.write`

`shell.exec` is constrained by the configured shell allowlist.
`file.read` / `file.write` are constrained by configured `file_root`.

## 7) Feature Flags

Default feature set enables MVP foundation:

- `config-toml`
- `memory-sqlite`
- `tool-shell`
- `tool-file`
- `channel-cli`
- `channel-telegram`
- `channel-feishu`
- `provider-openai`
- `provider-volcengine`

Build with no default features:

```bash
cargo build -p loongclaw-daemon --no-default-features
```

Build with selected feature set:

```bash
cargo build -p loongclaw-daemon --no-default-features --features "channel-cli,config-toml,memory-sqlite"
```
