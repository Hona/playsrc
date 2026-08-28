mod gameplay_protocol;
mod admission_metrics;
mod gameplay_replay;
mod memory;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod encoding_allocations;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[global_allocator]
static TEST_ALLOCATOR: encoding_allocations::Allocator = encoding_allocations::Allocator;
mod reply_output;
mod soundscapes;
mod acoustic_scene;
mod wearable;
mod map_particles;
mod smokestack;
mod legacy_visuals;
mod legacy_materials;
pub mod static_prop_artifact;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: memory::MeasuredAllocator = memory::MeasuredAllocator;

#[cfg(all(target_arch = "wasm32", feature = "threaded"))]
pub use wasm_bindgen_rayon::init_thread_pool;

use rayon::prelude::*;
use sha2::{Digest, Sha256};

const PRESENTATION_OUTPUT_LIMIT: usize = 512 * 1024 * 1024;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock, RwLock, Weak,
        atomic::{AtomicU32, Ordering},
    },
};
static SIMULATION_ERROR: AtomicU32 = AtomicU32::new(0);
static SIMULATION_ERROR_DETAIL: OnceLock<Mutex<String>> = OnceLock::new();
static GAME_ADVANCE_ERROR: AtomicU32 = AtomicU32::new(0);
static GAME_ADVANCE_DETAIL: OnceLock<Mutex<String>> = OnceLock::new();
static PRESENTATION_FAILURE_DETAIL: Mutex<String> = Mutex::new(String::new());
static PRESENTATION_MODEL_CACHE_HITS: AtomicU32 = AtomicU32::new(0);
static PRESENTATION_MODEL_CACHE_MISSES: AtomicU32 = AtomicU32::new(0);

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

#[derive(Default)]
struct PlayerHitboxModels {
    models: BTreeMap<String, Arc<RetainedPresentationModel>>,
    poses: BTreeMap<u32, (u64, playsrc_tf2::PlayerHitboxPose, Vec<playsrc_tf2::PosedPlayerHitbox>)>,
}

