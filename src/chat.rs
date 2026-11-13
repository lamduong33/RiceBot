//! Answers messages that mention the bot, using the local LLM.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, PoisonError, RwLock},
};

use anyhow::Result;
use serenity::{
    all::{ChannelId, CreateAllowedMentions, CreateMessage, GuildId, Message, UserId},
    client::Context,
    utils::{ContentSafeOptions, content_safe},
};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::{
    config::{ChatConfig, LlmConfig, MemoryScope, SearchConfig},
    llm::{ChatMessage, Completion, LlmClient, Role},
    people::People,
    personality::{self, Source},
    reply,
    search::{self, SearchClient},
};

/// Discord's limit on the length of a single message, in characters.
const MESSAGE_LIMIT: usize = 2000;

const EMPTY_PROMPT_REPLY: &str = "Hi! Mention me together with a question and I'll do my best to answer.";
/// Sent as `@RiceBot !reload`. The bot doesn't request the message content
/// intent, so commands have to mention it like any other message.
const RELOAD_COMMAND: &str = "!reload";
/// The name the personality gives the bot, stripped from the start of replies
/// along with its Discord username.
const BOT_NAME: &str = "RiceBot";
const LLM_ERROR_REPLY: &str = "Sorry, I can't think right now: my language model isn't responding. Try again in a bit.";
/// Longest quote of a replied-to message passed to the model, in characters.
const QUOTE_LIMIT: usize = 1000;

pub struct Chat {
    llm: LlmClient,
    /// Limits how many requests are sent to the model at once.
    permits: Semaphore,
    /// Where the personality and people files are, for `!reload`.
    files: ChatConfig,
    /// Personality and people, replaced together by `!reload`. Each message
    /// takes its own reference, so a reload never changes a reply halfway.
    knowledge: RwLock<Arc<Knowledge>>,
    /// Web search offered to the model as a tool, if `SEARXNG_URL` is set.
    search: Option<SearchClient>,
    history_len: usize,
    memory_scope: MemoryScope,
    /// Recent conversation per channel or per server, oldest first.
    history: Mutex<HashMap<MemoryKey, VecDeque<ChatMessage>>>,
}

/// Which conversation a message belongs to in chat memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MemoryKey {
    Channel(ChannelId),
    Server(GuildId),
}

/// What the bot is told before every conversation: who it is and who it knows.
struct Knowledge {
    /// The personality file, the fixed chat rules and everyone's notes. The
    /// same for every message, so llama-server can reuse its work on it.
    system_prompt: String,
    personality_source: Source,
    people: People,
}

impl Chat {
    pub fn new(chat: &ChatConfig, llm: &LlmConfig, search: &SearchConfig) -> Result<Self> {
        let search = SearchClient::new(search)?;
        match &search {
            Some(client) => info!(max_searches_per_message = client.max_calls(), "web search is on"),
            None => info!("web search is off (SEARXNG_URL isn't set)"),
        }
        Ok(Self {
            llm: LlmClient::new(llm)?,
            permits: Semaphore::new(llm.max_concurrency),
            files: chat.clone(),
            knowledge: RwLock::new(Arc::new(load_knowledge(chat, search.is_some())?)),
            search,
            history_len: chat.history_messages,
            memory_scope: chat.memory_scope,
            history: Mutex::default(),
        })
    }

