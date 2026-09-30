# v0.45 → v0.53 compatibility repair

The surviving v0.45 overlay installs `crates/phxclaw-active-project-runtime`, while the surviving v0.53 and v0.54 apply scripts require `crates/phxclaw-active-project-scheduler`.

The consolidated backup preserves the v0.45 implementation unchanged and adds a thin Rust facade crate named `phxclaw-active-project-scheduler` that re-exports `phxclaw-active-project-runtime`.

Status: `CONSOLIDATION_REPAIR`, not claimed as a byte-identical historical artifact.
