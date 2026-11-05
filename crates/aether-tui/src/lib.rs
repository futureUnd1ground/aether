mod agents;
use aether_agent::{Agent, Control, Event as AgentEvent};
use aether_core::agents::AgentRecord;
use aether_core::{Message, Session};
use agents::{AgentTree, format_messages};
use anyhow::{Context, Result};
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
    },
    execute,
};
use futures_util::StreamExt;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::collections::VecDeque;
use std::{
    io::{self, IsTerminal},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tui_textarea::TextArea;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
        ratatui::restore();
    }
}
struct View<'a> {
    input: TextArea<'a>,
    transcript: String,
    status: String,
    scroll: u16,
    follow: bool,
    help: bool,
    sessions: Vec<Session>,
    browser: bool,
    selected: usize,
    permission: Option<(String, oneshot::Sender<bool>)>,
    pending_permissions: VecDeque<(String, oneshot::Sender<bool>)>,
    tree: AgentTree,
}
impl View<'_> {
    fn new(messages: &[Message], draft: &str) -> Self {
        let mut input = TextArea::new(draft.split('\n').map(str::to_owned).collect());
        input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message · Enter send · Alt+Enter newline "),
        );
        input.set_style(
            Style::default()
                .fg(Color::Rgb(207, 220, 240))
                .bg(Color::Rgb(12, 18, 30)),
        );
        input.set_cursor_line_style(Style::default());
        Self {
            input,
            transcript: format_messages(messages),
            status: "Ready · Ctrl+H help".into(),
            scroll: 0,
            follow: true,
            help: false,
            sessions: vec![],
            browser: false,
            selected: 0,
            permission: None,
            pending_permissions: VecDeque::new(),
            tree: AgentTree::default(),
        }
    }
    fn queue_permission(&mut self, preview: String, reply: oneshot::Sender<bool>) {
        if self.permission.is_none() {
            self.scroll = 0;
            self.permission = Some((preview, reply));
        } else if self.pending_permissions.len() < 32 {
            self.pending_permissions.push_back((preview, reply));
        } else {
            let _ = reply.send(false);
            self.status = "Permission queue full; request denied".into();
        }
    }
    fn answer_permission(&mut self, approved: bool) {
        if let Some((_, reply)) = self.permission.take() {
            let _ = reply.send(approved);
        }
        self.permission = self.pending_permissions.pop_front();
        self.scroll = 0;
    }
    fn draw(&mut self, f: &mut Frame, session: &Session, model: &str, mode: &str, _budget: usize) {
        let area = f.area();
        f.render_widget(
            Block::default().style(
                Style::default()
                    .bg(Color::Rgb(12, 18, 30))
                    .fg(Color::Rgb(207, 220, 240)),
            ),
            area,
        );
        let rows = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(2),
            Constraint::Length(6),
            Constraint::Length(2),
        ])
        .split(area);
        f.render_widget(
            Paragraph::new(format!(
                " AETHER  ·  {}  ·  {}\n {}",
                model,
                session.title.chars().take(55).collect::<String>(),
                session.workspace
            ))
            .style(Style::default().fg(Color::Rgb(125, 207, 255))),
            rows[0],
        );
        let columns = if area.width >= 100 {
            Layout::horizontal([Constraint::Length(32), Constraint::Min(1)]).split(rows[1])
        } else {
            Layout::horizontal([Constraint::Length(0), Constraint::Min(1)]).split(rows[1])
        };
        if columns[0].width > 0 {
            self.tree.render(f, columns[0]);
        }
        let inspected = self.tree.body();
        let displayed = inspected.as_deref().unwrap_or(&self.transcript);
        let transcript = Paragraph::new(displayed)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(48, 61, 85)))
                    .title(if inspected.is_some() {
                        " Agent detail · F2 / Backspace parent "
                    } else {
                        " Session timeline "
                    }),
            )
            .wrap(Wrap { trim: false });
        if self.follow {
            self.scroll = transcript
                .line_count(columns[1].width.saturating_sub(2))
                .saturating_sub(columns[1].height.saturating_sub(2) as usize)
                .min(u16::MAX as usize) as u16;
        }
        f.render_widget(transcript.scroll((self.scroll, 0)), columns[1]);
        f.render_widget(&self.input, rows[2]);
        f.render_widget(
            Paragraph::new(format!(
                " {}\n {}",
                format_args!(
                    "{mode} · {model} · {} live agents · {} / {} tokens (session) · {}",
                    self.tree
                        .records
                        .iter()
                        .filter(|r| !r.status.terminal())
                        .count(),
                    self.tree
                        .records
                        .iter()
                        .map(|r| r.usage.prompt_tokens)
                        .sum::<u64>(),
                    self.tree
                        .records
                        .iter()
                        .map(|r| r.usage.completion_tokens)
                        .sum::<u64>(),
                    self.status
                ),
                if self.follow {
                    "Following output"
                } else {
                    "Scroll · End to follow"
                }
            )),
            rows[3],
        );
        if columns[0].width == 0 && self.tree.focused {
            let p = popup(area, 95, 80);
            f.render_widget(Clear, p);
            self.tree.render(f, p);
        }
        if self.browser {
            let popup = popup(area, 80, 70);
            f.render_widget(Clear, popup);
            let text = self
                .sessions
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    format!(
                        "{} {}  {}",
                        if i == self.selected { "▶" } else { " " },
                        s.id.chars().take(8).collect::<String>(),
                        s.title
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            f.render_widget(
                Paragraph::new(text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Sessions · ↑↓ Enter · Esc close "),
                    )
                    .scroll((
                        self.selected
                            .saturating_sub(popup.height.saturating_sub(3) as usize)
                            as u16,
                        0,
                    )),
                popup,
            );
        }
        if self.help {
            let p = popup(area, 80, 70);
            f.render_widget(Clear, p);
            f.render_widget(Paragraph::new("Enter: send     Alt+Enter: newline\nCtrl+Q: quit    Esc: cancel current turn\nCtrl+S: browse saved sessions\nCtrl+N: create session\nCtrl+H: help\nF2: agent tree · ↑↓ choose · Enter inspect\nBackspace: parent · x: cancel selected agent\nF5: refresh selected agent history\nPageUp/PageDown or mouse wheel: scroll\nEnd: follow output\nEditor: arrows, Home/End, Ctrl+Z undo, Ctrl+Y redo\nBracketed paste supported. Drafts save after 500ms idle.\nPermission dialog: y approve once, n/Esc deny\nCtrl+Up/Down scrolls a long permission preview.\n\nEvery shell command requires fresh approval.\nAsk, Plan and Review are read-only.\nAgent responses do not independently verify task correctness.\nNo automatic compaction, plugins or MCP in this release.").block(Block::default().borders(Borders::ALL).title(" Help ")).wrap(Wrap{trim:false}),p);
        }
        if let Some((preview, _)) = &self.permission {
            let p = popup(area, 90, 85);
            f.render_widget(Clear, p);
            f.render_widget(
                Paragraph::new(preview.as_str())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Permission · y once · n deny · Ctrl+↑↓ scroll "),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0)),
                p,
            );
        }
    }
}
fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let w = area.width * width / 100;
    let h = area.height * height / 100;
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

