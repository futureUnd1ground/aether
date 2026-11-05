# PROJECT AETHER — NEXT-GENERATION OPEN-SOURCE AI CODING AGENT

## YOUR ROLE

You are a principal systems architect, senior Rust engineer, AI agent researcher, terminal UI specialist, performance engineer, and open-source maintainer.

Your mission is to DESIGN AND IMPLEMENT **AETHER**, a production-grade, extremely fast, modular, extensible, open-source AI coding agent.

AETHER must compete directly with OpenCode, Claude Code, Codex CLI, and other terminal-based AI coding agents.

Our primary benchmark is **OpenCode**.

The ambition is not to create a basic OpenCode clone. The ambition is to build a better system through stronger architecture, lower overhead, superior user experience, reliable agent execution, powerful session management, advanced context engineering, and excellent support for both local and cloud-based LLMs.

**Do not build a prototype disguised as a finished product.**

Build real, maintainable software.

Do not implement fake features, placeholder implementations, decorative buttons, simulated model responses, nonfunctional menu entries, or hardcoded demonstration behavior.

When something cannot be fully implemented, explicitly document the limitation rather than pretending it works.

Do not stop after writing an architectural plan. Implement the project incrementally, compiling and testing the actual code.

Make reasonable technical decisions autonomously.

Ask questions only when a decision genuinely blocks implementation.

---

# 1. PRODUCT PHILOSOPHY

AETHER should feel like the next generation of terminal-based AI development environments.

Core principles:

1. Performance is a product feature.
2. Local-first, cloud-optional.
3. Keyboard-first, mouse-friendly.
4. Beautiful but information-dense TUI.
5. Every action should be observable.
6. Sessions are durable, searchable, branchable, and recoverable.
7. Context is managed intelligently instead of blindly accumulated.
8. Agents must be useful, predictable, and controllable.
9. Extensions must be powerful without sacrificing security.
10. Model providers must be interchangeable.
11. The interface must remain responsive during all operations.
12. No unnecessary telemetry.
13. No vendor lock-in.
14. Minimal resource consumption.
15. Reliability is more important than feature count.
16. Advanced features must not make ordinary workflows complicated.

Every major feature should be evaluated against these principles.

---

# 2. RESEARCH AND COMPETITIVE ANALYSIS

Before implementation, investigate the current OpenCode repository and official documentation.

Primary reference:

https://github.com/anomalyco/opencode

Study:

- Architecture.
- Current TUI.
- Session persistence.
- Agent orchestration.
- Plugin APIs.
- Model providers.
- MCP support.
- LSP integration.
- Tool execution.
- Permission handling.
- Context management.
- Configuration.
- CLI workflows.
- Existing performance characteristics.

Examine the actual source code and current APIs instead of relying on outdated assumptions.

Also investigate:

- Ratatui.
- Tokio.
- tree-sitter.
- SQLite.
- Wasmtime.
- Model Context Protocol.
- Agent Client Protocol.
- Ollama.
- llama.cpp.
- Claude Code and Codex CLI interaction patterns.

Create:

`docs/competitive-analysis.md`

For every important OpenCode capability, document:

- Current OpenCode behavior.
- AETHER target behavior.
- Possible improvement.
- Implementation complexity.
- Compatibility considerations.
- Measurable success criteria.

Do not claim AETHER is faster or better without evidence.

The goal is to reach feature parity in important workflows, then surpass the baseline in measurable dimensions.

Respect licenses and avoid copying incompatible or proprietary code.

---

# 3. TECH STACK

Build the primary application in **Rust**.

Preferred stack:

Core:
- Rust stable.
- Tokio.
- anyhow / thiserror.
- tracing.
- serde.
- toml.

TUI:
- Ratatui.
- Crossterm.
- unicode-width.
- tui-textarea or a custom editor where justified.

Networking:
- reqwest.
- rustls.
- SSE streaming.
- WebSocket support where required.

Persistence:
- SQLite.
- WAL mode.
- schema migrations.
- indexed searches.

Code intelligence:
- ripgrep.
- tree-sitter.
- optional LSP clients.

Plugin runtime:
- JavaScript/TypeScript compatibility through an isolated Bun or Node.js process.
- Native Rust extension API.
- WebAssembly plugins using Wasmtime.

Security:
- Explicit permission boundaries.
- Scoped filesystem access.
- Process execution policies.
- Plugin capability declarations.

Profiling:
- criterion.
- cargo flamegraph.
- tracing spans.
- benchmark harness.

Prefer mature libraries.

