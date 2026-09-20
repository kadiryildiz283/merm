use merm_core::{
    AdviceProposal, Advisor, ArchitectureGraph, AstRewriter, CheckReport, Command,
    DiagramExtractor, ExecutionResult, LayoutDirection, LayoutEngine, NodeBinding, NodeKind,
    NodeRunner, ProjectManifest, ReconciliationEngine, RenderedDiagram, RustScanner, Scaffolder,
    ThemeId,
};
use merm_ipc::EditorCommand;
use merm_render::{BackendType, RenderEngine, Transform2D};
use std::env;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::time::Duration;

use crate::modal::{ModalController, UiAction, UiMode};
use crate::worker::{AsyncWorker, WorkerResult, WorkerTask};

pub struct AppState {
    pub diagram_source: String,
    pub active_direction: LayoutDirection,
    pub current_diagram: Option<RenderedDiagram>,
    pub transform: Transform2D,
    pub modal: ModalController,
    pub render_engine: RenderEngine,
    pub active_node_id: Option<String>,
    pub theme: ThemeId,
    pub status_message: String,
    pub is_running: bool,

    // Project binding and execution subsystem
    pub manifest: Option<ProjectManifest>,
    pub pending_advice: Option<AdviceProposal>,
    pub last_check_report: Option<CheckReport>,
    pub last_execution_result: Option<ExecutionResult>,
    pub report_content: Option<String>,
    pub is_busy: bool,
    pub busy_message: String,
    pub worker: Option<AsyncWorker>,
}

impl AppState {
    pub fn new(source: String, force_software_render: bool) -> Self {
        Self::with_theme(source, force_software_render, ThemeId::CatppuccinMocha)
    }

    pub fn with_theme(source: String, force_software_render: bool, theme: ThemeId) -> Self {
        let render_engine = if force_software_render {
            RenderEngine::force_software()
        } else {
            RenderEngine::new_with_auto_fallback()
        };

        let mut state = Self {
            diagram_source: source,
            active_direction: LayoutDirection::TD,
            current_diagram: None,
            transform: Transform2D::default(),
            modal: ModalController::default(),
            render_engine,
            active_node_id: None,
            theme,
            status_message: "Ready".to_string(),
            is_running: true,
            manifest: None,
            pending_advice: None,
            last_check_report: None,
            last_execution_result: None,
            report_content: None,
            is_busy: false,
            busy_message: String::new(),
            worker: None,
        };

        // Try detecting current directory as a Rust project automatically
        if let Ok(curr) = env::current_dir() {
            if let Some(cargo_root) = ProjectManifest::detect_cargo_root(&curr) {
                let _ = state.bind_project(Some(cargo_root.to_str().unwrap_or(".")));
            }
        }

        state.recalculate_diagram();
        state
    }

    pub fn recalculate_diagram(&mut self) {
        let engine = LayoutEngine::new(
            Duration::from_millis(2000),
            32 * 1024 * 1024,
            self.theme.palette(),
        );
        let extracted = DiagramExtractor::extract(&self.diagram_source);

        let source_to_render = match extracted {
            Ok(blocks) if !blocks.is_empty() => blocks[0].source.clone(),
            _ => self.diagram_source.clone(),
        };

        match engine.render_with_watchdog(&source_to_render) {
            Ok(diagram) => {
                self.transform
                    .fit_to_viewport(diagram.width, diagram.height, 1280.0, 720.0);
                self.current_diagram = Some(diagram);
                self.status_message = format!(
                    "Loaded diagram (Backend: {:?})",
                    self.render_engine.active_backend()
                );
            }
            Err(e) => {
                self.status_message = format!("Error calculating layout: {}", e);
            }
        }
    }

