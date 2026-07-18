use std::{
    alloc::{GlobalAlloc, Layout, System},
    env,
    hint::black_box,
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use playsrc_simulation::{
    Configuration, DEFAULT_TICK_INTERVAL, FixedStepHost, Limits, Observation, Simulation,
    SimulationError, TickInput, TickOutput,
};

const SCHEDULE_FRAMES: usize = 600;
const COMMANDS_PER_FRAME: usize = 16;
const COMMAND_BYTES: usize = 24;
const SNAPSHOT_BYTES: usize = 1024;
const EVENT_BYTES: usize = 128;
const DRAIN_INTERVAL: usize = 16;

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Default)]
struct SerializationProbe {
    snapshot: Mutex<Vec<u64>>,
    events: Mutex<Vec<u64>>,
}

struct BenchmarkSimulation {
    state: u64,
    probe: Arc<SerializationProbe>,
}

impl BenchmarkSimulation {
    fn new(probe: Arc<SerializationProbe>) -> Self {
        Self {
            state: 0x6a09_e667_f3bc_c909,
            probe,
        }
    }
}

impl Simulation for BenchmarkSimulation {
    fn advance(&mut self, input: TickInput<'_>) -> Result<TickOutput, SimulationError> {
        fold(&mut self.state, input.context.host_frame);
        fold(&mut self.state, input.context.host_tick);
        fold(&mut self.state, u64::from(input.context.simulating));
        for command in input.commands {
            fold(&mut self.state, command.sequence);
            for byte in command.bytes.iter() {
                fold(&mut self.state, u64::from(*byte));
            }
        }

        let snapshot_started = Instant::now();
        let mut snapshot = vec![0_u8; SNAPSHOT_BYTES];
        serialize(&mut snapshot, self.state, input.context.host_tick);
        self.probe
            .snapshot
            .lock()
            .expect("snapshot probe")
            .push(nanoseconds(snapshot_started.elapsed()));

        let event_started = Instant::now();
        let mut events = vec![0_u8; EVENT_BYTES];
        serialize(
            &mut events,
            input.context.host_tick,
            self.state.rotate_left(17),
        );
        self.probe
            .events
            .lock()
            .expect("event probe")
            .push(nanoseconds(event_started.elapsed()));
        Ok(TickOutput::new(snapshot, events))
    }

    fn shutdown(&mut self) -> Result<(), SimulationError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Profile {
    name: &'static str,
    warmup_repetitions: usize,
    measured_repetitions: usize,
}

struct RunResult {
    end_to_end: Vec<u64>,
    simulation_nanoseconds: u64,
    publication_nanoseconds: u64,
    snapshot_serialization: Vec<u64>,
    event_serialization: Vec<u64>,
    command_staging_nanoseconds: u64,
    command_staging_allocations: u64,
    command_staging_bytes: u64,
    ticks: u64,
    publications: u64,
    command_count: u64,
    command_bytes: u64,
    snapshot_bytes: u64,
    event_bytes: u64,
    maximum_pending_frames: usize,
    maximum_pending_ticks: u64,
    maximum_publications: usize,
    maximum_snapshot_queue_bytes: usize,
    maximum_event_batches: usize,
    maximum_event_queue_bytes: usize,
    stall_ticks: Vec<u32>,
    checksum: u64,
}

impl RunResult {
    fn empty() -> Self {
        Self {
            end_to_end: Vec::new(),
            simulation_nanoseconds: 0,
            publication_nanoseconds: 0,
            snapshot_serialization: Vec::new(),
            event_serialization: Vec::new(),
            command_staging_nanoseconds: 0,
            command_staging_allocations: 0,
            command_staging_bytes: 0,
            ticks: 0,
            publications: 0,
            command_count: 0,
            command_bytes: 0,
            snapshot_bytes: 0,
            event_bytes: 0,
            maximum_pending_frames: 0,
            maximum_pending_ticks: 0,
            maximum_publications: 0,
            maximum_snapshot_queue_bytes: 0,
            maximum_event_batches: 0,
            maximum_event_queue_bytes: 0,
            stall_ticks: Vec::new(),
            checksum: 0,
        }
    }

