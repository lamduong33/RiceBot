# RiceBot

A Discord bot written with [serenity](https://github.com/serenity-rs/serenity) and
[songbird](https://github.com/serenity-rs/songbird) that:

- **Greets** people joining a voice channel: it joins, plays `welcome.mp3`, and
  leaves as soon as the file finishes.
- **Chats** when mentioned (`@RiceBot what's for dinner?`), answering with a
  locally hosted LLM. No external API is used.

## Architecture

```
Discord ⇄ rice_bot ──HTTP (127.0.0.1)──▶ llama-server ── model.gguf
```

| File | Purpose |
| --- | --- |
| `src/main.rs` | Startup, Discord client, graceful shutdown on Ctrl-C/SIGTERM |
| `src/config.rs` | Settings from environment variables / `.env` |
| `src/handler.rs` | Routes Discord events to the features |
| `src/greeter.rs` | Voice join → play → leave, one greeting per server at a time |
| `src/chat.rs` | Mention handling, per-channel history, splitting long replies |
| `src/people.rs` | People the bot knows (`people.toml`), keyed by Discord user ID |
| `src/llm.rs` | Client for the OpenAI-compatible `/v1/chat/completions` API |

## Setup

1. **Discord application:** create a bot in the
   [Developer Portal](https://discord.com/developers/applications) and invite it
   with the *Send Messages*, *Read Message History*, *Connect* and *Speak*
   permissions. No privileged intents are needed.
2. **LLM server:** build [llama.cpp](https://github.com/ggml-org/llama.cpp) and
   start it bound to localhost:
   ```sh
   llama-server -m model.gguf --host 127.0.0.1 --port 8080 -t <physical cores> --parallel 2
   ```
3. **Configure:** `cp .env.example .env` and set `DISCORD_TOKEN`.
   - **Personality:** `cp PERSONALITY.example.md PERSONALITY.md` and make it
     yours; it becomes the system prompt.
   - **People:** `cp people.example.toml people.toml` and fill in Discord user
     IDs (Developer Mode → right-click a user → *Copy User ID*). RiceBot calls
     them by `name` and sees their `notes` when they talk to it or are
     mentioned. Restart the bot after editing either file.
4. **Run:** `cargo run --release`

## Running in production

See [RUNBOOK.md](RUNBOOK.md) for running RiceBot as systemd services, the
`ricebot` helper command, debugging steps, and model/performance notes.

## Development

```sh
cargo test                 # unit tests
cargo test -- --ignored    # also talks to a running llama-server
cargo clippy --all-targets
```
