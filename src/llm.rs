//! Client for a local LLM server exposing the OpenAI-compatible chat API
//! (llama.cpp's `llama-server`, Ollama, etc.), including tool calls.

use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::config::LlmConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    /// The result of a tool call, answering an assistant message's `tool_calls`.
    Tool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// Tools the assistant asked to run (assistant messages only).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Which tool call this message answers (tool messages only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// The assistant's turn when it asked for tools instead of replying.
    pub fn tool_request(content: String, calls: Vec<ToolCall>) -> Self {
        Self {
            tool_calls: calls,
            ..Self::new(Role::Assistant, content)
        }
    }

    /// The output of the tool call with id `call_id`.
    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(call_id.into()),
            ..Self::new(Role::Tool, content)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type", default = "function_type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded arguments. Some servers send an object instead of a
    /// string; it's re-encoded so both look the same from here on.
    #[serde(default, deserialize_with = "arguments_as_string")]
    pub arguments: String,
}

fn function_type() -> String {
    "function".into()
}

fn arguments_as_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(text) => text,
        Value::Null => String::new(),
        other => other.to_string(),
    })
}

/// What the model did with a request.
#[derive(Debug)]
pub enum Completion {
    /// A normal reply.
    Reply(String),
    /// The model wants tools run first. `content` is any text it wrote
    /// alongside the calls, kept so the conversation can be replayed as is.
    ToolCalls { content: String, calls: Vec<ToolCall> },
}

#[derive(Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    /// Left out when unlimited, so the server generates until the model stops.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    temperature: f32,
    /// Tool definitions in the OpenAI format. Left out when there are none,
    /// so servers without tool support are unaffected.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [Value]>,
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
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

pub struct LlmClient {
    http: reqwest::Client,
    endpoint: String,
    model: String,
    max_tokens: Option<u32>,
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
        match self.chat(messages, None).await? {
            Completion::Reply(reply) => Ok(reply),
            Completion::ToolCalls { .. } => bail!("LLM asked for a tool although none were offered"),
        }
    }

    /// Sends the conversation, offering `tools` if given, and returns either
    /// the model's reply or the tools it wants run.
    pub async fn chat(&self, messages: &[ChatMessage], tools: Option<&[Value]>) -> Result<Completion> {
        let request = CompletionRequest {
            model: &self.model,
            messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            tools,
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

        let message = response
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message)
            .ok_or_else(|| anyhow!("LLM server returned no choices"))?;
        parse_message(message)
    }
}

fn parse_message(message: ResponseMessage) -> Result<Completion> {
    let content = message.content.unwrap_or_default().trim().to_owned();
    let mut calls = message.tool_calls.unwrap_or_default();
    if !calls.is_empty() {
        for call in &mut calls {
            if call.id.is_empty() {
                call.id = new_call_id();
            }
        }
        return Ok(Completion::ToolCalls { content, calls });
    }
    if content.is_empty() {
        bail!("LLM server returned an empty reply");
    }
    Ok(Completion::Reply(content))
}

/// An id for a tool call the server didn't give one. Nine alphanumeric
/// characters, because Mistral chat templates reject anything else.
fn new_call_id() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!("call{:05}", NEXT.fetch_add(1, Ordering::Relaxed) % 100_000)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    fn request(max_tokens: Option<u32>, tools: Option<&[Value]>) -> Value {
        serde_json::to_value(CompletionRequest {
            model: "local",
            messages: &[],
            max_tokens,
            temperature: 0.7,
            tools,
        })
        .unwrap()
    }

    #[test]
    fn unlimited_reply_omits_max_tokens() {
        assert!(request(None, None).get("max_tokens").is_none());
        assert_eq!(request(Some(200), None)["max_tokens"], 200);
    }

    #[test]
    fn tools_are_only_sent_when_offered() {
        assert!(request(None, None).get("tools").is_none());
        let tools = [json!({"type": "function"})];
        assert_eq!(request(None, Some(&tools))["tools"][0]["type"], "function");
    }

    #[test]
    fn plain_messages_serialize_without_tool_fields() {
        let message = serde_json::to_value(ChatMessage::new(Role::User, "hi")).unwrap();
        assert_eq!(message, json!({"role": "user", "content": "hi"}));
    }

    #[test]
    fn tool_messages_serialize_in_openai_format() {
        let call = ToolCall {
            id: "call00001".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "web_search".into(),
                arguments: r#"{"query":"rice"}"#.into(),
            },
        };
        let request = serde_json::to_value(ChatMessage::tool_request(String::new(), vec![call])).unwrap();
        assert_eq!(request["role"], "assistant");
        assert_eq!(request["tool_calls"][0]["function"]["name"], "web_search");
        assert_eq!(request["tool_calls"][0]["type"], "function");

        let result = serde_json::to_value(ChatMessage::tool_result("call00001", "1. Rice")).unwrap();
        assert_eq!(
            result,
            json!({"role": "tool", "content": "1. Rice", "tool_call_id": "call00001"})
        );
    }

    #[test]
    fn parses_tool_calls_with_string_or_object_arguments() {
        let response: CompletionResponse = serde_json::from_value(json!({
            "choices": [{"message": {"content": null, "tool_calls": [
                {"id": "", "type": "function", "function": {"name": "web_search", "arguments": "{\"query\":\"a\"}"}},
                {"function": {"name": "web_search", "arguments": {"query": "b"}}}
            ]}}]
        }))
        .unwrap();
        let message = response.choices.into_iter().next().unwrap().message;
        let Completion::ToolCalls { content, calls } = parse_message(message).unwrap() else {
            panic!("expected tool calls");
        };
        assert!(content.is_empty());
        assert_eq!(calls[0].function.arguments, r#"{"query":"a"}"#);
        assert_eq!(calls[1].function.arguments, r#"{"query":"b"}"#);
        assert!(calls.iter().all(|call| call.id.len() == 9 && call.kind == "function"));
    }

    #[test]
    fn parses_plain_replies_and_rejects_empty_ones() {
        let reply = ResponseMessage { content: Some("  hi  ".into()), tool_calls: Some(vec![]) };
        assert!(matches!(parse_message(reply).unwrap(), Completion::Reply(text) if text == "hi"));
        let empty = ResponseMessage { content: Some(" ".into()), tool_calls: None };
        assert!(parse_message(empty).is_err());
    }

    /// Run with `cargo test -- --ignored` while `llama-server` is running.
    #[tokio::test]
    #[ignore = "needs a local LLM server on LLM_URL (default http://127.0.0.1:8080)"]
    async fn answers_through_local_server() {
        let config = LlmConfig {
            base_url: std::env::var("LLM_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            model: "local".into(),
            timeout: Duration::from_secs(120),
            max_tokens: Some(32),
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
