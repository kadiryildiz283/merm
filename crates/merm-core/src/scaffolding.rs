use crate::command::NodeKind;
use crate::error::CoreError;
use crate::manifest::{NodeBinding, ProjectManifest};
use std::fs;
use std::path::Path;

pub struct Scaffolder;

impl Scaffolder {
    /// Appends a new node to the Mermaid diagram source
    pub fn add_node_to_diagram(diagram_source: &str, kind: &NodeKind, name: &str) -> String {
        let stereotype = match kind {
            NodeKind::Class => "<<class>>",
            NodeKind::Struct => "<<struct>>",
            NodeKind::Enum => "<<enum>>",
            NodeKind::Module => "<<module>>",
        };

        let node_block = format!(
            "\n    class {} {{\n        {}\n        +run(input: String) String\n    }}\n",
            name, stereotype
        );

        let mut out = diagram_source.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&node_block);
        out
    }

    /// Connects two nodes in the Mermaid diagram source
    pub fn connect_nodes_in_diagram(
        diagram_source: &str,
        from: &str,
        to: &str,
        label: Option<&str>,
    ) -> String {
        let edge = if let Some(lbl) = label {
            format!("\n    {} --> {} : {}\n", from, to, lbl)
        } else {
            format!("\n    {} --> {}\n", from, to)
        };

        let mut out = diagram_source.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&edge);
        out
    }

    /// Removes a node and its relationships from the diagram source
    pub fn remove_node_from_diagram(diagram_source: &str, node_id: &str) -> String {
        let mut out = Vec::new();
        let mut in_target_class = false;

        for line in diagram_source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ")
                && (trimmed.contains(&format!("class {} ", node_id))
                    || trimmed.contains(&format!("class {}{}", node_id, '{'))
                    || trimmed == format!("class {}", node_id))
            {
                if trimmed.ends_with('{') {
                    in_target_class = true;
                    continue;
                } else {
                    continue;
                }
            }
            if in_target_class {
                if trimmed == "}" {
                    in_target_class = false;
                }
                continue;
            }
            if (trimmed.contains("-->")
                || trimmed.contains("--|>")
                || trimmed.contains("..>")
                || trimmed.contains("--*")
                || trimmed.contains("--o"))
                && (trimmed.starts_with(node_id)
                    || trimmed.contains(&format!(" {} ", node_id))
                    || trimmed.ends_with(node_id))
            {
                continue;
            }
            out.push(line);
        }
        out.join("\n")
    }

    /// Updates stereotype and members for a node in the diagram source
    pub fn update_node_in_diagram(
        diagram_source: &str,
        node_id: &str,
        stereotype: Option<&str>,
        members: &[String],
    ) -> String {
        let mut out = Vec::new();
        let mut in_target_class = false;
        let mut replaced = false;

        let st_str = if let Some(st) = stereotype {
            format!("        <<{}>>", st)
        } else {
            String::new()
        };

        let members_str = members
            .iter()
            .map(|m| format!("        {}", m))
            .collect::<Vec<_>>()
            .join("\n");

        for line in diagram_source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ")
                && (trimmed.contains(&format!("class {} ", node_id))
                    || trimmed.contains(&format!("class {}{}", node_id, '{'))
                    || trimmed == format!("class {}", node_id))
                && trimmed.ends_with('{')
            {
                in_target_class = true;
                out.push(format!("    class {} {{", node_id));
                if !st_str.is_empty() {
                    out.push(st_str.clone());
                }
                if !members_str.is_empty() {
                    out.push(members_str.clone());
                }
                replaced = true;
                continue;
            }
            if in_target_class {
                if trimmed == "}" {
                    in_target_class = false;
                    out.push("    }".to_string());
                }
                continue;
            }
            out.push(line.to_string());
        }

        if !replaced {
            out.push(format!("    class {} {{", node_id));
            if !st_str.is_empty() {
                out.push(st_str);
            }
            if !members_str.is_empty() {
                out.push(members_str);
            }
            out.push("    }".to_string());
        }

        out.join("\n")
    }

    /// Scaffolds the Rust file and registers module in lib.rs / main.rs
    pub fn scaffold_rust_node(
        manifest: &mut ProjectManifest,
        kind: &NodeKind,
        name: &str,
    ) -> Result<String, CoreError> {
        let snake_name = to_snake_case(name);
        let rel_file = format!("src/{}.rs", snake_name);
        let abs_file = manifest.project_root.join(&rel_file);

        if !abs_file.exists() {
            let code = match kind {
                NodeKind::Struct | NodeKind::Class => format!(
                    r#"//! {} module generated by merm

pub struct {};

impl {} {{
    /// Executable entrypoint for merm interactive testing
    pub fn run(input: &str) -> String {{
        format!("{}: received '{{}}'", input)
    }}
}}
"#,
                    name, name, name, name
                ),
                NodeKind::Enum => format!(
                    r#"//! {} enum generated by merm

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum {} {{
    DefaultVariant,
    Active,
    Disabled,
}}

impl {} {{
    pub fn run(input: &str) -> String {{
        format!("{}: received '{{}}'", input)
    }}
}}
"#,
                    name, name, name, name
                ),
                NodeKind::Module => format!(
                    r#"//! {} module generated by merm

pub fn run(input: &str) -> String {{
    format!("{}: received '{{}}'", input)
}}
"#,
                    name, name
                ),
            };

            if let Some(parent) = abs_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&abs_file, code).map_err(|e| {
                CoreError::LayoutFailed(format!("Failed to write scaffolded Rust file: {}", e))
            })?;

            // Register module in src/lib.rs or src/main.rs
            Self::register_module_in_root(&manifest.project_root, &snake_name)?;
        }

        // Add binding to manifest
        manifest.add_binding(NodeBinding {
            id: name.to_string(),
            file: rel_file.clone(),
            symbol: name.to_string(),
            kind: kind.to_string(),
            executable: true,
            entrypoint: Some("run".to_string()),
            input_type: Some("String".to_string()),
            output_type: Some("String".to_string()),
        });
        let _ = manifest.save();

        Ok(rel_file)
    }

    fn register_module_in_root(project_root: &Path, module_name: &str) -> Result<(), CoreError> {
        let lib_rs = project_root.join("src").join("lib.rs");
        let main_rs = project_root.join("src").join("main.rs");

        let target_file = if lib_rs.is_file() {
            Some(lib_rs)
        } else if main_rs.is_file() {
            Some(main_rs)
        } else {
            None
        };

        if let Some(file) = target_file {
            let content = fs::read_to_string(&file).unwrap_or_default();
            let mod_decl = format!("pub mod {};", module_name);
            if !content.contains(&mod_decl) {
                let new_content = format!("{}\n{}", mod_decl, content);
                let _ = fs::write(&file, new_content);
            }
        }
        Ok(())
    }
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == ' ' {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("OrderService"), "order_service");
        assert_eq!(to_snake_case("AuthHandler"), "auth_handler");
        assert_eq!(to_snake_case("APIClient"), "a_p_i_client");
    }

    #[test]
    fn test_add_node_to_diagram() {
        let base = "classDiagram\n    class User {\n    }\n";
        let updated = Scaffolder::add_node_to_diagram(base, &NodeKind::Struct, "Account");
        assert!(updated.contains("class Account"));
        assert!(updated.contains("<<struct>>"));
    }
}
