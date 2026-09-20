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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSettings {
    pub llm_provider: String,
    pub llm_endpoint: String,
    pub llm_model: String,
    pub build_command: String,
    pub test_command: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            llm_provider: "ollama".to_string(),
            llm_endpoint: std::env::var("MERM_LLM_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
            llm_model: std::env::var("MERM_LLM_MODEL")
                .unwrap_or_else(|_| "qwen2.5-coder".to_string()),
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
        Self {
            version: 1,
            project_name: name,
            project_root: root,
            entry_diagram: None,
            bindings: HashMap::new(),
            settings: ProjectSettings::default(),
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

    pub fn load_or_init(root: &Path) -> Result<Self, CoreError> {
        let merm_dir = root.join(MERM_DIR);
        if !merm_dir.exists() {
            fs::create_dir_all(&merm_dir).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to create .merm directory: {}", e))
            })?;
        }

        let m_path = Self::manifest_path(root);
        if m_path.is_file() {
            let content = fs::read_to_string(&m_path).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to read .merm/manifest.json: {}", e))
            })?;
            let manifest: ProjectManifest = serde_json::from_str(&content).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to parse .merm/manifest.json: {}", e))
            })?;
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

    pub fn add_binding(&mut self, binding: NodeBinding) {
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
