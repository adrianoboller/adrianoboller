# Rust Official Knowledge — online + offline

PhxClaw treats the Rust project documentation as an authoritative knowledge
source named `rust-official`.

## Agents with access

- Research Agent
- Research Lead / Pesquisador PDCA
- Documentador
- Perséfone

They must possess the capability `knowledge.rust.read`.

## Resolution policy

1. Prefer `knowledge/offline/rust/html` for stable language/API questions.
2. Use the official online endpoint when freshness matters (release notes,
   current stable version, recently changed APIs, component availability).
3. Record the selected source, retrieval time and evidence UUIDv7.
4. Never treat crates.io package documentation as Rust-project authority; each
   crate has its own owner, version and license.
5. If online and offline sources disagree, report the version mismatch instead
   of silently choosing one.

## Offline synchronization

Linux/macOS:

```bash
./scripts/sync_rust_offline_docs.sh 1.98.1
```

Windows PowerShell:

```powershell
.\scripts\sync_rust_offline_docs.ps1 -Toolchain 1.98.1
```

The scripts install the `rust-docs` component with rustup, copy the official
local HTML tree into the PhxClaw knowledge directory and generate
`index/documents.jsonl` for local search.

The snapshot is pinned by default to Rust 1.98.1 for reproducibility. It can be
updated deliberately after Research + QA review.
