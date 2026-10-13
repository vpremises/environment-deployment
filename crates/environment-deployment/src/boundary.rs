use std::collections::BTreeMap;

use crate::{
    Environment, Error, Result, ServiceRequirements,
    model::{Component, Driver},
};

pub fn reject_secret_settings(component: &Component) -> Result<()> {
    if component.settings.keys().any(|key| {
        key == "password"
            || key == "private_key"
            || key == "credential_value"
            || key.ends_with("_secret")
            || key.ends_with("_token")
    }) {
        return Err(Error::new(format!(
            "component {} contains a secret-like setting",
            component.id
        )));
    }
    Ok(())
}

pub fn validate_driver(environment: &Environment) -> Result<()> {
    match environment.driver {
        Driver::Incus if environment.incus.is_none() => {
            Err(Error::new("Incus driver requires an incus placement"))
        }
        Driver::Native if environment.incus.is_some() => Err(Error::new(
            "native driver cannot contain an incus placement",
        )),
        _ => Ok(()),
    }
}

pub fn enforce(
    service: &ServiceRequirements,
    environment: &Environment,
    components: &BTreeMap<String, Component>,
) -> Result<()> {
    if service.process.user == "root" {
        return Err(Error::new("managed service cannot run as root"));
    }
    let edge = components
        .values()
        .any(|value| value.kind == "edge-gateway");
    if !matches!(service.process.execution.as_str(), "service" | "oneshot") {
        return Err(Error::new("process execution must be service or oneshot"));
    }
    if let Some(bind) = &service.process.bind {
        let loopback = bind.starts_with("127.0.0.1:") || bind.starts_with("[::1]:");
        if !loopback && !edge {
            return Err(Error::new("non-gateway service must bind to loopback"));
        }
    }
    if environment.network.public_listener && !edge {
        return Err(Error::new(
            "public_listener requires an edge-gateway profile",
        ));
    }
    Ok(())
}
