# Security policy

AETHER is pre-release. No version is advertised as security-audited or production
ready. Report vulnerabilities privately to the repository maintainer when a private
reporting channel exists; do not include secrets or live exploit credentials in
public issues. No reporting address has been invented for this new repository.

## Current protections

- Local-only default endpoint. Remote endpoints require trusted explicit opt-in,
  HTTPS and no redirects. API key resolved from environment; not printed to logs.
- Mode enforcement independent of tools advertised to the model.
- Central allow/ask/deny evaluation. Shell never auto-allows; deny wins.
- Once-only approvals, durable audit events, validated typed tool arguments.
- Scoped file operations: no parent traversal, outside absolute paths or symlinks.
- Checksum before writing, same-directory temporary file, atomic replacement,
  original contents in the session, and no-clobber for newly created files.
- Turn, provider and shell deadlines; bounded queues and captured output.
- Session OS lock prevents concurrent AETHER turns in the same session.
- Repository config cannot authorize remote access, grant `allow` permissions or
  select endpoints or a credential environment variable. Explicit `--config` is trusted input.
- Plugins do not run; no automatic discovery executes third-party code.

## Limits of the boundary

Shell is unrestricted code execution as the current user. Read the complete command
before approving; its working directory does not scope its filesystem/network
access. Commands may change Git, credentials or files outside the workspace. No
command classifier or prompt can turn a shell into a sandbox.

File checks defend against ordinary path escapes and editor conflicts, not an
attacker swapping directories between validation and open/rename. The final
checksum comparison and replacement are not a single OS compare-and-swap. External
editors and agents in different sessions may still race. Strong hostile-workspace
containment needs descriptor-relative openat/no-follow or an OS sandbox, planned
before production release. Workspace isolation is not yet implemented.

Loopback trust relies on configured host names; AETHER does not defend against a
compromised local server/proxy or malicious DNS/hosts configuration. Selecting a
remote server intentionally discloses requests and approved source/tool output to
it. Project configuration cannot change endpoints or credential variables;
review explicitly supplied config files before use. No provider routing happens implicitly.

Terminal control characters in model/tool output must not be trusted. Ratatui draws
text, but headless raw streaming output is currently not an ANSI sanitizer. Use
`--json` when storing/processing untrusted model output.

Sessions, exports and file backups contain plaintext source and tool output. The
log excludes prompt bodies by design, but `RUST_LOG` can enable dependency tracing;
inspect diagnostics before sharing. No at-rest encryption, secret scanner or
retention policy exists. Abrupt death can lose the latest uncommitted stream tail.
Windows descendant cleanup and non-Linux behavior have not been validated.
