use crate::error::CoreError;
use crate::manifest::ProjectSettings;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
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
    api_key: Option<String>,
    client: reqwest::Client,
}

impl HttpLlmProvider {
    pub fn new(endpoint: String, model: String) -> Self {
        Self::with_key(endpoint, model, None)
    }

    pub fn with_key(endpoint: String, model: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .unwrap_or_default();

        Self {
            endpoint,
            model,
            api_key,
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
            let mut req = self.client.post(&url).json(&request_payload);
            if let Some(ref key) = self.api_key {
                if !key.trim().is_empty() {
                    req = req.bearer_auth(key.trim());
                }
            }

            let resp = req.send().await.map_err(|e| {
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

/// Native binding to Google Antigravity CLI (`agy`)
pub struct AgyLlmProvider {
    bin_path: PathBuf,
    model: Option<String>,
}

impl AgyLlmProvider {
    pub fn new(model: Option<String>) -> Self {
        let bin_path = Self::find_agy_bin().unwrap_or_else(|| PathBuf::from("agy"));
        Self { bin_path, model }
    }

    pub fn with_bin_path(bin_path: PathBuf, model: Option<String>) -> Self {
        Self { bin_path, model }
    }

    pub fn find_agy_bin() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                let candidate = dir.join("agy");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let candidate = Path::new(&home).join(".local/bin/agy");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }
}

#[derive(Debug, Deserialize)]
struct AgyJsonEnvelope {
    #[serde(default)]
    response: Option<String>,
}

impl LlmProvider for AgyLlmProvider {
    fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>
    {
        let full_prompt = format!(
            "CRITICAL DIRECTIVE: DO NOT call any external tools, run terminal commands, or invoke subagents. Provide your complete, final response directly and immediately as plain text.\n\nInstructions:\n{}\n\nTask:\n{}",
            system_prompt, user_prompt
        );
        let bin = self.bin_path.clone();
        let model_opt = self.model.clone();

        Box::pin(async move {
            let mut cmd = tokio::process::Command::new(&bin);
            cmd.arg("-p")
                .arg(&full_prompt)
                .arg("--output-format")
                .arg("json")
                .arg("--disable-slash-commands")
                .arg("--dangerously-skip-permissions");

            if let Some(ref m) = model_opt {
                if m != "inherit" && !m.is_empty() {
                    cmd.arg("--model").arg(m);
                }
            }

            let output = cmd.output().await.map_err(|e| {
                CoreError::LayoutFailed(format!(
                    "Failed to execute Antigravity CLI ('{}'): {}. Ensure agy is installed in PATH.",
                    bin.display(),
                    e
                ))
            })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CoreError::LayoutFailed(format!(
                    "Antigravity CLI (agy) exited with code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                )));
            }

            let raw_stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if raw_stdout.is_empty() {
                return Err(CoreError::LayoutFailed(
                    "Antigravity CLI (agy) returned empty output".to_string(),
                ));
            }

            // Extract the clean response payload from the agy JSON envelope
            if let Ok(envelope) = serde_json::from_str::<AgyJsonEnvelope>(&raw_stdout) {
                if let Some(resp) = envelope.response {
                    let cleaned = resp.trim().to_string();
                    if !cleaned.is_empty() {
                        return Ok(cleaned);
                    }
                }
            }

            Ok(raw_stdout)
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

    pub fn from_settings(settings: &ProjectSettings) -> Self {
        match settings.llm_provider.to_lowercase().as_str() {
            "agy" | "antigravity" => {
                let model = if settings.llm_model.is_empty() || settings.llm_model == "inherit" {
                    None
                } else {
                    Some(settings.llm_model.clone())
                };
                Self {
                    provider: Box::new(AgyLlmProvider::new(model)),
                }
            }
            "openai" => {
                let endpoint = if settings.llm_endpoint.is_empty() {
                    "https://api.openai.com/v1".to_string()
                } else {
                    settings.llm_endpoint.clone()
                };
                Self {
                    provider: Box::new(HttpLlmProvider::with_key(
                        endpoint,
                        settings.llm_model.clone(),
                        settings.llm_api_key.clone(),
                    )),
                }
            }
            _ => Self {
                provider: Box::new(HttpLlmProvider::with_key(
                    settings.llm_endpoint.clone(),
                    settings.llm_model.clone(),
                    settings.llm_api_key.clone(),
                )),
            },
        }
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
