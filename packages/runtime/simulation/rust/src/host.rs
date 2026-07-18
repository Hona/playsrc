use std::{
    collections::VecDeque,
    error::Error,
    fmt::{self, Display, Formatter},
    sync::Arc,
    time::Instant,
};

use crate::{
    ClockFrame, ClockObservation, MAXIMUM_HOST_ELAPSED, MAXIMUM_TICK_INTERVAL,
    MINIMUM_TICK_INTERVAL, clock::WallClock,
};

type LimitField = (&'static str, fn(&Limits) -> usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_queued_commands: usize,
    pub max_queued_command_bytes: usize,
    pub max_pending_frames: usize,
    pub max_snapshot_bytes: usize,
    pub max_event_bytes_per_tick: usize,
    pub max_queued_publications: usize,
    pub max_queued_snapshot_bytes: usize,
    pub max_queued_event_batches: usize,
    pub max_queued_event_bytes: usize,
}

impl Limits {
    const FIELDS: [LimitField; 9] = [
        ("max_queued_commands", |limits| limits.max_queued_commands),
        ("max_queued_command_bytes", |limits| {
            limits.max_queued_command_bytes
        }),
        ("max_pending_frames", |limits| limits.max_pending_frames),
        ("max_snapshot_bytes", |limits| limits.max_snapshot_bytes),
        ("max_event_bytes_per_tick", |limits| {
            limits.max_event_bytes_per_tick
        }),
        ("max_queued_publications", |limits| {
            limits.max_queued_publications
        }),
        ("max_queued_snapshot_bytes", |limits| {
            limits.max_queued_snapshot_bytes
        }),
        ("max_queued_event_batches", |limits| {
            limits.max_queued_event_batches
        }),
        ("max_queued_event_bytes", |limits| {
            limits.max_queued_event_bytes
        }),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    tick_interval: f32,
    limits: Limits,
    maximum_ticks_per_host_frame: u32,
}

impl Configuration {
    pub fn new(tick_interval: f32, limits: Limits) -> Result<Self, ConfigurationError> {
        if !tick_interval.is_finite()
            || !(MINIMUM_TICK_INTERVAL..=MAXIMUM_TICK_INTERVAL).contains(&tick_interval)
        {
            return Err(ConfigurationError::InvalidTickInterval {
                bits: tick_interval.to_bits(),
            });
        }
        for (field, value) in Limits::FIELDS {
            if value(&limits) == 0 {
                return Err(ConfigurationError::ZeroLimit { field });
            }
        }
        if limits.max_snapshot_bytes > limits.max_queued_snapshot_bytes {
            return Err(ConfigurationError::InconsistentLimits {
                smaller: "max_queued_snapshot_bytes",
                larger: "max_snapshot_bytes",
            });
        }
        if limits.max_event_bytes_per_tick > limits.max_queued_event_bytes {
            return Err(ConfigurationError::InconsistentLimits {
                smaller: "max_queued_event_bytes",
                larger: "max_event_bytes_per_tick",
            });
        }

        let maximum_ticks_per_host_frame =
            (f64::from(MAXIMUM_HOST_ELAPSED) / f64::from(tick_interval)).ceil() as u32;
        if limits.max_queued_event_batches < maximum_ticks_per_host_frame as usize {
            return Err(ConfigurationError::FramePublicationCapacity {
                field: "max_queued_event_batches",
                required: maximum_ticks_per_host_frame as usize,
                actual: limits.max_queued_event_batches,
            });
        }
        let required_event_bytes = limits
            .max_event_bytes_per_tick
            .checked_mul(maximum_ticks_per_host_frame as usize)
            .ok_or(ConfigurationError::LimitArithmeticOverflow {
                field: "max_event_bytes_per_tick",
            })?;
        if limits.max_queued_event_bytes < required_event_bytes {
            return Err(ConfigurationError::FramePublicationCapacity {
                field: "max_queued_event_bytes",
                required: required_event_bytes,
                actual: limits.max_queued_event_bytes,
            });
        }
        Ok(Self {
            tick_interval,
            limits,
            maximum_ticks_per_host_frame,
        })
    }

    pub const fn tick_interval(self) -> f32 {
        self.tick_interval
    }

    pub const fn limits(self) -> Limits {
        self.limits
    }

