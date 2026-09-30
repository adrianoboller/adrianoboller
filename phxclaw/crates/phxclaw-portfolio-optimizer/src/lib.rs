//! PhxClaw v0.51 — Portfolio Optimizer & Scenario Search.
//! Read-only search over v0.50 Digital Twin. Recommendations are delegated to v0.49.
use phxclaw_portfolio_digital_twin::{simulate, PortfolioTwin, PortfolioSimulationResult, ScenarioSpec, Strategy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const DIRECT_MUTATION_ALLOWED: bool = false;
pub const DELEGATE_TO: &str = "executive_decision_center";
pub const MAX_CANDIDATES_HARD: usize = 25_000;
pub const MAX_FINAL_ITERATIONS_HARD: u32 = 100_000;

#[derive(Debug, Error)]
pub enum OptimizeError {
    #[error("invalid search space")]
    InvalidSearchSpace,
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("hard constraint violated")]
    HardConstraint,
    #[error("explicit objective weights are required for a single winner")]
    WeightsRequired,
    #[error("search budget exceeded")]
    SearchBudgetExceeded,
    #[error("recommendation cannot mutate project state directly")]
    DirectMutationForbidden,
    #[error("insufficient confidence or robustness")]
    InsufficientConfidence,
    #[error("digital twin simulation failed")]
    Simulation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RangeSpec { pub min:f64, pub max:f64, pub step:f64 }
impl RangeSpec {
    pub fn validate(&self)->Result<(),OptimizeError>{
        if self.min.is_finite()&&self.max.is_finite()&&self.step.is_finite()&&self.step>0.0&&self.min<=self.max {Ok(())} else {Err(OptimizeError::InvalidSearchSpace)}
    }
    pub fn values(&self)->Result<Vec<f64>,OptimizeError>{
        self.validate()?; let n=(((self.max-self.min)/self.step).floor() as usize).saturating_add(1); if n>1000{return Err(OptimizeError::SearchBudgetExceeded)}
        Ok((0..n).map(|i|(self.min+self.step*i as f64).min(self.max)).collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSpace {
    pub agent_capacity_multiplier: RangeSpec,
    pub model_quota_multiplier: RangeSpec,
    pub duration_assumption_multiplier: RangeSpec,
    pub cost_assumption_multiplier: RangeSpec,
    pub strategies: Vec<Strategy>,
    pub max_candidates: usize,
}
impl SearchSpace {
    pub fn validate(&self)->Result<(),OptimizeError>{
        self.agent_capacity_multiplier.validate()?; self.model_quota_multiplier.validate()?; self.duration_assumption_multiplier.validate()?; self.cost_assumption_multiplier.validate()?;
        if self.strategies.is_empty() || self.max_candidates==0 || self.max_candidates>MAX_CANDIDATES_HARD {return Err(OptimizeError::SearchBudgetExceeded)}
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HardConstraints {
    pub min_all_deadlines_probability: f64,
    pub max_budget_overrun_probability: f64,
    pub max_contention_probability: f64,
    pub max_cloud_share: f64,
    pub allow_budget_increase_without_approval: bool,
}
impl HardConstraints {
    pub fn validate(&self)->Result<(),OptimizeError>{
        let ps=[self.min_all_deadlines_probability,self.max_budget_overrun_probability,self.max_contention_probability,self.max_cloud_share];
        if ps.iter().all(|x|x.is_finite()&&(0.0..=1.0).contains(x)) && !self.allow_budget_increase_without_approval {Ok(())} else {Err(OptimizeError::HardConstraint)}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectiveWeights {
    pub cost_p90: f64,
    pub deadline_probability: f64,
    pub budget_overrun_probability: f64,
    pub contention_probability: f64,
    pub cloud_share: f64,
}
impl ObjectiveWeights {
    pub fn validate(&self)->Result<(),OptimizeError>{
        let xs=[self.cost_p90,self.deadline_probability,self.budget_overrun_probability,self.contention_probability,self.cloud_share];
        let sum: f64=xs.iter().sum();
        if xs.iter().all(|x|x.is_finite()&&*x>=0.0) && sum>0.0 {Ok(())} else {Err(OptimizeError::InvalidSearchSpace)}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateSpec {
    pub candidate_uuid: Uuid,
    pub strategy: Strategy,
    pub agent_capacity_multiplier:f64,
    pub model_quota_multiplier:f64,
    pub duration_assumption_multiplier:f64,
    pub cost_assumption_multiplier:f64,
    pub source_state_sha256:String,
}
impl CandidateSpec {
    pub fn canonical_hash(&self)->String{canonical_sha256(self)}
    pub fn to_scenario(&self,twin_uuid:Uuid,iterations:u32)->ScenarioSpec{
        let mut assumptions=BTreeMap::new(); assumptions.insert("agent_capacity_multiplier".into(),fmt(self.agent_capacity_multiplier)); assumptions.insert("model_quota_multiplier".into(),fmt(self.model_quota_multiplier)); assumptions.insert("duration_multiplier".into(),fmt(self.duration_assumption_multiplier)); assumptions.insert("cost_multiplier".into(),fmt(self.cost_assumption_multiplier)); assumptions.insert("human_assumption_ack".into(),"true".into());
        ScenarioSpec{scenario_uuid:self.candidate_uuid,twin_uuid,source_state_sha256:self.source_state_sha256.clone(),iterations:iterations.min(MAX_FINAL_ITERATIONS_HARD),seed:0,strategy:self.strategy.clone(),assumptions}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateMetrics {
    pub cost_p90:f64,
    pub all_deadlines_probability:f64,
    pub worst_project_budget_overrun_probability:f64,
    pub contention_probability:f64,
    pub cloud_share:f64,
}
impl CandidateMetrics {
    pub fn from_result(r:&PortfolioSimulationResult,cloud_share:f64)->Self{
        let worst=r.projects.iter().map(|p|p.budget_overrun_probability).fold(0.0_f64,f64::max);
        Self{cost_p90:r.portfolio_cost.p90,all_deadlines_probability:r.all_deadlines_probability,worst_project_budget_overrun_probability:worst,contention_probability:r.shared_resource_contention_probability,cloud_share}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluatedCandidate {
    pub candidate:CandidateSpec,
    pub candidate_hash:String,
    pub result:PortfolioSimulationResult,
    pub metrics:CandidateMetrics,
    pub feasible:bool,
    pub utility_score:Option<f64>,
    pub evidence_sha256:String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPlan {
    pub search_uuid:Uuid,
    pub twin_uuid:Uuid,
    pub source_state_sha256:String,
    pub search_space:SearchSpace,
    pub constraints:HardConstraints,
    pub objective_weights:Option<ObjectiveWeights>,
    pub coarse_iterations:u32,
    pub refine_iterations:u32,
    pub final_iterations:u32,
    pub refine_top_k:usize,
    pub final_top_k:usize,
    pub fruitful_seed_hashes:Vec<String>,
    pub excluded_unfruitful_hashes:Vec<String>,
}
impl SearchPlan {
    pub fn validate(&self,twin:&PortfolioTwin)->Result<(),OptimizeError>{
        if self.source_state_sha256!=twin.source_state_sha256||self.twin_uuid!=twin.twin_uuid{return Err(OptimizeError::SourceStateMismatch)}
        self.search_space.validate()?; self.constraints.validate()?;
        if let Some(w)=&self.objective_weights{w.validate()?}
        if self.coarse_iterations<100||self.refine_iterations<self.coarse_iterations||self.final_iterations<self.refine_iterations||self.final_iterations>MAX_FINAL_ITERATIONS_HARD{return Err(OptimizeError::SearchBudgetExceeded)}
        if self.refine_top_k==0||self.final_top_k==0||self.final_top_k>self.refine_top_k{return Err(OptimizeError::SearchBudgetExceeded)}
        Ok(())
    }
    pub fn plan_hash(&self)->String{canonical_sha256(self)}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParetoPoint { pub candidate_hash:String, pub metrics:CandidateMetrics }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub search_uuid:Uuid,
    pub plan_hash:String,
    pub source_state_sha256:String,
    pub generated_count:usize,
    pub coarse_evaluated_count:usize,
    pub refined_count:usize,
    pub final_evaluated_count:usize,
    pub feasible_count:usize,
    pub pareto_frontier:Vec<ParetoPoint>,
    pub final_candidates:Vec<EvaluatedCandidate>,
    pub recommended_candidate_hash:Option<String>,
    pub confidence:f64,
    pub rank_stability:f64,
    pub evidence_sha256:String,
}

fn fmt(x:f64)->String{format!("{x:.6}")}
fn quantized(x:f64)->i64{(x*1_000_000.0).round() as i64}
fn static_cloud_share(strategy:&Strategy)->f64{match strategy{Strategy::OllamaFirst=>0.10,Strategy::MinimizeCost=>0.25,Strategy::Balanced=>0.50,Strategy::ProtectDeadline=>0.70,Strategy::ReduceContention=>0.45,Strategy::PreservePlan=>0.50}}

pub fn generate_candidates(plan:&SearchPlan)->Result<Vec<CandidateSpec>,OptimizeError>{
    plan.search_space.validate()?;
    let a=plan.search_space.agent_capacity_multiplier.values()?; let m=plan.search_space.model_quota_multiplier.values()?; let d=plan.search_space.duration_assumption_multiplier.values()?; let c=plan.search_space.cost_assumption_multiplier.values()?;
    let mut out=Vec::new(); let mut seen=BTreeSet::new();
    'outer: for strategy in &plan.search_space.strategies { for av in &a { for mv in &m { for dv in &d { for cv in &c {
        let seed=(plan.search_uuid,&plan.source_state_sha256,strategy_tag(strategy),quantized(*av),quantized(*mv),quantized(*dv),quantized(*cv)); let seed_json=serde_json::to_vec(&seed).unwrap(); let mut h=Sha256::new();h.update(&seed_json);let digest=h.finalize();let mut bytes=[0u8;16];bytes.copy_from_slice(&digest[..16]);bytes[6]=(bytes[6]&0x0f)|0x70;bytes[8]=(bytes[8]&0x3f)|0x80;let id=Uuid::from_bytes(bytes);
        let x=CandidateSpec{candidate_uuid:id,strategy:strategy.clone(),agent_capacity_multiplier:*av,model_quota_multiplier:*mv,duration_assumption_multiplier:*dv,cost_assumption_multiplier:*cv,source_state_sha256:plan.source_state_sha256.clone()}; let ch=x.canonical_hash();
        if plan.excluded_unfruitful_hashes.contains(&ch){continue} if seen.insert(ch){out.push(x)} if out.len()>=plan.search_space.max_candidates{break 'outer}
    }}}}}
    // Promoted fruitful seeds are ordering hints only; they cannot bypass hard constraints.
    out.sort_by_key(|x|(!plan.fruitful_seed_hashes.contains(&x.canonical_hash()),x.canonical_hash())); Ok(out)
}

pub fn feasible(m:&CandidateMetrics,c:&HardConstraints)->bool{
    m.all_deadlines_probability>=c.min_all_deadlines_probability && m.worst_project_budget_overrun_probability<=c.max_budget_overrun_probability && m.contention_probability<=c.max_contention_probability && m.cloud_share<=c.max_cloud_share
}

fn dominates(a:&CandidateMetrics,b:&CandidateMetrics)->bool{
    let weak=a.cost_p90<=b.cost_p90 && a.all_deadlines_probability>=b.all_deadlines_probability && a.worst_project_budget_overrun_probability<=b.worst_project_budget_overrun_probability && a.contention_probability<=b.contention_probability && a.cloud_share<=b.cloud_share;
    let strict=a.cost_p90<b.cost_p90 || a.all_deadlines_probability>b.all_deadlines_probability || a.worst_project_budget_overrun_probability<b.worst_project_budget_overrun_probability || a.contention_probability<b.contention_probability || a.cloud_share<b.cloud_share; weak&&strict
}

pub fn pareto_frontier(xs:&[EvaluatedCandidate])->Vec<ParetoPoint>{
    let feasible_x:Vec<_>=xs.iter().filter(|x|x.feasible).collect(); let mut out=Vec::new();
    for x in &feasible_x { if !feasible_x.iter().any(|y|y.candidate_hash!=x.candidate_hash&&dominates(&y.metrics,&x.metrics)){out.push(ParetoPoint{candidate_hash:x.candidate_hash.clone(),metrics:x.metrics.clone()})} }
    out.sort_by(|a,b|a.metrics.cost_p90.total_cmp(&b.metrics.cost_p90).then_with(||b.metrics.all_deadlines_probability.total_cmp(&a.metrics.all_deadlines_probability)).then_with(||a.candidate_hash.cmp(&b.candidate_hash))); out
}

pub fn utility(m:&CandidateMetrics,w:&ObjectiveWeights,cost_reference:f64)->f64{
    let cr=cost_reference.max(1e-9); let cost=(m.cost_p90/cr).max(0.0); let deadline_penalty=1.0-m.all_deadlines_probability; w.cost_p90*cost + w.deadline_probability*deadline_penalty + w.budget_overrun_probability*m.worst_project_budget_overrun_probability + w.contention_probability*m.contention_probability + w.cloud_share*m.cloud_share
}

pub fn evaluate_candidate(twin:&PortfolioTwin,plan:&SearchPlan,candidate:CandidateSpec,iterations:u32,cost_reference:f64)->Result<EvaluatedCandidate,OptimizeError>{
    if candidate.source_state_sha256!=plan.source_state_sha256{return Err(OptimizeError::SourceStateMismatch)}
    let scenario=candidate.to_scenario(twin.twin_uuid,iterations); let result=simulate(twin,&scenario).map_err(|_|OptimizeError::Simulation)?; let metrics=CandidateMetrics::from_result(&result,static_cloud_share(&candidate.strategy)); let ok=feasible(&metrics,&plan.constraints); let utility_score=plan.objective_weights.as_ref().map(|w|utility(&metrics,w,cost_reference)); let candidate_hash=candidate.canonical_hash();
    #[derive(Serialize)] struct Ev<'a>{candidate_hash:&'a str,metrics:&'a CandidateMetrics,result_hash:&'a str,feasible:bool,utility_score:Option<f64>}
    let evidence_sha256=canonical_sha256(&Ev{candidate_hash:&candidate_hash,metrics:&metrics,result_hash:&result.evidence_sha256,feasible:ok,utility_score});
    Ok(EvaluatedCandidate{candidate,candidate_hash,result,metrics,feasible:ok,utility_score,evidence_sha256})
}

pub fn deterministic_select_for_refine(mut xs:Vec<EvaluatedCandidate>,k:usize,weighted:bool)->Vec<EvaluatedCandidate>{
    xs.retain(|x|x.feasible);
    if weighted {
        xs.sort_by(|a,b|a.utility_score.unwrap_or(f64::INFINITY).total_cmp(&b.utility_score.unwrap_or(f64::INFINITY)).then_with(||a.candidate_hash.cmp(&b.candidate_hash)));
        xs.truncate(k); return xs;
    }
    // No hidden objective weights: preserve Pareto diversity first, then fill deterministically by hash.
    let front=pareto_frontier(&xs); let front_hashes:BTreeSet<_>=front.iter().map(|p|p.candidate_hash.clone()).collect();
    let mut front_candidates:Vec<_>=xs.iter().filter(|x|front_hashes.contains(&x.candidate_hash)).cloned().collect();
    front_candidates.sort_by(|a,b|a.metrics.cost_p90.total_cmp(&b.metrics.cost_p90).then_with(||a.candidate_hash.cmp(&b.candidate_hash)));
    let mut selected=Vec::new();
    if front_candidates.len()>k && k>0 {
        for i in 0..k { let idx=if k==1{0}else{i*(front_candidates.len()-1)/(k-1)}; selected.push(front_candidates[idx].clone()); }
        return selected;
    }
    selected.extend(front_candidates); let chosen:BTreeSet<_>=selected.iter().map(|x|x.candidate_hash.clone()).collect();
    let mut rest:Vec<_>=xs.into_iter().filter(|x|!chosen.contains(&x.candidate_hash)).collect(); rest.sort_by(|a,b|a.candidate_hash.cmp(&b.candidate_hash));
    for x in rest { if selected.len()>=k{break} selected.push(x); } selected
}

pub fn weight_rank_stability(xs:&[EvaluatedCandidate],w:&ObjectiveWeights,cost_reference:f64,winner_hash:&str)->f64{
    let factors=[0.90,0.95,1.0,1.05,1.10]; let mut stable=0usize; let mut total=0usize;
    for f in factors { for axis in 0..5 { let mut pw=w.clone(); match axis {0=>pw.cost_p90*=f,1=>pw.deadline_probability*=f,2=>pw.budget_overrun_probability*=f,3=>pw.contention_probability*=f,_=>pw.cloud_share*=f}; if pw.validate().is_err(){continue} total+=1; let best=xs.iter().filter(|x|x.feasible).min_by(|a,b|utility(&a.metrics,&pw,cost_reference).total_cmp(&utility(&b.metrics,&pw,cost_reference)).then_with(||a.candidate_hash.cmp(&b.candidate_hash))); if best.map(|x|x.candidate_hash.as_str())==Some(winner_hash){stable+=1} } }
    if total==0{0.0}else{stable as f64/total as f64}
}

pub fn optimize_coarse_to_fine(twin:&PortfolioTwin,plan:&SearchPlan,cost_reference:f64)->Result<SearchResult,OptimizeError>{
    plan.validate(twin)?; let generated=generate_candidates(plan)?; let generated_count=generated.len();
    let mut coarse=Vec::with_capacity(generated_count); for c in generated { coarse.push(evaluate_candidate(twin,plan,c,plan.coarse_iterations,cost_reference)?); } let coarse_count=coarse.len();
    let weighted=plan.objective_weights.is_some(); let refine_specs=deterministic_select_for_refine(coarse,plan.refine_top_k,weighted).into_iter().map(|x|x.candidate).collect::<Vec<_>>(); let refined_count=refine_specs.len();
    let mut refined=Vec::with_capacity(refined_count); for c in refine_specs { refined.push(evaluate_candidate(twin,plan,c,plan.refine_iterations,cost_reference)?); }
    let final_specs=deterministic_select_for_refine(refined,plan.final_top_k,weighted).into_iter().map(|x|x.candidate).collect::<Vec<_>>();
    let mut finals=Vec::with_capacity(final_specs.len()); for c in final_specs { finals.push(evaluate_candidate(twin,plan,c,plan.final_iterations,cost_reference)?); } let final_count=finals.len();
    let recommended=choose_single_winner(&finals,plan.objective_weights.as_ref())?;
    let stability=match (&plan.objective_weights,&recommended){(Some(w),Some(h))=>weight_rank_stability(&finals,w,cost_reference,h),_=>1.0};
    make_result(plan,finals,stability,generated_count,coarse_count,refined_count,final_count)
}

pub fn choose_single_winner(xs:&[EvaluatedCandidate],weights:Option<&ObjectiveWeights>)->Result<Option<String>,OptimizeError>{
    if xs.is_empty(){return Ok(None)} let _=weights.ok_or(OptimizeError::WeightsRequired)?; let mut f:Vec<_>=xs.iter().filter(|x|x.feasible&&x.utility_score.is_some()).collect(); if f.is_empty(){return Ok(None)} f.sort_by(|a,b|a.utility_score.unwrap().total_cmp(&b.utility_score.unwrap()).then_with(||a.candidate_hash.cmp(&b.candidate_hash))); Ok(Some(f[0].candidate_hash.clone()))
}

pub fn rank_stability(base:&[EvaluatedCandidate],perturbed:&[Vec<EvaluatedCandidate>],winner_hash:&str)->f64{
    if perturbed.is_empty(){return 0.0} let mut stable=0usize; for run in perturbed { if let Some(best)=run.iter().filter(|x|x.feasible&&x.utility_score.is_some()).min_by(|a,b|a.utility_score.unwrap().total_cmp(&b.utility_score.unwrap()).then_with(||a.candidate_hash.cmp(&b.candidate_hash))) { if best.candidate_hash==winner_hash{stable+=1} } } let _=base; stable as f64/perturbed.len() as f64
}

pub fn make_result(plan:&SearchPlan,evaluated:Vec<EvaluatedCandidate>,rank_stability:f64,generated_count:usize,coarse_evaluated_count:usize,refined_count:usize,final_evaluated_count:usize)->Result<SearchResult,OptimizeError>{
    let feasible_count=evaluated.iter().filter(|x|x.feasible).count(); let pareto=pareto_frontier(&evaluated); let recommended=choose_single_winner(&evaluated,plan.objective_weights.as_ref())?; let confidence=((feasible_count as f64)/(evaluated.len().max(1) as f64)).sqrt()*rank_stability.clamp(0.0,1.0);
    if recommended.is_some() && (rank_stability<0.75||confidence<0.70){return Err(OptimizeError::InsufficientConfidence)}
    let mut finals=evaluated; finals.sort_by(|a,b|a.utility_score.unwrap_or(f64::INFINITY).total_cmp(&b.utility_score.unwrap_or(f64::INFINITY)).then_with(||a.candidate_hash.cmp(&b.candidate_hash)));
    let mut out=SearchResult{search_uuid:plan.search_uuid,plan_hash:plan.plan_hash(),source_state_sha256:plan.source_state_sha256.clone(),generated_count,coarse_evaluated_count,refined_count,final_evaluated_count,feasible_count,pareto_frontier:pareto,final_candidates:finals,recommended_candidate_hash:recommended,confidence,rank_stability,evidence_sha256:String::new()}; out.evidence_sha256=stable_result_hash(&out); Ok(out)
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DecisionDelegation { pub decision_case_uuid:Uuid,pub search_uuid:Uuid,pub recommendation_hash:String,pub evidence_sha256:String,pub delegate_to:String,pub direct_mutation:bool }
pub fn delegate_recommendation(result:&SearchResult)->Result<DecisionDelegation,OptimizeError>{
    let rh=result.recommended_candidate_hash.clone().ok_or(OptimizeError::WeightsRequired)?; Ok(DecisionDelegation{decision_case_uuid:Uuid::now_v7(),search_uuid:result.search_uuid,recommendation_hash:rh,evidence_sha256:result.evidence_sha256.clone(),delegate_to:DELEGATE_TO.into(),direct_mutation:false})
}
pub fn assert_no_direct_mutation(d:&DecisionDelegation)->Result<(),OptimizeError>{if d.direct_mutation{Err(OptimizeError::DirectMutationForbidden)}else{Ok(())}}

fn strategy_tag(s:&Strategy)->u8{match s{Strategy::PreservePlan=>0,Strategy::MinimizeCost=>1,Strategy::ProtectDeadline=>2,Strategy::ReduceContention=>3,Strategy::OllamaFirst=>4,Strategy::Balanced=>5}}
fn stable_result_hash(v:&SearchResult)->String{ #[derive(Serialize)] struct Stable<'a>{search_uuid:Uuid,plan_hash:&'a str,source_state_sha256:&'a str,generated_count:usize,coarse_evaluated_count:usize,refined_count:usize,final_evaluated_count:usize,feasible_count:usize,pareto_frontier:&'a [ParetoPoint],final_candidates:&'a [EvaluatedCandidate],recommended_candidate_hash:&'a Option<String>,confidence:f64,rank_stability:f64} canonical_sha256(&Stable{search_uuid:v.search_uuid,plan_hash:&v.plan_hash,source_state_sha256:&v.source_state_sha256,generated_count:v.generated_count,coarse_evaluated_count:v.coarse_evaluated_count,refined_count:v.refined_count,final_evaluated_count:v.final_evaluated_count,feasible_count:v.feasible_count,pareto_frontier:&v.pareto_frontier,final_candidates:&v.final_candidates,recommended_candidate_hash:&v.recommended_candidate_hash,confidence:v.confidence,rank_stability:v.rank_stability}) }
fn canonical_sha256<T:Serialize>(v:&T)->String{let b=serde_json::to_vec(v).expect("serializable");let mut h=Sha256::new();h.update(b);format!("{:x}",h.finalize())}
