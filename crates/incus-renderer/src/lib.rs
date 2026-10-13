//! Converts a resolved vPremises environment into provider-owned `OpenTofu` JSON.

mod render;
mod validate;

pub use render::render;
pub use validate::validate;
pub use vpremises_environment_deployment::ResolvedEnvironment;

pub type Result<T> = std::result::Result<T, String>;