Avoid building custom implementations of complicated infrastructure unless there is a strong technical justification.

Create a modular Cargo workspace.

Suggested structure:

```text
aether/
├── Cargo.toml
├── crates/
│   ├── aether-cli/
│   ├── aether-core/
│   ├── aether-tui/
│   ├── aether-agent/
│   ├── aether-providers/
│   ├── aether-tools/
│   ├── aether-context/
│   ├── aether-sessions/
│   ├── aether-storage/
│   ├── aether-plugins/
│   ├── aether-mcp/
│   ├── aether-lsp/
│   ├── aether-security/
│   ├── aether-git/
│   └── aether-bench/
├── packages/
│   ├── plugin-sdk/
│   └── compatibility-runtime/
├── docs/
├── tests/
├── examples/
└── .github/
```

This is a starting structure, not an obligation to create unnecessary crates.

Maintain clear boundaries and avoid circular dependencies.

---

# 4. APPLICATION ARCHITECTURE

Use an event-driven architecture.

Separate:

1. Terminal presentation.
2. Application state.
3. Agent execution.
4. Model communication.
5. Tool orchestration.
6. Context management.
7. Session storage.
8. Plugin execution.
9. Security decisions.

The TUI must never be blocked by:

- LLM generation.
- Shell commands.
- Filesystem scanning.
- Git operations.
- Plugin hooks.
- Database operations.
- Network requests.
- Context compaction.

Use asynchronous tasks and bounded channels.

Implement structured cancellation.

Every operation should have a lifecycle:

- Created.
- Queued.
- Running.
- Awaiting permission.
- Completed.
- Failed.
- Cancelled.

Implement event correlation IDs and structured tracing.

Do not use unbounded queues without justification.

Handle backpressure correctly.

Ensure reliable cleanup of child processes, subscriptions, and resources when a session ends or the application exits.

---

# 5. TERMINAL USER INTERFACE

The TUI is one of AETHER's most important competitive advantages.

It must feel polished, fast, intuitive, and professional.

Design a modern terminal interface inspired by the quality of excellent developer tools, without simply copying OpenCode.

## 5.1 Main layout

Suggested structure:

```text
┌───────────────────────────────────────────────────────────┐
│ AETHER  ~/projects/app        main ●       Claude Sonnet  │
├──────────────┬────────────────────────────────────────────┤
│ EXPLORER     │ SESSION: Refactor authentication           │
│              │                                            │
│ Sessions     │ User                                       │
│ > Current    │ Improve the auth middleware.              │
│   Previous   │                                            │
│   Branch A   │ Agent                                      │
│              │ Analyzing authentication flow...           │
│ Workspace    │                                            │
│ src/         │ ✓ Read 8 files                             │
│ tests/       │ ✓ Located middleware                       │
│ Cargo.toml   │ ◉ Editing src/auth.rs                       │
│              │                                            │
│ Agents       │ ┌─ DIFF ─────────────────────────────────┐ │
│ ● Main       │ │ - old implementation                   │ │
│ ○ Research   │ │ + new implementation                   │ │
│              │ └─────────────────────────────────────────┘ │
├──────────────┴────────────────────────────────────────────┤
│ > Ask anything...                                         │
├───────────────────────────────────────────────────────────┤
│ NORMAL │ Tokens 12.4k │ Context 18% │ Cost $0.04 │ 42ms   │
└───────────────────────────────────────────────────────────┘
```

This is conceptual.

Adapt intelligently to terminal width.

Do not render a large sidebar on narrow terminals.

## 5.2 Required UI capabilities

Implement:

- Fluid streaming output.
- Markdown rendering.
- Syntax-highlighted code.
- Inline diffs.
- Collapsible tool execution cards.
- Expandable reasoning summaries where available.
- Session browser.
- Agent activity view.
- File explorer.
- Git status view.
- Provider selector.
- Model selector.
- Command palette.
- Theme selector.
- Settings view.
- Permission dialogs.
- Search.
- Notifications.
- Help overlay.
- Keyboard shortcut discoverability.
- Mouse scrolling.
- Clipboard support with terminal-compatible fallbacks.

## 5.3 Input editor

Build an excellent multiline input editor.

Support:

- Multiline editing.
- History.
- Undo/redo.
- Word navigation.
- Keyboard shortcuts.
- File mentions.
- Command autocomplete.
- Slash commands.
- Agent mentions.
- Drag-and-drop file paths where terminals support it.
- Paste detection.
- Large pasted content.
- Draft persistence.

