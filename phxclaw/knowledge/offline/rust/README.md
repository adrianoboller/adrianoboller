# Rust Official Offline Knowledge

This directory is the local knowledge source used by PhxClaw agents with the
`knowledge.rust.read` capability.

Expected layout after synchronization:

```text
knowledge/offline/rust/
├── html/              # copy of rust-docs from the selected toolchain
├── index/
│   └── documents.jsonl
├── snapshot.json      # toolchain/version/hash metadata
└── README.md
```

The authoritative synchronization path is **rustup + rust-docs**. The project
scripts copy the local Rust documentation into this directory and build a small
search index. Research/Documentation agents use this copy first for stable
language/API questions, then use the online official source only when freshness
or release-specific verification is required.

Do not copy arbitrary crates.io documentation into this folder without checking
the individual crate license and provenance.