    pub fn bind_project(&mut self, target_path: Option<&str>) -> Result<(), String> {
        let path = if let Some(p) = target_path {
            PathBuf::from(p)
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let cargo_root = ProjectManifest::detect_cargo_root(&path)
            .ok_or_else(|| format!("No Cargo.toml found in {:?} or any parent directory", path))?;

        let mut manifest = ProjectManifest::load_or_init(&cargo_root).map_err(|e| e.to_string())?;

        // Scan project and auto-bind symbols to manifest
        if let Ok(scan_report) = RustScanner::scan_project(&cargo_root) {
            for sym in &scan_report.symbols {
                manifest.add_binding(NodeBinding {
                    id: sym.name.clone(),
                    file: sym.file_path.clone(),
                    symbol: sym.name.clone(),
                    kind: match sym.kind {
                        merm_core::RustSymbolKind::Struct => "struct".to_string(),
                        merm_core::RustSymbolKind::Enum => "enum".to_string(),
                        merm_core::RustSymbolKind::Module => "module".to_string(),
                        merm_core::RustSymbolKind::Function => "fn".to_string(),
                    },
                    executable: sym.is_executable,
                    entrypoint: sym.primary_entrypoint.clone(),
                    input_type: Some("String".to_string()),
                    output_type: Some("String".to_string()),
                });
            }
            let _ = manifest.save();

            // If current diagram is empty, minimal, or old sample, populate with scanned class diagram
            let is_sample_or_empty = self.diagram_source.contains("PaymentService")
                || self.diagram_source.contains("WelcomeToMerm")
                || self.diagram_source.trim().is_empty()
                || self.diagram_source.trim() == "classDiagram";

            if is_sample_or_empty {
                self.diagram_source = RustScanner::generate_mermaid_class_diagram(&scan_report);
                self.recalculate_diagram();
            }
        }

        let name = manifest.project_name.clone();
        let bound_count = manifest.bindings.len();
        self.manifest = Some(manifest);

        self.status_message = format!(
            "Bound to project '{}' ({} modules mapped to diagram classes)",
            name, bound_count
        );

        Ok(())
    }

    pub fn execute_command_str(&mut self, raw: &str) {
        let cmd = Command::parse(raw);
        match cmd {
            Command::Set { path } => {
                let p_str = path.as_deref();
                if let Err(e) = self.bind_project(p_str) {
                    self.status_message = format!("&set error: {}", e);
                }
            }
            Command::Check => {
                if let Some(ref manifest) = self.manifest {
                    if let Some(ref w) = self.worker {
                        self.is_busy = true;
                        self.busy_message = "Running &check...".to_string();
                        self.status_message =
                            "Running &check in background (AST, build & LLM)...".to_string();
                        let _ = w.dispatch(WorkerTask::Check {
                            manifest: manifest.clone(),
                            diagram_source: self.diagram_source.clone(),
                        });
                        return;
                    }
                    self.status_message =
                        "Running &check (verifying Rust AST, build & LLM)...".to_string();
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(runtime) = rt {
                        match runtime.block_on(Advisor::run_check(manifest, &self.diagram_source)) {
                            Ok(report) => {
                                let summary = report.format_text();
                                self.status_message = format!("&check: {}", report.summary());
                                self.report_content = Some(summary);
                                self.last_check_report = Some(report);
                                self.modal.mode = UiMode::Report;
                            }
                            Err(e) => {
                                self.status_message = format!("&check failed: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message = "No project bound! Run `&set [PATH]` first.".to_string();
                }
            }
            Command::Advice { prompt } => {
                if let Some(ref manifest) = self.manifest {
                    if let Some(ref w) = self.worker {
                        self.is_busy = true;
                        self.busy_message = "Querying LLM for advice...".to_string();
                        self.status_message =
                            format!("Querying LLM in background for '{}'...", prompt);
                        let _ = w.dispatch(WorkerTask::Advice {
                            manifest: manifest.clone(),
                            diagram_source: self.diagram_source.clone(),
                            prompt,
                        });
                        return;
                    }
                    self.status_message = format!("Querying LLM for advice on '{}'...", prompt);
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(runtime) = rt {
                        match runtime.block_on(Advisor::request_advice(
                            manifest,
                            &self.diagram_source,
                            &prompt,
                        )) {
                            Ok(proposal) => {
                                self.status_message =
                                    "&advice ready. Type `&ok` to apply proposal.".to_string();
                                self.report_content = Some(format!(
                                    "=== Architectural Advice Proposal ===\nQuery: {}\n\n{}\n\nType `&ok` to apply changes.",
                                    proposal.prompt, proposal.analysis
                                ));
                                self.pending_advice = Some(proposal);
                                self.modal.mode = UiMode::Report;
                            }
                            Err(e) => {
                                self.status_message = format!("&advice failed: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message = "No project bound! Run `&set [PATH]` first.".to_string();
                }
            }
            Command::Ok => {
                if let Some(ref proposal) = self.pending_advice.clone() {
                    if let Some(ref mut manifest) = self.manifest {
                        if let Some(ref w) = self.worker {
                            self.is_busy = true;
                            self.busy_message =
                                "Applying advice & verifying with rollback...".to_string();
                            self.status_message = "Applying advice in background...".to_string();
                            let _ = w.dispatch(WorkerTask::Ok {
                                manifest: manifest.clone(),
                                proposal: proposal.clone(),
                                diagram_source: self.diagram_source.clone(),
                            });
                            return;
                        }
                        match Advisor::apply_advice(manifest, proposal, &self.diagram_source) {
                            Ok(msg) => {
                                self.status_message = format!("&ok: {}", msg);
                                if let Some(ref new_diag) = proposal.suggested_diagram {
                                    self.diagram_source = new_diag.clone();
                                    self.recalculate_diagram();
                                }
                                self.pending_advice = None;
                            }
                            Err(e) => {
                                self.status_message = format!("&ok rollback: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message =
                        "No pending advice to apply. Run `&advice <QUERY>` first.".to_string();
                }
            }
            Command::Ai { prompt } => {
                if let Some(ref mut manifest) = self.manifest {
                    if let Some(ref w) = self.worker {
                        self.is_busy = true;
                        self.busy_message = "Running autonomous &ai...".to_string();
                        self.status_message =
                            format!("Running &ai in background for '{}'...", prompt);
                        let _ = w.dispatch(WorkerTask::Ai {
                            manifest: manifest.clone(),
                            diagram_source: self.diagram_source.clone(),
                            prompt,
                        });
                        return;
                    }
                    self.status_message = format!("Running autonomous &ai for '{}'...", prompt);
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(runtime) = rt {
                        match runtime.block_on(Advisor::execute_ai(
                            manifest,
                            &self.diagram_source,
                            &prompt,
                        )) {
                            Ok((explanation, new_diagram)) => {
                                self.status_message = format!("&ai completed: {}", explanation);
                                if let Some(d) = new_diagram {
                                    self.diagram_source = d;
                                    self.recalculate_diagram();
                                }
                            }
                            Err(e) => {
                                self.status_message = format!("&ai error: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message = "No project bound! Run `&set [PATH]` first.".to_string();
                }
            }
            Command::Add { kind, name } => {
                self.add_node(kind, &name);
            }
            Command::Connect { from, to, label } => {
                self.connect_nodes(&from, &to, label.as_deref());
            }
            Command::Test { node_id, input } => {
                let target_node = node_id.or_else(|| self.active_node_id.clone());
                if let Some(nid) = target_node {
                    self.execute_node_test(&nid, input.as_deref().unwrap_or(""));
                } else {
                    self.status_message = "No node selected or specified for testing.".to_string();
                }
            }
            Command::Help => {
                self.report_content = Some(
                    r#"=== merm Interactive Command Protocol ===
Commands:
  &set [PATH]                - Bind current Mermaid diagram to a Rust project
  &check                     - Test compatibility between project and Mermaid (AST + build + LLM)
  &advice <PROMPT>           - Request architecture advice from LLM (read-only)
  &ok                        - Apply recommendations from &advice with rollback protection
  &ai <PROMPT>               - Autonomous multi-file generation/refactoring with verification
  :add <class|struct|enum> <Name> - Add new node to diagram & scaffold Rust file
  :connect <From> <To> [lbl] - Connect two diagram nodes with arrow
  :test [Node] [Input]       - Execute node test harness with input/output capture
  :help                      - Show this command reference

Keybindings (NORMAL mode):
  : / &   - Open Command bar
  t       - Test selected node (opens Input/Output drawer)
  T       - Cycle theme
  a / o   - Add new class / struct
  c       - Connect nodes
  e       - Open bound Rust file in $EDITOR
  h,j,k,l - Pan diagram
  +, -    - Zoom in / out
  Tab     - Pivot direction (TD -> LR -> BT -> RL)
  n / N   - Select next / previous node
  q / Esc - Quit / close modal
"#
                    .to_string(),
                );
                self.modal.mode = UiMode::Report;
            }
            Command::Custom(s) => {
                self.status_message = format!("Unknown command: {}", s);
            }
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, name: &str) {
        self.diagram_source = Scaffolder::add_node_to_diagram(&self.diagram_source, &kind, name);
        self.recalculate_diagram();

        if let Some(ref mut manifest) = self.manifest {
            match Scaffolder::scaffold_rust_node(manifest, &kind, name) {
                Ok(rel_path) => {
                    self.status_message =
                        format!("Added node '{}' and scaffolded '{}'", name, rel_path);
                }
                Err(e) => {
                    self.status_message = format!(
                        "Added node '{}' to diagram (scaffolding error: {})",
                        name, e
                    );
                }
            }
        } else {
            self.status_message = format!("Added node '{}' to diagram", name);
        }
    }

    pub fn connect_nodes(&mut self, from: &str, to: &str, label: Option<&str>) {
        self.diagram_source =
            Scaffolder::connect_nodes_in_diagram(&self.diagram_source, from, to, label);
        self.recalculate_diagram();
        self.status_message = format!("Connected {} --> {}", from, to);
    }

    pub fn execute_node_test(&mut self, node_id: &str, input: &str) {
        if let Some(ref manifest) = self.manifest {
            let (file_path, entrypoint) = if let Some(b) = manifest.get_binding(node_id) {
                (b.file.as_str(), b.entrypoint.as_deref())
            } else {
                ("src/lib.rs", Some("run"))
            };

            if let Some(ref w) = self.worker {
                self.is_busy = true;
                self.busy_message = format!("Testing node '{}'...", node_id);
                self.status_message = format!(
                    "Executing node '{}' in background with input '{}'...",
                    node_id, input
                );
                let _ = w.dispatch(WorkerTask::Test {
                    project_root: manifest.project_root.clone(),
                    node_id: node_id.to_string(),
                    file_path: file_path.to_string(),
                    entrypoint: entrypoint.map(|s| s.to_string()),
                    input: input.to_string(),
                });
                return;
            }

            self.status_message = format!("Executing node '{}' with input '{}'...", node_id, input);

            match NodeRunner::execute_node(
                &manifest.project_root,
                node_id,
                file_path,
                entrypoint,
                input,
            ) {
                Ok(res) => {
                    let status = if res.success { "SUCCESS" } else { "FAILED" };
                    self.status_message = format!(
                        "Node '{}' [{}] ({}ms): {}",
                        node_id, status, res.duration_ms, res.output_payload
                    );
                    self.report_content = Some(format!(
                        "=== Node Execution Result: {} ===\nStatus: {} (Exit code: {:?})\nDuration: {}ms\n\n[Output Payload]:\n{}\n\n[Stdout]:\n{}\n\n[Stderr]:\n{}",
                        node_id,
                        status,
                        res.exit_code,
                        res.duration_ms,
                        res.output_payload,
                        res.stdout,
                        res.stderr
                    ));
                    self.last_execution_result = Some(res);
                }
                Err(e) => {
                    self.status_message = format!("Execution failed for node '{}': {}", node_id, e);
                }
            }
        } else {
            self.status_message =
                "No project bound! Run `&set [PATH]` before testing nodes.".to_string();
        }
    }

    pub fn open_node_file(&mut self, node_id: &str) {
        if let Some(ref manifest) = self.manifest {
            if let Some(binding) = manifest.get_binding(node_id) {
                let full_path = manifest.project_root.join(&binding.file);
                let editor = env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                log::info!("Opening {} in {}", full_path.display(), editor);
                let _ = StdCommand::new(&editor).arg(&full_path).spawn();
                self.status_message = format!("Opened {} in {}", binding.file, editor);
                return;
            }
        }
        self.status_message = format!("No file binding found for node '{}'", node_id);
    }

    pub fn handle_ipc_command(&mut self, command: EditorCommand) {
        match command {
            EditorCommand::CursorMoved(params) => {
                log::debug!("Editor cursor moved: {:?}", params);
                if let Some(sym) = params.symbol {
                    if let Some(ref diag) = self.current_diagram {
                        if let Some(node) =
                            diag.nodes.iter().find(|n| n.id == sym || n.label == sym)
                        {
                            self.active_node_id = Some(node.id.clone());
                            self.status_message = format!("Focused node: {}", node.label);
                        }
                    }
                }
            }
            EditorCommand::Reload(params) => {
                if let Some(content) = params.content {
                    self.diagram_source = content;
                    self.recalculate_diagram();
                    self.status_message = format!("Diagram reloaded from editor ({})", params.file);
                }
            }
            EditorCommand::JumpToDefinition(params) => {
                self.active_node_id = Some(params.node_id.clone());
                self.status_message = format!(
                    "Jump to definition -> {}:{}",
                    params.target_file, params.target_line
                );
            }
            EditorCommand::Ping => {}
            EditorCommand::Custom { method, .. } => {
                log::debug!("Unhandled custom IPC method: {}", method);
            }
        }
    }

    pub fn handle_worker_result(&mut self, res: WorkerResult) {
        self.is_busy = false;
        self.busy_message.clear();
        match res {
            WorkerResult::CheckFinished(Ok(report)) => {
                let status_str = if report.is_compatible { "PASS" } else { "FAIL" };
                self.status_message = format!("&check completed: Status: {}", status_str);
                self.report_content = Some(report.format_text());
                self.last_check_report = Some(report);
                self.modal.mode = UiMode::Report;
            }
            WorkerResult::CheckFinished(Err(e)) => {
                self.status_message = format!("&check failed: {}", e);
            }
            WorkerResult::AdviceFinished(Ok(proposal)) => {
                self.status_message = "&advice ready. Type `&ok` to apply proposal.".to_string();
                self.report_content = Some(format!(
                    "=== Architectural Advice Proposal ===\nQuery: {}\n\n{}\n\nType `&ok` to apply changes.",
                    proposal.prompt, proposal.analysis
                ));
                self.pending_advice = Some(proposal);
                self.modal.mode = UiMode::Report;
            }
            WorkerResult::AdviceFinished(Err(e)) => {
                self.status_message = format!("&advice failed: {}", e);
            }
            WorkerResult::OkFinished(Ok((msg, new_diag))) => {
                self.status_message = format!("&ok: {}", msg);
                if let Some(d) = new_diag {
                    self.diagram_source = d;
                    self.recalculate_diagram();
                }
                self.pending_advice = None;
            }
            WorkerResult::OkFinished(Err(e)) => {
                self.status_message = format!("&ok rollback: {}", e);
            }
            WorkerResult::AiFinished(Ok((explanation, new_diag))) => {
                self.status_message = format!("&ai completed: {}", explanation);
                if let Some(d) = new_diag {
                    self.diagram_source = d;
                    self.recalculate_diagram();
                }
            }
            WorkerResult::AiFinished(Err(e)) => {
                self.status_message = format!("&ai error: {}", e);
            }
            WorkerResult::TestFinished { node_id, result } => match result {
                Ok(res) => {
                    let status = if res.success { "SUCCESS" } else { "FAILED" };
                    self.status_message = format!(
                        "Node '{}' [{}] ({}ms): {}",
                        node_id, status, res.duration_ms, res.output_payload
                    );
                    self.report_content = Some(format!(
                        "=== Node Execution Result: {} ===\nStatus: {} (Exit code: {:?})\nDuration: {}ms\n\n[Output Payload]:\n{}\n\n[Stdout]:\n{}\n\n[Stderr]:\n{}",
                        node_id,
                        status,
                        res.exit_code,
                        res.duration_ms,
                        res.output_payload,
                        res.stdout,
                        res.stderr
                    ));
                    self.last_execution_result = Some(res);
                }
                Err(e) => {
                    self.status_message = format!("Execution failed for node '{}': {}", node_id, e);
                }
            },
        }
    }

    pub fn handle_source_files_changed(&mut self, paths: &[PathBuf]) {
        if let Some(ref manifest) = self.manifest {
            log::info!("Live reload: {} source files changed", paths.len());
            if let Ok(scan_report) = RustScanner::scan_project(&manifest.project_root) {
                let code_graph = ArchitectureGraph::from_project_symbols(&scan_report);
                let diag_graph = ArchitectureGraph::from_mermaid_source(&self.diagram_source);
                let reconcil = ReconciliationEngine::reconcile(&diag_graph, &code_graph);

                if reconcil.is_synchronized() {
                    self.status_message = format!(
                        "Live sync: {} file(s) updated. Architecture synchronized ({} nodes).",
                        paths.len(),
                        reconcil.matched_nodes.len()
                    );
                } else {
                    let div_count = reconcil.divergences.len() + reconcil.diagram_only_nodes.len();
                    self.status_message = format!(
                        "Live sync: {} file(s) updated. Divergence detected ({} nodes)! Press 'i' or run &check.",
                        paths.len(),
                        div_count
                    );
                }
            }
        }
    }

    pub fn handle_key_action(&mut self, action: UiAction) {
        match action {
            UiAction::Pan { dx, dy } => {
                self.transform.pan(dx, dy);
            }
            UiAction::Zoom { factor } => {
                self.transform.zoom_at(factor, 640.0, 360.0);
            }
            UiAction::ResetView => {
                if let Some(ref diag) = self.current_diagram {
                    self.transform
                        .fit_to_viewport(diag.width, diag.height, 1280.0, 720.0);
                }
            }
            UiAction::PivotDirection => {
                let next_dir = self.active_direction.next();
                self.diagram_source = AstRewriter::pivot_direction(&self.diagram_source, next_dir);
                self.active_direction = next_dir;
                self.recalculate_diagram();
                self.status_message = format!("Pivoted direction to {}", next_dir.as_str());
            }
            UiAction::CycleTheme => {
                self.theme = self.theme.next();
                if let Some(ref mut diag) = self.current_diagram {
                    diag.regenerate_svg(&self.theme.palette());
                } else {
                    self.recalculate_diagram();
                }
                self.status_message = format!("Theme: {}", self.theme.palette().name);
            }
            UiAction::SelectNextNode => {
                let next_info = self.current_diagram.as_ref().and_then(|diag| {
                    if diag.nodes.is_empty() {
                        None
                    } else {
                        let cur_idx = self
                            .active_node_id
                            .as_ref()
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id))
                            .unwrap_or(0);
                        let next_idx = (cur_idx + 1) % diag.nodes.len();
                        Some((
                            diag.nodes[next_idx].id.clone(),
                            diag.nodes[next_idx].label.clone(),
                        ))
                    }
                });
                if let Some((next_id, next_label)) = next_info {
                    self.select_node(Some(&next_id));
                    self.modal.active_test_node_id = Some(next_id.clone());
                    self.status_message = format!("Selected: {}", next_label);
                }
            }
            UiAction::SelectPrevNode => {
                let prev_info = self.current_diagram.as_ref().and_then(|diag| {
                    if diag.nodes.is_empty() {
                        None
                    } else {
                        let cur_idx = self
                            .active_node_id
                            .as_ref()
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id))
                            .unwrap_or(0);
                        let prev_idx = if cur_idx == 0 {
                            diag.nodes.len() - 1
                        } else {
                            cur_idx - 1
                        };
                        Some((
                            diag.nodes[prev_idx].id.clone(),
                            diag.nodes[prev_idx].label.clone(),
                        ))
                    }
                });
                if let Some((prev_id, prev_label)) = prev_info {
                    self.select_node(Some(&prev_id));
                    self.modal.active_test_node_id = Some(prev_id.clone());
                    self.status_message = format!("Selected: {}", prev_label);
                }
            }
            UiAction::ExecuteCommand(cmd_str) => {
                self.execute_command_str(&cmd_str);
            }
            UiAction::ExecuteNodeTest { node_id, input } => {
                self.execute_node_test(&node_id, &input);
            }
            UiAction::OpenEditor(_) => {
                if let Some(ref nid) = self.active_node_id.clone() {
                    self.open_node_file(nid);
                }
            }
            UiAction::Reload => {
                self.recalculate_diagram();
            }
            UiAction::Quit => {
                self.is_running = false;
            }
            UiAction::SetMode(_) | UiAction::None => {}
        }
    }

    pub fn move_node(&mut self, node_idx: usize, new_x: f32, new_y: f32) {
        if let Some(ref mut diag) = self.current_diagram {
            if node_idx < diag.nodes.len() {
                diag.nodes[node_idx].x = new_x;
                diag.nodes[node_idx].y = new_y;
                diag.regenerate_svg(&self.theme.palette());
            }
        }
    }

    pub fn select_node(&mut self, node_id: Option<&str>) {
        if let Some(ref mut diag) = self.current_diagram {
            diag.selected_node_id = node_id.map(|s| s.to_string());
            diag.regenerate_svg(&self.theme.palette());
        }
        self.active_node_id = node_id.map(|s| s.to_string());
        self.modal.active_test_node_id = node_id.map(|s| s.to_string());
    }

    pub fn render_current_frame(&mut self) -> Option<merm_render::RenderResult> {
        if let Some(ref diag) = self.current_diagram {
            let res = self.render_engine.render(&self.transform, &diag.svg);
            Some(res)
        } else {
            None
        }
    }

    pub fn hud_status(&self) -> String {
        if self.is_busy {
            return format!("[BUSY: {}] Working in background...", self.busy_message);
        }

        // Mode-specific status line
        match self.modal.mode {
            UiMode::Command => {
                return format!("{}█", self.modal.command_buffer);
            }
            UiMode::NodeTest => {
                let node = self
                    .modal
                    .active_test_node_id
                    .as_deref()
                    .unwrap_or("Unknown");
                return format!(
                    "[TEST NODE: {}] Input: {}█ (Press Enter to execute, Esc to exit)",
                    node, self.modal.test_input_buffer
                );
            }
            UiMode::Inspector => {
                let node = self.active_node_id.as_deref().unwrap_or("Unknown");
                return format!(
                    "[INSPECTOR: {}] Press 'e' to edit, 't' to test, Esc/q to exit",
                    node
                );
            }
            UiMode::Report => {
                return "[REPORT VIEW] Press Esc or q to return to diagram".to_string();
            }
            _ => {}
        }

        let backend_str = match self.render_engine.active_backend() {
            BackendType::HardwareWgpu => "WGPU 120FPS",
            BackendType::SoftwareFallback => "CPU (softbuffer) 60FPS",
        };

        let mode_str = match self.modal.mode {
            UiMode::Normal => "NORMAL",
            UiMode::Pan => "PAN",
            UiMode::Search => "SEARCH",
            UiMode::Jump => "JUMP",
            _ => "NORMAL",
        };

        let project_str = if let Some(ref m) = self.manifest {
            format!(" [Project: {}]", m.project_name)
        } else {
            String::new()
        };

        let sel_str = if let Some(ref sel_id) = self.active_node_id {
            if let Some(ref diag) = self.current_diagram {
                if let Some(node) = diag.nodes.iter().find(|n| &n.id == sel_id) {
                    if !node.attributes.is_empty()
                        || !node.methods.is_empty()
                        || node.stereotype.is_some()
                        || node.doc_comment.is_some()
                    {
                        let type_kind = node.stereotype.as_deref().unwrap_or("CLASS");
                        format!(
                            " [{}: {} ({} vars, {} funcs, exec: 't')]",
                            type_kind.to_uppercase(),
                            node.id,
                            node.attributes.len(),
                            node.methods.len()
                        )
                    } else {
                        format!(" [NODE: {} (exec: 't')]", node.label)
                    }
                } else {
                    format!(" [{}]", sel_id)
                }
            } else {
                format!(" [{}]", sel_id)
            }
        } else {
            String::new()
        };

        format!(
            "[MODE: {}]{} [{}] [{}] [Zoom: {:.1}x]{} | {}",
            mode_str,
            project_str,
            self.theme.palette().name,
            backend_str,
            self.transform.scale,
            sel_str,
            self.status_message
        )
    }
}
