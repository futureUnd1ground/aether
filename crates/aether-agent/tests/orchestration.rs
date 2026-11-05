//! Test-only model fixtures. Every delegated node still runs the actual engine,
//! provider trait, permission, tools, budgets and SQLite path.
use aether_agent::{Agent, Event};
use aether_core::{
    FunctionCall, Message, Mode, Rule, ToolCall, Usage,
    agents::AgentStatus,
    config::{Config, ProviderConfig},
};
use aether_providers::{Completion, Delta, Provider};
use aether_storage::Store;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct Fixture {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    models: Arc<Mutex<Vec<String>>>,
    model: String,
    root: PathBuf,
    children: usize,
    recursive: bool,
    fail: bool,
    slow: bool,
    write: bool,
    conflict: bool,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
fn tools(calls: Vec<(&str, Value)>) -> Completion {
    Completion {
        tools: calls
            .into_iter()
            .map(|(name, args)| ToolCall {
                id: uuid::Uuid::new_v4().to_string(),
                kind: "function".into(),
                function: FunctionCall {
                    name: name.into(),
                    arguments: args.to_string(),
                },
            })
            .collect(),
        usage: Usage {
            prompt_tokens: 100,
            completion_tokens: 25,
        },
        ..Default::default()
    }
}
fn text(t: &str) -> Completion {
    Completion {
        text: t.into(),
        usage: Usage {
            prompt_tokens: 100,
            completion_tokens: 25,
        },
        ..Default::default()
    }
}
fn has(messages: &[Message], tool: &str) -> bool {
    messages
        .iter()
        .any(|m| m.tool_calls.iter().any(|c| c.function.name == tool))
}
fn ids(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter(|m| m.role == "tool")
        .filter_map(|m| serde_json::from_str::<Value>(&m.content).ok())
        .filter_map(|v| v["agent_id"].as_str().map(str::to_owned))
        .collect()
}
#[async_trait::async_trait]
impl Provider for Fixture {
    fn configured(&self, config: ProviderConfig) -> Result<Arc<dyn Provider>> {
        let mut f = self.clone();
        f.model = config.model;
        Ok(Arc::new(f))
    }
    async fn complete(
        &self,
        messages: &[Message],
        _: &[Value],
        _: usize,
        _: mpsc::Sender<Delta>,
        cancel: CancellationToken,
    ) -> Result<Completion> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.models.lock().unwrap().push(self.model.clone());
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        let _active = Active(self.active.clone());
        let task = messages
            .iter()
            .find(|m| m.role == "user")
            .unwrap()
            .content
            .clone();
        let root = task.starts_with("ROOT");
        if !root {
            tokio::select! {_=cancel.cancelled()=>bail!("fixture cancelled"),_=tokio::time::sleep(Duration::from_millis(if self.slow{10000}else{60}))=>{}}
            if self.fail {
                bail!("deliberate child provider failure");
            }
            if self.write {
                if !has(messages, "read_file") {
                    return Ok(tools(vec![("read_file", json!({"path":"f"}))]));
                }
                if !has(messages, "write_file") {
                    return Ok(tools(vec![(
                        "write_file",
                        json!({"path":"f","content":"new","expected_sha256":aether_tools::hash(b"old")}),
                    )]));
                }
                return Ok(text(
                    "Changed f in isolation; integration not performed by child.",
                ));
            }
            if !self.recursive {
                return Ok(text("Inspected the assigned scope; no edits made."));
            }
        }
        if !has(messages, "spawn_agent") {
            let profile = if self.write { "build" } else { "research" };
            return Ok(tools((0..if root{self.children}else{1}).map(|i|("spawn_agent",json!({"profile":profile,"task":{"title":format!("Worker {i}"),"description":"CHILD: investigate only this task","success_criteria":["Report evidence"]}}))).collect()));
        }
        let children = ids(messages);
        if !children.is_empty() && !has(messages, "await_agent") {
            return Ok(tools(
                children
                    .iter()
                    .map(|id| ("await_agent", json!({"agent_id":id})))
                    .collect(),
            ));
        }
        if self.write && root && !children.is_empty() && !has(messages, "integrate_agent") {
            if self.conflict {
                std::fs::write(self.root.join("f"), "user edit").unwrap();
            }
            return Ok(tools(vec![(
                "integrate_agent",
                json!({"agent_id":children[0]}),
            )]));
        }
        Ok(text(
            "Collected recorded results; task correctness has not been independently verified.",
        ))
    }
}
async fn setup() -> (tempfile::TempDir, Agent, String, Fixture) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    let store = Store::open(dir.path().join("data/sessions.db"))
        .await
        .unwrap();
    let session = store.create(&root, "fixture", "tree").await.unwrap();
    let f = Fixture {
        active: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
        calls: Arc::new(AtomicUsize::new(0)),
        models: Arc::new(Mutex::new(vec![])),
        model: "fixture".into(),
        root: root.clone(),
        children: 2,
        recursive: false,
        fail: false,
        slow: false,
        write: false,
        conflict: false,
    };
    let mut config = Config::default();
    config.provider.model = "fixture".into();
    config.permissions.read = Rule::Allow;
    (
        dir,
        Agent {
            config,
            workspace: root,
            store,
            provider: Arc::new(f.clone()),
        },
        session.id,
        f,
    )
}
async fn run(
    a: &Agent,
    id: &str,
    cancel: CancellationToken,
    approve: bool,
    cancel_child: bool,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel(64);
    let c = cancel.clone();
    let consumer = tokio::spawn(async move {
        while let Some(e) = rx.recv().await {
            match e {
                Event::Permission { reply, .. } => {
                    let _ = reply.send(approve);
                }
                Event::AgentChanged(r)
                    if cancel_child
                        && r.parent_id.is_some()
                        && r.status == AgentStatus::Running =>
                {
                    c.cancel()
                }
                _ => {}
            }
        }
    });
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        a.run(
            id.into(),
            "ROOT: delegate independent work".into(),
            tx,
            cancel,
        ),
    )
    .await
    .expect("scheduler deadlocked");
    consumer.await.unwrap();
    result
}
fn git(root: &std::path::Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap()
            .status
            .success()
    );
}
fn repository(root: &std::path::Path) {
    git(root, &["init", "-q"]);
    std::fs::write(root.join("f"), "old").unwrap();
    std::fs::write(root.join("unrelated"), "baseline").unwrap();
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "base",
        ],
    );
}

