use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Methodology {
    Hybrid,
    Scrum,
    Kanban,
    Classical,
    Pdca,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrafficLight {
    Green,
    Yellow,
    Red,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependencyType {
    Fs,
    Ss,
    Ff,
    Sf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSkillRef {
    pub id: String,
    pub capability: String,
    pub approval_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealthInput {
    pub scope: f64,
    pub schedule: f64,
    pub cost: f64,
    pub quality: f64,
    pub risk: f64,
    pub resources: f64,
    pub flow: f64,
    pub stakeholders: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealth {
    pub score: f64,
    pub color: TrafficLight,
    pub weakest_dimension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmInput {
    pub pv: f64,
    pub ev: f64,
    pub ac: f64,
    pub bac: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmMetrics {
    pub cpi: Option<f64>,
    pub spi: Option<f64>,
    pub eac: Option<f64>,
    pub etc: Option<f64>,
    pub vac: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub uuid: Uuid,
    pub title: String,
    pub duration_minutes: i64,
    pub predecessors: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridPlan {
    pub project_uuid: Uuid,
    pub methodology: Methodology,
    pub ordered_items: Vec<Uuid>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum ProjectManagementError {
    #[error("health dimension out of range: {0}")]
    HealthRange(String),
    #[error("dependency cycle detected")]
    DependencyCycle,
    #[error("earned value input cannot be negative")]
    NegativeEvm,
}

pub fn evaluate_health(i: &ProjectHealthInput) -> Result<ProjectHealth, ProjectManagementError> {
    let dims = BTreeMap::from([
        ("scope", i.scope),
        ("schedule", i.schedule),
        ("cost", i.cost),
        ("quality", i.quality),
        ("risk", i.risk),
        ("resources", i.resources),
        ("flow", i.flow),
        ("stakeholders", i.stakeholders),
    ]);
    for (name, value) in &dims {
        if !(0.0..=1.0).contains(value) {
            return Err(ProjectManagementError::HealthRange((*name).into()));
        }
    }
    let score = dims.values().sum::<f64>() / dims.len() as f64;
    let color = if score >= 0.85 {
        TrafficLight::Green
    } else if score >= 0.65 {
        TrafficLight::Yellow
    } else {
        TrafficLight::Red
    };
    let weakest_dimension = dims
        .iter()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, _)| (*k).to_string())
        .unwrap_or_default();
    Ok(ProjectHealth {
        score,
        color,
        weakest_dimension,
    })
}

pub fn compute_evm(i: &EvmInput) -> Result<EvmMetrics, ProjectManagementError> {
    if [i.pv, i.ev, i.ac, i.bac].iter().any(|v| *v < 0.0) {
        return Err(ProjectManagementError::NegativeEvm);
    }
    let cpi = (i.ac > 0.0).then(|| i.ev / i.ac);
    let spi = (i.pv > 0.0).then(|| i.ev / i.pv);
    let eac = cpi.filter(|v| *v > 0.0).map(|v| i.bac / v);
    let etc = eac.map(|v| (v - i.ac).max(0.0));
    let vac = eac.map(|v| i.bac - v);
    Ok(EvmMetrics {
        cpi,
        spi,
        eac,
        etc,
        vac,
    })
}

pub fn topological_order(items: &[WorkItem]) -> Result<Vec<Uuid>, ProjectManagementError> {
    let ids: BTreeSet<_> = items.iter().map(|x| x.uuid).collect();
    let mut incoming: BTreeMap<Uuid, usize> = items
        .iter()
        .map(|x| {
            (
                x.uuid,
                x.predecessors.iter().filter(|p| ids.contains(p)).count(),
            )
        })
        .collect();
    let mut outgoing: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for i in items {
        for p in &i.predecessors {
            if ids.contains(p) {
                outgoing.entry(*p).or_default().push(i.uuid);
            }
        }
    }
    let mut ready: BTreeSet<Uuid> = incoming
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut out = Vec::with_capacity(items.len());
    while let Some(id) = ready.pop_first() {
        out.push(id);
        if let Some(next) = outgoing.get(&id) {
            for n in next {
                let e = incoming.get_mut(n).expect("known node");
                *e -= 1;
                if *e == 0 {
                    ready.insert(*n);
                }
            }
        }
    }
    if out.len() != items.len() {
        return Err(ProjectManagementError::DependencyCycle);
    }
    Ok(out)
}

pub fn hybrid_plan(
    project_uuid: Uuid,
    items: &[WorkItem],
) -> Result<HybridPlan, ProjectManagementError> {
    Ok(HybridPlan {
        project_uuid,
        methodology: Methodology::Hybrid,
        ordered_items: topological_order(items)?,
        generated_at: Utc::now(),
    })
}
