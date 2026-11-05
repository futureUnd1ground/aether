# Implementation status — 0.2.0-alpha.1

Updated 2026-11-05. **Advanced engine implemented; full product is still alpha.**
The original briefs remain in PROJECT_BRIEF.md / PHASE2_BRIEF.md. This progress file
points to actual checks and evidence; it is not itself proof of correctness.

## Completed milestones

1. **Completion reliability:** typed finish reasons distinguish complete, tool-only,
   blank, truncated, filtered and context-exhausted generations. Broken streams and
   malformed responses fail. Blank finals get one default bounded summary recovery,
   with tools disabled and the same time/token/step/cost limits. Reused historical
   tool-call IDs are refused. There is no replay of edits or commands.
2. **Real subagents:** the original engine/provider/tools run independent child
   contexts and SQLite sessions. Typed spawn/await/list/cancel tools, task structures,
   IDs, parent/root relationships, profiles, model/provider selection, scopes,
   permissions, resource accounting, results and errors are implemented.
3. **Recursive bounded scheduling:** conservative defaults (depth 2, concurrency 2,
   8 admitted agents including root), shared and per-node budgets, fair bounded slots,
   cancellation trees, and waiting parents that release slots. Root joins children
   before returning; unfinished descendants are cancelled when parents finish.
4. **Isolation and controlled integration:** approved detached Git worktrees from
   committed HEAD; sanitized offline Linux bubblewrap shell for subagents; no unsafe
   shell fallback. Integration previews, approval, exact-base conflict checks,
   original-content journals, guarded application/rollback and retained worktrees.
   Root shell still requires approval and remains unrestricted.
5. **Durable hierarchy:** schema 2 migration preserves schema-1 history. Agent trees,
   events, usage and terminal results remain queryable after restart. Lease-aware
   recovery marks abandoned nodes Interrupted without touching another live process.
   Serialized snapshot persistence prevents an old status overwriting newer usage.
6. **Multi-agent presentation:** real tree with status/resources, child history/tool
   inspection, parent navigation, targeted cancellation and a narrow-terminal overlay.
   Concurrent permission requests queue FIFO. Historical `agents`, `agent`, and
   approved `integrate` CLI workflows do not require another inference request.
7. **Publication preparation:** maintained Ratatui 0.30/textarea stack, English/Russian
   README, vector banner, real terminal captures, CI/issue templates, MIT license,
   guides and reproducible local release packaging. No fake controls or responses.

## Checks actually executed locally

Linux CachyOS x86_64; Rust/Cargo 1.99.0:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — passed.
- `cargo test --locked --workspace` — **49 passed**, zero failures/ignored tests.
- `cargo build --locked --release` — passed.
- `python scripts/verify_terminal.py` — passed: real PTY launch, multiline input,
  debounced drafts, resize/help, clean exit, SIGKILL/reopen and SQLite integrity,
  input/help while a network fixture streams, cancellation and draft preservation.
- Maintained `cargo-deny 0.20.2`, verified upstream release archive, native Linux
  dependency graph: **advisories, bans, licenses, sources passed**. Duplicate-version
  warnings remain. An initial archived-`paste` advisory was resolved by upgrading
  Ratatui/textarea, not by ignoring the advisory. No security-audited-product claim.
- Warm CLI-help benchmark executed; raw data in benchmark-results.json. It does not
  measure TUI cold startup or support comparison with OpenCode.

The tests include actual filesystem edits and shell exit codes, descendant cleanup,
real local Git worktrees/integration/conflicts, Linux sandbox behavior, provider SSE
fixtures and finish reasons, recursive single-slot scheduling, parallel engines,
shared/child budgets, cost/unknown-price refusal, cancellation/control, independent
contexts, schema migration/recovery and terminal/tree/permission behavior.
Test providers exist only in tests; they are not a runtime fallback.

## Real local-model evidence

Already installed Qwen3.5 2B Q4_K_M, LM Studio loopback endpoint, no cloud keys or
private source. Validation uses generated repositories, context 8192 and two model
slots. No claim that a 2B model is reliably autonomous.

- [phase2-local.json](validation/phase2-local.json): main actually spawned two
  read-only subagents, each read its assigned on-disk file, both completed, main
  awaited their structured results and returned a nonempty factual response.
- [phase2-local-partial.json](validation/phase2-local-partial.json): earlier run
  preserved a child's step-limit failure; parent collected and reported it. A real
  blank final was recovered with tools disabled. Failures are not hidden samples.
- [phase2-local-writing.json](validation/phase2-local-writing.json): a real writing
  child changed only its isolated worktree. Independent unittest execution passed
  inside that worktree while the parent's original test still failed and unrelated
  notes were preserved. The owner-attended turn timed out during command review;
  it is recorded Cancelled, not falsely Completed. No successful live integration
  or unattended end-to-end coding claim is made from that run. Integration is
  independently exercised with actual Git/filesystem changes in regression tests.
- [screenshots.json](validation/screenshots.json): real persisted-session terminal
  cells captured from the executable, not a mockup. pyte/Pillow render the cells;
  only the outer presentation frame is added. Raw readable screen text is included.

The original single-agent evidence also remains under validation/live-*.json for
historical comparison. It is not substituted for the new multi-agent evidence.

## Current practical limits

- Model quality remains material: the small local model can ignore requested
  delegation, invent wrong paths, repeat tools, or omit constraints in child tasks.
  Permission denials, scope checks, isolated workspaces, budgets and timeouts contain
  the execution; they do not make model reasoning reliable. Review actions/results.
- A Completed agent has a valid final response; `task_success_verified` currently
  stays false. Shell exit 0 is process evidence, not independent proof of a task.
- Timeouts include queued work, awaited children and owner permission review. Raise
  them intentionally for long attended reviews; interrupted actions are not replayed.
- Worktrees exclude dirty parent edits. UTF-8 integration supports existing parent
  directories, at most 100 files / 2 MiB / 256 KiB per file. Binary/symlink/mode/
  executable changes and missing directories need manual review. Worktrees persist.
- Integration is atomic per file, not across a project. External editor/hostile
  same-user races are not eliminated; interrupted/partial integration requires
  checkpoint inspection. Rollback refuses to overwrite concurrent user changes.
- Subagent shell requires working Linux namespaces/bubblewrap and cached offline
  dependencies. No cgroup quotas or syscall allowlist; resource-heavy commands still
  need careful review. Root shell has the current account's unrestricted access.
- Only OpenAI-compatible Chat Completions is implemented. LM Studio is live-verified;
  other compatible servers/models and native cloud-provider APIs are not fully tested.
- Token charges fall back to conservative reservations, not exact tokenization.
  Cost caps require operator-supplied prices; unknown cost remains null.
- No long-history virtualization, Markdown/syntax rendering, full context compaction,
  plugin host/SDK, OpenCode compatibility, MCP/ACP/LSP, full Git UI, or session import.
- Cross-platform runtime behavior and clean-machine builds outside CI are unverified.
  Shell captures bounded output and reports it after completion, not line-by-line.
- Sessions/exports/checkpoints contain plaintext source. No retention/encryption.
  Latest uncommitted stream tails can be lost; full committed chunks export separately.
- No comparative OpenCode benchmark and no production/security-readiness claim.

## Next engineering milestone

Broaden local-model coding acceptance tasks; improve task/constraint transfer and
progress diagnostics without weakening policy. Add virtualized, styled transcripts
and measured input latency. Strengthen descriptor-based filesystem operations and
multi-file integration recovery before production. Keep plugin compatibility behind
real pinned-plugin tests and capability/isolation gates.
