# ADR-0060 — Source Provenance & License Firewall

Status: **ACCEPTED FOR OVERLAY / NATIVE GATES PENDING**  
Version: 0.60.0  
Created: 2026-09-30T01:05:40.502013+00:00

## Context

PhxClaw ingests external repositories, source snapshots, plugins, skills and research evidence. A source bundle may mix permissively licensed projects, unidentified code and proprietary material. Importing them without a deterministic gate contaminates the codebase and breaks provenance.

## Decision

Add a fail-closed boundary before any external source can be copied, linked, compiled, indexed as reusable code, or used as a code-generation donor. Every source must have:

- SHA-256 of the exact artifact;
- known origin or explicit user-supplied classification;
- local license evidence;
- license class/SPDX when known;
- decision `ALLOW`, `QUARANTINE`, or `DENY`;
- reason codes and evidence records.

`DENY` wins over every other rule. `QUARANTINE` cannot feed code generation or build inputs.

## Current source-pack decisions

- Architect: ALLOW (MIT evidence present).
- Oh My ClaudeCode: ALLOW (MIT evidence present).
- Claw Code supplied snapshot: QUARANTINE until the exact snapshot is paired with/pinned to license evidence, despite current upstream reporting MIT.
- `claude-code-main.zip`: DENY because its own supplied license notice identifies it as proprietary leaked source and not for redistribution.
- `src.zip`: QUARANTINE; origin/license unknown.
- Phoenix COBOL absorber snapshot: QUARANTINE until ownership/license attestation is recorded.

## Consequences

- Third-party ideas may be studied only from sources whose provenance policy allows it.
- Denied source trees are never copied into overlays or runtime packages.
- A later replacement of a quarantined source requires a new hash/evidence record; history remains append-only.
- Release qualification must fail if a compiled/runtime dependency is not `ALLOW`.
