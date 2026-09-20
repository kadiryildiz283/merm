use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcErrorObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorMovedParams {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JumpToDefinitionParams {
    pub node_id: String,
    pub target_file: String,
    pub target_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReloadParams {
    pub file: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorCommand {
    CursorMoved(CursorMovedParams),
    JumpToDefinition(JumpToDefinitionParams),
    Reload(ReloadParams),
    Ping,
    Custom { method: String, params: Option<serde_json::Value> },
}

impl JsonRpcRequest {
    pub fn parse_command(&self) -> EditorCommand {
        match self.method.as_str() {
            "cursor_moved" | "merm/cursor_moved" => {
                if let Some(ref p) = self.params {
                    if let Ok(params) = serde_json::from_value::<CursorMovedParams>(p.clone()) {
                        return EditorCommand::CursorMoved(params);
                    }
                }
                EditorCommand::Custom {
                    method: self.method.clone(),
                    params: self.params.clone(),
                }
            }
            "jump_to_definition" | "merm/jump_to_definition" => {
                if let Some(ref p) = self.params {
                    if let Ok(params) = serde_json::from_value::<JumpToDefinitionParams>(p.clone()) {
                        return EditorCommand::JumpToDefinition(params);
                    }
                }
                EditorCommand::Custom {
                    method: self.method.clone(),
                    params: self.params.clone(),
                }
            }
            "reload" | "merm/reload" => {
                if let Some(ref p) = self.params {
                    if let Ok(params) = serde_json::from_value::<ReloadParams>(p.clone()) {
                        return EditorCommand::Reload(params);
                    }
                }
                EditorCommand::Custom {
                    method: self.method.clone(),
                    params: self.params.clone(),
                }
            }
            "ping" => EditorCommand::Ping,
            _ => EditorCommand::Custom {
                method: self.method.clone(),
                params: self.params.clone(),
            },
        }
    }
}
