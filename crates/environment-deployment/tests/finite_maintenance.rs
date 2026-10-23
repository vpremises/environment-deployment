#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Barrier},
};
use vpremises_environment_deployment::{
    Ledger, MaintenanceContext, MaintenanceObservation, MaintenancePlan, MaintenanceStep,
    maintenance_plan_digest,
};

fn hash(byte: char) -> String {
    byte.to_string().repeat(64)
}
fn plan() -> MaintenancePlan {
    MaintenancePlan {
        schema: "vpremises://maintenance/finite-plan/v1".into(),
        id: "fixture-batch".into(),
        environment_id: "fixture-host".into(),
        context: MaintenanceContext {
            owner: "unix-uid:fixture".into(),
            host: "fixture-host-id".into(),
            source_sha256: hash('1'),
            artifact_sha256: hash('2'),
            inventory_sha256: hash('3'),
            desired_sha256: hash('4'),
        },
        issued_at: 1000,
        expires_at: 1200,
        max_runtime_seconds: 60,
        steps: vec![
            MaintenanceStep {
                id: "first".into(),
                operation: "synthetic-update".into(),
                before_sha256: hash('a'),
                after_sha256: hash('b'),
            },
            MaintenanceStep {
                id: "second".into(),
                operation: "synthetic-checkpoint".into(),
                before_sha256: hash('b'),
                after_sha256: hash('c'),
            },
        ],
    }
}
fn observation(value: char, now: i64) -> MaintenanceObservation {
    MaintenanceObservation {
        state_sha256: hash(value),
        evidence_sha256: hash('e'),
        observed_at: now,
    }
}
fn fixture() -> (tempfile::TempDir, Ledger, MaintenancePlan, String) {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let mut ledger = Ledger::open_private(directory.path().join("ledger.sqlite3")).unwrap();
    let plan = plan();
    let digest = ledger
        .approve_maintenance(&plan, &plan.context, 1000)
        .unwrap();
    (directory, ledger, plan, digest)
}

#[test]
fn finite_batch_preserves_every_receipt_and_stops_after_the_last_step() {
    let (_dir, mut ledger, plan, digest) = fixture();
    for (index, (before, after)) in [('a', 'b'), ('b', 'c')].into_iter().enumerate() {
        let now = 1001 + i64::try_from(index).unwrap() * 2;
        let reservation = ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation(before, now),
                now,
            )
            .unwrap();
        assert_eq!(reservation.step, plan.steps[index]);
        assert!(!reservation.execution_authorized);
        let status = ledger
            .reconcile_maintenance(
                &reservation,
                &plan.context,
                &observation(after, now + 1),
                now + 1,
            )
            .unwrap();
        assert_eq!(usize::from(status.next_step), index + 1);
        assert_eq!(status.receipts.len(), index + 1);
        assert!(!status.execution_authorized);
        assert!(
            ledger
                .reconcile_maintenance(
                    &reservation,
                    &plan.context,
                    &observation(after, now + 1),
                    now + 1
                )
                .is_err()
        );
    }
    assert_eq!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1004)
            .unwrap()
            .phase,
        "completed"
    );
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('c', 1004),
                1004
            )
            .is_err()
    );
}

#[test]
fn crash_unknown_is_durable_and_new_ids_do_not_bypass_readback() {
    let (dir, mut ledger, plan, digest) = fixture();
    let reservation = ledger
        .reserve_maintenance(
            &plan.id,
            &digest,
            &plan.context,
            &observation('a', 1001),
            1001,
        )
        .unwrap();
    drop(ledger);
    let mut ledger = Ledger::open_private(dir.path().join("ledger.sqlite3")).unwrap();
    assert_eq!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1002)
            .unwrap()
            .phase,
        "unknown"
    );
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('a', 1002),
                1002
            )
            .is_err()
    );
    let mut renamed = plan.clone();
    renamed.id = "fresh-id-is-not-recovery".into();
    assert!(
        ledger
            .approve_maintenance(&renamed, &renamed.context, 1002)
            .is_err()
    );
    assert_eq!(
        ledger
            .reconcile_maintenance(&reservation, &plan.context, &observation('d', 1003), 1003)
            .unwrap()
            .phase,
        "unknown"
    );
    assert!(
        ledger
            .approve_maintenance(&renamed, &renamed.context, 1003)
            .is_err()
    );
    let recovered = ledger
        .reconcile_maintenance(&reservation, &plan.context, &observation('b', 1004), 1004)
        .unwrap();
    assert_eq!(recovered.phase, "approved");
    assert_eq!(recovered.next_step, 1);
}

#[test]
fn unchanged_readback_terminates_without_retrying_the_consumed_step() {
    let (_dir, mut ledger, plan, digest) = fixture();
    let reservation = ledger
        .reserve_maintenance(
            &plan.id,
            &digest,
            &plan.context,
            &observation('a', 1001),
            1001,
        )
        .unwrap();
    let status = ledger
        .reconcile_maintenance(&reservation, &plan.context, &observation('a', 1002), 1002)
        .unwrap();
    assert_eq!(status.phase, "failed");
    assert_eq!(status.receipts[0].outcome, "before-state-observed");
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('a', 1003),
                1003
            )
            .is_err()
    );
}

