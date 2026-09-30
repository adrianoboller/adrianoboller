//! PhxClaw v0.50 — Portfolio Digital Twin & Monte Carlo Planning.
//! Simulation is advisory/read-only. Real changes are delegated to v0.49/v0.47.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const DIRECT_MUTATION_ALLOWED: bool = false;
pub const DELEGATE_DECISIONS_TO: &str = "executive_decision_center";
pub const DELEGATE_EXECUTION_TO: &str = "autonomous_project_supervisor";

#[derive(Debug, Error)]
pub enum TwinError {
    #[error("invalid distribution bounds")]
    InvalidBounds,
    #[error("empirical distribution requires more samples")]
    InsufficientSamples,
    #[error("source state mismatch")]
    SourceStateMismatch,
    #[error("stale or unpromoted calibration evidence")]
    InvalidCalibration,
    #[error("cross-project dependency cycle")]
    DependencyCycle,
    #[error("hard portfolio constraint violated")]
    HardConstraint,
    #[error("simulation output cannot mutate project state directly")]
    DirectMutationForbidden,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag="kind", rename_all="snake_case")]
pub enum Distribution {
    Triangular { min: f64, mode: f64, max: f64 },
    Pert { min: f64, mode: f64, max: f64, lambda: f64 },
    Empirical { samples: Vec<f64> },
}
impl Distribution {
    pub fn validate(&self) -> Result<(),TwinError> {
        match self {
            Self::Triangular{min,mode,max} if *min <= *mode && *mode <= *max && min.is_finite() && mode.is_finite() && max.is_finite() => Ok(()),
            Self::Pert{min,mode,max,lambda} if *min <= *mode && *mode <= *max && min.is_finite() && mode.is_finite() && max.is_finite() && lambda.is_finite() && *lambda > 0.0 => Ok(()),
            Self::Empirical{samples} if samples.len() >= 12 && samples.iter().all(|x| x.is_finite()) => Ok(()),
            Self::Empirical{..} => Err(TwinError::InsufficientSamples),
            _ => Err(TwinError::InvalidBounds),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationEvidence {
    pub evidence_uuid: Uuid,
    pub source_state_sha256: String,
    pub promoted: bool,
    pub fresh: bool,
    pub sample_count: u32,
    pub evidence_sha256: String,
}
impl CalibrationEvidence {
    pub fn validate(&self, source_state: &str) -> Result<(),TwinError> {
        if self.source_state_sha256 != source_state { return Err(TwinError::SourceStateMismatch); }
        if !self.promoted || !self.fresh || self.sample_count == 0 { return Err(TwinError::InvalidCalibration); }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DependencyType { FS, SS, FF, SF }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectDependency {
    pub from_project_uuid: Uuid,
    pub to_project_uuid: Uuid,
    pub dependency_type: DependencyType,
    pub lag_days: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectTwin {
    pub project_uuid: Uuid,
    pub source_state_sha256: String,
    pub planned_cost: f64,
    pub hard_budget: f64,
    pub deadline_remaining_days: f64,
    pub remaining_duration_days: Distribution,
    pub remaining_cost: Distribution,
    pub rework_fraction: Distribution,
    pub required_agent_hours: BTreeMap<Uuid, Distribution>,
    pub required_model_units: BTreeMap<String, Distribution>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedResource {
    pub resource_uuid: Uuid,
    pub capacity_per_day: f64,
    pub calendar_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioTwin {
    pub twin_uuid: Uuid,
    pub tenant_uuid: Uuid,
    pub source_state_sha256: String,
    pub projects: Vec<ProjectTwin>,
    pub dependencies: Vec<ProjectDependency>,
    pub shared_resources: Vec<SharedResource>,
    pub model_quotas_per_day: BTreeMap<String, f64>,
    pub promoted_calibrations: Vec<CalibrationEvidence>,
}
impl PortfolioTwin {
    pub fn validate(&self) -> Result<(),TwinError> {
        for r in &self.shared_resources { if !r.capacity_per_day.is_finite() || r.capacity_per_day <= 0.0 { return Err(TwinError::HardConstraint); } }
        for q in self.model_quotas_per_day.values() { if !q.is_finite() || *q <= 0.0 { return Err(TwinError::HardConstraint); } }
        let resource_ids:BTreeSet<_>=self.shared_resources.iter().map(|r|r.resource_uuid).collect();
        for p in &self.projects {
            if p.source_state_sha256 != self.source_state_sha256 { return Err(TwinError::SourceStateMismatch); }
            if !p.deadline_remaining_days.is_finite() || p.deadline_remaining_days < 0.0 || !p.hard_budget.is_finite() || p.hard_budget < 0.0 { return Err(TwinError::HardConstraint); }
            p.remaining_duration_days.validate()?; p.remaining_cost.validate()?; p.rework_fraction.validate()?;
            for (agent,d) in &p.required_agent_hours { if !resource_ids.contains(agent) { return Err(TwinError::HardConstraint); } d.validate()?; }
            for (model,d) in &p.required_model_units { if !self.model_quotas_per_day.contains_key(model) { return Err(TwinError::HardConstraint); } d.validate()?; }
        }
        for e in &self.promoted_calibrations { e.validate(&self.source_state_sha256)?; }
        self.validate_dependency_dag()?;
        Ok(())
    }
    pub fn validate_dependency_dag(&self) -> Result<(),TwinError> {
        let ids:BTreeSet<_>=self.projects.iter().map(|p|p.project_uuid).collect();
        let mut indeg:BTreeMap<Uuid,usize>=ids.iter().map(|x|(*x,0)).collect();
        let mut adj:BTreeMap<Uuid,Vec<Uuid>>=BTreeMap::new();
        for d in &self.dependencies {
            if !ids.contains(&d.from_project_uuid)||!ids.contains(&d.to_project_uuid){return Err(TwinError::HardConstraint)}
            adj.entry(d.from_project_uuid).or_default().push(d.to_project_uuid);
            *indeg.entry(d.to_project_uuid).or_default()+=1;
        }
        let mut q:Vec<Uuid>=indeg.iter().filter_map(|(k,v)|(*v==0).then_some(*k)).collect();let mut n=0;
        while let Some(x)=q.pop(){n+=1;if let Some(v)=adj.get(&x){for y in v{let e=indeg.get_mut(y).unwrap();*e-=1;if *e==0{q.push(*y)}}}}
        if n!=ids.len(){Err(TwinError::DependencyCycle)}else{Ok(())}
    }
    pub fn topological_order(&self) -> Result<Vec<Uuid>,TwinError> {
        let ids:BTreeSet<_>=self.projects.iter().map(|p|p.project_uuid).collect();
        let mut indeg:BTreeMap<Uuid,usize>=ids.iter().map(|x|(*x,0)).collect();
        let mut adj:BTreeMap<Uuid,Vec<Uuid>>=BTreeMap::new();
        for d in &self.dependencies { adj.entry(d.from_project_uuid).or_default().push(d.to_project_uuid); *indeg.entry(d.to_project_uuid).or_default()+=1; }
        for v in adj.values_mut(){v.sort();}
        let mut ready:BTreeSet<Uuid>=indeg.iter().filter_map(|(k,v)|(*v==0).then_some(*k)).collect(); let mut out=Vec::with_capacity(ids.len());
        while let Some(x)=ready.iter().next().copied(){ ready.remove(&x); out.push(x); if let Some(v)=adj.get(&x){ for y in v { let e=indeg.get_mut(y).unwrap(); *e-=1; if *e==0 { ready.insert(*y); } } } }
        if out.len()!=ids.len(){Err(TwinError::DependencyCycle)}else{Ok(out)}
    }
    pub fn canonical_hash(&self)->String { canonical_sha256(self) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Strategy { PreservePlan, MinimizeCost, ProtectDeadline, ReduceContention, OllamaFirst, Balanced }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSpec {
    pub scenario_uuid: Uuid,
    pub twin_uuid: Uuid,
    pub source_state_sha256: String,
    pub iterations: u32,
    pub seed: u64,
    pub strategy: Strategy,
    pub assumptions: BTreeMap<String,String>,
}
impl ScenarioSpec {
    pub fn scenario_hash(&self)->String{canonical_sha256(self)}
    pub fn effective_seed(&self)->u64{
        let h=self.scenario_hash(); u64::from_str_radix(&h[..16],16).unwrap_or(self.seed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Percentiles { pub p50:f64,pub p80:f64,pub p90:f64,pub p95:f64 }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSimulationResult {
    pub project_uuid: Uuid,
    pub completion_days: Percentiles,
    pub total_cost: Percentiles,
    pub deadline_probability: f64,
    pub budget_overrun_probability: f64,
    pub rework_probability: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioSimulationResult {
    pub run_uuid: Uuid,
    pub scenario_uuid: Uuid,
    pub twin_hash: String,
    pub scenario_hash: String,
    pub iterations: u32,
    pub projects: Vec<ProjectSimulationResult>,
    pub portfolio_cost: Percentiles,
    pub all_deadlines_probability: f64,
    pub shared_resource_contention_probability: f64,
    pub bottleneck_projects: Vec<Uuid>,
    pub evidence_sha256: String,
}

// Deterministic xorshift RNG avoids nondeterministic evidence generation.
#[derive(Clone)] struct Rng(u64);
impl Rng {
    fn next(&mut self)->f64 { let mut x=self.0; x^=x<<13; x^=x>>7; x^=x<<17; self.0=x; (((x>>11) as f64)+1.0)/(((1u64<<53) as f64)+2.0) }
    fn normal(&mut self)->f64 { let u1=self.next(); let u2=self.next(); (-2.0*u1.ln()).sqrt()*(2.0*std::f64::consts::PI*u2).cos() }
}
fn gamma_shape(k:f64,r:&mut Rng)->f64 {
    // Marsaglia-Tsang, deterministic for a deterministic Rng.
    if k < 1.0 { let u=r.next(); return gamma_shape(k+1.0,r)*u.powf(1.0/k); }
    let d=k-1.0/3.0; let c=(1.0/(9.0*d)).sqrt();
    loop { let x=r.normal(); let v0=1.0+c*x; if v0<=0.0 {continue} let v=v0*v0*v0; let u=r.next(); if u<1.0-0.0331*x*x*x*x || u.ln()<0.5*x*x+d*(1.0-v+v.ln()) {return d*v} }
}
fn beta_sample(a:f64,b:f64,r:&mut Rng)->f64 { let x=gamma_shape(a,r); let y=gamma_shape(b,r); x/(x+y) }
fn sample(d:&Distribution,r:&mut Rng)->f64{
    match d {
        Distribution::Triangular{min,mode,max}=>{ if max==min{return *min} let u=r.next();let f=(mode-min)/(max-min);if u<f{min+((u*(max-min)*(mode-min)).sqrt())}else{max-(((1.0-u)*(max-min)*(max-mode)).sqrt())}},
        Distribution::Pert{min,mode,max,lambda}=>{ if max==min{return *min} let width=max-min; let a=1.0+lambda*(mode-min)/width; let b=1.0+lambda*(max-mode)/width; min+beta_sample(a,b,r)*width },
        Distribution::Empirical{samples}=>{let i=((r.next()*samples.len() as f64) as usize).min(samples.len()-1);samples[i]},
    }
}
fn pct(mut x:Vec<f64>,p:f64)->f64{if x.is_empty(){return 0.0}x.sort_by(|a,b|a.total_cmp(b));let i=(((x.len()-1) as f64)*(p/100.0)).round() as usize;x[i]}
fn pcts(x:&[f64])->Percentiles{Percentiles{p50:pct(x.to_vec(),50.0),p80:pct(x.to_vec(),80.0),p90:pct(x.to_vec(),90.0),p95:pct(x.to_vec(),95.0)}}
fn assumption_f64(s:&ScenarioSpec,key:&str)->Result<f64,TwinError>{ match s.assumptions.get(key){None=>Ok(1.0),Some(v)=>{let x=v.parse::<f64>().map_err(|_|TwinError::HardConstraint)?;if x.is_finite()&&(0.1..=10.0).contains(&x){Ok(x)}else{Err(TwinError::HardConstraint)}}} }
fn stable_result_sha256(v:&PortfolioSimulationResult)->String {
    #[derive(Serialize)] struct Stable<'a>{scenario_uuid:Uuid,twin_hash:&'a str,scenario_hash:&'a str,iterations:u32,projects:&'a [ProjectSimulationResult],portfolio_cost:&'a Percentiles,all_deadlines_probability:f64,shared_resource_contention_probability:f64,bottleneck_projects:&'a [Uuid]}
    canonical_sha256(&Stable{scenario_uuid:v.scenario_uuid,twin_hash:&v.twin_hash,scenario_hash:&v.scenario_hash,iterations:v.iterations,projects:&v.projects,portfolio_cost:&v.portfolio_cost,all_deadlines_probability:v.all_deadlines_probability,shared_resource_contention_probability:v.shared_resource_contention_probability,bottleneck_projects:&v.bottleneck_projects})
}

pub fn simulate(twin:&PortfolioTwin,s:&ScenarioSpec)->Result<PortfolioSimulationResult,TwinError>{
    twin.validate()?;
    if s.source_state_sha256!=twin.source_state_sha256||s.twin_uuid!=twin.twin_uuid{return Err(TwinError::SourceStateMismatch)}
    if twin.promoted_calibrations.is_empty() && s.assumptions.get("human_assumption_ack").map(String::as_str)!=Some("true") { return Err(TwinError::InvalidCalibration); }
    let duration_multiplier=assumption_f64(s,"duration_multiplier")?; let cost_multiplier=assumption_f64(s,"cost_multiplier")?; let agent_capacity_multiplier=assumption_f64(s,"agent_capacity_multiplier")?; let model_quota_multiplier=assumption_f64(s,"model_quota_multiplier")?;
    let iterations=s.iterations.clamp(100,100_000); let mut rng=Rng(s.effective_seed().max(1)); let order=twin.topological_order()?;
    let project_by_id:BTreeMap<Uuid,&ProjectTwin>=twin.projects.iter().map(|p|(p.project_uuid,p)).collect();
    let incoming: BTreeMap<Uuid,Vec<&ProjectDependency>> = { let mut m:BTreeMap<Uuid,Vec<&ProjectDependency>>=BTreeMap::new(); for d in &twin.dependencies {m.entry(d.to_project_uuid).or_default().push(d);} m };
    let cap:BTreeMap<Uuid,f64>=twin.shared_resources.iter().map(|r|(r.resource_uuid,r.capacity_per_day*agent_capacity_multiplier)).collect();
    let model_cap:BTreeMap<String,f64>=twin.model_quotas_per_day.iter().map(|(k,v)|(k.clone(),v*model_quota_multiplier)).collect();
    let mut completion:BTreeMap<Uuid,Vec<f64>>=BTreeMap::new(); let mut costs:BTreeMap<Uuid,Vec<f64>>=BTreeMap::new(); let mut reworks:BTreeMap<Uuid,u32>=BTreeMap::new();
    let mut total_costs=Vec::with_capacity(iterations as usize); let mut all_deadlines=0u32; let mut contention=0u32;
    for _ in 0..iterations {
        let mut sampled_duration:BTreeMap<Uuid,f64>=BTreeMap::new(); let mut sampled_cost:BTreeMap<Uuid,f64>=BTreeMap::new(); let mut agent_rate:BTreeMap<Uuid,BTreeMap<Uuid,f64>>=BTreeMap::new(); let mut model_rate:BTreeMap<Uuid,BTreeMap<String,f64>>=BTreeMap::new();
        for p in &twin.projects {
            let d=(sample(&p.remaining_duration_days,&mut rng).max(0.0)*duration_multiplier).max(0.001); let rw=sample(&p.rework_fraction,&mut rng).clamp(0.0,1.0); let c=sample(&p.remaining_cost,&mut rng).max(0.0)*cost_multiplier*(1.0+rw);
            sampled_duration.insert(p.project_uuid,d); sampled_cost.insert(p.project_uuid,c); if rw>0.15{*reworks.entry(p.project_uuid).or_default()+=1}
            let mut ar=BTreeMap::new(); for (a,dist) in &p.required_agent_hours{ar.insert(*a,sample(dist,&mut rng).max(0.0)/d);} agent_rate.insert(p.project_uuid,ar);
            let mut mr=BTreeMap::new(); for (m,dist) in &p.required_model_units{mr.insert(m.clone(),sample(dist,&mut rng).max(0.0)/d);} model_rate.insert(p.project_uuid,mr);
        }
        let mut starts:BTreeMap<Uuid,f64>=BTreeMap::new(); let mut finishes:BTreeMap<Uuid,f64>=BTreeMap::new();
        for id in &order {
            let d=*sampled_duration.get(id).unwrap(); let mut st=0.0_f64;
            if let Some(deps)=incoming.get(id){ for dep in deps { let fs=*starts.get(&dep.from_project_uuid).unwrap(); let ff=*finishes.get(&dep.from_project_uuid).unwrap(); let candidate=match dep.dependency_type{DependencyType::FS=>ff+dep.lag_days,DependencyType::SS=>fs+dep.lag_days,DependencyType::FF=>ff+dep.lag_days-d,DependencyType::SF=>fs+dep.lag_days-d}; st=st.max(candidate.max(0.0)); } }
            starts.insert(*id,st); finishes.insert(*id,st+d);
        }
        let mut total=0.0; let mut all_ok=true;
        for id in &order { let p=project_by_id[id]; let finish=finishes[id]; let c=sampled_cost[id]; completion.entry(*id).or_default().push(finish); costs.entry(*id).or_default().push(c); total+=c; if c>p.hard_budget || finish>p.deadline_remaining_days{all_ok=false} }
        if all_ok{all_deadlines+=1} total_costs.push(total);
        // Exact overlap intervals for daily rate contention.
        let mut bounds:Vec<f64>=starts.values().chain(finishes.values()).copied().collect(); bounds.sort_by(|a,b|a.total_cmp(b)); bounds.dedup_by(|a,b|(*a-*b).abs()<1e-9); let mut iter_contended=false;
        for w in bounds.windows(2){ let mid=(w[0]+w[1])/2.0; let mut ad:BTreeMap<Uuid,f64>=BTreeMap::new(); let mut md:BTreeMap<String,f64>=BTreeMap::new(); for id in &order { if starts[id]<=mid && mid<finishes[id] { for (a,v) in &agent_rate[id]{*ad.entry(*a).or_default()+=*v} for (m,v) in &model_rate[id]{*md.entry(m.clone()).or_default()+=*v} } } if ad.iter().any(|(id,d)|*d>cap[id]) || md.iter().any(|(id,d)|*d>model_cap[id]) { iter_contended=true; break; } }
        if iter_contended{contention+=1}
    }
    let projects=twin.projects.iter().map(|p|{let ds=completion.remove(&p.project_uuid).unwrap_or_default();let cs=costs.remove(&p.project_uuid).unwrap_or_default();let deadline=ds.iter().filter(|x|**x<=p.deadline_remaining_days).count() as f64/iterations as f64;let over=cs.iter().filter(|x|**x>p.hard_budget).count() as f64/iterations as f64;ProjectSimulationResult{project_uuid:p.project_uuid,completion_days:pcts(&ds),total_cost:pcts(&cs),deadline_probability:deadline,budget_overrun_probability:over,rework_probability:*reworks.get(&p.project_uuid).unwrap_or(&0) as f64/iterations as f64}}).collect::<Vec<_>>();
    let mut bottlenecks=projects.iter().filter(|p|p.deadline_probability<0.8||p.budget_overrun_probability>0.2).map(|p|p.project_uuid).collect::<Vec<_>>();bottlenecks.sort(); let twin_hash=twin.canonical_hash();let scenario_hash=s.scenario_hash();
    let mut out=PortfolioSimulationResult{run_uuid:Uuid::now_v7(),scenario_uuid:s.scenario_uuid,twin_hash,scenario_hash,iterations,projects,portfolio_cost:pcts(&total_costs),all_deadlines_probability:all_deadlines as f64/iterations as f64,shared_resource_contention_probability:contention as f64/iterations as f64,bottleneck_projects:bottlenecks,evidence_sha256:String::new()}; out.evidence_sha256=stable_result_sha256(&out);Ok(out)
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DecisionDelegation {
    pub decision_case_uuid: Uuid,
    pub scenario_uuid: Uuid,
    pub scenario_hash: String,
    pub simulation_evidence_sha256: String,
    pub delegate_to: String,
    pub direct_mutation: bool,
}
pub fn delegate(result:&PortfolioSimulationResult)->DecisionDelegation{DecisionDelegation{decision_case_uuid:Uuid::now_v7(),scenario_uuid:result.scenario_uuid,scenario_hash:result.scenario_hash.clone(),simulation_evidence_sha256:result.evidence_sha256.clone(),delegate_to:DELEGATE_DECISIONS_TO.into(),direct_mutation:false}}
pub fn assert_no_direct_mutation(d:&DecisionDelegation)->Result<(),TwinError>{if d.direct_mutation{Err(TwinError::DirectMutationForbidden)}else{Ok(())}}

fn canonical_sha256<T:Serialize>(v:&T)->String{let b=serde_json::to_vec(v).expect("serializable");let mut h=Sha256::new();h.update(b);format!("{:x}",h.finalize())}
