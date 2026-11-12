# RiceBot: running & debugging

RiceBot runs as two **systemd user services** that start at boot (linger is on),
restart on crash, and keep running after you log out:

- `llama-server`: llama.cpp serving the LLM on `127.0.0.1:8080`
- `ricebot`: the Discord bot (starts after llama-server)

## Day to day

```sh
ricebot              # are both up? which model?
ricebot logs         # live bot logs (Ctrl+C to exit); `ricebot logs llama-server` for the LLM
ricebot restart      # after editing .env, PERSONALITY.md or people.toml
ricebot stop|start   # take offline / bring back
ricebot update       # after code changes: git pull, build, restart
ricebot model cydonia    # switch to the 24B model (`mag-mell` back, or a path to any .gguf)
```

After a model switch, llama-server takes ~10–30 s to load; messages sent in that
window may fail.

## When something's wrong

### 1. Status and recent errors

```sh
ricebot status
journalctl --user -u ricebot -n 50 --no-pager          # last 50 bot lines
journalctl --user -u llama-server -n 50 --no-pager     # last 50 LLM lines
journalctl --user -u ricebot --since "1 hour ago" -p warning   # only warnings and errors
```

`activating (auto-restart)` or `failed` in status means it's crash-looping.

### 2. Test the LLM server on its own

```sh
curl -s localhost:8080/health
curl -s localhost:8080/v1/chat/completions -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"Say hi in 5 words"}],"max_tokens":20}'
```

If that works but the bot doesn't reply in Discord, the problem is on the bot side.

### 3. Run the bot in the foreground with debug logs

This is the main debugging loop. **Stop the service first**, or two copies of RiceBot
will both reply to every message.

```sh
systemctl --user stop ricebot          # llama-server keeps running
cd ~/Desktop/Git/RiceBot
RUST_LOG=rice_bot=debug cargo run --release
# ... edit code, Ctrl+C, re-run ...
systemctl --user start ricebot         # hand back to the service when done
```

`cargo run --release` rebuilds `target/release/rice_bot`, the same binary the service
runs. When you start the service again it runs **whatever you were just editing**,
including uncommitted changes. To go back to what's on GitHub, run
`git stash` (or commit), then `ricebot update`.

### 4. Try llama-server settings by hand

```sh
systemctl --user stop ricebot llama-server
~/llama.cpp/build/bin/llama-server -m ~/models/MN-12B-Mag-Mell-R1-Q4_K_M.gguf \
  --host 127.0.0.1 --port 8080 -t 12 -tb 24 --numa distribute --parallel 2 -c 8192
```

To make a change permanent, edit `~/.config/systemd/user/llama-server.service`, then:

```sh
systemctl --user daemon-reload && ricebot restart
```

## Gotchas

- **Any edit to a `.service` file needs `systemctl --user daemon-reload`** before
  restarting, or systemd keeps using the old version.
- **After too many crashes**, systemd stops retrying. Fix the cause, then:
  `systemctl --user reset-failed ricebot && ricebot start`
- **The service runs the built binary, not your source.** After code changes, run
  `ricebot update`, or `cargo build --release && ricebot restart` if you haven't
  pushed yet. Otherwise the old version keeps running.
- **Slow replies or timeouts** usually come from the CPU, not a bug.
  `ricebot logs llama-server` shows tokens/sec for each request.
- **Permanent debug logging:** set `RUST_LOG=rice_bot=debug` in `.env`, then `ricebot restart`.
- **The bot reads its files from its working directory.** `.env`, `PERSONALITY.md`,
  `people.toml` and `welcome.mp3` are relative paths, so `ricebot.service` needs
  `WorkingDirectory=%h/Desktop/Git/RiceBot`. Without it the bot quietly uses the
  built-in personality and knows no one, and greetings fail. If RiceBot suddenly
  sounds generic, check this first.

## Where things live

| What | Path |
|---|---|
| Bot code, `.env` (token, LLM settings) | `~/Desktop/Git/RiceBot/` |
| Personality (system prompt), known people | `~/Desktop/Git/RiceBot/{PERSONALITY.md,people.toml}` |
| Services | `~/.config/systemd/user/{llama-server,ricebot}.service` |
| Current model choice | `~/.config/ricebot/llama.env` |
| `ricebot` command | `~/.local/bin/ricebot` |
| llama.cpp build | `~/llama.cpp/build/bin/` |
| Models (symlinks into `~/.cache/huggingface/`, don't delete that cache) | `~/models/` |

## Models & speed (Threadripper 2970WX, CPU only)

| Model | Size | Prompt reading | Reply writing |
|---|---|---|---|
| **Mag-Mell 12B** (default) | 7.5 GB | ~22–29 tok/s | ~3.3 tok/s (12 threads) |
| Cydonia 24B v4.3 | 14.3 GB | not benchmarked (est. ~12 tok/s) | not benchmarked (est. ~1.5 tok/s) |

A typical reply on the 12B takes ~30–60 s behind the "typing…" indicator.
Reply length, timeout and temperature are set by `LLM_MAX_TOKENS`, `LLM_TIMEOUT_SECS`
and `LLM_TEMPERATURE` in the server's `.env` (see `.env.example` for all settings).
On the 12B, keep `LLM_TIMEOUT_SECS` well above `LLM_MAX_TOKENS` ÷ 3 tok/s plus prompt time.
