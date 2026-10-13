use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub profile_id: String,
    pub version: String,
    #[serde(default)]
    pub components: Vec<Component>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Process {
    pub user: String,
    pub execution: String,
    pub bind: Option<String>,
    pub artifact: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub directory: String,
    pub backup_required: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    pub path: String,
    pub timeout_seconds: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceRequirements {
    pub schema: String,
    pub service_id: String,
    pub service_ref: String,
    pub extends: Vec<String>,
    pub process: Process,
    pub state: State,
    pub health: Health,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Driver {
    Native,
    Incus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceType {
    Native,
    SystemContainer,
    VirtualMachine,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub cpu: u16,
    pub memory_mib: u32,
    pub root_disk_gib: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub profile: String,
    pub public_listener: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Maintenance {
    pub window: String,
    pub update_policy: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncusPlacement {
    pub instance_name: String,
    pub project: String,
    pub image: String,
    pub storage_pool: String,
    pub network: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub schema: String,
    pub environment_id: String,
    pub service_ref: String,
    pub driver: Driver,
    pub instance_type: InstanceType,
    #[serde(default)]
    pub extends: Vec<String>,
    pub resources: Resources,
    pub network: Network,
    pub maintenance: Maintenance,
    pub incus: Option<IncusPlacement>,
}
