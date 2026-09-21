use merm_core::{
    AdviceProposal, Advisor, AgyLlmProvider, ArchitectureGraph, AstRewriter, CheckReport, Command,
    DiagramExtractor, ExecutionResult, GraphMutationDelta, LayoutDirection, LayoutEngine,
    LlmProvider, NodeBinding, NodeKind, NodeRunner, PerformanceTelemetry, ProjectManifest,
    ProjectScanReport, ReconciliationEngine, RenderedDiagram, RustScanner, Scaffolder, ThemeId,
    UndoRedoStack,
};
use merm_ipc::EditorCommand;
use merm_render::{BackendType, RenderEngine, Transform2D};
use std::env;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::time::Duration;

use crate::clipboard::copy_to_clipboard;
use crate::modal::{ModalController, UiAction, UiMode};
use crate::worker::{AsyncWorker, WorkerResult, WorkerTask};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    Explorer,
    #[default]
    Diagrams,
    AstView,
    Executions,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPanelTab {
    Overview,
    #[default]
    Contract,
    Code,
    Runtime,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutAlgorithm {
    #[default]
    Hierarchical,
    ForceDirected,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasTool {
    #[default]
    Pointer,
    Pan,
    Zoom,
    Fit,
    Fullscreen,
}

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
    pub show_split_buffer: bool,
    pub is_busy: bool,
    pub busy_message: String,
    pub worker: Option<AsyncWorker>,

    // Performance & telemetry
    pub telemetry: PerformanceTelemetry,
    pub undo_stack: UndoRedoStack,
    pub active_drag_preview: Option<(usize, f32, f32)>,
    pub drag_start_pos: Option<(f32, f32)>,
    pub focus_mode_active: bool,
    pub cached_scan_report: Option<ProjectScanReport>,

    // Modern Studio UI Layout state
    pub active_sidebar_tab: SidebarTab,
    pub right_panel_tab: RightPanelTab,
    pub layout_algorithm: LayoutAlgorithm,
    pub active_tool: CanvasTool,
    pub show_left_sidebar: bool,
    pub show_right_panel: bool,
    pub active_workspace: String,
    pub workspaces: Vec<String>,
    pub active_code_file: String,
    pub active_code_content: String,
    pub selected_ast_symbol: Option<String>,
    pub command_palette_query: String,
    pub command_palette_selected_idx: usize,
    pub command_palette_visible: bool,
}

impl AppState {
    pub fn new(source: String, force_software_render: bool) -> Self {
        Self::with_theme(source, force_software_render, ThemeId::StudioDark)
    }

    pub fn with_theme(source: String, force_software_render: bool, theme: ThemeId) -> Self {
        let render_engine = if force_software_render {
            RenderEngine::force_software()
        } else {
            RenderEngine::new_with_auto_fallback()
        };

        let initial_source = if source.trim().is_empty() {
            r#"flowchart TD
    User["User <<actor>>"] -->|HTTPS| WebFrontend["Web Frontend <<app>>"]
    User -->|HTTPS| MobileApp["Mobile App <<app>>"]
    WebFrontend -->|HTTPS/REST| ApiGateway["API Gateway <<service>>"]
    MobileApp -->|HTTPS/REST| ApiGateway
    ApiGateway -->|gRPC| AuthService["Auth Service <<service>>"]
    ApiGateway -->|gRPC| UserService["User Service <<service>>"]
    ApiGateway -->|Events| NotificationService["Notification Service <<service>>"]
    AuthService -->|SQL| Postgres["PostgreSQL <<database>>"]
    AuthService -->|Cache| Redis["Redis <<cache>>"]
    UserService -->|SQL| Postgres
    NotificationService -->|Publish| MessageQueue["Message Queue <<queue>>"]
"#
            .to_string()
        } else {
            source
        };

        let default_code = r#"use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub expires_at: u64,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Token generation failed")]
    TokenGenerationFailed,
    #[error("Internal error: {0}")]
    Internal(String),
}

pub struct AuthService;

impl AuthService {
    pub fn new() -> Self {
        Self
    }

    pub fn login(&self, req: &AuthRequest) -> Result<AuthResponse, AuthError> {
        if req.user == "admin" && req.pass == "secret" {
            Ok(AuthResponse {
                token: "jwt_token_preview".to_string(),
                expires_at: 3600,
            })
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }

    pub fn validate_token(&self, _token: &str) -> bool {
        true
    }
}
"#
        .to_string();

        let mut state = Self {
            diagram_source: initial_source,
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
            show_split_buffer: false,
            is_busy: false,
            busy_message: String::new(),
            worker: None,
            telemetry: PerformanceTelemetry::new(),
            undo_stack: UndoRedoStack::new(100),
            active_drag_preview: None,
            drag_start_pos: None,
            focus_mode_active: false,
            cached_scan_report: None,
            active_sidebar_tab: SidebarTab::Diagrams,
            right_panel_tab: RightPanelTab::Contract,
            layout_algorithm: LayoutAlgorithm::Hierarchical,
            active_tool: CanvasTool::Pointer,
            show_left_sidebar: true,
            show_right_panel: true,
            active_workspace: "backend".to_string(),
            workspaces: vec![
                "backend".to_string(),
                "frontend".to_string(),
                "infrastructure".to_string(),
            ],
            active_code_file: "src/services/auth.rs".to_string(),
            active_code_content: default_code,
            selected_ast_symbol: Some("verify_token".to_string()),
            command_palette_query: String::new(),
            command_palette_selected_idx: 0,
            command_palette_visible: false,
        };

        // Try detecting current directory or parent project automatically
        if let Ok(curr) = env::current_dir() {
            if let Some(root) = ProjectManifest::detect_project_root(&curr) {
                let _ = state.bind_project(Some(root.to_str().unwrap_or(".")));
            }
        }

        let p_name = state
            .manifest
            .as_ref()
            .map(|m| m.project_name.as_str())
            .unwrap_or("backend");
        let initial_welcome = format!(
            "# 🤖 Merm Studio AI Architecture & Diagnostic Buffer\n\
            * Project: Bound to '{}' │ Rust syn AST Aware\n\
            * Modern Studio Navigation:\n\
              - Sidebar    : Explorer │ Diagrams │ AST View │ Executions │ Settings\n\
              - Inspector  : Overview │ Contract │ Code │ Runtime │ Logs\n\
              - Palette    : Press ':' or ⌘O to open Command Palette\n\
              - AST Split  : Click 'AST View' or 'Source' in Contract to view Code + AST tree\n\
              - Actions    : Click node to see golden halo and quick action pill [+ 日 ❐ 🗑]\n\
            * Instructions & Commands:\n\
              - &check     : Verify diagram vs project files (AST & LLM)\n\
              - &advice <Q>: Ask architectural guidance from LLM (preview in buffer)\n\
              - &ok        : Apply pending advice with automatic rollback protection\n\
              - &ai <p>    : Autonomous refactoring/scaffolding across project & diagram\n\
              - &agy <p>   : Query Google Antigravity directly\n\
              - :test <n>  : Run executable node test harness\n\
              - :split     : Toggle this AI split buffer (Shortcut: Ctrl+W)\n\
              - :w / :q    : Save diagram / Quit application\n\
            ---",
            p_name
        );
        state.report_content = Some(initial_welcome);

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
            Ok(mut diagram) => {
                // Populate contracts with rich metadata matching studio design
                for node in &mut diagram.nodes {
                    let role = node.role();
                    let clean = node.clean_title();
                    if node
                        .contract
                        .as_ref()
                        .map(|c| c.input_expected.is_none())
                        .unwrap_or(true)
                    {
                        let mut c = merm_core::engine::NodeContractInfo::default();
                        match role.as_str() {
                            "actor" | "user" => {
                                c.input_expected = Some("Credentials | JWT".to_string());
                                c.output_expected = Some("SessionCookie".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.source_location = Some("client/auth.ts:12".to_string());
                            }
                            "app" | "frontend" => {
                                c.input_expected = Some("HTTP Request".to_string());
                                c.output_expected = Some("HTML / JSON".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.last_duration_ms = 2.4;
                                c.exit_code = 0;
                                c.source_location = Some("frontend/src/App.tsx:25".to_string());
                            }
                            "service" if clean.contains("Auth") => {
                                c.input_expected = Some(
                                    "{\n  \"user\": \"string\",\n  \"pass\": \"string\"\n}"
                                        .to_string(),
                                );
                                c.input_example = Some(
                                    "{\n  \"user\": \"admin\",\n  \"pass\": \"secret\"\n}"
                                        .to_string(),
                                );
                                c.output_expected = Some(
                                    "{\n  \"token\": \"string\",\n  \"expires_at\": \"u64\"\n}"
                                        .to_string(),
                                );
                                c.output_default = Some("\"{}\"".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.last_duration_ms = 1.2;
                                c.exit_code = 0;
                                c.source_location = Some("src/services/auth.rs:42".to_string());
                            }
                            "service" if clean.contains("User") => {
                                c.input_expected = Some("{\n  \"user_id\": \"u64\"\n}".to_string());
                                c.input_example = Some("{\n  \"user_id\": 1001\n}".to_string());
                                c.output_expected = Some(
                                    "{\n  \"username\": \"string\",\n  \"email\": \"string\"\n}"
                                        .to_string(),
                                );
                                c.output_default = Some("\"{}\"".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.last_duration_ms = 0.9;
                                c.exit_code = 0;
                                c.source_location = Some("src/services/user.rs:18".to_string());
                            }
                            "service" if clean.contains("Notification") => {
                                c.input_expected = Some(
                                    "{\n  \"recipient\": \"string\",\n  \"message\": \"string\"\n}"
                                        .to_string(),
                                );
                                c.input_example = Some(
                                    "{\n  \"recipient\": \"admin@example.com\",\n  \"message\": \"System alert\"\n}".to_string(),
                                );
                                c.output_expected = Some(
                                    "{\n  \"status\": \"queued\",\n  \"id\": \"uuid\"\n}"
                                        .to_string(),
                                );
                                c.output_default = Some("\"{}\"".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.last_duration_ms = 1.5;
                                c.exit_code = 0;
                                c.source_location =
                                    Some("src/services/notification.rs:32".to_string());
                            }
                            "service" => {
                                c.input_expected = Some(
                                    "{\n  \"path\": \"string\",\n  \"method\": \"string\"\n}"
                                        .to_string(),
                                );
                                c.input_example = Some(
                                    "{\n  \"path\": \"/api/v1/auth/login\",\n  \"method\": \"POST\"\n}".to_string(),
                                );
                                c.output_expected = Some(
                                    "{\n  \"status\": 200,\n  \"body\": \"string\"\n}".to_string(),
                                );
                                c.output_default = Some("\"{}\"".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.last_duration_ms = 0.8;
                                c.exit_code = 0;
                                c.source_location = Some("src/gateway.rs:18".to_string());
                            }
                            "database" | "db" => {
                                c.input_expected = Some(
                                    r#"{"query": "String", "params": "Vec<Value>"}"#.to_string(),
                                );
                                c.output_expected = Some(r#"{"rows_affected": "u64"}"#.to_string());
                                c.output_default = Some(r#"{"rows_affected": 1}"#.to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.source_location = Some("src/db/postgres.rs:1".to_string());
                            }
                            "cache" => {
                                c.input_expected =
                                    Some(r#"{"key": "String", "ttl_secs": "u32"}"#.to_string());
                                c.output_expected = Some(r#"{"cached": "bool"}"#.to_string());
                                c.output_default = Some(r#"{"cached": true}"#.to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.source_location = Some("src/cache/redis.rs:1".to_string());
                            }
                            "queue" => {
                                c.input_expected =
                                    Some(r#"{"topic": "String", "payload": "Bytes"}"#.to_string());
                                c.output_expected = Some(r#"{"offset": "u64"}"#.to_string());
                                c.output_default = Some(r#"{"offset": 42}"#.to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.source_location = Some("src/mq/kafka.rs:1".to_string());
                            }
                            _ => {
                                c.input_expected = Some("{}".to_string());
                                c.output_expected = Some("1".to_string());
                                c.last_status = Some("🟢 Idle".to_string());
                                c.health = "🟢 Healthy".to_string();
                                c.source_location =
                                    Some(format!("src/{}.rs:1", node.id.to_lowercase()));
                            }
                        }
                        node.contract = Some(c);
                    }
                }

                diagram.regenerate_svg(&self.theme.palette());
                self.transform
                    .fit_to_viewport(diagram.width, diagram.height, 1280.0, 720.0);

                let auth_or_first = diagram
                    .nodes
                    .iter()
                    .find(|n| n.clean_title().contains("Auth"))
                    .or_else(|| diagram.nodes.first())
                    .map(|n| (n.id.clone(), n.label.clone()));

                self.current_diagram = Some(diagram);
                if self.active_node_id.is_none() {
                    if let Some((target_id, target_label)) = auth_or_first {
                        self.select_node(Some(&target_id));
                        self.status_message = format!("Selected: {}", target_label);
                    }
                } else {
                    self.status_message = format!(
                        "Loaded diagram (Backend: {:?})",
                        self.render_engine.active_backend()
                    );
                }
            }
            Err(e) => {
                self.status_message = format!("Error calculating layout: {}", e);
            }
        }
    }

    pub fn show_report(&mut self, text: String) {
        let prev_lines = self
            .report_content
            .as_ref()
            .map(|c| c.lines().count())
            .unwrap_or(0);
        if let Some(ref mut content) = self.report_content {
            content.push_str("\n\n");
            content.push_str(&text);
        } else {
            self.report_content = Some(text);
        }
        self.show_split_buffer = true;
        self.modal.mode = UiMode::Report;
        self.modal.report_scroll_offset = prev_lines;
    }

    pub fn bind_project(&mut self, target_path: Option<&str>) -> Result<(), String> {
        let path = if let Some(p) = target_path {
            PathBuf::from(p)
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let project_root = match target_path {
            Some(p) => PathBuf::from(p),
            None => match ProjectManifest::detect_project_root(&path) {
                Some(r) => r,
                None => return Ok(()),
            },
        };
        let mut manifest =
            ProjectManifest::load_or_init(&project_root).map_err(|e| e.to_string())?;

        // Scan project and auto-bind symbols to manifest
        if let Ok(scan_report) = RustScanner::scan_project(&project_root) {
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
                    input_type: Some("{}".to_string()),
                    output_type: Some("1".to_string()),
                });
            }

            // Ensure all nodes from current diagram are in manifest with default I/O
            if let Some(ref diag) = self.current_diagram {
                for node in &diag.nodes {
                    if !manifest.bindings.contains_key(&node.id) {
                        manifest.add_binding(NodeBinding {
                            id: node.id.clone(),
                            file: format!("src/{}.rs", node.id.to_lowercase()),
                            symbol: node.id.clone(),
                            kind: "struct".to_string(),
                            executable: true,
                            entrypoint: Some("run".to_string()),
                            input_type: Some("{}".to_string()),
                            output_type: Some("1".to_string()),
                        });
                    }
                }
            }

            // Guarantee every binding has non-empty default input and output
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
            let _ = manifest.save();

            let mut verified_on_disk = 0;
            let mut pending_on_disk = 0;
            for binding in manifest.bindings.values() {
                if manifest.project_root.join(&binding.file).is_file() {
                    verified_on_disk += 1;
                } else {
                    pending_on_disk += 1;
                }
            }

            let name = manifest.project_name.clone();
            self.cached_scan_report = RustScanner::scan_project(&manifest.project_root).ok();
            self.manifest = Some(manifest);

            self.status_message = if pending_on_disk > 0 {
                format!(
                    "Bound to '{}' ({} verified on disk, {} pending - run &check to scaffold)",
                    name, verified_on_disk, pending_on_disk
                )
            } else {
                format!(
                    "Bound to '{}' ({} files verified on disk, 100% matched)",
                    name, verified_on_disk
                )
            };

            return Ok(());
        }

        let name = manifest.project_name.clone();
        self.manifest = Some(manifest);
        self.status_message = format!("Bound to project '{}'", name);
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
                if self.manifest.is_none() {
                    let _ = self.bind_project(None);
                }
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
                                self.last_check_report = Some(report);
                                self.show_report(summary);
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
                let prompt = if prompt.trim().is_empty() {
                    if let Some(ref node) = self.active_node_id {
                        format!("Analyze node '{}', its responsibilities, dependencies, and propose modular architectural improvements.", node)
                    } else {
                        "Analyze the architecture diagram, verify domain cohesion and coupling, and recommend architectural improvements.".to_string()
                    }
                } else {
                    prompt
                };
                if self.manifest.is_none() {
                    let _ = self.bind_project(None);
                }
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
                                let content = format!(
                                    "=== Architectural Advice Proposal ===\nQuery: {}\n\n{}\n\nType `&ok` to apply changes.",
                                    proposal.prompt, proposal.analysis
                                );
                                self.pending_advice = Some(proposal);
                                self.show_report(content);
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
                                self.show_report(format!(
                                    "=== Architectural Advice Applied ===\nStatus: SUCCESS\n{}\nType `:w` to persist changes.",
                                    msg
                                ));
                            }
                            Err(e) => {
                                self.status_message = format!("&ok rollback: {}", e);
                                self.show_report(format!(
                                    "=== Architectural Advice Rollback ===\nStatus: FAILED\nRollback occurred: {}",
                                    e
                                ));
                            }
                        }
                    } else if let Some(ref new_diag) = proposal.suggested_diagram {
                        self.diagram_source = new_diag.clone();
                        self.recalculate_diagram();
                        let msg =
                            "&ok: Mermaid diagram updated on canvas from proposal.".to_string();
                        self.status_message = msg.clone();
                        self.pending_advice = None;
                        self.show_report(format!(
                            "=== Architectural Advice Applied ===\nStatus: SUCCESS\n{}",
                            msg
                        ));
                    } else {
                        let msg = "&ok: Advice contained architectural guidance without mutations."
                            .to_string();
                        self.status_message = msg.clone();
                        self.pending_advice = None;
                        self.show_report(format!(
                            "=== Architectural Advice Processed ===\nStatus: SUCCESS\n{}",
                            msg
                        ));
                    }
                } else {
                    let msg = "No pending advice to apply. Run `&advice <QUERY>` or `&agy <QUERY>` first.".to_string();
                    self.status_message = msg.clone();
                    self.show_report(format!("=== Command: &ok ===\nStatus: WARNING\n{}", msg));
                }
            }
            Command::Ai { prompt } => {
                let prompt = if prompt.trim().is_empty() {
                    if let Some(ref node) = self.active_node_id {
                        format!("Scaffold implementation, types, and test harness for node '{}' matching diagram specifications.", node)
                    } else {
                        "Verify diagram nodes against Rust project files and scaffold missing module bindings.".to_string()
                    }
                } else {
                    prompt
                };
                if self.manifest.is_none() {
                    let _ = self.bind_project(None);
                }
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
            Command::Agy { prompt } => {
                let prompt = if prompt.trim().is_empty() {
                    if let Some(ref node) = self.active_node_id {
                        format!("Perform deep architecture review and code quality assessment for node '{}' using Google Antigravity.", node)
                    } else {
                        "Review workspace architecture against clean code principles, test coverage, and scalable design using Google Antigravity.".to_string()
                    }
                } else {
                    prompt
                };
                if self.manifest.is_none() {
                    let _ = self.bind_project(None);
                }
                if let Some(ref manifest) = self.manifest {
                    let mut agy_manifest = manifest.clone();
                    agy_manifest.settings.llm_provider = "agy".to_string();
                    if let Some(ref w) = self.worker {
                        self.is_busy = true;
                        self.busy_message = "Querying Antigravity CLI (agy)...".to_string();
                        self.status_message =
                            format!("Invoking agy in background for '{}'...", prompt);
                        let _ = w.dispatch(WorkerTask::Advice {
                            manifest: agy_manifest,
                            diagram_source: self.diagram_source.clone(),
                            prompt,
                        });
                        return;
                    }
                    self.status_message = format!("Invoking agy for '{}'...", prompt);
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(runtime) = rt {
                        match runtime.block_on(Advisor::request_advice(
                            &agy_manifest,
                            &self.diagram_source,
                            &prompt,
                        )) {
                            Ok(proposal) => {
                                let diag_info = if proposal.suggested_diagram.is_some() {
                                    " [Diagram Update Detected]"
                                } else {
                                    ""
                                };
                                let files_info = if !proposal.suggested_files.is_empty() {
                                    format!(
                                        " [{} File(s) Proposed]",
                                        proposal.suggested_files.len()
                                    )
                                } else {
                                    String::new()
                                };
                                self.status_message = format!(
                                    "&agy advice ready.{}{} Type `&ok` to apply proposal to app.",
                                    diag_info, files_info
                                );
                                let content = format!(
                                    "=== Google Antigravity (AGY) Proposal ===\nQuery: {}\n{}{}\n\n{}\n\nType `&ok` to apply changes.",
                                    proposal.prompt, diag_info, files_info, proposal.analysis
                                );
                                self.pending_advice = Some(proposal);
                                self.show_report(content);
                            }
                            Err(e) => {
                                self.status_message = format!("&agy failed: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message = format!("Invoking agy on diagram for '{}'...", prompt);
                    let agy = AgyLlmProvider::new(None);
                    let system_prompt =
                        "You are an expert systems architect analyzing a Mermaid diagram.\nCRITICAL: DO NOT call any external tools or subagents. Respond directly in text.\nIf you suggest changes or a new architecture, ALWAYS provide the complete updated diagram in a ```mermaid ... ``` code block.";
                    let user_prompt = format!(
                        "Diagram:\n```mermaid\n{}\n```\n\nTask: {}\nProvide clean, architectural recommendations with the updated diagram.",
                        self.diagram_source, prompt
                    );
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    if let Ok(runtime) = rt {
                        match runtime.block_on(agy.query(system_prompt, &user_prompt)) {
                            Ok(ans) => {
                                let suggested_diag = DiagramExtractor::extract(&ans)
                                    .ok()
                                    .and_then(|blocks| blocks.into_iter().next().map(|b| b.source));

                                let diag_info = if suggested_diag.is_some() {
                                    " [Diagram Update Detected]"
                                } else {
                                    ""
                                };

                                self.status_message = format!(
                                    "&agy advice ready.{}. Type `&ok` to apply to canvas.",
                                    diag_info
                                );
                                let content = format!(
                                    "=== Google Antigravity (AGY) Response ===\nQuery: {}\n{}\n\n{}\n\nType `&ok` to apply diagram to canvas.",
                                    prompt, diag_info, ans
                                );
                                self.pending_advice = Some(AdviceProposal {
                                    prompt: prompt.clone(),
                                    analysis: ans,
                                    suggested_files: Vec::new(),
                                    suggested_diagram: suggested_diag,
                                });
                                self.show_report(content);
                            }
                            Err(e) => {
                                self.status_message = format!("&agy failed: {}", e);
                            }
                        }
                    }
                }
            }
            Command::Config { key, value } => {
                if self.manifest.is_none() {
                    let _ = self.bind_project(None);
                }

                if let Some(ref mut manifest) = self.manifest {
                    match (key.as_deref(), value) {
                        (None, _) | (Some("show"), _) | (Some("list"), _) => {
                            let key_status = if let Some(ref k) = manifest.settings.llm_api_key {
                                if k.len() > 8 {
                                    format!(
                                        "{}...{} ({} chars)",
                                        &k[..4],
                                        &k[k.len() - 4..],
                                        k.len()
                                    )
                                } else {
                                    "*** (Configured)".to_string()
                                }
                            } else {
                                "[Not Set] (Set via :config api_key <key> or OPENAI_API_KEY env)"
                                    .to_string()
                            };

                            let report = format!(
                                "=== Project Configuration & LLM Settings ===\n\
                                Project Name: {}\n\
                                Project Root: {}\n\
                                LLM Provider: {}\n\
                                LLM Endpoint: {}\n\
                                LLM Model:    {}\n\
                                LLM API Key:  {}\n\
                                Build Cmd:    {}\n\
                                Test Cmd:     {}\n\n\
                                Supported Providers:\n\
                                  - 'agy' or 'antigravity' (Native Google Antigravity CLI integration)\n\
                                  - 'openai' (Official OpenAI API or OpenAI-compatible endpoint)\n\
                                  - 'ollama' (Local Ollama LLM server)\n\n\
                                Usage:\n\
                                  :config provider <agy|openai|ollama>\n\
                                  :config api_key <YOUR_API_KEY>\n\
                                  :config model <gpt-4o|qwen2.5-coder|inherit>\n\
                                  :config endpoint <https://api.openai.com/v1>\n\
                                  :config build <cargo check>\n\
                                  :config test <cargo test>\n",
                                manifest.project_name,
                                manifest.project_root.display(),
                                manifest.settings.llm_provider,
                                manifest.settings.llm_endpoint,
                                manifest.settings.llm_model,
                                key_status,
                                manifest.settings.build_command,
                                manifest.settings.test_command
                            );
                            self.status_message =
                                format!("Config: provider={}", manifest.settings.llm_provider);
                            self.show_report(report);
                        }
                        (Some("provider"), Some(v)) => {
                            manifest.settings.llm_provider = v.clone();
                            if v == "openai"
                                && manifest.settings.llm_endpoint == "http://localhost:11434/v1"
                            {
                                manifest.settings.llm_endpoint =
                                    "https://api.openai.com/v1".to_string();
                                if manifest.settings.llm_model == "qwen2.5-coder" {
                                    manifest.settings.llm_model = "gpt-4o-mini".to_string();
                                }
                            }
                            let _ = manifest.save();
                            self.status_message = format!("LLM provider set to '{}'", v);
                        }
                        (Some("api_key") | Some("key"), Some(v)) => {
                            manifest.settings.llm_api_key = Some(v);
                            let _ = manifest.save();
                            self.status_message =
                                "OpenAI / LLM API key updated successfully.".to_string();
                        }
                        (Some("model"), Some(v)) => {
                            manifest.settings.llm_model = v.clone();
                            let _ = manifest.save();
                            self.status_message = format!("LLM model set to '{}'", v);
                        }
                        (Some("endpoint") | Some("url"), Some(v)) => {
                            manifest.settings.llm_endpoint = v.clone();
                            let _ = manifest.save();
                            self.status_message = format!("LLM endpoint set to '{}'", v);
                        }
                        (Some("build"), Some(v)) => {
                            manifest.settings.build_command = v.clone();
                            let _ = manifest.save();
                            self.status_message = format!("Build command set to '{}'", v);
                        }
                        (Some("test"), Some(v)) => {
                            manifest.settings.test_command = v.clone();
                            let _ = manifest.save();
                            self.status_message = format!("Test command set to '{}'", v);
                        }
                        (Some(unknown), _) => {
                            self.status_message = format!(
                                "Unknown config key '{}'. Run :config to see options.",
                                unknown
                            );
                        }
                    }
                } else {
                    self.status_message = "Failed to load or bind project manifest.".to_string();
                }
            }
            Command::Add { kind, name } => {
                self.add_node(kind, &name);
            }
            Command::Connect { from, to, label } => {
                self.connect_nodes(&from, &to, label.as_deref());
            }
            Command::Remove { name } => {
                self.remove_node(&name);
            }
            Command::Edit { node_id } => {
                self.open_node_editor(node_id.as_deref());
            }
            Command::Test { node_id, input } => {
                let target_node = node_id.or_else(|| self.active_node_id.clone()).or_else(|| {
                    self.current_diagram
                        .as_ref()
                        .and_then(|d| d.nodes.first().map(|n| n.id.clone()))
                });
                if let Some(nid) = target_node {
                    let input_payload = input.unwrap_or_else(|| "{}".to_string());
                    self.execute_node_test(&nid, &input_payload);
                } else {
                    self.status_message = "No node selected or found for testing.".to_string();
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
  &agy <PROMPT>              - Invoke Google Antigravity CLI (agy) directly
  :config [KEY] [VAL]        - View or update settings (provider, api_key, model, endpoint)
  :add <class|struct|enum> <Name> - Add new node to diagram & scaffold Rust file
  :connect <From> <To> [lbl] - Connect two diagram nodes with arrow
  :test [Node] [Input]       - Execute node test harness with input/output capture
  :perf / :fps               - Toggle live performance telemetry HUD
  :focus                     - Toggle focus mode scrim on selected node
  :u / :undo                 - Undo diagram mutations / node moves
  :redo                      - Redo diagram mutations
  :help                      - Show this command reference

Keybindings (NORMAL mode):
  : / &   - Open Command bar
  t       - Test selected node (opens Input/Output drawer)
  T       - Cycle theme
  u       - Undo last node move / diagram mutation
  Ctrl+R  - Redo last undone mutation
  F       - Toggle Focus Mode on selected node
  a / o   - Add new class / struct
  c       - Connect nodes
  e / E   - Open interactive Node Editor drawer
  g       - Open bound Rust source file in $EDITOR
  h,j,k,l - Pan diagram
  +, -    - Zoom in / out
  Arrows  - Navigate nodes in 2D direction (Right/Left/Down/Up)
  Tab / n - Select next node (N: previous node)
  p       - Pivot direction (TD -> LR -> BT -> RL)
  q / Esc - Quit / close modal
"#
                    .to_string(),
                );
                self.modal.mode = UiMode::Report;
                self.modal.report_scroll_offset = 0;
            }
            Command::Quit => {
                self.is_running = false;
                self.status_message = "Exiting merm...".to_string();
            }
            Command::Save => {
                if let Some(ref mut manifest) = self.manifest {
                    let _ = manifest.save();
                    let mmd_path = manifest.project_root.join("merm.mmd");
                    let _ = std::fs::write(&mmd_path, &self.diagram_source);
                    self.status_message =
                        format!("Saved diagram to {} and manifest.", mmd_path.display());
                } else {
                    let _ = std::fs::write("merm.mmd", &self.diagram_source);
                    self.status_message = "Saved diagram to merm.mmd".to_string();
                }
            }
            Command::SaveAndQuit => {
                if let Some(ref mut manifest) = self.manifest {
                    let _ = manifest.save();
                    let mmd_path = manifest.project_root.join("merm.mmd");
                    let _ = std::fs::write(&mmd_path, &self.diagram_source);
                } else {
                    let _ = std::fs::write("merm.mmd", &self.diagram_source);
                }
                self.is_running = false;
                self.status_message = "Saved and exiting merm...".to_string();
            }
            Command::Theme(name) => {
                let target = match name.trim().to_lowercase().as_str() {
                    "latte" | "light" => Some(ThemeId::CatppuccinLatte),
                    "dracula" => Some(ThemeId::Dracula),
                    "nord" => Some(ThemeId::Nord),
                    "tokyo" | "tokyonight" => Some(ThemeId::TokyoNight),
                    "gruvbox" => Some(ThemeId::GruvboxDark),
                    "monokai" => Some(ThemeId::Monokai),
                    "terminal" => Some(ThemeId::MonokaiTerminal),
                    "mocha" | "catppuccin" | "dark" => Some(ThemeId::CatppuccinMocha),
                    _ => None,
                };
                if let Some(t) = target {
                    self.theme = t;
                    if let Some(ref mut diag) = self.current_diagram {
                        diag.regenerate_svg(&self.theme.palette());
                    } else {
                        self.recalculate_diagram();
                    }
                    self.status_message = format!("Theme set to '{}'", self.theme.palette().name);
                } else {
                    self.status_message = format!(
                        "Unknown theme '{}'. Options: mocha, latte, dracula, nord, tokyo, gruvbox, monokai",
                        name
                    );
                }
            }
            Command::Dir(dir_str) => {
                let target_dir = match dir_str.trim().to_uppercase().as_str() {
                    "TD" | "TB" => Some(LayoutDirection::TD),
                    "LR" => Some(LayoutDirection::LR),
                    "BT" => Some(LayoutDirection::BT),
                    "RL" => Some(LayoutDirection::RL),
                    _ => None,
                };
                if let Some(d) = target_dir {
                    self.diagram_source = AstRewriter::pivot_direction(&self.diagram_source, d);
                    self.active_direction = d;
                    self.recalculate_diagram();
                    self.status_message = format!("Layout direction set to {}", d.as_str());
                } else {
                    self.status_message =
                        format!("Unknown direction '{}'. Options: TD, LR, BT, RL", dir_str);
                }
            }
            Command::Fit => {
                if let Some(ref diag) = self.current_diagram {
                    self.transform
                        .fit_to_viewport(diag.width, diag.height, 1280.0, 720.0);
                    self.status_message =
                        format!("Diagram view fitted (scale: {:.2}x)", self.transform.scale);
                }
            }
            Command::Reset => {
                self.transform = Transform2D::default();
                if let Some(ref diag) = self.current_diagram {
                    self.transform
                        .fit_to_viewport(diag.width, diag.height, 1280.0, 720.0);
                }
                self.status_message = "Reset view transform".to_string();
            }
            Command::Clear => {
                self.report_content = None;
                self.modal.mode = UiMode::Normal;
                self.modal.report_scroll_offset = 0;
                self.status_message = "Cleared report view".to_string();
            }
            Command::ToggleSplit => {
                self.show_split_buffer = !self.show_split_buffer;
                self.status_message = if self.show_split_buffer {
                    "Split buffer opened".to_string()
                } else {
                    "Split buffer closed (maximized diagram)".to_string()
                };
            }
            Command::Copy => {
                self.copy_report_to_clipboard();
            }
            Command::Perf => {
                self.telemetry.toggle();
                self.status_message = format!(
                    "Performance HUD: {}",
                    if self.telemetry.enabled { "ON" } else { "OFF" }
                );
            }
            Command::Undo => {
                self.undo();
            }
            Command::Redo => {
                self.redo();
            }
            Command::Focus => {
                self.toggle_focus_mode();
            }
            Command::Custom(s) => {
                if !s.trim().is_empty() {
                    let prompt = s.trim().to_string();
                    self.execute_command_str(&format!("&advice {}", prompt));
                } else {
                    self.status_message = "Ready".to_string();
                }
            }
        }
    }

    pub fn copy_report_to_clipboard(&mut self) -> bool {
        if let Some(ref text) = self.report_content {
            if text.trim().is_empty() {
                self.status_message = "Split buffer is empty.".to_string();
                return false;
            }
            if copy_to_clipboard(text) {
                let lines = text.lines().count();
                self.status_message = format!("✔ Copied {} lines to system clipboard!", lines);
                true
            } else {
                self.status_message =
                    "Failed to copy to clipboard (check wl-clipboard or xclip).".to_string();
                false
            }
        } else {
            self.status_message = "No split buffer content to copy.".to_string();
            false
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, name: &str) {
        self.diagram_source = Scaffolder::add_node_to_diagram(&self.diagram_source, &kind, name);
        self.recalculate_diagram();

        if let Some(ref mut manifest) = self.manifest {
            match Scaffolder::scaffold_rust_node(manifest, &kind, name) {
                Ok(rel_path) => {
                    let msg = format!("Added node '{}' and scaffolded '{}'", name, rel_path);
                    self.status_message = msg.clone();
                    self.show_report(format!(
                        "=== Added Node ===\nNode: {}\nKind: {:?}\nScaffolded: {}\nStatus: SUCCESS\n{}",
                        name, kind, rel_path, msg
                    ));
                }
                Err(e) => {
                    let msg = format!(
                        "Added node '{}' to diagram (scaffolding error: {})",
                        name, e
                    );
                    self.status_message = msg.clone();
                    self.show_report(format!(
                        "=== Added Node ===\nNode: {}\nKind: {:?}\nStatus: WARNING (Diagram updated, scaffolding failed: {})\n{}",
                        name, kind, e, msg
                    ));
                }
            }
        } else {
            let msg = format!("Added node '{}' to diagram", name);
            self.status_message = msg.clone();
            self.show_report(format!(
                "=== Added Node ===\nNode: {}\nKind: {:?}\nStatus: SUCCESS (Diagram updated, no bound project)\n{}",
                name, kind, msg
            ));
        }
    }

    pub fn connect_nodes(&mut self, from: &str, to: &str, label: Option<&str>) {
        self.diagram_source =
            Scaffolder::connect_nodes_in_diagram(&self.diagram_source, from, to, label);
        self.recalculate_diagram();
        let lbl_info = label.map(|l| format!(" [{}]", l)).unwrap_or_default();
        let msg = format!("Connected {} --> {}{}", from, to, lbl_info);
        self.status_message = msg.clone();
        self.show_report(format!("=== Node Connection ===\nStatus: SUCCESS\n{}", msg));
    }

    pub fn remove_node(&mut self, name: &str) {
        let target = if name.trim().is_empty() {
            self.active_node_id.clone()
        } else {
            Some(name.trim().to_string())
        };

        if let Some(node_name) = target {
            self.diagram_source =
                Scaffolder::remove_node_from_diagram(&self.diagram_source, &node_name);
            self.recalculate_diagram();
            self.active_node_id = None;
            let msg = format!("Removed node '{}' and associated connections.", node_name);
            self.status_message = msg.clone();
            self.show_report(format!(
                "=== Node Removed ===\nNode: {}\nStatus: SUCCESS\n{}",
                node_name, msg
            ));
        } else {
            self.status_message = "No node specified or selected to remove.".to_string();
        }
    }

    pub fn open_node_editor(&mut self, node_id: Option<&str>) {
        let target_id = node_id
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .or_else(|| self.active_node_id.clone())
            .or_else(|| {
                self.current_diagram
                    .as_ref()
                    .and_then(|d| d.nodes.first().map(|n| n.id.clone()))
            });

        if let Some(id) = target_id {
            self.select_node(Some(&id));
            // Find existing node info if available
            let (stereotype, members) = if let Some(ref diag) = self.current_diagram {
                if let Some(node) = diag.nodes.iter().find(|n| n.id == id || n.label == id) {
                    let mut mems = Vec::new();
                    for attr in &node.attributes {
                        let type_str = attr
                            .type_name
                            .as_ref()
                            .map(|t| format!(": {}", t))
                            .unwrap_or_default();
                        mems.push(format!("{}{}{}", attr.visibility, attr.name, type_str));
                    }
                    for meth in &node.methods {
                        let ret_str = meth
                            .type_name
                            .as_ref()
                            .map(|t| format!("() -> {}", t))
                            .unwrap_or_else(|| "()".to_string());
                        mems.push(format!("{}{}{}", meth.visibility, meth.name, ret_str));
                    }
                    (node.stereotype.clone(), mems)
                } else {
                    (None, Vec::new())
                }
            } else {
                (None, Vec::new())
            };

            self.modal.start_node_edit(id.clone(), stereotype, members);
            self.status_message = format!(
                "Editing node '{}' (Enter adds member or saves empty line, Esc cancels)",
                id
            );
        } else {
            self.status_message =
                "No node selected to edit. Select a node or use :edit <NodeName>".to_string();
        }
    }

    pub fn save_node_edit(&mut self, node_id: &str, stereotype: Option<&str>, members: &[String]) {
        self.diagram_source =
            Scaffolder::update_node_in_diagram(&self.diagram_source, node_id, stereotype, members);
        self.recalculate_diagram();
        let msg = format!("Updated node '{}' ({} members).", node_id, members.len());
        self.status_message = msg.clone();
        self.show_report(format!(
            "=== Node Updated ===\nNode: {}\nStereotype: {:?}\nMembers: {}\nStatus: SUCCESS\n{}",
            node_id,
            stereotype,
            members.len(),
            msg
        ));
    }

    pub fn execute_node_test(&mut self, node_id: &str, input: &str) {
        if self.manifest.is_none() {
            let _ = self.bind_project(None);
        }

        let input_payload = if input.trim().is_empty() {
            "{}".to_string()
        } else {
            input.trim().to_string()
        };

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
                    "Executing node '{}' in background (input: '{}')...",
                    node_id, input_payload
                );
                let _ = w.dispatch(WorkerTask::Test {
                    project_root: manifest.project_root.clone(),
                    node_id: node_id.to_string(),
                    file_path: file_path.to_string(),
                    entrypoint: entrypoint.map(|s| s.to_string()),
                    input: input_payload,
                });
                return;
            }

            self.status_message = format!(
                "Executing node '{}' (input: '{}')...",
                node_id, input_payload
            );

            match NodeRunner::execute_node(
                &manifest.project_root,
                node_id,
                file_path,
                entrypoint,
                &input_payload,
            ) {
                Ok(res) => {
                    let status = if res.success { "SUCCESS" } else { "FAILED" };
                    let output_display =
                        if res.output_payload.trim().is_empty() || res.output_payload == "()" {
                            "1".to_string()
                        } else {
                            res.output_payload.clone()
                        };
                    self.status_message = format!(
                        "Node '{}' [{}] ({}ms): output = {}",
                        node_id, status, res.duration_ms, output_display
                    );
                    let report_text = format!(
                        "=== Node Execution Result: {} ===\nStatus: {} (Exit code: {:?})\nDuration: {}ms\n\n[Default Input]:\n{}\n\n[Output Payload]:\n{}\n\n[Stdout]:\n{}\n\n[Stderr]:\n{}",
                        node_id,
                        status,
                        res.exit_code,
                        res.duration_ms,
                        input_payload,
                        output_display,
                        res.stdout,
                        res.stderr
                    );
                    self.last_execution_result = Some(res);
                    self.show_report(report_text);
                }
                Err(e) => {
                    self.status_message = format!("Execution failed for node '{}': {}", node_id, e);
                }
            }
        } else {
            self.status_message =
                "No project bound! Run `&set [PATH]` first to test diagram nodes.".to_string();
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
        match res {
            WorkerResult::Progress {
                phase,
                detail,
                is_tool: _,
            } => {
                self.is_busy = true;
                self.busy_message = format!("⚙️ {} — {}", phase, detail);
                let tool_card = format!(
                    "\n┌─ ⚙️  Tool Call: {} ───────────────────────────────\n\
                    │  Status: ⚡ In Progress...\n\
                    │  Detail: {}\n\
                    └─────────────────────────────────────────────────────────────\n",
                    phase, detail
                );
                if let Some(ref mut content) = self.report_content {
                    content.push_str(&tool_card);
                } else {
                    self.report_content = Some(tool_card);
                }
            }
            WorkerResult::CheckFinished(Ok(report)) => {
                self.is_busy = false;
                self.busy_message.clear();
                let status_str = if report.is_compatible { "PASS" } else { "FAIL" };
                self.status_message = format!("&check completed: Status: {}", status_str);
                let mut text = report.format_text();

                if !report.is_compatible {
                    let missing_nodes: Vec<String> = if let Some(ref diag) = self.current_diagram {
                        if let Some(ref manifest) = self.manifest {
                            if let Ok(scan) = RustScanner::scan_project(&manifest.project_root) {
                                let existing_names: std::collections::HashSet<_> =
                                    scan.symbols.iter().map(|s| s.name.as_str()).collect();
                                diag.nodes
                                    .iter()
                                    .filter(|n| !existing_names.contains(n.id.as_str()))
                                    .map(|n| n.id.clone())
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    let mut suggested_files = Vec::new();
                    for node in &missing_nodes {
                        let code = Scaffolder::generate_rust_module(node, None, &[], &[]);
                        suggested_files.push((format!("src/{}.rs", node.to_lowercase()), code));
                    }

                    let plan_text = format!(
                        "\n\n============================================================\n\
                        💡 Divergence Self-Healing Plan (Ready for `&ok`)\n\
                        ============================================================\n\
                        Mermaid diagram and Rust codebase are out of sync.\n\
                        Missing Rust module(s): {}\n\n\
                        ┌─ ⚙️  Autonomous Action: Scaffolder::generate_rust_module ────\n\
                        │  Modules to scaffold: {}\n\
                        │  Entrypoint: pub fn run(input: &str) -> String\n\
                        │  Default I/O: input = \"{{}}\", output = \"1\"\n\
                        │  Verification Gate: cargo check with atomic rollback\n\
                        └─────────────────────────────────────────────────────────────\n\n\
                        👉 Press 'o' or enter `&ok` to automatically scaffold code and heal divergence!",
                        if missing_nodes.is_empty() {
                            "AST mismatch / compiler diagnostics detected".to_string()
                        } else {
                            missing_nodes.join(", ")
                        },
                        if missing_nodes.is_empty() {
                            "Align AST & fix compiler errors".to_string()
                        } else {
                            missing_nodes.join(", ")
                        }
                    );
                    text.push_str(&plan_text);

                    let proposal = AdviceProposal {
                        prompt: "Auto-heal Mermaid diagram and Rust codebase divergence"
                            .to_string(),
                        analysis: text.clone(),
                        suggested_files,
                        suggested_diagram: None,
                    };
                    self.pending_advice = Some(proposal);
                    self.status_message =
                        "⚠️ Divergence detected! Press 'o' or run '&ok' to heal with AI."
                            .to_string();
                }

                self.last_check_report = Some(report);
                self.show_report(text);
            }
            WorkerResult::CheckFinished(Err(e)) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("&check failed: {}", e);
            }
            WorkerResult::AdviceFinished(Ok(proposal)) => {
                self.is_busy = false;
                self.busy_message.clear();
                let diag_info = if proposal.suggested_diagram.is_some() {
                    " [Diagram Update Detected]"
                } else {
                    ""
                };
                let files_info = if !proposal.suggested_files.is_empty() {
                    format!(" [{} File(s) Proposed]", proposal.suggested_files.len())
                } else {
                    String::new()
                };

                self.status_message = format!(
                    "&advice ready.{}{} Press 'o' or run `&ok` to apply proposal.",
                    diag_info, files_info
                );
                let content = format!(
                    "============================================================\n\
                    🤖 Fabric Architecture Proposal (improve_prompt + task_planner)\n\
                    Query: {}\n\
                    Status: Plan ready{}{}\n\
                    ============================================================\n\n\
                    {}\n\n\
                    ────────────────────────────────────────────────────────────\n\
                    💡 Next Step: Press 'o' or run `&ok` to apply this proposal with atomic rollback!",
                    proposal.prompt, diag_info, files_info, proposal.analysis
                );
                self.pending_advice = Some(proposal);
                self.show_report(content);
            }
            WorkerResult::AdviceFinished(Err(e)) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("&advice failed: {}", e);
            }
            WorkerResult::OkFinished(Ok((msg, new_diag))) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("✔ &ok applied: {}", msg);
                if let Some(d) = new_diag {
                    self.diagram_source = d;
                    self.recalculate_diagram();
                }
                self.pending_advice = None;
                let report_text = format!(
                    "============================================================\n\
                    ✔ Architecture Advice & Mutations Applied Successfully\n\
                    ============================================================\n\
                    {}\n\n\
                    All files verified with `cargo check` under atomic rollback.\n\
                    Type `:w` to persist diagram or `&check` to re-verify.",
                    msg
                );
                self.show_report(report_text);
            }
            WorkerResult::OkFinished(Err(e)) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("&ok rollback: {}", e);
                let report_text = format!(
                    "============================================================\n\
                    ✖ Architecture Advice Failed - Atomic Rollback Triggered\n\
                    ============================================================\n\
                    Error: {}\n\n\
                    All mutated files were safely rolled back to pre-transaction state.",
                    e
                );
                self.show_report(report_text);
            }
            WorkerResult::AiFinished(Ok((explanation, new_diag))) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("&ai completed: {}", explanation);
                if let Some(d) = new_diag {
                    self.diagram_source = d;
                    self.recalculate_diagram();
                }
                let report_text = format!(
                    "============================================================\n\
                    ✔ Autonomous AI Execution Completed\n\
                    ============================================================\n\
                    {}\n\n\
                    Type `:w` to save diagram or `&check` to re-verify.",
                    explanation
                );
                self.show_report(report_text);
            }
            WorkerResult::AiFinished(Err(e)) => {
                self.is_busy = false;
                self.busy_message.clear();
                self.status_message = format!("&ai error: {}", e);
            }
            WorkerResult::TestFinished { node_id, result } => {
                self.is_busy = false;
                self.busy_message.clear();
                match result {
                    Ok(res) => {
                        let status = if res.success { "SUCCESS" } else { "FAILED" };
                        let output_display =
                            if res.output_payload.trim().is_empty() || res.output_payload == "()" {
                                "1".to_string()
                            } else {
                                res.output_payload.clone()
                            };
                        self.status_message = format!(
                            "Node '{}' [{}] ({}ms): output = {}",
                            node_id, status, res.duration_ms, output_display
                        );
                        let report_text = format!(
                            "=== Node Execution Result: {} ===\nStatus: {} (Exit code: {:?})\nDuration: {}ms\n\n[Default Input]:\n{}\n\n[Output Payload]:\n{}\n\n[Stdout]:\n{}\n\n[Stderr]:\n{}",
                            node_id,
                            status,
                            res.exit_code,
                            res.duration_ms,
                            "{}",
                            output_display,
                            res.stdout,
                            res.stderr
                        );
                        self.last_execution_result = Some(res);
                        self.show_report(report_text);
                    }
                    Err(e) => {
                        self.status_message =
                            format!("Execution failed for node '{}': {}", node_id, e);
                    }
                }
            }
        }
    }

    pub fn handle_source_files_changed(&mut self, paths: &[PathBuf]) {
        if let Some(ref manifest) = self.manifest {
            log::info!("Live reload: {} source files changed", paths.len());
            let mut scan_report = if let Some(ref cached) = self.cached_scan_report {
                cached.clone()
            } else {
                RustScanner::scan_project(&manifest.project_root).unwrap_or_default()
            };

            // Incremental update: rescan only changed files
            for path in paths {
                let rel_path = path
                    .strip_prefix(&manifest.project_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                // Remove previous symbols from this file
                scan_report.symbols.retain(|s| s.file_path != rel_path);

                if path.is_file() {
                    if let Ok(mut new_syms) =
                        RustScanner::scan_single_file(path, &manifest.project_root)
                    {
                        scan_report.symbols.append(&mut new_syms);
                    }
                    if !scan_report.files.contains(&rel_path) {
                        scan_report.files.push(rel_path);
                    }
                } else {
                    // File was deleted
                    scan_report.files.retain(|f| f != &rel_path);
                }
            }

            self.cached_scan_report = Some(scan_report.clone());

            let code_graph = ArchitectureGraph::from_project_symbols(&scan_report);
            let diag_graph = ArchitectureGraph::from_mermaid_source(&self.diagram_source);
            let reconcil = ReconciliationEngine::reconcile(&diag_graph, &code_graph);

            if reconcil.is_synchronized() {
                self.status_message = format!(
                    "Live sync (incremental): {} file(s) updated. Architecture synchronized ({} nodes).",
                    paths.len(),
                    reconcil.matched_nodes.len()
                );
            } else {
                let div_count = reconcil.divergences.len() + reconcil.diagram_only_nodes.len();
                self.status_message = format!(
                    "Live sync (incremental): {} file(s) updated. Divergence detected ({} nodes)! Press 'i' or run &check.",
                    paths.len(),
                    div_count
                );
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
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id));
                        let next_idx = match cur_idx {
                            Some(idx) => (idx + 1) % diag.nodes.len(),
                            None => 0,
                        };
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
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id));
                        let prev_idx = match cur_idx {
                            Some(0) => diag.nodes.len() - 1,
                            Some(idx) => idx - 1,
                            None => diag.nodes.len().saturating_sub(1),
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
            UiAction::DeleteSelectedNode => {
                self.remove_node("");
            }
            UiAction::OpenNodeEditor(id) => {
                self.open_node_editor(if id.is_empty() { None } else { Some(&id) });
            }
            UiAction::SaveNodeEdit {
                node_id,
                stereotype,
                members,
            } => {
                self.save_node_edit(&node_id, stereotype.as_deref(), &members);
            }
            UiAction::Undo => {
                self.undo();
            }
            UiAction::Redo => {
                self.redo();
            }
            UiAction::TogglePerf => {
                self.telemetry.toggle();
                self.status_message = format!(
                    "Performance HUD: {}",
                    if self.telemetry.enabled { "ON" } else { "OFF" }
                );
            }
            UiAction::ToggleFocus => {
                self.toggle_focus_mode();
            }
            UiAction::SetMode(_) | UiAction::None => {}
        }
    }

    pub fn move_node_preview(&mut self, node_idx: usize, new_x: f32, new_y: f32) {
        self.active_drag_preview = Some((node_idx, new_x, new_y));
    }

    pub fn finalize_node_move(&mut self, node_idx: usize, new_x: f32, new_y: f32) {
        self.active_drag_preview = None;
        if let Some(ref mut diag) = self.current_diagram {
            if node_idx < diag.nodes.len() {
                let node_id = diag.nodes[node_idx].id.clone();
                let old_x = diag.nodes[node_idx].x;
                let old_y = diag.nodes[node_idx].y;

                diag.nodes[node_idx].x = new_x;
                diag.nodes[node_idx].y = new_y;
                diag.regenerate_svg(&self.theme.palette());

                if (old_x - new_x).abs() > 1.0 || (old_y - new_y).abs() > 1.0 {
                    self.undo_stack.push(GraphMutationDelta::MoveNode {
                        node_id,
                        old_x,
                        old_y,
                        new_x,
                        new_y,
                    });
                }
            }
        }
    }

    pub fn move_node(&mut self, node_idx: usize, new_x: f32, new_y: f32) {
        self.finalize_node_move(node_idx, new_x, new_y);
    }

    pub fn select_node(&mut self, node_id: Option<&str>) {
        if let Some(ref mut diag) = self.current_diagram {
            diag.selected_node_id = node_id.map(|s| s.to_string());
        }
        self.active_node_id = node_id.map(|s| s.to_string());
        self.modal.active_test_node_id = node_id.map(|s| s.to_string());
    }

    pub fn undo(&mut self) {
        if let Some(inverted) = self.undo_stack.undo() {
            match inverted {
                GraphMutationDelta::MoveNode {
                    node_id,
                    new_x,
                    new_y,
                    ..
                } => {
                    if let Some(ref mut diag) = self.current_diagram {
                        if let Some(node) = diag.nodes.iter_mut().find(|n| n.id == node_id) {
                            node.x = new_x;
                            node.y = new_y;
                        }
                        diag.regenerate_svg(&self.theme.palette());
                    }
                    self.status_message = format!("Undo: Restored '{}' position", node_id);
                }
                GraphMutationDelta::DiagramSourceChange {
                    source_after,
                    description,
                    ..
                } => {
                    self.diagram_source = source_after;
                    self.recalculate_diagram();
                    self.status_message = format!("Undo: {}", description);
                }
            }
        } else {
            self.status_message = "Already at oldest change (cannot undo)".to_string();
        }
    }

    pub fn redo(&mut self) {
        if let Some(delta) = self.undo_stack.redo() {
            match delta {
                GraphMutationDelta::MoveNode {
                    node_id,
                    new_x,
                    new_y,
                    ..
                } => {
                    if let Some(ref mut diag) = self.current_diagram {
                        if let Some(node) = diag.nodes.iter_mut().find(|n| n.id == node_id) {
                            node.x = new_x;
                            node.y = new_y;
                        }
                        diag.regenerate_svg(&self.theme.palette());
                    }
                    self.status_message = format!("Redo: Moved '{}' position", node_id);
                }
                GraphMutationDelta::DiagramSourceChange {
                    source_after,
                    description,
                    ..
                } => {
                    self.diagram_source = source_after;
                    self.recalculate_diagram();
                    self.status_message = format!("Redo: {}", description);
                }
            }
        } else {
            self.status_message = "Already at newest change (cannot redo)".to_string();
        }
    }

    pub fn toggle_focus_mode(&mut self) {
        self.focus_mode_active = !self.focus_mode_active;
        if self.focus_mode_active {
            let name = self.active_node_id.as_deref().unwrap_or("None");
            self.status_message = format!("Architecture Focus Mode: ON (Focused on <{}>)", name);
        } else {
            self.status_message = "Architecture Focus Mode: OFF".to_string();
        }
    }

    pub fn select_directional_node(&mut self, dx: f32, dy: f32) {
        let (target_id, target_label) = {
            let diag = match self.current_diagram.as_ref() {
                Some(d) if !d.nodes.is_empty() => d,
                _ => return,
            };

            let cur = self
                .active_node_id
                .as_ref()
                .and_then(|id| diag.nodes.iter().find(|n| &n.id == id));

            if let Some(cur) = cur {
                let mut best_node: Option<&merm_core::engine::DiagramNode> = None;
                let mut best_score = f32::MAX;

                for node in &diag.nodes {
                    if node.id == cur.id {
                        continue;
                    }
                    let diff_x = node.x - cur.x;
                    let diff_y = node.y - cur.y;

                    let in_direction = if dx > 0.0 {
                        diff_x > 15.0
                    } else if dx < 0.0 {
                        diff_x < -15.0
                    } else if dy > 0.0 {
                        diff_y > 15.0
                    } else if dy < 0.0 {
                        diff_y < -15.0
                    } else {
                        false
                    };

                    if in_direction {
                        let score = if dx != 0.0 {
                            diff_x.abs() + 2.5 * diff_y.abs()
                        } else {
                            diff_y.abs() + 2.5 * diff_x.abs()
                        };
                        if score < best_score {
                            best_score = score;
                            best_node = Some(node);
                        }
                    }
                }

                // If no node in that direction, cycle sequentially
                let picked = best_node.or_else(|| {
                    let cur_idx = diag.nodes.iter().position(|n| n.id == cur.id).unwrap_or(0);
                    if dx > 0.0 || dy > 0.0 {
                        let next_idx = (cur_idx + 1) % diag.nodes.len();
                        Some(&diag.nodes[next_idx])
                    } else {
                        let prev_idx = if cur_idx == 0 {
                            diag.nodes.len() - 1
                        } else {
                            cur_idx - 1
                        };
                        Some(&diag.nodes[prev_idx])
                    }
                });

                match picked {
                    Some(n) => (n.id.clone(), n.label.clone()),
                    None => return,
                }
            } else {
                // If none is selected, ANY arrow key immediately selects the very first node!
                let first = &diag.nodes[0];
                (first.id.clone(), first.label.clone())
            }
        };

        self.select_node(Some(&target_id));
        self.status_message = format!("Selected: {}", target_label);
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

    pub fn toggle_sidebar(&mut self) {
        self.show_left_sidebar = !self.show_left_sidebar;
        self.status_message = if self.show_left_sidebar {
            "Navigation sidebar expanded".to_string()
        } else {
            "Navigation sidebar collapsed".to_string()
        };
    }

    pub fn toggle_right_panel(&mut self) {
        self.show_right_panel = !self.show_right_panel;
        self.status_message = if self.show_right_panel {
            "Inspector drawer opened".to_string()
        } else {
            "Inspector drawer collapsed".to_string()
        };
    }

    pub fn set_sidebar_tab(&mut self, tab: SidebarTab) {
        self.active_sidebar_tab = tab;
        self.status_message = format!("View: {:?}", tab);
    }

    pub fn set_right_panel_tab(&mut self, tab: RightPanelTab) {
        self.right_panel_tab = tab;
        self.status_message = format!("Inspector tab: {:?}", tab);
    }

    pub fn set_layout_algorithm(&mut self, algo: LayoutAlgorithm) {
        self.layout_algorithm = algo;
        match algo {
            LayoutAlgorithm::Hierarchical => {
                self.active_direction = LayoutDirection::TD;
                self.diagram_source =
                    AstRewriter::pivot_direction(&self.diagram_source, LayoutDirection::TD);
            }
            LayoutAlgorithm::ForceDirected => {
                self.active_direction = LayoutDirection::LR;
                self.diagram_source =
                    AstRewriter::pivot_direction(&self.diagram_source, LayoutDirection::LR);
            }
            LayoutAlgorithm::Grid => {
                self.active_direction = LayoutDirection::BT;
                self.diagram_source =
                    AstRewriter::pivot_direction(&self.diagram_source, LayoutDirection::BT);
            }
        }
        self.recalculate_diagram();
        self.status_message = format!("Layout algorithm: {:?}", algo);
    }

    pub fn set_active_tool(&mut self, tool: CanvasTool) {
        self.active_tool = tool;
        match tool {
            CanvasTool::Fit => {
                if let Some(ref diag) = self.current_diagram {
                    self.transform
                        .fit_to_viewport(diag.width, diag.height, 1280.0, 720.0);
                }
            }
            CanvasTool::Fullscreen => {
                self.show_left_sidebar = !self.show_left_sidebar;
                self.show_right_panel = !self.show_right_panel;
            }
            _ => {}
        }
        self.status_message = format!("Active tool: {:?}", tool);
    }

    pub fn select_workspace(&mut self, ws: &str) {
        self.active_workspace = ws.to_string();
        self.status_message = format!("Active workspace: {}", ws);
    }

    pub fn open_code_editor_for_node(&mut self, node_id: &str) {
        self.active_sidebar_tab = SidebarTab::AstView;
        self.selected_ast_symbol = Some(node_id.to_string());
        if let Some(ref manifest) = self.manifest {
            if let Some(binding) = manifest.get_binding(node_id) {
                self.active_code_file = binding.file.clone();
                let full_path = manifest.project_root.join(&binding.file);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    self.active_code_content = content;
                    self.status_message = format!("Opened source: {}", binding.file);
                    return;
                }
            }
        }
        self.active_code_file = format!(
            "src/services/{}.rs",
            node_id.to_lowercase().replace(' ', "_")
        );
        self.status_message = format!("Viewing AST & code for '{}'", node_id);
    }

    pub fn open_command_palette(&mut self) {
        self.command_palette_visible = true;
        self.command_palette_query.clear();
        self.command_palette_selected_idx = 0;
        self.modal.mode = UiMode::Command;
        self.status_message = "Command Palette: Type a command or press Enter".to_string();
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette_visible = false;
        self.modal.mode = UiMode::Normal;
        self.status_message = "Ready".to_string();
    }
}
