//! Runtime configuration, read from environment variables (and an optional
//! `.env` file in the working directory).

use std::{env, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    /// Request the privileged Message Content intent, so the bot can read
    /// messages it's asked about in a reply. Must also be enabled for the bot
    /// in the Discord Developer Portal, or connecting fails.
    pub message_content_intent: bool,
    pub greeter: GreeterConfig,
    pub llm: LlmConfig,
    pub chat: ChatConfig,
    pub search: SearchConfig,
}

/// Web search through SearXNG. Off unless `SEARXNG_URL` is set.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Base URL of a SearXNG instance with the JSON format enabled.
    pub url: Option<String>,
    /// Searches the model may run for one message.
    pub max_calls: usize,
    /// Results handed to the model per search.
    pub max_results: usize,
    pub timeout: Duration,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            url: None,
            max_calls: 2,
            max_results: 5,
            timeout: Duration::from_secs(15),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GreeterConfig {
    /// Audio file played to people joining a voice channel.
    pub audio_path: PathBuf,
    /// Upper bound on how long the bot stays in a channel, in case the track
    /// never reports that it finished.
    pub max_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Base URL of a local OpenAI-compatible server, e.g. llama.cpp's `llama-server`.
    pub base_url: String,
    /// Model name sent with each request. `llama-server` serves whichever model it
    /// was started with, so this only matters for servers hosting several models.
    pub model: String,
    pub timeout: Duration,
    /// Longest reply in tokens, or `None` for no limit (`LLM_MAX_TOKENS=0`).
    pub max_tokens: Option<u32>,
    pub temperature: f32,
    /// Requests sent to the model at once. Match `llama-server --parallel`.
    pub max_concurrency: usize,
}

#[derive(Debug, Clone)]
pub struct ChatConfig {
    /// Markdown file describing RiceBot's personality. Read at startup and
    /// again on `!reload`.
    pub personality_file: PathBuf,
    /// Whether `personality_file` was set explicitly, making a missing file an error.
    pub personality_file_required: bool,
    /// TOML file of people the bot knows, keyed by Discord user ID.
    pub people_file: PathBuf,
    /// Whether `people_file` was set explicitly, making a missing file an error.
    pub people_file_required: bool,
    /// Previous messages sent along as context, per channel or per server
    /// depending on `memory_scope`.
    pub history_messages: usize,
    pub memory_scope: MemoryScope,
}

/// Whether chat memory is kept per text channel or shared across a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    Channel,
    Server,
}

impl FromStr for MemoryScope {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "channel" => Ok(Self::Channel),
            "server" => Ok(Self::Server),
            other => Err(format!("expected \"channel\" or \"server\", got {other:?}")),
        }
    }
}

impl Config {
    /// Reads settings from the environment. `main` loads `.env` beforehand.
    pub fn from_env() -> Result<Self> {
        let discord_token = required("DISCORD_TOKEN")?;

        let config = Self {
            discord_token,
            message_content_intent: optional("DISCORD_MESSAGE_CONTENT")?.unwrap_or(false),
            greeter: GreeterConfig {
                audio_path: optional("GREETING_AUDIO")?.unwrap_or_else(|| "welcome.mp3".into()),
                max_duration: Duration::from_secs(optional("GREETING_MAX_SECS")?.unwrap_or(30)),
            },
            llm: LlmConfig {
                base_url: optional::<String>("LLM_URL")?
                    .unwrap_or_else(|| "http://127.0.0.1:8080".into())
                    .trim_end_matches('/')
                    .to_owned(),
                model: optional("LLM_MODEL")?.unwrap_or_else(|| "local".into()),
                timeout: Duration::from_secs(optional("LLM_TIMEOUT_SECS")?.unwrap_or(180)),
                // 0 means no limit: the model writes until it decides it's done.
                max_tokens: match optional("LLM_MAX_TOKENS")?.unwrap_or(512) {
                    0 => None,
                    n => Some(n),
                },
                temperature: optional("LLM_TEMPERATURE")?.unwrap_or(0.7),
                max_concurrency: optional("LLM_MAX_CONCURRENCY")?.unwrap_or(2),
            },
            chat: ChatConfig {
                personality_file_required: optional::<String>("PERSONALITY_FILE")?.is_some(),
                personality_file: optional("PERSONALITY_FILE")?
                    .unwrap_or_else(|| "PERSONALITY.md".into()),
                people_file_required: optional::<String>("PEOPLE_FILE")?.is_some(),
                people_file: optional("PEOPLE_FILE")?.unwrap_or_else(|| "people.toml".into()),
                history_messages: optional("CHAT_HISTORY_MESSAGES")?.unwrap_or(10),
                memory_scope: optional("CHAT_MEMORY_SCOPE")?.unwrap_or(MemoryScope::Channel),
            },
            search: {
                let defaults = SearchConfig::default();
                SearchConfig {
                    url: optional::<String>("SEARXNG_URL")?
                        .map(|url| url.trim_end_matches('/').to_owned()),
                    max_calls: optional("SEARCH_MAX_CALLS")?.unwrap_or(defaults.max_calls),
                    max_results: optional("SEARCH_MAX_RESULTS")?.unwrap_or(defaults.max_results),
                    timeout: optional("SEARCH_TIMEOUT_SECS")?
                        .map(Duration::from_secs)
                        .unwrap_or(defaults.timeout),
                }
            },
        };

        if config.search.max_results == 0 {
            bail!("SEARCH_MAX_RESULTS must be at least 1");
        }
        if config.llm.max_concurrency == 0 {
            bail!("LLM_MAX_CONCURRENCY must be at least 1");
        }

        Ok(config)
    }
}

fn required(key: &str) -> Result<String> {
    let value = env::var(key).with_context(|| format!("{key} must be set"))?;
    let value = value.trim();
    if value.is_empty() {
        bail!("{key} must not be empty");
    }
    Ok(value.to_owned())
}

fn optional<T>(key: &str) -> Result<Option<T>>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .map(Some)
            .map_err(|e| anyhow::anyhow!("invalid value for {key}: {e}")),
        Ok(_) | Err(env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("could not read {key}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_scope() {
        assert_eq!("server".parse(), Ok(MemoryScope::Server));
        assert_eq!("Channel".parse(), Ok(MemoryScope::Channel));
        assert!("guild".parse::<MemoryScope>().is_err());
    }
}
