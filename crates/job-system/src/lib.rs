//! Rust-owned Job lifecycle and execution facade.
//!
//! Models, policy, execution and test seams remain in the private `lifecycle`
//! module; the crate root exports only the stable Job contract.

mod lifecycle;

pub use lifecycle::*;