enum Navigation {
    List(Vec<Session>),
    Open(Session, Vec<Message>, Vec<AgentRecord>),
    Agent(String, Vec<Message>),
}

pub async fn run(agent: Agent, mut session: Session) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("TUI requires a terminal; use `aether run` for headless execution");
    }
    let messages = agent.store.messages(&session.id).await?;
    agent.store.recover_agents_if_idle(&session.id).await?;
    let mut view = View::new(&messages, &session.draft);
    view.tree.records = agent.store.agents(Some(&session.id)).await?;
    view.sessions = agent.store.list(None).await?;
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
    let mut input = EventStream::new();
    let (tx, mut rx) = mpsc::channel(64);
    let mut task: Option<tokio::task::JoinHandle<Result<()>>> = None;
    let mut cancel = CancellationToken::new();
    let mut draft_dirty = false;
    let mut draft_at = tokio::time::Instant::now();
    let mut dirty = true;
    let mut save_task: Option<tokio::task::JoinHandle<Result<()>>> = None;
    let mut navigation: Option<tokio::task::JoinHandle<Result<Navigation>>> = None;
    let mut control: Option<mpsc::Sender<Control>> = None;
    let mut cancel_reply: Option<oneshot::Receiver<Result<(), String>>> = None;
    loop {
        if dirty {
            terminal.draw(|f| {
                view.draw(
                    f,
                    &session,
                    &agent.config.provider.model,
                    &format!("{:?}", agent.config.agent.mode),
                    agent.config.agent.context_tokens,
                )
            })?;
            dirty = false;
        }
        tokio::select! {
            event=input.next()=>{let Some(event)=event else{break};let event=event?;dirty=true;match event {
                Event::Key(key) if key.kind!=KeyEventKind::Release=>{
                    let ctrl=key.modifiers.contains(KeyModifiers::CONTROL);
                    if ctrl&&key.code==KeyCode::Char('q'){break;}
                    if view.permission.is_some() {
                        if ctrl&&matches!(key.code,KeyCode::Up|KeyCode::Down){if key.code==KeyCode::Up{view.scroll=view.scroll.saturating_sub(3)}else{view.scroll=view.scroll.saturating_add(3)};}
                        else if matches!(key.code,KeyCode::Char('y'|'n')|KeyCode::Esc){view.answer_permission(key.code==KeyCode::Char('y'));}
                        continue;
                    }
                    if view.help {view.help=false;continue;}
                    if navigation.is_some(){continue;}
                    if save_task.is_some()&&ctrl&&matches!(key.code,KeyCode::Char('s'|'n')){view.status="Draft is saving; try again in a moment".into();continue;}
                    if key.code==KeyCode::F(2){view.tree.focused = !view.tree.focused;continue;}
                    if view.tree.focused {
                        match key.code {
                            KeyCode::Up=>view.tree.selected=view.tree.selected.saturating_sub(1),
                            KeyCode::Down=>view.tree.selected=(view.tree.selected+1).min(view.tree.records.len().saturating_sub(1)),
                            KeyCode::Esc=>view.tree.focused=false,
                            KeyCode::Char('x')=>{if let (Some(selected),Some(tx))=(view.tree.current(),&control)&& !selected.status.terminal(){let (reply,rx)=oneshot::channel();if tx.try_send(Control::Cancel{id:selected.id,reply}).is_ok(){cancel_reply=Some(rx);view.status="Cancellation requested…".into();}}},
                            KeyCode::Enter|KeyCode::F(5)|KeyCode::Backspace=>{
                                let selected=if key.code==KeyCode::Backspace {view.tree.current().and_then(|r|r.parent_id).and_then(|id|{view.tree.select_id(&id);view.tree.current()})}else{view.tree.current()};
                                if let Some(r)=selected {if r.parent_id.is_none(){view.tree.inspected=None;view.tree.focused=false;}else{let store=agent.store.clone();let id=r.id.clone();let session=r.session_id;view.tree.inspected=Some(id.clone());view.tree.focused=false;view.follow=true;navigation=Some(tokio::spawn(async move{Ok(Navigation::Agent(id,store.messages(&session).await?))}));}}
                            },_=>{}
                        }continue;
                    }
                    if key.code==KeyCode::F(5)&&view.tree.inspected.is_some(){if let Some(r)=view.tree.records.iter().find(|r|Some(&r.id)==view.tree.inspected.as_ref()).cloned(){let store=agent.store.clone();let id=r.id.clone();navigation=Some(tokio::spawn(async move{Ok(Navigation::Agent(id,store.messages(&r.session_id).await?))}));}continue;}
                    if view.browser {
                        match key.code {
                            KeyCode::Esc=>view.browser=false,
                            KeyCode::Up=>view.selected=view.selected.saturating_sub(1),
                            KeyCode::Down=>view.selected=(view.selected+1).min(view.sessions.len().saturating_sub(1)),
                            KeyCode::Enter=>{if let Some(s)=view.sessions.get(view.selected).cloned(){
                                if s.workspace!=session.workspace {view.status="Session belongs to a different workspace; reopen from its project with --session".into();view.browser=false;continue;}
                                if s.model!=agent.config.provider.model {view.status="Session uses a different model; configure it before resuming".into();view.browser=false;continue;}
                                let store=agent.store.clone();let id=session.id.clone();let draft=view.input.lines().join("\n");
                                navigation=Some(tokio::spawn(async move{store.draft(&id,draft).await?;let current=store.get(&s.id).await?;let m=store.messages(&s.id).await?;store.recover_agents_if_idle(&s.id).await?;let records=store.agents(Some(&s.id)).await?;Ok(Navigation::Open(current,m,records))}));view.status="Loading session…".into();
                            }},_=>{}
                        }continue;
                    }
                    if ctrl&&key.code==KeyCode::Char('h'){view.help=true;}
                    else if ctrl&&key.code==KeyCode::Char('s')&&task.is_none(){let store=agent.store.clone();navigation=Some(tokio::spawn(async move{Ok(Navigation::List(store.list(None).await?))}));view.status="Loading sessions…".into();}
                    else if ctrl&&key.code==KeyCode::Char('n')&&task.is_none(){let store=agent.store.clone();let id=session.id.clone();let draft=view.input.lines().join("\n");let workspace=agent.workspace.clone();let model=agent.config.provider.model.clone();navigation=Some(tokio::spawn(async move{store.draft(&id,draft).await?;let s=store.create(&workspace,&model,"New session").await?;Ok(Navigation::Open(s,vec![],vec![]))}));view.status="Creating session…".into();}
                    else if key.code==KeyCode::Esc {cancel.cancel();view.status="Cancelling…".into();}
                    else if key.code==KeyCode::PageUp {view.follow=false;view.scroll=view.scroll.saturating_sub(10);}
                    else if key.code==KeyCode::PageDown {view.follow=false;view.scroll=view.scroll.saturating_add(10);}
                    else if key.code==KeyCode::End&&!ctrl {view.follow=true;}
                    else if key.code==KeyCode::Enter&&!key.modifiers.contains(KeyModifiers::ALT)&&!key.modifiers.contains(KeyModifiers::SHIFT) {
                        if task.is_some(){view.status="Agent is running; Esc cancels. Draft remains editable.".into();continue;}
                        let prompt=view.input.lines().join("\n");if prompt.trim().is_empty(){continue;}
                        view.transcript.push_str(&format!("USER\n{prompt}\n\nASSISTANT\n"));view.input.select_all();view.input.cut();view.follow=true;draft_dirty=false;
                        let a=agent.clone();let id=session.id.clone();let events=tx.clone();cancel=CancellationToken::new();let c=cancel.clone();let (command_tx,command_rx)=mpsc::channel(8);control=Some(command_tx);view.tree.inspected=None;task=Some(tokio::spawn(async move{a.run_controlled(id,prompt,events,c,command_rx).await}));view.status="Running".into();
                    }
                    else if key.code==KeyCode::Enter {view.input.insert_newline();draft_dirty=true;draft_at=tokio::time::Instant::now();}
                    else if view.input.input(key) {draft_dirty=true;draft_at=tokio::time::Instant::now();}
                },
                Event::Paste(text)=>{if navigation.is_none()&&text.len()<=256*1024 {view.input.insert_str(text);draft_dirty=true;draft_at=tokio::time::Instant::now();}else{view.status="Paste exceeds 256 KiB limit".into();}},
                Event::Mouse(mouse)=>match mouse.kind{MouseEventKind::ScrollUp=>{view.follow=false;view.scroll=view.scroll.saturating_sub(3);},MouseEventKind::ScrollDown=>{view.follow=false;view.scroll=view.scroll.saturating_add(3);},_=>{}},
                _=>{}
            }},
            result=async{navigation.as_mut().expect("guarded navigation").await},if navigation.is_some()=>{
                navigation=None;dirty=true;match result.context("navigation worker failed").and_then(|r|r){
                    Ok(Navigation::Agent(id,messages))=>view.tree.loaded(id,messages),
                    Ok(Navigation::List(sessions))=>{view.sessions=sessions;view.browser=true;view.selected=0;view.status="Ready".into();},
                    Ok(Navigation::Open(s,messages,records))=>{let sessions=std::mem::take(&mut view.sessions);view=View::new(&messages,&s.draft);view.sessions=sessions;view.tree.records=records;session=s;draft_dirty=false;},
                    Err(e)=>view.status=e.to_string(),
                }
            },
            Some(event)=rx.recv()=>{dirty=true;match event {
                AgentEvent::AgentChanged(record)=>{
                    view.status=format!("{}: {:?}",record.task.title,record.status);
                    if record.status.terminal()&&view.tree.inspected.as_ref()==Some(&record.id)&&navigation.is_none(){let store=agent.store.clone();let id=record.id.clone();let session=record.session_id.clone();navigation=Some(tokio::spawn(async move{Ok(Navigation::Agent(id,store.messages(&session).await?))}));}
                    view.tree.update(*record);
                },
                AgentEvent::AgentText{id,text}=>view.tree.stream(id,text),
                AgentEvent::AgentActivity{id,name,detail}=>view.tree.activity(id,name,detail),
                AgentEvent::Text(text)=>view.transcript.push_str(&text),
                AgentEvent::Usage(_)=>{},
                AgentEvent::State(s)=>view.status=format!("{s:?}"),
                AgentEvent::Tool{name,detail}=>view.transcript.push_str(&format!("\n[{name}] {detail}\n")),
                AgentEvent::Permission{preview,reply}=>view.queue_permission(preview,reply),
                AgentEvent::Finished{error}=>{view.transcript.push_str("\n\n");if let Some(e)=error{view.transcript.push_str(&format!("ERROR: {e}\n"));}view.permission=None;for (_,reply) in view.pending_permissions.drain(..){let _=reply.send(false);}},
            }},
            result=async {task.as_mut().expect("guarded task").await},if task.is_some()=>{
                task=None;control=None;dirty=true;if let Err(e)=result.context("agent worker crashed").and_then(|r|r){view.status=e.to_string();}else{view.status="Completed".into();}
            },
            reply=async{cancel_reply.as_mut().expect("guarded cancellation reply").await},if cancel_reply.is_some()=>{cancel_reply=None;view.status=match reply {Ok(Ok(()))=>"Agent cancellation requested".into(),Ok(Err(e))=>e,Err(_)=>"Agent control channel closed".into()};dirty=true;},
            _=tokio::time::sleep_until(draft_at+Duration::from_millis(500)),if draft_dirty&&save_task.is_none()&&navigation.is_none()=>{
                let store=agent.store.clone();let id=session.id.clone();let draft=view.input.lines().join("\n");save_task=Some(tokio::spawn(async move{store.draft(&id,draft).await}));draft_dirty=false;
            },
            result=async {save_task.as_mut().expect("guarded save").await},if save_task.is_some()=>{save_task=None;if let Err(e)=result.context("draft save failed").and_then(|r|r){view.status=e.to_string();dirty=true;}},
        }
    }
    cancel.cancel();
    drop(view.permission.take());
    for (_, reply) in view.pending_permissions.drain(..) {
        let _ = reply.send(false);
    }
    if let Some(mut t) = task {
        // Keep draining the bounded channel so final status persistence cannot deadlock shutdown.
        let shutdown = async {
            loop {
                tokio::select! {r=&mut t=>break r,event=rx.recv()=>{if let Some(AgentEvent::Permission{reply,..})=event{let _=reply.send(false);}}}
            }
        };
        if tokio::time::timeout(Duration::from_secs(5), shutdown)
            .await
            .is_err()
        {
            t.abort();
            let _ = t.await;
        }
    }
    if let Some(t) = navigation {
        let _ = t.await;
    }
    if let Some(t) = save_task {
        t.await??;
    }
    agent
        .store
        .draft(&session.id, view.input.lines().join("\n"))
        .await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    #[test]
    fn layouts_handle_narrow_and_wide() {
        for (w, h) in [(30, 12), (120, 32), (12, 6)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            let s = Session {
                id: "id".into(),
                title: "Test".into(),
                workspace: "/repo".into(),
                parent_id: None,
                model: "local".into(),
                draft: String::new(),
                updated_at: 0,
            };
            let mut v = View::new(&[], "hello\nworld");
            t.draw(|f| v.draw(f, &s, "local", "Ask", 8192)).unwrap();
            assert!(
                t.backend()
                    .buffer()
                    .content
                    .iter()
                    .any(|c| c.symbol() == "A")
            );
        }
    }
    #[test]
    fn multiline_undo() {
        let mut v = View::new(&[], "");
        v.input.insert_str("one");
        v.input.insert_newline();
        v.input.insert_str("two");
        assert_eq!(v.input.lines(), &["one", "two"]);
        v.input.undo();
        assert_eq!(v.input.lines(), &["one", ""]);
    }
    #[tokio::test]
    async fn simultaneous_agent_permissions_are_fifo_and_never_overwritten() {
        let mut v = View::new(&[], "");
        let (a, arx) = oneshot::channel();
        let (b, mut brx) = oneshot::channel();
        v.queue_permission("first".into(), a);
        v.queue_permission("second".into(), b);
        v.answer_permission(false);
        assert!(!arx.await.unwrap());
        assert!(brx.try_recv().is_err());
        assert_eq!(v.permission.as_ref().unwrap().0, "second");
        v.answer_permission(true);
        assert!(brx.await.unwrap());
    }
}
