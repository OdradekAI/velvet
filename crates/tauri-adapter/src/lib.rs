//! Framework-free adapter between Tauri commands/events and CoreTransport.
//!
//! The implementation lives in `transport`; native signal integration remains
//! in its owner-named module and no Tauri framework types cross this crate.

mod transport;

pub use transport::*;
