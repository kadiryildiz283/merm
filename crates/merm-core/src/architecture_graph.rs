use crate::engine::RelationKind;
use crate::rust_scanner::{ProjectScanReport, RustSymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchMember {
    pub visibility: char,
    pub name: String,
    pub type_name: Option<String>,
    pub comment: Option<String>,
    pub is_method: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchNode {
    pub id: String,
    pub name: String,
    pub stereotype: Option<String>,
    pub doc_comment: Option<String>,
    pub fields: Vec<ArchMember>,
    pub methods: Vec<ArchMember>,
    pub bound_file: Option<String>,
    pub executable: bool,
    pub entrypoint: Option<String>,
    pub input_schema: Option<String>,
    pub output_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation: RelationKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ArchitectureGraph {
    pub nodes: HashMap<String, ArchNode>,
    pub edges: Vec<ArchEdge>,
}

impl ArchitectureGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: ArchNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn get_node(&self, id: &str) -> Option<&ArchNode> {
        self.nodes.get(id)
    }

    pub fn add_edge(&mut self, edge: ArchEdge) {
        self.edges.push(edge);
    }

    /// Converts Mermaid diagram source text into an ArchitectureGraph AST
    pub fn from_mermaid_source(source: &str) -> Self {
        let mut graph = Self::new();

        // Extract metadata lines like %% @merm:node id="Auth" file="src/auth.rs" exec="true"
        let mut file_bindings: HashMap<String, (String, bool)> = HashMap::new();
        for line in source.lines() {
            let t = line.trim();
            if t.starts_with("%% @merm:node") {
                let id = extract_attr(t, "id");
                let file = extract_attr(t, "file");
                let exec = extract_attr(t, "exec")
                    .map(|s| s == "true")
                    .unwrap_or(false);
                if let (Some(id_val), Some(file_val)) = (id, file) {
                    file_bindings.insert(id_val, (file_val, exec));
                }
            }
        }

        // Parse using ClassDiagramParser rendered diagram or line extractor
        let mut current_class: Option<ArchNode> = None;

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("classDiagram")
                || trimmed.starts_with("direction ")
            {
                continue;
            }

            if trimmed.starts_with("class ") {
                if let Some(c) = current_class.take() {
                    graph.add_node(c);
                }

                let rest = trimmed.strip_prefix("class ").unwrap_or("").trim();
                let class_name = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('{')
                    .trim();

                let (bound_file, executable) = file_bindings
                    .get(class_name)
                    .cloned()
                    .unwrap_or_else(|| (format!("src/{}.rs", to_snake(class_name)), true));

                current_class = Some(ArchNode {
                    id: class_name.to_string(),
                    name: class_name.to_string(),
                    stereotype: None,
                    doc_comment: None,
                    fields: Vec::new(),
                    methods: Vec::new(),
                    bound_file: Some(bound_file),
                    executable,
                    entrypoint: Some("run".to_string()),
                    input_schema: Some("String".to_string()),
                    output_schema: Some("String".to_string()),
                });
                continue;
            }

            if trimmed == "}" {
                if let Some(c) = current_class.take() {
                    graph.add_node(c);
                }
                continue;
            }

            if let Some(ref mut c) = current_class {
                if trimmed.starts_with("<<") && trimmed.ends_with(">>") {
                    c.stereotype = Some(trimmed.trim_matches('<').trim_matches('>').to_string());
                } else if trimmed.starts_with("%%") || trimmed.starts_with("//") {
                    let comm = trimmed
                        .trim_start_matches('%')
                        .trim_start_matches('/')
                        .trim()
                        .to_string();
                    if c.doc_comment.is_none() {
                        c.doc_comment = Some(comm);
                    }
                } else {
                    let member = parse_arch_member(trimmed);
                    if member.is_method {
                        c.methods.push(member);
                    } else {
                        c.fields.push(member);
                    }
                }
                continue;
            }

            // Check relation line
            if let Some(edge) = parse_relation_edge(trimmed) {
                graph.add_edge(edge);
            }
        }

        if let Some(c) = current_class {
            graph.add_node(c);
        }

        graph
    }

    /// Converts project scanned symbols into an ArchitectureGraph
    pub fn from_project_symbols(report: &ProjectScanReport) -> Self {
        let mut graph = Self::new();

        for sym in &report.symbols {
            let stereotype = match sym.kind {
                RustSymbolKind::Struct => Some("struct".to_string()),
                RustSymbolKind::Enum => Some("enum".to_string()),
                RustSymbolKind::Module => Some("module".to_string()),
                RustSymbolKind::Function => Some("fn".to_string()),
            };

            let fields = sym
                .fields
                .iter()
                .map(|f| ArchMember {
                    visibility: if f.is_public { '+' } else { '-' },
                    name: f.name.clone(),
                    type_name: Some(f.type_name.clone()),
                    comment: f.comment.clone(),
                    is_method: false,
                })
                .collect();

            let methods = sym
                .methods
                .iter()
                .map(|m| ArchMember {
                    visibility: if m.is_public { '+' } else { '-' },
                    name: m.name.clone(),
                    type_name: m.return_type.clone(),
                    comment: m.comment.clone(),
                    is_method: true,
                })
                .collect();

            graph.add_node(ArchNode {
                id: sym.name.clone(),
                name: sym.name.clone(),
                stereotype,
                doc_comment: sym.doc_comment.clone(),
                fields,
                methods,
                bound_file: Some(sym.file_path.clone()),
                executable: sym.is_executable,
                entrypoint: sym.primary_entrypoint.clone(),
                input_schema: Some("String".to_string()),
                output_schema: Some("String".to_string()),
            });
        }

        graph
    }

    /// Serializes the ArchitectureGraph back into standard Mermaid classDiagram text
    pub fn to_mermaid_source(&self) -> String {
        let mut lines = Vec::new();
        lines.push("classDiagram".to_string());
        lines.push("    direction TD".to_string());
        lines.push("".to_string());

        for node in self.nodes.values() {
            lines.push(format!("    class {} {{", node.name));
            if let Some(ref st) = node.stereotype {
                lines.push(format!("        <<{}>>", st));
            }
            if let Some(ref doc) = node.doc_comment {
                lines.push(format!("        %% {}", doc));
            }
            for f in &node.fields {
                let ty = f.type_name.as_deref().unwrap_or("String");
                let comm = f
                    .comment
                    .as_ref()
                    .map(|c| format!(" // {}", c))
                    .unwrap_or_default();
                lines.push(format!(
                    "        {}{}: {}{}",
                    f.visibility, f.name, ty, comm
                ));
            }
            for m in &node.methods {
                let ret = m
                    .type_name
                    .as_ref()
                    .map(|r| format!(" {}", r))
                    .unwrap_or_default();
                let comm = m
                    .comment
                    .as_ref()
                    .map(|c| format!(" // {}", c))
                    .unwrap_or_default();
                lines.push(format!("        {}{}{}{}", m.visibility, m.name, ret, comm));
            }
            lines.push("    }".to_string());
            if let Some(ref file) = node.bound_file {
                lines.push(format!(
                    "    %% @merm:node id=\"{}\" file=\"{}\" exec=\"{}\"",
                    node.id, file, node.executable
                ));
            }
            lines.push("".to_string());
        }

        for edge in &self.edges {
            let rel_symbol = match edge.relation {
                RelationKind::Inheritance => "<|--",
                RelationKind::Realization => "..|>",
                RelationKind::Composition => "*--",
                RelationKind::Aggregation => "o--",
                RelationKind::Dependency => "..>",
                RelationKind::Association | RelationKind::Standard => "-->",
                RelationKind::Link => "---",
            };
            if let Some(ref lbl) = edge.label {
                lines.push(format!(
                    "    {} {} {} : {}",
                    edge.from, rel_symbol, edge.to, lbl
                ));
            } else {
                lines.push(format!("    {} {} {}", edge.from, rel_symbol, edge.to));
            }
        }

        lines.join("\n")
    }
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pat = format!("{}=\"", attr);
    let start = line.find(&pat)? + pat.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_arch_member(raw_line: &str) -> ArchMember {
    let mut s = raw_line.trim();
    let mut comment = None;
    if let Some(idx) = s.find("//") {
        comment = Some(s[idx + 2..].trim().to_string());
        s = s[..idx].trim();
    }

    let mut vis = '+';
    if s.starts_with('+') || s.starts_with('-') || s.starts_with('#') || s.starts_with('~') {
        vis = s.chars().next().unwrap();
        s = s[1..].trim();
    }

    let is_method = s.contains('(');
    let (name, type_name) = if is_method {
        if let Some(paren_close) = s.find(')') {
            let sig = s[..=paren_close].trim();
            let ret = s[paren_close + 1..].trim();
            let ret_type = if ret.is_empty() {
                None
            } else {
                Some(ret.trim_start_matches(':').trim().to_string())
            };
            (sig.to_string(), ret_type)
        } else {
            (s.to_string(), None)
        }
    } else if let Some((n, t)) = s.split_once(':') {
        (n.trim().to_string(), Some(t.trim().to_string()))
    } else {
        (s.to_string(), None)
    };

    ArchMember {
        visibility: vis,
        name,
        type_name,
        comment,
        is_method,
    }
}

fn parse_relation_edge(line: &str) -> Option<ArchEdge> {
    let relations = [
        ("<|--", RelationKind::Inheritance),
        ("..|>", RelationKind::Realization),
        ("*--", RelationKind::Composition),
        ("o--", RelationKind::Aggregation),
        ("-->", RelationKind::Association),
        ("..>", RelationKind::Dependency),
    ];

    for (symbol, kind) in relations {
        if let Some((left, right)) = line.split_once(symbol) {
            let from = left.trim().to_string();
            let (to, label) = if let Some((t, lbl)) = right.split_once(':') {
                (t.trim().to_string(), Some(lbl.trim().to_string()))
            } else {
                (right.trim().to_string(), None)
            };

            let id = format!("{}_{}_{}", from, symbol, to);
            return Some(ArchEdge {
                id,
                from,
                to,
                relation: kind,
                label,
            });
        }
    }
    None
}

/// Conflict-Aware Reconciliation Engine comparing Mermaid Architecture and Code AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Divergence {
    pub node_id: String,
    pub missing_fields: Vec<String>,
    pub extra_fields: Vec<String>,
    pub missing_methods: Vec<String>,
    pub extra_methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub matched_nodes: Vec<String>,
    pub diagram_only_nodes: Vec<String>,
    pub code_only_nodes: Vec<String>,
    pub divergences: Vec<Divergence>,
}

impl ReconciliationReport {
    pub fn is_synchronized(&self) -> bool {
        self.diagram_only_nodes.is_empty()
            && self.code_only_nodes.is_empty()
            && self.divergences.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "Reconciliation: {} in sync | {} diagram-only | {} code-only | {} diverged",
            self.matched_nodes.len(),
            self.diagram_only_nodes.len(),
            self.code_only_nodes.len(),
            self.divergences.len()
        )
    }

    pub fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Architecture Reconciliation Matrix ===".to_string());
        lines.push(format!(
            "Status: {}",
            if self.is_synchronized() {
                "SYNCHRONIZED"
            } else {
                "DIVERGENCE DETECTED"
            }
        ));
        lines.push(format!(
            "Synchronized Nodes: {}",
            self.matched_nodes.join(", ")
        ));

        if !self.diagram_only_nodes.is_empty() {
            lines.push(format!(
                "✖ Diagram Only (No Rust implementation): {}",
                self.diagram_only_nodes.join(", ")
            ));
        }
        if !self.code_only_nodes.is_empty() {
            lines.push(format!(
                "ℹ Code Only (Not represented in diagram): {}",
                self.code_only_nodes.join(", ")
            ));
        }
        for div in &self.divergences {
            lines.push(format!("⚠ Node '{}' Divergence:", div.node_id));
            if !div.missing_methods.is_empty() {
                lines.push(format!(
                    "   Missing methods in code: {}",
                    div.missing_methods.join(", ")
                ));
            }
            if !div.extra_methods.is_empty() {
                lines.push(format!(
                    "   Extra methods in code: {}",
                    div.extra_methods.join(", ")
                ));
            }
        }
        lines.join("\n")
    }
}

