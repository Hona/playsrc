//! Runtime-neutral fixed-step scheduling for one gameplay authority.
//!
//! The scheduler owns elapsed-time filtering, tick accumulation, command staging,
//! pause/suspension, immutable publication, bounds, faults, and shutdown. A game
//! adapter owns every gameplay transition behind [`Simulation`].

mod clock;
mod host;

pub use clock::{ClockFrame, ClockObservation, ElapsedAdjustment};
pub use host::{
    Command, CommandReceipt, Configuration, ConfigurationError, EventBatch, Fault, FaultCode,
    FixedStepHost, HostError, HostLifecycle, HostMetrics, HostStatus, Limits, Observation,
    Publication, PumpDisposition, PumpReport, ShutdownDisposition, Simulation, SimulationError,
    TickContext, TickInput, TickOutput,
};

/// Valve Source SDK 2013 `src/public/const.h` minimum simulation interval.
pub const MINIMUM_TICK_INTERVAL: f32 = 0.001;
/// Valve Source SDK 2013 `src/public/const.h` maximum simulation interval.
pub const MAXIMUM_TICK_INTERVAL: f32 = 0.1;
/// Valve Source SDK 2013 `src/public/const.h` default simulation interval.
pub const DEFAULT_TICK_INTERVAL: f32 = 0.015;

/// Ordinary host callbacks contribute at least one millisecond to the accumulator.
pub const MINIMUM_HOST_ELAPSED: f32 = 0.001;
/// Ordinary host callbacks contribute at most one tenth of a second to the accumulator.
pub const MAXIMUM_HOST_ELAPSED: f32 = 0.1;
