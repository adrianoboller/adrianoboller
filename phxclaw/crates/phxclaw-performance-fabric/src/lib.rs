use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    Low,
    Medium,
    High,
    Extreme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTier {
    Deterministic,
    OllamaFast,
    OllamaSpecialized,
    CloudEconomic,
    CloudPremium,
    IndependentReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    Classifier,
    Coder,
    Reviewer,
    LogAnalyzer,
    Rag,
    Planner,
    Docs,
    Embeddings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDemand {
    pub task_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub task_class: String,
    pub complexity: TaskComplexity,
    pub data_class: DataClass,
    pub required_capabilities: BTreeSet<String>,
    pub estimated_input_tokens: u32,
    pub expected_output_tokens: u32,
    pub quality_floor: f64,
    pub max_latency_ms: Option<u64>,
    pub budget_remaining: Option<f64>,
    pub interactive: bool,
    pub source_state_sha256: String,
    pub policy_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub profile_uuid: Uuid,
    pub provider: String,
    pub model: String,
    pub role: ModelRole,
    pub local: bool,
    pub promoted: bool,
    pub fresh: bool,
    pub healthy: bool,
    pub supports_structured_output: bool,
    pub supports_tools: bool,
    pub supports_embeddings: bool,
    pub capabilities: BTreeSet<String>,
    pub quality_score: f64,
    pub p95_latency_ms: Option<u64>,
    pub estimated_cost: Option<f64>,
    pub max_context_tokens: u32,
    pub warm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityProfile {
    pub agent_uuid: Uuid,
    pub task_class: String,
    pub model_profile_uuid: Uuid,
    pub source_state_sha256: String,
    pub success_rate: f64,
    pub avg_quality: f64,
    pub avg_cost: f64,
    pub avg_latency_ms: f64,
    pub sample_count: u32,
    pub fresh: bool,
    pub promoted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGuard {
    pub model_profile_uuid: Uuid,
    pub context_fingerprint: String,
    pub promoted_failure: bool,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudgetDecision {
    pub budget_tokens: u32,
    pub requested_tokens: u32,
    pub model_limit_tokens: u32,
    pub compressed_locally: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPromptPlan {
    pub prompt_uuid: Uuid,
    pub provider: String,
    pub model: String,
    pub system: String,
    pub context_blocks: Vec<String>,
    pub constraints: Vec<String>,
    pub tools: Vec<String>,
    pub output_schema: serde_json::Value,
    pub source_state_sha256: String,
    pub policy_sha256: String,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCacheKey {
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub policy_sha256: String,
    pub task_fingerprint: String,
    pub model_profile_uuid: Uuid,
    pub prompt_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub tier: ExecutionTier,
    pub provider: String,
    pub model: String,
    pub model_profile_uuid: Uuid,
    pub context_budget: ContextBudgetDecision,
    pub keep_alive: Option<String>,
    pub cache_key_sha256: String,
    pub decision_sha256: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOutcome {
    pub outcome_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub project_uuid: Uuid,
    pub task_uuid: Uuid,
    pub agent_uuid: Uuid,
    pub task_class: String,
    pub model_profile_uuid: Uuid,
    pub success: bool,
    pub quality_score: f64,
    pub actual_cost: f64,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tests_passed: Option<u32>,
    pub tests_failed: Option<u32>,
    pub failure_signature: Option<String>,
    pub failure_origin: Option<String>,
    pub source_state_sha256: String,
    pub evidence_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaRuntimeSample {
    pub sample_uuid: Uuid,
    pub model: String,
    pub loaded: bool,
    pub size_vram: Option<u64>,
    pub queue_depth: Option<u32>,
    pub latency_ms: Option<u64>,
    pub tokens_per_second: Option<f64>,
    pub context_length: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmPoolPlan {
    pub keep_hot: Vec<String>,
    pub load_on_demand: Vec<String>,
    pub reasons: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum FabricError {
    #[error("no eligible model satisfies hard gates")]
    NoEligibleModel,
    #[error("restricted data requires a local model under current policy")]
    RestrictedRequiresLocal,
    #[error("cloud candidate has unknown cost while budget enforcement is active")]
    UnknownCloudCost,
    #[error("task exceeds model context budget")]
    ContextBudgetExceeded,
}

pub struct PerformanceFabric;

impl PerformanceFabric {
    pub fn context_budget(
        complexity: TaskComplexity,
        requested: u32,
        model_limit: u32,
    ) -> Result<ContextBudgetDecision, FabricError> {
        let configured = match complexity {
            TaskComplexity::Low => 4_096,
            TaskComplexity::Medium => 8_192,
            TaskComplexity::High => 16_384,
            TaskComplexity::Extreme => 32_768,
        };
        let budget = configured.min(model_limit);
        if requested > model_limit {
            return Err(FabricError::ContextBudgetExceeded);
        }
        Ok(ContextBudgetDecision {
            budget_tokens: budget,
            requested_tokens: requested,
            model_limit_tokens: model_limit,
            compressed_locally: requested > budget,
        })
    }

    pub fn route(
        demand: &TaskDemand,
        candidates: &[ModelCandidate],
        affinities: &[AffinityProfile],
        knowledge_guards: &[KnowledgeGuard],
        context_fingerprint: &str,
    ) -> Result<RoutingDecision, FabricError> {
        let mut eligible: Vec<&ModelCandidate> = candidates
            .iter()
            .filter(|c| {
                c.promoted
                    && c.fresh
                    && c.healthy
                    && c.supports_structured_output
                    && c.quality_score >= demand.quality_floor
                    && demand.required_capabilities.is_subset(&c.capabilities)
            })
            .collect();

        if matches!(demand.data_class, DataClass::Restricted) {
            eligible.retain(|c| c.local);
            if eligible.is_empty() {
                return Err(FabricError::RestrictedRequiresLocal);
            }
        }

        eligible.retain(|c| {
            !knowledge_guards.iter().any(|g| {
                g.model_profile_uuid == c.profile_uuid
                    && g.context_fingerprint == context_fingerprint
                    && g.promoted_failure
            })
        });

        if demand.budget_remaining.is_some()
            && eligible
                .iter()
                .any(|c| !c.local && c.estimated_cost.is_none())
        {
            eligible.retain(|c| c.local || c.estimated_cost.is_some());
        }
        if eligible.is_empty() {
            return Err(FabricError::NoEligibleModel);
        }

        let affinity = |c: &ModelCandidate| -> f64 {
            affinities
                .iter()
                .find(|a| {
                    a.agent_uuid == demand.agent_uuid
                        && a.task_class == demand.task_class
                        && a.model_profile_uuid == c.profile_uuid
                        && a.source_state_sha256 == demand.source_state_sha256
                        && a.fresh
                        && a.promoted
                        && a.sample_count >= 3
                })
                .map(|a| a.success_rate * 0.55 + a.avg_quality * 0.45)
                .unwrap_or(0.0)
        };

        eligible.sort_by(|a, b| {
            let tier = |c: &ModelCandidate| if c.local { 0u8 } else { 1u8 };
            tier(a)
                .cmp(&tier(b))
                .then_with(|| {
                    affinity(b)
                        .partial_cmp(&affinity(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a.estimated_cost
                        .unwrap_or(f64::INFINITY)
                        .partial_cmp(&b.estimated_cost.unwrap_or(f64::INFINITY))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a.p95_latency_ms
                        .unwrap_or(u64::MAX)
                        .cmp(&b.p95_latency_ms.unwrap_or(u64::MAX))
                })
                .then_with(|| a.model.cmp(&b.model))
        });
        let c = eligible[0];
        if let (Some(rem), Some(cost)) = (demand.budget_remaining, c.estimated_cost) {
            if cost > rem {
                return Err(FabricError::NoEligibleModel);
            }
        }
        let requested = demand
            .estimated_input_tokens
            .saturating_add(demand.expected_output_tokens);
        let budget = Self::context_budget(demand.complexity, requested, c.max_context_tokens)?;
        let tier = if c.local {
            if matches!(
                demand.complexity,
                TaskComplexity::Low | TaskComplexity::Medium
            ) {
                ExecutionTier::OllamaFast
            } else {
                ExecutionTier::OllamaSpecialized
            }
        } else if matches!(demand.complexity, TaskComplexity::Extreme) {
            ExecutionTier::CloudPremium
        } else {
            ExecutionTier::CloudEconomic
        };
        let cache_key = SemanticCacheKey {
            tenant_uuid: demand.tenant_uuid,
            project_uuid: demand.project_uuid,
            source_state_sha256: demand.source_state_sha256.clone(),
            policy_sha256: demand.policy_sha256.clone(),
            task_fingerprint: hash_json(&(
                demand.task_class.clone(),
                context_fingerprint,
                demand.required_capabilities.clone(),
            )),
            model_profile_uuid: c.profile_uuid,
            prompt_sha256: "pending_prompt_compiler".into(),
        };
        let cache_key_sha256 = hash_json(&cache_key);
        let mut reasons = vec!["hard gates satisfied".to_string()];
        if c.local {
            reasons.push(
                "verified local candidate preferred to reduce cloud cost and exposure".into(),
            );
        }
        if affinity(c) > 0.0 {
            reasons.push("fresh promoted agent-model affinity available".into());
        }
        if c.warm {
            reasons.push("model already warm".into());
        }
        let decision_sha256 = hash_json(&(
            demand.task_uuid,
            c.profile_uuid,
            &tier,
            &cache_key_sha256,
            &reasons,
        ));
        Ok(RoutingDecision {
            tier,
            provider: c.provider.clone(),
            model: c.model.clone(),
            model_profile_uuid: c.profile_uuid,
            context_budget: budget,
            keep_alive: if c.local { Some("5m".into()) } else { None },
            cache_key_sha256,
            decision_sha256,
            reasons,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn compile_prompt(
        provider: &str,
        model: &str,
        system: &str,
        context_blocks: Vec<String>,
        constraints: Vec<String>,
        tools: Vec<String>,
        output_schema: serde_json::Value,
        source_state_sha256: &str,
        policy_sha256: &str,
    ) -> CompiledPromptPlan {
        let payload = serde_json::json!({"provider":provider,"model":model,"system":system,"context_blocks":context_blocks,"constraints":constraints,"tools":tools,"output_schema":output_schema,"source_state_sha256":source_state_sha256,"policy_sha256":policy_sha256});
        let prompt_sha256 = hash_json(&payload);
        CompiledPromptPlan {
            prompt_uuid: Uuid::now_v7(),
            provider: provider.into(),
            model: model.into(),
            system: system.into(),
            context_blocks: payload["context_blocks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect(),
            constraints: payload["constraints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect(),
            tools: payload["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect(),
            output_schema: payload["output_schema"].clone(),
            source_state_sha256: source_state_sha256.into(),
            policy_sha256: policy_sha256.into(),
            prompt_sha256,
        }
    }

    pub fn warm_pool(
        samples: &[OllamaRuntimeSample],
        interactive_models: &BTreeSet<String>,
        memory_pressure: bool,
    ) -> WarmPoolPlan {
        let mut keep_hot = Vec::new();
        let mut load_on_demand = Vec::new();
        let mut reasons = BTreeMap::new();
        for s in samples {
            if !memory_pressure && interactive_models.contains(&s.model) {
                keep_hot.push(s.model.clone());
                reasons.insert(
                    s.model.clone(),
                    "interactive demand and no memory pressure".into(),
                );
            } else {
                load_on_demand.push(s.model.clone());
                reasons.insert(
                    s.model.clone(),
                    if memory_pressure {
                        "memory pressure".into()
                    } else {
                        "background/on-demand role".into()
                    },
                );
            }
        }
        keep_hot.sort();
        keep_hot.dedup();
        load_on_demand.sort();
        load_on_demand.dedup();
        WarmPoolPlan {
            keep_hot,
            load_on_demand,
            reasons,
        }
    }
}

pub fn hash_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable");
    format!("{:x}", Sha256::digest(bytes))
}
