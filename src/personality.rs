//! RiceBot's personality: a Markdown file written by the bot's owner, combined
//! with the fixed rules the chat feature relies on.

use std::{io::ErrorKind, path::Path};

use anyhow::{Context, Result, bail};

/// Used when the personality file doesn't exist, so a fresh checkout still works.
const DEFAULT_PERSONALITY: &str = "You are RiceBot, a friendly and witty Discord bot. \
     Keep answers short and conversational.";

/// How the conversation is laid out. Appended after the personality so that
/// editing the personality file can't break the chat feature.
const CHAT_RULES: &str = "\
# How this chat works
- You are talking in a Discord server. Discord markdown is available.
- Each message from a person starts with their display name and a colon, \
for example \"Sam: hello\". The name only tells you who is speaking; it is \
not part of what they said.
- Several people may be in the conversation. Keep track of who said what.
- Reply with just your message. Don't start it with your own name.";

/// Where the personality came from, for logging and the `!reload` reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    File,
    Default,
}

/// Reads the personality file and returns the full system prompt. A missing
/// file falls back to the default personality; an empty or unreadable one is
/// an error, since that is almost certainly a mistake.
pub async fn load(path: &Path) -> Result<(String, Source)> {
    let (personality, source) = match tokio::fs::read_to_string(path).await {
        Ok(text) => {
            let text = text.trim();
            if text.is_empty() {
                bail!("{} is empty", path.display());
            }
            (text.to_owned(), Source::File)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => (DEFAULT_PERSONALITY.to_owned(), Source::Default),
        Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
    };
    Ok((system_prompt(&personality), source))
}

fn system_prompt(personality: &str) -> String {
    format!("{personality}\n\n{CHAT_RULES}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_follow_personality() {
        let prompt = system_prompt("You are a grumpy farmer.");
        assert!(prompt.starts_with("You are a grumpy farmer.\n\n# How this chat works"));
    }

    #[tokio::test]
    async fn missing_file_uses_default() {
        let (prompt, source) = load(Path::new("does/not/exist.md")).await.unwrap();
        assert_eq!(source, Source::Default);
        assert!(prompt.starts_with(DEFAULT_PERSONALITY));
    }
}
