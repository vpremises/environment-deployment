use sha2::{Digest, Sha256};

use crate::{ResolvedEnvironment, Result};

pub fn resolved(environment: &ResolvedEnvironment) -> Result<String> {
    let encoded = serde_json::to_vec(environment)?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub fn bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
