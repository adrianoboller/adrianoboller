use anyhow::{Context, Result};
use phxclaw_agent_catalog::AgentCatalog;
use phxclaw_code_workspace::{CodeWorkspace, WorkspacePolicy};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_live_bus::LiveEventHub;
use phxclaw_mission_runtime::{MissionRuntime, MissionSpec};
use std::{env, fs, path::PathBuf};

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        anyhow::bail!("usage: phxclaw-mission-cli <mission.json>");
    }
    let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mission: MissionSpec = serde_json::from_slice(&fs::read(&args[1])?)
        .context("invalid mission JSON")?;

    let agents = AgentCatalog::load_dir(package_root.join("config/agents"))?;
    let live_bus = LiveEventHub::new(512, 512);
    let evidence = EvidenceLedger::open(package_root.join("var/evidence/mission-runtime.jsonl"))?;
    let runtime = MissionRuntime::new(agents, live_bus, evidence);

    let policy = WorkspacePolicy {
        allow_git_mutation: true,
        allow_file_write: true,
        allowed_programs: vec![
            "git".into(), "cargo".into(), "rustc".into(), "python3".into(),
            "node".into(), "npm".into(),
        ],
        command_timeout_ms: 300_000,
        max_output_bytes: 4 * 1024 * 1024,
    };
    let state_root = env::var_os("PHXCLAW_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("phxclaw-state"));
    let workspace = CodeWorkspace::open(
        &mission.project_root,
        state_root,
        policy,
    )?;
    let report = runtime.run(&workspace, &mission)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
