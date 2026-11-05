use crate::Store;
use aether_core::agents::{AgentRecord, AgentStatus};
use anyhow::{Context, Result};
use rusqlite::params;

impl Store {
    pub async fn put_agent(&self, record: &AgentRecord) -> Result<()> {
        let r = record.clone();
        self.call(move|c|{
            c.execute("INSERT INTO agent_runs(id,root_id,parent_id,root_session_id,session_id,status,record) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET status=excluded.status,record=excluded.record",params![r.id,r.root_id,r.parent_id,r.root_session_id,r.session_id,serde_json::to_string(&r.status)?,serde_json::to_string(&r)?])?;Ok(())
        }).await
    }
    pub async fn agent(&self, id: &str) -> Result<AgentRecord> {
        let id = id.to_owned();
        self.call(move |c| {
            let s: String =
                c.query_row("SELECT record FROM agent_runs WHERE id=?1", [id], |r| {
                    r.get(0)
                })?;
            Ok(serde_json::from_str(&s)?)
        })
        .await
    }
    pub async fn agents(&self, root_session: Option<&str>) -> Result<Vec<AgentRecord>> {
        let root_session = root_session.map(str::to_owned);
        self.call(move|c|{
            let mut stmt=c.prepare("SELECT record FROM agent_runs WHERE (?1 IS NULL OR root_session_id=?1) ORDER BY rowid LIMIT 512")?;
            let rows=stmt.query_map([root_session],|r|r.get::<_,String>(0))?;
            rows.map(|r|Ok(serde_json::from_str(&r?)?)).collect()
        }).await
    }
    /// Caller must hold the root session lease. Never replay unknown-outcome work.
    pub async fn interrupt_abandoned_agents(&self, root_session: &str) -> Result<usize> {
        let id = root_session.to_owned();
        self.call(move|c|{
            let tx=c.transaction()?;
            let mut changed=0;
            let records={let mut stmt=tx.prepare("SELECT record FROM agent_runs WHERE root_session_id=?1")?;stmt.query_map([&id],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?};
            for s in records {let mut r:AgentRecord=serde_json::from_str(&s)?;if !r.status.terminal(){
                r.status=AgentStatus::Interrupted;r.error=Some("process exited before terminal state; inspect history and workspace, no actions were replayed".into());
                tx.execute("UPDATE agent_runs SET status=?2,record=?3 WHERE id=?1",params![r.id,serde_json::to_string(&r.status)?,serde_json::to_string(&r)?])?;changed+=1;
            }}
            tx.commit()?;Ok(changed)
        }).await
    }
    pub async fn recover_agents_if_idle(&self, root_session: &str) -> Result<usize> {
        let Some(_lease) = self.try_lease(root_session).await? else {
            return Ok(0);
        };
        self.interrupt_abandoned_agents(root_session).await
    }
    pub async fn try_lease(&self, id: &str) -> Result<Option<std::fs::File>> {
        let id = uuid::Uuid::parse_str(id)?.to_string();
        let p = self.path.with_file_name(format!("{id}.lock"));
        tokio::task::spawn_blocking(move || {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(p)?;
            match fs2::FileExt::try_lock_exclusive(&f) {
                Ok(()) => Ok(Some(f)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(e) => Err(e).context("acquire session lease"),
            }
        })
        .await?
    }
    pub fn worktree_directory(&self) -> std::path::PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("worktrees")
    }
}
