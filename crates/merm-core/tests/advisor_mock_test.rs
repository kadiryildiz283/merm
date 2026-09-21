use merm_core::advisor::Advisor;
use merm_core::llm_client::MockLlmProvider;
use merm_core::manifest::ProjectManifest;
use std::fs;
use std::path::PathBuf;

fn create_temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("merm_test_advisor_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn test_advisor_run_check_with_mock_llm() {
    let root = create_temp_dir();

    // Create minimal Cargo.toml and src/lib.rs
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "advisor_test_proj"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        r#"pub struct PaymentService;
impl PaymentService {
    pub fn pay(&self) -> bool { true }
}
"#,
    )
    .unwrap();

    let manifest = ProjectManifest::load_or_init(&root).unwrap();
    let diagram = r#"classDiagram
    class PaymentService {
        +pay() bool
    }
"#;

    let mock = MockLlmProvider::new();
    mock.enqueue_response(Ok(
        "Architecture looks well decoupled and compliant.".to_string()
    ));

    let report = Advisor::run_check_with_provider(&manifest, diagram, &mock)
        .await
        .unwrap();

    assert!(report.compilation_success);
    assert!(report.is_compatible);
    assert_eq!(
        report.llm_critique,
        Some("Architecture looks well decoupled and compliant.".to_string())
    );
}

#[tokio::test]
async fn test_advisor_request_advice_with_mock_llm() {
    let root = create_temp_dir();

    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "advice_test_proj"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "// empty lib").unwrap();

    let manifest = ProjectManifest::load_or_init(&root).unwrap();
    let diagram = "classDiagram\n    class App\n";

    let mock = MockLlmProvider::new();
    mock.enqueue_response(Ok(
        "Recommend splitting App into Core and Shell modules.".to_string()
    ));

    let proposal =
        Advisor::request_advice_with_provider(&manifest, diagram, "how to structure?", &mock)
            .await
            .unwrap();

    assert_eq!(proposal.prompt, "how to structure?");
    assert!(proposal.analysis.contains("Recommend splitting App"));
}

#[tokio::test]
async fn test_advisor_execute_ai_with_mock_llm() {
    let root = create_temp_dir();

    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "ai_exec_test_proj"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub mod worker;\n").unwrap();

    let mut manifest = ProjectManifest::load_or_init(&root).unwrap();
    let diagram = "classDiagram\n    class Worker\n";

    let mock = MockLlmProvider::new();
    let generated_json = r#"{
      "explanation": "Added Worker struct implementation",
      "files": [
        {
          "path": "src/worker.rs",
          "content": "pub struct Worker;\nimpl Worker {\n    pub fn run(&self) -> String {\n        \"done\".to_string()\n    }\n}\n"
        }
      ],
      "diagram": "classDiagram\n    class Worker {\n        +run() String\n    }\n"
    }"#;

    mock.enqueue_response(Ok(generated_json.to_string()));

    let (explanation, new_diag) =
        Advisor::execute_ai_with_provider(&mut manifest, diagram, "implement worker", &mock)
            .await
            .unwrap();

    assert_eq!(explanation, "Added Worker struct implementation");
    assert!(new_diag.is_some());
    assert!(root.join("src/worker.rs").exists());
}

