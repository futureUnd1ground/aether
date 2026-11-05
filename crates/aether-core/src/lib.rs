pub mod agents;
pub mod config;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type", default = "function_type")]
    pub kind: String,
    pub function: FunctionCall,
}
fn function_type() -> String {
    "function".into()
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
impl Message {
    pub fn text(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
    pub fn tool(id: &str, value: Value) -> Self {
        let mut m = Self::text("tool", value.to_string());
        m.tool_call_id = Some(id.into());
        m
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Ask,
    Plan,
    Build,
    Debug,
    Review,
}
impl Mode {
    pub fn writable(self) -> bool {
        matches!(self, Self::Build | Self::Debug)
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Allow,
    #[default]
    Ask,
    Deny,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Created,
    Queued,
    Running,
    AwaitingPermission,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub workspace: String,
    pub parent_id: Option<String>,
    pub model: String,
    pub draft: String,
    pub updated_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence: i64,
    pub kind: String,
    pub payload: Value,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}
/// Conservative heuristic, never an exact tokenizer.
pub fn estimated_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| {
            (m.content.len()
                + m.tool_calls
                    .iter()
                    .map(|c| c.function.arguments.len())
                    .sum::<usize>())
            .div_ceil(3)
                + 12
        })
        .sum()
}
