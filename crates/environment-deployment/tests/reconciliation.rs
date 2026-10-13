use std::{collections::BTreeMap, fs};

use vpremises_environment_deployment::{
    Environment, Ledger, PlanAction, Profile, Receipt, ServiceRequirements, resolve,
};

fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
    toml::from_str(&fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()).unwrap()
}

fn desired() -> vpremises_environment_deployment::ResolvedEnvironment {
    let profile: Profile = fixture("base.toml");
    resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile)]),
        fixture::<ServiceRequirements>("service.toml"),
        fixture::<Environment>("environment.toml"),
    )
    .unwrap()
}

#[test]
fn records_once_and_then_plans_noop() {
    let directory =
        std::env::temp_dir().join(format!("vpremises-infrastructure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let ledger = Ledger::open(directory.join("ledger.sqlite3")).unwrap();
    let desired = desired();
    let initial = ledger.plan(&desired).unwrap();
    assert_eq!(initial.action, PlanAction::Apply);
    let receipt = Receipt {
        schema: "vpremises://infrastructure/apply-receipt/v1".into(),
        operation_id: "operation-001".into(),
        environment_id: initial.environment_id,
        desired_digest: initial.desired_digest,
        driver: "incus".into(),
        status: "applied".into(),
        changed: true,
        observed_at: "2026-07-28T00:00:00Z".into(),
    };
    for invalid in [
        Receipt {
            driver: "native".into(),
            ..receipt.clone()
        },
        Receipt {
            operation_id: String::new(),
            ..receipt.clone()
        },
        Receipt {
            operation_id: "operation\n001".into(),
            ..receipt.clone()
        },
        Receipt {
            observed_at: "not-a-time".into(),
            ..receipt.clone()
        },
        Receipt {
            observed_at: "2999-01-01T00:00:00Z".into(),
            ..receipt.clone()
        },
    ] {
        assert!(ledger.record(&desired, &invalid).is_err());
        assert_eq!(ledger.plan(&desired).unwrap().action, PlanAction::Apply);
    }
    assert!(ledger.record(&desired, &receipt).unwrap());
    assert!(!ledger.record(&desired, &receipt).unwrap());
    assert_eq!(ledger.plan(&desired).unwrap().action, PlanAction::NoOp);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn rejects_public_listener_without_gateway_profile() {
    let profile: Profile = fixture("base.toml");
    let mut environment: Environment = fixture("environment.toml");
    environment.network.public_listener = true;
    let result = resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile)]),
        fixture::<ServiceRequirements>("service.toml"),
        environment,
    );
    assert!(result.is_err());
}
