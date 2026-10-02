use anyhow::{Context, Result};
use chrono::Utc;
use phxclaw_agent_catalog::AgentCatalog;
use phxclaw_agent_runtime::{AgentRuntime, LogicalAgentRuntime};
use phxclaw_evidence_ledger::EvidenceLedger;
use phxclaw_hypothesis::HypothesisCore;
use phxclaw_installer::InstallerCore;
use phxclaw_kernel::{Constitution, Kernel};
use phxclaw_live_bus::LiveEventHub;
use phxclaw_memory_context::{DataClassification, InMemoryStore, MemoryRecord, MemoryScope, MemoryStore};
use phxclaw_model_gateway::{ModelGateway, ModelRequest};
use phxclaw_plugin_registry::{PluginRegistry, TrustStore};
use phxclaw_process_protocol::{
    ProcessEnvelope, ProcessMessageKind, ProcessReply, ProcessRunner,
};
use phxclaw_research::ResearchCore;
use phxclaw_research_pipeline::{ResearchPipeline, ResearchTask};
use phxclaw_sandbox::{BwrapSandbox, SandboxBackend};
use phxclaw_skill_runtime::{LazySkillResolver, SkillResolutionPolicy};
use phxclaw_source_registry::SourceRegistry;
use phxclaw_task_graph::{ApprovalGate, PostgresTaskJournal, TaskGraph, TaskScheduler, TaskSpec};
use phxclaw_types::{AgentRequest, InstallAction};
use postgres::{Client, NoTls};
use serde_json::Value;
use std::{env, path::PathBuf};