#[tokio::test]
async fn real_subagent_engines_are_parallel_and_persist_independent_contexts() {
    let (d, a, id, f) = setup().await;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    let records = a.store.agents(Some(&id)).await.unwrap();
    assert_eq!(records.len(), 3);
    let root = records.iter().find(|r| r.parent_id.is_none()).unwrap();
    assert!(
        records
            .iter()
            .all(|r| r.status == AgentStatus::Completed && r.result.is_some())
    );
    assert_eq!(f.peak.load(Ordering::SeqCst), 2);
    assert_eq!(f.active.load(Ordering::SeqCst), 0);
    for r in records.iter().filter(|r| r.parent_id.is_some()) {
        assert_eq!(r.parent_id.as_deref(), Some(root.id.as_str()));
        let messages = a.store.messages(&r.session_id).await.unwrap();
        assert!(
            messages
                .iter()
                .filter(|m| m.role == "user")
                .all(|m| !m.content.starts_with("ROOT"))
        );
        assert!(!r.result.as_ref().unwrap().task_success_verified);
    }
    let reopened = Store::open(d.path().join("data/sessions.db"))
        .await
        .unwrap();
    assert_eq!(reopened.agents(Some(&id)).await.unwrap().len(), 3);
}
#[tokio::test]
async fn recursive_waits_do_not_hold_single_scheduler_slot() {
    let (_d, mut a, id, mut f) = setup().await;
    f.children = 1;
    f.recursive = true;
    a.provider = Arc::new(f.clone());
    a.config.orchestration.max_concurrent = 1;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    let records = a.store.agents(Some(&id)).await.unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records.iter().map(|r| r.depth).max(), Some(2));
    assert_eq!(f.peak.load(Ordering::SeqCst), 1);
    let deep = records.iter().find(|r| r.depth == 2).unwrap();
    assert!(
        a.store
            .messages(&deep.session_id)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.contains("maximum delegation depth"))
    );
}
#[tokio::test]
async fn agent_count_and_disabled_delegation_are_enforced() {
    for enabled in [true, false] {
        let (_d, mut a, id, mut f) = setup().await;
        f.children = 3;
        a.provider = Arc::new(f);
        a.config.orchestration.enabled = enabled;
        a.config.orchestration.max_agents = 3;
        run(&a, &id, CancellationToken::new(), true, false)
            .await
            .unwrap();
        let records = a.store.agents(Some(&id)).await.unwrap();
        assert_eq!(records.len(), if enabled { 3 } else { 1 });
    }
}
#[tokio::test]
async fn root_cancellation_reaches_children_and_leaves_terminal_history() {
    let (_d, mut a, id, mut f) = setup().await;
    f.slow = true;
    a.provider = Arc::new(f.clone());
    assert!(
        run(&a, &id, CancellationToken::new(), true, true)
            .await
            .is_err()
    );
    let records = a.store.agents(Some(&id)).await.unwrap();
    assert!(records.len() > 1);
    assert!(records.iter().all(|r| r.status.terminal()));
    assert!(
        records
            .iter()
            .any(|r| r.parent_id.is_some() && r.status == AgentStatus::Cancelled)
    );
    assert_eq!(f.active.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn child_failure_is_structured_and_does_not_corrupt_parent() {
    let (_d, mut a, id, mut f) = setup().await;
    f.fail = true;
    a.provider = Arc::new(f);
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    let records = a.store.agents(Some(&id)).await.unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|r| r.status == AgentStatus::Failed)
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.status == AgentStatus::Completed)
            .count(),
        1
    );
    assert!(
        records.iter().filter(|r| r.parent_id.is_some()).all(|r| r
            .error
            .as_deref()
            .unwrap()
            .contains("deliberate"))
    );
}
#[tokio::test]
async fn token_step_and_cost_budgets_block_requests_before_spending() {
    for budget in ["tokens", "steps", "cost", "unknown-price"] {
        let (_d, mut a, id, f) = setup().await;
        match budget {
            "tokens" => a.config.orchestration.total_tokens = 1,
            "steps" => a.config.orchestration.total_steps = 1,
            "cost" => {
                a.config.orchestration.max_cost_usd = Some(0.0);
                a.config.provider.input_cost_per_million = Some(1.0);
                a.config.provider.output_cost_per_million = Some(1.0);
            }
            _ => a.config.orchestration.max_cost_usd = Some(1.0),
        }
        assert!(
            run(&a, &id, CancellationToken::new(), true, false)
                .await
                .is_err(),
            "{budget}"
        );
        assert!(f.calls.load(Ordering::SeqCst) <= usize::from(budget == "steps"));
    }
}
#[tokio::test]
async fn child_model_assignment_uses_provider_reconfiguration() {
    let (_d, mut a, id, f) = setup().await;
    a.config.profiles.get_mut("research").unwrap().model = Some("research-model".into());
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    assert!(
        f.models
            .lock()
            .unwrap()
            .iter()
            .any(|s| s == "research-model")
    );
    assert!(
        a.store
            .agents(Some(&id))
            .await
            .unwrap()
            .iter()
            .filter(|r| r.parent_id.is_some())
            .all(|r| r.model == "research-model")
    );
}
#[tokio::test]
async fn remote_delegation_requires_explicit_spawn_approval() {
    let (_d, mut a, id, f) = setup().await;
    let remote = ProviderConfig {
        base_url: "https://example.invalid/v1".into(),
        model: "private-remote".into(),
        allow_remote: true,
        ..Default::default()
    };
    a.config.providers.insert("remote".into(), remote);
    a.config.profiles.get_mut("research").unwrap().provider = Some("remote".into());
    run(&a, &id, CancellationToken::new(), false, false)
        .await
        .unwrap();
    assert_eq!(a.store.agents(Some(&id)).await.unwrap().len(), 1);
    assert!(f.models.lock().unwrap().iter().all(|s| s == "fixture"));
}
#[tokio::test]
async fn isolated_writes_integrate_only_after_approval_and_preserve_unrelated_edits() {
    let (_d, mut a, id, mut f) = setup().await;
    repository(&a.workspace);
    std::fs::write(a.workspace.join("unrelated"), "user work").unwrap();
    f.children = 1;
    f.write = true;
    a.provider = Arc::new(f);
    a.config.agent.mode = Mode::Build;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(a.workspace.join("f")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(a.workspace.join("unrelated")).unwrap(),
        "user work"
    );
    let records = a.store.agents(Some(&id)).await.unwrap();
    let child = records.iter().find(|r| r.parent_id.is_some()).unwrap();
    assert!(child.isolated);
    assert_ne!(PathBuf::from(&child.workspace), a.workspace);
    assert_eq!(child.integration.as_deref(), Some("applied"));
}
#[tokio::test]
async fn integration_conflict_preserves_user_change() {
    let (_d, mut a, id, mut f) = setup().await;
    repository(&a.workspace);
    f.children = 1;
    f.write = true;
    f.conflict = true;
    a.provider = Arc::new(f);
    a.config.agent.mode = Mode::Build;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(a.workspace.join("f")).unwrap(),
        "user edit"
    );
    assert!(
        a.store
            .messages(&id)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.contains("integration conflict"))
    );
}

