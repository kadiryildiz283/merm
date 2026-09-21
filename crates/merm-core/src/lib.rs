pub mod advisor;
pub mod architecture_graph;
pub mod ast_rewriter;
pub mod class_diagram;
pub mod command;
pub mod engine;
pub mod error;
pub mod extractor;
pub mod fabric_prompts;
pub mod language_adapter;
pub mod llm_client;
pub mod manifest;
pub mod node_runner;
pub mod quickjs_engine;
pub mod rust_scanner;
pub mod scaffolding;
pub mod theme;
pub mod transactions;
pub mod xml_utils;

pub use advisor::{AdviceProposal, Advisor, CheckReport};
pub use architecture_graph::{
    ArchEdge, ArchMember, ArchNode, ArchitectureGraph, Divergence, ReconciliationEngine,
    ReconciliationReport,
};
pub use ast_rewriter::{AstRewriter, LayoutDirection};
pub use class_diagram::{ClassDef, ClassDiagramParser, ClassRelation};
pub use command::{Command, NodeKind};
pub use engine::{
    ClassMemberInfo, DiagramNode, FlowEdge, LayoutEngine, RelationKind, RenderedDiagram,
};
pub use error::CoreError;
pub use extractor::{DiagramBlock, DiagramExtractor, DiagramType};
pub use fabric_prompts::{
    ADVICE_COMPOSITE_SYSTEM, CREATE_DESIGN_DOCUMENT_SYSTEM, IMPROVE_PROMPT_SYSTEM,
    OK_EXECUTION_SYSTEM, SYSTEM_MD_TEMPLATE, TASK_PLANNER_SYSTEM,
};
pub use language_adapter::{BuildStatus, LanguageAdapter, RustAdapter};
pub use llm_client::{AgyLlmProvider, HttpLlmProvider, LlmClient, LlmProvider, MockLlmProvider};
pub use manifest::{NodeBinding, ProjectManifest, ProjectSettings, MANIFEST_FILE, MERM_DIR};
pub use node_runner::{ExecutionResult, NodeRunner};
pub use quickjs_engine::QuickJsEngine;
pub use rust_scanner::{
    CompatibilityReport, ProjectScanReport, RustFieldInfo, RustMethodInfo, RustScanner, RustSymbol,
    RustSymbolKind,
};
pub use scaffolding::Scaffolder;
pub use theme::{ColorPalette, ThemeId};
pub use transactions::TransactionSnapshot;
pub use xml_utils::{escape_xml, split_and_escape_lines};
