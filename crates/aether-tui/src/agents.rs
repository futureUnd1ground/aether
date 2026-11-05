use aether_core::{
    Message,
    agents::{AgentRecord, AgentStatus},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use std::collections::BTreeMap;

#[derive(Default)]
struct Detail {
    messages: Vec<Message>,
    live: String,
    activity: String,
}
#[derive(Default)]
pub(crate) struct AgentTree {
    pub records: Vec<AgentRecord>,
    pub focused: bool,
    pub selected: usize,
    pub inspected: Option<String>,
    details: BTreeMap<String, Detail>,
}
impl AgentTree {
    pub fn update(&mut self, record: AgentRecord) {
        if let Some(old) = self.records.iter_mut().find(|r| r.id == record.id) {
            *old = record;
        } else {
            self.records.push(record);
        }
    }
    pub fn rows(&self) -> Vec<&AgentRecord> {
        fn append<'a>(
            records: &'a [AgentRecord],
            parent: Option<&str>,
            rows: &mut Vec<&'a AgentRecord>,
        ) {
            for r in records.iter().filter(|r| r.parent_id.as_deref() == parent) {
                rows.push(r);
                append(records, Some(&r.id), rows);
            }
        }
        let mut rows = Vec::new();
        append(&self.records, None, &mut rows);
        rows
    }
    pub fn current(&self) -> Option<AgentRecord> {
        self.rows().get(self.selected).copied().cloned()
    }
    pub fn select_id(&mut self, id: &str) {
        if let Some(i) = self.rows().iter().position(|r| r.id == id) {
            self.selected = i;
        }
    }
    pub fn stream(&mut self, id: String, text: String) {
        self.details.entry(id).or_default().live.push_str(&text);
    }
    pub fn activity(&mut self, id: String, name: String, text: String) {
        self.details
            .entry(id)
            .or_default()
            .activity
            .push_str(&format!("\n[{name}] {text}\n"));
    }
    pub fn loaded(&mut self, id: String, messages: Vec<Message>) {
        let detail = self.details.entry(id).or_default();
        detail.messages = messages;
        detail.activity.clear();
    }
    pub fn body(&self) -> Option<String> {
        let id = self.inspected.as_ref()?;
        let r = self.records.iter().find(|r| &r.id == id)?;
        let mut body = format!(
            "{}\n{} · {} · {:?}\n{} input / {} output · {} charged tokens · {} requests\nWorkspace: {}\n\nTASK\n{}\n\n",
            r.task.title,
            r.profile,
            r.model,
            r.status,
            r.usage.prompt_tokens,
            r.usage.completion_tokens,
            r.charged_tokens,
            r.steps,
            r.workspace,
            r.task.description
        );
        if let Some(error) = &r.error {
            body.push_str(&format!("ERROR\n{error}\n\n"));
        }
        if let Some(d) = self.details.get(id) {
            let messages = if r.result.as_ref().is_some_and(|result| {
                d.messages.last().is_some_and(|m| {
                    m.role == "assistant" && m.tool_calls.is_empty() && m.content == result.summary
                })
            }) {
                &d.messages[..d.messages.len() - 1]
            } else {
                &d.messages[..]
            };
            body.push_str(&format_messages(messages));
            let committed = d
                .messages
                .iter()
                .filter(|m| m.role == "assistant")
                .map(|m| m.content.as_str())
                .collect::<String>();
            if let Some(partial) = d.live.strip_prefix(&committed)
                && !partial.is_empty()
            {
                body.push_str("\nASSISTANT · streaming\n");
                body.push_str(partial);
            }
            body.push_str(&d.activity);
        } else {
            body.push_str("Loading persisted history…\n");
        }
        if let Some(result) = &r.result {
            body.push_str(&format!(
                "\nRESULT\n{}\n\nTool errors: {} · independently verified task: {}\n",
                result.summary, result.tool_errors, result.task_success_verified
            ));
        }
        Some(body)
    }
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let rows = self.rows();
        let items: Vec<ListItem> = rows
            .iter()
            .map(|r| {
                let (icon, color) = match r.status {
                    AgentStatus::Completed => ("✓", Color::Rgb(158, 206, 106)),
                    AgentStatus::Failed | AgentStatus::Interrupted => {
                        ("!", Color::Rgb(247, 118, 142))
                    }
                    AgentStatus::Cancelled => ("×", Color::Rgb(120, 125, 145)),
                    AgentStatus::Running => ("●", Color::Rgb(125, 207, 255)),
                    _ => ("○", Color::Rgb(224, 175, 104)),
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::raw("  ".repeat(r.depth)),
                        Span::styled(format!("{icon} "), Style::default().fg(color)),
                        Span::raw(if r.parent_id.is_none() {
                            "Main agent"
                        } else {
                            r.task.title.as_str()
                        }),
                    ]),
                    Line::from(format!(
                        "{}{} · {}",
                        "  ".repeat(r.depth + 1),
                        r.profile,
                        r.model
                    ))
                    .style(Style::default().fg(Color::Rgb(126, 146, 178))),
                ])
            })
            .collect();
        let items = if items.is_empty() {
            vec![ListItem::new(
                "No task running\n\nSend a message to begin.\n\nF2 focuses this tree.\n↑↓ choose · Enter inspect\nBackspace parent · x cancel",
            )]
        } else {
            items
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if self.focused {
                        Color::Rgb(125, 207, 255)
                    } else {
                        Color::Rgb(48, 61, 85)
                    }))
                    .title(Span::styled(
                        " AGENTS · F2 ",
                        Style::default().fg(Color::Rgb(126, 146, 178)),
                    )),
            )
            .highlight_style(Style::default().bg(Color::Rgb(32, 48, 72)))
            .highlight_symbol("› ");
        let mut state = ListState::default();
        if self.focused && !rows.is_empty() {
            state.select(Some(self.selected.min(rows.len() - 1)));
        }
        f.render_stateful_widget(list, area, &mut state);
    }
}
pub(crate) fn format_messages(messages: &[Message]) -> String {
    let names: std::collections::BTreeMap<_, _> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter())
        .map(|c| (c.id.as_str(), c.function.name.as_str()))
        .collect();
    let mut result = String::new();
    for m in messages {
        if m.role == "tool" {
            let name = m
                .tool_call_id
                .as_deref()
                .and_then(|id| names.get(id))
                .copied()
                .unwrap_or("result");
            result.push_str(&format!("TOOL · {name}\n{}\n\n", tool_output(&m.content)));
        } else {
            if !m.content.trim().is_empty() {
                result.push_str(&format!("{}\n{}\n\n", m.role.to_uppercase(), m.content));
            }
            for call in &m.tool_calls {
                let args = serde_json::from_str::<serde_json::Value>(&call.function.arguments).ok();
                let detail = args
                    .as_ref()
                    .and_then(|a| {
                        a.get("path")
                            .or_else(|| a.get("command"))
                            .or_else(|| a.get("agent_id"))
                    })
                    .map(|v| v.as_str().unwrap_or("").to_owned())
                    .unwrap_or_else(|| {
                        args.as_ref()
                            .and_then(|a| a["task"]["title"].as_str())
                            .unwrap_or(&call.function.arguments)
                            .to_owned()
                    });
                result.push_str(&format!("› {} · {}\n", call.function.name, detail));
            }
        }
    }
    result
}
fn tool_output(content: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(content) else {
        return content.into();
    };
    let text = if let Some(error) = v["error"].as_str() {
        format!("Error: {error}")
    } else if let Some(id) = v["agent_id"].as_str() {
        format!(
            "Agent {} created · {}\n{}",
            &id[..id.len().min(8)],
            if v["isolated"] == true {
                "isolated worktree"
            } else {
                "scoped read-only workspace"
            },
            v["workspace"].as_str().unwrap_or("")
        )
    } else if v.get("profile").is_some() && v.get("status").is_some() {
        format!(
            "{} · {} · {}\n{}\nTask success: independently unverified",
            v["task"]["title"].as_str().unwrap_or("Agent"),
            v["profile"].as_str().unwrap_or(""),
            v["status"].as_str().unwrap_or(""),
            v["result"]["summary"]
                .as_str()
                .or_else(|| v["error"].as_str())
                .unwrap_or("No final response")
        )
    } else if let Some(file) = v["content"].as_str() {
        format!(
            "{} · sha256 {}\n\n{}",
            v["path"].as_str().unwrap_or("file"),
            v["sha256"].as_str().unwrap_or("unknown"),
            file
        )
    } else if v.get("exit_code").is_some() {
        format!(
            "Process exit: {} · success: {}\n{}{}",
            v["exit_code"],
            v["success"],
            v["stdout"].as_str().unwrap_or(""),
            v["stderr"].as_str().unwrap_or("")
        )
    } else {
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| content.into())
    };
    if text.len() > 12000 {
        format!(
            "{}\n… display excerpt; full result remains in the session export",
            text.chars().take(12000).collect::<String>()
        )
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::{Usage, agents::AgentTask};
    pub(super) fn record(id: &str, parent: Option<&str>, status: AgentStatus) -> AgentRecord {
        AgentRecord {
            id: id.into(),
            root_id: "main".into(),
            root_session_id: "session".into(),
            parent_id: parent.map(str::to_owned),
            session_id: format!("session-{id}"),
            depth: usize::from(parent.is_some()),
            profile: if parent.is_some() { "research" } else { "main" }.into(),
            task: AgentTask {
                title: id.into(),
                description: "Inspect actual source".into(),
                success_criteria: vec![],
            },
            model: "local".into(),
            provider_label: "default".into(),
            workspace: "/workspace".into(),
            isolated: false,
            base_commit: None,
            integration_target: None,
            status,
            usage: Usage::default(),
            charged_tokens: 123,
            steps: 1,
            cost_usd: None,
            result: None,
            error: None,
            integration: None,
        }
    }
    #[test]
    fn tree_order_and_parent_navigation_use_real_relationships() {
        let mut tree = AgentTree::default();
        tree.update(record("main", None, AgentStatus::Running));
        tree.update(record("a", Some("main"), AgentStatus::Completed));
        tree.update(record("b", Some("main"), AgentStatus::Queued));
        tree.update(record("a-child", Some("a"), AgentStatus::Running));
        assert_eq!(
            tree.rows()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "a", "a-child", "b"]
        );
        tree.select_id("a-child");
        assert_eq!(tree.current().unwrap().parent_id.as_deref(), Some("a"));
    }
    #[test]
    fn inspected_stream_does_not_duplicate_committed_assistant_text() {
        let mut tree = AgentTree::default();
        tree.update(record("child", Some("main"), AgentStatus::Running));
        tree.inspected = Some("child".into());
        tree.stream("child".into(), "committedpartial".into());
        tree.loaded(
            "child".into(),
            vec![Message::text("assistant", "committed")],
        );
        let body = tree.body().unwrap();
        assert_eq!(body.matches("committed").count(), 1);
        assert!(body.contains("partial"));
    }
    #[test]
    fn root_and_subagent_layouts_render_at_narrow_sizes() {
        use ratatui::backend::TestBackend;
        let mut tree = AgentTree::default();
        tree.update(record("main", None, AgentStatus::Running));
        tree.update(record("worker", Some("main"), AgentStatus::Failed));
        tree.focused = true;
        tree.selected = 1;
        for w in [18, 34, 80] {
            let mut terminal = Terminal::new(TestBackend::new(w, 12)).unwrap();
            terminal.draw(|f| tree.render(f, f.area())).unwrap();
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .any(|c| c.symbol() == "w")
            );
        }
    }
}