Support configurable Vim and Emacs-style keybindings.

## 5.4 Command palette

Implement a searchable command palette.

Examples:

- Switch model.
- Switch provider.
- Create session.
- Fork session.
- Rename session.
- Search sessions.
- Open file.
- Review changes.
- Change theme.
- Open settings.
- Manage plugins.
- Run diagnostics.
- Compact context.
- Show token usage.

Commands should be registered through a common command registry.

Plugins must be able to register commands.

## 5.5 Performance

Rendering must remain responsive during heavy agent activity.

Implement:

- Incremental updates.
- Virtualized message rendering.
- Cached Markdown parsing.
- Minimal unnecessary redraws.
- Bounded event queues.
- Debounced filesystem notifications.
- Efficient scrolling.

Target 60 FPS for animations or continuous UI updates on capable terminals, without rendering constantly when idle.

Idle CPU usage should be near zero.

Measure:

- Startup time.
- Input-to-render latency.
- Memory usage.
- Scroll performance.
- Rendering time for large sessions.

---

# 6. AGENT ENGINE

Implement a serious agentic execution engine.

The agent is not merely an LLM wrapper.

It should understand a task, inspect a project, make changes, run tools, validate results, recover from errors, and present a useful summary.

## 6.1 Agent execution loop

Implement a structured loop:

1. Receive user task.
2. Load session context.
3. Gather necessary workspace information.
4. Determine model and execution policies.
5. Send request.
6. Stream response.
7. Parse tool calls.
8. Validate tool permissions.
9. Execute tools.
10. Collect results.
11. Update context.
12. Continue until completion or stop condition.
13. Persist state.
14. Report results.

Support:

- Cancellation.
- Retries.
- Timeouts.
- Model failures.
- Tool failures.
- Context overflow.
- Rate limits.
- User interruption.
- Agent handoffs.
- Session recovery.

Never retry destructive actions blindly.

Never allow infinite agent loops without configurable budgets or stop conditions.

## 6.2 Agent modes

Implement:

### ASK

Read-only assistant.

Can inspect files and answer questions.

Cannot modify files without explicit approval.

### PLAN

Analyzes requirements and produces structured plans.

Can inspect project structure and relevant source files.

Does not modify source files unless explicitly authorized.

### BUILD

Full coding agent.

Can inspect files, edit code, run tests, and perform approved commands.

### DEBUG

Focused debugging agent.

Analyzes errors, stack traces, logs, and test failures.

Builds and verifies hypotheses.

### REVIEW

Code review agent.

Inspects changes, finds bugs, evaluates risks, and suggests fixes.

### CUSTOM

User-defined agents loaded through configuration or plugins.

Each agent should have configurable:

- System instructions.
- Model.
- Available tools.
- Permissions.
- Context budget.
- Maximum steps.
- Maximum cost.
- Temperature or supported sampling parameters.
- Timeout.
- Workspace scope.

---

# 7. MULTI-AGENT ORCHESTRATION

Build an optional multi-agent architecture.

Support specialist agents such as:

- Research.
- Architecture.
- Implementation.
- Testing.
- Review.
- Documentation.

Agents may work in parallel only when their tasks are sufficiently independent.

Implement:

- Parent/child task relationships.
- Explicit task delegation.
- Agent budgets.
- Cancellation propagation.
- Task progress tracking.
- Result aggregation.
- Structured agent-to-agent messages.

Avoid running expensive parallel agents unnecessarily.

Implement deterministic conflict-handling rules.

For parallel editing, prefer isolated Git worktrees or equivalent workspace isolation.

Never allow concurrent agents to overwrite each other's files silently.

Provide a TUI view showing agent hierarchy and current activity.

Implement a setting to disable multi-agent execution entirely.

---

# 8. SUPERIOR SESSION MANAGEMENT

Sessions are a core differentiator.

AETHER must treat sessions as durable development workspaces, not just chat logs.

## 8.1 Session features

Implement:

- Automatic persistence.
- Named sessions.
- Session tags.
- Session search.
- Session pinning.
- Session archiving.
- Session duplication.
- Session branching.
- Session merging where technically valid.
- Session export.
- Session import.
- Session recovery.
- Cross-project session browser.
- Session metadata.
- Token and cost statistics.
- Per-session model configuration.
- Per-session permissions.
- Draft recovery.

## 8.2 Session timelines

Represent sessions as an event history.

Important events include:

- User messages.
- Assistant messages.
- Tool executions.
- File changes.
- Model switches.
- Agent delegation.
- Permission approvals.
- Context compaction.
- Checkpoints.
- Errors.

