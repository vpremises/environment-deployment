use std::collections::BTreeMap;

use vpremises_environment_deployment::{Environment, Profile, ServiceRequirements, resolve};

#[test]
fn renders_the_closed_resolver_output_without_a_protocol_adapter() {
    let profile: Profile = toml::from_str(include_str!(
        "../../environment-deployment/tests/fixtures/base.toml"
    ))
    .unwrap();
    let service: ServiceRequirements = toml::from_str(include_str!(
        "../../environment-deployment/tests/fixtures/service.toml"
    ))
    .unwrap();
    let environment: Environment = toml::from_str(include_str!(
        "../../environment-deployment/tests/fixtures/environment.toml"
    ))
    .unwrap();
    let resolved = resolve(
        &BTreeMap::from([(profile.profile_id.clone(), profile)]),
        service,
        environment,
    )
    .unwrap();
    let rendered = vpremises_incus_driver::render(&resolved).unwrap();
    assert_eq!(
        rendered["resource"]["incus_instance"]["managed"]["name"],
        "fixture-primary"
    );
    let mut tampered = serde_json::to_value(&resolved).unwrap();
    tampered["environment"]["unexpected"] = true.into();
    assert!(
        serde_json::from_value::<vpremises_environment_deployment::ResolvedEnvironment>(tampered)
            .is_err()
    );
}
