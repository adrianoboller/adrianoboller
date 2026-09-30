# ADR-0065 — Installer Backup / Restore / Disaster Recovery

F03 must never mutate an installation without a verified rollback path. v0.65 replaces the plan-only installer with a content-addressed backup repository, SHA-256 manifests, verify-before-restore, staging restore with rollback preservation, transactional upgrade hooks, explicit PostgreSQL `pg_dump`/`pg_restore` execution and disaster-recovery evidence tables.

External database tools are invoked directly (no shell), with exact executable names and `PGPASSFILE`; passwords are never placed in argv. A failed or unverified backup cannot be restored. Upgrade apply/verify failures require rollback before the run can be considered handled.
