# OpenCode plugin compatibility

No plugins are loaded or executed in AETHER 0.1.0. Compatibility status is **none**,
not partial or full. No representative third-party plugin integration test has
passed; the compatibility host is a future milestone.

| API family | Observed baseline | Current support | Future gate |
|---|---|---|---|
| Legacy `@opencode-ai/plugin` | Older docs describe hooks and `tool()` | Unsupported | Pin package + real plugins and license review |
| `@opencode/plugin/promise` | Current researched commit has Context domains and MainSetup | Unsupported | Versioned RPC domain subset with lifecycle tests |
| Effect plugins | Current source includes Effect adapter | Unsupported | Explicit adapter, not implicit Promise compatibility |
| TUI/GUI plugin surfaces | Separate entrypoints in researched source | Unsupported | Define terminal-native supported subset |
| Native JS/TS | No host | Unsupported | User trust + subprocess/resource policy |
| Wasm | No Wasmtime host | Unsupported | Capabilities, memory/fuel/timeout and WASI tests |

The proposed host is a separate Node/Bun process using bounded versioned RPC. A
subprocess is **not a security sandbox**. Loading must require an explicit trust
decision; discovery must never execute plugins. Tool/command/event registrations
must pass typed validation, and unsupported domains must fail visibly. Document
filesystem/network permissions and OS sandbox availability per platform. Never
promise arbitrary OpenCode plugins can run inside Rust or emulate unknown APIs.

Plugin development SDK documentation will be published with a real versioned SDK.
There is intentionally no fake SDK or inert plugin-management command today.
