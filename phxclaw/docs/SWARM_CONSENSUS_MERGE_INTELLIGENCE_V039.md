# PhxClaw v0.39 — Swarm Consensus + Merge Intelligence

The v0.39 layer turns v0.38 team outputs into deterministic, contract-aware merge candidates.

## Merge authority

Evidence beats votes. A numerical majority cannot override failed QA, High/Critical security evidence, unresolved contract conflicts, failed replay, or mutation-gate failure.

## Semantic merge

The engine compares changed symbols and explicit contract surfaces. Overlapping text alone is not treated as enough evidence to auto-resolve a conflict.

## Ordering

Merge order is a deterministic topological sort over explicit intent dependencies. Unknown dependencies or cycles fail closed.

## Replay

Before merge, the integration branch is replayed against the exact base source hash, exact candidate artifact hashes, expected tests and side-effect digest.

## Mutation tests

Policy can require a minimum mutation score and minimum mutant count. Critical surviving mutants always block.

## Signed resolution

Blocking conflicts may require a trusted Ed25519 resolution document. The public key supplied by the document is not automatically trusted; it must match a configured trust-store entry.

## Rollback

v0.39 never replaces v0.38/F17 checkpoints. Commit and rollback remain separate governed effects.
