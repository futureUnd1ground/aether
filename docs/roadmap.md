# Measurable implementation roadmap

Each gate must pass before its milestone is marked complete. Scaffolding is not
acceptance evidence. Current results belong in IMPLEMENTATION_STATUS.md.

1. **Executable foundation:** strict config roundtrip, terminal enter/exit and
   resize tests, async bounded events, SQLite migrations, runnable release binary.
2. **First functional slice:** multiline editor, real SSE parsing, typed read/list
   tools with approval, durable messages, resume, headless mode. Test fragmented
   streams, malformed input, broken connections, denied actions and cancellation.
   A real configured model request remains a separate acceptance gate.
3. **Coding workflow:** checksum-guarded writes, approval preview, bounded shell,
   backups and actual failing-test → edit → passing-test fixture. Require explicit
   shell consent even under broad write permission. Prove child cleanup.
4. **TUI quality:** cached Markdown/syntax, virtualized timeline, common command
   registry/palette, settings/model switching, searchable session browser and
   diff cards. Benchmark typing p95 and scrolling at 100k events.
5. **Durable sessions:** shared immutable forks, validated imports, tags/archive,
   conflict-aware checkpoint preview/restore. Test abrupt kill, concurrent readers,
   and schema upgrade from every supported release.
6. **Context intelligence:** trustworthy layer inspector, tokenizer capabilities,
   derived compaction with task/constraints preserved. Test overflow and resume;
   original events remain exportable.
7. **Extensions:** versioned IPC, explicit trust and OS sandbox policy, plugin SDK,
   pinned real OpenCode plugin subset and revision-pinned MCP client. No full
   compatibility claim without a passing public matrix. Wasm after JS lifecycle.
8. **Advanced agents:** custom agents, budgets, hierarchy and isolated worktrees;
   cancellation propagation and deterministic edit conflicts. Disabled by default.
9. **Release gate:** LSP/on-demand intelligence, security review, dependency audit,
   actual local/cloud-provider checks, Linux/macOS/Windows matrices, reproducible
   performance comparison and release packaging. Full product definition of done
   from the user's brief applies; reaching milestone 3 is not production readiness.
