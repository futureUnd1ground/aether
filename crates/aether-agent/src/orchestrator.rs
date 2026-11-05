//! One runtime owns a bounded task tree. Waiting parents never hold scheduler slots.
use crate::{Agent, Control, Event, workspace};
use aether_core::{
    Message, OperationState, Rule, Usage,
    agents::{AgentRecord, AgentResult, AgentStatus, AgentTask},
    config::{Config, ProviderConfig},
};
use aether_providers::{Completion, Delta, Provider};
use aether_tools::Action;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, future::Future, path::PathBuf, pin::Pin, sync::Arc};
use tokio::sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MANAGEMENT: &[&str] = &[
    "spawn_agent",
    "await_agent",
    "list_agents",
    "cancel_agent",
    "integrate_agent",
];
#[derive(Clone)]
struct Node {
    record: AgentRecord,
    config: Config,
    cancel: CancellationToken,
    tools: Option<Vec<String>>,
    can_delegate: bool,
    instructions: String,
    max_tokens: u64,
    max_cost: Option<f64>,
    provider: Arc<dyn Provider>,
}
struct State {
    nodes: BTreeMap<String, Node>,
    handles: Vec<tokio::task::JoinHandle<()>>,
    admitted: usize,
    tokens: u64,
    cost: f64,
    steps: usize,
}
pub(crate) struct Runtime {
    base: Agent,
    events: mpsc::Sender<Event>,
    slots: Arc<Semaphore>,
    state: Mutex<State>,
    changed: Notify,
    cancel: CancellationToken,
    mutation: Mutex<()>,
    persistence: Mutex<()>,
}
#[derive(Clone)]
pub(crate) struct NodeContext {
    pub runtime: Arc<Runtime>,
    pub id: String,
}