    fn merge(&mut self, mut other: Self) {
        self.end_to_end.append(&mut other.end_to_end);
        self.simulation_nanoseconds = self
            .simulation_nanoseconds
            .saturating_add(other.simulation_nanoseconds);
        self.publication_nanoseconds = self
            .publication_nanoseconds
            .saturating_add(other.publication_nanoseconds);
        self.snapshot_serialization
            .append(&mut other.snapshot_serialization);
        self.event_serialization
            .append(&mut other.event_serialization);
        self.command_staging_nanoseconds = self
            .command_staging_nanoseconds
            .saturating_add(other.command_staging_nanoseconds);
        self.command_staging_allocations = self
            .command_staging_allocations
            .saturating_add(other.command_staging_allocations);
        self.command_staging_bytes = self
            .command_staging_bytes
            .saturating_add(other.command_staging_bytes);
        self.ticks = self.ticks.saturating_add(other.ticks);
        self.publications = self.publications.saturating_add(other.publications);
        self.command_count = self.command_count.saturating_add(other.command_count);
        self.command_bytes = self.command_bytes.saturating_add(other.command_bytes);
        self.snapshot_bytes = self.snapshot_bytes.saturating_add(other.snapshot_bytes);
        self.event_bytes = self.event_bytes.saturating_add(other.event_bytes);
        self.maximum_pending_frames = self
            .maximum_pending_frames
            .max(other.maximum_pending_frames);
        self.maximum_pending_ticks = self.maximum_pending_ticks.max(other.maximum_pending_ticks);
        self.maximum_publications = self.maximum_publications.max(other.maximum_publications);
        self.maximum_snapshot_queue_bytes = self
            .maximum_snapshot_queue_bytes
            .max(other.maximum_snapshot_queue_bytes);
        self.maximum_event_batches = self.maximum_event_batches.max(other.maximum_event_batches);
        self.maximum_event_queue_bytes = self
            .maximum_event_queue_bytes
            .max(other.maximum_event_queue_bytes);
        self.stall_ticks.append(&mut other.stall_ticks);
        fold(&mut self.checksum, other.checksum);
    }
}

#[derive(Clone, Copy)]
struct Distribution {
    minimum: u64,
    p50: u64,
    p95: u64,
    p99: u64,
    maximum: u64,
    mean: u64,
}

fn main() -> ExitCode {
    let profile = match env::args().nth(1).as_deref().unwrap_or("smoke") {
        "smoke" => Profile {
            name: "smoke",
            warmup_repetitions: 2,
            measured_repetitions: 5,
        },
        "full" => Profile {
            name: "full",
            warmup_repetitions: 20,
            measured_repetitions: 200,
        },
        value => {
            eprintln!("unknown benchmark profile {value}; expected smoke or full");
            return ExitCode::FAILURE;
        }
    };

    for repetition in 0..profile.warmup_repetitions {
        black_box(run_once(repetition as u64));
    }

    ALLOCATIONS.store(0, Ordering::SeqCst);
    ALLOCATED_BYTES.store(0, Ordering::SeqCst);
    let measured_started = Instant::now();
    let mut result = RunResult::empty();
    for repetition in 0..profile.measured_repetitions {
        result.merge(run_once(repetition as u64));
    }
    let measured_duration = measured_started.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::SeqCst);
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::SeqCst);
    if result.command_staging_nanoseconds == 0
        || result.simulation_nanoseconds == 0
        || result.publication_nanoseconds == 0
    {
        eprintln!("benchmark metrics clock did not measure every host phase");
        return ExitCode::FAILURE;
    }
    black_box(result.checksum);
    print_report(
        profile,
        &result,
        measured_duration,
        allocations,
        allocated_bytes,
    );
    ExitCode::SUCCESS
}

