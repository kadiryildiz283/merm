use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MERM_DIR: &str = ".merm";
pub const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeBinding {
    pub id: String,
    pub file: String,
    pub symbol: String,
    pub kind: String,
    pub executable: bool,
    pub entrypoint: Option<String>,
    pub input_type: Option<String>,
    pub output_type: Option<String>,
}

pub fn which_agy_available() -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join("agy").is_file() {
                return true;
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if Path::new(&home).join(".local/bin/agy").is_file() {
            return true;
        }
    }
    false
}

pub fn read_env_val(root: &Path, key: &str) -> Option<String> {
    if let Ok(val) = std::env::var(key) {
        if !val.trim().is_empty() {
            return Some(val);
        }
    }
    let env_file = root.join(".env");
    if let Ok(content) = fs::read_to_string(env_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || !line.contains('=') {
                continue;
            }
            let mut parts = line.splitn(2, '=');
            let k = parts.next()?.trim();
            let v = parts.next()?.trim().trim_matches('"').trim_matches('\'');
            if k == key && !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSettings {
    pub llm_provider: String,
    pub llm_endpoint: String,
    pub llm_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_api_key: Option<String>,
    pub build_command: String,
    pub test_command: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self::for_root(Path::new("."))
    }
}

impl ProjectSettings {
    pub fn for_root(root: &Path) -> Self {
        let api_key =
            read_env_val(root, "OPENAI_API_KEY").or_else(|| read_env_val(root, "MERM_LLM_API_KEY"));

        let provider_env = read_env_val(root, "MERM_LLM_PROVIDER");

        let default_provider = if let Some(p) = provider_env {
            p
        } else if which_agy_available() {
            "agy".to_string()
        } else if api_key.is_some() {
            "openai".to_string()
        } else {
            "ollama".to_string()
        };

        let default_endpoint = if default_provider == "openai" {
            "https://api.openai.com/v1".to_string()
        } else {
            read_env_val(root, "MERM_LLM_ENDPOINT")
                .unwrap_or_else(|| "http://localhost:11434/v1".to_string())
        };

        let default_model = if default_provider == "openai" {
            read_env_val(root, "OPENAI_MODEL").unwrap_or_else(|| "gpt-4o-mini".to_string())
        } else if default_provider == "agy" {
            read_env_val(root, "AGY_MODEL").unwrap_or_else(|| "inherit".to_string())
        } else {
            read_env_val(root, "MERM_LLM_MODEL").unwrap_or_else(|| "qwen2.5-coder".to_string())
        };

        Self {
            llm_provider: default_provider,
            llm_endpoint: default_endpoint,
            llm_model: default_model,
            llm_api_key: api_key,
            build_command: "cargo check".to_string(),
            test_command: "cargo test".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    pub version: u32,
    pub project_name: String,
    pub project_root: PathBuf,
    pub entry_diagram: Option<PathBuf>,
    pub bindings: HashMap<String, NodeBinding>,
    pub settings: ProjectSettings,
}

impl ProjectManifest {
    pub fn new(root: PathBuf, name: String) -> Self {
        let settings = ProjectSettings::for_root(&root);
        Self {
            version: 1,
            project_name: name,
            project_root: root,
            entry_diagram: None,
            bindings: HashMap::new(),
            settings,
        }
    }

    pub fn manifest_path(root: &Path) -> PathBuf {
        root.join(MERM_DIR).join(MANIFEST_FILE)
    }

    pub fn detect_cargo_root(start: &Path) -> Option<PathBuf> {
        let mut curr = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };

        loop {
            if curr.join("Cargo.toml").is_file() {
                return Some(curr);
            }
            if !curr.pop() {
                break;
            }
        }
        None
    }

    pub fn detect_project_root(start: &Path) -> Option<PathBuf> {
        let curr = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };

        let abs_curr = curr.canonicalize().unwrap_or(curr);
        let home_dir = std::env::var("HOME")
            .ok()
            .and_then(|h| PathBuf::from(h).canonicalize().ok());

        let mut check_curr = abs_curr;
        loop {
            // Never treat $HOME, /home, / or /tmp as a project root!
            if let Some(ref home) = home_dir {
                if &check_curr == home {
                    break;
                }
            }
            if check_curr.parent().is_none()
                || check_curr == Path::new("/home")
                || check_curr == Path::new("/tmp")
            {
                break;
            }

            if check_curr.join("Cargo.toml").is_file()
                || check_curr.join(".git").is_dir()
                || check_curr.join("pyproject.toml").is_file()
                || check_curr.join("package.json").is_file()
                || check_curr.join("requirements.txt").is_file()
            {
                return Some(check_curr);
            }

            if !check_curr.pop() {
                break;
            }
        }
        None
    }

    pub fn load_or_init(root: &Path) -> Result<Self, CoreError> {
        let merm_dir = root.join(MERM_DIR);
        if !merm_dir.exists() {
            fs::create_dir_all(&merm_dir).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to create .merm directory: {}", e))
            })?;
        }

        let system_md_path = merm_dir.join("system.md");
        if !system_md_path.exists() {
            let _ = fs::write(&system_md_path, crate::fabric_prompts::SYSTEM_MD_TEMPLATE);
        }

        let m_path = Self::manifest_path(root);
        if m_path.is_file() {
            let content = fs::read_to_string(&m_path).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to read .merm/manifest.json: {}", e))
            })?;
            let mut manifest: ProjectManifest = serde_json::from_str(&content).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to parse .merm/manifest.json: {}", e))
            })?;
            if manifest.settings.llm_api_key.is_none() {
                manifest.settings.llm_api_key = read_env_val(root, "OPENAI_API_KEY")
                    .or_else(|| read_env_val(root, "MERM_LLM_API_KEY"));
            }
            if manifest.settings.llm_provider == "ollama"
                && read_env_val(root, "MERM_LLM_PROVIDER").is_none()
                && which_agy_available()
            {
                manifest.settings.llm_provider = "agy".to_string();
                manifest.settings.llm_model = "inherit".to_string();
            }
            // Ensure all bindings have default input "{}" and default output "1"
            for binding in manifest.bindings.values_mut() {
                if binding
                    .input_type
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
                {
                    binding.input_type = Some("{}".to_string());
                }
                if binding
                    .output_type
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
                {
                    binding.output_type = Some("1".to_string());
                }
            }
            Ok(manifest)
        } else {
            let name = root
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed-project".to_string());

            let manifest = Self::new(root.to_path_buf(), name);
            manifest.save()?;
            Ok(manifest)
        }
    }

    pub fn save(&self) -> Result<(), CoreError> {
        let merm_dir = self.project_root.join(MERM_DIR);
        if !merm_dir.exists() {
            fs::create_dir_all(&merm_dir).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to create .merm directory: {}", e))
            })?;
        }

        let m_path = Self::manifest_path(&self.project_root);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| CoreError::LayoutFailed(format!("Failed to serialize manifest: {}", e)))?;
        fs::write(&m_path, content).map_err(|e| {
            CoreError::LayoutFailed(format!("Failed to write .merm/manifest.json: {}", e))
        })?;
        Ok(())
    }

    pub fn add_binding(&mut self, mut binding: NodeBinding) {
        if binding
            .input_type
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            binding.input_type = Some("{}".to_string());
        }
        if binding
            .output_type
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            binding.output_type = Some("1".to_string());
        }
        self.bindings.insert(binding.id.clone(), binding);
    }

    pub fn remove_binding(&mut self, node_id: &str) -> Option<NodeBinding> {
        self.bindings.remove(node_id)
    }

    pub fn get_binding(&self, node_id: &str) -> Option<&NodeBinding> {
        self.bindings.get(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_manifest_lifecycle() {
        let temp = temp_dir().join(format!("merm_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();

        let mut manifest = ProjectManifest::load_or_init(&temp).unwrap();
        assert_eq!(manifest.version, 1);

        manifest.add_binding(NodeBinding {
            id: "AuthService".to_string(),
            file: "src/auth.rs".to_string(),
            symbol: "AuthService".to_string(),
            kind: "struct".to_string(),
            executable: true,
            entrypoint: Some("run".to_string()),
            input_type: Some("String".to_string()),
            output_type: Some("String".to_string()),
        });

        manifest.save().unwrap();

        let loaded = ProjectManifest::load_or_init(&temp).unwrap();
        assert_eq!(loaded.bindings.len(), 1);
        let binding = loaded.get_binding("AuthService").unwrap();
        assert_eq!(binding.file, "src/auth.rs");
        assert!(binding.executable);

        // cleanup
        let _ = fs::remove_dir_all(&temp);
    }
}