pub(crate) async fn run(
    agent: Agent,
    session: String,
    prompt: String,
    events: mpsc::Sender<Event>,
    cancel: CancellationToken,
    control: Option<mpsc::Receiver<Control>>,
) -> Result<()> {
    let _lease = agent.store.lease(&session).await?;
    agent.store.interrupt_abandoned_agents(&session).await?;
    let id = Uuid::new_v4().to_string();
    let record = AgentRecord {
        id: id.clone(),
        root_id: id.clone(),
        root_session_id: session.clone(),
        parent_id: None,
        session_id: session,
        depth: 0,
        profile: "main".into(),
        task: AgentTask {
            title: prompt.chars().take(80).collect(),
            description: prompt.clone(),
            success_criteria: vec![],
        },
        model: agent.config.provider.model.clone(),
        provider_label: "default".into(),
        workspace: agent.workspace.to_string_lossy().into(),
        isolated: false,
        base_commit: None,
        integration_target: None,
        status: AgentStatus::Created,
        usage: Usage::default(),
        charged_tokens: 0,
        cost_usd: None,
        steps: 0,
        result: None,
        error: None,
        integration: None,
    };
    let node = Node {
        record: record.clone(),
        config: agent.config.clone(),
        cancel: cancel.clone(),
        tools: None,
        can_delegate: agent.config.orchestration.enabled,
        instructions: String::new(),
        max_tokens: agent.config.orchestration.total_tokens,
        max_cost: agent.config.orchestration.max_cost_usd,
        provider: agent.provider.clone(),
    };
    let runtime = Arc::new(Runtime {
        slots: Arc::new(Semaphore::new(agent.config.orchestration.max_concurrent)),
        base: agent,
        events,
        state: Mutex::new(State {
            nodes: BTreeMap::from([(id.clone(), node)]),
            handles: vec![],
            admitted: 1,
            tokens: 0,
            cost: 0.0,
            steps: 0,
        }),
        changed: Notify::new(),
        cancel,
        mutation: Mutex::new(()),
        persistence: Mutex::new(()),
    });
    runtime.base.store.put_agent(&record).await?;
    runtime.emit(Event::AgentChanged(Box::new(record))).await;
    let control_task=control.map(|mut commands|{
        let r=runtime.clone();
        tokio::spawn(async move {
            loop {tokio::select!{
                _=r.cancel.cancelled()=>break,
                command=commands.recv()=>match command {
                    Some(Control::Cancel{id,reply})=>{let outcome=match r.node(&id).await {
                        Ok(node) if !node.record.status.terminal()=>{node.cancel.cancel();Ok(())},
                        Ok(_)=>Err("agent already finished".into()),Err(_)=>Err("agent is not active in this task tree".into()),
                    };let _=reply.send(outcome);},None=>break,
                }
            }}
        })
    });
    let result = runtime.clone().drive(id, prompt).await;
    runtime.cancel.cancel();
    if let Some(task) = control_task {
        task.await.context("agent control worker panicked")?;
    }
    loop {
        let handles = std::mem::take(&mut runtime.state.lock().await.handles);
        if handles.is_empty() {
            break;
        }
        for handle in handles {
            handle.await.context("subagent worker panicked")?;
        }
    }
    result.map(|_| ())
}
impl Runtime {
    async fn emit(&self, event: Event) {
        let _ = self.events.send(event).await;
    }
    // Serialize snapshot writes and read the freshest state after acquiring the gate.
    // An older status event must never overwrite newer usage or a terminal result.
    async fn publish(&self, id: &str) -> Result<()> {
        let _gate = self.persistence.lock().await;
        let record = self.node(id).await?.record;
        self.base.store.put_agent(&record).await?;
        self.changed.notify_waiters();
        self.emit(Event::AgentChanged(Box::new(record))).await;
        Ok(())
    }
    async fn node(&self, id: &str) -> Result<Node> {
        self.state
            .lock()
            .await
            .nodes
            .get(id)
            .cloned()
            .context("agent not found")
    }
    async fn status(&self, id: &str, status: AgentStatus) -> Result<()> {
        let record = {
            let mut state = self.state.lock().await;
            let n = state.nodes.get_mut(id).context("agent not found")?;
            if n.record.status.terminal() {
                return Ok(());
            }
            n.record.status = status;
            n.record.clone()
        };
        self.publish(&record.id).await?;
        Ok(())
    }
    async fn permit(&self, id: &str, cancel: &CancellationToken) -> Result<OwnedSemaphorePermit> {
        self.status(id, AgentStatus::Queued).await?;
        let permit = tokio::select! {biased;_=cancel.cancelled()=>bail!("agent cancelled while queued"),p=self.slots.clone().acquire_owned()=>p?};
        self.status(id, AgentStatus::Running).await?;
        Ok(permit)
    }
    fn drive(
        self: Arc<Self>,
        id: String,
        prompt: String,
    ) -> Pin<Box<dyn Future<Output = Result<AgentResult>> + Send + 'static>> {
        Box::pin(async move {
            let node = self.node(&id).await?;
            let lease = if node.record.parent_id.is_some() {
                Some(self.base.store.lease(&node.record.session_id).await?)
            } else {
                None
            };
            let (tx, mut rx) = mpsc::channel(64);
            let relay_runtime = self.clone();
            let relay_id = id.clone();
            let is_root = node.record.parent_id.is_none();
            let relay = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        Event::State(state) => {
                            let status = match state {
                                OperationState::Created => Some(AgentStatus::Created),
                                OperationState::Queued => Some(AgentStatus::Queued),
                                OperationState::Running => Some(AgentStatus::Running),
                                OperationState::AwaitingPermission => {
                                    Some(AgentStatus::AwaitingPermission)
                                }
                                _ => None,
                            };
                            if let Some(status) = status {
                                relay_runtime.status(&relay_id, status).await?;
                            }
                            if is_root {
                                relay_runtime.emit(Event::State(state)).await;
                            }
                        }
                        Event::Text(text) => {
                            if is_root {
                                relay_runtime.emit(Event::Text(text)).await
                            } else {
                                relay_runtime
                                    .emit(Event::AgentText {
                                        id: relay_id.clone(),
                                        text,
                                    })
                                    .await
                            }
                        }
                        Event::Usage(usage) => {
                            if is_root {
                                relay_runtime.emit(Event::Usage(usage)).await
                            }
                        }
                        Event::Permission { preview, reply } => {
                            let r = relay_runtime.node(&relay_id).await?.record;
                            relay_runtime
                                .emit(Event::Permission {
                                    preview: format!(
                                        "Agent {} · {}\nWorkspace: {}\n\n{preview}",
                                        r.task.title, r.id, r.workspace
                                    ),
                                    reply,
                                })
                                .await;
                        }
                        Event::Tool { name, detail } => {
                            if is_root {
                                relay_runtime.emit(Event::Tool { name, detail }).await
                            } else {
                                relay_runtime
                                    .emit(Event::AgentActivity {
                                        id: relay_id.clone(),
                                        name,
                                        detail,
                                    })
                                    .await
                            }
                        }
                        Event::Finished { error } => {
                            if is_root {
                                relay_runtime.emit(Event::Finished { error }).await
                            }
                        }
                        Event::AgentChanged(record) => {
                            relay_runtime.emit(Event::AgentChanged(record)).await
                        }
                        Event::AgentText { id, text } => {
                            relay_runtime.emit(Event::AgentText { id, text }).await
                        }
                        Event::AgentActivity { id, name, detail } => {
                            relay_runtime
                                .emit(Event::AgentActivity { id, name, detail })
                                .await
                        }
                    }
                }
                Ok::<_, anyhow::Error>(())
            });
            let provider = Arc::new(BudgetProvider {
                inner: node.provider.clone(),
                runtime: self.clone(),
                id: id.clone(),
                config: node.config.provider.clone(),
            });
            let engine = Agent {
                config: node.config.clone(),
                workspace: PathBuf::from(&node.record.workspace),
                store: self.base.store.clone(),
                provider,
            };
            let result = engine
                .run_inner(
                    node.record.session_id.clone(),
                    prompt,
                    tx,
                    node.cancel.clone(),
                    NodeContext {
                        runtime: self.clone(),
                        id: id.clone(),
                    },
                )
                .await;
            let relay_result = relay
                .await
                .context("agent event relay panicked")
                .and_then(|r| r);
            // Finishing a parent cancels any unawaited descendants. Their worktrees/history remain.
            self.cancel_descendants(&id).await;
            let result = result.and(relay_result);
            let timeline = self.base.store.timeline(&node.record.session_id).await?;
            let result = match result {
                Ok(()) => {
                    let messages = self.base.store.messages(&node.record.session_id).await?;
                    let summary = messages
                        .iter()
                        .rev()
                        .find(|m| {
                            m.role == "assistant"
                                && m.tool_calls.is_empty()
                                && !m.content.trim().is_empty()
                        })
                        .map(|m| m.content.clone())
                        .context("no final assistant response persisted")?;
                    let tool_results: Vec<_> = timeline
                        .iter()
                        .filter(|e| e.kind == "tool_result")
                        .collect();
                    Ok(AgentResult {
                        summary,
                        task_success_verified: false,
                        tools_executed: tool_results.len(),
                        tool_errors: tool_results
                            .iter()
                            .filter(|e| {
                                e.payload["result"].get("error").is_some()
                                    || e.payload["result"]["success"] == false
                            })
                            .count(),
                        command_exit_codes: tool_results
                            .iter()
                            .filter(|e| e.payload["name"] == "shell")
                            .map(|e| e.payload["result"]["exit_code"].as_i64().map(|v| v as i32))
                            .collect(),
                    })
                }
                Err(e) => Err(e),
            };
            let record = {
                let mut state = self.state.lock().await;
                let n = state.nodes.get_mut(&id).context("agent vanished")?;
                n.record.status = if node.cancel.is_cancelled() {
                    AgentStatus::Cancelled
                } else if result.is_ok() {
                    AgentStatus::Completed
                } else {
                    AgentStatus::Failed
                };
                n.record.result = result.as_ref().ok().cloned();
                n.record.error = result.as_ref().err().map(ToString::to_string);
                n.record.clone()
            };
            self.publish(&record.id).await?;
            drop(lease);
            result
        })
    }
    async fn descendant(&self, child: &str, ancestor: &str) -> bool {
        let state = self.state.lock().await;
        is_descendant(&state.nodes, child, ancestor)
    }
    async fn cancel_descendants(&self, parent: &str) {
        let state = self.state.lock().await;
        for (id, n) in &state.nodes {
            if is_descendant(&state.nodes, id, parent) {
                n.cancel.cancel();
            }
        }
    }
    async fn request_permission(
        &self,
        parent: &str,
        preview: String,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let node = self.node(parent).await?;
        self.base
            .store
            .append(
                &node.record.session_id,
                "delegation_permission",
                json!({"preview":preview}),
            )
            .await?;
        self.status(parent, AgentStatus::AwaitingPermission).await?;
        let (reply, rx) = oneshot::channel();
        self.emit(Event::Permission { preview, reply }).await;
        let approved = tokio::select! {_=cancel.cancelled()=>false,r=rx=>r.unwrap_or(false)};
        self.base
            .store
            .append(
                &node.record.session_id,
                "delegation_permission_decision",
                json!({"approved":approved}),
            )
            .await?;
        self.status(parent, AgentStatus::Running).await?;
        if !approved {
            bail!("delegation/integration permission denied");
        }
        Ok(())
    }
    async fn spawn(self: &Arc<Self>, parent: &str, args: SpawnArgs) -> Result<Value> {
        let parent_node = self.node(parent).await?;
        if parent_node.cancel.is_cancelled() {
            bail!("parent agent is cancelled");
        }
        if !self.base.config.orchestration.enabled || !parent_node.can_delegate {
            bail!("delegation disabled for this agent");
        }
        let o = &self.base.config.orchestration;
        if parent_node.record.depth >= o.max_depth {
            bail!("maximum delegation depth reached");
        }
        if args.task.title.trim().is_empty()
            || args.task.description.trim().is_empty()
            || args.task.description.len() > 32768
            || args.task.title.len() > 200
            || args.task.success_criteria.len() > 32
            || args.task.success_criteria.iter().any(|s| s.len() > 4096)
        {
            bail!("invalid task size/title/description");
        }
        let profile = self
            .base
            .config
            .profiles
            .get(&args.profile)
            .context("unknown agent profile")?
            .clone();
        if profile.mode.writable() && !parent_node.config.agent.mode.writable() {
            bail!("read-only parent cannot delegate a writing profile");
        }
        {
            let mut s = self.state.lock().await;
            if s.admitted >= o.max_agents {
                bail!("maximum agents per task reached (root included)");
            }
            s.admitted += 1;
        }
        let id = Uuid::new_v4().to_string();
        let mut config = parent_node.config.clone();
        config.agent.mode = profile.mode;
        config.agent.max_steps = profile.max_steps;
        config.agent.context_tokens = profile.context_tokens;
        config.agent.timeout_secs = profile.timeout_secs;
        config.agent.output_tokens = config
            .agent
            .output_tokens
            .min(profile.context_tokens.saturating_sub(2304).max(128));
        config.permissions.read = intersect(
            parent_node.config.permissions.read,
            profile.permissions.read,
        );
        config.permissions.write = intersect(
            parent_node.config.permissions.write,
            profile.permissions.write,
        );
        config.permissions.shell = intersect(
            parent_node.config.permissions.shell,
            profile.permissions.shell,
        );
        let label = profile
            .provider
            .clone()
            .unwrap_or_else(|| parent_node.record.provider_label.clone());
        if let Some(alias) = &profile.provider {
            config.provider = self
                .base
                .config
                .providers
                .get(alias)
                .context("profile provider missing")?
                .clone();
        }
        if let Some(model) = &profile.model {
            config.provider.model = model.clone();
        }
        config.validate()?;
        let changed_provider = config.provider.base_url != parent_node.config.provider.base_url
            || config.provider.model != parent_node.config.provider.model;
        let writing = profile.mode.writable();
        if writing || changed_provider && config.provider.allow_remote {
            self.request_permission(parent,format!("Spawn {}: {}\nModel: {} ({})\nEndpoint: {}\n{}\n{}",args.profile,args.task.title,config.provider.model,label,config.provider.base_url,if writing{"Create a detached Git worktree from committed HEAD. Uncommitted parent edits are excluded. Changes require separate integration approval; no automatic merge."}else{"Task and approved file data will be sent to this explicitly configured remote model."},args.task.description),&parent_node.cancel).await?;
        }
        let scoped = workspace::scoped_directory(
            PathBuf::from(&parent_node.record.workspace).as_path(),
            &profile.scope,
        )
        .await?;
        let (root, base) = if writing {
            let _slot = self.permit(parent, &parent_node.cancel).await?;
            let (p, b) =
                workspace::isolate(&scoped, &self.base.store.worktree_directory(), &id).await?;
            (p, Some(b))
        } else {
            (scoped.clone(), None)
        };
        if parent_node.cancel.is_cancelled() {
            bail!(
                "parent cancelled; inspect retained workspace {}",
                root.display()
            );
        }
        let provider = if !changed_provider {
            parent_node.provider.clone()
        } else {
            self.base.provider.configured(config.provider.clone())?
        };
        let session = self
            .base
            .store
            .create(&root, &config.provider.model, &args.task.title)
            .await?;
        let record = AgentRecord {
            id: id.clone(),
            root_id: parent_node.record.root_id.clone(),
            root_session_id: parent_node.record.root_session_id.clone(),
            parent_id: Some(parent.into()),
            session_id: session.id.clone(),
            depth: parent_node.record.depth + 1,
            profile: args.profile,
            task: args.task.clone(),
            model: config.provider.model.clone(),
            provider_label: label,
            workspace: root.to_string_lossy().into(),
            isolated: writing,
            base_commit: base,
            integration_target: if writing {
                Some(scoped.to_string_lossy().into())
            } else {
                None
            },
            status: AgentStatus::Created,
            usage: Usage::default(),
            charged_tokens: 0,
            cost_usd: None,
            steps: 0,
            result: None,
            error: None,
            integration: None,
        };
        let allowed = profile
            .tools
            .iter()
            .filter(|t| {
                parent_node
                    .tools
                    .as_ref()
                    .is_none_or(|parent_tools| parent_tools.contains(t))
            })
            .cloned()
            .collect();
        let node = Node {
            record: record.clone(),
            config,
            cancel: parent_node.cancel.child_token(),
            tools: Some(allowed),
            can_delegate: profile.can_delegate,
            instructions: profile.instructions,
            max_tokens: profile.max_tokens,
            max_cost: profile.max_cost_usd,
            provider,
        };
        self.base.store.put_agent(&record).await?;
        self.base
            .store
            .append(
                &parent_node.record.session_id,
                "agent_spawned",
                json!({"agent_id":id,"profile":record.profile,"session_id":session.id}),
            )
            .await?;
        self.state.lock().await.nodes.insert(id.clone(), node);
        self.emit(Event::AgentChanged(Box::new(record.clone())))
            .await;
        let runtime = self.clone();
        let agent_id = id.clone();
        let prompt = format!(
            "Task: {}\n{}\nSuccess criteria:\n{}",
            args.task.title,
            args.task.description,
            args.task.success_criteria.join("\n")
        );
        let task = tokio::spawn(async move {
            if let Err(e) = runtime.clone().drive(agent_id.clone(), prompt).await {
                tracing::warn!(agent_id,error=%e,"subagent ended with error");
                // Driver errors before its terminal-state transaction must still be inspectable.
                if let Ok(mut n) = runtime.node(&agent_id).await
                    && !n.record.status.terminal()
                {
                    n.record.status = AgentStatus::Failed;
                    n.record.error = Some(e.to_string());
                    let record = n.record.clone();
                    runtime.state.lock().await.nodes.insert(agent_id, n);
                    let _ = runtime.base.store.put_agent(&record).await;
                    runtime.changed.notify_waiters();
                    runtime.emit(Event::AgentChanged(Box::new(record))).await;
                }
            }
        });
        self.state.lock().await.handles.push(task);
        Ok(
            json!({"agent_id":id,"session_id":session.id,"status":"created","workspace":record.workspace,"isolated":writing,"note":"Call await_agent to collect the structured result. Unawaited descendants are cancelled when their parent finishes; history/worktrees are retained."}),
        )
    }
    async fn await_agent(&self, parent: &str, id: &str) -> Result<Value> {
        if !self.descendant(id, parent).await {
            bail!("can only await your own descendants; waiting on self/ancestors is forbidden");
        }
        let cancel = self.node(parent).await?.cancel;
        self.status(parent, AgentStatus::Waiting).await?;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let r = self.node(id).await?.record;
            if r.status.terminal() {
                self.status(parent, AgentStatus::Running).await?;
                return Ok(serde_json::to_value(r)?);
            }
            tokio::select! {_=cancel.cancelled()=>bail!("await_agent cancelled"),_=changed=>{}}
        }
    }
    async fn integrate(&self, parent: &str, id: &str) -> Result<Value> {
        if !self.descendant(id, parent).await {
            bail!("only your own descendants may be integrated");
        }
        let parent_node = self.node(parent).await?;
        if !parent_node.config.agent.mode.writable()
            || parent_node.config.permissions.write == Rule::Deny
        {
            bail!("integration requires writable mode and write permission");
        }
        let child = self.node(id).await?.record;
        if child.parent_id.as_deref() != Some(parent) {
            bail!("integrate into the direct parent first");
        }
        let parent_scope = PathBuf::from(&parent_node.record.workspace);
        let parent_root = workspace::scoped_directory(
            &parent_scope,
            child
                .integration_target
                .as_deref()
                .context("missing integration target")?,
        )
        .await?;
        let slot = self.permit(parent, &parent_node.cancel).await?;
        let plan = workspace::plan(&parent_root, &child).await?;
        drop(slot);
        if plan.changes.is_empty() {
            return Ok(
                json!({"changed":false,"reason":"worktree has no supported content changes"}),
            );
        }
        self.request_permission(parent,format!("Integrate agent {} into {}\n{} file(s). Every parent file must match the original base.\n\n{}",child.task.title,parent_root.display(),plan.changes.len(),plan.diff),&parent_node.cancel).await?;
        let _slot = self.permit(parent, &parent_node.cancel).await?;
        let _mutation = self.mutation.lock().await;
        let current = workspace::plan(&parent_root, &child).await?;
        if current
            .changes
            .iter()
            .map(|c| (&c.path, &c.before, &c.after))
            .collect::<Vec<_>>()
            != plan
                .changes
                .iter()
                .map(|c| (&c.path, &c.before, &c.after))
                .collect::<Vec<_>>()
        {
            bail!("worktree changed after approval; request a new preview");
        }
        self.base
            .store
            .append(
                &parent_node.record.session_id,
                "integration_checkpoint",
                serde_json::to_value(&plan)?,
            )
            .await?;
        let result = workspace::apply(&parent_root, &plan, parent_node.cancel.child_token()).await;
        let mut record = self.node(id).await?.record;
        record.integration = Some(
            if result.is_ok() {
                "applied"
            } else {
                "failed; inspect integration checkpoint and rollback diagnostics"
            }
            .into(),
        );
        self.state
            .lock()
            .await
            .nodes
            .get_mut(id)
            .context("agent vanished")?
            .record = record.clone();
        self.publish(&record.id).await?;
        result?;
        Ok(
            json!({"integrated":id,"files":plan.changes.iter().map(|c|c.path.clone()).collect::<Vec<_>>(),"task_success_verified":false}),
        )
    }
}
impl NodeContext {
    pub async fn definitions(&self, mode: aether_core::Mode) -> Result<Vec<Value>> {
        let node = self.runtime.node(&self.id).await?;
        let mut definitions = aether_tools::definitions(mode);
        if self.runtime.base.config.orchestration.enabled && node.can_delegate {
            let profiles = self
                .runtime
                .base
                .config
                .profiles
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            definitions.push(schema("spawn_agent",&format!("Start a real independent subagent. Profiles: {profiles}. Read-only work shares scope; writers require approval and use committed-HEAD Git worktrees. You must await results before finishing."),json!({"profile":{"type":"string"},"task":{"type":"object","properties":{"title":{"type":"string"},"description":{"type":"string"},"success_criteria":{"type":"array","items":{"type":"string"}}},"required":["title","description"],"additionalProperties":false}}),&["profile","task"]));
            definitions.push(schema("await_agent","Wait for a descendant's durable structured result without holding an execution slot.",json!({"agent_id":{"type":"string"}}),&["agent_id"]));
            definitions.push(schema(
                "list_agents",
                "List yourself and descendants in the current task tree.",
                json!({}),
                &[],
            ));
            definitions.push(schema("cancel_agent","Cancel a descendant and its children. History and isolated changes remain inspectable.",json!({"agent_id":{"type":"string"}}),&["agent_id"]));
            if mode.writable() {
                definitions.push(schema("integrate_agent","Preview and explicitly approve a completed direct child's isolated text changes. Conflicts refuse all integration; no automatic merge.",json!({"agent_id":{"type":"string"}}),&["agent_id"]));
            }
        }
        if let Some(allowed) = &node.tools {
            definitions.retain(|v| {
                v["function"]["name"]
                    .as_str()
                    .is_some_and(|s| allowed.iter().any(|a| a == s))
            });
        }
        Ok(definitions)
    }
    pub async fn instructions(&self) -> Result<String> {
        let node = self.runtime.node(&self.id).await?;
        Ok(format!(
            "{}\nYou are agent {} at depth {}. Workspace: {}. Each child has independent context; include all necessary constraints in its task. Await every child you need before finalizing; unfinished descendants are cancelled on parent completion. A completed response does not prove task correctness. Never claim child changes were applied unless integrate_agent succeeded. {}",
            node.instructions,
            self.id,
            node.record.depth,
            node.record.workspace,
            if node.record.isolated {
                "Your shell uses a sanitized offline Linux bubblewrap environment; no access to host home credentials, and only this workspace is writable. The worktree excludes uncommitted parent changes."
            } else {
                ""
            }
        ))
    }
    pub async fn managed(&self, name: &str, args: &str) -> Result<Option<Value>> {
        if !MANAGEMENT.contains(&name) {
            return Ok(None);
        }
        let node = self.runtime.node(&self.id).await?;
        let allowed = self
            .definitions(node.config.agent.mode)
            .await?
            .iter()
            .any(|d| d["function"]["name"] == name);
        if !allowed {
            bail!("management tool denied by profile or mode");
        }
        let value = match name {
            "spawn_agent" => {
                self.runtime
                    .spawn(&self.id, serde_json::from_str(args)?)
                    .await?
            }
            "await_agent" => {
                let a: IdArgs = serde_json::from_str(args)?;
                self.runtime.await_agent(&self.id, &a.agent_id).await?
            }
            "cancel_agent" => {
                let a: IdArgs = serde_json::from_str(args)?;
                if !self.runtime.descendant(&a.agent_id, &self.id).await {
                    bail!("cannot cancel self, ancestors or unrelated agents");
                }
                self.runtime.node(&a.agent_id).await?.cancel.cancel();
                json!({"cancel_requested":a.agent_id})
            }
            "list_agents" => {
                let _: EmptyArgs = serde_json::from_str(args)?;
                let state = self.runtime.state.lock().await;
                json!(
                    state
                        .nodes
                        .iter()
                        .filter(|(id, _)| id.as_str() == self.id
                            || is_descendant(&state.nodes, id, &self.id))
                        .map(|(_, n)| n.record.clone())
                        .collect::<Vec<_>>()
                )
            }
            "integrate_agent" => {
                let a: IdArgs = serde_json::from_str(args)?;
                self.runtime.integrate(&self.id, &a.agent_id).await?
            }
            _ => unreachable!(),
        };
        Ok(Some(value))
    }
    pub async fn check_tool(&self, name: &str) -> Result<()> {
        let n = self.runtime.node(&self.id).await?;
        if n.tools
            .as_ref()
            .is_some_and(|allowed| !allowed.iter().any(|t| t == name))
        {
            bail!("tool denied by agent profile");
        }
        Ok(())
    }
    pub async fn preview(&self, action: &Action) -> Result<String> {
        let n = self.runtime.node(&self.id).await?;
        if let Action::Shell { command } = action
            && n.record.parent_id.is_some()
        {
            return Ok(format!(
                "Execute in sanitized, offline Linux bubblewrap sandbox. Only the isolated workspace and ephemeral temporary directories are writable. No unrestricted fallback.\nWorkspace: {}\n{command}",
                n.record.workspace
            ));
        }
        Ok(action.preview())
    }
    pub async fn execute(&self, action: Action, cancel: CancellationToken) -> Result<Value> {
        let node = self.runtime.node(&self.id).await?;
        let _slot = self.runtime.permit(&self.id, &cancel).await?;
        if let Action::Shell { command } = &action
            && node.record.parent_id.is_some()
        {
            if !node.record.isolated {
                bail!("subagent shell requires an isolated workspace");
            }
            return aether_tools::isolated_shell(
                PathBuf::from(node.record.workspace),
                command.clone(),
                cancel,
            )
            .await;
        }
        let _mutation = if matches!(action, Action::Write { .. }) {
            Some(self.runtime.mutation.lock().await)
        } else {
            None
        };
        aether_tools::execute(PathBuf::from(node.record.workspace), action, cancel).await
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnArgs {
    profile: String,
    task: AgentTask,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdArgs {
    agent_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyArgs {}
fn intersect(parent: Rule, child: Rule) -> Rule {
    if parent == Rule::Deny || child == Rule::Deny {
        Rule::Deny
    } else if parent == Rule::Ask || child == Rule::Ask {
        Rule::Ask
    } else {
        Rule::Allow
    }
}
fn is_descendant(nodes: &BTreeMap<String, Node>, child: &str, ancestor: &str) -> bool {
    let mut current = nodes.get(child).and_then(|n| n.record.parent_id.as_deref());
    let mut seen = 0;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        seen += 1;
        if seen > nodes.len() {
            return false;
        }
        current = nodes.get(id).and_then(|n| n.record.parent_id.as_deref());
    }
    false
}
fn schema(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}

struct Reservation {
    tokens: u64,
    cost: f64,
}
struct BudgetProvider {
    inner: Arc<dyn Provider>,
    runtime: Arc<Runtime>,
    id: String,
    config: ProviderConfig,
}
#[async_trait::async_trait]
impl Provider for BudgetProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[Value],
        max_tokens: usize,
        events: mpsc::Sender<Delta>,
        cancel: CancellationToken,
    ) -> Result<Completion> {
        let _slot = self.runtime.permit(&self.id, &cancel).await?;
        // A conservative byte-based input reservation includes tool definitions and framing.
        // Actual provider usage replaces it only when usage is reported; errors retain it.
        let input = messages
            .iter()
            .map(|m| serde_json::to_vec(m).map(|s| s.len() as u64))
            .collect::<serde_json::Result<Vec<_>>>()?
            .into_iter()
            .sum::<u64>()
            + serde_json::to_vec(tools)?.len() as u64
            + 1024;
        let reserve = Reservation {
            tokens: input + max_tokens as u64,
            cost: price(&self.config, input, max_tokens as u64).unwrap_or(0.0),
        };
        let node = self.runtime.node(&self.id).await?;
        if (self
            .runtime
            .base
            .config
            .orchestration
            .max_cost_usd
            .is_some()
            || node.max_cost.is_some())
            && price(&self.config, 0, 0).is_none()
        {
            bail!(
                "cost budget requires explicit provider input/output token prices; unknown pricing is never treated as free"
            );
        }
        let record = {
            let mut state = self.runtime.state.lock().await;
            let n = state.nodes.get(&self.id).context("agent missing")?;
            if state.steps >= self.runtime.base.config.orchestration.total_steps {
                bail!("total execution step budget exhausted");
            }
            if state.tokens.saturating_add(reserve.tokens)
                > self.runtime.base.config.orchestration.total_tokens
                || n.record.charged_tokens.saturating_add(reserve.tokens) > n.max_tokens
            {
                bail!("agent/task token budget exhausted before provider request");
            }
            if self
                .runtime
                .base
                .config
                .orchestration
                .max_cost_usd
                .is_some_and(|cap| state.cost + reserve.cost > cap)
                || n.max_cost
                    .is_some_and(|cap| n.record.cost_usd.unwrap_or(0.0) + reserve.cost > cap)
            {
                bail!("agent/task cost budget exhausted before provider request");
            }
            state.steps += 1;
            state.tokens += reserve.tokens;
            state.cost += reserve.cost;
            let n = state.nodes.get_mut(&self.id).context("agent missing")?;
            n.record.steps += 1;
            n.record.charged_tokens += reserve.tokens;
            if price(&self.config, 0, 0).is_some() {
                n.record.cost_usd = Some(n.record.cost_usd.unwrap_or(0.0) + reserve.cost);
            }
            n.record.clone()
        };
        self.runtime.publish(&record.id).await?;
        let completion = self
            .inner
            .complete(messages, tools, max_tokens, events, cancel.clone())
            .await?;
        let usage = &completion.usage;
        let actual = if usage.prompt_tokens.saturating_add(usage.completion_tokens) == 0 {
            reserve.tokens
        } else {
            usage.prompt_tokens.saturating_add(usage.completion_tokens)
        };
        let cost = if usage.prompt_tokens.saturating_add(usage.completion_tokens) == 0 {
            reserve.cost
        } else {
            price(&self.config, usage.prompt_tokens, usage.completion_tokens).unwrap_or(0.0)
        };
        let (record, overflow) = {
            let mut state = self.runtime.state.lock().await;
            state.tokens = state
                .tokens
                .saturating_sub(reserve.tokens)
                .saturating_add(actual);
            state.cost = (state.cost - reserve.cost).max(0.0) + cost;
            let n = state.nodes.get_mut(&self.id).context("agent missing")?;
            n.record.charged_tokens = n
                .record
                .charged_tokens
                .saturating_sub(reserve.tokens)
                .saturating_add(actual);
            n.record.usage.prompt_tokens = n
                .record
                .usage
                .prompt_tokens
                .saturating_add(usage.prompt_tokens);
            n.record.usage.completion_tokens = n
                .record
                .usage
                .completion_tokens
                .saturating_add(usage.completion_tokens);
            if n.record.cost_usd.is_some() {
                n.record.cost_usd =
                    Some((n.record.cost_usd.unwrap_or(0.0) - reserve.cost).max(0.0) + cost);
            }
            let overflow = n.record.charged_tokens > n.max_tokens
                || n.max_cost
                    .is_some_and(|cap| n.record.cost_usd.unwrap_or(0.0) > cap);
            let r = n.record.clone();
            (
                r,
                overflow
                    || state.tokens > self.runtime.base.config.orchestration.total_tokens
                    || self
                        .runtime
                        .base
                        .config
                        .orchestration
                        .max_cost_usd
                        .is_some_and(|cap| state.cost > cap),
            )
        };
        self.runtime.publish(&record.id).await?;
        if overflow {
            self.runtime.cancel.cancel();
            bail!(
                "provider-reported usage exceeded the reservation/budget; task tree cancelled before executing tools"
            );
        }
        Ok(completion)
    }
}
fn price(config: &ProviderConfig, input: u64, output: u64) -> Option<f64> {
    Some(
        (input as f64 * config.input_cost_per_million?
            + output as f64 * config.output_cost_per_million?)
            / 1_000_000.0,
    )
}