Allow navigation through the timeline.

## 8.3 Session checkpoints

Implement checkpoints before meaningful code modifications.

Users should be able to:

- Inspect a checkpoint.
- Compare changes.
- Restore agent-created changes.
- Fork from a checkpoint.
- Continue from an earlier point.

IMPORTANT:

Do not implement unsafe global project rollback.

Restoration must preserve unrelated user changes.

Detect working-tree conflicts.

Show a preview before destructive restoration.

Prefer patch-based or snapshot-based restoration with explicit conflict handling.

## 8.4 Session branching

Support:

```text
Main Session
├── Try implementation A
├── Try implementation B
└── Research alternative
```

Branches should preserve their own context and history.

Avoid unnecessary duplication of immutable data.

Use efficient shared storage.

## 8.5 Session search

Implement full-text search across:

- Session titles.
- User prompts.
- Assistant messages.
- Tool outputs.
- Referenced files.
- Tags.

Use SQLite FTS where appropriate.

Searching should remain responsive with thousands of sessions.

## 8.6 Session integrity

Use transactional persistence.

Handle process crashes and unexpected shutdowns.

Test:

- Abrupt termination.
- Interrupted streaming.
- Partial writes.
- Database migrations.
- Large histories.
- Concurrent readers.

No silent data loss.

---

# 9. CONTEXT ENGINE

Context engineering must be one of AETHER's strongest features.

Do not simply send the entire chat history to the model.

Build a dedicated context management engine.

## 9.1 Context layers

Separate:

1. System instructions.
2. Agent instructions.
3. User task.
4. Conversation history.
5. Project instructions.
6. Relevant source files.
7. Tool results.
8. Persistent summaries.
9. Active task state.

Give each layer its own lifecycle.

## 9.2 Context budgeting

Track:

- Model context window.
- Current estimated token usage.
- Reserved output budget.
- Tool result budget.
- Available headroom.

Use model-specific tokenizers when available.

When exact tokenization is unavailable, clearly label estimates.

Do not claim estimated counts are exact.

## 9.3 Intelligent compaction

Implement context compaction that preserves:

- Current user objective.
- Important constraints.
- Relevant code decisions.
- Active plan.
- Modified file list.
- Outstanding issues.
- Recent tool failures.
- Required next steps.

Never silently discard critical instructions.

Preserve the original history in storage.

The compacted context is a derived representation.

## 9.4 Tool-output optimization

Avoid flooding context with enormous tool outputs.

Implement:

- Output truncation.
- Structured summaries.
- Relevant excerpt extraction.
- References to stored full outputs.
- Re-query capability.

Do not destroy information irreversibly.

## 9.5 Project memory

Implement optional project-scoped memory.

Examples:

- Coding conventions.
- Project architecture.
- Build commands.
- Test commands.
- User preferences.
- Known constraints.

Store memory in an inspectable format.

Make it editable and removable.

Do not automatically promote untrusted repository content into high-priority instructions.

## 9.6 Context inspector

Create a UI showing:

- Context usage.
- Major context contributors.
- Compaction events.
- Estimated token savings.
- Active project instructions.
- Attached files.

Users should be able to understand why the agent knows something.

---

# 10. PROVIDER SYSTEM

Design a universal provider abstraction.

Support both cloud APIs and local inference servers.

## 10.1 Required providers

Initial implementations:

- OpenAI-compatible API.
- OpenAI.
- Anthropic.
- Gemini.
- Ollama.
- LM Studio.
- llama.cpp server.
- vLLM.

Prioritize high-quality implementations over large numbers of incomplete integrations.

## 10.2 Provider capabilities

Define a capability system.

Possible capabilities:

- Streaming.
- Tool calling.
- Structured output.
- Image input.
- Extended reasoning.
- Prompt caching.
- Token usage reporting.
- Context window metadata.

Never assume every model supports every capability.

## 10.3 Model registry

Implement a searchable model registry.

Allow users to:

- Add custom endpoints.
- Configure API keys.
- Select models.
- Configure per-model parameters.
- Define aliases.
- Configure fallbacks.
- Test connectivity.

Never log API keys.

Store credentials securely using operating-system credential facilities where available, with a documented fallback.

## 10.4 Local-first experience

Local models must be first-class citizens.

Detect running local endpoints where practical.

Support:

- Configurable endpoint URLs.
- Model discovery.
- Streaming.
- Tool calling when supported.
- Capability checks.
- Connection diagnostics.

