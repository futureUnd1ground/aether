# AETHER implementation status

Updated: 2026-11-05. Original product brief: [PROJECT_BRIEF.md](PROJECT_BRIEF.md).
This file records progress and evidence locations; it is not itself proof of a
working feature. **The complete product definition of done is NOT satisfied.**

## Delivered vertical slice

Seven implemented crates: core, storage, providers, tools, agent, tui, cli.

- Launchable Ratatui/Crossterm TUI with a real multiline editor, bracketed paste,
  undo/redo, word navigation supplied by tui-textarea, help, responsive sidebar,
  mouse/keyboard scrolling, session browser, editable draft during generation,
  permission previews and debounced durable drafts. Idle UI does not redraw on a timer.
- Strict TOML global/project/explicit config and environment overrides; unknown
  fields and unreasonable budgets/deadlines fail clearly. Repository config cannot
  authorize network endpoints/credentials or grant tool privileges.
- OpenAI-compatible native function tools over real HTTP SSE, fragmented UTF-8 and
  tool-call assembly, usage, bounded response, request timeout and no redirects.
- Sequential bounded agent loop, cancellation, read-only ask/plan/review modes,
  approved build/debug tools, step/context/turn limits, explicit terminal states.
- Scoped UTF-8 reads and directory listing. Relative paths and absolute paths
  inside workspace work; `..`, outside paths and symlinks are denied. Scope is
  checked before asking permission and again before execution.
- Checksum-protected atomic file writes; original existing contents must persist
  before modification. Fresh shell approval, 30s deadline, bounded capture, exit
  codes and Unix process-group cleanup. No OS sandbox claim.
- SQLite WAL/FULL transactional messages/events/search records, FTS5 session
  search, names at creation, copy-based forks, JSON export and resume. OS locks
  prevent concurrent turns in one session. Interrupted tool sequences are marked
  unknown-outcome on resume, without replay. Partial streams export separately.
- CLI chat/run/sessions/export/fork/models/doctor/config and headless JSON events.
- README, architecture, MIT license, contributing/security/changelog, provider
  configuration, troubleshooting, version-specific compatibility matrix and roadmap.
- GitHub Actions quality/security workflow (defined, not remotely executed).

## Checks actually executed

On Linux CachyOS x86_64, Rust 1.99.0 / Cargo 1.99.0:

- `cargo fmt --check` — passed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — passed.
- `cargo test --locked --workspace` — **26 passed**, no failed or ignored tests.
- `cargo build --locked --release` — passed.
- `python scripts/verify_terminal.py` — passed. Result: [terminal-validation.json](terminal-validation.json).
  Exercises actual binary in a PTY: first frame, multiline paste, debounced draft,
  resize/help, clean exit, SIGKILL/reopen with SQLite integrity, typing/help while a
  test HTTP server streams, cancellation and shutdown without losing the new draft.
- `python scripts/benchmark.py` — executed; raw warm help/startup samples in
  [benchmark-results.json](benchmark-results.json). This is NOT TUI cold-start latency.

Meaningful tests cover real HTTP framing and broken streams, malformed JSON/429,
provider cancellation, real file writes/hash conflict, scope/symlink denial,
mode/approval denial, actual shell exit codes and descendant cancellation,
agent step/context limits, pending-call recovery, concurrent SQLite commits,
exclusive session locks, fork isolation, config privilege escalation, and terminal
layouts/editor undo. Scripted test providers exist only inside test modules.

## Real local-model evidence, separate from fixtures

Used the already installed Qwen3.5 2B Q4_K_M through LM Studio on loopback. No cloud
endpoint or API key was used; validation used generated temporary repositories.
Context 8192, GPU ratio 0.5, temporary identifier `aether-validation`, TTL 300s.

1. A real streamed read-file request returned the unique on-disk marker, with
   native tool call and actual usage; process exit 0.
   Evidence: [validation/live-read.json](validation/live-read.json).
