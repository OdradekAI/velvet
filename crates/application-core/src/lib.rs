//! Application composition and transport facade.
//!
//! The implementation lives in the private `composition` module so this crate root
//! remains the stable public facade. Domain owners stay in their respective
//! crates; this crate only composes them and coordinates transport-facing work.

mod composition;

pub use composition::*;
