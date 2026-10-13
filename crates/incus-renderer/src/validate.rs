use crate::{ResolvedEnvironment, Result};

const RESOLVED_SCHEMA: &str = "vpremises://infrastructure/resolved-environment/v1";

/// Validates the strict subset accepted by the Incus renderer.
///
/// # Errors
///
/// Rejects invalid names, non-cloud images, missing placement, or unsafe resources.
pub fn validate(resolved: &ResolvedEnvironment) -> Result<()> {
    if resolved.schema != RESOLVED_SCHEMA
        || !matches!(
            resolved.environment.driver,
            vpremises_environment_deployment::Driver::Incus
        )
    {
        return Err("input must be a resolved Incus environment".into());
    }
    let incus = resolved
        .environment
        .incus
        .as_ref()
        .ok_or("missing Incus placement")?;
    validate_name(&incus.instance_name)?;
    validate_name(&incus.project)?;
    if !incus.image.starts_with("images:") || !incus.image.ends_with("/cloud") {
        return Err("Incus image must be an images: cloud-init image".into());
    }
    if incus.storage_pool.is_empty() || incus.network.is_empty() {
        return Err("Incus storage pool and network are required".into());
    }
    if resolved.environment.resources.cpu == 0
        || resolved.environment.resources.memory_mib < 128
        || resolved.environment.resources.root_disk_gib == 0
    {
        return Err("Incus resources must be finite and non-zero".into());
    }
    let user = &resolved.service.process.user;
    if user == "root"
        || !(1..=32).contains(&user.len())
        || !user.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
        || !user
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || *byte == b'_')
    {
        return Err("service user must be a non-root account name: [a-z_][a-z0-9_-]{0,31}".into());
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<()> {
    let valid = (1..=63).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric);
    if valid {
        Ok(())
    } else {
        Err(format!("invalid Incus DNS-compatible name: {value}"))
    }
}
