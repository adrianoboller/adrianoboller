# Installer Recovery v0.65

## Filesystem backup

- content-addressed SHA-256 blob store;
- deterministic manifest;
- configurable file/total size ceilings;
- symlink rejection by default;
- full hash verification before restore;
- restore into staging directory;
- existing target renamed to rollback path before commit;
- rollback copy retained after successful restore.

## PostgreSQL

`pg_dump --format=custom --no-password` and `pg_restore --single-transaction --exit-on-error` are invoked without a shell. Connection data is passed through PG* environment variables and credentials through `PGPASSFILE` only.

## Upgrade

Every upgrade starts with a verified pre-upgrade backup. `apply` then `verify`; either failure triggers `rollback`. Production release still requires native PostgreSQL restore and fresh-install/upgrade/rollback/restore E2E on a Rust/PostgreSQL host.