Do not assume local models have reliable tool calling.

Implement clear compatibility feedback.

Potential enhancement:

Provide a controlled fallback strategy for models with weak tool-calling capabilities.

Any fallback protocol must be tested and must not interpret arbitrary model text as executable commands.

## 10.5 Smart routing

Implement optional model routing.

Examples:

- Fast model for simple classification.
- Powerful model for architecture.
- Local model for private code analysis.
- Specialized model for review.

Routing must be predictable, configurable, and visible.

Do not silently send private code to external providers.

---

# 11. TOOL SYSTEM

Build a typed, extensible tool execution framework.

Required tools:

- Read file.
- Write file.
- Edit file.
- Apply patch.
- List directory.
- Glob.
- Grep.
- Search source.
- Execute shell command.
- Inspect Git status.
- View Git diff.
- Inspect commits.
- Run tests.
- Retrieve diagnostics.
- Request user approval.

Tools should define:

- Unique ID.
- Input schema.
- Output schema.
- Permissions.
- Timeout.
- Cancellation behavior.
- Execution cost.
- Logging policy.

Validate every tool input.

Do not execute malformed tool calls.

## 11.1 Efficient source search

Integrate ripgrep.

Prefer targeted searching over reading entire repositories.

Support ignore rules.

Avoid scanning enormous dependency directories unnecessarily.

## 11.2 Editing

Implement reliable patch application.

Features:

- Atomic writes where possible.
- Concurrent modification detection.
- Conflict reporting.
- Diff preview.
- File encoding awareness.
- Line ending preservation.
- Symlink security checks.

## 11.3 Shell execution

Support:

- Streaming stdout.
- Streaming stderr.
- Exit codes.
- Timeouts.
- Cancellation.
- Process tree cleanup.
- Configurable output limits.

Distinguish successful command execution from successful task completion.

Never assume a command succeeded merely because it produced output.

---

# 12. OPENCODE PLUGIN COMPATIBILITY

This is a critical product requirement.

AETHER must support useful compatibility with existing OpenCode plugins.

However, do not assume OpenCode plugins can execute natively inside Rust.

Research the current OpenCode plugin architecture and supported versions.

Implement a compatibility layer using an isolated JavaScript/TypeScript runtime.

Potential design:

```text
AETHER RUST CORE
       │
       │ Typed IPC / RPC
       ▼
PLUGIN COMPATIBILITY HOST
       │
       ├── OpenCode plugins
       ├── JavaScript plugins
       ├── TypeScript plugins
       └── Compatibility adapters
```

## Requirements

- Plugin discovery.
- Plugin loading.
- Lifecycle management.
- Configuration.
- Hook registration.
- Tool registration.
- Command registration.
- Event subscriptions.
- Error isolation.
- Plugin diagnostics.

Version compatibility must be explicit.

Create a compatibility matrix.

Implement integration tests using real representative plugins.

Do not claim full compatibility until verified.

## Plugin security

Assume third-party plugins may execute arbitrary code.

Do not describe a Node.js or Bun subprocess as a security sandbox by itself.

Use:

- Explicit trust prompts.
- Process isolation.
- Optional operating-system sandboxing.
- Configurable permissions.
- Clear filesystem and network access policies.

Document platform limitations.

Never silently execute newly discovered plugins.

---

# 13. NATIVE PLUGIN SDK

In addition to OpenCode compatibility, build an AETHER-native extension system.

Support:

- Rust extensions where appropriate.
- WebAssembly plugins.
- JavaScript/TypeScript SDK.

Provide stable interfaces for:

- Custom tools.
- Agent definitions.
- Model providers.
- Commands.
- Context processors.
- Session hooks.
- UI panels.
- Notifications.

Prefer versioned APIs.

Plugins should declare required capabilities.

Provide example plugins and SDK documentation.

Avoid letting extensions arbitrarily mutate internal application state.

---

# 14. MCP SUPPORT

Implement Model Context Protocol integration.

Support compatible MCP transports as defined by the current specification.

Features:

- Server configuration.
- Connection lifecycle.
- Tool discovery.
- Tool invocation.
- Timeouts.
- Error handling.
- Reconnection.
- Permission control.
- Server health information.

Show MCP server status inside the TUI.

Do not automatically trust remote MCP tool descriptions.

Treat external tool output as untrusted data.

---

# 15. LSP AND CODE INTELLIGENCE

Add optional Language Server Protocol integration.

Support:

