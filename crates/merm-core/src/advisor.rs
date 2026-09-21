use crate::error::CoreError;
use crate::extractor::DiagramExtractor;
use crate::llm_client::{LlmClient, LlmProvider};
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
        let llm = LlmClient::from_settings(&manifest.settings);
        Self::run_check_with_provider(manifest, diagram_source, &llm).await
    }

    pub async fn run_check_with_provider(
        manifest: &ProjectManifest,
        diagram_source: &str,
        provider: &dyn LlmProvider,
    ) -> Result<CheckReport, CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;
        let compat = RustScanner::verify_diagram_compatibility(diagram_source, &scan_report);

        // Run cargo check if Cargo project, otherwise verify file integrity
        let (compilation_success, compile_msg) = if manifest.project_root.join("Cargo.toml").is_file() {
            let cargo_check = StdCommand::new("cargo")
                .current_dir(&manifest.project_root)
                .arg("check")
                .output();

            match cargo_check {
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
            }
        } else {
            (true, format!("Project root verified ({} files scanned)", scan_report.files.len()))
        };

        let mut diagnostics = compat.diagnostics.clone();
        diagnostics.push(format!("Build check: {}", compile_msg));

        let system_prompt = "You are a senior Rust systems architect verifying Mermaid architecture diagrams against actual Rust code.";
        let user_prompt = format!(
            "Verify this Mermaid diagram against the project AST symbols.\nMermaid Diagram:\n{}\n\nRust Symbols:\n{:?}\n\nBuild Status: {}\nProvide a brief, actionable architecture critique.",
            diagram_source,
            scan_report.symbols.iter().map(|s| (&s.name, &s.file_path, &s.is_executable)).collect::<Vec<_>>(),
            if compilation_success { "Clean build" } else { "Compiler errors detected" }
        );

        let llm_critique = match provider.query(system_prompt, &user_prompt).await {
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
        let llm = LlmClient::from_settings(&manifest.settings);
        Self::request_advice_with_provider(manifest, diagram_source, user_prompt, &llm).await
    }

    pub async fn request_advice_with_provider(
        manifest: &ProjectManifest,
        diagram_source: &str,
        user_prompt: &str,
        provider: &dyn LlmProvider,
    ) -> Result<AdviceProposal, CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;

        let system_prompt = r#"You are a senior Rust systems architect. The user is asking for architectural advice or modifications for their Rust project and Mermaid diagram.
