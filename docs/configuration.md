# Configuration and provider guide

Install from source: `cargo build --locked --release`, then run
`./target/release/aether config --init`. Copy the binary to a directory on PATH if
desired; no installation or system service is performed by this repository.

Precedence: built-in defaults → global config → `.aether/config.toml` in the selected
workspace → `AETHER_BASE_URL` / `AETHER_MODEL` → CLI mode and `run --allow-read`.
`--config PATH` replaces both global/project sources and is explicitly trusted.
Nested tables merge; unknown keys are rejected with an error, never discarded.
The current schema has no config version or automatic migration; incompatible
future changes must provide a migration before release.

Project configuration may restrict access, but cannot use `allow` rules, enable
remote endpoints or choose an endpoint or API-key environment variable. Put those grants in
global config, use `--config` for a file you trust, or `run --allow-read` for scoped
reading. Default read/write/shell policy is `ask`; default mode is `ask`.

Provider settings are shown in examples/config.toml. `context_tokens` is your
actual model window; `output_tokens` reserves output, plus a 2048-token heuristic
margin. Counts are estimates. Oversized sessions stop with an actionable error;
fork preserves the same history and therefore does not reduce context size.

OpenAI-compatible examples:

| Server | Typical base URL | Notes |
|---|---|---|
| LM Studio | `http://127.0.0.1:1234/v1` | Start API server; use actual model ID |
| Ollama | `http://127.0.0.1:11434/v1` | Loaded model must support function tools |
| llama.cpp | `http://127.0.0.1:8080/v1` | Tool support depends on chat template |
| vLLM | `http://127.0.0.1:8000/v1` | Enable appropriate tool parser server-side |
| Hosted compatible API | Provider's HTTPS URL ending `/v1` | Trusted `allow_remote=true` required |

This table describes protocol configuration, not verified interoperability with
every server. Only Chat Completions SSE is implemented. `stream_options.include_usage`
is sent; a server rejecting it produces a visible HTTP error, with no silent retry.
The model must return native `tool_calls`; arbitrary text is never executed.
Anthropic Messages, Gemini GenerateContent and OpenAI Responses are not adapters
in this release. A model may not implement sampling/tools even at a compatible URL.

Set keys externally: `export AETHER_API_KEY=...` (avoid shell-history disclosure).
Change `api_key_env` only in trusted config. Keys never belong in base URLs.
AETHER_DATA_DIR overrides the data location; `RUST_LOG` controls tracing to the
local log file. Configuration is only persisted by `config --init`, which refuses
to overwrite existing files. Per-session model switching is not yet supported.

## Multi-agent configuration

See [multi-agent.md](multi-agent.md) and [the complete example](../examples/multi-agent.toml).
`[orchestration]` controls enabled/depth/concurrency/count and task-wide token/step/cost
caps. `[profiles.NAME]` defines independent instructions, model/provider alias, tool
allowlist, permissions, scope and per-agent budgets. `[providers.NAME]` defines a
trusted compatible endpoint. These sections are rejected in implicit repository
configuration; use global config or explicit `--config` to authorize them.

`agent.final_response_retries` defaults to 1 and accepts 0–2. It only recovers blank
final text, with tools disabled; it does not retry shell, edits, truncation or network
failures. Monetary caps require both `provider.input_cost_per_million` and
`provider.output_cost_per_million` for every used provider. All examples are operator
configuration, not a catalog of current prices. Model assignment never creates an
implicit provider fallback.
