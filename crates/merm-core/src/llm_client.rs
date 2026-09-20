use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

pub struct LlmClient {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(endpoint: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            endpoint,
            model,
            client,
        }
    }

    pub async fn query(&self, system_prompt: &str, user_prompt: &str) -> Result<String, CoreError> {
        let url = if self.endpoint.ends_with("/chat/completions") {
            self.endpoint.clone()
        } else {
            format!("{}/chat/completions", self.endpoint.trim_end_matches('/'))
        };

        let request_payload = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            temperature: Some(0.2),
        };

        let resp = self
            .client
            .post(&url)
            .json(&request_payload)
            .send()
            .await
            .map_err(|e| {
                CoreError::LayoutFailed(format!(
                    "LLM HTTP request failed (endpoint: {}): {}",
                    url, e
                ))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::LayoutFailed(format!(
                "LLM server returned error status {}: {}",
                status, body
            )));
        }

        let completion: ChatCompletionResponse = resp.json().await.map_err(|e| {
            CoreError::LayoutFailed(format!("Failed to parse LLM completion response: {}", e))
        })?;

        let first = completion.choices.into_iter().next().ok_or_else(|| {
            CoreError::LayoutFailed("LLM returned empty choices in completion".to_string())
        })?;

        Ok(first.message.content)
    }
}
