use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanAction {
    Apply,
    NoOp,
}

#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub schema: String,
    pub environment_id: String,
    pub desired_digest: String,
    pub previous_digest: Option<String>,
    pub action: PlanAction,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub operation_id: String,
    pub environment_id: String,
    pub desired_digest: String,
    pub driver: String,
    pub status: String,
    pub changed: bool,
    pub observed_at: String,
}
