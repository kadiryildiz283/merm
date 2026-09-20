use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Class,
    Struct,
    Enum,
    Module,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKind::Class => write!(f, "class"),
            NodeKind::Struct => write!(f, "struct"),
            NodeKind::Enum => write!(f, "enum"),
            NodeKind::Module => write!(f, "module"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Binds the current diagram to a project root (&set [PATH])
    Set { path: Option<String> },
    /// Tests compatibility between project and diagram via AST and LLM (&check)
    Check,
    /// Asks LLM for architectural advice or proposal (&advice <PROMPT>)
    Advice { prompt: String },
    /// Applies the pending recommendation/diff with snapshot and rollback (&ok)
    Ok,
    /// Autonomous multi-file generation/refactoring with verification (&ai <PROMPT>)
    Ai { prompt: String },
    /// Invokes Google Antigravity CLI (agy) directly (&agy <PROMPT>)
    Agy { prompt: String },
    /// Configures LLM provider, API keys, models or endpoints (:config [KEY] [VALUE])
    Config {
        key: Option<String>,
        value: Option<String>,
    },
    /// Adds a new class, struct, or enum to diagram and project (:add <kind> <name>)
    Add { kind: NodeKind, name: String },
    /// Connects two diagram nodes with a relation (:connect <from> <to> [label])
    Connect {
        from: String,
        to: String,
        label: Option<String>,
    },
    /// Runs executable test on a diagram node (:test [node_id] [input])
    Test {
        node_id: Option<String>,
        input: Option<String>,
    },
    /// Shows available commands and usage
    Help,
    /// Custom or unrecognized command
    Custom(String),
}

impl Command {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Command::Custom(String::new());
        }

        // Normalize prefix: allow both '&' and ':'
        let clean = if let Some(s) = trimmed.strip_prefix('&') {
            s.trim_start()
        } else if let Some(s) = trimmed.strip_prefix(':') {
            s.trim_start()
        } else {
            trimmed
        };

        let mut parts = clean.split_whitespace();
        let cmd_name = parts.next().unwrap_or("").to_lowercase();
        let remainder = clean[cmd_name.len()..].trim().to_string();

        match cmd_name.as_str() {
            "set" | "bind" => {
                let path = if remainder.is_empty() {
                    None
                } else {
                    Some(remainder)
                };
                Command::Set { path }
            }
            "check" | "verify" => Command::Check,
            "advice" | "advise" | "suggest" => Command::Advice { prompt: remainder },
            "ok" | "apply" | "yes" => Command::Ok,
            "ai" | "gen" | "refactor" => Command::Ai { prompt: remainder },
            "agy" | "antigravity" => Command::Agy { prompt: remainder },
            "config" | "cfg" => {
                let mut tokens = remainder.split_whitespace();
                let key = tokens.next().map(|s| s.to_string());
                let value = if key.is_some() {
                    let rest = tokens.collect::<Vec<&str>>().join(" ");
                    if rest.is_empty() {
                        None
                    } else {
                        Some(rest)
                    }
                } else {
                    None
                };
                Command::Config { key, value }
            }
            "add" | "new" => {
                let mut tokens = remainder.split_whitespace();
                let first = tokens.next().unwrap_or("class").to_lowercase();
                let (kind, name) = match first.as_str() {
                    "struct" => (
                        NodeKind::Struct,
                        tokens.next().unwrap_or("NewStruct").to_string(),
                    ),
                    "enum" => (
                        NodeKind::Enum,
                        tokens.next().unwrap_or("NewEnum").to_string(),
                    ),
                    "module" | "mod" => (
                        NodeKind::Module,
                        tokens.next().unwrap_or("new_mod").to_string(),
                    ),
                    "class" => (
                        NodeKind::Class,
                        tokens.next().unwrap_or("NewClass").to_string(),
                    ),
                    other => (NodeKind::Class, other.to_string()),
                };
                Command::Add { kind, name }
            }
            "connect" | "link" => {
                let tokens: Vec<&str> = remainder.split_whitespace().collect();
                if tokens.len() >= 2 {
                    let from = tokens[0].to_string();
                    let to = tokens[1].to_string();
                    let label = if tokens.len() > 2 {
                        Some(tokens[2..].join(" "))
                    } else {
                        None
                    };
                    Command::Connect { from, to, label }
                } else {
                    Command::Custom(input.to_string())
                }
            }
            "test" | "run" | "exec" => {
                let mut tokens = remainder.split_whitespace();
                let node_id = tokens.next().map(|s| s.to_string());
                let input_str = if node_id.is_some() {
                    let rest = tokens.collect::<Vec<&str>>().join(" ");
                    if rest.is_empty() {
                        None
                    } else {
                        Some(rest)
                    }
                } else {
                    None
                };
                Command::Test {
                    node_id,
                    input: input_str,
                }
            }
            "help" => Command::Help,
            _ => Command::Custom(input.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commands() {
        assert_eq!(
            Command::parse("&set /home/user/project"),
            Command::Set {
                path: Some("/home/user/project".to_string())
            }
        );
        assert_eq!(Command::parse(":check"), Command::Check);
        assert_eq!(
            Command::parse("&advice Add authentication layer"),
            Command::Advice {
                prompt: "Add authentication layer".to_string()
            }
        );
        assert_eq!(Command::parse("&ok"), Command::Ok);
        assert_eq!(
            Command::parse("&ai create billing service"),
            Command::Ai {
                prompt: "create billing service".to_string()
            }
        );
        assert_eq!(
            Command::parse(":add struct PaymentProcessor"),
            Command::Add {
                kind: NodeKind::Struct,
                name: "PaymentProcessor".to_string()
            }
        );
        assert_eq!(
            Command::parse(":connect Order Payment calls"),
            Command::Connect {
                from: "Order".to_string(),
                to: "Payment".to_string(),
                label: Some("calls".to_string())
            }
        );
        assert_eq!(
            Command::parse("&test AuthService {\"user\":\"admin\"}"),
            Command::Test {
                node_id: Some("AuthService".to_string()),
                input: Some("{\"user\":\"admin\"}".to_string()),
            }
        );
        assert_eq!(
            Command::parse("&agy review architecture"),
            Command::Agy {
                prompt: "review architecture".to_string()
            }
        );
        assert_eq!(
            Command::parse(":config provider openai"),
            Command::Config {
                key: Some("provider".to_string()),
                value: Some("openai".to_string())
            }
        );
        assert_eq!(
            Command::parse(":cfg api_key sk-12345"),
            Command::Config {
                key: Some("api_key".to_string()),
                value: Some("sk-12345".to_string())
            }
        );
        assert_eq!(
            Command::parse(":config"),
            Command::Config {
                key: None,
                value: None
            }
        );
    }
}
