use aether_core::{Message, Session, TimelineEvent};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = Self {
            path: path.as_ref().to_owned(),
        };
        store.call(|c| {
            let version: i64=c.pragma_query_value(None,"user_version",|r|r.get(0))?;
            if version>1 { bail!("database schema is newer than this AETHER version"); }
            c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
            let tx=c.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, workspace TEXT NOT NULL, parent_id TEXT,
                model TEXT NOT NULL, draft TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL DEFAULT (unixepoch()));
                CREATE TABLE IF NOT EXISTS events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(id), kind TEXT NOT NULL, payload TEXT NOT NULL);
                CREATE INDEX IF NOT EXISTS events_session ON events(session_id,sequence);
                CREATE VIRTUAL TABLE IF NOT EXISTS event_search USING fts5(session_id UNINDEXED, text);
                PRAGMA user_version=1;")?;
            tx.commit()?; Ok(())
        }).await?;
        Ok(store)
    }
    async fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut c = Connection::open(path)?;
            c.busy_timeout(std::time::Duration::from_secs(5))?;
            c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")?;
            f(&mut c)
        })
        .await
        .context("database worker failed")?
    }
    /// OS advisory lock prevents two AETHER turns from running in the same session.
    /// The OS releases it even after SIGKILL. This is not a workspace-wide editor lock.
    pub async fn lease(&self, id: &str) -> Result<std::fs::File> {
        let id = Uuid::parse_str(id)?.to_string();
        let p = self.path.with_file_name(format!("{id}.lock"));
        tokio::task::spawn_blocking(move || {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(p)?;
            fs2::FileExt::try_lock_exclusive(&f)
                .context("session is already running in another process")?;
            Ok(f)
        })
        .await?
    }
    pub async fn create(&self, workspace: &Path, model: &str, title: &str) -> Result<Session> {
        let session = Session {
            id: Uuid::new_v4().to_string(),
            title: title.chars().take(100).collect(),
            workspace: workspace.to_string_lossy().into(),
            parent_id: None,
            model: model.into(),
            draft: String::new(),
            updated_at: 0,
        };
        let s = session.clone();
        self.call(move |c| {
            c.execute(
                "INSERT INTO sessions(id,title,workspace,model) VALUES(?1,?2,?3,?4)",
                params![s.id, s.title, s.workspace, s.model],
            )?;
            Ok(())
        })
        .await?;
        Ok(session)
    }
    pub async fn list(&self, query: Option<String>) -> Result<Vec<Session>> {
        self.call(move |c| {
            let sql=if query.is_some() { "SELECT id,title,workspace,parent_id,model,draft,updated_at FROM sessions WHERE title LIKE ?1 OR id IN (SELECT session_id FROM event_search WHERE event_search MATCH ?2) ORDER BY updated_at DESC LIMIT 200" } else { "SELECT id,title,workspace,parent_id,model,draft,updated_at FROM sessions ORDER BY updated_at DESC LIMIT 200" };
            let mut stmt=c.prepare(sql)?;
            let map=|r:&rusqlite::Row<'_>| Ok(Session{id:r.get(0)?,title:r.get(1)?,workspace:r.get(2)?,parent_id:r.get(3)?,model:r.get(4)?,draft:r.get(5)?,updated_at:r.get(6)?});
            let rows=if let Some(q)=query { stmt.query_map(params![format!("%{q}%"),format!("\"{}\"",q.replace('"',"\"\""))],map)? } else { stmt.query_map([],map)? };
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        }).await
    }
    pub async fn get(&self, id: &str) -> Result<Session> {
        let id = id.to_owned();
        self.call(move |c| Ok(c.query_row("SELECT id,title,workspace,parent_id,model,draft,updated_at FROM sessions WHERE id=?1",[id],|r|Ok(Session{id:r.get(0)?,title:r.get(1)?,workspace:r.get(2)?,parent_id:r.get(3)?,model:r.get(4)?,draft:r.get(5)?,updated_at:r.get(6)?}))?)).await
    }
    pub async fn append(&self, id: &str, kind: &str, payload: Value) -> Result<()> {
        let (id, kind) = (id.to_owned(), kind.to_owned());
        self.call(move |c| {
            let tx = c.transaction()?;
            let text = payload.to_string();
            tx.execute(
                "INSERT INTO events(session_id,kind,payload) VALUES(?1,?2,?3)",
                params![id, kind, text],
            )?;
            tx.execute(
                "INSERT INTO event_search(session_id,text) VALUES(?1,?2)",
                params![id, text],
            )?;
            tx.execute(
                "UPDATE sessions SET updated_at=unixepoch() WHERE id=?1",
                [id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }
    pub async fn message(&self, id: &str, m: &Message) -> Result<()> {
        self.append(id, "message", serde_json::to_value(m)?).await
    }
    pub async fn timeline(&self, id: &str) -> Result<Vec<TimelineEvent>> {
        let id = id.to_owned();
        self.call(move |c| {
            let mut stmt = c.prepare(
                "SELECT sequence,kind,payload FROM events WHERE session_id=?1 ORDER BY sequence",
            )?;
            let rows = stmt.query_map([id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            rows.map(|r| {
                let (sequence, kind, payload) = r?;
                Ok(TimelineEvent {
                    sequence,
                    kind,
                    payload: serde_json::from_str(&payload)?,
                })
            })
            .collect()
        })
        .await
    }
    pub async fn messages(&self, id: &str) -> Result<Vec<Message>> {
        self.timeline(id)
            .await?
            .into_iter()
            .filter(|e| e.kind == "message")
            .map(|e| Ok(serde_json::from_value(e.payload)?))
            .collect()
    }
    pub async fn draft(&self, id: &str, draft: String) -> Result<()> {
        let id = id.to_owned();
        self.call(move |c| {
            c.execute(
                "UPDATE sessions SET draft=?2 WHERE id=?1",
                params![id, draft],
            )?;
            Ok(())
        })
        .await
    }
    pub async fn fork(&self, id: &str, title: &str) -> Result<String> {
        let (id, title, new) = (id.to_owned(), title.to_owned(), Uuid::new_v4().to_string());
        let result = new.clone();
        self.call(move |c|{
            let tx=c.transaction()?;
            if tx.execute("INSERT INTO sessions(id,title,workspace,parent_id,model,draft) SELECT ?2,?3,workspace,id,model,draft FROM sessions WHERE id=?1",params![id,new,title])?!=1 {bail!("session not found");}
            tx.execute("INSERT INTO events(session_id,kind,payload) SELECT ?2,kind,payload FROM events WHERE session_id=?1 ORDER BY sequence",params![id,new])?;
            tx.execute("INSERT INTO event_search(session_id,text) SELECT ?2,payload FROM events WHERE session_id=?1",params![id,new])?;
            tx.commit()?;Ok(())
        }).await?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reopen_search_fork_and_isolate() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.db");
        let s = Store::open(&p).await.unwrap();
        let a = s.create(dir.path(), "test", "main").await.unwrap();
        s.message(&a.id, &Message::text("user", "needle"))
            .await
            .unwrap();
        s.draft(&a.id, "draft".into()).await.unwrap();
        let b = s.fork(&a.id, "fork").await.unwrap();
        s.message(&b, &Message::text("assistant", "branch only"))
            .await
            .unwrap();
        drop(s);
        let s = Store::open(p).await.unwrap();
        assert_eq!(s.messages(&a.id).await.unwrap().len(), 1);
        assert_eq!(s.messages(&b).await.unwrap().len(), 2);
        assert_eq!(s.get(&a.id).await.unwrap().draft, "draft");
        assert_eq!(s.list(Some("needle".into())).await.unwrap().len(), 2);
    }
    #[tokio::test]
    async fn session_lease_releases_and_excludes_concurrent_turns() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("s.db")).await.unwrap();
        let s = store.create(d.path(), "m", "title").await.unwrap();
        let lock = store.lease(&s.id).await.unwrap();
        assert!(store.lease(&s.id).await.is_err());
        drop(lock);
        assert!(store.lease(&s.id).await.is_ok());
    }
    #[tokio::test]
    async fn concurrent_event_commits_are_not_lost() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("s.db")).await.unwrap();
        let s = store.create(d.path(), "m", "concurrency").await.unwrap();
        let mut workers = tokio::task::JoinSet::new();
        for n in 0..16 {
            let copy = store.clone();
            let id = s.id.clone();
            workers
                .spawn(async move { copy.append(&id, "probe", serde_json::json!({"n":n})).await });
        }
        while let Some(r) = workers.join_next().await {
            r.unwrap().unwrap();
        }
        assert_eq!(store.timeline(&s.id).await.unwrap().len(), 16);
    }
}