- Diagnostics.
- Symbol lookup.
- Go to definition.
- Find references.
- Workspace symbols.
- Document symbols.

Use LSP capabilities when available.

Fallback to ripgrep and tree-sitter when appropriate.

Do not require language servers for basic operation.

Avoid launching unnecessary language servers.

Make code intelligence resource-aware.

---

# 16. GIT INTEGRATION

Build strong Git awareness.

Required:

- Branch detection.
- Changed file indicators.
- Diff previews.
- Git status.
- Commit inspection.
- Worktree awareness.
- Conflict detection.

Optional advanced features:

- Isolated agent worktrees.
- Per-task branches.
- Checkpoint comparisons.
- Automated commit proposal.

Never silently commit, push, force-push, reset, or discard user changes.

Require explicit approval for destructive Git operations.

---

# 17. SECURITY MODEL

Security must be designed before plugin and tool execution.

Create a central permissions engine.

Permission categories:

- Read files.
- Modify files.
- Execute commands.
- Access network.
- Access credentials.
- Modify Git state.
- Execute plugins.
- Launch subagents.

Support:

- Ask every time.
- Allow once.
- Allow for session.
- Allow for project.
- Deny.

Certain high-risk operations should require fresh approval regardless of broad permissions.

Implement command risk analysis as a defense-in-depth feature, not as a perfect security boundary.

A safer execution architecture should combine permissions with operating-system isolation when available.

Prevent obvious path traversal and symlink escape issues in scoped file operations.

Clearly distinguish:

- Trusted system instructions.
- User instructions.
- Repository files.
- Tool outputs.
- External web content.
- Plugin data.

Treat repository instructions and external content as potentially untrusted.

Protect against prompt injection.

Provide a readable security audit log.

---

# 18. ERROR HANDLING AND RECOVERY

AETHER should behave predictably when things go wrong.

Handle:

- Provider outages.
- Network timeouts.
- Invalid API responses.
- Broken streaming.
- Rate limits.
- Missing tools.
- Plugin crashes.
- Database errors.
- Shell execution failures.
- Context exhaustion.
- Invalid patches.
- Interrupted updates.

Display actionable errors.

Do not show raw stack traces to ordinary users unless diagnostic mode is enabled.

Preserve enough diagnostic information for debugging.

Implement graceful shutdown.

Test abrupt exits and recovery.

---

# 19. PERFORMANCE ENGINEERING

Performance is a primary objective.

Do not merely choose Rust and assume the result is fast.

Measure real performance.

## Benchmark targets

Set provisional goals, then validate against real hardware and representative workloads.

Suggested targets:

- Cold startup under 100 ms on a representative modern development machine, excluding optional network discovery.
- Idle CPU usage near zero.
- Smooth scrolling across large sessions.
- Responsive typing during streaming and tool execution.
- Low baseline memory footprint.
- Fast session switching.
- Efficient search across large repositories.
- Minimal latency between tool completion and the next agent step.

These are targets, not guaranteed results.

Create reproducible benchmarks.

Compare against the installed OpenCode version using identical machines, tasks, repositories, model endpoints, and cache conditions.

Measure:

1. Cold startup.
2. Warm startup.
3. Idle memory.
4. Idle CPU.
5. Input latency.
6. Streaming render latency.
7. Session load time.
8. Search performance.
9. Tool execution overhead.
10. Memory usage with large histories.
11. Agent task completion time.
12. Agent reliability.
13. Token consumption.
14. Cost per successfully completed task.

Separate model inference latency from application overhead.

Do not manipulate benchmark conditions to make AETHER look better.

Store benchmark methodology and results in the repository.

---

# 20. UX FEATURES THAT SHOULD DIFFERENTIATE AETHER

Implement these progressively after the reliable foundation exists.

## 20.1 Session overview

Show sessions with:

- Title.
- Project.
- Agent status.
- Model.
- Last activity.
- Token usage.
- Cost.
- Current Git branch.

## 20.2 Context inspector

Let users inspect context composition and compaction.

## 20.3 Agent execution timeline

Provide a chronological view of:

- Reasoning summaries.
- Tool calls.
- File modifications.
- Test execution.
- Errors.
- Agent delegation.

## 20.4 Review before apply

Optional mode in which edits are prepared and reviewed before being committed to the working tree.

## 20.5 Checkpoint explorer

Visually navigate agent-created changes.

## 20.6 Model comparison

Allow the user to submit the same prompt to multiple models intentionally.

Keep outputs separate.

Display usage and latency metrics.

Do not send data to multiple providers without the user's explicit action.

