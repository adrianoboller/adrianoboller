use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactoryStage {
    Intake,
    Decompose,
    Research,
    Architecture,
    Implement,
    Integrate,
    Qa,
    Security,
    Documentation,
    ReleaseCandidate,
    Approval,
    Delivery,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FactoryRun {
    pub run_uuid: Uuid,
    pub requirement_id: String,
    pub stage: FactoryStage,
    pub source_state_sha256: String,
    pub checkpoint_uuid: Option<Uuid>,
    pub approved: bool,
}
#[derive(Debug, Error)]
pub enum FactoryError {
    #[error("requirement id required")]
    RequirementRequired,
    #[error("approval required")]
    ApprovalRequired,
    #[error("checkpoint required before mutation")]
    CheckpointRequired,
}
impl FactoryRun {
    pub fn new(requirement_id: String, source_state_sha256: String) -> Result<Self, FactoryError> {
        if requirement_id.trim().is_empty() {
            return Err(FactoryError::RequirementRequired);
        }
        Ok(Self {
            run_uuid: Uuid::now_v7(),
            requirement_id,
            stage: FactoryStage::Intake,
            source_state_sha256,
            checkpoint_uuid: None,
            approved: false,
        })
    }
    pub fn enter_implement(&mut self, checkpoint_uuid: Uuid) -> Result<(), FactoryError> {
        self.checkpoint_uuid = Some(checkpoint_uuid);
        self.stage = FactoryStage::Implement;
        Ok(())
    }
    pub fn approve(&mut self) {
        self.approved = true;
        self.stage = FactoryStage::Delivery;
    }
    pub fn deliver(&self) -> Result<(), FactoryError> {
        if !self.approved {
            return Err(FactoryError::ApprovalRequired);
        }
        if self.checkpoint_uuid.is_none() {
            return Err(FactoryError::CheckpointRequired);
        }
        Ok(())
    }
}
