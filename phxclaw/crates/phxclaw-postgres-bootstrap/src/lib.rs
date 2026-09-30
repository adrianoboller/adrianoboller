#![forbid(unsafe_code)]

use phxclaw_types::new_uuid_v7;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_STABLE_MAJOR: u16 = 18;
pub const DEFAULT_STABLE_VERSION: &str = "18.6";
pub const DEFAULT_PORT: u16 = 55432;
pub const DEFAULT_DATABASE: &str = "phxclaw";
pub const DEFAULT_APP_ROLE: &str = "phxclaw_app";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostPlatform {
    WindowsX64,
    DebianLike,
    RedHatLike,
    MacOs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgreSqlBootstrapConfig {
    pub uuid: Uuid,
    pub stable_major: u16,
    pub stable_version: String,
    pub port: u16,
    pub database: String,
    pub app_role: String,
    pub listen_address: String,
    pub managed: bool,
}

impl Default for PostgreSqlBootstrapConfig {
    fn default() -> Self {
        Self {
            uuid: new_uuid_v7(),
            stable_major: DEFAULT_STABLE_MAJOR,
            stable_version: DEFAULT_STABLE_VERSION.into(),
            port: DEFAULT_PORT,
            database: DEFAULT_DATABASE.into(),
            app_role: DEFAULT_APP_ROLE.into(),
            listen_address: "127.0.0.1".into(),
            managed: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallStep {
    pub id: String,
    pub description: String,
    pub command: Vec<String>,
    pub requires_admin: bool,
    pub destructive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgreSqlInstallPlan {
    pub uuid: Uuid,
    pub platform: HostPlatform,
    pub version: String,
    pub port: u16,
    pub steps: Vec<InstallStep>,
    pub rollback: Vec<String>,
}

pub fn build_install_plan(
    platform: HostPlatform,
    cfg: &PostgreSqlBootstrapConfig,
) -> Result<PostgreSqlInstallPlan, BootstrapError> {
    if cfg.port == 0 {
        return Err(BootstrapError::InvalidPort);
    }
    let mut steps = Vec::new();
    match platform {
        HostPlatform::WindowsX64 => {
            steps.push(InstallStep {
                id: "download".into(),
                description: "Download the pinned EDB PostgreSQL binary ZIP and verify SHA-256".into(),
                command: vec!["phoenix-installer".into(), "postgres-download-windows".into()],
                requires_admin: false,
                destructive: false,
            });
            steps.push(InstallStep {
                id: "initdb".into(),
                description: "Initialize a private SCRAM-authenticated cluster under var/runtime/postgresql".into(),
                command: vec!["initdb".into(), "--auth=scram-sha-256".into()],
                requires_admin: false,
                destructive: false,
            });
        }
        HostPlatform::DebianLike => steps.push(InstallStep {
            id: "pgdg-apt".into(),
            description: "Install PostgreSQL from the official PGDG APT repository".into(),
            command: vec!["apt-get".into(), "install".into(), format!("postgresql-{}", cfg.stable_major)],
            requires_admin: true,
            destructive: false,
        }),
        HostPlatform::RedHatLike => steps.push(InstallStep {
            id: "pgdg-rpm".into(),
            description: "Install PostgreSQL from the official PGDG RPM repository".into(),
            command: vec!["dnf".into(), "install".into(), format!("postgresql{}-server", cfg.stable_major)],
            requires_admin: true,
            destructive: false,
        }),
        HostPlatform::MacOs => steps.push(InstallStep {
            id: "brew".into(),
            description: "Install PostgreSQL with Homebrew".into(),
            command: vec!["brew".into(), "install".into(), format!("postgresql@{}", cfg.stable_major)],
            requires_admin: false,
            destructive: false,
        }),
    }
    steps.extend([
        InstallStep {
            id: "configure".into(),
            description: "Bind PostgreSQL to loopback only and enable SCRAM authentication".into(),
            command: vec!["phoenix-installer".into(), "postgres-configure".into()],
            requires_admin: false,
            destructive: false,
        },
        InstallStep {
            id: "roles".into(),
            description: "Create PhxClaw database and least-privilege application role".into(),
            command: vec!["phoenix-installer".into(), "postgres-provision".into()],
            requires_admin: false,
            destructive: false,
        },
        InstallStep {
            id: "migrate".into(),
            description: "Apply ordered PhxClaw SQL migrations transactionally".into(),
            command: vec!["phx".into(), "db".into(), "migrate".into()],
            requires_admin: false,
            destructive: false,
        },
    ]);
    Ok(PostgreSqlInstallPlan {
        uuid: new_uuid_v7(),
        platform,
        version: cfg.stable_version.clone(),
        port: cfg.port,
        steps,
        rollback: vec![
            "stop managed PostgreSQL".into(),
            "restore pre-install config snapshot".into(),
            "remove only PhxClaw-managed runtime directory when explicitly approved".into(),
        ],
    })
}

pub fn bootstrap_sql(database: &str, app_role: &str) -> Result<String, BootstrapError> {
    validate_identifier(database)?;
    validate_identifier(app_role)?;
    Ok(format!(
        "CREATE DATABASE {database};\nCREATE ROLE {app_role} LOGIN;\nGRANT CONNECT ON DATABASE {database} TO {app_role};\n"
    ))
}

fn validate_identifier(value: &str) -> Result<(), BootstrapError> {
    if value.is_empty()
        || value.len() > 63
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        || !value.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return Err(BootstrapError::InvalidIdentifier(value.into()));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("invalid PostgreSQL port")]
    InvalidPort,
    #[error("invalid SQL identifier: {0}")]
    InvalidIdentifier(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_non_destructive_by_default() {
        let plan = build_install_plan(HostPlatform::WindowsX64, &Default::default()).unwrap();
        assert!(plan.steps.iter().all(|s| !s.destructive));
        assert!(!plan.rollback.is_empty());
    }

    #[test]
    fn unsafe_identifier_is_rejected() {
        assert!(bootstrap_sql("phxclaw;drop", "app").is_err());
    }
}
