# Contributing

Read ARCHITECTURE.md and docs/IMPLEMENTATION_STATUS.md first. Keep crate boundaries
acyclic. Implement vertical slices with observable behavior and meaningful tests.
Avoid placeholder tools, inert UI entries, implicit network routing and permission
bypasses. Preserve user changes; never use global Git reset for checkpoints.

Run `cargo fmt --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo test --locked --workspace`, and `cargo build --locked --release`.
CI also runs cargo-deny and RustSec auditing; a workflow definition is not proof of
a successful remote CI run. Keep Cargo.lock committed for the application.

Provider fixtures are test-only, never a runtime fallback. Real inference evidence
must be recorded separately from protocol tests. Benchmark claims need raw samples,
hardware, application versions and the exact methodology. Security limitations
must remain explicit until regression tests prove the boundary.
