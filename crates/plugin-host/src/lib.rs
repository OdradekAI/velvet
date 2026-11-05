//! Plugin lifecycle facade.
//!
//! This crate owns extension lifecycle orchestration and host-owned capability
//! boundaries. Implementation details live in the private `host` module and
//! the owner-named storage, trust and runtime modules below.

mod host;

pub use host::*;
