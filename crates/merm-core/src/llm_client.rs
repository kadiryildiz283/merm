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
    project_root: Option<PathBuf>,
}

impl AgyLlmProvider {
    pub fn new(model: Option<String>) -> Self {
        Self::new_with_root(model, None)
    }

    pub fn new_with_root(model: Option<String>, project_root: Option<PathBuf>) -> Self {
        let bin_path = Self::find_agy_bin().unwrap_or_else(|| PathBuf::from("agy"));
        Self {
            bin_path,
            model,
            project_root,
        }
    }

    pub fn with_bin_path(bin_path: PathBuf, model: Option<String>) -> Self {
        Self {
            bin_path,
            model,
            project_root: None,
        }
    }

    pub fn with_project_root(mut self, root: PathBuf) -> Self {
        self.project_root = Some(root);
        self
    }

    pub fn find_agy_bin() -> Option<PathBuf> {
        if let Ok(env_path) = std::env::var("AGY_BIN_PATH") {
            let p = PathBuf::from(env_path);
            if p.is_file() {
                return Some(p);
            }
        }
        let static_candidates = ["/usr/local/bin/agy", "/usr/bin/agy"];
        for sc in &static_candidates {
            let p = PathBuf::from(sc);
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let candidate = Path::new(&home).join(".local/bin/agy");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if let Ok(path) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path) {
                let candidate = dir.join("agy");
                if candidate.is_file() {
                    return Some(candidate);
                }
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
        let project_root_opt = self.project_root.clone();

        Box::pin(async move {
            let mut cmd = tokio::process::Command::new(&bin);
            if let Some(ref root) = project_root_opt {
                cmd.current_dir(root);
            }
            cmd.arg("-p")
                .arg(&full_prompt)
                .arg("--output-format")
                .arg("text")
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

            if output.status.success() {
                let raw_stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !raw_stdout.is_empty() {
                    return Ok(raw_stdout);
                }
            }

            // Fallback to JSON format if text format produced nothing or exited with error
            let mut json_cmd = tokio::process::Command::new(&bin);
            if let Some(ref root) = project_root_opt {
                json_cmd.current_dir(root);
            }
            json_cmd
                .arg("-p")
                .arg(&full_prompt)
                .arg("--output-format")
                .arg("json")
                .arg("--disable-slash-commands")
                .arg("--dangerously-skip-permissions");

            if let Some(ref m) = model_opt {
                if m != "inherit" && !m.is_empty() {
                    json_cmd.arg("--model").arg(m);
                }
            }

            let json_output = json_cmd.output().await.map_err(|e| {
                CoreError::LayoutFailed(format!(
                    "Failed to execute Antigravity CLI ('{}'): {}",
                    bin.display(),
                    e
                ))
            })?;

            if !json_output.status.success() {
                let stderr = String::from_utf8_lossy(&json_output.stderr);
                return Err(CoreError::LayoutFailed(format!(
                    "Antigravity CLI (agy) exited with code {:?}: {}",
                    json_output.status.code(),
                    stderr.trim()
                )));
            }

            let raw_stdout = String::from_utf8_lossy(&json_output.stdout)
                .trim()
                .to_string();
            if raw_stdout.is_empty() {
                return Err(CoreError::LayoutFailed(
                    "Antigravity CLI (agy) returned empty output".to_string(),
                ));
            }

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

/// Provider that tries a primary provider first, and falls back to a secondary provider upon failure
pub struct FallbackLlmProvider {
    primary: Box<dyn LlmProvider>,
    secondary: Box<dyn LlmProvider>,
    secondary_name: String,
}

impl FallbackLlmProvider {
    pub fn new(
        primary: Box<dyn LlmProvider>,
        secondary: Box<dyn LlmProvider>,
        secondary_name: impl Into<String>,
    ) -> Self {
        Self {
            primary,
            secondary,
            secondary_name: secondary_name.into(),
        }
    }
}

impl LlmProvider for FallbackLlmProvider {
    fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, CoreError>> + Send + '_>>
    {
        let sys = system_prompt.to_string();
        let usr = user_prompt.to_string();
        Box::pin(async move {
            match self.primary.query(&sys, &usr).await {
                Ok(resp) => Ok(resp),
                Err(err) => {
                    log::warn!(
                        "Primary LLM provider failed ({}); falling back to {}...",
                        err,
                        self.secondary_name
                    );
                    self.secondary.query(&sys, &usr).await
                }
            }
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

    pub fn from_manifest(manifest: &crate::manifest::ProjectManifest) -> Self {
        Self::from_settings_with_root(&manifest.settings, Some(manifest.project_root.clone()))
    }

    pub fn from_settings(settings: &ProjectSettings) -> Self {
        Self::from_settings_with_root(settings, None)
    }

    pub fn from_settings_with_root(
        settings: &ProjectSettings,
        project_root: Option<PathBuf>,
    ) -> Self {
        match settings.llm_provider.to_lowercase().as_str() {
            "agy" | "antigravity" => {
                let model = if settings.llm_model.is_empty() || settings.llm_model == "inherit" {
                    None
                } else {
                    Some(settings.llm_model.clone())
                };
                Self {
                    provider: Box::new(AgyLlmProvider::new_with_root(model, project_root)),
                }
            }
            "openai" => {
                let endpoint = if settings.llm_endpoint.is_empty() {
                    "https://api.openai.com/v1".to_string()
                } else {
                    settings.llm_endpoint.clone()
                };
                let primary = Box::new(HttpLlmProvider::with_key(
                    endpoint,
                    settings.llm_model.clone(),
                    settings.llm_api_key.clone(),
                ));
                if AgyLlmProvider::find_agy_bin().is_some() {
                    let agy = Box::new(AgyLlmProvider::new_with_root(None, project_root));
                    Self {
                        provider: Box::new(FallbackLlmProvider::new(
                            primary,
                            agy,
                            "Antigravity CLI (agy)",
                        )),
                    }
                } else {
                    Self { provider: primary }
                }
            }
            "ollama" => {
                if AgyLlmProvider::find_agy_bin().is_some() {
                    // When agy is present on machine, use agy as priority provider for ollama config
                    let agy = Box::new(AgyLlmProvider::new_with_root(None, project_root.clone()));
                    let primary = Box::new(HttpLlmProvider::with_key(
                        settings.llm_endpoint.clone(),
                        settings.llm_model.clone(),
                        settings.llm_api_key.clone(),
                    ));
                    Self {
                        provider: Box::new(FallbackLlmProvider::new(agy, primary, "Ollama")),
                    }
                } else {
                    Self {
                        provider: Box::new(HttpLlmProvider::with_key(
                            settings.llm_endpoint.clone(),
                            settings.llm_model.clone(),
                            settings.llm_api_key.clone(),
                        )),
                    }
                }
            }
            _ => {
                let primary = Box::new(HttpLlmProvider::with_key(
                    settings.llm_endpoint.clone(),
                    settings.llm_model.clone(),
                    settings.llm_api_key.clone(),
                ));
                if AgyLlmProvider::find_agy_bin().is_some() {
                    let agy = Box::new(AgyLlmProvider::new_with_root(None, project_root));
                    Self {
                        provider: Box::new(FallbackLlmProvider::new(
                            primary,
                            agy,
                            "Antigravity CLI (agy)",
                        )),
                    }
                } else {
                    Self { provider: primary }
                }
            }
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

    #[tokio::test]
    async fn test_fallback_llm_provider() {
        let primary_mock = MockLlmProvider::new();
        primary_mock.enqueue_response(Err(CoreError::LayoutFailed(
            "Connection refused".to_string(),
        )));

        let secondary_mock = MockLlmProvider::new();
        secondary_mock.enqueue_response(Ok("Fallback success from secondary".to_string()));

        let fallback = FallbackLlmProvider::new(
            Box::new(primary_mock),
            Box::new(secondary_mock),
            "SecondaryMock",
        );

        let res = fallback.query("sys", "user").await.unwrap();
        assert_eq!(res, "Fallback success from secondary");
    }
}
