# PhxClaw v0.37 — Autonomous Engineering Workflow

The Mission Runtime now has a governed engineering pipeline:

`Intake -> Research -> Code Map -> Hypothesis -> Plan -> Implement -> Test -> Review -> Explain -> Deliver`

## Authority boundaries

- v0.36 Skill Router selects optional external skills; it does not own workspace mutation.
- Research Core owns evidence gathering and source freshness.
- Hypothesis Core owns falsifiable hypotheses.
- Mission Runtime owns typed workspace mutation in an isolated Git worktree.
- F17 Checkpoint/Rollback protects mutation stages.
- Evidence Ledger/F25 record lineage and proof.
- Medium+ implementation, production/destructive actions and any external publish require approval.
- No external skill may execute arbitrary shell through this workflow.

## Practical system-change flow

1. QMD/local search first when useful; then Agent Reach/Last 30 Days/Deep Research/User Research when network is allowed.
2. Graphify/Understand Anything map the code; findings remain evidence-linked.
3. Hypothesis Core creates a testable hypothesis; Tech Debt/Ponytail/Napkin may add observations.
4. Plan is hashed with risk and rollback strategy.
5. A checkpoint is recorded before mutation; Mission Runtime writes only inside the isolated worktree.
6. Deterministic tests run before independent review.
7. Explain/visual skills may generate user-facing artifacts without changing engineering facts.
8. Humanizer/SEO/video/FFmpeg may refine deliverables. Publish/schedule remains a separately approved side effect.

Failures in mutation trigger rollback before retry when the checkpoint is verifiable. If rollback cannot be proven, the workflow stops.