fn run_once(repetition: u64) -> RunResult {
    let probe = Arc::new(SerializationProbe::default());
    let simulation = BenchmarkSimulation::new(probe.clone());
    let mut host = FixedStepHost::new(configuration(), simulation);
    let mut result = RunResult::empty();
    let mut now = 1000.0 + repetition as f64 * 1000.0;
    host.observe(now).expect("baseline");

    for frame_index in 1..=SCHEDULE_FRAMES {
        for command_index in 0..COMMANDS_PER_FRAME {
            let mut command = [0_u8; COMMAND_BYTES];
            command[0..8].copy_from_slice(&(frame_index as u64).to_le_bytes());
            command[8..16].copy_from_slice(&(command_index as u64).to_le_bytes());
            command[16..24].copy_from_slice(&repetition.to_le_bytes());
            host.submit(&command).expect("command");
        }

        if frame_index == 300 {
            host.set_paused(true).expect("pause");
        } else if frame_index == 315 {
            host.set_paused(false).expect("resume");
        } else if frame_index == 420 {
            host.set_suspended(true, now).expect("suspend");
            now += 30.0;
            host.observe(now).expect("hidden sample");
            host.set_suspended(false, now).expect("visible");
        }

        let delta = match frame_index % 8 {
            0 => 0.030,
            1 => 0.008,
            2 => 0.016,
            3 => 0.017,
            4 => 0.014,
            5 => 0.015,
            6 => 0.018,
            _ => 0.012,
        };
        now += if frame_index == 200 { 0.250 } else { delta };
        let started = Instant::now();
        let observation = host.observe(now).expect("frame");
        result.end_to_end.push(nanoseconds(started.elapsed()));
        if matches!(frame_index, 200..=202) {
            let Observation::Frame { selected_ticks, .. } = observation else {
                panic!("frame observation")
            };
            result.stall_ticks.push(selected_ticks);
        }
        if frame_index % DRAIN_INTERVAL == 0 {
            fold_publications(&mut result.checksum, host.drain_publications());
            host.pump().expect("post-drain pump");
        }
    }
    fold_publications(&mut result.checksum, host.drain_publications());
    while host.status().pending_frames > 0 {
        host.pump().expect("final pump");
        fold_publications(&mut result.checksum, host.drain_publications());
    }

    let metrics = host.metrics();
    result.simulation_nanoseconds = metrics.simulation_nanoseconds;
    result.publication_nanoseconds = metrics.publication_nanoseconds;
    result.command_staging_nanoseconds = metrics.command_staging_nanoseconds;
    result.command_staging_allocations = metrics.command_staging_allocations;
    result.command_staging_bytes = metrics.command_staging_bytes;
    result.ticks = metrics.executed_ticks;
    result.publications = metrics.published_frames;
    result.command_count = metrics.submitted_commands;
    result.command_bytes = metrics.submitted_command_bytes;
    result.snapshot_bytes = metrics.snapshot_bytes;
    result.event_bytes = metrics.event_bytes;
    result.maximum_pending_frames = metrics.maximum_pending_frames;
    result.maximum_pending_ticks = metrics.maximum_pending_ticks;
    result.maximum_publications = metrics.maximum_publications;
    result.maximum_snapshot_queue_bytes = metrics.maximum_snapshot_bytes;
    result.maximum_event_batches = metrics.maximum_event_batches;
    result.maximum_event_queue_bytes = metrics.maximum_event_bytes;
    result.snapshot_serialization = probe.snapshot.lock().expect("snapshot probe").clone();
    result.event_serialization = probe.events.lock().expect("event probe").clone();
    host.shutdown().expect("shutdown");
    result
}

fn configuration() -> Configuration {
    Configuration::new(
        DEFAULT_TICK_INTERVAL,
        Limits {
            max_queued_commands: COMMANDS_PER_FRAME * 2,
            max_queued_command_bytes: COMMANDS_PER_FRAME * COMMAND_BYTES * 2,
            max_pending_frames: 64,
            max_snapshot_bytes: SNAPSHOT_BYTES,
            max_event_bytes_per_tick: EVENT_BYTES,
            max_queued_publications: 64,
            max_queued_snapshot_bytes: 64 * SNAPSHOT_BYTES,
            max_queued_event_batches: 512,
            max_queued_event_bytes: 512 * EVENT_BYTES,
        },
    )
    .expect("benchmark configuration")
}

fn serialize(output: &mut [u8], first: u64, second: u64) {
    let mut state = first ^ second.rotate_left(29);
    for chunk in output.chunks_mut(8) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
}

