mod model;
mod private;
mod transitions;
use crate::{Error, Ledger, Result};
pub use model::*;
use rusqlite::{OptionalExtension, Transaction, params};

fn load(
    tx: &Transaction<'_>,
    id: &str,
    expected: &str,
) -> Result<(MaintenancePlan, MaintenanceRunStatus, Option<i64>)> {
    model::identifier(id)?;
    model::hash(expected)?;
    let row = tx.query_row("SELECT plan_sha256,plan_json,phase,next_step,revoked,stopped,deadline_at,active_json,observation_json FROM maintenance_runs WHERE id=?1",
        [id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,usize>(3)?,r.get::<_,bool>(4)?,r.get::<_,bool>(5)?,r.get::<_,Option<i64>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?)))
        .optional()?.ok_or_else(||Error::new("maintenance plan is not approved"))?;
    let plan: MaintenancePlan = serde_json::from_str(&row.1)?;
    if row.0 != expected || maintenance_plan_digest(&plan)? != expected || plan.id != id {
        return Err(Error::new("maintenance plan digest mismatch"));
    }
    let receipts = tx.prepare("SELECT step_id,reserved_at,state_sha256,evidence_sha256,observed_at,outcome FROM maintenance_receipts WHERE plan_id=?1 ORDER BY rowid LIMIT 17")?
        .query_map([id], |r|Ok(MaintenanceStepReceipt {step_id:r.get(0)?,reserved_at:r.get(1)?,
            observed:MaintenanceObservation{state_sha256:r.get(2)?,evidence_sha256:r.get(3)?,observed_at:r.get(4)?},outcome:r.get(5)?}))?
        .collect::<std::result::Result<Vec<_>,_>>()?;
    if receipts.len() > plan.steps.len() {
        return Err(Error::new("maintenance receipt count refused"));
    }
    let status = MaintenanceRunStatus {
        plan_id: id.into(),
        plan_sha256: expected.into(),
        phase: row.2,
        next_step: row.3,
        revoked: row.4,
        stopped: row.5,
        expires_at: plan.expires_at,
        active: row.7.map(|s| serde_json::from_str(&s)).transpose()?,
        last_observation: row.8.map(|s| serde_json::from_str(&s)).transpose()?,
        receipts,
        execution_authorized: false,
    };
    Ok((plan, status, row.6))
}

fn clock(tx: &Transaction<'_>, now: i64) -> Result<()> {
    let watermark: i64 = tx.query_row(
        "SELECT observed_at FROM maintenance_clock WHERE id=1",
        [],
        |r| r.get(0),
    )?;
    if now < watermark {
        return Err(Error::new("maintenance clock rollback refused"));
    }
    tx.execute(
        "UPDATE maintenance_clock SET observed_at=?1 WHERE id=1",
        [now],
    )?;
    Ok(())
}

fn context(plan: &MaintenancePlan, observed: &MaintenanceContext) -> Result<()> {
    if &plan.context != observed {
        return Err(Error::new("maintenance context mismatch"));
    }
    Ok(())
}

fn refuse_unknown_scope(tx: &Transaction<'_>, plan: &MaintenancePlan) -> Result<()> {
    let unknown: i64 = tx.query_row("SELECT count(*) FROM maintenance_runs WHERE environment_id=?1 AND host=?2 AND phase='unknown'",
        params![plan.environment_id, plan.context.host], |r| r.get(0))?;
    if unknown != 0 {
        return Err(Error::new(
            "unknown maintenance effect blocks this host/environment; reconcile before any new plan",
        ));
    }
    Ok(())
}

impl Ledger {
    fn require_private(&self) -> Result<()> {
        if !self.private {
            return Err(Error::new("maintenance requires a private owner ledger"));
        }
        Ok(())
    }

    /// Records local consent to one exact finite plan, without granting host authority.
    /// # Errors
    /// Refuses repeated IDs, changed contexts, stale plans and exhausted journal capacity.
    pub fn approve_maintenance(
        &mut self,
        plan: &MaintenancePlan,
        observed: &MaintenanceContext,
        now: i64,
    ) -> Result<String> {
        self.require_private()?;
        plan.admit(observed, now)?;
        let digest = maintenance_plan_digest(plan)?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        clock(&tx, now)?;
        refuse_unknown_scope(&tx, plan)?;
        let count: i64 = tx.query_row("SELECT count(*) FROM maintenance_runs", [], |r| r.get(0))?;
        if count >= 4096 {
            return Err(Error::new("maintenance journal capacity reached"));
        }
        tx.execute("INSERT INTO maintenance_runs(id,environment_id,host,plan_sha256,plan_json,phase) VALUES(?1,?2,?3,?4,?5,'approved')",
            params![plan.id,plan.environment_id,plan.context.host,digest,serde_json::to_string(plan)?])?;
        tx.commit()?;
        Ok(digest)
    }

    /// Reads local lifecycle metadata; no execution or provider validation is implied.
    /// # Errors
    /// Refuses unapproved IDs, altered plans, changed contexts and time rollback.
    pub fn maintenance_status(
        &mut self,
        id: &str,
        digest: &str,
        observed: &MaintenanceContext,
        now: i64,
    ) -> Result<MaintenanceRunStatus> {
        self.require_private()?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        clock(&tx, now)?;
        let (plan, mut status, deadline) = load(&tx, id, digest)?;
        context(&plan, observed)?;
        if status.phase == "approved"
            && (now >= plan.expires_at || deadline.is_some_and(|end| now >= end))
        {
            status.phase = "expired".into();
        }
        tx.commit()?;
        Ok(status)
    }
}
