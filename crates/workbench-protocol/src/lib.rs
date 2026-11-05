//! Canonical Rust transport contract for the desktop and ApplicationCore.
//!
//! The protocol implementation is kept in a private module while this crate
//! root remains the stable public facade for envelopes, DTOs and CoreTransport.

mod protocol;

pub use protocol::*;
