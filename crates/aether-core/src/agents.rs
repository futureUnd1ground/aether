use crate::{Mode, Rule, Usage, config::Permissions};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OrchestrationConfig {
    pub enabled: bool,
    /// Root has depth zero and counts toward max_agents.
    pub max_depth: usize,
    pub max_concurrent: usize,
    pub max_agents: usize,
    pub total_tokens: u64,
    pub total_steps: usize,
    pub max_cost_usd: Option<f64>,
}
impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_depth: 2,
            max_concurrent: 2,
            max_agents: 8,
            total_tokens: 131072,
            total_steps: 60,
            max_cost_usd: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentProfile {
    pub instructions: String,
    pub mode: Mode,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tools: Vec<String>,
    pub permissions: Permissions,
    pub scope: String,
    pub max_steps: usize,
    pub max_tokens: u64,
    pub context_tokens: usize,
    pub timeout_secs: u64,
    pub max_cost_usd: Option<f64>,
    pub can_delegate: bool,
}
impl Default for AgentProfile {
    fn default() -> Self {
        Self {
            instructions: String::new(),
            mode: Mode::Ask,
            provider: None,
            model: None,
            tools: vec!["read_file".into(), "list_directory".into()],
            permissions: Permissions {
                read: Rule::Allow,
                write: Rule::Deny,
                shell: Rule::Deny,
            },
            scope: ".".into(),
            max_steps: 12,
            max_tokens: 32768,
            context_tokens: 16384,
            timeout_secs: 300,
            max_cost_usd: None,
            can_delegate: false,
        }
    }
}
pub fn builtin_profiles() -> BTreeMap<String, AgentProfile> {
    let mut result = BTreeMap::new();
    for (name, instructions) in [
        (
            "explore",
            "Inspect the relevant project files. Return paths and concrete findings. Do not change files.",
        ),
        (
            "research",
            "Research the project architecture using scoped tools. Separate evidence, hypotheses, and open questions.",
        ),
        (
            "review",
            "Review changes and identify actionable defects with file evidence. Do not change files.",
        ),
        (
            "build",
            "Implement the assigned task in your isolated workspace. Run available validation only with approval. Return exact changed files, results and limitations. Do not integrate changes into a parent workspace yourself.",
        ),
        (
            "debug",
            "Investigate a concrete failure. Make a minimal fix in isolation and report verified outcomes and unresolved hypotheses.",
        ),
        (
            "test",
            "Run targeted tests in isolation with approval. Report the actual command, exit code and failures. Do not claim task correctness just because a command exited successfully.",
        ),
    ] {
        let mut p = AgentProfile {
            instructions: instructions.into(),
            ..Default::default()
        };
        if matches!(name, "build" | "debug" | "test") {
            p.mode = if name == "debug" {
                Mode::Debug
            } else {
                Mode::Build
            };
            p.permissions.shell = Rule::Ask;
            p.tools.push("shell".into());
            if name != "test" {
                p.permissions.write = Rule::Ask;
                p.tools.push("write_file".into());
            }
        } else if name == "review" {
            p.mode = Mode::Review;
        }
        if name != "test" {
            p.can_delegate = true;
            p.tools.extend(
                ["spawn_agent", "await_agent", "list_agents", "cancel_agent"].map(str::to_owned),
            );
        }
        result.insert(name.into(), p);
    }
    result
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTask {
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub success_criteria: Vec<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Created,
    Queued,
    Running,
    AwaitingPermission,
    Waiting,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
impl AgentStatus {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub summary: String,
    /// A completed response is not independent verification of the user's task.
    pub task_success_verified: bool,
    pub tools_executed: usize,
    pub tool_errors: usize,
    pub command_exit_codes: Vec<Option<i32>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub root_id: String,
    pub root_session_id: String,
    pub parent_id: Option<String>,
    pub session_id: String,
    pub depth: usize,
    pub profile: String,
    pub task: AgentTask,
    pub model: String,
    pub provider_label: String,
    pub workspace: String,
    pub isolated: bool,
    pub base_commit: Option<String>,
    #[serde(default)]
    pub integration_target: Option<String>,
    pub status: AgentStatus,
    pub usage: Usage,
    pub charged_tokens: u64,
    pub cost_usd: Option<f64>,
    pub steps: usize,
    pub result: Option<AgentResult>,
    pub error: Option<String>,
    pub integration: Option<String>,
}
