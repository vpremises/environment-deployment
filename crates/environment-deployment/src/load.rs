use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;

use crate::{Environment, Error, Profile, Result, ServiceRequirements};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONFIG_BYTES {
        return Err(Error::new(format!(
            "configuration must be a regular file no larger than 1 MiB: {}",
            path.display()
        )));
    }
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

/// Loads one closed environment overlay.
///
/// # Errors
///
/// Rejects missing, oversized, non-regular, malformed, or unknown-field input.
pub fn load_environment(path: impl AsRef<Path>) -> Result<Environment> {
    read(path.as_ref())
}

/// Loads one product-owned service requirement.
///
/// # Errors
///
/// Rejects missing, oversized, non-regular, malformed, or unknown-field input.
pub fn load_service(path: impl AsRef<Path>) -> Result<ServiceRequirements> {
    read(path.as_ref())
}

/// Loads a flat directory of uniquely identified profiles.
///
/// # Errors
///
/// Rejects inaccessible input, malformed profiles, and duplicate identifiers.
pub fn load_profile_directory(path: impl AsRef<Path>) -> Result<BTreeMap<String, Profile>> {
    let mut files: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|value| value == "toml"))
        .collect();
    files.sort();
    let mut profiles = BTreeMap::new();
    for file in files {
        let profile: Profile = read(&file)?;
        if profiles
            .insert(profile.profile_id.clone(), profile)
            .is_some()
        {
            return Err(Error::new("duplicate profile_id"));
        }
    }
    Ok(profiles)
}
