#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use phxclaw_evidence_ledger::{EvidenceDraft, EvidenceLedger, EvidenceOutcome};
use phxclaw_types::{InstallAction, InstallPlan, new_uuid_v7};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};
use thiserror::Error;
use uuid::Uuid;
use walkdir::WalkDir;

const BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum InstallerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("walk error: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("source root does not exist or is not a directory: {0}")]
    InvalidSource(String),
    #[error("unsafe relative path: {0}")]
    UnsafePath(String),
    #[error("symbolic link rejected by backup policy: {0}")]
    SymlinkRejected(String),
    #[error("file exceeds backup policy limit: {path} ({bytes} bytes)")]
    FileTooLarge { path: String, bytes: u64 },
    #[error("backup exceeds total byte limit")]
    BackupTooLarge,
    #[error("backup manifest not found: {0}")]
    ManifestNotFound(Uuid),
    #[error("backup verification failed: {0}")]
    VerificationFailed(String),
    #[error("restore target has no parent: {0}")]
    InvalidTarget(String),
    #[error("external program rejected: {0}")]
    UnsafeProgram(String),
    #[error("external program failed: {program} ({status})")]
    ProcessFailed { program: String, status: String },
    #[error("upgrade apply failed: {0}")]
    UpgradeApply(String),
    #[error("upgrade verification failed: {0}")]
    UpgradeVerify(String),
    #[error("upgrade rollback failed: {0}")]
    UpgradeRollback(String),
    #[error("evidence ledger failed: {0}")]
    Evidence(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPolicy {
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub reject_symlinks: bool,
    pub excluded_components: BTreeSet<String>,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self {
            max_file_bytes: 8 * 1024 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024 * 1024,
            reject_symlinks: true,
            excluded_components: [".git", "target", "node_modules", ".cache"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u16,
    pub backup_uuid: Uuid,
    pub source_root: String,
    pub source_version: String,
    pub created_at: DateTime<Utc>,
    pub entries: Vec<BackupEntry>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReceipt {
    pub backup_uuid: Uuid,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub deduplicated_blobs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBackupReport {
    pub backup_uuid: Uuid,
    pub valid: bool,
    pub checked_files: usize,
    pub missing_blobs: Vec<String>,
    pub hash_mismatches: Vec<String>,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReceipt {
    pub restore_uuid: Uuid,
    pub backup_uuid: Uuid,
    pub target_path: String,
    pub rollback_path: Option<String>,
    pub restored_files: usize,
    pub verified: bool,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProcessReceipt {
    pub action_uuid: Uuid,
    pub program: String,
    pub status_code: i32,
    pub artifact_path: String,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConnectionSpec {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub pgpassfile: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresToolchain {
    pub pg_dump: PathBuf,
    pub pg_restore: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeReceipt {
    pub run_uuid: Uuid,
    pub from_version: String,
    pub to_version: String,
    pub pre_upgrade_backup_uuid: Uuid,
    pub apply_succeeded: bool,
    pub verify_succeeded: bool,
    pub rollback_attempted: bool,
    pub rollback_succeeded: bool,
    pub completed_at: DateTime<Utc>,
}

pub trait UpgradeExecutor {
    fn apply(&mut self) -> Result<(), String>;
    fn verify(&mut self) -> Result<(), String>;
    fn rollback(&mut self) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct BackupRepository {
    root: PathBuf,
}

impl BackupRepository {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, InstallerError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("manifests"))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create_backup(
        &self,
        source: impl AsRef<Path>,
        source_version: impl Into<String>,
        policy: &BackupPolicy,
        ledger: Option<&EvidenceLedger>,
    ) -> Result<BackupReceipt, InstallerError> {
        let source = source.as_ref();
        if !source.is_dir() {
            return Err(InstallerError::InvalidSource(source.display().to_string()));
        }
        let backup_uuid = new_uuid_v7();
        let action_uuid = new_uuid_v7();
        evidence(
            ledger,
            action_uuid,
            "installer.backup",
            "backup.create",
            EvidenceOutcome::Requested,
            json!({"source":source.display().to_string()}),
            json!({}),
            vec![],
        )?;
        let mut entries = Vec::new();
        let mut total_bytes = 0u64;
        let mut deduplicated_blobs = 0usize;
        for item in WalkDir::new(source).follow_links(false).sort_by_file_name() {
            let item = item?;
            let path = item.path();
            if path == source {
                continue;
            }
            let rel = path
                .strip_prefix(source)
                .map_err(|_| InstallerError::UnsafePath(path.display().to_string()))?;
            if excluded(rel, &policy.excluded_components) {
                continue;
            }
            if item.file_type().is_symlink() {
                if policy.reject_symlinks {
                    return Err(InstallerError::SymlinkRejected(rel.display().to_string()));
                }
                continue;
            }
            if !item.file_type().is_file() {
                continue;
            }
            validate_relative(rel)?;
            let meta = item.metadata()?;
            if meta.len() > policy.max_file_bytes {
                return Err(InstallerError::FileTooLarge {
                    path: rel.display().to_string(),
                    bytes: meta.len(),
                });
            }
            total_bytes = total_bytes
                .checked_add(meta.len())
                .ok_or(InstallerError::BackupTooLarge)?;
            if total_bytes > policy.max_total_bytes {
                return Err(InstallerError::BackupTooLarge);
            }
            let digest = sha256_file(path)?;
            let blob = self.blob_path(&digest);
            if blob.exists() {
                deduplicated_blobs += 1;
            } else {
                if let Some(parent) = blob.parent() {
                    fs::create_dir_all(parent)?;
                }
                let tmp = blob.with_extension(format!("tmp-{}", backup_uuid));
                copy_file_synced(path, &tmp)?;
                if sha256_file(&tmp)? != digest {
                    let _ = fs::remove_file(&tmp);
                    return Err(InstallerError::VerificationFailed(format!(
                        "blob changed while copying: {}",
                        rel.display()
                    )));
                }
                match fs::rename(&tmp, &blob) {
                    Ok(_) => {}
                    Err(e) if blob.exists() => {
                        let _ = fs::remove_file(&tmp);
                        let _ = e;
                    }
                    Err(e) => return Err(InstallerError::Io(e)),
                }
            }
            entries.push(BackupEntry {
                path: rel_to_string(rel)?,
                sha256: digest,
                size_bytes: meta.len(),
                readonly: meta.permissions().readonly(),
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = BackupManifest {
            schema_version: 1,
            backup_uuid,
            source_root: source.display().to_string(),
            source_version: source_version.into(),
            created_at: Utc::now(),
            entries,
            total_bytes,
        };
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        let manifest_sha256 = sha256_bytes(&bytes);
        let manifest_path = self.manifest_path(backup_uuid);
        atomic_write(&manifest_path, &bytes)?;
        let report = self.verify_backup(backup_uuid)?;
        if !report.valid {
            return Err(InstallerError::VerificationFailed(format!(
                "backup {} did not verify",
                backup_uuid
            )));
        }
        let receipt = BackupReceipt {
            backup_uuid,
            manifest_path: manifest_path.display().to_string(),
            manifest_sha256: manifest_sha256.clone(),
            file_count: manifest.entries.len(),
            total_bytes,
            deduplicated_blobs,
        };
        evidence(
            ledger,
            action_uuid,
            "installer.backup",
            "backup.create",
            EvidenceOutcome::Succeeded,
            json!({"backup_uuid":backup_uuid}),
            serde_json::to_value(&receipt)?,
            vec![receipt.manifest_path.clone()],
        )?;
        Ok(receipt)
    }

    pub fn verify_backup(&self, backup_uuid: Uuid) -> Result<VerifyBackupReport, InstallerError> {
        let manifest_path = self.manifest_path(backup_uuid);
        if !manifest_path.is_file() {
            return Err(InstallerError::ManifestNotFound(backup_uuid));
        }
        let bytes = fs::read(&manifest_path)?;
        let manifest_sha256 = sha256_bytes(&bytes);
        let manifest: BackupManifest = serde_json::from_slice(&bytes)?;
        if manifest.backup_uuid != backup_uuid {
            return Err(InstallerError::VerificationFailed(
                "manifest UUID mismatch".into(),
            ));
        }
        let mut missing_blobs = Vec::new();
        let mut hash_mismatches = Vec::new();
        for entry in &manifest.entries {
            validate_relative(Path::new(&entry.path))?;
            let blob = self.blob_path(&entry.sha256);
            if !blob.is_file() {
                missing_blobs.push(entry.path.clone());
                continue;
            }
            let meta = fs::metadata(&blob)?;
            if meta.len() != entry.size_bytes || sha256_file(&blob)? != entry.sha256 {
                hash_mismatches.push(entry.path.clone());
            }
        }
        Ok(VerifyBackupReport {
            backup_uuid,
            valid: missing_blobs.is_empty() && hash_mismatches.is_empty(),
            checked_files: manifest.entries.len(),
            missing_blobs,
            hash_mismatches,
            manifest_sha256,
        })
    }

    pub fn restore_backup(
        &self,
        backup_uuid: Uuid,
        target: impl AsRef<Path>,
        ledger: Option<&EvidenceLedger>,
    ) -> Result<RestoreReceipt, InstallerError> {
        let target = target.as_ref();
        let parent = target
            .parent()
            .ok_or_else(|| InstallerError::InvalidTarget(target.display().to_string()))?;
        fs::create_dir_all(parent)?;
        let verify = self.verify_backup(backup_uuid)?;
        if !verify.valid {
            return Err(InstallerError::VerificationFailed(
                "source backup invalid".into(),
            ));
        }
        let restore_uuid = new_uuid_v7();
        let action_uuid = new_uuid_v7();
        evidence(
            ledger,
            action_uuid,
            "installer.restore",
            "restore.begin",
            EvidenceOutcome::Requested,
            json!({"backup_uuid":backup_uuid,"target":target.display().to_string()}),
            json!({}),
            vec![],
        )?;
        let manifest: BackupManifest =
            serde_json::from_slice(&fs::read(self.manifest_path(backup_uuid))?)?;
        let staging = parent.join(format!(".phx-restore-{}", restore_uuid));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        let build_result = (|| -> Result<(), InstallerError> {
            for entry in &manifest.entries {
                let rel = Path::new(&entry.path);
                validate_relative(rel)?;
                let out = staging.join(rel);
                if let Some(p) = out.parent() {
                    fs::create_dir_all(p)?;
                }
                copy_file_synced(&self.blob_path(&entry.sha256), &out)?;
                let mut perms = fs::metadata(&out)?.permissions();
                perms.set_readonly(entry.readonly);
                fs::set_permissions(&out, perms)?;
                if sha256_file(&out)? != entry.sha256 {
                    return Err(InstallerError::VerificationFailed(format!(
                        "restored hash mismatch: {}",
                        entry.path
                    )));
                }
            }
            Ok(())
        })();
        if let Err(e) = build_result {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        let rollback = parent.join(format!(".phx-rollback-{}", restore_uuid));
        let had_target = target.exists();
        if had_target {
            if rollback.exists() {
                fs::remove_dir_all(&rollback)?;
            }
            fs::rename(target, &rollback)?;
        }
        if let Err(e) = fs::rename(&staging, target) {
            if had_target && rollback.exists() && !target.exists() {
                let _ = fs::rename(&rollback, target);
            }
            return Err(InstallerError::Io(e));
        }
        let receipt = RestoreReceipt {
            restore_uuid,
            backup_uuid,
            target_path: target.display().to_string(),
            rollback_path: had_target.then(|| rollback.display().to_string()),
            restored_files: manifest.entries.len(),
            verified: true,
            completed_at: Utc::now(),
        };
        evidence(
            ledger,
            action_uuid,
            "installer.restore",
            "restore.commit",
            EvidenceOutcome::Succeeded,
            json!({"backup_uuid":backup_uuid}),
            serde_json::to_value(&receipt)?,
            vec![],
        )?;
        Ok(receipt)
    }

    fn manifest_path(&self, backup_uuid: Uuid) -> PathBuf {
        self.root
            .join("manifests")
            .join(format!("{backup_uuid}.json"))
    }
    fn blob_path(&self, sha256: &str) -> PathBuf {
        let prefix = &sha256[..sha256.len().min(2)];
        self.root.join("blobs").join(prefix).join(sha256)
    }
}

#[derive(Debug, Default)]
pub struct InstallerCore;

impl InstallerCore {
    pub fn plan(&self, action: InstallAction, target: impl Into<String>) -> InstallPlan {
        let target = target.into();
        InstallPlan {
            uuid: new_uuid_v7(),
            action,
            target: target.clone(),
            preflight_checks: vec![
                "validate constitution".into(),
                "validate target manifest".into(),
                "validate disk/database availability".into(),
                "verify backup repository writeability".into(),
                "verify rollback path before mutation".into(),
            ],
            steps: vec![
                format!("create pre-change backup for {target}"),
                format!("execute plan for {target}"),
                format!("verify resulting state for {target}"),
            ],
            rollback_steps: vec![format!("restore verified previous state for {target}")],
            created_at: Utc::now(),
        }
    }
}

pub fn execute_transactional_upgrade<E: UpgradeExecutor>(
    repo: &BackupRepository,
    source: &Path,
    from_version: &str,
    to_version: &str,
    policy: &BackupPolicy,
    executor: &mut E,
    ledger: Option<&EvidenceLedger>,
) -> Result<UpgradeReceipt, InstallerError> {
    let run_uuid = new_uuid_v7();
    let backup = repo.create_backup(source, from_version, policy, ledger)?;
    let mut receipt = UpgradeReceipt {
        run_uuid,
        from_version: from_version.into(),
        to_version: to_version.into(),
        pre_upgrade_backup_uuid: backup.backup_uuid,
        apply_succeeded: false,
        verify_succeeded: false,
        rollback_attempted: false,
        rollback_succeeded: false,
        completed_at: Utc::now(),
    };
    if let Err(e) = executor.apply() {
        receipt.rollback_attempted = true;
        receipt.rollback_succeeded = executor.rollback().is_ok();
        receipt.completed_at = Utc::now();
        if !receipt.rollback_succeeded {
            return Err(InstallerError::UpgradeRollback(e));
        }
        return Err(InstallerError::UpgradeApply(e));
    }
    receipt.apply_succeeded = true;
    if let Err(e) = executor.verify() {
        receipt.rollback_attempted = true;
        receipt.rollback_succeeded = executor.rollback().is_ok();
        receipt.completed_at = Utc::now();
        if !receipt.rollback_succeeded {
            return Err(InstallerError::UpgradeRollback(e));
        }
        return Err(InstallerError::UpgradeVerify(e));
    }
    receipt.verify_succeeded = true;
    receipt.completed_at = Utc::now();
    Ok(receipt)
}

pub fn pg_dump_custom(
    tools: &PostgresToolchain,
    connection: &PostgresConnectionSpec,
    output: &Path,
    ledger: Option<&EvidenceLedger>,
) -> Result<ExternalProcessReceipt, InstallerError> {
    validate_program(&tools.pg_dump, "pg_dump")?;
    validate_pgpass(connection)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let action_uuid = new_uuid_v7();
    evidence(
        ledger,
        action_uuid,
        "installer.postgresql.backup",
        "pg_dump",
        EvidenceOutcome::Requested,
        json!({"database":connection.database,"output":output.display().to_string()}),
        json!({}),
        vec![],
    )?;
    let status = base_pg_command(&tools.pg_dump, connection)
        .arg("--format=custom")
        .arg("--no-password")
        .arg("--file")
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()?;
    process_result(
        status,
        &tools.pg_dump,
        output,
        action_uuid,
        ledger,
        "installer.postgresql.backup",
        "pg_dump",
    )
}

pub fn pg_restore_custom(
    tools: &PostgresToolchain,
    connection: &PostgresConnectionSpec,
    input: &Path,
    clean_before_restore: bool,
    ledger: Option<&EvidenceLedger>,
) -> Result<ExternalProcessReceipt, InstallerError> {
    validate_program(&tools.pg_restore, "pg_restore")?;
    validate_pgpass(connection)?;
    if !input.is_file() {
        return Err(InstallerError::InvalidSource(input.display().to_string()));
    }
    let action_uuid = new_uuid_v7();
    evidence(
        ledger,
        action_uuid,
        "installer.postgresql.restore",
        "pg_restore",
        EvidenceOutcome::Requested,
        json!({"database":connection.database,"input":input.display().to_string(),"clean":clean_before_restore}),
        json!({}),
        vec![],
    )?;
    let mut cmd = base_pg_command(&tools.pg_restore, connection);
    cmd.arg("--no-password")
        .arg("--exit-on-error")
        .arg("--single-transaction")
        .arg("--dbname")
        .arg(&connection.database);
    if clean_before_restore {
        cmd.arg("--clean").arg("--if-exists");
    }
    let status = cmd
        .arg(input)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()?;
    process_result(
        status,
        &tools.pg_restore,
        input,
        action_uuid,
        ledger,
        "installer.postgresql.restore",
        "pg_restore",
    )
}

fn base_pg_command(program: &Path, c: &PostgresConnectionSpec) -> Command {
    let mut cmd = Command::new(program);
    cmd.env_clear()
        .env("PGHOST", &c.host)
        .env("PGPORT", c.port.to_string())
        .env("PGDATABASE", &c.database)
        .env("PGUSER", &c.user)
        .env("PGPASSFILE", &c.pgpassfile)
        .env("LC_ALL", "C");
    cmd
}

fn validate_pgpass(c: &PostgresConnectionSpec) -> Result<(), InstallerError> {
    if !c.pgpassfile.is_file() {
        return Err(InstallerError::InvalidSource(
            c.pgpassfile.display().to_string(),
        ));
    }
    Ok(())
}

fn validate_program(path: &Path, expected: &str) -> Result<(), InstallerError> {
    if !path.is_file() || path.file_name() != Some(OsStr::new(expected)) {
        return Err(InstallerError::UnsafeProgram(path.display().to_string()));
    }
    Ok(())
}

fn process_result(
    status: ExitStatus,
    program: &Path,
    artifact: &Path,
    action_uuid: Uuid,
    ledger: Option<&EvidenceLedger>,
    capability: &str,
    action: &str,
) -> Result<ExternalProcessReceipt, InstallerError> {
    let code = status.code().unwrap_or(-1);
    if !status.success() {
        evidence(
            ledger,
            action_uuid,
            capability,
            action,
            EvidenceOutcome::Failed,
            json!({}),
            json!({"status_code":code}),
            vec![],
        )?;
        return Err(InstallerError::ProcessFailed {
            program: program.display().to_string(),
            status: code.to_string(),
        });
    }
    let receipt = ExternalProcessReceipt {
        action_uuid,
        program: program.display().to_string(),
        status_code: code,
        artifact_path: artifact.display().to_string(),
        completed_at: Utc::now(),
    };
    evidence(
        ledger,
        action_uuid,
        capability,
        action,
        EvidenceOutcome::Succeeded,
        json!({}),
        serde_json::to_value(&receipt)?,
        vec![artifact.display().to_string()],
    )?;
    Ok(receipt)
}

fn excluded(rel: &Path, excluded: &BTreeSet<String>) -> bool {
    rel.components().any(|c| match c {
        Component::Normal(v) => excluded.contains(&v.to_string_lossy().to_string()),
        _ => false,
    })
}

fn validate_relative(path: &Path) -> Result<(), InstallerError> {
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err(InstallerError::UnsafePath(path.display().to_string()));
    }
    for c in path.components() {
        if !matches!(c, Component::Normal(_)) {
            return Err(InstallerError::UnsafePath(path.display().to_string()));
        }
    }
    Ok(())
}

fn rel_to_string(path: &Path) -> Result<String, InstallerError> {
    validate_relative(path)?;
    Ok(path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn sha256_file(path: &Path) -> Result<String, InstallerError> {
    let mut file = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; BUFFER_BYTES];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn copy_file_synced(src: &Path, dst: &Path) -> Result<(), InstallerError> {
    let mut input = File::open(src)?;
    let mut output = File::create(dst)?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    output.sync_all()?;
    Ok(())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), InstallerError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", new_uuid_v7()));
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn evidence(
    ledger: Option<&EvidenceLedger>,
    action_uuid: Uuid,
    capability: &str,
    action: &str,
    outcome: EvidenceOutcome,
    request: serde_json::Value,
    result: serde_json::Value,
    artifacts: Vec<String>,
) -> Result<(), InstallerError> {
    if let Some(l) = ledger {
        l.append(EvidenceDraft {
            action_uuid,
            correlation_uuid: None,
            actor: "phxclaw-installer".into(),
            capability: capability.into(),
            action: action.into(),
            outcome,
            request_summary: request,
            result_summary: result,
            artifact_uris: artifacts,
        })
        .map_err(|e| InstallerError::Evidence(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("phxclaw-installer-{name}-{}", new_uuid_v7()))
    }

    #[test]
    fn installer_always_has_rollback() {
        let plan = InstallerCore.plan(InstallAction::Verify, "phxclaw");
        assert!(!plan.rollback_steps.is_empty());
    }

    #[test]
    fn backup_restore_roundtrip() {
        let source = temp("source");
        let repo = temp("repo");
        let target = temp("target");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("a.txt"), b"alpha").unwrap();
        fs::write(source.join("nested/b.txt"), b"beta").unwrap();
        let r = BackupRepository::open(&repo).unwrap();
        let receipt = r
            .create_backup(&source, "0.65.0", &BackupPolicy::default(), None)
            .unwrap();
        assert!(r.verify_backup(receipt.backup_uuid).unwrap().valid);
        let restored = r
            .restore_backup(receipt.backup_uuid, &target, None)
            .unwrap();
        assert!(restored.verified);
        assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"alpha");
        assert_eq!(fs::read(target.join("nested/b.txt")).unwrap(), b"beta");
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(repo);
        let _ = fs::remove_dir_all(target);
    }
}
