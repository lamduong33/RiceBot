//! Answers messages that mention the bot, using the local LLM.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, PoisonError},
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
};

/// Discord's limit on the length of a single message, in characters.
const MESSAGE_LIMIT: usize = 2000;

const EMPTY_PROMPT_REPLY: &str = "Hi! Mention me together with a question and I'll do my best to answer.";
const LLM_ERROR_REPLY: &str = "Sorry, I can't think right now: my language model isn't responding. Try again in a bit.";

pub struct Chat {
    llm: LlmClient,
    /// Limits how many requests are sent to the model at once.
    permits: Semaphore,
    system_prompt: String,
    people: People,
    history_len: usize,
    /// Recent conversation per channel, oldest first.
    history: Mutex<HashMap<ChannelId, VecDeque<ChatMessage>>>,
}

impl Chat {
    pub fn new(chat: &ChatConfig, llm: &LlmConfig) -> Result<Self> {
        let people = People::load(&chat.people_file, chat.people_file_required)?;
        info!(count = people.len(), file = %chat.people_file.display(), "loaded people");
        Ok(Self {
            llm: LlmClient::new(llm)?,
            permits: Semaphore::new(llm.max_concurrency),
            system_prompt: chat.system_prompt.clone(),
            people,
            history_len: chat.history_messages,
            history: Mutex::default(),
        })
    }

    pub async fn on_message(&self, ctx: &Context, msg: &Message) {
        let bot_id = ctx.cache.current_user().id;
        if msg.author.bot || !msg.mentions_user_id(bot_id) {
            return;
        }

        let prompt = clean_prompt(ctx, msg, bot_id, &self.people);
        if prompt.is_empty() {
            send_reply(ctx, msg, EMPTY_PROMPT_REPLY).await;
            return;
        }

        let author = match self.people.get(msg.author.id) {
            Some(person) => person.name.clone(),
            None => msg
                .member
                .as_ref()
                .and_then(|member| member.nick.clone())
                .unwrap_or_else(|| msg.author.display_name().to_owned()),
        };
        info!(user = %msg.author.id, channel = %msg.channel_id, "answering mention");

        let question = ChatMessage::new(Role::User, format!("{author}: {prompt}"));
        let mut conversation = vec![ChatMessage::new(
            Role::System,
            self.system_prompt_for(msg, bot_id),
        )];
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

    /// The system prompt plus notes on the author and anyone they mentioned.
    /// Notes go in the single system message because some chat templates
    /// reject more than one.
    fn system_prompt_for(&self, msg: &Message, bot_id: UserId) -> String {
        let mentioned = msg
            .mentions
            .iter()
            .map(|user| user.id)
            .filter(|&id| id != bot_id);
        match self
            .people
            .describe(std::iter::once(msg.author.id).chain(mentioned))
        {
            Some(notes) => format!("{}\n\n{notes}", self.system_prompt),
            None => self.system_prompt.clone(),
        }
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
