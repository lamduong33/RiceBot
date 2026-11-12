# RiceBot: running & debugging

RiceBot runs as two **systemd user services** that start at boot (linger is on),
restart on crash, and keep running after you log out:

- `llama-server`: llama.cpp serving the LLM on `127.0.0.1:8080`
- `ricebot`: the Discord bot (starts after llama-server)

## Day to day

```sh
ricebot              # are both up? which model?
ricebot logs         # live bot logs (Ctrl+C to exit); `ricebot logs llama-server` for the LLM
ricebot restart      # after editing .env (PERSONALITY.md / people.toml: `@RiceBot !reload` in Discord)
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
  --host 127.0.0.1 --port 8080 -t 12 -tb 24 --numa distribute --parallel 2 -c 16384
```

To make a change permanent, edit `deploy/llama-server.service` in the repo (systemd
reads it through a symlink), then:

```sh
systemctl --user daemon-reload && ricebot restart
```

Commit the change so the repo stays the source of truth.

## Gotchas

- **Any edit to a `.service` file needs `systemctl --user daemon-reload`** before
  restarting, or systemd keeps using the old version. `ricebot update` does this for you.
- **A crashing service retries forever** (every 5 s for llama-server, 10 s for the bot),
  so a broken config shows up as `activating (auto-restart)` rather than `failed`.
  `ricebot stop` while you fix it, so the logs don't fill with the same error. If a
  service ever does end up `failed`:
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
| Services (symlinks to `deploy/` in the repo) | `~/.config/systemd/user/{llama-server,ricebot}.service` |
| Current model choice (not in git; template `deploy/llama.env.example`) | `~/.config/ricebot/llama.env` |
| `ricebot` command (symlink to `deploy/ricebot`) | `~/.local/bin/ricebot` |
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

### Context size

`-c 16384` in `deploy/llama-server.service` is shared between the `--parallel 2`
slots, so each request gets **~8,192 tokens** for everything: personality, chat
rules, all `people.toml` notes, chat history, the new message and the reply. When
it's full, llama-server stops the reply early, so replies that end mid-sentence
with `LLM_MAX_TOKENS=0` usually mean the context ran out.

- **RAM:** about 160 KB per token on both models, so 16384 ≈ 2.6 GB on top of the
  model. Check `free -g` before raising it further.
- **Quality:** Mistral Nemo models like Mag-Mell get unreliable past ~16k tokens
  per request, so there's little point going above `-c 32768` with `--parallel 2`.
- **Speed:** a bigger `-c` costs nothing by itself; only text actually sent does.
  The system prompt is identical every message, so llama-server reuses it and only
  reads the new history and message.
- After changing `-c`: `systemctl --user daemon-reload && systemctl --user restart llama-server`.

## First-time setup

For a fresh machine, or rebuilding after a disk failure. Needs Rust, git, cmake, a
C++ compiler and the Hugging Face CLI (`pip install -U huggingface_hub`, which
provides `hf`).

```sh
# 1. Code and secrets
git clone https://github.com/lamduong33/RiceBot.git ~/Desktop/Git/RiceBot
cd ~/Desktop/Git/RiceBot
cp .env.example .env                     # set DISCORD_TOKEN, tune LLM_* settings
cp PERSONALITY.example.md PERSONALITY.md # or restore your real one from a backup
cp people.example.toml people.toml       # or restore your real one from a backup
cargo build --release

# 2. llama.cpp
git clone https://github.com/ggml-org/llama.cpp ~/llama.cpp
cmake -S ~/llama.cpp -B ~/llama.cpp/build -DCMAKE_BUILD_TYPE=Release
cmake --build ~/llama.cpp/build -j --target llama-server

# 3. Models (downloaded into ~/.cache/huggingface/, linked into ~/models/)
mkdir -p ~/models
ln -sf "$(hf download bartowski/MN-12B-Mag-Mell-R1-GGUF MN-12B-Mag-Mell-R1-Q4_K_M.gguf)" ~/models/
ln -sf "$(hf download bartowski/TheDrummer_Cydonia-24B-v4.3-GGUF TheDrummer_Cydonia-24B-v4.3-Q4_K_M.gguf)" ~/models/

# 4. Model choice (absolute path: systemd doesn't expand ~ in this file)
mkdir -p ~/.config/ricebot
echo "MODEL=$HOME/models/MN-12B-Mag-Mell-R1-Q4_K_M.gguf" > ~/.config/ricebot/llama.env

# 5. Helper command and services, linked so the repo stays the source of truth
mkdir -p ~/.local/bin
ln -sf ~/Desktop/Git/RiceBot/deploy/ricebot ~/.local/bin/ricebot
systemctl --user enable ~/Desktop/Git/RiceBot/deploy/{llama-server,ricebot}.service

# 6. Keep services running at boot and after logout
sudo loginctl enable-linger "$USER"

ricebot start && ricebot
```

`PERSONALITY.md`, `people.toml` and `.env` are not in git, so back them up somewhere
else. They're the only parts of the setup this section can't rebuild.

If the repo isn't at `~/Desktop/Git/RiceBot`, change `WorkingDirectory` and `ExecStart`
in `deploy/ricebot.service`, and set `RICEBOT_REPO` for the `ricebot` command.
Tune `-t`/`-tb` in `deploy/llama-server.service` to the new CPU's core count.
