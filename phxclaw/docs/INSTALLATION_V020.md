# PhxClaw v0.20 — Installation

## One-command bootstrap

Windows PowerShell:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\installer\install.ps1
```

Linux/macOS:

```bash
./installer/install.sh
```

Or inspect before changing the host:

```bash
./bin/phx db plan
./bin/phx db doctor
```

## PostgreSQL policy

Production defaults to PostgreSQL 18.6. PostgreSQL 19 remains preview/beta and is not enabled for production automatically.

### Windows

PhxClaw downloads the pinned EDB PostgreSQL 18.6 binary ZIP into `var/cache/postgresql`, verifies SHA-256, extracts it under `var/runtime/postgresql`, creates a private cluster with `initdb`, binds it only to loopback on port 55432, enables SCRAM-SHA-256, provisions PhxClaw roles/database, stores the application password through the PhxClaw Secret Broker, then applies migrations.

No PostgreSQL superuser password is passed in command-line arguments: `initdb --pwfile` and `PGPASSWORD` environment isolation are used.

### Debian/Ubuntu

The installer uses the official PostgreSQL PGDG repository bootstrap and installs `postgresql-18` / `postgresql-client-18`. System-cluster provisioning is separated from package installation so distro ownership/service rules are not guessed.

### macOS

The installer uses Homebrew `postgresql@18` when an existing supported PostgreSQL installation is not found.

### Existing PostgreSQL

If a supported server/client already exists, PhxClaw prefers it and does not download another copy.

## Runtime configuration

`config/runtime.local.json` stores only connection metadata plus a `password_secret_uuid`. Plaintext database passwords are not written to runtime config.

## Core usage

Before native compilation:

```bash
./bin/phx status
./bin/phx core status
./bin/phx sprints list
```

After release build:

```bash
cargo build --release
./bin/phx core start
```

The bootstrap CLI hands off to `target/release/phxclaw` when the native binary exists.
