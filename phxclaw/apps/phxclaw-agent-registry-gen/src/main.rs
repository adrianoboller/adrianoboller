use anyhow::{Context, Result, bail};
use calamine::{Reader, open_workbook_auto};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
struct AgentDependency {
    uuid: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct Permission {
    name: String,
    scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SourceRef {
    workbook: String,
    sheet: String,
    row: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AgentManifest {
    manifest_version: String,
    uuid: Uuid,
    agent_id: u32,
    name: String,
    macroarea: String,
    nucleus: String,
    role_type: String,
    mission: String,
    responsibilities: String,
    resources: String,
    authorized_actions: String,
    trigger: String,
    declared_dependencies: String,
    resolved_agent_dependencies: Vec<AgentDependency>,
    deliverables: String,
    limits: String,
    next_gate: String,
    execution: String,
    models_allowed: Vec<String>,
    criticality: String,
    modules: Vec<String>,
    capabilities: Vec<String>,
    permissions: Vec<Permission>,
    lifecycle: Vec<String>,
    memory_skill_state: String,
    event_topics: String,
    execution_policy: String,
    references: String,
    knowledge_sources: Vec<String>,
    source: SourceRef,
}

fn text(row: &[calamine::Data], index: usize) -> String {
    row.get(index)
        .map(ToString::to_string)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn split_pipe(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

fn models(value: &str) -> Vec<String> {
    value
        .split(['/', ',', '|'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| x.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn module_capabilities(modules: &[String]) -> BTreeSet<String> {
    let mut caps = BTreeSet::new();
    for module in modules {
        if module.starts_with("F14") {
            caps.insert("desktop.host.request".to_string());
        }
        if module.starts_with("F15") {
            caps.insert("skill.read".to_string());
            caps.insert("memory.read".to_string());
            caps.insert("context.compile".to_string());
        }
        if module.starts_with("F16") {
            caps.insert("job.read".to_string());
        }
        if module.starts_with("F17") {
            caps.insert("checkpoint.read".to_string());
        }
        if module.starts_with("F18") {
            caps.insert("mcp.invoke".to_string());
            caps.insert("lsp.read".to_string());
        }
        if module.starts_with("F19") {
            caps.insert("repo.read".to_string());
            caps.insert("tree_sitter.analyze".to_string());
        }
        if module.starts_with("F20") {
            caps.insert("team.message".to_string());
        }
        if module.starts_with("F21") {
            caps.insert("channel.read".to_string());
        }
        if module.starts_with("F22") {
            caps.insert("node.observe".to_string());
        }
        if module.starts_with("F23") {
            caps.insert("secret.lease.request".to_string());
        }
        if module.starts_with("F24") {
            caps.insert("skill.candidate.propose".to_string());
        }
        if module.starts_with("F25") {
            caps.insert("knowledge.read".to_string());
            caps.insert("evidence.read".to_string());
        }
    }
    caps
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let workbook_path = PathBuf::from(
        args.next()
            .context("usage: phxclaw-agent-registry-gen <workbook.xlsx> <out-dir>")?,
    );
    let out_dir = PathBuf::from(args.next().context("missing output directory")?);
    fs::create_dir_all(&out_dir)?;

    let mut workbook = open_workbook_auto(&workbook_path)
        .with_context(|| format!("open {}", workbook_path.display()))?;
    let range = workbook
        .worksheet_range("Equipe 110")
        .context("read sheet Equipe 110")?;

    let rows: Vec<_> = range
        .rows()
        .skip(1)
        .filter(|row| !text(row, 3).is_empty())
        .collect();
    if rows.len() != 110 {
        bail!("expected 110 agent rows, found {}", rows.len());
    }

    let mut identities = BTreeMap::<String, Uuid>::new();
    for row in &rows {
        let name = text(row, 3);
        let id =
            Uuid::parse_str(&text(row, 17)).with_context(|| format!("invalid UUID for {name}"))?;
        if (id.as_bytes()[6] >> 4) != 7 {
            bail!("{name} does not use UUIDv7");
        }
        identities.insert(name, id);
    }

    let workbook_name = workbook_path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("agents.xlsx")
        .to_string();
    let mut index = Vec::<serde_json::Value>::new();

    for (offset, row) in rows.iter().enumerate() {
        let name = text(row, 3);
        let uuid = identities[&name];
        let raw_dependencies = format!("{} | {}", text(row, 10), text(row, 13));
        let resolved_agent_dependencies = identities
            .iter()
            .filter(|(candidate, _)| {
                *candidate != &name
                    && candidate.len() >= 5
                    && raw_dependencies.contains(candidate.as_str())
            })
            .map(|(candidate, id)| AgentDependency {
                uuid: *id,
                name: candidate.clone(),
            })
            .collect::<Vec<_>>();

        let modules = split_pipe(&text(row, 18));
        let mut capabilities = module_capabilities(&modules);
        let principal = text(row, 19);
        if !principal.is_empty() {
            capabilities.insert(principal);
        }
        let mut knowledge_sources = vec![];
        if matches!(
            name.as_str(),
            "Research Agent" | "Research Lead / Pesquisador PDCA" | "Documentador" | "Perséfone"
        ) {
            capabilities.insert("knowledge.rust.read".into());
            capabilities.insert("knowledge.source_registry.read".into());
            knowledge_sources.push("rust-official".into());
        }
        if name == "Research Agent" || name == "Research Lead / Pesquisador PDCA" {
            capabilities.insert("research.source.read".into());
        }
        if name == "Documentador" || name == "Perséfone" {
            capabilities.insert("documentation.write".into());
        }

        let permissions = capabilities
            .iter()
            .map(|cap| Permission {
                name: cap.clone(),
                scopes: if cap.starts_with("knowledge.rust") {
                    vec!["rust-official".into()]
                } else {
                    vec!["project".into()]
                },
            })
            .collect();

        let manifest = AgentManifest {
            manifest_version: "1.0.0".into(),
            uuid,
            agent_id: text(row, 0).parse().unwrap_or((offset + 1) as u32),
            name: name.clone(),
            macroarea: text(row, 1),
            nucleus: text(row, 2),
            role_type: text(row, 4),
            mission: text(row, 5),
            responsibilities: text(row, 6),
            resources: text(row, 7),
            authorized_actions: text(row, 8),
            trigger: text(row, 9),
            declared_dependencies: text(row, 10),
            resolved_agent_dependencies,
            deliverables: text(row, 11),
            limits: text(row, 12),
            next_gate: text(row, 13),
            execution: text(row, 14),
            models_allowed: models(&text(row, 15)),
            criticality: text(row, 16),
            modules,
            capabilities: capabilities.into_iter().collect(),
            permissions,
            lifecycle: vec![
                "registered",
                "starting",
                "ready",
                "busy",
                "draining",
                "stopped",
                "failed",
                "quarantined",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            memory_skill_state: text(row, 20),
            event_topics: text(row, 21),
            execution_policy: text(row, 22),
            references: text(row, 23),
            knowledge_sources,
            source: SourceRef {
                workbook: workbook_name.clone(),
                sheet: "Equipe 110".into(),
                row: offset + 2,
            },
        };

        let file_name = format!(
            "{:03}-{}.agent.json",
            manifest.agent_id,
            slug(&manifest.name)
        );
        fs::write(
            out_dir.join(&file_name),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        index.push(serde_json::json!({"uuid": manifest.uuid, "agent_id": manifest.agent_id, "name": manifest.name, "file": file_name}));
    }

    fs::write(
        out_dir.join("registry.index.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "manifest_version": "1.0.0",
            "source_workbook": workbook_name,
            "sheet": "Equipe 110",
            "count": index.len(),
            "agents": index
        }))?,
    )?;
    println!(
        "generated {} manifests in {}",
        index.len(),
        out_dir.display()
    );
    Ok(())
}
