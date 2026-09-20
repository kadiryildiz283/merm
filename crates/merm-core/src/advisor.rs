use crate::error::CoreError;
use crate::llm_client::LlmClient;
use crate::manifest::ProjectManifest;
use crate::rust_scanner::RustScanner;
use crate::transactions::TransactionSnapshot;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command as StdCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    pub is_compatible: bool,
    pub compilation_success: bool,
    pub total_classes: usize,
    pub matched_classes: usize,
    pub diagnostics: Vec<String>,
    pub llm_critique: Option<String>,
}

impl CheckReport {
    pub fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Mermaid-Rust Compatibility Report ===".to_string());
        lines.push(format!(
            "Status: {} | Compilation: {}",
            if self.is_compatible { "PASS" } else { "FAIL" },
            if self.compilation_success {
                "OK"
            } else {
                "ERRORS"
            }
        ));
        lines.push(format!(
            "Nodes Matched: {} / {}",
            self.matched_classes, self.total_classes
        ));
        lines.push("--- Diagnostics ---".to_string());
        for d in &self.diagnostics {
            lines.push(format!("  {}", d));
        }
        if let Some(ref critique) = self.llm_critique {
            lines.push("--- LLM Architectural Critique ---".to_string());
            lines.push(critique.clone());
        }
        lines.join("\n")
    }

    pub fn summary(&self) -> String {
        format!(
            "Compatibility: {}/{} matched | Status: {}",
            self.matched_classes,
            self.total_classes,
            if self.is_compatible { "PASS" } else { "FAIL" }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdviceProposal {
    pub prompt: String,
    pub analysis: String,
    pub suggested_files: Vec<(String, String)>,
    pub suggested_diagram: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiOutputSchema {
    pub explanation: String,
    #[serde(default)]
    pub files: Vec<AiFileEntry>,
    #[serde(default)]
    pub diagram: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiFileEntry {
    pub path: String,
    pub content: String,
}

pub struct Advisor;

impl Advisor {
    pub async fn run_check(
        manifest: &ProjectManifest,
        diagram_source: &str,
    ) -> Result<CheckReport, CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;
        let compat = RustScanner::verify_diagram_compatibility(diagram_source, &scan_report);

        // Run cargo check
        let cargo_check = StdCommand::new("cargo")
            .current_dir(&manifest.project_root)
            .arg("check")
            .output();

        let (compilation_success, compile_msg) = match cargo_check {
            Ok(out) => {
                let success = out.status.success();
                let msg = if success {
                    "cargo check passed with 0 errors".to_string()
                } else {
                    format!(
                        "cargo check failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    )
                };
                (success, msg)
            }
            Err(e) => (false, format!("Failed to run cargo check: {}", e)),
        };

        let mut diagnostics = compat.diagnostics.clone();
        diagnostics.push(format!("Build check: {}", compile_msg));

        // Query LLM if configured
        let llm = LlmClient::new(
            manifest.settings.llm_endpoint.clone(),
            manifest.settings.llm_model.clone(),
        );

        let system_prompt = "You are a senior Rust systems architect verifying Mermaid architecture diagrams against actual Rust code.";
        let user_prompt = format!(
            "Verify this Mermaid diagram against the project AST symbols.\nMermaid Diagram:\n{}\n\nRust Symbols:\n{:?}\n\nBuild Status: {}\nProvide a brief, actionable architecture critique.",
            diagram_source,
            scan_report.symbols.iter().map(|s| (&s.name, &s.file_path, &s.is_executable)).collect::<Vec<_>>(),
            if compilation_success { "Clean build" } else { "Compiler errors detected" }
        );

        let llm_critique = match llm.query(system_prompt, &user_prompt).await {
            Ok(critique) => Some(critique),
            Err(e) => Some(format!(
                "[Offline Mode] LLM endpoint unreachable ({}); deterministic checks verified.",
                e
            )),
        };

        Ok(CheckReport {
            is_compatible: compat.is_compatible && compilation_success,
            compilation_success,
            total_classes: compat.total_classes,
            matched_classes: compat.matched_classes,
            diagnostics,
            llm_critique,
        })
    }

    pub async fn request_advice(
        manifest: &ProjectManifest,
        diagram_source: &str,
        user_prompt: &str,
    ) -> Result<AdviceProposal, CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;

        let llm = LlmClient::new(
            manifest.settings.llm_endpoint.clone(),
            manifest.settings.llm_model.clone(),
        );

        let system_prompt = r#"You are a senior Rust systems architect. The user is asking for architectural advice for their Rust project and its Mermaid diagram.
Analyze the request and provide your advice. If files or diagrams should be changed, you can describe them.
Always structure your advice clearly with rationale and trade-offs."#;

        let user_query = format!(
            "User Query: {}\n\nMermaid Diagram:\n{}\n\nExisting Rust Modules:\n{:?}\n",
            user_prompt,
            diagram_source,
            scan_report
                .symbols
                .iter()
                .map(|s| (&s.name, &s.file_path))
                .collect::<Vec<_>>()
        );

        let analysis = match llm.query(system_prompt, &user_query).await {
            Ok(ans) => ans,
            Err(e) => format!(
                "[Offline Architectural Recommendation for '{}']\n- Review module cohesion and decoupled responsibilities.\n- Ensure each bound class exposes an executable `run(input: &str) -> String` test entrypoint.\n(Note: LLM endpoint offline: {})",
                user_prompt, e
            ),
        };

        Ok(AdviceProposal {
            prompt: user_prompt.to_string(),
            analysis,
            suggested_files: Vec::new(),
            suggested_diagram: None,
        })
    }

    pub fn apply_advice(
        manifest: &mut ProjectManifest,
        proposal: &AdviceProposal,
        current_diagram: &str,
    ) -> Result<String, CoreError> {
        if proposal.suggested_files.is_empty() && proposal.suggested_diagram.is_none() {
            return Ok("Advice contained architectural guidance without file modifications. No files were mutated.".to_string());
        }

        let file_paths: Vec<String> = proposal
            .suggested_files
            .iter()
            .map(|(p, _)| p.clone())
            .collect();

        // 1. Create rollback snapshot
        let snapshot =
            TransactionSnapshot::create(&manifest.project_root, &file_paths, current_diagram)?;

        // 2. Apply proposed file mutations
        for (rel_path, content) in &proposal.suggested_files {
            let abs_path = manifest.project_root.join(rel_path);
            if let Some(parent) = abs_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::write(&abs_path, content) {
                let _ = snapshot.rollback();
                return Err(CoreError::LayoutFailed(format!(
                    "Failed to write proposed file {}: {}",
                    rel_path, e
                )));
            }
        }

        // 3. Verify cargo check or rollback
        snapshot.verify_or_rollback()?;

        Ok(format!(
            "Successfully applied advice recommendations ({} files modified, build verified).",
            proposal.suggested_files.len()
        ))
    }

    pub async fn execute_ai(
        manifest: &mut ProjectManifest,
        diagram_source: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<String>), CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;

        let llm = LlmClient::new(
            manifest.settings.llm_endpoint.clone(),
            manifest.settings.llm_model.clone(),
        );

        let system_prompt = r#"You are an autonomous Rust engineer. Return ONLY a JSON object conforming to this schema:
{
  "explanation": "Brief explanation of changes",
  "files": [
    { "path": "src/example.rs", "content": "..." }
  ],
  "diagram": "classDiagram\n..."
}
Do not include any conversational preamble or markdown code fences outside the JSON."#;

        let user_query = format!(
            "Task: {}\n\nCurrent Mermaid Diagram:\n{}\n\nExisting Modules:\n{:?}",
            user_prompt,
            diagram_source,
            scan_report
                .symbols
                .iter()
                .map(|s| (&s.name, &s.file_path))
                .collect::<Vec<_>>()
        );

        let raw_response = llm.query(system_prompt, &user_query).await?;

        // Extract JSON payload
        let json_str = if let Some(start) = raw_response.find('{') {
            if let Some(end) = raw_response.rfind('}') {
                &raw_response[start..=end]
            } else {
                &raw_response[start..]
            }
        } else {
            &raw_response
        };

        let ai_data: AiOutputSchema = serde_json::from_str(json_str).map_err(|e| {
            CoreError::LayoutFailed(format!(
                "Failed to parse AI refactoring JSON: {}\nRaw output:\n{}",
                e, raw_response
            ))
        })?;

        let file_paths: Vec<String> = ai_data.files.iter().map(|f| f.path.clone()).collect();
        let snapshot =
            TransactionSnapshot::create(&manifest.project_root, &file_paths, diagram_source)?;

        for f in &ai_data.files {
            let abs_path = manifest.project_root.join(&f.path);
            if let Some(parent) = abs_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::write(&abs_path, &f.content) {
                let _ = snapshot.rollback();
                return Err(CoreError::LayoutFailed(format!(
                    "Failed to write file {}: {}",
                    f.path, e
                )));
            }
        }

        // Verify with cargo check
        snapshot.verify_or_rollback()?;

        Ok((ai_data.explanation, ai_data.diagram))
    }
}
