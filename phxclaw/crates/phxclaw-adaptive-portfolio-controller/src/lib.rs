//! Compatibility facade created during v0.60 general-source consolidation.
//! v0.53 ships `phxclaw-adaptive-portfolio-execution-controller`; v0.55's
//! surviving apply script checks the shorter historical path.
pub use phxclaw_adaptive_portfolio_execution_controller::*;
