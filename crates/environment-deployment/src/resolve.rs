use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Environment, Error, Profile, ResolvedEnvironment, Result, ServiceRequirements, boundary,
    model::{Component, ResolvedProfile},
};

const PROFILE_SCHEMA: &str = "vpremises://infrastructure/profile/v1";
const SERVICE_SCHEMA: &str = "vpremises://infrastructure/service-requirements/v1";
const ENVIRONMENT_SCHEMA: &str = "vpremises://infrastructure/environment/v1";

/// Composes profiles, product requirements, and placement into one closed document.
///
/// # Errors
///
/// Rejects missing profiles, conflicts, boundary violations, and incompatible drivers.
pub fn resolve(
    profiles: &BTreeMap<String, Profile>,
    service: ServiceRequirements,
    environment: Environment,
) -> Result<ResolvedEnvironment> {
    validate_headers(&service, &environment)?;
    crate::MaintenanceWindow::parse(&environment.maintenance.window)?;
    let refs = ordered_refs(&service, &environment);
    let mut selected = Vec::new();
    let mut components = BTreeMap::<String, Component>::new();
    for profile_ref in refs {
        let profile = profiles
            .get(&profile_ref)
            .ok_or_else(|| Error::new(format!("unknown profile: {profile_ref}")))?;
        if profile.schema != PROFILE_SCHEMA {
            return Err(Error::new(format!(
                "unsupported profile schema: {profile_ref}"
            )));
        }
        selected.push(ResolvedProfile {
            profile_id: profile.profile_id.clone(),
            version: profile.version.clone(),
        });
        merge_components(&mut components, &profile.components)?;
    }
    boundary::enforce(&service, &environment, &components)?;
    Ok(ResolvedEnvironment {
        schema: "vpremises://infrastructure/resolved-environment/v1".into(),
        environment,
        service,
        profiles: selected,
        components: components.into_values().collect(),
    })
}

fn validate_headers(service: &ServiceRequirements, environment: &Environment) -> Result<()> {
    if service.schema != SERVICE_SCHEMA || environment.schema != ENVIRONMENT_SCHEMA {
        return Err(Error::new("unsupported service or environment schema"));
    }
    if service.service_ref != environment.service_ref {
        return Err(Error::new(
            "environment service_ref does not match service manifest",
        ));
    }
    if environment.resources.cpu == 0
        || environment.resources.memory_mib < 128
        || environment.resources.root_disk_gib == 0
    {
        return Err(Error::new("resource limits must be finite and non-zero"));
    }
    boundary::validate_driver(environment)?;
    Ok(())
}

fn ordered_refs(service: &ServiceRequirements, environment: &Environment) -> Vec<String> {
    let mut seen = BTreeSet::new();
    environment
        .extends
        .iter()
        .chain(service.extends.iter())
        .filter(|value| seen.insert((*value).clone()))
        .cloned()
        .collect()
}

fn merge_components(
    merged: &mut BTreeMap<String, Component>,
    additions: &[Component],
) -> Result<()> {
    for component in additions {
        if let Some(existing) = merged.get(&component.id) {
            if existing != component {
                return Err(Error::new(format!(
                    "profile component conflict: {}",
                    component.id
                )));
            }
        } else {
            boundary::reject_secret_settings(component)?;
            merged.insert(component.id.clone(), component.clone());
        }
    }
    Ok(())
}
