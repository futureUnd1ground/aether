# Multi-agent engine

AETHER's `spawn_agent` creates a real execution, not a prompt impersonating another
agent. Children use the existing provider, permission, filesystem and streaming
engine. They have UUIDs, parent/root IDs, independent SQLite sessions and context,
a model, explicit tools and permissions, budgets, scope, cancellation, status,
resource usage and a structured result.

## Tools and lifecycle

- `spawn_agent {"profile":"explore","task":{"title":"Inspect module","description":"Read src/auth.rs and report findings","success_criteria":["Cite concrete file evidence"]}}`
- `await_agent {"agent_id":"..."}` returns the persisted result/error/status.
- `list_agents {}` lists the caller and its descendants.
- `cancel_agent {"agent_id":"..."}` requests descendant cancellation recursively.
- `integrate_agent {"agent_id":"..."}` is available to writable parents: inspect a
  completed direct child's isolated changes and request fresh integration approval.

An agent can only await/cancel descendants; self/ancestor waits are rejected.
Parents must await needed children before finalizing. Finishing a parent cancels
unawaited descendants. Histories and worktrees remain, including failed work.
Waiting for a child or permission holds no execution slot, preventing nested-wait
deadlocks even with `max_concurrent=1`. Slots bound model requests and tool work.

Task descriptions are passed explicitly. Children do not inherit the whole parent
conversation. Put relevant objectives and constraints into the delegated task.
Repository/tool output remains untrusted data, not authority to alter permissions.

## Conservative limits

```toml
[orchestration]
enabled = true
max_depth = 2
max_concurrent = 2
max_agents = 8
total_tokens = 131072
total_steps = 60
```

Root depth is zero and the root counts toward the agent cap. Spawn admissions
consume the cap even if subsequent approval/setup fails; this prevents endless
setup attempts. A maximum of 64 agents, 16 concurrent slots and depth 8 is accepted
by configuration validation. Each agent also has a step/token/time limit.

Each request reserves a conservative byte-based input allowance, output budget and
configured price before network execution. Shared reservations are serialized, so
parallel agents cannot each spend the same remaining budget. Reported usage replaces
the reservation; if usage is absent or a request fails, the conservative charge is
retained. These charges are not exact tokenization or billing. A provider reporting
more than the reservation triggers cancellation before executing its tools.

For a finite `max_cost_usd`, provide **both** `input_cost_per_million` and
`output_cost_per_million` in every used provider config. Unknown pricing fails
before the request; it is never guessed to be zero. Pricing remains operator input,
not a maintained external price catalog. `cost_usd = null` means unknown.

## Profiles and models

Built-ins: explore, research, build, debug, test, review. The read-only profiles have
scoped read/list tools; writing/testing profiles require isolated workspaces. Custom
profiles use the same validated schema. [Complete example](../examples/multi-agent.toml).

```toml
[profiles.my_reviewer]
instructions = "Review src without modifying it. Report evidence and uncertainty."
mode = "review"
model = "your-loaded-review-model"
scope = "src"
tools = ["read_file", "list_directory"]
can_delegate = false
max_steps = 8
max_tokens = 20000
context_tokens = 8192
timeout_secs = 180

[profiles.my_reviewer.permissions]
read = "allow"
write = "deny"
shell = "deny"
```

Profile grants are intersected with the parent's permissions and tool allowlist;
Deny wins, Ask cannot become Allow through a child. Read-only parents cannot spawn
writing profiles. A child cannot escape its parent's directory scope. Trust-sensitive
profiles, routing and orchestration settings must come from global or explicit
`--config` files; repository config cannot grant them.

`provider = "alias"` selects a named `[providers.alias]` configuration; `model`
selects an explicit model. The provider trait performs reconfiguration, keeping
orchestration independent of vendor names. There is no fallback router. Changing
to a remote model/provider requires spawn approval even when that named endpoint
was enabled in trusted configuration. Native Anthropic/Gemini APIs are not adapters;
use a deliberately configured compatible endpoint if available.

## Writing and integration

Writers receive detached Git worktrees from the parent's committed HEAD. Read-only
agents share the current workspace. Worktree creation requires approval; configured
checkout filters and hooks are disabled for the manager's checkout. Uncommitted
parent work is excluded and this is shown before creating a worktree.

Subagent commands require Linux bubblewrap. The environment is cleared, networking
and process namespaces isolated, host home credentials hidden, and only the worktree
scope plus private temporary directories writable. Runtime directories and optional
Rust toolchain/registry caches are mounted read-only. Root shell stays unrestricted
and always asks. Missing/broken sandbox support never falls back to root shell.

Integration supports scoped UTF-8 additions/modifications/deletions within existing
parent directories, up to 100 files and 2 MiB, with individual 256 KiB limits.
Binary files, symlinks, executable/mode changes and missing parent directories require
manual review. Gitignored outputs are not integration inputs. Each parent file must
still exactly match the worktree base; conflicting parent edits refuse the plan.

The plan is previewed, approved, regenerated to detect changes after approval, and
journaled before applying. Writes are checksum-checked and atomic per file. On failure,
rollback restores only bytes still matching AETHER's writes; concurrent user edits
cause explicit rollback-conflict diagnostics. This is not a transactional filesystem
commit or a race-proof compare-and-swap. Crashes during multi-file application require
inspection of `integration_checkpoint`; no automatic global rollback occurs.

Retained edits can be reviewed after restarting:

```bash
aether --session ROOT_SESSION agents --json
aether agent AGENT_ID
aether --workspace /parent/workspace --mode build integrate AGENT_ID
```

No commits, pushes, resets or merges are performed as part of integration. Worktrees
are retained for inspection; AETHER does not silently delete them. Remove an inspected
worktree with Git yourself when its changes are no longer needed.

## Durable states and completion

SQLite schema 2 adds indexed agent records without replacing schema-1 messages or
events. An idle session lease allows stale nonterminal agents to become Interrupted
after process death. Live sessions in another process are left alone. No tool or
subagent request is replayed automatically.

A normal final response, a tool-only response, a blank response, truncation, content
filtering, provider context exhaustion and broken streams are distinct outcomes.
Blank/whitespace finals get at most `agent.final_response_retries` attempts (default
one, maximum two), within all existing budgets, with tools disabled. Recovery cannot
repeat an edit or test. Exhausted/truncated/interrupted generations fail explicitly.

Completed means a nonempty assistant response was persisted. Structured results
retain tool errors, real command exit codes, usage and `task_success_verified=false`.
Model assertions are not independent validation. Real coding acceptance tests must
check resulting files and tests, not just text.
