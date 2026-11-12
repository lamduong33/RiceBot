//! Answers messages that mention the bot, using the local LLM.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, PoisonError, RwLock},
};

use anyhow::Result;
use serenity::{
    all::{ChannelId, CreateAllowedMentions, CreateMessage, Message, UserId},
    client::Context,
    utils::{ContentSafeOptions, content_safe},
};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::{
    config::{ChatConfig, LlmConfig},
    llm::{ChatMessage, LlmClient, Role},
    people::People,
    personality::{self, Source},
};

/// Discord's limit on the length of a single message, in characters.
const MESSAGE_LIMIT: usize = 2000;

const EMPTY_PROMPT_REPLY: &str = "Hi! Mention me together with a question and I'll do my best to answer.";
/// Sent as `@RiceBot !reload`. The bot doesn't request the message content
/// intent, so commands have to mention it like any other message.
const RELOAD_COMMAND: &str = "!reload";
const LLM_ERROR_REPLY: &str = "Sorry, I can't think right now: my language model isn't responding. Try again in a bit.";

pub struct Chat {
    llm: LlmClient,
    /// Limits how many requests are sent to the model at once.
    permits: Semaphore,
    /// Where the personality and people files are, for `!reload`.
    files: ChatConfig,
    /// Personality and people, replaced together by `!reload`. Each message
    /// takes its own reference, so a reload never changes a reply halfway.
    knowledge: RwLock<Arc<Knowledge>>,
    history_len: usize,
    /// Recent conversation per channel, oldest first.
    history: Mutex<HashMap<ChannelId, VecDeque<ChatMessage>>>,
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
    pub fn new(chat: &ChatConfig, llm: &LlmConfig) -> Result<Self> {
        Ok(Self {
            llm: LlmClient::new(llm)?,
            permits: Semaphore::new(llm.max_concurrency),
            files: chat.clone(),
            knowledge: RwLock::new(Arc::new(load_knowledge(chat)?)),
            history_len: chat.history_messages,
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

        let author = match knowledge.people.get(msg.author.id) {
            Some(person) => person.name.clone(),
            None => msg
                .member
                .as_ref()
                .and_then(|member| member.nick.clone())
                .unwrap_or_else(|| msg.author.display_name().to_owned()),
        };
        info!(user = %msg.author.id, channel = %msg.channel_id, "answering mention");

        let question = ChatMessage::new(Role::User, format!("{author}: {prompt}"));
        let mut conversation = vec![ChatMessage::new(Role::System, knowledge.system_prompt.as_str())];
        conversation.extend(self.recent_history(msg.channel_id));
        conversation.push(question.clone());

        let typing = msg.channel_id.start_typing(&ctx.http);
        let result = match self.permits.acquire().await {
            Ok(_permit) => self.llm.complete(&conversation).await,
            Err(_) => return, // The semaphore is never closed.
        };
        typing.stop();

        match result {
            Ok(answer) => {
                send_reply(ctx, msg, &answer).await;
                self.remember(msg.channel_id, question, ChatMessage::new(Role::Assistant, answer));
            }
            Err(e) => {
                error!("LLM request failed: {e:#}");
                send_reply(ctx, msg, LLM_ERROR_REPLY).await;
            }
        }
    }

    /// Re-reads the personality and people files and clears chat memory. Only
    /// the bot's owner (or members of the team that owns it) may do this.
    async fn reload(&self, ctx: &Context, msg: &Message) {
        if !is_bot_owner(ctx, msg.author.id).await {
            info!(user = %msg.author.id, "ignored !reload from someone who doesn't own the bot");
            send_reply(ctx, msg, "Only my owner can reload my personality.").await;
            return;
        }

        let reply = match load_knowledge(&self.files) {
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

    fn recent_history(&self, channel_id: ChannelId) -> Vec<ChatMessage> {
        self.history
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&channel_id)
            .map(|messages| messages.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn remember(&self, channel_id: ChannelId, question: ChatMessage, answer: ChatMessage) {
        if self.history_len == 0 {
            return;
        }
        let mut history = self.history.lock().unwrap_or_else(PoisonError::into_inner);
        let messages = history.entry(channel_id).or_default();
        messages.extend([question, answer]);
        while messages.len() > self.history_len {
            messages.pop_front();
        }
    }
}

/// Reads the personality and people files, logging what was loaded.
fn load_knowledge(files: &ChatConfig) -> Result<Knowledge> {
    let (mut system_prompt, personality_source) =
        personality::load(&files.personality_file, files.personality_file_required)?;
    let people = People::load(&files.people_file, files.people_file_required)?;
    // Everyone's notes go in the single system message, because some chat
    // templates reject more than one.
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
    let mut without_bot = strip_mention(&msg.content, bot_id);
    for user in &msg.mentions {
        if let Some(person) = people.get(user.id) {
            without_bot = replace_mention(&without_bot, user.id, &format!("@{}", person.name));
        }
    }
    let mut options = ContentSafeOptions::default();
    if let Some(guild_id) = msg.guild_id {
        options = options.display_as_member_from(guild_id);
    }
    content_safe(&ctx.cache, without_bot, &options, &msg.mentions)
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
