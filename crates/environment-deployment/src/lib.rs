//! Resolves versioned infrastructure requirements without owning provider credentials.

mod boundary;
mod digest;
mod error;
mod load;
mod maintenance;
mod model;
mod resolve;
mod state;

pub use error::{Error, Result};
pub use load::{load_environment, load_profile_directory, load_service};
pub use maintenance::{
    MaintenanceBinding, MaintenanceTimer, MaintenanceWindow, render_maintenance_timer,
};
pub use model::{
    Component, Driver, Environment, Health, IncusPlacement, InstanceType, Maintenance, Network,
    Plan, PlanAction, Process, Profile, Receipt, ResolvedEnvironment, Resources,
    ServiceRequirements, State,
};
pub use resolve::resolve;
pub use state::Ledger;

/// Computes the stable digest attested by driver receipts.
///
/// # Errors
///
/// Returns an error when the resolved document cannot be encoded.
pub fn desired_digest(environment: &ResolvedEnvironment) -> Result<String> {
    digest::resolved(environment)
}
