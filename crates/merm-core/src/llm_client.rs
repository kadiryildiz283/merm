use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
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

/// Abstract LLM provider interface for real or mock test execution
pub trait LlmProvider: Send + Sync {
    fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>;
}

pub struct HttpLlmProvider {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl HttpLlmProvider {
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
}

impl LlmProvider for HttpLlmProvider {
    fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>
    {
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

        Box::pin(async move {
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
        })
    }
}

/// Deterministic mock LLM provider for CI test suites without live model requirements
pub struct MockLlmProvider {
    responses: Mutex<VecDeque<Result<String, CoreError>>>,
}

impl MockLlmProvider {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
        }
    }

    pub fn with_responses(responses: Vec<Result<String, CoreError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }

    pub fn enqueue_response(&self, response: Result<String, CoreError>) {
        self.responses.lock().unwrap().push_back(response);
    }
}

impl Default for MockLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for MockLlmProvider {
    fn query(
        &self,
        _system_prompt: &str,
        _user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>
    {
        let next = self.responses.lock().unwrap().pop_front();
        Box::pin(async move {
            next.unwrap_or_else(|| {
                Ok("Mock LLM Response: Architectural analysis completed successfully.".to_string())
            })
        })
    }
}

/// High-level client wrapping any LlmProvider
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
}

impl LlmClient {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            provider: Box::new(HttpLlmProvider::new(endpoint, model)),
        }
    }

    pub fn from_provider(provider: Box<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn query(&self, system_prompt: &str, user_prompt: &str) -> Result<String, CoreError> {
        self.provider.query(system_prompt, user_prompt).await
    }
}

impl LlmProvider for LlmClient {
    fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>
    {
        self.provider.query(system_prompt, user_prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_llm_provider() {
        let mock = MockLlmProvider::new();
        mock.enqueue_response(Ok("Deterministic answer".to_string()));

        let client = LlmClient::from_provider(Box::new(mock));
        let res = client.query("system", "user").await.unwrap();
        assert_eq!(res, "Deterministic answer");
    }
}
