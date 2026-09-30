# Historical Backfill v0.59

The backfill is an indexing/migration layer into the v0.58 Project Trace Tree. It never deletes or rewrites source domain records.

Priority: direct project rows first, then reference-resolved rows, then portfolio/tenant scopes. Orphans are explicit and retryable.
