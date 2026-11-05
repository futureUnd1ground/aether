# Changelog

## 0.2.0-alpha.1

- Real engine-backed subagents with independent context/session, profiles, tools,
  inherited permissions, model selection, status, resource usage and results.
- Recursive delegation, bounded parallel scheduling and token/cost/step budgets.
- Parent-child cancellation, addressable owner control and crash-safe inspection.
- Detached writing worktrees, preview/approval, conflict detection, journaled
  integration and conservative rollback. No automatic commit/merge or project reset.
- Sanitized offline Linux bubblewrap shell for subagents; no unrestricted fallback.
- Agent tree/inspector, historical CLI commands and queued parallel permissions.
- Bounded empty-final summary recovery; explicit provider finish/error classification.
- SQLite schema 2 migration preserving existing sessions.
- Ratatui 0.30 and maintained textarea fork; removed archived paste dependency.
- Real local-model validation, terminal captures, expanded integration tests and CI.

## 0.1.0 — foundation

- Modular Rust CLI/TUI, strict local-first configuration and tracing.
- OpenAI-compatible SSE and structured function tools.
- Scoped file access, checksum writes and approved bounded shell execution.
- SQLite WAL history, session search/fork/export, resume and durable drafts.
