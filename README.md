# AETHER

A local-first Rust terminal coding agent. **Early development; not production ready.**
The first implementation includes a working streaming agent, permission-gated
read/write/shell tools, persistent sessions and an asynchronous terminal UI. It is
not yet an OpenCode replacement. No comparative performance claim is made.

## Build and run

Requires Rust stable (tested with 1.99), a C compiler for bundled SQLite, and an
OpenAI-compatible server supporting SSE and structured function tools. Linux is
the currently exercised platform. `sh` is needed for approved shell commands.

```bash
cargo build --locked --release
./target/release/aether --help
./target/release/aether config --init
```

Edit the printed global config path. Set `provider.model` to an actual model ID.
The default endpoint is LM Studio at `http://127.0.0.1:1234/v1`; there is no automatic
model download, server launch or cloud fallback. Copyable config: [examples/config.toml](examples/config.toml).

```bash
./target/release/aether models
./target/release/aether doctor
./target/release/aether
./target/release/aether --mode build
./target/release/aether run --allow-read "Read Cargo.toml and explain the workspace"
./target/release/aether run --json "Explain ownership in Rust"
./target/release/aether sessions --json
./target/release/aether sessions --search "ownership"
./target/release/aether --session SESSION_ID
./target/release/aether fork SESSION_ID "Alternative approach"
./target/release/aether export SESSION_ID > session.json
```

`--workspace PATH`, `--config PATH`, `--mode MODE`, and `--session ID` are global
options. Modes: ask (default), plan, build, debug, review. Ask/plan/review are
strictly read-only; debug shares build's approved tools. Noninteractive and JSON
runs deny approval requests. `--allow-read` explicitly grants scoped reading for
that invocation; it grants neither writes nor shell. Interactive headless runs
ask for individual approvals. Shell always needs a fresh approval and runs with
your account privileges, without an OS sandbox. Errors exit with code 1.

## TUI controls

Enter sends; Alt+Enter inserts a newline. Bracketed multiline paste, editor
undo/redo and word movement are supplied by tui-textarea. Ctrl+S opens sessions,
Ctrl+N creates one, Ctrl+H opens help, Esc cancels, Ctrl+Q exits. PageUp/PageDown
and mouse wheel scroll; End follows output. Permission dialogs use `y` once or
`n`/Esc to deny. Ctrl+Up/Down scrolls long previews. Drafts save after 500ms idle
and on clean exit. The sidebar hides on narrow terminals.

## Data and privacy

Global configuration: `$XDG_CONFIG_HOME/aether/config.toml` (normally
`~/.config/aether/config.toml`). Data: `$XDG_DATA_HOME/aether` (normally
`~/.local/share/aether`); override with `AETHER_DATA_DIR`. The data directory has
0700 permissions on Unix. SQLite stores messages, tool outputs, permission
records, partial streams and pre-write file contents **in plaintext**. Do not
share exports without inspecting them. No telemetry or analytics.

Cloud-compatible endpoints require `provider.allow_remote = true` in trusted
configuration and HTTPS. Keys are read from the variable named by `api_key_env`.
Review [SECURITY.md](SECURITY.md), especially filesystem-race and shell limitations.

## Verification and status

```bash
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release
python3 scripts/benchmark.py
python3 scripts/verify_terminal.py
```

[Architecture](ARCHITECTURE.md) · [Implementation status](docs/IMPLEMENTATION_STATUS.md)
· [Roadmap](docs/roadmap.md) · [Research](docs/competitive-analysis.md)
· [Configuration/providers](docs/configuration.md) · [Troubleshooting](docs/troubleshooting.md)
· [Plugin compatibility](docs/opencode-compatibility.md) · [Benchmarks](docs/benchmarks.md)

Not implemented: Markdown/syntax rendering, virtualized timelines, command palette,
complete context compaction, checkpoint restore, session import, native cloud
provider protocols, plugins, MCP, LSP, parallel agents, full Git UI and Windows
process-tree cancellation. Fork copies events rather than sharing immutable history.
See the roadmap for acceptance gates; there are no fake menu entries for these features.
