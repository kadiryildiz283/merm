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
    assert!(rendered.svg.contains("deposit(amount) bool"));
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

    let rendered_td = ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::TD)).unwrap();
    let rendered_lr = ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::LR)).unwrap();

    assert!(rendered_td.width > 0.0);
    assert!(rendered_lr.width > 0.0);
}