## 20.7 Smart status bar

Show useful information without clutter:

- Current mode.
- Model.
- Provider.
- Context usage.
- Session cost.
- Git status.
- Current execution state.

## 20.8 Terminal themes

Ship polished dark and light themes.

Support custom themes.

Support graceful rendering in terminals with limited color capabilities.

Respect `NO_COLOR`.

## 20.9 Headless mode

AETHER must work without TUI.

Example commands:

```bash
aether
aether chat
aether run "Fix failing tests"
aether sessions
aether models
aether plugins
aether doctor
aether benchmark
```

Support structured JSON output where appropriate.

Exit codes must be meaningful.

---

# 21. CONFIGURATION

Support global and project-specific configuration.

Example:

```toml
[ui]
theme = "default-dark"
show_sidebar = true
vim_mode = false

[agent]
default_mode = "build"
max_steps = 80
auto_compact = true

[provider]
default = "ollama"

[models]
default = "qwen-coder"

[sessions]
auto_save = true
checkpoints = true

[plugins]
enabled = true
```

The example is illustrative; define and validate a real configuration schema.

Support environment-variable overrides.

Document precedence.

Provide migration behavior for future schema changes.

Never silently discard unknown user configuration.

---

# 22. TESTING STRATEGY

Tests are mandatory.

## Unit tests

Cover:

- State transitions.
- Context budgeting.
- Session branching.
- Configuration parsing.
- Provider protocol parsing.
- Permission evaluation.
- Patch handling.

## Integration tests

Cover:

- Mock provider streaming.
- Tool execution lifecycle.
- Session persistence.
- Plugin host communication.
- MCP interactions.
- Agent cancellation.
- Crash recovery.

## TUI tests

Use deterministic terminal backends.

Test:

- Layout.
- Resizing.
- Keyboard navigation.
- Input editing.
- Scrolling.
- Permission dialogs.
- Large message histories.

## End-to-end tests

Create reproducible coding tasks in test repositories.

Examples:

- Fix a failing unit test.
- Add a small feature.
- Refactor a module.
- Diagnose a compile error.
- Update documentation.
- Review a patch.

Validate resulting changes rather than just checking whether the model produced text.

## Quality gates

Require:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

Also add security auditing, dependency checks, and CI validation.

Do not claim checks passed unless they were actually executed successfully.

---

# 23. DOCUMENTATION

Create useful documentation:

- README.md
- CONTRIBUTING.md
- ARCHITECTURE.md
- SECURITY.md
- CHANGELOG.md
- LICENSE
- Installation guide.
- Configuration guide.
- Provider guide.
- Plugin development guide.
- OpenCode compatibility guide.
- Troubleshooting guide.
- Performance benchmarks.

Use accurate examples.

Document known limitations.

Never write marketing claims unsupported by implementation.

---

# 24. DEVELOPMENT PHASES

Implement in coherent vertical slices.

## PHASE 1 — Foundation

Deliver:

- Cargo workspace.
- Real executable.
- Configuration.
- Logging.
- TUI skeleton.
- Async runtime.
- Basic provider abstraction.
- SQLite initialization.

Acceptance test:

AETHER starts, renders correctly, exits cleanly, and persists configuration.

## PHASE 2 — Functional coding agent

Deliver:

- Streaming model interaction.
- Agent loop.
- File reading.
- File editing.
- Shell execution.
- Permission prompts.
- Basic conversation history.

Acceptance test:

AETHER can inspect a real repository, modify a file, run a test, and report the actual outcome.

## PHASE 3 — Excellent TUI

Deliver:

- Session sidebar.
- Markdown rendering.
- Diff views.
- Command palette.
- Input editor.
- Model selector.
- Responsive layouts.

Acceptance test:

The TUI remains responsive during streaming and shell execution.

## PHASE 4 — Durable sessions

Deliver:

- Search.
- Branching.
- Recovery.
- Checkpoints.
- Export/import.
- Timeline.

Acceptance test:

Sessions survive application crashes and can be reopened without losing committed history.

## PHASE 5 — Context intelligence

Deliver:

- Token estimation.
- Context budgeting.
- Compaction.
- Context inspector.
- Persistent project instructions.

Acceptance test:

Large sessions remain functional while preserving task-critical information.

## PHASE 6 — Extensibility

Deliver:

- Plugin SDK.
- OpenCode compatibility host.
- MCP integration.
- Plugin diagnostics.

Acceptance test:

Representative real plugins and MCP servers work through tested interfaces.

