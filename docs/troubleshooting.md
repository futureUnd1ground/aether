# Troubleshooting

- **Connection failed:** start your model's compatible API server. Use `aether
  models`, put an advertised ID in config, then run `aether doctor`. AETHER does not
  start or download models automatically.
- **HTTP 400:** check loaded model's structured-tool support and chat template;
  compatible server may not support `stream_options` or tools. No fallback executes
  plain model text as a command. Provider error bodies are not logged to avoid
  credential or code leakage.
- **Remote access rejected:** use HTTPS and authorize `allow_remote` in trusted
  global config or explicit `--config`; review what source may be transmitted.
- **Read denied in JSON/pipe mode:** use explicit `run --allow-read` for scoped reads.
  No interactive approval is assumed when stdin is redirected.
- **Write denied:** use `--mode build` or `--mode debug`, review the proposed file
  and approve. Existing files require a fresh read SHA-256. Parent directories must
  already exist; symlinks and files over 256 KiB are refused.
- **Session workspace/model mismatch:** reopen from the recorded workspace and
  model. Export or fork without modifying Git state. Fork does not shorten context.
- **Context exhausted:** export the session, start a new one, and explicitly supply
  a concise task summary. Automatic summarization is not implemented.
- **Shell hangs:** 30s deadline bounds the tool; Esc cancels in TUI. Unix process
  groups are killed; detached processes that leave the group need OS isolation.
- **Unexpected termination:** reopen with `--session ID`. Committed events remain;
  incomplete tool calls become unknown-outcome errors, never automatically replayed.
  Partial streams and original file snapshots are available in `export`.
- **Damaged terminal after a host crash:** `reset` usually restores terminal state.
  Normal exit/error uses RAII restoration. SIGKILL cannot run cleanup code.

Logs: data directory's `aether.log`. Session export includes source and tool output;
review it before sharing. Never attach credential environment variables.
