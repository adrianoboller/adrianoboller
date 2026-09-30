# PhxClaw v0.38 — Autonomous Engineering Swarm

## Teams
Research, Architecture, Coding, QA, Security, Documentation.

## Parallelism model
Each team receives an isolated Git worktree/branch and a lease/fencing token. Research begins first. Architecture consumes Research. Coding consumes Architecture. QA and Security run in parallel after Coding. Documentation may begin from Architecture and must be refreshed for public/behavioral changes after final integration.

## Merge authority
There is no majority vote that can override evidence. A merge candidate is blocked by any unresolved conflict, failed QA, High/Critical security finding, missing architecture evidence for architecture changes, or missing documentation evidence for public/behavioral changes. Main/production still requires human approval.

## Conflict model
Overlapping writes, source drift, contradictory evidence, architecture disagreements, QA failures and security findings become explicit conflict records. Resolution is append-only and requires an independent resolver for critical conflicts.

## Rollback
Mutating teams need checkpoints. A failed mutation is rolled back to the team checkpoint before retry; rollback must be verified and stale fencing tokens are rejected.
