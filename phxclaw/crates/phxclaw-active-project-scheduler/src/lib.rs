//! Compatibility facade created during v0.60 general-source consolidation.
//! v0.45 shipped the implementation as `phxclaw-active-project-runtime`, while
//! v0.53/v0.54 require the historical name `phxclaw-active-project-scheduler`.
pub use phxclaw_active_project_runtime::*;