fn fold(state: &mut u64, value: u64) {
    *state ^= value;
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

fn fold_publications(state: &mut u64, publications: Vec<playsrc_simulation::Publication>) {
    for publication in publications {
        fold(state, publication.host_frame);
        fold(state, publication.last_host_tick);
        for byte in publication.snapshot.iter().step_by(64) {
            fold(state, u64::from(*byte));
        }
        for event in publication.events {
            fold(state, event.host_tick);
            for byte in event.bytes.iter().step_by(16) {
                fold(state, u64::from(*byte));
            }
        }
    }
}

fn distribution(samples: &[u64]) -> Distribution {
    assert!(!samples.is_empty());
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let percentile = |numerator: usize| {
        let index = (sorted.len() - 1) * numerator / 100;
        sorted[index]
    };
    Distribution {
        minimum: sorted[0],
        p50: percentile(50),
        p95: percentile(95),
        p99: percentile(99),
        maximum: sorted[sorted.len() - 1],
        mean: (sorted.iter().map(|value| u128::from(*value)).sum::<u128>() / sorted.len() as u128)
            as u64,
    }
}

fn nanoseconds(duration: Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}

fn print_distribution(name: &str, value: Distribution, trailing_comma: bool) {
    println!(
        "    \"{name}\": {{\"min\":{},\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{},\"mean\":{}}}{}",
        value.minimum,
        value.p50,
        value.p95,
        value.p99,
        value.maximum,
        value.mean,
        if trailing_comma { "," } else { "" }
    );
}

fn print_report(
    profile: Profile,
    result: &RunResult,
    measured_duration: Duration,
    allocations: u64,
    allocated_bytes: u64,
) {
    let measured_nanoseconds = nanoseconds(measured_duration);
    let tick_throughput = result.ticks as f64 / measured_duration.as_secs_f64();
    let phase_total = result
        .simulation_nanoseconds
        .saturating_add(result.publication_nanoseconds)
        .saturating_add(result.command_staging_nanoseconds);
    let scheduler_other = measured_nanoseconds.saturating_sub(phase_total);
    println!("{{");
    println!("  \"schema\": \"playsrc-simulation-benchmark-v1\",");
    println!("  \"profile\": \"{}\",", profile.name);
    println!(
        "  \"build\": \"{}\",",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("  \"os\": \"{}\",", env::consts::OS);
    println!("  \"arch\": \"{}\",", env::consts::ARCH);
    println!(
        "  \"schedule\": {{\"identity\":\"irregular-600-v1\",\"framesPerRepetition\":{SCHEDULE_FRAMES},\"commandsPerFrame\":{COMMANDS_PER_FRAME},\"commandBytes\":{COMMAND_BYTES},\"snapshotBytesPerTick\":{SNAPSHOT_BYTES},\"eventBytesPerTick\":{EVENT_BYTES},\"drainIntervalFrames\":{DRAIN_INTERVAL},\"stallFrame\":200,\"stallSeconds\":0.25,\"pauseFrames\":[300,315],\"suspensionFrame\":420}},"
    );
    println!(
        "  \"sampling\": {{\"warmupRepetitions\":{},\"measuredRepetitions\":{},\"endToEndSamples\":{},\"tickSamples\":{}}},",
        profile.warmup_repetitions,
        profile.measured_repetitions,
        result.end_to_end.len(),
        result.snapshot_serialization.len()
    );
    println!(
        "  \"work\": {{\"ticks\":{},\"publications\":{},\"commands\":{},\"commandBytes\":{},\"snapshotBytes\":{},\"eventBytes\":{},\"checksum\":\"{:016x}\"}},",
        result.ticks,
        result.publications,
        result.command_count,
        result.command_bytes,
        result.snapshot_bytes,
        result.event_bytes,
        result.checksum
    );
    println!(
        "  \"throughput\": {{\"measuredNanoseconds\":{measured_nanoseconds},\"ticksPerSecond\":{tick_throughput:.3}}},"
    );
    println!("  \"latencyNanoseconds\": {{");
    print_distribution("endToEndFrame", distribution(&result.end_to_end), true);
    print_distribution(
        "snapshotSerialization",
        distribution(&result.snapshot_serialization),
        true,
    );
    print_distribution(
        "eventSerialization",
        distribution(&result.event_serialization),
        false,
    );
    println!("  }},");
    println!(
        "  \"phaseTotalsNanoseconds\": {{\"commandStaging\":{},\"simulationCallback\":{},\"publication\":{},\"schedulerAndHarness\":{scheduler_other}}},",
        result.command_staging_nanoseconds,
        result.simulation_nanoseconds,
        result.publication_nanoseconds
    );
    println!(
        "  \"queues\": {{\"maximumPendingFrames\":{},\"maximumPendingTicks\":{},\"maximumPublications\":{},\"maximumSnapshotBytes\":{},\"maximumEventBatches\":{},\"maximumEventBytes\":{}}},",
        result.maximum_pending_frames,
        result.maximum_pending_ticks,
        result.maximum_publications,
        result.maximum_snapshot_queue_bytes,
        result.maximum_event_batches,
        result.maximum_event_queue_bytes
    );
    println!(
        "  \"allocations\": {{\"calls\":{allocations},\"bytes\":{allocated_bytes},\"commandStagingCalls\":{},\"commandStagingBytes\":{}}},",
        result.command_staging_allocations, result.command_staging_bytes
    );
    let stall_signature = &result.stall_ticks[..3];
    let stall_matches = result
        .stall_ticks
        .chunks_exact(3)
        .all(|sample| sample == stall_signature);
    println!(
        "  \"stallRecovery\": {{\"samples\":{},\"selectedTicks\":[{}],\"allSamplesMatch\":{stall_matches}}}",
        result.stall_ticks.len() / 3,
        stall_signature
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    println!("}}");
}
