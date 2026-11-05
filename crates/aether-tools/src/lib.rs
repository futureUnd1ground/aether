use aether_core::{Mode, Rule, config::Permissions};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

pub const FILE_LIMIT: u64 = 256 * 1024;
#[derive(Debug, Clone)]
pub enum Action {
    Read {
        path: String,
    },
    List {
        path: String,
    },
    Write {
        path: String,
        content: String,
        expected_sha256: Option<String>,
    },
    Shell {
        command: String,
    },
}
impl Action {
    pub fn parse(name: &str, args: &str) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PathArgs {
            path: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WriteArgs {
            path: String,
            content: String,
            expected_sha256: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ShellArgs {
            command: String,
        }
        Ok(match name {
            "read_file" => {
                let a: PathArgs = serde_json::from_str(args)?;
                Self::Read { path: a.path }
            }
            "list_directory" => {
                let a: PathArgs = serde_json::from_str(args)?;
                Self::List { path: a.path }
            }
            "write_file" => {
                let a: WriteArgs = serde_json::from_str(args)?;
                if a.content.len() > FILE_LIMIT as usize {
                    bail!("content exceeds limit");
                }
                Self::Write {
                    path: a.path,
                    content: a.content,
                    expected_sha256: a.expected_sha256,
                }
            }
            "shell" => {
                let a: ShellArgs = serde_json::from_str(args)?;
                if a.command.trim().is_empty() || a.command.len() > 8192 {
                    bail!("invalid command length");
                }
                Self::Shell { command: a.command }
            }
            _ => bail!("unknown tool {name}"),
        })
    }
    pub fn rule(&self, p: &Permissions, mode: Mode) -> Rule {
        match self {
            Self::Read { .. } | Self::List { .. } => p.read,
            Self::Write { .. } => {
                if mode.writable() {
                    p.write
                } else {
                    Rule::Deny
                }
            }
            Self::Shell { .. } => {
                if mode.writable() && p.shell != Rule::Deny {
                    Rule::Ask
                } else {
                    Rule::Deny
                }
            }
        }
    }
    pub fn preview(&self) -> String {
        match self {
            Self::Read { path } => format!("Read {path}"),
            Self::List { path } => format!("List {path}"),
            Self::Write {
                path,
                content,
                expected_sha256,
            } => format!(
                "Write {path} ({} bytes, expected: {})\n{}",
                content.len(),
                expected_sha256.as_deref().unwrap_or("new file"),
                content
            ),
            Self::Shell { command } => {
                format!("Execute shell with your account permissions (no OS sandbox):\n{command}")
            }
        }
    }
}
pub fn definitions(mode: Mode) -> Vec<Value> {
    let mut tools = vec![
        schema(
            "read_file",
            "Read a workspace UTF-8 regular file, up to 256 KiB. Returns content and sha256. Prefer a relative path, e.g. answer.py or src/main.rs. Absolute paths are allowed only inside the workspace.",
            json!({"path":{"type":"string"}}),
            &["path"],
        ),
        schema(
            "list_directory",
            "List up to 500 entries within workspace. Use path . for the workspace root, never /workspace or /tmp.",
            json!({"path":{"type":"string"}}),
            &["path"],
        ),
    ];
    if mode.writable() {
        tools.push(schema("write_file","Atomic write. Existing files require the sha256 returned by read_file; new files require null. User sees full proposed content before approval.",json!({"path":{"type":"string"},"content":{"type":"string"},"expected_sha256":{"type":["string","null"]}}),&["path","content","expected_sha256"]));
        tools.push(schema("shell","Execute shell in workspace; ALWAYS requires fresh approval, unrestricted OS account access. 30-second timeout; bounded captured output. Use for tests, rg and git inspection.",json!({"command":{"type":"string"}}),&["command"]));
    }
    tools
}
fn schema(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}
fn scoped(root: &Path, path: &str, new: bool) -> Result<PathBuf> {
    let supplied = Path::new(path);
    let relative = if supplied.is_absolute() {
        supplied
            .strip_prefix(root)
            .context("absolute path is outside workspace; use a workspace-relative path")?
    } else {
        supplied
    };
    if relative.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::Prefix(_)
        )
    }) {
        bail!(
            "path must stay within workspace without ..; use answer.py, src/lib.rs or . for root"
        );
    }
    let mut current = root.to_owned();
    let parts: Vec<_> = relative.components().collect();
    for (i, part) in parts.iter().enumerate() {
        current.push(part.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    bail!("symlink access denied");
                }
            }
            Err(e) if new && i == parts.len() - 1 && e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if !current.starts_with(root) {
        bail!("path escapes workspace");
    }
    Ok(current)
}
/// Validate scope before requesting user consent, without reading file contents.
pub async fn validate_scope(root: PathBuf, action: &Action) -> Result<()> {
    let (path, new) = match action {
        Action::Read { path } | Action::List { path } => (path.clone(), false),
        Action::Write { path, .. } => (path.clone(), true),
        Action::Shell { .. } => return Ok(()),
    };
    tokio::task::spawn_blocking(move || scoped(&root, &path, new).map(|_| ())).await?
}
pub async fn execute(root: PathBuf, action: Action, cancel: CancellationToken) -> Result<Value> {
    if cancel.is_cancelled() {
        bail!("cancelled");
    }
    if let Action::Shell { command } = action {
        return shell(root, command, cancel).await;
    }
    tokio::task::spawn_blocking(move || execute_file(&root, action)).await?
}
fn execute_file(root: &Path, action: Action) -> Result<Value> {
    match action {
        Action::Read { path } => {
            let p = scoped(root, &path, false)?;
            let m = std::fs::metadata(&p)?;
            if !m.is_file() || m.len() > FILE_LIMIT {
                bail!("not a regular file or file exceeds 256 KiB");
            }
            let bytes = std::fs::read(p)?;
            if bytes.len() > FILE_LIMIT as usize {
                bail!("file grew beyond limit");
            }
            Ok(
                json!({"path":path,"sha256":hash(&bytes),"content":std::str::from_utf8(&bytes).context("not UTF-8")?}),
            )
        }
        Action::List { path } => {
            let p = scoped(root, &path, false)?;
            let mut entries=std::fs::read_dir(p)?.take(501).map(|e|{let e=e?;Ok(json!({"name":e.file_name().to_string_lossy(),"directory":e.file_type()?.is_dir(),"symlink":e.file_type()?.is_symlink()}))}).collect::<Result<Vec<_>>>()?;
            let truncated = entries.len() > 500;
            entries.truncate(500);
            entries.sort_by_key(|e| e["name"].as_str().unwrap_or("").to_owned());
            Ok(json!({"entries":entries,"truncated":truncated}))
        }
        Action::Write {
            path,
            content,
            expected_sha256,
        } => {
            let p = scoped(root, &path, true)?;
            if let Ok(m) = std::fs::metadata(&p)
                && (!m.is_file() || m.len() > FILE_LIMIT)
            {
                bail!("not a regular file or file exceeds limit");
            }
            let before = match std::fs::read(&p) {
                Ok(b) => Some(b),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            match (&before, &expected_sha256) {
                (Some(b), Some(h)) if hash(b) == *h => {}
                (None, None) if !p.exists() => {}
                _ => bail!("file changed or expected_sha256 missing; read file again"),
            }
            if let Ok(m) = std::fs::metadata(&p) {
                if !m.is_file() {
                    bail!("not a regular file");
                }
                if m.len() > FILE_LIMIT {
                    bail!("existing file exceeds limit");
                }
            }
            let mut tmp = tempfile::NamedTempFile::new_in(p.parent().context("missing parent")?)?;
            if let Ok(m) = std::fs::metadata(&p) {
                tmp.as_file().set_permissions(m.permissions())?;
            }
            tmp.write_all(content.as_bytes())?;
            tmp.as_file().sync_all()?;
            // Revalidate after preparing data; detects ordinary editor races, not a hostile OS user.
            scoped(root, &path, true)?;
            if std::fs::read(&p).ok() != before {
                bail!("concurrent file modification detected");
            }
            if before.is_none() {
                tmp.persist_noclobber(&p)?;
            } else {
                tmp.persist(&p)?;
            }
            Ok(json!({"path":path,"sha256":hash(content.as_bytes()),"bytes":content.len()}))
        }
        Action::Shell { .. } => unreachable!(),
    }
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
async fn capture<R: tokio::io::AsyncRead + Unpin>(mut reader: R) -> Result<(String, bool)> {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        let take = n.min((64 * 1024usize).saturating_sub(kept.len()));
        kept.extend_from_slice(&buffer[..take]);
        truncated |= take < n;
    }
    Ok((String::from_utf8_lossy(&kept).into(), truncated))
}
#[cfg(unix)]
struct ProcessGroup(u32);
#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(self.0 as i32),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
}
async fn shell(root: PathBuf, command: String, cancel: CancellationToken) -> Result<Value> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn().context("launch shell")?;
    #[cfg(unix)]
    let _group = ProcessGroup(child.id().context("missing child PID")?);
    let out = capture(child.stdout.take().context("stdout missing")?);
    let err = capture(child.stderr.take().context("stderr missing")?);
    let work = async {
        let (status, out, err) = tokio::try_join!(
            async { Ok::<_, anyhow::Error>(child.wait().await?) },
            out,
            err
        )?;
        Ok::<_, anyhow::Error>(
            json!({"exit_code":status.code(),"success":status.success(),"stdout":out.0,"stderr":err.0,"truncated":out.1||err.1}),
        )
    };
    tokio::select! {biased;_=cancel.cancelled()=>bail!("shell cancelled"),r=tokio::time::timeout(Duration::from_secs(30),work)=>r.context("shell timed out after 30s")?}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn file_conflicts_and_scope() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().to_owned();
        std::fs::write(root.join("f"), "old").unwrap();
        assert!(
            execute(
                root.clone(),
                Action::Read {
                    path: "../secret".into()
                },
                CancellationToken::new()
            )
            .await
            .is_err()
        );
        assert!(
            execute(
                root.clone(),
                Action::Write {
                    path: "f".into(),
                    content: "new".into(),
                    expected_sha256: Some(hash(b"stale"))
                },
                CancellationToken::new()
            )
            .await
            .is_err()
        );
        execute(
            root.clone(),
            Action::Write {
                path: "f".into(),
                content: "new".into(),
                expected_sha256: Some(hash(b"old")),
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(root.join("f")).unwrap(), "new");
    }
    #[cfg(unix)]
    #[test]
    fn symlink_denied() {
        let d = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/etc/passwd", d.path().join("escape")).unwrap();
        assert!(scoped(d.path(), "escape", false).is_err());
    }
    #[test]
    fn readonly_and_shell_policy() {
        let p = Permissions {
            read: Rule::Allow,
            write: Rule::Allow,
            shell: Rule::Allow,
        };
        let a = Action::Shell {
            command: "echo hello".into(),
        };
        assert_eq!(a.rule(&p, Mode::Ask), Rule::Deny);
        assert_eq!(a.rule(&p, Mode::Build), Rule::Ask);
    }
    #[tokio::test]
    async fn shell_exit_and_cancel() {
        let d = tempfile::tempdir().unwrap();
        let v = shell(
            d.path().into(),
            "printf hi; exit 7".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(v["exit_code"], 7);
        assert_eq!(v["stdout"], "hi");
        let c = CancellationToken::new();
        c.cancel();
        assert!(shell(d.path().into(), "sleep 10".into(), c).await.is_err());
    }
    #[tokio::test]
    async fn absolute_paths_remain_scoped() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("f"), "data").unwrap();
        let inside = d.path().join("f").to_string_lossy().into_owned();
        assert!(
            execute(
                d.path().into(),
                Action::Read { path: inside },
                CancellationToken::new()
            )
            .await
            .is_ok()
        );
        assert!(scoped(d.path(), "/etc/passwd", false).is_err());
        assert!(
            scoped(
                d.path(),
                &format!("{}/../outside", d.path().display()),
                true
            )
            .is_err()
        );
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancellation_kills_shell_descendant() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().to_path_buf();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let worker = tokio::spawn(async move {
            shell(
                root,
                "sleep 30 & printf '%s' \"$!\" > child.pid; wait".into(),
                token,
            )
            .await
        });
        let path = d.path().join("child.pid");
        let pid = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(s) = tokio::fs::read_to_string(&path).await
                    && let Ok(pid) = s.parse::<u32>()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        cancel.cancel();
        assert!(worker.await.unwrap().is_err());
        let stopped = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match tokio::fs::read_to_string(format!("/proc/{pid}/stat")).await {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                    Ok(stat)
                        if stat
                            .rsplit_once(") ")
                            .is_some_and(|(_, s)| s.starts_with('Z')) =>
                    {
                        break;
                    }
                    _ => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        })
        .await;
        assert!(stopped.is_ok(), "shell descendant is still running");
    }
}
