use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs};
use vpremises_environment_deployment::{
    Environment, MaintenanceBinding, MaintenanceWindow, Profile, ResolvedEnvironment,
    ServiceRequirements, desired_digest, render_maintenance_timer, resolve,
};

fn desired(window: &str) -> ResolvedEnvironment {
    let profile: Profile =
        toml::from_str(&fs::read_to_string("tests/fixtures/base.toml").unwrap()).unwrap();
    let service: ServiceRequirements =
        toml::from_str(&fs::read_to_string("tests/fixtures/service.toml").unwrap()).unwrap();
    let mut environment: Environment =
        toml::from_str(&fs::read_to_string("tests/fixtures/environment.toml").unwrap()).unwrap();
    environment.maintenance.window = window.into();
    resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile)]),
        service,
        environment,
    )
    .unwrap()
}

fn binding(desired: &ResolvedEnvironment) -> MaintenanceBinding {
    MaintenanceBinding {
        environment_id: desired.environment.environment_id.clone(),
        desired_digest: desired_digest(desired).unwrap(),
        inventory_path: format!(
            "/etc/vpremises/inventory/{}.ini",
            desired.environment.environment_id
        ),
        inventory_digest: format!("{:x}", Sha256::digest(b"reviewed-inventory")),
    }
}

#[test]
fn schedules_include_timezone_and_the_entire_jitter_stays_inside_window() {
    for zone in ["UTC", "Asia/Tokyo"] {
        let gateway = desired(&format!("Sun 02:00-03:00 {zone}"));
        let primary = desired(&format!("Sun 03:00-05:00 {zone}"));
        let timer =
            render_maintenance_timer(&gateway, &binding(&gateway), b"reviewed-inventory", 1800)
                .unwrap();
        assert!(timer.unit.contains(&format!("02:00:00 {zone}")));
        assert!(timer.unit.contains("RandomizedDelaySec=1800s"));
        assert!(!timer.execution_authorized);
        assert_ne!(
            timer.unit,
            render_maintenance_timer(&primary, &binding(&primary), b"reviewed-inventory", 1800)
                .unwrap()
                .unit
        );
        assert!(
            render_maintenance_timer(&gateway, &binding(&gateway), b"reviewed-inventory", 3599)
                .is_ok()
        );
        assert!(
            render_maintenance_timer(&gateway, &binding(&gateway), b"reviewed-inventory", 3600)
                .is_err()
        );
    }
}

#[test]
fn identity_inventory_and_configuration_mismatches_are_rejected() {
    let desired = desired("Sun 02:00-03:00 Asia/Tokyo");
    let valid = binding(&desired);
    for invalid in [
        MaintenanceBinding {
            environment_id: "other".into(),
            ..valid.clone()
        },
        MaintenanceBinding {
            desired_digest: "0".repeat(64),
            ..valid.clone()
        },
        MaintenanceBinding {
            inventory_path: "/etc/vpremises/inventory/other.ini".into(),
            ..valid.clone()
        },
        MaintenanceBinding {
            inventory_digest: "0".repeat(64),
            ..valid.clone()
        },
    ] {
        assert!(render_maintenance_timer(&desired, &invalid, b"reviewed-inventory", 0).is_err());
    }
    assert!(render_maintenance_timer(&desired, &valid, b"modified-inventory", 0).is_err());
}

#[test]
fn malformed_missing_and_ambiguous_windows_are_rejected() {
    for value in [
        "",
        "Sun 02:00-03:00",
        "Sun 2:00-03:00 UTC",
        "Sun 02:00-02:00 UTC",
        "Sun 23:00-01:00 UTC",
        "Sun 02:00-03:00 unknown",
        "Sun 02:00-24:00 UTC",
        "*-*-* 02:00-03:00 UTC",
        "Sun 02:00-03:00 UTC extra",
    ] {
        assert!(MaintenanceWindow::parse(value).is_err(), "{value}");
    }
}
