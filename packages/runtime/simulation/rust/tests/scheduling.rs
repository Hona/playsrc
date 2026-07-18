use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use playsrc_simulation::{
    Configuration, ConfigurationError, DEFAULT_TICK_INTERVAL, ElapsedAdjustment, FaultCode,
    FixedStepHost, HostError, HostLifecycle, Limits, MAXIMUM_TICK_INTERVAL, MINIMUM_TICK_INTERVAL,
    MetricsClock, Observation, Publication, PumpDisposition, Simulation, SimulationError,
    TickInput, TickOutput,
};

#[derive(Clone, Debug)]
struct DeterministicSimulation {
    state: u64,
    fail_at: Option<u64>,
    snapshot_bytes: usize,
    event_bytes: usize,
    shutdowns: Arc<AtomicUsize>,
}

struct StepMetricsClock {
    nanoseconds: u64,
    step: u64,
}

impl MetricsClock for StepMetricsClock {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        let value = self.nanoseconds;
        self.nanoseconds = self.nanoseconds.saturating_add(self.step);
        value
    }
}

impl DeterministicSimulation {
    fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
            fail_at: None,
            snapshot_bytes: 48,
            event_bytes: 32,
            shutdowns: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Simulation for DeterministicSimulation {
    fn advance(&mut self, input: TickInput<'_>) -> Result<TickOutput, SimulationError> {
        if self.fail_at == Some(input.context.host_tick) {
            return Err(SimulationError::new("Injected", "fixed failure"));
        }
        mix(&mut self.state, input.context.host_frame);
        mix(&mut self.state, input.context.host_tick);
        mix(&mut self.state, u64::from(input.context.tick_index));
        mix(&mut self.state, u64::from(input.context.ticks_remaining));
        mix(&mut self.state, u64::from(input.context.simulating));
        for command in input.commands {
            mix(&mut self.state, command.sequence);
            for byte in command.bytes.iter() {
                mix(&mut self.state, u64::from(*byte));
            }
        }

        let mut snapshot = vec![0; self.snapshot_bytes];
        write_u64(&mut snapshot, 0, self.state);
        write_u64(&mut snapshot, 8, input.context.host_frame);
        write_u64(&mut snapshot, 16, input.context.host_tick);
        write_u32(&mut snapshot, 24, input.context.tick_index);
        write_u32(&mut snapshot, 28, input.context.ticks_remaining);
        write_u32(&mut snapshot, 32, input.context.tick_interval.to_bits());
        write_u32(
            &mut snapshot,
            36,
            input.context.first_tick_extra_sample.to_bits(),
        );
        write_u32(&mut snapshot, 40, input.commands.len() as u32);
        snapshot[44] = u8::from(input.context.final_tick);
        snapshot[45] = u8::from(input.context.simulating);

        let mut events = vec![0; self.event_bytes];
        write_u64(&mut events, 0, input.context.host_tick);
        write_u64(&mut events, 8, self.state);
        write_u32(&mut events, 16, input.context.tick_index);
        write_u32(&mut events, 20, input.context.ticks_remaining);
        write_u32(&mut events, 24, input.commands.len() as u32);
        events[28] = u8::from(input.context.final_tick);
        events[29] = u8::from(input.context.simulating);
        Ok(TickOutput::new(snapshot, events))
    }

    fn shutdown(&mut self) -> Result<(), SimulationError> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn mix(state: &mut u64, value: u64) {
    *state ^= value;
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    if offset + 8 <= bytes.len() {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    if offset + 4 <= bytes.len() {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn limits() -> Limits {
    Limits {
        max_queued_commands: 64,
        max_queued_command_bytes: 4 * 1024,
        max_pending_frames: 256,
        max_snapshot_bytes: 1024,
        max_event_bytes_per_tick: 256,
        max_queued_publications: 256,
        max_queued_snapshot_bytes: 256 * 1024,
        max_queued_event_batches: 2048,
        max_queued_event_bytes: 512 * 1024,
    }
}

fn configuration() -> Configuration {
    Configuration::new(DEFAULT_TICK_INTERVAL, limits()).expect("configuration")
}

fn frame(observation: Observation) -> (u32, u32, f64, f32, PumpDisposition) {
    let Observation::Frame {
        clock,
        selected_ticks: _,
        remainder,
        interpolation,
        pump,
        ..
    } = observation
    else {
        panic!("expected frame")
    };
    (
        clock.raw_elapsed.to_bits(),
        clock.admitted_elapsed.to_bits(),
        remainder,
        interpolation,
        pump.disposition,
    )
}

#[test]
fn validates_complete_configuration_before_construction() {
    assert_eq!(DEFAULT_TICK_INTERVAL.to_bits(), 0x3c75_c28f);
    assert_eq!(configuration().maximum_ticks_per_host_frame(), 7);
    assert!(Configuration::new(MINIMUM_TICK_INTERVAL, limits()).is_ok());
    assert!(Configuration::new(MAXIMUM_TICK_INTERVAL, limits()).is_ok());
    assert!(matches!(
        Configuration::new(f32::NAN, limits()),
        Err(ConfigurationError::InvalidTickInterval { .. })
    ));
    assert!(matches!(
        Configuration::new(0.000_999, limits()),
        Err(ConfigurationError::InvalidTickInterval { .. })
    ));
    assert!(matches!(
        Configuration::new(0.100_001, limits()),
        Err(ConfigurationError::InvalidTickInterval { .. })
    ));
    let zero_limits = [
        (
            "max_queued_commands",
            Limits {
                max_queued_commands: 0,
                ..limits()
            },
        ),
        (
            "max_queued_command_bytes",
            Limits {
                max_queued_command_bytes: 0,
                ..limits()
            },
        ),
        (
            "max_pending_frames",
            Limits {
                max_pending_frames: 0,
                ..limits()
            },
        ),
        (
            "max_snapshot_bytes",
            Limits {
                max_snapshot_bytes: 0,
                ..limits()
            },
        ),
        (
            "max_event_bytes_per_tick",
            Limits {
                max_event_bytes_per_tick: 0,
                ..limits()
            },
        ),
        (
            "max_queued_publications",
            Limits {
                max_queued_publications: 0,
                ..limits()
            },
        ),
        (
            "max_queued_snapshot_bytes",
            Limits {
                max_queued_snapshot_bytes: 0,
                ..limits()
            },
        ),
        (
            "max_queued_event_batches",
            Limits {
                max_queued_event_batches: 0,
                ..limits()
            },
        ),
        (
            "max_queued_event_bytes",
            Limits {
                max_queued_event_bytes: 0,
                ..limits()
            },
        ),
    ];
    for (field, zero) in zero_limits {
        assert_eq!(
            Configuration::new(DEFAULT_TICK_INTERVAL, zero),
            Err(ConfigurationError::ZeroLimit { field })
        );
    }
    let mut short = limits();
    short.max_queued_event_batches = 6;
    assert!(matches!(
        Configuration::new(DEFAULT_TICK_INTERVAL, short),
        Err(ConfigurationError::FramePublicationCapacity {
            field: "max_queued_event_batches",
            required: 7,
            actual: 6
        })
    ));
}

#[test]
fn filters_elapsed_and_retains_source_fixed_step_arithmetic() {
    let mut host = FixedStepHost::new(configuration(), DeterministicSimulation::new());
    assert!(matches!(
        host.observe(0.0).unwrap(),
        Observation::Baseline { .. }
    ));

    let same = host.observe(0.0).unwrap();
    let (raw, admitted, remainder, _, disposition) = frame(same);
    assert_eq!(raw, 0.0_f32.to_bits());
    assert_eq!(admitted, 0.001_f32.to_bits());
    assert_eq!(remainder, f64::from(0.001_f32));
    assert_eq!(disposition, PumpDisposition::Idle);

    let long = host.observe(10.0).unwrap();
    let Observation::Frame {
        clock,
        selected_ticks,
        pump,
        ..
    } = long
    else {
        panic!("frame")
    };
    assert_eq!(clock.adjustment, ElapsedAdjustment::ClampedToMaximum);
    assert_eq!(clock.admitted_elapsed.to_bits(), 0.1_f32.to_bits());
    assert_eq!(selected_ticks, 6);
    assert_eq!(pump.executed_ticks, 6);
    assert_eq!(host.status().host_tick, 6);

    let next = host.observe(10.1).unwrap();
    let Observation::Frame { selected_ticks, .. } = next else {
        panic!("frame")
    };
    assert_eq!(selected_ticks, 7);
    assert_eq!(host.status().host_tick, 13);
}

#[test]
fn clock_reversal_advances_exactly_to_the_next_tick() {
    let mut host = FixedStepHost::new(configuration(), DeterministicSimulation::new());
    host.observe(5.0).unwrap();
    let first = host.observe(5.01).unwrap();
    assert_eq!(frame(first).4, PumpDisposition::Idle);
    let before = host.status();
    assert_eq!(before.host_tick, 0);

    let Observation::Frame {
        clock,
        selected_ticks,
        pump,
        ..
    } = host.observe(4.0).unwrap()
    else {
        panic!("frame")
    };
    assert_eq!(clock.adjustment, ElapsedAdjustment::ClockReversal);
    assert_eq!(selected_ticks, 1);
    assert_eq!(pump.executed_ticks, 1);
    assert_eq!(host.status().host_tick, 1);
}

#[test]
fn non_finite_clock_is_atomic() {
    let mut host = FixedStepHost::new(configuration(), DeterministicSimulation::new());
    host.observe(5.0).unwrap();
    let before = host.status();
    assert_eq!(host.observe(f64::NAN), Err(HostError::NonFiniteClock));
    assert_eq!(host.status(), before);
}

#[test]
fn pause_ticks_and_suspension_does_not_consume_hidden_time() {
    let mut host = FixedStepHost::new(configuration(), DeterministicSimulation::new());
    host.observe(0.0).unwrap();
    host.observe(0.01).unwrap();
    assert_eq!(host.status().host_tick, 0);

    assert!(host.set_paused(true).unwrap());
    host.observe(0.015).unwrap();
    let paused = host.drain_publications();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].events[0].bytes[29], 0);

    assert!(host.set_suspended(true, 0.015).unwrap());
    assert!(matches!(
        host.observe(500.0).unwrap(),
        Observation::Suspended { .. }
    ));
    let retained = host.status().remainder;
    assert!(host.set_suspended(false, 500.0).unwrap());
    assert!(host.set_paused(false).unwrap());
    host.observe(500.015).unwrap();
    assert_eq!(host.status().host_tick, 2);
    assert!(host.status().remainder <= retained + f64::from(DEFAULT_TICK_INTERVAL));
    let resumed = host.drain_publications();
    assert_eq!(resumed[0].events[0].bytes[29], 1);
}

#[test]
fn command_submission_is_atomic_fifo_and_consumed_once() {
    let mut command_limits = limits();
    command_limits.max_queued_commands = 2;
    command_limits.max_queued_command_bytes = 4;
    let mut host = FixedStepHost::new(
        Configuration::new(DEFAULT_TICK_INTERVAL, command_limits).unwrap(),
        DeterministicSimulation::new(),
    );
    assert_eq!(host.submit(&[1, 2]).unwrap().sequence, 1);
    assert_eq!(host.submit(&[3, 4]).unwrap().sequence, 2);
    assert!(matches!(
        host.submit(&[5]),
        Err(HostError::CommandCountLimit {
            attempted: 3,
            limit: 2
        })
    ));
    assert_eq!(host.status().queued_commands, 2);
    assert_eq!(host.status().queued_command_bytes, 4);

    host.observe(0.0).unwrap();
    host.observe(0.03).unwrap();
    let publications = host.drain_publications();
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].events.len(), 2);
    assert_eq!(
        &publications[0].events[0].bytes[24..28],
        &2_u32.to_le_bytes()
    );
    assert_eq!(
        &publications[0].events[1].bytes[24..28],
        &0_u32.to_le_bytes()
    );
    assert_eq!(host.status().queued_commands, 0);
}

#[test]
fn command_byte_limit_and_empty_command_are_atomic() {
    let mut command_limits = limits();
    command_limits.max_queued_commands = 3;
    command_limits.max_queued_command_bytes = 4;
    let mut host = FixedStepHost::new(
        Configuration::new(DEFAULT_TICK_INTERVAL, command_limits).unwrap(),
        DeterministicSimulation::new(),
    );
    assert_eq!(host.submit(&[]), Err(HostError::EmptyCommand));
    host.submit(&[1, 2, 3, 4]).unwrap();
    let before = host.status();
    assert_eq!(
        host.submit(&[5]),
        Err(HostError::CommandByteLimit {
            attempted: 5,
            limit: 4
        })
    );
    assert_eq!(host.status(), before);
}

#[test]
fn output_backpressure_preserves_pending_ticks_and_commands() {
    let mut bounded = limits();
    bounded.max_queued_publications = 1;
    bounded.max_queued_snapshot_bytes = bounded.max_snapshot_bytes;
    bounded.max_queued_event_batches = 7;
    bounded.max_queued_event_bytes = bounded.max_event_bytes_per_tick * 7;
    let mut host = FixedStepHost::new(
        Configuration::new(DEFAULT_TICK_INTERVAL, bounded).unwrap(),
        DeterministicSimulation::new(),
    );
    host.observe(0.0).unwrap();
    host.submit(&[1]).unwrap();
    host.observe(0.015).unwrap();
    assert_eq!(host.status().queued_publications, 1);
    host.submit(&[2]).unwrap();
    let Observation::Frame { pump, .. } = host.observe(0.03).unwrap() else {
        panic!("frame")
    };
    assert_eq!(pump.disposition, PumpDisposition::Backpressured);
    assert_eq!(host.status().host_tick, 1);
    assert_eq!(host.status().pending_ticks, 1);
    assert_eq!(host.status().queued_commands, 1);

    let first = host.drain_publications();
    assert_eq!(first.len(), 1);
    let pumped = host.pump().unwrap();
    assert_eq!(pumped.executed_ticks, 1);
    assert_eq!(host.status().host_tick, 2);
    assert_eq!(host.status().pending_ticks, 0);
    assert_eq!(host.status().queued_commands, 0);
}

#[test]
fn sustained_output_overload_faults_at_the_pending_frame_limit() {
    let mut bounded = limits();
    bounded.max_pending_frames = 2;
    bounded.max_queued_publications = 1;
    bounded.max_queued_snapshot_bytes = bounded.max_snapshot_bytes;
    bounded.max_queued_event_batches = 7;
    bounded.max_queued_event_bytes = bounded.max_event_bytes_per_tick * 7;
    let mut host = FixedStepHost::new(
        Configuration::new(DEFAULT_TICK_INTERVAL, bounded).unwrap(),
        DeterministicSimulation::new(),
    );
    host.observe(0.0).unwrap();
    host.observe(0.015).unwrap();
    host.observe(0.030).unwrap();
    host.observe(0.045).unwrap();
    let before = host.status();
    assert_eq!(before.pending_frames, 2);
    assert_eq!(before.pending_ticks, 2);

    let error = host.observe(0.060).unwrap_err();
    let HostError::Faulted(fault) = error else {
        panic!("fault")
    };
    assert_eq!(fault.code, FaultCode::PendingFrameLimit);
    assert_eq!(host.status().pending_frames, 2);
    assert_eq!(host.status().pending_ticks, 2);
    assert_eq!(host.status().queued_publications, 1);
}

#[test]
fn over_bound_output_faults_before_publication() {
    let mut simulation = DeterministicSimulation::new();
    simulation.snapshot_bytes = limits().max_snapshot_bytes + 1;
    let mut host = FixedStepHost::new(configuration(), simulation);
    host.observe(0.0).unwrap();
    let HostError::Faulted(fault) = host.observe(0.015).unwrap_err() else {
        panic!("fault")
    };
    assert_eq!(fault.code, FaultCode::SnapshotLimit);
    assert_eq!(host.status().queued_publications, 0);
    assert_eq!(host.status().host_tick, 0);
}

#[test]
fn over_bound_event_batch_faults_before_publication() {
    let mut simulation = DeterministicSimulation::new();
    simulation.event_bytes = limits().max_event_bytes_per_tick + 1;
    let mut host = FixedStepHost::new(configuration(), simulation);
    host.observe(0.0).unwrap();
    let HostError::Faulted(fault) = host.observe(0.015).unwrap_err() else {
        panic!("fault")
    };
    assert_eq!(fault.code, FaultCode::EventLimit);
    assert_eq!(host.status().queued_publications, 0);
    assert_eq!(host.status().host_tick, 0);
}

#[test]
fn adapter_failure_is_terminal_and_retains_last_publication() {
    let mut simulation = DeterministicSimulation::new();
    simulation.fail_at = Some(2);
    let mut host = FixedStepHost::new(configuration(), simulation);
    host.observe(0.0).unwrap();
    host.observe(0.015).unwrap();
    assert_eq!(host.status().queued_publications, 1);

    assert!(matches!(host.observe(0.03), Err(HostError::Faulted(_))));
    assert_eq!(host.status().lifecycle, HostLifecycle::Faulted);
    assert_eq!(host.status().host_tick, 1);
    assert_eq!(host.drain_publications().len(), 1);
    assert!(matches!(host.submit(&[1]), Err(HostError::Faulted(_))));
}

#[test]
fn shutdown_is_single_call_terminal_and_clears_queues() {
    let simulation = DeterministicSimulation::new();
    let shutdowns = simulation.shutdowns.clone();
    let mut host = FixedStepHost::new(configuration(), simulation);
    host.submit(&[1]).unwrap();
    host.observe(0.0).unwrap();
    host.observe(0.015).unwrap();
    assert_eq!(
        host.shutdown().unwrap(),
        playsrc_simulation::ShutdownDisposition::Closed
    );
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(host.status().lifecycle, HostLifecycle::Closed);
    assert_eq!(host.status().queued_commands, 0);
    assert_eq!(host.status().queued_publications, 0);
    assert_eq!(
        host.shutdown().unwrap(),
        playsrc_simulation::ShutdownDisposition::AlreadyClosed
    );
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn fast_and_delayed_consumers_receive_identical_publications() {
    let fast = run_consumer_schedule(1);
    let delayed = run_consumer_schedule(37);
    assert_eq!(encode_publications(&fast), encode_publications(&delayed));
}

#[test]
fn injected_metrics_clock_records_every_phase_without_changing_trace() {
    let mut host = FixedStepHost::with_metrics_clock(
        configuration(),
        DeterministicSimulation::new(),
        StepMetricsClock {
            nanoseconds: 100,
            step: 13,
        },
    );
    host.observe(0.0).unwrap();
    host.submit(&[1, 2, 3]).unwrap();
    host.observe(0.015).unwrap();
    let metrics = host.metrics();
    assert_eq!(metrics.command_staging_nanoseconds, 13);
    assert_eq!(metrics.simulation_nanoseconds, 13);
    assert_eq!(metrics.publication_nanoseconds, 13);

    let injected = complete_trace_with_host(FixedStepHost::with_metrics_clock(
        configuration(),
        DeterministicSimulation::new(),
        StepMetricsClock {
            nanoseconds: 0,
            step: 1_000,
        },
    ));
    assert_eq!(injected, complete_trace());
}

#[test]
fn complete_fixed_trace_is_repeatable_and_fixed() {
    let first = complete_trace();
    let second = complete_trace();
    assert_eq!(first, second);
    assert_eq!(fnv1a(&first), 0x211f_0b7e_c080_5e17);
}

#[test]
fn host_can_move_to_one_selected_worker_before_use() {
    let simulation = DeterministicSimulation::new();
    let shutdowns = simulation.shutdowns.clone();
    let host = FixedStepHost::new(configuration(), simulation);
    let publications = std::thread::spawn(move || {
        let mut host = host;
        host.observe(0.0).unwrap();
        host.observe(0.1).unwrap();
        let publications = host.drain_publications();
        host.shutdown().unwrap();
        publications
    })
    .join()
    .unwrap();
    assert_eq!(publications.len(), 1);
    assert_eq!(publications[0].selected_ticks, 6);
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

fn run_consumer_schedule(drain_every: usize) -> Vec<Publication> {
    let mut host = FixedStepHost::new(configuration(), DeterministicSimulation::new());
    let mut collected = Vec::new();
    host.observe(100.0).unwrap();
    for frame_index in 1..=180 {
        if frame_index % 11 == 0 {
            host.submit(&[frame_index as u8, 0x5a]).unwrap();
        }
        if frame_index == 45 {
            host.set_paused(true).unwrap();
        }
        if frame_index == 52 {
            host.set_paused(false).unwrap();
        }
        let seconds = if frame_index < 120 {
            100.0 + frame_index as f64 / 60.0
        } else {
            100.2 + frame_index as f64 / 60.0
        };
        host.observe(seconds).unwrap();
        if frame_index % drain_every == 0 {
            collected.extend(host.drain_publications());
            host.pump().unwrap();
        }
    }
    collected.extend(host.drain_publications());
    while host.status().pending_frames > 0 {
        host.pump().unwrap();
        collected.extend(host.drain_publications());
    }
    collected
}

fn complete_trace() -> Vec<u8> {
    complete_trace_with_host(FixedStepHost::new(
        configuration(),
        DeterministicSimulation::new(),
    ))
}

fn complete_trace_with_host<C: MetricsClock>(
    mut host: FixedStepHost<DeterministicSimulation, C>,
) -> Vec<u8> {
    let mut output = Vec::new();
    record_observation(&mut output, host.observe(10.0).unwrap());
    host.submit(&[1, 2, 3]).unwrap();
    for now in [10.016, 10.031, 10.047, 10.147, 10.150] {
        record_observation(&mut output, host.observe(now).unwrap());
    }
    host.set_paused(true).unwrap();
    record_observation(&mut output, host.observe(10.165).unwrap());
    host.set_suspended(true, 10.165).unwrap();
    record_observation(&mut output, host.observe(1000.0).unwrap());
    host.set_suspended(false, 1000.0).unwrap();
    host.set_paused(false).unwrap();
    record_observation(&mut output, host.observe(1000.015).unwrap());
    record_observation(&mut output, host.observe(999.0).unwrap());
    output.extend_from_slice(&encode_publications(&host.drain_publications()));
    let status = host.status();
    output.extend_from_slice(&status.host_frame.to_le_bytes());
    output.extend_from_slice(&status.host_tick.to_le_bytes());
    output.extend_from_slice(&status.remainder.to_bits().to_le_bytes());
    output.extend_from_slice(&status.interpolation.to_bits().to_le_bytes());
    output
}

fn record_observation(output: &mut Vec<u8>, observation: Observation) {
    match observation {
        Observation::Baseline { pump } => {
            output.push(0);
            record_pump(output, pump);
        }
        Observation::Suspended { pump } => {
            output.push(1);
            record_pump(output, pump);
        }
        Observation::Frame {
            host_frame,
            clock,
            previous_remainder,
            remainder,
            interpolation,
            selected_ticks,
            pump,
        } => {
            output.push(2);
            output.extend_from_slice(&host_frame.to_le_bytes());
            output.extend_from_slice(&clock.raw_elapsed.to_bits().to_le_bytes());
            output.extend_from_slice(&clock.admitted_elapsed.to_bits().to_le_bytes());
            output.push(match clock.adjustment {
                ElapsedAdjustment::None => 0,
                ElapsedAdjustment::RaisedToMinimum => 1,
                ElapsedAdjustment::ClampedToMaximum => 2,
                ElapsedAdjustment::ClockReversal => 3,
            });
            output.extend_from_slice(&previous_remainder.to_bits().to_le_bytes());
            output.extend_from_slice(&remainder.to_bits().to_le_bytes());
            output.extend_from_slice(&interpolation.to_bits().to_le_bytes());
            output.extend_from_slice(&selected_ticks.to_le_bytes());
            record_pump(output, pump);
        }
    }
}

fn record_pump(output: &mut Vec<u8>, pump: playsrc_simulation::PumpReport) {
    output.push(match pump.disposition {
        PumpDisposition::Idle => 0,
        PumpDisposition::Completed => 1,
        PumpDisposition::Backpressured => 2,
        PumpDisposition::Suspended => 3,
    });
    output.extend_from_slice(&pump.executed_ticks.to_le_bytes());
    output.extend_from_slice(&pump.published_frames.to_le_bytes());
    output.extend_from_slice(&(pump.pending_frames as u64).to_le_bytes());
    output.extend_from_slice(&pump.pending_ticks.to_le_bytes());
}

fn encode_publications(publications: &[Publication]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&(publications.len() as u64).to_le_bytes());
    for publication in publications {
        output.extend_from_slice(&publication.host_frame.to_le_bytes());
        output.extend_from_slice(&publication.first_host_tick.to_le_bytes());
        output.extend_from_slice(&publication.last_host_tick.to_le_bytes());
        output.extend_from_slice(&publication.selected_ticks.to_le_bytes());
        output.extend_from_slice(&publication.interpolation.to_bits().to_le_bytes());
        output.extend_from_slice(&(publication.snapshot.len() as u64).to_le_bytes());
        output.extend_from_slice(&publication.snapshot);
        output.extend_from_slice(&(publication.events.len() as u64).to_le_bytes());
        for event in &publication.events {
            output.extend_from_slice(&event.host_tick.to_le_bytes());
            output.extend_from_slice(&(event.bytes.len() as u64).to_le_bytes());
            output.extend_from_slice(&event.bytes);
        }
    }
    output
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}