CRITICAL INSTRUCTIONS:
1. DO NOT call any external tools, shell commands, or subagents. Respond directly in text.
2. If your recommendation updates or changes the Mermaid architecture diagram, ALWAYS provide the COMPLETE updated diagram in a ```mermaid ... ``` code block.
3. If your recommendation adds or modifies Rust files, provide each file clearly using:
```rust
// File: src/module.rs
<complete code>
```
or embed a JSON block conforming to {"files": [{"path": "src/...", "content": "..."}], "diagram": "classDiagram..."}.
4. Structure your advice clearly with executive rationale, updated diagram, code implementation, and trade-offs."#;

        let user_query = format!(
            "User Query: {}\n\nCurrent Mermaid Diagram:\n{}\n\nExisting Rust Modules:\n{:?}\n",
            user_prompt,
            diagram_source,
            scan_report
                .symbols
                .iter()
                .map(|s| (&s.name, &s.file_path))
                .collect::<Vec<_>>()
        );

        let analysis = match provider.query(system_prompt, &user_query).await {
            Ok(ans) => ans,
            Err(e) => format!(
                "[Offline Architectural Recommendation for '{}']\n- Review module cohesion and decoupled responsibilities.\n- Ensure each bound class exposes an executable `run(input: &str) -> String` test entrypoint.\n(Note: LLM endpoint offline: {})",
                user_prompt, e
            ),
        };

        // 1. Extract suggested Mermaid diagram from analysis
        let mut suggested_diagram = DiagramExtractor::extract(&analysis)
            .ok()
            .and_then(|blocks| blocks.into_iter().next().map(|b| b.source));

        // 2. Extract suggested files from analysis
        let mut suggested_files = Self::extract_files_from_text(&analysis);

        // 3. Check if JSON schema was embedded
        if let Some(json_ai) = Self::try_extract_json(&analysis) {
            if suggested_diagram.is_none() && json_ai.diagram.is_some() {
                suggested_diagram = json_ai.diagram;
            }
            if suggested_files.is_empty() && !json_ai.files.is_empty() {
                suggested_files = json_ai
                    .files
                    .into_iter()
                    .map(|f| (f.path, f.content))
                    .collect();
            }
        }

        Ok(AdviceProposal {
            prompt: user_prompt.to_string(),
            analysis,
            suggested_files,
            suggested_diagram,
        })
    }

    pub fn extract_files_from_text(text: &str) -> Vec<(String, String)> {
        let mut files = Vec::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("```mermaid") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    i += 1;
                }
                if i < lines.len() {
                    i += 1; // skip closing fence
                }
                continue;
            }

            if line.starts_with("```") {
                let fence_meta = line.trim_start_matches('`').trim();
                let mut candidate_path: Option<String> = None;

                if fence_meta.starts_with(':') {
                    candidate_path = Some(fence_meta.trim_start_matches(':').trim().to_string());
                }

                if candidate_path.is_none() && i > 0 {
                    let prev = lines[i - 1].trim();
                    candidate_path = Self::parse_file_path_comment(prev);
                }

                i += 1;
                let mut block_lines = Vec::new();
                let mut inside = true;

                while i < lines.len() && inside {
                    let cur = lines[i];
                    if cur.trim().starts_with("```") {
                        inside = false;
                    } else {
                        if candidate_path.is_none() && block_lines.is_empty() {
                            candidate_path = Self::parse_file_path_comment(cur.trim());
                            if candidate_path.is_none() {
                                block_lines.push(cur);
                            }
                        } else {
                            block_lines.push(cur);
                        }
                    }
                    i += 1;
                }

                if let Some(path) = candidate_path {
                    let clean_path = path
                        .trim()
                        .trim_matches('`')
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if clean_path.contains('.') || clean_path.contains('/') {
                        files.push((clean_path, block_lines.join("\n")));
                    }
                }
            } else {
                i += 1;
            }
        }

        files
    }

    fn parse_file_path_comment(line: &str) -> Option<String> {
        let trimmed = line
            .trim()
            .trim_matches('#')
            .trim()
            .trim_matches('*')
            .trim();
        for prefix in &[
            "// File:", "// file:", "// path:", "File:", "file:", "Path:", "path:",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let path = rest.trim().trim_matches('`').trim_matches(':').trim();
                if !path.is_empty() && (path.ends_with(".rs") || path.contains('/')) {
                    return Some(path.to_string());
                }
            }
        }
        if trimmed.starts_with('`') && trimmed.ends_with('`') {
            let inner = trimmed.trim_matches('`');
            if inner.ends_with(".rs") && inner.contains('/') {
                return Some(inner.to_string());
            }
        }
        None
    }

    fn try_extract_json(text: &str) -> Option<AiOutputSchema> {
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    let slice = &text[start..=end];
                    if let Ok(data) = serde_json::from_str::<AiOutputSchema>(slice) {
                        return Some(data);
                    }
                }
            }
        }
        None
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

        if !file_paths.is_empty() {
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

            // 4. Update manifest bindings if new files were added
            if let Ok(scan_report) = RustScanner::scan_project(&manifest.project_root) {
                for sym in &scan_report.symbols {
                    if manifest.get_binding(&sym.name).is_none() {
                        manifest.add_binding(crate::manifest::NodeBinding {
                            id: sym.name.clone(),
                            file: sym.file_path.clone(),
                            symbol: sym.name.clone(),
                            kind: format!("{:?}", sym.kind).to_lowercase(),
                            executable: sym.is_executable,
                            entrypoint: sym.primary_entrypoint.clone(),
                            input_type: Some("String".to_string()),
                            output_type: Some("String".to_string()),
                        });
                    }
                }
                let _ = manifest.save();
            }

            let diag_info = if proposal.suggested_diagram.is_some() {
                " and diagram updated"
            } else {
                ""
            };

            Ok(format!(
                "Applied advice: {} file(s) mutated{} (build verified).",
                proposal.suggested_files.len(),
                diag_info
            ))
        } else if proposal.suggested_diagram.is_some() {
            Ok("Successfully applied advice: Mermaid diagram updated on canvas.".to_string())
        } else {
            Ok("No modifications were required.".to_string())
        }
    }

    pub async fn execute_ai(
        manifest: &mut ProjectManifest,
        diagram_source: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<String>), CoreError> {
        let llm = LlmClient::from_settings(&manifest.settings);
        Self::execute_ai_with_provider(manifest, diagram_source, user_prompt, &llm).await
    }

    pub async fn execute_ai_with_provider(
        manifest: &mut ProjectManifest,
        diagram_source: &str,
        user_prompt: &str,
        provider: &dyn LlmProvider,
    ) -> Result<(String, Option<String>), CoreError> {
        let scan_report = RustScanner::scan_project(&manifest.project_root)?;

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

        let raw_response = provider.query(system_prompt, &user_query).await?;

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