2. A real coding run read `answer.py` and its unittest, used the current read hash
   to change subtraction to addition, and ran the approved `python3 -m unittest -v`.
   The test demonstrably failed before the change and passed afterwards, exit 0.
3. The model then emitted an empty final completion. AETHER correctly failed that
   turn, exit 1, instead of fabricating a final summary. An explicit resumed ASK
   turn streamed a real model summary citing exit 0 and ended successfully. This
   proves durable continuation but **not** reliable unattended single-turn coding.
   Evidence, including before/after tests, model/tool history and resumed output:
   [validation/live-coding.json](validation/live-coding.json).

Earlier exploratory calls encountered absolute-path mistakes and denied writes
when stdin was noninteractive; they are not counted as successful task samples.
Within-root absolute paths were subsequently supported without allowing escapes.
No benchmark claim is based on these model runs. Qwen made redundant calls; model
quality and prompt sensitivity remain material reliability limits.

The temporary validation model was unloaded and the API server started for this
work was stopped. The existing LM Studio application was left open.

## Unverified / incomplete

- Full product, feature parity, production readiness and security review.
- Reliable unattended coding across representative tasks/models. The local coding
  turn required explicit follow-up after an empty completion.
- Native Anthropic/Gemini/Responses adapters, provider routing and capability-aware
  model selection. Ollama/llama.cpp/vLLM interoperability is not live-verified.
- Markdown/syntax highlighting, virtualized long timelines, command palette/common
  registry, theme/settings/model pickers, explorer, diff cards, clipboard, Vim mode,
  file/agent mentions and history search.
- Dedicated layered context engine, exact tokenizer, compaction, project memory
  and trustworthy context inspector. Current sidebar token number is approximate
  display size, not an exact model-request count. Context overflow stops explicitly.
- Cheap shared-history branches, merge/import, tags/pins/archive/rename UI,
  checkpoint restoration/preview and user-change conflict resolution. Snapshots
  exist as timeline data only; no rollback is exposed.
- MCP/ACP/LSP/tree-sitter integration, JS compatibility host, native/Wasmtime SDK,
  plugins and multi-agent worktrees. No inert command advertises these as working.
- Shell output streams to the model/UI after completion, not live per line. Capture
  is bounded. Detached children leaving the Unix group need stronger OS isolation.
- File APIs are not safe against hostile same-user path/rename races; shell has
  unrestricted account access after approval. Different sessions share a workspace.
- TUI transcript currently wraps the whole loaded session; large-history memory,
  scrolling latency, FPS and cold-start targets have not been demonstrated.
- A committed partial-stream chunk can recover through export, but partial text is
  not reconstructed in the main transcript after a crash. Latest uncommitted tail
  can be lost. No at-rest encryption or session retention policy.
- macOS/Windows runs, reproducibility on a fresh host, release signatures and remote CI.
- Local `cargo audit`/`cargo deny`: NOT passed or completed. `cargo-audit` was absent;
  installation was stopped after download/compilation delayed primary checks.
  CI includes both audits, but its result is unknown. No security-audited claim.
- No direct OpenCode performance comparison. Warm help measurements are narrowly
  scoped and cannot support “faster than OpenCode” claims.

## Next milestone

Finish reliable single-turn coding acceptance with a representative local model
and explicit empty/unsupported-response diagnostics; add structured provider
capabilities and better action progress. Then implement the shared command registry,
searchable palette, cached Markdown/diffs and virtualized timeline, with deterministic
TUI interaction tests and actual input-latency measurements. Do not rush plugin
compatibility or checkpoint restoration ahead of their security/acceptance gates.

Run the four Rust gates after any source change. Keep fixture evidence separate
from live-model evidence.

To reproduce a real-model check with manual approvals:
`python scripts/verify_live.py --config /absolute/path/to/trusted-config.toml --resume-summary`.
The runner creates an actual failing unittest fixture, checks that tests were not
modified, records outcomes, and never hides an initial turn failure with a successful
follow-up. It requires an already configured server; no automatic model launch.