#[tokio::test]
async fn recovery_marks_only_idle_nonterminal_agents_interrupted_without_replay() {
    let (_d, a, id, _) = setup().await;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    let mut record = a
        .store
        .agents(Some(&id))
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.parent_id.is_some())
        .unwrap();
    record.status = AgentStatus::Running;
    record.result = None;
    a.store.put_agent(&record).await.unwrap();
    let live_lease = a.store.lease(&id).await.unwrap();
    assert_eq!(a.store.recover_agents_if_idle(&id).await.unwrap(), 0);
    drop(live_lease);
    assert_eq!(a.store.recover_agents_if_idle(&id).await.unwrap(), 1);
    let recovered = a.store.agent(&record.id).await.unwrap();
    assert_eq!(recovered.status, AgentStatus::Interrupted);
    assert!(
        recovered
            .error
            .unwrap()
            .contains("no actions were replayed")
    );
}

#[tokio::test]
async fn child_token_budget_is_independent_and_failures_are_returned() {
    let (_d, mut a, id, f) = setup().await;
    a.config.profiles.get_mut("research").unwrap().max_tokens = 1;
    run(&a, &id, CancellationToken::new(), true, false)
        .await
        .unwrap();
    assert!(
        a.store
            .agents(Some(&id))
            .await
            .unwrap()
            .iter()
            .filter(|r| r.parent_id.is_some())
            .all(|r| r.status == AgentStatus::Failed
                && r.error.as_deref().unwrap().contains("token budget"))
    );
    assert_eq!(
        f.models
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.as_str() == "fixture")
            .count(),
        3
    );
}

