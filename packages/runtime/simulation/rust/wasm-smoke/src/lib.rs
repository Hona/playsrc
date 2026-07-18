use playsrc_simulation::{
    Configuration, DEFAULT_TICK_INTERVAL, FixedStepHost, Limits, MetricsClock, Observation,
    Simulation, SimulationError, TickInput, TickOutput,
};

struct SmokeSimulation;

struct SmokeMetricsClock {
    nanoseconds: u64,
}

impl MetricsClock for SmokeMetricsClock {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        let value = self.nanoseconds;
        self.nanoseconds += 11;
        value
    }
}

impl Simulation for SmokeSimulation {
    fn advance(&mut self, input: TickInput<'_>) -> Result<TickOutput, SimulationError> {
        Ok(TickOutput::new(
            input.context.host_tick.to_le_bytes().to_vec(),
            input.context.host_frame.to_le_bytes().to_vec(),
        ))
    }

    fn shutdown(&mut self) -> Result<(), SimulationError> {
        Ok(())
    }
}

fn configuration() -> Option<Configuration> {
    Configuration::new(
        DEFAULT_TICK_INTERVAL,
        Limits {
            max_queued_commands: 4,
            max_queued_command_bytes: 64,
            max_pending_frames: 4,
            max_snapshot_bytes: 64,
            max_event_bytes_per_tick: 64,
            max_queued_publications: 4,
            max_queued_snapshot_bytes: 256,
            max_queued_event_batches: 28,
            max_queued_event_bytes: 1_792,
        },
    )
    .ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn simulation_wasm_smoke() -> u32 {
    let Some(configuration) = configuration() else {
        return 0;
    };
    let mut host = FixedStepHost::with_metrics_clock(
        configuration,
        SmokeSimulation,
        SmokeMetricsClock { nanoseconds: 0 },
    );
    if !matches!(host.observe(0.0), Ok(Observation::Baseline { .. })) {
        return 0;
    }
    if host.submit(&[1, 2, 3, 4]).is_err() {
        return 0;
    }
    if !matches!(host.observe(0.015), Ok(Observation::Frame { .. })) {
        return 0;
    }
    let publications = host.drain_publications();
    let metrics = host.metrics();
    u32::from(
        host.status().host_tick == 1
            && publications.len() == 1
            && publications[0].snapshot.as_ref() == 1_u64.to_le_bytes()
            && metrics.command_staging_nanoseconds == 11
            && metrics.simulation_nanoseconds == 11
            && metrics.publication_nanoseconds == 11,
    )
}