fn main() -> Result<()> {
    let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let constitution = Constitution::from_json(include_str!("../../../config/constitution.json"))
        .context("invalid constitution.json")?;
    let kernel = Kernel::boot(constitution).map_err(anyhow::Error::msg)?;

    let trust_store = TrustStore::from_json(include_str!(
        "../../../config/trust/plugin-signers.json"
    ))
    .context("invalid plugin trust store")?;
    let mut registry = PluginRegistry::new(
        kernel.constitution.plugin_api_version.clone(),
        &package_root,
        trust_store,
    );
    let discovery = registry
        .discover_tree(&package_root.join("plugins"))
        .context("plugin discovery failed")?;

    let research = ResearchCore;
    let hypothesis = HypothesisCore;
    let installer = InstallerCore;
    let research_record = research.create_record(
        "phxclaw-task-graph",
        "O fluxo F06 consegue manter dependências, retries e approval gates determinísticos?",
        vec![],
    );
    let hypothesis_record = hypothesis.propose(
        "O Task Graph impede execução fora de ordem e o Model Gateway aplica política de custo/localidade antes do provider.",
        "F06 valida DAG e F07 filtra providers por capability, classificação, localidade e orçamento.",
        vec![
            "validar DAG".into(),
            "executar scheduler".into(),
            "testar gate humano".into(),
            "rotear provider local".into(),
        ],
        vec![
            "dependências respeitadas".into(),
            "gate impede publicação antes da aprovação".into(),
            "budget/local policy aplicada".into(),
        ],
        vec!["tarefa dependente executada antes do predecessor".into()],
    );
    let install_plan = installer.plan(InstallAction::Verify, "phxclaw-core-v0.8");

    // F05: agentes plugáveis e sandbox fail-closed.
    let sandbox = BwrapSandbox::new(&package_root);
    let sandbox_probe = sandbox.probe();
    let process_runner = ProcessRunner::new(sandbox.clone());
    let mut agent_runtime = AgentRuntime::new(sandbox);
    let agent_ids = registry
        .manifests()
        .filter(|manifest| manifest.kind == "agent")
        .map(|manifest| agent_runtime.register(manifest.clone()))
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("agent registration failed")?;

    let mut routed_agent = None;
    let mut process_health = None;
    if sandbox_probe.is_ok() {
        for agent_id in agent_ids {
            agent_runtime
                .start(agent_id)
                .with_context(|| format!("could not start agent {agent_id}"))?;
        }
        let request = AgentRequest::new(
            "decision.technical",
            serde_json::json!({"subject": "task-graph-and-model-gateway"}),
        );
        let route = agent_runtime.route(&request)?;
        routed_agent = Some(route.agent_name.clone());
        if let Some(instance) = agent_runtime.agent(&route.agent_uuid) {
            let health = ProcessEnvelope::new(
                ProcessMessageKind::Health,
                serde_json::json!({"source": "phxclaw-cli"}),
            );
            let reply: ProcessReply<Value> = process_runner
                .request(&instance.manifest, &health)
                .context("process protocol health request failed")?;
            process_health = Some(reply.status);
        }
    }

    // F06: DAG determinístico com idempotência, retry e aprovação humana.
    let research_task = TaskSpec::new(
        "Pesquisar evidências",
        "research.execute",
        serde_json::json!({"topic": "PhxClaw v0.8"}),
        "demo:research:v08",
    );
    let research_task_uuid = research_task.uuid;

    let mut technical_task = TaskSpec::new(
        "Decisão técnica",
        "decision.technical",
        serde_json::json!({"subject": "release-v0.8"}),
        "demo:decision:v08",
    );
    technical_task.dependencies.push(research_task_uuid);
    technical_task.priority = 200;
    let technical_task_uuid = technical_task.uuid;

    let mut release_task = TaskSpec::new(
        "Publicar release",
        "release.publish",
        serde_json::json!({"version": "0.11.0"}),
        "demo:release:v08",
    );
    release_task.dependencies.push(technical_task_uuid);
    release_task.approval = Some(ApprovalGate {
        required: true,
        role: "product_owner".into(),
        reason: "Publicação exige aprovação humana explícita".into(),
    });
    let release_task_uuid = release_task.uuid;

    let graph = TaskGraph::new(vec![research_task, technical_task, release_task])?;
    let graph_for_persistence = graph.clone();
    let mut scheduler = TaskScheduler::new(graph);
    let now = Utc::now();
    if let Some(run) = scheduler.claim_ready(1, now).into_iter().next() {
        scheduler.succeed(run.uuid, serde_json::json!({"evidence": "demo-ok"}), now)?;
    }
    if let Some(run) = scheduler.claim_ready(1, now).into_iter().next() {
        scheduler.succeed(run.uuid, serde_json::json!({"decision": "continue"}), now)?;
    }
    let release_before_approval = scheduler
        .state(&release_task_uuid)
        .map(|state| state.status);
    scheduler.approve(
        release_task_uuid,
        "product-owner-demo",
        Some("gate F06 aprovado no bootstrap".into()),
        now,
    )?;
    let release_run = scheduler.claim_ready(1, now).into_iter().next();

    // F07: providers permanecem plugins; roteamento não depende de um fornecedor específico.
    let mut model_gateway = ModelGateway::new();
    for manifest in registry
        .manifests()
        .filter(|manifest| manifest.kind == "model_provider")
    {
        model_gateway.register_provider(manifest.clone())?;
    }
    model_gateway.set_budget("default", 1_000_000);
    let mut model_request = ModelRequest::new(
        "model.reasoning",
        serde_json::json!({"prompt": "Validar arquitetura PhxClaw v0.8"}),
    );
    model_request.requires_local = true;
    model_request.max_cost_microunits = 50_000;
    let model_route = model_gateway.route(&model_request)?;
    model_gateway.reserve_route(&model_request, &model_route)?;

    // F15/F16 v0.8: Agent Catalog + lazy Skill Resolver + Source Registry +
    // query-bounded Context Compiler + Live Event Bus + Evidence Ledger.
    let agent_catalog = AgentCatalog::load_dir(package_root.join("config/agents"))
        .context("could not load declarative agent catalog")?;
    let mut logical_runtime = LogicalAgentRuntime::from_catalog(agent_catalog)
        .context("could not initialize logical agent runtime")?;
    logical_runtime.start_all();
    let skill_resolver = LazySkillResolver::load(package_root.join("config/skills/registry.index.json"))
        .context("could not load lazy skill index")?;
    let source_registry = SourceRegistry::load_dir(package_root.join("config/knowledge-sources"))
        .context("could not load knowledge source registry")?;
    let live_bus = LiveEventHub::new(256, 256);
    let evidence_ledger = EvidenceLedger::open(package_root.join("var/evidence/research-cli.jsonl"))
        .context("could not open research evidence ledger")?;
    let research_pipeline = ResearchPipeline::new(
        &package_root,
        logical_runtime,
        skill_resolver,
        SkillResolutionPolicy {
            require_promoted: false,
            allow_validated: true,
        },
        source_registry,
        live_bus.clone(),
        evidence_ledger,
    );
    let mut research_memory = InMemoryStore::default();
    research_memory.put(MemoryRecord::new(
        "project",
        "rust-policy",
        serde_json::json!({
            "rule": "Prefer official Rust documentation and bounded context; record evidence for every source."
        }),
        MemoryScope::Project("phxclaw".into()),
        DataClassification::Internal,
        vec![],
    )?)?;
    let rust_task = ResearchTask::rust(
        "Como implementar propagacao de erros com Result e ? em Rust?",
        "phxclaw",
    );
    let prepared_research = research_pipeline
        .prepare(&research_memory, &rust_task)
        .context("F15/F16 research pipeline preparation failed")?;

    if let Some(database_url) =
        phxclaw_config_runtime::agente::carga::texto_do_processo("postgres.database_url")
    {
        let mut client = Client::connect(&database_url, NoTls)
            .context("could not connect to postgres.database_url (PHXCLAW_DATABASE_URL)")?;
        registry
            .persist_postgres(&mut client)
            .context("could not persist plugin registry")?;
        PostgresTaskJournal::persist_graph(&mut client, &graph_for_persistence)
            .context("could not persist task graph")?;
        PostgresTaskJournal::append_events(&mut client, scheduler.events())
            .context("could not persist task events")?;
    }

    println!("PhxClaw {} booted", kernel.constitution.core_version);
    println!("session_uuid={}", kernel.session_uuid);
    println!("plugins_accepted={}", discovery.accepted);
    println!("plugins_quarantined={}", registry.quarantine().len());
    println!("agents_registered={}", agent_runtime.agents().count());
    println!(
        "sandbox_status={}",
        sandbox_probe
            .as_ref()
            .map(|_| "available")
            .unwrap_or("unavailable_fail_closed")
    );
    if let Some(agent) = routed_agent {
        println!("decision.technical routed_to={agent}");
    }
    if let Some(status) = process_health {
        println!("process_protocol_health={status:?}");
    }
    println!("task_graph_nodes={}", graph_for_persistence.tasks().count());
    println!("task_graph_events={}", scheduler.events().len());
    println!("release_before_approval={release_before_approval:?}");
    println!("release_claimed_after_approval={}", release_run.is_some());
    println!("model_provider={}", model_route.selected.provider_name);
    println!("model_id={}", model_route.selected.model_id);
    println!(
        "model_estimated_cost_microunits={}",
        model_route.selected.estimated_cost_microunits
    );
    println!("logical_research_agent={}", prepared_research.agent.agent_name);
    println!("logical_research_skill={}@{}", prepared_research.skill.skill.name, prepared_research.skill.skill.version);
    println!("logical_research_context_pack={}", prepared_research.context.pack.uuid);
    println!("logical_research_source_hits={}", prepared_research.source_hits.len());
    println!("logical_research_evidence={}", prepared_research.evidence.len());
    println!("live_event_replay={}", live_bus.snapshot(256).map(|events| events.len()).unwrap_or(0));
    println!("research_uuid={}", research_record.uuid);
    println!("hypothesis_uuid={}", hypothesis_record.uuid);
    println!("install_plan_uuid={}", install_plan.uuid);
    Ok(())
}
