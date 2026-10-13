use serde::{Deserialize, Serialize};

use super::{Component, Environment, ServiceRequirements};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResolvedProfile {
    pub profile_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResolvedEnvironment {
    pub schema: String,
    pub environment: Environment,
    pub service: ServiceRequirements,
    pub profiles: Vec<ResolvedProfile>,
    pub components: Vec<Component>,
}
