use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSnapshot {
    pub id: String,
    pub timestamp: u64,
    pub project_root: PathBuf,
    pub original_files: HashMap<String, String>,
    pub diagram_source: String,
}

impl TransactionSnapshot {
    pub fn create(
        project_root: &Path,
        files: &[String],
        diagram_source: &str,
    ) -> Result<Self, CoreError> {
        let id = Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut original_files = HashMap::new();
        for rel_path in files {
            let full_path = project_root.join(rel_path);
            if full_path.is_file() {
                if let Ok(content) = fs::read_to_string(&full_path) {
                    original_files.insert(rel_path.clone(), content);
                }
            }
        }

        let snapshot = Self {
            id: id.clone(),
            timestamp,
            project_root: project_root.to_path_buf(),
            original_files,
            diagram_source: diagram_source.to_string(),
        };

        // Persist snapshot to disk in .merm/snapshots/<id>/
        let snap_dir = project_root.join(".merm").join("snapshots").join(&id);
        fs::create_dir_all(&snap_dir).map_err(|e| {
            CoreError::LayoutFailed(format!("Failed to create snapshot directory: {}", e))
        })?;

        let meta_json = serde_json::to_string_pretty(&snapshot).map_err(|e| {
            CoreError::LayoutFailed(format!("Failed to serialize snapshot metadata: {}", e))
        })?;
        fs::write(snap_dir.join("metadata.json"), meta_json).map_err(|e| {
            CoreError::LayoutFailed(format!("Failed to write snapshot metadata: {}", e))
        })?;

        Ok(snapshot)
    }

    pub fn rollback(&self) -> Result<(), CoreError> {
        log::warn!("Rolling back transaction snapshot {}", self.id);

        for (rel_path, content) in &self.original_files {
            let full_path = self.project_root.join(rel_path);
            if let Some(parent) = full_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&full_path, content).map_err(|e| {
                CoreError::LayoutFailed(format!(
                    "Failed to restore file {} during rollback: {}",
                    rel_path, e
                ))
            })?;
        }

        Ok(())
    }

    pub fn verify_or_rollback(&self) -> Result<(), CoreError> {
        if !self.project_root.join("Cargo.toml").is_file() {
            return Ok(());
        }

        let mut cmd = StdCommand::new("cargo");
        cmd.current_dir(&self.project_root).arg("check");

        let out = cmd.output().map_err(|e| {
            let _ = self.rollback();
            CoreError::LayoutFailed(format!("Failed to execute cargo check verification: {}", e))
        })?;

        if !out.status.success() {
            let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
            let _ = self.rollback();
            return Err(CoreError::LayoutFailed(format!(
                "Cargo check verification failed. Automatically rolled back all changes.\nErrors:\n{}",
                err_msg
            )));
        }

        Ok(())
    }
}