struct SharedWorld {
    player_hitboxes: Arc<Mutex<PlayerHitboxModels>>,
    world: Arc<playsrc_collision::World>,
    snapshot: Arc<RwLock<Arc<playsrc_collision::Snapshot>>>,
    impact_surfaces: Arc<BTreeMap<i16, Vec<(u32, u8, [f32; 4])>>>,
    movement_time: Arc<AtomicU32>,
    movement_queries: Arc<Mutex<playsrc_collision::QueryScratch>>,
}
impl Clone for SharedWorld {
    fn clone(&self) -> Self {
        Self {
            player_hitboxes: self.player_hitboxes.clone(),
            world: self.world.clone(),
            snapshot: self.snapshot.clone(),
            impact_surfaces: self.impact_surfaces.clone(),
            movement_time: self.movement_time.clone(),
            movement_queries: self.movement_queries.clone(),
        }
    }
}
impl SharedWorld {
    fn new(
        world: Arc<playsrc_collision::World>,
        snapshot: playsrc_collision::Snapshot,
        impact_surfaces: BTreeMap<i16, Vec<(u32, u8, [f32; 4])>>,
    ) -> Self {
        Self {
            player_hitboxes: Arc::new(Mutex::new(PlayerHitboxModels::default())),
            world,
            snapshot: Arc::new(RwLock::new(Arc::new(snapshot))),
            impact_surfaces: Arc::new(impact_surfaces),
            movement_time: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            movement_queries: Arc::new(Mutex::new(playsrc_collision::QueryScratch::default())),
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

    fn set_movement_time(&self, value: f32) {
        self.movement_time.store(value.to_bits(), Ordering::Relaxed);
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
            .trace_snapshot_hull_with_scratch(
                &snapshot,
                playsrc_collision::SnapshotTraceRequest {
                    start,
                    end,
                    hull,
                    mask,
                    scope: playsrc_collision::TraceScope::Everything,
                    ignored: &[],
                },
                &mut self
                    .movement_queries
                    .lock()
                    .expect("TF2 movement query scratch"),
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

    fn impact_surface(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        mask: u32,
    ) -> Result<Option<playsrc_movement::ImpactSurface>, playsrc_movement::Error> {
        let snapshot = self.snapshot();
        let trace = self
            .world
            .trace_snapshot_hull(
                &snapshot,
                playsrc_collision::SnapshotTraceRequest {
                    start,
                    end,
                    hull: playsrc_collision::Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
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
                    "impact surface collision",
                )
            })?;
        let Some(brush) = trace.brush.and_then(|index| self.world.brushes.get(index)) else {
            return Ok(None);
        };
        let Some(normal) = trace.plane.map(|plane| plane.normal) else {
            return Ok(None);
        };
        let mut selected: Option<(f32, u32, u8)> = None;
        for side in &self.world.sides[brush.first_side..brush.first_side + brush.side_count] {
            let Some(candidates) = self.impact_surfaces.get(&side.texture_info) else {
                continue;
            };
            for &(face, material, plane) in candidates {
                let alignment = dot(normal, [plane[0], plane[1], plane[2]]).abs();
                if alignment < 0.999 {
                    continue;
                }
                let distance = (dot(trace.end, [plane[0], plane[1], plane[2]]) - plane[3]).abs();
                if selected.is_none_or(|(prior, _, _)| distance < prior) {
                    selected = Some((distance, face, material));
                }
            }
        }
        Ok(
            selected.map(|(_, face, game_material)| playsrc_movement::ImpactSurface {
                game_material,
                surface_flags: trace.surface_flags,
                face: Some(face),
            }),
        )
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

    fn movement_time(&self) -> Option<f32> {
        Some(f32::from_bits(self.movement_time.load(Ordering::Relaxed)))
    }
}

impl playsrc_tf2::GameplayWorld for SharedWorld {
    fn has_player_hitbox_models(&self) -> bool { true }

    fn pose_player_hitboxes(&self, actors: &[playsrc_tf2::PlayerHitboxPose], tick: u64, interval: f32) -> Result<Vec<playsrc_tf2::PosedPlayerHitbox>, playsrc_movement::Error> {
        let mut retained = self.player_hitboxes.lock().expect("player hitbox models");
        retained.poses.retain(|identity, _| actors.iter().any(|actor| actor.identity == *identity));
        let mut output = Vec::new();
        for actor in actors {
            if retained.poses.get(&actor.identity).is_none_or(|(previous_tick, previous, _)| *previous_tick != tick || previous != actor) {
                let poses = pose_player_hitboxes(&retained.models, std::slice::from_ref(actor), tick, interval).map_err(|_| playsrc_movement::Error::new(
                    playsrc_movement::Operation::Trace, playsrc_movement::FailureKind::Malformed, "authored player hitbox pose"))?;
                retained.poses.insert(actor.identity, (tick, actor.clone(), poses));
            }
            output.extend_from_slice(&retained.poses[&actor.identity].2);
        }
        Ok(output)
    }

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

    fn overlaps_transformed_model_hull(&self, model:usize, transform:playsrc_entity::Transform, position:[f32;3], hull:playsrc_collision::Hull) -> Result<bool,playsrc_movement::Error> {
        self.world.overlaps_transformed_model_hull(model,playsrc_collision::Transform {origin:transform.origin,angles:transform.angles},position,hull)
            .map_err(|_|playsrc_movement::Error::new(playsrc_movement::Operation::Trace,playsrc_movement::FailureKind::Malformed,"transformed capture-area overlap"))
    }
}

#[derive(Clone)]
struct CollisionObjectTemplate {
    input: playsrc_collision::ObjectInput,
    runtime_transform: bool,
}

fn static_prop_collision_identity(source: usize) -> Result<u64, ()> {
    Ok(0x8000_0000_0000_0000 | u64::try_from(source).map_err(|_| ())?)
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

struct RuntimeRefractMaterial {
    material: playsrc_material::Material,
    normal_frame_count: u32,
}

struct RuntimeEnvironment {
    world: playsrc_map::WorldEnvironment,
    node_cull_modes: Vec<i8>,
    water_materials: BTreeMap<String, playsrc_material::Material>,
    refract_materials: BTreeMap<String, RuntimeRefractMaterial>,
    world_materials: BTreeMap<String, RuntimeWorldMaterial>,
    map_materials: BTreeMap<usize, String>,
    normal_frame_counts: BTreeMap<String, u32>,
    water_lod: Option<[f32; 2]>,
}

struct CombatDecalSurface {
    surface: playsrc_map::Surface,
    receiving: playsrc_material::DecalState,
}

#[derive(Clone)]
struct CombatDecalVariant {
    reference: String,
    offset: [f32; 2],
    scale: [f32; 2],
    dimensions: [f32; 2],
    weight: f32,
}

struct CombatDecalWorld {
    surfaces: BTreeMap<u32, CombatDecalSurface>,
    variants: BTreeMap<u8, Vec<CombatDecalVariant>>,
    random: playsrc_tf2::UniformRandomStream,
    serial: u32,
}

struct CombatDecal {
    identity: u32,
    reference: String,
    fragment: playsrc_map::MarkFragment,
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
    flex: Arc<playsrc_studio_model::ModelFlex>,
    model: Arc<RetainedPresentationModel>,
    identity: [u8; 32],
    illumination_position: playsrc_studio_model::Vector3,
    illumination_attachment: i32,
    eyes: Vec<playsrc_studio_model::EyeDefinition>,
}

struct CachedPresentationModel {
    flex: Arc<playsrc_studio_model::ModelFlex>,
    model: Weak<RetainedPresentationModel>,
    identity: [u8; 32],
    illumination_position: playsrc_studio_model::Vector3,
    illumination_attachment: i32,
    eyes: Vec<playsrc_studio_model::EyeDefinition>,
}

fn release_static_only_models<'a, T>(
    models: &mut BTreeMap<String, T>,
    static_identities: impl IntoIterator<Item = &'a str>,
    dynamic_identities: &std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let mut released = std::collections::BTreeSet::new();
    for identity in static_identities {
        if !dynamic_identities.contains(identity) && models.remove(identity).is_some() {
            released.insert(identity.to_owned());
        }
    }
    released
}

type PresentationModelCacheKey = (String, u8, u8);

fn presentation_model_cache()
-> &'static Mutex<BTreeMap<PresentationModelCacheKey, CachedPresentationModel>> {
    static CACHE: OnceLock<Mutex<BTreeMap<PresentationModelCacheKey, CachedPresentationModel>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[derive(Clone)]
struct StudioModelLightingMetadata {
    flex: Arc<playsrc_studio_model::ModelFlex>,
    position: playsrc_studio_model::Vector3,
    attachment: i32,
    eyes: Vec<playsrc_studio_model::EyeDefinition>,
}

#[derive(Clone, Copy)]
struct PresentationInputs<'a, 'source> {
    canonical: &'a playsrc_map::CanonicalMap,
    bsp: &'a playsrc_bsp::Bsp,
    graph: &'a playsrc_entity::Graph,
    bundle: &'a BTreeMap<String, &'source [u8]>,
    decoders: &'a TextureDecoders<'source>,
    resource_hashes: &'a BTreeMap<String, [u8; 32]>,
    map_materials: &'a [playsrc_material::Material],
    particle_presentation: &'a BTreeMap<String, CompiledParticlePresentation>,
    profile: playsrc_map::LightingProfile,
    visibility: &'a playsrc_visibility::World,
    collision: &'a playsrc_collision::World,
    additional_model_roots: &'a [String],
}

#[derive(Default)]
struct ClassPreview {
    model: String,
    scene: playsrc_tf2::class_selection::ScenePlayer,
    flex: playsrc_tf2::class_selection::ModelPanelFlexState,
}

struct Slot {
    legacy_visuals: legacy_visuals::Runtime,
    generation: u16,
    payload: Option<Vec<u8>>,
    payload_bytes: usize,
    presentation: Vec<u8>,
    presentation_bytes: usize,
    coverage: Vec<u8>,
    particles: Option<playsrc_particle::ParticleWorld>,
    map_particles: Option<(playsrc_particle::ParticleWorld, map_particles::MapParticles)>,
    smokestacks: Option<smokestack::Frames>,
    legacy_visual_output: Vec<u8>,
    particle_materials: Vec<String>,
    particle_sheets: BTreeMap<String, playsrc_particle::ParticleMaterial>,
    combat_decals: Option<CombatDecalWorld>,
    particle_output: Vec<u8>,
    studio_models: BTreeMap<String, Arc<RetainedPresentationModel>>,
    model_lighting_metadata: BTreeMap<String, StudioModelLightingMetadata>,
    model_lighting_world: Option<playsrc_map::ModelLightingWorld<'static>>,
    model_material_opacity: BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    viewmodel_bob: BTreeMap<u32, playsrc_studio_model::ViewModelBobState>,
    weapon_animations: BTreeMap<u32, weapon_pose::AnimationState>,
    class_scenes: BTreeMap<u32, ClassPreview>,
    wearable_particles: wearable::ParticleStates,
    model_output: Vec<u8>,
    visibility: Option<playsrc_visibility::World>,
    soundscapes: playsrc_audio::soundscape::ZoneIndex,
    acoustic_scene: Option<acoustic_scene::Scene>,
    acoustic_output: Vec<u8>,
    visibility_candidates: Option<playsrc_visibility::CandidateSet>,
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
    snapshot: Arc<[u8]>,
    compile_metrics: [u64; 17],
    memory_metrics: [usize; 13],
    texture_inspections: [u32; 2],
    model_cache: [u32; 2],
}

struct Tf2Simulation {
    handle: u32,
    current_command: Option<Arc<[u8]>>,
}

fn continuation_command(command: &[u8]) -> Result<Arc<[u8]>, playsrc_simulation::SimulationError> {
    let mut continuation = command
        .get(..gameplay_protocol::HEADER_BYTES)
        .ok_or_else(|| playsrc_simulation::SimulationError::new("command", "continuation"))?
        .to_vec();
    continuation[32..36].copy_from_slice(&0_u32.to_le_bytes());
    continuation[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    continuation[40..42].fill(0);
    let nextbot_stop = u16::from_le_bytes([continuation[42], continuation[43]]) & 0x8000;
    continuation[42..44].copy_from_slice(&nextbot_stop.to_le_bytes());
    continuation[44..48].fill(0);
    continuation[48..52].copy_from_slice(&(gameplay_protocol::HEADER_BYTES as u32).to_le_bytes());
    continuation[52..gameplay_protocol::HEADER_BYTES].fill(0);
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
            let latest_bot = u16::from_le_bytes(
                merged[42..44]
                    .try_into()
                    .map_err(|_| playsrc_simulation::SimulationError::new("command", "bot"))?,
            );
            let nextbot_stop = latest_bot & 0x8000;
            let mut bot = latest_bot & 0x7fff;
            let mut bot_configuration =
                u32::from_le_bytes(merged[44..48].try_into().map_err(|_| {
                    playsrc_simulation::SimulationError::new("command", "bot-configuration")
                })?);
            let mut objective_configuration =
                u32::from_le_bytes(merged[52..56].try_into().map_err(|_| {
                    playsrc_simulation::SimulationError::new("command", "objective-configuration")
                })?);
            let mut control = <[u8; 28]>::try_from(&merged[56..84])
                .map_err(|_| playsrc_simulation::SimulationError::new("command", "bot-control"))?;
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
                let next_bot = u16::from_le_bytes(
                    value.bytes[42..44]
                        .try_into()
                        .map_err(|_| playsrc_simulation::SimulationError::new("command", "bot"))?,
                );
                if next_bot & 0x7fff != 0 {
                    bot = next_bot & 0x7fff;
                }
                let next_bot_configuration =
                    u32::from_le_bytes(value.bytes[44..48].try_into().map_err(|_| {
                        playsrc_simulation::SimulationError::new("command", "bot-configuration")
                    })?);
                if next_bot_configuration != 0 {
                    bot_configuration = next_bot_configuration;
                }
                let next_objective_configuration =
                    u32::from_le_bytes(value.bytes[52..56].try_into().map_err(|_| {
                        playsrc_simulation::SimulationError::new(
                            "command",
                            "objective-configuration",
                        )
                    })?);
                if next_objective_configuration != 0 {
                    objective_configuration = next_objective_configuration;
                }
                if value.bytes.get(56).copied().unwrap_or_default() != 0 {
                    control.copy_from_slice(value.bytes.get(56..84).ok_or_else(|| {
                        playsrc_simulation::SimulationError::new("command", "bot-control")
                    })?);
                }
            }
            merged[28..32].copy_from_slice(&flags.to_le_bytes());
            merged[32..36].copy_from_slice(&selectors.to_le_bytes());
            merged[36..40].copy_from_slice(&activate.to_le_bytes());
            merged[42..44].copy_from_slice(&(bot | nextbot_stop).to_le_bytes());
            merged[44..48].copy_from_slice(&bot_configuration.to_le_bytes());
            merged[52..56].copy_from_slice(&objective_configuration.to_le_bytes());
            let preferences = input.commands.iter().rev().find_map(|command| command.bytes.get(57).copied().filter(|value| *value != 0)).unwrap_or(0);
            merged[56..84].copy_from_slice(&control);
            merged[57] = preferences;
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
        let replay_started = gameplay_replay::tick_started(self.handle);
        if unsafe { playsrc_game_advance(self.handle, command.as_ptr(), command.len(), 1) } != 1 {
            return Err(playsrc_simulation::SimulationError::new(
                "tf2-transition",
                format!(
                    "TF2 gameplay transition failed at game-advance:{}{}",
                    GAME_ADVANCE_ERROR.load(Ordering::Relaxed),
                    GAME_ADVANCE_DETAIL
                        .get_or_init(|| Mutex::new(String::new()))
                        .lock()
                        .expect("game advance detail")
                ),
            ));
        }
        let snapshot = with(self.handle, |slot| slot.snapshot.clone()).ok_or_else(|| {
            playsrc_simulation::SimulationError::new(
                "stale-handle",
                "TF2 gameplay handle is unavailable",
            )
        })?;
        gameplay_replay::tick(
            self.handle,
            input.context.host_tick,
            &command,
            &snapshot,
            replay_started,
        );
        Ok(playsrc_simulation::TickOutput {
            snapshot: snapshot.clone(),
            events: snapshot,
        })
    }
    fn shutdown(&mut self) -> Result<(), playsrc_simulation::SimulationError> {
        self.current_command = None;
        Ok(())
    }
}

struct SimulationHostEntry {
    host: playsrc_simulation::FixedStepHost<Tf2Simulation, RuntimeMetricsClock>,
    output: Vec<u8>,
    snapshots: snapshot_transport::Encoder,
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
mod snapshot_transport;
fn slots() -> &'static Mutex<Vec<Slot>> {
    static S: OnceLock<Mutex<Vec<Slot>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}
fn resource_output() -> &'static Mutex<Vec<u8>> {
    static OUTPUT: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    OUTPUT.get_or_init(|| Mutex::new(Vec::new()))
}
fn resource_backings() -> &'static Mutex<BTreeMap<usize, Arc<Vec<u8>>>> {
    static BACKINGS: OnceLock<Mutex<BTreeMap<usize, Arc<Vec<u8>>>>> = OnceLock::new();
    BACKINGS.get_or_init(|| Mutex::new(BTreeMap::new()))
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
    unsafe { decode_resource_batch(pointer, length, false) }
}

#[unsafe(no_mangle)]
/// Decode the browser's authenticated immutable resource graph acquisition.
///
/// # Safety
/// A nonempty pointer/length pair must identify readable bytes in this module's memory.
/// The caller must have authenticated each descriptor against the selected resource
/// graph and each exact encoded object against that descriptor before transferring it.
pub unsafe extern "C" fn playsrc_resource_decode_authenticated(
    pointer: *const u8,
    length: usize,
) -> u32 {
    unsafe { decode_resource_batch(pointer, length, true) }
}

unsafe fn decode_resource_batch(pointer: *const u8, length: usize, authenticated: bool) -> u32 {
    let bytes = if length == 0 {
        &[]
    } else if pointer.is_null() {
        return 0;
    } else {
        unsafe { std::slice::from_raw_parts(pointer, length) }
    };
    let decoded = if authenticated {
        playsrc_asset_graph::decode_authenticated_resource_set(bytes)
    } else {
        playsrc_asset_graph::decode_to_resource_set(bytes)
    };
    let Ok(decoded) = decoded else {
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
    let bytes = Arc::new(std::mem::take(&mut *output));
    let pointer = bytes.as_ptr() as *mut u8;
    playsrc_studio_model::register_authored_backing(&bytes);
    resource_backings()
        .lock()
        .expect("resource backings")
        .insert(pointer as usize, bytes);
    pointer
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_resource_release(pointer: *const u8, length: usize) -> u32 {
    let mut backings = resource_backings().lock().expect("resource backings");
    match backings.get(&(pointer as usize)) {
        Some(bytes) if bytes.len() == length => {
            backings.remove(&(pointer as usize));
            1
        }
        _ => 0,
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ResourceSection {
    pub pointer: *const u8,
    pub length: usize,
}

unsafe fn resource_sections<'a>(
    pointer: *const ResourceSection,
    count: usize,
) -> Result<BTreeMap<String, &'a [u8]>, ()> {
    if pointer.is_null() || count == 0 || count > playsrc_asset_graph::MAX_GRAPH_CHUNKS {
        return Err(());
    }
    let sections = unsafe { std::slice::from_raw_parts(pointer, count) };
    let mut resources = BTreeMap::new();
    let mut total = 0usize;
    for section in sections {
        if section.pointer.is_null() || section.length < 12 || section.length > 1024 * 1024 * 1024 {
            return Err(());
        }
        total = total.checked_add(section.length).ok_or(())?;
        if total > 1024 * 1024 * 1024 {
            return Err(());
        }
        let bytes = unsafe { std::slice::from_raw_parts(section.pointer, section.length) };
        for (identity, value) in bundle(bytes)? {
            if resources.insert(identity, value).is_some() || resources.len() > 4_096 {
                return Err(());
            }
        }
    }
    Ok(resources)
}

fn resource_set_identity(resources: &BTreeMap<String, &[u8]>) -> Result<(usize, [u8; 32]), ()> {
    let mut hash = Sha256::new();
    hash.update(b"PSRE");
    hash.update(1_u32.to_le_bytes());
    hash.update(
        u32::try_from(resources.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    let mut length = 12usize;
    for (identity, bytes) in resources {
        let path_length = u32::try_from(identity.len()).map_err(|_| ())?;
        let byte_length = u32::try_from(bytes.len()).map_err(|_| ())?;
        hash.update(path_length.to_le_bytes());
        hash.update(identity.as_bytes());
        hash.update(byte_length.to_le_bytes());
        hash.update(bytes);
        length = length
            .checked_add(8)
            .and_then(|value| value.checked_add(identity.len()))
            .and_then(|value| value.checked_add(bytes.len()))
            .ok_or(())?;
    }
    (length <= 1024 * 1024 * 1024)
        .then_some((length, hash.finalize().into()))
        .ok_or(())
}

#[unsafe(no_mangle)]
/// # Safety
/// Every section must identify readable module memory and `output` must contain 32 writable bytes.
pub unsafe extern "C" fn playsrc_resource_sections_hash(
    pointer: *const ResourceSection,
    count: usize,
    output: *mut u8,
) -> usize {
    if output.is_null() {
        return 0;
    }
    let Ok(resources) = (unsafe { resource_sections(pointer, count) }) else {
        return 0;
    };
    let Ok((length, hash)) = resource_set_identity(&resources) else {
        return 0;
    };
    unsafe { std::ptr::copy_nonoverlapping(hash.as_ptr(), output, hash.len()) };
    length
}

#[unsafe(no_mangle)]
/// # Safety
/// Each nonempty pointer/length pair must identify readable bytes in this module's memory.
pub unsafe extern "C" fn playsrc_compile_map(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const ResourceSection,
    configuration_count: usize,
    configuration_sha256_pointer: *const u8,
    retain_payload: u32,
) -> u32 {
    unsafe {
        compile_map(
            bsp_pointer,
            bsp_length,
            profile,
            configuration_pointer,
            configuration_count,
            configuration_sha256_pointer,
            None,
            retain_payload,
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
    configuration_pointer: *const ResourceSection,
    configuration_count: usize,
    configuration_sha256_pointer: *const u8,
    presentation_pointer: *const u8,
    presentation_length: usize,
    retain_payload: u32,
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
            configuration_count,
            configuration_sha256_pointer,
            presentation,
            retain_payload,
        )
    }
}

unsafe fn compile_map(
    bsp_pointer: *const u8,
    bsp_length: usize,
    profile: u32,
    configuration_pointer: *const ResourceSection,
    configuration_count: usize,
    configuration_sha256_pointer: *const u8,
    cached_presentation: Option<&[u8]>,
    retain_payload: u32,
) -> u32 {
    PRESENTATION_FAILURE_DETAIL.lock().unwrap().clear();
    PRESENTATION_MODEL_CACHE_HITS.store(0, Ordering::Relaxed);
    PRESENTATION_MODEL_CACHE_MISSES.store(0, Ordering::Relaxed);
    let mut metrics_clock = RuntimeMetricsClock::new();
    let compile_started =
        playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    let mut phase_started = compile_started;
    let mut compile_metrics = [0_u64; 17];
    let mut memory_metrics = [0_usize; 13];
    memory_metrics[0] = memory::live_bytes();
    let bsp_bytes = if bsp_length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bsp_pointer, bsp_length) }
    };
    let result = (|| {
        let mut bsp = playsrc_bsp::parse(
            bsp_bytes,
            playsrc_bsp::Profile::Source2013V20,
            playsrc_bsp::Limits::default(),
        )
        .map_err(|_| 1_u32)?;
        let bsp_sha: [u8; 32] = Sha256::digest(bsp_bytes).into();
        memory_metrics[1] = memory::live_bytes();
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[0] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let profile = match profile {
            0 => playsrc_map::LightingProfile::Ldr,
            1 => playsrc_map::LightingProfile::Hdr,
            _ => return Err(2),
        };
        let retain_payload = match retain_payload {
            0 => false,
            1 => true,
            _ => return Err(2),
        };
        let resources = unsafe { resource_sections(configuration_pointer, configuration_count) }
            .map_err(|_| 7_u32)?;
        if configuration_sha256_pointer.is_null() {
            return Err(7);
        }
        let configuration_sha256: [u8; 32] = unsafe {
            std::slice::from_raw_parts(configuration_sha256_pointer, 32)
                .try_into()
                .map_err(|_| 7_u32)?
        };
        let decoders = TextureDecoders::new(&resources);
        let resource_hashes = resources
            .par_iter()
            .map(|(identity, bytes)| (identity.clone(), Sha256::digest(bytes).into()))
            .collect::<BTreeMap<_, _>>();
        let entity_graph =
            playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default())
                .map_err(|_| 3_u32)?;
        let legacy_visuals = legacy_visuals::Runtime::new(legacy_visuals::World::compile(&entity_graph,&resources,&decoders,profile).map_err(|_| 9_u32)?);
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
        bsp.release_lump_payload(8);
        bsp.release_lump_payload(53);
        memory_metrics[2] = memory::live_bytes();
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
        memory_metrics[3] = memory::live_bytes();
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[2] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[3] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let (particles, particle_materials, particle_sheets, particle_presentation) =
            compile_particles(&resources, &decoders, &playsrc_tf2::particle_resources::roots(&entity_graph), &smokestack_materials(&entity_graph)?).map_err(|_| 10_u32)?;
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
            resource_hashes: &resource_hashes,
            map_materials: &map_materials,
            particle_presentation: &particle_presentation,
            profile,
            visibility: &visibility_world,
            collision: &collision_world,
            additional_model_roots: &static_model_roots,
        };
        let (
            (
                presentation,
                mut studio_models,
                mut model_lighting_metadata,
                mut model_material_opacity,
                environment,
            ),
            presentation_metrics,
            _presentation_ledger,
        ) = if let Some(cached) = cached_presentation {
            load_cached_presentation(presentation_inputs, cached).map_err(|_| 9_u32)?
        } else {
            compile_presentation(presentation_inputs).map_err(|_| 9_u32)?
        };
        memory_metrics[4] = memory::live_bytes();
        let registry = surface_property_registry(&resources).map_err(|_| 5_u32)?;
        let acoustic_scene = acoustic_scene::Scene::new(acoustic_scene::Materials::compile(
            &canonical, &bsp, &map_materials, &resources, &registry).map_err(|_| 5_u32)?);
        drop(bsp);
        memory_metrics[10] = memory::live_bytes();
        compile_metrics[11..17].copy_from_slice(&presentation_metrics);
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[4] = phase_finished.saturating_sub(phase_started);
        phase_started = phase_finished;
        let (runtime_models, model_occurrences) = resolve_models(
            Some(&entity_graph),
            &studio_models,
            &resources,
            &decoders,
            profile,
            Some(&canonical.static_props),
        )
        .map_err(|_| 8_u32)?;
        memory_metrics[5] = memory::live_bytes();
        let studio_model_checksums = studio_models
            .iter()
            .map(|(identity, model)| (identity.clone(), model.checksum))
            .collect();
        let dynamic_entity_models = entity_graph
            .entities
            .iter()
            .map(authored_entity_model)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| 8_u32)?
            .into_iter()
            .flatten()
            .collect::<std::collections::BTreeSet<_>>();
        let released_static_models = release_static_only_models(
            &mut studio_models,
            canonical
                .static_props
                .models
                .iter()
                .map(|model| model.logical_path.as_str()),
            &dynamic_entity_models,
        );
        for identity in released_static_models {
            model_lighting_metadata.remove(&identity);
            model_material_opacity.remove(&identity);
        }
        presentation_model_cache()
            .lock()
            .expect("presentation model cache")
            .retain(|_, entry| entry.model.strong_count() > 0);
        memory_metrics[11] = memory::live_bytes();
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
        let mut runtime = playsrc_map::assemble_prepared_runtime(
            canonical,
            entity_graph,
            collision_world,
            visibility_world,
            bsp_sha,
            playsrc_map::RuntimeAssembly {
                retain_payload,
                compiler_identity: if profile == playsrc_map::LightingProfile::Hdr {
                    if displacement_runtime {
                        "playsrc-map-runtime-hdr-2"
                    } else {
                        "playsrc-map-runtime-hdr-1"
                    }
                } else {
                    if displacement_runtime {
                        "playsrc-map-runtime-3"
                    } else {
                        "playsrc-map-runtime-2"
                    }
                },
                configuration_sha256,
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
        memory_metrics[6] = memory::live_bytes();
        memory_metrics[12] = runtime.descriptor.payload.capacity();
        drop(runtime_models);
        drop(model_occurrences);
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
        let collision_templates = collision_object_templates(
            &runtime.map,
            &runtime.entities,
            &resources,
            &studio_model_checksums,
        )
        .map_err(|_| 5_u32)?;
        let model_lighting_world = playsrc_map::ModelLightingWorld::new(runtime.map.lighting);
        let visibility = runtime.visibility;
        let area_state = playsrc_map::compile_area_portal_state(&runtime.entities, &visibility)
            .map_err(|_| 3_u32)?;
        let collision = Arc::new(runtime.collision);
        let initial_collision = compile_collision_snapshot(
            None,
            &collision,
            &collision_templates,
            1,
            None,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .map_err(|_| 5_u32)?;
        let mut impact_surfaces = BTreeMap::<i16, Vec<(u32, u8, [f32; 4])>>::new();
        for surface in &runtime.map.surfaces {
            let Some(material) = map_materials.get(surface.material) else {
                return Err(5_u32);
            };
            let property = registry
                .resolve(material.surface_property.as_deref())
                .or_else(|| registry.resolve(Some(b"default")))
                .ok_or(5_u32)?;
            let texture = i16::try_from(surface.texture_info).map_err(|_| 5_u32)?;
            impact_surfaces.entry(texture).or_default().push((
                u32::try_from(surface.face).map_err(|_| 5_u32)?,
                property.game_material,
                surface.plane,
            ));
        }
        let decals = combat_decal_world(
            std::mem::take(&mut runtime.map.surfaces),
            &map_materials,
            &resources,
            &decoders,
        )
        .map_err(|_| 5_u32)?;
        let gameplay_world =
            SharedWorld::new(collision.clone(), initial_collision, impact_surfaces);
        memory_metrics[7] = memory::live_bytes();
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
        let mut map = playsrc_tf2::MapRuntime::compile(
            &runtime.entities,
            playsrc_movement::Configuration::default().tick_interval,
            u64::from_le_bytes(bsp_sha[..8].try_into().expect("BSP identity prefix")),
            model_bounds,
        )
        .map_err(|_| 5_u32)?;
        let soundscapes = soundscapes::prepare(&resources, &runtime.entities, &mut map, &visibility, bsp_sha)
            .map_err(|_| 5_u32)?;
        map.install_studio_models(&studio_models.iter().map(|(identity, model)|
            (identity.clone(), Arc::clone(model.source()))).collect())
            .map_err(|_| 5_u32)?;
        let mut sprite_models=BTreeMap::new();
        for entity in &runtime.entities.entities {
            if !entity.classname.as_deref().is_some_and(playsrc_entity::sprite::is_sprite) { continue; }
            let model=entity_scalar(entity,b"model").ok_or(9_u32)?;
            let path=playsrc_entity::visual_resources::sprite_material(std::str::from_utf8(model).map_err(|_|9_u32)?).ok_or(9_u32)?;
            let material=resolve_material_semantics(&path,&resources,material_environment(profile,false)).map_err(|_|9_u32)?;
            let (_,_,metadata)=selected_texture(&material,&decoders).map_err(|_|9_u32)?;
            sprite_models.insert(model.to_vec(),u32::from(metadata.frame_count));
        }
        map.install_sprite_models(sprite_models).map_err(|_|9_u32)?;
        map.install_spotlights(Arc::clone(&collision),collision_templates.iter().map(|template|template.input.clone()).collect()).map_err(|_|9_u32)?;
        let rules = playsrc_tf2::team_selection::TeamRules {
            attack_defend: map.control_points().is_some_and(|points| !points.rounds().is_empty() || points.master().switch_teams) || runtime.entities.entities.iter().any(|entity| {
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
        session.set_initial_view_angles(spawn.angles);
        session.restore_equipment(&local_equipment().lock().expect("local equipment").persist()).map_err(|_| 11_u32)?;
        if let Some((_, bytes)) = resources
            .iter()
            .find(|(identity, _)| identity.starts_with("maps/") && identity.ends_with(".nav"))
        {
            let mesh = playsrc_nav::parse(
                bytes,
                playsrc_nav::Profile::TeamFortress2,
                Some(u32::try_from(bsp_length).map_err(|_| 11_u32)?),
                playsrc_nav::Limits::default(),
            )
            .map_err(|_| 11_u32)?;
            session
                .configure_navigation(mesh, &runtime.entities)
                .map_err(|error| {
                    #[cfg(not(target_arch = "wasm32"))]
                    eprintln!("TF2 navigation configuration failed: {error:?}");
                    #[cfg(target_arch = "wasm32")]
                    let _ = error;
                    11_u32
                })?;
        }
        session.set_movement_modifiers(playsrc_tf2::MovementModifiers {
            noclip_allowed: true,
            ..playsrc_tf2::MovementModifiers::default()
        });
        memory_metrics[8] = memory::live_bytes();
        memory_metrics[9] = memory::high_water_bytes();
        let phase_finished =
            playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
        compile_metrics[9] = phase_finished.saturating_sub(phase_started);
        compile_metrics[10] = phase_finished.saturating_sub(compile_started);
        Ok((
            runtime.descriptor.payload,
            runtime.descriptor.payload_bytes,
            runtime.descriptor.payload_sha256,
            runtime.descriptor.derived_sha256,
            presentation,
            coverage,
            particles,
            particle_materials,
            particle_sheets,
            decals,
            studio_models,
            model_lighting_metadata,
            model_lighting_world,
            model_material_opacity,
            environment,
            visibility,
            soundscapes,
            acoustic_scene,
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
            legacy_visuals,
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
            payload_bytes,
            hash,
            derived_hash,
            presentation,
            coverage,
            particles,
            particle_materials,
            particle_sheets,
            decals,
            studio_models,
            model_lighting_metadata,
            model_lighting_world,
            model_material_opacity,
            environment,
            visibility,
            soundscapes,
            acoustic_scene,
            area_state,
            collision,
            gameplay_world,
            collision_templates,
            bsp_hash,
            spawn,
            session,
            texture_inspections,
            legacy_visuals,
        )) => Slot {
            generation,
            payload: Some(payload),
            payload_bytes,
            presentation_bytes: cached_presentation.map_or(presentation.len(), <[u8]>::len),
            presentation,
            coverage,
            legacy_visuals,
            map_particles: (!session.map_particle_systems().is_empty())
                .then(|| (particles.independent(), map_particles::MapParticles::default())),
            smokestacks: (!session.map_smokestacks().is_empty()).then(|| smokestack::Frames::new(
                (playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut RuntimeMetricsClock::new()) as u32 & 0x7fff_ffff) as i32)),
            legacy_visual_output: Vec::new(),
            particles: Some(particles),
            particle_materials,
            particle_sheets,
            combat_decals: Some(decals),
            particle_output: Vec::new(),
            studio_models,
            model_lighting_metadata,
            model_lighting_world: Some(model_lighting_world),
            model_material_opacity,
            viewmodel_bob: BTreeMap::new(),
            weapon_animations: BTreeMap::new(),
            class_scenes: BTreeMap::new(),
            wearable_particles: wearable::ParticleStates::default(),
            model_output: Vec::new(),
            visibility_candidates: playsrc_visibility::CandidateSet::compile(&visibility, 0, &[])
                .ok(),
            visibility: Some(visibility),
            soundscapes,
            acoustic_scene: Some(acoustic_scene),
            acoustic_output: Vec::new(),
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
            snapshot: Arc::from([]),
            compile_metrics,
            memory_metrics,
            texture_inspections,
            model_cache: [
                PRESENTATION_MODEL_CACHE_HITS.load(Ordering::Relaxed),
                PRESENTATION_MODEL_CACHE_MISSES.load(Ordering::Relaxed),
            ],
        },
        Err(error) => Slot {
            generation,
            payload: Some(Vec::new()),
            payload_bytes: 0,
            presentation: Vec::new(),
            presentation_bytes: 0,
            coverage: Vec::new(),
            particles: None,
            map_particles: None,
            smokestacks: None,
            legacy_visual_output: Vec::new(),
            legacy_visuals: legacy_visuals::Runtime::default(),
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            combat_decals: None,
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_lighting_metadata: BTreeMap::new(),
            model_lighting_world: None,
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            weapon_animations: BTreeMap::new(),
            class_scenes: BTreeMap::new(),
            wearable_particles: wearable::ParticleStates::default(),
            model_output: Vec::new(),
            visibility: None,
            soundscapes: Default::default(),
            acoustic_scene: None,
            acoustic_output: Vec::new(),
            visibility_candidates: None,
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
            snapshot: Arc::from([]),
            compile_metrics,
            memory_metrics,
            texture_inspections: [0; 2],
            model_cache: [0; 2],
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

/// Bounded native acceptance through the same compiled-map transaction used by
/// the browser. This does not replace headed presentation or timing evidence.
#[cfg(not(target_arch = "wasm32"))]
pub fn verify_control_point_match(bsp: &[u8], resources: &[u8], mut observe: impl FnMut(&playsrc_tf2::Snapshot)) -> Result<(), String> {
    let section = ResourceSection { pointer: resources.as_ptr(), length: resources.len() };
    let hash: [u8; 32] = Sha256::digest(resources).into();
    let handle = unsafe { playsrc_compile_map(bsp.as_ptr(), bsp.len(), 1, &section, 1, hash.as_ptr(), 1) };
    let result = (|| {
        let failure = playsrc_result_error(handle);
        if failure != 0 { return Err(format!("map compilation failed: {failure}")); }
        let (index, _) = decode(handle).ok_or("invalid map handle")?;
        let mut command = [0_u8; gameplay_protocol::HEADER_BYTES];
        command[..4].copy_from_slice(b"PCMD");
        command[4..8].copy_from_slice(&9_u32.to_le_bytes());
        command[48..52].copy_from_slice(&(gameplay_protocol::HEADER_BYTES as u32).to_le_bytes());
        command[32..36].copy_from_slice(&(3_u32 | (2 << 16)).to_le_bytes());
        command[42..44].copy_from_slice(&(1_u16 | (15 << 2) | (1 << 7) | (2 << 11) | (2 << 13)).to_le_bytes());
        for batch in 0..189 {
            if unsafe { playsrc_game_advance(handle, command.as_ptr(), command.len(), if batch == 0 { 1 } else if batch == 188 { 31 } else { 64 }) } == 0 {
                return Err(format!("gameplay transaction failed: {}", GAME_ADVANCE_ERROR.load(Ordering::Relaxed)));
            }
            command[32..36].fill(0); command[42..44].fill(0);
            let values = slots().lock().expect("TF2 slots");
            let snapshot = values[index].latest_game_snapshot.as_ref().ok_or("missing gameplay snapshot")?;
            observe(snapshot);
            if snapshot.round.winning_team == Some(playsrc_tf2::PlayerTeam::Red) { return Ok(()); }
        }
        let values = slots().lock().expect("TF2 slots");
        let paths = values[index].session.as_ref().and_then(|session| session.bot_world()).map_or_else(Vec::new, |bots|bots.navigation_diagnostics());
        Err(format!("walking bots did not complete the control-point round within 180 simulation seconds: {}", paths.join("\n")))
    })();
    playsrc_dispose(handle);
    result
}

pub fn compile_artifact(
    bsp: &[u8],
    profile: u32,
    configuration: &[u8],
) -> Result<CompiledArtifact, u32> {
    let section = ResourceSection {
        pointer: configuration.as_ptr(),
        length: configuration.len(),
    };
    let configuration_sha256: [u8; 32] = Sha256::digest(configuration).into();
    let handle = unsafe {
        playsrc_compile_map(
            bsp.as_ptr(),
            bsp.len(),
            profile,
            &section,
            1,
            configuration_sha256.as_ptr(),
            1,
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
        compile_particles(&resources, &decoders, &playsrc_tf2::particle_resources::roots(&entities), &smokestack_materials(&entities)?).map_err(|_| 10u32)?;
    let ((_, models, _, _, _), metrics, mut ledger) = compile_presentation(PresentationInputs {
        canonical: &canonical,
        bsp: &bsp,
        graph: &entities,
        bundle: &resources,
        decoders: &decoders,
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
    ledger.displacement_input_count = collision.displacement_input_count();
    ledger.static_prop_collision_count = templates
        .iter()
        .filter(|template| template.input.role == playsrc_collision::ObjectRole::StaticProp)
        .count();
    ledger.phase_milliseconds = metrics.map(|nanoseconds| nanoseconds / 1_000_000);
    Ok(ledger)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_length(handle: u32) -> usize {
    with(handle, |slot| slot.payload_bytes).unwrap_or(0)
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
pub extern "C" fn playsrc_memory_bytes(index: usize) -> usize {
    match index {
        0 => memory::live_bytes(),
        1 => memory::high_water_bytes(),
        2 => playsrc_studio_model::authored_source_residency().0,
        3 => playsrc_studio_model::authored_source_residency().1,
        4 => playsrc_studio_model::authored_source_residency().2,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_compile_memory_bytes(handle: u32, index: usize) -> usize {
    with(handle, |slot| {
        slot.memory_metrics.get(index).copied().unwrap_or(0)
    })
    .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_texture_inspection_count(handle: u32, index: usize) -> u32 {
    with(handle, |slot| {
        slot.texture_inspections.get(index).copied().unwrap_or(0)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_model_cache_count(handle: u32, index: usize) -> u32 {
    with(handle, |slot| {
        slot.model_cache.get(index).copied().unwrap_or(0)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_presentation_length(handle: u32) -> usize {
    with(handle, |slot| slot.presentation_bytes).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_presentation_take(handle: u32) -> *mut u8 {
    let Some((index, generation)) = decode(handle) else {
        return std::ptr::null_mut();
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return std::ptr::null_mut();
    };
    if slot.generation != generation || slot.presentation.is_empty() {
        return std::ptr::null_mut();
    }
    slot.presentation_bytes = 0;
    Box::into_raw(std::mem::take(&mut slot.presentation).into_boxed_slice()) as *mut u8
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
    slot.presentation_bytes = 0;
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
    let Ok((mut events, request, legacy_frame, visibility_view, visibility_samples)) = decode_particle_transaction(bytes) else {
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
    if let Some(frame) = legacy_frame {
        return transact_legacy_particle_frame(slot, request, frame);
    }
    let Some(collision_world) = slot.collision.clone() else {
        return 0;
    };
    slot.wearable_particles.retain_gameplay(slot.latest_game_snapshot.as_ref(), request.to_seconds);
    let map_systems = slot.session.as_ref().map(|session| session.map_particle_systems()).unwrap_or_default();
    let Some(world) = slot.particles.as_mut() else {
        return 0;
    };
    let (Some(visibility), Some(lighting)) =
        (slot.visibility.as_ref(), slot.model_lighting_world.as_ref())
    else {
        return 0;
    };
    let mut collision = ParticleCollision {
        world: collision_world,
        visibility,
        lighting,
        lighting_cache: BTreeMap::new(),
    };
    if !slot.wearable_particles.pending.is_empty() {
        events.extend(slot.wearable_particles.pending.iter().cloned());
        events.sort_by(|left, right| left.timestamp_seconds.total_cmp(&right.timestamp_seconds));
        for (order, event) in events.iter_mut().enumerate() { event.source_order = order as u32; }
    }
    let wearable_controls = slot.wearable_particles.controls.iter().map(|(id, cp)| (*id, cp.clone())).collect::<Vec<_>>();
    let encode = |items: Vec<playsrc_particle::RenderItem>, bounds| {
        let items = playsrc_particle::resolve_render_output(items, &slot.particle_sheets)?;
        playsrc_particle::encode_render_output(&items, bounds, &slot.particle_materials, 64 * 1024 * 1024)
    };
    let result = if let Some((map_world, map_state)) = slot.map_particles.as_mut() {
        let mut candidate = map_state.clone();
        let sky = slot.environment.as_ref().and_then(|environment| environment.world.controllers.iter().find_map(|controller| {
            match controller.state { playsrc_map::ControllerState::SkyCamera { area, origin, scale, .. } => Some((area, origin, scale)), _ => None }
        }));
        let (map_events, attached) = candidate.prepare(&map_systems, map_world, request, |position| {
            visibility.locate_leaf(position).ok().and_then(|leaf| visibility.leaves.get(leaf))
                .is_some_and(|leaf| Some(usize::from(leaf.area_and_flags & 0x1ff)) == sky.map(|sky| sky.0))
        });
        let sky_position = sky.map(|(_, origin, scale)| std::array::from_fn(|axis| origin[axis] + request.camera_position[axis] / scale.max(1) as f32));
        let mut map_collision = ParticleCollision { world: collision.world.clone(), visibility, lighting, lighting_cache: BTreeMap::new() };
        world.transact(&events, &wearable_controls, &visibility_samples, visibility_view, request, &mut collision, |game_items, game_bounds| {
            map_world.transact_views(&map_events, &attached, &visibility_samples, visibility_view, request,
                |identity| if candidate.is_sky(identity) { sky_position.expect("sky effect controller") } else { request.camera_position },
                &mut map_collision, |mut items, map_bounds| {
                candidate.classify(&mut items);
                items.extend(game_items);
                let bounds = match (map_bounds, game_bounds) {
                    (Some(left), Some(right)) => Some(playsrc_particle::Bounds {
                        minimum: std::array::from_fn(|axis| left.minimum[axis].min(right.minimum[axis])),
                        maximum: std::array::from_fn(|axis| left.maximum[axis].max(right.maximum[axis])),
                    }),
                    (left, right) => left.or(right),
                };
                encode(items, bounds)
            })
        }).inspect(|_| *map_state = candidate)
    } else {
        world.transact(&events, &wearable_controls, &visibility_samples, visibility_view, request, &mut collision, encode)
    };
    let output = match result {
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
    slot.wearable_particles.pending.clear();
    slot.wearable_particles.controls.clear();
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_legacy_particle_frames(handle: u32) -> u32 {
    with(handle, |slot| u32::from(slot.smokestacks.is_some()||slot.legacy_visuals.required())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_legacy_visual_output_length(handle: u32) -> usize {
    with(handle, |slot| slot.legacy_visual_output.len()).unwrap_or(0)
}

#[cfg(not(target_arch="wasm32"))]
pub fn map_sun_presentation(handle:u32,source:u32)->Option<playsrc_entity::sun::Presentation>{with(handle,|slot|slot.session.as_ref()?.map_sun_state(source)).flatten()}
#[cfg(not(target_arch="wasm32"))]
pub fn map_spotlight_presentation(handle:u32,source:u32)->Option<(playsrc_entity::spotlight::Beam,playsrc_entity::EntityRenderState)>{with(handle,|slot|slot.session.as_ref()?.map_spotlight_state(source)).flatten()}

/// Select a real, PVS-admitted opaque-world occluder for the headed map probe.
/// This does not mutate entities, collision, render depth, or particle state.
#[cfg(not(target_arch = "wasm32"))]
pub fn smokestack_occlusion_probe(handle: u32) -> Option<(usize, [f32; 3], [f32; 2], f32)> {
    with(handle, |slot| {
        let (world, visibility, area, candidates, environment, session) = (slot.collision.as_ref()?, slot.visibility.as_ref()?,
            slot.area_state.as_ref()?, slot.visibility_candidates.as_ref()?, slot.environment.as_ref()?, slot.session.as_ref()?);
        for state in session.map_smokestacks() {
            let origin = state.transform.origin;
            let source_leaf = visibility.locate_leaf(origin).ok()?;
            let source_area = visibility.leaves[source_leaf].area_and_flags & 0x1ff;
            let target = [origin[0], origin[1], origin[2] + state.parameters.jet_length * 0.5];
            for distance in [128.0, 256.0, 512.0] {
                for offset in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]] {
                    let position = [target[0] + offset[0] * distance, target[1] + offset[1] * distance, target[2]];
                    let camera_leaf = visibility.locate_leaf(position).ok()?;
                    if visibility.leaves[camera_leaf].cluster < 0 || visibility.leaves[camera_leaf].area_and_flags & 0x1ff != source_area { continue; }
                    let trace = world.trace_hull(position, target, playsrc_collision::Hull { mins: [0.0; 3], maxs: [0.0; 3] }, 1).ok()?;
                    if trace.start_solid || trace.all_solid || !(0.1..0.9).contains(&trace.fraction)
                        || trace.surface_flags & (0x80 | 0x4) != 0 || !matches!(trace.hit, Some(playsrc_collision::Hit::WorldBrush { .. })) { continue; }
                    let yaw = (target[1] - position[1]).atan2(target[0] - position[0]).to_degrees();
                    let input = [position[0], position[1], position[2], position[0], position[1], position[2], yaw, 0.0, 75.0, 16.0 / 9.0, 1.0, 32_768.0, 0.0, -1.0];
                    let view = visibility.view(area, candidates, &playsrc_visibility::ViewQuery { origins: vec![position], bypass_pvs: false }).ok()?;
                    let view = smokestack::RenderView::from_query(&input, &view.leaves, visibility, &environment.node_cull_modes).ok()?;
                    let point = playsrc_particle::Bounds { minimum: origin, maximum: origin };
                    if !view.in_pvs(visibility, point) || view.render_leaf(visibility, point, point).is_none() { continue; }
                    let bounds = playsrc_particle::Bounds { minimum: [origin[0] - 16.0, origin[1] - 16.0, origin[2]], maximum: [origin[0] + 16.0, origin[1] + 16.0, origin[2] + state.parameters.jet_length] };
                    if view.render_leaf(visibility, bounds, bounds).is_none() { continue; }
                    let covered = (0..8).all(|corner| {
                        let point = std::array::from_fn(|axis| if corner & (1 << axis) == 0 { bounds.minimum[axis] } else { bounds.maximum[axis] });
                        world.trace_hull(position, point, playsrc_collision::Hull { mins: [0.0; 3], maxs: [0.0; 3] }, 1)
                            .is_ok_and(|hit| !hit.start_solid && hit.fraction < 0.95 && hit.surface_flags & (0x80 | 0x4) == 0 && matches!(hit.hit, Some(playsrc_collision::Hit::WorldBrush { .. })))
                    });
                    if covered { return Some((state.source, position, [yaw, 0.0], trace.fraction)); }
                }
            }
        }
        None
    }).flatten()
}

#[derive(Clone, Copy, Debug)]
struct LegacyParticleFrame<'a> { seconds: f32, accepted: u32, identity: u32, view: [f32; 4], visual_payload: &'a [u8] }

fn transact_legacy_particle_frame(slot: &mut Slot, request: playsrc_particle::AdvanceRequest, frame: LegacyParticleFrame<'_>) -> u32 {
    let visuals=if frame.visual_payload.is_empty() {
        if slot.legacy_visuals.required() {return 0;} None
    } else {
        match legacy_visuals::prepare(slot,frame.visual_payload,frame.identity,frame.accepted,frame.seconds) {
            Ok(value)=>Some(value),Err(())=>{
                *SIMULATION_ERROR_DETAIL.get_or_init(||Mutex::new(String::new())).lock().expect("legacy frame error detail")="Invalid native legacy visual client frame".into();return 0;
            }
        }
    };
    if slot.smokestacks.is_none() {
        let Some((state,bytes))=visuals else {return 0;};
        let Ok(particles)=playsrc_particle::encode_render_output(&[],None,&slot.particle_materials,64*1024*1024) else {return 0;};
        slot.legacy_visuals=state;slot.legacy_visual_output=bytes;slot.particle_output=particles;return 1;
    }
    let (Some(smoke), Some(visibility)) = (slot.smokestacks.as_mut(), slot.visibility.as_ref()) else { return 0; };
    let sky = slot.environment.as_ref().and_then(|environment| environment.world.controllers.iter().find_map(|controller| {
        match controller.state { playsrc_map::ControllerState::SkyCamera { area, origin, scale, .. } => Some((area, origin, scale)), _ => None }
    }));
    for index in 0..if sky.is_some() { 2 } else { 1 } {
        let (origin, position, area_filter) = if index == 0 { (request.camera_position, request.camera_position, -1.0) }
            else { let (area, origin, scale) = sky.unwrap(); (origin, std::array::from_fn(|axis| origin[axis] + request.camera_position[axis] / scale.max(1) as f32), area as f32) };
        let input = [origin[0], origin[1], origin[2], position[0], position[1], position[2],
            frame.view[0], frame.view[1], frame.view[2], frame.view[3], 1.0, 30_000.0, request.to_seconds, area_filter];
        if smoke.views[index].as_ref().is_none_or(|view| !view.matches(&input)) {
            let (Some(area), Some(candidates)) = (slot.area_state.as_ref(), slot.visibility_candidates.as_ref()) else { return 0; };
            let Ok(view) = visibility.view(area, candidates, &playsrc_visibility::ViewQuery { origins: vec![origin], bypass_pvs: false }) else { return 0; };
            let leaves: Vec<_> = view.leaves.into_iter().filter(|&leaf| index == 0 || usize::from(visibility.leaves[leaf].area_and_flags & 0x1ff) == sky.unwrap().0).collect();
            let Some(environment) = slot.environment.as_ref() else { return 0; };
            let Ok(view) = smokestack::RenderView::from_query(&input, &leaves, visibility, &environment.node_cull_modes) else { return 0; };
            smoke.views[index] = Some(view);
        }
    }
    let views = smoke.views.clone();
    let mut candidate = smoke.candidate(frame.accepted);
    let states = slot.session.as_ref().map(|session| session.map_smokestacks()).unwrap_or_default();
    let items = candidate.advance(&states, request.to_seconds, frame.seconds,
        |sky| views[usize::from(sky)].as_ref().expect("owning particle view").camera,
        |position| visibility.locate_leaf(position).ok().and_then(|leaf| visibility.leaves.get(leaf))
            .is_some_and(|leaf| Some(usize::from(leaf.area_and_flags & 0x1ff)) == sky.map(|sky| sky.0)),
        |sky, bounds, registered| views[usize::from(sky)].as_ref().and_then(|view| view.render_leaf(visibility, bounds, registered)),
        |state| views[0].as_ref().unwrap().in_pvs(visibility, playsrc_particle::Bounds { minimum: state.transform.origin, maximum: state.transform.origin }));
    let result = playsrc_particle::resolve_render_output(items, &slot.particle_sheets)
        .and_then(|items| playsrc_particle::encode_render_output(&items, None, &slot.particle_materials, 64 * 1024 * 1024));
    match result {
        Ok(output) => {
            smoke.prepare(frame.identity,candidate);slot.particle_output=output;
            if let Some((state,bytes))=visuals {slot.legacy_visuals=state;slot.legacy_visual_output=bytes;} else {slot.legacy_visual_output.clear();}
            1
        }
        Err(error) => {
            *SIMULATION_ERROR_DETAIL.get_or_init(|| Mutex::new(String::new())).lock().expect("particle error detail") = error.to_string();
            0
        }
    }
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

#[derive(Clone, Debug)]
struct ModelPoseRequest {
    barrel_angle: Option<f32>,
    item_definition: Option<u32>,
    activity_start_tick: Option<u64>,
    allow_idle_transition: bool,
    identity: u32,
    control_point: Option<u32>,
    class_selection: bool,
    model_panel: bool,
    model_panel_reset: bool,
    flex_controllers: Option<BTreeMap<String, f32>>,
    actor_identity: u32,
    preparation: bool,
    hud_model: bool,
    cloak: Option<playsrc_tf2::spy::CloakRenderState>,
    world_item: bool,
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
    equipped_items: Vec<playsrc_tf2::equipment::EquippedItem>,
    lighting: Option<ModelPoseLightingRequest>,
}

#[derive(Clone, Copy, Debug)]
struct ModelPoseLightingRequest {
    origin: [f32; 3],
    angles: [f32; 3],
    camera: [f32; 3],
    camera_angles: [f32; 3],
}

mod equipment_models;
mod weapon_pose;

struct ModelPoseWorld<'a> {
    metadata: &'a BTreeMap<String, StudioModelLightingMetadata>,
    lighting: Option<&'a mut playsrc_map::ModelLightingWorld<'static>>,
    visibility: Option<&'a playsrc_visibility::World>,
    collision: Option<&'a playsrc_collision::World>,
    snapshot: Option<&'a playsrc_collision::Snapshot>,
    gameplay: Option<&'a playsrc_tf2::Snapshot>,
    cubemaps: &'a [playsrc_map::CubemapSample],
    particle_inputs: Option<wearable::ParticleInputs<'a>>,
    wearable_particles: &'a mut wearable::ParticleStates,
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
    if handle == 0 { return u32::from(equipment_models::transact(&requests).is_ok()); }
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
    let Some(gameplay) = slot.gameplay_world.as_ref() else {
        return 0;
    };
    let snapshot = gameplay.snapshot();
    let Some(lighting) = slot.model_lighting_world.as_mut() else {
        return 0;
    };
    let Some(visibility) = slot.visibility.as_ref() else {
        return 0;
    };
    let Some(collision) = slot.collision.as_ref() else {
        return 0;
    };
    let Some(environment) = slot.environment.as_ref() else {
        return 0;
    };
    let mut wearable_particles = slot.wearable_particles.clone();
    let mut weapon_animations = slot.weapon_animations.clone();
    wearable_particles.retain(&requests);
    let mut world = ModelPoseWorld {
        metadata: &slot.model_lighting_metadata,
        lighting: Some(lighting),
        visibility: Some(visibility),
        collision: Some(collision),
        snapshot: Some(&snapshot),
        gameplay: slot.latest_game_snapshot.as_ref(),
        cubemaps: &environment.world.cubemaps,
        particle_inputs: slot.particles.as_ref().map(|template| wearable::ParticleInputs { template, materials: &slot.particle_sheets, identities: &slot.particle_materials }),
        wearable_particles: &mut wearable_particles,
    };
    let Ok(output) = encode_model_poses(
        &slot.studio_models,
        &slot.model_material_opacity,
        &mut slot.viewmodel_bob,
        &mut slot.class_scenes,
        &mut weapon_animations,
        &requests,
        &mut world,
        std::mem::take(&mut slot.model_output),
    ) else {
        return 0;
    };
    slot.model_output = output;
    slot.wearable_particles = wearable_particles;
    slot.weapon_animations = weapon_animations;
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_model_output_length(handle: u32) -> usize {
    if handle == 0 { return equipment_models::output_length(); }
    with(handle, |slot| slot.model_output.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_model_output_capacity(handle: u32) -> usize {
    if handle == 0 { return equipment_models::output_capacity(); }
    with(handle, |slot| slot.model_output.capacity()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_model_output_take(handle: u32) -> *mut u8 {
    if handle == 0 { return equipment_models::output_take(); }
    let Some((index, generation)) = decode(handle) else {
        return std::ptr::null_mut();
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return std::ptr::null_mut();
    };
    if slot.generation != generation || slot.model_output.is_empty() {
        return std::ptr::null_mut();
    }
    let mut bytes = std::mem::take(&mut slot.model_output);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}

#[unsafe(no_mangle)]
/// # Safety
/// The exact pointer and capacity from `playsrc_model_output_take` must be returned once,
/// only after every browser read has finished. A stale map handle releases the allocation.
pub unsafe extern "C" fn playsrc_model_output_recycle(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) {
    if pointer.is_null() {
        return;
    }
    let bytes = unsafe { Vec::from_raw_parts(pointer, 0, capacity) };
    if handle == 0 { equipment_models::recycle(bytes); return; }
    let Some((index, generation)) = decode(handle) else {
        return;
    };
    let mut slots = slots().lock().expect("TF2 slots");
    if let Some(slot) = slots.get_mut(index)
        && slot.generation == generation
        && slot.model_output.is_empty()
        && slot.model_output.capacity() < capacity
    {
        slot.model_output = bytes;
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable bytes of at least `capacity`.
pub unsafe extern "C" fn playsrc_model_output_copy(
    handle: u32,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    if handle == 0 { return unsafe { equipment_models::copy_output(pointer, capacity) }; }
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
    if reader.take(4)? != b"PMRQ" || reader.u32()? != 12 {
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
        let actor_identity = reader.u32()?;
        let local_factor = reader.f32()?;
        let world_factor = reader.f32()?;
        let raw_factor = reader.f32()?;
        let player_tint = [reader.f32()?, reader.f32()?, reader.f32()?];
        if [local_factor, world_factor, raw_factor]
            .into_iter()
            .chain(player_tint)
            .any(|value| !(0.0..=1.0).contains(&value) || (actor_identity == 0 && value != 0.0))
        {
            return Err(());
        }
        let sample_tick = reader.u64()?;
        let kind = reader.u8()?;
        let attachments_only = reader.u8()?;
        let has_fire_view = reader.u8()?;
        let flags = reader.u8()?;
        let model_panel_reset = flags & 1;
        let has_cloak = flags & 2 != 0;
        let preparation = flags & 4 != 0;
        let hud_model = flags & 8 != 0;
        if flags > 15 || (has_cloak && actor_identity == 0) || (hud_model && (!matches!(kind, 3 | 4 | 6) || actor_identity == 0)) || (!has_cloak && [local_factor, world_factor, raw_factor].into_iter().chain(player_tint).any(|value| value != 0.0)) { return Err(()); }
        let cloak = has_cloak.then_some(playsrc_tf2::spy::CloakRenderState { local_factor, world_factor, raw_factor, player_tint });
        let definition = reader.u32()?;
        let item_definition = (definition != u32::MAX).then_some(definition);
        let start_tick = reader.u64()?;
        let activity_start_tick = (start_tick != u64::MAX).then_some(start_tick);
        if kind > 7
            || (kind == 7 && (actor_identity == 0 || has_cloak || [local_factor, world_factor, raw_factor].into_iter().chain(player_tint).any(|v| v != 0.0)))
            || (matches!(kind, 3 | 4 | 6) && actor_identity != 0 && !hud_model)
            || attachments_only > 1
            || has_fire_view > 1
            || (attachments_only == 1 && (kind != 1 || has_fire_view != 1))
            || (has_fire_view == 1 && kind != 1)
            || model_panel_reset > 1 || (model_panel_reset != 0 && !matches!(kind, 3 | 4 | 6))
            || item_definition.is_some_and(|definition| playsrc_tf2::equipment::supported_item(definition).is_none())
            || item_definition.is_some() && !matches!(kind, 3 | 4 | 5 | 6) && activity_start_tick.is_none()
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
            0 | 2 | 3 | 4 | 7 if item_text.is_empty() => None,
            1 | 5 | 6 if !item_text.is_empty() => Some(item_text),
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
        let idle_transition = reader.u8()?;
        if reflected_viewmodel > 1 || owner_alive > 1 || idle_transition > 1 {
            return Err(());
        }
        let phase = match phase_code {
            0 => Some(playsrc_studio_model::ViewModelPhase::Draw),
            1 => Some(playsrc_studio_model::ViewModelPhase::PrimaryFire),
            2 => Some(playsrc_studio_model::ViewModelPhase::ReloadStart),
            3 => Some(playsrc_studio_model::ViewModelPhase::ReloadInsertOrLoop),
            4 => Some(playsrc_studio_model::ViewModelPhase::ReloadFinish),
            5 => Some(playsrc_studio_model::ViewModelPhase::Idle),
            u8::MAX if matches!(kind, 0 | 3 | 4 | 5 | 6 | 7) => None,
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
        let equipped_count = reader.u32()?;
        if equipped_count > 19 { return Err(()); }
        let mut equipped_items = Vec::with_capacity(equipped_count as usize);
        for _ in 0..equipped_count {
            let item_id = reader.u32()?;
            let definition_index = reader.u32()?;
            let quality = reader.u8()?;
            let style = reader.u8()?;
            let slot = playsrc_tf2::schema::LoadoutPosition::try_from(reader.u8()?).map_err(|_| ())?;
            let attribute_count = reader.u8()?;
            if item_id == 0 || quality > 15 || attribute_count > 64 || equipped_items.iter().any(|item: &playsrc_tf2::equipment::EquippedItem| item.item_id == item_id) { return Err(()); }
            let attributes = (0..attribute_count).map(|_| Ok(playsrc_tf2::equipment::ItemAttribute { definition: reader.u32()?, value: reader.f32()? })).collect::<Result<Vec<_>, ()>>()?;
            equipped_items.push(playsrc_tf2::equipment::EquippedItem { item_id, definition_index, quality, style, slot, attributes });
        }
        let has_lighting = reader.u8()?;
        if has_lighting > 1 || reader.take(3)? != [0; 3] {
            return Err(());
        }
        let lighting_origin = [reader.f32()?, reader.f32()?, reader.f32()?];
        let lighting_angles = [reader.f32()?, reader.f32()?, reader.f32()?];
        let lighting_camera = [reader.f32()?, reader.f32()?, reader.f32()?];
        let lighting_camera_angles = [reader.f32()?, reader.f32()?, reader.f32()?];
        let lighting = if has_lighting == 1 {
            Some(ModelPoseLightingRequest {
                origin: lighting_origin,
                angles: lighting_angles,
                camera: lighting_camera,
                camera_angles: lighting_camera_angles,
            })
        } else {
            if lighting_origin
                .into_iter()
                .chain(lighting_angles)
                .chain(lighting_camera)
                .chain(lighting_camera_angles)
                .any(|value| value != 0.0)
            {
                return Err(());
            }
            None
        };
        identities.insert(identity, (sample_tick, attachments_only == 1));
        requests.push(ModelPoseRequest {
            barrel_angle: None,
            item_definition,
            activity_start_tick,
            allow_idle_transition: idle_transition != 0,
            identity,
            control_point: (kind == 7).then_some(actor_identity),
            class_selection: kind == 3,
            model_panel: matches!(kind, 3 | 4 | 6),
            model_panel_reset: model_panel_reset != 0,
            flex_controllers: None,
            actor_identity: if kind == 7 { 0 } else { actor_identity },
            cloak: if kind == 7 { None } else { cloak },
            preparation,
            hud_model,
            world_item: matches!(kind, 5 | 6),
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
            equipped_items,
            lighting,
        });
    }
    (reader.at == bytes.len()).then_some(requests).ok_or(())
}

fn pose_player_hitboxes(
    models: &BTreeMap<String, Arc<RetainedPresentationModel>>,
    actors: &[playsrc_tf2::PlayerHitboxPose],
    tick: u64,
    interval: f32,
) -> Result<Vec<playsrc_tf2::PosedPlayerHitbox>, String> {
    let mut output = Vec::new();
    for bot in actors {
        let model = models.get(bot.class.data().model).ok_or_else(|| {
            format!(
                "missing model {} for bot {}",
                bot.class.data().model,
                bot.identity
            )
        })?;
        let moving =
            (bot.velocity[0] * bot.velocity[0] + bot.velocity[1] * bot.velocity[1]).sqrt() > 1.0;
        let activity = playsrc_tf2::weapon_presentation::world_activity(bot.definition, bot.class,
            if moving { "ACT_MP_RUN" } else { "ACT_MP_STAND_IDLE" }).ok_or_else(|| format!("missing player weapon activity {}", bot.definition))?;
        let sequence =
            *playsrc_studio_model::sequences_for_activity_name(model, activity.as_bytes())
                .first()
                .ok_or_else(|| {
                    format!(
                        "missing activity {} for bot {} model {}",
                        activity,
                        bot.identity,
                        bot.class.data().model
                    )
                })?;
        let parameters = model
            .pose_parameters
            .iter()
            .map(|_| playsrc_studio_model::Float32(0))
            .collect::<Vec<_>>();
        let elapsed = tick as f32 * interval;
        let timing = playsrc_studio_model::sequence_timing(model, sequence, &parameters)
            .map_err(|error| format!("bot {} sequence timing: {error:?}", bot.identity))?;
        let pose = playsrc_studio_model::sample_pose_at_time(
            model,
            &playsrc_studio_model::AnimationState {
                bone_rotations: Vec::new(),
                base_sequence: sequence,
                cycle: playsrc_studio_model::Float32(pose_cycle(elapsed, timing).to_bits()),
                pose_parameters: parameters,
                layers: Vec::new(),
            },
            playsrc_studio_model::Float32(elapsed.to_bits()),
        )
        .map_err(|error| format!("bot {} sampled pose: {error:?}", bot.identity))?;
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
        let world = playsrc_studio_model::apply_entity_transform(model, &pose, matrix)
            .map_err(|error| format!("bot {} world pose: {error:?}", bot.identity))?;
        let Some(set) = model.hitbox_sets.first() else {
            continue;
        };
        for hitbox in &set.hitboxes {
            let bone_index = usize::try_from(hitbox.bone)
                .map_err(|_| format!("bot {} invalid hitbox bone {}", bot.identity, hitbox.bone))?;
            let bone = model.bones.get(bone_index).ok_or_else(|| {
                format!("bot {} missing hitbox bone {}", bot.identity, bone_index)
            })?;
            let transform = world.bone_matrices.get(bone_index).ok_or_else(|| {
                format!(
                    "bot {} missing hitbox transform {}",
                    bot.identity, bone_index
                )
            })?;
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
    models: &BTreeMap<String, Arc<RetainedPresentationModel>>,
    material_opacity: &BTreeMap<String, Vec<playsrc_studio_model::ViewModelMaterialOpacity>>,
    viewmodel_bob: &mut BTreeMap<u32, playsrc_studio_model::ViewModelBobState>,
    class_scenes: &mut BTreeMap<u32, ClassPreview>,
    weapon_animations: &mut BTreeMap<u32, weapon_pose::AnimationState>,
    requests: &[ModelPoseRequest],
    world: &mut ModelPoseWorld<'_>,
    mut out: Vec<u8>,
) -> Result<Vec<u8>, ()> {
    out.clear();
    out.extend_from_slice(b"PMPO");
    out.extend_from_slice(&11u32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    let mut output_count = 0_u32;
    let mut sampled_poses =
        BTreeMap::<(String, usize, u32, u32, Vec<u32>, Option<u32>), playsrc_studio_model::SampledPose>::new();
    for original in requests {
        let mut scene_request;
        let mut scene_event_clock = None;
        let request = if original.class_selection {
            let scene = playsrc_tf2::class_selection::scene_for_model(&original.model).ok_or(())?;
            let retained = class_scenes.entry(original.identity).or_default();
            if retained.model != original.model || original.model_panel_reset {
                retained.model = original.model.clone();
                retained.scene = Default::default();
            }
            let controllers = retained.flex.decay(&world.metadata.get(&original.model).ok_or(())?.flex.controllers);
            let sample = retained.scene.advance(scene, original.elapsed, controllers).map_err(|_| ())?;
            scene_event_clock = sample.event_sequence.map(|sequence| (sequence, sample.event_elapsed));
            scene_request = original.clone();
            scene_request.flex_controllers = Some(sample.controllers);
            if let Some(sequence) = sample.sequence { scene_request.activity = sequence.to_owned(); }
            scene_request.elapsed = sample.sequence_elapsed;
            scene_request.previous_elapsed = (sample.sequence_elapsed - original.frame_time).max(0.0);
            &scene_request
        } else { original };
        let resolved_weapon;
        let request = if let Some(resolved) = weapon_pose::prepare(request, models, weapon_animations, world.gameplay)? {
            resolved_weapon = resolved;
            &resolved_weapon
        } else { request };
        let model = models.get(&request.model).ok_or(())?;
        let control_point = if let Some(identity) = request.control_point {
            let points = world.gameplay.and_then(|g| g.control_points.as_ref()).ok_or(())?;
            let point = points.points.iter().find(|p| p.identity == identity).ok_or(())?;
            if point.models[point.owner as usize] != request.model { return Err(()); }
            let area = points.areas.iter().rev().find(|a| a.point == point.index);
            Some((point,area,points.configuration))
        } else { None };
        let mut bodygroups = if let Some(body) = request.packed_body {
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
        let wearables = request.equipped_items.iter().filter_map(|item| match wearable::model(item, &request.model) {
            Ok(Some(model)) => Some(Ok((item, model))), Ok(None) => None, Err(()) => Some(Err(())),
        }).collect::<Result<Vec<_>, ()>>()?;
        if !wearables.is_empty() {
            if request.phase.is_some() { return Err(()); }
            if let Some(index) = model.body_parts.iter().position(|part| part.name.eq_ignore_ascii_case(b"hat")) {
                if bodygroups.len() != model.body_parts.len() { return Err(()); }
                bodygroups[index] = 1;
            }
        }
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
            .map(|parameter| {
                let mut value: f32 = if request.class_selection && parameter.name.eq_ignore_ascii_case(b"move_x") { 1.0 } else { 0.0 };
                if let Some((point,area,configuration)) = control_point {
                    for team in [playsrc_tf2::PlayerTeam::Red, playsrc_tf2::PlayerTeam::Blue] {
                        if parameter.name.eq_ignore_ascii_case(format!("cappoint_{}_percentage",team as u8).as_bytes()) {
                            value = if let Some(a) = area.filter(|a| a.capturing_team.is_gameplay()) {
                                if a.capturing_team == team { a.progress(configuration) } else if point.owner == team { 1.0-a.progress(configuration) } else { 0.0 }
                            } else if point.owner == team { 1.0 } else { 0.0 };
                        }
                    }
                }
                playsrc_studio_model::Float32(value.to_bits())
            })
            .collect::<Vec<_>>();
        let timing = playsrc_studio_model::sequence_timing(model, sequence, &pose_parameters)
            .map_err(|_| ())?;
        let previous_cycle = pose_cycle(request.previous_elapsed, timing);
        let cycle = pose_cycle(request.elapsed, timing);
        let class_pose_parameters = request.class_selection.then(|| pose_parameters.clone());
        if let Some(item_identity) = request.item.as_ref().filter(|_| !request.world_item) {
            let item = models.get(item_identity).ok_or(())?;
            let frame = playsrc_studio_model::produce_viewmodel_frame(
                model,
                item,
                &playsrc_studio_model::ViewModelFrameRequest {
                    phase: request.phase.ok_or(())?,
                    previous_cycle: playsrc_studio_model::Float32(previous_cycle.to_bits()),
                    composition: playsrc_studio_model::ViewModelCompositionRequest {
                        item_bone_rotations: weapon_pose::bone_rotations(request, item),
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
                    None,
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
                    world,
                )?;
                part_count = part_count.checked_add(1).ok_or(())?;
            }
            if part_count != if request.attachments_only { 1 } else { 2 } {
                return Err(());
            }
            output_count = output_count.checked_add(part_count).ok_or(())?;
        } else {
            let pose_key = (
                request.model.clone(),
                sequence,
                cycle.to_bits(),
                request.elapsed.to_bits(),
                pose_parameters.iter().map(|p| p.0).collect(),
                request.barrel_angle.map(f32::to_bits),
            );
            if let std::collections::btree_map::Entry::Vacant(entry) =
                sampled_poses.entry(pose_key.clone())
            {
                let (base_sequence, base_cycle, layers) = if request.class_selection {
                    let base = *playsrc_studio_model::sequences_for_activity_name(model, original.activity.as_bytes()).first().ok_or(())?;
                    let base_timing = playsrc_studio_model::sequence_timing(model, base, &pose_parameters).map_err(|_| ())?;
                    let base_cycle = original.elapsed * f32::from_bits(timing.frames_per_second.0)
                        / (f32::from_bits(base_timing.weighted_frame_count.0) - 1.0).max(1.0);
                    (base, base_cycle - base_cycle.trunc(), vec![playsrc_studio_model::AnimationLayer {
                        sequence, cycle: playsrc_studio_model::Float32(cycle.to_bits()), weight: playsrc_studio_model::Float32(1.0f32.to_bits()),
                    }])
                } else { (sequence, cycle, Vec::new()) };
                let pose = playsrc_studio_model::sample_pose_at_time(
                    model,
                    &playsrc_studio_model::AnimationState {
                        bone_rotations: weapon_pose::bone_rotations(request, model),
                        base_sequence,
                        cycle: playsrc_studio_model::Float32(base_cycle.to_bits()),
                        pose_parameters,
                        layers,
                    },
                    playsrc_studio_model::Float32(original.elapsed.to_bits()),
                )
                .map_err(|_| ())?;
                entry.insert(pose);
            }
            let pose = sampled_poses.get(&pose_key).ok_or(())?;
            let selected = playsrc_studio_model::select_primitives(
                model,
                &bodygroups,
                playsrc_studio_model::source_skin_family(
                    i32::try_from(request.skin).unwrap_or(i32::MAX),
                    model.skins.len(),
                ),
                request.lod,
            )
            .map_err(|_| ())?;
            let events = if request.class_selection {
                if let Some((label, elapsed)) = scene_event_clock {
                    let event_sequence = model.sequences.iter().find(|sequence| sequence.label.eq_ignore_ascii_case(label.as_bytes())).ok_or(())?;
                    let event_timing = playsrc_studio_model::sequence_timing(model, event_sequence.index, class_pose_parameters.as_ref().ok_or(())?).map_err(|_| ())?;
                    let (previous, current) = class_scenes.get_mut(&original.identity).ok_or(())?.scene.event_range(event_sequence.index,
                        elapsed * f32::from_bits(event_timing.cycles_per_second.0));
                    playsrc_studio_model::model_panel_events(&event_sequence.events, previous, current)
                } else { Vec::new() }
            } else { playsrc_studio_model::presentation_events_between(
                model,
                sequence,
                playsrc_studio_model::Float32(previous_cycle.to_bits()),
                playsrc_studio_model::Float32(cycle.to_bits()),
            )
            .map_err(|_| ())? };
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
                None,
                model,
                sequence,
                timing,
                previous_cycle,
                cycle,
                &events,
                pose,
                &selected,
                selected.len(),
                legacy_view.as_ref(),
                world,
            )?;
            output_count = output_count.checked_add(1).ok_or(())?;
            if request.world_item || request.class_selection {
                let item_identity = if request.class_selection {
                    playsrc_tf2::class_selection::scene_for_model(&request.model).ok_or(())?.held_model
                } else { request.item.as_deref().ok_or(())? };
                let item = models.get(item_identity).ok_or(())?;
                let parameters = vec![playsrc_studio_model::Float32(0); item.pose_parameters.len()];
                let sampled_item = playsrc_studio_model::sample_pose(
                    item,
                    &playsrc_studio_model::AnimationState {
                        bone_rotations: weapon_pose::bone_rotations(request, item),
                        base_sequence: 0,
                        cycle: playsrc_studio_model::Float32(0),
                        pose_parameters: parameters.clone(),
                        layers: Vec::new(),
                    },
                )
                .map_err(|_| ())?;
                let merged =
                    playsrc_studio_model::merge_model_pose(model, pose, item, &sampled_item)
                        .map_err(|_| ())?;
                let panel_bodygroups = request.class_selection.then(|| vec![0; item.body_parts.len()]);
                let primitives = playsrc_studio_model::select_primitives(
                    item,
                    panel_bodygroups.as_deref().unwrap_or(&request.item_bodygroups),
                    playsrc_studio_model::source_skin_family(request.skin as i32, item.skins.len()),
                    if request.class_selection { 0 } else { request.lod },
                )
                .map_err(|_| ())?;
                let item_timing =
                    playsrc_studio_model::sequence_timing(item, 0, &parameters).map_err(|_| ())?;
                encode_model_pose_part(
                    &mut out,
                    request,
                    2,
                    None,
                    item,
                    0,
                    item_timing,
                    0.0,
                    0.0,
                    &[],
                    &merged,
                    &primitives,
                    primitives.len(),
                    None,
                    world,
                )?;
                output_count = output_count.checked_add(1).ok_or(())?;
            }
            for (equipped, model_path) in &wearables {
                let item = models.get(*model_path).ok_or(())?;
                let parameters = vec![playsrc_studio_model::Float32(0); item.pose_parameters.len()];
                let sampled = playsrc_studio_model::sample_pose(item, &playsrc_studio_model::AnimationState {
                    bone_rotations: Vec::new(),
                    base_sequence: 0, cycle: playsrc_studio_model::Float32(0), pose_parameters: parameters.clone(), layers: Vec::new(),
                }).map_err(|_| ())?;
                let merged = playsrc_studio_model::merge_model_pose(model, pose, item, &sampled).map_err(|_| ())?;
                let primitives = playsrc_studio_model::select_primitives(item, &vec![0; item.body_parts.len()],
                    playsrc_studio_model::source_skin_family(request.skin as i32, item.skins.len()), request.lod).map_err(|_| ())?;
                let timing = playsrc_studio_model::sequence_timing(item, 0, &parameters).map_err(|_| ())?;
                encode_model_pose_part(&mut out, request, 3, Some(equipped), item, 0, timing, 0.0, 0.0,
                    &[], &merged, &primitives, primitives.len(), None, world)?;
                output_count = output_count.checked_add(1).ok_or(())?;
            }
        }
        if out.len() > 64 * 1024 * 1024 {
            return Err(());
        }
    }
    out[8..12].copy_from_slice(&output_count.to_le_bytes());
    Ok(out)
}

mod model_palette;
use model_palette::RetainedPresentationModel;

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
    equipped: Option<&playsrc_tf2::equipment::EquippedItem>,
    model: &RetainedPresentationModel,
    sequence: usize,
    timing: playsrc_studio_model::SequenceTiming,
    previous_cycle: f32,
    cycle: f32,
    events: &[&playsrc_studio_model::SequenceEvent],
    pose: &playsrc_studio_model::SampledPose,
    selected: &[playsrc_studio_model::SelectedPrimitive],
    opaque_count: usize,
    view: Option<&ViewOutput>,
    world: &mut ModelPoseWorld<'_>,
) -> Result<(), ()> {
    let authored_selection = selected;
    let template_selection = if role == 0 && request.equipped_items.iter().any(|item| item.definition_index == 378) {
        Some(playsrc_studio_model::select_primitives(model, &vec![0; model.body_parts.len()],
            playsrc_studio_model::source_skin_family(request.skin as i32, model.skins.len()), request.lod).map_err(|_| ())?)
    } else { None };
    let selected = template_selection.as_deref().unwrap_or(selected);
    if authored_selection.iter().any(|item| !selected.iter().any(|entry| entry.primitive == item.primitive)) { return Err(()); }
    out.extend_from_slice(&request.identity.to_le_bytes());
    if let Some(state) = request.cloak {
        out.extend_from_slice(&[
            1,
            u8::from(request.actor_identity == playsrc_tf2::PLAYER_IDENTITY),
            u8::from(view.is_none() && role == 0),
            0,
        ]);
        for value in [state.local_factor, state.world_factor, state.raw_factor]
            .into_iter()
            .chain(state.player_tint)
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
    } else {
        out.extend_from_slice(&[0; 28]);
    }
    out.extend_from_slice(&request.sample_tick.to_le_bytes());
    out.extend_from_slice(&[
        role,
        u8::from(request.attachments_only),
        u8::from(request.fire_view.is_some()),
        0,
    ]);
    if let Some(equipped) = equipped {
        if role != 3 { return Err(()); }
        out.extend_from_slice(&equipped.item_id.to_le_bytes());
        out.extend_from_slice(&wearable::effect(equipped)?.to_le_bytes());
        let transform = wearable::control_point(model, pose, request.model_panel)?;
        for value in transform.0 { out.extend_from_slice(&value.0.to_le_bytes()); }
        let particles = if request.preparation { Vec::new() } else { world.wearable_particles.sample(world.particle_inputs.as_ref().ok_or(())?, request, equipped, model, pose)? };
        pbytes(out, &particles)?;
    }
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
    let palette = model.selected_palette(selected, request.attachments_only)?;
    out.extend_from_slice(&u32::try_from(palette.len()).map_err(|_| ())?.to_le_bytes());
    for bone in palette.iter() {
        let matrix = pose.skinning_matrices.get(usize::from(bone)).ok_or(())?;
        for value in matrix.0 {
            out.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    out.extend_from_slice(&u32::try_from(selected.len()).map_err(|_| ())?.to_le_bytes());
    for selected in selected {
        let primitive = model.geometry.get(selected.primitive).ok_or(())?;
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
            &u32::try_from(primitive.vertices.len())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        let authored_index = authored_selection.iter().position(|entry| entry.primitive == selected.primitive);
        out.extend_from_slice(&[u8::from(authored_index.is_some_and(|index| index >= opaque_count)), u8::from(authored_index.is_some()), 0, 0]);
    }
    let attachments = named_attachments(&pose.attachments, |attachment| &attachment.name);
    out.extend_from_slice(
        &u32::try_from(attachments.clone().count())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for attachment in attachments {
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
    encode_model_lighting(out, request, model, pose, selected, world)?;
    let mut flex_primitives = Vec::new();
    if let Some(controllers) = &request.flex_controllers {
        let flex = &world.metadata.get(&model.identity).ok_or(())?.flex;
        let weights = flex.weights(controllers).map_err(|_| ())?;
        let deltas = flex.deltas(&weights);
        for selected in selected {
            let geometry = &model.geometry[selected.primitive];
            let vertices = geometry.vertices.iter().enumerate().filter_map(|(index, vertex)| {
                deltas.get(&vertex.source_index).map(|(position, normal)| (index, std::array::from_fn::<_, 3, _>(|axis| f32::from_bits(vertex.position.0[axis].0) + position[axis]),
                    std::array::from_fn::<_, 3, _>(|axis| f32::from_bits(vertex.normal.0[axis].0) + normal[axis])))
            }).collect::<Vec<_>>();
            if !vertices.is_empty() { flex_primitives.push((selected.primitive, vertices)); }
        }
    }
    out.extend_from_slice(&(flex_primitives.len() as u32).to_le_bytes());
    for (primitive, vertices) in flex_primitives {
        out.extend_from_slice(&(primitive as u32).to_le_bytes());
        out.extend_from_slice(&(vertices.len() as u32).to_le_bytes());
        for (index, position, normal) in vertices {
            out.extend_from_slice(&(index as u32).to_le_bytes());
            for value in position.into_iter().chain(normal) { out.extend_from_slice(&value.to_le_bytes()); }
        }
    }
    Ok(())
}

fn encode_model_lighting(
    out: &mut Vec<u8>,
    request: &ModelPoseRequest,
    model: &playsrc_studio_model::PresentationModel,
    pose: &playsrc_studio_model::SampledPose,
    selected: &[playsrc_studio_model::SelectedPrimitive],
    world: &mut ModelPoseWorld<'_>,
) -> Result<(), ()> {
    let Some(lighting_request) = request.lighting else {
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&0_u32.to_le_bytes());
        return Ok(());
    };
    let metadata = world.metadata.get(&model.identity).ok_or(())?;
    let vector = |values: [f32; 3]| {
        playsrc_studio_model::Vector3(
            values.map(|value| playsrc_studio_model::Float32(value.to_bits())),
        )
    };
    let transform = playsrc_studio_model::source_entity_transform(
        vector(lighting_request.origin),
        vector(lighting_request.angles),
    )
    .map_err(|_| ())?;
    if request.model_panel {
        // CPotteryWheelPanel's default light rig is independent of the loaded map.
        out.extend_from_slice(&[1, 1, 1, 0]);
        for value in [0.0f32; 3].into_iter().chain(lighting_request.camera) { out.extend_from_slice(&value.to_le_bytes()); }
        for _ in 0..18 { out.extend_from_slice(&0.4f32.to_le_bytes()); }
        out.extend_from_slice(&[1, 0, 0, 0]);
        for value in [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        pbytes(out, b"")?;
        return encode_model_eyes(out, request, lighting_request, model, pose, selected, world, transform);
    }
    let posed = if metadata.attachment == 0 {
        None
    } else {
        Some(playsrc_studio_model::apply_entity_transform(model, pose, transform).map_err(|_| ())?)
    };
    let is_wearable = request.equipped_items.iter().any(|item| wearable::model(item, &request.model).ok().flatten() == Some(model.identity.as_str()));
    let origin = if is_wearable {
        // Econ wearables sample ambient lighting at their owner's WorldSpaceCenter.
        wearable::owner_lighting_origin(&request.model, lighting_request.origin)?
    } else if model.profile == playsrc_studio_model::PresentationProfile::ViewModel
        && let Some(player) = world.gameplay
    {
        source_tf2_viewmodel_lighting_origin(player.movement, player.class)
    } else {
        playsrc_studio_model::source_model_lighting_origin(
            metadata.position,
            metadata.attachment,
            transform,
            posed.as_ref(),
            &model.identity,
        )
        .map_err(|_| ())?
        .0
        .map(|value| f32::from_bits(value.0))
    };
    let mut state =
        world
            .lighting.as_mut().ok_or(())?
            .sample(origin, world.visibility.ok_or(())?, world.collision.ok_or(())?, world.snapshot.ok_or(())?)?;
    playsrc_map::apply_model_ambient_boost(
        &mut state.ambient_cube,
        &state.local_lights,
        state.origin,
        model.flags,
    );
    encode_world_lighting(out, &state, lighting_request.camera, world.cubemaps)?;
    encode_model_eyes(
        out,
        request,
        lighting_request,
        model,
        pose,
        selected,
        world,
        transform,
    )
}

fn source_tf2_viewmodel_lighting_origin(
    movement: playsrc_movement::State,
    class: playsrc_tf2::PlayerClass,
) -> [f32; 3] {
    let policy = playsrc_tf2::MovementPolicy {
        class,
        modifiers: playsrc_tf2::MovementModifiers::default(),
    }
    .resolve();
    let hull = movement.active_hull(policy);
    std::array::from_fn(|axis| movement.position[axis] + (hull.mins[axis] + hull.maxs[axis]) * 0.5)
}

fn encode_world_lighting(
    out: &mut Vec<u8>,
    state: &playsrc_map::ModelWorldLighting,
    camera: [f32; 3],
    cubemaps: &[playsrc_map::CubemapSample],
) -> Result<(), ()> {
    let lights = state
        .local_lights
        .iter()
        .filter_map(|selected| material_world_light(&selected.light))
        .collect::<Result<Vec<_>, ()>>()?;
    if lights.len() > playsrc_studio_model::MAX_MODEL_LOCAL_LIGHTS {
        return Err(());
    }
    out.extend_from_slice(&[1, lights.len() as u8, 1, 0]);
    for value in state.origin.into_iter().chain(camera) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in state.ambient_cube.iter().flatten() {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for light in lights {
        out.extend_from_slice(&[light.kind, 0, 0, 0]);
        for value in light
            .color
            .into_iter()
            .chain(light.position)
            .chain(light.direction)
            .chain([light.range, light.falloff])
            .chain(light.attenuation)
            .chain([light.theta, light.phi])
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    let environment = playsrc_map::select_cubemap(cubemaps, state.origin, None)
        .ok()
        .map_or("", |cubemap| cubemap.logical_path.as_str());
    pbytes(out, environment.as_bytes())
}

#[allow(clippy::too_many_arguments)]
fn encode_model_eyes(
    out: &mut Vec<u8>,
    request: &ModelPoseRequest,
    lighting: ModelPoseLightingRequest,
    model: &playsrc_studio_model::PresentationModel,
    pose: &playsrc_studio_model::SampledPose,
    selected: &[playsrc_studio_model::SelectedPrimitive],
    world: &ModelPoseWorld<'_>,
    transform: playsrc_studio_model::Matrix3x4,
) -> Result<(), ()> {
    let definitions = &world.metadata.get(&model.identity).ok_or(())?.eyes;
    let target = if request.model_panel { lighting.camera } else { source_tf2_eye_target(request.identity, lighting, world.gameplay) };
    let states = source_model_eye_states(
        model,
        definitions,
        pose,
        selected,
        transform,
        target,
        lighting.camera_angles,
        false,
    )?;
    encode_eye_states(out, &states)
}

#[allow(clippy::too_many_arguments)]
fn source_model_eye_states(
    model: &playsrc_studio_model::PresentationModel,
    definitions: &[playsrc_studio_model::EyeDefinition],
    pose: &playsrc_studio_model::SampledPose,
    selected: &[playsrc_studio_model::SelectedPrimitive],
    transform: playsrc_studio_model::Matrix3x4,
    target: [f32; 3],
    camera_angles: [f32; 3],
    runtime_order: bool,
) -> Result<Vec<(usize, playsrc_studio_model::EyeDrawState)>, ()> {
    if definitions.is_empty() || selected.is_empty() {
        return Ok(Vec::new());
    }
    let posed =
        playsrc_studio_model::apply_entity_transform(model, pose, transform).map_err(|_| ())?;
    let vector = |values: [f32; 3]| {
        playsrc_studio_model::Vector3(
            values.map(|value| playsrc_studio_model::Float32(value.to_bits())),
        )
    };
    let yaw = camera_angles[1].to_radians();
    let pitch = camera_angles[0].to_radians();
    let (yaw_sine, yaw_cosine) = yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = pitch.sin_cos();
    let view_right = [yaw_sine, -yaw_cosine, 0.0];
    let view_up = [pitch_sine * yaw_cosine, pitch_sine * yaw_sine, pitch_cosine];
    let mut output = Vec::new();
    for (index, primitive) in selected.iter().enumerate() {
        let geometry = model.geometry.get(primitive.primitive).ok_or(())?;
        let Some(definition) = definitions.iter().find(|definition| {
            definition.body_part == geometry.body_part
                && definition.submodel == geometry.model
                && definition.mesh == geometry.mesh
        }) else {
            continue;
        };
        let states = playsrc_studio_model::eye_draw_states_for_definitions(
            &model.identity,
            std::slice::from_ref(definition),
            &playsrc_studio_model::EyeDrawRequest {
                body_part: definition.body_part,
                submodel: definition.submodel,
                bone_to_world: &posed.bone_matrices,
                world_target: vector(target),
                view_right: vector(view_right),
                view_up: vector(view_up),
                configuration: playsrc_studio_model::EyeConfiguration {
                    move_eyes: true,
                    shift: vector([0.0; 3]),
                    size: playsrc_studio_model::Float32(0),
                },
            },
        )
        .map_err(|_| ())?;
        if states.len() != 1 {
            return Err(());
        }
        output.push((
            if runtime_order {
                index
            } else {
                primitive.primitive
            },
            states.into_iter().next().ok_or(())?,
        ));
    }
    Ok(output)
}

fn encode_eye_states(
    out: &mut Vec<u8>,
    states: &[(usize, playsrc_studio_model::EyeDrawState)],
) -> Result<(), ()> {
    out.extend_from_slice(&u32::try_from(states.len()).map_err(|_| ())?.to_le_bytes());
    for (primitive, eye) in states {
        for value in [*primitive, eye.mesh, eye.eyeball, eye.texture] {
            out.extend_from_slice(&u32::try_from(value).map_err(|_| ())?.to_le_bytes());
        }
        for value in eye
            .world_origin
            .0
            .into_iter()
            .chain(eye.authored_up.0)
            .chain(eye.iris_u)
            .chain(eye.iris_v)
            .chain(eye.glint_u)
            .chain(eye.glint_v)
        {
            out.extend_from_slice(&value.0.to_le_bytes());
        }
    }
    Ok(())
}

fn source_tf2_eye_target(
    identity: u32,
    request: ModelPoseLightingRequest,
    snapshot: Option<&playsrc_tf2::Snapshot>,
) -> [f32; 3] {
    let yaw = request.angles[1].to_radians();
    let pitch = request.angles[0].to_radians();
    let (yaw_sine, yaw_cosine) = yaw.sin_cos();
    let (pitch_sine, pitch_cosine) = pitch.sin_cos();
    let forward = [
        pitch_cosine * yaw_cosine,
        pitch_cosine * yaw_sine,
        -pitch_sine,
    ];
    if let Some(snapshot) = snapshot {
        let local = (
            1_u32,
            snapshot.movement.position,
            [
                snapshot.movement.position[0] + snapshot.movement.view_offset[0],
                snapshot.movement.position[1] + snapshot.movement.view_offset[1],
                snapshot.movement.position[2] + snapshot.movement.view_offset[2],
            ],
            snapshot.health > 0.0,
        );
        let bot_identity = identity.checked_sub(0x6000_0000);
        let bot = snapshot.bots.iter().map(|bot| {
            (
                bot.identity,
                bot.position,
                [
                    bot.position[0],
                    bot.position[1],
                    bot.position[2] + bot.class.standing_eye_height(),
                ],
                bot.lifecycle == playsrc_tf2::PlayerLifecycle::Active,
            )
        });
        for (candidate, position, eye, alive) in std::iter::once(local).chain(bot) {
            if !alive || Some(candidate) == bot_identity {
                continue;
            }
            let difference = sub3(position, request.origin);
            let distance = dot(difference, difference).sqrt();
            if distance <= 0.0 || distance > 300.0 {
                continue;
            }
            let direction = difference.map(|value| value / distance);
            if dot(forward, direction) >= 0.0 {
                return eye;
            }
        }
    }
    std::array::from_fn(|axis| request.origin[axis] + forward[axis] * 512.0)
}

struct MaterialWorldLight {
    kind: u8,
    color: [f32; 3],
    position: [f32; 3],
    direction: [f32; 3],
    range: f32,
    falloff: f32,
    attenuation: [f32; 3],
    theta: f32,
    phi: f32,
}

fn material_world_light(light: &playsrc_map::WorldLight) -> Option<Result<MaterialWorldLight, ()>> {
    let (kind, attenuation, theta, phi, falloff) = match light.kind {
        0 => (
            2,
            [0.0, 0.0, 1.0],
            std::f32::consts::PI,
            std::f32::consts::PI,
            1.0,
        ),
        1 => (
            0,
            [
                light.constant_attenuation,
                light.linear_attenuation,
                light.quadratic_attenuation,
            ],
            0.0,
            0.0,
            0.0,
        ),
        2 => (
            2,
            [
                light.constant_attenuation,
                light.linear_attenuation,
                light.quadratic_attenuation,
            ],
            2.0 * light.stop_dot.acos(),
            2.0 * light.stop_dot2.acos(),
            if light.exponent == 0.0 {
                1.0
            } else {
                light.exponent
            },
        ),
        3 => (1, [1.0, 0.0, 0.0], 0.0, 0.0, 0.0),
        4 | 5 => return None,
        _ => return Some(Err(())),
    };
    let attenuation = if attenuation == [0.0; 3] {
        [1.0, 0.0, 0.0]
    } else {
        attenuation
    };
    Some(Ok(MaterialWorldLight {
        kind,
        color: light.intensity,
        position: light.origin,
        direction: light.normal,
        range: light.radius,
        falloff,
        attenuation,
        theta,
        phi,
    }))
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

#[inline]
fn admit_world_bit(words: &mut [u64], index: usize) -> Result<bool, ()> {
    let word = words.get_mut(index / 64).ok_or(())?;
    let mask = 1_u64 << (index % 64);
    let absent = *word & mask == 0;
    *word |= mask;
    Ok(absent)
}

fn encode_sorted_world_bits(output: &mut Vec<u8>, words: &[u64]) {
    output.extend_from_slice(&words.iter().map(|word| word.count_ones()).sum::<u32>().to_le_bytes());
    for (index, mut word) in words.iter().copied().enumerate() {
        while word != 0 {
            output.extend_from_slice(&(index as u32 * 64 + word.trailing_zeros()).to_le_bytes());
            word &= word - 1;
        }
    }
}

fn frustum_world_leaves(
    world: &playsrc_visibility::World,
    node_cull_modes: &[i8],
    allowed_leaves: &[usize],
    origin: [f32; 3],
    angles: [f32; 2],
    vertical_fov: f32,
    aspect: f32,
) -> Result<Vec<usize>, ()> {
    const SUPPRESS: u8 = u8::MAX;
    let planes = world_frustum(origin, angles[0], angles[1], vertical_fov, aspect);
    if node_cull_modes.len() != world.nodes.len() {
        return Err(());
    }
    let mut allowed = vec![0_u64; world.leaves.len().div_ceil(64)];
    for &leaf in allowed_leaves {
        admit_world_bit(&mut allowed, leaf)?;
    }
    let mut leaves = Vec::new();
    let mut stack = vec![(world.models.first().ok_or(())?.head_node, 0b1111_u8)];
    while let Some((child, mut mask)) = stack.pop() {
        if child < 0 {
            let leaf = (-1_i64 - i64::from(child)) as usize;
            let record = world.leaves.get(leaf).ok_or(())?;
            if record.contents == 1 || allowed[leaf / 64] & (1_u64 << (leaf % 64)) == 0 {
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
            if node_cull_modes[index] == -1 {
                if cull_world_bounds(node.mins, node.maxs, &planes, &mut mask) {
                    continue;
                }
            } else if node_cull_modes[index] == -2 {
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
    Ok(leaves)
}

fn frustum_world_surfaces(
    world: &playsrc_visibility::World,
    node_cull_modes: &[i8],
    allowed_leaves: &[usize],
    origin: [f32; 3],
    angles: [f32; 2],
    vertical_fov: f32,
    aspect: f32,
) -> Result<Vec<u16>, ()> {
    let leaves = frustum_world_leaves(world, node_cull_modes, allowed_leaves, origin, angles, vertical_fov, aspect)?;
    let mut seen = [0_u64; 1024];
    let mut surfaces = Vec::new();
    for leaf in leaves {
        let record = &world.leaves[leaf];
        let start = usize::from(record.first_leaf_face);
        let end = start + usize::from(record.leaf_face_count);
        for face in &world.leaf_faces[start..end] {
            if admit_world_bit(&mut seen, usize::from(*face))? {
                surfaces.push(*face);
            }
        }
        for face in &world.leaf_displacements[leaf] {
            if admit_world_bit(&mut seen, usize::from(*face))? {
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
    slot.visibility_output.clear();
    let (Some(world), Some(candidates), Some(area), Some(environment)) = (
        slot.visibility.as_ref(),
        slot.visibility_candidates.as_ref(),
        slot.area_state.as_ref(),
        slot.environment.as_ref(),
    ) else {
        return 0;
    };
    let view = match world.view(
        area,
        candidates,
        &playsrc_visibility::ViewQuery {
            origins: vec![visibility_position],
            bypass_pvs: false,
        },
    ) {
        Ok(view) => view,
        Err(error) => { slot.visibility_output = format!("PVQEvisibility: {error:?}").into_bytes(); return 0; }
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
    if let Some(smoke) = slot.smokestacks.as_mut() {
        let Ok(view) = smokestack::RenderView::from_query(input, &qualified_leaves, world, &environment.node_cull_modes) else { return 0; };
        smoke.views[usize::from(area_filter.is_some())] = Some(view);
    }
    let Ok(world_surfaces) = frustum_world_surfaces(
        world,
        &environment.node_cull_modes,
        &qualified_leaves,
        position,
        [input[6], input[7]],
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
    let preliminary = match environment.world.water.plan_view(world, base_water_input) {
        Ok(plan) => plan,
        Err(error) => { slot.visibility_output = format!("PVQEwater selection: {error:?}").into_bytes(); return 0; }
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
    let water_plan = match environment.world.water.plan_view(
        world,
        playsrc_map::WaterViewInput {
            near_plane_intersects_selected_volume: intersects,
            ..base_water_input
        },
    ) {
        Ok(plan) => plan,
        Err(error) => { slot.visibility_output = format!("PVQEwater view: {error:?}").into_bytes(); return 0; }
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
    let ordinary_water_surface = water_plan.visible_water.as_ref().is_some_and(|water| {
        water.state.is_none()
            && water_plan.render.cheap && !water_plan.render.reflect && !water_plan.render.refract
    });
    if water_plan.visible_water.is_some() && evaluated_water.is_none() && !ordinary_water_surface {
        slot.visibility_output = format!("PVQEwater material evaluation: {:?}", water_plan.visible_water.as_ref().map(|w| &w.material)).into_bytes();
        return 0;
    }
    let evaluated_overlay = water_plan
        .visible_water
        .as_ref()
        .filter(|water| water.eye_in_volume)
        .and(evaluated_water.as_ref())
        .and_then(|(water_identity, _)| {
            let water = environment.water_materials.get(water_identity)?;
            let overlay = water.water.as_ref()?.underwater_overlay.as_ref()?;
            let identity = overlay.logical_path.to_ascii_lowercase();
            let refract = environment.refract_materials.get(&identity)?;
            let context = playsrc_material::ProxyEvaluationContext {
                time: input[12],
                frame_time: 0.015,
                water_lod: None,
                texture_frames: BTreeMap::from([(
                    b"$normalmap".to_vec(),
                    refract.normal_frame_count,
                )]),
                model_inputs: playsrc_material::ModelProxyInputs::default(),
            };
            playsrc_material::evaluate_refract_material(&refract.material, &context)
                .ok()
                .map(|evaluated| (identity, evaluated))
        });
    if water_plan
        .visible_water
        .as_ref()
        .is_some_and(|water| water.eye_in_volume)
        && evaluated_water.as_ref().is_some_and(|(identity, _)| {
            environment
                .water_materials
                .get(identity)
                .and_then(|material| material.water.as_ref())
                .and_then(|water| water.underwater_overlay.as_ref())
                .is_some()
        })
        && evaluated_overlay.is_none()
    {
        return 0;
    }
    // Membership fields in PVIS are ascending sets, distinct from the ordered
    // draw-surface traversal above. Their Source u16 face and 9-bit area domains
    // do not need a freshly allocated tree for every main/sky query.
    let mut visible_surfaces = [0_u64; 1024];
    let mut visible_areas = [0_u64; 8];
    for leaf in &qualified_leaves {
        let record = &world.leaves[*leaf];
        let area = usize::from(record.area_and_flags & 0x01ff);
        visible_areas[area / 64] |= 1_u64 << (area % 64);
        let start = usize::from(record.first_leaf_face);
        let end = start + usize::from(record.leaf_face_count);
        for face in world.leaf_faces[start..end].iter().chain(&world.leaf_displacements[*leaf]) {
            visible_surfaces[usize::from(*face) / 64] |= 1_u64 << (*face % 64);
        }
    }
    let mut output = b"PVIS".to_vec();
    output.extend_from_slice(&8u32.to_le_bytes());
    output.extend_from_slice(&view_identity);
    output.extend_from_slice(&world.identity);
    output.extend_from_slice(&[u8::from(view.outside_world), view.sky as u8, 0, 0]);
    encode_sorted_world_bits(&mut output, &visible_surfaces);
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
    encode_sorted_world_bits(&mut output, &visible_areas);
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
    if let Some(water) = &water_plan.visible_water {
        let identity = match &water.material {
            playsrc_map::WaterMaterialIdentity::Map(index) => {
                let Some(identity) = environment.map_materials.get(index) else { return 0; };
                identity
            },
            playsrc_map::WaterMaterialIdentity::Dependency(identity) => identity,
        };
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
            u8::from(evaluated_overlay.is_some()),
            u8::from(evaluated_water.is_some()),
        ]);
        output.extend_from_slice(&water.surface_z.to_le_bytes());
        output.extend_from_slice(
            &u32::from(water.distance_to_water.unwrap_or(u16::MAX)).to_le_bytes(),
        );
        if pbytes(&mut output, identity.as_bytes()).is_err() {
            return 0;
        }
        if let Some((_, evaluated)) = &evaluated_water {
            output.extend_from_slice(&evaluated.normal_frame.to_le_bytes());
            for value in evaluated.normal_transform {
                output.extend_from_slice(&value.to_le_bytes());
            }
            output.extend_from_slice(&evaluated.cheap_start.to_le_bytes());
            output.extend_from_slice(&evaluated.cheap_end.to_le_bytes());
        }
        if let Some((identity, overlay)) = &evaluated_overlay {
            if pbytes(&mut output, identity.as_bytes()).is_err() {
                return 0;
            }
            output.extend_from_slice(&overlay.normal_frame.to_le_bytes());
            for value in overlay.normal_transform {
                output.extend_from_slice(&value.to_le_bytes());
            }
        }
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
    let bleeding = slot.latest_game_snapshot.as_ref().is_some_and(|snapshot| snapshot.conditions & (1 << playsrc_tf2::Condition::Bleeding as u8) != 0);
    output.extend_from_slice(&[u8::from(bleeding), 0, 0, 0]);
    if bleeding {
        let identity = "materials/effects/bleed_overlay.vmt";
        let Some(material) = environment.refract_materials.get(identity) else { return 0; };
        let context = playsrc_material::ProxyEvaluationContext { time: input[12], frame_time: 0.015, water_lod: None,
            texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), material.normal_frame_count)]), model_inputs: Default::default() };
        let Ok(evaluated) = playsrc_material::evaluate_refract_material(&material.material, &context) else { return 0; };
        if pbytes(&mut output, identity.as_bytes()).is_err() { return 0; }
        output.extend_from_slice(&evaluated.normal_frame.to_le_bytes());
        for value in evaluated.normal_transform.into_iter().chain(evaluated.refract_tint) { output.extend_from_slice(&value.to_le_bytes()); }
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
pub extern "C" fn playsrc_visibility_output_pointer(handle: u32) -> *const u8 {
    with(handle, |slot| slot.visibility_output.as_ptr()).unwrap_or(std::ptr::null())
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
pub extern "C" fn playsrc_result_take(handle: u32) -> *mut u8 {
    let Some((index, generation)) = decode(handle) else {
        return std::ptr::null_mut();
    };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else {
        return std::ptr::null_mut();
    };
    if slot.generation != generation {
        return std::ptr::null_mut();
    }
    let Some(payload) = slot.payload.as_mut() else {
        return std::ptr::null_mut();
    };
    if payload.is_empty() {
        return std::ptr::null_mut();
    }
    slot.payload_bytes = 0;
    Box::into_raw(std::mem::take(payload).into_boxed_slice()) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_result_release(handle: u32) -> u32 {
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
    let Some(payload) = slot.payload.as_mut() else {
        return 0;
    };
    payload.clear();
    payload.shrink_to_fit();
    slot.payload_bytes = 0;
    1
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
    let success = session.select_team_choice(selected).is_ok();
    if success {
        gameplay_replay::mutation(handle, gameplay_replay::Mutation::Team, &choice.to_le_bytes());
    }
    u32::from(success)
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify writable module memory of at least `capacity` bytes.
pub unsafe extern "C" fn playsrc_equipment_state_copy(handle: u32, pointer: *mut u8, capacity: usize) -> usize {
    if pointer.is_null() || (handle != 0 && with(handle, |_| ()).is_none()) { return 0; }
    let bytes = local_equipment().lock().expect("local equipment").encode_state();
    if bytes.len() > capacity { return 0; }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len()) };
    bytes.len()
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify readable module memory of `length` bytes.
pub unsafe extern "C" fn playsrc_equipment_update(handle: u32, pointer: *const u8, length: usize) -> u32 {
    if pointer.is_null() || length == 0 || length > 1024 { return 0; }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    if handle != 0 && with(handle, |_| ()).is_none() { return 0; }
    if bytes[0] != 2 {
        let mut equipment = local_equipment().lock().expect("local equipment");
        let result = match (bytes[0], length) {
            (0, 693) => playsrc_tf2::equipment::Equipment::restore(&bytes[1..]).map(|restored| *equipment = restored),
            (1, 7) => {
                let Ok(class) = playsrc_tf2::PlayerClass::try_from(bytes[1]) else { return 0; };
                let Ok(position) = playsrc_tf2::schema::LoadoutPosition::try_from(bytes[2]) else { return 0; };
                let definition = u32::from_le_bytes(bytes[3..7].try_into().unwrap());
                equipment.equip(class, position, (definition != u32::MAX).then_some(definition)).map(|_| ())
            },
            _ => return 0,
        };
        if result.is_err() { return 0; }
        let saved = equipment.persist();
        drop(equipment);
        for slot in slots().lock().expect("TF2 slots").iter_mut() {
            if let Some(session) = slot.session.as_mut() { session.restore_equipment(&saved).expect("validated local equipment"); }
        }
        gameplay_replay::local_equipment_mutation(bytes);
        return 1;
    }
    let Some((index, generation)) = decode(handle) else { return 0; };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index) else { return 0; };
    if slot.generation != generation || slot.payload.is_none() { return 0; }
    let Some(session) = slot.session.as_mut() else { return 0; };
    let result = match (bytes[0], length) {
        (2, 9) => {
            let identity = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            let definition = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
            session.equip_bot_cosmetic(identity, (definition != u32::MAX).then_some(definition)).map(|_| ())
        },
        _ => return 0,
    };
    if result.is_ok() { gameplay_replay::mutation(handle, gameplay_replay::Mutation::Equipment, bytes); }
    u32::from(result.is_ok())
}

fn local_equipment() -> &'static Mutex<playsrc_tf2::equipment::Equipment> {
    static EQUIPMENT: OnceLock<Mutex<playsrc_tf2::equipment::Equipment>> = OnceLock::new();
    EQUIPMENT.get_or_init(|| Mutex::new(playsrc_tf2::equipment::Equipment::default()))
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
    acknowledged_snapshot: u64,
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
    gameplay_replay::observe(
        handle,
        now_seconds,
        suspended,
        acknowledged_snapshot,
        command,
    );
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
            snapshots: snapshot_transport::Encoder::default(),
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
    let Some(output) = entry
        .snapshots
        .encode(&entry.host.drain_publications(), acknowledged_snapshot)
    else {
        return 0;
    };
    entry.output = output;
    playsrc_tf2::admission_metrics::emit_value(playsrc_tf2::admission_metrics::PUBLISHED, 0, entry.output.len() as u64);
    gameplay_replay::published(handle, &entry.output);
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
/// The owning Worker must synchronously copy this range before another observe
/// or dispose call. It is never an asynchronously retained shared-memory lease.
pub extern "C" fn playsrc_simulation_output_pointer(handle: u32) -> *const u8 {
    simulation_hosts()
        .lock()
        .expect("TF2 Simulation hosts")
        .get(&handle)
        .map_or(std::ptr::null(), |entry| entry.output.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_dispose(handle: u32) -> u32 {
    gameplay_replay::dispose(handle);
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
    slot.presentation = Vec::new();
    slot.presentation_bytes = 0;
    slot.coverage = Vec::new();
    slot.particles = None;
    slot.map_particles = None;
    slot.smokestacks = None;
    slot.legacy_visual_output = Vec::new();
    slot.particle_materials = Vec::new();
    slot.wearable_particles = wearable::ParticleStates::default();
    slot.particle_sheets = BTreeMap::new();
    slot.particle_output = Vec::new();
    slot.studio_models = BTreeMap::new();
    slot.model_lighting_metadata = BTreeMap::new();
    slot.model_lighting_world = None;
    slot.model_material_opacity = BTreeMap::new();
    slot.viewmodel_bob = BTreeMap::new();
    slot.weapon_animations = BTreeMap::new();
    slot.class_scenes = BTreeMap::new();
    slot.model_output = Vec::new();
    slot.visibility = None;
    slot.soundscapes = Default::default();
    slot.acoustic_scene = None;
    slot.acoustic_output = Vec::new();
    slot.visibility_candidates = None;
    slot.area_state = None;
    slot.visibility_output = Vec::new();
    slot.environment = None;
    slot.collision = None;
    slot.gameplay_world = None;
    slot.collision_templates = Vec::new();
    slot.collision_revision = 0;
    slot.pushers = BTreeMap::new();
    slot.latest_game_snapshot = None;
    slot.hash = [0; 32];
    slot.derived_hash = [0; 32];
    slot.bsp_hash = [0; 32];
    slot.error = 0;
    slot.spawn = None;
    slot.session = None;
    slot.snapshot = Arc::from([]);
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
    GAME_ADVANCE_DETAIL
        .get_or_init(|| Mutex::new(String::new()))
        .lock()
        .expect("game advance detail")
        .clear();
    macro_rules! fail {
        ($code:expr) => {{
            GAME_ADVANCE_ERROR.store($code, Ordering::Relaxed);
            playsrc_tf2::admission_metrics::emit_value(playsrc_tf2::admission_metrics::ROLLBACK, 0, $code as u64);
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
    playsrc_tf2::admission_metrics::begin_tick(session.tick());
    playsrc_tf2::admission_metrics::emit(playsrc_tf2::admission_metrics::TRANSACTION, 0);
    let mut candidate = session.clone();
    playsrc_tf2::admission_metrics::emit(playsrc_tf2::admission_metrics::CLONED, 0);
    let Some(collision) = slot.collision.clone() else {
        fail!(7);
    };
    let Some(gameplay_world) = slot.gameplay_world.clone() else {
        fail!(8);
    };
    let mut collision_transaction = CollisionSnapshotTransaction::new(gameplay_world.clone());
    let templates = &slot.collision_templates;
    let mut collision_revision = slot.collision_revision;
    let mut pushers = slot.pushers.clone();
    let mut snapshot: Option<playsrc_tf2::Snapshot> = None;
    let mut producer: Option<playsrc_tf2::ProducerSnapshot> = None;
    let mut random_draws = Vec::new();
    let mut audio_events = Vec::new();
    let mut consumed_rocket_results = Vec::new();
    let mut consumed_mover_results = Vec::new();
    let mut collision_snapshot_bytes = Vec::new();
    for index in 0..tick_count {
        playsrc_tf2::admission_metrics::begin_tick(candidate.tick());
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
                    hierarchy: templates.iter().filter_map(|template| {
                        // The prop children are rigid members of this pusher;
                        // independently driven brush movers keep their own requests.
                        if !matches!(&template.input.shape, playsrc_collision::SnapshotShape::Physics(_)) { return None; }
                        let identity = u32::try_from(template.input.identity).ok()?;
                        if !candidate.entity_descends_from(identity, request.entity) { return None; }
                        let transform = candidate.entity_world_transform(identity)?;
                        Some(playsrc_movement::PusherHierarchyMemberRequest {
                            identity: u64::from(identity),
                            start: playsrc_collision::Transform { origin: transform.origin, angles: transform.angles },
                        })
                    }).collect(),
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
        let prior_collision = gameplay_world.snapshot();
        let current_collision = match retain_collision_snapshot(
            &prior_collision,
            templates,
            current_revision,
            Some(&|identity| candidate.entity_collision_state(identity)),
            &transforms,
            &velocities,
        ) {
            Some(value) => value,
            None => match compile_collision_snapshot(
                Some(&prior_collision),
                &collision,
                templates,
                current_revision,
                snapshot.as_ref().or(slot.latest_game_snapshot.as_ref()),
                Some(&|identity| candidate.entity_collision_state(identity)),
                &transforms,
                &velocities,
            ) {
                Ok(value) => value,
                Err(_) => fail!(11),
            },
        };
        let mut mover_phase = playsrc_tf2::MapPhase::default();
        playsrc_tf2::admission_metrics::emit(playsrc_tf2::admission_metrics::COLLISION, 0);
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
                let mut superseded = false;
                let mut consumed_records = 0;
                for record in &records {
                    match candidate.apply_mover_results(std::slice::from_ref(record)) {
                        Ok(phase) => {
                            consumed_records += 1;
                            superseded |= phase.mover_requests.iter().any(|replacement| {
                                replacement.entity == record.entity
                                    && replacement.request_id != record.request_id
                            });
                            mover_phase.append(phase);
                            if superseded {
                                break;
                            }
                        }
                        Err(error) => {
                            *GAME_ADVANCE_DETAIL
                                .get_or_init(|| Mutex::new(String::new()))
                                .lock()
                                .expect("game advance detail") = format!(
                                "; mover entity={} request={} kind={:?} error={error:?}",
                                record.entity, record.request_id, record.kind
                            );
                            fail!(14)
                        }
                    }
                }
                for result in &frame.results {
                    transforms.insert(result.identity, result.transform);
                    velocities.insert(result.identity, result.trajectory_velocity);
                    for child in &result.hierarchy {
                        transforms.insert(child.identity, child.transform);
                        // CBaseEntity::CalcAbsoluteVelocity adds the parent's
                        // absolute velocity to the child's rotated local one.
                        // These rigid hierarchy members have zero local velocity.
                        velocities.insert(child.identity, result.trajectory_velocity);
                    }
                }
                consumed_mover_results.extend(records.into_iter().take(consumed_records));
                if frame.next.active_count() != 0 && !superseded {
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
                Some(&current_collision),
                &collision,
                templates,
                collision_revision,
                snapshot.as_ref().or(slot.latest_game_snapshot.as_ref()),
                Some(&|identity| candidate.entity_collision_state(identity)),
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
            candidate.tick(),
        ) else {
            fail!(17);
        };
        consumed_rocket_results.extend_from_slice(&rocket_results);
        gameplay_world.replace_snapshot(collision_snapshot);
        gameplay_world.set_movement_time(
            candidate.tick() as f32 * playsrc_simulation::DEFAULT_TICK_INTERVAL,
        );
        let physics_results = if index == 0 {
            input.physics_results.as_slice()
        } else {
            &[]
        };
        if index == 0 {
            let mut retained = gameplay_world.player_hitboxes.lock().expect("player hitbox models");
            for class in playsrc_tf2::PlayerClass::ALL {
                let path = class.data().model;
                if !retained.models.contains_key(path) && let Some(model) = slot.studio_models.get(path) {
                    retained.models.insert(path.to_owned(), model.clone());
                }
            }
        }
        match candidate.into_advanced(input.command, physics_results, &rocket_results, None) {
            Ok((advanced, mut value)) => {
                playsrc_tf2::admission_metrics::emit(playsrc_tf2::admission_metrics::ADVANCED, 0);
                candidate = advanced;
                if !slot.soundscapes.is_empty() {
                    let ear = std::array::from_fn(|axis| value.movement.position[axis] + value.movement.view_offset[axis]);
                    let Some(visibility) = &slot.visibility else { fail!(21); };
                    let leaf = match visibility.locate_leaf(ear) { Ok(leaf) => leaf, Err(_) => fail!(21) };
                    let candidates = slot.soundscapes.candidates(visibility.leaves[leaf].cluster);
                    let mut trace_failed = false;
                    let phase = candidate.update_soundscape(ear, candidates, |start, end| {
                        match soundscapes::trace(&gameplay_world, start, end) {
                            Ok(trace) => trace,
                            Err(()) => {
                                trace_failed = true;
                                playsrc_entity::soundscape::Trace { fraction: 0.0, start_solid: true }
                            }
                        }
                    });
                    if trace_failed { fail!(21); }
                    let phase = match phase { Ok(phase) => phase, Err(error) => fail!(gameplay_error_code(&error)) };
                    value.entity_events.extend(phase.events);
                    mover_phase.effects.extend(phase.effects);
                }
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
                producer = Some(current_producer);
                snapshot = Some(value);
            }
            Err(error) => {
                *GAME_ADVANCE_DETAIL
                    .get_or_init(|| Mutex::new(String::new()))
                    .lock()
                    .expect("game advance detail") = format!("; gameplay={error:?}");
                fail!(gameplay_error_code(&error))
            }
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
    let combat_decals = match slot.combat_decals.as_mut() {
        Some(world) => match world.project(&snapshot.events) {
            Ok(decals) => decals,
            Err(_) => fail!(20),
        },
        None => fail!(20),
    };
    playsrc_tf2::admission_metrics::emit(playsrc_tf2::admission_metrics::ENCODE, 0);
    let Some(encoded) = encode_snapshot(
        &snapshot,
        &producer,
        candidate.respawn_touch_count(),
        candidate.last_movement_result(),
        SnapshotExtensions {
            random_state: candidate.random_state(),
            random_draws: &random_draws,
            audio_events: &audio_events,
            soundscape: candidate.soundscape_selection(),
            rocket_results: &consumed_rocket_results,
            mover_results: &consumed_mover_results,
            collision_snapshot: &collision_snapshot_bytes,
            entity_presentation: &entity_presentation,
            payload_constraint_blocked: candidate.payload_constraint_blocked(),
            combat_decals: &combat_decals,
        },
    ) else {
        fail!(20);
    };
    slot.session = Some(candidate);
    slot.pushers = pushers;
    slot.latest_game_snapshot = Some(snapshot);
    slot.collision_revision = collision_revision;
    slot.snapshot = Arc::from(encoded);
    playsrc_tf2::admission_metrics::emit_value(playsrc_tf2::admission_metrics::SNAPSHOT_ENCODED, 0, slot.snapshot.len() as u64);
    slot.error = 0;
    collision_transaction.committed = true;
    1
}

fn gameplay_error_code(error: &playsrc_tf2::Error) -> u32 {
    1800 + match error {
        playsrc_tf2::Error::Movement(_) => 1,
        playsrc_tf2::Error::PickupTrace(_) => 15,
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
        playsrc_tf2::Error::ControlPoints(_) => 15,
        playsrc_tf2::Error::MissingWeapon { .. } => 17,
        playsrc_tf2::Error::Damage(_) => 16,
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
    gameplay_replay::mutation(handle, gameplay_replay::Mutation::Course, bytes);
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
    let success = session.set_position([x, y, z]).is_ok();
    if success {
        let mut bytes = [0; 12];
        for (chunk, value) in bytes.chunks_exact_mut(4).zip([x, y, z]) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        gameplay_replay::mutation(handle, gameplay_replay::Mutation::Position, &bytes);
    }
    u32::from(success)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_snapshot_length(handle: u32) -> usize {
    with(handle, |slot| slot.snapshot.len()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn playsrc_entity_fire(handle: u32, pointer: *const u8, length: usize) -> u32 {
    if pointer.is_null() || length < 8 || length > 3078 { return 0; }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let delay = f32::from_le_bytes(bytes[..4].try_into().unwrap());
    let fields: Vec<_> = bytes[4..].split(|byte| *byte == 0).collect();
    let [target, input, value] = fields.as_slice() else { return 0; };
    if target.is_empty() || input.is_empty() || !delay.is_finite() || fields.iter().any(|value| value.len() > 1024) { return 0; }
    let Some((index, generation)) = decode(handle) else { return 0; };
    let mut slots = slots().lock().expect("TF2 slots");
    let Some(slot) = slots.get_mut(index).filter(|slot| slot.generation == generation) else { return 0; };
    let Some(session) = slot.session.as_mut() else { return 0; };
    let success = session.fire_entity_input(target, input, value, delay).is_ok();
    if success { gameplay_replay::mutation(handle, gameplay_replay::Mutation::EntityInput, bytes); }
    u32::from(success)
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
    soundscape: playsrc_entity::soundscape::Selection,
    rocket_results: &'a [playsrc_tf2::RocketTraceResult],
    mover_results: &'a [playsrc_tf2::MoverResult],
    collision_snapshot: &'a [u8],
    entity_presentation: &'a playsrc_tf2::EntityPresentationSnapshot,
    payload_constraint_blocked: bool,
    combat_decals: &'a [CombatDecal],
}

fn encode_snapshot(
    snapshot: &playsrc_tf2::Snapshot,
    producer: &playsrc_tf2::ProducerSnapshot,
    respawn_touch_count: u32,
    movement_tick: Option<&playsrc_movement::StepResult>,
    extensions: SnapshotExtensions<'_>,
) -> Option<Vec<u8>> {
    // Charge the borrowed Collision payload against the transaction bound now,
    // but insert it only after the remaining exact output size is known. Growing
    // the vector after inserting that large section repeatedly moves its bytes.
    #[allow(non_snake_case)]
    let MAX = (64_usize * 1024 * 1024).checked_sub(extensions.collision_snapshot.len())?;
    let mut out = Vec::new();
    extend(&mut out, b"PSSN", MAX)?;
    u32_field(&mut out, 30, MAX)?;
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
    let jump_length = out.len();
    u32_field(&mut out, 0, MAX)?;
    let movement_length = out.len();
    u32_field(&mut out, 0, MAX)?;
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
    ] {
        u32_field(&mut out, u32::try_from(count).ok()?, MAX)?;
    }
    let random_length = out.len();
    u32_field(&mut out, 0, MAX)?;
    let entity_length = out.len();
    u32_field(&mut out, 0, MAX)?;
    let movement_tick_length = out.len();
    u32_field(&mut out, 0, MAX)?;
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
    snapshot_section(&mut out, movement_length, |out| {
        snapshot.movement.append_snapshot(out);
        (out.len() <= MAX).then_some(())
    })?;
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
                projectile_visual_code(projectile.kind, projectile.mini_rocket, projectile.trail, projectile.practice_explosion),
                team_code(projectile.team),
                projectile.state as u8 | ((weapon_code(projectile.weapon) & 31) << 3),
                u8::from(projectile.contact_normal.is_some()) | (u8::from(projectile.critical) << 1)
                    | ((weapon_code(projectile.weapon) >> 5) << 2)
                    | (u8::from(projectile.self_blast_only) << 4)
                    | (u8::from(projectile.model_visible) << 5)
                    | (u8::from(projectile.air_burst) << 6)
                    | (u8::from(projectile.underwater_explosion) << 7),
                    // Model visibility is a Source spawn-age rule, not a browser timer.
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
                event.kind as u8 | ((weapon_code(event.weapon) & 31) << 3),
                projectile_visual_code(event.projectile_kind, event.mini_rocket, event.trail, event.practice_explosion),
                team_code(event.team),
                u8::from(event.contact_normal.is_some())
                    | (u8::from(event.launcher_pose.is_some()) << 1)
                    | (u8::from(event.critical) << 2)
                    | ((weapon_code(event.weapon) >> 5) << 3)
                    | (u8::from(event.self_blast_only) << 5)
                    | (u8::from(event.underwater_explosion) << 6)
                    | (u8::from(event.air_burst) << 7),
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
                    playsrc_tf2::weapon::WeaponActivity::MeleeCritical => 8,
                    playsrc_tf2::weapon::WeaponActivity::MeleePrimary => 9,
                    playsrc_tf2::weapon::WeaponActivity::FistLeft => 10,
                    playsrc_tf2::weapon::WeaponActivity::FistRight => 11,
                    playsrc_tf2::weapon::WeaponActivity::Prefire => 12,
                    playsrc_tf2::weapon::WeaponActivity::Postfire => 13,
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
    snapshot_section(&mut out, random_length, |out| {
        encode_random_state(out, extensions.random_state, MAX)
    })?;
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
    let collision_offset = out.len();
    snapshot_section(&mut out, jump_length, |out| {
        match snapshot.jump.as_ref() {
            Some(value) => encode_jump(out, value, MAX),
            None => Some(()),
        }
    })?;
    snapshot_section(&mut out, movement_tick_length, |out| {
        encode_movement_tick(out, movement_tick, MAX)
    })?;
    snapshot_section(&mut out, entity_length, |out| {
        encode_entity_presentation(out, extensions.entity_presentation, MAX)
    })?;
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
                bot.animation_role as u8,
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
    encode_control_points(&mut out, snapshot.control_points.as_ref(), MAX)?;
    encode_objectives(&mut out, snapshot.objectives.as_ref(), MAX)?;
    u32_field(&mut out, u32::from(snapshot.metal), MAX)?;
    u32_field(&mut out, u32::try_from(snapshot.pickups.len()).ok()?, MAX)?;
    for pickup in &snapshot.pickups {
        u32_field(&mut out, pickup.identity, MAX)?;
        extend(
            &mut out,
            &[
                pickup.kind as u8,
                pickup.size as u8,
                pickup.team.unwrap_or(0),
                u8::from(pickup.available) | (u8::from(pickup.disabled) << 1),
            ],
            MAX,
        )?;
        floats(
            &mut out,
            pickup.origin.into_iter().chain(pickup.angles),
            MAX,
        )?;
        u64_field(&mut out, pickup.respawn_tick.unwrap_or(u64::MAX), MAX)?;
    }
    let scoreboard = &snapshot.scoreboard;
    i32_field(&mut out, scoreboard.red_score, MAX)?;
    i32_field(&mut out, scoreboard.blue_score, MAX)?;
    extend(
        &mut out,
        &[
            scoreboard.red_count,
            scoreboard.blue_count,
            u8::try_from(scoreboard.players.len()).ok()?,
            0,
        ],
        MAX,
    )?;
    for player in &scoreboard.players {
        let name = player.name.as_bytes();
        if name.is_empty() || name.len() > 31 {
            return None;
        }
        u32_field(&mut out, player.identity, MAX)?;
        extend(
            &mut out,
            &[
                class_code(player.class),
                team_code(player.team),
                player.alive as u8,
                player.fake as u8,
            ],
            MAX,
        )?;
        i32_field(&mut out, player.score, MAX)?;
        u32_field(&mut out, player.counters.kills, MAX)?;
        u32_field(&mut out, player.counters.deaths, MAX)?;
        u32_field(&mut out, player.counters.captures, MAX)?;
        u32_field(&mut out, player.counters.damage, MAX)?;
        u32_field(&mut out, player.counters.assists, MAX)?;
        extend(&mut out, &[u8::try_from(name.len()).ok()?], MAX)?;
        extend(&mut out, name, MAX)?;
    }
    extend(
        &mut out,
        &[
            u8::try_from(snapshot.buildings.len()).ok()?,
            u8::from(snapshot.placement.is_some()),
            0,
            0,
        ],
        MAX,
    )?;
    if let Some(placement) = snapshot.placement {
        extend(
            &mut out,
            &[
                placement.object.kind as u8,
                placement.object.mode as u8,
                u8::from(placement.valid),
                0,
            ],
            MAX,
        )?;
        floats(
            &mut out,
            placement
                .position
                .into_iter()
                .chain([placement.yaw_degrees]),
            MAX,
        )?;
    }
    for building in &snapshot.buildings {
        u32_field(&mut out, building.identity, MAX)?;
        u32_field(&mut out, building.owner, MAX)?;
        extend(
            &mut out,
            &[
                building.object.kind as u8,
                building.object.mode as u8,
                team_code(building.team),
                building.phase as u8,
                building.level,
                0,
            ],
            MAX,
        )?;
        extend(&mut out, &building.maximum_health.to_le_bytes(), MAX)?;
        f32_field(&mut out, building.health, MAX)?;
        for value in [
            building.upgrade_metal,
            building.shells,
            building.maximum_shells,
            building.rockets,
            building.maximum_rockets,
            building.dispenser_metal,
        ] {
            extend(&mut out, &value.to_le_bytes(), MAX)?;
        }
        u32_field(&mut out, building.target.unwrap_or(u32::MAX), MAX)?;
        floats(
            &mut out,
            building
                .position
                .into_iter()
                .chain([building.yaw_degrees, building.construction]),
            MAX,
        )?;
        u64_field(
            &mut out,
            building.recharge_end_tick.unwrap_or(u64::MAX),
            MAX,
        )?;
        u64_field(&mut out, building.started_tick, MAX)?;
        u32_field(&mut out, building.times_used, MAX)?;
    }
    encode_round(&mut out, &snapshot.round, MAX)?;
    f32_field(&mut out, snapshot.medigun_charge, MAX)?;
    u32_field(&mut out, snapshot.medigun_target.unwrap_or(u32::MAX), MAX)?;
    u32_field(&mut out, u32::from(snapshot.medigun_releasing), MAX)?;
    u32_field(
        &mut out,
        u32::try_from(extensions.combat_decals.len()).ok()?,
        MAX,
    )?;
    for decal in extensions.combat_decals {
        u32_field(&mut out, decal.identity, MAX)?;
        u32_field(&mut out, u32::try_from(decal.fragment.face).ok()?, MAX)?;
        u32_field(
            &mut out,
            u32::try_from(decal.fragment.positions.len()).ok()?,
            MAX,
        )?;
        u32_field(
            &mut out,
            u32::try_from(decal.fragment.triangles.len()).ok()?,
            MAX,
        )?;
        u32_field(&mut out, u32::try_from(decal.reference.len()).ok()?, MAX)?;
        extend(&mut out, decal.reference.as_bytes(), MAX)?;
        for ((position, normal), uv) in decal
            .fragment
            .positions
            .iter()
            .zip(&decal.fragment.normals)
            .zip(&decal.fragment.uv)
        {
            floats(
                &mut out,
                position.iter().chain(normal).chain(uv).copied(),
                MAX,
            )?;
        }
        for triangle in &decal.fragment.triangles {
            for index in triangle {
                u32_field(&mut out, *index, MAX)?;
            }
        }
    }
    let cloak_count = usize::from(snapshot.spy.is_some())
        + snapshot.bots.iter().filter(|bot| bot.spy.is_some()).count();
    u32_field(&mut out, cloak_count as u32, MAX)?;
    let mut cloak = |identity: u32,
                     spy: playsrc_tf2::spy::SpyState,
                     team: playsrc_tf2::PlayerTeam,
                     alive: bool|
     -> Option<()> {
        let state = playsrc_tf2::spy::cloak_render_state(
            if alive { spy.invisibility } else { 0.0 },
            alive && spy.blink(snapshot.tick as f32 * 0.015),
            false,
            team,
            snapshot.team.is_gameplay() && team != snapshot.team,
            false,
        );
        u32_field(&mut out, identity, MAX)?;
        floats(
            &mut out,
            [state.local_factor, state.world_factor, state.raw_factor]
                .into_iter()
                .chain(state.player_tint),
            MAX,
        )
    };
    if let Some(spy) = snapshot.spy {
        cloak(
            playsrc_tf2::PLAYER_IDENTITY,
            spy,
            snapshot.team,
            snapshot.health > 0.0,
        )?;
    }
    for bot in &snapshot.bots {
        if let Some(spy) = bot.spy {
            cloak(
                bot.identity,
                spy,
                bot.team,
                bot.lifecycle == playsrc_tf2::PlayerLifecycle::Active,
            )?;
        }
    }
    playsrc_tf2::equipment::encode_items(&mut out, &snapshot.equipped_items);
    for bot in &snapshot.bots {
        playsrc_tf2::equipment::encode_items(&mut out, &bot.equipped_items);
        for word in bot.conditions { out.extend_from_slice(&word.to_le_bytes()); }
        out.extend_from_slice(&(bot.class.standing_eye_height() + 20.0).to_le_bytes());
    }
    for value in snapshot.view_angle_offset { f32_field(&mut out, value, MAX)?; }
    extend(&mut out, &producer.decapitations.to_le_bytes(), MAX)?;
    extend(&mut out, &producer.revenge_crits.to_le_bytes(), MAX)?;
    f32_field(&mut out, snapshot.weapon_crosshair_scale, MAX)?;
    i32_field(&mut out, extensions.soundscape.entity, MAX)?;
    i32_field(&mut out, extensions.soundscape.soundscape, MAX)?;
    u32_field(&mut out, u32::from(extensions.soundscape.position_bits), MAX)?;
    for position in extensions.soundscape.positions { floats(&mut out, position, MAX)?; }
    out.reserve_exact(extensions.collision_snapshot.len());
    // Keep the large borrowed section on the bulk-copy path. Vec::splice fills
    // its gap through Iterator::next, which costs a per-byte loop in WASM.
    let end = out.len();
    let collision_end = collision_offset + extensions.collision_snapshot.len();
    out.resize(end + extensions.collision_snapshot.len(), 0);
    out.copy_within(collision_offset..end, collision_end);
    out[collision_offset..collision_end].copy_from_slice(extensions.collision_snapshot);
    Some(out)
}

fn encode_round(
    out: &mut Vec<u8>,
    round: &playsrc_tf2::round::Snapshot,
    maximum: usize,
) -> Option<()> {
    use playsrc_tf2::round::Event;
    extend(out, b"PGRL", maximum)?;
    u32_field(out, 4, maximum)?;
    let timer = round.timer;
    let flags = u8::from(round.waiting_for_players)
        | (u8::from(round.in_setup) << 1)
        | (u8::from(round.in_overtime) << 2)
        | (u8::from(timer.is_some()) << 3)
        | (u8::from(timer.is_some_and(|value| value.paused)) << 4)
        | (u8::from(timer.is_some_and(|value| value.configuration.show_in_hud)) << 5)
        | (u8::from(timer.is_some_and(|value| value.disabled)) << 6)
        | (u8::from(round.koth_timers.is_some()) << 7);
    extend(
        out,
        &[
            round.state as u8 | (u8::from(!round.full_round) << 7),
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
        u32_field(out, value as u32, maximum)?;
    }
    u32_field(out, u32::try_from(round.events.len()).ok()?, maximum)?;
    if let Some(timers) = round.koth_timers {
        for timer in timers {
            u32_field(out, timer.configuration.identity, maximum)?;
            f32_field(out, timer.remaining, maximum)?;
            for seconds in [timer.configuration.initial_seconds, timer.configuration.setup_seconds, timer.configuration.maximum_seconds] {
                u32_field(out, seconds as u32, maximum)?;
            }
            u32_field(out, u32::from(timer.paused) | (u32::from(timer.configuration.show_in_hud) << 1) | (u32::from(timer.disabled) << 2), maximum)?;
        }
    }
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
            Event::TimerThreshold { timer, seconds } => (13, (seconds & 255) as u8, 0, (seconds >> 8) as u8, timer),
            Event::TimerWarning { timer, seconds, .. } => (14, seconds as u8, 0, 0, timer),
            Event::MapRoundWin { entity } => (15, 0, 0, 0, entity),
            Event::TimerTimeAdded { timer, .. } => (16, 0, 0, 0, timer),
            Event::WinningCapper { player } => (17, 0, 0, 0, player),
        };
        extend(out, &[kind, detail, team, flags], maximum)?;
        u32_field(out, identity, maximum)?;
        i32_field(out, match *event { Event::TimerTimeAdded { seconds, .. } => seconds, _ => 0 }, maximum)?;
    }
    Some(())
}

fn encode_control_points(out: &mut Vec<u8>, points: Option<&playsrc_tf2::control_point::Snapshot>, maximum: usize) -> Option<()> {
    use playsrc_tf2::PlayerTeam;
    let begin = out.len();
    extend(out, b"PCPN", maximum)?;
    u32_field(out, 0, maximum)?;
    u32_field(out, points.map_or(0, |p| p.points.len() as u32), maximum)?;
    if let Some(world) = points {
        for value in world.master.custom_position { f32_field(out,value,maximum)?; }
        u16_field(out,u16::try_from(world.master.cap_layout.len()).ok()?,maximum)?;
        extend(out,world.master.cap_layout.as_bytes(),maximum)?;
        i32_field(out,world.local_point.map_or(-1,|p| p as i32),maximum)?;
        u16_field(out,u16::try_from(world.local_capture_text.len()).ok()?,maximum)?;
        extend(out,world.local_capture_text.as_bytes(),maximum)?;
        for point in &world.points {
            let area = world.areas.iter().rev().find(|a| a.point == point.index);
            let flags = u8::from(point.locked) | (u8::from(point.visible)<<1) | (u8::from(point.model_visible)<<2) | ((point.skin() as u8)<<6)
                | (u8::from(area.is_some_and(|a| a.blocked))<<3)
                | (u8::from(world.may_capture[point.index][0] && area.is_some_and(|a| a.teams[2].can_cap))<<4)
                | (u8::from(world.may_capture[point.index][1] && area.is_some_and(|a| a.teams[3].can_cap))<<5);
            u32_field(out,point.identity,maximum)?;
            extend(out,&[team_code(point.owner),area.map_or(0,|a| team_code(a.capturing_team)),area.map_or(0,|a| team_code(a.team_in_zone)),flags],maximum)?;
            for value in [area.map_or(0.0,|a| a.remaining),world.display_progress[point.index],point.unlock_at.unwrap_or(-1.0),
                area.map_or(0.0,|a| a.total_time(PlayerTeam::Red,world.configuration)),area.map_or(0.0,|a| a.total_time(PlayerTeam::Blue,world.configuration))] { f32_field(out,value,maximum)?; }
            for team in [2,3] { i32_field(out,area.map_or(0,|a| a.num_players[team]),maximum)?; }
            for team in [2,3] { i32_field(out,area.map_or(1,|a| a.teams[team].required),maximum)?; }
            floats(out,point.position.into_iter().chain(point.angles),maximum)?;
            i32_field(out,point.body(),maximum)?;
            for text in [&point.print_name,&point.icons[point.owner as usize],&point.models[point.owner as usize],&point.overlays[point.owner as usize]] {
                u16_field(out,u16::try_from(text.len()).ok()?,maximum)?; extend(out,text.as_bytes(),maximum)?;
            }
            u32_field(out,area.map_or(0,|a| a.touching.len() as u32),maximum)?;
            if let Some(area) = area { for player in &area.touching { u32_field(out,*player,maximum)?; } }
        }
    }
    let length = u32::try_from(out.len()-begin).ok()?;
    out[begin+4..begin+8].copy_from_slice(&length.to_le_bytes());
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

// Section lengths come from the same writes as their payload, never a second
// schema/size authority. This vector is private to the transaction until all
// writes and length patches succeed; outstanding publication leases are untouched.
fn snapshot_section(
    out: &mut Vec<u8>,
    length_field: usize,
    encode: impl FnOnce(&mut Vec<u8>) -> Option<()>,
) -> Option<()> {
    let start = out.len();
    encode(out)?;
    let length = u32::try_from(out.len().checked_sub(start)?).ok()?;
    out.get_mut(length_field..length_field.checked_add(4)?)?
        .copy_from_slice(&length.to_le_bytes());
    Some(())
}

fn encode_entity_presentation(
    output: &mut Vec<u8>,
    snapshot: &playsrc_tf2::EntityPresentationSnapshot,
    limit: usize,
) -> Option<()> {
    #[allow(non_snake_case)]
    let MAX = output.len().checked_add(8 * 1024 * 1024)?.min(limit);
    let e = &snapshot.entities;
    let mut out = output;
    extend(&mut out, b"PEBP", MAX)?;
    u32_field(&mut out, 3, MAX)?;
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
    u32_field(
        &mut out,
        u32::try_from(snapshot.studio_models.len()).ok()?,
        MAX,
    )?;
    for model in &snapshot.studio_models {
        u32_field(&mut out, u32::try_from(model.source_index).ok()?, MAX)?;
        floats(
            &mut out,
            model
                .world_transform
                .origin
                .into_iter()
                .chain(model.world_transform.angles),
            MAX,
        )?;
        u32_field(&mut out, u32::from(model.draw) | ((model.skin.max(0) as u32) << 1), MAX)?;
    }
    u32_field(
        &mut out,
        u32::try_from(snapshot.studio_animations.len()).ok()?,
        MAX,
    )?;
    for animation in &snapshot.studio_animations {
        u32_field(&mut out, animation.source_index, MAX)?;
        floats(&mut out, [animation.elapsed_seconds], MAX)?;
        u32_field(&mut out, u32::try_from(animation.sequence.len()).ok()?, MAX)?;
        floats(&mut out, animation.bounds.into_iter().flatten(), MAX)?;
        extend(&mut out, &animation.sequence, MAX)?;
    }
    Some(())
}

fn encode_random_state(output: &mut Vec<u8>, state: playsrc_tf2::Tf2RandomState, limit: usize) -> Option<()> {
    let start = output.len();
    if start.checked_add(368)? > limit { return None; }
    output.extend_from_slice(b"PRNG");
    output.extend_from_slice(&4_u32.to_le_bytes());
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
        state.sound_selection.fist_miss_available
            | state.sound_selection.bonesaw_hit_flesh_available << 2,
        state.sound_selection.fist_hit_world_available
            | state.sound_selection.bonesaw_hit_world_available << 2,
        state.sound_selection.fist_hit_flesh_available,
        state.sound_selection.kukri_hit_flesh_available
            | state.sound_selection.kukri_hit_world_available << 3
            | state.sound_selection.wrench_hit_flesh_available << 5,
        state.sound_selection.flag_enemy_stolen_available,
        state.sound_selection.flag_enemy_dropped_available,
        state.sound_selection.flag_enemy_captured_available,
        state.sound_selection.flag_enemy_returned_available,
        state.sound_selection.flag_team_dropped_available,
        state.sound_selection.overtime_available,
        state.sound_selection.control_point_available as u8,
        (state.sound_selection.control_point_available >> 8) as u8,
    ]);
    output.extend_from_slice(&state.sound_selection.configured_available);
    output.extend_from_slice(&[
        state.sound_selection.projectile_unlock_available[0]
            | state.sound_selection.projectile_unlock_available[1] << 3
            | (state.sound_selection.projectile_unlock_available[2] & 3) << 6,
        state.sound_selection.projectile_unlock_available[2] >> 2
            | state.sound_selection.projectile_unlock_available[3] << 1
            | state.sound_selection.projectile_unlock_available[4] << 4,
        state.sound_selection.projectile_unlock_available[5],
        0,
    ]);
    for mask in state.sound_selection.payload_warning_available { output.extend_from_slice(&mask.to_le_bytes()); }
    (output.len().checked_sub(start)? == 368).then_some(())
}

fn encode_random_draw(
    output: &mut Vec<u8>,
    draw: playsrc_tf2::RandomDraw,
    limit: usize,
) -> Option<()> {
    let (decision, definition, phase) = match draw.decision {
        playsrc_tf2::RandomDecision::SoundVolume { definition, phase } => {
            (1, definition.code(), phase)
        }
        playsrc_tf2::RandomDecision::SoundPitch { definition, phase } => {
            (2, definition.code(), phase)
        }
        playsrc_tf2::RandomDecision::SoundWave { definition, phase } => {
            (3, definition.code(), phase)
        }
        playsrc_tf2::RandomDecision::SoundLevel { definition, phase } => {
            (4, definition.code(), phase)
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
        playsrc_tf2::RandomDecision::SyringePitchSpread => {
            (9, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
        playsrc_tf2::RandomDecision::SyringeYawSpread => {
            (10, 0, playsrc_tf2::SoundQueryPhase::Inspect)
        }
        playsrc_tf2::RandomDecision::WeaponCritical => (14, 0, playsrc_tf2::SoundQueryPhase::Inspect),
        playsrc_tf2::RandomDecision::EnemySpeedOnHit => (64, 0, playsrc_tf2::SoundQueryPhase::Inspect),
        playsrc_tf2::RandomDecision::BulletSpread => (65, 0, playsrc_tf2::SoundQueryPhase::Inspect),
        playsrc_tf2::RandomDecision::ScorchShotBounceVelocity => (11, 0, playsrc_tf2::SoundQueryPhase::Inspect),
        playsrc_tf2::RandomDecision::ScorchShotBounceAngle => (12, 0, playsrc_tf2::SoundQueryPhase::Inspect),
        playsrc_tf2::RandomDecision::ScorchShotBounceSign => (13, 0, playsrc_tf2::SoundQueryPhase::Inspect),
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
                playsrc_tf2::AudioEventIdentity::ItemPickup => 3,
                playsrc_tf2::AudioEventIdentity::ItemMaterialize => 4,
                playsrc_tf2::AudioEventIdentity::PlayerFeedback => 5,
            },
            event.definition.code(),
            match event.source_kind {
                playsrc_tf2::AudioSourceKind::Entity => 1,
                playsrc_tf2::AudioSourceKind::World => 2,
                playsrc_tf2::AudioSourceKind::LocalListener => 3,
                playsrc_tf2::AudioSourceKind::ControlPoint => 4,
            },
            u8::from(event.owner_identity.is_some()),
            event.samples.wave,
            match event.action {
                playsrc_tf2::AudioAction::Play => 0,
                playsrc_tf2::AudioAction::PlayAtPitch(_) => 4,
                playsrc_tf2::AudioAction::Stop => 1,
                playsrc_tf2::AudioAction::FadeIn(_) => 2,
                playsrc_tf2::AudioAction::FadeOut(_) => 3,
            },
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
    floats(
        output,
        [match event.action {
            playsrc_tf2::AudioAction::FadeIn(seconds)
            | playsrc_tf2::AudioAction::FadeOut(seconds) => seconds,
            playsrc_tf2::AudioAction::PlayAtPitch(pitch) => pitch,
            _ => 0.0,
        }],
        limit,
    )
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
            ..
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
    weapon as u8
}
fn projectile_code(kind: playsrc_tf2::ProjectileKind) -> u8 {
    match kind {
        playsrc_tf2::ProjectileKind::Rocket => 1,
        playsrc_tf2::ProjectileKind::Sticky => 2,
        playsrc_tf2::ProjectileKind::Syringe => 3,
        playsrc_tf2::ProjectileKind::Flare => 4,
    }
}

fn projectile_visual_code(kind: playsrc_tf2::ProjectileKind, mini: bool, trail: playsrc_tf2::ProjectileTrail, practice: bool) -> u8 {
    projectile_code(kind) | (u8::from(mini) << 3) | ((trail as u8) << 4) | (u8::from(practice) << 7)
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
    if let playsrc_tf2::Event::PlayerKilled { attacker, victim, weapon, killing_weapon, assister, damage_bits, critical, custom } = event {
        extend(output, &[18, weapon.map_or(0, weapon_code)], limit)?;
        u16_field(output, u16::try_from(killing_weapon.len()).ok()?, limit)?;
        u32_field(output, *victim, limit)?;
        u32_field(output, *attacker, limit)?;
        u32_field(output, *assister, limit)?;
        u32_field(output, *damage_bits | if *critical { 1 << 20 } else { 0 }, limit)?;
        u32_field(output, u32::from(*custom), limit)?;
        u32_field(output, 0, limit)?; // No rivalry/silent/feign facts are produced by current combat.
        return extend(output, killing_weapon.as_bytes(), limit);
    }
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
        playsrc_tf2::Event::Damaged {
            amount,
            health,
            origin,
        } => {
            let position = origin.unwrap_or([0.0; 3]);
            (
                6,
                u8::from(origin.is_some()),
                position[2].to_bits(),
                0,
                [*amount, *health, position[0], position[1]],
            )
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
        playsrc_tf2::Event::HitscanFired {
            weapon,
            pellets,
            owner,
            origin,
        } => (
            12,
            weapon_code(*weapon),
            u32::from(*pellets),
            *owner,
            [origin[0], origin[1], origin[2], 0.0],
        ),
        playsrc_tf2::Event::HitscanImpact {
            weapon,
            target,
            pellet,
            hitgroup,
            critical,
            position,
            damage,
            surface,
        } => (
            13,
            weapon_code(*weapon),
            target.unwrap_or_else(|| {
                surface
                    .and_then(|value| value.face)
                    .map_or(0, |face| face | 0x8000_0000)
            }),
            u32::from(*pellet)
                | (u32::from(*hitgroup) << 8)
                | (u32::from(*critical) << 16)
                | (u32::from(surface.map_or(0, |value| value.game_material)) << 24),
            [position[0], position[1], position[2], *damage],
        ),
        playsrc_tf2::Event::MeleeImpact {
            weapon,
            owner,
            target,
            position,
            damage,
        } => (
            14,
            weapon_code(*weapon),
            target.unwrap_or(0),
            *owner,
            [position[0], position[1], position[2], *damage],
        ),
        playsrc_tf2::Event::PickedUp {
            entity,
            player,
            kind,
            size,
            amount,
            health,
            weapon,
            clip,
            reserve,
        } => (
            if *kind == playsrc_tf2::pickup::MapPickupKind::Health {
                15
            } else {
                16
            },
            *size as u8,
            *entity,
            *player,
            [
                *amount as f32,
                *health as f32,
                *clip as f32,
                if weapon.is_some() {
                    *reserve as f32
                } else {
                    0.0
                },
            ],
        ),
        playsrc_tf2::Event::PlayerDamaged {
            attacker,
            victim,
            weapon,
            amount,
            health,
            crit,
            custom,
        } => (
            17,
            weapon_code(*weapon),
            *victim,
            *attacker,
            [
                *amount as f32,
                *health as f32,
                match crit { playsrc_tf2::damage::CritKind::None => 0.0, playsrc_tf2::damage::CritKind::Full => 1.0, playsrc_tf2::damage::CritKind::Mini => 2.0 },
                f32::from(*custom),
            ],
        ),
        playsrc_tf2::Event::PlayerKilled { .. } => unreachable!(),
        playsrc_tf2::Event::PlayerRespawned { player, team } => {
            (19, team_code(*team), *player, 0, [0.0; 4])
        }
        playsrc_tf2::Event::ProjectileWeaponEffect { weapon, effect } => (24, weapon_code(*weapon), *effect as u32, 0, [0.0; 4]),
    };
    extend(output, &[kind, detail, 0, 0], limit)?;
    u32_field(output, subject, limit)?;
    u32_field(output, auxiliary, limit)?;
    floats(output, values, limit)
}

fn encode_jump(bytes: &mut Vec<u8>, output: &playsrc_tf2::jump::TickOutput, limit: usize) -> Option<()> {
    #[allow(non_snake_case)]
    let MAX = bytes.len().checked_add(4 * 1024 * 1024)?.min(limit);
    let mut bytes = bytes;
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
    Some(())
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

fn combat_decal_world(
    surfaces: Vec<playsrc_map::Surface>,
    materials: &[playsrc_material::Material],
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
) -> Result<CombatDecalWorld, ()> {
    let bytes = bundle
        .get("scripts/decals_subrect.txt")
        .copied()
        .ok_or(())?;
    let document = playsrc_keyvalues::parse_text(
        bytes,
        playsrc_keyvalues::EscapeMode::Escaped,
        playsrc_keyvalues::Limits::default(),
    )
    .map_err(|_| ())?;
    let groups = document
        .roots
        .iter()
        .map(|node| (node.key.bytes.to_ascii_lowercase(), node))
        .collect::<BTreeMap<_, _>>();
    let translation = groups.get(b"translationdata".as_slice()).ok_or(())?;
    let playsrc_keyvalues::Value::Object(entries) = &translation.value else {
        return Err(());
    };
    let mut variants = BTreeMap::new();
    for entry in entries {
        if entry.key.bytes.len() != 1 {
            return Err(());
        }
        let playsrc_keyvalues::Value::Scalar(target) = &entry.value else {
            return Err(());
        };
        if target.token.bytes.is_empty() {
            continue;
        }
        let Some(group) = groups.get(&target.token.bytes.to_ascii_lowercase()) else {
            return Err(());
        };
        let playsrc_keyvalues::Value::Object(children) = &group.value else {
            return Err(());
        };
        let mut choices = Vec::new();
        for child in children {
            let playsrc_keyvalues::Value::Scalar(weight) = &child.value else {
                return Err(());
            };
            let weight = std::str::from_utf8(&weight.token.bytes)
                .map_err(|_| ())?
                .parse::<f32>()
                .map_err(|_| ())?;
            let reference = std::str::from_utf8(&child.key.bytes).map_err(|_| ())?;
            let path = dependency_path(&child.key.bytes)?;
            let material = resolve_material_semantics(
                &path,
                bundle,
                playsrc_material::SelectionEnvironment::default(),
            )?;
            let size = |name: &[u8]| -> Result<[f32; 2], ()> {
                let value = material.first_parameters.get(name).ok_or(())?;
                let values = std::str::from_utf8(value)
                    .map_err(|_| ())?
                    .split_whitespace()
                    .map(|component| component.parse::<f32>().map_err(|_| ()))
                    .collect::<Result<Vec<_>, _>>()?;
                values.try_into().map_err(|_| ())
            };
            let position = size(b"$pos")?;
            let dimensions = size(b"$size")?;
            let texture = material
                .textures
                .iter()
                .find(|texture| material.selected_textures.contains(&texture.role))
                .and_then(|texture| texture.logical_path.as_ref())
                .ok_or(())?;
            let atlas = decoders.metadata(&texture.to_ascii_lowercase())?;
            let width = atlas.width as f32;
            let height = atlas.height as f32;
            choices.push(CombatDecalVariant {
                reference: reference.to_owned(),
                offset: [(position[0] + 1.0) / width, (position[1] + 1.0) / height],
                scale: [
                    (dimensions[0] - 2.0) / width,
                    (dimensions[1] - 2.0) / height,
                ],
                dimensions: dimensions.map(|value| value * material.decal.scale),
                weight,
            });
        }
        variants.insert(entry.key.bytes[0].to_ascii_uppercase(), choices);
    }
    let mut output = BTreeMap::new();
    for mut surface in surfaces {
        let receiving = materials.get(surface.material).ok_or(())?.decal;
        surface.normals.clear();
        surface.alpha.clear();
        surface.uv.clear();
        surface.lightmap_uv.clear();
        surface.triangles.clear();
        output.insert(
            u32::try_from(surface.face).map_err(|_| ())?,
            CombatDecalSurface { surface, receiving },
        );
    }
    Ok(CombatDecalWorld {
        surfaces: output,
        variants,
        random: playsrc_tf2::UniformRandomStream::from_seed(0).map_err(|_| ())?,
        serial: 0,
    })
}

impl CombatDecalWorld {
    fn project(&mut self, events: &[playsrc_tf2::Event]) -> Result<Vec<CombatDecal>, ()> {
        let mut decals = Vec::new();
        for event in events {
            let playsrc_tf2::Event::HitscanImpact {
                target: None,
                position,
                surface: Some(surface),
                ..
            } = event
            else {
                continue;
            };
            let Some(face) = surface.face.and_then(|face| self.surfaces.get(&face)) else {
                continue;
            };
            let Some(choices) = self.variants.get(&surface.game_material) else {
                continue;
            };
            let mut selected = None;
            let mut total = 0.0;
            for variant in choices {
                if total == 0.0 {
                    selected = Some(variant);
                }
                total += variant.weight;
                if total == 0.0 || self.random.random_float(0.0, total) < variant.weight {
                    selected = Some(variant);
                }
            }
            let Some(variant) = selected else {
                continue;
            };
            let Some(mut fragment) = playsrc_map::project_combat_decal(
                &face.surface,
                &face.receiving,
                *position,
                variant.dimensions,
            )
            .map_err(|_| ())?
            else {
                continue;
            };
            for uv in &mut fragment.uv {
                uv[0] = variant.offset[0] + uv[0] * variant.scale[0];
                uv[1] = variant.offset[1] + uv[1] * variant.scale[1];
            }
            self.serial = self.serial.checked_add(1).ok_or(())?;
            decals.push(CombatDecal {
                identity: self.serial,
                reference: variant.reference.clone(),
                fragment,
            });
        }
        Ok(decals)
    }
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
    if identity.ends_with("_subrect.vmt") {
        let source = playsrc_keyvalues::parse_text(
            root,
            playsrc_keyvalues::EscapeMode::Escaped,
            playsrc_keyvalues::Limits::default(),
        )
        .map_err(|_| ())?;
        let document = source
            .roots
            .first()
            .filter(|document| document.key.bytes.eq_ignore_ascii_case(b"Subrect"))
            .ok_or(())?;
        let playsrc_keyvalues::Value::Object(fields) = &document.value else {
            return Err(());
        };
        let parent = fields
            .iter()
            .find(|field| field.key.bytes.eq_ignore_ascii_case(b"$material"))
            .and_then(|field| match &field.value {
                playsrc_keyvalues::Value::Scalar(value) => Some(value.token.bytes.as_slice()),
                _ => None,
            })
            .ok_or(())?;
        let mut material =
            resolve_material_semantics(&dependency_path(parent)?, bundle, environment)?;
        for field in fields {
            if let playsrc_keyvalues::Value::Scalar(value) = &field.value {
                material
                    .first_parameters
                    .entry(field.key.bytes.to_ascii_lowercase())
                    .or_insert_with(|| value.token.bytes.clone());
            }
        }
        if let Some(scale) = material.first_parameters.get(b"$decalscale".as_slice()) {
            material.decal.scale = std::str::from_utf8(scale)
                .map_err(|_| ())?
                .parse::<f32>()
                .map_err(|_| ())?;
        }
        return Ok(material);
    }
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
        playsrc_material::Shader::DecalModulate => 11,
        playsrc_material::Shader::Modulate => 12,
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
    playsrc_map::CubeFace::ALL
        .into_iter()
        .map(|face| {
            let identity = face.material_path(sky).to_ascii_lowercase();
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
    format: playsrc_vtf::ImageFormat,
) -> Option<playsrc_map::SkyEncoding> {
    match selected {
        [playsrc_material::TextureRole::Base | playsrc_material::TextureRole::HdrBase] => Some(
            if matches!(format, playsrc_vtf::ImageFormat::Rgba16F | playsrc_vtf::ImageFormat::Rgba16) {
                playsrc_map::SkyEncoding::Linear
            } else {
                playsrc_map::SkyEncoding::Srgb
            }
        ),
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
    let mut render_bounds = BTreeMap::new();
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
            runtime_transform: true,
        });
    }
    for prop in &map.static_props.occurrences {
        if prop.solidity == 0 {
            continue;
        }
        let model = &map
            .static_props
            .models
            .get(prop.model)
            .ok_or(())?
            .logical_path;
        if prop.solidity == 2 {
            let bounds = if let Some(bounds) = render_bounds.get(model) {
                *bounds
            } else {
                let bounds = playsrc_studio_model::read_model_render_bounds(model, bundle.get(model).ok_or(())?, playsrc_studio_model::Limits::default()).map_err(|_| ())?;
                render_bounds.insert(model.clone(), bounds);
                bounds
            };
            let vector = |values: [f32; 3]| playsrc_studio_model::Vector3(values.map(|value| playsrc_studio_model::Float32(value.to_bits())));
            let transform = playsrc_studio_model::source_entity_transform(vector(prop.origin), vector(prop.angles)).map_err(|_| ())?;
            let [mins, maxs] = playsrc_studio_model::transform_model_render_bounds(bounds, transform).map_err(|_| ())?;
            output.push(CollisionObjectTemplate {
                input: playsrc_collision::ObjectInput {
                    identity: static_prop_collision_identity(prop.source)?,
                    role: playsrc_collision::ObjectRole::StaticProp,
                    enabled: true, volume_contents: false,
                    transform: playsrc_collision::Transform { origin: prop.origin, angles: [0.0; 3] },
                    linear_velocity: [0.0; 3], angular_velocity: [0.0; 3], collision_group: 0,
                    contents: CONTENTS_SOLID, surface_flags: 0,
                    shape: playsrc_collision::SnapshotShape::BoundingBox { bounds: playsrc_collision::Hull {
                        mins: std::array::from_fn(|axis| f32::from_bits(mins.0[axis].0) - prop.origin[axis]),
                        maxs: std::array::from_fn(|axis| f32::from_bits(maxs.0[axis].0) - prop.origin[axis]),
                    } },
                },
                runtime_transform: false,
            });
            continue;
        }
        if prop.solidity != 6 { return Err(()); }
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
                identity: static_prop_collision_identity(prop.source)?,
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

// Dynamic props (including gate children) share the entity world's resolved
// parent transform with presentation. A brush-only snapshot cannot own their
// collision pose. Removed entities must not leave a solid at the authored pose.
fn collision_template_transform(
    template: &CollisionObjectTemplate,
    entity_transform: Option<&dyn Fn(u32) -> Option<(playsrc_entity::Transform,bool)>>,
    overrides: &BTreeMap<u64, playsrc_collision::Transform>,
) -> (bool, playsrc_collision::Transform) {
    let mut enabled = template.input.enabled;
    let mut transform = template.input.transform;
    if template.runtime_transform && let Some(resolve) = entity_transform {
        match u32::try_from(template.input.identity).ok().and_then(resolve) {
            Some((value,solid)) => { transform = playsrc_collision::Transform { origin: value.origin, angles: value.angles }; enabled=solid || template.input.volume_contents; },
            None => enabled = false,
        }
    }
    if let Some(value) = overrides.get(&template.input.identity) { transform = *value; }
    (enabled, transform)
}

fn compile_collision_snapshot(
    previous: Option<&playsrc_collision::Snapshot>,
    world: &playsrc_collision::World,
    templates: &[CollisionObjectTemplate],
    revision: u64,
    latest: Option<&playsrc_tf2::Snapshot>,
    entity_transform: Option<&dyn Fn(u32) -> Option<(playsrc_entity::Transform,bool)>>,
    transform_overrides: &BTreeMap<u64, playsrc_collision::Transform>,
    velocity_overrides: &BTreeMap<u64, [f32; 3]>,
) -> Result<playsrc_collision::Snapshot, playsrc_collision::Error> {
    let inputs = templates
        .iter()
        .map(|template| {
            let mut input = template.input.clone();
            (input.enabled, input.transform) = collision_template_transform(template, entity_transform, transform_overrides);
            if let Some(velocity) = velocity_overrides.get(&input.identity) {
                input.linear_velocity = *velocity;
            }
            input
        })
        .chain(
            latest
                .into_iter()
                .flat_map(|snapshot| snapshot.buildings.iter())
                .map(|building| playsrc_collision::ObjectInput {
                    identity: u64::from(building.identity),
                    role: playsrc_collision::ObjectRole::Entity,
                    enabled: true,
                    volume_contents: false,
                    transform: playsrc_collision::Transform {
                        origin: building.position,
                        angles: [0.0, building.yaw_degrees, 0.0],
                    },
                    linear_velocity: [0.0; 3],
                    angular_velocity: [0.0; 3],
                    collision_group: 0,
                    contents: playsrc_collision::CONTENTS_SOLID,
                    surface_flags: 0,
                    shape: playsrc_collision::SnapshotShape::OrientedBox {
                        bounds: building.object.hull(),
                    },
                }),
        )
        .collect();
    match previous {
        Some(previous) => previous.recompile(world, revision, inputs),
        None => playsrc_collision::Snapshot::compile(
            world,
            revision,
            inputs,
            playsrc_collision::SnapshotLimits::default(),
        ),
    }
}

fn retain_collision_snapshot(
    previous: &playsrc_collision::Snapshot,
    templates: &[CollisionObjectTemplate],
    revision: u64,
    entity_transform: Option<&dyn Fn(u32) -> Option<(playsrc_entity::Transform,bool)>>,
    transform_overrides: &BTreeMap<u64, playsrc_collision::Transform>,
    velocity_overrides: &BTreeMap<u64, [f32; 3]>,
) -> Option<playsrc_collision::Snapshot> {
    if previous.records().len() != templates.len() {
        return None;
    }
    for (record, template) in previous.records().iter().zip(templates) {
        let identity = template.input.identity;
        if record.identity != identity {
            return None;
        }
        let (enabled, transform) = collision_template_transform(template, entity_transform, transform_overrides);
        let velocity = velocity_overrides
            .get(&identity)
            .copied()
            .unwrap_or(template.input.linear_velocity);
        if record.enabled != enabled || record.transform != transform || record.linear_velocity != velocity {
            return None;
        }
    }
    Some(previous.with_identity(revision))
}

#[cfg(test)]
#[test]
fn setup_gate_child_collision_follows_parent_and_invalidates_retention() {
    use playsrc_collision::{ObjectInput, ObjectRole, SnapshotShape, Transform, CONTENTS_SOLID};
    let graph = playsrc_entity::parse(br#"
        {"classname" "func_door" "targetname" "gate" "model" "*1" "movedir" "-90 0 0" "lip" "0" "wait" "-1"}
        {"classname" "prop_dynamic" "parentname" "gate" "origin" "0 0 0" "solid" "6" "model" "models/gate.mdl"}
    "#, playsrc_entity::Limits::default()).unwrap();
    let mut map = playsrc_tf2::MapRuntime::compile(&graph, 0.015, 1,
        vec![playsrc_entity::ModelBounds { model: 1, mins: [-64.0,-4.0,0.0], maxs: [64.0,4.0,128.0] }]).unwrap();
    let world = playsrc_collision::World::empty();
    let templates = [CollisionObjectTemplate {
        input: ObjectInput { identity: 1, role: ObjectRole::Entity, enabled: true, volume_contents: false,
            transform: Transform { origin: [0.0;3], angles: [0.0;3] }, linear_velocity: [0.0;3], angular_velocity: [0.0;3],
            collision_group: 0, contents: CONTENTS_SOLID, surface_flags: 0,
            shape: SnapshotShape::BoundingBox { bounds: playsrc_collision::Hull { mins: [-64.0,-4.0,0.0], maxs: [64.0,4.0,128.0] } } },
        runtime_transform: true,
    }];
    let transforms = BTreeMap::new();
    let velocities = BTreeMap::new();
    let closed = compile_collision_snapshot(None, &world, &templates, 1, None, None, &transforms, &velocities).unwrap();
    let request = map.input(0, 0, b"Open", playsrc_entity::Variant::Void).unwrap().mover_requests[0];
    map.apply_mover_results(1, &[playsrc_tf2::MoverResult { request_id: request.request_id, entity: 0,
        kind: playsrc_tf2::MoverResultKind::Completed, transform: playsrc_entity::Transform { origin: [0.0,0.0,128.0], angles: [0.0;3] }, carry: [0.0;3] }]).unwrap();
    assert!(map.entity_descends_from(1,0));
    assert!(!map.entity_descends_from(0,1));
    let resolve = |identity| map.entity_collision_state(identity);
    assert!(retain_collision_snapshot(&closed, &templates, 2, Some(&resolve), &transforms, &velocities).is_none(), "moving a gate child must invalidate the static collision cache");
    let opened = compile_collision_snapshot(Some(&closed), &world, &templates, 2, None, Some(&resolve), &transforms, &velocities).unwrap();
    let child = map.entity_world_transform(1).unwrap();
    assert!(child.origin[2] > 120.0);
    assert_eq!(opened.records()[0].transform.origin, child.origin);
    assert!(retain_collision_snapshot(&opened, &templates, 3, Some(&resolve), &transforms, &velocities).is_some());
    map.input(2,1,b"DisableCollision",playsrc_entity::Variant::Void).unwrap();
    let resolve=|identity|map.entity_collision_state(identity);
    assert!(retain_collision_snapshot(&opened,&templates,4,Some(&resolve),&transforms,&velocities).is_none());
    let disabled=compile_collision_snapshot(Some(&opened),&world,&templates,4,None,Some(&resolve),&transforms,&velocities).unwrap();
    assert!(!disabled.records()[0].enabled,"checkpoint sign collision inputs reach collision, not just rendering");
    map.input(3,1,b"EnableCollision",playsrc_entity::Variant::Void).unwrap();
    let resolve=|identity|map.entity_collision_state(identity);
    let enabled=compile_collision_snapshot(Some(&disabled),&world,&templates,5,None,Some(&resolve),&transforms,&velocities).unwrap();
    assert!(enabled.records()[0].enabled);
    let removed = |_| None;
    assert!(retain_collision_snapshot(&opened, &templates, 4, Some(&removed), &transforms, &velocities).is_none());
    let removed = compile_collision_snapshot(Some(&opened), &world, &templates, 4, None, Some(&removed), &transforms, &velocities).unwrap();
    assert!(!removed.records()[0].enabled);
}

fn authored_entity_model(entity: &playsrc_entity::Entity) -> Result<Option<String>, ()> {
    if entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(b"prop_dynamic"))
    {
        return entity
            .model
            .as_deref()
            .map(|value| {
                std::str::from_utf8(value)
                    .map(str::to_ascii_lowercase)
                    .map_err(|_| ())
            })
            .transpose();
    }
    let Some(definition) = entity
        .classname
        .as_deref()
        .and_then(playsrc_tf2::pickup::map_pickup_definition)
    else {
        return Ok(None);
    };
    let selected = entity_scalar(entity, b"powerup_model")
        .filter(|value| !value.is_empty())
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| ())?
        .unwrap_or(definition.model);
    Ok(Some(selected.to_ascii_lowercase()))
}

fn resolve_models(
    graph: Option<&playsrc_entity::Graph>,
    studio_models: &BTreeMap<String, Arc<RetainedPresentationModel>>,
    bundle: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    profile: playsrc_map::LightingProfile,
    static_props: Option<&playsrc_map::StaticProps>,
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
    let mut output_skins = BTreeMap::<Vec<u8>, Vec<i32>>::new();
    for connection in graph.into_iter().flat_map(|graph| &graph.entities).flat_map(|entity| &entity.connections) {
        if let playsrc_entity::Connection::Parsed { target, input, parameter, .. } = connection {
            if input.eq_ignore_ascii_case(b"Skin") { output_skins.entry(target.to_ascii_lowercase()).or_default().push(playsrc_keyvalues::NumericValue::Bytes(parameter).get_int()); }
        }
    }
    let mut entity_skins = BTreeMap::<String, std::collections::BTreeSet<usize>>::new();
    for entity in graph.into_iter().flat_map(|graph| &graph.entities) {
        if let Some(identity) = authored_entity_model(entity)? {
            let count = studio_models.get(&identity).ok_or(())?.skins.len();
            let family = entity_skins.entry(identity).or_default();
            let skin = entity_scalar(entity, b"skin").map_or(0, |value| playsrc_keyvalues::NumericValue::Bytes(value).get_int());
            family.insert(playsrc_studio_model::source_skin_family(skin, count));
            if let Some(skins) = entity.targetname.as_ref().and_then(|name| output_skins.get(&name.to_ascii_lowercase())) {
                family.extend(skins.iter().map(|skin| playsrc_studio_model::source_skin_family(*skin, count)));
            }
        }
    }
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
        let bodygroups = model.body_parts.iter().map(|_| 0).collect::<Vec<_>>();
        let pose_parameters = model
            .pose_parameters
            .iter()
            .map(|_| playsrc_studio_model::Float32(0))
            .collect::<Vec<_>>();
        let pose = playsrc_studio_model::sample_pose(
            model,
            &playsrc_studio_model::AnimationState {
                bone_rotations: Vec::new(),
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
        if let Some(families) = entity_skins.get(identity) { selected_skins.extend(families); }
        if graph.is_some_and(|graph| graph.entities.iter().any(|entity| entity.classname.as_deref() == Some(b"team_control_point")
            && [b"team_model_0".as_slice(),b"team_model_2",b"team_model_3"].into_iter().any(|key| entity_scalar(entity,key).is_some_and(|name| name.eq_ignore_ascii_case(identity.as_bytes()))))) {
            selected_skins.insert(playsrc_studio_model::source_skin_family(2, model.skins.len()));
        }
        for flag in graph.into_iter().flat_map(|graph| &graph.entities) {
            if !flag
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"item_teamflag"))
            {
                continue;
            }
            let selected = entity_scalar(flag, b"flag_model")
                .filter(|value| !value.is_empty())
                .unwrap_or(playsrc_tf2::ctf::FLAG_MODEL.as_bytes());
            if !selected.eq_ignore_ascii_case(identity.as_bytes()) {
                continue;
            }
            let team = match entity_scalar(flag, b"TeamNum") {
                Some(b"2") => 0,
                Some(b"3") => 1,
                _ => continue,
            };
            selected_skins.insert(team + 3);
        }
        for (static_props, prop) in static_props.into_iter().flat_map(|props| props.occurrences.iter().map(move |prop| (props, prop))) {
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
        let mut bodies = std::collections::BTreeSet::from([0]);
        if graph.is_some_and(|graph| graph.entities.iter().any(|e| e.classname.as_deref() == Some(b"team_control_point") && [b"team_model_0".as_slice(),b"team_model_2",b"team_model_3"].into_iter().any(|key| entity_scalar(e,key).is_some_and(|name| name.eq_ignore_ascii_case(identity.as_bytes()))))) {
            if let Some(part) = model.body_parts.first() {
                for team in [2,3] { if team < part.model_names.len() { bodies.insert(part.base * team as i32); } }
            }
        }
        for skin_index in selected_skins {
            if skin_index >= model.skins.len() {
                presentation_failure(&format!("model skin {identity}:{skin_index}/{}", model.skins.len()));
                return Err(());
            }
            for body in &bodies {
            let bodygroups = if *body == 0 { bodygroups.clone() } else { model.body_parts.iter().map(|p| ((*body / p.base) as usize) % p.model_names.len()).collect() };
            let mut primitives = Vec::new();
            for selected in
                playsrc_studio_model::select_primitives(model, &bodygroups, skin_index, 0)
                    .map_err(|_| ())?
            {
                let primitive = model.geometry.get(selected.primitive).ok_or(())?;
                let (positions, normals, _) = posed_vertices(model, primitive, &pose)?;
                let bone_palette = model.primitive_palette(selected.primitive)?
                    .iter()
                    .map(u16::from)
                    .collect();
                primitives.push(playsrc_map::RuntimeModelPrimitive {
                    material: selected.material,
                    positions,
                    normals,
                    bind_positions: primitive
                        .vertices
                        .iter()
                        .map(|vertex| vertex.position.0.map(|value| f32::from_bits(value.0)))
                        .collect(),
                    bind_normals: primitive
                        .vertices
                        .iter()
                        .map(|vertex| vertex.normal.0.map(|value| f32::from_bits(value.0)))
                        .collect(),
                    bind_tangents: primitive
                        .vertices
                        .iter()
                        .map(|vertex| vertex.tangent.map(|value| f32::from_bits(value.0)))
                        .collect(),
                    bone_indices: primitive
                        .vertices
                        .iter()
                        .map(|vertex| {
                            let mut indices = [0_u16; 4];
                            for influence in 0..usize::from(vertex.bone_count) {
                                indices[influence] = u16::from(vertex.bones[influence]);
                            }
                            indices
                        })
                        .collect(),
                    bone_weights: primitive
                        .vertices
                        .iter()
                        .map(|vertex| {
                            let mut weights = [0.0_f32; 4];
                            for influence in 0..usize::from(vertex.bone_count) {
                                weights[influence] = f32::from_bits(vertex.weights[influence].0);
                            }
                            weights
                        })
                        .collect(),
                    bone_palette,
                    uv: primitive
                        .vertices
                        .iter()
                        .map(|vertex| vertex.uv.map(|value| f32::from_bits(value.0)))
                        .collect(),
                    triangles: primitive.triangles.clone(),
                });
            }
            let logical_path = format!("{identity}{}{}", if *body == 0 { String::new() } else { format!("#body={body}") }, if skin_index == 0 { String::new() } else { format!("#skin={skin_index}") });
            indexes.insert(logical_path.clone(), models.len());
            models.push(playsrc_map::RuntimeModel {
                logical_path,
                materials: materials.clone(),
                primitives,
            });
            }
        }
    }
    let mut occurrences = Vec::new();
    for entity in graph.into_iter().flat_map(|graph| &graph.entities) {
        let Some(identity) = authored_entity_model(entity)? else {
            continue;
        };
        let skin = entity_scalar(entity, b"skin").map_or(0, |value| playsrc_keyvalues::NumericValue::Bytes(value).get_int());
        let skin = playsrc_studio_model::source_skin_family(skin, studio_models.get(&identity).ok_or(())?.skins.len());
        let model_key = if skin == 0 { identity } else { format!("{identity}#skin={skin}") };
        occurrences.push(playsrc_map::RuntimeModelOccurrence {
            entity: entity.index,
            model: *indexes.get(&model_key).ok_or(())?,
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
    resource_hashes: &BTreeMap<String, [u8; 32]>,
    profile: playsrc_map::LightingProfile,
    presentation_profile: playsrc_studio_model::PresentationProfile,
) -> Result<Box<CompiledPresentationModel>, ()> {
    let key = (
        identity.to_owned(),
        u8::from(profile == playsrc_map::LightingProfile::Hdr),
        match presentation_profile {
            playsrc_studio_model::PresentationProfile::World => 0,
            playsrc_studio_model::PresentationProfile::ViewModel => 1,
        },
    );
    {
        let mut cache = presentation_model_cache()
            .lock()
            .expect("presentation model cache");
        if let Some(retained) = cache.get(&key) {
            if let Some(model) = retained.model.upgrade() {
                let valid = model.dependencies.iter().all(|dependency| {
                    match (
                        dependency.sha256,
                        bundle.get(dependency.logical_path.as_str()),
                        resource_hashes.get(dependency.logical_path.as_str()),
                    ) {
                        (Some(expected), Some(bytes), Some(actual)) => {
                            expected == *actual && dependency.byte_length == bytes.len()
                        }
                        (None, None, None) => dependency.byte_length == 0,
                        _ => false,
                    }
                });
                if valid {
                    PRESENTATION_MODEL_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                    return Ok(Box::new(CompiledPresentationModel {
                        flex: Arc::clone(&retained.flex),
                        model,
                        identity: retained.identity,
                        illumination_position: retained.illumination_position,
                        illumination_attachment: retained.illumination_attachment,
                        eyes: retained.eyes.clone(),
                    }));
                }
            }
            cache.remove(&key);
        }
    }
    PRESENTATION_MODEL_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let built = playsrc_tf2::presentation::build_model(
        identity,
        bundle,
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
    let model = Arc::new(RetainedPresentationModel::new(*model));
    let identity = digest.finalize().into();
    presentation_model_cache()
        .lock()
        .expect("presentation model cache")
        .insert(
            key,
            CachedPresentationModel {
                flex: Arc::clone(&built.flex),
                model: Arc::downgrade(&model),
                identity,
                illumination_position: built.illumination_position,
                illumination_attachment: built.illumination_attachment,
                eyes: built.eyes.clone(),
            },
        );
    Ok(Box::new(CompiledPresentationModel {
        flex: built.flex,
        model,
        identity,
        illumination_position: built.illumination_position,
        illumination_attachment: built.illumination_attachment,
        eyes: built.eyes,
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
    model_lighting: BTreeMap<usize, CompiledModelOccurrenceLighting>,
}

struct CompiledModelOccurrenceLighting {
    lighting: playsrc_map::ModelWorldLighting,
    eyes: Vec<(usize, playsrc_studio_model::EyeDrawState)>,
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
    let snapshot = if canonical.static_props.occurrences.is_empty() {
        playsrc_collision::Snapshot::compile(
            collision,
            1,
            Vec::new(),
            playsrc_collision::SnapshotLimits::default(),
        )
        .map_err(|_| ())?
    } else {
        let checksums = model_indexes
            .iter()
            .map(|(identity, (_, checksum))| ((*identity).to_owned(), *checksum))
            .collect();
        let templates = collision_object_templates(canonical, graph, bundle, &checksums)
            .inspect_err(|_| presentation_failure("static prop collision templates"))?;
        compile_collision_snapshot(
            None,
            collision,
            &templates,
            1,
            None,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .map_err(|_| ())?
    };
    let mut model_lighting_world = playsrc_map::ModelLightingWorld::borrowed(&canonical.lighting);
    let mut model_lighting = BTreeMap::new();
    for entity in &graph.entities {
        let Some(identity) = authored_entity_model(entity)? else {
            continue;
        };
        let (model_index, _) = *model_indexes.get(identity.as_str()).ok_or(())?;
        let model = &models.get(model_index as usize).ok_or(())?.1;
        if model.illumination_attachment != 0 {
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("entity {} model {identity} requires lighting attachment {}", entity.index, model.illumination_attachment);
            return Err(());
        }
        let vector = |values: [f32; 3]| {
            playsrc_studio_model::Vector3(
                values.map(|value| playsrc_studio_model::Float32(value.to_bits())),
            )
        };
        let transform = playsrc_studio_model::source_entity_transform(
            vector(entity_vector(entity, b"origin")?),
            vector(entity_vector(entity, b"angles")?),
        )
        .map_err(|_| ())?;
        let origin = playsrc_studio_model::source_model_lighting_origin(
            model.illumination_position,
            model.illumination_attachment,
            transform,
            None,
            &identity,
        )
        .map_err(|_| ())?
        .0
        .map(|value| f32::from_bits(value.0));
        let mut lighting = model_lighting_world.sample(origin, visibility, collision, &snapshot).inspect_err(|_| {
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("entity {} model {identity} lighting sample {:?} failed", entity.index, origin);
        })?;
        playsrc_map::apply_model_ambient_boost(
            &mut lighting.ambient_cube,
            &lighting.local_lights,
            lighting.origin,
            model.model.flags,
        );
        let eyes = if model.eyes.is_empty() {
            Vec::new()
        } else {
            let bodygroups = model.model.body_parts.iter().map(|_| 0).collect::<Vec<_>>();
            let pose_parameters = model
                .model
                .pose_parameters
                .iter()
                .map(|_| playsrc_studio_model::Float32(0))
                .collect();
            let pose = playsrc_studio_model::sample_pose(
                &model.model,
                &playsrc_studio_model::AnimationState {
                    bone_rotations: Vec::new(),
                    base_sequence: 0,
                    cycle: playsrc_studio_model::Float32(0),
                    pose_parameters,
                    layers: Vec::new(),
                },
            )
            .map_err(|_| ())?;
            let skin = entity_scalar(entity, b"skin")
                .map(|value| {
                    std::str::from_utf8(value)
                        .map_err(|_| ())?
                        .parse::<i32>()
                        .map_err(|_| ())
                })
                .transpose()?
                .unwrap_or(0);
            let selected = playsrc_studio_model::select_primitives(
                &model.model,
                &bodygroups,
                playsrc_studio_model::source_skin_family(skin, model.model.skins.len()),
                0,
            )
            .map_err(|_| ())?;
            let angles = entity_vector(entity, b"angles")?;
            let eye_request = ModelPoseLightingRequest {
                origin: entity_vector(entity, b"origin")?,
                angles,
                camera: origin,
                camera_angles: angles,
            };
            source_model_eye_states(
                &model.model,
                &model.eyes,
                &pose,
                &selected,
                transform,
                source_tf2_eye_target(entity.index as u32, eye_request, None),
                angles,
                true,
            )?
        };
        if model_lighting
            .insert(
                entity.index,
                CompiledModelOccurrenceLighting { lighting, eyes },
            )
            .is_some()
        {
            return Err(());
        }
    }
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
            model_lighting,
        });
    }
    let aggregate = static_prop_artifact::decode_aggregate(
        bundle
            .get(static_prop_artifact::AGGREGATE_PATH)
            .copied()
            .ok_or(())?,
    ).inspect_err(|_| presentation_failure("static prop aggregate"))?;
    let object_indexes = static_prop_artifact::object_indexes(&aggregate).inspect_err(|_| presentation_failure("static prop object index"))?;
    let water_materials = environment
        .world
        .water
        .surfaces
        .iter()
        .map(|surface| surface.material)
        .collect();
    let surface_world =
        playsrc_map::SurfaceLightingWorld::compile(canonical, visibility, water_materials)
            .map_err(|_| ()).inspect_err(|_| presentation_failure("static prop surface lighting world"))?;
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
    for prop in &runtime_props {
        let origin = lighting_origin(prop).inspect_err(|_| presentation_failure("static prop lighting origin"))?;
        let origin_leaf = visibility.locate_leaf(origin).map_err(|_| ())?;
        let leaf = visibility.leaves.get(origin_leaf).ok_or(())?;
        let origin_cluster = leaf.cluster;
        for (light_index, light) in canonical.lighting.world_lights.iter().enumerate() {
            if light.kind == 5
                || (light.kind == 3 && (leaf.area_and_flags >> 9) & 0x05 == 0)
                || (light.kind != 3
                    && (origin_cluster < 0 || light.cluster < 0
                        || !visibility.visible(origin_cluster as usize, light.cluster as usize)))
            {
                continue;
            }
            let (end, direction, ratio) = playsrc_map::source_world_light_ray(origin, light)?;
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
                    static_prop_collision_identity(prop.source)?,
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
        let mut ambient_cube = surface_world
            .ambient_cube(
                origin,
                &static_prop_artifact::SOURCE_AMBIENT_DIRECTIONS,
                |_| false,
            )
            .map_err(|error| {
                #[cfg(not(target_arch = "wasm32"))]
                eprintln!("static prop {} ambient cube at {origin:?}: {error:?}", prop.source);
                #[cfg(target_arch = "wasm32")]
                let _ = error;
            })?;
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
            let angle = playsrc_map::source_world_light_angle(light, *direction)?;
            let illumination = ratio * dot(light.intensity, [0.299, 0.587, 0.114]);
            let record = RuntimeLight {
                source: u32::try_from(*light_index).map_err(|_| ())?,
                kind: light.kind,
                style: light.style,
                ratio: *ratio,
                direction: *direction,
                intensity: light.intensity,
                origin: light.origin,
                normal: light.normal,
                stop_dot: light.stop_dot,
                stop_dot2: light.stop_dot2,
                exponent: light.exponent,
                radius: light.radius,
                attenuation: [
                    light.constant_attenuation,
                    light.linear_attenuation,
                    light.quadratic_attenuation,
                ],
            };
            if light.kind != 0 && illumination < 0.0002 {
                playsrc_map::add_world_light_to_cube(
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
                playsrc_map::add_world_light_to_cube(
                    &mut ambient_cube,
                    demoted.direction,
                    demoted.intensity,
                    demoted.ratio
                        * playsrc_map::source_world_light_angle(demoted_source, demoted.direction)?,
                )?;
            } else {
                playsrc_map::add_world_light_to_cube(
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
        let ownership = static_prop_artifact::classify_ownership(&areas, sky_area).inspect_err(|_| {
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("static prop {} has ambiguous sky ownership: {areas:?} sky={sky_area:?}", prop.source);
        })?;
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
        model_lighting,
    })
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
    for reference in [b"decals/decals_mod2x".as_slice(), b"VGUI/damageindicator"] {
        let path = dependency_path(&reference)?;
        insert_material_state_target(&mut targets, path.clone(), path, false)?;
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
    let legacy=legacy_materials::compile(graph,bundle,decoders,profile)?;
    for asset in &legacy.materials {
        // CEngineSprite creates separate render-mode materials from one VMT.
        // Their explicit compiled states are distinct from ordinary path aliases.
        if let Some(existing)=targets.get(&asset.identity) {
            if existing!=&(asset.source.clone(),false) {return Err(());}
        } else {targets.insert(asset.identity.clone(),(asset.source.clone(),false));}
    }
    let start = out.len();
    out.extend_from_slice(b"PMST");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(targets.len()).map_err(|_| ())?.to_le_bytes());
    for (identity, (source, model)) in targets {
        if let Some(asset)=legacy.materials.iter().find(|asset|asset.identity==identity) {
            encode_resolved_material_state(out,&identity,&asset.state,Some(decoders.metadata(&asset.texture)?))?;
        } else if let Some(particle) = particle_materials.get(&identity) {
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
        playsrc_vtf::ImageFormat::Rgb888
            | playsrc_vtf::ImageFormat::Bgr888
            | playsrc_vtf::ImageFormat::Dxt1
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
                    .map_err(|_| ()).inspect_err(|_| presentation_failure(&format!("model material semantics {identity}")))?;
            if material.model.is_none() { presentation_failure(&format!("model shader state {identity}")); }
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
                model_authored_texture(&path, decoders, resource_hashes, true).map_err(|_| ()).inspect_err(|_| presentation_failure(&format!("model texture {path}")))?,
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
    material_section.extend_from_slice(&4_u32.to_le_bytes());
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
    let mut cloak_proxy = 0;
    let mut player_tint = false;
    for proxy in &material.proxies {
        if proxy.name.eq_ignore_ascii_case(b"invis") {
            cloak_proxy = 1;
        } else if proxy.name.eq_ignore_ascii_case(b"weapon_invis") {
            cloak_proxy = 2;
        } else if proxy.name.eq_ignore_ascii_case(b"vm_invis") {
            cloak_proxy = 3;
        } else if proxy.name.eq_ignore_ascii_case(b"spy_invis") {
            cloak_proxy = 2;
            player_tint = true;
        }
    }
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
            playsrc_material::ModelShader::Refract => 6,
            playsrc_material::ModelShader::UnlitGeneric => 3,
            playsrc_material::ModelShader::UnlitTwoTexture => 4,
            playsrc_material::ModelShader::Modulate => 5,
            playsrc_material::ModelShader::VertexLitGeneric => 0,
            playsrc_material::ModelShader::EyeRefract => 1,
            playsrc_material::ModelShader::Eyes => 2,
        },
        cloak_proxy | (u8::from(player_tint) << 2),
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
        State::Refract => {
            let state = playsrc_material::refract_material_output(material).map_err(|_| ())?.ok_or(())?;
            out.extend_from_slice(&state.normal_frame.to_le_bytes());
            for value in state.normal_transform.into_iter().chain([state.refract_amount]).chain(state.refract_tint) { out.extend_from_slice(&value.to_le_bytes()); }
            out.extend_from_slice(&[state.blur_amount, u8::from(state.ignore_depth), 0, 0]);
        }
        State::UnlitGeneric(state) => {
            for value in state.color_modulation { out.extend_from_slice(&value.to_le_bytes()); }
        }
        State::Modulate => {}
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

/// Named attachment consumers follow Studio_FindAttachment's first match;
/// the StudioModel and sampled pose retain every authored indexed attachment.
fn named_attachments<T>(items: &[T], name: fn(&T) -> &[u8]) -> impl Iterator<Item = &T> + Clone {
    items.iter().enumerate().filter(move |(index, value)|
        !items[..*index].iter().any(|prior| name(prior).eq_ignore_ascii_case(name(value))))
        .map(|(_, value)| value)
}

fn encode_particle_textures(
    out: &mut Vec<u8>,
    materials: &BTreeMap<String, CompiledParticlePresentation>,
    roots: &[playsrc_particle::DefinitionLookup<'_>],
) -> Result<(), ()> {
    out.extend_from_slice(b"PPTM");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, material) in materials {
        pbytes(out, identity.as_bytes())?;
        pbytes(out, material.source_path.as_bytes())?;
        encode_model_authored_texture(out, &material.texture_path, &material.texture)?;
        out.extend_from_slice(&u32::from(material.sprite_card.is_some()).to_le_bytes());
        if let Some(state) = &material.sprite_card {
            out.extend_from_slice(&u32::from(state.depth_blend.as_ref().is_some_and(|v| v.value)).to_le_bytes());
            out.extend_from_slice(&u32::from(state.blend_frames.value).to_le_bytes());
            for value in [state.add_self.value, state.overbright_factor.value, state.depth_blend_scale.value,
                state.minimum_size.value, state.start_fade_size.value, state.end_fade_size.value, state.maximum_size.value,
                 state.maximum_distance.value, state.far_fade_interval.value] { out.extend_from_slice(&value.to_le_bytes()); }
        }
        out.extend_from_slice(&u32::from(material.additive_sprite.is_some()).to_le_bytes());
        if let Some(state) = &material.additive_sprite {
            out.extend_from_slice(&u32::from(state.srgb).to_le_bytes());
            out.extend_from_slice(&u32::from(state.vertex_color).to_le_bytes());
            for value in state.color.into_iter().chain([state.hdr_color_scale]) { out.extend_from_slice(&value.to_le_bytes()); }
        }
    }
    out.extend_from_slice(&u32::try_from(roots.len()).map_err(|_| ())?.to_le_bytes());
    for root in roots {
        let playsrc_particle::DefinitionLookup::Name(name) = root else { return Err(()); };
        pbytes(out, name.as_bytes())?;
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

fn encode_audio_documents(out: &mut Vec<u8>, bundle: &BTreeMap<String, &[u8]>, graph: &playsrc_entity::Graph) -> Result<(), ()> {
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
        "Weapon_SyringeGun.Single",
        "Weapon_SyringeGun.WorldReload",
        "Weapon_BoneSaw.Miss",
        "Weapon_BoneSaw.HitFlesh",
        "Weapon_BoneSaw.HitWorld",
        "WeaponMedigun.HealingHealer",
        "WeaponMedigun.HealingDetachHealer",
        "WeaponMedigun.Charged",
        "Player.HitSoundDefaultDing",
        "Player.KillSoundDefaultDing",
    ];
    let flag_targets: Vec<&str> = playsrc_tf2::audio::FLAG_SOUNDS.iter().map(|definition| definition.identity()).collect();
    let item_targets: &[&str] = &[
        "HealthKit.Touch",
        "AmmoPack.Touch",
        "Regenerate.Touch",
        "Item.Materialize",
    ];
    let item_and_round_targets: &[&str] = &[
        "HealthKit.Touch",
        "AmmoPack.Touch",
        "Regenerate.Touch",
        "Item.Materialize",
        "Game.YourTeamWon",
        "Game.YourTeamLost",
    ];
    let player_targets: &[&str] = &["Player.Spy_Cloak", "Player.Spy_UnCloak", "TFPlayer.CritHit"];
    let impact_targets: &[&str] = &[
        "Default.BulletImpact",
        "Concrete.BulletImpact",
        "Wood.BulletImpact",
        "SolidMetal.BulletImpact",
        "Dirt.BulletImpact",
        "Sand.BulletImpact",
        "Glass.BulletImpact",
        "Flesh.BulletImpact",
    ];
    let mut documents = vec![
        ("scripts/game_sounds_weapons.txt", weapon_targets),
        ("scripts/game_sounds_player.txt", player_targets),
        ("scripts/game_sounds_physics.txt", impact_targets),
    ];
    let has_class = |name: &[u8]| graph.entities.iter().any(|entity| entity.classname.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(name)));
    let flags = has_class(b"item_teamflag");
    let control_points = has_class(b"team_control_point_master");
    let koth = has_class(b"tf_logic_koth");
    let timer_audio = koth || graph.entities.iter().any(|entity| entity.classname.as_deref().is_some_and(|value| value.eq_ignore_ascii_case(b"team_round_timer")) && entity_scalar(entity, b"show_in_hud") == Some(b"1"));
    let mut voice_targets = Vec::new();
    if flags { voice_targets.extend_from_slice(&flag_targets); }
    if timer_audio { voice_targets.extend_from_slice(playsrc_tf2::audio::TIMER_VOICE_SOUNDS); }
    if control_points { voice_targets.extend(playsrc_tf2::control_point::VOICE_SOUNDS.iter().map(|definition| definition.identity())); }
    if has_class(b"team_train_watcher") { voice_targets.extend_from_slice(playsrc_tf2::payload::VOICE_SOUNDS); }
    if !voice_targets.is_empty() { documents.push(("scripts/game_sounds_vo.txt", &voice_targets)); }
    let mut round_targets = if flags || timer_audio { item_and_round_targets.to_vec() } else { item_targets.to_vec() };
    if timer_audio { round_targets.extend_from_slice(playsrc_tf2::audio::TIMER_GENERAL_SOUNDS); }
    if control_points { round_targets.extend(playsrc_tf2::control_point::GENERAL_SOUNDS.iter().map(|definition| definition.identity())); }
    if has_class(b"team_train_watcher") { round_targets.extend_from_slice(playsrc_tf2::payload::GENERAL_SOUNDS); }
    documents.push(("scripts/game_sounds.txt", &round_targets));
    for (_, path, _) in playsrc_tf2::audio::CONFIGURED_SOUNDS {
        if !documents.iter().any(|(current, _)| current == path) { documents.push((path, &[])); }
    }
    let wanted: std::collections::BTreeSet<&str> = documents.iter().flat_map(|(_, targets)| targets.iter().copied())
        .chain(playsrc_tf2::SoundDefinition::NATIVE.iter().filter(|definition| !definition.map_scoped()).map(|definition| definition.identity()))
        .chain(playsrc_tf2::audio::CONFIGURED_SOUNDS.iter().map(|(name, _, _)| *name)).collect();
    for (path, source) in bundle {
        if !path.starts_with("scripts/game_sounds") || !path.ends_with(".txt") || path.ends_with("_manifest.txt") || documents.iter().any(|(current, _)| *current == path) { continue; }
        let document = playsrc_keyvalues::parse_text(source, playsrc_keyvalues::EscapeMode::LiteralBackslash, playsrc_keyvalues::Limits::default()).map_err(|_| ())?;
        if document.roots.iter().any(|node| wanted.iter().any(|name| node.key.bytes.eq_ignore_ascii_case(name.as_bytes()))) {
            documents.push((path.as_str(), &[]));
        }
    }
    let manifest = playsrc_keyvalues::parse_text(bundle.get("scripts/game_sounds_manifest.txt").ok_or(())?, playsrc_keyvalues::EscapeMode::LiteralBackslash, playsrc_keyvalues::Limits::default()).map_err(|_| ())?
        .evaluated(&playsrc_keyvalues::ConditionEnvironment::new([(b"$WIN32".to_vec(), true), (b"$X360".to_vec(), false)]));
    let Some(playsrc_keyvalues::Value::Object(files)) = manifest.roots.first().map(|node| &node.value) else { return Err(()); };
    let order: Vec<_> = files.iter().filter_map(|node| match &node.value {
        playsrc_keyvalues::Value::Scalar(value) if node.key.bytes.eq_ignore_ascii_case(b"precache_file") || node.key.bytes.eq_ignore_ascii_case(b"preload_file") => std::str::from_utf8(&value.token.bytes).ok(), _ => None,
    }).collect();
    documents.sort_by_key(|(path, _)| order.iter().position(|file| file.eq_ignore_ascii_case(path)).unwrap_or(usize::MAX));
    let mixer = *bundle.get("scripts/soundmixers.txt").ok_or(())?;
    out.extend_from_slice(b"PAUD");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&<[u8; 32]>::from(Sha256::digest(mixer)));
    out.extend_from_slice(&0.72f32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(documents.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    let mut emitted = std::collections::BTreeSet::new();
    for (logical_path, _) in documents {
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
        let targets: Vec<_> = wanted.iter().copied().filter(|name| document.roots.iter().any(|node| node.key.bytes.eq_ignore_ascii_case(name.as_bytes())) && emitted.insert(name.to_ascii_lowercase())).collect();
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
    if wanted.iter().any(|name| !emitted.contains(&name.to_ascii_lowercase())) { return Err(()); }
    // Sound patches retain each admitted WAV's authored cue, including equipped
    // minigun loops. No caller-side filename list owns loop behavior.
    let patches: Vec<_> = bundle.keys().map(String::as_str).filter(|path| path.starts_with("sound/") && path.ends_with(".wav")).collect();
    out.extend_from_slice(&(patches.len() as u32).to_le_bytes());
    for path in patches {
        let metadata = playsrc_wav::pcm_metadata(bundle.get(path).ok_or(())?).map_err(|_| ())?;
        pbytes(out, path.as_bytes())?;
        out.extend_from_slice(&metadata.sample_rate.to_le_bytes());
        out.extend_from_slice(&metadata.frames.to_le_bytes());
        out.extend_from_slice(&metadata.cue_frame.unwrap_or(u32::MAX).to_le_bytes());
    }
    let absences = sound_precache_absences(bundle)?;
    out.extend_from_slice(&(absences.len() as u32).to_le_bytes());
    for path in absences { pbytes(out, path.as_bytes())?; }
    Ok(())
}

fn sound_precache_absences(bundle: &BTreeMap<String, &[u8]>) -> Result<Vec<String>, ()> {
    let bytes = *bundle.get(playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_PATH).ok_or(())?;
    if bytes.len() > 128 * 1024 || !bytes.ends_with(b"\n") { return Err(()); }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?.strip_prefix(playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_HEADER).ok_or(())?;
    let paths = text.split_terminator('\n').map(str::to_owned).collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    if paths.len() > 128 || paths.iter().any(|path| !path.starts_with("sound/") || path != &path.to_ascii_lowercase()
        || path.contains('\\') || path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || bundle.contains_key(path) || !seen.insert(path)) { return Err(()); }
    Ok(paths)
}

#[cfg(test)]
#[test]
fn payload_timer_audio_includes_setup_and_checkpoint_contracts() {
    let scripts = playsrc_tf2::SoundDefinition::NATIVE.iter().map(|sound| sound.identity())
        .chain(playsrc_tf2::audio::CONFIGURED_SOUNDS.iter().map(|(name, _, _)| *name))
        .chain(["Announcer.TimeAddedForEnemy", "Announcer.TimeAwardedForTeam"])
        .chain(["TFPlayer.CritHit", "Player.HitSoundDefaultDing", "Player.KillSoundDefaultDing"])
        .chain(["Default.BulletImpact", "Concrete.BulletImpact", "Wood.BulletImpact", "SolidMetal.BulletImpact", "Dirt.BulletImpact", "Sand.BulletImpact", "Glass.BulletImpact", "Flesh.BulletImpact"])
        .map(|name| format!("\"{name}\" {{ \"wave\" \"fixture.wav\" }}\n")).collect::<String>();
    let mut bundle = BTreeMap::new();
    for path in ["scripts/game_sounds_weapons.txt", "scripts/game_sounds_player.txt", "scripts/game_sounds_physics.txt", "scripts/game_sounds.txt", "scripts/game_sounds_vo.txt"]
        .into_iter().chain(playsrc_tf2::audio::CONFIGURED_SOUNDS.iter().map(|(_, path, _)| *path)) {
        bundle.insert(path.to_owned(), b"\"unused\" {}".as_slice());
    }
    bundle.insert("scripts/game_sounds_weapons.txt".to_owned(), scripts.as_bytes());
    bundle.insert("scripts/game_sounds_manifest.txt".to_owned(), b"\"game_sounds_manifest\" {}".as_slice());
    bundle.insert("scripts/soundmixers.txt".to_owned(), b"".as_slice());
    bundle.insert(playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_PATH.to_owned(), playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_HEADER.as_bytes());
    for timer in [false, true] {
        let entities = if timer { br#"{"classname" "team_control_point_master"} {"classname" "team_train_watcher"} {"classname" "team_round_timer" "show_in_hud" "1"}"#.as_slice() } else { b"".as_slice() };
        let graph = playsrc_entity::parse(entities, playsrc_entity::Limits::default()).unwrap();
        let mut bytes = Vec::new();
        encode_audio_documents(&mut bytes, &bundle, &graph).unwrap();
        for name in ["Announcer.RoundBegins60Seconds", "Announcer.RoundBegins1Seconds", "Announcer.RoundEnds60seconds", "Ambient.Siren"] {
            assert_eq!(bytes.windows(name.len()).any(|value| value == name.as_bytes()), timer, "{name}");
        }
        assert_eq!(bytes.windows(b"Hud.PointCaptured".len()).any(|value| value == b"Hud.PointCaptured"), timer);
    }
}

#[cfg(test)]
#[test]
fn precache_absence_requires_an_explicit_noncontradictory_source_witness() {
    let identity = playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_PATH.to_owned();
    let mut bundle = BTreeMap::new();
    assert!(sound_precache_absences(&bundle).is_err());
    let witness = format!("{}sound/items/itembk2.wav\n", playsrc_tf2::audio::SOUND_PRECACHE_ABSENCES_HEADER);
    bundle.insert(identity, witness.as_bytes());
    assert_eq!(sound_precache_absences(&bundle).unwrap(), vec!["sound/items/itembk2.wav"]);
    bundle.insert("sound/items/itembk2.wav".to_owned(), b"unexpected".as_slice());
    assert!(sound_precache_absences(&bundle).is_err());
}

fn map_prop_pipeline_animations(graph: &playsrc_entity::Graph) -> BTreeMap<usize, Vec<&[u8]>> {
    let mut pipeline_animations = graph.entities.iter()
        .filter_map(|entity| playsrc_tf2::regenerate_associated_model(graph, entity))
        .map(|entity| (entity.index, vec![b"open".as_slice(), b"close".as_slice()])).collect::<BTreeMap<_, _>>();
    // Prepare door-motion outputs, not unrelated staging/UI relays. Other
    // animated props are selected by the authoritative runtime snapshot.
    // CEventQueue tries authored target names before classname fallback. An
    // unresolved activator or wildcard is not a guessed preparation target;
    // its selected animation still arrives through the runtime snapshot.
    for source in &graph.entities {
        if !source.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(b"func_door") || name.eq_ignore_ascii_case(b"func_door_rotating")) { continue; }
        for connection in &source.connections {
            let playsrc_entity::Connection::Parsed { target, input, parameter, .. } = connection else { continue };
            if !input.eq_ignore_ascii_case(b"SetAnimation") || parameter.is_empty() || target.contains(&b'*') { continue; }
            let targets = if target.eq_ignore_ascii_case(b"!self") || target.eq_ignore_ascii_case(b"!caller") {
                vec![source]
            } else if target.starts_with(b"!") {
                Vec::new()
            } else {
                let named = graph.entities.iter().filter(|entity| entity.targetname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(target))).collect::<Vec<_>>();
                if named.is_empty() {
                    graph.entities.iter().filter(|entity| entity.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(target))).collect()
                } else { named }
            };
            for entity in targets {
                if entity.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(b"prop_dynamic")) {
                    pipeline_animations.entry(entity.index).or_default().push(parameter.as_slice());
                }
            }
        }
    }
    pipeline_animations
}

#[cfg(test)]
mod map_prop_pipeline_tests {
    use super::*;

    #[test]
    fn dormant_authored_inputs_join_only_their_named_prop_and_resupply_association() {
        let graph = playsrc_entity::parse(br#"
          {"classname" "prop_dynamic" "targetname" "door"}
          {"classname" "prop_dynamic" "targetname" "unused"}
          {"classname" "prop_dynamic" "targetname" "locker"}
          {"classname" "func_door" "OnOpen" "DOOR,SetAnimation,open,0,-1" "OnClose" "door,SetAnimation,close,0,-1"}
          {"classname" "func_regenerate" "associatedmodel" "locker"}
        "#, playsrc_entity::Limits::default()).unwrap();
        let selected = map_prop_pipeline_animations(&graph);
        assert_eq!(selected.keys().copied().collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!(selected[&0], vec![b"open".as_slice(), b"close".as_slice()]);
        assert_eq!(selected[&2], vec![b"open".as_slice(), b"close".as_slice()]);
    }

    #[test]
    fn named_non_props_prevent_classname_fallback_and_dynamic_activators_are_not_guessed() {
        let graph = playsrc_entity::parse(br#"
          {"classname" "info_target" "targetname" "prop_dynamic"}
          {"classname" "prop_dynamic" "targetname" "door"}
          {"classname" "func_door" "OnOpen" "prop_dynamic,SetAnimation,open,0,-1" "OnUser1" "!activator,SetAnimation,close,0,-1"}
        "#, playsrc_entity::Limits::default()).unwrap();
        assert!(map_prop_pipeline_animations(&graph).is_empty());
    }

    #[test]
    fn unrelated_staging_relays_do_not_expand_door_preparation() {
        let graph = playsrc_entity::parse(br#"
          {"classname" "prop_dynamic" "targetname" "banner"}
          {"classname" "logic_relay" "OnTrigger" "banner,SetAnimation,intro,0,-1"}
        "#, playsrc_entity::Limits::default()).unwrap();
        assert!(map_prop_pipeline_animations(&graph).is_empty());
    }
}

fn encode_model_occurrence_matrices(
    out: &mut Vec<u8>,
    graph: &playsrc_entity::Graph,
    lighting: &BTreeMap<usize, CompiledModelOccurrenceLighting>,
    cubemaps: &[playsrc_map::CubemapSample],
    models: &[(String, Box<CompiledPresentationModel>)],
) -> Result<(), ()> {
    let occurrences = graph
        .entities
        .iter()
        .map(|entity| authored_entity_model(entity).map(|model| (entity, model)))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|(entity, model)| model.map(|model| (entity, model)))
        .collect::<Vec<_>>();
    let pipeline_animations = map_prop_pipeline_animations(graph);
    out.extend_from_slice(b"PMTX");
    out.extend_from_slice(&5u32.to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(occurrences.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (entity, identity) in occurrences {
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
        let animation = pipeline_animations.get(&entity.index)
            .and_then(|names| names.iter().copied().find(|name| models.iter().any(|(model, artifact)| model.eq_ignore_ascii_case(&identity)
                && artifact.model.sequences.iter().any(|sequence| sequence.label.eq_ignore_ascii_case(name)))))
            .unwrap_or_default();
        pbytes(out, animation)?;
        for value in entity_vector(entity, b"origin")?
            .into_iter()
            .chain(entity_vector(entity, b"angles")?)
        {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in matrix.0 {
            out.extend_from_slice(&value.0.to_le_bytes());
        }
        let state = lighting.get(&entity.index).ok_or(())?;
        encode_world_lighting(out, &state.lighting, state.lighting.origin, cubemaps)?;
        encode_eye_states(out, &state.eyes)?;
    }
    Ok(())
}
struct DecodedTexture {
    logical_path: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct ReferencedDirectionalTexture {
    logical_path: String,
    width: u32,
    height: u32,
    source_sha256: [u8; 32],
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
    BTreeMap<String, Arc<RetainedPresentationModel>>,
    BTreeMap<String, StudioModelLightingMetadata>,
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
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ())?) != 15
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
    let mut expected = std::collections::BTreeSet::from([
        "models/weapons/w_models/w_rocket.mdl".to_owned(),
        "models/weapons/w_models/w_rocket_airstrike/w_rocket_airstrike.mdl".to_owned(),
        "models/weapons/w_models/w_flaregun_shell.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/w_models/w_syringe_proj.mdl".to_owned(),
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
        "models/player/items/soldier/soldier_officer.mdl".to_owned(),
        "models/player/items/medic/medic_officer.mdl".to_owned(),
        "models/player/items/heavy/heavy_officer.mdl".to_owned(),
        "models/player/pyro.mdl".to_owned(),
        "models/player/spy.mdl".to_owned(),
        "models/player/engineer.mdl".to_owned(),
        "models/vgui/ui_class01.mdl".to_owned(),
        "models/class_menu/random_class_icon.mdl".to_owned(),
        "models/weapons/c_models/c_medic_arms.mdl".to_owned(),
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
        "models/weapons/c_models/c_engineer_arms.mdl".to_owned(),
        "models/weapons/c_models/c_wrench/c_wrench.mdl".to_owned(),
        "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl".to_owned(),
        "models/weapons/c_models/c_toolbox/c_toolbox.mdl".to_owned(),
        "models/buildables/sentry1_blueprint.mdl".to_owned(),
        "models/buildables/sentry1.mdl".to_owned(),
        "models/buildables/sentry1_heavy.mdl".to_owned(),
        "models/buildables/sentry2.mdl".to_owned(),
        "models/buildables/sentry2_heavy.mdl".to_owned(),
        "models/buildables/sentry3.mdl".to_owned(),
        "models/buildables/sentry3_heavy.mdl".to_owned(),
        "models/buildables/dispenser_blueprint.mdl".to_owned(),
        "models/buildables/dispenser.mdl".to_owned(),
        "models/buildables/dispenser_light.mdl".to_owned(),
        "models/buildables/dispenser_lvl2.mdl".to_owned(),
        "models/buildables/dispenser_lvl2_light.mdl".to_owned(),
        "models/buildables/dispenser_lvl3.mdl".to_owned(),
        "models/buildables/dispenser_lvl3_light.mdl".to_owned(),
        "models/buildables/teleporter_blueprint_enter.mdl".to_owned(),
        "models/buildables/teleporter_blueprint_exit.mdl".to_owned(),
        "models/buildables/teleporter.mdl".to_owned(),
        "models/buildables/teleporter_light.mdl".to_owned(),
        "models/weapons/c_models/c_syringegun/c_syringegun.mdl".to_owned(),
        "models/weapons/c_models/c_medigun/c_medigun.mdl".to_owned(),
        "models/weapons/c_models/c_bonesaw/c_bonesaw.mdl".to_owned(),
    ]);
    expected.extend(playsrc_tf2::equipment::stock_weapon_models().map(str::to_owned));
    let expected = graph
        .entities
        .iter()
        .try_fold(expected, |mut output, entity| {
            if let Some(model) = authored_entity_model(entity).map_err(|_| 3_u32)? {
                output.insert(model);
            } else if entity.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(b"team_control_point")) {
                output.extend(control_point_model_roots(entity).map_err(|_| 3_u32)?);
            } else if entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"item_teamflag"))
            {
                output.insert(
                    std::str::from_utf8(
                        entity_scalar(entity, b"flag_model")
                            .unwrap_or(b"models/flag/briefcase.mdl"),
                    )
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
    let metadata = models
        .iter()
        .map(|(identity, artifact)| {
            (
                identity.clone(),
                StudioModelLightingMetadata {
                    flex: Arc::clone(&artifact.flex),
                    position: artifact.illumination_position,
                    attachment: artifact.illumination_attachment,
                    eyes: artifact.eyes.clone(),
                },
            )
        })
        .collect();
    let models: BTreeMap<String, Arc<RetainedPresentationModel>> = models
        .into_iter()
        .map(|(identity, artifact)| (identity, artifact.model))
        .collect();
    let output = (
        Vec::new(),
        models,
        metadata,
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
    directional: &[(String, u8, ReferencedDirectionalTexture)],
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
        add(72 + identity.len() + texture.logical_path.len())?;
    }
    for identity in particle_materials {
        add(4 + identity.len())?;
    }
    add(4 + environment.len())?;
    add(12)?;
    for (identity, particle) in particles {
        add(60 + identity.len() + particle.source_path.len())?;
        add(encoded_model_authored_texture_length(&particle.texture_path, &particle.texture)?)?;
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

fn presentation_failure(stage: &str) {
    let mut detail = PRESENTATION_FAILURE_DETAIL.lock().unwrap();
    if detail.len() + stage.len() + 2 <= 4096 {
        if !detail.is_empty() { detail.push_str("; "); }
        detail.push_str(stage);
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("TF2 presentation failed: {stage}");
}

fn presentation_stage(stage: &str) {
    let mut detail = PRESENTATION_FAILURE_DETAIL.lock().unwrap();
    detail.clear();
    detail.push_str(stage);
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_presentation_failure_length() -> usize { PRESENTATION_FAILURE_DETAIL.lock().unwrap().len() }

#[unsafe(no_mangle)]
/// # Safety
/// The output must identify writable module memory for `capacity` bytes.
pub unsafe extern "C" fn playsrc_presentation_failure_copy(output: *mut u8, capacity: usize) -> usize {
    let detail = PRESENTATION_FAILURE_DETAIL.lock().unwrap();
    if output.is_null() || capacity < detail.len() { return 0; }
    unsafe { std::ptr::copy_nonoverlapping(detail.as_ptr(), output, detail.len()); }
    detail.len()
}

fn control_point_model_roots(entity: &playsrc_entity::Entity) -> Result<Vec<String>, ()> {
    [b"team_model_0".as_slice(), b"team_model_2", b"team_model_3"].into_iter()
        .filter_map(|key| entity_scalar(entity, key).filter(|name| !name.is_empty()))
        .map(|name| std::str::from_utf8(name).map(str::to_ascii_lowercase).map_err(|_| ())).collect()
}

fn compile_presentation(inputs: PresentationInputs<'_, '_>) -> Result<MeasuredPresentation, ()> {
    let PresentationInputs {
        canonical,
        bsp,
        graph,
        bundle,
        decoders,
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
        "models/weapons/w_models/w_rocket_airstrike/w_rocket_airstrike.mdl".to_owned(),
        "models/weapons/w_models/w_flaregun_shell.mdl".to_owned(),
        "models/weapons/w_models/w_stickybomb.mdl".to_owned(),
        "models/weapons/w_models/w_syringe_proj.mdl".to_owned(),
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
        "models/player/items/soldier/soldier_officer.mdl".to_owned(),
        "models/player/items/medic/medic_officer.mdl".to_owned(),
        "models/player/items/heavy/heavy_officer.mdl".to_owned(),
        "models/player/pyro.mdl".to_owned(),
        "models/player/spy.mdl".to_owned(),
        "models/player/engineer.mdl".to_owned(),
        "models/vgui/ui_class01.mdl".to_owned(),
        "models/class_menu/random_class_icon.mdl".to_owned(),
        "models/weapons/c_models/c_medic_arms.mdl".to_owned(),
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
        "models/weapons/c_models/c_engineer_arms.mdl".to_owned(),
        "models/weapons/c_models/c_wrench/c_wrench.mdl".to_owned(),
        "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl".to_owned(),
        "models/weapons/c_models/c_toolbox/c_toolbox.mdl".to_owned(),
        "models/buildables/sentry1_blueprint.mdl".to_owned(),
        "models/buildables/sentry1.mdl".to_owned(),
        "models/buildables/sentry1_heavy.mdl".to_owned(),
        "models/buildables/sentry2.mdl".to_owned(),
        "models/buildables/sentry2_heavy.mdl".to_owned(),
        "models/buildables/sentry3.mdl".to_owned(),
        "models/buildables/sentry3_heavy.mdl".to_owned(),
        "models/buildables/dispenser_blueprint.mdl".to_owned(),
        "models/buildables/dispenser.mdl".to_owned(),
        "models/buildables/dispenser_light.mdl".to_owned(),
        "models/buildables/dispenser_lvl2.mdl".to_owned(),
        "models/buildables/dispenser_lvl2_light.mdl".to_owned(),
        "models/buildables/dispenser_lvl3.mdl".to_owned(),
        "models/buildables/dispenser_lvl3_light.mdl".to_owned(),
        "models/buildables/teleporter_blueprint_enter.mdl".to_owned(),
        "models/buildables/teleporter_blueprint_exit.mdl".to_owned(),
        "models/buildables/teleporter.mdl".to_owned(),
        "models/buildables/teleporter_light.mdl".to_owned(),
        "models/weapons/c_models/c_syringegun/c_syringegun.mdl".to_owned(),
        "models/weapons/c_models/c_medigun/c_medigun.mdl".to_owned(),
        "models/weapons/c_models/c_bonesaw/c_bonesaw.mdl".to_owned(),
    ]);
    for entity in &graph.entities {
        if let Some(model) = authored_entity_model(entity)? {
            roots.insert(model);
        } else if entity.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(b"team_control_point")) {
            roots.extend(control_point_model_roots(entity)?);
        } else if entity
            .classname
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(b"item_teamflag"))
        {
            roots.insert(
                std::str::from_utf8(
                    entity_scalar(entity, b"flag_model").unwrap_or(b"models/flag/briefcase.mdl"),
                )
                .map_err(|_| ())?
                .to_ascii_lowercase(),
            );
        }
    }
    roots.extend(additional_model_roots.iter().cloned());
    roots.extend(playsrc_tf2::equipment::stock_weapon_models().map(str::to_owned));
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
                    | "models/weapons/c_models/c_pda_engineer/c_pda_engineer.mdl"
                    | "models/weapons/c_models/c_toolbox/c_toolbox.mdl"
                    | "models/weapons/c_models/c_medic_arms.mdl"
                    | "models/weapons/c_models/c_syringegun/c_syringegun.mdl"
                    | "models/weapons/c_models/c_medigun/c_medigun.mdl"
                    | "models/weapons/c_models/c_bonesaw/c_bonesaw.mdl"
            ) || playsrc_tf2::equipment::stock_weapon_models().any(|model| model == id) {
                playsrc_studio_model::PresentationProfile::ViewModel
            } else {
                playsrc_studio_model::PresentationProfile::World
            };
            let artifact = build_model_presentation(
                &id,
                bundle,
                resource_hashes,
                profile,
                presentation_profile,
            ).inspect_err(|_| presentation_failure(&id))?;
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
        let metadata = decoders.metadata(&path)?;
        directional.push((
            identity,
            u8::from(material.features.ss_bump),
            ReferencedDirectionalTexture {
                logical_path: path.clone(),
                width: u32::from(metadata.width),
                height: u32::from(metadata.height),
                source_sha256: *resource_hashes.get(&path).ok_or(())?,
            },
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
    ).inspect_err(|_| presentation_failure("environment"))?;
    let static_props = compile_static_prop_section(
        canonical,
        graph,
        bundle,
        visibility,
        collision,
        &environment,
        &models,
    ).inspect_err(|_| presentation_failure("static props"))?;
    phase_finished = playsrc_simulation::MetricsClock::monotonic_nanoseconds(&mut metrics_clock);
    metrics[4] = phase_finished.saturating_sub(phase_started);
    phase_started = phase_finished;
    let prepared_model_materials =
        prepare_model_materials(&models, bundle, decoders, resource_hashes, profile)
            .inspect_err(|_| presentation_failure("model materials"))?;
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
    out.extend_from_slice(&15u32.to_le_bytes());
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
        let attachments = named_attachments(&a.model.attachments, |attachment| &attachment.name);
        out.extend_from_slice(
            &u32::try_from(attachments.clone().count())
                .map_err(|_| ())?
                .to_le_bytes(),
        );
        let attachment_pose = playsrc_studio_model::sample_pose(
            &a.model,
            &playsrc_studio_model::AnimationState {
                bone_rotations: Vec::new(),
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
        for v in attachments {
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
        out.extend_from_slice(&texture.source_sha256);
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
    encode_particle_textures(&mut out, particle_presentation, &playsrc_tf2::particle_resources::roots(graph).into_iter().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>())?;
    encode_audio_documents(&mut out, bundle, graph).inspect_err(|_| presentation_failure("audio documents"))?;
    encode_model_occurrence_matrices(
        &mut out,
        graph,
        &static_props.model_lighting,
        &environment.world.cubemaps,
        &models,
    )?;
    encode_model_materials(&mut out, &prepared_model_materials)?;
    legacy_visuals::encode_materials(&mut out, graph, bundle, decoders, resource_hashes, profile)?;
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
    let metadata = models
        .iter()
        .map(|(identity, artifact)| {
            (
                identity.clone(),
                StudioModelLightingMetadata {
                    flex: Arc::clone(&artifact.flex),
                    position: artifact.illumination_position,
                    attachment: artifact.illumination_attachment,
                    eyes: artifact.eyes.clone(),
                },
            )
        })
        .collect();
    let models: BTreeMap<String, Arc<RetainedPresentationModel>> = models
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
        (out, models, metadata, model_material_opacity, environment),
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

fn encode_refract_material(
    out: &mut Vec<u8>,
    identity: &str,
    material: &playsrc_material::Material,
) -> Result<(), ()> {
    let state = playsrc_material::refract_material_output(material)
        .map_err(|_| ())?
        .ok_or(())?;
    pbytes(out, identity.as_bytes())?;
    encode_texture_request(out, &state.normal)?;
    out.extend_from_slice(&[state.blur_amount, u8::from(state.ignore_depth), 0, 0]);
    out.extend_from_slice(&state.refract_amount.to_le_bytes());
    for value in state.refract_tint {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&state.normal_frame.to_le_bytes());
    for value in state.normal_transform {
        out.extend_from_slice(&value.to_le_bytes());
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
    presentation_stage("environment inputs");
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
    let mut sky_parameters = BTreeMap::new();
    presentation_stage("sky metadata");
    for face in playsrc_map::CubeFace::ALL {
        let path = face.material_path(sky);
        let source_sha256 = *resource_hashes.get(&path).ok_or_else(|| presentation_failure(&format!("sky material absent: {path}")))?;
        let m = resolve_material_semantics(&path, bundle, material_environment(profile, false)).inspect_err(|_| presentation_failure(&format!("sky material: {path}")))?;
        let first_texture = m.textures.iter().find(|texture| m.selected_textures.contains(&texture.role)).and_then(|texture| texture.logical_path.as_deref()).ok_or(())?;
        let format = decoders.metadata(&first_texture.to_ascii_lowercase()).inspect_err(|_| presentation_failure(&format!("sky texture: {first_texture}")))?.high_format;
        let encoding = selected_sky_encoding(&m.selected_textures, format).ok_or(())?;
        let mut parameters = playsrc_material::sky_parameters(&m).map_err(|_| ())?;
        if matches!(m.selected_textures.as_slice(), [playsrc_material::TextureRole::Base | playsrc_material::TextureRole::HdrBase])
            && (format == playsrc_vtf::ImageFormat::Rgba16 || (format == playsrc_vtf::ImageFormat::Rgba16F && profile == playsrc_map::LightingProfile::Hdr)) {
            parameters.color = parameters.color.map(|channel| channel * 16.0);
        }
        sky_parameters.insert(face as u8, parameters);
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
        presentation_failure(&format!("cubemap map identity count: {}", map_names.len()));
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
        let m = decoders.metadata(&path).inspect_err(|_| presentation_failure(&format!("cubemap texture: {path}")))?;
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
    presentation_stage("decal metadata");
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
    presentation_stage("game decal metadata");
    for reference in [b"decals/decals_mod2x".as_slice(), b"VGUI/damageindicator"] {
        let reference = reference.to_vec();
        let path = dependency_path(&reference)?;
        let source_sha256 = *resource_hashes.get(&path).ok_or(())?;
        let material =
            resolve_material_semantics(&path, bundle, material_environment(profile, false))?;
        let texture = material
            .textures
            .iter()
            .find(|texture| {
                material.selected_textures.contains(&texture.role)
                    && texture.disposition == playsrc_material::TextureDisposition::Source
            })
            .ok_or(())?;
        let metadata = decoders.metadata(
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
                width: metadata.width,
                height: metadata.height,
                state: material.decal,
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
    presentation_stage("decal receivers");
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
    presentation_stage("environment graph");
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
    .map_err(|error| {
        presentation_failure(&format!("environment compilation: {error:?}"));
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("TF2 environment compilation failed: {error:?}");
        #[cfg(target_arch = "wasm32")]
        let _ = error;
    })?;
    let mut out = b"PENV".to_vec();
    out.extend_from_slice(&7u32.to_le_bytes());
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
    presentation_stage("decal textures");
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
    presentation_stage("water materials");
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
    let mut overlay_paths = std::collections::BTreeSet::new();
    overlay_paths.insert("materials/effects/bleed_overlay.vmt".to_owned());
    for (_, _, material) in &water_materials {
        let state = playsrc_material::water_material_output(material)
            .map_err(|_| ())?
            .ok_or(())?;
        if let Some(overlay) = state.underwater_overlay {
            overlay_paths.insert(overlay.logical_path.to_ascii_lowercase());
        }
    }
    presentation_stage("overlay materials");
    let refract_materials = overlay_paths
        .into_iter()
        .map(|identity| {
            let material = resolve_material_semantics(
                &identity,
                bundle,
                material_environment(profile, false),
            )?;
            if playsrc_material::refract_material_output(&material)
                .map_err(|_| ())?
                .is_none()
            {
                return Err(());
            }
            Ok((identity, material))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    out.extend_from_slice(
        &u32::try_from(refract_materials.len())
            .map_err(|_| ())?
            .to_le_bytes(),
    );
    for (identity, material) in &refract_materials {
        encode_refract_material(&mut out, identity, material)?;
    }
    presentation_stage("world materials");
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
    presentation_stage("environment texture paths");
    let mut environment_texture_paths = std::collections::BTreeSet::new();
    for (_, material) in &refract_materials {
        let overlay = playsrc_material::refract_material_output(material)
            .map_err(|_| ())?
            .ok_or(())?;
        environment_texture_paths.insert(
            overlay
                .normal
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase(),
        );
    }
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
                    || texture.role == playsrc_material::TextureRole::Detail
                    || texture.role == playsrc_material::TextureRole::Bump
                    || texture.role == playsrc_material::TextureRole::Normal)
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
            let parameters = sky_parameters.get(&(face.face as u8)).ok_or(())?;
            for value in parameters.texture_transform.into_iter().chain(parameters.color) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    } else {
        out.extend_from_slice(&0_u32.to_le_bytes());
    }
    let mut runtime_materials = BTreeMap::new();
    let mut runtime_refract_materials = BTreeMap::new();
    let mut runtime_world_materials = BTreeMap::new();
    let mut map_materials = BTreeMap::new();
    let mut normal_frame_counts = BTreeMap::new();
    for (identity, material) in refract_materials {
        let output = playsrc_material::refract_material_output(&material)
            .map_err(|_| ())?
            .ok_or(())?;
        let path = output.normal.logical_path.as_ref().ok_or(())?;
        runtime_refract_materials.insert(
            identity,
            RuntimeRefractMaterial {
                normal_frame_count: u32::from(decoders.metadata(path)?.frame_count),
                material,
            },
        );
    }
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
        let reference = canonical.materials.get(index).ok_or(())?;
        if material.water.is_none() {
            if env.water.volumes.iter().any(|volume| volume.surface_material == reference.index) {
                map_materials.insert(reference.index, reference.logical_path.to_ascii_lowercase());
            }
            continue;
        }
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
            node_cull_modes: world_node_cull_modes(visibility),
            water_materials: runtime_materials,
            refract_materials: runtime_refract_materials,
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
    sprite_card: Option<playsrc_material::ParticleMaterialState>,
    additive_sprite: Option<playsrc_material::AdditiveSpriteState>,
    state: playsrc_material::StaticState,
    metadata: playsrc_vtf::Metadata,
    texture_path: String,
    texture: ModelAuthoredTexture,
}

fn compile_particles(
    b: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    roots: &[&str],
    legacy_materials: &[String],
) -> Result<CompiledParticles, ()> {
    let paths = std::str::from_utf8(b.get(playsrc_tf2::particle_resources::SOURCE_LIST).ok_or(())?)
        .map_err(|_| ())?.lines().collect::<Vec<_>>();
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
    let roots = roots.iter().copied().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>();
    compile_particle_materials(&registry, &roots, b, decoders, legacy_materials)
}

fn smokestack_materials(graph: &playsrc_entity::Graph) -> Result<Vec<String>, u32> {
    let entities = graph.entities.iter().filter(|entity| entity.classname.as_deref().is_some_and(|class| class.eq_ignore_ascii_case(b"env_smokestack")));
    Ok(playsrc_entity::visual_resources::from_entities(entities).map_err(|_| 10_u32)?.materials.into_iter()
        .map(|path| path.trim_start_matches("materials/").trim_end_matches(".vmt").to_owned()).collect())
}

fn compile_cosmetic_particles(b: &BTreeMap<String, &[u8]>, decoders: &TextureDecoders<'_>) -> Result<CompiledParticles, ()> {
    let registry = playsrc_particle::Registry::from_pcf(&[playsrc_particle::PcfSource {
        logical_path: "particles/item_fx.pcf", bytes: b.get("particles/item_fx.pcf").ok_or(())?,
    }], playsrc_particle::RegistryLimits::default()).map_err(|_| ())?;
    compile_particle_materials(&registry, &[playsrc_particle::DefinitionLookup::Name("superrare_burning1")], b, decoders, &[])
}

fn particle_material_dependencies(registry: &playsrc_particle::Registry, roots: &[playsrc_particle::DefinitionLookup<'_>]) -> Result<Vec<String>, ()> {
    let closure = registry.dependency_closure(roots).map_err(|error| { eprintln!("Particle admission: {error}"); })?;
    let mut materials = std::collections::BTreeSet::new();
    for index in closure.definitions {
        let definition = registry.definition_at(index).ok_or(())?;
        if !definition.material.is_empty() && definition.functions.iter().any(|function|
            function.category == playsrc_particle::FunctionCategory::Renderer || function.identity.eq_ignore_ascii_case("Lifetime From Sequence")) {
            materials.insert(definition.material.clone());
        }
    }
    Ok(materials.into_iter().collect())
}

fn compile_particle_materials(
    registry: &playsrc_particle::Registry,
    roots: &[playsrc_particle::DefinitionLookup<'_>],
    b: &BTreeMap<String, &[u8]>,
    decoders: &TextureDecoders<'_>,
    legacy_materials: &[String],
) -> Result<CompiledParticles, ()> {
    // Precache is dependency admission, not execution. ParticleWorld validates
    // an effect's operator closure transactionally when that effect starts.
    let mut materials = particle_material_dependencies(registry, roots)?;
    materials.extend_from_slice(legacy_materials);
    materials.sort();
    materials.dedup();
    let compiled = materials
        .iter()
        .map(|identity| {
            let material_path = dependency_path(identity.as_bytes())?;
            let material = resolve_material_semantics(
                &material_path,
                b,
                playsrc_material::SelectionEnvironment { sprite_card_default_depth_blend: Some(true), ..Default::default() },
            ).map_err(|_| eprintln!("Particle material semantics: {material_path}"))?;
            let (selected, _bytes, metadata) = selected_texture(&material, decoders).map_err(|_| eprintln!("Particle base texture: {material_path}"))?;
            let texture_path = selected
                .logical_path
                .as_ref()
                .ok_or(())?
                .to_ascii_lowercase();
            let hashes = BTreeMap::from([(texture_path.clone(), <[u8; 32]>::from(Sha256::digest(b.get(&texture_path).ok_or(())?)))]);
            let texture = model_authored_texture(&texture_path, decoders, &hashes, true).map_err(|_| eprintln!("Particle mip texture: {texture_path}"))?;
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
                playsrc_material::BlendFactor::DestinationColor
                | playsrc_material::BlendFactor::SourceColor => {
                    unreachable!("authored particle materials cannot use decal modulation")
                }
            };
            Ok((
                identity.clone(),
                (
                    playsrc_particle::ParticleMaterial {
                        mapping_height: u32::from(metadata.height),
                        shader: if material.particle.is_some() {
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
                        sprite_card: material.particle.clone(),
                        additive_sprite: playsrc_material::additive_sprite_state(&material).map_err(|_| ())?,
                        state,
                        metadata: metadata.clone(),
                        texture_path,
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
        playsrc_particle::ParticleWorld::new(registry, &sheets, playsrc_particle::WorldLimits::default())
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
    decode_particle_sheet_data(resource)
}

fn decode_particle_sheet_data(resource: &[u8]) -> Result<playsrc_particle::ParticleSheet, ()> {
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
        if !(0..64).contains(&identity) {
            return Err(());
        }
        let clamp = reader.u32()? != 0;
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
struct ParticleCollision<'a> {
    world: Arc<playsrc_collision::World>,
    visibility: &'a playsrc_visibility::World,
    lighting: &'a playsrc_map::ModelLightingWorld<'static>,
    lighting_cache: BTreeMap<[u32; 3], [u8; 3]>,
}
impl playsrc_particle::CollisionQuery for ParticleCollision<'_> {
    fn trace_batch(
        &mut self,
        requests: &[playsrc_particle::TraceRequest],
    ) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> {
        Ok(requests
            .iter()
            .map(|r| {
                let radius = r.radius.max(0.0);
                let trace = self
                    .world
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

    fn lighting_at(&mut self, position: [f32; 3]) -> Result<[u8; 3], playsrc_particle::Error> {
        let identity = position.map(f32::to_bits);
        if let Some(value) = self.lighting_cache.get(&identity).copied() {
            return Ok(value);
        }
        let failure = || playsrc_particle::Error {
            code: playsrc_particle::ErrorCode::MissingQuery,
            source: "particle-lighting".into(),
            offset: 0,
            detail: "authored map lighting query is unavailable".into(),
        };
        let mut node = 0_i32;
        while node >= 0 {
            let entry = self
                .visibility
                .nodes
                .get(node as usize)
                .ok_or_else(failure)?;
            let plane = self
                .visibility
                .planes
                .get(entry.plane_index as usize)
                .ok_or_else(failure)?;
            node = entry.children[usize::from(dot(position, plane.normal) < plane.distance)];
        }
        let mut index = usize::try_from(-node - 1).map_err(|_| failure())?;
        let mut ambient = self
            .lighting
            .ambient_indexes()
            .get(index)
            .ok_or_else(failure)?;
        if ambient.sample_count == 0 && ambient.first_sample != 0 {
            index = usize::from(ambient.first_sample);
            ambient = self
                .lighting
                .ambient_indexes()
                .get(index)
                .ok_or_else(failure)?;
        }
        let leaf = self.visibility.leaves.get(index).ok_or_else(failure)?;
        let mut cube = [[0.0_f32; 3]; 6];
        let mut total = 0.0;
        let first = usize::from(ambient.first_sample);
        let last = first + usize::from(ambient.sample_count);
        for sample in self
            .lighting
            .ambient_samples()
            .get(first..last)
            .ok_or_else(failure)?
        {
            let point = std::array::from_fn(|axis| {
                f32::from(leaf.mins[axis])
                    + f32::from(sample.position[axis])
                        * f32::from(leaf.maxs[axis] - leaf.mins[axis])
                        / 255.0
            });
            let delta = sub3(point, position);
            let factor = (dot(delta, delta) + 1.0).recip();
            total += factor;
            for (face, source) in cube.iter_mut().zip(sample.cube) {
                for channel in 0..3 {
                    face[channel] += source[channel] * factor;
                }
            }
        }
        if total > 0.0 {
            for face in &mut cube {
                for channel in face {
                    *channel /= total;
                }
            }
        }
        for light in self.lighting.world_lights() {
            let Ok((end, direction, ratio)) = playsrc_map::source_world_light_ray(position, light)
            else {
                continue;
            };
            if ratio <= 0.0 {
                continue;
            }
            let trace = self
                .world
                .trace_hull(
                    position,
                    end,
                    playsrc_collision::Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    u32::MAX,
                )
                .map_err(|_| failure())?;
            let distance = dot(sub3(end, position), sub3(end, position)).sqrt();
            if if light.kind == 3 {
                trace.surface_flags & 0x0004 == 0
            } else {
                (1.0 - trace.fraction) * distance > 8.0
            } {
                continue;
            }
            let angle =
                playsrc_map::source_world_light_angle(light, direction).map_err(|_| failure())?;
            playsrc_map::add_world_light_to_cube(
                &mut cube,
                direction,
                light.intensity,
                ratio * angle,
            )
            .map_err(|_| failure())?;
        }
        let value = std::array::from_fn(|channel| {
            let value = cube.iter().map(|face| face[channel]).sum::<f32>() / 6.0;
            (value.clamp(0.0, 1.0) * 255.0) as u8
        });
        self.lighting_cache.insert(identity, value);
        Ok(value)
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

    fn cp(&mut self, index: u8) -> Result<playsrc_particle::ControlPoint, ()> {
        let position = [self.f32()?, self.f32()?, self.f32()?];
        let orientation = [self.f32()?, self.f32()?, self.f32()?, self.f32()?];
        Ok(playsrc_particle::ControlPoint {
            index,
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
        Option<LegacyParticleFrame<'_>>,
        Option<playsrc_particle::VisibilityView>,
        Vec<playsrc_particle::VisibilitySample>,
    ),
    (),
> {
    let mut r = ParticleReader { bytes, at: 0 };
    if r.take(4)? != b"PPTX" || r.u32()? != 5 {
        return Err(());
    }
    let from = r.f32()?;
    let to = r.f32()?;
    let camera_position = [r.f32()?, r.f32()?, r.f32()?];
    let count = r.u32()? as usize;
    if count == 0x8000_0000 {
        let frame_seconds = r.f32()?;
        let accepted = r.u32()?;
        let identity = r.u32()?;
        let view = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        let visual_length = r.u32()? as usize;
        let visual_payload = r.take(visual_length)?;
        if from < 0.0 || from != to || frame_seconds < 0.0 || accepted >= identity || view[2] <= 0.0 || view[2] >= 180.0 || view[3] <= 0.0 || r.at != bytes.len() { return Err(()); }
        return Ok((Vec::new(), playsrc_particle::AdvanceRequest { from_seconds: from, to_seconds: to,
            maximum_step_seconds: 1.0 / 60.0, camera_position }, Some(LegacyParticleFrame { seconds: frame_seconds, accepted, identity, view, visual_payload }), None, Vec::new()));
    }
    if count > 4096 || from < 0.0 || to < from {
        return Err(());
    }
    let mut events = Vec::with_capacity(count);
    let mut prior_timestamp = from;
    for order in 0..count {
        let kind = r.u8()?;
        let mode = r.u8()?;
        let controls = r.u8()?;
        if r.u8()? != 0
            || (kind != 3 && mode != 0)
            || (kind == 1 && !(1..=2).contains(&controls))
            || (kind == 2 && controls > 30)
            || (kind == 3 && controls != 0)
        {
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
                let mut control_points = Vec::with_capacity(usize::from(controls));
                for index in 0..controls {
                    control_points.push(r.cp(index)?);
                }
                playsrc_particle::EventCommand::Create {
                    effect_identity,
                    definition,
                    seed,
                    owner_identity: owner,
                    control_points,
                }
            }
            2 => playsrc_particle::EventCommand::SetControlPoint {
                effect_identity,
                control_point: r.cp(controls)?,
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
    let visibility_view = match r.u32()? {
        0 => None,
        1 => {
            let view = playsrc_particle::VisibilityView { yaw_degrees: r.f32()?, pitch_degrees: r.f32()?, vertical_fov_degrees: r.f32()?, width: r.f32()?, height: r.f32()? };
            if !view.valid() { return Err(()); }
            Some(view)
        },
        _ => return Err(()),
    };
    let sample_count = r.u32()? as usize;
    if sample_count > 4096 || (sample_count > 0 && visibility_view.is_none()) { return Err(()); }
    let mut visibility_samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let identity = r.u64()?;
        let visible_pixels = r.u32()? as i32;
        let possible_pixels = r.u32()? as i32;
        let clip_fraction = r.f32()?;
        if visibility_samples.last().is_some_and(|prior: &playsrc_particle::VisibilitySample| prior.identity >= identity)
            || visible_pixels < -1 || possible_pixels < -1 || !(0.0..=1.0).contains(&clip_fraction) { return Err(()); }
        visibility_samples.push(playsrc_particle::VisibilitySample { identity, visible_pixels, possible_pixels, clip_fraction });
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
        None,
        visibility_view,
        visibility_samples,
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
    #[test]
    fn named_attachment_projection_keeps_first_case_insensitive_match_without_changing_indices() {
        let attachments = [(b"muzzle".to_vec(), 0), (b"MUZZLE".to_vec(), 1), (b"shell".to_vec(), 2)];
        let selected = named_attachments(&attachments, |attachment| attachment.0.as_slice());
        assert_eq!(selected.clone().count(), 2);
        assert_eq!(selected.map(|attachment| attachment.1).collect::<Vec<_>>(), [0, 2]);
        assert_eq!(attachments[1].1, 1);
    }

    #[test]
    fn damage_event_wire_preserves_full_and_mini_critical_kinds() {
        for (crit, code) in [(playsrc_tf2::damage::CritKind::None, 0.0), (playsrc_tf2::damage::CritKind::Full, 1.0), (playsrc_tf2::damage::CritKind::Mini, 2.0)] {
            let mut bytes = Vec::new();
            super::encode_game_event(&mut bytes, &playsrc_tf2::Event::PlayerDamaged {
                attacker: 1, victim: 2, weapon: playsrc_tf2::Weapon::Bat, amount: 10, health: 0, crit, custom: 0,
            }, 28).unwrap();
            assert_eq!(bytes.len(), 28);
            assert_eq!(bytes[0], 17);
            assert_eq!(f32::from_le_bytes(bytes[20..24].try_into().unwrap()), code);
        }
    }
    #[test]
    fn sound_patch_wire_actions_preserve_samples_source_and_order() {
        use playsrc_tf2::{
            AudioAction, AudioEvent, AudioEventIdentity, AudioSourceKind, SoundDefinition,
            SoundSamples,
        };
        for (ordinal, action) in [
            AudioAction::Play,
            AudioAction::Stop,
            AudioAction::FadeIn(3.5),
            AudioAction::FadeOut(3.5),
        ]
        .into_iter()
        .enumerate()
        {
            let mut bytes = Vec::new();
            super::encode_audio_event(
                &mut bytes,
                AudioEvent {
                    tick: 17,
                    ordinal: ordinal as u16,
                    action,
                    identity: AudioEventIdentity::WeaponSingle,
                    definition: SoundDefinition::FlameLoop,
                    source_kind: AudioSourceKind::Entity,
                    source_identity: 29,
                    owner_identity: Some(29),
                    position: [1.0, 2.0, 3.0],
                    samples: SoundSamples {
                        volume: 0.25,
                        pitch: 0.5,
                        wave: 0,
                        sound_level: 0.75,
                    },
                },
                52,
            )
            .unwrap();
            assert_eq!(bytes.len(), 52);
            assert_eq!(bytes[15], ordinal as u8);
            assert_eq!(bytes[11], 38);
            assert_eq!(&bytes[16..20], &29u32.to_le_bytes());
            assert_eq!(&bytes[36..40], &0.25f32.to_le_bytes());
            assert_eq!(
                &bytes[48..52],
                &(if ordinal >= 2 { 3.5f32 } else { 0.0 }).to_le_bytes()
            );
        }
    }

    use super::*;

    fn canonical_bone_palette(bones: impl IntoIterator<Item = u8>) -> Vec<u8> {
        model_palette::BonePalette::from_bones(bones).iter().collect()
    }

    #[test]
    fn authored_bone_palettes_are_canonical_sparse_and_bounded_by_source_bone_ids() {
        assert_eq!(
            canonical_bone_palette([19, 4, 255, 4, 0, 19]),
            vec![0, 4, 19, 255]
        );
        assert!(canonical_bone_palette([]).is_empty());
        assert_eq!(
            canonical_bone_palette((0_u8..=255).rev()),
            (0_u8..=255).collect::<Vec<_>>()
        );
        let repeated = canonical_bone_palette(std::iter::repeat_n(19, 100_000));
        assert_eq!(repeated, [19]);
        assert!(repeated.capacity() <= 256, "palette allocation must not scale with vertex references");
    }

    #[test]
    fn bounded_palette_preserves_every_sort_dedup_result_and_matrix_order() {
        let mut state = 0x12ab_7f89_u32;
        for length in [0, 1, 3, 32, 255, 256, 1024, 65_536] {
            let references = (0..length).map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 24) as u8
            }).collect::<Vec<_>>();
            let mut expected = references.clone();
            expected.sort_unstable();
            expected.dedup();
            let actual = canonical_bone_palette(references);
            assert_eq!(actual, expected);
            assert!(actual.capacity() <= 256);
            // The palette's ascending IDs own the output matrix order, including
            // sparse high IDs. The compact transport does not resend the IDs.
            let encode = |palette: &[u8]| palette.iter().flat_map(|bone| {
                [u32::from(*bone), 0x8000_0000, 0x3f80_0000].into_iter().flat_map(u32::to_le_bytes)
            }).collect::<Vec<_>>();
            assert_eq!(encode(&actual), encode(&expected));
        }
    }

    #[test]
    fn static_model_ownership_ends_without_releasing_dynamic_or_replacement_models() {
        let static_model = Arc::new(vec![1_u8; 4096]);
        let released_source = Arc::downgrade(&static_model);
        let dynamic_model = Arc::new(vec![2_u8; 2048]);
        let retained_source = Arc::downgrade(&dynamic_model);
        let mut models = BTreeMap::from([
            ("models/props/static.mdl".to_owned(), static_model),
            ("models/props/shared.mdl".to_owned(), dynamic_model),
            ("models/player/soldier.mdl".to_owned(), Arc::new(vec![3_u8])),
        ]);
        let dynamic = std::collections::BTreeSet::from(["models/props/shared.mdl".to_owned()]);
        let released = release_static_only_models(
            &mut models,
            ["models/props/static.mdl", "models/props/shared.mdl"],
            &dynamic,
        );
        assert_eq!(
            released,
            std::collections::BTreeSet::from(["models/props/static.mdl".to_owned()])
        );
        assert!(released_source.upgrade().is_none());
        assert!(retained_source.upgrade().is_some());

        let replacement = Arc::new(vec![4_u8; 1024]);
        let replacement_source = Arc::downgrade(&replacement);
        models.insert("models/props/static.mdl".to_owned(), replacement);
        assert!(replacement_source.upgrade().is_some());
        release_static_only_models(
            &mut models,
            ["models/props/static.mdl"],
            &std::collections::BTreeSet::new(),
        );
        assert!(replacement_source.upgrade().is_none());
        assert!(retained_source.upgrade().is_some());
    }

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
        assert_eq!(playsrc_resource_release(pointer, 4), 1);
        assert_eq!(playsrc_resource_release(pointer, 4), 0);
        assert!(playsrc_resource_take().is_null());
    }

    #[test]
    fn authored_resource_sections_survive_release_until_the_final_model_owner() {
        *resource_output().lock().unwrap() = b"prefix:borrowed model:suffix".to_vec();
        let pointer = playsrc_resource_take();
        let section = unsafe { std::slice::from_raw_parts(pointer, 28) };
        let model = playsrc_studio_model::retain_authored_source(
            "models/player/wasm-owner.mdl",
            &section[7..21],
            [93; 32],
        );
        assert_eq!(model.as_ptr(), unsafe { pointer.add(7) });
        assert_eq!(playsrc_resource_release(pointer, 27), 0);
        assert_eq!(playsrc_resource_release(pointer, 28), 1);
        assert_eq!(model.as_ref().as_ref(), b"borrowed model");
        drop(model);
        assert_eq!(playsrc_resource_release(pointer, 28), 0);
    }

    #[test]
    fn bounded_resource_sections_preserve_the_canonical_monolithic_identity() {
        let first =
            playsrc_asset_graph::encode_resource_set(&[playsrc_asset_graph::DecodedEntry {
                logical_path: "materials/b.vmt".to_owned(),
                bytes: vec![2, 3, 4],
            }])
            .unwrap();
        let second =
            playsrc_asset_graph::encode_resource_set(&[playsrc_asset_graph::DecodedEntry {
                logical_path: "materials/a.vmt".to_owned(),
                bytes: vec![7, 8],
            }])
            .unwrap();
        let sections = [
            ResourceSection {
                pointer: first.as_ptr(),
                length: first.len(),
            },
            ResourceSection {
                pointer: second.as_ptr(),
                length: second.len(),
            },
        ];
        let canonical = playsrc_asset_graph::encode_resource_set(&[
            playsrc_asset_graph::DecodedEntry {
                logical_path: "materials/a.vmt".to_owned(),
                bytes: vec![7, 8],
            },
            playsrc_asset_graph::DecodedEntry {
                logical_path: "materials/b.vmt".to_owned(),
                bytes: vec![2, 3, 4],
            },
        ])
        .unwrap();
        let mut actual = [0; 32];
        assert_eq!(
            unsafe {
                playsrc_resource_sections_hash(
                    sections.as_ptr(),
                    sections.len(),
                    actual.as_mut_ptr(),
                )
            },
            canonical.len()
        );
        assert_eq!(actual, Sha256::digest(&canonical).as_slice());
        let duplicate = [sections[0], sections[0]];
        assert_eq!(
            unsafe {
                playsrc_resource_sections_hash(
                    duplicate.as_ptr(),
                    duplicate.len(),
                    actual.as_mut_ptr(),
                )
            },
            0
        );
    }

    #[test]
    #[ignore = "requires the configured particle registry resource graph"]
    fn configured_game_particle_registry_is_executable() {
        let graph = std::env::var("PLAYSRC_PYRO_GRAPH").unwrap();
        let bytes = playsrc_asset_graph::read_resource_set(std::path::Path::new(&graph), Some("gameplay")).unwrap();
        let resources = bundle(&bytes).unwrap();
        let paths = std::str::from_utf8(resources[playsrc_tf2::particle_resources::SOURCE_LIST]).unwrap();
        let sources = paths.lines().map(|path| playsrc_particle::PcfSource { logical_path: path, bytes: resources[path] }).collect::<Vec<_>>();
        let registry = playsrc_particle::Registry::from_pcf(&sources, playsrc_particle::RegistryLimits::default()).unwrap();
        let roots = playsrc_tf2::particle_resources::GAME_SYSTEMS.iter().copied().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>();
        registry.target_closure(&roots).unwrap();
        if resources.keys().any(|path| path.starts_with("materials/")) {
            compile_particle_materials(&registry, &roots, &resources, &TextureDecoders::new(&resources), &[]).unwrap();
        }
        if let Ok(path) = std::env::var("PLAYSRC_PARTICLE_BSP") {
            let bsp_bytes = std::fs::read(path).unwrap();
            let bsp = playsrc_bsp::parse(&bsp_bytes, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).unwrap();
            let graph = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default()).unwrap();
            let roots = playsrc_tf2::particle_resources::roots(&graph);
            let legacy = smokestack_materials(&graph).unwrap();
            let (mut world, _, _, _) = compile_particles(&resources, &TextureDecoders::new(&resources), &roots, &legacy).unwrap();
            use playsrc_particle::{AdvanceRequest, ControlPoint, Event, EventCommand};
            struct NoQueries;
            impl playsrc_particle::CollisionQuery for NoQueries {
                fn trace_batch(&mut self, _: &[playsrc_particle::TraceRequest]) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> { panic!("unexpected cart-light collision query") }
            }
            let create = Event { identity: 1, timestamp_seconds: 0.0, source_order: 0, command: EventCommand::Create {
                effect_identity: 1, definition: "cart_flashinglight".into(), seed: 1, owner_identity: None,
                control_points: vec![ControlPoint { index: 0, position: [10.0, 0.0, 0.0], previous_position: [10.0, 0.0, 0.0],
                    orientation: [0.0,0.0,0.0,1.0], velocity: [0.0;3], radius: 0.0, density: 0.0, duration: 0.0, parent: None, object_identity: None }] } };
            let request = |from_seconds, to_seconds| AdvanceRequest { from_seconds, to_seconds, maximum_step_seconds: 0.015, camera_position: [0.0;3] };
            let (first, _) = world.advance(&[create], request(0.0, 0.015), &mut NoQueries).unwrap();
            let (second, _) = world.advance(&[], request(0.015, 0.03), &mut NoQueries).unwrap();
            assert!(!first.is_empty());
            assert_eq!(first[0].orientation_type, 1);
            assert!((second[0].yaw_radians - first[0].yaw_radians - 0.015 * std::f32::consts::TAU * std::f32::consts::PI / 3.0).abs() < 0.00001);
        }
    }

    #[test]
    #[ignore = "requires the exact configured projectile graph and map BSP"]
    fn configured_pyro_particle_materials_compile() {
        let graph = std::env::var("PLAYSRC_PYRO_GRAPH").expect("configured Pyro graph path");
        let bytes =
            playsrc_asset_graph::read_resource_set(std::path::Path::new(&graph), Some("gameplay")).unwrap();
        let resources = bundle(&bytes).unwrap();
        let decoders = TextureDecoders::new(&resources);
        let sources = std::str::from_utf8(resources[playsrc_tf2::particle_resources::SOURCE_LIST]).unwrap().lines().map(|path| playsrc_particle::PcfSource {
            logical_path: path,
            bytes: resources[path],
        }).collect::<Vec<_>>();
        let registry = playsrc_particle::Registry::from_pcf(
            &sources,
            playsrc_particle::RegistryLimits::default(),
        )
        .unwrap();
        let bsp_bytes = std::fs::read(std::env::var("PLAYSRC_PYRO_BSP").expect("configured map BSP path")).unwrap();
        let bsp = playsrc_bsp::parse(&bsp_bytes, playsrc_bsp::Profile::Source2013V20, playsrc_bsp::Limits::default()).unwrap();
        let entities = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), playsrc_entity::Limits::default()).unwrap();
        let names = playsrc_tf2::particle_resources::roots(&entities);
        let roots = names.iter().copied().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>();
        let legacy = smokestack_materials(&entities).unwrap();
        for identity in particle_material_dependencies(&registry, &roots).unwrap().into_iter().chain(legacy.iter().cloned()) {
            eprintln!("compiling selected particle material {identity}");
            let path = dependency_path(identity.as_bytes()).unwrap();
            let material = resolve_material_semantics(
                &path,
                &resources,
                playsrc_material::SelectionEnvironment { sprite_card_default_depth_blend: Some(true), ..Default::default() },
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
            let hashes = BTreeMap::from([(texture.clone(), <[u8;32]>::from(Sha256::digest(resources[&texture])))]);
            model_authored_texture(&texture, &decoders, &hashes, true).unwrap_or_else(|_| panic!("authored texture {identity}: {texture}"));
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
        assert!(compile_particles(&resources, &decoders, &names, &legacy).is_ok());
    }

    #[test]
    #[ignore = "requires an exact configured map resource set and BSP"]
    fn configured_map_particle_admission() {
        let bsp = std::fs::read(std::env::var("PLAYSRC_PARTICLE_BSP").expect("configured BSP path")).unwrap();
        let bsp = playsrc_bsp::parse(&bsp, playsrc_bsp::Profile::Source2013V20, Default::default()).unwrap();
        let entities = playsrc_entity::parse(bsp.lumps[0].bytes(&bsp), Default::default()).unwrap();
        let bytes = std::fs::read(std::env::var("PLAYSRC_PARTICLE_RESOURCE_SET").expect("configured resource set path")).unwrap();
        let resources = bundle(&bytes).unwrap();
        let paths = std::str::from_utf8(resources[playsrc_tf2::particle_resources::SOURCE_LIST]).unwrap();
        let sources = paths.lines().map(|path| playsrc_particle::PcfSource { logical_path: path, bytes: resources[path] }).collect::<Vec<_>>();
        let registry = playsrc_particle::Registry::from_pcf(&sources, Default::default()).unwrap();
        let roots = playsrc_tf2::particle_resources::roots(&entities);
        registry.target_closure(&roots.iter().copied().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>()).unwrap();
        let decoders = TextureDecoders::new(&resources);
        assert!(compile_particles(&resources, &decoders, &roots, &smokestack_materials(&entities).unwrap()).is_ok());
    }

    #[test]
    #[ignore = "requires the configured TF2 graph with Team Captain and item_fx"]
    fn configured_burning_flames_models_lifetimes_and_independent_effects() {
        use playsrc_particle::{AdvanceRequest, ControlPoint, Event, EventCommand};
        let graph = std::env::var("PLAYSRC_COSMETIC_GRAPH").expect("configured graph path");
        let bytes = playsrc_asset_graph::read_resource_set(std::path::Path::new(&graph), Some("gameplay")).unwrap();
        let resources = bundle(&bytes).unwrap();
        let decoders = TextureDecoders::new(&resources);
        let registry = playsrc_particle::Registry::from_pcf(&[playsrc_particle::PcfSource { logical_path: "particles/item_fx.pcf", bytes: resources["particles/item_fx.pcf"] }], Default::default()).unwrap();
        for identity in registry.target_closure(&[playsrc_particle::DefinitionLookup::Name("superrare_burning1")]).unwrap().materials {
            let path = dependency_path(identity.as_bytes()).unwrap();
            let material = resolve_material_semantics(&path, &resources, playsrc_material::SelectionEnvironment { sprite_card_default_depth_blend: Some(true), ..Default::default() }).unwrap_or_else(|_| panic!("resolve {identity}"));
            let (selected, _, metadata) = selected_texture(&material, &decoders).unwrap_or_else(|_| panic!("select {identity}"));
            rgba_texture(&selected.logical_path.as_ref().unwrap().to_ascii_lowercase(), &decoders).unwrap_or_else(|_| panic!("texture {identity}"));
            playsrc_material::static_state(&material, playsrc_material::TextureAlphaFacts { base: metadata.alpha_flags.one_bit || metadata.alpha_flags.eight_bit }).unwrap_or_else(|error| panic!("state {identity}: {error:?}"));
            decode_particle_sheet(metadata).unwrap_or_else(|_| panic!("sheet {identity}"));
        }
        let (mut world, _, materials, presentation) = compile_particles(&resources, &decoders, playsrc_tf2::particle_resources::GAME_SYSTEMS, &[]).unwrap();
        let fire = "particle/flamethrowerfire/flamethrowerfire102.vmt";
        let state = presentation[fire].sprite_card.as_ref().unwrap();
        assert_eq!(state.add_self.value, 0.5);
        assert_eq!(state.overbright_factor.value, 5.0);
        assert!(state.depth_blend.as_ref().unwrap().value);
        assert_eq!(state.depth_blend_scale.value, 100.0);
        assert_eq!(presentation[fire].texture.manifest().mip_count, 11);
        assert_eq!(presentation["effects/rocketrailsmoke.vmt"].texture.manifest().mip_count, 10);
        assert!(matches!(presentation[fire].texture, ModelAuthoredTexture::Referenced(_)));
        assert_eq!(materials[fire].sheet.sequences.values().map(|s| s.duration_seconds).collect::<Vec<_>>(), [19.0,21.0,18.0,19.0,19.0]);
        let hashes = resources.iter().map(|(path, bytes)| (path.clone(), <[u8;32]>::from(Sha256::digest(bytes)))).collect();
        for class in ["soldier", "medic", "heavy"] {
            let path = format!("models/player/items/{class}/{class}_officer.mdl");
            let hat = playsrc_tf2::presentation::build_model(&path, &resources, &hashes, false, playsrc_studio_model::PresentationProfile::World).unwrap();
            let parent = playsrc_tf2::presentation::build_model(&format!("models/player/{class}.mdl"), &resources, &hashes, false, playsrc_studio_model::PresentationProfile::World).unwrap();
            let sample = |m: &playsrc_studio_model::PresentationModel| playsrc_studio_model::sample_pose(m, &playsrc_studio_model::AnimationState { base_sequence: 0, cycle: playsrc_studio_model::Float32(0), pose_parameters: vec![playsrc_studio_model::Float32(0); m.pose_parameters.len()], layers: Vec::new(), bone_rotations: Vec::new() }).unwrap();
            let parent_pose = sample(&parent.model);
            let merged = playsrc_studio_model::merge_model_pose(&parent.model, &parent_pose, &hat.model, &sample(&hat.model)).unwrap();
            assert!(!merged.skinning_matrices.is_empty());
            let root = wearable::control_point(&hat.model, &merged, false).unwrap();
            assert!(root.0.iter().all(|v| f32::from_bits(v.0).is_finite()));
            let head = hat.model.bones.iter().position(|b| b.name == b"bip_head").unwrap();
            let parent_head = parent.model.bones.iter().position(|b| b.name == b"bip_head").unwrap();
            assert_eq!(merged.model_matrices[head], parent_pose.model_matrices[parent_head]);
        }
        struct NoQueries;
        impl playsrc_particle::CollisionQuery for NoQueries {
            fn trace_batch(&mut self, _: &[playsrc_particle::TraceRequest]) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> { panic!("unexpected collision query") }
        }
        let create = |identity: u32, x| Event { identity: identity as u64, timestamp_seconds: 0.0, source_order: identity,
            command: EventCommand::Create { effect_identity: identity, definition: "superrare_burning1".into(), seed: identity as u64, owner_identity: Some(identity),
                control_points: vec![ControlPoint { index: 0, position: [x,0.0,72.0], previous_position: [x,0.0,72.0], orientation: [0.0,0.0,0.0,1.0], velocity: [0.0;3], radius: 0.0, density: 0.0, duration: 0.0, parent: None, object_identity: Some(identity) }] } };
        let mut peak = 0;
        for tick in 0..240 {
            let events = if tick == 0 { vec![create(7, 0.0), create(8, 1000.0)] } else if tick == 120 { vec![Event { identity: 9, timestamp_seconds: tick as f32 * 0.015, source_order: 0, command: EventCommand::Destroy { effect_identity: 7 } }] } else { vec![] };
            let (items, _) = world.advance(&events, AdvanceRequest { from_seconds: tick as f32*0.015, to_seconds: (tick+1) as f32*0.015, maximum_step_seconds: 0.05, camera_position: [0.0;3] }, &mut NoQueries).unwrap();
            peak = peak.max(items.len());
            assert!(items.len() <= 40);
            if tick > 121 { assert!(items.iter().all(|p| p.effect_identity == 8)); }
            for item in items {
                if item.material == fire { assert_eq!(item.lifetime_seconds, materials[fire].sheet.sequences[&item.sequence].duration_seconds/30.0); }
                if item.effect_identity == 8 { assert!(item.position[0] > 900.0); }
            }
        }
        assert!(peak >= 30, "both authored systems must emit on both actors");
        assert_eq!(world.effect_count(), 1);
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

    #[test]
    fn source_world_bitsets_preserve_first_face_order_and_reject_invalid_membership() {
        let mut words = [0_u64; 1024];
        let faces = [0, 63, 64, u16::MAX, 63, 0, u16::MAX, 65];
        let visible = faces
            .into_iter()
            .filter(|&face| admit_world_bit(&mut words, usize::from(face)).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(visible, [0, 63, 64, u16::MAX, 65]);
        assert!(admit_world_bit(&mut words, usize::from(u16::MAX) + 1).is_err());
    }

    #[test]
    fn visible_membership_bitsets_preserve_tree_wire_order_and_domain_edges() {
        for ids in [
            vec![],
            vec![65_535, 64, 63, 0, 511, 512, 65_535, 63],
            (0..=65_535_u32).rev().chain(0..=65_535).collect(),
        ] {
            let mut words = [0_u64; 1024];
            let mut reference = std::collections::BTreeSet::new();
            for id in ids {
                admit_world_bit(&mut words, id as usize).unwrap();
                reference.insert(id);
            }
            let mut expected = (reference.len() as u32).to_le_bytes().to_vec();
            for id in reference { expected.extend_from_slice(&id.to_le_bytes()); }
            let mut actual = Vec::with_capacity(expected.len());
            let capacity = actual.capacity();
            encode_sorted_world_bits(&mut actual, &words);
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), capacity);
        }
        let mut areas = [0_u64; 8];
        for id in [511, 0, 63, 64, 511] { admit_world_bit(&mut areas, id).unwrap(); }
        let mut output = Vec::new();
        encode_sorted_world_bits(&mut output, &areas);
        assert_eq!(output, [4_u32, 0, 63, 64, 511].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>());
    }

    #[test]
    fn authored_three_channel_planes_remain_source_backed_until_their_frame_is_selected() {
        for format in [2_i32, 3_i32] {
            let mut source = vec![0_u8; 70];
            source[..4].copy_from_slice(b"VTF\0");
            source[4..8].copy_from_slice(&7_u32.to_le_bytes());
            source[8..12].copy_from_slice(&1_u32.to_le_bytes());
            source[12..16].copy_from_slice(&64_u32.to_le_bytes());
            source[16..18].copy_from_slice(&1_u16.to_le_bytes());
            source[18..20].copy_from_slice(&1_u16.to_le_bytes());
            source[24..26].copy_from_slice(&2_u16.to_le_bytes());
            source[52..56].copy_from_slice(&format.to_le_bytes());
            source[56] = 1;
            source[57..61].copy_from_slice(&(-1_i32).to_le_bytes());
            source[64..70].copy_from_slice(&[3, 2, 1, 6, 5, 4]);
            let path = "materials/animated.vtf".to_owned();
            let bundle = BTreeMap::from([(path.clone(), source.as_slice())]);
            let hashes = BTreeMap::from([(path.clone(), [7_u8; 32])]);
            let texture =
                model_authored_texture(&path, &TextureDecoders::new(&bundle), &hashes, false)
                    .expect("authored RGB animation must retain its source ranges");
            let ModelAuthoredTexture::Referenced(texture) = texture else {
                panic!("RGB animation was decoded before frame selection")
            };
            assert_eq!(texture.format, format);
            assert_eq!(texture.manifest.frame_count, 2);
            assert_eq!(texture.planes.len(), 2);
            assert_eq!(texture.planes[0].range, 64..67);
            assert_eq!(texture.planes[1].range, 67..70);
        }
    }

    fn particle_stop_transaction(mode: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PPTX");
        bytes.extend_from_slice(&5_u32.to_le_bytes());
        bytes.extend_from_slice(&0.0_f32.to_le_bytes());
        bytes.extend_from_slice(&0.015_f32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[3, mode, 0, 0]);
        bytes.extend_from_slice(&7_u64.to_le_bytes());
        bytes.extend_from_slice(&0.015_f32.to_le_bytes());
        bytes.extend_from_slice(&9_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes
    }

    #[test]
    fn legacy_frame_envelope_owns_one_clock_ack_and_exact_visual_payload_range() {
        let mut bytes = vec![0_u8; 68];
        bytes[..4].copy_from_slice(b"PPTX");
        bytes[4..8].copy_from_slice(&5_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0.3_f32.to_le_bytes());
        bytes[36..40].copy_from_slice(&7_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&8_u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&75.0_f32.to_le_bytes());
        bytes[56..60].copy_from_slice(&(16.0_f32 / 9.0).to_le_bytes());
        bytes[60..64].copy_from_slice(&4_u32.to_le_bytes());
        bytes[64..].copy_from_slice(b"PLVQ");
        let (events, request, frame, _, _) = decode_particle_transaction(&bytes).unwrap();
        assert!(events.is_empty());
        assert_eq!(request.from_seconds, request.to_seconds);
        let frame = frame.unwrap();
        assert_eq!((frame.seconds, frame.accepted, frame.identity), (0.3, 7, 8));
        assert_eq!(frame.visual_payload, b"PLVQ");
        assert!(decode_particle_transaction(&bytes[..67]).is_err());
        bytes[36..40].copy_from_slice(&8_u32.to_le_bytes());
        assert!(decode_particle_transaction(&bytes).is_err());
    }

    #[test]
    fn particle_transaction_uses_one_stop_opcode_and_explicit_source_modes() {
        for (mode, expected) in [
            (0, playsrc_particle::StopMode::Graceful),
            (1, playsrc_particle::StopMode::Immediate),
        ] {
            let bytes = particle_stop_transaction(mode);
            let (events, request, legacy, view, samples) = decode_particle_transaction(&bytes)
                .expect("current particle transaction must decode");
            assert!(legacy.is_none());
            assert!(view.is_none() && samples.is_empty());
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
    fn particle_sheets_apply_later_authored_sequence_records_to_the_same_slot() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        for span in [1.0_f32, 2.0] {
            for value in [3u32, 1, 1] { bytes.extend_from_slice(&value.to_le_bytes()); }
            bytes.extend_from_slice(&span.to_le_bytes());
            bytes.extend_from_slice(&span.to_le_bytes());
            for _ in 0..4 { for value in [0.0_f32, 0.0, 1.0, 1.0] { bytes.extend_from_slice(&value.to_le_bytes()); } }
        }
        let sheet = decode_particle_sheet_data(&bytes).unwrap();
        assert_eq!(sheet.sequences.len(), 1);
        assert_eq!(sheet.sequences[&3].duration_seconds, 2.0);
        assert_eq!(sheet.sequences[&3].frames[0].duration_seconds, 2.0);
        for length in 0..bytes.len() { assert!(decode_particle_sheet_data(&bytes[..length]).is_err()); }
    }

    #[test]
    fn executable_particle_roots_publish_once_including_projectile_unlocks() {
        let graph = playsrc_entity::parse(b"{\"classname\"\"worldspawn\"}\0", playsrc_entity::Limits::default()).unwrap();
        let roots = playsrc_tf2::particle_resources::roots(&graph).into_iter().map(playsrc_particle::DefinitionLookup::Name).collect::<Vec<_>>();
        let mut names = std::collections::BTreeSet::new();
        for root in &roots { let playsrc_particle::DefinitionLookup::Name(name) = root else { panic!("named native root"); }; assert!(names.insert(*name), "duplicate {name}"); }
        for name in playsrc_tf2::projectile_weapon::PARTICLE_ROOTS { assert!(names.contains(name)); }
        assert!(names.contains("muzzle_revolver"));
        let mut bytes = Vec::new();
        encode_particle_textures(&mut bytes, &BTreeMap::new(), &roots).unwrap();
        assert_eq!(&bytes[..4], b"PPTM");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize, roots.len());
        let mut reader = ParticleReader { bytes: &bytes, at: 16 };
        for root in roots { let playsrc_particle::DefinitionLookup::Name(name) = root else { unreachable!() }; assert_eq!(reader.text().unwrap(), name); }
        assert_eq!(reader.at, bytes.len());
    }

    #[test]
    fn particle_color_control_points_keep_sparse_indices_and_no_entity_binding() {
        for index in [9, 10] {
            let mut bytes = particle_stop_transaction(0);
            bytes.truncate(bytes.len() - 8);
            bytes[32] = 2;
            bytes[34] = index;
            for value in [0.72_f32, 0.22, 0.23, 0.0, 0.0, 0.0, 1.0] { bytes.extend_from_slice(&value.to_le_bytes()); }
            bytes.extend_from_slice(&u32::MAX.to_le_bytes());
            bytes.extend_from_slice(&[0; 8]);
            let (events, _, _, _, _) = decode_particle_transaction(&bytes).unwrap();
            let playsrc_particle::EventCommand::SetControlPoint { control_point, .. } = &events[0].command else { panic!("expected sparse control point"); };
            assert_eq!(control_point.index, index);
            assert_eq!(control_point.position, [0.72, 0.22, 0.23]);
            assert_eq!(control_point.object_identity, None);
            bytes[34] = 31;
            assert!(decode_particle_transaction(&bytes).is_err());
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
        bytes.extend_from_slice(&15_u32.to_le_bytes());
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
        let (_, direction, ratio) = playsrc_map::source_world_light_ray([0.0; 3], &light).unwrap();
        assert_eq!(direction, [1.0, 0.0, 0.0]);
        assert_eq!(ratio, 1.0 / 4097.0);
        light.kind = 2;
        assert_eq!(
            playsrc_map::source_world_light_angle(&light, direction).unwrap(),
            1.0
        );
        light.normal = [1.0, 0.0, 0.0];
        assert_eq!(
            playsrc_map::source_world_light_angle(&light, direction).unwrap(),
            0.0
        );
        light.kind = 1;
        light.radius = 1.0;
        assert_eq!(
            playsrc_map::source_world_light_ray([0.0; 3], &light)
                .unwrap()
                .2,
            0.0
        );
        assert_eq!(
            playsrc_map::source_lightcache_bounds([-0.5, 32.0, -128.5]),
            ([-32.0, 32.0, -256.0], [0.0, 64.0, -128.0])
        );
    }

    #[test]
    fn tf2_eye_targets_preserve_the_authored_forward_look_distance() {
        let request = ModelPoseLightingRequest {
            origin: [10.0, 20.0, 30.0],
            angles: [0.0, 90.0, 0.0],
            camera: [0.0; 3],
            camera_angles: [0.0; 3],
        };
        let target = source_tf2_eye_target(0x6000_0002, request, None);
        assert!((target[0] - 10.0).abs() < 0.0001);
        assert!((target[1] - 532.0).abs() < 0.0001);
        assert_eq!(target[2], 30.0);
    }

    #[test]
    fn tf2_viewmodels_use_owner_collision_center_instead_of_camera_or_model_center() {
        let class = playsrc_tf2::PlayerClass::Soldier;
        let policy = playsrc_tf2::MovementPolicy {
            class,
            modifiers: playsrc_tf2::MovementModifiers::default(),
        }
        .resolve();
        for (crouched, expected) in [(false, 41.0), (true, 31.0)] {
            let movement = playsrc_movement::State::from_player(
                playsrc_movement::Player {
                    position: [10.0, 20.0, 30.0],
                    velocity: [0.0; 3],
                    grounded: true,
                    crouched,
                    jump_latched: false,
                },
                policy,
            );
            assert_eq!(
                source_tf2_viewmodel_lighting_origin(movement, class),
                [10.0, 20.0, 30.0 + expected],
            );
        }
    }
    #[test]
    fn sky_texture_roles_select_explicit_render_encoding() {
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::Base], playsrc_vtf::ImageFormat::Dxt1),
            Some(playsrc_map::SkyEncoding::Srgb)
        );
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::HdrBase], playsrc_vtf::ImageFormat::Rgba16F),
            Some(playsrc_map::SkyEncoding::Linear)
        );
        assert_eq!(
            selected_sky_encoding(&[playsrc_material::TextureRole::HdrCompressed], playsrc_vtf::ImageFormat::Dxt5),
            Some(playsrc_map::SkyEncoding::HdrRgbs)
        );
        assert_eq!(
            selected_sky_encoding(&[
                playsrc_material::TextureRole::HdrCompressed0,
                playsrc_material::TextureRole::HdrCompressed1,
                playsrc_material::TextureRole::HdrCompressed2,
            ], playsrc_vtf::ImageFormat::Dxt1),
            None
        );
        assert_eq!(selected_sky_encoding(&[playsrc_material::TextureRole::HdrBase], playsrc_vtf::ImageFormat::Dxt1), Some(playsrc_map::SkyEncoding::Srgb));
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
            legacy_visuals: legacy_visuals::Runtime::default(),
            generation: 1,
            payload: Some(vec![1, 2]),
            payload_bytes: 2,
            presentation: vec![7, 8, 9],
            presentation_bytes: 3,
            coverage: Vec::new(),
            particles: None,
            map_particles: None,
            smokestacks: None,
            legacy_visual_output: Vec::new(),
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            combat_decals: None,
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_lighting_metadata: BTreeMap::new(),
            model_lighting_world: None,
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            weapon_animations: BTreeMap::new(),
            class_scenes: BTreeMap::new(),
            wearable_particles: wearable::ParticleStates::default(),
            model_output: vec![0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0xc0, 0x7f],
            visibility: None,
            soundscapes: Default::default(),
            acoustic_scene: None,
            acoustic_output: Vec::new(),
            visibility_candidates: None,
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
            snapshot: Arc::from([]),
            compile_metrics: [0; 17],
            memory_metrics: [0; 13],
            texture_inspections: [0; 2],
            model_cache: [0; 2],
        });
        drop(guard);
        let old = encode(0, 1);
        reply_output::tests::assert_reply_ownership(old);
        assert_eq!(playsrc_model_output_length(old), 8);
        let capacity = playsrc_model_output_capacity(old);
        let model = playsrc_model_output_take(old);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(model, 8) },
            &[0x00, 0x00, 0x00, 0x80, 0x01, 0x00, 0xc0, 0x7f]
        );
        assert_eq!(playsrc_model_output_length(old), 0);
        assert!(playsrc_model_output_take(old).is_null());
        unsafe { playsrc_model_output_recycle(old, model, capacity) };
        assert_eq!(playsrc_model_output_capacity(old), capacity);
        assert_eq!(playsrc_model_output_length(old), 0);
        {
            let mut guard = slots().lock().unwrap();
            guard[0].model_output.extend_from_slice(&[1, 2, 3]);
        }
        let reused = playsrc_model_output_take(old);
        assert_eq!(reused, model);
        assert_eq!(unsafe { std::slice::from_raw_parts(reused, 3) }, &[1, 2, 3]);
        // A lease may finish after map replacement; stale owners may only free it.
        unsafe { playsrc_model_output_recycle(encode(0, 2), reused, capacity) };
        assert_eq!(playsrc_model_output_capacity(old), 0);
        assert_eq!(playsrc_result_length(old), 2);
        assert_eq!(playsrc_presentation_length(old), 3);
        let presentation = playsrc_presentation_take(old);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(presentation, 3) },
            &[7, 8, 9]
        );
        assert_eq!(playsrc_presentation_length(old), 0);
        unsafe { playsrc_free(presentation, 3) };
        let payload = playsrc_result_take(old);
        assert_eq!(unsafe { std::slice::from_raw_parts(payload, 2) }, &[1, 2]);
        assert_eq!(playsrc_result_length(old), 0);
        unsafe { playsrc_free(payload, 2) };
        // Identity-only output still owns a live generation, but no byte lease.
        slots().lock().unwrap()[0].payload_bytes = 123;
        assert_eq!(playsrc_result_length(old), 123);
        assert!(playsrc_result_take(old).is_null());
        let mut sentinel = [0xa5; 4];
        assert_eq!(unsafe { playsrc_result_copy(old, sentinel.as_mut_ptr(), sentinel.len()) }, 0);
        assert_eq!(sentinel, [0xa5; 4]);
        assert_eq!(playsrc_result_release(old), 1);
        assert_eq!(playsrc_result_length(old), 0);
        let mut hash = [0; 32];
        assert_eq!(unsafe { playsrc_result_hash(old, hash.as_mut_ptr()) }, 1);
        assert_eq!(hash, [3; 32]);
        assert_eq!(playsrc_dispose(old), 1);
        let mut guard = slots().lock().unwrap();
        guard[0] = Slot {
            legacy_visuals: legacy_visuals::Runtime::default(),
            generation: 2,
            payload: Some(vec![4]),
            payload_bytes: 1,
            presentation: Vec::new(),
            presentation_bytes: 0,
            coverage: Vec::new(),
            particles: None,
            map_particles: None,
            smokestacks: None,
            legacy_visual_output: Vec::new(),
            particle_materials: Vec::new(),
            particle_sheets: BTreeMap::new(),
            combat_decals: None,
            particle_output: Vec::new(),
            studio_models: BTreeMap::new(),
            model_lighting_metadata: BTreeMap::new(),
            model_lighting_world: None,
            model_material_opacity: BTreeMap::new(),
            viewmodel_bob: BTreeMap::new(),
            weapon_animations: BTreeMap::new(),
            class_scenes: BTreeMap::new(),
            wearable_particles: wearable::ParticleStates::default(),
            model_output: Vec::new(),
            visibility: None,
            soundscapes: Default::default(),
            acoustic_scene: None,
            acoustic_output: Vec::new(),
            visibility_candidates: None,
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
            snapshot: Arc::from([]),
            compile_metrics: [0; 17],
            memory_metrics: [0; 13],
            texture_inspections: [0; 2],
            model_cache: [0; 2],
        };
        drop(guard);
        assert_eq!(playsrc_result_length(old), 0);
        assert_eq!(playsrc_result_length(encode(0, 2)), 1);
        gameplay_replay::tests::assert_mutations(encode(0, 2));
        assert_eq!(playsrc_dispose(encode(0, 2)), 1);
        let mut header = vec![0; playsrc_bsp::HEADER_BYTES];
        header[..4].copy_from_slice(b"VBSP");
        header[4..8].copy_from_slice(&20_i32.to_le_bytes());
        let invalid_presentation = b"PTF2";
        for retain in [0, 1] {
            for (source, profile, expected) in [(b"".as_slice(), 3, 1), (header.as_slice(), 3, 2), (header.as_slice(), 0, 7)] {
                for cached in [false, true] {
                    let handle = unsafe {
                        if cached {
                            playsrc_compile_map_cached(source.as_ptr(), source.len(), profile, std::ptr::null(), 0, std::ptr::null(), invalid_presentation.as_ptr(), invalid_presentation.len(), retain)
                        } else {
                            playsrc_compile_map(source.as_ptr(), source.len(), profile, std::ptr::null(), 0, std::ptr::null(), retain)
                        }
                    };
                    assert_eq!(playsrc_result_error(handle), expected);
                    assert_eq!(playsrc_result_length(handle), 0);
                    assert_eq!(playsrc_dispose(handle), 1);
                }
            }
        }
    }

    #[test]
    fn command_and_snapshot_binary_contract_is_stable() {
        let mut bytes = vec![0; 84];
        bytes[..4].copy_from_slice(b"PCMD");
        bytes[4..8].copy_from_slice(&9_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&240_f32.to_le_bytes());
        bytes[16..20].copy_from_slice(&100_f32.to_le_bytes());
        bytes[24..28].copy_from_slice(&(-30_f32).to_le_bytes());
        bytes[28..32].copy_from_slice(&0xad_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x0202_0304_u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&77_u32.to_le_bytes());
        bytes[48..52].copy_from_slice(&84_u32.to_le_bytes());
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
        for flags in 0..4 {
            bytes[57] = 0x80 | flags;
            assert_eq!(gameplay_protocol::decode(&bytes).unwrap().command.weapon_preferences,
                Some(playsrc_tf2::WeaponPreferences { remember_active: flags & 1 != 0, remember_last: flags & 2 != 0 }));
        }
        for invalid in [1, 4, 0x84, 0xff] { bytes[57] = invalid; assert!(gameplay_protocol::decode(&bytes).is_none()); }
        bytes[57] = 0;
        bytes[52..56].copy_from_slice(&(0x8000_0000_u32 | 1 | (1 << 16)).to_le_bytes());
        assert_eq!(
            gameplay_protocol::decode(&bytes)
                .unwrap()
                .command
                .objective_configuration,
            Some(playsrc_tf2::ctf::RuleConfiguration {
                captures_per_round: 1,
                return_on_touch: true,
            })
        );
        for invalid in [1_u32, 0x8002_0000_u32] {
            bytes[52..56].copy_from_slice(&invalid.to_le_bytes());
            assert!(gameplay_protocol::decode(&bytes).is_none());
        }
        bytes[52..56].fill(0);
        bytes[56] = 1;
        bytes[60..64].copy_from_slice(&4_u32.to_le_bytes());
        for (index, value) in [10_f32, 20.0, 30.0, -5.0, 90.0].into_iter().enumerate() {
            bytes[64 + index * 4..68 + index * 4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            gameplay_protocol::decode(&bytes)
                .unwrap()
                .command
                .bot_control,
            Some(playsrc_tf2::bot::Control::Teleport {
                identity: 4,
                position: [10.0, 20.0, 30.0],
                pitch_degrees: -5.0,
                yaw_degrees: 90.0,
            })
        );
        bytes[56..84].fill(0);
        bytes[44..48].copy_from_slice(
            &(0x8000_0000_u32 | 7 | (24 << 6) | (2 << 12) | (1 << 14) | (1 << 16) | (1 << 18))
                .to_le_bytes(),
        );
        assert_eq!(
            gameplay_protocol::decode(&bytes)
                .unwrap()
                .command
                .bot_configuration,
            Some(playsrc_tf2::bot::Configuration {
                quota: 7,
                maximum_players: 24,
                mode: playsrc_tf2::bot::QuotaMode::Fill,
                difficulty: playsrc_tf2::bot::Difficulty::Hard,
                join_after_player: true,
                auto_vacate: false,
                offline_practice: true,
            })
        );
        for invalid in [
            0x8000_0000_u32,
            0x8000_0000 | 32 | (24 << 6),
            0x8000_0000 | (24 << 6) | (3 << 14),
        ] {
            bytes[44..48].copy_from_slice(&invalid.to_le_bytes());
            assert!(gameplay_protocol::decode(&bytes).is_none());
        }
        bytes[44..48].fill(0);
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
            air_burst: false,
            underwater_explosion: false,
            model_visible: true,
            trail: playsrc_tf2::ProjectileTrail::Standard,
            mini_rocket: false,
            practice_explosion: false,
            source_weapon: None,
            launcher_source: None,
            launcher_weapon: playsrc_tf2::Weapon::Original,
            weapon: playsrc_tf2::Weapon::Original,
            critical: false,
            damage: 90.0,
            deflected: false,
            original_owner_identity: 1,
            self_blast_only: false,
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
            view_angle_offset: [0.0; 3],
            weapon_crosshair_scale: 1.0,
            equipped_items: Vec::new(),
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
                minigun_state: playsrc_tf2::weapon::MinigunState::Idle,
                prefire_playback_rate: 1.0,
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
                air_burst: false,
                underwater_explosion: false,
                trail: playsrc_tf2::ProjectileTrail::Standard,
                mini_rocket: false,
                practice_explosion: false,
                weapon: playsrc_tf2::Weapon::Original,
                critical: false,
                self_blast_only: false,
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
            control_points: None,
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
            pickups: Vec::new(),
            metal: 100,
            scoreboard: playsrc_tf2::scoreboard::Snapshot::default(),
            buildings: Vec::new(),
            placement: None,
            medigun_charge: 0.0,
            medigun_target: None,
            medigun_releasing: false,
        };
        let producer = playsrc_tf2::ProducerSnapshot {
            decapitations: 800,
            revenge_crits: 35,
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
                hitscan: playsrc_tf2::hitscan::State::default(),
                deploy_multiplier: 1.0,
                spinup_seconds: 0.75,
                postfire_until: None,
                discard_chambered_on_reload: false,
                generation: 0,
                critical: playsrc_tf2::critical::WeaponState::default(),
                last_flare_deny_time: 0.0,
                last_extinguish_time: 0.0,
                resolved_profile: playsrc_tf2::weapon::WeaponProfile::configured(playsrc_tf2::Weapon::Original),
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
                push_due_time: None,
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
                configured_available: [0; 64],
                projectile_unlock_available: [7, 7, 7, 7, 15, 7],
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
                bonesaw_hit_flesh_available: 7,
                bonesaw_hit_world_available: 3,
                overtime_available: 15,
                control_point_available: 0xffff,
                payload_warning_available: [0x3ff;2],
            },
        };
        let mut collision_snapshot = b"CSNP".to_vec();
        collision_snapshot.extend_from_slice(&1_u32.to_le_bytes());
        collision_snapshot.extend_from_slice(&9_u64.to_le_bytes());
        collision_snapshot.extend_from_slice(&0_u32.to_le_bytes());
        let entity_presentation = playsrc_tf2::EntityPresentationSnapshot {
            collision_revision: 9,
            studio_models: Vec::new(),
            studio_animations: Vec::new(),
            entities: playsrc_entity::BrushModelPresentation {
                source_identity: 1,
                registry_identity: 2,
                tick: 9,
                revision: 1,
                models: Vec::new(),
            },
        };
        let (encoded, metrics) = encoding_allocations::measure(|| Arc::<[u8]>::from(encode_snapshot(
            &snapshot,
            &producer,
            2,
            None,
            SnapshotExtensions {
                random_state,
                random_draws: &[],
                audio_events: &[],
                soundscape: playsrc_entity::soundscape::Selection {
                    entity: 9, soundscape: 7, position_bits: 0x81, positions: [[1.0, 2.0, 3.0]; 8],
                },
                rocket_results: &[],
                mover_results: &[],
                collision_snapshot: &collision_snapshot,
                entity_presentation: &entity_presentation,
                payload_constraint_blocked: false,
                combat_decals: &[],
            },
        )
        .unwrap()));
        eprintln!("snapshot encoding: bytes={} {metrics:?} sha256={:x}", encoded.len(), Sha256::digest(&encoded));
        // Two ten-wave masks add four wire bytes without another allocation.
        assert!(metrics.requests <= 10 && metrics.bytes <= 5376, "snapshot encoder retains redundant staging/growth");
        assert_eq!(metrics.live, 1288);
        let expected_hash = "c3e218d905c914634d2025e7f000776ad34fcd1799f3a1709fb8fcb13e7c44b5";
        assert_eq!(format!("{:x}", Sha256::digest(&encoded)), expected_hash);
        assert_eq!(&encoded[..8], b"PSSN\x1e\0\0\0");
        assert_eq!(encoded.len(), 1268);
        assert_eq!(&encoded[1132..1148], &[0; 16]);
        assert_eq!(i32::from_le_bytes(encoded[1148..1152].try_into().unwrap()), 800);
        assert_eq!(i32::from_le_bytes(encoded[1152..1156].try_into().unwrap()), 35);
        assert_eq!(f32::from_le_bytes(encoded[1156..1160].try_into().unwrap()), 1.0);
        assert_eq!(&encoded[1016..1020], b"PCPN");
        assert_eq!(&encoded[1028..1036], b"PCTF\x01\0\0\0");
        assert_eq!(&encoded[1064..1072], b"PGRL\x04\0\0\0");
        assert_eq!(&encoded[1160..1172], &[9, 0, 0, 0, 7, 0, 0, 0, 0x81, 0, 0, 0]);
        for position in encoded[1172..].chunks_exact(12) {
            assert_eq!(f32::from_le_bytes(position[8..12].try_into().unwrap()), 3.0);
        }
        assert_eq!(
            u32::from_le_bytes(encoded[160..164].try_into().unwrap()),
            playsrc_tf2::FL_CLIENT | playsrc_tf2::FL_INWATER
        );
        assert_eq!(u32::from_le_bytes(encoded[56..60].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[60..64].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(encoded[64..68].try_into().unwrap()), 1);
        assert_eq!(&encoded[324..328], &[12, 0, 0, 0]);
        assert_eq!(&encoded[328..332], &[1, 3, 17, 32]);
        assert_eq!(&encoded[408..412], &[22, 1, 3, 0]);
        assert_eq!(&encoded[544..552], &[1, 1, 0, 0, 2, 1, 0, 0]);
        assert_eq!(&encoded[552..560], b"PRNG\x04\0\0\0");
        assert_eq!(&encoded[552 + 293..552 + 296], &[15, 255, 255]);
        assert_eq!(&encoded[552 + 360..552 + 364], &[255, 255, 7, 0]);
        assert_eq!(&encoded[552 + 364..552 + 368], &[255, 3, 255, 3]);

        let constrained = encode_snapshot(
            &snapshot,
            &producer,
            2,
            None,
            SnapshotExtensions {
                random_state,
                random_draws: &[],
                audio_events: &[],
                soundscape: Default::default(),
                rocket_results: &[],
                mover_results: &[],
                collision_snapshot: &collision_snapshot,
                entity_presentation: &entity_presentation,
                payload_constraint_blocked: true,
                combat_decals: &[],
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
        assert_eq!(&constrained[556..564], b"PRNG\x04\0\0\0");
        assert_eq!(constrained.len(), encoded.len() + 4);

        // Exercise growth boundaries while holding every older immutable lease.
        // Collision bytes are an opaque, already-authenticated section here.
        let mut leases = Vec::new();
        let collision_at=552+368;
        for size in [0, 1, 4095, 65536, 1024 * 1024] {
            let collision = vec![0xa5; size];
            let (lease, metrics) = encoding_allocations::measure(|| Arc::<[u8]>::from(encode_snapshot(
                &snapshot, &producer, 2, None, SnapshotExtensions {
                    random_state, random_draws: &[], audio_events: &[], soundscape: playsrc_entity::soundscape::Selection {
                        entity: 9, soundscape: 7, position_bits: 0x81, positions: [[1.0, 2.0, 3.0]; 8],
                    }, rocket_results: &[], mover_results: &[],
                    collision_snapshot: &collision, entity_presentation: &entity_presentation,
                    payload_constraint_blocked: false, combat_decals: &[],
                }).unwrap()));
            assert_eq!(lease.len(), encoded.len() - collision_snapshot.len() + size);
            assert_eq!(u32::from_le_bytes(lease[144..148].try_into().unwrap()) as usize, size);
            assert_eq!(&lease[collision_at..collision_at + size], collision);
            assert_eq!(&lease[..144], &encoded[..144]);
            assert_eq!(&lease[148..collision_at], &encoded[148..collision_at]);
            assert_eq!(&lease[collision_at + size..], &encoded[collision_at + collision_snapshot.len()..]);
            assert!(metrics.requests <= 11, "section staging must not return: {metrics:?}");
            assert!(metrics.bytes <= 2 * lease.len() + 4112, "Collision insertion must not regrow the full payload: {metrics:?}");
            assert!(metrics.peak <= (2 * lease.len() + 1024) as isize, "redundant payload owners: {metrics:?}");
            assert_eq!(metrics.live as usize, (lease.len() + 16).next_multiple_of(8));
            eprintln!("snapshot collision_bytes={size} output_bytes={} {metrics:?}", lease.len());
            leases.push(lease);
        }
        assert_eq!(format!("{:x}", Sha256::digest(&encoded)), expected_hash);
        for (lease, size) in leases.iter().zip([0, 1, 4095, 65536, 1024 * 1024]) {
            assert!(lease[collision_at..collision_at + size].iter().all(|byte| *byte == 0xa5));
        }
        let oversized = vec![0; 64 * 1024 * 1024];
        let (failed, metrics) = encoding_allocations::measure(|| encode_snapshot(
            &snapshot, &producer, 2, None, SnapshotExtensions {
                random_state, random_draws: &[], audio_events: &[], soundscape: playsrc_entity::soundscape::Selection {
                    entity: 9, soundscape: 7, position_bits: 0x81, positions: [[1.0, 2.0, 3.0]; 8],
                }, rocket_results: &[], mover_results: &[],
                collision_snapshot: &oversized, entity_presentation: &entity_presentation,
                payload_constraint_blocked: false, combat_decals: &[],
            }));
        assert!(failed.is_none()); assert_eq!(metrics.live, 0);
        assert_eq!(format!("{:x}", Sha256::digest(&encoded)), expected_hash);
    }

    #[test]
    fn fixed_tick_continuation_retains_buttons_and_consumes_results_and_selectors() {
        let mut bytes = vec![0; 84 + 80];
        bytes[..4].copy_from_slice(b"PCMD");
        bytes[4..8].copy_from_slice(&9_u32.to_le_bytes());
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
        bytes[48..52].copy_from_slice(&byte_length.to_le_bytes());
        let continued = continuation_command(&bytes).unwrap();
        assert_eq!(continued.len(), 84);
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
        assert_eq!(u32::from_le_bytes(continued[44..48].try_into().unwrap()), 0);
        assert_eq!(
            u32::from_le_bytes(continued[48..52].try_into().unwrap()),
            84
        );
        assert!(continued[52..84].iter().all(|value| *value == 0));
        bytes[43] |= 0x80;
        let stopped = continuation_command(&bytes).unwrap();
        assert_eq!(
            u16::from_le_bytes(stopped[42..44].try_into().unwrap()),
            0x8000
        );
        let stopped_command = gameplay_protocol::decode(&stopped).unwrap().command;
        assert!(stopped_command.nextbot_stop);
        assert!(stopped_command.bot_request.is_none());
    }
}
