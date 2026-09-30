# PhxClaw v0.58 — Project Trace Tree & Decision Index

Adds a hierarchical PostgreSQL project-control tree over specialized PhxClaw domain tables.

## Core idea

`Project → Domain/Module → Sprint/Task → Run → Source/Artifact/Evidence → Knowledge/Decision → descendants`

The hierarchy answers **where is this?**. Typed cross-links answer **why does this exist / what supports it / what caused it / what superseded it?**.

Specialized tables such as Experience Ledger, Fruitful/Unfruitful Knowledge, Evolution, Execution Ledger and Self-Improvement remain authoritative. `phx_project_trace_bindings` links them into one navigable index.

## Fast lookup
- btree parent/object/source-state indexes
- GIN `uuid[]` materialized path for subtree lookup
- GIN full-text search
- GIN JSONB metadata search
- direct SHA-256 and Git commit lookup

## Safety
- UUIDv7 identities
- append-only trace records
- PostgreSQL RLS + FORCE RLS
- project/tenant composite keys
- source SHA-256 + provenance
- raw secret fields forbidden in trace metadata

## Status
Source/static overlay. Native PostgreSQL E2E remains required before release.