pub struct ReconciliationEngine;

impl ReconciliationEngine {
    pub fn reconcile(
        diagram_graph: &ArchitectureGraph,
        code_graph: &ArchitectureGraph,
    ) -> ReconciliationReport {
        let diag_keys: HashSet<&String> = diagram_graph.nodes.keys().collect();
        let code_keys: HashSet<&String> = code_graph.nodes.keys().collect();

        let matched: Vec<String> = diag_keys
            .intersection(&code_keys)
            .map(|s| (*s).clone())
            .collect();

        let diagram_only: Vec<String> = diag_keys
            .difference(&code_keys)
            .map(|s| (*s).clone())
            .collect();

        let code_only: Vec<String> = code_keys
            .difference(&diag_keys)
            .map(|s| (*s).clone())
            .collect();

        let mut divergences = Vec::new();

        for node_id in &matched {
            let diag_node = diagram_graph.nodes.get(node_id).unwrap();
            let code_node = code_graph.nodes.get(node_id).unwrap();

            let diag_methods: HashSet<&String> =
                diag_node.methods.iter().map(|m| &m.name).collect();
            let code_methods: HashSet<&String> =
                code_node.methods.iter().map(|m| &m.name).collect();

            let missing_methods: Vec<String> = diag_methods
                .difference(&code_methods)
                .map(|s| (*s).clone())
                .collect();

            let extra_methods: Vec<String> = code_methods
                .difference(&diag_methods)
                .map(|s| (*s).clone())
                .collect();

            if !missing_methods.is_empty() || !extra_methods.is_empty() {
                divergences.push(Divergence {
                    node_id: node_id.clone(),
                    missing_fields: Vec::new(),
                    extra_fields: Vec::new(),
                    missing_methods,
                    extra_methods,
                });
            }
        }

        ReconciliationReport {
            matched_nodes: matched,
            diagram_only_nodes: diagram_only,
            code_only_nodes: code_only,
            divergences,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_roundtrip_mermaid() {
        let sample = r#"classDiagram
    direction TD

    class AuthService {
        <<struct>>
        %% Authentication service
        +user_id: u64
        +run(input: String) String
    }
    %% @merm:node id="AuthService" file="src/auth.rs" exec="true"

    class UserService {
        +get_user(): String
    }

    AuthService --> UserService : calls
"#;
        let graph = ArchitectureGraph::from_mermaid_source(sample);
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key("AuthService"));
        assert!(graph.nodes.contains_key("UserService"));
        assert_eq!(graph.edges.len(), 1);

        let serialized = graph.to_mermaid_source();
        assert!(serialized.contains("class AuthService"));
        assert!(serialized.contains("AuthService --> UserService : calls"));
    }

    #[test]
    fn test_reconciliation_detects_divergence() {
        let diagram_source = r#"classDiagram
    class PaymentGateway {
        +charge(amount: f64) bool
    }
"#;
        let mut code_graph = ArchitectureGraph::new();
        code_graph.add_node(ArchNode {
            id: "PaymentGateway".to_string(),
            name: "PaymentGateway".to_string(),
            stereotype: Some("struct".to_string()),
            doc_comment: None,
            fields: Vec::new(),
            methods: vec![ArchMember {
                visibility: '+',
                name: "refund".to_string(),
                type_name: Some("bool".to_string()),
                comment: None,
                is_method: true,
            }],
            bound_file: Some("src/payment.rs".to_string()),
            executable: true,
            entrypoint: Some("refund".to_string()),
            input_schema: None,
            output_schema: None,
        });

        let diag_graph = ArchitectureGraph::from_mermaid_source(diagram_source);
        let report = ReconciliationEngine::reconcile(&diag_graph, &code_graph);

        assert!(!report.is_synchronized());
        assert_eq!(report.matched_nodes.len(), 1);
        assert_eq!(report.divergences.len(), 1);
        let div = &report.divergences[0];
        assert_eq!(div.missing_methods, vec!["charge(amount: f64)".to_string()]);
        assert_eq!(div.extra_methods, vec!["refund".to_string()]);
    }
}
