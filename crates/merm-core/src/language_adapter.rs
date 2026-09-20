use crate::command::NodeKind;
use crate::error::CoreError;
use crate::manifest::ProjectManifest;
use crate::node_runner::{ExecutionResult, NodeRunner};
use crate::rust_scanner::{ProjectScanReport, RustScanner};
use crate::scaffolding::Scaffolder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command as StdCommand;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildStatus {
    pub success: bool,
    pub message: String,
    pub error_count: usize,
    pub warning_count: usize,
}

/// Language abstraction allowing merm to support multiple languages seamlessly
pub trait LanguageAdapter: Send + Sync {
    fn language_name(&self) -> &'static str;

    fn discover_symbols(&self, project_root: &Path) -> Result<ProjectScanReport, CoreError>;

    fn verify_build(&self, project_root: &Path) -> Result<BuildStatus, CoreError>;

    fn execute_node(
        &self,
        project_root: &Path,
        node_id: &str,
        file_path: &str,
        entrypoint: Option<&str>,
        input: &str,
    ) -> Result<ExecutionResult, CoreError>;

    fn scaffold_node(
        &self,
        manifest: &mut ProjectManifest,
        kind: &NodeKind,
        name: &str,
    ) -> Result<String, CoreError>;
}

/// Official Rust language adapter powered by syn AST inspection and cargo
pub struct RustAdapter;

impl RustAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAdapter for RustAdapter {
    fn language_name(&self) -> &'static str {
        "rust"
    }

    fn discover_symbols(&self, project_root: &Path) -> Result<ProjectScanReport, CoreError> {
        RustScanner::scan_project(project_root)
    }

    fn verify_build(&self, project_root: &Path) -> Result<BuildStatus, CoreError> {
        let out = StdCommand::new("cargo")
            .current_dir(project_root)
            .arg("check")
            .output()
            .map_err(|e| CoreError::LayoutFailed(format!("Failed to run cargo check: {}", e)))?;

        let success = out.status.success();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let error_count = stderr.matches("error[").count();
        let warning_count = stderr.matches("warning:").count();

        let message = if success {
            "Build clean (cargo check: 0 errors)".to_string()
        } else {
            stderr
        };

        Ok(BuildStatus {
            success,
            message,
            error_count,
            warning_count,
        })
    }

    fn execute_node(
        &self,
        project_root: &Path,
        node_id: &str,
        file_path: &str,
        entrypoint: Option<&str>,
        input: &str,
    ) -> Result<ExecutionResult, CoreError> {
        NodeRunner::execute_node(project_root, node_id, file_path, entrypoint, input)
    }

    fn scaffold_node(
        &self,
        manifest: &mut ProjectManifest,
        kind: &NodeKind,
        name: &str,
    ) -> Result<String, CoreError> {
        Scaffolder::scaffold_rust_node(manifest, kind, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_adapter_info() {
        let adapter = RustAdapter::new();
        assert_eq!(adapter.language_name(), "rust");
    }
}
