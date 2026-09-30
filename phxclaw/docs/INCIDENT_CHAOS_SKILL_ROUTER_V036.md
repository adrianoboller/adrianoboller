# PhxClaw v0.36 — Incident Commander + Chaos/Resilience Lab + Skill Router

## Skill routing

When a task arrives:

```text
Task
  -> classify intent/phase
  -> license + provenance gate
  -> capability + permission gate
  -> local knowledge first when applicable
  -> route one or more skills
  -> execute through Extension Host/sandbox
  -> validate result
  -> Evidence Ledger
  -> next phase
```

Default engineering chain:

```text
Research & Validate
  -> Understand & Improve
  -> Create & Explain
  -> Refine & Deliver
```

A practical software-change chain is:

```text
research problem -> map code/system -> propose change -> review/validate -> deliver
```

The router never grants permissions that the selected skill did not declare and never grants permissions absent from the task policy.

## Incident Commander

Roles are logical capabilities, not unrestricted agents: Commander, Investigator, Scribe, Mitigator, Verifier and Communicator. High/Critical side effects require approval and v0.30 leader fencing according to signed incident policy. Every timeline/action/recovery fact is evidence-linked.

## Chaos / Resilience Lab

Sandbox/test/staging are allowed by policy. Production chaos is disabled by default and requires a signed policy, human approval, bounded blast radius, mandatory abort conditions, recovery runbook, v0.30 leader/fencing and post-experiment steady-state verification.

Forbidden by contract: destructive data experiments, credential exfiltration, arbitrary shell and unbounded partitions.

Chaos experiments do not prove production resilience until executed on an authorized native environment with actual telemetry and recovery evidence.
