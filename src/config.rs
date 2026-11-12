//! Runtime configuration, read from environment variables (and an optional
//! `.env` file in the working directory).

use std::{env, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub greeter: GreeterConfig,
    pub llm: LlmConfig,
    pub chat: ChatConfig,
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
    pub max_tokens: u32,
    pub temperature: f32,
    /// Requests sent to the model at once. Match `llama-server --parallel`.
    pub max_concurrency: usize,
}

#[derive(Debug, Clone)]
pub struct ChatConfig {
    /// Markdown file describing RiceBot's personality and knowledge. Read at
    /// startup and again on `!reload`.
    pub personality_path: PathBuf,
    /// Previous messages per channel sent along as context.
    pub history_messages: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // A missing .env file is fine; everything can come from the environment.
        let _ = dotenvy::dotenv();

        let discord_token = required("DISCORD_TOKEN")?;

        let config = Self {
            discord_token,
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
                max_tokens: optional("LLM_MAX_TOKENS")?.unwrap_or(512),
                temperature: optional("LLM_TEMPERATURE")?.unwrap_or(0.7),
                max_concurrency: optional("LLM_MAX_CONCURRENCY")?.unwrap_or(2),
            },
            chat: ChatConfig {
                personality_path: optional("PERSONALITY_FILE")?
                    .unwrap_or_else(|| "PERSONALITY.md".into()),
                history_messages: optional("CHAT_HISTORY_MESSAGES")?.unwrap_or(10),
            },
        };

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
