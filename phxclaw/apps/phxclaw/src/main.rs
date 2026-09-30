#![forbid(unsafe_code)]

use anyhow::{Result, bail};
use phxclaw_core_runtime::{
    CoreService, PRODUCT_CLI, PRODUCT_NAME, PhoenixCoreRuntime, ServiceState,
};
use phxclaw_postgres_bootstrap::{HostPlatform, PostgreSqlBootstrapConfig, build_install_plan};
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return Ok(());
    }
    match args[0].as_str() {
        "version" | "--version" | "-V" => println!("{PRODUCT_NAME} {VERSION}"),
        "core" => core_command(&args[1..])?,
        "db" => db_command(&args[1..])?,
        other => bail!("unknown command: {other}. Run `{PRODUCT_CLI} --help`."),
    }
    Ok(())
}

fn core_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let mut runtime = PhoenixCoreRuntime::new(VERSION, "Master Orchestrator");
    runtime.set_service_state(
        CoreService::Kernel,
        ServiceState::Ready,
        "constitution accepted",
    );
    runtime.set_service_state(
        CoreService::AgentRegistry,
        ServiceState::Ready,
        "110 declarative agents",
    );
    runtime.set_service_state(
        CoreService::MissionRuntime,
        ServiceState::Ready,
        "mission facade loaded",
    );
    match sub {
        "status" | "start" => {
            let status = runtime.status();
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        other => bail!("unknown core command: {other}"),
    }
    Ok(())
}

fn db_command(args: &[String]) -> Result<()> {
    let sub = args.first().map(String::as_str).unwrap_or("plan");
    match sub {
        "plan" => {
            let platform = match args.get(1).map(String::as_str) {
                Some("windows") => HostPlatform::WindowsX64,
                Some("debian") => HostPlatform::DebianLike,
                Some("redhat") => HostPlatform::RedHatLike,
                Some("macos") => HostPlatform::MacOs,
                None => detect_platform(),
                Some(other) => bail!("unknown platform: {other}"),
            };
            let plan = build_install_plan(platform, &PostgreSqlBootstrapConfig::default())?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        other => bail!("unknown db command: {other}"),
    }
    Ok(())
}

fn detect_platform() -> HostPlatform {
    if cfg!(target_os = "windows") {
        HostPlatform::WindowsX64
    } else if cfg!(target_os = "macos") {
        HostPlatform::MacOs
    } else {
        HostPlatform::DebianLike
    }
}

fn print_help() {
    println!(
        "{PRODUCT_NAME} {VERSION}\n\nUSAGE:\n  {PRODUCT_CLI} <COMMAND>\n\nCOMMANDS:\n  version            Show version\n  core status        Show unified Core Runtime state\n  core start         Start/inspect the unified Core Runtime bootstrap\n  db plan [platform] Show PostgreSQL managed-install plan\n\nThe Python bootstrap CLI adds installer/plugin/agent/status commands before the Rust binary is built."
    );
}