    pub const fn maximum_ticks_per_host_frame(self) -> u32 {
        self.maximum_ticks_per_host_frame
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
    InvalidTickInterval {
        bits: u32,
    },
    ZeroLimit {
        field: &'static str,
    },
    InconsistentLimits {
        smaller: &'static str,
        larger: &'static str,
    },
    FramePublicationCapacity {
        field: &'static str,
        required: usize,
        actual: usize,
    },
    LimitArithmeticOverflow {
        field: &'static str,
    },
}

impl Display for ConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTickInterval { bits } => {
                write!(formatter, "invalid tick interval bits {bits:#010x}")
            }
            Self::ZeroLimit { field } => write!(formatter, "{field} must be positive"),
            Self::InconsistentLimits { smaller, larger } => {
                write!(formatter, "{smaller} cannot be smaller than {larger}")
            }
            Self::FramePublicationCapacity {
                field,
                required,
                actual,
            } => write!(
                formatter,
                "{field} is {actual}, but one maximum host frame requires {required}"
            ),
            Self::LimitArithmeticOverflow { field } => {
                write!(formatter, "{field} overflows one maximum host frame")
            }
        }
    }
}

impl Error for ConfigurationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub sequence: u64,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandReceipt {
    pub sequence: u64,
    pub queued_commands: usize,
    pub queued_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickContext {
    pub host_frame: u64,
    pub host_tick: u64,
    pub tick_index: u32,
    pub ticks_remaining: u32,
    pub final_tick: bool,
    pub simulating: bool,
    pub tick_interval: f32,
    pub first_tick_extra_sample: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TickInput<'a> {
    pub context: TickContext,
    pub commands: &'a [Command],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickOutput {
    pub snapshot: Arc<[u8]>,
    pub events: Arc<[u8]>,
}

impl TickOutput {
    pub fn new(snapshot: Vec<u8>, events: Vec<u8>) -> Self {
        Self {
            snapshot: Arc::from(snapshot.into_boxed_slice()),
            events: Arc::from(events.into_boxed_slice()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationError {
    pub code: String,
    pub detail: String,
}

impl SimulationError {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

impl Display for SimulationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl Error for SimulationError {}

pub trait Simulation: Send {
    fn advance(&mut self, input: TickInput<'_>) -> Result<TickOutput, SimulationError>;
    fn shutdown(&mut self) -> Result<(), SimulationError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventBatch {
    pub host_tick: u64,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Publication {
    pub host_frame: u64,
    pub first_host_tick: u64,
    pub last_host_tick: u64,
    pub selected_ticks: u32,
    pub interpolation: f32,
    pub snapshot: Arc<[u8]>,
    pub events: Vec<EventBatch>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpDisposition {
    Idle,
    Completed,
    Backpressured,
    Suspended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PumpReport {
    pub disposition: PumpDisposition,
    pub executed_ticks: u64,
    pub published_frames: u64,
    pub pending_frames: usize,
    pub pending_ticks: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Observation {
    Baseline {
        pump: PumpReport,
    },
    Suspended {
        pump: PumpReport,
    },
    Frame {
        host_frame: u64,
        clock: ClockFrame,
        previous_remainder: f64,
        remainder: f64,
        interpolation: f32,
        selected_ticks: u32,
        pump: PumpReport,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostLifecycle {
    Running,
    Faulted,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultCode {
    PendingFrameLimit,
    HostFrameOverflow,
    HostTickOverflow,
    Simulation,
    EmptySnapshot,
    SnapshotLimit,
    EventLimit,
    QueueArithmeticOverflow,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub code: FaultCode,
    pub host_frame: Option<u64>,
    pub host_tick: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HostStatus {
    pub lifecycle: HostLifecycle,
    pub paused: bool,
    pub suspended: bool,
    pub host_frame: u64,
    pub host_tick: u64,
    pub remainder: f64,
    pub interpolation: f32,
    pub queued_commands: usize,
    pub queued_command_bytes: usize,
    pub pending_frames: usize,
    pub pending_ticks: u64,
    pub queued_publications: usize,
    pub queued_snapshot_bytes: usize,
    pub queued_event_batches: usize,
    pub queued_event_bytes: usize,
    pub fault: Option<Fault>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostMetrics {
    pub clock_samples: u64,
    pub host_frames: u64,
    pub selected_ticks: u64,
    pub executed_ticks: u64,
    pub published_frames: u64,
    pub submitted_commands: u64,
    pub admitted_commands: u64,
    pub submitted_command_bytes: u64,
    pub snapshot_bytes: u64,
    pub event_bytes: u64,
    pub command_staging_nanoseconds: u64,
    pub command_staging_allocations: u64,
    pub command_staging_bytes: u64,
    pub simulation_nanoseconds: u64,
    pub publication_nanoseconds: u64,
    pub maximum_pending_frames: usize,
    pub maximum_pending_ticks: u64,
    pub maximum_publications: usize,
    pub maximum_snapshot_bytes: usize,
    pub maximum_event_batches: usize,
    pub maximum_event_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostError {
    NonFiniteClock,
    EmptyCommand,
    CommandCountLimit {
        attempted: usize,
        limit: usize,
    },
    CommandByteLimit {
        attempted: usize,
        limit: usize,
    },
    CommandSequenceOverflow,
    State {
        operation: &'static str,
        lifecycle: HostLifecycle,
    },
    Faulted(Fault),
    Shutdown(SimulationError),
}

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteClock => write!(formatter, "clock sample must be finite"),
            Self::EmptyCommand => write!(formatter, "command bytes must not be empty"),
            Self::CommandCountLimit { attempted, limit } => {
                write!(formatter, "command count {attempted} exceeds {limit}")
            }
            Self::CommandByteLimit { attempted, limit } => {
                write!(formatter, "command bytes {attempted} exceeds {limit}")
            }
            Self::CommandSequenceOverflow => write!(formatter, "command sequence overflow"),
            Self::State {
                operation,
                lifecycle,
            } => write!(formatter, "{operation} is invalid while {lifecycle:?}"),
            Self::Faulted(fault) => {
                write!(formatter, "host fault {:?}: {}", fault.code, fault.detail)
            }
            Self::Shutdown(error) => write!(formatter, "simulation shutdown failed: {error}"),
        }
    }
}

impl Error for HostError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownDisposition {
    Closed,
    AlreadyClosed,
}

#[derive(Debug)]
struct PendingFrame {
    sequence: u64,
    selected_ticks: u32,
    previous_remainder: f64,
    interpolation: f32,
    simulating: bool,
}

pub struct FixedStepHost<S: Simulation> {
    configuration: Configuration,
    simulation: S,
    clock: WallClock,
    lifecycle: HostLifecycle,
    paused: bool,
    suspended: bool,
    remainder: f64,
    host_frame: u64,
    host_tick: u64,
    next_command_sequence: u64,
    commands: Vec<Command>,
    queued_command_bytes: usize,
    pending_frames: VecDeque<PendingFrame>,
    pending_ticks: u64,
    publications: VecDeque<Publication>,
    queued_snapshot_bytes: usize,
    queued_event_batches: usize,
    queued_event_bytes: usize,
    fault: Option<Fault>,
    shutdown_called: bool,
    metrics: HostMetrics,
}

impl<S: Simulation> FixedStepHost<S> {
    pub fn new(configuration: Configuration, simulation: S) -> Self {
        Self {
            configuration,
            simulation,
            clock: WallClock::new(),
            lifecycle: HostLifecycle::Running,
            paused: false,
            suspended: false,
            remainder: 0.0,
            host_frame: 0,
            host_tick: 0,
            next_command_sequence: 1,
            commands: Vec::new(),
            queued_command_bytes: 0,
            pending_frames: VecDeque::new(),
            pending_ticks: 0,
            publications: VecDeque::new(),
            queued_snapshot_bytes: 0,
            queued_event_batches: 0,
            queued_event_bytes: 0,
            fault: None,
            shutdown_called: false,
            metrics: HostMetrics::default(),
        }
    }

    pub const fn configuration(&self) -> Configuration {
        self.configuration
    }

    pub const fn metrics(&self) -> HostMetrics {
        self.metrics
    }

    pub fn status(&self) -> HostStatus {
        HostStatus {
            lifecycle: self.lifecycle,
            paused: self.paused,
            suspended: self.suspended,
            host_frame: self.host_frame,
            host_tick: self.host_tick,
            remainder: self.remainder,
            interpolation: self.interpolation(),
            queued_commands: self.commands.len(),
            queued_command_bytes: self.queued_command_bytes,
            pending_frames: self.pending_frames.len(),
            pending_ticks: self.pending_ticks,
            queued_publications: self.publications.len(),
            queued_snapshot_bytes: self.queued_snapshot_bytes,
            queued_event_batches: self.queued_event_batches,
            queued_event_bytes: self.queued_event_bytes,
            fault: self.fault.clone(),
        }
    }

    pub fn submit(&mut self, bytes: &[u8]) -> Result<CommandReceipt, HostError> {
        self.ensure_running("submit")?;
        if bytes.is_empty() {
            return Err(HostError::EmptyCommand);
        }
        let limits = self.configuration.limits;
        let attempted_commands =
            self.commands
                .len()
                .checked_add(1)
                .ok_or(HostError::CommandCountLimit {
                    attempted: usize::MAX,
                    limit: limits.max_queued_commands,
                })?;
        if attempted_commands > limits.max_queued_commands {
            return Err(HostError::CommandCountLimit {
                attempted: attempted_commands,
                limit: limits.max_queued_commands,
            });
        }
        let attempted_bytes = self.queued_command_bytes.checked_add(bytes.len()).ok_or(
            HostError::CommandByteLimit {
                attempted: usize::MAX,
                limit: limits.max_queued_command_bytes,
            },
        )?;
        if attempted_bytes > limits.max_queued_command_bytes {
            return Err(HostError::CommandByteLimit {
                attempted: attempted_bytes,
                limit: limits.max_queued_command_bytes,
            });
        }
        let sequence = self.next_command_sequence;
        self.next_command_sequence = sequence
            .checked_add(1)
            .ok_or(HostError::CommandSequenceOverflow)?;
        self.commands.push(Command {
            sequence,
            bytes: Arc::from(bytes),
        });
        self.queued_command_bytes = attempted_bytes;
        self.metrics.submitted_commands = self.metrics.submitted_commands.saturating_add(1);
        self.metrics.submitted_command_bytes = self
            .metrics
            .submitted_command_bytes
            .saturating_add(bytes.len() as u64);
        Ok(CommandReceipt {
            sequence,
            queued_commands: attempted_commands,
            queued_bytes: attempted_bytes,
        })
    }

    pub fn set_paused(&mut self, paused: bool) -> Result<bool, HostError> {
        self.ensure_running("set_paused")?;
        let changed = self.paused != paused;
        self.paused = paused;
        Ok(changed)
    }

    pub fn set_suspended(&mut self, suspended: bool, now: f64) -> Result<bool, HostError> {
        self.ensure_running("set_suspended")?;
        if !now.is_finite() {
            return Err(HostError::NonFiniteClock);
        }
        let changed = self.suspended != suspended;
        self.suspended = suspended;
        self.clock.rebase(now);
        Ok(changed)
    }

    pub fn observe(&mut self, now: f64) -> Result<Observation, HostError> {
        self.ensure_running("observe")?;
        if !now.is_finite() {
            return Err(HostError::NonFiniteClock);
        }
        self.metrics.clock_samples = self.metrics.clock_samples.saturating_add(1);
        let duration_to_next_tick = self.duration_to_next_tick();
        match self
            .clock
            .observe(now, duration_to_next_tick, self.suspended)
        {
            ClockObservation::Baseline => Ok(Observation::Baseline { pump: self.pump()? }),
            ClockObservation::Suspended => Ok(Observation::Suspended { pump: self.pump()? }),
            ClockObservation::Frame(clock) => self.schedule_frame(clock),
        }
    }

    pub fn pump(&mut self) -> Result<PumpReport, HostError> {
        self.ensure_running("pump")?;
        if self.suspended {
            return Ok(self.pump_report(PumpDisposition::Suspended, 0, 0));
        }
        let mut executed_ticks = 0_u64;
        let mut published_frames = 0_u64;
        let mut disposition = PumpDisposition::Idle;

        while let Some(frame) = self.pending_frames.front() {
            if !self.publication_capacity(frame.selected_ticks)? {
                disposition = PumpDisposition::Backpressured;
                break;
            }
            let frame = self.pending_frames.pop_front().expect("pending frame");
            self.pending_ticks = self
                .pending_ticks
                .checked_sub(u64::from(frame.selected_ticks))
                .expect("pending tick accounting");
            let first_host_tick = self
                .host_tick
                .checked_add(1)
                .ok_or_else(|| self.set_fault(FaultCode::HostTickOverflow, "host tick overflow"))?;
            let mut latest_snapshot = None;
            let mut event_batches = Vec::with_capacity(frame.selected_ticks as usize);
            let mut event_bytes = 0_usize;
            let command_staging_started = Instant::now();
            let command_count = self.commands.len();
            self.metrics.command_staging_nanoseconds = self
                .metrics
                .command_staging_nanoseconds
                .saturating_add(duration_nanoseconds(command_staging_started.elapsed()));

            for tick_index in 0..frame.selected_ticks {
                let host_tick = self.host_tick.checked_add(1).ok_or_else(|| {
                    self.set_fault(FaultCode::HostTickOverflow, "host tick overflow")
                })?;
                let context = TickContext {
                    host_frame: frame.sequence,
                    host_tick,
                    tick_index,
                    ticks_remaining: frame.selected_ticks - tick_index,
                    final_tick: tick_index + 1 == frame.selected_ticks,
                    simulating: frame.simulating,
                    tick_interval: self.configuration.tick_interval,
                    first_tick_extra_sample: if tick_index == 0 {
                        frame.previous_remainder.max(0.0) as f32
                    } else {
                        0.0
                    },
                };
                let tick_commands: &[Command] = if tick_index == 0 { &self.commands } else { &[] };
                let started = Instant::now();
                let advanced = self.simulation.advance(TickInput {
                    context,
                    commands: tick_commands,
                });
                let output = match advanced {
                    Ok(output) => output,
                    Err(error) => {
                        return Err(self.set_fault(
                            FaultCode::Simulation,
                            format!("{}: {}", error.code, error.detail),
                        ));
                    }
                };
                self.metrics.simulation_nanoseconds = self
                    .metrics
                    .simulation_nanoseconds
                    .saturating_add(duration_nanoseconds(started.elapsed()));
                self.validate_output(&output)?;
                if tick_index == 0 {
                    self.metrics.admitted_commands = self
                        .metrics
                        .admitted_commands
                        .saturating_add(command_count as u64);
                    self.commands.clear();
                    self.queued_command_bytes = 0;
                }
                self.host_tick = host_tick;
                event_bytes = event_bytes
                    .checked_add(output.events.len())
                    .ok_or_else(|| {
                        self.set_fault(FaultCode::QueueArithmeticOverflow, "event byte overflow")
                    })?;
                self.metrics.snapshot_bytes = self
                    .metrics
                    .snapshot_bytes
                    .saturating_add(output.snapshot.len() as u64);
                self.metrics.event_bytes = self
                    .metrics
                    .event_bytes
                    .saturating_add(output.events.len() as u64);
                event_batches.push(EventBatch {
                    host_tick,
                    bytes: output.events,
                });
                latest_snapshot = Some(output.snapshot);
                executed_ticks += 1;
                self.metrics.executed_ticks = self.metrics.executed_ticks.saturating_add(1);
            }

            let snapshot = latest_snapshot.expect("positive selected tick count");
            let publication_started = Instant::now();
            self.queued_snapshot_bytes = self
                .queued_snapshot_bytes
                .checked_add(snapshot.len())
                .ok_or_else(|| {
                    self.set_fault(FaultCode::QueueArithmeticOverflow, "snapshot byte overflow")
                })?;
            self.queued_event_batches = self
                .queued_event_batches
                .checked_add(event_batches.len())
                .ok_or_else(|| {
                    self.set_fault(FaultCode::QueueArithmeticOverflow, "event batch overflow")
                })?;
            self.queued_event_bytes = self
                .queued_event_bytes
                .checked_add(event_bytes)
                .ok_or_else(|| {
                    self.set_fault(FaultCode::QueueArithmeticOverflow, "event queue overflow")
                })?;
            self.publications.push_back(Publication {
                host_frame: frame.sequence,
                first_host_tick,
                last_host_tick: self.host_tick,
                selected_ticks: frame.selected_ticks,
                interpolation: frame.interpolation,
                snapshot,
                events: event_batches,
            });
            self.metrics.publication_nanoseconds = self
                .metrics
                .publication_nanoseconds
                .saturating_add(duration_nanoseconds(publication_started.elapsed()));
            self.metrics.published_frames = self.metrics.published_frames.saturating_add(1);
            published_frames += 1;
            disposition = PumpDisposition::Completed;
            self.update_queue_maxima();
        }

        Ok(self.pump_report(disposition, executed_ticks, published_frames))
    }

    pub fn drain_publications(&mut self) -> Vec<Publication> {
        let publications = self.publications.drain(..).collect::<Vec<_>>();
        self.queued_snapshot_bytes = 0;
        self.queued_event_batches = 0;
        self.queued_event_bytes = 0;
        publications
    }

    pub fn shutdown(&mut self) -> Result<ShutdownDisposition, HostError> {
        if self.lifecycle == HostLifecycle::Closed {
            return Ok(ShutdownDisposition::AlreadyClosed);
        }
        if !self.shutdown_called {
            self.shutdown_called = true;
            if let Err(error) = self.simulation.shutdown() {
                self.fault = Some(Fault {
                    code: FaultCode::Shutdown,
                    host_frame: (self.host_frame != 0).then_some(self.host_frame),
                    host_tick: self.host_tick,
                    detail: format!("{}: {}", error.code, error.detail),
                });
                self.clear_queues();
                self.lifecycle = HostLifecycle::Closed;
                return Err(HostError::Shutdown(error));
            }
        }
        self.clear_queues();
        self.lifecycle = HostLifecycle::Closed;
        Ok(ShutdownDisposition::Closed)
    }

    fn schedule_frame(&mut self, clock: ClockFrame) -> Result<Observation, HostError> {
        let host_frame = self
            .host_frame
            .checked_add(1)
            .ok_or_else(|| self.set_fault(FaultCode::HostFrameOverflow, "host frame overflow"))?;
        let previous_remainder = self.remainder;
        let interval = f64::from(self.configuration.tick_interval);
        let accumulated = previous_remainder + f64::from(clock.admitted_elapsed);
        let selected_ticks_f64 = (accumulated / interval).floor();
        let selected_ticks = selected_ticks_f64 as u32;
        debug_assert!(selected_ticks <= self.configuration.maximum_ticks_per_host_frame);
        let remainder = accumulated - f64::from(selected_ticks) * interval;
        let interpolation = (remainder / interval) as f32;

        if selected_ticks > 0
            && self.pending_frames.len() >= self.configuration.limits.max_pending_frames
        {
            return Err(self.set_fault(
                FaultCode::PendingFrameLimit,
                format!(
                    "pending frame count {} reached {}; host frame {host_frame} selected {selected_ticks} ticks from admitted elapsed bits {:#010x}",
                    self.pending_frames.len(),
                    self.configuration.limits.max_pending_frames,
                    clock.admitted_elapsed.to_bits(),
                ),
            ));
        }

        self.host_frame = host_frame;
        self.remainder = remainder;
        self.metrics.host_frames = self.metrics.host_frames.saturating_add(1);
        self.metrics.selected_ticks = self
            .metrics
            .selected_ticks
            .saturating_add(u64::from(selected_ticks));
        if selected_ticks > 0 {
            self.pending_frames.push_back(PendingFrame {
                sequence: host_frame,
                selected_ticks,
                previous_remainder,
                interpolation,
                simulating: !self.paused,
            });
            self.pending_ticks = self
                .pending_ticks
                .checked_add(u64::from(selected_ticks))
                .ok_or_else(|| {
                    self.set_fault(FaultCode::QueueArithmeticOverflow, "pending tick overflow")
                })?;
            self.metrics.maximum_pending_frames = self
                .metrics
                .maximum_pending_frames
                .max(self.pending_frames.len());
            self.metrics.maximum_pending_ticks =
                self.metrics.maximum_pending_ticks.max(self.pending_ticks);
        }
        let pump = self.pump()?;
        Ok(Observation::Frame {
            host_frame,
            clock,
            previous_remainder,
            remainder,
            interpolation,
            selected_ticks,
            pump,
        })
    }

    fn publication_capacity(&mut self, selected_ticks: u32) -> Result<bool, HostError> {
        let limits = self.configuration.limits;
        let publications = self.publications.len().checked_add(1).ok_or_else(|| {
            self.set_fault(
                FaultCode::QueueArithmeticOverflow,
                "publication count overflow",
            )
        })?;
        let snapshots = self
            .queued_snapshot_bytes
            .checked_add(limits.max_snapshot_bytes)
            .ok_or_else(|| {
                self.set_fault(
                    FaultCode::QueueArithmeticOverflow,
                    "snapshot reserve overflow",
                )
            })?;
        let event_batches = self
            .queued_event_batches
            .checked_add(selected_ticks as usize)
            .ok_or_else(|| {
                self.set_fault(
                    FaultCode::QueueArithmeticOverflow,
                    "event batch reserve overflow",
                )
            })?;
        let frame_event_bytes = limits
            .max_event_bytes_per_tick
            .checked_mul(selected_ticks as usize)
            .ok_or_else(|| {
                self.set_fault(FaultCode::QueueArithmeticOverflow, "event reserve overflow")
            })?;
        let event_bytes = self
            .queued_event_bytes
            .checked_add(frame_event_bytes)
            .ok_or_else(|| {
                self.set_fault(
                    FaultCode::QueueArithmeticOverflow,
                    "event reserve queue overflow",
                )
            })?;
        Ok(publications <= limits.max_queued_publications
            && snapshots <= limits.max_queued_snapshot_bytes
            && event_batches <= limits.max_queued_event_batches
            && event_bytes <= limits.max_queued_event_bytes)
    }

    fn validate_output(&mut self, output: &TickOutput) -> Result<(), HostError> {
        if output.snapshot.is_empty() {
            return Err(self.set_fault(
                FaultCode::EmptySnapshot,
                "simulation returned an empty snapshot",
            ));
        }
        if output.snapshot.len() > self.configuration.limits.max_snapshot_bytes {
            return Err(self.set_fault(
                FaultCode::SnapshotLimit,
                format!(
                    "snapshot bytes {} exceeded {}",
                    output.snapshot.len(),
                    self.configuration.limits.max_snapshot_bytes
                ),
            ));
        }
        if output.events.len() > self.configuration.limits.max_event_bytes_per_tick {
            return Err(self.set_fault(
                FaultCode::EventLimit,
                format!(
                    "event bytes {} exceeded {}",
                    output.events.len(),
                    self.configuration.limits.max_event_bytes_per_tick
                ),
            ));
        }
        Ok(())
    }

    fn duration_to_next_tick(&self) -> f32 {
        (f64::from(self.configuration.tick_interval) - self.remainder) as f32
    }

    fn interpolation(&self) -> f32 {
        (self.remainder / f64::from(self.configuration.tick_interval)) as f32
    }

    fn pump_report(
        &self,
        disposition: PumpDisposition,
        executed_ticks: u64,
        published_frames: u64,
    ) -> PumpReport {
        PumpReport {
            disposition,
            executed_ticks,
            published_frames,
            pending_frames: self.pending_frames.len(),
            pending_ticks: self.pending_ticks,
        }
    }

    fn update_queue_maxima(&mut self) {
        self.metrics.maximum_publications = self
            .metrics
            .maximum_publications
            .max(self.publications.len());
        self.metrics.maximum_snapshot_bytes = self
            .metrics
            .maximum_snapshot_bytes
            .max(self.queued_snapshot_bytes);
        self.metrics.maximum_event_batches = self
            .metrics
            .maximum_event_batches
            .max(self.queued_event_batches);
        self.metrics.maximum_event_bytes = self
            .metrics
            .maximum_event_bytes
            .max(self.queued_event_bytes);
    }

    fn ensure_running(&self, operation: &'static str) -> Result<(), HostError> {
        match self.lifecycle {
            HostLifecycle::Running => Ok(()),
            HostLifecycle::Faulted => Err(HostError::Faulted(
                self.fault.clone().expect("faulted host has a fault"),
            )),
            HostLifecycle::Closed => Err(HostError::State {
                operation,
                lifecycle: HostLifecycle::Closed,
            }),
        }
    }

    fn set_fault(&mut self, code: FaultCode, detail: impl Into<String>) -> HostError {
        let fault = Fault {
            code,
            host_frame: (self.host_frame != 0).then_some(self.host_frame),
            host_tick: self.host_tick,
            detail: detail.into(),
        };
        self.lifecycle = HostLifecycle::Faulted;
        self.fault = Some(fault.clone());
        HostError::Faulted(fault)
    }

    fn clear_queues(&mut self) {
        self.commands.clear();
        self.queued_command_bytes = 0;
        self.pending_frames.clear();
        self.pending_ticks = 0;
        self.publications.clear();
        self.queued_snapshot_bytes = 0;
        self.queued_event_batches = 0;
        self.queued_event_bytes = 0;
    }
}

fn duration_nanoseconds(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}
