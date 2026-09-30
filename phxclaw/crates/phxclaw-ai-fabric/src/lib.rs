use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum DataClassification { Public, Internal, Confidential, Restricted }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all="snake_case")]
pub enum ModelCapability { Text, Tools, StructuredOutput, Vision, Embeddings, Reasoning, Audio }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum ModelLocation { Local, Cloud }
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum CircuitState { Closed, Open, HalfOpen }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelRecord {
    pub tenant_uuid: Uuid,
    pub provider_uuid: Uuid,
    pub provider_name: String,
    pub provider_priority: u16,
    pub model_id: String,
    pub location: ModelLocation,
    pub capabilities: BTreeSet<ModelCapability>,
    pub context_tokens: u64,
    pub reasoning_tier: u8,
    pub input_cost_micro_usd_per_million: Option<u64>,
    pub output_cost_micro_usd_per_million: Option<u64>,
    pub p95_latency_ms: Option<u64>,
    pub health_basis_points: u16,
    pub data_controls: BTreeSet<String>,
    pub observed_at_unix: i64,
    pub catalog_ttl_seconds: u64,
    pub health_observed_at_unix: i64,
    pub health_ttl_seconds: u64,
    pub circuit: CircuitState,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoutingPolicy {
    pub restricted_local_only: bool,
    pub confidential_cloud_requires_explicit_allow: bool,
    pub default_allow_cloud: bool,
    pub prefer_local_default: bool,
    pub max_fallbacks: usize,
    pub fail_closed_when_cost_unknown: bool,
    pub reservation_required_for_cloud: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteRequest {
    pub tenant_uuid: Uuid,
    pub request_uuid: Uuid,
    pub classification: DataClassification,
    pub required_capabilities: BTreeSet<ModelCapability>,
    pub required_data_controls: BTreeSet<String>,
    pub min_context_tokens: u64,
    pub min_reasoning_tier: u8,
    pub estimated_input_tokens: u64,
    pub estimated_output_tokens: u64,
    pub max_cost_micro_usd: Option<u64>,
    pub max_p95_latency_ms: Option<u64>,
    pub allow_cloud: Option<bool>,
    pub prefer_local: Option<bool>,
    pub allow_fallback: bool,
    pub allowed_provider_uuids: BTreeSet<Uuid>,
    pub prompt_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateDecision {
    pub provider_uuid: Uuid,
    pub model_id: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub projected_cost_micro_usd: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteSelection {
    pub request_uuid: Uuid,
    pub selected_provider_uuid: Uuid,
    pub selected_model_id: String,
    pub fallback_chain: Vec<(Uuid,String)>,
    pub candidates: Vec<CandidateDecision>,
    pub decision_sha256: String,
}
#[derive(Error, Debug, PartialEq, Eq)]
pub enum FabricError {
    #[error("invalid prompt hash")] InvalidPromptHash,
    #[error("no eligible model")] NoEligibleModel,
    #[error("tenant mismatch")] TenantMismatch,
    #[error("budget required but projected cost is unknown")] UnknownCost,
}
fn sha256_hex(s:&str)->String { hex::encode(Sha256::digest(s.as_bytes())) }
fn valid_sha(s:&str)->bool { s.len()==64 && s.bytes().all(|b| b.is_ascii_hexdigit()) }
fn fresh(now:i64, observed:i64, ttl:u64)->bool { observed <= now && now.saturating_sub(observed) <= ttl as i64 }
fn cost(m:&ModelRecord, r:&RouteRequest)->Option<u64> {
    let i=m.input_cost_micro_usd_per_million? as u128 * r.estimated_input_tokens as u128;
    let o=m.output_cost_micro_usd_per_million? as u128 * r.estimated_output_tokens as u128;
    Some(((i+o+999_999)/1_000_000).min(u64::MAX as u128) as u64)
}
fn cloud_allowed(p:&RoutingPolicy,r:&RouteRequest)->bool {
    let explicit=r.allow_cloud.unwrap_or(p.default_allow_cloud);
    if r.classification==DataClassification::Restricted && p.restricted_local_only { return false; }
    if r.classification==DataClassification::Confidential && p.confidential_cloud_requires_explicit_allow { return r.allow_cloud==Some(true); }
    explicit
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key { privacy:u8, cost:u64, latency:u64, health_penalty:u16, priority:u16, provider:String, model:String }
impl Ord for Key { fn cmp(&self,o:&Self)->Ordering { (self.privacy,self.cost,self.latency,self.health_penalty,self.priority,&self.provider,&self.model).cmp(&(o.privacy,o.cost,o.latency,o.health_penalty,o.priority,&o.provider,&o.model)) } }
impl PartialOrd for Key { fn partial_cmp(&self,o:&Self)->Option<Ordering>{Some(self.cmp(o))} }

pub fn route(now_unix:i64, policy:&RoutingPolicy, req:&RouteRequest, models:&[ModelRecord])->Result<RouteSelection,FabricError>{
    if !valid_sha(&req.prompt_sha256) { return Err(FabricError::InvalidPromptHash); }
    let prefer_local=req.prefer_local.unwrap_or(policy.prefer_local_default);
    let can_cloud=cloud_allowed(policy,req);
    let mut decisions=Vec::new(); let mut eligible:Vec<(Key,&ModelRecord,Option<u64>)>=Vec::new();
    for m in models {
        let mut reasons=Vec::new();
        if m.tenant_uuid!=req.tenant_uuid { reasons.push("tenant_mismatch".into()); }
        if !req.allowed_provider_uuids.is_empty() && !req.allowed_provider_uuids.contains(&m.provider_uuid) { reasons.push("provider_not_allowlisted".into()); }
        if m.location==ModelLocation::Cloud && !can_cloud { reasons.push("cloud_disallowed_by_privacy_policy".into()); }
        if !fresh(now_unix,m.observed_at_unix,m.catalog_ttl_seconds) { reasons.push("stale_catalog".into()); }
        if !fresh(now_unix,m.health_observed_at_unix,m.health_ttl_seconds) { reasons.push("stale_health".into()); }
        if m.circuit!=CircuitState::Closed { reasons.push("circuit_not_closed".into()); }
        if !req.required_capabilities.is_subset(&m.capabilities) { reasons.push("missing_capability".into()); }
        if !req.required_data_controls.is_subset(&m.data_controls) { reasons.push("missing_data_control".into()); }
        if m.context_tokens<req.min_context_tokens { reasons.push("context_too_small".into()); }
        if m.reasoning_tier<req.min_reasoning_tier { reasons.push("reasoning_tier_too_low".into()); }
        if let Some(mx)=req.max_p95_latency_ms { if m.p95_latency_ms.map_or(true,|v|v>mx){reasons.push("latency_limit".into());} }
        let projected=cost(m,req);
        if m.location==ModelLocation::Cloud && policy.reservation_required_for_cloud && policy.fail_closed_when_cost_unknown && projected.is_none(){ reasons.push("unknown_cost".into()); }
        if let (Some(mx),Some(c))=(req.max_cost_micro_usd,projected){ if c>mx{reasons.push("budget_limit".into());} }
        if req.max_cost_micro_usd.is_some() && projected.is_none() && policy.fail_closed_when_cost_unknown { reasons.push("unknown_cost".into()); }
        reasons.sort(); reasons.dedup();
        let ok=reasons.is_empty();
        decisions.push(CandidateDecision{provider_uuid:m.provider_uuid,model_id:m.model_id.clone(),eligible:ok,reasons:reasons.clone(),projected_cost_micro_usd:projected});
        if ok {
            let privacy=if prefer_local && m.location==ModelLocation::Cloud {1}else{0};
            let key=Key{privacy,cost:projected.unwrap_or(u64::MAX/4),latency:m.p95_latency_ms.unwrap_or(u64::MAX/4),health_penalty:10_000u16.saturating_sub(m.health_basis_points),priority:m.provider_priority,provider:m.provider_uuid.to_string(),model:m.model_id.clone()};
            eligible.push((key,m,projected));
        }
    }
    eligible.sort_by(|a,b|a.0.cmp(&b.0));
    decisions.sort_by(|a,b| (a.provider_uuid.to_string(), &a.model_id).cmp(&(b.provider_uuid.to_string(), &b.model_id)));
    let selected=eligible.first().ok_or(FabricError::NoEligibleModel)?.1;
    let mut fallback=Vec::new();
    if req.allow_fallback { for (_,m,_) in eligible.iter().skip(1).take(policy.max_fallbacks) { fallback.push((m.provider_uuid,m.model_id.clone())); } }
    let canonical=serde_json::to_string(&(req.request_uuid,selected.provider_uuid,&selected.model_id,&fallback,&decisions)).expect("serializable route decision");
    Ok(RouteSelection{request_uuid:req.request_uuid,selected_provider_uuid:selected.provider_uuid,selected_model_id:selected.model_id.clone(),fallback_chain:fallback,candidates:decisions,decision_sha256:sha256_hex(&canonical)})
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetReservation { pub tenant_uuid:Uuid, pub request_uuid:Uuid, pub reserved_micro_usd:u64, pub settled_micro_usd:Option<u64> }
pub fn budget_delta(r:&BudgetReservation)->i128 { r.reserved_micro_usd as i128-r.settled_micro_usd.unwrap_or(r.reserved_micro_usd) as i128 }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitObservation { pub success:bool, pub consecutive_failures:u32, pub opened_at_unix:Option<i64> }
pub fn next_circuit(now:i64, threshold:u32, cooldown:u64, o:&CircuitObservation)->CircuitState {
    if let Some(t)=o.opened_at_unix { if now.saturating_sub(t) >= cooldown as i64 { return CircuitState::HalfOpen; } return CircuitState::Open; }
    if !o.success && o.consecutive_failures>=threshold { CircuitState::Open } else { CircuitState::Closed }
}

pub fn evidence_projection(req:&RouteRequest, sel:&RouteSelection)->BTreeMap<String,String>{
    let mut m=BTreeMap::new(); m.insert("request_uuid".into(),req.request_uuid.to_string()); m.insert("prompt_sha256".into(),req.prompt_sha256.clone()); m.insert("decision_sha256".into(),sel.decision_sha256.clone()); m.insert("provider_uuid".into(),sel.selected_provider_uuid.to_string()); m.insert("model_id".into(),sel.selected_model_id.clone()); m
}
