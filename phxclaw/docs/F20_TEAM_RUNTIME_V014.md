# F20 Team Runtime — v0.14

State: **review**.

Contracts implemented in source:
- team session and task models;
- dependency-aware ready queue;
- max parallelism;
- worker lease;
- heartbeat;
- monotonically increasing fencing token;
- stale worker rejection;
- cancellation;
- expired lease recovery;
- Live Event Bus hooks;
- Evidence Ledger hooks;
- Mission Runtime TeamExecutor injection.

Operational bootstrap: `phx team demo`.

Not yet release-ready until `cargo check/test/clippy`, PostgreSQL lease concurrency, process crash recovery and Mission E2E execute on a Rust-capable host.
