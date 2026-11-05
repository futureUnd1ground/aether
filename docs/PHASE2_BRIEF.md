# AETHER — PHASE 2: ADVANCED MULTI-AGENT ENGINE

Continue developing the existing AETHER Rust repository.

**CRITICAL: Do not recreate the project, replace its architecture unnecessarily, or break the already working functionality.**

First inspect the actual codebase, architecture, tests, and `docs/IMPLEMENTATION_STATUS.md`.

The existing implementation reportedly supports TUI, SSE streaming, an agent loop, permissions, filesystem tools, shell execution, and SQLite sessions.

Verify these capabilities rather than assuming they work.

Your next mission is to transform AETHER into a powerful multi-agent coding environment.

## PRIORITY 0 — Fix agent completion reliability

Investigate the issue where a local Qwen model successfully modifies files and runs tests but finishes with an empty user-facing response.

Implement reliable detection of:

- Completed assistant responses.
- Tool-call-only responses.
- Empty final messages.
- Truncated generations.
- Interrupted streams.
- Context exhaustion.
- Provider-specific finish reasons.

Distinguish successful task completion from merely finishing tool execution.

Allow a bounded recovery attempt when the final response is missing.

Do not introduce infinite continuation loops.

Preserve session integrity.

Add regression tests.

## PRIORITY 1 — Real subagents

Implement first-class subagents inside the existing Rust agent engine.

Each subagent must have:

- Unique ID.
- Parent agent ID.
- Independent execution state.
- Independent conversation context.
- Configurable model.
- Configurable tools.
- Permission scope.
- Token and step budgets.
- Cancellation token.
- Structured task description.
- Persisted execution history.
- Structured result.

The primary agent must be able to delegate tasks through a typed `spawn_agent` tool.

Implement `await_agent`, `list_agents`, and `cancel_agent`.

Do not simulate agents by generating fake responses.

Every subagent must execute through the real provider and tool infrastructure.

## PRIORITY 2 — Recursive delegation

Allow subagents to create child agents.

Enforce configurable limits:

- Maximum nesting depth.
- Maximum concurrent agents.
- Maximum agents per task.
- Total token budget.
- Total cost budget.
- Maximum execution steps.

Defaults should be conservative.

Prevent uncontrolled recursive spawning and resource exhaustion.

## PRIORITY 3 — Parallel execution

Support concurrent independent agent tasks through Tokio.

For example:

- Research Agent analyzes architecture.
- Testing Agent investigates failing tests.
- Review Agent inspects changes.

Provide a scheduler with bounded concurrency.

Propagate cancellation from parent to children.

Handle partial failures without corrupting the parent session.

## PRIORITY 4 — Workspace isolation

Parallel agents must not silently overwrite each other's edits.

Read-only agents may share the workspace.

Writing agents should use isolated Git worktrees or another explicitly designed isolation mechanism.

Implement conflict detection and controlled integration of changes.

Do not silently merge conflicting modifications.

Preserve unrelated user changes.

## PRIORITY 5 — Agent session tree

Persist parent-child agent relationships in SQLite.

Users must be able to inspect past subagent executions after restarting AETHER.

Store:

- Agent hierarchy.
- Task.
- Model.
- Status.
- Tool execution history.
- Results.
- Errors.
- Resource usage.

Use migrations compatible with existing databases.

## PRIORITY 6 — Multi-agent TUI

Add an interactive agent tree.

Example:

```text
AGENTS
● Main Agent
  ├─ ✓ Research
  ├─ ◉ Backend
  │   ├─ ✓ Database
  │   └─ ◉ Authentication
  └─ ○ Review
```

Allow users to:

- Open any agent.
- Inspect its messages.
- Inspect tool calls.
- View progress.
- Cancel execution.
- See errors.
- Navigate back to the parent.
- View the final result.

The TUI must remain responsive during concurrent execution.

## PRIORITY 7 — Model assignment

Allow different models for different agents.

Examples:

- Main: Claude through API.
- Research: local Qwen.
- Review: GPT through API.

Use existing provider abstractions.

Do not hardcode provider names into orchestration logic.

Never silently transmit code to external providers.

## PRIORITY 8 — Agent definitions

Support configurable agent profiles.

Provide built-in profiles:

- Explore.
- Research.
- Build.
- Debug.
- Test.
- Review.

Allow custom profiles through configuration files.

Profiles must specify permissions and available tools explicitly.

## PRIORITY 9 — Testing

Create tests for:

1. Agent spawning.
2. Parent-child relationships.
3. Recursive depth limits.
4. Parallel scheduling.
5. Budget enforcement.
6. Cancellation propagation.
7. Subagent failures.
8. Session recovery.
9. Workspace conflicts.
10. Local-model execution.
11. Empty final responses.
12. TUI responsiveness.

Run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

Fix failures.

## IMPLEMENTATION REQUIREMENTS

Work incrementally.

Reuse existing code where possible.

Preserve backward compatibility with existing sessions.

Do not introduce unnecessary dependencies.

Do not add placeholder agent APIs.

Do not claim successful execution without actual tests.

Implement the smallest working vertical slice first: the main agent spawns one real read-only subagent, receives a result, and persists the relationship.

Then incrementally add concurrency, recursion, isolation, and the TUI.

Update `docs/IMPLEMENTATION_STATUS.md` after each completed milestone.

Continue implementation beyond planning documents.

**Success condition:** A user can ask AETHER to investigate and implement a coding task, AETHER can delegate independent work to real subagents, the user can inspect their activity, and all executions remain recoverable after a restart.

Start by inspecting the current repository and fixing the empty-final-response issue.