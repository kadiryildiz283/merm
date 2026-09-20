use merm_core::{ClassDiagramParser, LayoutDirection, LayoutEngine};

#[test]
fn test_class_diagram_parsing_and_rendering() {
    let source = r#"
classDiagram
    direction LR
    class BankAccount {
        +String owner
        -BigDecimal balance
        +deposit(amount) bool
        +withdraw(amount) bool
    }
    class CheckingAccount {
        +BigDecimal overdraftLimit
        +processCheck()
    }
    BankAccount <|-- CheckingAccount : inherits
    BankAccount *-- Transaction : contains
"#;

    assert!(ClassDiagramParser::is_class_diagram(source));

    let engine = LayoutEngine::default();
    let rendered = engine.render_with_watchdog(source).expect("Must render class diagram");

    // Check SVG contents
    assert!(rendered.svg.contains("<svg"));
    assert!(rendered.svg.contains("BankAccount"));
    assert!(rendered.svg.contains("CheckingAccount"));
    assert!(rendered.svg.contains("Transaction"));
    assert!(rendered.svg.contains("deposit(amount)"));
    assert!(rendered.svg.contains("bool"));
    assert!(rendered.svg.contains("overdraftLimit"));

    // Check UML markers
    assert!(rendered.svg.contains(r##"id="inheritance""##));
    assert!(rendered.svg.contains(r##"id="composition""##));
    assert!(rendered.svg.contains("inherits"));
    assert!(rendered.svg.contains("contains"));

    // Check nodes extracted
    assert_eq!(rendered.nodes.len(), 3);
    assert!(rendered.nodes.iter().any(|n| n.id == "BankAccount"));
    assert!(rendered.nodes.iter().any(|n| n.id == "CheckingAccount"));
    assert!(rendered.nodes.iter().any(|n| n.id == "Transaction"));
}

#[test]
fn test_class_diagram_direction_override() {
    let source = r#"
classDiagram
    class Animal {
        +eat()
    }
    class Duck {
        +quack()
    }
    Animal <|-- Duck
"#;

    let rendered_td = ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::TD), None).unwrap();
    let rendered_lr = ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::LR), None).unwrap();

    assert!(rendered_td.width > 0.0);
    assert!(rendered_lr.width > 0.0);
}

#[test]
fn test_class_diagram_comments_and_syntax_colors() {
    let source = r#"
classDiagram
    class PaymentProcessor {
        %% Core financial processor
        <<interface>>
        +String apiKey // API authentication key
        -SecretKey secretKey %% Private cryptographic key
        +processPayment(amount) bool // Executes external payment
        #validateToken(token) bool
    }
"#;

    let engine = LayoutEngine::default();
    let rendered = engine.render_with_watchdog(source).expect("Must render payment processor");

    // Verify SVG contents
    assert!(rendered.svg.contains("PaymentProcessor"));
    assert!(rendered.svg.contains("Core financial processor"));
    assert!(rendered.svg.contains("«interface»"));
    assert!(rendered.svg.contains("apiKey"));
    assert!(rendered.svg.contains("API authentication key"));
    assert!(rendered.svg.contains("secretKey"));
    assert!(rendered.svg.contains("Private cryptographic key"));
    assert!(rendered.svg.contains("processPayment(amount)"));
    assert!(rendered.svg.contains("Executes external payment"));

    // Verify DiagramNode metadata
    assert_eq!(rendered.nodes.len(), 1);
    let node = &rendered.nodes[0];
    assert_eq!(node.id, "PaymentProcessor");
    assert_eq!(node.stereotype.as_deref(), Some("interface"));
    assert_eq!(node.doc_comment.as_deref(), Some("Core financial processor"));
    assert_eq!(node.attributes.len(), 2);
    assert_eq!(node.methods.len(), 2);
    assert_eq!(node.attributes[0].comment.as_deref(), Some("API authentication key"));
    assert_eq!(node.attributes[1].comment.as_deref(), Some("Private cryptographic key"));
    assert_eq!(node.methods[0].comment.as_deref(), Some("Executes external payment"));
}
