//! Deterministic recorded-state Replay authority.
//!
//! Replay applies only caller-decoded recorded operations. No gameplay
//! simulation, movement, physics, prediction, or inferred state interface is
//! accepted by this crate.

mod model;
mod session;

pub use model::*;
pub use session::*;
