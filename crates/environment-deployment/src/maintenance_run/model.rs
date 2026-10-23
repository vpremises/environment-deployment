use crate::{Error, Result, digest};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceContext {
    pub owner: String,
    pub host: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub inventory_sha256: String,
    pub desired_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceStep {
    pub id: String,
    pub operation: String,
    pub before_sha256: String,
    pub after_sha256: String,
}

/// Product-local consent, never a provider credential or Crowsi execution grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenancePlan {
    pub schema: String,
    pub id: String,
    pub environment_id: String,
    pub context: MaintenanceContext,
    pub issued_at: i64,
    pub expires_at: i64,
    pub max_runtime_seconds: u16,
    pub steps: Vec<MaintenanceStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceReservation {
    pub plan_id: String,
    pub plan_sha256: String,
    pub step: MaintenanceStep,
    pub reserved_at: i64,
    pub deadline_at: i64,
    pub execution_authorized: bool,
}

/// A host adapter's independent observation, not an assertion of execution cause.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceObservation {
    pub state_sha256: String,
    pub evidence_sha256: String,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceStepReceipt {
    pub step_id: String,
    pub reserved_at: i64,
    pub observed: MaintenanceObservation,
    pub outcome: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRunStatus {
    pub plan_id: String,
    pub plan_sha256: String,
    pub phase: String,
    pub next_step: u16,
    pub revoked: bool,
    pub stopped: bool,
    pub expires_at: i64,
    pub active: Option<MaintenanceReservation>,
    pub last_observation: Option<MaintenanceObservation>,
    pub receipts: Vec<MaintenanceStepReceipt>,
    pub execution_authorized: bool,
}

pub(super) fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
    {
        return Err(Error::new("invalid maintenance identity"));
    }
    Ok(())
}

pub(super) fn hash(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::new("invalid maintenance digest"));
    }
    Ok(())
}

impl MaintenancePlan {
    pub(super) fn validate(&self) -> Result<()> {
        if self.schema != "vpremises://maintenance/finite-plan/v1"
            || self.issued_at < 0
            || self.expires_at <= self.issued_at
            || self.expires_at.saturating_sub(self.issued_at) > 3600
            || !(1..=300).contains(&self.max_runtime_seconds)
            || self.steps.is_empty()
            || self.steps.len() > 16
        {
            return Err(Error::new("maintenance plan bounds refused"));
        }
        for id in [
            &self.id,
            &self.environment_id,
            &self.context.owner,
            &self.context.host,
        ] {
            identifier(id)?;
        }
        for value in [
            &self.context.source_sha256,
            &self.context.artifact_sha256,
            &self.context.inventory_sha256,
            &self.context.desired_sha256,
        ] {
            hash(value)?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for step in &self.steps {
            identifier(&step.id)?;
            identifier(&step.operation)?;
            hash(&step.before_sha256)?;
            hash(&step.after_sha256)?;
            if !ids.insert(&step.id) || step.before_sha256 == step.after_sha256 {
                return Err(Error::new("duplicate or ambiguous maintenance step"));
            }
        }
        if serde_json::to_vec(self)?.len() > 16384 {
            return Err(Error::new("maintenance plan byte limit"));
        }
        Ok(())
    }

    pub(super) fn admit(&self, context: &MaintenanceContext, now: i64) -> Result<()> {
        self.validate()?;
        if &self.context != context || now < self.issued_at || now >= self.expires_at {
            return Err(Error::new("maintenance context or validity refused"));
        }
        Ok(())
    }
}

/// Computes the digest of the closed typed plan, including all steps and bindings.
/// # Errors
/// Rejects malformed, excessive or ambiguous plans.
pub fn maintenance_plan_digest(plan: &MaintenancePlan) -> Result<String> {
    plan.validate()?;
    Ok(digest::bytes(&serde_json::to_vec(plan)?))
}
