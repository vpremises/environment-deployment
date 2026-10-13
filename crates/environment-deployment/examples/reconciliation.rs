//! Synthetic planning/ledger example. No effect adapter, network or installed timer.
use std::collections::BTreeMap;
use vpremises_environment_deployment::{
    Environment, Ledger, PlanAction, Profile, Receipt, ServiceRequirements, resolve,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile: Profile = toml::from_str(include_str!("../tests/fixtures/base.toml"))?;
    let service: ServiceRequirements =
        toml::from_str(include_str!("../tests/fixtures/service.toml"))?;
    let environment: Environment =
        toml::from_str(include_str!("../tests/fixtures/environment.toml"))?;
    let desired = resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile.clone())]),
        service.clone(),
        environment.clone(),
    )?;
    let ledger = Ledger::open(":memory:")?;
    let initial = ledger.plan(&desired)?;
    assert_eq!(initial.action, PlanAction::Apply);
    let receipt = Receipt {
        schema: "vpremises://infrastructure/apply-receipt/v1".into(),
        operation_id: "synthetic-001".into(),
        environment_id: initial.environment_id,
        desired_digest: initial.desired_digest,
        driver: "incus".into(),
        status: "applied".into(),
        changed: true,
        observed_at: "2026-07-28T00:00:00Z".into(),
    };
    let wrong = Receipt {
        desired_digest: "0".repeat(64),
        ..receipt.clone()
    };
    assert!(ledger.record(&desired, &wrong).is_err());
    assert_eq!(ledger.plan(&desired)?.action, PlanAction::Apply);
    assert!(ledger.record(&desired, &receipt)?);
    assert!(!ledger.record(&desired, &receipt)?);
    assert_eq!(ledger.plan(&desired)?.action, PlanAction::NoOp);
    let mut changed_environment = environment;
    changed_environment.maintenance.window = "Sun 04:00-05:00 UTC".into();
    let changed = resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile)]),
        service,
        changed_environment,
    )?;
    assert_eq!(ledger.plan(&changed)?.action, PlanAction::Apply);
    assert!(ledger.record(&changed, &receipt).is_err());
    let revised = Receipt {
        operation_id: "synthetic-002".into(),
        desired_digest: vpremises_environment_deployment::desired_digest(&changed)?,
        ..receipt
    };
    assert!(ledger.record(&changed, &revised)?);
    assert_eq!(ledger.plan(&desired)?.action, PlanAction::Apply);
    println!(
        "Synthetic plan/receipt/refusal/replay/change/rollback-plan passed; external state unobserved"
    );
    Ok(())
}
