use chrono::Utc;
use phxclaw_agent_runtime::{AgentRuntimeError, LogicalAgentRuntime, LogicalRouteDecision};
use phxclaw_evidence_ledger::{
    EvidenceDraft, EvidenceLedger, EvidenceOutcome, LedgerError,
};
use phxclaw_live_bus::{LiveBusError, LiveEventHub};
use phxclaw_memory_context::{
    ContextBundle, ContextCompiler, ContextPolicy, ContextScopeFilter, MemoryError, MemoryStore,
};
use phxclaw_research::ResearchCore;
use phxclaw_skill_runtime::{
    LazySkillResolver, SkillError, SkillResolution, SkillResolutionPolicy, SkillResolutionRequest,
};
use phxclaw_source_registry::{
    OfflineSearchHit, ResolvedSource, SourceFreshness, SourceRegistry, SourceRegistryError,
};
use phxclaw_types::{new_uuid_v7, AgentRequest, EvidenceRef, PermissionClaim, ResearchRecord};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchTask {
    pub uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub query: String,
    pub project: String,
    pub preferred_agent: String,
    pub source_name: String,
    pub skill_name: String,
    pub skill_version_requirement: String,
    pub freshness: SourceFreshness,
    pub max_source_hits: usize,
    pub context_policy: ContextPolicy,
    pub scope_filter: ContextScopeFilter,
}

