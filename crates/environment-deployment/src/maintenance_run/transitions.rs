use super::{
    MaintenanceContext, MaintenanceObservation, MaintenanceReservation, MaintenanceRunStatus,
    clock, context, load, model, refuse_unknown_scope,
};
use crate::{Error, Ledger, Result};
use rusqlite::params;

impl Ledger {
    /// Consumes the next step durably before a separately authorized adapter can dispatch.
    /// # Errors
    /// Refuses replay, concurrent/unknown work, stop/revocation, stale budgets and mismatches.
    pub fn reserve_maintenance(
        &mut self,
        id: &str,
        digest: &str,
        observed: &MaintenanceContext,
        preflight: &MaintenanceObservation,
        now: i64,
    ) -> Result<MaintenanceReservation> {
        self.require_private()?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        clock(&tx, now)?;
        let (plan, status, deadline) = load(&tx, id, digest)?;
        plan.admit(observed, now)?;
        refuse_unknown_scope(&tx, &plan)?;
        if status.revoked || status.stopped || status.phase != "approved" {
            return Err(Error::new("maintenance dispatch refused; inspect status"));
        }
        let deadline = deadline.unwrap_or_else(|| {
            now.saturating_add(i64::from(plan.max_runtime_seconds))
                .min(plan.expires_at)
        });
        if now >= deadline {
            return Err(Error::new("maintenance execution budget expired"));
        }
        let step = plan
            .steps
            .get(status.next_step)
            .ok_or_else(|| Error::new("maintenance batch already consumed"))?
            .clone();
        model::hash(&preflight.state_sha256)?;
        model::hash(&preflight.evidence_sha256)?;
        if preflight.state_sha256 != step.before_sha256
            || preflight.observed_at > now
            || preflight.observed_at < now.saturating_sub(10)
        {
            return Err(Error::new("fresh maintenance precondition refused"));
        }
        let reservation = MaintenanceReservation {
            plan_id: id.into(),
            plan_sha256: digest.into(),
            step,
            reserved_at: now,
            deadline_at: deadline,
            execution_authorized: false,
        };
        tx.execute(
            "UPDATE maintenance_runs SET phase='unknown',active_json=?2,deadline_at=?3 WHERE id=?1",
            params![id, serde_json::to_string(&reservation)?, deadline],
        )?;
        tx.commit()?;
        Ok(reservation)
    }

    /// Reconciles an independent adapter observation, even after expiry or revocation.
    /// # Errors
    /// Refuses substituted reservations, duplicate receipts, wrong hosts and invalid evidence times.
    pub fn reconcile_maintenance(
        &mut self,
        reservation: &MaintenanceReservation,
        observed: &MaintenanceContext,
        evidence: &MaintenanceObservation,
        now: i64,
    ) -> Result<MaintenanceRunStatus> {
        self.require_private()?;
        model::hash(&evidence.state_sha256)?;
        model::hash(&evidence.evidence_sha256)?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        clock(&tx, now)?;
        let (plan, status, _) = load(&tx, &reservation.plan_id, &reservation.plan_sha256)?;
        context(&plan, observed)?;
        if status.phase != "unknown"
            || status.active.as_ref() != Some(reservation)
            || reservation.execution_authorized
            || evidence.observed_at < reservation.reserved_at
            || evidence.observed_at > now
        {
            return Err(Error::new("maintenance readback binding refused"));
        }
        let (after, before) = (
            evidence.state_sha256 == reservation.step.after_sha256,
            evidence.state_sha256 == reservation.step.before_sha256,
        );
        let next = status.next_step + usize::from(after);
        let phase = if after && next == plan.steps.len() {
            "completed"
        } else if after && (status.revoked || status.stopped || now >= reservation.deadline_at) {
            "stopped"
        } else if after {
            "approved"
        } else if before {
            "failed"
        } else {
            "unknown"
        };
        if after || before {
            tx.execute(
                "INSERT INTO maintenance_receipts VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    reservation.plan_id,
                    reservation.step.id,
                    reservation.reserved_at,
                    evidence.state_sha256,
                    evidence.evidence_sha256,
                    evidence.observed_at,
                    if after {
                        "after-state-observed"
                    } else {
                        "before-state-observed"
                    }
                ],
            )?;
        }
        tx.execute("UPDATE maintenance_runs SET phase=?2,next_step=?3,observation_json=?4,active_json=?5 WHERE id=?1",
            params![reservation.plan_id,phase,next,serde_json::to_string(evidence)?,if phase=="unknown" {Some(serde_json::to_string(reservation)?)}else{None}])?;
        let (_, result, _) = load(&tx, &reservation.plan_id, &reservation.plan_sha256)?;
        tx.commit()?;
        Ok(result)
    }

    /// Revokes remaining local consent; in-flight unknown results require readback.
    /// # Errors
    /// Refuses changed identity/digests or time rollback. It does not cancel a provider call.
    pub fn revoke_maintenance(
        &mut self,
        id: &str,
        digest: &str,
        observed: &MaintenanceContext,
        now: i64,
    ) -> Result<MaintenanceRunStatus> {
        self.stop_or_revoke(id, digest, observed, now, true)
    }

    /// Stops the remaining finite batch without claiming to undo an external effect.
    /// # Errors
    /// Refuses changed identity/digests or time rollback; unknown work stays unknown.
    pub fn stop_maintenance(
        &mut self,
        id: &str,
        digest: &str,
        observed: &MaintenanceContext,
        now: i64,
    ) -> Result<MaintenanceRunStatus> {
        self.stop_or_revoke(id, digest, observed, now, false)
    }

    fn stop_or_revoke(
        &mut self,
        id: &str,
        digest: &str,
        observed: &MaintenanceContext,
        now: i64,
        revoke: bool,
    ) -> Result<MaintenanceRunStatus> {
        self.require_private()?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        clock(&tx, now)?;
        let (plan, _, _) = load(&tx, id, digest)?;
        context(&plan, observed)?;
        tx.execute("UPDATE maintenance_runs SET revoked=max(revoked,?2),stopped=max(stopped,?3),phase=CASE WHEN phase='approved' THEN 'stopped' ELSE phase END WHERE id=?1",
            params![id,revoke,!revoke])?;
        let (_, status, _) = load(&tx, id, digest)?;
        tx.commit()?;
        Ok(status)
    }
}
