# v0.60 integration notes

1. Copy `crates/phxclaw-provenance-core` into the cumulative PhxClaw workspace.
2. Add the crate to the root Cargo workspace members.
3. Assign the next canonical numeric PostgreSQL migration number to `sql/v060_source_provenance.sql`.
4. Wire the provenance gate before these boundaries:
   - plugin install/update;
   - source/vendor import;
   - research-code harvesting;
   - skill/agent package import;
   - build/runtime dependency promotion.
5. Reject `DENY`; isolate `QUARANTINE`; allow only `ALLOW`.
6. Record the decision in Evidence Ledger and Event Bus.
7. Native release gates remain blocked until Cargo and PostgreSQL tests run.

## Required events

- `source.provenance.registered`
- `source.license.evaluated`
- `source.ingestion.allowed`
- `source.ingestion.quarantined`
- `source.ingestion.denied`

## Required capabilities

- `source.provenance.read`
- `source.provenance.write`
- `source.quarantine.read`
- `source.quarantine.release` (human approval + new evidence)