## PHASE 7 — Advanced agents

Deliver:

- Agent modes.
- Delegation.
- Optional parallel subagents.
- Workspace isolation.
- Review workflows.

Acceptance test:

Independent agent tasks complete without uncontrolled file conflicts.

## PHASE 8 — Optimization and release

Deliver:

- Benchmark suite.
- Profiling.
- Performance improvements.
- Security review.
- Release artifacts.
- Installation documentation.

Acceptance test:

Publish reproducible benchmark data and known limitations.

Do not abandon an earlier phase with failing tests to rush toward later features.

---

# 25. ARCHITECTURAL QUALITY REQUIREMENTS

Follow these engineering practices:

- Clear ownership of application state.
- Dependency inversion.
- Typed interfaces.
- Explicit error handling.
- Small, composable modules.
- Bounded concurrency.
- Structured cancellation.
- Deterministic testing.
- Transactional persistence.
- Minimal global mutable state.
- No unnecessary cloning of large objects.
- Avoid blocking operations on async executor threads.
- Safe subprocess management.
- Versioned extension interfaces.

Prefer readability and correctness over clever abstractions.

Avoid premature micro-optimization.

Profile before optimizing.

Do not create thousands of lines of untested code in one iteration.

---

# 26. WHAT NOT TO DO

DO NOT:

- Build only a visual mockup.
- Build only a chatbot.
- Copy OpenCode's UI without improvement.
- Pretend plugins work.
- Fake streamed responses.
- Use unsafe Rust without clear justification.
- Store API keys in plaintext logs.
- Execute untrusted plugins silently.
- Use unlimited agent loops.
- Swallow errors.
- Silently overwrite user changes.
- Spawn unlimited parallel agents.
- Send local code to cloud providers without user authorization.
- Claim benchmarks without running them.
- Mark features complete when they are placeholders.
- Produce a giant architectural essay instead of working code.
- Sacrifice reliability for an impressive feature list.

---

# 27. DEFINITION OF DONE

AETHER can be considered production-ready only when:

1. It builds reproducibly.
2. The TUI is stable and responsive.
3. Users can configure real providers.
4. Local models work through supported endpoints.
5. Agent tools execute reliably.
6. Permissions are enforced.
7. Sessions persist and recover.
8. File changes are inspectable.
9. Errors are handled properly.
10. Plugin compatibility is documented and tested.
11. Performance measurements exist.
12. CI passes.
13. Documentation reflects reality.
14. The application is usable for real coding tasks.

No feature should be described as complete until its acceptance criteria pass.

---

# 28. IMMEDIATE EXECUTION INSTRUCTIONS

Start working now.

First:

1. Inspect the current repository.
2. Determine whether an existing codebase is present.
3. Research current OpenCode architecture and plugin APIs.
4. Create a concise competitive analysis.
5. Write `ARCHITECTURE.md`.
6. Create an implementation roadmap with measurable milestones.
7. Initialize the Rust workspace.
8. Implement the first working vertical slice.

Do not spend the entire execution producing planning documents.

The first vertical slice must include:

- Launchable TUI.
- Working multiline input.
- Provider configuration.
- Real streaming from at least one OpenAI-compatible endpoint.
- A minimal functional agent loop.
- Basic file reading with permission checks.
- Persistent sessions.
- Clean shutdown.

Then execute relevant formatting, linting, compilation, and tests.

Fix failures before proceeding.

At the end of each milestone:

- Summarize implemented features.
- Identify changed modules.
- Report tests actually executed.
- Document remaining limitations.
- State the next implementation milestone.
- Continue implementation if execution capacity permits.

If the working context becomes limited, persist progress in `docs/IMPLEMENTATION_STATUS.md` so another Codex run can continue without losing architectural decisions.

Treat that file as a progress record, not as evidence that features work.

Do not stop merely because the repository structure exists.

Do not mark the project finished until the actual definition of done is satisfied.

---

# 29. FINAL PRODUCT EXPECTATION

The final product should be a terminal-native AI development environment that users choose because it provides:

- Lower application overhead.
- Excellent responsiveness.
- Dependable agent execution.
- Superior session navigation.
- Powerful context inspection.
- First-class local model support.
- Flexible model providers.
- Extensible tooling.
- Practical OpenCode plugin compatibility.
- Strong safety controls.
- A clean, beautiful interface.
- Reliable recovery.
- A genuinely enjoyable developer experience.

**Build AETHER as a serious open-source engineering product, not a weekend demonstration.**

Start implementation immediately.