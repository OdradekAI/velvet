//! Plugin lifecycle facade.
//!
//! This crate owns extension lifecycle orchestration and host-owned capability
//! boundaries. Implementation details live in the private `host` module and
//! the owner-named storage, trust and runtime modules below.

mod host;
mod registry;

pub use host::*;
pub use registry::{
    ContributionRegistration, ContributionRegistrationHandle, ContributionRegistrySnapshot,
    CONTRIBUTION_REGISTRY_SCHEMA_VERSION, CONTRIBUTION_SCHEMA_VERSION,
};