impl ResearchTask {
    pub fn rust(query: impl Into<String>, project: impl Into<String>) -> Self {
        let project = project.into();
        let correlation_uuid = new_uuid_v7();
        Self {
            uuid: new_uuid_v7(),
            correlation_uuid,
            query: query.into(),
            project: project.clone(),
            preferred_agent: "Research Agent".into(),
            source_name: "rust-official".into(),
            skill_name: "rust.research.official".into(),
            skill_version_requirement: "^1".into(),
            freshness: SourceFreshness::Stable,
            max_source_hits: 8,
            context_policy: ContextPolicy {
                max_items: 24,
                max_bytes: 96 * 1024,
                ..ContextPolicy::default()
            },
            scope_filter: ContextScopeFilter {
                project: Some(project),
                ..ContextScopeFilter::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PreparedSource {
    Offline {
        source_name: String,
        root: String,
        index: String,
        verified_version: Option<String>,
    },
    Online {
        source_name: String,
        endpoint_name: String,
        url: String,
        authority: String,
        retrieval_required: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedAgent {
    pub request_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub agent_name: String,
    pub capability: String,
    pub models_allowed: Vec<String>,
    pub knowledge_sources: Vec<String>,
}

impl From<LogicalRouteDecision> for PreparedAgent {
    fn from(value: LogicalRouteDecision) -> Self {
        Self {
            request_uuid: value.request_uuid,
            agent_uuid: value.agent_uuid,
            agent_name: value.agent_name,
            capability: value.capability,
            models_allowed: value.models_allowed,
            knowledge_sources: value.knowledge_sources,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedResearchTask {
    pub task_uuid: Uuid,
    pub correlation_uuid: Uuid,
    pub agent: PreparedAgent,
    pub skill: SkillResolution,
    pub source: PreparedSource,
    pub source_hits: Vec<OfflineSearchHit>,
    pub context: ContextBundle,
    pub research_record: ResearchRecord,
    pub evidence: Vec<EvidenceRef>,
    pub model_context: Value,
    pub prepared_at: chrono::DateTime<Utc>,
}

pub struct ResearchPipeline {
    project_root: PathBuf,
    agent_runtime: LogicalAgentRuntime,
    skill_resolver: LazySkillResolver,
    skill_policy: SkillResolutionPolicy,
    source_registry: SourceRegistry,
    live_bus: LiveEventHub,
    evidence_ledger: EvidenceLedger,
}

impl ResearchPipeline {
    pub fn new(
        project_root: impl Into<PathBuf>,
        agent_runtime: LogicalAgentRuntime,
        skill_resolver: LazySkillResolver,
        skill_policy: SkillResolutionPolicy,
        source_registry: SourceRegistry,
        live_bus: LiveEventHub,
        evidence_ledger: EvidenceLedger,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            agent_runtime,
            skill_resolver,
            skill_policy,
            source_registry,
            live_bus,
            evidence_ledger,
        }
    }

    pub fn prepare<S: MemoryStore>(
        &self,
        memory_store: &S,
        task: &ResearchTask,
    ) -> Result<PreparedResearchTask, ResearchPipelineError> {
        let started = self.live_bus.publish_json(
            "research.pipeline",
            "started",
            json!({
                "task_uuid": task.uuid,
                "query": &task.query,
                "source": &task.source_name,
            }),
            Some(task.correlation_uuid),
            None,
        )?;

        match self.prepare_inner(memory_store, task, started.uuid) {
            Ok(prepared) => {
                self.live_bus.publish_json(
                    "research.pipeline",
                    "ready",
                    json!({
                        "task_uuid": task.uuid,
                        "agent_uuid": prepared.agent.agent_uuid,
                        "context_pack_uuid": prepared.context.pack.uuid,
                        "source_hits": prepared.source_hits.len(),
                        "evidence": prepared.evidence.iter().map(|e| e.uuid).collect::<Vec<_>>(),
                    }),
                    Some(task.correlation_uuid),
                    Some(started.uuid),
                )?;
                Ok(prepared)
            }
            Err(error) => {
                let _ = self.live_bus.publish_json(
                    "research.pipeline",
                    "failed",
                    json!({
                        "task_uuid": task.uuid,
                        "error": error.to_string(),
                    }),
                    Some(task.correlation_uuid),
                    Some(started.uuid),
                );
                let _ = self.evidence_ledger.append(EvidenceDraft {
                    action_uuid: task.uuid,
                    correlation_uuid: Some(task.correlation_uuid),
                    actor: "phxclaw-research-pipeline".into(),
                    capability: "research.pipeline.prepare".into(),
                    action: "prepare_research_context".into(),
                    outcome: EvidenceOutcome::Failed,
                    request_summary: json!({
                        "query_sha256": sha256_hex(task.query.as_bytes()),
                        "source": &task.source_name,
                    }),
                    result_summary: json!({"error": error.to_string()}),
                    artifact_uris: vec![],
                });
                Err(error)
            }
        }
    }

    fn prepare_inner<S: MemoryStore>(
        &self,
        memory_store: &S,
        task: &ResearchTask,
        mut causation_uuid: Uuid,
    ) -> Result<PreparedResearchTask, ResearchPipelineError> {
        let mut request = AgentRequest::new(
            "research.collect",
            json!({
                "task_uuid": task.uuid,
                "query": &task.query,
                "source": &task.source_name,
            }),
        );
        request.requested_permissions = vec![
            PermissionClaim {
                name: "research.collect".into(),
                scope: "project".into(),
            },
            PermissionClaim {
                name: "knowledge.rust.read".into(),
                scope: task.source_name.clone(),
            },
            PermissionClaim {
                name: "context.compile".into(),
                scope: "project".into(),
            },
            PermissionClaim {
                name: "skill.read".into(),
                scope: "project".into(),
            },
        ];

        let route = self.agent_runtime.route_named(&task.preferred_agent, &request)?;
        let route_event = self.live_bus.publish_json(
            "agent.runtime",
            "routed",
            json!({
                "task_uuid": task.uuid,
                "request_uuid": route.request_uuid,
                "agent_uuid": route.agent_uuid,
                "agent_name": &route.agent_name,
                "capability": &route.capability,
            }),
            Some(task.correlation_uuid),
            Some(causation_uuid),
        )?;
        causation_uuid = route_event.uuid;

        let agent_instance = self
            .agent_runtime
            .agent(&route.agent_uuid)
            .ok_or(ResearchPipelineError::MissingAgent(route.agent_uuid))?;
        let agent_capabilities = agent_instance.manifest.capability_set();
        if !agent_instance.manifest.can_use_source(&task.source_name) {
            return Err(ResearchPipelineError::AgentSourceDenied {
                agent: route.agent_name.clone(),
                source: task.source_name.clone(),
            });
        }

        let skill = self.skill_resolver.resolve(
            &SkillResolutionRequest {
                query: task.query.clone(),
                preferred_name: Some(task.skill_name.clone()),
                version_requirement: Some(task.skill_version_requirement.clone()),
                knowledge_source: Some(task.source_name.clone()),
            },
            &agent_capabilities,
            &self.skill_policy,
        )?;
        let skill_event = self.live_bus.publish_json(
            "skill.runtime",
            "resolved",
            json!({
                "task_uuid": task.uuid,
                "skill_uuid": skill.skill.uuid,
                "skill": &skill.skill.name,
                "version": &skill.skill.version,
                "state": skill.skill.state,
                "score": skill.score,
                "matched_triggers": &skill.matched_triggers,
            }),
            Some(task.correlation_uuid),
            Some(causation_uuid),
        )?;
        causation_uuid = skill_event.uuid;

        let resolved = self.source_registry.resolve_for_query(
            &task.source_name,
            &self.project_root,
            &route.agent_name,
            &agent_capabilities,
            task.freshness,
            &task.query,
        )?;

        let (source, source_hits, mut source_evidence) = match resolved {
            ResolvedSource::Offline {
                root,
                index,
                verified_version,
            } => {
                let hits = self.source_registry.search_offline(
                    &task.source_name,
                    &self.project_root,
                    &route.agent_name,
                    &agent_capabilities,
                    &task.query,
                    task.max_source_hits,
                )?;
                let refs = hits
                    .iter()
                    .map(|hit| EvidenceRef {
                        uuid: new_uuid_v7(),
                        uri: hit.uri.clone(),
                        source_type: "knowledge_source_offline".into(),
                        retrieved_at: Utc::now(),
                        sha256: hit.sha256.clone(),
                        notes: Some(format!("score={}; title={}", hit.score, hit.title)),
                    })
                    .collect::<Vec<_>>();
                (
                    PreparedSource::Offline {
                        source_name: task.source_name.clone(),
                        root: root.display().to_string(),
                        index: index.display().to_string(),
                        verified_version,
                    },
                    hits,
                    refs,
                )
            }
            ResolvedSource::Online {
                name,
                url,
                authority,
            } => {
                let url_string = url.to_string();
                (
                    PreparedSource::Online {
                        source_name: task.source_name.clone(),
                        endpoint_name: name,
                        url: url_string.clone(),
                        authority,
                        retrieval_required: true,
                    },
                    vec![],
                    vec![EvidenceRef {
                        uuid: new_uuid_v7(),
                        uri: url_string.clone(),
                        source_type: "knowledge_source_online_plan".into(),
                        retrieved_at: Utc::now(),
                        sha256: sha256_hex(url_string.as_bytes()),
                        notes: Some(
                            "Official source selected; content retrieval is delegated to the HTTP/Browser capability under egress policy."
                                .into(),
                        ),
                    }],
                )
            }
        };

        let source_event = self.live_bus.publish_json(
            "knowledge.source",
            if source_hits.is_empty() {
                "resolved"
            } else {
                "retrieved_offline"
            },
            json!({
                "task_uuid": task.uuid,
                "source": &source,
                "hit_count": source_hits.len(),
                "evidence": source_evidence.iter().map(|item| item.uuid).collect::<Vec<_>>(),
            }),
            Some(task.correlation_uuid),
            Some(causation_uuid),
        )?;
        causation_uuid = source_event.uuid;

        let context = ContextCompiler.compile_for_query(
            memory_store,
            task.correlation_uuid,
            &task.query,
            &task.context_policy,
            &ContextScopeFilter {
                agent_uuid: Some(route.agent_uuid),
                project: task.scope_filter.project.clone(),
                session_uuid: task.scope_filter.session_uuid,
                organization: task.scope_filter.organization.clone(),
            },
        )?;
        let context_event = self.live_bus.publish_json(
            "context.compiler",
            "compiled",
            json!({
                "task_uuid": task.uuid,
                "context_pack_uuid": context.pack.uuid,
                "items": context.pack.items.len(),
                "bytes_estimate": context.pack.bytes_estimate,
                "query_sha256": &context.query_sha256,
            }),
            Some(task.correlation_uuid),
            Some(causation_uuid),
        )?;
        causation_uuid = context_event.uuid;

        let source_artifacts = source_evidence
            .iter()
            .map(|item| item.uri.clone())
            .collect::<Vec<_>>();
        let stage_record = self.evidence_ledger.append(EvidenceDraft {
            action_uuid: task.uuid,
            correlation_uuid: Some(task.correlation_uuid),
            actor: route.agent_name.clone(),
            capability: "research.pipeline.prepare".into(),
            action: "resolve_agent_skill_source_context".into(),
            outcome: EvidenceOutcome::Succeeded,
            request_summary: json!({
                "query_sha256": sha256_hex(task.query.as_bytes()),
                "source": &task.source_name,
                "freshness": task.freshness,
                "skill": &task.skill_name,
            }),
            result_summary: json!({
                "agent_uuid": route.agent_uuid,
                "skill_uuid": skill.skill.uuid,
                "context_pack_uuid": context.pack.uuid,
                "source_hits": source_hits.len(),
            }),
            artifact_uris: source_artifacts,
        })?;
        let ledger_ref = EvidenceRef {
            uuid: stage_record.uuid,
            uri: format!("evidence://{}", stage_record.uuid),
            source_type: "evidence_ledger".into(),
            retrieved_at: stage_record.occurred_at,
            sha256: stage_record.record_hash.clone(),
            notes: Some("Research pipeline preparation evidence".into()),
        };
        source_evidence.push(ledger_ref.clone());

        self.live_bus.publish_json(
            "evidence.ledger",
            "recorded",
            json!({
                "task_uuid": task.uuid,
                "evidence_uuid": stage_record.uuid,
                "record_hash": &stage_record.record_hash,
            }),
            Some(task.correlation_uuid),
            Some(causation_uuid),
        )?;

        let research_core = ResearchCore;
        let mut research_record = research_core.create_record(
            "rust",
            task.query.clone(),
            source_evidence.clone(),
        );
        research_record.findings = if source_hits.is_empty() {
            vec!["Official online source selected; content retrieval is still required under egress policy.".into()]
        } else {
            source_hits
                .iter()
                .map(|hit| format!("{} — {}", hit.title, hit.excerpt))
                .collect()
        };

        let model_context = json!({
            "task": {
                "uuid": task.uuid,
                "correlation_uuid": task.correlation_uuid,
                "query": &task.query,
                "freshness": task.freshness,
            },
            "agent": {
                "uuid": route.agent_uuid,
                "name": &route.agent_name,
                "models_allowed": &route.models_allowed,
            },
            "skill": {
                "uuid": skill.skill.uuid,
                "name": &skill.skill.name,
                "version": &skill.skill.version,
                "steps": &skill.skill.steps,
            },
            "source": &source,
            "source_hits": &source_hits,
            "memory_context": &context,
            "evidence": &source_evidence,
        });

        Ok(PreparedResearchTask {
            task_uuid: task.uuid,
            correlation_uuid: task.correlation_uuid,
            agent: route.into(),
            skill,
            source,
            source_hits,
            context,
            research_record,
            evidence: source_evidence,
            model_context,
            prepared_at: Utc::now(),
        })
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Error)]
pub enum ResearchPipelineError {
    #[error(transparent)]
    Agent(#[from] AgentRuntimeError),
    #[error(transparent)]
    Skill(#[from] SkillError),
    #[error(transparent)]
    Source(#[from] SourceRegistryError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error(transparent)]
    Evidence(#[from] LedgerError),
    #[error(transparent)]
    LiveBus(#[from] LiveBusError),
    #[error("logical agent disappeared after routing: {0}")]
    MissingAgent(Uuid),
    #[error("agent {agent} is not allowed to use source {source}")]
    AgentSourceDenied { agent: String, source: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxclaw_agent_catalog::AgentCatalog;
    use phxclaw_memory_context::{DataClassification, InMemoryStore, MemoryRecord, MemoryScope};
    use std::fs;

    #[test]
    fn task_factory_uses_uuidv7() {
        let task = ResearchTask::rust("How do I use Result in Rust?", "phxclaw");
        assert_eq!((task.uuid.as_bytes()[6] >> 4), 7);
        assert_eq!((task.correlation_uuid.as_bytes()[6] >> 4), 7);
    }

    #[test]
    fn pipeline_prepares_offline_rust_context_end_to_end() {
        let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let catalog = AgentCatalog::load_dir(project_root.join("config/agents")).unwrap();
        let mut runtime = LogicalAgentRuntime::from_catalog(catalog).unwrap();
        runtime.start_all();
        let resolver = LazySkillResolver::load(project_root.join("config/skills/registry.index.json")).unwrap();
        let sources = SourceRegistry::load_dir(project_root.join("tests/fixtures/source-registry")).unwrap();
        let ledger_path = std::env::temp_dir().join(format!("phoenix-research-{}.jsonl", new_uuid_v7()));
        let ledger = EvidenceLedger::open(&ledger_path).unwrap();
        let live_bus = LiveEventHub::new(64, 64);
        let bus_probe = live_bus.clone();
        let pipeline = ResearchPipeline::new(
            &project_root,
            runtime,
            resolver,
            SkillResolutionPolicy {
                require_promoted: false,
                allow_validated: true,
            },
            sources,
            live_bus,
            ledger.clone(),
        );
        let mut memory = InMemoryStore::default();
        memory.put(MemoryRecord::new(
            "rust",
            "error-handling",
            json!({"note": "Use Result for recoverable errors and ? to propagate them"}),
            MemoryScope::Project("phxclaw".into()),
            DataClassification::Internal,
            vec![],
        ).unwrap()).unwrap();

        let task = ResearchTask::rust("How do I propagate Result errors in Rust?", "phxclaw");
        let prepared = pipeline.prepare(&memory, &task).unwrap();
        assert_eq!(prepared.agent.agent_name, "Research Agent");
        assert_eq!(prepared.skill.skill.name, "rust.research.official");
        assert!(!prepared.source_hits.is_empty());
        assert!(prepared.source_hits[0].title.contains("Result"));
        assert_eq!(prepared.context.items.len(), 1);
        assert!(prepared.evidence.len() >= 2);
        assert!(ledger.verify().unwrap().valid);
        let events = bus_probe.snapshot(64).unwrap();
        assert!(events.iter().any(|event| event.topic == "research.pipeline" && event.event_type == "ready"));
        assert!(events.iter().all(|event| event.correlation_uuid == Some(task.correlation_uuid)));
        let _ = fs::remove_file(ledger_path);
    }
}
