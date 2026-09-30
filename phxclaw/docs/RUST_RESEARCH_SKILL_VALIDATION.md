# Rust Research Skill — Validation Evidence

This document validates the declarative procedure `rust.research.official@1.0.0`.

Validated properties:

1. routes only to an agent that declares `research.collect`;
2. requires `knowledge.rust.read`, `knowledge.source_registry.read`, `context.compile` and `skill.read`;
3. resolves the authoritative `rust-official` source through the Source Registry;
4. prefers the local rust-docs index for stable knowledge when a synchronized snapshot exists;
5. uses official online endpoints only when the local snapshot is unavailable or freshness requires it;
6. compiles a bounded context pack instead of injecting all memory;
7. preserves task/correlation UUIDv7 values through Event Bus stages;
8. records direct source evidence and Evidence Ledger records;
9. does not promote learned skills automatically;
10. production policy may require `promoted`; development may allow `validated` for integration testing.

This is structural/design validation for the procedure. Native Rust `cargo test` remains a separate build gate.
