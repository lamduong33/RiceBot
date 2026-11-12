# RiceBot

A Discord bot written with [serenity](https://github.com/serenity-rs/serenity) and
[songbird](https://github.com/serenity-rs/songbird) that:

- **Greets** people joining a voice channel: it joins, plays `welcome.mp3`, and
  leaves as soon as the file finishes.
- **Chats** when mentioned (`@RiceBot what's for dinner?`), answering with a
  locally hosted LLM. No cloud AI API is used.
- **Looks things up** on the web when it needs to (optional): the model decides
  when to search, through a private, self-hosted [SearXNG](https://docs.searxng.org/).

## Architecture

```
Discord ⇄ rice_bot ──HTTP (127.0.0.1)──▶ llama-server ── model.gguf
              │
              └──HTTP (127.0.0.1)──▶ SearXNG ──▶ search engines   (optional)
```

| File | Purpose |
| --- | --- |
| `src/main.rs` | Startup, Discord client, graceful shutdown on Ctrl-C/SIGTERM |
| `src/config.rs` | Settings from environment variables / `.env` |
| `src/handler.rs` | Routes Discord events to the features |
| `src/greeter.rs` | Voice join → play → leave, one greeting per server at a time |
| `src/chat.rs` | Mention handling, per-channel history, splitting long replies, `!reload` |
| `src/personality.rs` | Loads `PERSONALITY.md` and adds the fixed chat rules |
| `src/reply.rs` | Cleans transcript-style replies (`RiceBot: Garrett: …`) before sending |
| `src/people.rs` | People the bot knows (`people.toml`), keyed by Discord user ID |
| `src/llm.rs` | Client for the OpenAI-compatible `/v1/chat/completions` API, with tool calls |
| `src/search.rs` | `web_search` tool backed by SearXNG |

## Setup

1. **Discord application:** create a bot in the
   [Developer Portal](https://discord.com/developers/applications) and invite it
   with the *Send Messages*, *Read Message History*, *Connect* and *Speak*
   permissions. No privileged intents are needed, unless you want RiceBot to
   read the message you're replying to when you mention it: then turn on
   *Message Content Intent* under *Bot* and set `DISCORD_MESSAGE_CONTENT=true`.
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
     them by `name` and always knows everyone's `notes`, whether or not they're
     tagged. After editing either file, send `@RiceBot !reload` in Discord
     (or restart the bot).
4. **Run:** `cargo run --release`

## Running in production

See [RUNBOOK.md](RUNBOOK.md) for running RiceBot as systemd services, the
`ricebot` helper command, debugging steps, and model/performance notes.

## Using RiceBot

RiceBot has no slash commands. It reacts to voice activity and to messages that
mention it.

| What you do | What RiceBot does | Who can do it |
| --- | --- | --- |
| Join a voice channel | Joins, plays the welcome sound, then leaves | Everyone |
| `@RiceBot <message>` | Replies in character using the local LLM | Everyone |
| `@RiceBot` on its own | Replies with a short hint on how to talk to it | Everyone |
| Reply to a message and mention `@RiceBot` | Answers with the replied-to message as context (needs `DISCORD_MESSAGE_CONTENT=true`, except for replies to RiceBot's own messages) | Everyone |
| `@RiceBot !reload` | Re-reads `PERSONALITY.md` and `people.toml`, clears its chat memory | Bot owner only |

### Voice greeting

When someone joins a voice channel from outside voice, RiceBot joins that channel,
plays `GREETING_AUDIO` (default `welcome.mp3`) and leaves as soon as it finishes, or
after `GREETING_MAX_SECS` (default 30) at most.

- Moving between channels, muting, deafening and leaving don't trigger a greeting.
- Other bots joining are ignored.
- One greeting runs per server at a time. If two people join at once, only the first
  is greeted.

### Chatting

Mention RiceBot anywhere in a message to talk to it:

```
@RiceBot what should I cook tonight?
what do you think about pineapple on pizza @RiceBot
```

- **Replies** go to your message as a Discord reply. RiceBot shows a typing
  indicator while it thinks. Replies longer than Discord's 2,000-character limit are
  split over several messages. `LLM_MAX_TOKENS` caps reply length (`0` for no cap).
- **Names:** RiceBot sees your name before each message, so it knows who is talking:
  your `name` from `people.toml` if you're listed there, otherwise your server
  nickname or display name. Other people you mention are shown to it the same way,
  and its replies never ping anyone except you.
- **People:** RiceBot always has everyone's `notes` from `people.toml`, so "tell me
  about Kevin" works without tagging Kevin, and so do follow-ups like "what else is
  he into?".
- **Memory:** RiceBot remembers its recent exchanges: the last
  `CHAT_HISTORY_MESSAGES` messages (default 10, which is 5 questions and answers).
  It only sees messages that mention it, not the rest of the channel. Memory is
  kept in RAM, so it's lost on restart and on `!reload`.
  - `CHAT_MEMORY_SCOPE=channel` (default): each channel has its own memory.
  - `CHAT_MEMORY_SCOPE=server`: one memory for the whole server, so RiceBot can
    bring up what was said in another channel. Each remembered message is tagged
    with its channel, like `[#gaming] Kevin: gg`, so it can tell conversations
    apart. The history limit then covers every channel together, so raise
    `CHAT_HISTORY_MESSAGES` if conversations feel forgetful.
- **Errors:** if the LLM server isn't running or doesn't answer within
  `LLM_TIMEOUT_SECS`, RiceBot apologises instead of replying.

### Web search

If `SEARXNG_URL` is set, RiceBot can search the web on its own: ask
`@RiceBot who won the game last night?` and the model decides whether it needs to
look it up. It searches at most `SEARCH_MAX_CALLS` times per message (default 2),
reads the top `SEARCH_MAX_RESULTS` results (default 5), then answers in character,
with a link when useful.

- **Needs a model that can call tools** and llama-server started with `--jinja`
  (already in `deploy/llama-server.service`). Roleplay finetunes are often bad at
  it; see the runbook for model choices. If the server rejects tools, RiceBot
  answers without searching.
- **Private search:** SearXNG runs on the same machine, listens on 127.0.0.1 only,
  and needs no account or API key. Search engines see queries from the server's IP
  with no identity attached. Setup is in `RUNBOOK.md` and `deploy/searxng/`.
- **Safety:** results are text from strangers, so:
  - only titles, snippets and links are used. Pages are never opened, so a result
    can't make the bot request addresses on your network;
  - results are labelled as untrusted information, not instructions;
  - search is the only tool. The bot can't run commands, read files or post
    anywhere but its reply, and replies can't ping anyone but the author.

  A page can still try to mislead the model, so treat answers built on search
  results like anything else you read online.
- **Slower:** a search means the model writes twice (the search, then the answer)
  and reads the results, so expect noticeably longer replies when it searches.
- Searches and their result counts are logged at info level (`ricebot logs`).

### `!reload`

`@RiceBot !reload` applies changes to `PERSONALITY.md` and `people.toml` without
restarting the bot:

1. RiceBot checks that you own the bot's Discord application, or are on the team that
   owns it. Anyone else gets a refusal.
2. It re-reads both files and replaces its personality and the people it knows.
3. It clears its chat memory in every channel, so its old replies don't pull it back
   towards the previous personality.
4. It confirms in the channel. If either file is empty, unreadable or invalid, it
   keeps everything as it was and replies with the error instead.

The mention is required. RiceBot only receives the text of messages that mention it,
so a plain `!reload` never reaches it. The command isn't case-sensitive. Changes to
`.env` still need a restart.

## Personality

`PERSONALITY.md` decides who RiceBot is and what it knows. It's a plain Markdown file
in the project folder, read when the bot starts and again on `@RiceBot !reload`.

### How it works

With every message, RiceBot sends the model a *system prompt*: standing instructions
it reads before the conversation. That prompt is built from three parts:

1. **Your personality file**, exactly as written.
2. **Fixed chat rules** added by the code (in `src/personality.rs`). These explain
   that it's on Discord, that each message starts with the sender's name, and that
   its replies shouldn't start with its own name.
3. **Everyone's notes from `people.toml`**, in file order. People without notes are
   left out.

Because the chat rules are added separately, you can rewrite the personality file
however you like without breaking how the chat works. The system prompt is the same for
every message, so llama-server can reuse its work on it instead of re-reading it
each time. After the system prompt come the remembered messages for that channel, then the new message.

| Situation | Result |
| --- | --- |
| File exists and has text | That personality is used |
| `PERSONALITY.md` doesn't exist | A short default personality is used, and a warning is logged |
| `PERSONALITY_FILE` is set but the file doesn't exist | Error |
| File is empty | Error |

An error stops the bot from starting, or makes `!reload` keep the current
personality.

### Setting it up

```sh
cp PERSONALITY.example.md PERSONALITY.md
```

`PERSONALITY.md` is in `.gitignore`, because it usually contains details about your
friends and your server. `PERSONALITY.example.md` is the committed template. To use a
file with a different name or location, set `PERSONALITY_FILE` in `.env`, for example
`PERSONALITY_FILE=personalities/pirate.md`.

### Writing a good personality

Write the file as instructions addressed to RiceBot ("You are…", "You know…"). The
template uses these sections, but any structure works:

| Section | What to put in it |
| --- | --- |
| **Who you are** | The character in a few sentences: who RiceBot is and its attitude |
| **Personality and voice** | Traits, sense of humour, catchphrases, things it loves or hates |
| **What you know** | Facts about your server, inside jokes, recurring events |
| **How you talk** | Reply length, tone, emoji, formatting, whether to stay in character |
| **Example exchanges** | A few sample messages and the replies you'd want |

Tips:

- **Examples beat adjectives.** Two or three sample exchanges teach a voice far better
  than "be witty". Write them as `Name: message` and `RiceBot: reply`, the same way
  RiceBot sees real messages.
- **Put facts about people in `people.toml`.** They're keyed by Discord ID, so they
  survive nickname changes. Be specific: "always picks the healer and complains about it" gives
  RiceBot something to work with; "is cool" doesn't.
- **Keep it to a page or two.** The whole file, plus every note in `people.toml`, is
  sent with every message, and it shares the model's context window (llama-server's
  `-c` option) with the chat history. A long prompt leaves less room for the
  conversation and makes the first reply after a start or `!reload` slower.
- **Say what to avoid.** If there are topics or jokes that are off limits, list them.
- **Tune with `!reload`.** Edit the file, send `@RiceBot !reload`, chat a little,
  repeat.
- **The model matters too.** Heavily safety-tuned models break character more often.
  `LLM_TEMPERATURE` in `.env` (default 0.7) controls how creative replies are: higher
  is more varied, lower is more predictable.

## Development

```sh
cargo test                 # unit tests
cargo test -- --ignored    # also talks to a running llama-server
cargo clippy --all-targets
```
