use merm_core::{
    Command, NodeBinding, NodeKind, ProjectManifest, RustScanner, Scaffolder, TransactionSnapshot,
};
use std::fs;
use std::path::PathBuf;

fn create_mock_project() -> PathBuf {
    let temp = std::env::temp_dir().join(format!("merm_mock_proj_{}", uuid::Uuid::new_v4()));
    let src = temp.join("src");
    fs::create_dir_all(&src).unwrap();

    let cargo_toml = r#"[package]
name = "mock-erp"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    fs::write(temp.join("Cargo.toml"), cargo_toml).unwrap();

    let lib_code = r#"//! Mock ERP core library

pub mod auth;
pub mod order;

pub fn run(input: &str) -> String {
    format!("mock-erp root received: {}", input)
}
"#;
    fs::write(src.join("lib.rs"), lib_code).unwrap();

    let auth_code = r#"//! Authentication module

pub struct AuthService {
    pub session_id: String,
    secret_key: String,
}

impl AuthService {
    pub fn run(input: &str) -> String {
        format!("AuthService processed login for: {}", input)
    }

    pub fn logout(&self) -> bool {
        true
    }
}
"#;
    fs::write(src.join("auth.rs"), auth_code).unwrap();

    let order_code = r#"//! Order management

pub enum OrderStatus {
    Pending,
    Shipped,
    Delivered,
}

impl OrderStatus {
    pub fn run(input: &str) -> String {
        format!("OrderStatus evaluated: {}", input)
    }
}
"#;
    fs::write(src.join("order.rs"), order_code).unwrap();

    temp
}

#[test]
fn test_project_manifest_and_scanning() {
    let proj = create_mock_project();

    // 1. Scan project
    let scan = RustScanner::scan_project(&proj).unwrap();
    assert_eq!(scan.symbols.len(), 3); // run (lib.rs), AuthService (auth.rs), OrderStatus (order.rs)

    let auth = scan.find_symbol("AuthService").unwrap();
    assert_eq!(auth.methods.len(), 2);
    assert!(auth.is_executable);
    assert_eq!(auth.primary_entrypoint.as_deref(), Some("run"));

    // 2. Generate Mermaid diagram
    let mermaid = RustScanner::generate_mermaid_class_diagram(&scan);
    assert!(mermaid.contains("class AuthService"));
    assert!(mermaid.contains("class OrderStatus"));
    assert!(mermaid.contains("<<struct>>"));
    assert!(mermaid.contains("<<enum>>"));

    // 3. Verify compatibility
    let compat = RustScanner::verify_diagram_compatibility(&mermaid, &scan);
    assert!(compat.is_compatible);
    assert_eq!(compat.matched_classes, 3);
    assert!(compat.missing_in_rust.is_empty());

    // 4. Manifest lifecycle
    let mut manifest = ProjectManifest::load_or_init(&proj).unwrap();
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

    let reloaded = ProjectManifest::load_or_init(&proj).unwrap();
    assert!(reloaded.get_binding("AuthService").is_some());

    let _ = fs::remove_dir_all(&proj);
}

#[test]
fn test_scaffolding_and_connection() {
    let proj = create_mock_project();
    let mut manifest = ProjectManifest::load_or_init(&proj).unwrap();

    let initial_diagram = "classDiagram\n    direction TD\n";

    // Add node
    let with_node =
        Scaffolder::add_node_to_diagram(initial_diagram, &NodeKind::Struct, "PaymentEngine");
    assert!(with_node.contains("class PaymentEngine"));

    let rel_file =
        Scaffolder::scaffold_rust_node(&mut manifest, &NodeKind::Struct, "PaymentEngine").unwrap();
    assert_eq!(rel_file, "src/payment_engine.rs");
    assert!(proj.join("src/payment_engine.rs").is_file());

    // Connect nodes
    let connected = Scaffolder::connect_nodes_in_diagram(
        &with_node,
        "AuthService",
        "PaymentEngine",
        Some("authorizes"),
    );
    assert!(connected.contains("AuthService --> PaymentEngine : authorizes"));

    let _ = fs::remove_dir_all(&proj);
}

#[test]
fn test_transaction_snapshot_and_rollback() {
    let proj = create_mock_project();

    let original_content = fs::read_to_string(proj.join("src/auth.rs")).unwrap();

    // Create snapshot
    let snapshot =
        TransactionSnapshot::create(&proj, &["src/auth.rs".to_string()], "classDiagram\n").unwrap();

    // Mutate file
    fs::write(proj.join("src/auth.rs"), "// corrupted content").unwrap();
    assert_eq!(
        fs::read_to_string(proj.join("src/auth.rs")).unwrap(),
        "// corrupted content"
    );

    // Rollback
    snapshot.rollback().unwrap();
    assert_eq!(
        fs::read_to_string(proj.join("src/auth.rs")).unwrap(),
        original_content
    );

    let _ = fs::remove_dir_all(&proj);
}

#[test]
fn test_command_parser_suite() {
    assert_eq!(
        Command::parse("&set ."),
        Command::Set {
            path: Some(".".to_string())
        }
    );
    assert_eq!(Command::parse("&check"), Command::Check);
    assert_eq!(Command::parse("&ok"), Command::Ok);
    assert_eq!(
        Command::parse("&advice optimize relations"),
        Command::Advice {
            prompt: "optimize relations".to_string()
        }
    );
    assert_eq!(
        Command::parse("&ai add telemetry module"),
        Command::Ai {
            prompt: "add telemetry module".to_string()
        }
    );
    assert_eq!(
        Command::parse(":test AuthService {\"test\":1}"),
        Command::Test {
            node_id: Some("AuthService".to_string()),
            input: Some("{\"test\":1}".to_string()),
        }
    );
}
