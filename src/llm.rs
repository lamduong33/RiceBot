//! Client for a local LLM server exposing the OpenAI-compatible chat API
//! (llama.cpp's `llama-server`, Ollama, etc.).

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config::LlmConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self { role, content: content.into() }
    }
}

#[derive(Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    max_tokens: u32,
    temperature: f32,
}

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

pub struct LlmClient {
    http: reqwest::Client,
    endpoint: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            http,
            endpoint: format!("{}/v1/chat/completions", config.base_url),
            model: config.model.clone(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        })
    }

    /// Sends the conversation to the model and returns its reply.
    pub async fn complete(&self, messages: &[ChatMessage]) -> Result<String> {
        let request = CompletionRequest {
            model: &self.model,
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        };

        let response: CompletionResponse = self
            .http
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .with_context(|| format!("could not reach LLM server at {}", self.endpoint))?
            .error_for_status()
            .context("LLM server returned an error")?
            .json()
            .await
            .context("LLM server sent an unexpected response")?;

        let reply = response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .map(|content| content.trim().to_owned())
            .unwrap_or_default();

        if reply.is_empty() {
            return Err(anyhow!("LLM server returned an empty reply"));
        }
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Run with `cargo test -- --ignored` while `llama-server` is running.
    #[tokio::test]
    #[ignore = "needs a local LLM server on LLM_URL (default http://127.0.0.1:8080)"]
    async fn answers_through_local_server() {
        let config = LlmConfig {
            base_url: std::env::var("LLM_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            model: "local".into(),
            timeout: Duration::from_secs(120),
            max_tokens: 32,
            temperature: 0.0,
            max_concurrency: 1,
        };
        let client = LlmClient::new(&config).unwrap();
        let reply = client
            .complete(&[
                ChatMessage::new(Role::System, "You are RiceBot."),
                ChatMessage::new(Role::User, "tester: Say hello."),
            ])
            .await
            .unwrap();
        assert!(!reply.is_empty());
    }
}
