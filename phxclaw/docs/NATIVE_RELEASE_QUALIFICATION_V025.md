# PhxClaw v0.25 — Native Build & Release Qualification

## Goal

Turn release readiness into an executable, evidence-bound process on a real host.

## Hard rules

1. Every gate is bound to the same canonical source-state SHA-256.
2. A command that was not executed is `unavailable`, never `verified`.
3. Exit code 0 alone is not enough when a gate requires a marker or structured evidence.
4. Native Cargo gates run against the whole workspace and all targets.
5. PostgreSQL gates use `ON_ERROR_STOP=1` and a disposable database supplied by `PHXCLAW_TEST_DATABASE_URL`.
6. RLS is tested with a separate non-superuser/non-BYPASSRLS role; an admin connection is never accepted as RLS proof.
7. PostgreSQL passwords are parsed into `PGPASSWORD` and never placed in the `psql` argv.
8. Mission/Tauri/provider gates are command-injected by explicit environment variables; absent commands remain unavailable.
9. Evidence files are append-only artifacts under a new qualification output directory.
10. Public release requires an Ed25519-signed attestation and cryptographic verification; signature presence alone never authorizes publication.
11. Generated evidence directories are excluded from the canonical source-state hash, so recording proof does not mutate the state being proved.

## Required external commands

- Rust: `cargo`, `rustc`
- PostgreSQL: `psql`
- `PHXCLAW_TEST_DATABASE_URL`: migration/owner connection
- `PHXCLAW_RLS_DATABASE_URL`: separate non-superuser, non-BYPASSRLS connection
- optional supply-chain tooling: `cargo-deny` or `cargo-audit`
- explicit E2E commands through environment variables

## Commands

```bash
python3 tools/qualify_release.py . --output reports/release-qualification/run-001
python3 tools/qualify_release.py . --output reports/release-qualification/run-001 --strict
```

Windows can invoke the same Python entry point from PowerShell.

## Public release signing

Public publication requires an Ed25519 signature over the canonical v0.25 attestation payload. The signer key ID must exist in `config/trusted-release-signers.v025.json`; signer-provided public keys are never trusted implicitly. `PHXCLAW_RELEASE_SIGN_CMD` receives the canonical payload file path and must output JSON with `signer_key_id` and `signature_b64`.

## Signing protocol

`PHXCLAW_RELEASE_SIGN_CMD` receives the canonical signing-payload file path and must emit JSON with `signer_key_id`, `signature_b64`, and `signature_algorithm=ed25519`. `PHXCLAW_RELEASE_VERIFY_CMD` receives the payload path followed by the final attestation path; public release requires exit code 0 from this independent verifier.

The Python qualification host uses `cryptography` for local Ed25519 verification. Install `requirements-release-qualification.txt`.

`PHXCLAW_RELEASE_VERIFY_CMD` may add an external verifier after the built-in trusted-key Ed25519 verification; it cannot replace or bypass the built-in check.