#[test]
fn stop_and_revocation_preserve_unknowns_and_never_dispatch_the_next_step() {
    for revoke in [false, true] {
        let (_dir, mut ledger, plan, digest) = fixture();
        let reservation = ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('a', 1001),
                1001,
            )
            .unwrap();
        let held = if revoke {
            ledger.revoke_maintenance(&plan.id, &digest, &plan.context, 1002)
        } else {
            ledger.stop_maintenance(&plan.id, &digest, &plan.context, 1002)
        }
        .unwrap();
        assert_eq!(held.phase, "unknown");
        assert_eq!(held.revoked, revoke);
        assert_eq!(held.stopped, !revoke);
        let read = ledger
            .reconcile_maintenance(&reservation, &plan.context, &observation('b', 1003), 1003)
            .unwrap();
        assert_eq!(read.phase, "stopped");
        assert!(read.active.is_none());
        assert!(
            ledger
                .reserve_maintenance(
                    &plan.id,
                    &digest,
                    &plan.context,
                    &observation('b', 1004),
                    1004
                )
                .is_err()
        );
    }
}

#[test]
fn revoked_or_stopped_approval_never_yields_a_reservation() {
    for revoke in [false, true] {
        let (_dir, mut ledger, plan, digest) = fixture();
        if revoke {
            ledger
                .revoke_maintenance(&plan.id, &digest, &plan.context, 1001)
                .unwrap();
        } else {
            ledger
                .stop_maintenance(&plan.id, &digest, &plan.context, 1001)
                .unwrap();
        }
        assert!(
            ledger
                .reserve_maintenance(
                    &plan.id,
                    &digest,
                    &plan.context,
                    &observation('a', 1002),
                    1002
                )
                .is_err()
        );
    }
}

#[test]
fn owner_host_source_artifact_inventory_and_digest_are_all_bound() {
    let (_dir, mut ledger, plan, digest) = fixture();
    for field in 0..6 {
        let mut wrong = plan.context.clone();
        match field {
            0 => wrong.owner = "other-owner".into(),
            1 => wrong.host = "other-host".into(),
            2 => wrong.source_sha256 = hash('4'),
            3 => wrong.artifact_sha256 = hash('5'),
            4 => wrong.inventory_sha256 = hash('6'),
            _ => wrong.desired_sha256 = hash('7'),
        }
        assert!(
            ledger
                .reserve_maintenance(&plan.id, &digest, &wrong, &observation('a', 1001), 1001)
                .is_err()
        );
    }
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &hash('0'),
                &plan.context,
                &observation('a', 1001),
                1001
            )
            .is_err()
    );
    assert!(
        ledger
            .approve_maintenance(&plan, &plan.context, 1001)
            .is_err()
    );
    assert_eq!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1001)
            .unwrap()
            .next_step,
        0
    );
}

#[test]
fn fresh_precondition_time_and_overall_runtime_are_enforced() {
    let (_dir, mut ledger, plan, digest) = fixture();
    for evidence in [
        observation('d', 1001),
        observation('a', 900),
        observation('a', 1100),
    ] {
        assert!(
            ledger
                .reserve_maintenance(&plan.id, &digest, &plan.context, &evidence, 1001)
                .is_err()
        );
    }
    let reservation = ledger
        .reserve_maintenance(
            &plan.id,
            &digest,
            &plan.context,
            &observation('a', 1001),
            1001,
        )
        .unwrap();
    assert_eq!(reservation.deadline_at, 1061);
    assert_eq!(
        ledger
            .reconcile_maintenance(&reservation, &plan.context, &observation('b', 1062), 1062)
            .unwrap()
            .phase,
        "stopped"
    );
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('b', 1063),
                1063
            )
            .is_err()
    );
}

#[test]
fn expired_approval_cannot_dispatch_but_unknown_readback_is_still_available() {
    let (_dir, mut ledger, plan, digest) = fixture();
    assert!(
        ledger
            .reserve_maintenance(
                &plan.id,
                &digest,
                &plan.context,
                &observation('a', 1200),
                1200
            )
            .is_err()
    );
    assert_eq!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1200)
            .unwrap()
            .phase,
        "expired"
    );
    let (_dir, mut ledger, plan, digest) = fixture();
    let r = ledger
        .reserve_maintenance(
            &plan.id,
            &digest,
            &plan.context,
            &observation('a', 1001),
            1001,
        )
        .unwrap();
    assert_eq!(
        ledger
            .reconcile_maintenance(&r, &plan.context, &observation('b', 1300), 1300)
            .unwrap()
            .phase,
        "stopped"
    );
}

