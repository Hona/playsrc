mod gameplay_protocol;
pub mod static_prop_artifact;

#[cfg(all(target_arch = "wasm32", feature = "threaded"))]
pub use wasm_bindgen_rayon::init_thread_pool;

use rayon::prelude::*;
use sha2::{Digest, Sha256};

const PRESENTATION_OUTPUT_LIMIT: usize = 512 * 1024 * 1024;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock, RwLock,
        atomic::{AtomicU32, Ordering},
    },
};
static SIMULATION_ERROR: AtomicU32 = AtomicU32::new(0);
static SIMULATION_ERROR_DETAIL: OnceLock<Mutex<String>> = OnceLock::new();
static GAME_ADVANCE_ERROR: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "playsrc_metrics")]
unsafe extern "C" {
    fn monotonic_milliseconds() -> f64;
}

struct RuntimeMetricsClock {
    #[cfg(not(target_arch = "wasm32"))]
    origin: std::time::Instant,
    prior: u64,
}

impl RuntimeMetricsClock {
    fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            origin: std::time::Instant::now(),
            prior: 0,
        }
    }
}

impl playsrc_simulation::MetricsClock for RuntimeMetricsClock {
    fn monotonic_nanoseconds(&mut self) -> u64 {
        #[cfg(target_arch = "wasm32")]
        let value = unsafe { monotonic_milliseconds() };
        #[cfg(target_arch = "wasm32")]
        let value = if value.is_finite() && value >= 0.0 {
            (value * 1_000_000.0).min(u64::MAX as f64) as u64
        } else {
            self.prior
        };
        #[cfg(not(target_arch = "wasm32"))]
        let value = self.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.prior = self.prior.max(value);
        self.prior
    }
}

struct SharedWorld {
    world: Arc<playsrc_collision::World>,
    snapshot: Arc<RwLock<Arc<playsrc_collision::Snapshot>>>,
}
impl Clone for SharedWorld {
    fn clone(&self) -> Self {
        Self {
            world: self.world.clone(),
            snapshot: self.snapshot.clone(),
        }
    }
}
impl SharedWorld {
    fn new(world: Arc<playsrc_collision::World>, snapshot: playsrc_collision::Snapshot) -> Self {
        Self {
            world,
            snapshot: Arc::new(RwLock::new(Arc::new(snapshot))),
        }
    }

    fn replace_snapshot(&self, snapshot: playsrc_collision::Snapshot) {
        *self.snapshot.write().expect("TF2 Collision snapshot") = Arc::new(snapshot);
    }

    fn replace_snapshot_arc(&self, snapshot: Arc<playsrc_collision::Snapshot>) {
        *self.snapshot.write().expect("TF2 Collision snapshot") = snapshot;
    }

    fn snapshot(&self) -> Arc<playsrc_collision::Snapshot> {
        self.snapshot
            .read()
            .expect("TF2 Collision snapshot")
            .clone()
    }

    fn movement_trace(
        trace: playsrc_collision::Trace,
    ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        let hit = match trace.hit {
            Some(playsrc_collision::Hit::WorldBrush { .. }) => Some(0),
            Some(playsrc_collision::Hit::Object { identity, .. }) => Some(identity),
            None => None,
        };
        Ok(playsrc_movement::Trace {
            fraction: trace.fraction,
            start_solid: trace.start_solid,
            all_solid: trace.all_solid,
            end: trace.end,
            normal: trace.plane.map(|plane| plane.normal),
            hit,
            contents: trace.contents,
        })
    }
}

struct CollisionSnapshotTransaction {
    world: SharedWorld,
    original: Arc<playsrc_collision::Snapshot>,
    committed: bool,
}
impl CollisionSnapshotTransaction {
    fn new(world: SharedWorld) -> Self {
        Self {
            original: world.snapshot(),
            world,
            committed: false,
        }
    }
}
impl Drop for CollisionSnapshotTransaction {
    fn drop(&mut self) {
        if !self.committed {
            self.world.replace_snapshot_arc(self.original.clone());
        }
    }
}
impl playsrc_movement::Tracer for SharedWorld {
    fn trace(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: playsrc_collision::Hull,
        mask: u32,
    ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        let snapshot = self.snapshot();
        let trace = self
            .world
            .trace_snapshot_hull(
                &snapshot,
                playsrc_collision::SnapshotTraceRequest {
                    start,
                    end,
                    hull,
                    mask,
                    scope: playsrc_collision::TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::Trace,
                    playsrc_movement::FailureKind::Malformed,
                    "snapshot collision",
                )
            })?;
        Self::movement_trace(trace)
    }

    fn point_contents(&self, point: [f32; 3]) -> Result<u32, playsrc_movement::Error> {
        self.world
            .point_contents_snapshot_value(&self.snapshot(), point)
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::PointContents,
                    playsrc_movement::FailureKind::Malformed,
                    "snapshot point contents",
                )
            })
    }

    fn support_velocity(&self, support: u64) -> Result<[f32; 3], playsrc_movement::Error> {
        Ok(self
            .snapshot()
            .object_velocity(support)
            .map_or([0.0; 3], |value| value.0))
    }

    fn is_world(&self, hit: u64) -> bool {
        hit == 0
    }

    fn trace_without(
        &self,
        ignored: u64,
        start: [f32; 3],
        end: [f32; 3],
        hull: playsrc_collision::Hull,
        mask: u32,
    ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        let snapshot = self.snapshot();
        let trace = self
            .world
            .trace_snapshot_hull(
                &snapshot,
                playsrc_collision::SnapshotTraceRequest {
                    start,
                    end,
                    hull,
                    mask,
                    scope: playsrc_collision::TraceScope::Everything,
                    ignored: &[ignored],
                },
                |_| true,
            )
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::Trace,
                    playsrc_movement::FailureKind::Malformed,
                    "snapshot collision",
                )
            })?;
        Self::movement_trace(trace)
    }

    fn overlaps_mover(
        &self,
        mover: u64,
        position: [f32; 3],
        hull: playsrc_collision::Hull,
    ) -> Result<bool, playsrc_movement::Error> {
        let snapshot = self.snapshot();
        let transform = snapshot.object_transform(mover).ok_or_else(|| {
            playsrc_movement::Error::new(
                playsrc_movement::Operation::Mover,
                playsrc_movement::FailureKind::Missing,
                "mover collision object",
            )
        })?;
        self.world
            .overlaps_object_hull_at(
                &snapshot,
                playsrc_collision::ObjectOverlapRequest {
                    identity: mover,
                    transform,
                    position,
                    hull,
                    mask: u32::MAX,
                },
            )
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::Mover,
                    playsrc_movement::FailureKind::Malformed,
                    "mover collision overlap",
                )
            })
    }
}

impl playsrc_tf2::GameplayWorld for SharedWorld {
    fn collision_snapshot_revision(&self) -> Option<u64> {
        Some(self.snapshot().identity())
    }
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: playsrc_collision::Hull,
    ) -> Result<bool, playsrc_movement::Error> {
        self.world
            .overlaps_model_hull(model, origin, position, hull)
            .map_err(|_| {
                playsrc_movement::Error::new(
                    playsrc_movement::Operation::Trace,
                    playsrc_movement::FailureKind::Malformed,
                    "model overlap",
                )
            })
    }
}

#[derive(Clone)]
struct CollisionObjectTemplate {
    input: playsrc_collision::ObjectInput,
    runtime_transform: bool,
}

#[derive(Clone, Copy)]
struct Spawn {
    entity: u32,
    hammer_id: u32,
    position: [f32; 3],
    angles: [f32; 3],
}

struct RuntimeWorldMaterial {
    map_material: usize,
    material: playsrc_material::Material,
    texture_frames: BTreeMap<Vec<u8>, u32>,
}

struct RuntimeEnvironment {
    world: playsrc_map::WorldEnvironment,
    water_materials: BTreeMap<String, playsrc_material::Material>,
    world_materials: BTreeMap<String, RuntimeWorldMaterial>,
    map_materials: BTreeMap<usize, String>,
    normal_frame_counts: BTreeMap<String, u32>,
    water_lod: Option<[f32; 2]>,
}

type InspectedTexture<'source> =
    OnceLock<Result<playsrc_vtf::Decoder<'source>, playsrc_vtf::Error>>;

struct TextureDecoders<'source> {
    entries: BTreeMap<String, (&'source [u8], InspectedTexture<'source>)>,
    requests: AtomicU32,
    inspections: AtomicU32,
}

impl<'source> TextureDecoders<'source> {
    fn new(bundle: &BTreeMap<String, &'source [u8]>) -> Self {
        Self {
            entries: bundle
                .iter()
                .filter(|(identity, _)| identity.ends_with(".vtf"))
                .map(|(identity, bytes)| (identity.clone(), (*bytes, OnceLock::new())))
                .collect(),
            requests: AtomicU32::new(0),
            inspections: AtomicU32::new(0),
        }
    }

    fn decoder(&self, path: &str) -> Result<&playsrc_vtf::Decoder<'source>, ()> {
        let (bytes, inspected) = self.entries.get(path).ok_or(())?;
        self.requests.fetch_add(1, Ordering::Relaxed);
        inspected
            .get_or_init(|| {
                self.inspections.fetch_add(1, Ordering::Relaxed);
                playsrc_vtf::Decoder::new(
                    bytes,
                    playsrc_vtf::Dialect::Source2013Pc,
                    playsrc_vtf::Limits::default(),
                )
            })
            .as_ref()
            .map_err(|_| ())
    }

    fn metadata(&self, path: &str) -> Result<&playsrc_vtf::Metadata, ()> {
        Ok(self.decoder(path)?.metadata())
    }

    fn bytes(&self, path: &str) -> Result<&'source [u8], ()> {
        self.entries.get(path).map(|(bytes, _)| *bytes).ok_or(())
    }
}

struct CompiledPresentationModel {
    model: playsrc_studio_model::PresentationModel,
    identity: [u8; 32],
    illumination_position: playsrc_studio_model::Vector3,
    illumination_attachment: i32,
}

#[derive(Clone, Copy)]
struct PresentationInputs<'a, 'source> {
    canonical: &'a playsrc_map::CanonicalMap,
    bsp: &'a playsrc_bsp::Bsp,
    graph: &'a playsrc_entity::Graph,
    bundle: &'a BTreeMap<String, &'source [u8]>,
    decoders: &'a TextureDecoders<'source>,
    model_resources: &'a BTreeMap<String, Arc<[u8]>>,
    resource_hashes: &'a BTreeMap<String, [u8; 32]>,
    map_materials: &'a [playsrc_material::Material],
    particle_presentation: &'a BTreeMap<String, CompiledParticlePresentation>,
    profile: playsrc_map::LightingProfile,
    visibility: &'a playsrc_visibility::World,
    collision: &'a playsrc_collision::World,
    additional_model_roots: &'a [String],
}

struct Slot {
    generation: u16,
    payload: Option<Vec<u8>>,
    presentation: Vec<u8>,
    coverage: Vec<u8>,
    particles: Option<playsrc_particle::ParticleWorld>,
    particle_materials: Vec<String>,
    particle_sheets: BTreeMap<String, playsrc_particle::ParticleMaterial>,
    particle_output: Vec<u8>,
    studio_models: BTreeMap<String, playsrc_studio_model::PresentationModel>,
    model_material_opacity: BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    viewmodel_bob: BTreeMap<u32, playsrc_studio_model::ViewModelBobState>,
    model_output: Vec<u8>,
    visibility: Option<playsrc_visibility::World>,
    area_state: Option<playsrc_visibility::AreaState>,
    visibility_output: Vec<u8>,
    environment: Option<RuntimeEnvironment>,
    collision: Option<Arc<playsrc_collision::World>>,
    gameplay_world: Option<SharedWorld>,
    collision_templates: Vec<CollisionObjectTemplate>,
    collision_revision: u64,
    pushers: BTreeMap<u64, playsrc_movement::PusherSnapshot>,
    latest_game_snapshot: Option<playsrc_tf2::Snapshot>,
    hash: [u8; 32],
    derived_hash: [u8; 32],
    bsp_hash: [u8; 32],
    error: u32,
    spawn: Option<Spawn>,
    session: Option<playsrc_tf2::Session<SharedWorld>>,
    snapshot: Vec<u8>,
    compile_metrics: [u64; 17],
    texture_inspections: [u32; 2],
}

struct Tf2Simulation {
    handle: u32,
    current_command: Option<Arc<[u8]>>,
}

fn continuation_command(command: &[u8]) -> Result<Arc<[u8]>, playsrc_simulation::SimulationError> {
    let mut continuation = command
        .get(..48)
        .ok_or_else(|| playsrc_simulation::SimulationError::new("command", "continuation"))?
        .to_vec();
    continuation[32..36].copy_from_slice(&0_u32.to_le_bytes());
    continuation[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    continuation[40..44].fill(0);
    continuation[44..48].copy_from_slice(&48_u32.to_le_bytes());
    Ok(Arc::from(continuation))
}

impl playsrc_simulation::Simulation for Tf2Simulation {
    fn advance(
        &mut self,
        input: playsrc_simulation::TickInput<'_>,
    ) -> Result<playsrc_simulation::TickOutput, playsrc_simulation::SimulationError> {
        let command = if let Some(latest) = input.commands.last() {
            let mut merged = latest.bytes.to_vec();
            let mut flags = 0u32;
            let mut selectors =
                u32::from_le_bytes(merged[32..36].try_into().map_err(|_| {
                    playsrc_simulation::SimulationError::new("command", "selectors")
                })?);
            let mut activate =
                u32::from_le_bytes(merged[36..40].try_into().map_err(|_| {
                    playsrc_simulation::SimulationError::new("command", "activate")
                })?);
            for value in input.commands {
                flags |=
                    u32::from_le_bytes(value.bytes[28..32].try_into().map_err(|_| {
                        playsrc_simulation::SimulationError::new("command", "flags")
                    })?);
                let next = u32::from_le_bytes(value.bytes[32..36].try_into().map_err(|_| {
                    playsrc_simulation::SimulationError::new("command", "selectors")
                })?);
                for shift in [0, 8, 16, 24] {
                    if ((next >> shift) & 255) != 0 {
                        selectors = (selectors & !(255 << shift)) | (next & (255 << shift))
                    }
                }
                let next_activate =
                    u32::from_le_bytes(value.bytes[36..40].try_into().map_err(|_| {
                        playsrc_simulation::SimulationError::new("command", "activate")
                    })?);
                if next_activate != u32::MAX {
                    activate = next_activate
                }
            }
            merged[28..32].copy_from_slice(&flags.to_le_bytes());
            merged[32..36].copy_from_slice(&selectors.to_le_bytes());
            merged[36..40].copy_from_slice(&activate.to_le_bytes());
            let merged = Arc::<[u8]>::from(merged);
            self.current_command = Some(continuation_command(&merged)?);
            merged
        } else {
            self.current_command.clone().ok_or_else(|| {
                playsrc_simulation::SimulationError::new(
                    "missing-command",
                    "TF2 Simulation tick has no admitted command",
                )
            })?
        };
        if unsafe { playsrc_game_advance(self.handle, command.as_ptr(), command.len(), 1) } != 1 {
            return Err(playsrc_simulation::SimulationError::new(
                "tf2-transition",
                format!(
                    "TF2 gameplay transition failed at game-advance:{}",
                    GAME_ADVANCE_ERROR.load(Ordering::Relaxed)
                ),
            ));
        }
        let snapshot = with(self.handle, |slot| slot.snapshot.clone()).ok_or_else(|| {
            playsrc_simulation::SimulationError::new(
                "stale-handle",
                "TF2 gameplay handle is unavailable",
            )
        })?;
        Ok(playsrc_simulation::TickOutput::new(
            snapshot.clone(),
            snapshot,
        ))
    }
    fn shutdown(&mut self) -> Result<(), playsrc_simulation::SimulationError> {
        self.current_command = None;
        Ok(())
    }
}

struct SimulationHostEntry {
    host: playsrc_simulation::FixedStepHost<Tf2Simulation, RuntimeMetricsClock>,
    output: Vec<u8>,
}
fn simulation_hosts() -> &'static Mutex<BTreeMap<u32, SimulationHostEntry>> {
    static HOSTS: OnceLock<Mutex<BTreeMap<u32, SimulationHostEntry>>> = OnceLock::new();
    HOSTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}
fn simulation_configuration() -> Option<playsrc_simulation::Configuration> {
    const LIMIT: usize = 64 * 1024 * 1024;
    playsrc_simulation::Configuration::new(
        playsrc_simulation::DEFAULT_TICK_INTERVAL,
        playsrc_simulation::Limits {
            max_queued_commands: 4096,
            max_queued_command_bytes: LIMIT,
            max_pending_frames: 256,
            max_snapshot_bytes: LIMIT,
            max_event_bytes_per_tick: LIMIT,
            max_queued_publications: 256,
            max_queued_snapshot_bytes: 256 * 1024 * 1024,
            max_queued_event_batches: 1792,
            max_queued_event_bytes: 448 * 1024 * 1024,
        },
    )
    .ok()
}
fn encode_simulation_publications(
    publications: &[playsrc_simulation::Publication],
) -> Option<Vec<u8>> {
    let mut output = b"PSIM".to_vec();
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(publications.len()).ok()?.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    for publication in publications {
        output.extend_from_slice(&publication.host_frame.to_le_bytes());
        output.extend_from_slice(&publication.first_host_tick.to_le_bytes());
        output.extend_from_slice(&publication.last_host_tick.to_le_bytes());
        output.extend_from_slice(&publication.selected_ticks.to_le_bytes());
        output.extend_from_slice(&publication.interpolation.to_le_bytes());
        output.extend_from_slice(
            &u32::try_from(publication.snapshot.len())
                .ok()?
                .to_le_bytes(),
        );
        output.extend_from_slice(&u32::try_from(publication.events.len()).ok()?.to_le_bytes());
        output.extend_from_slice(&publication.snapshot);
        for event in &publication.events {
            output.extend_from_slice(&event.host_tick.to_le_bytes());
            output.extend_from_slice(&u32::try_from(event.bytes.len()).ok()?.to_le_bytes());
            output.extend_from_slice(&event.bytes);
        }
        if output.len() > 512 * 1024 * 1024 {
            return None;
        }
    }
    Some(output)
}
fn slots() -> &'static Mutex<Vec<Slot>> {
    static S: OnceLock<Mutex<Vec<Slot>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}
fn resource_output() -> &'static Mutex<Vec<u8>> {
    static OUTPUT: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    OUTPUT.get_or_init(|| Mutex::new(Vec::new()))
}
fn encode(index: usize, generation: u16) -> u32 {
    ((generation as u32) << 16) | (index as u32 + 1)
}
fn decode(handle: u32) -> Option<(usize, u16)> {
    let index = (handle & 0xffff).checked_sub(1)? as usize;
    Some((index, (handle >> 16) as u16))
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_alloc(length: usize) -> *mut u8 {
    let mut bytes = Vec::<u8>::with_capacity(length);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must be a live allocation returned by `playsrc_alloc` with the same capacity.
pub unsafe extern "C" fn playsrc_free(pointer: *mut u8, length: usize) {
    if !pointer.is_null() {
        drop(unsafe { Vec::from_raw_parts(pointer, 0, length) });
    }
}
#[unsafe(no_mangle)]
/// # Safety
/// A nonempty pointer/length pair must identify readable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_resource_decode(pointer: *const u8, length: usize) -> u32 {
    let bytes = if length == 0 {
        &[]
    } else if pointer.is_null() {
        return 0;
    } else {
        unsafe { std::slice::from_raw_parts(pointer, length) }
    };
    let Ok(decoded) = playsrc_asset_graph::decode_to_resource_set(bytes) else {
        resource_output().lock().expect("resource output").clear();
        return 0;
    };
    *resource_output().lock().expect("resource output") = decoded;
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_resource_length() -> usize {
    resource_output().lock().expect("resource output").len()
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_resource_take() -> *mut u8 {
    let mut output = resource_output().lock().expect("resource output");
    if output.is_empty() {
        return std::ptr::null_mut();
    }
    let bytes = std::mem::take(&mut *output).into_boxed_slice();
    Box::into_raw(bytes) as *mut u8
}
#[unsafe(no_mangle)]
/// # Safety
/// Each nonempty pointer/length pair must identify readable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_compile_map(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const u8,
    configuration_length: usize,
) -> u32 {
    unsafe {
        compile_map(
            bsp_pointer,
            bsp_length,
            profile,
            configuration_pointer,
            configuration_length,
            None,
        )
    }
}
#[unsafe(no_mangle)]
/// # Safety
/// Each nonempty pointer/length pair must identify readable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_compile_map_cached(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const u8,
    configuration_length: usize,
    presentation_pointer: *const u8,
    presentation_length: usize,
) -> u32 {
    let presentation = if presentation_length == 0 {
        None
    } else if presentation_pointer.is_null() {
        return 0;
    } else {
        Some(unsafe { std::slice::from_raw_parts(presentation_pointer, presentation_length) })
    };
    unsafe {
        compile_map(
            bsp_pointer,
            bsp_length,
            profile,
            configuration_pointer,
            configuration_length,
            presentation,
        )
    }
}

unsafe fn compile_map(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const u8,
    configuration_length: usize,
    cached_presentation: Option<&[u8]>,
) -> u32 {
    let mut metrics_clock = RuntimeMetricsClock::new();
    let compile_started =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    let mut phase_started = compile_started;
    let mut compile_metrics = [0_u64; 17];
    let bsp_bytes = if bsp_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bsp_pointer, bsp_length) }
    };
    let configuration = if configuration_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(configuration_pointer, configuration_length) }
    };
    let result = (|| {
        let bsp = playsrc_bsp::parse(
            bsp_bytes,
            playsrc_bsp::Profile::Source2013V20,
            playsrc_bsp::Limits::default(),
        )
        .map_err(|_| 1_u32)?;
        let bsp_sha: [u8; 32] = Sha256::digest(bsp_bytes).into();
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[0] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let profile = match profile {
            0 => playsrc_map::LightingProfile::Ldr,
            1 => playsrc_map::LightingProfile::Hdr,
            _ => return Err(2),
        };
        let resources = bundle(configuration).map_err(|_| 7_u32)?;
        let decoders = TextureDecoders::new(&resources);
        let resource_hashes = resources
            .par_iter()
            .map(|(identity, bytes)| (identity.clone(), Sha256::digest(bytes).into()))
            .collect::<BTreeMap<_, _>>();
        let model_resource_bytes = resources
            .par_iter()
            .filter(|(identity, _)| {
                [".mdl", ".vvd", ".vtx", ".ani", ".phy"]
                    .iter()
                    .any(|suffix| identity.ends_with(suffix))
            })
            .map(|(identity, bytes)| (identity.clone(), Arc::<[u8]>::from(*bytes)))
            .collect::<BTreeMap<_, _>>();
        let entity_graph =
            playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
                .map_err(|_| 3_u32)?;
        let collision_world = playsrc_collision::compile(&bsp).map_err(|_| 3_u32)?;
        let visibility_world = playsrc_visibility::compile(&bsp).map_err(|_| 3_u32)?;
        let mut canonical =
            playsrc_map::compile_prepared(&bsp, profile, &entity_graph, &collision_world).map_err(
                |error| {
                    if error.code == playsrc_map::ErrorCode::IncompleteLightingProfile {
                        6_u32
                    } else {
                        3_u32
                    }
                },
            )?;
        let visibility_world =
            playsrc_map::attach_displacement_visibility(&canonical, &visibility_world)
                .map_err(|_| 3_u32)?;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[1] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let (resolved_materials, map_materials) =
            resolve_materials(&canonical, &resources, &decoders, profile).map_err(|_| 7_u32)?;
        let collision_world = attach_displacement_collision_inputs(
            collision_world,
            &canonical,
            &resources,
            &map_materials,
        )
        .map_err(|_| 3_u32)?;
        canonical.collision_world_identity = collision_world.identity;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[2] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[3] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let (particles, particle_materials, particle_sheets, particle_presentation) =
            compile_particles(&resources, &decoders).map_err(|_| 10_u32)?;
        let static_model_roots = canonical
            .static_props
            .models
            .iter()
            .map(|model| model.logical_path.clone())
            .chain(
                [
                    "models/vgui/ui_team01.mdl",
                    "models/vgui/ui_team01_blue.mdl",
                    "models/vgui/ui_team01_red.mdl",
                    "models/vgui/ui_team01_random.mdl",
                    "models/vgui/ui_team01_spectate.mdl",
                ]
                .into_iter()
                .map(str::to_owned),
            )
            .collect::<Vec<_>>();
        let presentation_inputs = PresentationInputs {
            canonical: &canonical,
            bsp: &bsp,
            graph: &entity_graph,
            bundle: &resources,
            decoders: &decoders,
            model_resources: &model_resource_bytes,
            resource_hashes: &resource_hashes,
            map_materials: &map_materials,
            particle_presentation: &particle_presentation,
            profile,
            visibility: &visibility_world,
            collision: &collision_world,
            additional_model_roots: &static_model_roots,
        };
        let (
            (presentation, studio_models, model_material_opacity, environment),
            presentation_metrics,
            _presentation_ledger,
        ) = if let Some(cached) = cached_presentation {
            load_cached_presentation(presentation_inputs, cached).map_err(|_| 9_u32)?
        } else {
            compile_presentation(presentation_inputs).map_err(|_| 9_u32)?
        };
        compile_metrics[11..17].copy_from_slice(&presentation_metrics);
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[4] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let (runtime_models, model_occurrences) = resolve_models(
            &entity_graph,
            &studio_models,
            &resources,
            &decoders,
            profile,
            &canonical.static_props,
        )
        .map_err(|_| 8_u32)?;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[5] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let profile_materials = resolve_profile_materials(
            &entity_graph,
            &resources,
            &decoders,
            &resource_hashes,
            profile,
        )
        .map_err(|_| 7_u32)?;
        let inputs = runtime_inputs(&resources, &resource_hashes);
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[6] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let displacement_runtime = canonical
            .surfaces
            .iter()
            .any(|surface| surface.displacement.is_some());
        let runtime = playsrc_map::assemble_prepared_runtime(
            canonical,
            entity_graph,
            collision_world,
            visibility_world,
            bsp_sha,
            playsrc_map::RuntimeAssembly {
                compiler_identity: if profile == playsrc_map::LightingProfile::Hdr {
                    if displacement_runtime {
                        "playsrc-map-runtime-hdr-2"
                    } else {
                        "playsrc-map-runtime-hdr-1"
                    }
                } else {
                    "playsrc-map-runtime-2"
                },
                configuration,
                materials: &resolved_materials,
                profile_materials: &profile_materials,
                inputs: &inputs,
                output_role: if profile == playsrc_map::LightingProfile::Hdr {
                    "map-runtime-hdr"
                } else {
                    "map-runtime"
                },
                models: &runtime_models,
                model_occurrences: &model_occurrences,
            },
        )
        .map_err(|error| {
            if error.code == playsrc_map::ErrorCode::IncompleteLightingProfile {
                6_u32
            } else {
                3_u32
            }
        })?;
        let coverage = encode_profile_coverage(
            &runtime.map.lighting,
            &runtime.visibility,
            &runtime.collision,
            &runtime.entities,
        )
        .map_err(|_| 5_u32)?;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[7] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let spawn = spawn(&runtime.entities).ok_or(4_u32)?;
        let studio_model_checksums = studio_models
            .iter()
            .map(|(identity, model)| (identity.clone(), model.checksum))
            .collect();
        let collision_templates = collision_object_templates(
            &runtime.map,
            &runtime.entities,
            &resources,
            &studio_model_checksums,
        )
        .map_err(|_| 5_u32)?;
        let visibility = runtime.visibility;
        let area_state = playsrc_visibility::AreaState::new(&visibility);
        let collision = Arc::new(runtime.collision);
        let initial_collision = compile_collision_snapshot(
            &collision,
            &collision_templates,
            1,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .map_err(|_| 5_u32)?;
        let gameplay_world = SharedWorld::new(collision.clone(), initial_collision);
        let model_bounds = collision
            .models
            .iter()
            .enumerate()
            .map(|(model, value)| playsrc_entity::ModelBounds {
                model,
                mins: [
                    value.mins.x.value(),
                    value.mins.y.value(),
                    value.mins.z.value(),
                ],
                maxs: [
                    value.maxs.x.value(),
                    value.maxs.y.value(),
                    value.maxs.z.value(),
                ],
            })
            .collect();
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[8] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let map = playsrc_tf2::MapRuntime::compile(
            &runtime.entities,
            playsrc_movement::Configuration::default().tick_interval,
            u64::from_le_bytes(bsp_sha[..8].try_into().expect("BSP identity prefix")),
            model_bounds,
        )
        .map_err(|_| 5_u32)?;
        let rules = playsrc_tf2::team_selection::TeamRules {
            attack_defend: runtime.entities.entities.iter().any(|entity| {
                entity
                    .classname
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(b"team_train_watcher"))
            }),
            mann_vs_machine: runtime.entities.entities.iter().any(|entity| {
                entity
                    .classname
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(b"tf_logic_mann_vs_machine"))
            }),
            ..playsrc_tf2::team_selection::TeamRules::default()
        };
        let mut session =
            playsrc_tf2::Session::connected(gameplay_world.clone(), spawn.position, map, rules);
        if let Some(bytes) = resources.get("maps/pl_upward.nav") {
            let mesh = playsrc_nav::parse(
                bytes,
                playsrc_nav::Profile::TeamFortress2,
                Some(u32::try_from(bsp_bytes.len()).map_err(|_| 11_u32)?),
                playsrc_nav::Limits::default(),
            )
            .map_err(|_| 11_u32)?;
            session
                .configure_navigation(mesh, &runtime.entities)
                .map_err(|_| 11_u32)?;
        }
        session.set_movement_modifiers(playsrc_tf2::MovementModifiers {
            noclip_allowed: true,
            ..playsrc_tf2::MovementModifiers::default()
        });
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[9] = phase_finished.saturating_sub(phase_started);
        compile_metrics[10] = phase_finished.saturating_sub(compile_started);
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_sha256,
            runtime.descriptor.derived_sha256,
            presentation,
            coverage,
            particles,
            particle_materials,
            particle_sheets,
            studio_models,
            model_material_opacity,
            environment,
            visibility,
            area_state,
            collision,
            gameplay_world,
            collision_templates,
            bsp_sha,
            spawn,
            session,
            [
                decoders.requests.load(Ordering::Relaxed),
                decoders.inspections.load(Ordering::Relaxed),
            ],
        ))
    })();
    let mut slots = slots().lock().expect("TF2 slots");
    let index = slots
        .iter()
        .position(|slot| slot.payload.is_none())
        .unwrap_or(slots.len());
    let generation = if index == slots.len() {
        1
    } else {
        slots[index].generation.wrapping_add(1).max(1)
    };
    let slot = match result {
        Ok((
            payload,
            hash,
            derived_hash,
            presentation,
            coverage,
            particles,
            particle_materials,
            particle_sheets,
            studio_models,
            model_material_opacity,
            environment,
            visibility,
            area_state,
            collision,
            gameplay_world,
            collision_templates,
            bsp_hash,
            spawn,
            session,
            texture_inspections,
        )) => Slot {
            generation,
            payload: Some(payload),
            presentation,
            coverage,
            particles: Some(particles),
            particle_materials,
            particle_sheets,
            particle_output: Vec::new(),
            studio_models,
            model_material_opacity,
            viewmodel_bob: BTreeMap::new(),
            model_output: Vec::new(),
            visibility: Some(visibility),
            area_state: Some(area_state),
            visibility_output: Vec::new(),
            environment: Some(environment),
            collision: Some(collision),
            gameplay_world: Some(gameplay_world),
            collision_templates,
            collision_revision: 1,
            pushers: BTreeMap::new(),
            latest_game_snapshot: None,
            hash,
            derived_hash,
            bsp_hash,
            error: 0,
            spawn: Some(spawn),
            session: Some(session),
            snapshot: Vec::new(),
            compile_metrics,
            texture_inspections,
        },
        Err(error) => Slot {
            generation,
            payload: Some(Vec::new()),
            presentation: Vec::new(),
            coverage: Vec::new(),
            particles: None,
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            model_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            environment: None,
            collision: None,
            gameplay_world: None,
            collision_templates: Vec::new(),
            collision_revision: 0,
            pushers: BTreeMap::new(),
            latest_game_snapshot: None,
            hash: [0; 32],
            derived_hash: [0; 32],
            bsp_hash: [0; 32],
            error,
            spawn: None,
            session: None,
            snapshot: Vec::new(),
            compile_metrics,
            texture_inspections: [0; 2],
        },
    };
    if index == slots.len() {
        slots.push(slot)
    } else {
        slots[index] = slot
    }
    encode(index, generation)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledArtifact {
    pub payload: Vec<u8>,
    pub payload_sha256: [u8; 32],
    pub derived_sha256: [u8; 32],
}
pub fn compile_artifact(
    bsp: &[u8],
    profile: u32,
    configuration: &[u8],
) -> Result<CompiledArtifact, u32> {
    let handle = unsafe {
        playsrc_compile_map(
            bsp.as_ptr(),
            bsp.len(),
            profile,
            configuration.as_ptr(),
            configuration.len(),
        )
    };
    let failure = playsrc_result_error(handle);
    if failure != 0 {
        playsrc_dispose(handle);
        return Err(failure);
    }
    let mut payload = vec![0; playsrc_result_length(handle)];
    if unsafe { playsrc_result_copy(handle, payload.as_mut_ptr(), payload.len()) } != payload.len()
    {
        playsrc_dispose(handle);
        return Err(u32::MAX);
    }
    let mut payload_sha256 = [0; 32];
    let mut derived_sha256 = [0; 32];
    if unsafe { playsrc_result_hash(handle, payload_sha256.as_mut_ptr()) } != 1
        || unsafe { playsrc_result_derived_hash(handle, derived_sha256.as_mut_ptr()) } != 1
    {
        playsrc_dispose(handle);
        return Err(u32::MAX);
    }
    playsrc_dispose(handle);
    Ok(CompiledArtifact {
        payload,
        payload_sha256,
        derived_sha256,
    })
}

#[cfg(feature = "presentation-bound-diagnostic")]
pub fn diagnose_presentation_bound(
    bsp_bytes: &[u8],
    resources: &BTreeMap<String, Vec<u8>>,
    additional_model_roots: &[String],
) -> Result<PresentationSizeLedger, u32> {
    let resources = resources
        .iter()
        .map(|(identity, bytes)| (identity.clone(), bytes.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let decoders = TextureDecoders::new(&resources);
    let resource_hashes = resources
        .iter()
        .map(|(identity, bytes)| (identity.clone(), Sha256::digest(bytes).into()))
        .collect::<BTreeMap<_, _>>();
    let model_resources = resources
        .iter()
        .filter(|(identity, _)| {
            [".mdl", ".vvd", ".vtx", ".ani", ".phy"]
                .iter()
                .any(|suffix| identity.ends_with(suffix))
        })
        .map(|(identity, bytes)| (identity.clone(), Arc::<[u8]>::from(*bytes)))
        .collect::<BTreeMap<_, _>>();
    let bsp = playsrc_bsp::parse(
        bsp_bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .map_err(|_| 1u32)?;
    let entities =
        playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
            .map_err(|_| 3u32)?;
    let collision = playsrc_collision::compile(&bsp).map_err(|_| 3u32)?;
    let visibility = playsrc_visibility::compile(&bsp).map_err(|_| 3u32)?;
    let mut canonical = playsrc_map::compile_prepared(
        &bsp,
        playsrc_map::LightingProfile::Hdr,
        &entities,
        &collision,
    )
    .map_err(|_| 3u32)?;
    let map_materials = canonical
        .materials
        .iter()
        .map(|material| {
            resolve_material_semantics(
                &material.logical_path.to_ascii_lowercase(),
                &resources,
                material_environment(playsrc_map::LightingProfile::Hdr, false),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| 3u32)?;
    let collision =
        attach_displacement_collision_inputs(collision, &canonical, &resources, &map_materials)
            .map_err(|_| 3u32)?;
    canonical.collision_world_identity = collision.identity;
    let visibility =
        playsrc_map::attach_displacement_visibility(&canonical, &visibility).map_err(|_| 3u32)?;
    let (_, _, _, particle_presentation) =
        compile_particles(&resources, &decoders).map_err(|_| 10u32)?;
    let ((_, models, _, _), metrics, mut ledger) = compile_presentation(PresentationInputs {
        canonical: &canonical,
        bsp: &bsp,
        graph: &entities,
        bundle: &resources,
        decoders: &decoders,
        model_resources: &model_resources,
        resource_hashes: &resource_hashes,
        map_materials: &map_materials,
        particle_presentation: &particle_presentation,
        profile: playsrc_map::LightingProfile::Hdr,
        visibility: &visibility,
        collision: &collision,
        additional_model_roots,
    })
    .map_err(|_| 9u32)?;
    let studio_model_checksums = models
        .iter()
        .map(|(identity, model)| (identity.clone(), model.checksum))
        .collect();
    let templates =
        collision_object_templates(&canonical, &entities, &resources, &studio_model_checksums)
            .map_err(|_| 5u32)?;
    ledger.displacement_input_count = collision.displacement_inputs.len();
    ledger.static_prop_collision_count = templates
        .iter()
        .filter(|template| template.input.role == playsrc_collision::ObjectRole::StaticProp)
        .count();
    ledger.phase_milliseconds = metrics.map(|nanoseconds| nanoseconds / 1_000_000);
    Ok(ledger)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_length(handle: u32) -> usize {
    with(handle, |slot| slot.payload.as_ref().map_or(0, Vec::len)).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_error(handle: u32) -> u32 {
    with(handle, |slot| slot.error).unwrap_or(u32::MAX)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_compile_metric_milliseconds(handle: u32, index: usize) -> f64 {
    with(handle, |slot| {
        slot.compile_metrics
            .get(index)
            .map_or(0.0, |value| *value as f64 / 1_000_000.0)
    })
    .unwrap_or(0.0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_texture_inspection_count(handle: u32, index: usize) -> u32 {
    with(handle, |slot| {
        slot.texture_inspections.get(index).copied().unwrap_or(0)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_presentation_length(handle: u32) -> usize {
    with(handle, |slot| slot.presentation.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_presentation_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.presentation.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.presentation.as_ptr(),
                pointer,
                slot.presentation.len(),
            )
        };
        slot.presentation.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_presentation_release(handle: u32) -> u32 {
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    slot.presentation.clear();
    slot.presentation.shrink_to_fit();
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_coverage_length(handle: u32) -> usize {
    with(handle, |slot| slot.coverage.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable memory for `capacity` bytes.
pub unsafe extern "C" fn playsrc_coverage_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if pointer.is_null() || capacity < slot.coverage.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(slot.coverage.as_ptr(), pointer, slot.coverage.len())
        };
        slot.coverage.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify `length` readable transaction bytes.
pub unsafe extern "C" fn playsrc_particle_transact(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if length == 0 || length > 4 * 1024 * 1024 {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Ok((events, request)) = decode_particle_transaction(bytes) else {
        *SIMULATION_ERROR_DETAIL
            .get_or_init(|| Mutex::new(String::new()))
            .lock()
            .expect("particle decode error detail") = format!(
            "particle transaction rejected: bytes={}, events={}, from={:?}, to={:?}",
            bytes.len(),
            bytes
                .get(28..32)
                .and_then(|value| value.try_into().ok())
                .map(u32::from_le_bytes)
                .unwrap_or(u32::MAX),
            bytes
                .get(8..12)
                .and_then(|value| value.try_into().ok())
                .map(f32::from_le_bytes),
            bytes
                .get(12..16)
                .and_then(|value| value.try_into().ok())
                .map(f32::from_le_bytes),
        );
        return 0;
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    let Some(collision_world) = slot.collision.clone() else {
        return 0;
    };
    let Some(world) = slot.particles.as_mut() else {
        return 0;
    };
    let mut collision = ParticleCollision(collision_world);
    let output = match world.transact_render_output(
        &events,
        request,
        &mut collision,
        &slot.particle_sheets,
        &slot.particle_materials,
        64 * 1024 * 1024,
    ) {
        Ok(output) => output,
        Err(error) => {
            *SIMULATION_ERROR_DETAIL
                .get_or_init(|| Mutex::new(String::new()))
                .lock()
                .expect("particle error detail") = error.to_string();
            return 0;
        }
    };
    slot.particle_output = output;
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_particle_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.particle_output.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_particle_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.particle_output.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.particle_output.as_ptr(),
                pointer,
                slot.particle_output.len(),
            )
        };
        slot.particle_output.len()
    })
    .unwrap_or(0)
}

#[derive(Debug)]
struct ModelPoseRequest {
    identity: u32,
    sample_tick: u64,
    attachments_only: bool,
    fire_view: Option<([f32; 3], [f32; 4])>,
    model: String,
    item: Option<String>,
    activity: String,
    previous_elapsed: f32,
    elapsed: f32,
    current_time: f32,
    frame_time: f32,
    planar_speed: f32,
    screen_aspect_ratio: f32,
    world_far_plane: f32,
    skin: usize,
    lod: usize,
    phase: Option<playsrc_studio_model::ViewModelPhase>,
    reflected_viewmodel: bool,
    owner_alive: bool,
    packed_body: Option<i32>,
    bodygroups: Vec<usize>,
    item_bodygroups: Vec<usize>,
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify exactly `length` readable model-pose request bytes.
pub unsafe extern "C" fn playsrc_model_transact(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if !(12..=1024 * 1024).contains(&length) {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Ok(requests) = decode_model_requests(bytes) else {
        return 0;
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    let Ok(output) = encode_model_poses(
        &slot.studio_models,
        &slot.model_material_opacity,
        &mut slot.viewmodel_bob,
        &requests,
    ) else {
        return 0;
    };
    slot.model_output = output;
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_model_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.model_output.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_model_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.model_output.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.model_output.as_ptr(),
                pointer,
                slot.model_output.len(),
            )
        };
        slot.model_output.len()
    })
    .unwrap_or(0)
}

fn decode_model_requests(bytes: &[u8]) -> Result<Vec<ModelPoseRequest>, ()> {
    let mut reader = ParticleReader { bytes, at: 0 };
    if reader.take(4)? != b"PMRQ" || reader.u32()? != 6 {
        return Err(());
    }
    let count = reader.u32()? as usize;
    if count > 128 {
        return Err(());
    }
    let mut requests = Vec::with_capacity(count);
    let mut identities = std::collections::BTreeMap::<u32, (u64, bool)>::new();
    for _ in 0..count {
        let identity = reader.u32()?;
        let sample_tick = reader.u64()?;
        let kind = reader.u8()?;
        let attachments_only = reader.u8()?;
        let has_fire_view = reader.u8()?;
        if kind > 2
            || attachments_only > 1
            || has_fire_view > 1
            || (attachments_only == 1 && (kind != 1 || has_fire_view != 1))
            || (has_fire_view == 1 && kind != 1)
            || reader.u8()? != 0
        {
            return Err(());
        }
        let eye = [reader.f32()?, reader.f32()?, reader.f32()?];
        let orientation = [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?];
        let fire_view = if has_fire_view == 1 {
            let magnitude = orientation.iter().map(|value| value * value).sum::<f32>();
            if (magnitude - 1.0).abs() > 1.0e-4 {
                return Err(());
            }
            Some((eye, orientation))
        } else {
            if eye.into_iter().chain(orientation).any(|value| value != 0.0) {
                return Err(());
            }
            None
        };
        let model = reader.text()?.to_ascii_lowercase();
        let item_text = reader.text()?.to_ascii_lowercase();
        let item = match kind {
            0 | 2 if item_text.is_empty() => None,
            1 if !item_text.is_empty() => Some(item_text),
            _ => return Err(()),
        };
        let activity = reader.text()?;
        let previous_elapsed = reader.f32()?;
        let elapsed = reader.f32()?;
        let current_time = reader.f32()?;
        let frame_time = reader.f32()?;
        let planar_speed = reader.f32()?;
        let screen_aspect_ratio = reader.f32()?;
        let world_far_plane = reader.f32()?;
        let skin = reader.u32()? as usize;
        let lod = reader.u32()? as usize;
        let phase_code = reader.u8()?;
        let reflected_viewmodel = reader.u8()?;
        let owner_alive = reader.u8()?;
        if reflected_viewmodel > 1 || owner_alive > 1 || reader.u8()? != 0 {
            return Err(());
        }
        let phase = match phase_code {
            0 => Some(playsrc_studio_model::ViewModelPhase::Draw),
            1 => Some(playsrc_studio_model::ViewModelPhase::PrimaryFire),
            2 => Some(playsrc_studio_model::ViewModelPhase::ReloadStart),
            3 => Some(playsrc_studio_model::ViewModelPhase::ReloadInsertOrLoop),
            4 => Some(playsrc_studio_model::ViewModelPhase::ReloadFinish),
            5 => Some(playsrc_studio_model::ViewModelPhase::Idle),
            u8::MAX if kind == 0 => None,
            _ => return Err(()),
        };
        let packed_body_value = i32::from_le_bytes(reader.take(4)?.try_into().map_err(|_| ())?);
        let packed_body = (packed_body_value != i32::MIN).then_some(packed_body_value);
        if (kind != 0 && packed_body.is_some()) || packed_body.is_some_and(|value| value < 0) {
            return Err(());
        }
        let bodygroup_count = reader.u32()? as usize;
        if identity == 0
            || identities
                .get(&identity)
                .is_some_and(|(prior_tick, prior_attachment)| {
                    !*prior_attachment
                        || sample_tick < *prior_tick
                        || (sample_tick == *prior_tick && attachments_only == 1)
                })
            || model.is_empty()
            || activity.is_empty()
            || previous_elapsed < 0.0
            || elapsed < previous_elapsed
            || current_time < 0.0
            || frame_time < 0.0
            || planar_speed < 0.0
            || screen_aspect_ratio <= 0.0
            || world_far_plane <= 0.0
            || bodygroup_count > 64
        {
            return Err(());
        }
        let bodygroups = (0..bodygroup_count)
            .map(|_| reader.u32().map(|value| value as usize))
            .collect::<Result<Vec<_>, _>>()?;
        let item_bodygroup_count = reader.u32()? as usize;
        if item_bodygroup_count > 64 || (item.is_none() && item_bodygroup_count != 0) {
            return Err(());
        }
        let item_bodygroups = (0..item_bodygroup_count)
            .map(|_| reader.u32().map(|value| value as usize))
            .collect::<Result<Vec<_>, _>>()?;
        identities.insert(identity, (sample_tick, attachments_only == 1));
        requests.push(ModelPoseRequest {
            identity,
            sample_tick,
            attachments_only: attachments_only == 1,
            fire_view,
            model,
            item,
            activity,
            previous_elapsed,
            elapsed,
            current_time,
            frame_time,
            planar_speed,
            screen_aspect_ratio,
            world_far_plane,
            skin,
            lod,
            phase,
            reflected_viewmodel: reflected_viewmodel != 0,
            owner_alive: owner_alive != 0,
            packed_body,
            bodygroups,
            item_bodygroups,
        });
    }
    (reader.at == bytes.len()).then_some(requests).ok_or(())
}

fn pose_bot_hitboxes(
    models: &BTreeMap<String, playsrc_studio_model::PresentationModel>,
    bots: &[playsrc_tf2::bot::Snapshot],
    tick: u64,
) -> Result<Vec<playsrc_tf2::PosedPlayerHitbox>, ()> {
    let mut output = Vec::new();
    for bot in bots {
        if bot.lifecycle != playsrc_tf2::PlayerLifecycle::Active {
            continue;
        }
        let model = models.get(bot.class.data().model).ok_or(())?;
        let role = match bot.weapon.map(|weapon| weapon.weapon) {
            Some(
                playsrc_tf2::Weapon::Bat
                | playsrc_tf2::Weapon::Shovel
                | playsrc_tf2::Weapon::Fists
                | playsrc_tf2::Weapon::Kukri
                | playsrc_tf2::Weapon::Wrench,
            ) => "MELEE",
            Some(
                playsrc_tf2::Weapon::Pistol
                | playsrc_tf2::Weapon::Shotgun
                | playsrc_tf2::Weapon::HeavyShotgun
                | playsrc_tf2::Weapon::Smg
                | playsrc_tf2::Weapon::EngineerPistol
                | playsrc_tf2::Weapon::StickybombLauncher,
            ) => "SECONDARY",
            None if bot.class == playsrc_tf2::PlayerClass::Spy => "MELEE",
            _ => "PRIMARY",
        };
        let moving =
            (bot.velocity[0] * bot.velocity[0] + bot.velocity[1] * bot.velocity[1]).sqrt() > 1.0;
        let activity = format!("ACT_MP_{}_{}", if moving { "RUN" } else { "STAND" }, role);
        let sequence =
            *playsrc_studio_model::sequences_for_activity_name(model, activity.as_bytes())
                .first()
                .ok_or(())?;
        let parameters = model
            .pose_parameters
            .iter()
            .map(|_| playsrc_studio_model::Float32(0))
            .collect::<Vec<_>>();
        let elapsed = tick as f32 * 0.015;
        let timing =
            playsrc_studio_model::sequence_timing(model, sequence, &parameters).map_err(|_| ())?;
        let pose = playsrc_studio_model::sample_pose_at_time(
            model,
            &playsrc_studio_model::AnimationState {
                base_sequence: sequence,
                cycle: playsrc_studio_model::Float32(pose_cycle(elapsed, timing).to_bits()),
                pose_parameters: parameters,
                layers: Vec::new(),
            },
            playsrc_studio_model::Float32(elapsed.to_bits()),
        )
        .map_err(|_| ())?;
        let (sine, cosine) = bot.yaw_degrees.to_radians().sin_cos();
        let matrix = playsrc_studio_model::Matrix3x4(
            [
                cosine,
                -sine,
                0.0,
                bot.position[0],
                sine,
                cosine,
                0.0,
                bot.position[1],
                0.0,
                0.0,
                1.0,
                bot.position[2],
            ]
            .map(|value| playsrc_studio_model::Float32(value.to_bits())),
        );
        let world =
            playsrc_studio_model::apply_entity_transform(model, &pose, matrix).map_err(|_| ())?;
        let Some(set) = model.hitbox_sets.first() else {
            continue;
        };
        for hitbox in &set.hitboxes {
            let bone_index = usize::try_from(hitbox.bone).map_err(|_| ())?;
            let bone = model.bones.get(bone_index).ok_or(())?;
            let transform = world.bone_matrices.get(bone_index).ok_or(())?;
            output.push(playsrc_tf2::PosedPlayerHitbox {
                entity: bot.identity,
                team: bot.team,
                hitbox: hitbox.index,
                group: hitbox.group,
                bone: bone_index,
                physics_bone: bone.physics_bone,
                bone_contents: bone.contents as u32,
                minimum: hitbox.bounds_min.0.map(|value| f32::from_bits(value.0)),
                maximum: hitbox.bounds_max.0.map(|value| f32::from_bits(value.0)),
                bone_to_world: transform.0.map(|value| f32::from_bits(value.0)),
                origin: bot.position,
            });
        }
    }
    Ok(output)
}

fn pose_cycle(elapsed: f32, timing: playsrc_studio_model::SequenceTiming) -> f32 {
    let value = elapsed * f32::from_bits(timing.cycles_per_second.0);
    if timing.looping {
        value - value.floor()
    } else {
        value.clamp(0.0, 1.0)
    }
}
struct ViewOutput {
    transform: playsrc_studio_model::ViewModelTransform,
    pass: playsrc_studio_model::ViewModelPassState,
    item_translucent: bool,
    phase: playsrc_studio_model::ViewModelPhase,
    draw_disposition: playsrc_studio_model::ViewModelDrawDisposition,
    reflected: bool,
    hand_facing: playsrc_studio_model::GeometryFacing,
    item_facing: playsrc_studio_model::GeometryFacing,
    hand_bodygroups: Vec<usize>,
    item_bodygroups: Vec<usize>,
    item_bodygroup_mutations: Vec<playsrc_studio_model::ViewModelBodygroupMutation>,
}

fn encode_model_poses(
    models: &BTreeMap<String, playsrc_studio_model::PresentationModel>,
    material_opacity: &BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    viewmodel_bob: &mut BTreeMap<u32, playsrc_studio_model::ViewModelBobState>,
    requests: &[ModelPoseRequest],
) -> Result<Vec<u8>, ()> {
    let mut out = b"PMPO".to_vec();
    out.extend_from_slice(&5u32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    let mut output_count = 0_u32;
    for request in requests {
        let model = models.get(&request.model).ok_or(())?;
        let bodygroups = if let Some(body) = request.packed_body {
            model
                .body_parts
                .iter()
                .map(|part| {
                    let count = i32::try_from(part.model_names.len()).map_err(|_| ())?;
                    if part.base <= 0 || count <= 0 {
                        return Err(());
                    }
                    usize::try_from((body / part.base) % count).map_err(|_| ())
                })
                .collect::<Result<Vec<_>, ()>>()?
        } else {
            request.bodygroups.clone()
        };
        let sequence =
            playsrc_studio_model::sequences_for_activity_name(model, request.activity.as_bytes())
                .first()
                .copied()
                .or_else(|| {
                    model
                        .sequences
                        .iter()
                        .find(|sequence| {
                            sequence
                                .label
                                .eq_ignore_ascii_case(request.activity.as_bytes())
                        })
                        .map(|sequence| sequence.index)
                })
                .ok_or(())?;
        let pose_parameters = model
            .pose_parameters
            .iter()
            .map(|_| playsrc_studio_model::Float32(0))
            .collect::<Vec<_>>();
        let timing = playsrc_studio_model::sequence_timing(model, sequence, &pose_parameters)
            .map_err(|_| ())?;
        let previous_cycle = pose_cycle(request.previous_elapsed, timing);
        let cycle = pose_cycle(request.elapsed, timing);
        let pose = playsrc_studio_model::sample_pose_at_time(
            model,
            &playsrc_studio_model::AnimationState {
                base_sequence: sequence,
                cycle: playsrc_studio_model::Float32(cycle.to_bits()),
                pose_parameters: pose_parameters.clone(),
                layers: Vec::new(),
            },
            playsrc_studio_model::Float32(request.elapsed.to_bits()),
        )
        .map_err(|_| ())?;
        let selected =
            playsrc_studio_model::select_primitives(model, &bodygroups, request.skin, request.lod)
                .map_err(|_| ())?;
        let events = playsrc_studio_model::presentation_events_between(
            model,
            sequence,
            playsrc_studio_model::Float32(previous_cycle.to_bits()),
            playsrc_studio_model::Float32(cycle.to_bits()),
        )
        .map_err(|_| ())?;
        if let Some(item_identity) = request.item.as_ref() {
            let item = models.get(item_identity).ok_or(())?;
            let frame = playsrc_studio_model::produce_viewmodel_frame(
                model,
                item,
                &playsrc_studio_model::ViewModelFrameRequest {
                    phase: request.phase.ok_or(())?,
                    previous_cycle: playsrc_studio_model::Float32(previous_cycle.to_bits()),
                    composition: playsrc_studio_model::ViewModelCompositionRequest {
                        translated_activity: request.activity.as_bytes().to_vec(),
                        hand_sequence: sequence,
                        cycle: playsrc_studio_model::Float32(cycle.to_bits()),
                        time: playsrc_studio_model::Float32(request.elapsed.to_bits()),
                        hand_pose_parameters: pose_parameters,
                        hand_layers: Vec::new(),
                        skin: request.skin,
                        hand_bodygroups: bodygroups,
                        item_bodygroups: request.item_bodygroups.clone(),
                        lod: request.lod,
                    },
                    hand_material_opacity: material_opacity.get(&request.model).ok_or(())?.clone(),
                    item_material_opacity: material_opacity.get(item_identity).ok_or(())?.clone(),
                    draw_eligibility: playsrc_studio_model::ViewModelDrawEligibility {
                        client_mode: true,
                        render_request: true,
                        render_viewmodels: true,
                        local_player_visible: false,
                        draw_entities: true,
                        player_view_entity: true,
                        base_should_draw: true,
                        fully_lowered: false,
                        observer_owner_matches: true,
                        owner_alive: request.owner_alive,
                        ready: true,
                        fx_blend: 255,
                    },
                    occurrence_orientation: playsrc_studio_model::TransformOrientation::Preserving,
                    reflected_viewmodel: request.reflected_viewmodel,
                },
            )
            .map_err(|_| ())?;
            let bob = playsrc_studio_model::update_viewmodel_bob(
                viewmodel_bob
                    .get(&request.identity)
                    .copied()
                    .unwrap_or_default(),
                playsrc_studio_model::ViewModelBobRequest {
                    current_time: playsrc_studio_model::Float32(request.current_time.to_bits()),
                    frame_time: playsrc_studio_model::Float32(request.frame_time.to_bits()),
                    planar_speed: playsrc_studio_model::Float32(request.planar_speed.to_bits()),
                    cycle: playsrc_studio_model::Float32(0.8f32.to_bits()),
                    up_fraction: playsrc_studio_model::Float32(0.5f32.to_bits()),
                },
            )
            .map_err(|_| ())?;
            viewmodel_bob.insert(request.identity, bob);
            let transform = playsrc_studio_model::apply_viewmodel_bob(
                playsrc_studio_model::ViewModelTransform {
                    origin: playsrc_studio_model::Vector3([playsrc_studio_model::Float32(0); 3]),
                    angles: playsrc_studio_model::Vector3([playsrc_studio_model::Float32(0); 3]),
                },
                bob,
            )
            .map_err(|_| ())?;
            let configured = match model.descriptor {
                playsrc_studio_model::PresentationDescriptor::ViewModel {
                    default_horizontal_fov_4_by_3,
                    ..
                } => default_horizontal_fov_4_by_3,
                _ => return Err(()),
            };
            let pass = playsrc_studio_model::viewmodel_pass_state(
                playsrc_studio_model::ViewModelProjectionRequest {
                    configured_horizontal_fov_4_by_3: configured,
                    default_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                    current_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                    screen_aspect_ratio: playsrc_studio_model::Float32(
                        request.screen_aspect_ratio.to_bits(),
                    ),
                    world_far_plane: playsrc_studio_model::Float32(
                        request.world_far_plane.to_bits(),
                    ),
                },
            )
            .map_err(|_| ())?;
            let state = ViewOutput {
                transform,
                pass,
                item_translucent: frame.draw_plan.item_entity_translucent,
                phase: frame.phase,
                draw_disposition: frame.draw_disposition,
                reflected: request.reflected_viewmodel,
                hand_facing: frame.hand_facing,
                item_facing: frame.item_facing,
                hand_bodygroups: frame.hand_bodygroups,
                item_bodygroups: frame.item_bodygroups,
                item_bodygroup_mutations: frame.item_bodygroup_mutations,
            };
            let event_refs = frame.crossed_events.iter().collect::<Vec<_>>();
            let mut part_count = 0_u32;
            for part in frame.draw_plan.parts {
                if request.attachments_only
                    && part.part != playsrc_studio_model::ViewModelPart::Item
                {
                    continue;
                }
                let (role, part_model, pose) = match part.part {
                    playsrc_studio_model::ViewModelPart::Hand => {
                        (1, model, &frame.composition.hand.pose)
                    }
                    playsrc_studio_model::ViewModelPart::Item => {
                        (2, item, &frame.composition.item.pose)
                    }
                };
                let selected = if request.attachments_only {
                    Vec::new()
                } else {
                    part.opaque_primitives
                        .iter()
                        .chain(&part.translucent_primitives)
                        .copied()
                        .collect::<Vec<_>>()
                };
                encode_model_pose_part(
                    &mut out,
                    request,
                    role,
                    part_model,
                    sequence,
                    timing,
                    previous_cycle,
                    cycle,
                    &event_refs,
                    pose,
                    &selected,
                    part.opaque_primitives.len(),
                    Some(&state),
                )?;
                part_count = part_count.checked_add(1).ok_or(())?;
            }
            if part_count != if request.attachments_only { 1 } else { 2 } {
                return Err(());
            }
            output_count = output_count.checked_add(part_count).ok_or(())?;
        } else if let Some(phase) = request.phase {
            let bob = playsrc_studio_model::update_viewmodel_bob(
                viewmodel_bob
                    .get(&request.identity)
                    .copied()
                    .unwrap_or_default(),
                playsrc_studio_model::ViewModelBobRequest {
                    current_time: playsrc_studio_model::Float32(request.current_time.to_bits()),
                    frame_time: playsrc_studio_model::Float32(request.frame_time.to_bits()),
                    planar_speed: playsrc_studio_model::Float32(request.planar_speed.to_bits()),
                    cycle: playsrc_studio_model::Float32(0.8f32.to_bits()),
                    up_fraction: playsrc_studio_model::Float32(0.5f32.to_bits()),
                },
            )
            .map_err(|_| ())?;
            viewmodel_bob.insert(request.identity, bob);
            let transform = playsrc_studio_model::apply_viewmodel_bob(
                playsrc_studio_model::ViewModelTransform {
                    origin: playsrc_studio_model::Vector3([playsrc_studio_model::Float32(0); 3]),
                    angles: playsrc_studio_model::Vector3([playsrc_studio_model::Float32(0); 3]),
                },
                bob,
            )
            .map_err(|_| ())?;
            let configured = match model.descriptor {
                playsrc_studio_model::PresentationDescriptor::ViewModel {
                    default_horizontal_fov_4_by_3,
                    ..
                } => default_horizontal_fov_4_by_3,
                _ => return Err(()),
            };
            let pass = playsrc_studio_model::viewmodel_pass_state(
                playsrc_studio_model::ViewModelProjectionRequest {
                    configured_horizontal_fov_4_by_3: configured,
                    default_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                    current_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                    screen_aspect_ratio: playsrc_studio_model::Float32(
                        request.screen_aspect_ratio.to_bits(),
                    ),
                    world_far_plane: playsrc_studio_model::Float32(
                        request.world_far_plane.to_bits(),
                    ),
                },
            )
            .map_err(|_| ())?;
            let facing = playsrc_studio_model::GeometryFacing {
                front_face: playsrc_studio_model::TriangleWinding::Clockwise,
                cull_face: playsrc_studio_model::CullFace::Back,
            };
            let state = ViewOutput {
                transform,
                pass,
                item_translucent: false,
                phase,
                draw_disposition: playsrc_studio_model::ViewModelDrawDisposition::Draw,
                reflected: request.reflected_viewmodel,
                hand_facing: facing,
                item_facing: facing,
                hand_bodygroups: bodygroups,
                item_bodygroups: Vec::new(),
                item_bodygroup_mutations: Vec::new(),
            };
            encode_model_pose_part(
                &mut out,
                request,
                1,
                model,
                sequence,
                timing,
                previous_cycle,
                cycle,
                &events,
                &pose,
                &selected,
                selected.len(),
                Some(&state),
            )?;
            output_count = output_count.checked_add(1).ok_or(())?;
        } else {
            let legacy_view = if let playsrc_studio_model::PresentationDescriptor::ViewModel {
                default_horizontal_fov_4_by_3,
                ..
            } = model.descriptor
            {
                let bob = playsrc_studio_model::update_viewmodel_bob(
                    viewmodel_bob
                        .get(&request.identity)
                        .copied()
                        .unwrap_or_default(),
                    playsrc_studio_model::ViewModelBobRequest {
                        current_time: playsrc_studio_model::Float32(request.current_time.to_bits()),
                        frame_time: playsrc_studio_model::Float32(request.frame_time.to_bits()),
                        planar_speed: playsrc_studio_model::Float32(request.planar_speed.to_bits()),
                        cycle: playsrc_studio_model::Float32(0.8f32.to_bits()),
                        up_fraction: playsrc_studio_model::Float32(0.5f32.to_bits()),
                    },
                )
                .map_err(|_| ())?;
                viewmodel_bob.insert(request.identity, bob);
                let transform = playsrc_studio_model::apply_viewmodel_bob(
                    playsrc_studio_model::ViewModelTransform {
                        origin: playsrc_studio_model::Vector3(
                            [playsrc_studio_model::Float32(0); 3],
                        ),
                        angles: playsrc_studio_model::Vector3(
                            [playsrc_studio_model::Float32(0); 3],
                        ),
                    },
                    bob,
                )
                .map_err(|_| ())?;
                let pass = playsrc_studio_model::viewmodel_pass_state(
                    playsrc_studio_model::ViewModelProjectionRequest {
                        configured_horizontal_fov_4_by_3: default_horizontal_fov_4_by_3,
                        default_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                        current_world_fov: playsrc_studio_model::Float32(75f32.to_bits()),
                        screen_aspect_ratio: playsrc_studio_model::Float32(
                            request.screen_aspect_ratio.to_bits(),
                        ),
                        world_far_plane: playsrc_studio_model::Float32(
                            request.world_far_plane.to_bits(),
                        ),
                    },
                )
                .map_err(|_| ())?;
                let facing = playsrc_studio_model::GeometryFacing {
                    front_face: playsrc_studio_model::TriangleWinding::Clockwise,
                    cull_face: playsrc_studio_model::CullFace::Back,
                };
                Some(ViewOutput {
                    transform,
                    pass,
                    item_translucent: false,
                    phase: if request.activity.eq_ignore_ascii_case("ACT_VM_DRAW") {
                        playsrc_studio_model::ViewModelPhase::Draw
                    } else {
                        playsrc_studio_model::ViewModelPhase::Idle
                    },
                    draw_disposition: playsrc_studio_model::ViewModelDrawDisposition::Draw,
                    reflected: false,
                    hand_facing: facing,
                    item_facing: facing,
                    hand_bodygroups: bodygroups,
                    item_bodygroups: Vec::new(),
                    item_bodygroup_mutations: Vec::new(),
                })
            } else {
                None
            };
            encode_model_pose_part(
                &mut out,
                request,
                u8::from(legacy_view.is_some()),
                model,
                sequence,
                timing,
                previous_cycle,
                cycle,
                &events,
                &pose,
                &selected,
                selected.len(),
                legacy_view.as_ref(),
            )?;
            output_count = output_count.checked_add(1).ok_or(())?;
        }
        if out.len() > 64 * 1024 * 1024 {
            return Err(());
        }
    }
    out[8..12].copy_from_slice(&output_count.to_le_bytes());
    Ok(out)
}

fn viewmodel_phase_code(value: playsrc_studio_model::ViewModelPhase) -> u8 {
    match value {
        playsrc_studio_model::ViewModelPhase::Draw => 0,
        playsrc_studio_model::ViewModelPhase::PrimaryFire => 1,
        playsrc_studio_model::ViewModelPhase::ReloadStart => 2,
        playsrc_studio_model::ViewModelPhase::ReloadInsertOrLoop => 3,
        playsrc_studio_model::ViewModelPhase::ReloadFinish => 4,
        playsrc_studio_model::ViewModelPhase::Idle => 5,
    }
}

fn viewmodel_suppression_code(value: playsrc_studio_model::ViewModelDrawSuppression) -> u8 {
    match value {
        playsrc_studio_model::ViewModelDrawSuppression::ClientMode => 1,
        playsrc_studio_model::ViewModelDrawSuppression::RenderRequest => 2,
        playsrc_studio_model::ViewModelDrawSuppression::RenderViewModels => 3,
        playsrc_studio_model::ViewModelDrawSuppression::LocalPlayerVisible => 4,
        playsrc_studio_model::ViewModelDrawSuppression::EntitiesDisabled => 5,
        playsrc_studio_model::ViewModelDrawSuppression::NonPlayerViewEntity => 6,
        playsrc_studio_model::ViewModelDrawSuppression::BaseShouldDraw => 7,
        playsrc_studio_model::ViewModelDrawSuppression::FullyLowered => 8,
        playsrc_studio_model::ViewModelDrawSuppression::ObserverOwnerMismatch => 9,
        playsrc_studio_model::ViewModelDrawSuppression::OwnerDead => 10,
        playsrc_studio_model::ViewModelDrawSuppression::NotReady => 11,
        playsrc_studio_model::ViewModelDrawSuppression::ZeroBlend => 12,
    }
}

fn viewmodel_draw_codes(value: playsrc_studio_model::ViewModelDrawDisposition) -> [u8; 2] {
    match value {
        playsrc_studio_model::ViewModelDrawDisposition::Draw => [0, 0],
        playsrc_studio_model::ViewModelDrawDisposition::SuppressedSuccess(reason) => {
            [1, viewmodel_suppression_code(reason)]
        }
        playsrc_studio_model::ViewModelDrawDisposition::Suppressed(reason) => {
            [2, viewmodel_suppression_code(reason)]
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_model_pose_part(
    out: &mut Vec<u8>,
    request: &ModelPoseRequest,
    role: u8,
    model: &playsrc_studio_model::PresentationModel,
    sequence: usize,
    timing: playsrc_studio_model::SequenceTiming,
    previous_cycle: f32,
    cycle: f32,
    events: &[&playsrc_studio_model::SequenceEvent],
    pose: &playsrc_studio_model::SampledPose,
    selected: &[playsrc_studio_model::SelectedPrimitive],
    opaque_count: usize,
    view: Option<&ViewOutput>,
) -> Result<(), ()> {
    out.extend_from_slice(&request.identity.to_le_bytes());
    out.extend_from_slice(&request.sample_tick.to_le_bytes());
    out.extend_from_slice(&[
        role,
        u8::from(request.attachments_only),
        u8::from(request.fire_view.is_some()),
        0,
    ]);
    pbytes(out, model.identity.as_bytes())?;
    pbytes(out, request.activity.as_bytes())?;
    out.extend_from_slice(&u32::try_from(sequence).map_err(|_| ())?.to_le_bytes());
    for value in [
        timing.frames_per_second,
        timing.weighted_frame_count,
        timing.cycles_per_second,
        timing.duration_seconds,
    ] {
        out.extend_from_slice(&value.0.to_le_bytes());
    }
    out.extend_from_slice(&[u8::from(timing.looping), 0, 0, 0]);
    out.extend_from_slice(&previous_cycle.to_le_bytes());
    out.extend_from_slice(&cycle.to_le_bytes());
    if let Some(view) = view {
        out.extend_from_slice(&[1, 0, 0, 0]);
        for value in view
            .transform
            .origin
            .0
            .into_iter()
            .chain(view.transform.angles.0)
            .chain([
                view.pass.projection.unscaled_horizontal_fov_4_by_3,
                view.pass.projection.horizontal_fov,
                view.pass.projection.aspect_ratio,
                view.pass.projection.near_plane,
                view.pass.projection.far_plane,
            ])
            .chain(view.pass.view_depth_range)
            .chain(view.pass.restored_depth_range)
        {
            out.extend_from_slice(&value.0.to_le_bytes())
        }
        out.extend_from_slice(&[
            u8::from(view.pass.projection_restored && view.pass.view_restored),
            u8::from(
                view.pass.restored_depth_range
                    == [
                        playsrc_studio_model::Float32(0),
                        playsrc_studio_model::Float32(1f32.to_bits()),
                    ],
            ),
            u8::from(view.item_translucent),
            0,
        ])
    } else {
        out.extend_from_slice(&[0; 68])
    }
    if let Some(view) = view {
        let [draw_disposition, suppression] = viewmodel_draw_codes(view.draw_disposition);
        let facing = if role == 1 {
            view.hand_facing
        } else if role == 2 {
            view.item_facing
        } else {
            return Err(());
        };
        out.extend_from_slice(&[
            viewmodel_phase_code(view.phase),
            draw_disposition,
            suppression,
            u8::from(view.reflected),
            match facing.front_face {
                playsrc_studio_model::TriangleWinding::Clockwise => 0,
                playsrc_studio_model::TriangleWinding::CounterClockwise => 1,
            },
            0,
            match view.pass.restored_cull_mode {
                playsrc_studio_model::ViewModelCullMode::CounterClockwise => 0,
                playsrc_studio_model::ViewModelCullMode::Clockwise => 1,
            },
            0,
        ]);
        out.extend_from_slice(
            &u32::try_from(view.hand_bodygroups.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for value in &view.hand_bodygroups {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(view.item_bodygroups.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for value in &view.item_bodygroups {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(view.item_bodygroup_mutations.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for mutation in &view.item_bodygroup_mutations {
            out.extend_from_slice(&u32::try_from(mutation.event).map_err(|_| ())?.to_le_bytes());
            out.extend_from_slice(
                &u32::try_from(mutation.bodygroup)
                    .map_err(|_| ())?
                    .to_le_bytes(),
            );
            out.extend_from_slice(&mutation.value.to_le_bytes());
            pbytes(out, &mutation.name)?;
        }
    } else {
        out.extend_from_slice(&[0; 8]);
        out.extend_from_slice(&[0; 12]);
    }
    out.extend_from_slice(&u32::try_from(events.len()).map_err(|_| ())?.to_le_bytes());
    for event in events {
        out.extend_from_slice(&u32::try_from(event.index).map_err(|_| ())?.to_le_bytes());
        out.extend_from_slice(&event.cycle.0.to_le_bytes());
        out.extend_from_slice(&event.event.to_le_bytes());
        out.extend_from_slice(&event.event_type.to_le_bytes());
        out.extend_from_slice(&event.options);
        pbytes(out, &event.name)?;
    }
    out.extend_from_slice(&u32::try_from(selected.len()).map_err(|_| ())?.to_le_bytes());
    for (selected_index, selected) in selected.iter().enumerate() {
        let primitive = model.geometry.get(selected.primitive).ok_or(())?;
        let (positions, normals, tangents) = posed_vertices(model, primitive, pose)?;
        out.extend_from_slice(
            &u32::try_from(selected.primitive)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(selected.material)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(positions.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(&[u8::from(selected_index >= opaque_count), 0, 0, 0]);
        for ((position, normal), tangent) in positions.iter().zip(&normals).zip(&tangents) {
            for value in position.iter().chain(normal).chain(tangent) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    out.extend_from_slice(
        &u32::try_from(pose.attachments.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for attachment in &pose.attachments {
        pbytes(out, &attachment.name)?;
        out.extend_from_slice(&[u8::from(attachment.world_aligned), 0, 0, 0]);
        let matrix = if let Some((eye, orientation)) = request.fire_view {
            let state = view.ok_or(())?;
            playsrc_studio_model::position_viewmodel_attachment(
                attachment.model_transform,
                playsrc_studio_model::Vector3(
                    eye.map(|value| playsrc_studio_model::Float32(value.to_bits())),
                ),
                orientation.map(|value| playsrc_studio_model::Float32(value.to_bits())),
                state.transform,
                playsrc_studio_model::Float32(75.0_f32.to_bits()),
                state.pass.projection.unscaled_horizontal_fov_4_by_3,
            )
            .map_err(|_| ())?
        } else {
            attachment.model_transform
        };
        for value in matrix.0 {
            out.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct WorldFrustumPlane {
    normal: [f32; 3],
    distance: f32,
}

fn world_frustum(
    origin: [f32; 3],
    yaw: f32,
    pitch: f32,
    vertical_fov: f32,
    aspect: f32,
) -> [WorldFrustumPlane; 4] {
    let (yaw_sine, yaw_cosine) = yaw.to_radians().sin_cos();
    let (pitch_sine, pitch_cosine) = pitch.to_radians().sin_cos();
    let forward = [
        pitch_cosine * yaw_cosine,
        pitch_cosine * yaw_sine,
        -pitch_sine,
    ];
    let right = [yaw_sine, -yaw_cosine, 0.0];
    let up = [pitch_sine * yaw_cosine, pitch_sine * yaw_sine, pitch_cosine];
    let vertical_tangent = (vertical_fov.to_radians() * 0.5).tan();
    let horizontal_tangent = vertical_tangent * aspect;
    let plane = |axis: [f32; 3], tangent: f32| {
        let mut normal = [
            axis[0] + tangent * forward[0],
            axis[1] + tangent * forward[1],
            axis[2] + tangent * forward[2],
        ];
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        for value in &mut normal {
            *value /= length;
        }
        WorldFrustumPlane {
            normal,
            distance: normal[0] * origin[0] + normal[1] * origin[1] + normal[2] * origin[2],
        }
    };
    [
        plane([-right[0], -right[1], -right[2]], horizontal_tangent),
        plane(right, horizontal_tangent),
        plane([-up[0], -up[1], -up[2]], vertical_tangent),
        plane(up, vertical_tangent),
    ]
}

fn cull_world_bounds(
    minimum: [i16; 3],
    maximum: [i16; 3],
    planes: &[WorldFrustumPlane; 4],
    mask: &mut u8,
) -> bool {
    let center = [
        (f32::from(minimum[0]) + f32::from(maximum[0])) * 0.5,
        (f32::from(minimum[1]) + f32::from(maximum[1])) * 0.5,
        (f32::from(minimum[2]) + f32::from(maximum[2])) * 0.5,
    ];
    let half = [
        f32::from(maximum[0]) - center[0],
        f32::from(maximum[1]) - center[1],
        f32::from(maximum[2]) - center[2],
    ];
    let mut next = 0;
    for (index, plane) in planes.iter().enumerate() {
        let bit = 1 << index;
        if *mask & bit == 0 {
            continue;
        }
        let center_distance =
            plane.normal[0] * center[0] + plane.normal[1] * center[1] + plane.normal[2] * center[2]
                - plane.distance;
        let radius = plane.normal[0].abs() * half[0]
            + plane.normal[1].abs() * half[1]
            + plane.normal[2].abs() * half[2];
        if center_distance + radius < 0.0 {
            return true;
        }
        if center_distance - radius < 0.0 {
            next |= bit;
        }
    }
    *mask = next;
    false
}

fn world_node_cull_modes(world: &playsrc_visibility::World) -> Vec<i8> {
    let mut modes = vec![-1_i8; world.nodes.len()];
    for index in 0..world.nodes.len() {
        if modes[index] != -1 {
            continue;
        }
        let node = &world.nodes[index];
        let half = [
            (f32::from(node.maxs[0]) - f32::from(node.mins[0])) * 0.5,
            (f32::from(node.maxs[1]) - f32::from(node.mins[1])) * 0.5,
            (f32::from(node.maxs[2]) - f32::from(node.mins[2])) * 0.5,
        ];
        if half.iter().all(|value| *value <= 50.0) {
            let mut stack = Vec::from(node.children);
            while let Some(child) = stack.pop() {
                let Ok(child) = usize::try_from(child) else {
                    continue;
                };
                modes[child] = -2;
                stack.extend_from_slice(&world.nodes[child].children);
            }
        } else {
            for root in node.children {
                let mut stack = vec![root];
                while let Some(child) = stack.pop() {
                    let Ok(child) = usize::try_from(child) else {
                        continue;
                    };
                    let candidate = &world.nodes[child];
                    let candidate_half = [
                        (f32::from(candidate.maxs[0]) - f32::from(candidate.mins[0])) * 0.5,
                        (f32::from(candidate.maxs[1]) - f32::from(candidate.mins[1])) * 0.5,
                        (f32::from(candidate.maxs[2]) - f32::from(candidate.mins[2])) * 0.5,
                    ];
                    if (0..3).all(|axis| half[axis] - candidate_half[axis] < 5.0) {
                        modes[child] = -3;
                        stack.extend_from_slice(&candidate.children);
                    }
                }
            }
        }
    }
    modes
}

fn frustum_world_surfaces(
    world: &playsrc_visibility::World,
    allowed_leaves: &[usize],
    origin: [f32; 3],
    yaw: f32,
    pitch: f32,
    vertical_fov: f32,
    aspect: f32,
) -> Result<Vec<u16>, ()> {
    const SUPPRESS: u8 = u8::MAX;
    let planes = world_frustum(origin, yaw, pitch, vertical_fov, aspect);
    let modes = world_node_cull_modes(world);
    let allowed = allowed_leaves
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut leaves = Vec::new();
    let mut stack = vec![(world.models.first().ok_or(())?.head_node, 0b1111_u8)];
    while let Some((child, mut mask)) = stack.pop() {
        if child < 0 {
            let leaf = (-1_i64 - i64::from(child)) as usize;
            let record = world.leaves.get(leaf).ok_or(())?;
            if record.contents == 1 || !allowed.contains(&leaf) {
                continue;
            }
            if mask != SUPPRESS && cull_world_bounds(record.mins, record.maxs, &planes, &mut mask) {
                continue;
            }
            leaves.push(leaf);
            continue;
        }
        let index = child as usize;
        let node = world.nodes.get(index).ok_or(())?;
        if mask != SUPPRESS {
            if modes[index] == -1 {
                if cull_world_bounds(node.mins, node.maxs, &planes, &mut mask) {
                    continue;
                }
            } else if modes[index] == -2 {
                mask = SUPPRESS;
            }
        }
        let plane = world.planes.get(node.plane_index as usize).ok_or(())?;
        let distance =
            plane.normal[0] * origin[0] + plane.normal[1] * origin[1] + plane.normal[2] * origin[2]
                - plane.distance;
        let near = usize::from(distance < 0.0);
        stack.push((node.children[1 - near], mask));
        stack.push((node.children[near], mask));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut surfaces = Vec::new();
    for leaf in leaves {
        let record = &world.leaves[leaf];
        let start = usize::from(record.first_leaf_face);
        let end = start + usize::from(record.leaf_face_count);
        for face in &world.leaf_faces[start..end] {
            if seen.insert(*face) {
                surfaces.push(*face);
            }
        }
        for face in &world.leaf_displacements[leaf] {
            if seen.insert(*face) {
                surfaces.push(*face);
            }
        }
    }
    Ok(surfaces)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify fourteen readable finite f32 values.
pub unsafe extern "C" fn playsrc_visibility_query(handle: u32, pointer: *const f32) -> u32 {
    if pointer.is_null() {
        return 0;
    }
    let input = unsafe { std::slice::from_raw_parts(pointer, 14) };
    if input.iter().any(|v| !v.is_finite())
        || input[8] <= 0.0
        || input[8] >= 180.0
        || input[9] <= 0.0
        || input[10] <= 0.0
        || input[11] <= input[10]
        || input[12] < 0.0
        || input[13] < -1.0
        || input[13] > 511.0
        || input[13].fract() != 0.0
    {
        return 0;
    }
    let visibility_position = [input[0], input[1], input[2]];
    let position = [input[3], input[4], input[5]];
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    let (Some(world), Some(area), Some(environment)) = (
        slot.visibility.as_ref(),
        slot.area_state.as_ref(),
        slot.environment.as_ref(),
    ) else {
        return 0;
    };
    let Ok(candidates) = playsrc_visibility::CandidateSet::compile(world, 0, &[]) else {
        return 0;
    };
    let Ok(view) = world.view(
        area,
        &candidates,
        &playsrc_visibility::ViewQuery {
            origins: vec![visibility_position],
            bypass_pvs: false,
        },
    ) else {
        return 0;
    };
    let area_filter = (input[13] >= 0.0).then_some(input[13] as u16);
    let qualified_leaves = view
        .leaves
        .iter()
        .copied()
        .filter(|leaf| {
            area_filter.is_none_or(|area| world.leaves[*leaf].area_and_flags & 0x01ff == area)
        })
        .collect::<Vec<_>>();
    let Ok(world_surfaces) = frustum_world_surfaces(
        world,
        &qualified_leaves,
        position,
        input[6],
        input[7],
        input[8],
        input[9],
    ) else {
        return 0;
    };
    let mut view_identity = Sha256::new();
    view_identity.update(view.cache_identity);
    for value in input {
        view_identity.update(value.to_bits().to_le_bytes());
    }
    let view_identity: [u8; 32] = view_identity.finalize().into();
    let policy = playsrc_map::WaterViewPolicy {
        draw_water: true,
        expensive_supported: true,
        draw_reflection: true,
        draw_refraction: true,
        force_expensive: false,
        force_reflect_entities: false,
        fast_clipping: false,
        height_clipping: true,
        eye_water_epsilon: 10.0,
    };
    let base_water_input = playsrc_map::WaterViewInput {
        origin: position,
        angles: [input[7], input[6], 0.0],
        eye_leaf: *view.origin_leaves.first().unwrap_or(&usize::MAX),
        qualified_visible_leaves: &view.leaves,
        near_plane_intersects_selected_volume: false,
        draw_sky_2d: view.sky == playsrc_visibility::SkyVisibility::Sky2d,
        policy,
    };
    let Ok(preliminary) = environment.world.water.plan_view(world, base_water_input) else {
        return 0;
    };
    let intersects = preliminary.visible_water.as_ref().is_some_and(|water| {
        near_plane_intersects_water(
            world,
            &environment.world,
            water.volume,
            water.surface_z,
            position,
            input[6],
            input[7],
            input[8],
            input[9],
            input[10],
        )
    });
    let Ok(water_plan) = environment.world.water.plan_view(
        world,
        playsrc_map::WaterViewInput {
            near_plane_intersects_selected_volume: intersects,
            ..base_water_input
        },
    ) else {
        return 0;
    };
    let evaluated_water = water_plan.visible_water.as_ref().and_then(|water| {
        let identity = match &water.material {
            playsrc_map::WaterMaterialIdentity::Map(index) => {
                environment.map_materials.get(index)?.clone()
            }
            playsrc_map::WaterMaterialIdentity::Dependency(identity) => {
                identity.to_ascii_lowercase()
            }
        };
        let material = environment.water_materials.get(&identity)?;
        let frame_count = environment.normal_frame_counts.get(&identity).copied();
        let context = playsrc_material::ProxyEvaluationContext {
            time: input[12],
            frame_time: 0.015,
            water_lod: environment.water_lod,
            texture_frames: frame_count
                .map(|count| BTreeMap::from([(b"$normalmap".to_vec(), count)]))
                .unwrap_or_default(),
            model_inputs: playsrc_material::ModelProxyInputs::default(),
        };
        playsrc_material::evaluate_water_material(material, &context)
            .ok()
            .map(|state| (identity, state))
    });
    if water_plan.visible_water.is_some() != evaluated_water.is_some() {
        return 0;
    }
    let mut visible_surfaces = std::collections::BTreeSet::new();
    let mut visible_areas = std::collections::BTreeSet::new();
    for leaf in &qualified_leaves {
        let record = &world.leaves[*leaf];
        visible_areas.insert(usize::from(record.area_and_flags & 0x01ff));
        let start = usize::from(record.first_leaf_face);
        let end = start + usize::from(record.leaf_face_count);
        visible_surfaces.extend(world.leaf_faces[start..end].iter().copied());
        visible_surfaces.extend(world.leaf_displacements[*leaf].iter().copied());
    }
    let mut output = b"PVIS".to_vec();
    output.extend_from_slice(&5u32.to_le_bytes());
    output.extend_from_slice(&view_identity);
    output.extend_from_slice(&world.identity);
    output.extend_from_slice(&[u8::from(view.outside_world), view.sky as u8, 0, 0]);
    output.extend_from_slice(&(visible_surfaces.len() as u32).to_le_bytes());
    for face in &visible_surfaces {
        output.extend_from_slice(&u32::from(*face).to_le_bytes());
    }
    output.extend_from_slice(&(world_surfaces.len() as u32).to_le_bytes());
    for face in &world_surfaces {
        output.extend_from_slice(&u32::from(*face).to_le_bytes());
    }
    output.extend_from_slice(
        &view
            .origin_leaves
            .first()
            .and_then(|value| u32::try_from(*value).ok())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    output.extend_from_slice(
        &u32::try_from(qualified_leaves.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for leaf in &qualified_leaves {
        output.extend_from_slice(&u32::try_from(*leaf).unwrap_or(u32::MAX).to_le_bytes());
    }
    output.extend_from_slice(
        &u32::try_from(visible_areas.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for area in &visible_areas {
        output.extend_from_slice(&u32::try_from(*area).unwrap_or(u32::MAX).to_le_bytes());
    }
    output.extend_from_slice(&[
        u8::from(water_plan.visible_water.is_some()),
        u8::from(water_plan.render.cheap),
        u8::from(water_plan.render.reflect),
        u8::from(water_plan.render.refract),
        u8::from(water_plan.render.reflect_entities),
        u8::from(water_plan.render.draw_surface),
        u8::from(water_plan.render.opaque),
        u8::from(intersects),
    ]);
    if let (Some(water), Some((identity, evaluated))) = (&water_plan.visible_water, evaluated_water)
    {
        output.extend_from_slice(
            &u32::try_from(water.volume)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(water.visible_leaf)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(water.eye_leaf)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(&[
            u8::from(water.eye_in_volume),
            u8::from(water.translucent),
            0,
            0,
        ]);
        output.extend_from_slice(&water.surface_z.to_le_bytes());
        output.extend_from_slice(
            &u32::from(water.distance_to_water.unwrap_or(u16::MAX)).to_le_bytes(),
        );
        if pbytes(&mut output, identity.as_bytes()).is_err() {
            return 0;
        }
        output.extend_from_slice(&evaluated.normal_frame.to_le_bytes());
        for value in evaluated.normal_transform {
            output.extend_from_slice(&value.to_le_bytes());
        }
        output.extend_from_slice(&evaluated.cheap_start.to_le_bytes());
        output.extend_from_slice(&evaluated.cheap_end.to_le_bytes());
    }
    output.extend_from_slice(
        &u32::try_from(water_plan.passes.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for pass in &water_plan.passes {
        output.extend_from_slice(&[
            match pass.kind {
                playsrc_map::EnvironmentViewKind::Reflection => 0,
                playsrc_map::EnvironmentViewKind::Refraction => 1,
                playsrc_map::EnvironmentViewKind::Main => 2,
                playsrc_map::EnvironmentViewKind::Intersection => 3,
            },
            u8::from(pass.render_above_water),
            u8::from(pass.render_under_water),
            u8::from(pass.render_water_surface),
            u8::from(pass.draw_entities),
            u8::from(pass.draw_sky_2d),
            u8::from(pass.clip.is_some()),
            match pass.clip.map(|value| value.keep) {
                None => 0,
                Some(playsrc_map::WaterClipKeep::Above) => 1,
                Some(playsrc_map::WaterClipKeep::Below) => 2,
            },
        ]);
        for value in pass.origin.into_iter().chain(pass.angles) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        output.extend_from_slice(&pass.clip.map_or(0.0, |value| value.height).to_le_bytes());
        output.extend_from_slice(
            &pass
                .forced_visibility_leaf
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        match pass.fog {
            playsrc_map::ViewFog::World => output.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]),
            playsrc_map::ViewFog::Water { volume, height_fog } => {
                output.extend_from_slice(&[1, u8::from(height_fog), 0, 0]);
                output.extend_from_slice(&u32::try_from(volume).unwrap_or(u32::MAX).to_le_bytes());
            }
        }
        let pass_origin = pass
            .forced_visibility_leaf
            .and_then(|leaf| {
                world.leaves.get(leaf).map(|record| {
                    [
                        (f32::from(record.mins[0]) + f32::from(record.maxs[0])) * 0.5,
                        (f32::from(record.mins[1]) + f32::from(record.maxs[1])) * 0.5,
                        (f32::from(record.mins[2]) + f32::from(record.maxs[2])) * 0.5,
                    ]
                })
            })
            .unwrap_or(pass.origin);
        let Ok(pass_view) = world.view(
            area,
            &candidates,
            &playsrc_visibility::ViewQuery {
                origins: vec![pass_origin],
                bypass_pvs: false,
            },
        ) else {
            return 0;
        };
        output.extend_from_slice(
            &u32::try_from(pass_view.world_surfaces.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for face in pass_view.world_surfaces {
            output.extend_from_slice(&u32::from(face).to_le_bytes());
        }
    }
    output.extend_from_slice(
        &u32::try_from(environment.world_materials.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for (identity, material) in &environment.world_materials {
        let context = playsrc_material::ProxyEvaluationContext {
            time: input[12],
            frame_time: 0.015,
            water_lod: environment.water_lod,
            texture_frames: material.texture_frames.clone(),
            model_inputs: playsrc_material::ModelProxyInputs::default(),
        };
        let Ok(evaluated) = playsrc_material::evaluate_world_material(&material.material, &context)
        else {
            return 0;
        };
        if pbytes(&mut output, identity.as_bytes()).is_err() {
            return 0;
        }
        output.extend_from_slice(
            &u32::try_from(material.map_material)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(evaluated.textures.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for texture in evaluated.textures {
            output.extend_from_slice(&[
                texture.role as u8,
                u8::from(texture.frame.is_some()),
                u8::from(texture.transform.is_some()),
                0,
            ]);
            output.extend_from_slice(&texture.frame.unwrap_or(0).to_le_bytes());
            for value in texture.transform.unwrap_or([0.0; 16]) {
                output.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    slot.visibility_output = output;
    1
}

#[allow(clippy::too_many_arguments)]
fn near_plane_intersects_water(
    visibility: &playsrc_visibility::World,
    environment: &playsrc_map::WorldEnvironment,
    volume: usize,
    water_z: f32,
    origin: [f32; 3],
    yaw_degrees: f32,
    pitch_degrees: f32,
    vertical_fov_degrees: f32,
    aspect_ratio: f32,
    near: f32,
) -> bool {
    let yaw = yaw_degrees.to_radians();
    let pitch = pitch_degrees.to_radians();
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let forward = [cp * cy, cp * sy, -sp];
    let right = [sy, -cy, 0.0];
    let up = [sp * cy, sp * sy, cp];
    let half_height = near * (vertical_fov_degrees.to_radians() * 0.5).tan();
    let half_width = half_height * aspect_ratio;
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut above = false;
    let mut below = false;
    for right_sign in [-1.0, 1.0] {
        for up_sign in [-1.0, 1.0] {
            let mut point = origin;
            for axis in 0..3 {
                point[axis] += forward[axis] * near
                    + right[axis] * half_width * right_sign
                    + up[axis] * half_height * up_sign;
                minimum[axis] = minimum[axis].min(point[axis]);
                maximum[axis] = maximum[axis].max(point[axis]);
            }
            above |= point[2] + 7.0 > water_z;
            below |= point[2] - 7.0 < water_z;
        }
    }
    if !above || !below {
        return false;
    }
    for axis in 0..3 {
        minimum[axis] -= 7.0;
        maximum[axis] += 7.0;
    }
    let Some(volume) = environment
        .water
        .volumes
        .iter()
        .find(|value| value.index == volume)
    else {
        return false;
    };
    volume.leaves.iter().any(|leaf| {
        visibility.leaves.get(*leaf).is_some_and(|record| {
            (0..3).all(|axis| {
                f32::from(record.maxs[axis]) >= minimum[axis]
                    && f32::from(record.mins[axis]) <= maximum[axis]
            })
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_visibility_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.visibility_output.len()).unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_visibility_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.visibility_output.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                slot.visibility_output.as_ptr(),
                pointer,
                slot.visibility_output.len(),
            )
        };
        slot.visibility_output.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_result_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        let Some(payload) = &slot.payload else {
            return 0;
        };
        if capacity < payload.len() {
            return 0;
        }
        unsafe { std::ptr::copy_nonoverlapping(payload.as_ptr(), pointer, payload.len()) };
        payload.len()
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify at least 32 writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_result_hash(handle: u32, pointer: *mut u8) -> u32 {
    with(handle, |slot| {
        unsafe { std::ptr::copy_nonoverlapping(slot.hash.as_ptr(), pointer, 32) };
        1
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify at least 32 writable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_result_derived_hash(handle: u32, pointer: *mut u8) -> u32 {
    with(handle, |slot| {
        unsafe { std::ptr::copy_nonoverlapping(slot.derived_hash.as_ptr(), pointer, 32) };
        1
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_spawn_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    const LENGTH: usize = 40;
    with(handle, |slot| {
        let Some(spawn) = slot.spawn else {
            return 0;
        };
        if capacity < LENGTH {
            return 0;
        }
        let mut bytes = [0_u8; LENGTH];
        bytes[0..4].copy_from_slice(b"PSIV");
        bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&spawn.entity.to_le_bytes());
        bytes[12..16].copy_from_slice(&spawn.hammer_id.to_le_bytes());
        for (index, value) in spawn.position.into_iter().chain(spawn.angles).enumerate() {
            let start = 16 + index * 4;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, LENGTH) };
        LENGTH
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_team_state_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    const LENGTH: usize = 12;
    if pointer.is_null() || capacity < LENGTH {
        return 0;
    }
    with(handle, |slot| {
        let Some(session) = slot.session.as_ref() else {
            return 0;
        };
        let snapshot = session.team_snapshot();
        let Ok(red) = u8::try_from(snapshot.red_count) else {
            return 0;
        };
        let Ok(blue) = u8::try_from(snapshot.blue_count) else {
            return 0;
        };
        let mut bytes = [0_u8; LENGTH];
        bytes[..4].copy_from_slice(b"PTEM");
        bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
        bytes[8] = snapshot.local_team as u8;
        bytes[9] = red;
        bytes[10] = blue;
        bytes[11] = snapshot.wire_flags();
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, LENGTH) };
        LENGTH
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_team_select(handle: u32, choice: u32) -> u32 {
    let selected = match choice {
        1 => playsrc_tf2::team_selection::TeamChoice::Spectator,
        2 => playsrc_tf2::team_selection::TeamChoice::Red,
        3 => playsrc_tf2::team_selection::TeamChoice::Blue,
        4 => playsrc_tf2::team_selection::TeamChoice::Auto,
        _ => return 0,
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation || slot.payload.is_none() {
        return 0;
    }
    let Some(session) = slot.session.as_mut() else {
        return 0;
    };
    u32::from(session.select_team_choice(selected).is_ok())
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify one complete version-4 gameplay command in this module's memory.
pub unsafe extern "C" fn playsrc_simulation_observe(
    handle: u32,
    now_seconds: f64,
    pointer: *const u8,
    length: usize,
    suspended: u32,
) -> u32 {
    SIMULATION_ERROR.store(0, Ordering::Relaxed);
    SIMULATION_ERROR_DETAIL
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .expect("Simulation error detail")
        .clear();
    if !now_seconds.is_finite()
        || pointer.is_null()
        || length == 0
        || length > 64 * 1024
        || suspended > 1
        || with(handle, |_| ()).is_none()
    {
        return 0;
    }
    let command = unsafe { std::slice::from_raw_parts(pointer, length) };
    if gameplay_protocol::decode(command).is_none() {
        return 0;
    }
    let mut hosts = simulation_hosts().lock().expect("TF2 Simulation hosts");
    if let std::collections::btree_map::Entry::Vacant(entry) = hosts.entry(handle) {
        let Some(configuration) = simulation_configuration() else {
            return 0;
        };
        entry.insert(SimulationHostEntry {
            host: playsrc_simulation::FixedStepHost::with_metrics_clock(
                configuration,
                Tf2Simulation {
                    handle,
                    current_command: None,
                },
                RuntimeMetricsClock::new(),
            ),
            output: Vec::new(),
        });
    }
    let entry = hosts.get_mut(&handle).expect("inserted Simulation host");
    if entry.host.submit(command).is_err() {
        SIMULATION_ERROR.store(3, Ordering::Relaxed);
        return 0;
    }
    let should_suspend = suspended == 1;
    if entry.host.status().suspended != should_suspend
        && entry
            .host
            .set_suspended(should_suspend, now_seconds)
            .is_err()
    {
        SIMULATION_ERROR.store(4, Ordering::Relaxed);
        return 0;
    }
    if let Err(error) = entry.host.observe(now_seconds) {
        *SIMULATION_ERROR_DETAIL
            .get_or_init(|| Mutex::new(String::new()))
            .lock()
            .expect("Simulation error detail") = error.to_string();
        let code = match error {
            playsrc_simulation::HostError::Faulted(fault) => {
                100 + match fault.code {
                    playsrc_simulation::FaultCode::PendingFrameLimit => 1,
                    playsrc_simulation::FaultCode::HostFrameOverflow => 2,
                    playsrc_simulation::FaultCode::HostTickOverflow => 3,
                    playsrc_simulation::FaultCode::Simulation => 4,
                    playsrc_simulation::FaultCode::EmptySnapshot => 5,
                    playsrc_simulation::FaultCode::SnapshotLimit => 6,
                    playsrc_simulation::FaultCode::EventLimit => 7,
                    playsrc_simulation::FaultCode::QueueArithmeticOverflow => 8,
                    playsrc_simulation::FaultCode::Shutdown => 9,
                }
            }
            _ => 5,
        };
        SIMULATION_ERROR.store(code, Ordering::Relaxed);
        return 0;
    }
    let Some(output) = encode_simulation_publications(&entry.host.drain_publications()) else {
        return 0;
    };
    entry.output = output;
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_simulation_error() -> u32 {
    SIMULATION_ERROR.load(Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_simulation_error_length() -> usize {
    SIMULATION_ERROR_DETAIL
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .expect("Simulation error detail")
        .len()
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable memory for `capacity` bytes.
pub unsafe extern "C" fn playsrc_simulation_error_copy(pointer: *mut u8, capacity: usize) -> usize {
    let detail = SIMULATION_ERROR_DETAIL
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .expect("Simulation error detail");
    if pointer.is_null() || capacity < detail.len() {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(detail.as_ptr(), pointer, detail.len()) };
    detail.len()
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_simulation_output_length(handle: u32) -> usize {
    simulation_hosts()
        .lock()
        .expect("TF2 Simulation hosts")
        .get(&handle)
        .map_or(0, |entry| entry.output.len())
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_simulation_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    let hosts = simulation_hosts().lock().expect("TF2 Simulation hosts");
    let Some(entry) = hosts.get(&handle) else {
        return 0;
    };
    if pointer.is_null() || capacity < entry.output.len() {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(entry.output.as_ptr(), pointer, entry.output.len()) };
    entry.output.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_dispose(handle: u32) -> u32 {
    if let Some(mut entry) = simulation_hosts()
        .lock()
        .expect("TF2 Simulation hosts")
        .remove(&handle)
    {
        let _ = entry.host.shutdown();
    }
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    slot.payload = None;
    slot.presentation.clear();
    slot.particles = None;
    slot.particle_materials.clear();
    slot.particle_sheets.clear();
    slot.particle_output.clear();
    slot.studio_models.clear();
    slot.viewmodel_bob.clear();
    slot.model_output.clear();
    slot.visibility = None;
    slot.area_state = None;
    slot.visibility_output.clear();
    slot.collision = None;
    slot.gameplay_world = None;
    slot.collision_templates.clear();
    slot.collision_revision = 0;
    slot.pushers.clear();
    slot.latest_game_snapshot = None;
    slot.hash = [0; 32];
    slot.derived_hash = [0; 32];
    slot.bsp_hash = [0; 32];
    slot.error = 0;
    slot.spawn = None;
    slot.session = None;
    slot.snapshot.clear();
    1
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify one complete version-4 gameplay transaction in this module's memory.
pub unsafe extern "C" fn playsrc_game_advance(
    handle: u32,
    pointer: *const u8,
    length: usize,
    tick_count: u32,
) -> u32 {
    GAME_ADVANCE_ERROR.store(0, Ordering::Relaxed);
    macro_rules! fail {
        ($code:expr) => {{
            GAME_ADVANCE_ERROR.store($code, Ordering::Relaxed);
            return 0;
        }};
    }
    if tick_count == 0 || tick_count > 64 {
        fail!(1);
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Some(input) = gameplay_protocol::decode(bytes) else {
        fail!(2);
    };
    let Some((index, generation)) = decode(handle) else {
        fail!(3);
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        fail!(4);
    };
    if slot.generation != generation {
        fail!(5);
    }
    let Some(session) = slot.session.as_ref() else {
        fail!(6);
    };
    let mut candidate = session.clone();
    let Some(collision) = slot.collision.clone() else {
        fail!(7);
    };
    let Some(gameplay_world) = slot.gameplay_world.clone() else {
        fail!(8);
    };
    let mut collision_transaction = CollisionSnapshotTransaction::new(gameplay_world.clone());
    let templates = slot.collision_templates.clone();
    let mut collision_revision = slot.collision_revision;
    let mut pushers = slot.pushers.clone();
    let mut latest_game_snapshot = slot.latest_game_snapshot.clone();
    let mut snapshot: Option<playsrc_tf2::Snapshot> = None;
    let mut producer: Option<playsrc_tf2::ProducerSnapshot> = None;
    let mut random_draws = Vec::new();
    let mut audio_events = Vec::new();
    let mut consumed_rocket_results = Vec::new();
    let mut consumed_mover_results = Vec::new();
    let mut collision_snapshot_bytes = Vec::new();
    for index in 0..tick_count {
        pushers.retain(|request_id, _| {
            candidate
                .mover_requests()
                .iter()
                .any(|request| request.request_id == *request_id)
        });
        let pending_movers = candidate
            .mover_requests()
            .iter()
            .filter(|request| !pushers.contains_key(&request.request_id))
            .copied()
            .collect::<Vec<_>>();
        if !pending_movers.is_empty() {
            for request in pending_movers {
                let angular_speed = request
                    .angular_velocity
                    .into_iter()
                    .map(|value| value * value)
                    .sum::<f32>()
                    .sqrt();
                let input = playsrc_movement::TransformPusherRequest {
                    request_id: request.request_id,
                    identity: u64::from(request.entity),
                    start: playsrc_collision::Transform {
                        origin: request.start,
                        angles: request.start_angles,
                    },
                    destination: playsrc_collision::Transform {
                        origin: request.destination,
                        angles: request.destination_angles,
                    },
                    linear_speed: request.speed,
                    angular_speed,
                    hierarchy: Vec::new(),
                };
                let pusher = match playsrc_movement::PusherSnapshot::start_transforms(
                    collision_revision,
                    std::slice::from_ref(&input),
                    playsrc_movement::PusherLimits::default(),
                ) {
                    Ok(value) => value,
                    Err(_) => fail!(10),
                };
                pushers.insert(request.request_id, pusher);
            }
        }
        let mut transforms = BTreeMap::new();
        let mut velocities = BTreeMap::new();
        let current_revision = collision_revision.saturating_add(1);
        let current_collision = match compile_collision_snapshot(
            &collision,
            &templates,
            current_revision,
            latest_game_snapshot.as_ref(),
            &transforms,
            &velocities,
        ) {
            Ok(value) => value,
            Err(_) => fail!(11),
        };
        let mut mover_phase = playsrc_tf2::MapPhase::default();
        if !pushers.is_empty() {
            let movement = candidate.movement_state();
            let hull = candidate.last_movement_result().map_or_else(
                || {
                    movement.active_hull(
                        playsrc_tf2::MovementPolicy {
                            class: candidate.producer_snapshot().class,
                            modifiers: playsrc_tf2::MovementModifiers {
                                noclip_allowed: true,
                                ..playsrc_tf2::MovementModifiers::default()
                            },
                        }
                        .resolve(),
                    )
                },
                |result| result.selected_hull,
            );
            let subjects = [playsrc_movement::PushSubject {
                identity: u64::from(playsrc_tf2::PLAYER_IDENTITY),
                root_identity: u64::from(playsrc_tf2::PLAYER_IDENTITY),
                position: movement.position,
                hull,
                mask: playsrc_movement::Configuration::default().solid_mask,
                collision_group: 0,
                support: movement.ground.and_then(|ground| ground.support),
                pushability: playsrc_movement::Pushability::Pushable,
                solid: movement.mode == playsrc_movement::Mode::Walk,
                point_sized: false,
                volume_contents: false,
                unblockable: false,
            }];
            let next_revision = current_revision.saturating_add(1);
            let mut next_pushers = BTreeMap::new();
            for (request_id, prior) in std::mem::take(&mut pushers) {
                let frame = match playsrc_movement::advance_linear_pushers(
                    &collision,
                    &current_collision,
                    &prior,
                    next_revision,
                    &subjects,
                    playsrc_movement::Configuration::default().tick_interval,
                    |_, _| true,
                    |_, _| true,
                ) {
                    Ok(value) => value,
                    Err(_) => fail!(12),
                };
                let Some(records) = mover_records(&frame) else {
                    fail!(13);
                };
                for record in &records {
                    match candidate.apply_mover_results(std::slice::from_ref(record)) {
                        Ok(phase) => mover_phase.append(phase),
                        Err(_) => fail!(14),
                    }
                }
                for result in &frame.results {
                    transforms.insert(result.identity, result.transform);
                    velocities.insert(result.identity, result.trajectory_velocity);
                }
                consumed_mover_results.extend(records);
                if frame.next.active_count() != 0 {
                    next_pushers.insert(request_id, frame.next);
                }
            }
            collision_revision = next_revision;
            pushers = next_pushers;
        } else {
            collision_revision = current_revision;
        }
        let collision_snapshot = if transforms.is_empty() && velocities.is_empty() {
            current_collision
        } else {
            match compile_collision_snapshot(
                &collision,
                &templates,
                collision_revision,
                latest_game_snapshot.as_ref(),
                &transforms,
                &velocities,
            ) {
                Ok(value) => value,
                Err(_) => fail!(15),
            }
        };
        collision_snapshot_bytes = match collision_snapshot.snapshot_bytes() {
            Ok(value) => value,
            Err(_) => fail!(16),
        };
        let Some(rocket_results) = resolve_rocket_traces(
            &collision,
            &collision_snapshot,
            candidate.rocket_trace_requests(),
            candidate.producer_snapshot().tick,
        ) else {
            fail!(17);
        };
        consumed_rocket_results.extend_from_slice(&rocket_results);
        gameplay_world.replace_snapshot(collision_snapshot);
        let physics_results = if index == 0 {
            input.physics_results.as_slice()
        } else {
            &[]
        };
        if input.command.fire {
            let bots = candidate
                .bot_world()
                .map_or_else(Vec::new, playsrc_tf2::bot::BotWorld::snapshots);
            let hitboxes = match pose_bot_hitboxes(
                &slot.studio_models,
                &bots,
                candidate.producer_snapshot().tick,
            ) {
                Ok(value) => value,
                Err(_) => fail!(18),
            };
            candidate.set_posed_player_hitboxes(hitboxes);
        }
        match candidate.advance_with_external(input.command, physics_results, &rocket_results, None)
        {
            Ok(mut value) => {
                let mut current_producer = candidate.producer_snapshot();
                if !mover_phase.events.is_empty()
                    || !mover_phase.effects.is_empty()
                    || !mover_phase.mover_requests.is_empty()
                {
                    value
                        .entity_events
                        .splice(0..0, mover_phase.events.drain(..));
                    current_producer
                        .map_effects
                        .splice(0..0, mover_phase.effects);
                    current_producer
                        .mover_requests
                        .splice(0..0, mover_phase.mover_requests);
                }
                if let Some(previous) = snapshot.as_mut() {
                    value
                        .events
                        .splice(0..0, std::mem::take(&mut previous.events));
                    value
                        .projectile_events
                        .splice(0..0, std::mem::take(&mut previous.projectile_events));
                    value
                        .entity_events
                        .splice(0..0, std::mem::take(&mut previous.entity_events));
                    if let (Some(prior), Some(current)) =
                        (previous.jump.as_mut(), value.jump.as_mut())
                    {
                        current
                            .events
                            .splice(0..0, std::mem::take(&mut prior.events));
                        if current.result.is_none() {
                            current.result = prior.result.take();
                        }
                    }
                }
                if let Some(previous) = producer.take() {
                    merge_producer(&mut current_producer, previous);
                }
                random_draws.extend_from_slice(candidate.random_draws());
                audio_events.extend_from_slice(candidate.audio_events());
                latest_game_snapshot = Some(value.clone());
                producer = Some(current_producer);
                snapshot = Some(value);
            }
            Err(error) => fail!(gameplay_error_code(&error)),
        }
    }
    let snapshot = snapshot.expect("positive tick count");
    let producer = producer.expect("positive tick count");
    let entity_presentation =
        match candidate.entity_presentation(playsrc_tf2::PresentationRevision {
            entity: candidate.entity_revision(),
            collision: collision_revision,
        }) {
            Ok(value) => value,
            Err(_) => fail!(19),
        };
    let Some(encoded) = encode_snapshot(
        &snapshot,
        &producer,
        candidate.respawn_touch_count(),
        candidate.last_movement_result(),
        SnapshotExtensions {
            random_state: candidate.random_state(),
            random_draws: &random_draws,
            audio_events: &audio_events,
            rocket_results: &consumed_rocket_results,
            mover_results: &consumed_mover_results,
            collision_snapshot: &collision_snapshot_bytes,
            entity_presentation: &entity_presentation,
            payload_constraint_blocked: candidate.payload_constraint_blocked(),
        },
    ) else {
        fail!(20);
    };
    slot.session = Some(candidate);
    slot.pushers = pushers;
    slot.latest_game_snapshot = latest_game_snapshot;
    slot.collision_revision = collision_revision;
    slot.snapshot = encoded;
    slot.error = 0;
    collision_transaction.committed = true;
    1
}

fn gameplay_error_code(error: &playsrc_tf2::Error) -> u32 {
    1800 + match error {
        playsrc_tf2::Error::Movement(_) => 1,
        playsrc_tf2::Error::Entity(_) => 2,
        playsrc_tf2::Error::Jump(_) => 3,
        playsrc_tf2::Error::MissingEntity(_) => 4,
        playsrc_tf2::Error::InvalidCourseTrigger(_) => 5,
        playsrc_tf2::Error::ProjectileLimit => 6,
        playsrc_tf2::Error::InvalidStickyLaunchRandom => 7,
        playsrc_tf2::Error::InvalidProjectilePhysics => 8,
        playsrc_tf2::Error::InvalidPlayerPosition => 13,
        playsrc_tf2::Error::Random(_) => 9,
        playsrc_tf2::Error::UnsupportedJumpClass(_) => 10,
        playsrc_tf2::Error::Bot(_) => 11,
        playsrc_tf2::Error::TeamSelection(_) => 12,
        playsrc_tf2::Error::Objectives(_) => 13,
        playsrc_tf2::Error::Round(_) => 14,
    }
}

fn mover_records(frame: &playsrc_movement::PusherFrame) -> Option<Vec<playsrc_tf2::MoverResult>> {
    let mut output = Vec::new();
    for contact in &frame.contacts {
        let result = frame
            .results
            .iter()
            .find(|result| result.identity == contact.pusher)?;
        output.push(playsrc_tf2::MoverResult {
            request_id: result.request_id,
            entity: u32::try_from(result.identity).ok()?,
            kind: match contact.kind {
                playsrc_movement::BlockContactKind::End => playsrc_tf2::MoverResultKind::BlockedEnd,
                playsrc_movement::BlockContactKind::Start => {
                    playsrc_tf2::MoverResultKind::BlockedStart
                }
                playsrc_movement::BlockContactKind::Stay => {
                    playsrc_tf2::MoverResultKind::BlockedStay
                }
            },
            transform: playsrc_entity::Transform {
                origin: result.transform.origin,
                angles: result.transform.angles,
            },
            carry: [0.0; 3],
        });
    }
    for result in &frame.results {
        let kind = match result.status {
            playsrc_movement::PusherStatus::Progress => playsrc_tf2::MoverResultKind::Progress,
            playsrc_movement::PusherStatus::Completed => playsrc_tf2::MoverResultKind::Completed,
            playsrc_movement::PusherStatus::Blocked => continue,
        };
        let carry = result
            .subject_moves
            .iter()
            .find(|moved| moved.subject == u64::from(playsrc_tf2::PLAYER_IDENTITY))
            .map_or([0.0; 3], |moved| moved.displacement);
        output.push(playsrc_tf2::MoverResult {
            request_id: result.request_id,
            entity: u32::try_from(result.identity).ok()?,
            kind,
            transform: playsrc_entity::Transform {
                origin: result.transform.origin,
                angles: result.transform.angles,
            },
            carry,
        });
    }
    Some(output)
}

fn resolve_rocket_traces(
    world: &playsrc_collision::World,
    snapshot: &playsrc_collision::Snapshot,
    requests: &[playsrc_tf2::RocketTraceRequest],
    result_tick: u64,
) -> Option<Vec<playsrc_tf2::RocketTraceResult>> {
    requests
        .iter()
        .map(|request| {
            let trace = world
                .trace_snapshot_ray(
                    snapshot,
                    playsrc_collision::SnapshotRayRequest {
                        start: request.start,
                        end: request.end,
                        mask: request.mask,
                        scope: playsrc_collision::TraceScope::Everything,
                        ignored: &[u64::from(playsrc_tf2::PLAYER_IDENTITY)],
                    },
                    |_| true,
                )
                .ok()?;
            let solid = trace.did_hit();
            let sky = solid && trace.is_sky();
            Some(playsrc_tf2::RocketTraceResult {
                projectile: request.projectile,
                tick: result_tick,
                end: trace.end,
                solid,
                sky,
                normal: if solid && !sky {
                    Some(trace.plane?.normal)
                } else {
                    None
                },
                direct_target: trace
                    .entity_identity()
                    .and_then(|identity| identity.try_into().ok()),
            })
        })
        .collect()
}

fn merge_producer(
    current: &mut playsrc_tf2::ProducerSnapshot,
    mut previous: playsrc_tf2::ProducerSnapshot,
) {
    current
        .activities
        .splice(0..0, previous.activities.drain(..));
    current
        .lifecycle_events
        .splice(0..0, previous.lifecycle_events.drain(..));
    current
        .physics_requests
        .splice(0..0, previous.physics_requests.drain(..));
    current
        .rocket_trace_requests
        .splice(0..0, previous.rocket_trace_requests.drain(..));
    current
        .radius_damage_requests
        .splice(0..0, previous.radius_damage_requests.drain(..));
    current
        .mover_requests
        .splice(0..0, previous.mover_requests.drain(..));
    current
        .contact_reconcile_requests
        .splice(0..0, previous.contact_reconcile_requests.drain(..));
    current
        .map_effects
        .splice(0..0, previous.map_effects.drain(..));
    current
        .regenerate_animation_events
        .splice(0..0, previous.regenerate_animation_events.drain(..));
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify exactly `length` readable course-definition bytes.
pub unsafe extern "C" fn playsrc_jump_configure(
    handle: u32,
    pointer: *const u8,
    length: usize,
) -> u32 {
    if !(52..=64 * 1024).contains(&length) {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Ok(definition) =
        playsrc_tf2::jump::decode_course(bytes, playsrc_tf2::jump::Limits::default())
    else {
        return 0;
    };
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation || definition.map_identity != slot.bsp_hash {
        return 0;
    }
    let Some(session) = slot.session.as_mut() else {
        return 0;
    };
    let mut candidate = session.clone();
    if candidate.configure_jump(definition).is_err() {
        return 0;
    }
    *session = candidate;
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_player_set_position(handle: u32, x: f32, y: f32, z: f32) -> u32 {
    let Some((index, generation)) = decode(handle) else {
        return 0;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation {
        return 0;
    }
    let Some(session) = slot.session.as_mut() else {
        return 0;
    };
    u32::from(session.set_position([x, y, z]).is_ok())
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_snapshot_length(handle: u32) -> usize {
    with(handle, |slot| slot.snapshot.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_teleport_count(handle: u32) -> usize {
    with(handle, |slot| {
        slot.session
            .as_ref()
            .map_or(0, |session| session.map_counts().teleport_triggers as usize)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_teleport_destination_count(handle: u32) -> usize {
    with(handle, |slot| {
        slot.session.as_ref().map_or(0, |session| {
            session.map_counts().teleport_destinations as usize
        })
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_runtime_count(handle: u32, kind: u32) -> usize {
    with(handle, |slot| {
        let Some(session) = slot.session.as_ref() else {
            return 0;
        };
        let counts = session.map_counts();
        match kind {
            1 => counts.buttons as usize,
            2 => counts.doors as usize,
            3 => counts.linear_movers as usize,
            4 => counts.multiple_triggers as usize,
            5 => counts.hurt_triggers as usize,
            6 => counts.push_triggers as usize,
            7 => counts.catapult_triggers as usize,
            8 => counts.teleport_triggers as usize,
            9 => counts.regenerate_zones as usize,
            10 => counts.teleport_destinations as usize,
            _ => 0,
        }
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_snapshot_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    with(handle, |slot| {
        if capacity < slot.snapshot.len() {
            return 0;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(slot.snapshot.as_ptr(), pointer, slot.snapshot.len())
        };
        slot.snapshot.len()
    })
    .unwrap_or(0)
}

struct SnapshotExtensions<'a> {
    random_state: playsrc_tf2::Tf2RandomState,
    random_draws: &'a [playsrc_tf2::RandomDraw],
    audio_events: &'a [playsrc_tf2::AudioEvent],
    rocket_results: &'a [playsrc_tf2::RocketTraceResult],
    mover_results: &'a [playsrc_tf2::MoverResult],
    collision_snapshot: &'a [u8],
    entity_presentation: &'a playsrc_tf2::EntityPresentationSnapshot,
    payload_constraint_blocked: bool,
}

fn encode_snapshot(
    snapshot: &playsrc_tf2::Snapshot,
    producer: &playsrc_tf2::ProducerSnapshot,
    respawn_touch_count: u32,
    movement_tick: Option<&playsrc_movement::StepResult>,
    extensions: SnapshotExtensions<'_>,
) -> Option<Vec<u8>> {
    const MAX: usize = 64 * 1024 * 1024;
    let movement = snapshot.movement.snapshot_bytes();
    let jump = match snapshot.jump.as_ref() {
        Some(value) => encode_jump(value)?,
        None => Vec::new(),
    };
    let random_state = encode_random_state(extensions.random_state)?;
    let entity_presentation = encode_entity_presentation(extensions.entity_presentation)?;
    let mut movement_tick_bytes = Vec::new();
    encode_movement_tick(&mut movement_tick_bytes, movement_tick, MAX)?;
    let mut out = Vec::new();
    extend(&mut out, b"PSSN", MAX)?;
    u32_field(&mut out, 16, MAX)?;
    u64_field(&mut out, snapshot.tick, MAX)?;
    extend(
        &mut out,
        &[
            class_code(snapshot.class),
            team_code(snapshot.team),
            snapshot.weapon.map_or(0, weapon_code),
            snapshot.movement.mode as u8,
        ],
        MAX,
    )?;
    f32_field(&mut out, snapshot.health, MAX)?;
    f32_field(&mut out, snapshot.maximum_health, MAX)?;
    extend(
        &mut out,
        &[
            match producer.lifecycle {
                playsrc_tf2::PlayerLifecycle::Active => 1,
                playsrc_tf2::PlayerLifecycle::Dying => 2,
                playsrc_tf2::PlayerLifecycle::Welcome => 3,
                playsrc_tf2::PlayerLifecycle::Observer => 4,
            },
            snapshot
                .spy
                .and_then(|state| state.disguise)
                .map_or(0, |value| class_code(value.class)),
            snapshot
                .spy
                .and_then(|state| state.disguise)
                .map_or(0, |value| team_code(value.team)),
            snapshot
                .spy
                .and_then(|state| state.desired_disguise)
                .map_or(0, |value| {
                    class_code(value.class) | (team_code(value.team) << 4)
                }),
        ],
        MAX,
    )?;
    for condition in producer.conditions {
        u32_field(&mut out, condition, MAX)?;
    }
    u32_field(&mut out, respawn_touch_count, MAX)?;
    u32_field(&mut out, u32::try_from(producer.weapons.len()).ok()?, MAX)?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.projectiles.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.projectile_events.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.entity_transforms.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(snapshot.entity_events.len()).ok()?,
        MAX,
    )?;
    u32_field(&mut out, u32::try_from(snapshot.events.len()).ok()?, MAX)?;
    u32_field(&mut out, u32::try_from(jump.len()).ok()?, MAX)?;
    u32_field(&mut out, u32::try_from(movement.len()).ok()?, MAX)?;
    for count in [
        producer.activities.len(),
        producer.lifecycle_events.len(),
        producer.physics_requests.len(),
        producer.rocket_trace_requests.len(),
        producer.radius_damage_requests.len(),
        producer.mover_requests.len(),
        producer.contact_reconcile_requests.len(),
        producer.map_effects.len(),
        producer.regenerate_animation_events.len(),
        2 + usize::from(extensions.payload_constraint_blocked),
    ] {
        u32_field(&mut out, u32::try_from(count).ok()?, MAX)?;
    }
    for count in [
        extensions.random_draws.len(),
        extensions.audio_events.len(),
        extensions.rocket_results.len(),
        extensions.mover_results.len(),
        extensions.collision_snapshot.len(),
        random_state.len(),
        entity_presentation.len(),
        movement_tick_bytes.len(),
    ] {
        u32_field(&mut out, u32::try_from(count).ok()?, MAX)?;
    }
    u32_field(&mut out, producer.player_flags, MAX)?;
    u32_field(&mut out, snapshot.movement.water_type, MAX)?;
    u32_field(
        &mut out,
        u32::try_from(producer.flame_points.len()).ok()?,
        MAX,
    )?;
    u32_field(
        &mut out,
        u32::try_from(producer.shotgun_pellets.len()).ok()?,
        MAX,
    )?;
    u32_field(&mut out, u32::from(producer.flame_firing), MAX)?;
    extend(&mut out, &movement, MAX)?;
    for state in &producer.weapons {
        let profile = state.profile();
        extend(
            &mut out,
            &[weapon_code(state.weapon), state.reload as u8, 0, 0],
            MAX,
        )?;
        u16_field(&mut out, state.clip, MAX)?;
        u16_field(&mut out, state.reserve, MAX)?;
        u16_field(&mut out, profile.maximum_clip, MAX)?;
        u16_field(&mut out, profile.maximum_reserve, MAX)?;
        u64_field(&mut out, state.next_primary_tick, MAX)?;
        u64_field(&mut out, state.reload_due_tick.unwrap_or(u64::MAX), MAX)?;
        u64_field(&mut out, state.charge_begin_tick.unwrap_or(u64::MAX), MAX)?;
        u64_field(&mut out, state.first_primary_tick, MAX)?;
        let scalar = snapshot
            .spy
            .map_or(state.charged_damage, |spy| match state.weapon {
                playsrc_tf2::Weapon::InvisibilityWatch => spy.cloak_meter,
                playsrc_tf2::Weapon::DisguiseKit => spy.invisibility,
                playsrc_tf2::Weapon::Knife => spy.disguise_complete_time,
                playsrc_tf2::Weapon::Sapper => spy.no_attack_until,
                _ => state.charged_damage,
            });
        f32_field(&mut out, scalar, MAX)?;
    }
    for point in &producer.flame_points {
        extend(&mut out, &[point.slot, point.walls_hit, 0, 0], MAX)?;
        u64_field(&mut out, point.spawn_tick, MAX)?;
        floats(
            &mut out,
            [point.spawn_time, point.lifetime]
                .into_iter()
                .chain(point.initial_position)
                .chain(point.previous_position)
                .chain(point.position)
                .chain(point.velocity)
                .chain(point.attacker_velocity),
            MAX,
        )?;
    }
    for pellet in &producer.shotgun_pellets {
        extend(&mut out, &[pellet.index, 0, 0, 0], MAX)?;
        floats(
            &mut out,
            pellet
                .direction
                .into_iter()
                .chain([pellet.damage, pellet.range]),
            MAX,
        )?;
    }
    for projectile in &snapshot.projectiles {
        u32_field(&mut out, projectile.identity, MAX)?;
        extend(
            &mut out,
            &[
                projectile_code(projectile.kind),
                team_code(projectile.team),
                projectile.state as u8,
                u8::from(projectile.contact_normal.is_some()),
            ],
            MAX,
        )?;
        u32_field(&mut out, projectile.owner_identity, MAX)?;
        u32_field(&mut out, projectile.launcher_identity, MAX)?;
        floats(
            &mut out,
            projectile
                .position
                .into_iter()
                .chain(projectile.velocity)
                .chain(projectile.orientation)
                .chain(projectile.angular_velocity)
                .chain(projectile.contact_normal.unwrap_or([0.0; 3]))
                .chain([projectile.age_seconds]),
            MAX,
        )?;
    }
    for event in &snapshot.projectile_events {
        extend(
            &mut out,
            &[
                event.kind as u8,
                projectile_code(event.projectile_kind),
                team_code(event.team),
                u8::from(event.contact_normal.is_some())
                    | (u8::from(event.launcher_pose.is_some()) << 1),
            ],
            MAX,
        )?;
        u32_field(&mut out, event.projectile, MAX)?;
        u32_field(&mut out, event.owner_identity, MAX)?;
        u32_field(&mut out, event.launcher_identity, MAX)?;
        u64_field(&mut out, event.tick, MAX)?;
        floats(
            &mut out,
            event
                .position
                .into_iter()
                .chain(event.orientation)
                .chain(event.contact_normal.unwrap_or([0.0; 3]))
                .chain(
                    event
                        .launcher_pose
                        .map_or([0.0; 3], |pose| pose.eye_position),
                )
                .chain(
                    event
                        .launcher_pose
                        .map_or([0.0; 4], |pose| pose.view_orientation),
                ),
            MAX,
        )?;
    }
    for transform in &snapshot.entity_transforms {
        u32_field(&mut out, transform.identity, MAX)?;
        u32_field(&mut out, transform.model, MAX)?;
        floats(
            &mut out,
            transform.position.into_iter().chain(transform.angles),
            MAX,
        )?;
    }
    for event in &snapshot.entity_events {
        u64_field(&mut out, event.sequence, MAX)?;
        extend(
            &mut out,
            &[
                event.kind as u8,
                u8::from(event.accepted),
                event.contact.map_or(0, |value| match value {
                    playsrc_entity::ContactKind::Enter => 1,
                    playsrc_entity::ContactKind::Stay => 2,
                    playsrc_entity::ContactKind::Exit => 3,
                }),
                0,
            ],
            MAX,
        )?;
        u32_field(&mut out, event.entity, MAX)?;
        u32_field(&mut out, event.subject.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut out, u32::try_from(event.name.len()).ok()?, MAX)?;
        extend(&mut out, &event.name, MAX)?;
    }
    for event in &snapshot.events {
        encode_game_event(&mut out, event, MAX)?;
    }
    for event in &producer.activities {
        u64_field(&mut out, event.tick, MAX)?;
        extend(
            &mut out,
            &[
                weapon_code(event.weapon),
                match event.activity {
                    playsrc_tf2::weapon::WeaponActivity::Draw => 1,
                    playsrc_tf2::weapon::WeaponActivity::PrimaryAttack => 2,
                    playsrc_tf2::weapon::WeaponActivity::ReloadStart => 3,
                    playsrc_tf2::weapon::WeaponActivity::ReloadLoop => 4,
                    playsrc_tf2::weapon::WeaponActivity::ReloadFinish => 5,
                    playsrc_tf2::weapon::WeaponActivity::Idle => 6,
                    playsrc_tf2::weapon::WeaponActivity::SecondaryAttack => 7,
                },
                0,
                0,
                0,
                0,
                0,
                0,
            ],
            MAX,
        )?;
    }
    for event in &producer.lifecycle_events {
        u64_field(&mut out, event.tick, MAX)?;
        extend(
            &mut out,
            &[
                match event.kind {
                    playsrc_tf2::LifecycleEventKind::Died => 1,
                    playsrc_tf2::LifecycleEventKind::Respawned => 2,
                    playsrc_tf2::LifecycleEventKind::ClassChanged => 3,
                    playsrc_tf2::LifecycleEventKind::TeamChanged => 4,
                },
                class_code(event.class),
                team_code(event.team),
                0,
                0,
                0,
                0,
                0,
            ],
            MAX,
        )?;
    }
    for request in &producer.physics_requests {
        extend(
            &mut out,
            &[
                match request.operation {
                    playsrc_tf2::ProjectilePhysicsOperation::Create => 1,
                    playsrc_tf2::ProjectilePhysicsOperation::Step => 2,
                    playsrc_tf2::ProjectilePhysicsOperation::DisableMotion => 3,
                    playsrc_tf2::ProjectilePhysicsOperation::Destroy => 4,
                },
                0,
                0,
                0,
            ],
            MAX,
        )?;
        u32_field(&mut out, request.projectile, MAX)?;
        u64_field(&mut out, request.tick, MAX)?;
        floats(
            &mut out,
            request
                .position
                .into_iter()
                .chain(request.velocity)
                .chain(request.orientation)
                .chain(request.angular_velocity)
                .chain(request.hull.mins)
                .chain(request.hull.maxs)
                .chain([request.gravity_scale, request.friction, request.elasticity]),
            MAX,
        )?;
    }
    for request in &producer.rocket_trace_requests {
        u32_field(&mut out, request.projectile, MAX)?;
        u32_field(&mut out, 0, MAX)?;
        u64_field(&mut out, request.tick, MAX)?;
        floats(&mut out, request.start.into_iter().chain(request.end), MAX)?;
        u32_field(&mut out, request.mask, MAX)?;
    }
    for request in &producer.radius_damage_requests {
        u32_field(&mut out, request.projectile, MAX)?;
        extend(&mut out, &[projectile_code(request.kind), 0, 0, 0], MAX)?;
        floats(
            &mut out,
            request.source.into_iter().chain([
                request.base_damage,
                request.radius,
                request.self_radius,
            ]),
            MAX,
        )?;
        u32_field(&mut out, request.direct_target.unwrap_or(u32::MAX), MAX)?;
    }
    for request in &producer.mover_requests {
        u64_field(&mut out, request.request_id, MAX)?;
        u32_field(&mut out, request.entity, MAX)?;
        u32_field(&mut out, request.model.unwrap_or(u32::MAX), MAX)?;
        floats(
            &mut out,
            request
                .start
                .into_iter()
                .chain(request.destination)
                .chain([request.speed]),
            MAX,
        )?;
        extend(&mut out, &[u8::from(request.opening), 0, 0, 0], MAX)?;
    }
    for request in &producer.contact_reconcile_requests {
        u64_field(&mut out, request.tick, MAX)?;
        floats(
            &mut out,
            request
                .position
                .into_iter()
                .chain(request.hull.mins)
                .chain(request.hull.maxs),
            MAX,
        )?;
    }
    for effect in &producer.map_effects {
        encode_map_effect(&mut out, effect, MAX)?;
    }
    for event in &producer.regenerate_animation_events {
        u32_field(&mut out, event.zone, MAX)?;
        u32_field(&mut out, event.associated_model, MAX)?;
        u64_field(&mut out, event.open_tick, MAX)?;
        u64_field(&mut out, event.close_tick, MAX)?;
        i32_field(&mut out, event.body, MAX)?;
        extend(
            &mut out,
            &[
                match event.open_animation {
                    playsrc_tf2::RegenerateModelAnimation::Open => 1,
                    playsrc_tf2::RegenerateModelAnimation::Close => 2,
                },
                match event.close_animation {
                    playsrc_tf2::RegenerateModelAnimation::Open => 1,
                    playsrc_tf2::RegenerateModelAnimation::Close => 2,
                },
                0,
                0,
            ],
            MAX,
        )?;
    }
    extend(&mut out, &[1, 1, 0, 0, 2, 1, 0, 0], MAX)?;
    if extensions.payload_constraint_blocked {
        extend(&mut out, &[3, 1, 0, 0], MAX)?;
    }
    extend(&mut out, &random_state, MAX)?;
    for draw in extensions.random_draws {
        encode_random_draw(&mut out, *draw, MAX)?;
    }
    for event in extensions.audio_events {
        encode_audio_event(&mut out, *event, MAX)?;
    }
    for result in extensions.rocket_results {
        encode_rocket_result(&mut out, *result, MAX)?;
    }
    for result in extensions.mover_results {
        encode_mover_result(&mut out, *result, MAX)?;
    }
    extend(&mut out, extensions.collision_snapshot, MAX)?;
    extend(&mut out, &jump, MAX)?;
    extend(&mut out, &movement_tick_bytes, MAX)?;
    extend(&mut out, &entity_presentation, MAX)?;
    u32_field(&mut out, u32::try_from(snapshot.bots.len()).ok()?, MAX)?;
    for bot in &snapshot.bots {
        u32_field(&mut out, bot.identity, MAX)?;
        extend(
            &mut out,
            &[
                class_code(bot.class),
                team_code(bot.team),
                match bot.lifecycle {
                    playsrc_tf2::PlayerLifecycle::Active => 1,
                    playsrc_tf2::PlayerLifecycle::Dying => 2,
                    playsrc_tf2::PlayerLifecycle::Welcome => 3,
                    playsrc_tf2::PlayerLifecycle::Observer => 4,
                },
                bot.difficulty as u8,
                bot.objective as u8,
                0,
                0,
                0,
            ],
            MAX,
        )?;
        i32_field(&mut out, bot.health, MAX)?;
        i32_field(&mut out, bot.maximum_health, MAX)?;
        u32_field(&mut out, bot.target.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut out, bot.area.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut out, bot.remaining_path_areas, MAX)?;
        f32_field(&mut out, bot.yaw_degrees, MAX)?;
        floats(&mut out, bot.position.into_iter().chain(bot.velocity), MAX)?;
        f32_field(&mut out, bot.pitch_degrees, MAX)?;
        extend(
            &mut out,
            &[
                bot.weapon.map_or(0, |weapon| weapon.weapon as u8),
                bot.weapon.map_or(0, |weapon| weapon.reload as u8),
                u8::from(bot.carrying_flag),
                0,
            ],
            MAX,
        )?;
        for value in [
            bot.weapon.map_or(0, |weapon| weapon.clip),
            bot.weapon.map_or(0, |weapon| weapon.reserve),
            bot.weapon.map_or(0, |weapon| weapon.maximum_clip),
            bot.weapon.map_or(0, |weapon| weapon.maximum_reserve),
        ] {
            u16_field(&mut out, value, MAX)?;
        }
        for value in [bot.shots, bot.hits, bot.kills, bot.deaths, bot.captures] {
            u32_field(&mut out, value, MAX)?;
        }
        u64_field(&mut out, bot.last_fire_tick.unwrap_or(u64::MAX), MAX)?;
        u64_field(&mut out, bot.respawn_tick.unwrap_or(u64::MAX), MAX)?;
        u64_field(
            &mut out,
            bot.weapon.map_or(0, |weapon| weapon.next_primary_tick),
            MAX,
        )?;
        u64_field(
            &mut out,
            bot.weapon.map_or(0, |weapon| weapon.next_reload_tick),
            MAX,
        )?;
    }
    encode_objectives(&mut out, snapshot.objectives.as_ref(), MAX)?;
    encode_round(&mut out, &snapshot.round, MAX)?;
    Some(out)
}

fn encode_round(
    out: &mut Vec<u8>,
    round: &playsrc_tf2::round::Snapshot,
    maximum: usize,
) -> Option<()> {
    use playsrc_tf2::round::Event;
    extend(out, b"PGRL", maximum)?;
    u32_field(out, 1, maximum)?;
    let timer = round.timer;
    let flags = u8::from(round.waiting_for_players)
        | (u8::from(round.in_setup) << 1)
        | (u8::from(round.in_overtime) << 2)
        | (u8::from(timer.is_some()) << 3)
        | (u8::from(timer.is_some_and(|value| value.paused)) << 4)
        | (u8::from(timer.is_some_and(|value| value.configuration.show_in_hud)) << 5)
        | (u8::from(timer.is_some_and(|value| value.disabled)) << 6);
    extend(
        out,
        &[
            round.state as u8,
            flags,
            round.winning_team.map_or(0, team_code),
            round.win_reason,
        ],
        maximum,
    )?;
    u16_field(out, round.red_score, maximum)?;
    u16_field(out, round.blue_score, maximum)?;
    u32_field(out, round.rounds_played, maximum)?;
    f32_field(out, round.waiting_remaining.unwrap_or(-1.0), maximum)?;
    u32_field(
        out,
        timer.map_or(u32::MAX, |value| value.configuration.identity),
        maximum,
    )?;
    f32_field(out, timer.map_or(-1.0, |value| value.remaining), maximum)?;
    for value in [
        timer.map_or(0, |value| value.configuration.initial_seconds),
        timer.map_or(0, |value| value.configuration.setup_seconds),
        timer.map_or(0, |value| value.configuration.maximum_seconds),
    ] {
        u32_field(out, value, maximum)?;
    }
    u32_field(out, u32::try_from(round.events.len()).ok()?, maximum)?;
    for event in &round.events {
        let (kind, detail, team, flags, identity) = match *event {
            Event::StateChanged { previous, current } => (1, current as u8, 0, previous as u8, 0),
            Event::WaitingBegan => (2, 0, 0, 0, 0),
            Event::WaitingAboutToEnd => (3, 0, 0, 0, 0),
            Event::WaitingEnded => (4, 0, 0, 0, 0),
            Event::RoundStarted { full_reset } => (5, 0, 0, u8::from(full_reset), 0),
            Event::RoundActive => (6, 0, 0, 0, 0),
            Event::SetupFinished { timer } => (7, 0, 0, 0, timer),
            Event::TimerFinished { timer } => (8, 0, 0, 0, timer),
            Event::OvertimeChanged { active } => (9, 0, 0, u8::from(active), 0),
            Event::RoundWon { team, reason } => (10, reason, team_code(team), 0, 0),
            Event::RoundRespawn => (11, 0, 0, 0, 0),
            Event::ScoresReset => (12, 0, 0, 0, 0),
        };
        extend(out, &[kind, detail, team, flags], maximum)?;
        u32_field(out, identity, maximum)?;
    }
    Some(())
}

fn encode_objectives(
    out: &mut Vec<u8>,
    objectives: Option<&playsrc_tf2::ctf::Snapshot>,
    maximum: usize,
) -> Option<()> {
    extend(out, b"PCTF", maximum)?;
    u32_field(out, 1, maximum)?;
    extend(out, &[u8::from(objectives.is_some()), 0, 0, 0], maximum)?;
    let Some(objectives) = objectives else {
        return Some(());
    };
    for score in [
        objectives.scores.red_captures,
        objectives.scores.blue_captures,
        objectives.scores.red_score,
        objectives.scores.blue_score,
        objectives.scores.limit,
    ] {
        u16_field(out, score, maximum)?;
    }
    extend(
        out,
        &[objectives.scores.winner.map_or(0, team_code), 0],
        maximum,
    )?;
    for count in [
        objectives.flags.len(),
        objectives.zones.len(),
        objectives.events.len(),
    ] {
        u32_field(out, u32::try_from(count).ok()?, maximum)?;
    }
    for flag in &objectives.flags {
        u32_field(out, flag.identity, maximum)?;
        extend(
            out,
            &[
                team_code(flag.team),
                flag.status as u8,
                u8::from(flag.disabled)
                    | (u8::from(flag.visible_when_disabled) << 1)
                    | (u8::from(flag.shot_clock) << 2)
                    | (u8::from(flag.allow_owner_pickup) << 3)
                    | (u8::from(flag.trail_enabled) << 4)
                    | (u8::from(flag.captured) << 5),
                flag.skin,
            ],
            maximum,
        )?;
        for identity in [flag.carrier, flag.previous_carrier, flag.initial_carrier] {
            u32_field(out, identity.unwrap_or(u32::MAX), maximum)?;
        }
        f32_field(out, flag.return_deadline.unwrap_or(-1.0), maximum)?;
        f32_field(out, flag.maximum_return_seconds, maximum)?;
        f32_field(out, flag.owner_pickup_deadline.unwrap_or(-1.0), maximum)?;
        u16_field(out, flag.configured_return_seconds, maximum)?;
        u16_field(out, 0, maximum)?;
        floats(
            out,
            flag.position
                .into_iter()
                .chain(flag.home)
                .chain(flag.angles)
                .chain(flag.home_angles),
            maximum,
        )?;
        for value in [
            &flag.model,
            &flag.icon,
            &flag.paper_effect,
            &flag.trail_effect,
        ] {
            u16_field(out, u16::try_from(value.len()).ok()?, maximum)?;
            extend(out, value.as_bytes(), maximum)?;
        }
    }
    for zone in &objectives.zones {
        u32_field(out, zone.identity, maximum)?;
        extend(
            out,
            &[
                zone.team.map_or(0, team_code),
                u8::from(zone.disabled),
                0,
                0,
            ],
            maximum,
        )?;
        u32_field(out, u32::try_from(zone.model).ok()?, maximum)?;
        floats(out, zone.origin.into_iter().chain(zone.center), maximum)?;
        i32_field(out, zone.capture_point, maximum)?;
    }
    for event in &objectives.events {
        use playsrc_tf2::ctf::Event;
        let (kind, detail, team, flags, subject, player, auxiliary, value, extra) = match event {
            Event::StatusChanged {
                flag,
                status,
                owner,
            } => (
                1,
                *status as u8,
                0,
                0,
                *flag,
                owner.unwrap_or(u32::MAX),
                0,
                0.0,
                0.0,
            ),
            Event::Flag {
                flag,
                kind,
                player,
                team,
                priority,
                home,
            } => (
                2,
                *kind as u8,
                team_code(*team),
                home.map_or(0, |value| 1 | (u8::from(value) << 1)),
                *flag,
                player.unwrap_or(u32::MAX),
                u32::from(*priority),
                0.0,
                0.0,
            ),
            Event::Announcer {
                flag,
                recipient,
                sound,
                exclude_player,
            } => (
                3,
                *sound as u8,
                recipient.source_number(),
                0,
                *flag,
                exclude_player.unwrap_or(u32::MAX),
                0,
                0.0,
                0.0,
            ),
            Event::Notification {
                flag,
                recipient,
                notification,
                exclude_player,
            } => (
                4,
                *notification as u8,
                team_code(*recipient),
                0,
                *flag,
                exclude_player.unwrap_or(u32::MAX),
                0,
                0.0,
                0.0,
            ),
            Event::MapOutput {
                entity,
                output,
                activator,
            } => (
                5,
                match *output {
                    "OnReturn" => 1,
                    "OnPickUp" => 2,
                    "OnPickup1" => 3,
                    "OnPickupTeam1" => 4,
                    "OnPickupTeam2" => 5,
                    "OnDrop" => 6,
                    "OnDrop1" => 7,
                    "OnCapture" => 8,
                    "OnCapture1" => 9,
                    "OnCapTeam1" => 10,
                    "OnCapTeam2" => 11,
                    "OnTouchSameTeam" => 12,
                    _ => return None,
                },
                0,
                0,
                *entity,
                activator.unwrap_or(u32::MAX),
                0,
                0.0,
                0.0,
            ),
            Event::Captured {
                zone,
                player,
                team,
                team_score,
            } => (
                6,
                0,
                team_code(*team),
                0,
                *zone,
                *player,
                u32::from(*team_score),
                0.0,
                0.0,
            ),
            Event::CaptureBonus {
                team,
                condition,
                duration,
            } => (
                7,
                *condition,
                team_code(*team),
                0,
                0,
                u32::MAX,
                0,
                *duration,
                0.0,
            ),
            Event::RoundWon {
                team,
                reason,
                capture_limit,
            } => (
                8,
                *reason,
                team_code(*team),
                0,
                0,
                u32::MAX,
                u32::from(*capture_limit),
                0.0,
                0.0,
            ),
        };
        extend(out, &[kind, detail, team, flags], maximum)?;
        for field in [subject, player, auxiliary] {
            u32_field(out, field, maximum)?;
        }
        f32_field(out, value, maximum)?;
        f32_field(out, extra, maximum)?;
    }
    Some(())
}

fn encode_entity_presentation(
    snapshot: &playsrc_tf2::EntityPresentationSnapshot,
) -> Option<Vec<u8>> {
    const MAX: usize = 8 * 1024 * 1024;
    let e = &snapshot.entities;
    let mut out = b"PEBP".to_vec();
    u32_field(&mut out, 1, MAX)?;
    for value in [
        e.source_identity,
        e.registry_identity,
        e.tick,
        e.revision,
        snapshot.collision_revision,
    ] {
        u64_field(&mut out, value, MAX)?;
    }
    u32_field(&mut out, u32::try_from(e.models.len()).ok()?, MAX)?;
    for model in &e.models {
        u16_field(&mut out, model.handle.slot, MAX)?;
        u16_field(&mut out, 0, MAX)?;
        u32_field(&mut out, model.handle.generation, MAX)?;
        u32_field(&mut out, u32::try_from(model.source_index).ok()?, MAX)?;
        u32_field(&mut out, u32::try_from(model.model).ok()?, MAX)?;
        floats(
            &mut out,
            model
                .local_transform
                .origin
                .into_iter()
                .chain(model.local_transform.angles)
                .chain(model.world_transform.origin)
                .chain(model.world_transform.angles),
            MAX,
        )?;
        extend(
            &mut out,
            &[
                u8::from(model.parent.is_some()),
                model.render_mode,
                model.render_fx,
                u8::from(model.draw),
            ],
            MAX,
        )?;
        u16_field(&mut out, model.parent.map_or(0, |p| p.slot), MAX)?;
        u16_field(&mut out, 0, MAX)?;
        u32_field(&mut out, model.parent.map_or(0, |p| p.generation), MAX)?;
        extend(&mut out, &model.color, MAX)?;
        u16_field(&mut out, model.effects, MAX)?;
        let (kind, position, positioned) = model.mover.as_ref().map_or((0, 0, 0), |m| {
            let kind = match m.kind {
                playsrc_entity::MoverKind::Button => 1,
                playsrc_entity::MoverKind::Door => 2,
                playsrc_entity::MoverKind::Linear => 3,
            };
            let (p, b) = match m.position {
                playsrc_entity::MoverPosition::Closed => (1, 0),
                playsrc_entity::MoverPosition::Opening => (2, 0),
                playsrc_entity::MoverPosition::Open => (3, 0),
                playsrc_entity::MoverPosition::Closing => (4, 0),
                playsrc_entity::MoverPosition::Positioned(b) => (5, b),
            };
            (kind, p, b)
        });
        extend(&mut out, &[kind, position], MAX)?;
        u32_field(
            &mut out,
            model.mover.as_ref().map_or(0, |m| m.progress_bits),
            MAX,
        )?;
        u64_field(
            &mut out,
            model
                .mover
                .as_ref()
                .and_then(|m| m.request_id)
                .unwrap_or(u64::MAX),
            MAX,
        )?;
        floats(
            &mut out,
            model
                .mover
                .as_ref()
                .and_then(|m| m.local_destination)
                .unwrap_or([0.0; 3])
                .into_iter()
                .chain(
                    model
                        .mover
                        .as_ref()
                        .and_then(|m| m.world_destination)
                        .unwrap_or([0.0; 3]),
                ),
            MAX,
        )?;
        extend(
            &mut out,
            &[
                model
                    .mover
                    .as_ref()
                    .and_then(|m| m.opening)
                    .map_or(0, |v| if v { 1 } else { 2 }),
                0,
                0,
                0,
            ],
            MAX,
        )?;
        u32_field(&mut out, positioned, MAX)?;
    }
    Some(out)
}

fn encode_random_state(state: playsrc_tf2::Tf2RandomState) -> Option<Vec<u8>> {
    let mut output = b"PRNG".to_vec();
    output.extend_from_slice(&1_u32.to_le_bytes());
    for stream in [state.authority, state.predicted_presentation] {
        output.extend_from_slice(&stream.current.to_le_bytes());
        output.extend_from_slice(&stream.shuffled.to_le_bytes());
        for value in stream.table {
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
    output.extend_from_slice(&[
        state.sound_selection.rocket_explosion_available
            | state.sound_selection.fire_axe_hit_world_available << 3,
        state.sound_selection.sticky_explosion_available
            | state.sound_selection.fire_axe_hit_flesh_available << 3,
        state.sound_selection.bat_hit_world_available
            | state.sound_selection.bottle_hit_flesh_available << 2
            | state.sound_selection.bottle_hit_world_available << 5,
        state.sound_selection.shovel_hit_world_available
            | state.sound_selection.shovel_hit_flesh_available << 2
            | state.sound_selection.knife_hit_flesh_available << 5,
        state.sound_selection.fist_miss_available,
        state.sound_selection.fist_hit_world_available,
        state.sound_selection.fist_hit_flesh_available,
        state.sound_selection.kukri_hit_flesh_available
            | state.sound_selection.kukri_hit_world_available << 3
            | state.sound_selection.wrench_hit_flesh_available << 5,
        state.sound_selection.flag_enemy_stolen_available,
        state.sound_selection.flag_enemy_dropped_available,
        state.sound_selection.flag_enemy_captured_available,
        state.sound_selection.flag_enemy_returned_available,
        state.sound_selection.flag_team_dropped_available,
        0,
        0,
        0,
    ]);
    (output.len() == 296).then_some(output)
}

fn sound_definition_code(value: playsrc_tf2::SoundDefinition) -> u8 {
    match value {
        playsrc_tf2::SoundDefinition::RocketSingle => 1,
        playsrc_tf2::SoundDefinition::OriginalSingle => 2,
        playsrc_tf2::SoundDefinition::StickySingle => 3,
        playsrc_tf2::SoundDefinition::RocketExplosion => 4,
        playsrc_tf2::SoundDefinition::OriginalExplosion => 5,
        playsrc_tf2::SoundDefinition::StickyExplosion => 6,
        playsrc_tf2::SoundDefinition::ScattergunSingle => 7,
        playsrc_tf2::SoundDefinition::PistolSingle => 8,
        playsrc_tf2::SoundDefinition::BatMiss => 9,
        playsrc_tf2::SoundDefinition::BatHitFlesh => 10,
        playsrc_tf2::SoundDefinition::BatHitWorld => 11,
        playsrc_tf2::SoundDefinition::ScattergunReload => 12,
        playsrc_tf2::SoundDefinition::PistolReload => 13,

        playsrc_tf2::SoundDefinition::ShotgunSingle => 14,
        playsrc_tf2::SoundDefinition::ShotgunReload => 15,
        playsrc_tf2::SoundDefinition::ShovelMiss => 16,
        playsrc_tf2::SoundDefinition::ShovelHitFlesh => 17,
        playsrc_tf2::SoundDefinition::ShovelHitWorld => 18,
        playsrc_tf2::SoundDefinition::MinigunWindUp => 19,
        playsrc_tf2::SoundDefinition::MinigunWindDown => 20,
        playsrc_tf2::SoundDefinition::MinigunSpin => 21,
        playsrc_tf2::SoundDefinition::MinigunFire => 22,
        playsrc_tf2::SoundDefinition::FistMiss => 23,
        playsrc_tf2::SoundDefinition::FistHitWorld => 24,
        playsrc_tf2::SoundDefinition::FistHitFlesh => 25,
        playsrc_tf2::SoundDefinition::SniperSingle => 26,
        playsrc_tf2::SoundDefinition::SmgSingle => 27,
        playsrc_tf2::SoundDefinition::KukriMiss => 28,
        playsrc_tf2::SoundDefinition::KukriHitFlesh => 29,
        playsrc_tf2::SoundDefinition::KukriHitWorld => 30,
        playsrc_tf2::SoundDefinition::SmgReload => 31,
        playsrc_tf2::SoundDefinition::ShotgunEmpty => 32,
        playsrc_tf2::SoundDefinition::PistolEmpty => 33,
        playsrc_tf2::SoundDefinition::WrenchMiss => 34,
        playsrc_tf2::SoundDefinition::WrenchHitFlesh => 35,
        playsrc_tf2::SoundDefinition::WrenchHitWorld => 36,
        playsrc_tf2::SoundDefinition::FlameFire => 37,
        playsrc_tf2::SoundDefinition::FlameLoop => 38,
        playsrc_tf2::SoundDefinition::FlameEnd => 39,
        playsrc_tf2::SoundDefinition::FlameAirblast => 40,
        playsrc_tf2::SoundDefinition::FireAxeMiss => 41,
        playsrc_tf2::SoundDefinition::FireAxeHitFlesh => 42,
        playsrc_tf2::SoundDefinition::FireAxeHitWorld => 43,
        playsrc_tf2::SoundDefinition::FlagEnemyStolen => 44,
        playsrc_tf2::SoundDefinition::FlagEnemyDropped => 45,
        playsrc_tf2::SoundDefinition::FlagEnemyCaptured => 46,
        playsrc_tf2::SoundDefinition::FlagEnemyReturned => 47,
        playsrc_tf2::SoundDefinition::FlagTeamStolen => 48,
        playsrc_tf2::SoundDefinition::FlagTeamDropped => 49,
        playsrc_tf2::SoundDefinition::FlagTeamCaptured => 50,
        playsrc_tf2::SoundDefinition::FlagTeamReturned => 51,
        playsrc_tf2::SoundDefinition::FlagSpawn => 52,
        playsrc_tf2::SoundDefinition::TeamWon => 53,
        playsrc_tf2::SoundDefinition::TeamLost => 54,
        playsrc_tf2::SoundDefinition::BottleMiss => 55,
        playsrc_tf2::SoundDefinition::BottleHitFlesh => 56,
        playsrc_tf2::SoundDefinition::BottleHitWorld => 57,
        playsrc_tf2::SoundDefinition::RevolverSingle => 58,
        playsrc_tf2::SoundDefinition::RevolverReload => 59,
        playsrc_tf2::SoundDefinition::KnifeMiss => 60,
        playsrc_tf2::SoundDefinition::KnifeHitFlesh => 61,
        playsrc_tf2::SoundDefinition::KnifeHitWorld => 62,
        playsrc_tf2::SoundDefinition::SpyCloak => 63,
        playsrc_tf2::SoundDefinition::SpyUncloak => 64,
    }
}

fn encode_random_draw(
    output: &mut Vec<u8>,
    draw: playsrc_tf2::RandomDraw,
    limit: usize,
) -> Option<()> {
    let (decision, definition, phase) = match draw.decision {
        playsrc_tf2::RandomDecision::SoundVolume { definition, phase } => {
            (1, sound_definition_code(definition), phase)
        }
        playsrc_tf2::RandomDecision::SoundPitch { definition, phase } => {
            (2, sound_definition_code(definition), phase)
        }
        playsrc_tf2::RandomDecision::SoundWave { definition, phase } => {
            (3, sound_definition_code(definition), phase)
        }
        playsrc_tf2::RandomDecision::SoundLevel { definition, phase } => {
            (4, sound_definition_code(definition), phase)
        }
        playsrc_tf2::RandomDecision::StickyRightVelocity => {
            (5, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
        playsrc_tf2::RandomDecision::StickyUpVelocity => {
            (6, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
        playsrc_tf2::RandomDecision::StickyAngularY => {
            (7, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
        playsrc_tf2::RandomDecision::ClassSelection => {
            (8, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
    };
    extend(
        output,
        &[
            match draw.context {
                playsrc_tf2::RandomContext::Authority => 1,
                playsrc_tf2::RandomContext::PredictedPresentation => 2,
            },
            decision,
            definition,
            if definition == 0 {
                0
            } else {
                match phase {
                    playsrc_tf2::SoundQueryPhase::Inspect => 1,
                    playsrc_tf2::SoundQueryPhase::Emit => 2,
                }
            },
        ],
        limit,
    )?;
    i32_field(output, draw.raw, limit)?;
    let (kind, result) = match draw.result {
        playsrc_tf2::RandomResult::FloatBits(value) => (1, value),
        playsrc_tf2::RandomResult::Integer(value) => (2, value as u32),
        playsrc_tf2::RandomResult::RejectedIntegerCandidate => (3, 0),
    };
    extend(output, &[kind, 0, 0, 0], limit)?;
    u32_field(output, result, limit)
}

fn encode_audio_event(
    output: &mut Vec<u8>,
    event: playsrc_tf2::AudioEvent,
    limit: usize,
) -> Option<()> {
    u64_field(output, event.tick, limit)?;
    u16_field(output, event.ordinal, limit)?;
    extend(
        output,
        &[
            match event.identity {
                playsrc_tf2::AudioEventIdentity::WeaponSingle => 1,
                playsrc_tf2::AudioEventIdentity::ExplosionSpecial1 => 2,
            },
            sound_definition_code(event.definition),
            match event.source_kind {
                playsrc_tf2::AudioSourceKind::Entity => 1,
                playsrc_tf2::AudioSourceKind::World => 2,
            },
            u8::from(event.owner_identity.is_some()),
            event.samples.wave,
            0,
        ],
        limit,
    )?;
    u32_field(output, event.source_identity, limit)?;
    u32_field(output, event.owner_identity.unwrap_or(u32::MAX), limit)?;
    floats(
        output,
        event.position.into_iter().chain([
            event.samples.volume,
            event.samples.pitch,
            event.samples.sound_level,
        ]),
        limit,
    )?;
    u32_field(output, 0, limit)
}

fn encode_rocket_result(
    output: &mut Vec<u8>,
    result: playsrc_tf2::RocketTraceResult,
    limit: usize,
) -> Option<()> {
    u32_field(output, result.projectile, limit)?;
    u64_field(output, result.tick, limit)?;
    extend(
        output,
        &[
            u8::from(result.solid),
            u8::from(result.sky),
            u8::from(result.normal.is_some()),
            u8::from(result.direct_target.is_some()),
        ],
        limit,
    )?;
    floats(
        output,
        result
            .end
            .into_iter()
            .chain(result.normal.unwrap_or([0.0; 3])),
        limit,
    )?;
    u32_field(output, result.direct_target.unwrap_or(u32::MAX), limit)
}

fn encode_mover_result(
    output: &mut Vec<u8>,
    result: playsrc_tf2::MoverResult,
    limit: usize,
) -> Option<()> {
    u64_field(output, result.request_id, limit)?;
    u32_field(output, result.entity, limit)?;
    extend(
        output,
        &[
            match result.kind {
                playsrc_tf2::MoverResultKind::Progress => 1,
                playsrc_tf2::MoverResultKind::Completed => 2,
                playsrc_tf2::MoverResultKind::BlockedStart => 3,
                playsrc_tf2::MoverResultKind::BlockedStay => 4,
                playsrc_tf2::MoverResultKind::BlockedEnd => 5,
            },
            0,
            0,
            0,
        ],
        limit,
    )?;
    floats(
        output,
        result
            .transform
            .origin
            .into_iter()
            .chain(result.transform.angles)
            .chain(result.carry),
        limit,
    )
}

fn encode_map_effect(
    output: &mut Vec<u8>,
    effect: &playsrc_tf2::MapEffect,
    limit: usize,
) -> Option<()> {
    let (kind, detail, team, contact, subject, auxiliary, values) = match *effect {
        playsrc_tf2::MapEffect::Teleport {
            trigger,
            destination,
            position,
            angles,
        } => (
            1,
            u8::from(angles.is_some()),
            0,
            0,
            trigger,
            destination,
            [
                position[0],
                position[1],
                position[2],
                angles.unwrap_or([0.0; 3])[0],
                angles.unwrap_or([0.0; 3])[1],
                angles.unwrap_or([0.0; 3])[2],
            ],
        ),
        playsrc_tf2::MapEffect::Hurt {
            trigger,
            damage_per_second,
            contact,
        } => (
            2,
            0,
            0,
            contact_code(contact),
            trigger,
            u32::MAX,
            [damage_per_second, 0.0, 0.0, 0.0, 0.0, 0.0],
        ),
        playsrc_tf2::MapEffect::Push {
            trigger,
            velocity,
            replace,
        } => (
            3,
            u8::from(replace),
            0,
            0,
            trigger,
            u32::MAX,
            [velocity[0], velocity[1], velocity[2], 0.0, 0.0, 0.0],
        ),
        playsrc_tf2::MapEffect::Regenerate {
            entity,
            team,
            associated_model,
        } => (
            4,
            0,
            team.unwrap_or(0),
            0,
            entity,
            associated_model.unwrap_or(u32::MAX),
            [0.0; 6],
        ),
        playsrc_tf2::MapEffect::RespawnRoom {
            entity,
            team,
            contact,
        } => (
            5,
            0,
            team.unwrap_or(0),
            contact_code(contact),
            entity,
            u32::MAX,
            [0.0; 6],
        ),
    };
    extend(output, &[kind, detail, team, contact], limit)?;
    u32_field(output, subject, limit)?;
    u32_field(output, auxiliary, limit)?;
    u32_field(output, 0, limit)?;
    floats(output, values, limit)
}

fn contact_code(value: playsrc_entity::ContactKind) -> u8 {
    match value {
        playsrc_entity::ContactKind::Enter => 1,
        playsrc_entity::ContactKind::Stay => 2,
        playsrc_entity::ContactKind::Exit => 3,
    }
}

fn encode_movement_tick(
    output: &mut Vec<u8>,
    result: Option<&playsrc_movement::StepResult>,
    limit: usize,
) -> Option<()> {
    extend(output, b"PMTK", limit)?;
    u32_field(output, 1, limit)?;
    let Some(result) = result else {
        extend(output, &[0, 0, 0, 0], limit)?;
        return Some(());
    };
    extend(
        output,
        &[
            1,
            result.state.mode as u8,
            result.state.crouch.phase as u8,
            u8::from(result.state.ground.is_some()),
        ],
        limit,
    )?;
    floats(
        output,
        result
            .wish_state
            .direction
            .into_iter()
            .chain([result.wish_state.speed, result.wish_state.uncapped_speed])
            .chain(result.wish_velocity)
            .chain(result.jump_velocity)
            .chain([result.climbed_step])
            .chain(result.selected_hull.mins)
            .chain(result.selected_hull.maxs),
        limit,
    )?;
    for value in [
        result.queries.len(),
        result.point_queries.len(),
        result.contacts.len(),
        result.events.len(),
    ] {
        u32_field(output, u32::try_from(value).ok()?, limit)?;
    }
    match result.mover_result {
        None => extend(output, &[0, 0, 0, 0], limit),
        Some(mover) => {
            extend(output, &[1, mover.status as u8, 0, 0], limit)?;
            u64_field(output, mover.identity, limit)?;
            floats(
                output,
                mover.displacement.into_iter().chain(mover.support_velocity),
                limit,
            )?;
            u64_field(output, mover.blocker.unwrap_or(u64::MAX), limit)
        }
    }
}

fn class_code(class: playsrc_tf2::PlayerClass) -> u8 {
    class.source_number()
}
fn team_code(team: playsrc_tf2::PlayerTeam) -> u8 {
    team.source_number()
}
fn weapon_code(weapon: playsrc_tf2::Weapon) -> u8 {
    match weapon {
        playsrc_tf2::Weapon::RocketLauncher => 1,
        playsrc_tf2::Weapon::Original => 2,
        playsrc_tf2::Weapon::StickybombLauncher => 3,
        playsrc_tf2::Weapon::Scattergun => 4,
        playsrc_tf2::Weapon::Pistol => 5,
        playsrc_tf2::Weapon::Bat => 6,

        playsrc_tf2::Weapon::Shotgun => 7,
        playsrc_tf2::Weapon::Shovel => 8,
        playsrc_tf2::Weapon::Minigun => 9,
        playsrc_tf2::Weapon::HeavyShotgun => 10,
        playsrc_tf2::Weapon::Fists => 11,
        playsrc_tf2::Weapon::SniperRifle => 12,
        playsrc_tf2::Weapon::Smg => 13,
        playsrc_tf2::Weapon::Kukri => 14,
        playsrc_tf2::Weapon::Bottle => 17,
        playsrc_tf2::Weapon::GrenadeLauncher => 18,
        playsrc_tf2::Weapon::EngineerShotgun => 40,
        playsrc_tf2::Weapon::EngineerPistol => 41,
        playsrc_tf2::Weapon::Wrench => 42,
        playsrc_tf2::Weapon::Flamethrower => 15,
        playsrc_tf2::Weapon::FireAxe => 16,
        playsrc_tf2::Weapon::Revolver => 50,
        playsrc_tf2::Weapon::Knife => 51,
        playsrc_tf2::Weapon::Sapper => 52,
        playsrc_tf2::Weapon::DisguiseKit => 53,
        playsrc_tf2::Weapon::InvisibilityWatch => 54,
    }
}
fn projectile_code(kind: playsrc_tf2::ProjectileKind) -> u8 {
    match kind {
        playsrc_tf2::ProjectileKind::Rocket => 1,
        playsrc_tf2::ProjectileKind::Sticky => 2,
    }
}

fn extend(output: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Option<()> {
    (output.len().checked_add(bytes.len())? <= limit).then(|| output.extend_from_slice(bytes))
}

fn u16_field(output: &mut Vec<u8>, value: u16, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn u32_field(output: &mut Vec<u8>, value: u32, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn i32_field(output: &mut Vec<u8>, value: i32, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn u64_field(output: &mut Vec<u8>, value: u64, limit: usize) -> Option<()> {
    extend(output, &value.to_le_bytes(), limit)
}

fn f32_field(output: &mut Vec<u8>, value: f32, limit: usize) -> Option<()> {
    value
        .is_finite()
        .then_some(())
        .and_then(|()| extend(output, &value.to_le_bytes(), limit))
}

fn floats(output: &mut Vec<u8>, values: impl IntoIterator<Item = f32>, limit: usize) -> Option<()> {
    for value in values {
        f32_field(output, value, limit)?;
    }
    Some(())
}

fn encode_game_event(output: &mut Vec<u8>, event: &playsrc_tf2::Event, limit: usize) -> Option<()> {
    let (kind, detail, subject, auxiliary, values) = match event {
        playsrc_tf2::Event::ClassChanged(value) => (1, class_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::TeamChanged(value) => (2, team_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::WeaponChanged(value) => (3, weapon_code(*value), 0, 0, [0.0; 4]),
        playsrc_tf2::Event::Reloaded {
            weapon,
            clip,
            reserve,
        } => (
            4,
            weapon_code(*weapon),
            0,
            0,
            [*clip as f32, *reserve as f32, 0.0, 0.0],
        ),
        playsrc_tf2::Event::Resupplied {
            entity,
            health,
            weapon,
            clip,
            reserve,
        } => (
            5,
            weapon_code(*weapon),
            *entity,
            0,
            [*health, *clip as f32, *reserve as f32, 0.0],
        ),
        playsrc_tf2::Event::Damaged { amount, health } => {
            (6, 0, 0, 0, [*amount, *health, 0.0, 0.0])
        }
        playsrc_tf2::Event::Healed {
            amount,
            health,
            trigger,
        } => (7, 0, *trigger, 0, [*amount, *health, 0.0, 0.0]),
        playsrc_tf2::Event::BlastImpulse { velocity } => {
            (8, 0, 0, 0, [velocity[0], velocity[1], velocity[2], 0.0])
        }
        playsrc_tf2::Event::Teleported {
            trigger,
            destination,
            position,
            yaw_degrees,
        } => (
            9,
            u8::from(yaw_degrees.is_some()),
            *trigger,
            *destination,
            [
                position[0],
                position[1],
                position[2],
                yaw_degrees.unwrap_or(0.0),
            ],
        ),
        playsrc_tf2::Event::TriggerVelocity { trigger, velocity } => (
            10,
            0,
            *trigger,
            0,
            [velocity[0], velocity[1], velocity[2], 0.0],
        ),
        playsrc_tf2::Event::Respawned => (11, 0, 0, 0, [0.0; 4]),
        playsrc_tf2::Event::HitscanFired { weapon, pellets } => {
            (12, weapon_code(*weapon), u32::from(*pellets), 0, [0.0; 4])
        }
        playsrc_tf2::Event::HitscanImpact {
            weapon,
            target,
            pellet,
            hitgroup,
            critical,
            position,
            damage,
        } => (
            13,
            weapon_code(*weapon),
            target.unwrap_or(0),
            u32::from(*pellet) | (u32::from(*hitgroup) << 8) | (u32::from(*critical) << 16),
            [position[0], position[1], position[2], *damage],
        ),
        playsrc_tf2::Event::MeleeImpact {
            weapon,
            target,
            position,
            damage,
        } => (
            14,
            weapon_code(*weapon),
            target.unwrap_or(0),
            0,
            [position[0], position[1], position[2], *damage],
        ),
    };
    extend(output, &[kind, detail, 0, 0], limit)?;
    u32_field(output, subject, limit)?;
    u32_field(output, auxiliary, limit)?;
    floats(output, values, limit)
}

fn encode_jump(output: &playsrc_tf2::jump::TickOutput) -> Option<Vec<u8>> {
    const MAX: usize = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    extend(&mut bytes, b"PJOF", MAX)?;
    u32_field(&mut bytes, 1, MAX)?;
    match &output.run {
        None => extend(&mut bytes, &[0; 4], MAX)?,
        Some(run) => {
            extend(
                &mut bytes,
                &[1, run.disposition as u8, run.class as u8, run.team as u8],
                MAX,
            )?;
            u32_field(&mut bytes, run.instance, MAX)?;
            u32_field(&mut bytes, run.player_identity, MAX)?;
            extend(
                &mut bytes,
                &[run.invalidation.map_or(0, |value| value as u8), 0, 0, 0],
                MAX,
            )?;
            u64_field(&mut bytes, run.start_tick, MAX)?;
            u64_field(&mut bytes, run.end_tick.unwrap_or(u64::MAX), MAX)?;
            u32_field(&mut bytes, u32::try_from(run.checkpoints.len()).ok()?, MAX)?;
            for visit in &run.checkpoints {
                u32_field(&mut bytes, visit.zone_identity, MAX)?;
                u32_field(&mut bytes, visit.index, MAX)?;
                u64_field(&mut bytes, visit.tick, MAX)?;
            }
        }
    }
    u32_field(&mut bytes, u32::try_from(output.events.len()).ok()?, MAX)?;
    for event in &output.events {
        let (kind, detail) = match event.kind {
            playsrc_tf2::jump::EventKind::Started => (1, 0),
            playsrc_tf2::jump::EventKind::CheckpointAccepted => (2, 0),
            playsrc_tf2::jump::EventKind::Rejected(reason) => (3, reason as u8),
            playsrc_tf2::jump::EventKind::Completed => (4, 0),
            playsrc_tf2::jump::EventKind::Invalidated(reason) => (5, reason as u8),
            playsrc_tf2::jump::EventKind::ResetRequested => (6, 0),
            playsrc_tf2::jump::EventKind::RespawnObserved => (7, 0),
            playsrc_tf2::jump::EventKind::TeleportObserved => (8, 0),
        };
        u64_field(&mut bytes, event.sequence, MAX)?;
        u64_field(&mut bytes, event.tick, MAX)?;
        extend(&mut bytes, &[kind, detail, 0, 0], MAX)?;
        u32_field(&mut bytes, event.run_instance, MAX)?;
        u32_field(&mut bytes, event.zone_identity.unwrap_or(u32::MAX), MAX)?;
        u32_field(&mut bytes, event.zone_index.unwrap_or(u32::MAX), MAX)?;
    }
    match &output.result {
        None => extend(&mut bytes, &[0, 0, 0, 0], MAX)?,
        Some(result) => {
            extend(
                &mut bytes,
                &[
                    1,
                    result.class as u8,
                    result.team as u8,
                    result.run_kind as u8,
                ],
                MAX,
            )?;
            u64_field(&mut bytes, result.course_identity, MAX)?;
            extend(&mut bytes, &result.map_identity, MAX)?;
            u32_field(&mut bytes, result.run_instance, MAX)?;
            u32_field(&mut bytes, result.player_identity, MAX)?;
            u32_field(&mut bytes, result.zone_index, MAX)?;
            extend(&mut bytes, &[result.disposition as u8, 0, 0, 0], MAX)?;
            u64_field(&mut bytes, result.start_tick, MAX)?;
            u64_field(&mut bytes, result.end_tick, MAX)?;
            u64_field(&mut bytes, result.elapsed_ticks, MAX)?;
            u32_field(&mut bytes, result.tick_interval_bits, MAX)?;
            u32_field(
                &mut bytes,
                u32::try_from(result.checkpoints.len()).ok()?,
                MAX,
            )?;
            for visit in &result.checkpoints {
                u32_field(&mut bytes, visit.zone_identity, MAX)?;
                u32_field(&mut bytes, visit.index, MAX)?;
                u64_field(&mut bytes, visit.tick, MAX)?;
            }
        }
    }
    Some(bytes)
}

fn bundle(bytes: &[u8]) -> Result<BTreeMap<String, &[u8]>, ()> {
    if bytes.is_empty() {
        return Ok(BTreeMap::new());
    }
    if bytes.len() < 12
        || &bytes[..4] != b"PSRE"
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ())?) != 1
    {
        return Err(());
    }
    let count = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| ())?) as usize;
    if count > 4_096 {
        return Err(());
    }
    let mut offset = 12;
    let mut output = BTreeMap::new();
    for _ in 0..count {
        let path = bundle_field(bytes, &mut offset)?;
        let value = bundle_field(bytes, &mut offset)?;
        let path = std::str::from_utf8(path).map_err(|_| ())?;
        if path.is_empty()
            || path.len() > 1_024
            || path != path.to_ascii_lowercase()
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || output.insert(path.to_owned(), value).is_some()
        {
            return Err(());
        }
    }
    (offset == bytes.len()).then_some(output).ok_or(())
}

fn bundle_field<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a [u8], ()> {
    let end = (*offset).checked_add(4).ok_or(())?;
    let length = u32::from_le_bytes(
        bytes
            .get(*offset..end)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    ) as usize;
    *offset = end;
    let end = (*offset).checked_add(length).ok_or(())?;
    let value = bytes.get(*offset..end).ok_or(())?;
    *offset = end;
    Ok(value)
}

fn dependency_path(token: &[u8]) -> Result<String, ()> {
    let value = std::str::from_utf8(token)
        .map_err(|_| ())?
        .replace('\\', "/");
    let mut path = if value.to_ascii_lowercase().starts_with("materials/") {
        value
    } else {
        format!("materials/{value}")
    };
    if !path.to_ascii_lowercase().ends_with(".vmt") {
        path.push_str(".vmt");
    }
    Ok(path.to_ascii_lowercase())
}

fn surface_property_registry(
    bundle: &BTreeMap<String, &[u8]>,
) -> Result<playsrc_material::SurfacePropertyRegistry, ()> {
    let manifest_path = "scripts/surfaceproperties_manifest.txt";
    let manifest = bundle.get(manifest_path).copied().ok_or(())?;
    let document = playsrc_keyvalues::parse_text(
        manifest,
        playsrc_keyvalues::EscapeMode::Escaped,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|_| ())?;
    let roots = if document.roots.len() == 1 {
        match &document.roots[0].value {
            playsrc_keyvalues::Value::Object(children) => children.as_slice(),
            _ => document.roots.as_slice(),
        }
    } else {
        document.roots.as_slice()
    };
    let mut paths = Vec::new();
    for node in roots {
        let playsrc_keyvalues::Value::Scalar(value) = &node.value else {
            return Err(());
        };
        if !node.key.bytes.eq_ignore_ascii_case(b"file") || node.condition.is_some() {
            return Err(());
        }
        paths.push(
            std::str::from_utf8(&value.token.bytes)
                .map_err(|_| ())?
                .replace('\\', "/")
                .to_ascii_lowercase(),
        );
    }
    let files = paths
        .iter()
        .map(|path| {
            Ok(playsrc_material::SurfacePropertyFile {
                logical_path: path,
                bytes: bundle.get(path).copied().ok_or(())?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    playsrc_material::SurfacePropertyRegistry::compile(&files).map_err(|_| ())
}

fn attach_displacement_collision_inputs(
    world: playsrc_collision::World,
    map: &playsrc_map::CanonicalMap,
    bundle: &BTreeMap<String, &[u8]>,
    materials: &[playsrc_material::Material],
) -> Result<playsrc_collision::World, ()> {
    let registry = surface_property_registry(bundle)?;
    let inputs = map
        .collision_displacements
        .iter()
        .map(|patch| {
            let material = materials.get(patch.material).ok_or(())?;
            let primary = registry
                .resolve(material.surface_property.as_deref())
                .or_else(|| registry.resolve(Some(b"default")))
                .ok_or(())?;
            let secondary = material
                .secondary_surface_property
                .as_deref()
                .map(|name| registry.resolve(Some(name)).ok_or(()))
                .transpose()?;
            let has_secondary = secondary.is_some();
            Ok(playsrc_collision::DisplacementInput {
                source: patch.source,
                parent_face: patch.parent_face,
                contents: patch.contents,
                positions: patch.positions.clone(),
                triangles: patch.triangles.clone(),
                triangle_tags: patch.triangle_tags.clone(),
                primary_surface: playsrc_collision::SurfaceIdentity {
                    registry: registry.identity,
                    index: primary.index,
                },
                secondary_surface: secondary.map(|record| playsrc_collision::SurfaceIdentity {
                    registry: registry.identity,
                    index: record.index,
                }),
                use_secondary_surface: patch
                    .secondary_surface
                    .iter()
                    .map(|selected| *selected && has_secondary)
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    world.with_displacement_inputs(inputs).map_err(|_| ())
}

fn resolve_materials(
    map: &playsrc_map::CanonicalMap,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    profile: playsrc_map::LightingProfile,
) -> Result<
    (
        Vec<playsrc_map::RuntimeMaterial>,
        Vec<playsrc_material::Material>,
    ),
    (),
> {
    if bundle.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut textures = BTreeMap::new();
    let mut runtime = Vec::with_capacity(map.materials.len());
    let mut semantics = Vec::with_capacity(map.materials.len());
    for reference in &map.materials {
        let identity = reference.logical_path.to_ascii_lowercase();
        let material =
            resolve_material_semantics(&identity, bundle, material_environment(profile, false))?;
        runtime.push(resolve_material_output(
            &identity,
            &material,
            decoders,
            true,
            &mut textures,
        )?);
        semantics.push(material);
    }
    Ok((runtime, semantics))
}

fn resolve_material(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    include_texture: bool,
    environment: playsrc_material::SelectionEnvironment,
) -> Result<playsrc_map::RuntimeMaterial, ()> {
    let material = resolve_material_semantics(identity, bundle, environment)?;
    resolve_material_output(
        identity,
        &material,
        decoders,
        include_texture,
        &mut BTreeMap::new(),
    )
}

fn resolve_material_output(
    identity: &str,
    material: &playsrc_material::Material,
    decoders: &TextureDecoders<'_>,
    include_texture: bool,
    textures: &mut BTreeMap<String, playsrc_map::RuntimeTexture>,
) -> Result<playsrc_map::RuntimeMaterial, ()> {
    let selected_role = material.selected_textures.first().copied();
    let base = if include_texture
        && let Some(texture) = material.textures.iter().find(|texture| {
            Some(texture.role) == selected_role
                && texture.disposition == playsrc_material::TextureDisposition::Source
        }) {
        let path = texture
            .logical_path
            .as_ref()
            .ok_or(())?
            .to_ascii_lowercase();
        Some(cached_runtime_texture(&path, decoders, textures)?)
    } else {
        None
    };
    let detail = if include_texture {
        material
            .detail
            .as_ref()
            .map(|detail| {
                let path = detail
                    .texture
                    .logical_path
                    .as_ref()
                    .ok_or(())?
                    .to_ascii_lowercase();
                Ok(playsrc_map::RuntimeDetail {
                    texture: cached_runtime_texture(&path, decoders, textures)?,
                    scale: detail.scale,
                    blend_mode: detail.blend_mode,
                    blend_factor: detail.blend_factor,
                    tint: detail.tint,
                })
            })
            .transpose()?
    } else {
        None
    };
    let second_texture = if include_texture {
        material
            .textures
            .iter()
            .find(|texture| {
                texture.role == playsrc_material::TextureRole::Base2
                    && texture.disposition == playsrc_material::TextureDisposition::Source
            })
            .map(|texture| {
                let path = texture
                    .logical_path
                    .as_ref()
                    .ok_or(())?
                    .to_ascii_lowercase();
                cached_runtime_texture(&path, decoders, textures)
            })
            .transpose()?
    } else {
        None
    };
    Ok(playsrc_map::RuntimeMaterial {
        logical_path: identity.to_owned(),
        shader: shader_code(material.shader),
        features: feature_bits(&material.features),
        texture_role: selected_role.map_or(0, texture_role),
        base_texture: base,
        second_texture,
        detail,
    })
}

fn cached_runtime_texture(
    path: &str,
    decoders: &TextureDecoders<'_>,
    textures: &mut BTreeMap<String, playsrc_map::RuntimeTexture>,
) -> Result<playsrc_map::RuntimeTexture, ()> {
    if let Some(texture) = textures.get(path) {
        return Ok(texture.clone());
    }
    let texture = runtime_texture(path, decoders)?;
    textures.insert(path.to_owned(), texture.clone());
    Ok(texture)
}

fn runtime_texture(
    path: &str,
    decoders: &TextureDecoders<'_>,
) -> Result<playsrc_map::RuntimeTexture, ()> {
    let metadata = decoders.metadata(path)?;
    Ok(playsrc_map::RuntimeTexture {
        logical_path: path.to_owned(),
        width: metadata.width,
        height: metadata.height,
    })
}

fn resolve_material_semantics(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    environment: playsrc_material::SelectionEnvironment,
) -> Result<playsrc_material::Material, ()> {
    let root = *bundle.get(identity).ok_or(())?;
    let mut responses = Vec::new();
    let material = loop {
        match playsrc_vmt::compose(
            root,
            identity.to_owned(),
            &responses,
            &playsrc_keyvalues::ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|_| ())?
        {
            playsrc_vmt::Composition::Complete(document) => {
                break playsrc_material::resolve_for_environment(&document, environment)
                    .map_err(|_| ())?;
            }
            playsrc_vmt::Composition::Needs(requests) => {
                for request in requests {
                    let path = dependency_path(&request.target_token)?;
                    responses.push(playsrc_vmt::DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path.clone(),
                        bytes: Some(bundle.get(&path).ok_or(())?.to_vec()),
                    });
                }
            }
        }
    };
    Ok(material)
}

fn shader_code(shader: playsrc_material::Shader) -> u8 {
    match shader {
        playsrc_material::Shader::LightmappedGeneric => 1,
        playsrc_material::Shader::VertexLitGeneric => 2,
        playsrc_material::Shader::UnlitGeneric => 3,
        playsrc_material::Shader::UnlitTwoTexture => 10,
        playsrc_material::Shader::WorldVertexTransition => 4,
        playsrc_material::Shader::Water => 5,
        playsrc_material::Shader::Refract => 6,
        playsrc_material::Shader::Sprite => 7,
        playsrc_material::Shader::SkyHdr => 8,
        playsrc_material::Shader::SkyLdr => 9,
        playsrc_material::Shader::Unsupported => 255,
    }
}

fn feature_bits(features: &playsrc_material::Features) -> u8 {
    u8::from(features.translucent)
        | (u8::from(features.additive) << 1)
        | (u8::from(features.alpha_test) << 2)
        | (u8::from(features.no_cull) << 3)
        | (u8::from(features.self_illum) << 4)
        | (u8::from(features.ss_bump) << 5)
}

fn material_environment(
    profile: playsrc_map::LightingProfile,
    model: bool,
) -> playsrc_material::SelectionEnvironment {
    playsrc_material::SelectionEnvironment {
        hdr_mode: if profile == playsrc_map::LightingProfile::Hdr {
            playsrc_material::HdrMode::Integer
        } else {
            playsrc_material::HdrMode::None
        },
        model,
        ..playsrc_material::SelectionEnvironment::default()
    }
}

fn texture_role(role: playsrc_material::TextureRole) -> u8 {
    match role {
        playsrc_material::TextureRole::Base => 0,
        playsrc_material::TextureRole::HdrBase => 1,
        playsrc_material::TextureRole::HdrCompressed => 2,
        playsrc_material::TextureRole::HdrCompressed0 => 3,
        playsrc_material::TextureRole::HdrCompressed1 => 4,
        playsrc_material::TextureRole::HdrCompressed2 => 5,
        _ => 255,
    }
}

fn resolve_profile_materials(
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    profile: playsrc_map::LightingProfile,
) -> Result<Vec<playsrc_map::RuntimeProfileMaterial>, ()> {
    if profile != playsrc_map::LightingProfile::Hdr {
        return Ok(Vec::new());
    }
    let world = graph
        .entities
        .iter()
        .find(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"worldspawn"))
        })
        .ok_or(())?;
    let sky = world
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(b"skyname"))
        .ok_or(())?;
    let sky = std::str::from_utf8(&sky.value).map_err(|_| ())?;
    if sky.is_empty()
        || !sky
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(());
    }
    ["rt", "lf", "bk", "ft", "up", "dn"]
        .into_iter()
        .map(|suffix| {
            let identity = format!("materials/skybox/{sky}_hdr{suffix}.vmt").to_ascii_lowercase();
            let material =
                resolve_material_semantics(&identity, bundle, material_environment(profile, false))
                    .map_err(|_| ())?;
            let role = *material.selected_textures.first().ok_or(())?;
            let texture = material
                .textures
                .iter()
                .find(|texture| texture.role == role)
                .ok_or(())?;
            let path = texture
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            let bytes = decoders.bytes(&path)?;
            let metadata = decoders.metadata(&path)?;
            if metadata.frame_count != 1
                || metadata.depth != 1
                || metadata.width == 0
                || metadata.height == 0
                || metadata.width > 4_096
                || metadata.height > 4_096
            {
                return Err(());
            }
            let source_sha256 = *resource_hashes.get(&path).ok_or(())?;
            Ok(playsrc_map::RuntimeProfileMaterial {
                logical_path: identity,
                shader: shader_code(material.shader),
                features: feature_bits(&material.features),
                texture_role: texture_role(role),
                texture: playsrc_map::RuntimeProfileTexture {
                    logical_path: path,
                    width: metadata.width,
                    height: metadata.height,
                    format: metadata.high_format.code(),
                    source_sha256,
                    source_bytes: bytes.to_vec(),
                },
            })
        })
        .collect()
}

fn runtime_inputs(
    bundle: &BTreeMap<String, &[u8]>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
) -> Vec<playsrc_map::RuntimeInput> {
    bundle
        .keys()
        .map(|logical_path| playsrc_map::RuntimeInput {
            role: 1,
            logical_path: logical_path.clone(),
            sha256: resource_hashes[logical_path],
        })
        .collect()
}

fn entity_vector(entity: &playsrc_entity::Entity, key: &[u8]) -> Result<[f32; 3], ()> {
    let Some(pair) = entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
    else {
        return Ok([0.; 3]);
    };
    let values = std::str::from_utf8(&pair.value)
        .map_err(|_| ())?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return Err(());
    }
    Ok([values[0], values[1], values[2]])
}

fn entity_scalar<'a>(entity: &'a playsrc_entity::Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn selected_sky_encoding(
    selected: &[playsrc_material::TextureRole],
) -> Option<playsrc_map::SkyEncoding> {
    match selected {
        [playsrc_material::TextureRole::Base] => Some(playsrc_map::SkyEncoding::Srgb),
        [playsrc_material::TextureRole::HdrBase] => Some(playsrc_map::SkyEncoding::Linear),
        [playsrc_material::TextureRole::HdrCompressed] => Some(playsrc_map::SkyEncoding::HdrRgbs),
        _ => None,
    }
}

fn collision_object_templates(
    map: &playsrc_map::CanonicalMap,
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    studio_model_checksums: &BTreeMap<String, i32>,
) -> Result<Vec<CollisionObjectTemplate>, ()> {
    const CONTENTS_SOLID: u32 = 0x1;
    let limits = playsrc_collision::SnapshotLimits::default();
    let mut output = Vec::new();
    let mut physics_shapes = BTreeMap::new();
    for entity in &graph.entities {
        let identity = u64::try_from(entity.index).map_err(|_| ())?;
        if identity == 0 {
            continue;
        }
        let transform = playsrc_collision::Transform {
            origin: entity_vector(entity, b"origin")?,
            angles: entity_vector(entity, b"angles")?,
        };
        if let Some(model) = entity.model.as_deref()
            && let Some(index) = model.strip_prefix(b"*")
        {
            let classname = entity.classname.as_deref().ok_or(())?;
            let water_volume = classname.eq_ignore_ascii_case(b"func_water")
                || classname.eq_ignore_ascii_case(b"func_water_analog");
            let ordinary_mover = classname.eq_ignore_ascii_case(b"func_door")
                || classname.eq_ignore_ascii_case(b"func_button")
                || classname.eq_ignore_ascii_case(b"func_movelinear")
                || classname.eq_ignore_ascii_case(b"func_tracktrain")
                || water_volume;
            let brush = classname.eq_ignore_ascii_case(b"func_brush");
            if !ordinary_mover && !brush {
                continue;
            }
            let model = std::str::from_utf8(index)
                .map_err(|_| ())?
                .parse::<usize>()
                .map_err(|_| ())?;
            output.push(CollisionObjectTemplate {
                input: playsrc_collision::ObjectInput {
                    identity,
                    role: playsrc_collision::ObjectRole::Entity,
                    enabled: ordinary_mover
                        || entity_scalar(entity, b"Solidity") != Some(b"1".as_slice())
                            && entity_scalar(entity, b"StartDisabled") != Some(b"1".as_slice()),
                    volume_contents: water_volume,
                    transform,
                    linear_velocity: [0.0; 3],
                    angular_velocity: [0.0; 3],
                    collision_group: 0,
                    contents: 0,
                    surface_flags: 0,
                    shape: playsrc_collision::SnapshotShape::BrushModel { model },
                },
                runtime_transform: true,
            });
            continue;
        }
        if !entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
            || entity_scalar(entity, b"solid") != Some(b"6".as_slice())
        {
            continue;
        }
        let model = std::str::from_utf8(entity.model.as_deref().ok_or(())?)
            .map_err(|_| ())?
            .to_ascii_lowercase();
        let phy_path = model.strip_suffix(".mdl").ok_or(())?.to_owned() + ".phy";
        let Some(bytes) = bundle.get(&phy_path).copied() else {
            continue;
        };
        let shape = if let Some(shape) = physics_shapes.get(&model) {
            Arc::clone(shape)
        } else {
            let asset = playsrc_phy::parse_standalone(
                bytes,
                playsrc_phy::Profile::SourcePcPolygon,
                playsrc_phy::Limits::default(),
            )
            .map_err(|_| ())?;
            if asset.header.as_ref().map(|header| header.checksum)
                != Some(*studio_model_checksums.get(&model).ok_or(())?)
            {
                return Err(());
            }
            let shape_identity =
                u64::from_le_bytes(Sha256::digest(bytes)[..8].try_into().map_err(|_| ())?);
            let shape = Arc::new(
                playsrc_collision::PhysicsShape::from_phy(
                    shape_identity,
                    &asset,
                    0,
                    limits,
                    |_| CONTENTS_SOLID,
                )
                .map_err(|_| ())?,
            );
            physics_shapes.insert(model, Arc::clone(&shape));
            shape
        };
        output.push(CollisionObjectTemplate {
            input: playsrc_collision::ObjectInput {
                identity,
                role: playsrc_collision::ObjectRole::Entity,
                enabled: true,
                volume_contents: false,
                transform,
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: CONTENTS_SOLID,
                surface_flags: 0,
                shape: playsrc_collision::SnapshotShape::Physics(shape),
            },
            runtime_transform: false,
        });
    }
    for prop in &map.static_props.occurrences {
        if prop.solidity == 0 {
            continue;
        }
        if prop.solidity != 6 {
            return Err(());
        }
        let model = &map
            .static_props
            .models
            .get(prop.model)
            .ok_or(())?
            .logical_path;
        let phy_path = model.strip_suffix(".mdl").ok_or(())?.to_owned() + ".phy";
        let Some(bytes) = bundle.get(&phy_path).copied() else {
            continue;
        };
        let shape = if let Some(shape) = physics_shapes.get(model) {
            Arc::clone(shape)
        } else {
            let asset = playsrc_phy::parse_standalone(
                bytes,
                playsrc_phy::Profile::SourcePcPolygon,
                playsrc_phy::Limits::default(),
            )
            .map_err(|_| ())?;
            let identity =
                u64::from_le_bytes(Sha256::digest(bytes)[..8].try_into().map_err(|_| ())?);
            let shape = Arc::new(
                playsrc_collision::PhysicsShape::from_phy(identity, &asset, 0, limits, |_| {
                    CONTENTS_SOLID
                })
                .map_err(|_| ())?,
            );
            physics_shapes.insert(model.clone(), Arc::clone(&shape));
            shape
        };
        output.push(CollisionObjectTemplate {
            input: playsrc_collision::ObjectInput {
                identity: 0x8000_0000_0000_0000u64 | u64::try_from(prop.source).map_err(|_| ())?,
                role: playsrc_collision::ObjectRole::StaticProp,
                enabled: true,
                volume_contents: false,
                transform: playsrc_collision::Transform {
                    origin: prop.origin,
                    angles: prop.angles,
                },
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: CONTENTS_SOLID,
                surface_flags: 0,
                shape: playsrc_collision::SnapshotShape::Physics(shape),
            },
            runtime_transform: false,
        });
    }
    Ok(output)
}

fn compile_collision_snapshot(
    world: &playsrc_collision::World,
    templates: &[CollisionObjectTemplate],
    revision: u64,
    latest: Option<&playsrc_tf2::Snapshot>,
    transform_overrides: &BTreeMap<u64, playsrc_collision::Transform>,
    velocity_overrides: &BTreeMap<u64, [f32; 3]>,
) -> Result<playsrc_collision::Snapshot, playsrc_collision::Error> {
    let runtime_transforms = latest
        .into_iter()
        .flat_map(|snapshot| &snapshot.entity_transforms)
        .map(|value| {
            (
                u64::from(value.identity),
                playsrc_collision::Transform {
                    origin: value.position,
                    angles: value.angles,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let inputs = templates
        .iter()
        .map(|template| {
            let mut input = template.input.clone();
            if template.runtime_transform
                && let Some(transform) = runtime_transforms.get(&input.identity)
            {
                input.transform = *transform;
            }
            if let Some(transform) = transform_overrides.get(&input.identity) {
                input.transform = *transform;
            }
            if let Some(velocity) = velocity_overrides.get(&input.identity) {
                input.linear_velocity = *velocity;
            }
            input
        })
        .collect();
    playsrc_collision::Snapshot::compile(
        world,
        revision,
        inputs,
        playsrc_collision::SnapshotLimits::default(),
    )
}

fn resolve_models(
    graph: &playsrc_entity::Graph,
    studio_models: &BTreeMap<String, playsrc_studio_model::PresentationModel>,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    profile: playsrc_map::LightingProfile,
    static_props: &playsrc_map::StaticProps,
) -> Result<
    (
        Vec<playsrc_map::RuntimeModel>,
        Vec<playsrc_map::RuntimeModelOccurrence>,
    ),
    (),
> {
    if bundle.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut models = Vec::new();
    let mut indexes = BTreeMap::new();
    let mut material_cache = BTreeMap::<String, playsrc_map::RuntimeMaterial>::new();
    for (identity, model) in studio_models {
        let mut materials = Vec::new();
        for material in &model.materials {
            let path = model
                .dependencies
                .get(material.material_dependency)
                .ok_or(())?
                .logical_path
                .to_ascii_lowercase();
            if let Some(material) = material_cache.get(&path) {
                materials.push(material.clone());
            } else {
                let mut material = resolve_material(
                    &path,
                    bundle,
                    decoders,
                    false,
                    material_environment(profile, true),
                )?;
                material.base_texture = None;
                material_cache.insert(path, material.clone());
                materials.push(material);
            }
        }
        if model.skins.is_empty() || model.sequences.is_empty() {
            return Err(());
        }
        indexes.insert(identity.clone(), models.len());
        let bodygroups = model.body_parts.iter().map(|_| 0).collect::<Vec<_>>();
        let pose_parameters = model
            .pose_parameters
            .iter()
            .map(|_| playsrc_studio_model::Float32(0))
            .collect::<Vec<_>>();
        let pose = playsrc_studio_model::sample_pose(
            model,
            &playsrc_studio_model::AnimationState {
                base_sequence: 0,
                cycle: playsrc_studio_model::Float32(0),
                pose_parameters,
                layers: Vec::new(),
            },
        )
        .map_err(|_| ())?;
        let mut selected_skins = std::collections::BTreeSet::from([0usize]);
        if model.skins.len() > 1 {
            selected_skins.insert(1);
        }
        for prop in &static_props.occurrences {
            if static_props
                .models
                .get(prop.model)
                .is_some_and(|entry| entry.logical_path == *identity)
            {
                selected_skins.insert(playsrc_studio_model::source_skin_family(
                    prop.skin,
                    model.skins.len(),
                ));
            }
        }
        for skin_index in selected_skins {
            if skin_index >= model.skins.len() {
                return Err(());
            }
            let mut primitives = Vec::new();
            for selected in
                playsrc_studio_model::select_primitives(model, &bodygroups, skin_index, 0)
                    .map_err(|_| ())?
            {
                let primitive = model.geometry.get(selected.primitive).ok_or(())?;
                let (positions, normals, _) = posed_vertices(model, primitive, &pose)?;
                primitives.push(playsrc_map::RuntimeModelPrimitive {
                    material: selected.material,
                    positions,
                    normals,
                    uv: primitive
                        .vertices
                        .iter()
                        .map(|vertex| vertex.uv.map(|value| f32::from_bits(value.0)))
                        .collect(),
                    triangles: primitive.triangles.clone(),
                });
            }
            models.push(playsrc_map::RuntimeModel {
                logical_path: if skin_index == 0 {
                    identity.clone()
                } else {
                    format!("{identity}#skin={skin_index}")
                },
                materials: materials.clone(),
                primitives,
            });
        }
    }
    let mut occurrences = Vec::new();
    for entity in &graph.entities {
        if !entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
        {
            continue;
        }
        let identity = std::str::from_utf8(entity.model.as_deref().ok_or(())?)
            .map_err(|_| ())?
            .to_ascii_lowercase();
        occurrences.push(playsrc_map::RuntimeModelOccurrence {
            entity: entity.index,
            model: *indexes.get(&identity).ok_or(())?,
            position: entity_vector(entity, b"origin")?,
            angles: entity_vector(entity, b"angles")?,
        });
    }
    Ok((models, occurrences))
}

type PosedVertices = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>);

fn posed_vertices(
    model: &playsrc_studio_model::PresentationModel,
    primitive: &playsrc_studio_model::GeometryPrimitive,
    pose: &playsrc_studio_model::SampledPose,
) -> Result<PosedVertices, ()> {
    let static_root = matches!(
        model.descriptor,
        playsrc_studio_model::PresentationDescriptor::World {
            root_bone: playsrc_studio_model::RootBoneContract::StaticPropBoneZeroIsEntity,
            ..
        }
    );
    let mut positions = Vec::with_capacity(primitive.vertices.len());
    let mut normals = Vec::with_capacity(primitive.vertices.len());
    let mut tangents = Vec::with_capacity(primitive.vertices.len());
    for vertex in &primitive.vertices {
        let source_position = vertex.position.0.map(|value| f32::from_bits(value.0));
        let source_normal = vertex.normal.0.map(|value| f32::from_bits(value.0));
        let source_tangent = vertex.tangent.map(|value| f32::from_bits(value.0));
        let mut position = [0.0; 3];
        let mut normal = [0.0; 3];
        let mut tangent = [0.0; 3];
        for influence in 0..usize::from(vertex.bone_count) {
            let weight = f32::from_bits(vertex.weights[influence].0);
            let bone = usize::from(vertex.bones[influence]);
            let identity = playsrc_studio_model::Matrix3x4(
                [
                    1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                ]
                .map(|value| playsrc_studio_model::Float32(value.to_bits())),
            );
            let matrix = if static_root && bone == 0 {
                &identity
            } else {
                pose.skinning_matrices.get(bone).ok_or(())?
            };
            let transformed_position = transform_point(matrix, source_position);
            let transformed_normal = transform_normal(matrix, source_normal);
            let transformed_tangent = transform_normal(
                matrix,
                [source_tangent[0], source_tangent[1], source_tangent[2]],
            );
            for axis in 0..3 {
                position[axis] = transformed_position[axis].mul_add(weight, position[axis]);
                normal[axis] = transformed_normal[axis].mul_add(weight, normal[axis]);
                tangent[axis] = transformed_tangent[axis].mul_add(weight, tangent[axis]);
            }
        }
        positions.push(position);
        normals.push(normal);
        tangents.push([tangent[0], tangent[1], tangent[2], source_tangent[3]]);
    }
    Ok((positions, normals, tangents))
}

fn build_model_presentation(
    identity: &str,
    bundle: &BTreeMap<String, &[u8]>,
    model_resources: &BTreeMap<String, Arc<[u8]>>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    profile: playsrc_map::LightingProfile,
    presentation_profile: playsrc_studio_model::PresentationProfile,
) -> Result<Box<CompiledPresentationModel>, ()> {
    let built = playsrc_tf2::presentation::build_model(
        identity,
        bundle,
        model_resources,
        resource_hashes,
        profile == playsrc_map::LightingProfile::Hdr,
        presentation_profile,
    )
    .map_err(|_| ())?;
    let model = built.model;
    let mut digest = Sha256::new();
    digest.update(b"playsrc-tf2-presentation-model-1\0");
    digest.update(model.identity.as_bytes());
    digest.update(model.checksum.to_le_bytes());
    digest.update([match model.profile {
        playsrc_studio_model::PresentationProfile::World => 0,
        playsrc_studio_model::PresentationProfile::ViewModel => 1,
    }]);
    for dependency in &model.dependencies {
        digest.update((dependency.logical_path.len() as u64).to_le_bytes());
        digest.update(dependency.logical_path.as_bytes());
        if let Some(sha256) = dependency.sha256 {
            digest.update([1]);
            digest.update(sha256);
        } else {
            digest.update([0]);
        }
        digest.update((dependency.byte_length as u64).to_le_bytes());
    }
    Ok(Box::new(CompiledPresentationModel {
        model: *model,
        identity: digest.finalize().into(),
        illumination_position: built.illumination_position,
        illumination_attachment: built.illumination_attachment,
    }))
}

fn transform_point(matrix: &playsrc_studio_model::Matrix3x4, point: [f32; 3]) -> [f32; 3] {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    [
        m[0].mul_add(
            point[0],
            m[1].mul_add(point[1], m[2].mul_add(point[2], m[3])),
        ),
        m[4].mul_add(
            point[0],
            m[5].mul_add(point[1], m[6].mul_add(point[2], m[7])),
        ),
        m[8].mul_add(
            point[0],
            m[9].mul_add(point[1], m[10].mul_add(point[2], m[11])),
        ),
    ]
}
fn transform_normal(matrix: &playsrc_studio_model::Matrix3x4, normal: [f32; 3]) -> [f32; 3] {
    let m = matrix.0.map(|value| f32::from_bits(value.0));
    [
        m[0].mul_add(normal[0], m[1].mul_add(normal[1], m[2] * normal[2])),
        m[4].mul_add(normal[0], m[5].mul_add(normal[1], m[6] * normal[2])),
        m[8].mul_add(normal[0], m[9].mul_add(normal[1], m[10] * normal[2])),
    ]
}
fn pbytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ()> {
    let length = u32::try_from(bytes.len()).map_err(|_| ())?;
    let required = bytes.len().checked_add(4).ok_or(())?;
    let total = out.len().checked_add(required).ok_or(())?;
    if total > PRESENTATION_OUTPUT_LIMIT {
        return Err(());
    }
    if out.capacity().saturating_sub(out.len()) < required {
        out.try_reserve_exact(required).map_err(|_| ())?;
    }
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

struct CompiledStaticProps {
    bytes: Vec<u8>,
    section: static_prop_artifact::Section,
    aggregate_object_count: usize,
}

fn compile_static_prop_section(
    canonical: &playsrc_map::CanonicalMap,
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    visibility: &playsrc_visibility::World,
    collision: &playsrc_collision::World,
    environment: &RuntimeEnvironment,
    models: &[(String, Box<CompiledPresentationModel>)],
) -> Result<CompiledStaticProps, ()> {
    use static_prop_artifact::{Lighting, Occurrence, RuntimeLight, ViewOwnership};
    if canonical.static_props.occurrences.is_empty() {
        let section = static_prop_artifact::Section {
            aggregate_sha256: [0; 32],
            model_count: u32::try_from(models.len()).map_err(|_| ())?,
            occurrences: Vec::new(),
        };
        return Ok(CompiledStaticProps {
            bytes: static_prop_artifact::encode_section(&section)?,
            section,
            aggregate_object_count: 0,
        });
    }
    let aggregate = static_prop_artifact::decode_aggregate(
        bundle
            .get(static_prop_artifact::AGGREGATE_PATH)
            .copied()
            .ok_or(())?,
    )?;
    let object_indexes = static_prop_artifact::object_indexes(&aggregate)?;
    let model_indexes = models
        .iter()
        .enumerate()
        .map(|(index, (identity, artifact))| {
            Ok((
                identity.as_str(),
                (
                    u32::try_from(index).map_err(|_| ())?,
                    artifact.model.checksum,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ()>>()?;
    let checksums = model_indexes
        .iter()
        .map(|(identity, (_, checksum))| ((*identity).to_owned(), *checksum))
        .collect();
    let templates = collision_object_templates(canonical, graph, bundle, &checksums)?;
    let snapshot = compile_collision_snapshot(
        collision,
        &templates,
        1,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .map_err(|_| ())?;
    let water_materials = environment
        .world
        .water
        .surfaces
        .iter()
        .map(|surface| surface.material)
        .collect();
    let surface_world =
        playsrc_map::SurfaceLightingWorld::compile(canonical, visibility, water_materials)
            .map_err(|_| ())?;
    let sky_area =
        environment
            .world
            .controllers
            .iter()
            .find_map(|controller| match controller.state {
                playsrc_map::ControllerState::SkyCamera { area, .. } => u16::try_from(area).ok(),
                _ => None,
            });
    let runtime_props = canonical
        .static_props
        .occurrences
        .iter()
        .filter(|prop| {
            prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0
                || !object_indexes.contains_key(&(prop.source as u32, 0))
                || !object_indexes.contains_key(&(prop.source as u32, 1))
        })
        .collect::<Vec<_>>();
    let lighting_origin = |prop: &playsrc_map::StaticPropOccurrence| -> Result<[f32; 3], ()> {
        if let Some(origin) = prop.lighting_origin {
            return Ok(origin);
        }
        let identity = &canonical
            .static_props
            .models
            .get(prop.model)
            .ok_or(())?
            .logical_path;
        let (index, _) = *model_indexes.get(identity.as_str()).ok_or(())?;
        let model = &models.get(index as usize).ok_or(())?.1;
        let vector = |values: [f32; 3]| {
            playsrc_studio_model::Vector3(
                values.map(|value| playsrc_studio_model::Float32(value.to_bits())),
            )
        };
        let transform =
            playsrc_studio_model::source_entity_transform(vector(prop.origin), vector(prop.angles))
                .map_err(|_| ())?;
        playsrc_studio_model::source_model_lighting_origin(
            model.illumination_position,
            model.illumination_attachment,
            transform,
            None,
            identity,
        )
        .map(|origin| origin.0.map(|value| f32::from_bits(value.0)))
        .map_err(|_| ())
    };
    let mut direct_requests = Vec::new();
    let mut direct_candidates = Vec::new();
    let mut origin_query_identities = BTreeMap::new();
    for prop in &runtime_props {
        let origin = lighting_origin(prop)?;
        let origin_identity =
            0x4000_0000_0000_0000u64 | u64::try_from(prop.source).map_err(|_| ())?;
        origin_query_identities.insert(prop.source, origin_identity);
        direct_requests.push(playsrc_collision::LightingRay {
            identity: origin_identity,
            start: origin,
            end: origin,
            ignored_static_prop: Some(
                0x8000_0000_0000_0000u64 | u64::try_from(prop.source).map_err(|_| ())?,
            ),
        });
        let origin_leaf = visibility.locate_leaf(origin).map_err(|_| ())?;
        let origin_cluster = visibility.leaves.get(origin_leaf).ok_or(())?.cluster;
        if origin_cluster < 0 {
            return Err(());
        }
        for (light_index, light) in canonical.lighting.world_lights.iter().enumerate() {
            if light.kind == 5
                || (light.kind != 3
                    && (light.cluster < 0
                        || !visibility.visible(origin_cluster as usize, light.cluster as usize)))
            {
                continue;
            }
            let (end, direction, ratio) = direct_light_ray(origin, light)?;
            if ratio <= 0.0
                || (light.kind != 0
                    && light.intensity.into_iter().fold(0.0_f32, f32::max) * ratio < 0.0002)
            {
                continue;
            }
            let identity = (u64::try_from(prop.source).map_err(|_| ())? << 32)
                | u64::try_from(light_index).map_err(|_| ())?;
            direct_requests.push(playsrc_collision::LightingRay {
                identity,
                start: origin,
                end,
                ignored_static_prop: Some(
                    0x8000_0000_0000_0000u64 | u64::try_from(prop.source).map_err(|_| ())?,
                ),
            });
            direct_candidates.push((prop.source, light_index, identity, direction, ratio));
        }
    }
    let direct_batch = collision
        .trace_lighting_rays(
            &snapshot,
            u64::from_le_bytes(aggregate.sha256[..8].try_into().map_err(|_| ())?),
            playsrc_collision::LightingOccluders::WorldAndStaticProps,
            &direct_requests,
            playsrc_collision::LightingRayLimits {
                max_rays: direct_requests.len().max(1),
                max_output_bytes: 4 * 1024 * 1024,
            },
            |_| false,
        )
        .map_err(|_| ())?;
    let direct_results = direct_batch
        .rays
        .iter()
        .map(|result| (result.identity, result))
        .collect::<BTreeMap<_, _>>();
    let mut runtime_lighting = BTreeMap::new();
    for prop in runtime_props {
        let origin = lighting_origin(prop)?;
        let origin_trace = direct_results
            .get(origin_query_identities.get(&prop.source).ok_or(())?)
            .ok_or(())?;
        if origin_trace.trace.start_solid || origin_trace.trace.all_solid {
            return Err(());
        }
        let mut ambient_cube = surface_world
            .ambient_cube(
                origin,
                &static_prop_artifact::SOURCE_AMBIENT_DIRECTIONS,
                |_| false,
            )
            .map_err(|_| ())?;
        let mut selected = Vec::<(f32, RuntimeLight)>::new();
        for (_, light_index, identity, direction, ratio) in direct_candidates
            .iter()
            .filter(|(source, ..)| *source == prop.source)
        {
            let result = direct_results.get(identity).ok_or(())?;
            let light = canonical
                .lighting
                .world_lights
                .get(*light_index)
                .ok_or(())?;
            let distance = distance(
                origin,
                if light.kind == 3 {
                    result.trace.end
                } else {
                    light.origin
                },
            );
            let admitted = if light.kind == 3 {
                result.trace.surface_flags & 0x0004 != 0
            } else {
                (1.0 - result.trace.fraction) * distance <= 8.0
            };
            if !admitted {
                continue;
            }
            let angle = direct_light_angle(light, *direction)?;
            let illumination = ratio * dot(light.intensity, [0.299, 0.587, 0.114]);
            let record = RuntimeLight {
                source: u32::try_from(*light_index).map_err(|_| ())?,
                kind: light.kind,
                style: light.style,
                ratio: *ratio,
                direction: *direction,
                intensity: light.intensity,
            };
            if light.kind != 0 && illumination < 0.0002 {
                add_light_to_cube(
                    &mut ambient_cube,
                    *direction,
                    light.intensity,
                    ratio * angle,
                )?;
            } else if selected.len() < playsrc_studio_model::MAX_MODEL_LOCAL_LIGHTS {
                selected.push((illumination, record));
            } else if let Some((minimum, _)) = selected
                .iter()
                .enumerate()
                .min_by(|(_, left), (_, right)| left.0.total_cmp(&right.0))
                && illumination > selected[minimum].0
            {
                let (_, demoted) =
                    std::mem::replace(&mut selected[minimum], (illumination, record));
                let demoted_source = canonical
                    .lighting
                    .world_lights
                    .get(demoted.source as usize)
                    .ok_or(())?;
                add_light_to_cube(
                    &mut ambient_cube,
                    demoted.direction,
                    demoted.intensity,
                    demoted.ratio * direct_light_angle(demoted_source, demoted.direction)?,
                )?;
            } else {
                add_light_to_cube(
                    &mut ambient_cube,
                    *direction,
                    light.intensity,
                    ratio * angle,
                )?;
            }
        }
        let mut digest = Sha256::new();
        digest.update(b"playsrc-static-prop-runtime-lighting-v1\0");
        digest.update(canonical.lighting.closure_sha256);
        digest.update(collision.identity);
        digest.update(snapshot.identity().to_le_bytes());
        digest.update(u32::try_from(prop.source).map_err(|_| ())?.to_le_bytes());
        for value in origin {
            digest.update(value.to_le_bytes());
        }
        for value in ambient_cube.iter().flatten() {
            digest.update(value.to_le_bytes());
        }
        let lights = selected
            .into_iter()
            .map(|(_, light)| light)
            .collect::<Vec<_>>();
        for light in &lights {
            digest.update(light.source.to_le_bytes());
            digest.update(light.ratio.to_le_bytes());
        }
        runtime_lighting.insert(
            prop.source,
            Lighting::Runtime {
                sample_identity: digest.finalize().into(),
                ambient_cube,
                lights,
            },
        );
    }
    let mut occurrences = Vec::with_capacity(canonical.static_props.occurrences.len());
    for prop in &canonical.static_props.occurrences {
        let model = canonical.static_props.models.get(prop.model).ok_or(())?;
        let (presentation_model, checksum) =
            *model_indexes.get(model.logical_path.as_str()).ok_or(())?;
        let ldr = object_indexes
            .get(&(u32::try_from(prop.source).map_err(|_| ())?, 0))
            .copied();
        let hdr = object_indexes
            .get(&(u32::try_from(prop.source).map_err(|_| ())?, 1))
            .copied();
        let lighting = if prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0
            || ldr.is_none()
            || hdr.is_none()
        {
            if prop.flags & playsrc_bsp::STATIC_PROP_NO_PER_VERTEX_LIGHTING != 0
                && (ldr.is_some() || hdr.is_some())
            {
                return Err(());
            }
            runtime_lighting.remove(&prop.source).ok_or(())?
        } else {
            let ldr = ldr.ok_or(())?;
            let hdr = hdr.ok_or(())?;
            for index in [ldr, hdr] {
                let object = aggregate.objects.get(index as usize).ok_or(())?;
                if object.model as usize != prop.model
                    || object.meshes.is_empty()
                    || object.join_sha256 == [0; 32]
                {
                    return Err(());
                }
            }
            if aggregate.objects[hdr as usize].source_sha256 == [0; 32] || checksum == 0 {
                return Err(());
            }
            Lighting::Vertex { ldr, hdr }
        };
        let mut areas = Vec::with_capacity(prop.leaves.len());
        for leaf in &prop.leaves {
            let area = visibility
                .leaves
                .get(usize::from(*leaf))
                .ok_or(())?
                .area_and_flags
                & 0x01ff;
            areas.push(area);
        }
        let ownership = static_prop_artifact::classify_ownership(&areas, sky_area)?;
        let lod = match &lighting {
            Lighting::Vertex { hdr, .. } => {
                aggregate.objects[*hdr as usize]
                    .meshes
                    .first()
                    .ok_or(())?
                    .lod
            }
            Lighting::Runtime { .. } => models
                .get(presentation_model as usize)
                .and_then(|(_, artifact)| {
                    artifact
                        .model
                        .geometry
                        .iter()
                        .map(|primitive| primitive.lod)
                        .min()
                })
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(())?,
        };
        occurrences.push(Occurrence {
            source: u32::try_from(prop.source).map_err(|_| ())?,
            dictionary_model: u32::try_from(prop.model).map_err(|_| ())?,
            presentation_model,
            origin: prop.origin,
            angles: prop.angles,
            skin: i32::try_from(playsrc_studio_model::source_skin_family(
                prop.skin,
                models
                    .get(presentation_model as usize)
                    .ok_or(())?
                    .1
                    .model
                    .skins
                    .len(),
            ))
            .map_err(|_| ())?,
            body: 0,
            lod,
            fade_minimum: prop.fade_minimum,
            fade_maximum: prop.fade_maximum,
            forced_fade_scale: prop.forced_fade_scale,
            flags: prop.flags,
            solidity: prop.solidity,
            lighting_origin: if matches!(lighting, Lighting::Runtime { .. }) {
                Some(lighting_origin(prop)?)
            } else {
                prop.lighting_origin
            },
            leaves: prop.leaves.clone(),
            areas,
            ownership: match ownership {
                ViewOwnership::Main => ViewOwnership::Main,
                ViewOwnership::Sky3d => ViewOwnership::Sky3d,
            },
            lighting,
        });
    }
    if !runtime_lighting.is_empty() {
        return Err(());
    }
    let section = static_prop_artifact::Section {
        aggregate_sha256: aggregate.sha256,
        model_count: u32::try_from(models.len()).map_err(|_| ())?,
        occurrences,
    };
    let bytes = static_prop_artifact::encode_section(&section)?;
    if static_prop_artifact::decode_section(&bytes)? != section {
        return Err(());
    }
    Ok(CompiledStaticProps {
        bytes,
        section,
        aggregate_object_count: aggregate.objects.len(),
    })
}

fn direct_light_ray(
    origin: [f32; 3],
    light: &playsrc_map::WorldLight,
) -> Result<([f32; 3], [f32; 3], f32), ()> {
    if !(0..=5).contains(&light.kind) {
        return Err(());
    }
    if light.kind == 3 {
        let direction = light.normal.map(|value| -value);
        return Ok((
            add3(
                origin,
                scale3(direction, playsrc_map::SOURCE_AMBIENT_RAY_LENGTH),
            ),
            direction,
            1.0,
        ));
    }
    let delta = sub3(light.origin, origin);
    let distance_squared = dot(delta, delta);
    let distance = distance_squared.sqrt();
    if !distance.is_finite() || distance == 0.0 {
        return Err(());
    }
    let direction = scale3(delta, distance.recip());
    let (cache_minimum, cache_maximum) = lightcache_bounds(origin);
    let cache_delta = sub3(cache_maximum, origin);
    let sphere_radius = dot(cache_delta, cache_delta).sqrt();
    if matches!(light.kind, 1 | 2) {
        let closest = std::array::from_fn(|axis| {
            light.origin[axis].clamp(cache_minimum[axis], cache_maximum[axis])
        });
        let closest_delta = sub3(closest, light.origin);
        if dot(closest_delta, closest_delta) > light.radius * light.radius {
            return Ok((light.origin, direction, 0.0));
        }
    }
    if light.kind == 2 {
        let sine = (1.0 - light.stop_dot2 * light.stop_dot2).max(0.0).sqrt();
        if !sphere_intersects_cone(
            origin,
            sphere_radius,
            light.origin,
            light.normal,
            sine,
            light.stop_dot2,
        )? {
            return Ok((light.origin, direction, 0.0));
        }
    } else if light.kind == 0
        && (distance > sphere_radius + light.radius
            || !sphere_intersects_cone(
                origin,
                sphere_radius,
                light.origin,
                light.normal,
                1.0,
                0.0,
            )?)
    {
        return Ok((light.origin, direction, 0.0));
    }
    let ratio = match light.kind {
        0 => {
            if light.radius != 0.0 && distance > light.radius {
                0.0
            } else {
                distance_squared.max(1.0).recip()
            }
        }
        1 | 2 => (light.constant_attenuation
            + light.linear_attenuation * distance
            + light.quadratic_attenuation * distance_squared)
            .recip(),
        4 => (light.linear_attenuation - distance).max(0.0),
        _ => 0.0,
    };
    if !ratio.is_finite() {
        return Err(());
    }
    Ok((light.origin, direction, ratio))
}

fn lightcache_bounds(origin: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let sizes = [32.0, 32.0, 128.0];
    let minimum = std::array::from_fn(|axis| {
        let cell = (origin[axis].abs() as i32) / sizes[axis] as i32;
        if origin[axis] >= 0.0 {
            cell as f32 * sizes[axis]
        } else {
            -((cell + 1) as f32) * sizes[axis]
        }
    });
    let maximum = std::array::from_fn(|axis| minimum[axis] + sizes[axis]);
    (minimum, maximum)
}

fn sphere_intersects_cone(
    center: [f32; 3],
    radius: f32,
    origin: [f32; 3],
    normal: [f32; 3],
    sine: f32,
    cosine: f32,
) -> Result<bool, ()> {
    if !sine.is_finite() || sine <= 0.0 {
        return Err(());
    }
    let back = sub3(origin, scale3(normal, radius / sine));
    let delta = sub3(center, back);
    let length = dot(delta, delta).sqrt();
    if dot(normal, delta) >= length * cosine {
        let delta = sub3(center, origin);
        let length = dot(delta, delta).sqrt();
        if -dot(normal, delta) >= length * sine {
            return Ok(length <= radius);
        }
        return Ok(true);
    }
    Ok(false)
}

fn direct_light_angle(light: &playsrc_map::WorldLight, direction: [f32; 3]) -> Result<f32, ()> {
    let value = match light.kind {
        0 => (-dot(direction, light.normal)).max(0.0),
        1 | 4 => 1.0,
        2 => {
            let cone = -dot(direction, light.normal);
            if cone <= light.stop_dot2 {
                0.0
            } else if cone >= light.stop_dot {
                1.0
            } else {
                let value = (cone - light.stop_dot2) / (light.stop_dot - light.stop_dot2);
                if light.exponent == 0.0 || light.exponent == 1.0 {
                    value
                } else {
                    value.powf(light.exponent)
                }
            }
        }
        3 => 1.0,
        _ => 0.0,
    };
    value.is_finite().then_some(value).ok_or(())
}

fn add_light_to_cube(
    cube: &mut [[f32; 3]; 6],
    direction: [f32; 3],
    intensity: [f32; 3],
    ratio: f32,
) -> Result<(), ()> {
    for (face, axis) in cube.iter_mut().zip([
        [1., 0., 0.],
        [-1., 0., 0.],
        [0., 1., 0.],
        [0., -1., 0.],
        [0., 0., 1.],
        [0., 0., -1.],
    ]) {
        let weight = dot(axis, direction);
        if weight > 0.0 {
            for channel in 0..3 {
                face[channel] += ratio * weight * intensity[channel];
                if !face[channel].is_finite() {
                    return Err(());
                }
            }
        }
    }
    Ok(())
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    dot(sub3(a, b), sub3(a, b)).sqrt()
}

fn encode_profile_coverage(
    lighting: &playsrc_map::LightingData,
    visibility: &playsrc_visibility::World,
    collision: &playsrc_collision::World,
    entities: &playsrc_entity::Graph,
) -> Result<Vec<u8>, ()> {
    if lighting.ambient_indexes.len() != visibility.leaves.len() {
        return Err(());
    }
    let mut records = Vec::new();
    for (leaf_index, (leaf, index)) in visibility
        .leaves
        .iter()
        .zip(&lighting.ambient_indexes)
        .enumerate()
    {
        if leaf.contents as u32 & playsrc_collision::CONTENTS_SOLID != 0
            || leaf.cluster < 0
            || index.sample_count == 0
        {
            continue;
        }
        let start = usize::from(index.first_sample);
        let end = start
            .checked_add(usize::from(index.sample_count))
            .ok_or(())?;
        let mut selected = None;
        for sample in lighting.ambient_samples.get(start..end).ok_or(())? {
            let position = std::array::from_fn(|axis| {
                let minimum = f32::from(leaf.mins[axis]);
                let maximum = f32::from(leaf.maxs[axis]);
                minimum + (maximum - minimum) * (f32::from(sample.position[axis]) / 255.0)
            });
            if visibility.locate_leaf(position).map_err(|_| ())? != leaf_index {
                continue;
            }
            let hull = playsrc_collision::Hull {
                mins: [0.0; 3],
                maxs: [0.0; 3],
            };
            if collision
                .trace_hull(
                    position,
                    position,
                    hull,
                    playsrc_collision::MASK_PLAYERSOLID,
                )
                .map_err(|_| ())?
                .start_solid
            {
                continue;
            }
            let mut entity_overlap = false;
            for entity in &entities.entities {
                if entity.index == 0 {
                    continue;
                }
                if !entity
                    .classname
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(b"trigger_teleport"))
                {
                    continue;
                }
                let Some(model) = entity
                    .model
                    .as_deref()
                    .and_then(|value| value.strip_prefix(b"*"))
                    .and_then(|value| std::str::from_utf8(value).ok())
                    .and_then(|value| value.parse::<usize>().ok())
                else {
                    continue;
                };
                if collision
                    .overlaps_model_hull(model, entity_vector(entity, b"origin")?, position, hull)
                    .map_err(|_| ())?
                {
                    entity_overlap = true;
                    break;
                }
            }
            if !entity_overlap {
                selected = Some(position);
                break;
            }
        }
        if let Some(position) = selected {
            records.push((
                leaf_index,
                leaf.cluster,
                leaf.area_and_flags & 0x01ff,
                position,
            ));
        }
    }
    let mut out = Vec::with_capacity(12 + records.len() * 24);
    out.extend_from_slice(b"PCOV");
    out.extend_from_slice(&1_u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(records.len()).map_err(|_| ())?.to_le_bytes());
    for (leaf, cluster, area, position) in records {
        out.extend_from_slice(&u32::try_from(leaf).map_err(|_| ())?.to_le_bytes());
        out.extend_from_slice(&cluster.to_le_bytes());
        out.extend_from_slice(&area.to_le_bytes());
        for value in position {
            out.extend_from_slice(&value.to_le_bytes())
        }
        out.extend_from_slice(&0_u32.to_le_bytes())
    }
    Ok(out)
}

fn selected_texture<'material, 'source, 'cache>(
    material: &'material playsrc_material::Material,
    decoders: &'cache TextureDecoders<'source>,
) -> Result<
    (
        &'material playsrc_material::TextureRequest,
        &'source [u8],
        &'cache playsrc_vtf::Metadata,
    ),
    (),
> {
    let selected = material
        .selected_textures
        .first()
        .and_then(|role| {
            material
                .textures
                .iter()
                .find(|texture| texture.role == *role)
        })
        .filter(|texture| texture.disposition == playsrc_material::TextureDisposition::Source)
        .ok_or(())?;
    let logical_path = selected
        .logical_path
        .as_ref()
        .ok_or(())?
        .to_ascii_lowercase();
    Ok((
        selected,
        decoders.bytes(&logical_path)?,
        decoders.metadata(&logical_path)?,
    ))
}

fn encode_one_material_state(
    out: &mut Vec<u8>,
    identity: &str,
    material: &playsrc_material::Material,
    decoders: &TextureDecoders<'_>,
) -> Result<(), ()> {
    let metadata = selected_texture(material, decoders)
        .ok()
        .map(|(_, _, metadata)| metadata);
    let state = playsrc_material::static_state(
        material,
        playsrc_material::TextureAlphaFacts {
            base: metadata.as_ref().is_some_and(|metadata| {
                metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit
            }),
        },
    )
    .map_err(|_| ())?;
    encode_resolved_material_state(out, identity, &state, metadata)
}

fn encode_resolved_material_state(
    out: &mut Vec<u8>,
    identity: &str,
    state: &playsrc_material::StaticState,
    metadata: Option<&playsrc_vtf::Metadata>,
) -> Result<(), ()> {
    let sampling = metadata.map(|metadata| {
        playsrc_vtf::sampling_state(
            metadata,
            playsrc_vtf::SamplingEnvironment {
                shader_model: 90,
                force_anisotropy: 1,
                maximum_anisotropy: 16,
                force_trilinear: false,
            },
        )
    });
    pbytes(out, identity.as_bytes())?;
    out.extend_from_slice(&[
        state.lighting as u8,
        u8::from(state.blend.enabled),
        state.blend.source as u8,
        state.blend.destination as u8,
        u8::from(state.alpha_test),
        state.cull as u8,
        u8::from(state.depth_test),
        u8::from(state.depth_write),
        state.depth_function as u8,
        state.polygon_offset as u8,
        state.fog as u8,
        u8::from(state.wireframe),
        u8::from(state.no_draw),
        u8::from(state.vertex_color),
        u8::from(state.vertex_alpha),
        u8::from(state.translucent_queue),
        sampling.map_or(u8::MAX, |value| value.wrap_s as u8),
        sampling.map_or(u8::MAX, |value| value.wrap_t as u8),
        sampling.map_or(u8::MAX, |value| value.wrap_u as u8),
        sampling.map_or(u8::MAX, |value| value.min_filter as u8),
        sampling.map_or(u8::MAX, |value| value.mag_filter as u8),
        sampling.map_or(u8::MAX, |value| u8::from(value.mipmapped)),
        sampling.map_or(u8::MAX, |value| u8::from(value.no_lod)),
        sampling.map_or(u8::MAX, |value| u8::from(value.all_mips)),
    ]);
    out.extend_from_slice(&state.alpha_test_reference.to_le_bytes());
    out.extend_from_slice(&state.alpha_modulation.to_le_bytes());
    let ownership = u16::from(state.alpha_ownership.base_texture_available)
        | (u16::from(state.alpha_ownership.opacity) << 1)
        | (u16::from(state.alpha_ownership.alpha_test) << 2)
        | (u16::from(state.alpha_ownership.self_illumination_mask) << 3)
        | (u16::from(state.alpha_ownership.environment_mask) << 4)
        | (u16::from(state.alpha_ownership.phong_mask) << 5)
        | (u16::from(state.alpha_ownership.tint_mask) << 6)
        | (u16::from(state.alpha_ownership.vertex_alpha) << 7)
        | (u16::from(state.alpha_ownership.material_alpha_modulation) << 8);
    out.extend_from_slice(&ownership.to_le_bytes());
    let (discard, source, pass, reference) = match state.fragment_discard {
        playsrc_material::FragmentDiscardRequirement::None => (0, 0, 0, 0.0),
        playsrc_material::FragmentDiscardRequirement::Alpha {
            source,
            pass,
            reference,
        } => (
            1,
            match source {
                playsrc_material::FragmentAlphaSource::BaseTextureOrOne => 0,
                playsrc_material::FragmentAlphaSource::ShaderOutput => 1,
            },
            match pass {
                playsrc_material::CompareFunction::Greater => 0,
                playsrc_material::CompareFunction::GreaterOrEqual => 1,
            },
            reference,
        ),
    };
    out.extend_from_slice(&[discard, source, pass, 0]);
    out.extend_from_slice(&reference.to_le_bytes());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_material_states(
    out: &mut Vec<u8>,
    canonical: &playsrc_map::CanonicalMap,
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    profile: playsrc_map::LightingProfile,
    models: &[(String, Box<CompiledPresentationModel>)],
    map_materials: &[playsrc_material::Material],
    model_materials: &PreparedModelMaterials,
    particle_materials: &BTreeMap<String, CompiledParticlePresentation>,
) -> Result<(), ()> {
    let mut targets = BTreeMap::<String, (String, bool)>::new();
    for material in &canonical.materials {
        let identity = material.logical_path.to_ascii_lowercase();
        insert_material_state_target(&mut targets, identity.clone(), identity, false)?;
    }
    for entity in &graph.entities {
        if entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"infodecal"))
            && let Some(reference) = entity
                .pairs
                .iter()
                .find(|pair| pair.key.eq_ignore_ascii_case(b"texture"))
        {
            let path = dependency_path(&reference.value)?;
            if bundle.contains_key(&path) {
                insert_material_state_target(&mut targets, path.clone(), path, false)?;
            }
        }
    }
    for (_, artifact) in models {
        for material in &artifact.model.materials {
            let path = artifact
                .model
                .dependencies
                .get(material.material_dependency)
                .ok_or(())?
                .logical_path
                .to_ascii_lowercase();
            insert_material_state_target(&mut targets, path.clone(), path, true)?;
        }
    }
    for (identity, particle) in particle_materials {
        insert_material_state_target(
            &mut targets,
            identity.clone(),
            particle.source_path.clone(),
            false,
        )?;
    }
    let start = out.len();
    out.extend_from_slice(b"PMST");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(targets.len()).map_err(|_| ())?.to_le_bytes());
    for (identity, (source, model)) in targets {
        if let Some(particle) = particle_materials.get(&identity) {
            encode_resolved_material_state(
                out,
                &identity,
                &particle.state,
                Some(&particle.metadata),
            )?;
        } else {
            let owned;
            let material = if model {
                let index = model_materials
                    .materials
                    .binary_search_by(|(path, _)| path.as_str().cmp(&source))
                    .map_err(|_| ())?;
                &model_materials.materials[index].1
            } else if let Some(index) = canonical
                .materials
                .iter()
                .position(|reference| reference.logical_path.eq_ignore_ascii_case(&source))
            {
                map_materials.get(index).ok_or(())?
            } else {
                owned = resolve_material_semantics(
                    &source,
                    bundle,
                    material_environment(profile, false),
                )?;
                &owned
            };
            encode_one_material_state(out, &identity, material, decoders)?;
        }
    }
    (out.len() - start <= 4 * 1024 * 1024)
        .then_some(())
        .ok_or(())
}

fn insert_material_state_target(
    targets: &mut BTreeMap<String, (String, bool)>,
    identity: String,
    source: String,
    model: bool,
) -> Result<(), ()> {
    if let Some(existing) = targets.get(&identity) {
        return (existing == &(source, model)).then_some(()).ok_or(());
    }
    if targets
        .values()
        .any(|(existing_source, _)| existing_source == &source)
    {
        return Err(());
    }
    targets.insert(identity, (source, model));
    Ok(())
}

#[derive(Clone)]
struct EncodedTexturePlane {
    identity: playsrc_material::TextureSubresourceIdentity,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[derive(Clone)]
struct EncodedAuthoredTexture {
    source_sha256: [u8; 32],
    manifest: playsrc_material::TextureMetadataManifest,
    scalar_encoding: playsrc_vtf::ScalarEncoding,
    planes: Vec<EncodedTexturePlane>,
}

#[derive(Clone)]
struct ReferencedTexturePlane {
    identity: playsrc_material::TextureSubresourceIdentity,
    width: u32,
    height: u32,
    range: std::ops::Range<usize>,
}

#[derive(Clone)]
struct ReferencedAuthoredTexture {
    source_sha256: [u8; 32],
    manifest: playsrc_material::TextureMetadataManifest,
    format: i32,
    planes: Vec<ReferencedTexturePlane>,
}

#[derive(Clone)]
enum ModelAuthoredTexture {
    Decoded(EncodedAuthoredTexture),
    Referenced(ReferencedAuthoredTexture),
}

impl ModelAuthoredTexture {
    fn manifest(&self) -> &playsrc_material::TextureMetadataManifest {
        match self {
            Self::Decoded(texture) => &texture.manifest,
            Self::Referenced(texture) => &texture.manifest,
        }
    }
}

fn material_texture_manifest(
    metadata: &playsrc_vtf::Metadata,
) -> playsrc_material::TextureMetadataManifest {
    let environment = playsrc_vtf::SamplingEnvironment {
        shader_model: 90,
        force_anisotropy: 1,
        maximum_anisotropy: 16,
        force_trilinear: false,
    };
    let sampling = playsrc_vtf::sampling_state(metadata, environment);
    let face = |value| match value {
        playsrc_vtf::Face::Right => playsrc_material::TextureFace::Right,
        playsrc_vtf::Face::Left => playsrc_material::TextureFace::Left,
        playsrc_vtf::Face::Back => playsrc_material::TextureFace::Back,
        playsrc_vtf::Face::Front => playsrc_material::TextureFace::Front,
        playsrc_vtf::Face::Up => playsrc_material::TextureFace::Up,
        playsrc_vtf::Face::Down => playsrc_material::TextureFace::Down,
        playsrc_vtf::Face::Sphere => playsrc_material::TextureFace::Sphere,
    };
    playsrc_material::TextureMetadataManifest {
        width: metadata.width,
        height: metadata.height,
        depth: metadata.depth,
        mip_count: metadata.mip_count,
        frame_count: metadata.frame_count,
        faces: metadata.faces.iter().copied().map(face).collect(),
        sampling: playsrc_material::TextureSamplingState {
            wrap_s: match sampling.wrap_s {
                playsrc_vtf::WrapMode::Repeat => playsrc_material::TextureWrapMode::Repeat,
                playsrc_vtf::WrapMode::Clamp => playsrc_material::TextureWrapMode::Clamp,
                playsrc_vtf::WrapMode::Border => playsrc_material::TextureWrapMode::Border,
            },
            wrap_t: match sampling.wrap_t {
                playsrc_vtf::WrapMode::Repeat => playsrc_material::TextureWrapMode::Repeat,
                playsrc_vtf::WrapMode::Clamp => playsrc_material::TextureWrapMode::Clamp,
                playsrc_vtf::WrapMode::Border => playsrc_material::TextureWrapMode::Border,
            },
            wrap_u: match sampling.wrap_u {
                playsrc_vtf::WrapMode::Repeat => playsrc_material::TextureWrapMode::Repeat,
                playsrc_vtf::WrapMode::Clamp => playsrc_material::TextureWrapMode::Clamp,
                playsrc_vtf::WrapMode::Border => playsrc_material::TextureWrapMode::Border,
            },
            min_filter: match sampling.min_filter {
                playsrc_vtf::MinFilter::Nearest => playsrc_material::TextureMinFilter::Nearest,
                playsrc_vtf::MinFilter::Linear => playsrc_material::TextureMinFilter::Linear,
                playsrc_vtf::MinFilter::LinearMipmapNearest => {
                    playsrc_material::TextureMinFilter::LinearMipmapNearest
                }
                playsrc_vtf::MinFilter::LinearMipmapLinear => {
                    playsrc_material::TextureMinFilter::LinearMipmapLinear
                }
                playsrc_vtf::MinFilter::Anisotropic => {
                    playsrc_material::TextureMinFilter::Anisotropic
                }
            },
            mag_filter: match sampling.mag_filter {
                playsrc_vtf::MagFilter::Nearest => playsrc_material::TextureMagFilter::Nearest,
                playsrc_vtf::MagFilter::Linear => playsrc_material::TextureMagFilter::Linear,
                playsrc_vtf::MagFilter::Anisotropic => {
                    playsrc_material::TextureMagFilter::Anisotropic
                }
            },
            anisotropy_level: if sampling.min_filter == playsrc_vtf::MinFilter::Anisotropic
                || sampling.mag_filter == playsrc_vtf::MagFilter::Anisotropic
            {
                (environment.maximum_anisotropy / 4).clamp(2, 8)
            } else {
                1
            },
            mipmapped: sampling.mipmapped,
            no_lod: sampling.no_lod,
            all_mips: sampling.all_mips,
        },
        subresources: metadata
            .subresources
            .iter()
            .filter_map(|subresource| match subresource.identity {
                playsrc_vtf::SubresourceIdentity::LowResolution => None,
                playsrc_vtf::SubresourceIdentity::HighResolution {
                    mip,
                    frame,
                    face: value,
                    slice,
                } => Some(playsrc_material::TextureSubresourceIdentity {
                    mip,
                    frame,
                    face: face(value),
                    slice,
                }),
            })
            .collect(),
    }
}

fn vtf_subresource(
    identity: playsrc_material::TextureSubresourceIdentity,
) -> playsrc_vtf::SubresourceIdentity {
    playsrc_vtf::SubresourceIdentity::HighResolution {
        mip: identity.mip,
        frame: identity.frame,
        face: match identity.face {
            playsrc_material::TextureFace::Right => playsrc_vtf::Face::Right,
            playsrc_material::TextureFace::Left => playsrc_vtf::Face::Left,
            playsrc_material::TextureFace::Back => playsrc_vtf::Face::Back,
            playsrc_material::TextureFace::Front => playsrc_vtf::Face::Front,
            playsrc_material::TextureFace::Up => playsrc_vtf::Face::Up,
            playsrc_material::TextureFace::Down => playsrc_vtf::Face::Down,
            playsrc_material::TextureFace::Sphere => playsrc_vtf::Face::Sphere,
        },
        slice: identity.slice,
    }
}

fn authored_texture(
    path: &str,
    decoder: &playsrc_vtf::Decoder<'_>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
) -> Result<EncodedAuthoredTexture, ()> {
    let manifest = material_texture_manifest(decoder.metadata());
    let mut evidence = Vec::with_capacity(manifest.subresources.len());
    let mut planes = Vec::with_capacity(manifest.subresources.len());
    let mut scalar_encoding = None;
    for identity in &manifest.subresources {
        let plane = decoder.decode(vtf_subresource(*identity)).map_err(|_| ())?;
        if plane.row_order != playsrc_vtf::RowOrder::TopToBottom
            || scalar_encoding.is_some_and(|value| value != plane.scalar_encoding)
        {
            return Err(());
        }
        scalar_encoding = Some(plane.scalar_encoding);
        evidence.push(playsrc_material::AuthoredTexturePlane {
            identity: *identity,
            width: plane.width,
            height: plane.height,
            row_stride: plane.row_stride,
            sample_bytes: plane.samples.len(),
        });
        let component_bytes = match plane.scalar_encoding {
            playsrc_vtf::ScalarEncoding::U8 => 1,
            playsrc_vtf::ScalarEncoding::F16 => 2,
        };
        let rgba = match plane.channel_layout {
            playsrc_vtf::ChannelLayout::Rgba => plane.samples,
            playsrc_vtf::ChannelLayout::Rgb => {
                let mut output = Vec::with_capacity(
                    plane.width as usize * plane.height as usize * 4 * component_bytes,
                );
                for pixel in plane.samples.chunks_exact(3 * component_bytes) {
                    output.extend_from_slice(pixel);
                    match plane.scalar_encoding {
                        playsrc_vtf::ScalarEncoding::U8 => output.push(255),
                        playsrc_vtf::ScalarEncoding::F16 => {
                            output.extend_from_slice(&0x3c00_u16.to_le_bytes())
                        }
                    }
                }
                output
            }
        };
        if rgba.len() != plane.width as usize * plane.height as usize * 4 * component_bytes {
            return Err(());
        }
        planes.push(EncodedTexturePlane {
            identity: *identity,
            width: plane.width,
            height: plane.height,
            rgba,
        });
    }
    let placeholder = playsrc_material::TextureRequest {
        role: playsrc_material::TextureRole::Base,
        parameter: b"$basetexture".to_vec(),
        reference: path.as_bytes().to_vec(),
        logical_path: Some(path.to_owned()),
        disposition: playsrc_material::TextureDisposition::Source,
        color_read: playsrc_material::TextureColorRead::Srgb,
    };
    let binding =
        playsrc_material::bind_authored_texture(&placeholder, &manifest).map_err(|_| ())?;
    playsrc_material::validate_authored_planes(&binding, &evidence).map_err(|_| ())?;
    Ok(EncodedAuthoredTexture {
        source_sha256: *resource_hashes.get(path).ok_or(())?,
        manifest,
        scalar_encoding: scalar_encoding.ok_or(())?,
        planes,
    })
}

fn model_authored_texture(
    path: &str,
    decoders: &TextureDecoders<'_>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    allow_channel_order: bool,
) -> Result<ModelAuthoredTexture, ()> {
    let decoder = decoders.decoder(path)?;
    let metadata = decoder.metadata();
    let direct = matches!(
        metadata.high_format,
        playsrc_vtf::ImageFormat::Rgba8888 | playsrc_vtf::ImageFormat::Rgba16F
    );
    let two_dimensional = metadata.faces.len() == 1 && metadata.depth == 1;
    let converted_or_compressed = matches!(
        metadata.high_format,
        playsrc_vtf::ImageFormat::Dxt1
            | playsrc_vtf::ImageFormat::Dxt1OneBitAlpha
            | playsrc_vtf::ImageFormat::Dxt3
            | playsrc_vtf::ImageFormat::Dxt5
    ) || allow_channel_order
        && matches!(
            metadata.high_format,
            playsrc_vtf::ImageFormat::Abgr8888
                | playsrc_vtf::ImageFormat::Argb8888
                | playsrc_vtf::ImageFormat::Bgra8888
                | playsrc_vtf::ImageFormat::Bgrx8888
        );
    if direct || two_dimensional && converted_or_compressed {
        let manifest = material_texture_manifest(metadata);
        let planes = metadata
            .subresources
            .iter()
            .filter_map(|subresource| match subresource.identity {
                playsrc_vtf::SubresourceIdentity::LowResolution => None,
                playsrc_vtf::SubresourceIdentity::HighResolution {
                    mip,
                    frame,
                    face,
                    slice,
                } => Some(ReferencedTexturePlane {
                    identity: playsrc_material::TextureSubresourceIdentity {
                        mip,
                        frame,
                        face: match face {
                            playsrc_vtf::Face::Right => playsrc_material::TextureFace::Right,
                            playsrc_vtf::Face::Left => playsrc_material::TextureFace::Left,
                            playsrc_vtf::Face::Back => playsrc_material::TextureFace::Back,
                            playsrc_vtf::Face::Front => playsrc_material::TextureFace::Front,
                            playsrc_vtf::Face::Up => playsrc_material::TextureFace::Up,
                            playsrc_vtf::Face::Down => playsrc_material::TextureFace::Down,
                            playsrc_vtf::Face::Sphere => playsrc_material::TextureFace::Sphere,
                        },
                        slice,
                    },
                    width: subresource.width,
                    height: subresource.height,
                    range: subresource.encoded_range.clone(),
                }),
            })
            .collect();
        return Ok(ModelAuthoredTexture::Referenced(
            ReferencedAuthoredTexture {
                source_sha256: *resource_hashes.get(path).ok_or(())?,
                manifest,
                format: metadata.high_format.code(),
                planes,
            },
        ));
    }
    authored_texture(path, decoder, resource_hashes).map(ModelAuthoredTexture::Decoded)
}

fn texture_role_code(role: playsrc_material::TextureRole) -> u8 {
    role as u8
}

fn model_texture_role_code(role: playsrc_material::ModelTextureRole) -> u8 {
    role as u8
}

fn color_read_code(value: playsrc_material::TextureColorRead) -> u8 {
    match value {
        playsrc_material::TextureColorRead::Srgb => 0,
        playsrc_material::TextureColorRead::Linear => 1,
        playsrc_material::TextureColorRead::FormatDependent => 2,
    }
}

struct PreparedModelMaterials {
    materials: Vec<(String, playsrc_material::Material)>,
    textures: BTreeMap<String, ModelAuthoredTexture>,
    material_section: Vec<u8>,
}

fn prepare_model_materials(
    models: &[(String, Box<CompiledPresentationModel>)],
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    profile: playsrc_map::LightingProfile,
) -> Result<PreparedModelMaterials, ()> {
    let mut identities = std::collections::BTreeSet::new();
    for (_, artifact) in models {
        for material in &artifact.model.materials {
            let Some(dependency) = artifact
                .model
                .dependencies
                .get(material.material_dependency)
            else {
                return Err(());
            };
            identities.insert(dependency.logical_path.to_ascii_lowercase());
        }
    }
    let materials = identities
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|identity| {
            let material =
                resolve_material_semantics(&identity, bundle, material_environment(profile, true))
                    .map_err(|_| ())?;
            material
                .model
                .is_some()
                .then_some((identity, material))
                .ok_or(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut texture_paths = std::collections::BTreeSet::new();
    for (_, material) in &materials {
        for texture in &material.textures {
            if texture.disposition != playsrc_material::TextureDisposition::Source {
                continue;
            }
            let Some(logical_path) = texture.logical_path.as_ref() else {
                return Err(());
            };
            texture_paths.insert(logical_path.to_ascii_lowercase());
        }
        for texture in &material.model_textures {
            texture_paths.insert(texture.logical_path.to_ascii_lowercase());
        }
    }
    let textures = texture_paths
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|path| {
            Ok((
                path.clone(),
                model_authored_texture(&path, decoders, resource_hashes, true).map_err(|_| ())?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ()>>()?;
    for (_, material) in &materials {
        for texture in &material.textures {
            if texture.disposition != playsrc_material::TextureDisposition::Source {
                continue;
            }
            let path = texture
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            let resource = textures.get(&path).ok_or(())?;
            let binding = playsrc_material::bind_authored_texture(texture, resource.manifest())
                .map_err(|_| ())?;
            if !binding.logical_path.eq_ignore_ascii_case(&path) {
                return Err(());
            }
        }
        for texture in &material.model_textures {
            let path = texture.logical_path.to_ascii_lowercase();
            let resource = textures.get(&path).ok_or(())?;
            let binding =
                playsrc_material::bind_authored_model_texture(texture, resource.manifest())
                    .map_err(|_| ())?;
            if !binding.logical_path.eq_ignore_ascii_case(&path) {
                return Err(());
            }
        }
    }
    let mut material_section = Vec::new();
    material_section.extend_from_slice(b"PMDL");
    material_section.extend_from_slice(&1_u32.to_le_bytes());
    material_section.extend_from_slice(
        &u32::try_from(materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, material) in &materials {
        encode_model_material(&mut material_section, identity, material, decoders)?;
    }
    Ok(PreparedModelMaterials {
        materials,
        textures,
        material_section,
    })
}

fn encode_model_materials(out: &mut Vec<u8>, prepared: &PreparedModelMaterials) -> Result<(), ()> {
    out.extend_from_slice(&prepared.material_section);
    out.extend_from_slice(b"PMIP");
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(prepared.textures.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, texture) in &prepared.textures {
        encode_model_authored_texture(out, identity, texture)?;
    }
    Ok(())
}

fn encode_texture_binding(
    out: &mut Vec<u8>,
    kind: u8,
    role: u8,
    color_read: playsrc_material::TextureColorRead,
    path: &str,
) -> Result<(), ()> {
    out.extend_from_slice(&[kind, role, color_read_code(color_read), 0]);
    pbytes(out, path.as_bytes())
}

fn encode_model_material(
    out: &mut Vec<u8>,
    identity: &str,
    material: &playsrc_material::Material,
    decoders: &TextureDecoders<'_>,
) -> Result<(), ()> {
    use playsrc_material::ModelShaderState as State;
    let model = material.model.as_ref().ok_or(())?;
    pbytes(out, identity.as_bytes())?;
    let requirements = model.vertex_requirements;
    let requirement_bits = u16::from(requirements.position)
        | u16::from(requirements.normal) << 1
        | u16::from(requirements.tangent_space) << 2
        | u16::from(requirements.texture_coordinate_0) << 3
        | u16::from(requirements.ambient_cube) << 4
        | u16::from(requirements.local_lights) << 5
        | u16::from(requirements.camera_position) << 6
        | u16::from(requirements.studio_eye_parameters) << 7;
    out.extend_from_slice(&[
        match model.shader {
            playsrc_material::ModelShader::UnlitGeneric => 3,
            playsrc_material::ModelShader::UnlitTwoTexture => 4,
            playsrc_material::ModelShader::VertexLitGeneric => 0,
            playsrc_material::ModelShader::EyeRefract => 1,
            playsrc_material::ModelShader::Eyes => 2,
        },
        0,
    ]);
    out.extend_from_slice(&requirement_bits.to_le_bytes());
    let source_textures = material
        .textures
        .iter()
        .filter(|texture| texture.disposition == playsrc_material::TextureDisposition::Source)
        .collect::<Vec<_>>();
    out.extend_from_slice(
        &u32::try_from(source_textures.len() + material.model_textures.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for texture in source_textures {
        encode_texture_binding(
            out,
            0,
            texture_role_code(texture.role),
            texture.color_read,
            texture.logical_path.as_ref().ok_or(())?,
        )?;
    }
    for texture in &material.model_textures {
        encode_texture_binding(
            out,
            1,
            model_texture_role_code(texture.role),
            texture.color_read,
            &texture.logical_path,
        )?;
    }
    if let Some(environment) = &material.environment_map {
        out.extend_from_slice(&[1, 0, 0, 0]);
        for value in environment
            .tint
            .into_iter()
            .chain([environment.contrast, environment.saturation])
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
    } else {
        out.extend_from_slice(&[0; 24]);
    }
    match &model.state {
        State::UnlitGeneric(_) => {}
        State::UnlitTwoTexture(state) => {
            out.extend_from_slice(&[
                u8::from(state.second_frame_rate.is_some()),
                u8::from(state.second_scroll_rate.is_some()),
                0,
                0,
            ]);
            for value in [
                state.second_frame_rate.unwrap_or(0.0),
                state.second_scroll_rate.unwrap_or(0.0),
                state.second_scroll_angle.unwrap_or(0.0),
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        State::VertexLitGeneric(state) => {
            let self_illumination = state.self_illumination;
            out.extend_from_slice(&[
                u8::from(state.half_lambert),
                u8::from(self_illumination.is_some()),
                self_illumination.map_or(0, |value| value.source as u8),
                u8::from(state.phong.is_some()),
            ]);
            let self_illumination = self_illumination.map_or([0.0; 6], |value| {
                [
                    value.tint[0],
                    value.tint[1],
                    value.tint[2],
                    value.fresnel_min_max_exponent[0],
                    value.fresnel_min_max_exponent[1],
                    value.fresnel_min_max_exponent[2],
                ]
            });
            for value in self_illumination {
                out.extend_from_slice(&value.to_le_bytes());
            }
            if let Some(phong) = &state.phong {
                out.extend_from_slice(&[
                    phong.mask_source as u8,
                    u8::from(phong.invert_mask),
                    u8::from(phong.albedo_tint),
                    u8::from(phong.rim.is_some()),
                ]);
                for value in [phong.exponent, phong.exponent_factor]
                    .into_iter()
                    .chain(phong.tint)
                    .chain([phong.boost])
                    .chain(phong.fresnel_ranges)
                    .chain(phong.packed_fresnel_ranges)
                {
                    out.extend_from_slice(&value.to_le_bytes());
                }
                if let Some(rim) = phong.rim {
                    for value in [rim.exponent, rim.boost] {
                        out.extend_from_slice(&value.to_le_bytes());
                    }
                    out.extend_from_slice(&[u8::from(rim.exponent_texture_alpha_mask), 0, 0, 0]);
                } else {
                    out.extend_from_slice(&[0; 12]);
                }
            } else {
                out.extend_from_slice(&[0; 64]);
            }
            encode_cloak(out, state.cloak);
            out.extend_from_slice(&[
                u8::from(state.sheen.enabled),
                u8::from(state.sheen.source_alpha_blend),
                u8::from(state.sheen.depth_write),
                0,
            ]);
            for value in [
                state.sheen.mask_frame,
                state.sheen.mask_direction,
                state.sheen.shader_index,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            for value in state
                .sheen
                .tint
                .into_iter()
                .chain(state.sheen.mask_scale)
                .chain(state.sheen.mask_offset)
            {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        State::EyeRefract(state) => {
            out.extend_from_slice(&[
                u8::from(state.sphere_texture_kill),
                u8::from(state.raytrace_sphere),
                u8::from(state.half_lambert),
                0,
            ]);
            for value in [
                state.dilation,
                state.glossiness,
                state.parallax_strength,
                state.cornea_bump_strength,
            ]
            .into_iter()
            .chain(state.ambient_occlusion_color)
            .chain([state.eyeball_radius])
            {
                out.extend_from_slice(&value.to_le_bytes());
            }
            encode_cloak(out, state.cloak);
        }
        State::Eyes(state) => {
            out.extend_from_slice(&[u8::from(state.half_lambert), 0, 0, 0]);
            out.extend_from_slice(&state.dilation.to_le_bytes());
        }
    }
    let alpha = selected_texture(material, decoders)
        .ok()
        .is_some_and(|(_, _, m)| m.alpha_flags.one_bit || m.alpha_flags.eight_bit);
    let draw = playsrc_material::model_draw_state(
        material,
        playsrc_material::TextureAlphaFacts { base: alpha },
        playsrc_material::ModelRuntimeInputs {
            alpha_modulation: 1.0,
            cloak_factor: Some(0.0),
        },
    )
    .map_err(|_| ())?;
    out.extend_from_slice(&[
        match draw.opacity {
            playsrc_material::ModelOpacity::Opaque => 0,
            playsrc_material::ModelOpacity::Translucent => 1,
        },
        match draw.framebuffer {
            playsrc_material::ModelFramebufferRequirement::None => 0,
            playsrc_material::ModelFramebufferRequirement::Potential => 1,
            playsrc_material::ModelFramebufferRequirement::Current => 2,
        },
        u8::try_from(draw.required_inputs.len()).map_err(|_| ())?,
        0,
    ]);
    for input in draw.required_inputs {
        out.push(match input {
            playsrc_material::ModelDrawInput::AmbientCube => 1,
            playsrc_material::ModelDrawInput::LocalLights => 2,
            playsrc_material::ModelDrawInput::CameraPosition => 3,
            playsrc_material::ModelDrawInput::StudioEyeParameters => 4,
            playsrc_material::ModelDrawInput::LocalEnvironment => 5,
            playsrc_material::ModelDrawInput::CurrentFramebuffer => 6,
            playsrc_material::ModelDrawInput::AuthoredTexturePlanes => 7,
            playsrc_material::ModelDrawInput::GameProxyValues => 8,
        })
    }
    Ok(())
}

fn encode_cloak(out: &mut Vec<u8>, cloak: playsrc_material::CloakState) {
    out.extend_from_slice(&[u8::from(cloak.enabled), 0, 0, 0]);
    for value in [cloak.factor]
        .into_iter()
        .chain(cloak.color_tint)
        .chain([cloak.refract_amount])
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn texture_face_code(value: playsrc_material::TextureFace) -> u8 {
    match value {
        playsrc_material::TextureFace::Right => 0,
        playsrc_material::TextureFace::Left => 1,
        playsrc_material::TextureFace::Back => 2,
        playsrc_material::TextureFace::Front => 3,
        playsrc_material::TextureFace::Up => 4,
        playsrc_material::TextureFace::Down => 5,
        playsrc_material::TextureFace::Sphere => 6,
    }
}

fn encode_model_authored_texture(
    out: &mut Vec<u8>,
    identity: &str,
    texture: &ModelAuthoredTexture,
) -> Result<(), ()> {
    let (source_sha256, manifest, scalar_encoding, plane_count) = match texture {
        ModelAuthoredTexture::Decoded(texture) => (
            texture.source_sha256,
            &texture.manifest,
            texture.scalar_encoding,
            texture.planes.len(),
        ),
        ModelAuthoredTexture::Referenced(texture) => (
            texture.source_sha256,
            &texture.manifest,
            if texture.format == playsrc_vtf::ImageFormat::Rgba16F.code() {
                playsrc_vtf::ScalarEncoding::F16
            } else {
                playsrc_vtf::ScalarEncoding::U8
            },
            texture.planes.len(),
        ),
    };
    pbytes(out, identity.as_bytes())?;
    out.extend_from_slice(&source_sha256);
    out.extend_from_slice(&manifest.width.to_le_bytes());
    out.extend_from_slice(&manifest.height.to_le_bytes());
    out.extend_from_slice(&manifest.depth.to_le_bytes());
    out.push(manifest.mip_count);
    out.push(match scalar_encoding {
        playsrc_vtf::ScalarEncoding::U8 => 0,
        playsrc_vtf::ScalarEncoding::F16 => 1,
    });
    out.extend_from_slice(&manifest.frame_count.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(manifest.faces.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for face in &manifest.faces {
        out.push(texture_face_code(*face));
    }
    let sampling = manifest.sampling;
    out.extend_from_slice(&[
        sampling.wrap_s as u8,
        sampling.wrap_t as u8,
        sampling.wrap_u as u8,
        sampling.min_filter as u8,
        sampling.mag_filter as u8,
        sampling.anisotropy_level,
        u8::from(sampling.mipmapped),
        u8::from(sampling.no_lod),
        u8::from(sampling.all_mips),
        0,
        0,
        0,
    ]);
    out.extend_from_slice(&u32::try_from(plane_count).map_err(|_| ())?.to_le_bytes());
    match texture {
        ModelAuthoredTexture::Decoded(texture) => {
            for plane in &texture.planes {
                out.extend_from_slice(&[
                    plane.identity.mip,
                    texture_face_code(plane.identity.face),
                ]);
                out.extend_from_slice(&plane.identity.frame.to_le_bytes());
                out.extend_from_slice(&plane.identity.slice.to_le_bytes());
                out.extend_from_slice(&0_u16.to_le_bytes());
                out.extend_from_slice(&plane.width.to_le_bytes());
                out.extend_from_slice(&plane.height.to_le_bytes());
                out.extend_from_slice(&[0, 0, 0, 0]);
                out.extend_from_slice(&(-1_i32).to_le_bytes());
                pbytes(out, &plane.rgba)?;
            }
        }
        ModelAuthoredTexture::Referenced(texture) => {
            for plane in &texture.planes {
                out.extend_from_slice(&[
                    plane.identity.mip,
                    texture_face_code(plane.identity.face),
                ]);
                out.extend_from_slice(&plane.identity.frame.to_le_bytes());
                out.extend_from_slice(&plane.identity.slice.to_le_bytes());
                out.extend_from_slice(&0_u16.to_le_bytes());
                out.extend_from_slice(&plane.width.to_le_bytes());
                out.extend_from_slice(&plane.height.to_le_bytes());
                out.extend_from_slice(&[1, 0, 0, 0]);
                out.extend_from_slice(&texture.format.to_le_bytes());
                out.extend_from_slice(
                    &u32::try_from(plane.range.start)
                        .map_err(|_| ())?
                        .to_le_bytes(),
                );
                out.extend_from_slice(
                    &u32::try_from(plane.range.len())
                        .map_err(|_| ())?
                        .to_le_bytes(),
                );
            }
        }
    }
    Ok(())
}

fn rgba_texture(path: &str, decoders: &TextureDecoders<'_>) -> Result<DecodedTexture, ()> {
    decoded_texture(path, decoders)
}

fn encode_particle_textures(
    out: &mut Vec<u8>,
    materials: &BTreeMap<String, CompiledParticlePresentation>,
) -> Result<(), ()> {
    out.extend_from_slice(b"PPTM");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, material) in materials {
        let texture = &material.texture;
        pbytes(out, identity.as_bytes())?;
        pbytes(out, material.source_path.as_bytes())?;
        pbytes(out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        out.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(texture.rgba.as_slice())));
        pbytes(out, &texture.rgba)?;
    }
    Ok(())
}

fn encode_sound_node(out: &mut Vec<u8>, node: &playsrc_keyvalues::Node) -> Result<(), ()> {
    pbytes(out, &node.key.bytes)?;
    match &node.value {
        playsrc_keyvalues::Value::Scalar(value) => {
            out.push(0);
            pbytes(out, &value.token.bytes)?;
        }
        playsrc_keyvalues::Value::Object(children) => {
            out.push(1);
            out.extend_from_slice(&u32::try_from(children.len()).map_err(|_| ())?.to_le_bytes());
            for child in children {
                encode_sound_node(out, child)?;
            }
        }
    }
    Ok(())
}

fn encode_audio_documents(out: &mut Vec<u8>, bundle: &BTreeMap<String, &[u8]>) -> Result<(), ()> {
    let weapon_targets: &[&str] = &[
        "Weapon_RPG.Single",
        "Weapon_QuakeRPG.Single",
        "Weapon_StickyBombLauncher.Single",
        "BaseExplosionEffect.Sound",
        "Weapon_QuakeRPG.Explode",
        "Weapon_Grenade_Pipebomb.Explode",
        "Weapon_Scatter_Gun.Single",
        "Weapon_Pistol.Single",
        "Weapon_Bat.Miss",
        "Weapon_Bat.HitFlesh",
        "Weapon_Bat.HitWorld",
        "Weapon_Scatter_Gun.WorldReload",
        "Weapon_Pistol.WorldReload",
        "Weapon_Shotgun.Single",
        "Weapon_Shotgun.WorldReload",
        "Weapon_Shovel.Miss",
        "Weapon_Shovel.HitFlesh",
        "Weapon_Shovel.HitWorld",
        "Weapon_Minigun.WindUp",
        "Weapon_Minigun.WindDown",
        "Weapon_Minigun.Spin",
        "Weapon_Minigun.Fire",
        "Weapon_Fist.Miss",
        "Weapon_Fist.HitWorld",
        "Weapon_Fist.HitFlesh",
        "Weapon_SniperRifle.Single",
        "Weapon_SMG.Single",
        "Weapon_Machete.Miss",
        "Weapon_Machete.HitFlesh",
        "Weapon_Machete.HitWorld",
        "Weapon_SMG.WorldReload",
        "Weapon_Shotgun.Empty",
        "Weapon_Pistol.ClipEmpty",
        "Weapon_Wrench.Miss",
        "Weapon_Wrench.HitFlesh",
        "Weapon_Wrench.HitWorld",
        "Weapon_FlameThrower.Fire",
        "Weapon_FlameThrower.FireLoop",
        "Weapon_FlameThrower.WindDown",
        "Weapon_FlameThrower.AirBurstAttack",
        "Weapon_FireAxe.Miss",
        "Weapon_FireAxe.HitFlesh",
        "Weapon_FireAxe.HitWorld",
        "Weapon_Bottle.Miss",
        "Weapon_Bottle.HitFlesh",
        "Weapon_Bottle.HitWorld",
        "Weapon_Revolver.Single",
        "Weapon_Revolver.WorldReload",
        "Weapon_Knife.Miss",
        "Weapon_Knife.HitFlesh",
        "Weapon_Knife.HitWorld",
    ];
    let flag_targets: &[&str] = &[
        "CaptureFlag.EnemyStolen",
        "CaptureFlag.EnemyDropped",
        "CaptureFlag.EnemyCaptured",
        "CaptureFlag.EnemyReturned",
        "CaptureFlag.TeamStolen",
        "CaptureFlag.TeamDropped",
        "CaptureFlag.TeamCaptured",
        "CaptureFlag.TeamReturned",
        "CaptureFlag.FlagSpawn",
    ];
    let round_targets: &[&str] = &["Game.YourTeamWon", "Game.YourTeamLost"];
    let player_targets: &[&str] = &["Player.Spy_Cloak", "Player.Spy_UnCloak"];
    let mut documents = vec![
        ("scripts/game_sounds_weapons.txt", weapon_targets),
        ("scripts/game_sounds_player.txt", player_targets),
    ];
    if bundle.contains_key("scripts/game_sounds_vo.txt") {
        documents.push(("scripts/game_sounds_vo.txt", flag_targets));
        documents.push(("scripts/game_sounds.txt", round_targets));
    }
    let mixer = *bundle.get("scripts/soundmixers.txt").ok_or(())?;
    out.extend_from_slice(b"PAUD");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(mixer)));
    out.extend_from_slice(&0.72f32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(documents.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (logical_path, targets) in documents {
        let source = *bundle.get(logical_path).ok_or(())?;
        let document = playsrc_keyvalues::parse_text(
            source,
            playsrc_keyvalues::EscapeMode::LiteralBackslash,
            playsrc_keyvalues::Limits::default(),
        )
        .map_err(|_| ())?
        .evaluated(&playsrc_keyvalues::ConditionEnvironment::new([
            (b"$WIN32".to_vec(), true),
            (b"$X360".to_vec(), false),
        ]));
        pbytes(out, logical_path.as_bytes())?;
        out.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(source)));
        out.extend_from_slice(&u32::try_from(targets.len()).map_err(|_| ())?.to_le_bytes());
        for target in targets {
            let node = document
                .roots
                .iter()
                .find(|node| node.key.bytes.eq_ignore_ascii_case(target.as_bytes()))
                .ok_or(())?;
            encode_sound_node(out, node)?;
        }
    }
    Ok(())
}

fn encode_model_occurrence_matrices(
    out: &mut Vec<u8>,
    graph: &playsrc_entity::Graph,
) -> Result<(), ()> {
    let occurrences = graph
        .entities
        .iter()
        .filter(|entity| {
            entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
        })
        .collect::<Vec<_>>();
    out.extend_from_slice(b"PMTX");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(occurrences.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for entity in occurrences {
        let identity = std::str::from_utf8(entity.model.as_deref().ok_or(())?)
            .map_err(|_| ())?
            .to_ascii_lowercase();
        let source_vector = |value: [f32; 3]| {
            playsrc_studio_model::Vector3(
                value.map(|component| playsrc_studio_model::Float32(component.to_bits())),
            )
        };
        let matrix = playsrc_studio_model::source_entity_transform(
            source_vector(entity_vector(entity, b"origin")?),
            source_vector(entity_vector(entity, b"angles")?),
        )
        .map_err(|_| ())?;
        out.extend_from_slice(&entity.index.to_le_bytes());
        pbytes(out, identity.as_bytes())?;
        let integer = |key: &[u8]| -> Result<i32, ()> {
            entity_scalar(entity, key)
                .map(|value| {
                    std::str::from_utf8(value)
                        .map_err(|_| ())?
                        .parse()
                        .map_err(|_| ())
                })
                .transpose()
                .map(|value| value.unwrap_or(0))
        };
        out.extend_from_slice(&integer(b"skin")?.to_le_bytes());
        out.extend_from_slice(&integer(b"SetBodyGroup")?.to_le_bytes());
        for value in entity_vector(entity, b"origin")?
            .into_iter()
            .chain(entity_vector(entity, b"angles")?)
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in matrix.0 {
            out.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    Ok(())
}
struct DecodedTexture {
    logical_path: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decoded_texture(path: &str, decoders: &TextureDecoders<'_>) -> Result<DecodedTexture, ()> {
    let plane = decoders
        .decoder(path)?
        .decode(playsrc_vtf::SubresourceIdentity::HighResolution {
            mip: 0,
            frame: 0,
            face: playsrc_vtf::Face::Right,
            slice: 0,
        })
        .map_err(|_| ())?;
    let rgba = match plane.channel_layout {
        playsrc_vtf::ChannelLayout::Rgba => plane.samples,
        playsrc_vtf::ChannelLayout::Rgb => {
            let mut output = Vec::with_capacity(plane.width as usize * plane.height as usize * 4);
            for pixel in plane.samples.chunks_exact(3) {
                output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255])
            }
            output
        }
    };
    Ok(DecodedTexture {
        logical_path: path.to_owned(),
        width: plane.width,
        height: plane.height,
        rgba,
    })
}
type CompiledPresentation = (
    Vec<u8>,
    BTreeMap<String, playsrc_studio_model::PresentationModel>,
    BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    RuntimeEnvironment,
);
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationSizeLedger {
    pub model_count: usize,
    pub model_vertices: usize,
    pub model_triangles: usize,
    pub decoded_texture_count: usize,
    pub distinct_decoded_texture_count: usize,
    pub decoded_texture_bytes: usize,
    pub unique_decoded_texture_bytes: usize,
    pub repeated_decoded_texture_bytes: usize,
    pub source_texture_count: usize,
    pub distinct_source_texture_count: usize,
    pub source_texture_bytes: usize,
    pub unique_source_texture_bytes: usize,
    pub section_ends: [usize; 8],
    pub default_bound_first_exceeded_at: Option<usize>,
    pub final_length: usize,
    pub final_capacity: usize,
    pub phase_milliseconds: [u64; 6],
    pub displacement_input_count: usize,
    pub static_prop_collision_count: usize,
    pub static_prop_occurrence_count: usize,
    pub static_prop_vhv_object_count: usize,
    pub static_prop_runtime_lighting_count: usize,
    pub static_prop_section_bytes: usize,
    pub static_prop_section_sha256: [u8; 32],
    pub static_prop_runtime_sources: Vec<usize>,
    pub static_prop_runtime_light_records: Vec<(usize, usize, u8)>,
    pub static_prop_main_count: usize,
    pub static_prop_sky_count: usize,
    pub static_prop_vertex_lighting_count: usize,
}

type MeasuredPresentation = (CompiledPresentation, [u64; 6], PresentationSizeLedger);

fn presentation_cache_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, ()> {
    let end = offset.checked_add(4).ok_or(())?;
    let value = u32::from_le_bytes(
        bytes
            .get(*offset..end)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?,
    );
    *offset = end;
    Ok(value)
}

fn presentation_cache_skip(bytes: &[u8], offset: &mut usize, length: usize) -> Result<(), ()> {
    let end = offset.checked_add(length).ok_or(())?;
    bytes.get(*offset..end).ok_or(())?;
    *offset = end;
    Ok(())
}

fn cached_presentation_models(
    bytes: &[u8],
) -> Result<Vec<(String, playsrc_studio_model::PresentationProfile, [u8; 32])>, ()> {
    if bytes.len() < 24
        || &bytes[..4] != b"PTF2"
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ())?) != 13
        || static_prop_artifact::decode_section(static_prop_artifact::section_from_presentation(
            bytes,
        )?)
        .is_err()
    {
        return Err(());
    }
    let mut offset = 8;
    let model_count = presentation_cache_u32(bytes, &mut offset)? as usize;
    if model_count > 4_096 {
        return Err(());
    }
    presentation_cache_skip(bytes, &mut offset, 12)?;
    let mut models = Vec::with_capacity(model_count);
    for _ in 0..model_count {
        let identity = std::str::from_utf8(bundle_field(bytes, &mut offset)?)
            .map_err(|_| ())?
            .to_owned();
        let profile = *bytes.get(offset).ok_or(())?;
        if profile > 1 || bytes.get(offset + 1..offset + 4).ok_or(())? != [0, 0, 0] {
            return Err(());
        }
        offset += 4;
        let expected_hash: [u8; 32] = bytes
            .get(offset..offset + 32)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?;
        offset += 32;
        let _skin_count = presentation_cache_u32(bytes, &mut offset)?;
        let bodygroups = presentation_cache_u32(bytes, &mut offset)? as usize;
        presentation_cache_skip(bytes, &mut offset, bodygroups.checked_mul(4).ok_or(())?)?;
        let attachments = presentation_cache_u32(bytes, &mut offset)? as usize;
        for _ in 0..attachments {
            let _name = bundle_field(bytes, &mut offset)?;
            presentation_cache_skip(bytes, &mut offset, 48)?;
        }
        let sequences = presentation_cache_u32(bytes, &mut offset)? as usize;
        for _ in 0..sequences {
            let _label = bundle_field(bytes, &mut offset)?;
            let _activity = bundle_field(bytes, &mut offset)?;
            presentation_cache_skip(bytes, &mut offset, 28)?;
        }
        let descriptor = bytes.get(offset..offset + 4).ok_or(())?;
        if descriptor[0] != profile || descriptor[1] > 1 || descriptor[2] > 1 || descriptor[3] != 0
        {
            return Err(());
        }
        offset += 4;
        presentation_cache_skip(bytes, &mut offset, 40)?;
        if !bundle_field(bytes, &mut offset)?.is_empty() {
            return Err(());
        }
        models.push((
            identity,
            if profile == 0 {
                playsrc_studio_model::PresentationProfile::World
            } else {
                playsrc_studio_model::PresentationProfile::ViewModel
            },
            expected_hash,
        ));
    }
    Ok(models)
}

fn load_cached_presentation(
    inputs: PresentationInputs<'_, '_>,
    presentation: &[u8],
) -> Result<MeasuredPresentation, u32> {
    let PresentationInputs {
        canonical,
        bsp,
        graph,
        bundle,
        decoders,
        model_resources,
        resource_hashes,
        map_materials,
        particle_presentation: _,
        profile,
        visibility,
        collision,
        additional_model_roots,
    } = inputs;
    let mut metrics_clock = RuntimeMetricsClock::new();
    let mut phase_started =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    let mut metrics = [0_u64; 6];
    let mut phase_finished =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[0] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let model_headers = cached_presentation_models(presentation).map_err(|_| 2_u32)?;
    let expected = std::collections::BTreeSet::from([
        "models/weapons/w_models/w_rocket.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/c_models/c_soldier_arms.mdl".to_owned(),
        "models/weapons/c_models/c_demo_arms.mdl".to_owned(),
        "models/weapons/c_models/c_scout_arms.mdl".to_owned(),
        "models/weapons/c_models/c_engineer_arms.mdl".to_owned(),
        "models/weapons/c_models/c_sniper_arms.mdl".to_owned(),
        "models/weapons/c_models/c_spy_arms.mdl".to_owned(),
        "models/player/scout.mdl".to_owned(),
        "models/player/sniper.mdl".to_owned(),
        "models/player/soldier.mdl".to_owned(),
        "models/player/demo.mdl".to_owned(),
        "models/player/medic.mdl".to_owned(),
        "models/player/heavy.mdl".to_owned(),
        "models/player/pyro.mdl".to_owned(),
        "models/player/spy.mdl".to_owned(),
        "models/player/engineer.mdl".to_owned(),
        "models/vgui/ui_class01.mdl".to_owned(),
        "models/class_menu/random_class_icon.mdl".to_owned(),
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl".to_owned(),
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl".to_owned(),
        "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl".to_owned(),
        "models/weapons/c_models/c_bottle/c_bottle.mdl".to_owned(),
        "models/weapons/c_models/c_scattergun.mdl".to_owned(),
        "models/weapons/c_models/c_pistol/c_pistol.mdl".to_owned(),
        "models/weapons/c_models/c_bat.mdl".to_owned(),
        "models/weapons/c_models/c_shotgun/c_shotgun.mdl".to_owned(),
        "models/weapons/c_models/c_shovel/c_shovel.mdl".to_owned(),
        "models/weapons/c_models/c_wrench/c_wrench.mdl".to_owned(),
        "models/weapons/w_models/w_shotgun.mdl".to_owned(),
        "models/weapons/w_models/w_pistol.mdl".to_owned(),
        "models/weapons/w_models/w_wrench.mdl".to_owned(),
        "models/weapons/c_models/c_heavy_arms.mdl".to_owned(),
        "models/weapons/c_models/c_minigun/c_minigun.mdl".to_owned(),
        "models/weapons/c_models/c_sniperrifle/c_sniperrifle.mdl".to_owned(),
        "models/weapons/c_models/c_smg/c_smg.mdl".to_owned(),
        "models/weapons/c_models/c_machete/c_machete.mdl".to_owned(),
        "models/weapons/c_models/c_pyro_arms.mdl".to_owned(),
        "models/weapons/c_models/c_flamethrower/c_flamethrower.mdl".to_owned(),
        "models/weapons/c_models/c_fireaxe_pyro/c_fireaxe_pyro.mdl".to_owned(),
        "models/weapons/c_models/c_revolver/c_revolver.mdl".to_owned(),
        "models/weapons/c_models/c_knife/c_knife.mdl".to_owned(),
        "models/weapons/c_models/c_sapper/c_sapper.mdl".to_owned(),
        "models/weapons/v_models/v_pda_spy.mdl".to_owned(),
        "models/weapons/v_models/v_watch_spy.mdl".to_owned(),
    ]);
    let expected = graph
        .entities
        .iter()
        .try_fold(expected, |mut output, entity| {
            let classname = entity.classname.as_deref();
            let model = if classname
                .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
            {
                entity.model.as_deref()
            } else if classname.is_some_and(|value| value.eq_ignore_ascii_case(b"item_teamflag")) {
                Some(entity_scalar(entity, b"flag_model").unwrap_or(b"models/flag/briefcase.mdl"))
            } else {
                None
            };
            if let Some(model) = model {
                output.insert(
                    std::str::from_utf8(model)
                        .map_err(|_| 3_u32)?
                        .to_ascii_lowercase(),
                );
            }
            Ok::<_, u32>(output)
        })?;
    let expected = additional_model_roots
        .iter()
        .fold(expected, |mut roots, model| {
            roots.insert(model.clone());
            roots
        });
    let actual = model_headers
        .iter()
        .map(|(identity, _, _)| identity.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(3);
    }
    let models = model_headers
        .into_par_iter()
        .map(|(identity, presentation_profile, expected_hash)| {
            let artifact = build_model_presentation(
                &identity,
                bundle,
                model_resources,
                resource_hashes,
                profile,
                presentation_profile,
            )
            .map_err(|_| 4_u32)?;
            (artifact.identity == expected_hash)
                .then_some((identity, artifact))
                .ok_or(5_u32)
        })
        .collect::<Result<Vec<_>, _>>()?;
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[1] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let (_, environment) = compile_environment_artifact(
        canonical,
        bsp,
        graph,
        bundle,
        decoders,
        resource_hashes,
        map_materials,
        profile,
        visibility,
        collision,
    )
    .map_err(|_| 6_u32)?;
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[4] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let model_material_opacity =
        model_material_opacity(&models, bundle, decoders, profile, None).map_err(|_| 7_u32)?;
    let models: BTreeMap<String, playsrc_studio_model::PresentationModel> = models
        .into_iter()
        .map(|(identity, artifact)| (identity, artifact.model))
        .collect();
    let output = (
        presentation.to_vec(),
        models,
        model_material_opacity,
        environment,
    );
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[5] = phase_finished.saturating_sub(phase_started);
    Ok((output, metrics, PresentationSizeLedger::default()))
}

#[allow(clippy::too_many_arguments)]
fn presentation_capacity(
    models: &[(String, Box<CompiledPresentationModel>)],
    directional: &[(String, u8, DecodedTexture)],
    particles: &BTreeMap<String, CompiledParticlePresentation>,
    particle_materials: &[String],
    environment: &[u8],
    static_props: &[u8],
    model_materials: &PreparedModelMaterials,
    map: &playsrc_map::CanonicalMap,
    resource_count: usize,
) -> Result<usize, ()> {
    let mut bytes = 24usize;
    let mut add = |value: usize| -> Result<(), ()> {
        bytes = bytes.checked_add(value).ok_or(())?;
        Ok(())
    };
    for (identity, artifact) in models {
        let model = &artifact.model;
        add(96 + identity.len())?;
        add(model.body_parts.len().checked_mul(4).ok_or(())?)?;
        for attachment in &model.attachments {
            add(52 + attachment.name.len())?;
        }
        for sequence in &model.sequences {
            add(36 + sequence.label.len() + sequence.activity_name.len())?;
        }
    }
    for (identity, _, texture) in directional {
        add(76 + identity.len() + texture.logical_path.len())?;
        add(texture.rgba.len())?;
    }
    for identity in particle_materials {
        add(4 + identity.len())?;
    }
    add(4 + environment.len())?;
    add(12)?;
    for (identity, particle) in particles {
        add(56
            + identity.len()
            + particle.source_path.len()
            + particle.texture.logical_path.len())?;
        add(particle.texture.rgba.len())?;
    }
    for model in &map.brush_models {
        add(68)?;
        add(model.materials.len().checked_mul(4).ok_or(())?)?;
        add(model.entities.len().checked_mul(4).ok_or(())?)?;
    }
    add(static_props.len() + 8)?;
    add(model_materials.material_section.len())?;
    add(12)?;
    for (identity, texture) in &model_materials.textures {
        add(encoded_model_authored_texture_length(identity, texture)?)?;
    }
    add(resource_count.checked_mul(256).ok_or(())?)?;
    Ok(bytes.min(PRESENTATION_OUTPUT_LIMIT))
}

fn encoded_model_authored_texture_length(
    identity: &str,
    texture: &ModelAuthoredTexture,
) -> Result<usize, ()> {
    let manifest = texture.manifest();
    let mut length = 72usize
        .checked_add(identity.len())
        .and_then(|value| value.checked_add(manifest.faces.len()))
        .ok_or(())?;
    match texture {
        ModelAuthoredTexture::Decoded(texture) => {
            for plane in &texture.planes {
                length = length
                    .checked_add(28)
                    .and_then(|value| value.checked_add(plane.rgba.len()))
                    .ok_or(())?;
            }
        }
        ModelAuthoredTexture::Referenced(texture) => {
            length = length
                .checked_add(texture.planes.len().checked_mul(32).ok_or(())?)
                .ok_or(())?;
        }
    }
    Ok(length)
}

fn compile_presentation(inputs: PresentationInputs<'_, '_>) -> Result<MeasuredPresentation, ()> {
    let PresentationInputs {
        canonical,
        bsp,
        graph,
        bundle,
        decoders,
        model_resources,
        resource_hashes,
        map_materials,
        particle_presentation,
        profile,
        visibility,
        collision,
        additional_model_roots,
    } = inputs;
    let mut metrics_clock = RuntimeMetricsClock::new();
    let mut phase_started =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    let mut metrics = [0_u64; 6];
    let mut phase_finished =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[0] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let mut roots = std::collections::BTreeSet::from([
        "models/weapons/w_models/w_rocket.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/c_models/c_soldier_arms.mdl".to_owned(),
        "models/weapons/c_models/c_demo_arms.mdl".to_owned(),
        "models/weapons/c_models/c_scout_arms.mdl".to_owned(),
        "models/weapons/c_models/c_engineer_arms.mdl".to_owned(),
        "models/weapons/c_models/c_sniper_arms.mdl".to_owned(),
        "models/weapons/c_models/c_spy_arms.mdl".to_owned(),
        "models/player/scout.mdl".to_owned(),
        "models/player/sniper.mdl".to_owned(),
        "models/player/soldier.mdl".to_owned(),
        "models/player/demo.mdl".to_owned(),
        "models/player/medic.mdl".to_owned(),
        "models/player/heavy.mdl".to_owned(),
        "models/player/pyro.mdl".to_owned(),
        "models/player/spy.mdl".to_owned(),
        "models/player/engineer.mdl".to_owned(),
        "models/vgui/ui_class01.mdl".to_owned(),
        "models/class_menu/random_class_icon.mdl".to_owned(),
        "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl".to_owned(),
        "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl".to_owned(),
        "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl".to_owned(),
        "models/weapons/c_models/c_bottle/c_bottle.mdl".to_owned(),
        "models/weapons/c_models/c_scattergun.mdl".to_owned(),
        "models/weapons/c_models/c_pistol/c_pistol.mdl".to_owned(),
        "models/weapons/c_models/c_bat.mdl".to_owned(),
        "models/weapons/c_models/c_shotgun/c_shotgun.mdl".to_owned(),
        "models/weapons/c_models/c_shovel/c_shovel.mdl".to_owned(),
        "models/weapons/c_models/c_wrench/c_wrench.mdl".to_owned(),
        "models/weapons/w_models/w_shotgun.mdl".to_owned(),
        "models/weapons/w_models/w_pistol.mdl".to_owned(),
        "models/weapons/w_models/w_wrench.mdl".to_owned(),
        "models/weapons/c_models/c_heavy_arms.mdl".to_owned(),
        "models/weapons/c_models/c_minigun/c_minigun.mdl".to_owned(),
        "models/weapons/c_models/c_sniperrifle/c_sniperrifle.mdl".to_owned(),
        "models/weapons/c_models/c_smg/c_smg.mdl".to_owned(),
        "models/weapons/c_models/c_machete/c_machete.mdl".to_owned(),
        "models/weapons/c_models/c_pyro_arms.mdl".to_owned(),
        "models/weapons/c_models/c_flamethrower/c_flamethrower.mdl".to_owned(),
        "models/weapons/c_models/c_fireaxe_pyro/c_fireaxe_pyro.mdl".to_owned(),
        "models/weapons/c_models/c_revolver/c_revolver.mdl".to_owned(),
        "models/weapons/c_models/c_knife/c_knife.mdl".to_owned(),
        "models/weapons/c_models/c_sapper/c_sapper.mdl".to_owned(),
        "models/weapons/v_models/v_pda_spy.mdl".to_owned(),
        "models/weapons/v_models/v_watch_spy.mdl".to_owned(),
    ]);
    for entity in &graph.entities {
        let classname = entity.classname.as_deref();
        let model = if classname.is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic")) {
            entity.model.as_deref()
        } else if classname.is_some_and(|value| value.eq_ignore_ascii_case(b"item_teamflag")) {
            Some(entity_scalar(entity, b"flag_model").unwrap_or(b"models/flag/briefcase.mdl"))
        } else {
            None
        };
        if let Some(model) = model {
            roots.insert(
                std::str::from_utf8(model)
                    .map_err(|_| ())?
                    .to_ascii_lowercase(),
            );
        }
    }
    roots.extend(additional_model_roots.iter().cloned());
    let models = roots
        .into_iter()
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|id| {
            let presentation_profile = if matches!(
                id.as_str(),
                "models/weapons/c_models/c_soldier_arms.mdl"
                    | "models/weapons/c_models/c_demo_arms.mdl"
                    | "models/weapons/c_models/c_scout_arms.mdl"
                    | "models/weapons/c_models/c_engineer_arms.mdl"
                    | "models/weapons/c_models/c_sniper_arms.mdl"
                    | "models/weapons/c_models/c_spy_arms.mdl"
                    | "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl"
                    | "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl"
                    | "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl"
                    | "models/weapons/c_models/c_bottle/c_bottle.mdl"
                    | "models/weapons/c_models/c_scattergun.mdl"
                    | "models/weapons/c_models/c_pistol/c_pistol.mdl"
                    | "models/weapons/c_models/c_bat.mdl"
                    | "models/weapons/c_models/c_shotgun/c_shotgun.mdl"
                    | "models/weapons/c_models/c_shovel/c_shovel.mdl"
                    | "models/weapons/c_models/c_wrench/c_wrench.mdl"
                    | "models/weapons/c_models/c_heavy_arms.mdl"
                    | "models/weapons/c_models/c_minigun/c_minigun.mdl"
                    | "models/weapons/c_models/c_sniperrifle/c_sniperrifle.mdl"
                    | "models/weapons/c_models/c_smg/c_smg.mdl"
                    | "models/weapons/c_models/c_machete/c_machete.mdl"
                    | "models/weapons/c_models/c_pyro_arms.mdl"
                    | "models/weapons/c_models/c_flamethrower/c_flamethrower.mdl"
                    | "models/weapons/c_models/c_fireaxe_pyro/c_fireaxe_pyro.mdl"
                    | "models/weapons/c_models/c_revolver/c_revolver.mdl"
                    | "models/weapons/c_models/c_knife/c_knife.mdl"
                    | "models/weapons/c_models/c_sapper/c_sapper.mdl"
                    | "models/weapons/v_models/v_pda_spy.mdl"
                    | "models/weapons/v_models/v_watch_spy.mdl"
            ) {
                playsrc_studio_model::PresentationProfile::ViewModel
            } else {
                playsrc_studio_model::PresentationProfile::World
            };
            let artifact = build_model_presentation(
                &id,
                bundle,
                model_resources,
                resource_hashes,
                profile,
                presentation_profile,
            )?;
            Ok((id, artifact))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let model_vertices = models
        .iter()
        .flat_map(|(_, artifact)| &artifact.model.geometry)
        .map(|primitive| primitive.vertices.len())
        .sum();
    let model_triangles = models
        .iter()
        .flat_map(|(_, artifact)| &artifact.model.geometry)
        .map(|primitive| primitive.triangles.len())
        .sum();
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[1] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let decoded_texture_count = 0;
    let decoded_texture_bytes = 0;
    let distinct_decoded_texture_count = 0;
    let unique_decoded_texture_bytes = 0;
    let repeated_decoded_texture_bytes = 0;
    let source_textures = bundle
        .iter()
        .filter(|(identity, _)| identity.ends_with(".vtf"))
        .map(|(identity, bytes)| (*bytes, resource_hashes[identity]))
        .collect::<Vec<_>>();
    let source_texture_count = source_textures.len();
    let source_texture_bytes = source_textures
        .iter()
        .map(|(bytes, _)| bytes.len())
        .sum::<usize>();
    let source_texture_identities = source_textures
        .iter()
        .map(|(bytes, identity)| (*identity, bytes.len()))
        .collect::<BTreeMap<_, _>>();
    let distinct_source_texture_count = source_texture_identities.len();
    let unique_source_texture_bytes = source_texture_identities.values().sum::<usize>();
    let mut directional = Vec::new();
    for (reference, material) in canonical.materials.iter().zip(map_materials) {
        let identity = reference.logical_path.to_ascii_lowercase();
        let Some(texture) = material.textures.iter().find(|texture| {
            matches!(
                texture.role,
                playsrc_material::TextureRole::Bump | playsrc_material::TextureRole::Normal
            ) && texture.disposition == playsrc_material::TextureDisposition::Source
        }) else {
            continue;
        };
        let path = texture
            .logical_path
            .as_ref()
            .ok_or(())?
            .to_ascii_lowercase();
        directional.push((
            identity,
            u8::from(material.features.ss_bump),
            decoded_texture(&path, decoders)?,
        ));
    }
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[2] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let particle_materials = particle_presentation.keys().cloned().collect::<Vec<_>>();
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[3] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let (environment_bytes, environment) = compile_environment_artifact(
        canonical,
        bsp,
        graph,
        bundle,
        decoders,
        resource_hashes,
        map_materials,
        profile,
        visibility,
        collision,
    )?;
    let static_props = compile_static_prop_section(
        canonical,
        graph,
        bundle,
        visibility,
        collision,
        &environment,
        &models,
    )?;
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[4] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let prepared_model_materials =
        prepare_model_materials(&models, bundle, decoders, resource_hashes, profile)?;
    let capacity = presentation_capacity(
        &models,
        &directional,
        particle_presentation,
        &particle_materials,
        &environment_bytes,
        &static_props.bytes,
        &prepared_model_materials,
        canonical,
        bundle.len(),
    )?;
    let mut out = Vec::new();
    out.try_reserve_exact(capacity).map_err(|_| ())?;
    out.extend_from_slice(b"PTF2");
    out.extend_from_slice(&13u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(models.len()).map_err(|_| ())?.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(directional.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    let mut section_ends = [0usize; 8];
    section_ends[0] = out.len();
    out.extend_from_slice(
        &u32::try_from(particle_materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(canonical.brush_models.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (id, a) in &models {
        pbytes(&mut out, id.as_bytes())?;
        out.extend_from_slice(&[
            if a.model.profile == playsrc_studio_model::PresentationProfile::World {
                0
            } else {
                1
            },
            0,
            0,
            0,
        ]);
        out.extend_from_slice(&a.identity);
        out.extend_from_slice(
            &u32::try_from(a.model.skins.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(a.model.body_parts.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for p in &a.model.body_parts {
            out.extend_from_slice(
                &u32::try_from(p.model_names.len())
                    .map_err(|_| ())?
                    .to_le_bytes(),
            );
        }
        out.extend_from_slice(
            &u32::try_from(a.model.attachments.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        let attachment_pose = playsrc_studio_model::sample_pose(
            &a.model,
            &playsrc_studio_model::AnimationState {
                base_sequence: 0,
                cycle: playsrc_studio_model::Float32(0),
                pose_parameters: a
                    .model
                    .pose_parameters
                    .iter()
                    .map(|_| playsrc_studio_model::Float32(0))
                    .collect(),
                layers: Vec::new(),
            },
        )
        .map_err(|_| ())?;
        for v in &a.model.attachments {
            pbytes(&mut out, &v.name)?;
            let sampled = attachment_pose
                .attachments
                .iter()
                .find(|attachment| attachment.index == v.index)
                .ok_or(())?;
            for value in sampled.model_transform.0 {
                out.extend_from_slice(&value.0.to_le_bytes());
            }
        }
        out.extend_from_slice(
            &u32::try_from(a.model.sequences.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for v in &a.model.sequences {
            pbytes(&mut out, &v.label)?;
            pbytes(&mut out, &v.activity_name)?;
            out.extend_from_slice(&u32::try_from(v.index).map_err(|_| ())?.to_le_bytes());
            let timing = playsrc_studio_model::sequence_timing(
                &a.model,
                v.index,
                &a.model
                    .pose_parameters
                    .iter()
                    .map(|_| playsrc_studio_model::Float32(0))
                    .collect::<Vec<_>>(),
            )
            .ok();
            out.extend_from_slice(&[u8::from(timing.is_some()), 0, 0, 0]);
            if let Some(timing) = timing {
                for value in [
                    timing.frames_per_second,
                    timing.weighted_frame_count,
                    timing.cycles_per_second,
                    timing.duration_seconds,
                ] {
                    out.extend_from_slice(&value.0.to_le_bytes());
                }
                out.extend_from_slice(&[u8::from(timing.looping), 0, 0, 0]);
            } else {
                out.extend_from_slice(&[0; 20]);
            }
        }
        match a.model.descriptor {
            playsrc_studio_model::PresentationDescriptor::World {
                geometry,
                root_bone,
                depth_range,
                ..
            } => {
                out.extend_from_slice(&[
                    0,
                    match root_bone {
                        playsrc_studio_model::RootBoneContract::AnimatedBelowEntity => 0,
                        playsrc_studio_model::RootBoneContract::StaticPropBoneZeroIsEntity => 1,
                    },
                    match geometry.facing.front_face {
                        playsrc_studio_model::TriangleWinding::Clockwise => 0,
                        playsrc_studio_model::TriangleWinding::CounterClockwise => 1,
                    },
                    match geometry.facing.cull_face {
                        playsrc_studio_model::CullFace::Back => 0,
                    },
                ]);
                for value in depth_range {
                    out.extend_from_slice(&value.0.to_le_bytes());
                }
                out.extend_from_slice(&[0; 32]);
            }
            playsrc_studio_model::PresentationDescriptor::ViewModel {
                geometry,
                default_horizontal_fov_4_by_3,
                minimum_fov,
                maximum_fov,
                near_plane,
                depth_range,
                draws_after_world,
                opaque_before_translucent,
                ..
            } => {
                out.extend_from_slice(&[
                    1,
                    0,
                    match geometry.facing.front_face {
                        playsrc_studio_model::TriangleWinding::Clockwise => 0,
                        playsrc_studio_model::TriangleWinding::CounterClockwise => 1,
                    },
                    match geometry.facing.cull_face {
                        playsrc_studio_model::CullFace::Back => 0,
                    },
                ]);
                for value in [
                    default_horizontal_fov_4_by_3,
                    minimum_fov,
                    maximum_fov,
                    near_plane,
                ] {
                    out.extend_from_slice(&value.0.to_le_bytes());
                }
                for value in depth_range {
                    out.extend_from_slice(&value.0.to_le_bytes());
                }
                out.extend_from_slice(&[
                    u8::from(draws_after_world),
                    u8::from(opaque_before_translucent),
                    0,
                    0,
                ]);
                out.extend_from_slice(&[0; 12]);
            }
        }
        pbytes(&mut out, &[])?;
    }
    section_ends[1] = out.len();
    section_ends[2] = out.len();
    for (material, kind, texture) in directional {
        pbytes(&mut out, material.as_bytes())?;
        out.extend_from_slice(&[kind, 0, 0, 0]);
        pbytes(&mut out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(texture.rgba.as_slice()).into();
        out.extend_from_slice(&hash);
        pbytes(&mut out, &texture.rgba)?;
        for value in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0] {
            out.extend_from_slice(&value.to_le_bytes())
        }
    }
    section_ends[3] = out.len();
    for material in &particle_materials {
        pbytes(&mut out, material.as_bytes())?;
    }
    section_ends[4] = out.len();
    pbytes(&mut out, &environment_bytes)?;
    section_ends[5] = out.len();
    encode_material_states(
        &mut out,
        canonical,
        graph,
        bundle,
        decoders,
        profile,
        &models,
        map_materials,
        &prepared_model_materials,
        particle_presentation,
    )?;
    encode_particle_textures(&mut out, particle_presentation)?;
    encode_audio_documents(&mut out, bundle)?;
    encode_model_occurrence_matrices(&mut out, graph)?;
    encode_model_materials(&mut out, &prepared_model_materials)?;
    section_ends[6] = out.len();
    for model in &canonical.brush_models {
        out.extend_from_slice(&u32::try_from(model.index).map_err(|_| ())?.to_le_bytes());
        for value in model.bounds[0]
            .into_iter()
            .chain(model.bounds[1])
            .chain(model.origin)
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&model.head_node.to_le_bytes());
        for value in [
            model.surface_range.start,
            model.surface_range.end,
            model.vertex_count,
            model.triangle_count,
            model.materials.len(),
            model.entities.len(),
        ] {
            out.extend_from_slice(&u32::try_from(value).map_err(|_| ())?.to_le_bytes());
        }
        for value in &model.materials {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
        for value in &model.entities {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
    }
    out.extend_from_slice(&static_props.bytes);
    out.extend_from_slice(
        &u32::try_from(static_props.bytes.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    out.extend_from_slice(b"PSPF");
    let model_material_opacity = model_material_opacity(
        &models,
        bundle,
        decoders,
        profile,
        Some(&prepared_model_materials),
    )?;
    let models: BTreeMap<String, playsrc_studio_model::PresentationModel> = models
        .into_iter()
        .map(|(identity, artifact)| (identity, artifact.model))
        .collect();
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[5] = phase_finished.saturating_sub(phase_started);
    section_ends[7] = out.len();
    if out.len() > PRESENTATION_OUTPUT_LIMIT {
        return Err(());
    }
    let static_prop_runtime_sources = static_props
        .section
        .occurrences
        .iter()
        .filter_map(|occurrence| {
            matches!(
                occurrence.lighting,
                static_prop_artifact::Lighting::Runtime { .. }
            )
            .then_some(occurrence.source as usize)
        })
        .collect::<Vec<_>>();
    let static_prop_runtime_light_records = static_props
        .section
        .occurrences
        .iter()
        .flat_map(|occurrence| match &occurrence.lighting {
            static_prop_artifact::Lighting::Runtime { lights, .. } => lights
                .iter()
                .map(|light| {
                    (
                        occurrence.source as usize,
                        light.source as usize,
                        light.style,
                    )
                })
                .collect::<Vec<_>>(),
            static_prop_artifact::Lighting::Vertex { .. } => Vec::new(),
        })
        .collect();
    let ledger = PresentationSizeLedger {
        model_count: models.len(),
        model_vertices,
        model_triangles,
        decoded_texture_count,
        distinct_decoded_texture_count,
        decoded_texture_bytes,
        unique_decoded_texture_bytes,
        repeated_decoded_texture_bytes,
        source_texture_count,
        distinct_source_texture_count,
        source_texture_bytes,
        unique_source_texture_bytes,
        section_ends,
        default_bound_first_exceeded_at: None,
        final_length: out.len(),
        final_capacity: out.capacity(),
        phase_milliseconds: [0; 6],
        displacement_input_count: 0,
        static_prop_collision_count: 0,
        static_prop_occurrence_count: canonical.static_props.occurrences.len(),
        static_prop_vhv_object_count: static_props.aggregate_object_count,
        static_prop_runtime_lighting_count: static_prop_runtime_sources.len(),
        static_prop_section_bytes: static_props.bytes.len(),
        static_prop_section_sha256: Sha256::digest(&static_props.bytes).into(),
        static_prop_runtime_sources,
        static_prop_runtime_light_records,
        static_prop_main_count: static_props
            .section
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.ownership == static_prop_artifact::ViewOwnership::Main)
            .count(),
        static_prop_sky_count: static_props
            .section
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.ownership == static_prop_artifact::ViewOwnership::Sky3d)
            .count(),
        static_prop_vertex_lighting_count: static_props
            .section
            .occurrences
            .iter()
            .filter(|occurrence| {
                matches!(
                    occurrence.lighting,
                    static_prop_artifact::Lighting::Vertex { .. }
                )
            })
            .count(),
    };
    Ok((
        (out, models, model_material_opacity, environment),
        metrics,
        ledger,
    ))
}

fn model_material_opacity(
    models: &[(String, Box<CompiledPresentationModel>)],
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    profile: playsrc_map::LightingProfile,
    prepared: Option<&PreparedModelMaterials>,
) -> Result<BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>, ()> {
    models
        .iter()
        .map(|(identity, artifact)| {
            let values = artifact
                .model
                .materials
                .iter()
                .map(|selected| {
                    let dependency = artifact
                        .model
                        .dependencies
                        .get(selected.material_dependency)
                        .ok_or(())?;
                    let path = dependency.logical_path.to_ascii_lowercase();
                    let owned;
                    let material = if let Some(prepared) = prepared {
                        let index = prepared
                            .materials
                            .binary_search_by(|(identity, _)| identity.as_str().cmp(&path))
                            .map_err(|_| ())?;
                        &prepared.materials[index].1
                    } else {
                        owned = resolve_material_semantics(
                            &path,
                            bundle,
                            material_environment(profile, true),
                        )?;
                        &owned
                    };
                    let texture_alpha = selected_texture(material, decoders).ok().is_some_and(
                        |(_, _, metadata)| {
                            metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit
                        },
                    );
                    let draw = playsrc_material::model_draw_state(
                        material,
                        playsrc_material::TextureAlphaFacts {
                            base: texture_alpha,
                        },
                        playsrc_material::ModelRuntimeInputs {
                            alpha_modulation: 1.0,
                            cloak_factor: Some(0.0),
                        },
                    )
                    .map_err(|_| ())?;
                    Ok(match draw.opacity {
                        playsrc_material::ModelOpacity::Opaque => {
                            playsrc_studio_model::ViewModelMaterialOpacity::Opaque
                        }
                        playsrc_material::ModelOpacity::Translucent => {
                            playsrc_studio_model::ViewModelMaterialOpacity::Translucent
                        }
                    })
                })
                .collect::<Result<Vec<_>, ()>>()?;
            Ok((identity.clone(), values))
        })
        .collect()
}

fn encode_parameter_origin(out: &mut Vec<u8>, value: playsrc_material::ParameterOrigin) {
    out.push(match value {
        playsrc_material::ParameterOrigin::Authored => 0,
        playsrc_material::ParameterOrigin::ShaderInitializer => 1,
        playsrc_material::ParameterOrigin::TypeInitializer => 2,
    });
}

fn encode_effective_f32(out: &mut Vec<u8>, value: &playsrc_material::EffectiveParameter<f32>) {
    out.extend_from_slice(&value.value.to_le_bytes());
    encode_parameter_origin(out, value.origin);
    out.extend_from_slice(&[0; 3]);
}

fn encode_effective_i32(out: &mut Vec<u8>, value: &playsrc_material::EffectiveParameter<i32>) {
    out.extend_from_slice(&value.value.to_le_bytes());
    encode_parameter_origin(out, value.origin);
    out.extend_from_slice(&[0; 3]);
}

fn encode_effective_bool(out: &mut Vec<u8>, value: &playsrc_material::EffectiveParameter<bool>) {
    out.extend_from_slice(&[
        u8::from(value.value),
        match value.origin {
            playsrc_material::ParameterOrigin::Authored => 0,
            playsrc_material::ParameterOrigin::ShaderInitializer => 1,
            playsrc_material::ParameterOrigin::TypeInitializer => 2,
        },
        0,
        0,
    ]);
}

fn encode_effective_vec2(
    out: &mut Vec<u8>,
    value: &playsrc_material::EffectiveParameter<[f32; 2]>,
) {
    for component in value.value {
        out.extend_from_slice(&component.to_le_bytes());
    }
    encode_parameter_origin(out, value.origin);
    out.extend_from_slice(&[0; 3]);
}

fn encode_effective_vec3(
    out: &mut Vec<u8>,
    value: &playsrc_material::EffectiveParameter<[f32; 3]>,
) {
    for component in value.value {
        out.extend_from_slice(&component.to_le_bytes());
    }
    encode_parameter_origin(out, value.origin);
    out.extend_from_slice(&[0; 3]);
}

fn encode_texture_request(
    out: &mut Vec<u8>,
    value: &playsrc_material::TextureRequest,
) -> Result<(), ()> {
    out.extend_from_slice(&[
        value.role as u8,
        value.disposition as u8,
        color_read_code(value.color_read),
        0,
    ]);
    pbytes(out, &value.parameter)?;
    pbytes(out, &value.reference)?;
    pbytes(out, value.logical_path.as_deref().unwrap_or("").as_bytes())
}

fn encode_water_material(
    out: &mut Vec<u8>,
    identity: &str,
    map_material: Option<usize>,
    material: &playsrc_material::Material,
) -> Result<(), ()> {
    let state = playsrc_material::water_material_output(material)
        .map_err(|_| ())?
        .ok_or(())?;
    pbytes(out, identity.as_bytes())?;
    out.extend_from_slice(
        &map_material
            .map(|value| u32::try_from(value).map_err(|_| ()))
            .transpose()?
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    let textures = [
        state.textures.base.as_ref(),
        state.textures.normal.as_ref(),
        state.textures.flow.as_ref(),
        state.textures.environment.as_ref(),
        state.textures.reflection.as_ref(),
        state.textures.refraction.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    out.extend_from_slice(&[
        match state.shader {
            playsrc_material::WaterShaderVariant::Dx90 => 0,
            playsrc_material::WaterShaderVariant::Dx9Hdr => 1,
        },
        match state.opacity {
            playsrc_material::WaterSurfaceOpacity::Opaque => 0,
            playsrc_material::WaterSurfaceOpacity::Translucent => 1,
        },
        u8::try_from(textures.len()).map_err(|_| ())?,
        u8::try_from(state.required_inputs.len()).map_err(|_| ())?,
    ]);
    for texture in textures {
        encode_texture_request(out, texture)?;
    }
    pbytes(
        out,
        state
            .bottom_material
            .as_ref()
            .map_or(&[], |value| value.logical_path.as_bytes()),
    )?;
    pbytes(
        out,
        state
            .underwater_overlay
            .as_ref()
            .map_or(&[], |value| value.logical_path.as_bytes()),
    )?;
    encode_effective_i32(out, &state.base_frame);
    encode_effective_i32(out, &state.normal_frame);
    encode_effective_i32(out, &state.environment_frame);
    pbytes(out, &state.normal_transform.parameter)?;
    for value in state.normal_transform.matrix {
        out.extend_from_slice(&value.to_le_bytes());
    }
    encode_parameter_origin(out, state.normal_transform.origin);
    out.extend_from_slice(&[u8::from(state.normal_transform.proxy_mutated), 0, 0]);
    encode_effective_vec2(out, &state.scale);
    encode_effective_f32(out, &state.time);
    encode_effective_f32(out, &state.water_depth);
    encode_effective_bool(out, &state.above_water);
    encode_effective_f32(out, &state.reflect_amount);
    encode_effective_f32(out, &state.refract_amount);
    encode_effective_vec3(out, &state.reflect_tint);
    encode_effective_vec3(out, &state.refract_tint);
    encode_effective_f32(out, &state.reflection_blend_factor);
    if let Some(value) = &state.fog.enabled {
        out.extend_from_slice(&[1, 0, 0, 0]);
        encode_effective_bool(out, value);
    } else {
        out.extend_from_slice(&[0; 8]);
    }
    encode_effective_vec3(out, &state.fog.color);
    encode_effective_f32(out, &state.fog.start);
    encode_effective_f32(out, &state.fog.end);
    encode_effective_f32(out, &state.cheap_start);
    encode_effective_f32(out, &state.cheap_end);
    encode_effective_bool(out, &state.force_cheap);
    encode_effective_bool(out, &state.force_expensive);
    encode_effective_bool(out, &state.reflect_entities);
    encode_effective_bool(out, &state.blur_refraction);
    encode_effective_bool(out, &state.no_low_end_lightmap);
    for value in &state.scroll {
        encode_effective_vec3(out, value);
    }
    out.extend_from_slice(&[u8::from(state.fresnel.cheap_enabled), 0, 0, 0]);
    for value in state.fresnel.expensive_constant {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for requirement in state.required_inputs {
        out.push(match requirement {
            playsrc_material::WaterInputRequirement::AuthoredTexturePlanes(role) => {
                1_u8.checked_add(role as u8).ok_or(())?
            }
            playsrc_material::WaterInputRequirement::Lightmap => 32,
            playsrc_material::WaterInputRequirement::LocalEnvironment => 33,
            playsrc_material::WaterInputRequirement::ReflectionFramebuffer => 34,
            playsrc_material::WaterInputRequirement::RefractionFramebuffer => 35,
            playsrc_material::WaterInputRequirement::CameraPosition => 36,
            playsrc_material::WaterInputRequirement::WaterSurfacePlane => 37,
            playsrc_material::WaterInputRequirement::EyeWaterSide => 38,
            playsrc_material::WaterInputRequirement::DistanceToWater => 39,
            playsrc_material::WaterInputRequirement::RuntimeWaterPolicy => 40,
            playsrc_material::WaterInputRequirement::WaterFogVolume => 41,
            playsrc_material::WaterInputRequirement::PresentationTime => 42,
            playsrc_material::WaterInputRequirement::WaterLodController => 43,
        });
    }
    Ok(())
}

fn encode_world_material(
    out: &mut Vec<u8>,
    identity: &str,
    map_material: usize,
    material: &playsrc_material::Material,
    state: &playsrc_material::WorldMaterialOutput,
) -> Result<(), ()> {
    pbytes(out, identity.as_bytes())?;
    out.extend_from_slice(&u32::try_from(map_material).map_err(|_| ())?.to_le_bytes());
    out.extend_from_slice(&[
        shader_code(state.shader),
        u8::try_from(state.textures.len()).map_err(|_| ())?,
        u8::try_from(material.proxy_program.entries.len()).map_err(|_| ())?,
        u8::from(state.environment_map.is_some()),
    ]);
    for binding in &state.textures {
        encode_texture_request(out, &binding.texture)?;
        let usage = material
            .texture_uses
            .iter()
            .find(|usage| usage.role == binding.texture.role)
            .ok_or(())?;
        let frame_mutated = matches!(
            usage.frame,
            playsrc_material::TextureFrameSelection::Static {
                proxy_mutated: true,
                ..
            }
        );
        let transform_mutated = usage
            .transform
            .as_ref()
            .is_some_and(|transform| transform.proxy_mutated);
        out.extend_from_slice(&[
            u8::from(binding.initial_frame.is_some()),
            u8::from(frame_mutated),
            u8::from(binding.transform.is_some()),
            u8::from(transform_mutated),
        ]);
        out.extend_from_slice(&binding.initial_frame.unwrap_or(0).to_le_bytes());
        for value in binding.transform.unwrap_or([0.0; 16]) {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for proxy in &material.proxy_program.entries {
        pbytes(out, &proxy.name)?;
        out.extend_from_slice(&[
            match proxy.disposition {
                playsrc_material::ProxyDisposition::Handled => 0,
                playsrc_material::ProxyDisposition::Malformed => 1,
                playsrc_material::ProxyDisposition::Unsupported => 2,
            },
            0,
            0,
            0,
        ]);
    }
    if let Some(environment) = &state.environment_map {
        for value in environment
            .tint
            .into_iter()
            .chain([environment.contrast, environment.saturation])
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out.extend_from_slice(&state.fresnel_reflection.to_le_bytes());
    Ok(())
}

fn encode_fog_state(out: &mut Vec<u8>, value: &playsrc_map::FogState) {
    out.extend_from_slice(&[
        u8::from(value.enabled),
        u8::from(value.blend),
        u8::from(value.radial),
        u8::from(value.far_z.is_some()),
    ]);
    for component in value.direction {
        out.extend_from_slice(&component.to_le_bytes());
    }
    out.extend_from_slice(&value.primary);
    out.extend_from_slice(&value.secondary);
    for component in [
        value.start,
        value.end,
        value.maximum_density,
        value.far_z.unwrap_or(0.0),
        value.transition_duration,
    ] {
        out.extend_from_slice(&component.to_le_bytes());
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_environment_artifact(
    canonical: &playsrc_map::CanonicalMap,
    bsp: &playsrc_bsp::Bsp,
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    materials: &[playsrc_material::Material],
    profile: playsrc_map::LightingProfile,
    visibility: &playsrc_visibility::World,
    collision: &playsrc_collision::World,
) -> Result<(Vec<u8>, RuntimeEnvironment), ()> {
    if materials.len() != canonical.materials.len() {
        return Err(());
    }
    let bindings = canonical
        .materials
        .iter()
        .zip(materials)
        .map(|(reference, material)| playsrc_map::MaterialBinding {
            material_index: reference.index,
            material,
        })
        .collect::<Vec<_>>();
    let dependent_paths = materials
        .iter()
        .flat_map(|material| &material.material_requests)
        .filter(|request| request.role == playsrc_material::MaterialRole::Bottom)
        .map(|request| request.logical_path.to_ascii_lowercase())
        .filter(|path| {
            !canonical
                .materials
                .iter()
                .any(|material| material.logical_path.eq_ignore_ascii_case(path))
        })
        .collect::<std::collections::BTreeSet<_>>();
    let dependent_materials = dependent_paths
        .into_iter()
        .map(|path| {
            Ok((
                path.clone(),
                resolve_material_semantics(&path, bundle, material_environment(profile, false))?,
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let dependent_bindings = dependent_materials
        .iter()
        .map(
            |(logical_path, material)| playsrc_map::NamedMaterialBinding {
                logical_path,
                material,
            },
        )
        .collect::<Vec<_>>();
    let world = graph
        .entities
        .iter()
        .find(|e| {
            e.classname
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case(b"worldspawn"))
        })
        .ok_or(())?;
    let sky = std::str::from_utf8(
        &world
            .pairs
            .iter()
            .find(|p| p.key.eq_ignore_ascii_case(b"skyname"))
            .ok_or(())?
            .value,
    )
    .map_err(|_| ())?;
    let mut dependencies = Vec::new();
    for (face, face_suffix) in [
        (playsrc_map::CubeFace::Right, "rt"),
        (playsrc_map::CubeFace::Left, "lf"),
        (playsrc_map::CubeFace::Back, "bk"),
        (playsrc_map::CubeFace::Front, "ft"),
        (playsrc_map::CubeFace::Up, "up"),
        (playsrc_map::CubeFace::Down, "dn"),
    ] {
        let suffix = if profile == playsrc_map::LightingProfile::Hdr {
            "_hdr"
        } else {
            ""
        };
        let path = format!("materials/skybox/{sky}{suffix}{face_suffix}.vmt");
        let source_sha256 = *resource_hashes.get(&path).ok_or(())?;
        let m = resolve_material_semantics(&path, bundle, material_environment(profile, false))?;
        let encoding = selected_sky_encoding(&m.selected_textures).ok_or(())?;
        let selected_textures = m
            .textures
            .iter()
            .filter(|t| {
                m.selected_textures.contains(&t.role)
                    && t.disposition == playsrc_material::TextureDisposition::Source
            })
            .map(|t| {
                let logical_path = t.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
                Ok(playsrc_map::ResolvedTexture {
                    sha256: *resource_hashes.get(&logical_path).ok_or(())?,
                    logical_path,
                })
            })
            .collect::<Result<Vec<_>, ()>>()?;
        dependencies.push(playsrc_map::DependencyResponse {
            request: playsrc_map::DependencyRequest {
                role: playsrc_map::DependencyRole::SkyMaterial(face),
                profile,
                logical_path: path,
            },
            metadata: playsrc_map::DependencyMetadata::SkyMaterial {
                source_sha256,
                encoding,
                selected_textures,
            },
        })
    }
    let records = match &bsp.lumps[42].records {
        playsrc_bsp::LumpData::Cubemaps(v) => v.as_slice(),
        _ => return Err(()),
    };
    let first = records.first().ok_or(())?;
    let profile_suffix = if profile == playsrc_map::LightingProfile::Hdr {
        ".hdr"
    } else {
        ""
    };
    let leaf = format!(
        "/c{}_{}_{}{profile_suffix}.vtf",
        first.origin[0], first.origin[1], first.origin[2]
    );
    let mut map_names = bundle
        .keys()
        .filter_map(|path| path.strip_prefix("materials/maps/")?.strip_suffix(&leaf))
        .collect::<std::collections::BTreeSet<_>>();
    if map_names.len() != 1 {
        return Err(());
    }
    let map_name = map_names.pop_first().ok_or(())?.to_owned();
    let logical_map_path = format!("maps/{map_name}.bsp");
    for (index, r) in records.iter().enumerate() {
        let suffix = if profile == playsrc_map::LightingProfile::Hdr {
            ".hdr"
        } else {
            ""
        };
        let path = format!(
            "materials/maps/{map_name}/c{}_{}_{}{suffix}.vtf",
            r.origin[0], r.origin[1], r.origin[2]
        );
        let m = decoders.metadata(&path)?;
        let source_sha256 = *resource_hashes.get(&path).ok_or(())?;
        dependencies.push(playsrc_map::DependencyResponse {
            request: playsrc_map::DependencyRequest {
                role: playsrc_map::DependencyRole::CubemapTexture { sample: index },
                profile,
                logical_path: path,
            },
            metadata: playsrc_map::DependencyMetadata::CubemapTexture {
                source_sha256,
                width: m.width,
                height: m.height,
                mip_count: m.mip_count,
                source_face_count: m.faces.len() as u8,
            },
        })
    }
    let mut marks = BTreeMap::new();
    for e in &graph.entities {
        if !e
            .classname
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case(b"infodecal"))
        {
            continue;
        }
        let Some(reference) = e
            .pairs
            .iter()
            .find(|p| p.key.eq_ignore_ascii_case(b"texture"))
            .map(|p| p.value.clone())
        else {
            continue;
        };
        let path = dependency_path(&reference)?;
        if !bundle.contains_key(&path) {
            continue;
        }
        let source_sha256 = *resource_hashes.get(&path).ok_or(())?;
        let m = resolve_material_semantics(&path, bundle, material_environment(profile, false))?;
        let texture = m
            .textures
            .iter()
            .find(|t| {
                m.selected_textures.contains(&t.role)
                    && t.disposition == playsrc_material::TextureDisposition::Source
            })
            .ok_or(())?;
        let tm = decoders.metadata(
            &texture
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase(),
        )?;
        marks
            .entry(reference.to_ascii_lowercase())
            .or_insert(playsrc_map::MarkMaterial {
                reference,
                logical_path: path,
                source_sha256,
                width: tm.width,
                height: tm.height,
                state: m.decal,
            });
    }
    let marks = marks.into_values().collect::<Vec<_>>();
    let receiver_inputs = canonical
        .brush_model_occurrences
        .iter()
        .filter(|occurrence| {
            matches!(
                occurrence.classname.as_slice(),
                b"func_door" | b"func_button" | b"func_movelinear" | b"func_brush"
            )
        })
        .map(|occurrence| {
            let brush = occurrence.classname.eq_ignore_ascii_case(b"func_brush");
            playsrc_collision::ObjectInput {
                identity: occurrence.entity as u64,
                role: playsrc_collision::ObjectRole::Entity,
                enabled: !brush
                    || occurrence.solidity.as_deref() != Some(b"1")
                        && occurrence.start_disabled.as_deref() != Some(b"1"),
                volume_contents: false,
                transform: playsrc_collision::Transform {
                    origin: occurrence.origin,
                    angles: occurrence.angles,
                },
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: 0,
                surface_flags: 0,
                shape: playsrc_collision::SnapshotShape::BrushModel {
                    model: occurrence.model,
                },
            }
        })
        .collect();
    let receiver_snapshot = playsrc_collision::Snapshot::compile(
        collision,
        1,
        receiver_inputs,
        playsrc_collision::SnapshotLimits::default(),
    )
    .map_err(|_| ())?;
    let mark_placements = playsrc_map::MarkPlacementSnapshot {
        revision: 1,
        placements: graph
            .entities
            .iter()
            .filter(|entity| {
                entity
                    .classname
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(b"infodecal"))
            })
            .map(|entity| {
                Ok(playsrc_map::MarkPlacement {
                    entity: entity.index,
                    world_origin: entity_vector(entity, b"origin")?,
                    parent_entity: entity.parentname.as_deref().and_then(|name| {
                        graph
                            .entities
                            .iter()
                            .find(|candidate| candidate.targetname.as_deref() == Some(name))
                            .map(|candidate| candidate.index)
                    }),
                })
            })
            .collect::<Result<Vec<_>, ()>>()?,
    };
    let env = playsrc_map::compile_environment_prepared(
        canonical,
        bsp,
        playsrc_map::EnvironmentInputs {
            logical_map_path: &logical_map_path,
            entities: graph,
            visibility,
            collision,
            receiver_snapshot: &receiver_snapshot,
            mark_placements: &mark_placements,
            materials: &bindings,
            dependent_materials: &dependent_bindings,
            mark_materials: &marks,
            dependencies: &dependencies,
            limits: playsrc_map::EnvironmentLimits::default(),
        },
        visibility.identity,
    )
    .map_err(|_| ())?;
    let mut out = b"PENV".to_vec();
    out.extend_from_slice(&5u32.to_le_bytes());
    out.extend_from_slice(&[
        if profile == playsrc_map::LightingProfile::Hdr {
            1
        } else {
            0
        },
        0,
        0,
        0,
    ]);
    out.extend_from_slice(&visibility.identity);
    for value in [
        visibility.cluster_count,
        visibility.nodes.len(),
        visibility.leaves.len(),
        env.sky.as_ref().map_or(0, |s| s.surface_faces.len()),
        env.cubemaps.len(),
        env.water.surfaces.len(),
        env.water.volumes.len(),
        env.marks.records.len(),
        env.marks.fragment_count,
        env.controllers.len(),
    ] {
        out.extend_from_slice(&(value as u32).to_le_bytes())
    }
    for mark in &env.marks.records {
        out.extend_from_slice(&[
            mark.status as u8,
            mark.kind as u8,
            u8::from(mark.initially_enabled),
            u8::from(mark.dynamic),
        ]);
        pbytes(&mut out, mark.material_path.as_bytes())?;
        out.extend_from_slice(&(mark.fragments.len() as u32).to_le_bytes());
        for fragment in &mark.fragments {
            out.extend_from_slice(&(fragment.model as u32).to_le_bytes());
            out.extend_from_slice(&(fragment.face as u32).to_le_bytes());
            out.extend_from_slice(&(fragment.positions.len() as u32).to_le_bytes());
            for position in &fragment.positions {
                for value in position {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            for normal in &fragment.normals {
                for value in normal {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            for uv in &fragment.uv {
                for value in uv {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
            out.extend_from_slice(&(fragment.triangles.len() as u32).to_le_bytes());
            for triangle in &fragment.triangles {
                for value in triangle {
                    out.extend_from_slice(&value.to_le_bytes())
                }
            }
        }
    }
    let mut decal_textures = BTreeMap::new();
    for mark in &marks {
        if let Some(texture) = resolve_material(
            &mark.logical_path,
            bundle,
            decoders,
            true,
            material_environment(profile, false),
        )?
        .base_texture
        {
            decal_textures.insert(
                mark.logical_path.clone(),
                decoded_texture(&texture.logical_path, decoders)?,
            );
        }
    }
    out.extend_from_slice(&(decal_textures.len() as u32).to_le_bytes());
    for (material, texture) in decal_textures {
        pbytes(&mut out, material.as_bytes())?;
        pbytes(&mut out, texture.logical_path.as_bytes())?;
        out.extend_from_slice(&texture.width.to_le_bytes());
        out.extend_from_slice(&texture.height.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(texture.rgba.as_slice()).into();
        out.extend_from_slice(&hash);
        pbytes(&mut out, &texture.rgba)?
    }
    if let Some(sky) = &env.sky {
        out.push(1);
        pbytes(&mut out, &sky.name)?;
        out.extend_from_slice(&(sky.faces.len() as u32).to_le_bytes());
        for face in &sky.faces {
            out.extend_from_slice(&[face.face as u8, face.encoding as u8, 0, 0]);
            pbytes(&mut out, face.material_path.as_bytes())?;
            out.extend_from_slice(&face.material_sha256)
        }
    } else {
        out.push(0)
    }
    for sample in &env.cubemaps {
        out.extend_from_slice(&(sample.index as u32).to_le_bytes());
        for value in sample.origin {
            out.extend_from_slice(&value.to_le_bytes())
        }
        pbytes(&mut out, sample.logical_path.as_bytes())?;
        out.extend_from_slice(&sample.source_sha256);
        out.extend_from_slice(&sample.width.to_le_bytes());
        out.extend_from_slice(&sample.height.to_le_bytes())
    }
    let cubemap_code = |selection: Option<&playsrc_map::CubemapSelection>| -> (u8, u32) {
        match selection {
            Some(playsrc_map::CubemapSelection::Nearest { sample }) => (0, *sample as u32),
            Some(playsrc_map::CubemapSelection::Declared { sample }) => (1, *sample as u32),
            Some(playsrc_map::CubemapSelection::External { .. }) => (2, u32::MAX),
            None => (3, u32::MAX),
        }
    };
    for surface in &env.water.surfaces {
        out.extend_from_slice(&[
            if surface.profile == playsrc_map::LightingProfile::Hdr {
                1
            } else {
                0
            },
            u8::from(surface.selected),
            0,
            0,
        ]);
        for value in [surface.face, surface.model, surface.material] {
            out.extend_from_slice(&(value as u32).to_le_bytes())
        }
        for bound in surface.bounds {
            for value in bound {
                out.extend_from_slice(&value.to_le_bytes())
            }
        }
        let (kind, sample) = cubemap_code(surface.bindings.environment.as_ref());
        out.extend_from_slice(&[kind, 0, 0, 0]);
        out.extend_from_slice(&sample.to_le_bytes())
    }
    for volume in &env.water.volumes {
        out.extend_from_slice(&(volume.index as u32).to_le_bytes());
        out.extend_from_slice(&volume.surface_z.to_le_bytes());
        out.extend_from_slice(&volume.minimum_z.to_le_bytes());
        for bound in volume.bounds {
            for value in bound {
                out.extend_from_slice(&value.to_le_bytes())
            }
        }
        out.extend_from_slice(&(volume.leaves.len() as u32).to_le_bytes());
        for leaf in &volume.leaves {
            out.extend_from_slice(&(*leaf as u32).to_le_bytes())
        }
        let (kind, sample) = cubemap_code(volume.surface_bindings.environment.as_ref());
        out.extend_from_slice(&[kind, 0, 0, 0]);
        out.extend_from_slice(&sample.to_le_bytes())
    }
    for controller in &env.controllers {
        out.extend_from_slice(&(controller.entity as u32).to_le_bytes());
        pbytes(&mut out, &controller.classname)?;
        out.push(match &controller.state {
            playsrc_map::ControllerState::Fog(_) => 0,
            playsrc_map::ControllerState::SkyCamera { .. } => 1,
            playsrc_map::ControllerState::WaterLod { .. } => 2,
            playsrc_map::ControllerState::EnvironmentLight { .. } => 3,
            playsrc_map::ControllerState::Shadow { .. } => 4,
            playsrc_map::ControllerState::ToneMap => 5,
        });
    }
    out.extend_from_slice(&env.marks.collision_world_identity);
    out.extend_from_slice(&env.marks.receiver_snapshot_revision.to_le_bytes());
    out.extend_from_slice(&env.marks.placement_revision.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(env.marks.records.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for mark in &env.marks.records {
        out.extend_from_slice(
            &u32::try_from(mark.source_index)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &mark
                .entity
                .map(|value| u32::try_from(value).map_err(|_| ()))
                .transpose()?
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        out.extend_from_slice(&mark.overlay_id.unwrap_or(i32::MIN).to_le_bytes());
        for value in mark.origin {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&[
            u8::from(mark.material_sha256.is_some()),
            mark.render_order,
            u8::from(mark.low_priority),
            match mark.activation {
                playsrc_map::MarkActivation::MapActivation => 0,
                playsrc_map::MarkActivation::Input => 1,
                playsrc_map::MarkActivation::Compiled => 2,
            },
            match mark.lifetime {
                playsrc_map::MarkLifetime::Permanent => 0,
                playsrc_map::MarkLifetime::PoolManaged => 1,
            },
            u8::from(mark.fade_distances_squared.is_some()),
            mark.render.polygon_offset as u8,
            0,
        ]);
        out.extend_from_slice(&mark.material_sha256.unwrap_or([0; 32]));
        for value in mark.fade_distances_squared.unwrap_or([0.0; 2]) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&mark.render.normal_offset.to_le_bytes());
        out.extend_from_slice(
            &mark
                .parent_entity
                .map(|value| u32::try_from(value).map_err(|_| ()))
                .transpose()?
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        if let Some(receiver) = mark.receiver {
            out.extend_from_slice(&[1, 0, 0, 0]);
            out.extend_from_slice(&receiver.entity.unwrap_or(u64::MAX).to_le_bytes());
            out.extend_from_slice(&u32::try_from(receiver.model).map_err(|_| ())?.to_le_bytes());
            out.extend_from_slice(
                &receiver
                    .parent_entity
                    .map(|value| u32::try_from(value).map_err(|_| ()))
                    .transpose()?
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            for value in receiver
                .local_origin
                .into_iter()
                .chain(receiver.transform.origin)
                .chain(receiver.transform.angles)
            {
                out.extend_from_slice(&value.to_le_bytes());
            }
        } else {
            out.extend_from_slice(&[0; 56]);
        }
        out.extend_from_slice(
            &u32::try_from(mark.target_faces.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for value in &mark.target_faces {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(mark.fragments.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for fragment in &mark.fragments {
            match &fragment.visibility {
                playsrc_map::MarkVisibility::World {
                    leaves,
                    clusters,
                    areas,
                } => {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                    for count in [leaves.len(), clusters.len(), areas.len()] {
                        out.extend_from_slice(&u32::try_from(count).map_err(|_| ())?.to_le_bytes());
                    }
                    for value in leaves {
                        out.extend_from_slice(
                            &u32::try_from(*value).map_err(|_| ())?.to_le_bytes(),
                        );
                    }
                    for value in clusters {
                        out.extend_from_slice(&value.to_le_bytes());
                    }
                    for value in areas {
                        out.extend_from_slice(
                            &u32::try_from(*value).map_err(|_| ())?.to_le_bytes(),
                        );
                    }
                }
                playsrc_map::MarkVisibility::BrushModel { entity, model } => {
                    out.extend_from_slice(&[1, 0, 0, 0]);
                    out.extend_from_slice(&entity.to_le_bytes());
                    out.extend_from_slice(&u32::try_from(*model).map_err(|_| ())?.to_le_bytes());
                }
            }
            out.extend_from_slice(
                &u32::try_from(fragment.lightmap_uv.len())
                    .map_err(|_| ())?
                    .to_le_bytes(),
            );
            for value in &fragment.lightmap_uv {
                for component in value {
                    out.extend_from_slice(&component.to_le_bytes());
                }
            }
        }
    }
    let leaf_distances = env
        .water
        .leaf_minimum_distance_to_water
        .as_deref()
        .unwrap_or(&[]);
    out.extend_from_slice(
        &u32::try_from(leaf_distances.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for value in leaf_distances {
        out.extend_from_slice(&value.to_le_bytes());
    }
    let mut water_materials = Vec::new();
    for (index, material) in materials.iter().enumerate() {
        if material.water.is_some() {
            let reference = canonical.materials.get(index).ok_or(())?;
            water_materials.push((
                reference.logical_path.to_ascii_lowercase(),
                Some(reference.index),
                material,
            ));
        }
    }
    for (identity, material) in &dependent_materials {
        if material.water.is_some() {
            water_materials.push((identity.clone(), None, material));
        }
    }
    out.extend_from_slice(
        &u32::try_from(water_materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, map_material, material) in &water_materials {
        encode_water_material(&mut out, identity, *map_material, material)?;
    }
    let world_materials = canonical
        .materials
        .iter()
        .zip(materials)
        .filter(|(_, material)| {
            matches!(
                material.shader,
                playsrc_material::Shader::LightmappedGeneric
                    | playsrc_material::Shader::WorldVertexTransition
            ) && material.bump.is_some()
                && material.proxy_program.entries.iter().any(|entry| {
                    matches!(
                        entry.operation,
                        Some(
                            playsrc_material::ProxyOperation::AnimatedTexture { .. }
                                | playsrc_material::ProxyOperation::TextureTransform { .. }
                                | playsrc_material::ProxyOperation::TextureScroll { .. }
                        )
                    )
                })
        })
        .map(|(reference, material)| {
            Ok((
                reference.logical_path.to_ascii_lowercase(),
                reference.index,
                material,
                playsrc_material::world_material_output(material)
                    .map_err(|_| ())?
                    .ok_or(())?,
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    out.extend_from_slice(
        &u32::try_from(world_materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, map_material, material, output) in &world_materials {
        encode_world_material(&mut out, identity, *map_material, material, output)?;
    }
    let mut environment_texture_paths = std::collections::BTreeSet::new();
    for (_, _, _, world) in &world_materials {
        for texture in &world.textures {
            if texture.texture.disposition == playsrc_material::TextureDisposition::Source {
                environment_texture_paths.insert(
                    texture
                        .texture
                        .logical_path
                        .as_ref()
                        .ok_or(())?
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    for (_, _, material) in &water_materials {
        let water = playsrc_material::water_material_output(material)
            .map_err(|_| ())?
            .ok_or(())?;
        for texture in [
            water.textures.base.as_ref(),
            water.textures.normal.as_ref(),
            water.textures.flow.as_ref(),
            water.textures.environment.as_ref(),
            water.textures.reflection.as_ref(),
            water.textures.refraction.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if texture.disposition == playsrc_material::TextureDisposition::Source
                && texture.is_defined()
            {
                environment_texture_paths.insert(
                    texture
                        .logical_path
                        .as_ref()
                        .ok_or(())?
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    for material in materials {
        for texture in &material.textures {
            if texture.disposition == playsrc_material::TextureDisposition::Source
                && (material.selected_textures.contains(&texture.role)
                    || texture.role == playsrc_material::TextureRole::Base2
                    || texture.role == playsrc_material::TextureRole::Detail)
            {
                environment_texture_paths.insert(
                    texture
                        .logical_path
                        .as_ref()
                        .ok_or(())?
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    for mark in &marks {
        let material = resolve_material_semantics(
            &mark.logical_path,
            bundle,
            material_environment(profile, false),
        )?;
        for texture in &material.textures {
            if texture.disposition == playsrc_material::TextureDisposition::Source
                && material.selected_textures.contains(&texture.role)
            {
                environment_texture_paths.insert(
                    texture
                        .logical_path
                        .as_ref()
                        .ok_or(())?
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    for sample in &env.cubemaps {
        environment_texture_paths.insert(sample.logical_path.to_ascii_lowercase());
    }
    if let Some(sky) = &env.sky {
        for face in &sky.faces {
            for texture in &face.selected_textures {
                environment_texture_paths.insert(texture.logical_path.to_ascii_lowercase());
            }
        }
    }
    out.extend_from_slice(
        &u32::try_from(environment_texture_paths.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for identity in environment_texture_paths {
        let texture = model_authored_texture(&identity, decoders, resource_hashes, false)?;
        encode_model_authored_texture(&mut out, &identity, &texture)?;
    }
    out.extend_from_slice(
        &u32::try_from(env.water.surfaces.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for surface in &env.water.surfaces {
        out.extend_from_slice(&u32::try_from(surface.face).map_err(|_| ())?.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(surface.texture_info)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &surface
                .fog_volume
                .map(|value| u32::try_from(value).map_err(|_| ()))
                .transpose()?
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for value in surface.plane {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&[
            u8::from(surface.bindings.environment.is_some()),
            u8::from(surface.bindings.reflection),
            u8::from(surface.bindings.refraction),
            0,
        ]);
    }
    out.extend_from_slice(
        &u32::try_from(env.water.volumes.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for volume in &env.water.volumes {
        out.extend_from_slice(
            &u32::try_from(volume.texture_info)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(volume.surface_material)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        match &volume.bottom_material {
            None => out.extend_from_slice(&[0, 0, 0, 0]),
            Some(playsrc_map::WaterMaterialIdentity::Map(index)) => {
                out.extend_from_slice(&[1, 0, 0, 0]);
                out.extend_from_slice(&u32::try_from(*index).map_err(|_| ())?.to_le_bytes());
            }
            Some(playsrc_map::WaterMaterialIdentity::Dependency(identity)) => {
                out.extend_from_slice(&[2, 0, 0, 0]);
                pbytes(&mut out, identity.as_bytes())?;
            }
        }
        out.extend_from_slice(&volume.contents.to_le_bytes());
        for value in volume.plane {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(volume.clusters.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for value in &volume.clusters {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(
            &u32::try_from(volume.areas.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for value in &volume.areas {
            out.extend_from_slice(&u32::try_from(*value).map_err(|_| ())?.to_le_bytes());
        }
        out.extend_from_slice(&[
            u8::from(volume.surface_translucent),
            match volume.bottom_translucent {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            },
            0,
            0,
        ]);
        out.extend_from_slice(&[
            u8::from(volume.surface_bindings.environment.is_some()),
            u8::from(volume.surface_bindings.reflection),
            u8::from(volume.surface_bindings.refraction),
            u8::from(volume.bottom_bindings.is_some()),
        ]);
        let bottom = volume.bottom_bindings.as_ref();
        out.extend_from_slice(&[
            u8::from(bottom.is_some_and(|value| value.environment.is_some())),
            u8::from(bottom.is_some_and(|value| value.reflection)),
            u8::from(bottom.is_some_and(|value| value.refraction)),
            0,
        ]);
    }
    out.extend_from_slice(
        &u32::try_from(env.controllers.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for controller in &env.controllers {
        out.extend_from_slice(
            &u32::try_from(controller.entity)
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(controller.raw_fields.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for (key, value) in &controller.raw_fields {
            pbytes(&mut out, key)?;
            pbytes(&mut out, value)?;
        }
        match &controller.state {
            playsrc_map::ControllerState::Fog(value) => {
                out.push(0);
                encode_fog_state(&mut out, value);
            }
            playsrc_map::ControllerState::SkyCamera {
                origin,
                scale,
                area,
                fog,
            } => {
                out.push(1);
                for value in origin {
                    out.extend_from_slice(&value.to_le_bytes());
                }
                out.extend_from_slice(&scale.to_le_bytes());
                out.extend_from_slice(&u32::try_from(*area).map_err(|_| ())?.to_le_bytes());
                encode_fog_state(&mut out, fog);
            }
            playsrc_map::ControllerState::WaterLod { start, end } => {
                out.push(2);
                out.extend_from_slice(&start.to_le_bytes());
                out.extend_from_slice(&end.to_le_bytes());
            }
            playsrc_map::ControllerState::EnvironmentLight {
                origin,
                angles,
                pitch,
                sunlight,
                sunlight_hdr,
                sunlight_hdr_scale,
                ambient,
                ambient_hdr,
                ambient_hdr_scale,
                sun_spread_angle,
            } => {
                out.push(3);
                for value in origin
                    .iter()
                    .chain(angles)
                    .chain(core::slice::from_ref(pitch))
                    .chain(sunlight)
                    .chain(sunlight_hdr)
                    .chain(core::slice::from_ref(sunlight_hdr_scale))
                    .chain(ambient)
                    .chain(ambient_hdr)
                    .chain(core::slice::from_ref(ambient_hdr_scale))
                    .chain(core::slice::from_ref(sun_spread_angle))
                {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
            playsrc_map::ControllerState::Shadow {
                angles,
                color,
                maximum_distance,
                disabled,
            } => {
                out.push(4);
                for value in angles {
                    out.extend_from_slice(&value.to_le_bytes());
                }
                out.extend_from_slice(color);
                out.extend_from_slice(&maximum_distance.to_le_bytes());
                out.extend_from_slice(&[u8::from(*disabled), 0, 0, 0]);
            }
            playsrc_map::ControllerState::ToneMap => out.push(5),
        }
    }
    out.extend_from_slice(
        &env.master_fog_controller
            .map(|value| u32::try_from(value).map_err(|_| ()))
            .transpose()?
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    if let Some(sky) = &env.sky {
        out.extend_from_slice(
            &u32::try_from(sky.faces.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        for face in &sky.faces {
            out.extend_from_slice(
                &u32::try_from(face.selected_textures.len())
                    .map_err(|_| ())?
                    .to_le_bytes(),
            );
            for texture in &face.selected_textures {
                pbytes(&mut out, texture.logical_path.as_bytes())?;
                out.extend_from_slice(&texture.sha256);
            }
        }
    } else {
        out.extend_from_slice(&0_u32.to_le_bytes());
    }
    let mut runtime_materials = BTreeMap::new();
    let mut runtime_world_materials = BTreeMap::new();
    let mut map_materials = BTreeMap::new();
    let mut normal_frame_counts = BTreeMap::new();
    for (identity, map_material, material, output) in &world_materials {
        let mut texture_frames = BTreeMap::new();
        for texture in &output.textures {
            if texture.texture.disposition != playsrc_material::TextureDisposition::Source {
                continue;
            }
            let path = texture
                .texture
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            texture_frames.insert(
                texture.texture.parameter.clone(),
                u32::from(decoders.metadata(&path)?.frame_count),
            );
        }
        runtime_world_materials.insert(
            identity.clone(),
            RuntimeWorldMaterial {
                map_material: *map_material,
                material: (*material).clone(),
                texture_frames,
            },
        );
    }
    for (index, material) in materials.iter().enumerate() {
        if material.water.is_none() {
            continue;
        }
        let reference = canonical.materials.get(index).ok_or(())?;
        let identity = reference.logical_path.to_ascii_lowercase();
        let output = playsrc_material::water_material_output(material)
            .map_err(|_| ())?
            .ok_or(())?;
        if let Some(normal) = output
            .textures
            .normal
            .as_ref()
            .filter(|value| value.is_defined())
        {
            let path = normal.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
            let metadata = decoders.metadata(&path)?;
            normal_frame_counts.insert(identity.clone(), u32::from(metadata.frame_count));
        }
        map_materials.insert(reference.index, identity.clone());
        runtime_materials.insert(identity, material.clone());
    }
    for (identity, material) in dependent_materials {
        if material.water.is_none() {
            continue;
        }
        let output = playsrc_material::water_material_output(&material)
            .map_err(|_| ())?
            .ok_or(())?;
        if let Some(normal) = output
            .textures
            .normal
            .as_ref()
            .filter(|value| value.is_defined())
        {
            let path = normal.logical_path.as_ref().ok_or(())?.to_ascii_lowercase();
            let metadata = decoders.metadata(&path)?;
            normal_frame_counts.insert(identity.clone(), u32::from(metadata.frame_count));
        }
        runtime_materials.insert(identity, material);
    }
    let water_lods = env
        .controllers
        .iter()
        .filter_map(|controller| match controller.state {
            playsrc_map::ControllerState::WaterLod { start, end } => Some([start, end]),
            _ => None,
        })
        .collect::<Vec<_>>();
    if water_lods.len() > 1 {
        return Err(());
    }
    Ok((
        out,
        RuntimeEnvironment {
            world: env,
            water_materials: runtime_materials,
            world_materials: runtime_world_materials,
            map_materials,
            normal_frame_counts,
            water_lod: water_lods.first().copied(),
        },
    ))
}

type CompiledParticles = (
    playsrc_particle::ParticleWorld,
    Vec<String>,
    BTreeMap<String, playsrc_particle::ParticleMaterial>,
    BTreeMap<String, CompiledParticlePresentation>,
);

struct CompiledParticlePresentation {
    source_path: String,
    state: playsrc_material::StaticState,
    metadata: playsrc_vtf::Metadata,
    texture: DecodedTexture,
}

fn compile_particles(
    b: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
) -> Result<CompiledParticles, ()> {
    let paths = [
        "particles/rockettrail.pcf",
        "particles/rocketbackblast.pcf",
        "particles/stickybomb.pcf",
        "particles/muzzle_flash.pcf",
        "particles/explosion.pcf",
        "particles/flamethrower.pcf",
    ];
    let sources = paths
        .iter()
        .map(|path| {
            Ok(playsrc_particle::PcfSource {
                logical_path: path,
                bytes: b.get(*path).ok_or(())?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let registry =
        playsrc_particle::Registry::from_pcf(&sources, playsrc_particle::RegistryLimits::default())
            .map_err(|_| ())?;
    let roots = [
        "rockettrail",
        "rocketbackblast",
        "stickybombtrail_red",
        "stickybombtrail_blue",
        "stickybomb_pulse_red",
        "stickybomb_pulse_blue",
        "muzzle_pipelauncher",
        "muzzle_scattergun",
        "muzzle_pistol",
        "muzzle_shotgun",
        "ExplosionCore_Wall",
        "ExplosionCore_MidAir",
        "new_flame",
        "new_flame_crit_red",
        "new_flame_crit_blue",
        "flamethrower_underwater",
        "pyro_blast",
        "muzzle_shotgun",
    ]
    .map(playsrc_particle::DefinitionLookup::Name);
    let materials = registry.target_closure(&roots).map_err(|_| ())?.materials;
    let compiled = materials
        .iter()
        .map(|identity| {
            let material_path = dependency_path(identity.as_bytes())?;
            let material = resolve_material_semantics(
                &material_path,
                b,
                playsrc_material::SelectionEnvironment::default(),
            )?;
            let (selected, _bytes, metadata) = selected_texture(&material, decoders)?;
            let texture_path = selected
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            let texture = rgba_texture(&texture_path, decoders)?;
            let state = playsrc_material::static_state(
                &material,
                playsrc_material::TextureAlphaFacts {
                    base: metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit,
                },
            )
            .map_err(|_| ())?;
            let factor = |value| match value {
                playsrc_material::BlendFactor::Zero => playsrc_particle::ParticleBlendFactor::Zero,
                playsrc_material::BlendFactor::One => playsrc_particle::ParticleBlendFactor::One,
                playsrc_material::BlendFactor::SourceAlpha => {
                    playsrc_particle::ParticleBlendFactor::SourceAlpha
                }
                playsrc_material::BlendFactor::OneMinusSourceAlpha => {
                    playsrc_particle::ParticleBlendFactor::OneMinusSourceAlpha
                }
            };
            Ok((
                identity.clone(),
                (
                    playsrc_particle::ParticleMaterial {
                        shader: if material.shader == playsrc_material::Shader::Sprite {
                            playsrc_particle::ParticleMaterialShader::SpriteCard
                        } else {
                            playsrc_particle::ParticleMaterialShader::MeshSprite
                        },
                        blend: playsrc_particle::ParticleBlendState {
                            source: factor(state.blend.source),
                            destination: factor(state.blend.destination),
                        },
                        color_space: playsrc_particle::ParticleColorSpace::SrgbTextureLinearTint,
                        dual_sequence: false,
                        sheet: decode_particle_sheet(metadata)?,
                    },
                    CompiledParticlePresentation {
                        source_path: material_path,
                        state,
                        metadata: metadata.clone(),
                        texture,
                    },
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, ()>>()?;
    let mut sheets = BTreeMap::new();
    let mut presentation = BTreeMap::new();
    for (identity, (sheet, state)) in compiled {
        sheets.insert(identity.clone(), sheet);
        presentation.insert(identity, state);
    }
    Ok((
        playsrc_particle::ParticleWorld::new(&registry, playsrc_particle::WorldLimits::default())
            .map_err(|_| ())?,
        materials,
        sheets,
        presentation,
    ))
}

fn decode_particle_sheet(
    metadata: &playsrc_vtf::Metadata,
) -> Result<playsrc_particle::ParticleSheet, ()> {
    let resource = metadata.resources.iter().find_map(|resource| {
        if resource.tag == [0x10, 0, 0]
            && let playsrc_vtf::ResourceData::External { bytes, .. } = &resource.data
        {
            Some(bytes.as_slice())
        } else {
            None
        }
    });
    let Some(resource) = resource else {
        return Ok(playsrc_particle::ParticleSheet {
            sequences: BTreeMap::from([(
                0,
                playsrc_particle::SheetSequence {
                    clamp: true,
                    duration_seconds: 1.0,
                    frames: vec![playsrc_particle::SheetFrame {
                        duration_seconds: 1.0,
                        images: [[0.0, 0.0, 1.0, 1.0]; 4],
                    }],
                },
            )]),
        });
    };
    let mut reader = ParticleReader {
        bytes: resource,
        at: 0,
    };
    let version = reader.u32()?;
    if version > 1 {
        return Err(());
    }
    let sequence_count = reader.u32()? as usize;
    if sequence_count == 0 || sequence_count > 64 {
        return Err(());
    }
    let mut sequences = BTreeMap::new();
    for _ in 0..sequence_count {
        let identity = i32::from_le_bytes(reader.take(4)?.try_into().map_err(|_| ())?);
        if !(0..64).contains(&identity) || sequences.contains_key(&identity) {
            return Err(());
        }
        let clamp = match reader.u32()? {
            0 => false,
            1 => true,
            _ => return Err(()),
        };
        let frame_count = reader.u32()? as usize;
        if frame_count == 0 || frame_count > 1_024 {
            return Err(());
        }
        let duration_seconds = reader.f32()?;
        if duration_seconds <= 0.0 {
            return Err(());
        }
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let frame_duration = reader.f32()?;
            if frame_duration <= 0.0 {
                return Err(());
            }
            let image_count = if version == 0 { 1 } else { 4 };
            let mut images = [[0.0; 4]; 4];
            for image in images.iter_mut().take(image_count) {
                *image = [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?];
            }
            if image_count == 1 {
                let first = images[0];
                images.fill(first);
            }
            frames.push(playsrc_particle::SheetFrame {
                duration_seconds: frame_duration,
                images,
            });
        }
        sequences.insert(
            identity,
            playsrc_particle::SheetSequence {
                clamp,
                duration_seconds,
                frames,
            },
        );
    }
    (reader.at == resource.len())
        .then_some(playsrc_particle::ParticleSheet { sequences })
        .ok_or(())
}
struct ParticleCollision(Arc<playsrc_collision::World>);
impl playsrc_particle::CollisionQuery for ParticleCollision {
    fn trace_batch(
        &mut self,
        requests: &[playsrc_particle::TraceRequest],
    ) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> {
        Ok(requests
            .iter()
            .map(|r| {
                let radius = r.radius.max(0.0);
                let trace = self
                    .0
                    .trace_hull(
                        r.start,
                        r.end,
                        playsrc_collision::Hull {
                            mins: [-radius; 3],
                            maxs: [radius; 3],
                        },
                        u32::MAX,
                    )
                    .expect("validated particle collision query");
                playsrc_particle::CollisionResult {
                    identity: r.identity,
                    fraction: trace.fraction,
                    start_solid: trace.start_solid,
                    normal: trace.plane.map_or([0.0, 0.0, 1.0], |plane| plane.normal),
                }
            })
            .collect())
    }
}
struct ParticleReader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> ParticleReader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ()> {
        let end = self.at.checked_add(n).ok_or(())?;
        let v = self.bytes.get(self.at..end).ok_or(())?;
        self.at = end;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8, ()> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, ()> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| ())?,
        ))
    }
    fn u64(&mut self) -> Result<u64, ()> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| ())?,
        ))
    }
    fn f32(&mut self) -> Result<f32, ()> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().map_err(|_| ())?);
        v.is_finite().then_some(v).ok_or(())
    }
    fn text(&mut self) -> Result<String, ()> {
        let n = self.u32()? as usize;
        if n > 1024 {
            return Err(());
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| ())
    }
    fn flame_cp(&mut self) -> Result<playsrc_particle::ControlPoint, ()> {
        let index = self.u8()?;
        if !(1..=30).contains(&index) || self.take(3)? != [0, 0, 0] {
            return Err(());
        }
        let position = [self.f32()?, self.f32()?, self.f32()?];
        let orientation = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
        let velocity = [self.f32()?, self.f32()?, self.f32()?];
        let radius = self.f32()?;
        let density = self.f32()?;
        let duration = self.f32()?;
        if radius < 0.0 || density < 0.0 || duration < 0.0 {
            return Err(());
        }
        Ok(playsrc_particle::ControlPoint {
            index,
            position,
            previous_position: position,
            orientation,
            velocity,
            radius,
            density,
            duration,
            parent: None,
            object_identity: match self.u32()? {
                u32::MAX => None,
                value => Some(value),
            },
        })
    }

    fn cp(&mut self) -> Result<playsrc_particle::ControlPoint, ()> {
        let position = [self.f32()?, self.f32()?, self.f32()?];
        let orientation = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
        Ok(playsrc_particle::ControlPoint {
            index: 0,
            position,
            previous_position: position,
            orientation,
            velocity: [0.0; 3],
            radius: 0.0,
            density: 1.0,
            duration: 0.0,
            parent: None,
            object_identity: match self.u32()? {
                u32::MAX => None,
                v => Some(v),
            },
        })
    }
}
fn decode_particle_transaction(
    bytes: &[u8],
) -> Result<
    (
        Vec<playsrc_particle::Event>,
        playsrc_particle::AdvanceRequest,
    ),
    (),
> {
    let mut r = ParticleReader { bytes, at: 0 };
    if r.take(4)? != b"PPTX" || r.u32()? != 2 {
        return Err(());
    }
    let from = r.f32()?;
    let to = r.f32()?;
    let camera_position = [r.f32()?, r.f32()?, r.f32()?];
    let count = r.u32()? as usize;
    if count > 4096 || from < 0.0 || to < from {
        return Err(());
    }
    let mut events = Vec::with_capacity(count);
    let mut prior_timestamp = from;
    for order in 0..count {
        let kind = r.u8()?;
        let mode = r.u8()?;
        if r.take(2)? != [0, 0] || (kind != 3 && mode != 0) {
            return Err(());
        }
        let identity = r.u64()?;
        let timestamp_seconds = r.f32()?;
        if timestamp_seconds < prior_timestamp || timestamp_seconds > to {
            return Err(());
        }
        prior_timestamp = timestamp_seconds;
        let effect_identity = r.u32()?;
        let command = match kind {
            1 => {
                let seed = r.u64()?;
                let owner = match r.u32()? {
                    u32::MAX => None,
                    v => Some(v),
                };
                let definition = r.text()?;
                let cp = r.cp()?;
                playsrc_particle::EventCommand::Create {
                    effect_identity,
                    definition,
                    seed,
                    owner_identity: owner,
                    control_points: vec![cp],
                }
            }
            2 => playsrc_particle::EventCommand::SetControlPoint {
                effect_identity,
                control_point: r.cp()?,
            },
            3 => playsrc_particle::EventCommand::StopEmission {
                effect_identity,
                mode: match mode {
                    0 => playsrc_particle::StopMode::Graceful,
                    1 => playsrc_particle::StopMode::Immediate,
                    _ => return Err(()),
                },
            },
            4 => playsrc_particle::EventCommand::SetControlPoint {
                effect_identity,
                control_point: r.flame_cp()?,
            },
            _ => return Err(()),
        };
        events.push(playsrc_particle::Event {
            identity,
            timestamp_seconds,
            source_order: order as u32,
            command,
        });
    }
    if r.at != bytes.len() {
        return Err(());
    }
    Ok((
        events,
        playsrc_particle::AdvanceRequest {
            from_seconds: from,
            to_seconds: to,
            maximum_step_seconds: 1.0 / 60.0,
            camera_position,
        },
    ))
}

fn spawn(graph: &playsrc_entity::Graph) -> Option<Spawn> {
    let entity = graph.entities.iter().find(|entity| {
        entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"info_player_teamspawn"))
    })?;
    let hammer_id = match entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(b"hammerid"))
    {
        Some(pair) => std::str::from_utf8(&pair.value).ok()?.parse().ok()?,
        None => u32::MAX,
    };
    if !entity
        .pairs
        .iter()
        .any(|pair| pair.key.eq_ignore_ascii_case(b"origin"))
    {
        return None;
    }
    Some(Spawn {
        entity: entity.index.try_into().ok()?,
        hammer_id,
        position: entity_vector(entity, b"origin").ok()?,
        angles: entity_vector(entity, b"angles").ok()?,
    })
}
fn with<T>(handle: u32, read: impl FnOnce(&Slot) -> T) -> Option<T> {
    let (index, generation) = decode(handle)?;
    let slots = slots().lock().ok()?;
    let slot = slots.get(index)?;
    (slot.generation == generation && slot.payload.is_some()).then(|| read(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_resource_ownership_moves_without_another_wasm_allocation() {
        *resource_output().lock().unwrap() = vec![1, 2, 3, 4];
        assert_eq!(playsrc_resource_length(), 4);
        let pointer = playsrc_resource_take();
        assert!(!pointer.is_null());
        assert_eq!(playsrc_resource_length(), 0);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(pointer, 4) },
            &[1, 2, 3, 4]
        );
        unsafe { playsrc_free(pointer, 4) };
        assert!(playsrc_resource_take().is_null());
    }

    #[test]
    #[ignore = "requires the exact configured Pyro source graph"]
    fn configured_pyro_particle_materials_compile() {
        let graph = std::env::var("PLAYSRC_PYRO_GRAPH").expect("configured Pyro graph path");
        let bytes =
            playsrc_asset_graph::read_resource_set(std::path::Path::new(&graph), None).unwrap();
        let resources = bundle(&bytes).unwrap();
        let decoders = TextureDecoders::new(&resources);
        let paths = [
            "particles/rockettrail.pcf",
            "particles/rocketbackblast.pcf",
            "particles/stickybomb.pcf",
            "particles/muzzle_flash.pcf",
            "particles/explosion.pcf",
            "particles/flamethrower.pcf",
        ];
        let sources = paths.map(|path| playsrc_particle::PcfSource {
            logical_path: path,
            bytes: resources[path],
        });
        let registry = playsrc_particle::Registry::from_pcf(
            &sources,
            playsrc_particle::RegistryLimits::default(),
        )
        .unwrap();
        let roots = [
            "rockettrail",
            "rocketbackblast",
            "stickybombtrail_red",
            "stickybombtrail_blue",
            "stickybomb_pulse_red",
            "stickybomb_pulse_blue",
            "muzzle_pipelauncher",
            "ExplosionCore_Wall",
            "ExplosionCore_MidAir",
            "new_flame",
            "new_flame_crit_red",
            "new_flame_crit_blue",
            "flamethrower_underwater",
            "pyro_blast",
            "muzzle_shotgun",
        ]
        .map(playsrc_particle::DefinitionLookup::Name);
        for identity in registry.target_closure(&roots).unwrap().materials {
            let path = dependency_path(identity.as_bytes()).unwrap();
            let material = resolve_material_semantics(
                &path,
                &resources,
                playsrc_material::SelectionEnvironment::default(),
            )
            .unwrap_or_else(|_| panic!("material {identity}"));
            let (selected, _, metadata) =
                selected_texture(&material, &decoders).unwrap_or_else(|_| {
                    panic!(
                        "selected texture {identity}: shader={:?} selected={:?} textures={:?}",
                        material.shader, material.selected_textures, material.textures
                    )
                });
            let texture = selected.logical_path.as_ref().unwrap().to_ascii_lowercase();
            rgba_texture(&texture, &decoders)
                .unwrap_or_else(|_| panic!("rgba texture {identity}: {texture}"));
            playsrc_material::static_state(
                &material,
                playsrc_material::TextureAlphaFacts {
                    base: metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit,
                },
            )
            .unwrap_or_else(|error| panic!("static state {identity}: {error:?}"));
            decode_particle_sheet(metadata)
                .unwrap_or_else(|_| panic!("particle sheet {identity}: {texture}"));
        }
        assert!(compile_particles(&resources, &decoders).is_ok());
    }

    #[test]
    fn texture_decoders_inspect_each_immutable_source_once_even_across_parallel_consumers() {
        let mut source = vec![0_u8; 67];
        source[..4].copy_from_slice(b"VTF\0");
        source[4..8].copy_from_slice(&7_u32.to_le_bytes());
        source[8..12].copy_from_slice(&1_u32.to_le_bytes());
        source[12..16].copy_from_slice(&64_u32.to_le_bytes());
        source[16..18].copy_from_slice(&1_u16.to_le_bytes());
        source[18..20].copy_from_slice(&1_u16.to_le_bytes());
        source[24..26].copy_from_slice(&1_u16.to_le_bytes());
        source[52..56].copy_from_slice(&3_i32.to_le_bytes());
        source[56] = 1;
        source[57..61].copy_from_slice(&(-1_i32).to_le_bytes());
        source[64..67].copy_from_slice(&[1, 2, 3]);
        let invalid = [0_u8; 1];
        let bundle = BTreeMap::from([
            ("materials/invalid.vtf".to_owned(), invalid.as_slice()),
            ("materials/valid.vtf".to_owned(), source.as_slice()),
        ]);
        let cache = TextureDecoders::new(&bundle);
        (0..64_usize).into_par_iter().for_each(|_| {
            let metadata = cache.metadata("materials/valid.vtf").unwrap();
            assert_eq!((metadata.width, metadata.frame_count), (1, 1));
        });
        assert_eq!(cache.requests.load(Ordering::Relaxed), 64);
        assert_eq!(cache.inspections.load(Ordering::Relaxed), 1);
        assert!(cache.decoder("materials/invalid.vtf").is_err());
        assert!(cache.decoder("materials/invalid.vtf").is_err());
        assert_eq!(cache.requests.load(Ordering::Relaxed), 66);
        assert_eq!(cache.inspections.load(Ordering::Relaxed), 2);
    }

    fn particle_stop_transaction(mode: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PPTX");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0.0_f32.to_le_bytes());
        bytes.extend_from_slice(&0.015_f32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[3, mode, 0, 0]);
        bytes.extend_from_slice(&7_u64.to_le_bytes());
        bytes.extend_from_slice(&0.015_f32.to_le_bytes());
        bytes.extend_from_slice(&9_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn particle_transaction_uses_one_stop_opcode_and_explicit_source_modes() {
        for (mode, expected) in [
            (0, playsrc_particle::StopMode::Graceful),
            (1, playsrc_particle::StopMode::Immediate),
        ] {
            let (events, request) = decode_particle_transaction(&particle_stop_transaction(mode))
                .expect("current particle transaction must decode");
            assert_eq!(request.from_seconds, 0.0);
            assert_eq!(request.to_seconds, 0.015);
            assert_eq!(
                events[0].command,
                playsrc_particle::EventCommand::StopEmission {
                    effect_identity: 9,
                    mode: expected,
                }
            );
        }
    }

    #[test]
    fn particle_transaction_rejects_old_versions_duplicate_opcodes_and_malformed_inputs() {
        let current = particle_stop_transaction(0);
        let mutations: Vec<Vec<u8>> = vec![
            {
                let mut bytes = current.clone();
                bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes[32] = 4;
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes[33] = 2;
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes[34] = 1;
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes[44..48].copy_from_slice(&0.03_f32.to_le_bytes());
                bytes
            },
            {
                let mut bytes = current.clone();
                bytes.push(0);
                bytes
            },
        ];
        for (index, bytes) in mutations.iter().enumerate() {
            assert!(
                decode_particle_transaction(bytes).is_err(),
                "malformed mutation {index} was accepted"
            );
        }
    }

    #[test]
    fn material_state_targets_reject_conflicting_identity_or_source() {
        let mut targets = BTreeMap::new();
        insert_material_state_target(
            &mut targets,
            "effects/rocketrailsmoke.vmt".to_owned(),
            "materials/effects/rocketrailsmoke.vmt".to_owned(),
            false,
        )
        .unwrap();
        insert_material_state_target(
            &mut targets,
            "effects/rocketrailsmoke.vmt".to_owned(),
            "materials/effects/rocketrailsmoke.vmt".to_owned(),
            false,
        )
        .unwrap();
        assert!(
            insert_material_state_target(
                &mut targets,
                "effects/rocketrailsmoke.vmt".to_owned(),
                "materials/effects/other.vmt".to_owned(),
                false,
            )
            .is_err()
        );
        assert!(
            insert_material_state_target(
                &mut targets,
                "materials/effects/rocketrailsmoke.vmt".to_owned(),
                "materials/effects/rocketrailsmoke.vmt".to_owned(),
                false,
            )
            .is_err()
        );
        assert!(
            insert_material_state_target(
                &mut targets,
                "effects/rocketrailsmoke.vmt".to_owned(),
                "materials/effects/rocketrailsmoke.vmt".to_owned(),
                true,
            )
            .is_err()
        );
        assert_eq!(targets.len(), 1);
    }
    #[test]
    fn cached_model_headers_consume_complete_sequence_records() {
        let mut bytes = b"PTF2".to_vec();
        bytes.extend_from_slice(&13_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        pbytes(&mut bytes, b"models/test.mdl").unwrap();
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[7; 32]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        pbytes(&mut bytes, b"idle").unwrap();
        pbytes(&mut bytes, b"ACT_IDLE").unwrap();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 0, 0]);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0; 40]);
        pbytes(&mut bytes, &[]).unwrap();
        let static_props = static_prop_artifact::encode_section(&static_prop_artifact::Section {
            aggregate_sha256: [0; 32],
            model_count: 1,
            occurrences: Vec::new(),
        })
        .unwrap();
        bytes.extend_from_slice(&static_props);
        bytes.extend_from_slice(&(static_props.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"PSPF");
        let models = cached_presentation_models(&bytes).unwrap();
        assert_eq!(
            models,
            vec![(
                "models/test.mdl".to_owned(),
                playsrc_studio_model::PresentationProfile::World,
                [7; 32]
            )]
        );
    }
    #[test]
    fn static_prop_direct_light_uses_source_attenuation_cone_and_cache_bounds() {
        let mut light = playsrc_map::WorldLight {
            origin: [64.0, 0.0, 0.0],
            intensity: [1.0; 3],
            normal: [-1.0, 0.0, 0.0],
            cluster: 0,
            kind: 1,
            style: 0,
            stop_dot: 0.9,
            stop_dot2: 0.8,
            exponent: 1.0,
            radius: 128.0,
            constant_attenuation: 1.0,
            linear_attenuation: 0.0,
            quadratic_attenuation: 1.0,
            flags: 0,
            texture_info: -1,
            owner: -1,
        };
        let (_, direction, ratio) = direct_light_ray([0.0; 3], &light).unwrap();
        assert_eq!(direction, [1.0, 0.0, 0.0]);
        assert_eq!(ratio, 1.0 / 4097.0);
        light.kind = 2;
        assert_eq!(direct_light_angle(&light, direction).unwrap(), 1.0);
        light.normal = [1.0, 0.0, 0.0];
        assert_eq!(direct_light_angle(&light, direction).unwrap(), 0.0);
        light.kind = 1;
        light.radius = 1.0;
        assert_eq!(direct_light_ray([0.0; 3], &light).unwrap().2, 0.0);
        assert_eq!(
            lightcache_bounds([-0.5, 32.0, -128.5]),
            ([-32.0, 32.0, -256.0], [0.0, 64.0, -128.0])
        );
    }
    #[test]
    fn sky_texture_roles_select_explicit_render_encoding() {
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::Base]),
            Some(playsrc_map::SkyEncoding::Srgb)
        );
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::HdrBase]),
            Some(playsrc_map::SkyEncoding::Linear)
        );
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::HdrCompressed]),
            Some(playsrc_map::SkyEncoding::HdrRgbs)
        );
        assert_eq!(
            selected_sky_encoding(&[
                playsrc_material::TextureRole::HdrCompressed0,
                playsrc_material::TextureRole::HdrCompressed1,
                playsrc_material::TextureRole::HdrCompressed2,
            ]),
            None
        );
    }

    #[test]
    fn rocket_trace_preserves_direct_entity_identity() {
        let world = playsrc_collision::World::empty();
        let snapshot = playsrc_collision::Snapshot::compile(
            &world,
            1,
            vec![playsrc_collision::ObjectInput {
                identity: 42,
                role: playsrc_collision::ObjectRole::Entity,
                enabled: true,
                volume_contents: false,
                transform: playsrc_collision::Transform::IDENTITY,
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: 1,
                surface_flags: 0,
                shape: playsrc_collision::SnapshotShape::BoundingBox {
                    bounds: playsrc_collision::Hull {
                        mins: [-1.0; 3],
                        maxs: [1.0; 3],
                    },
                },
            }],
            playsrc_collision::SnapshotLimits::default(),
        )
        .unwrap();
        let results = resolve_rocket_traces(
            &world,
            &snapshot,
            &[playsrc_tf2::RocketTraceRequest {
                projectile: 7,
                tick: 3,
                start: [-10.0, 0.0, 0.0],
                end: [10.0, 0.0, 0.0],
                mask: 1,
            }],
            4,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].solid);
        assert_eq!(results[0].direct_target, Some(42));
    }

    #[test]
    fn stale_handles_do_not_read_reused_slots() {
        let mut guard = slots().lock().unwrap();
        guard.clear();
        guard.push(Slot {
            generation: 1,
            payload: Some(vec![1, 2]),
            presentation: Vec::new(),
            coverage: Vec::new(),
            particles: None,
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            model_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            environment: None,
            collision: None,
            gameplay_world: None,
            collision_templates: Vec::new(),
            collision_revision: 0,
            pushers: BTreeMap::new(),
            latest_game_snapshot: None,
            hash: [3; 32],
            derived_hash: [6; 32],
            bsp_hash: [8; 32],
            error: 0,
            spawn: None,
            session: None,
            snapshot: Vec::new(),
            compile_metrics: [0; 17],
            texture_inspections: [0; 2],
        });
        drop(guard);
        let old = encode(0, 1);
        assert_eq!(playsrc_result_length(old), 2);
        assert_eq!(playsrc_dispose(old), 1);
        let mut guard = slots().lock().unwrap();
        guard[0] = Slot {
            generation: 2,
            payload: Some(vec![4]),
            presentation: Vec::new(),
            coverage: Vec::new(),
            particles: None,
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            model_output: Vec::new(),
            visibility: None,
            area_state: None,
            visibility_output: Vec::new(),
            environment: None,
            collision: None,
            gameplay_world: None,
            collision_templates: Vec::new(),
            collision_revision: 0,
            pushers: BTreeMap::new(),
            latest_game_snapshot: None,
            hash: [5; 32],
            derived_hash: [7; 32],
            bsp_hash: [9; 32],
            error: 0,
            spawn: None,
            session: None,
            snapshot: Vec::new(),
            compile_metrics: [0; 17],
            texture_inspections: [0; 2],
        };
        drop(guard);
        assert_eq!(playsrc_result_length(old), 0);
        assert_eq!(playsrc_result_length(encode(0, 2)), 1);
    }

    #[test]
    fn command_and_snapshot_binary_contract_is_stable() {
        let mut bytes = vec![0; 48];
        bytes[..4].copy_from_slice(b"PCMD");
        bytes[4..8].copy_from_slice(&6_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&240_f32.to_le_bytes());
        bytes[16..20].copy_from_slice(&100_f32.to_le_bytes());
        bytes[24..28].copy_from_slice(&(-30_f32).to_le_bytes());
        bytes[28..32].copy_from_slice(&0xad_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x0202_0304_u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&77_u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&48_u32.to_le_bytes());
        let input = gameplay_protocol::decode(&bytes).unwrap();
        let command = input.command;
        assert_eq!(command.movement.forward, 240.);
        assert_eq!(command.up, 100.);
        assert_eq!(command.pitch_degrees, -30.);
        assert!(command.movement.jump && command.fire && command.reload && command.respawn);
        assert_eq!(command.mode_request, Some(playsrc_movement::Mode::Noclip));
        assert_eq!(command.activate_entity, Some(77));
        assert_eq!(
            command.select_class,
            Some(playsrc_tf2::PlayerClass::Demoman)
        );
        assert_eq!(command.select_team, Some(playsrc_tf2::PlayerTeam::Red));
        assert_eq!(
            command.select_weapon,
            Some(playsrc_tf2::Weapon::StickybombLauncher)
        );
        assert!(input.physics_results.is_empty());
        for class in playsrc_tf2::PlayerClass::ALL {
            let selection = 0x0202_0300_u32 | u32::from(class.source_number());
            bytes[32..36].copy_from_slice(&selection.to_le_bytes());
            assert_eq!(
                gameplay_protocol::decode(&bytes)
                    .unwrap()
                    .command
                    .select_class,
                Some(class)
            );
        }
        bytes[32..36].copy_from_slice(&0x0201_0303_u32.to_le_bytes());
        assert!(gameplay_protocol::decode(&bytes).is_none());
        bytes[32..36].copy_from_slice(&0x0202_030a_u32.to_le_bytes());
        assert!(gameplay_protocol::decode(&bytes).is_none());
        bytes[32..36].copy_from_slice(&0x0202_0304_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(gameplay_protocol::decode(&bytes).is_none());
        let movement = playsrc_movement::State::from_player(
            playsrc_movement::Player {
                position: [1., 2., 3.],
                velocity: [4., 5., 6.],
                grounded: true,
                crouched: false,
                jump_latched: true,
            },
            playsrc_movement::Policy::default(),
        );
        let projectile = playsrc_tf2::Projectile {
            identity: 12,
            kind: playsrc_tf2::ProjectileKind::Rocket,
            team: playsrc_tf2::PlayerTeam::Blue,
            owner_identity: 1,
            launcher_identity: 2,
            state: playsrc_tf2::ProjectileState::Flying,
            position: [7., 8., 9.],
            velocity: [10., 11., 12.],
            orientation: [0., 0., 0., 1.],
            angular_velocity: [0.; 3],
            contact_normal: None,
            age_seconds: 0.5,
        };
        let snapshot = playsrc_tf2::Snapshot {
            tick: 9,
            class: playsrc_tf2::PlayerClass::Soldier,
            team: playsrc_tf2::PlayerTeam::Blue,
            weapon: Some(playsrc_tf2::Weapon::Original),
            player_flags: playsrc_tf2::FL_CLIENT | playsrc_tf2::FL_INWATER,
            movement,
            health: 175.,
            maximum_health: 200.,
            spy: None,
            loadout: vec![playsrc_tf2::WeaponState {
                weapon: playsrc_tf2::Weapon::Original,
                clip: 3,
                reserve: 20,
                maximum_clip: 4,
                maximum_reserve: 20,
                reload: playsrc_tf2::ReloadState::Idle,
                next_primary_tick: 20,
                next_reload_tick: 0,
            }],
            conditions: 0,
            projectiles: vec![projectile.clone()],
            projectile_events: vec![playsrc_tf2::ProjectileEvent {
                kind: playsrc_tf2::ProjectileEventKind::Explode,
                projectile: 12,
                projectile_kind: playsrc_tf2::ProjectileKind::Rocket,
                owner_identity: 1,
                launcher_identity: 2,
                team: playsrc_tf2::PlayerTeam::Blue,
                tick: 9,
                position: projectile.position,
                orientation: projectile.orientation,
                contact_normal: None,
                launcher_pose: None,
            }],
            entity_transforms: Vec::new(),
            entity_events: Vec::new(),
            objectives: None,
            round: playsrc_tf2::round::Rules::active(playsrc_tf2::round::Configuration::default())
                .unwrap()
                .snapshot(Vec::new()),
            jump: None,
            events: vec![playsrc_tf2::Event::Teleported {
                trigger: 20,
                destination: 21,
                position: [13., 14., 15.],
                yaw_degrees: Some(90.),
            }],
            bots: Vec::new(),
        };
        let producer = playsrc_tf2::ProducerSnapshot {
            tick: 9,
            lifecycle: playsrc_tf2::PlayerLifecycle::Active,
            class: playsrc_tf2::PlayerClass::Soldier,
            team: playsrc_tf2::PlayerTeam::Blue,
            active_weapon: Some(playsrc_tf2::Weapon::Original),
            player_flags: playsrc_tf2::FL_CLIENT | playsrc_tf2::FL_INWATER,
            health: 175,
            maximum_health: 200,
            conditions: [0; 5],
            weapons: vec![playsrc_tf2::weapon::WeaponRuntime {
                weapon: playsrc_tf2::Weapon::Original,
                clip: 3,
                reserve: 20,
                reload: playsrc_tf2::weapon::ReloadPhase::Ready,
                next_primary_tick: 20,
                reload_due_tick: None,
                charge_begin_tick: None,
                first_primary_tick: 0,

                minigun_state: playsrc_tf2::weapon::MinigunState::Idle,
                spin_begin_tick: None,
                firing_begin_tick: None,
                idle_due_tick: None,
                smack_due_tick: None,
                charged_damage: 0.0,
                next_secondary_tick: 0,
                unzoom_due_tick: None,
                rezoom_due_tick: None,
                rezoom_after_shot: false,
            }],
            flame_points: Vec::new(),
            shotgun_pellets: Vec::new(),
            flame_firing: false,
            projectiles: vec![projectile],
            activities: vec![playsrc_tf2::weapon::ActivityEvent {
                tick: 9,
                weapon: playsrc_tf2::Weapon::Original,
                activity: playsrc_tf2::weapon::WeaponActivity::PrimaryAttack,
            }],
            lifecycle_events: Vec::new(),
            physics_requests: Vec::new(),
            rocket_trace_requests: Vec::new(),
            radius_damage_requests: Vec::new(),
            mover_requests: Vec::new(),
            contact_reconcile_requests: Vec::new(),
            map_effects: Vec::new(),
            regenerate_animation_events: Vec::new(),
        };
        let authority = playsrc_tf2::UniformRandomStream::from_seed(0)
            .unwrap()
            .state();
        let random_state = playsrc_tf2::Tf2RandomState {
            authority,
            predicted_presentation: authority,
            sound_selection: playsrc_tf2::SoundSelectionState {
                rocket_explosion_available: 7,
                sticky_explosion_available: 7,
                bat_hit_world_available: 3,

                shovel_hit_world_available: 3,
                shovel_hit_flesh_available: 7,
                fist_miss_available: 3,
                fist_hit_world_available: 3,
                fist_hit_flesh_available: 7,
                kukri_hit_flesh_available: 0b111,
                kukri_hit_world_available: 0b11,
                wrench_hit_flesh_available: 0b111,
                fire_axe_hit_world_available: 3,
                fire_axe_hit_flesh_available: 7,
                flag_enemy_stolen_available: 15,
                flag_enemy_dropped_available: 3,
                flag_enemy_captured_available: 7,
                flag_enemy_returned_available: 7,
                flag_team_dropped_available: 3,
                bottle_hit_flesh_available: 0b111,
                bottle_hit_world_available: 0b111,
                knife_hit_flesh_available: 7,
            },
        };
        let mut collision_snapshot = b"CSNP".to_vec();
        collision_snapshot.extend_from_slice(&1_u32.to_le_bytes());
        collision_snapshot.extend_from_slice(&9_u64.to_le_bytes());
        collision_snapshot.extend_from_slice(&0_u32.to_le_bytes());
        let entity_presentation = playsrc_tf2::EntityPresentationSnapshot {
            collision_revision: 9,
            entities: playsrc_entity::BrushModelPresentation {
                source_identity: 1,
                registry_identity: 2,
                tick: 9,
                revision: 1,
                models: Vec::new(),
            },
        };
        let encoded = encode_snapshot(
            &snapshot,
            &producer,
            2,
            None,
            SnapshotExtensions {
                random_state,
                random_draws: &[],
                audio_events: &[],
                rocket_results: &[],
                mover_results: &[],
                collision_snapshot: &collision_snapshot,
                entity_presentation: &entity_presentation,
                payload_constraint_blocked: false,
            },
        )
        .unwrap();
        assert_eq!(encoded.len(), 996);
        assert_eq!(&encoded[936..944], b"PCTF\x01\0\0\0");
        assert_eq!(&encoded[948..956], b"PGRL\x01\0\0\0");
        assert_eq!(
            u32::from_le_bytes(encoded[160..164].try_into().unwrap()),
            playsrc_tf2::FL_CLIENT | playsrc_tf2::FL_INWATER
        );
        assert_eq!(u32::from_le_bytes(encoded[56..60].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[60..64].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[64..68].try_into().unwrap()), 1);
        assert_eq!(&encoded[324..328], &[12, 0, 0, 0]);
        assert_eq!(&encoded[408..412], &[6, 1, 3, 0]);
        assert_eq!(&encoded[544..552], &[1, 1, 0, 0, 2, 1, 0, 0]);
        assert_eq!(&encoded[552..560], b"PRNG\x01\0\0\0");

        let constrained = encode_snapshot(
            &snapshot,
            &producer,
            2,
            None,
            SnapshotExtensions {
                random_state,
                random_draws: &[],
                audio_events: &[],
                rocket_results: &[],
                mover_results: &[],
                collision_snapshot: &collision_snapshot,
                entity_presentation: &entity_presentation,
                payload_constraint_blocked: true,
            },
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(constrained[124..128].try_into().unwrap()),
            3
        );
        assert_eq!(
            &constrained[544..556],
            &[1, 1, 0, 0, 2, 1, 0, 0, 3, 1, 0, 0]
        );
        assert_eq!(&constrained[556..564], b"PRNG\x01\0\0\0");
        assert_eq!(constrained.len(), encoded.len() + 4);
    }

    #[test]
    fn fixed_tick_continuation_retains_buttons_and_consumes_results_and_selectors() {
        let mut bytes = vec![0; 48 + 80];
        bytes[..4].copy_from_slice(b"PCMD");
        bytes[4..8].copy_from_slice(&6_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&240_f32.to_le_bytes());
        bytes[12..16].copy_from_slice(&(-120_f32).to_le_bytes());
        bytes[16..20].copy_from_slice(&100_f32.to_le_bytes());
        bytes[28..32].copy_from_slice(&0xff_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x0202_0304_u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&77_u32.to_le_bytes());
        bytes[40..42].copy_from_slice(&1_u16.to_le_bytes());
        bytes[42..44]
            .copy_from_slice(&(1_u16 | (2 << 2) | (1 << 7) | (2 << 9) | (1 << 11)).to_le_bytes());
        let byte_length = bytes.len() as u32;
        bytes[44..48].copy_from_slice(&byte_length.to_le_bytes());
        let continued = continuation_command(&bytes).unwrap();
        assert_eq!(continued.len(), 48);
        assert_eq!(
            &continued[8..20],
            &[0, 0, 112, 67, 0, 0, 240, 194, 0, 0, 200, 66]
        );
        assert_eq!(
            u32::from_le_bytes(continued[28..32].try_into().unwrap()),
            0xff
        );
        assert_eq!(u32::from_le_bytes(continued[32..36].try_into().unwrap()), 0);
        assert_eq!(
            u32::from_le_bytes(continued[36..40].try_into().unwrap()),
            u32::MAX
        );
        assert_eq!(u16::from_le_bytes(continued[40..42].try_into().unwrap()), 0);
        assert_eq!(u16::from_le_bytes(continued[42..44].try_into().unwrap()), 0);
        assert!(
            gameplay_protocol::decode(&continued)
                .unwrap()
                .command
                .bot_request
                .is_none()
        );
        assert_eq!(
            u32::from_le_bytes(continued[44..48].try_into().unwrap()),
            48
        );
    }
}
