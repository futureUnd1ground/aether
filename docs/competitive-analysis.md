# Competitive baseline

Research date: 2026-11-05. Primary source pinned to OpenCode `dev` commit
[`74fb86137c10e4260399fca59aaa4b0bf0960a7b`](https://github.com/anomalyco/opencode/tree/74fb86137c10e4260399fca59aaa4b0bf0960a7b).
This is an observed source snapshot, not a claim that all installed versions expose
this API. No OpenCode performance measurements have been collected.

The current repository differs substantially from older examples. The old
`packages/plugin/src/index.ts` returns 404 at this snapshot. Source now has
`packages/core`, `packages/tui`, `packages/plugin` with Effect and Promise plugin
surfaces. The TUI package declares 2.2.5 and uses OpenTUI/Solid, not Ratatui.

| Area | Observed OpenCode behavior/source | AETHER target / possible improvement | Complexity | Compatibility concern | Acceptance criterion |
|---|---|---|---|---|---|
| Runtime/UI | Separate core and OpenTUI/Solid TUI | Bounded Rust tasks, idle redraw suppression | High | Plugin UI cannot map directly to Ratatui | Measure typing p95 <50ms while streaming; idle CPU measured |
| Sessions | Drizzle SQLite `session_v2`, parent/fork boundary, usage/cost metadata | Transactional event timeline and safe recovery | High | Import requires versioned semantic conversion | Kill/reopen loses no committed event |
| Agent | Effect-based runner step with retry/recovery/continuation outcomes | Typed loop with cancellation and explicit budgets | High | Tool schemas/provider semantics differ | Real coding-task assertions, no blind action replay |
| Providers | Dedicated provider plugins, including Ollama, LM Studio, vLLM | Local default, explicit remote authorization | High | Compatible endpoints have partial tool support | Protocol fixtures plus real endpoint trial |
| Permissions | Allow/ask/deny and persisted project/action/resource records | Once-only approval and central mode enforcement; OS isolation later | High | Shell analysis is not a sandbox | Denied actions cannot mutate workspace |
| Context | Compaction module has buffer, keep, precompute controls | Inspectable layers and preserved original events | High | Summary state is not original history | Critical constraints survive a bounded-context task |
| Tools/Git | Runner integrates tool output, snapshots and permissions | Checksum edits, backups, conflict-aware restoration | High | Preserve unrelated user changes | Concurrent-edit and restore-conflict tests |
| Plugins | Promise context includes tool, command, event, permission, provider, storage, session, worktree domains | Versioned JS subprocess adapter with declared unsupported domains | Very high | Old `@opencode-ai/plugin` differs from `@opencode/plugin/promise` | Real pinned third-party plugin integration tests |
| MCP | Core has MCP domain/plugin hooks; documented server integration | Revision-pinned stdio and Streamable HTTP client | High | Current spec differs from 2025 handshake semantics | Discovery, invoke, cancel, timeout tests per revision |
| LSP | Documented optional language-server integration | On-demand clients with rg fallback | High | Server capabilities differ | Real diagnostics/definition/reference tests |
| Config/CLI | Provider, permissions, TUI, session and CLI docs | Strict TOML merging and JSON headless events | Medium | No promise to accept OpenCode config | Roundtrip config; unknown keys actionable |
| Performance | No measurements collected in this environment | Reproducible same-machine comparison | High | Match model, cache, tasks and versions | Publish raw samples, hardware and methodology |

## Source evidence

Reviewed source, not just search summaries:

- [TUI dependencies](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/tui/package.json).
- [Session schema](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/core/src/session/sql.ts).
- [Agent step](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/core/src/session/runner/step.ts).
- [Compaction](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/core/src/session/compaction.ts).
- [Promise plugin context](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/plugin/src/promise/plugin.ts),
  [tool hooks](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/plugin/src/promise/tool.ts),
  [permission schema](https://github.com/anomalyco/opencode/blob/74fb86137c10e4260399fca59aaa4b0bf0960a7b/packages/core/src/permission/sql.ts).
- [OpenCode permissions](https://opencode.ai/docs/permissions/), [plugins documentation](https://docs.opencode.ai/docs/plugins/).

## Supporting technology decisions

- [Ratatui async event streams](https://ratatui.rs/tutorials/counter-async-app/async-event-stream/)
  and [Tokio shutdown](https://tokio.rs/tokio/topics/shutdown): keep presentation,
  work and cancellation separate; use mature editor widget rather than homemade text editing.
- [SQLite WAL](https://sqlite.org/wal.html): transactional readers/writer with FULL
  durability; bundled SQLite pinned by Cargo.lock. WAL is not a substitute for backups.
- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/): incremental syntax trees;
  defer grammar loading until indexed search and resource budgets exist.
- [Wasmtime security](https://docs.wasmtime.dev/security.html): future capability
  host must bound fuel, memory and exposed WASI resources; not yet implemented.
- [MCP transport specification, 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports):
  stdio and Streamable HTTP; protocol revision must be explicit, not assumed from older examples.
- [ACP v1](https://agentclientprotocol.com/protocol/v1/overview): editor/agent session,
  tool updates and cancellation. Investigated, not implemented.
- [Ollama compatibility](https://docs.ollama.com/api/openai-compatibility),
  [llama.cpp server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md):
  reuse OpenAI-compatible adapter first; verify native tool support per loaded model.

[Claude Code interactive documentation](https://code.claude.com/docs/en/interactive-mode)
and the [Codex CLI repository](https://github.com/openai/codex) informed interaction goals: discoverable approval, interruptible
turns, editable prompt while busy, explicit results and resumable sessions. AETHER
has no proprietary implementation dependency and copies no source from them.

License: OpenCode snapshot is MIT. This project is independently implemented under
MIT. Future plugin dependencies require their own license review.