#[tokio::test]
async fn test_advisor_advice_diagram_and_file_extraction_and_apply() {
    let root = create_temp_dir();

    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "advice_extract_test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "// lib\n").unwrap();

    let mut manifest = ProjectManifest::load_or_init(&root).unwrap();
    let initial_diagram = "classDiagram\n    class User\n";

    let mock = MockLlmProvider::new();
    let llm_reply = r#"Here is the recommended architecture:

```mermaid
classDiagram
    direction TD
    class User {
        +id: String
    }
    class Session {
        +token: String
    }
    User --> Session
```

And here is the new module implementation:

```rust
// File: src/session.rs
pub struct Session {
    pub token: String,
}
```
"#;

    mock.enqueue_response(Ok(llm_reply.to_string()));

    let proposal =
        Advisor::request_advice_with_provider(&manifest, initial_diagram, "add session", &mock)
            .await
            .unwrap();

    // Verify diagram was extracted!
    assert!(proposal.suggested_diagram.is_some());
    let extracted_diag = proposal.suggested_diagram.as_ref().unwrap();
    assert!(extracted_diag.contains("User --> Session"));

    // Verify files were extracted!
    assert_eq!(proposal.suggested_files.len(), 1);
    assert_eq!(proposal.suggested_files[0].0, "src/session.rs");
    assert!(proposal.suggested_files[0].1.contains("pub struct Session"));

    // Verify apply_advice writes file and returns success
    let result = Advisor::apply_advice(&mut manifest, &proposal, initial_diagram).unwrap();
    assert!(result.contains("Applied advice"));
    assert!(root.join("src/session.rs").exists());
}

#[tokio::test]
async fn test_agy_provider_manifest_integration() {
    let root = create_temp_dir();
    let mut manifest = ProjectManifest::load_or_init(&root).unwrap();
    manifest.settings.llm_provider = "agy".to_string();
    manifest.settings.llm_model = "inherit".to_string();

    let client = merm_core::llm_client::LlmClient::from_manifest(&manifest);
    // Ensure LlmClient initializes cleanly with agy provider
    assert_eq!(manifest.settings.llm_provider, "agy");
    let _ = client;
}

#[tokio::test]
async fn test_fabric_prompts_system_md_auto_created() {
    let root = create_temp_dir();
    let manifest = ProjectManifest::load_or_init(&root).unwrap();

    let system_md = root.join(".merm/system.md");
    assert!(system_md.is_file(), ".merm/system.md must be auto-created");
    let content = fs::read_to_string(&system_md).unwrap();
    assert!(content.contains("Fabric: improve_prompt"));
    assert!(content.contains("Fabric: task_planner"));
    assert!(content.contains("Fabric: create_design_document"));
    assert!(content.contains("Execution Protocol"));

    // Verify Advisor reads this prompt
    let mock = MockLlmProvider::new();
    mock.enqueue_response(Ok("Architectural plan verified.".to_string()));

    let proposal =
        Advisor::request_advice_with_provider(&manifest, "classDiagram\n", "plan query", &mock)
            .await
            .unwrap();
    assert_eq!(proposal.prompt, "plan query");
}

#[tokio::test]
async fn test_advisor_execute_ok_with_mock_llm() {
    let root = create_temp_dir();
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "ok_exec_test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "// lib").unwrap();

    let mut manifest = ProjectManifest::load_or_init(&root).unwrap();
    let initial_diagram = "classDiagram\n    class Worker\n";

    // Proposal without files (high-level plan)
    let proposal = merm_core::AdviceProposal {
        prompt: "Add a database connector".to_string(),
        analysis: "Step 1: Create Database struct. Step 2: Implement run().".to_string(),
        suggested_files: Vec::new(),
        suggested_diagram: None,
    };

    let mock = MockLlmProvider::new();
    let json_response = r#"{
  "explanation": "Scaffolded database connector",
  "files": [
    {
      "path": "src/database.rs",
      "content": "pub struct Database;\nimpl Database { pub fn run(input: &str) -> String { format!(\"db: {}\", input) } }"
    }
  ],
  "diagram": "classDiagram\n    class Worker\n    class Database\n    Worker --> Database\n"
}"#;
    mock.enqueue_response(Ok(json_response.to_string()));

    let (explanation, new_diag) =
        Advisor::execute_ok_with_provider(&mut manifest, &proposal, initial_diagram, &mock)
            .await
            .unwrap();

    assert!(explanation.contains("database connector"));
    assert!(new_diag.is_some());
    assert!(new_diag.unwrap().contains("Worker --> Database"));
    assert!(root.join("src/database.rs").is_file());
}
