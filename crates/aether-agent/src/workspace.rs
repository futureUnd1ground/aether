//! Private worktree creation and conflict-aware integration. No commits or merges.
use aether_core::agents::AgentRecord;
use aether_tools::{Action, FILE_LIMIT, hash};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

async fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(root)
        .kill_on_drop(true)
        .output()
        .await
        .context("execute git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(output.stdout)
}
pub async fn scoped_directory(root: &Path, scope: &str) -> Result<PathBuf> {
    aether_tools::validate_scope(root.into(), &Action::List { path: scope.into() }).await?;
    let p = if Path::new(scope).is_absolute() {
        PathBuf::from(scope)
    } else {
        root.join(scope)
    }
    .canonicalize()?;
    if !p.is_dir() || !p.starts_with(root) {
        bail!("agent scope must be a directory inside the parent workspace");
    }
    Ok(p)
}
pub async fn isolate(root: &Path, directory: &Path, id: &str) -> Result<(PathBuf, String)> {
    uuid::Uuid::parse_str(id)?;
    let top = PathBuf::from(
        String::from_utf8(git(root, &["rev-parse", "--show-toplevel"]).await?)?.trim(),
    );
    let base = String::from_utf8(git(root, &["rev-parse", "--verify", "HEAD"]).await?)?
        .trim()
        .to_owned();
    let relative = root
        .strip_prefix(&top)
        .context("workspace is not inside git repository")?;
    tokio::fs::create_dir_all(directory).await?;
    let path = directory.join(id);
    // Disable repository-specified checkout filters as well as hooks. Checkout must
    // not be an implicit arbitrary-code execution channel outside the shell sandbox.
    let filters = Command::new("git")
        .args(["config", "--name-only", "--get-regexp", "^filter\\."])
        .current_dir(root)
        .output()
        .await?;
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.fsmonitor=false",
    ]);
    let mut names = BTreeSet::new();
    for key in String::from_utf8(filters.stdout)?.lines() {
        if let Some(name) = key
            .strip_prefix("filter.")
            .and_then(|s| s.rsplit_once('.').map(|p| p.0))
        {
            names.insert(name.to_owned());
        }
    }
    for name in names {
        for (suffix, value) in [("smudge", ""), ("process", ""), ("required", "false")] {
            cmd.arg("-c").arg(format!("filter.{name}.{suffix}={value}"));
        }
    }
    let out = cmd
        .args(["worktree", "add", "--detach"])
        .arg(&path)
        .arg(&base)
        .current_dir(root)
        .kill_on_drop(true)
        .output()
        .await?;
    if !out.status.success() {
        bail!(
            "cannot create isolated worktree: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let workspace = path.join(relative);
    if !workspace.is_dir() {
        bail!(
            "scope does not exist in committed HEAD; isolated worktree retained at {}",
            path.display()
        );
    }
    Ok((workspace.canonicalize()?, base))
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationPlan {
    pub agent_id: String,
    pub changes: Vec<Change>,
    pub diff: String,
}
pub async fn plan(parent: &Path, child: &AgentRecord) -> Result<IntegrationPlan> {
    if !child.isolated || !child.status.terminal() {
        bail!("integration requires a finished isolated agent");
    }
    if child.status != aether_core::agents::AgentStatus::Completed {
        bail!(
            "failed/cancelled/interrupted agents require manual inspection; automatic integration refused"
        );
    }
    if child.integration.is_some() {
        bail!("agent changes were already integrated or integration needs manual recovery");
    }
    let workspace = Path::new(&child.workspace);
    let base = child
        .base_commit
        .as_deref()
        .context("missing worktree base commit")?;
    let top = PathBuf::from(
        String::from_utf8(git(workspace, &["rev-parse", "--show-toplevel"]).await?)?.trim(),
    );
    let prefix = workspace.strip_prefix(&top)?;
    let summary = String::from_utf8(
        git(
            &top,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--summary",
                "--no-renames",
                base,
                "--",
            ],
        )
        .await?,
    )?;
    if summary
        .lines()
        .any(|line| line.trim_start().starts_with("mode change"))
    {
        bail!("file mode changes require manual review; automatic integration refused");
    }
    let mut paths = BTreeSet::new();
    for args in [
        vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            base,
            "--",
        ],
        vec!["ls-files", "--others", "--exclude-standard", "-z"],
    ] {
        for path in git(&top, &args)
            .await?
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
        {
            paths.insert(String::from_utf8(path.to_vec())?);
        }
    }
    if paths.len() > 100 {
        bail!("integration exceeds 100 files; review manually");
    }
    let mut changes = Vec::new();
    let mut total = 0;
    for path in paths {
        let relative = Path::new(&path)
            .strip_prefix(prefix)
            .context("agent changed files outside its declared scope")?;
        let relative = relative.to_str().context("non-UTF-8 file name")?.to_owned();
        let before_output = Command::new("git")
            .args([
                "-c",
                "core.fsmonitor=false",
                "show",
                &format!("{base}:{path}"),
            ])
            .current_dir(&top)
            .output()
            .await?;
        let before = if before_output.status.success() {
            Some(
                String::from_utf8(before_output.stdout)
                    .context("binary integration is unsupported")?,
            )
        } else {
            None
        };
        let target = workspace.join(&relative);
        let after = match tokio::fs::symlink_metadata(&target).await {
            Ok(m) => {
                if !m.is_file() || m.file_type().is_symlink() || m.len() > FILE_LIMIT {
                    bail!("unsupported file type/size in integration: {path}");
                }
                Some(tokio::fs::read_to_string(&target).await?)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if before
            .as_ref()
            .is_some_and(|s| s.len() > FILE_LIMIT as usize)
        {
            bail!("original file exceeds integration limit");
        }
        #[cfg(unix)]
        if before.is_none() && after.is_some() {
            use std::os::unix::fs::PermissionsExt;
            if tokio::fs::metadata(&target).await?.permissions().mode() & 0o111 != 0 {
                bail!("new executable files require manual review; automatic integration refused");
            }
        }
        if before == after {
            continue;
        }
        // Validate parent scope and exact original contents. Never merge over local edits.
        aether_tools::validate_scope(
            parent.into(),
            &Action::Write {
                path: relative.clone(),
                content: String::new(),
                expected_sha256: None,
            },
        )
        .await?;
        let current = match tokio::fs::read(parent.join(&relative)).await {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if current.as_deref() != before.as_deref().map(str::as_bytes) {
            bail!(
                "integration conflict in {relative}: parent differs from worktree base; no files were changed"
            );
        }
        total += before.as_ref().map_or(0, String::len) + after.as_ref().map_or(0, String::len);
        if total > 2 * 1024 * 1024 {
            bail!("integration exceeds 2 MiB; review manually");
        }
        changes.push(Change {
            path: relative,
            before,
            after,
        });
    }
    let mut diff = String::from_utf8_lossy(
        &git(
            &top,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                base,
                "--",
            ],
        )
        .await?,
    )
    .into_owned();
    for c in &changes {
        if c.before.is_none() {
            diff.push_str(&format!(
                "\n+++ new file {}\n{}\n",
                c.path,
                c.after.as_deref().unwrap_or("")
            ));
        }
    }
    Ok(IntegrationPlan {
        agent_id: child.id.clone(),
        changes,
        diff,
    })
}
/// Every changed file is checksum-checked again. If an editor races this operation,
/// rollback restores only bytes still matching our writes, never the editor's bytes.
pub async fn apply(parent: &Path, plan: &IntegrationPlan, cancel: CancellationToken) -> Result<()> {
    let mut applied: Vec<Change> = Vec::new();
    for change in &plan.changes {
        let result = apply_one(parent, change, &cancel).await;
        if let Err(e) = result {
            let mut conflicts = Vec::new();
            for c in applied.iter().rev() {
                let reverse = Change {
                    path: c.path.clone(),
                    before: c.after.clone(),
                    after: c.before.clone(),
                };
                if apply_one(parent, &reverse, &CancellationToken::new())
                    .await
                    .is_err()
                {
                    conflicts.push(c.path.clone());
                }
            }
            bail!(
                "integration failed: {e}; rollback conflicts requiring manual recovery: {conflicts:?}"
            );
        }
        applied.push(change.clone());
    }
    Ok(())
}
async fn apply_one(parent: &Path, c: &Change, cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        bail!("integration cancelled");
    }
    match &c.after {
        Some(content) => {
            aether_tools::execute(
                parent.into(),
                Action::Write {
                    path: c.path.clone(),
                    content: content.clone(),
                    expected_sha256: c.before.as_ref().map(|s| hash(s.as_bytes())),
                },
                cancel.child_token(),
            )
            .await?;
        }
        None => {
            aether_tools::validate_scope(
                parent.into(),
                &Action::Read {
                    path: c.path.clone(),
                },
            )
            .await?;
            let current = tokio::fs::read(parent.join(&c.path)).await?;
            if Some(current.as_slice()) != c.before.as_deref().map(str::as_bytes) {
                bail!("concurrent modification in {}", c.path);
            }
            tokio::fs::remove_file(parent.join(&c.path)).await?;
        }
    }
    Ok(())
}