    fn knowledge(&self) -> Arc<Knowledge> {
        Arc::clone(&self.knowledge.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub async fn on_message(&self, ctx: &Context, msg: &Message) {
        let bot_id = ctx.cache.current_user().id;
        if msg.author.bot || !msg.mentions_user_id(bot_id) {
            return;
        }

        let knowledge = self.knowledge();
        let prompt = clean_prompt(ctx, msg, bot_id, &knowledge.people);
        if prompt.is_empty() {
            send_reply(ctx, msg, EMPTY_PROMPT_REPLY).await;
            return;
        }
        if prompt.eq_ignore_ascii_case(RELOAD_COMMAND) {
            self.reload(ctx, msg).await;
            return;
        }

        let author = author_name(msg, bot_id, &knowledge.people);
        // When the mention is a reply, the model sees what's being replied to.
        let quote = msg
            .referenced_message
            .as_deref()
            .map(|quoted| quote_of(ctx, quoted, bot_id, &knowledge.people))
            .unwrap_or_default();
        info!(user = %msg.author.id, channel = %msg.channel_id, "answering mention");

        // With server-wide memory, each message says which channel it came
        // from. Messages outside a server (DMs) keep per-channel memory.
        let (key, tag) = match (self.memory_scope, msg.guild_id) {
            (MemoryScope::Server, Some(guild_id)) => {
                let channel = msg
                    .channel_id
                    .name(ctx)
                    .await
                    .unwrap_or_else(|_| "unknown-channel".into());
                (MemoryKey::Server(guild_id), format!("[#{channel}] "))
            }
            _ => (MemoryKey::Channel(msg.channel_id), String::new()),
        };

        let question = ChatMessage::new(Role::User, format!("{tag}{author}: {quote}{prompt}"));
        let history = self.recent_history(key);
        let speakers = speaker_names(&author, &knowledge.people, &history);
        let mut system_prompt = knowledge.system_prompt.clone();
        if self.search.is_some() {
            // So the model knows what "recent" means, followed by the warning
            // that makes it search for things newer than its training, which
            // works best as the last thing in the prompt. Added last, so they
            // only change the cached prompt once a day.
            let today = msg.timestamp.to_string();
            system_prompt = format!(
                "{system_prompt}\n\nToday's date (UTC): {}\n\n{}",
                &today[..10.min(today.len())],
                personality::STALE_KNOWLEDGE_WARNING
            );
        }
        let mut conversation = vec![ChatMessage::new(Role::System, system_prompt)];
        conversation.extend(history);
        conversation.push(question.clone());

        let typing = msg.channel_id.start_typing(&ctx.http);
        let result = match self.permits.acquire().await {
            Ok(_permit) => self.answer(&mut conversation).await,
            Err(_) => return, // The semaphore is never closed.
        };
        typing.stop();

        match result {
            Ok(raw) => {
                let bot_name = ctx.cache.current_user().name.clone();
                let others: Vec<&str> = speakers.iter().map(String::as_str).collect();
                let answer = reply::clean(&raw, &[BOT_NAME, &bot_name], &others);
                if answer != raw {
                    debug!(raw = %raw, "removed transcript-style text from the reply");
                }
                // Saving the cleaned reply keeps a bad one from teaching the
                // model to do it again.
                send_reply(ctx, msg, &answer).await;
                self.remember(key, question, ChatMessage::new(Role::Assistant, answer));
            }
            Err(e) => {
                error!("LLM request failed: {e:#}");
                send_reply(ctx, msg, LLM_ERROR_REPLY).await;
            }
        }
    }

    /// Gets the model's reply, letting it search the web first when search is
    /// on. Only the final reply is kept in chat memory, not the searches.
    async fn answer(&self, conversation: &mut Vec<ChatMessage>) -> Result<String> {
        let Some(search) = &self.search else {
            return self.llm.complete(conversation).await;
        };
        let tools = [search::tool_definition()];
        let mut searches = 0;
        // Each round either runs a search or refuses one, so this ends after
        // at most max_calls + 1 rounds with tools, then one without.
        for round in 0..=search.max_calls() {
            if searches >= search.max_calls() {
                break;
            }
            let completion = match self.llm.chat(conversation, Some(&tools)).await {
                Ok(completion) => completion,
                // Usually llama-server running without --jinja, or a chat
                // template without tool support. Answer without searching.
                Err(e) if round == 0 => {
                    warn!("LLM request with tools failed, answering without web search: {e:#}");
                    return self.llm.complete(conversation).await;
                }
                Err(e) => return Err(e),
            };
            let (content, calls) = match completion {
                Completion::Reply(reply) => return Ok(reply),
                Completion::ToolCalls { content, calls } => (content, calls),
            };

            conversation.push(ChatMessage::tool_request(content, calls.clone()));
            for call in calls {
                let result = if call.function.name != search::TOOL_NAME {
                    format!("There is no tool called {:?}. Only web_search is available.", call.function.name)
                } else if searches >= search.max_calls() {
                    "Search limit reached for this message. Answer with what you have.".into()
                } else {
                    searches += 1;
                    search.run_tool(&call.function.arguments).await
                };
                conversation.push(ChatMessage::tool_result(call.id, result));
            }
        }
        // Out of searches: the last request offers no tools, so the model has
        // to answer with what it found.
        self.llm.complete(conversation).await
    }

    /// Re-reads the personality and people files and clears chat memory. Only
    /// the bot's owner (or members of the team that owns it) may do this.
    async fn reload(&self, ctx: &Context, msg: &Message) {
        if !is_bot_owner(ctx, msg.author.id).await {
            info!(user = %msg.author.id, "ignored !reload from someone who doesn't own the bot");
            send_reply(ctx, msg, "Only my owner can reload my personality.").await;
            return;
        }

        let reply = match load_knowledge(&self.files, self.search.is_some()) {
            Ok(knowledge) => {
                let personality = match knowledge.personality_source {
                    Source::File => format!("my personality from `{}`", self.files.personality_file.display()),
                    Source::Default => format!(
                        "my default personality (`{}` doesn't exist)",
                        self.files.personality_file.display()
                    ),
                };
                let people = knowledge.people.len();
                *self.knowledge.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(knowledge);
                // Old replies would keep nudging the model towards the old personality.
                self.history.lock().unwrap_or_else(PoisonError::into_inner).clear();
                format!(
                    "Reloaded {personality} and {people} {} from `{}`, and cleared my chat memory.",
                    if people == 1 { "person" } else { "people" },
                    self.files.people_file.display()
                )
            }
            Err(e) => {
                warn!("could not reload personality or people: {e:#}");
                format!("Couldn't reload, so I'm keeping everything as it was: {e:#}")
            }
        };
        send_reply(ctx, msg, &reply).await;
    }

    fn recent_history(&self, key: MemoryKey) -> Vec<ChatMessage> {
        self.history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .map(|messages| messages.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Stores a question and its answer together, so they stay next to each
    /// other even when replies in several channels finish at the same time.
    fn remember(&self, key: MemoryKey, question: ChatMessage, answer: ChatMessage) {
        if self.history_len == 0 {
            return;
        }
        let mut history = self.history.lock().unwrap_or_else(PoisonError::into_inner);
        let messages = history.entry(key).or_default();
        messages.extend([question, answer]);
        while messages.len() > self.history_len {
            messages.pop_front();
        }
    }
}

/// Reads the personality and people files, logging what was loaded.
fn load_knowledge(files: &ChatConfig, search: bool) -> Result<Knowledge> {
    let (mut system_prompt, personality_source) =
        personality::load(&files.personality_file, files.personality_file_required)?;
    let people = People::load(&files.people_file, files.people_file_required)?;
    // Everyone's notes go in the single system message, because some chat
    // templates reject more than one.
    if files.memory_scope == MemoryScope::Server {
        system_prompt = format!("{system_prompt}\n{}", personality::SERVER_MEMORY_RULES);
    }
    if search {
        system_prompt = format!("{system_prompt}\n\n{}", personality::SEARCH_RULES);
    }
    if let Some(notes) = people.describe_all() {
        system_prompt = format!("{system_prompt}\n\n{notes}");
    }

    match personality_source {
        Source::File => info!(file = %files.personality_file.display(), "loaded personality"),
        Source::Default => warn!(
            file = %files.personality_file.display(),
            "personality file not found, using the default personality"
        ),
    }
    info!(count = people.len(), file = %files.people_file.display(), "loaded people");

    Ok(Knowledge { system_prompt, personality_source, people })
}

/// Names that can appear as "Name:" labels for people other than the bot:
/// the author, everyone in `people.toml` and everyone in chat memory.
fn speaker_names(author: &str, people: &People, history: &[ChatMessage]) -> Vec<String> {
    let from_history = history
        .iter()
        .filter(|message| message.role == Role::User)
        .filter_map(|message| reply::strip_channel_tag(&message.content).split_once(": "))
        .map(|(name, _)| name);
    let mut names: Vec<String> = Vec::new();
    for name in std::iter::once(author).chain(people.names()).chain(from_history) {
        let name = name.trim();
        if !name.is_empty() && !names.iter().any(|known| known.eq_ignore_ascii_case(name)) {
            names.push(name.to_owned());
        }
    }
    names
}

/// What the bot calls the author of `msg`: their name in `people`, else their
/// server nickname, else their Discord display name.
fn author_name(msg: &Message, bot_id: UserId, people: &People) -> String {
    if msg.author.id == bot_id {
        return BOT_NAME.to_owned();
    }
    match people.get(msg.author.id) {
        Some(person) => person.name.clone(),
        None => msg
            .member
            .as_ref()
            .and_then(|member| member.nick.clone())
            .unwrap_or_else(|| msg.author.display_name().to_owned()),
    }
}

/// Describes the message being replied to, for the start of the question:
/// `(replying to Kevin: "text") `.
fn quote_of(ctx: &Context, quoted: &Message, bot_id: UserId, people: &People) -> String {
    let author = author_name(quoted, bot_id, people);
    let text = readable_content(ctx, quoted, &quoted.content, people);
    let text = if !text.is_empty() {
        truncate(&text, QUOTE_LIMIT)
    } else if !quoted.attachments.is_empty() || !quoted.embeds.is_empty() {
        "[an attachment or embed]".to_owned()
    } else {
        // Discord leaves the text out for bots without the Message Content
        // intent, unless the message mentions the bot or is the bot's own.
        debug!(message = %quoted.id, "replied-to message has no readable text");
        return format!("(replying to a message from {author} that you can't see) ");
    };
    format!("(replying to {author}: \"{}\") ", text.replace('\n', " "))
}

/// The first `limit` characters of `text`, with "…" if any were cut.
fn truncate(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", text[..end].trim_end()),
        None => text.to_owned(),
    }
}

/// Whether `user` owns the bot's Discord application, directly or through a team.
async fn is_bot_owner(ctx: &Context, user: UserId) -> bool {
    match ctx.http.get_current_application_info().await {
        Ok(app) => {
            app.owner.as_ref().is_some_and(|owner| owner.id == user)
                || app
                    .team
                    .as_ref()
                    .is_some_and(|team| team.members.iter().any(|member| member.user.id == user))
        }
        Err(e) => {
            warn!("could not look up the bot's owner: {e}");
            false
        }
    }
}

/// Removes the bot's own mention and turns any other mentions into plain
/// names, so the model sees readable text. People in `people` are shown by
/// their known name rather than their nickname.
fn clean_prompt(ctx: &Context, msg: &Message, bot_id: UserId, people: &People) -> String {
    readable_content(ctx, msg, &strip_mention(&msg.content, bot_id), people)
}

/// Turns the mentions in `content`, which comes from `msg`, into plain names.
fn readable_content(ctx: &Context, msg: &Message, content: &str, people: &People) -> String {
    let mut content = content.to_owned();
    for user in &msg.mentions {
        if let Some(person) = people.get(user.id) {
            content = replace_mention(&content, user.id, &format!("@{}", person.name));
        }
    }
    let mut options = ContentSafeOptions::default();
    if let Some(guild_id) = msg.guild_id {
        options = options.display_as_member_from(guild_id);
    }
    content_safe(&ctx.cache, content, &options, &msg.mentions)
        .trim()
        .to_owned()
}

fn strip_mention(content: &str, user_id: UserId) -> String {
    replace_mention(content, user_id, "")
}

fn replace_mention(content: &str, user_id: UserId, with: &str) -> String {
    content
        .replace(&format!("<@{user_id}>"), with)
        .replace(&format!("<@!{user_id}>"), with)
}

/// Replies to `msg`, splitting long text over several messages. Pings no one
/// except the author of the message being replied to, whatever the text says.
async fn send_reply(ctx: &Context, msg: &Message, text: &str) {
    for (i, chunk) in split_message(text, MESSAGE_LIMIT).into_iter().enumerate() {
        let mut builder = CreateMessage::new().content(chunk);
        builder = if i == 0 {
            builder
                .reference_message(msg)
                .allowed_mentions(CreateAllowedMentions::new().replied_user(true))
        } else {
            builder.allowed_mentions(CreateAllowedMentions::new())
        };

        if let Err(e) = msg.channel_id.send_message(&ctx.http, builder).await {
            warn!(channel = %msg.channel_id, "failed to send reply: {e}");
            return;
        }
    }
}

/// Splits `text` into chunks of at most `limit` characters, preferring to
/// break at newlines, then spaces.
fn split_message(text: &str, limit: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let end = match rest.char_indices().nth(limit) {
            None => rest.len(),
            Some((hard_end, _)) => {
                let window = &rest[..hard_end];
                window
                    .rfind('\n')
                    .or_else(|| window.rfind(' '))
                    .filter(|&i| i > 0)
                    .unwrap_or(hard_end)
            }
        };
        chunks.push(rest[..end].trim_end());
        rest = rest[end..].trim_start();
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end tests of the search loop against fake LLM and SearXNG
    /// servers, so tool calling is checked without a real model.
    mod search_loop {
        use std::{
            io::{BufRead, BufReader, Read, Write},
            net::TcpListener,
            path::PathBuf,
            sync::{Arc, Mutex},
            thread,
            time::Duration,
        };

        use serde_json::{Value, json};

        use super::*;

        /// Answers each HTTP request with the next queued (status, body) and
        /// records the request bodies.
        struct FakeServer {
            url: String,
            requests: Arc<Mutex<Vec<String>>>,
        }

        impl FakeServer {
            fn start(responses: Vec<(u16, Value)>) -> Self {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                let url = format!("http://{}", listener.local_addr().unwrap());
                let requests = Arc::new(Mutex::new(Vec::new()));
                let seen = Arc::clone(&requests);
                thread::spawn(move || {
                    for (stream, (status, body)) in listener.incoming().zip(responses) {
                        let mut stream = stream.unwrap();
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut first = String::new();
                        reader.read_line(&mut first).unwrap();
                        let mut length = 0;
                        loop {
                            let mut line = String::new();
                            reader.read_line(&mut line).unwrap();
                            if line == "\r\n" {
                                break;
                            }
                            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                                length = value.trim().parse().unwrap();
                            }
                        }
                        let mut request_body = vec![0; length];
                        reader.read_exact(&mut request_body).unwrap();
                        let request_body = String::from_utf8(request_body).unwrap();
                        seen.lock().unwrap().push(format!("{}{request_body}", first.trim_end()));
                        let body = body.to_string();
                        write!(
                            stream,
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .unwrap();
                    }
                });
                Self { url, requests }
            }

            fn requests(&self) -> Vec<String> {
                self.requests.lock().unwrap().clone()
            }
        }

        fn reply(text: &str) -> (u16, Value) {
            (200, json!({"choices": [{"message": {"content": text}}]}))
        }

        fn search_call(query: &str) -> (u16, Value) {
            (200, json!({"choices": [{"message": {"content": "", "tool_calls": [{
                "id": "abc123XYZ", "type": "function",
                "function": {"name": "web_search", "arguments": json!({"query": query}).to_string()}
            }]}}]}))
        }

        fn searx_results() -> (u16, Value) {
            (200, json!({"results": [{"title": "Rice prices", "url": "https://rice.example", "content": "Rice is $1."}]}))
        }

        fn chat(llm: &FakeServer, searx: Option<&FakeServer>, max_calls: usize) -> Chat {
            let llm_config = LlmConfig {
                base_url: llm.url.clone(),
                model: "local".into(),
                timeout: Duration::from_secs(5),
                max_tokens: None,
                temperature: 0.7,
                max_concurrency: 1,
            };
            let search = SearchConfig {
                url: searx.map(|server| server.url.clone()),
                max_calls,
                ..SearchConfig::default()
            };
            Chat {
                llm: LlmClient::new(&llm_config).unwrap(),
                permits: Semaphore::new(1),
                files: ChatConfig {
                    personality_file: PathBuf::from("does/not/exist.md"),
                    personality_file_required: false,
                    people_file: PathBuf::from("does/not/exist.toml"),
                    people_file_required: false,
                    history_messages: 10,
                    memory_scope: MemoryScope::Channel,
                },
                knowledge: RwLock::new(Arc::new(Knowledge {
                    system_prompt: String::new(),
                    personality_source: Source::Default,
                    people: People::default(),
                })),
                search: SearchClient::new(&search).unwrap(),
                history_len: 10,
                memory_scope: MemoryScope::Channel,
                history: Mutex::default(),
            }
        }

        fn conversation() -> Vec<ChatMessage> {
            vec![
                ChatMessage::new(Role::System, "You are RiceBot."),
                ChatMessage::new(Role::User, "Sam: how much is rice?"),
            ]
        }

        #[tokio::test]
        async fn searches_then_answers() {
            let llm = FakeServer::start(vec![search_call("rice price"), reply("Rice is $1, sprout.")]);
            let searx = FakeServer::start(vec![searx_results()]);
            let chat = chat(&llm, Some(&searx), 2);

            let mut conversation = conversation();
            assert_eq!(chat.answer(&mut conversation).await.unwrap(), "Rice is $1, sprout.");

            let llm_requests = llm.requests();
            assert_eq!(llm_requests.len(), 2);
            assert!(llm_requests[0].contains(r#""tools":[{"#));
            // The search result goes back as a tool message answering the call.
            assert!(llm_requests[1].contains(r#""role":"tool""#));
            assert!(llm_requests[1].contains(r#""tool_call_id":"abc123XYZ""#));
            assert!(llm_requests[1].contains("Rice is $1."));
            assert!(searx.requests()[0].starts_with("GET /search?q=rice+price&format=json"));
        }

        #[tokio::test]
        async fn stops_offering_tools_at_the_limit() {
            let llm = FakeServer::start(vec![search_call("one"), reply("done")]);
            let searx = FakeServer::start(vec![searx_results()]);
            let chat = chat(&llm, Some(&searx), 1);

            assert_eq!(chat.answer(&mut conversation()).await.unwrap(), "done");
            let llm_requests = llm.requests();
            assert!(llm_requests[0].contains(r#""tools":"#));
            assert!(!llm_requests[1].contains(r#""tools":"#), "last request must not offer tools");
            assert_eq!(searx.requests().len(), 1);
        }

        #[tokio::test]
        async fn falls_back_when_the_server_rejects_tools() {
            let llm = FakeServer::start(vec![(500, json!({"error": "tools need --jinja"})), reply("plain answer")]);
            let searx = FakeServer::start(vec![]);
            let chat = chat(&llm, Some(&searx), 2);

            assert_eq!(chat.answer(&mut conversation()).await.unwrap(), "plain answer");
            assert!(!llm.requests()[1].contains(r#""tools":"#));
        }

        #[tokio::test]
        async fn search_errors_are_told_to_the_model() {
            let llm = FakeServer::start(vec![search_call("rice"), reply("couldn't look it up")]);
            let searx = FakeServer::start(vec![(403, json!({}))]);
            let chat = chat(&llm, Some(&searx), 2);

            assert_eq!(chat.answer(&mut conversation()).await.unwrap(), "couldn't look it up");
            assert!(llm.requests()[1].contains("The web search failed"));
        }

        #[tokio::test]
        async fn without_searxng_no_tools_are_offered() {
            let llm = FakeServer::start(vec![reply("hi")]);
            let chat = chat(&llm, None, 2);

            assert_eq!(chat.answer(&mut conversation()).await.unwrap(), "hi");
            assert!(!llm.requests()[0].contains(r#""tools":"#));
        }
    }

    #[test]
    fn speaker_names_include_tagged_history_once() {
        let history = [
            ChatMessage::new(Role::User, "[#gaming] Kevin: gg"),
            ChatMessage::new(Role::Assistant, "Someone: not a speaker"),
            ChatMessage::new(Role::User, "Garrett: hi"),
            ChatMessage::new(Role::User, "[#general] kevin: again"),
        ];
        assert_eq!(
            speaker_names("Garrett", &People::default(), &history),
            vec!["Garrett", "Kevin"]
        );
    }

    #[test]
    fn truncates_long_quotes_on_char_boundaries() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("héllo world", 5), "héllo…");
        assert_eq!(truncate("ab cd", 3), "ab…");
    }

    #[test]
    fn strips_both_mention_forms() {
        let id = UserId::new(42);
        assert_eq!(strip_mention("<@42> hello <@!42>", id).trim(), "hello");
        assert_eq!(strip_mention("<@43> hello", id), "<@43> hello");
    }

    #[test]
    fn replaces_mentions_with_a_name() {
        let id = UserId::new(42);
        assert_eq!(
            replace_mention("hi <@42> and <@!42>", id, "@Alex"),
            "hi @Alex and @Alex"
        );
    }

    #[test]
    fn short_text_is_one_chunk() {
        assert_eq!(split_message("  hello  ", 10), vec!["hello"]);
        assert!(split_message("   ", 10).is_empty());
    }

    #[test]
    fn splits_at_newline_then_space() {
        assert_eq!(split_message("aaa\nbbb ccc", 8), vec!["aaa", "bbb ccc"]);
        assert_eq!(split_message("aaa bbb ccc", 8), vec!["aaa bbb", "ccc"]);
    }

    #[test]
    fn hard_splits_long_words_on_char_boundaries() {
        let chunks = split_message("ééééé", 2);
        assert_eq!(chunks, vec!["éé", "éé", "é"]);
        assert!(chunks.iter().all(|c| c.chars().count() <= 2));
    }
}
