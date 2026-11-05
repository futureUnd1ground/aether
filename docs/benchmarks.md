# Benchmark methodology

`scripts/benchmark.py` measures sequential warm process startup, CLI help and exit
using `perf_counter_ns`, with raw samples and median/p95. It does not initialize
the TUI, database or provider and must not be described as interactive cold-start
latency. Run `cargo build --locked --release` first. Machine and OS are recorded.
There is no measured OpenCode comparison, idle CPU/memory or agent-quality score.

Future comparison protocol: record exact AETHER/OpenCode commits, CPU/RAM/GPU,
terminal emulator, OS, power profile, repository and model/server configuration.
Use the same endpoints, deterministic fixtures, warm/cold cache conditions and
sample counts. Measure startup-to-first-frame separately from startup/help. Capture
PTY input timestamps versus first changed frame during streaming and shell work.
Measure resident memory and CPU over an idle interval; compare identical large
histories and FTS queries. Separate provider inference and transport time from
application overhead. Coding tasks pass only by resulting file/test assertions.
Report failed tasks and denominator, token use, cost only when actual pricing is
configured, and raw measurements. No claimed improvement until data supports it.