#[tokio::test]
async fn interactive_control_cancels_selected_child_not_its_sibling_or_parent() {
    let (_d, mut a, id, mut f) = setup().await;
    f.slow = true;
    a.provider = Arc::new(f);
    let (tx, mut rx) = mpsc::channel(64);
    let (control, commands) = mpsc::channel(8);
    let a2 = a.clone();
    let sid = id.clone();
    let task = tokio::spawn(async move {
        a2.run_controlled(
            sid,
            "ROOT: delegate independent work".into(),
            tx,
            CancellationToken::new(),
            commands,
        )
        .await
    });
    let mut targets = std::collections::BTreeSet::new();
    while let Some(event) = rx.recv().await {
        if let Event::AgentChanged(r) = event
            && r.parent_id.is_some()
            && r.status == AgentStatus::Running
            && targets.insert(r.id.clone())
        {
            let (reply, answer) = tokio::sync::oneshot::channel();
            control
                .send(aether_agent::Control::Cancel { id: r.id, reply })
                .await
                .unwrap();
            answer.await.unwrap().unwrap();
        }
    }
    task.await.unwrap().unwrap();
    let records = a.store.agents(Some(&id)).await.unwrap();
    assert_eq!(targets.len(), 2);
    assert_eq!(
        records
            .iter()
            .filter(|r| r.status == AgentStatus::Cancelled)
            .count(),
        2
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r.parent_id.is_none() && r.status == AgentStatus::Completed)
            .count(),
        1
    );
}