#[test]
fn clock_watermark_survives_reopen_and_rejected_context_cannot_raise_it() {
    let (dir, mut ledger, plan, digest) = fixture();
    ledger
        .maintenance_status(&plan.id, &digest, &plan.context, 1002)
        .unwrap();
    let mut wrong = plan.context.clone();
    wrong.owner = "wrong".into();
    assert!(
        ledger
            .maintenance_status(&plan.id, &digest, &wrong, 1100)
            .is_err()
    );
    drop(ledger);
    let mut ledger = Ledger::open_private(dir.path().join("ledger.sqlite3")).unwrap();
    assert!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1001)
            .is_err()
    );
    assert!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1003)
            .is_ok()
    );
}

#[test]
fn altered_reservation_or_fabricated_time_cannot_complete_unknown_work() {
    let (_dir, mut ledger, plan, digest) = fixture();
    let r = ledger
        .reserve_maintenance(
            &plan.id,
            &digest,
            &plan.context,
            &observation('a', 1001),
            1001,
        )
        .unwrap();
    let mut changed = r.clone();
    changed.execution_authorized = true;
    assert!(
        ledger
            .reconcile_maintenance(&changed, &plan.context, &observation('b', 1002), 1002)
            .is_err()
    );
    changed = r.clone();
    changed.step.operation = "different-operation".into();
    assert!(
        ledger
            .reconcile_maintenance(&changed, &plan.context, &observation('b', 1002), 1002)
            .is_err()
    );
    for time in [900, 1100] {
        assert!(
            ledger
                .reconcile_maintenance(&r, &plan.context, &observation('b', time), 1002)
                .is_err()
        );
    }
    assert_eq!(
        ledger
            .maintenance_status(&plan.id, &digest, &plan.context, 1002)
            .unwrap()
            .phase,
        "unknown"
    );
}

#[test]
fn two_connections_can_reserve_a_step_only_once() {
    let (dir, ledger, plan, digest) = fixture();
    drop(ledger);
    let first = Ledger::open_private(dir.path().join("ledger.sqlite3")).unwrap();
    let second = Ledger::open_private(dir.path().join("ledger.sqlite3")).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let jobs = [first, second]
        .into_iter()
        .map(|mut ledger| {
            let barrier = barrier.clone();
            let plan = plan.clone();
            let digest = digest.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ledger
                    .reserve_maintenance(
                        &plan.id,
                        &digest,
                        &plan.context,
                        &observation('a', 1001),
                        1001,
                    )
                    .is_ok()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        jobs.into_iter()
            .filter_map(|job| job.join().ok())
            .filter(|ok| *ok)
            .count(),
        1
    );
}

#[test]
fn closed_plan_limits_reject_unknown_fields_duplicate_steps_and_oversized_batches() {
    let original = plan();
    let mut raw = serde_json::to_value(&original).unwrap();
    raw["execution_authorized"] = true.into();
    assert!(serde_json::from_value::<MaintenancePlan>(raw).is_err());
    for case in 0..7 {
        let mut p = original.clone();
        match case {
            0 => p.steps.clear(),
            1 => p.steps = vec![p.steps[0].clone(); 17],
            2 => p.steps[1].id = p.steps[0].id.clone(),
            3 => p.expires_at = p.issued_at + 3601,
            4 => p.max_runtime_seconds = 301,
            5 => p.context.artifact_sha256 = "short".into(),
            _ => p.steps[0].after_sha256 = p.steps[0].before_sha256.clone(),
        }
        assert!(maintenance_plan_digest(&p).is_err());
    }
}

#[test]
fn public_linked_or_aliased_ledger_paths_fail_without_permission_repair() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("ledger.sqlite3");
    fs::write(&path, b"untouched").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Ledger::open_private(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"untouched");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.path().join("hard");
    fs::hard_link(&path, &link).unwrap();
    assert!(Ledger::open_private(&path).is_err());
    fs::remove_file(link).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(dir.path().join("missing"), &path).unwrap();
    assert!(Ledger::open_private(&path).is_err());
    fs::remove_file(&path).unwrap();
    let alias = dir.path().join("alias");
    symlink(dir.path(), &alias).unwrap();
    assert!(Ledger::open_private(alias.join("other.sqlite3")).is_err());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Ledger::open_private(&path).is_err());
}

#[test]
fn writable_ancestor_cannot_replace_private_parent() {
    let outer = tempfile::tempdir().unwrap();
    fs::set_permissions(outer.path(), fs::Permissions::from_mode(0o777)).unwrap();
    let inner = outer.path().join("private");
    fs::create_dir(&inner).unwrap();
    fs::set_permissions(&inner, fs::Permissions::from_mode(0o700)).unwrap();
    let path = inner.join("ledger.sqlite3");
    assert!(Ledger::open_private(&path).is_err());
    assert!(!path.exists());
    fs::set_permissions(outer.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(Ledger::open_private(&path).is_ok());
}

#[test]
fn old_reconciliation_constructor_cannot_act_as_a_private_maintenance_ledger() {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let mut ledger = Ledger::open(dir.path().join("legacy.sqlite3")).unwrap();
    let p = plan();
    assert!(ledger.approve_maintenance(&p, &p.context, 1000).is_err());
}
