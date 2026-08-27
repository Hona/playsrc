use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{
    Definition, DefinitionLookup, Error, ErrorCode, Function, FunctionCategory, ParticleSheet,
    Registry, SheetSample, SheetSampleRequest, Value, sample_sheet,
    source_random::SOURCE_RANDOM_FLOAT_BITS,
};

const CONTROL_POINT_LIMIT: usize = 64;
const DEFAULT_STEP_SECONDS: f32 = 0.05;
const OUTPUT_HEADER_BYTES: usize = 40;
const OUTPUT_RECORD_BYTES: usize = 436;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldLimits {
    pub max_effects: usize,
    pub max_systems: usize,
    pub max_particles_per_system: usize,
    pub max_particles_total: usize,
    pub max_control_points: usize,
    pub max_events_per_advance: usize,
    pub max_substeps: usize,
    pub max_queries_per_advance: usize,
    pub max_render_items: usize,
}

impl Default for WorldLimits {
    fn default() -> Self {
        Self {
            max_effects: 4_096,
            max_systems: 16_384,
            max_particles_per_system: 5_000,
            max_particles_total: 65_536,
            max_control_points: CONTROL_POINT_LIMIT,
            max_events_per_advance: 4_096,
            max_substeps: 256,
            max_queries_per_advance: 65_536,
            max_render_items: 65_536,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlPoint {
    pub index: u8,
    pub position: [f32; 3],
    pub previous_position: [f32; 3],
    pub orientation: [f32; 4],
    pub velocity: [f32; 3],
    pub radius: f32,
    pub density: f32,
    pub duration: f32,
    pub parent: Option<u8>,
    pub object_identity: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopMode {
    Graceful,
    Immediate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventCommand {
    Create {
        effect_identity: u32,
        definition: String,
        seed: u64,
        owner_identity: Option<u32>,
        control_points: Vec<ControlPoint>,
    },
    Replace {
        effect_identity: u32,
        definition: String,
        seed: u64,
        owner_identity: Option<u32>,
        control_points: Vec<ControlPoint>,
    },
    SetControlPoint {
        effect_identity: u32,
        control_point: ControlPoint,
    },
    StartEmission {
        effect_identity: u32,
    },
    StopEmission {
        effect_identity: u32,
        mode: StopMode,
    },
    Restart {
        effect_identity: u32,
    },
    SetDormant {
        effect_identity: u32,
        dormant: bool,
    },
    Destroy {
        effect_identity: u32,
    },
    Reset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub identity: u64,
    pub timestamp_seconds: f32,
    pub source_order: u32,
    pub command: EventCommand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvanceRequest {
    pub from_seconds: f32,
    pub to_seconds: f32,
    pub maximum_step_seconds: f32,
    pub camera_position: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Primitive {
    Sprite,
    Trail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticleMaterialShader {
    SpriteCard,
    MeshSprite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ParticleBlendFactor {
    Zero = 0,
    One = 1,
    SourceAlpha = 2,
    OneMinusSourceAlpha = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticleBlendState {
    pub source: ParticleBlendFactor,
    pub destination: ParticleBlendFactor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticleColorSpace {
    SrgbTextureLinearTint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleMaterial {
    pub shader: ParticleMaterialShader,
    pub blend: ParticleBlendState,
    pub color_space: ParticleColorSpace,
    pub dual_sequence: bool,
    pub sheet: ParticleSheet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticleMaterialState {
    pub shader: ParticleMaterialShader,
    pub blend: ParticleBlendState,
    pub color_space: ParticleColorSpace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderItem {
    pub sky: bool,
    pub effect_identity: u32,
    pub system_uuid: [u8; 16],
    pub particle_identity: u32,
    pub renderer_index: u16,
    pub primitive: Primitive,
    pub material: String,
    pub position: [f32; 3],
    pub previous_position: [f32; 3],
    pub radius: f32,
    pub roll_radians: f32,
    pub yaw_radians: f32,
    pub color: [u8; 3],
    pub opacity: f32,
    pub sequence: i32,
    pub secondary_sequence: i32,
    pub trail_length_scale: f32,
    pub sort_key: f32,
    pub age_seconds: f32,
    pub lifetime_seconds: f32,
    pub animation_rate: f32,
    pub secondary_animation_rate: f32,
    pub step_seconds: f32,
    pub trail_min_length: f32,
    pub trail_max_length: f32,
    pub trail_fade_in_seconds: f32,
    pub orientation_type: i32,
    pub animation_fit_lifetime: bool,
    pub animation_rate_as_fps: bool,
    pub primary_sheet: Option<SheetSample>,
    pub secondary_sheet: Option<SheetSample>,
    pub trail_end_position: [f32; 3],
    pub trail_width: f32,
    pub trail_length: f32,
    pub material_state: Option<ParticleMaterialState>,
    pub stable_tie_identity: u64,
}

pub fn resolve_render_output(
    items: Vec<RenderItem>,
    materials: &BTreeMap<String, ParticleMaterial>,
) -> Result<Vec<RenderItem>, Error> {
    let mut output = items;
    let mut retained = 0;
    for index in 0..output.len() {
        let item = &mut output[index];
        let material = materials.get(&item.material).ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDependency,
                "particle-output",
                0,
                format!("particle material {} is missing", item.material),
            )
        })?;
        if item.primitive == Primitive::Trail
            && material.shader == ParticleMaterialShader::SpriteCard
        {
            continue;
        }
        if material.shader == ParticleMaterialShader::MeshSprite && item.opacity == 0.0 {
            continue;
        }
        let fit_lifetime =
            item.animation_fit_lifetime && material.shader == ParticleMaterialShader::MeshSprite;
        item.primary_sheet = Some(sample_sheet(
            &material.sheet,
            SheetSampleRequest {
                sequence: item.sequence,
                age_seconds: item.age_seconds,
                lifetime_seconds: item.lifetime_seconds,
                animation_rate: item.animation_rate,
                fit_lifetime,
                animation_rate_as_fps: item.animation_rate_as_fps,
            },
        )?);
        item.secondary_sheet = material
            .dual_sequence
            .then(|| {
                sample_sheet(
                    &material.sheet,
                    SheetSampleRequest {
                        sequence: item.secondary_sequence,
                        age_seconds: item.age_seconds,
                        lifetime_seconds: item.lifetime_seconds,
                        animation_rate: item.secondary_animation_rate,
                        fit_lifetime: false,
                        animation_rate_as_fps: false,
                    },
                )
            })
            .transpose()?;
        if item.primitive == Primitive::Trail {
            let delta = sub(item.previous_position, item.position);
            let movement = length_squared(delta).sqrt();
            let fade = if item.age_seconds >= item.trail_fade_in_seconds {
                1.0
            } else if item.trail_fade_in_seconds > 0.0 {
                item.age_seconds / item.trail_fade_in_seconds
            } else {
                1.0
            };
            let raw_length = fade * movement / item.step_seconds * item.trail_length_scale;
            if raw_length <= 0.0 {
                continue;
            }
            let length = source_clamp(raw_length, item.trail_min_length, item.trail_max_length);
            item.trail_length = length;
            item.trail_width = item.radius.min(length);
            item.trail_end_position = add(
                item.position,
                mul(normalize(delta).unwrap_or([0.0; 3]), length),
            );
        }
        item.material_state = Some(ParticleMaterialState {
            shader: material.shader,
            blend: material.blend,
            color_space: material.color_space,
        });
        output.swap(retained, index);
        retained += 1;
    }
    output.truncate(retained);
    Ok(output)
}

pub fn encode_render_output(
    items: &[RenderItem],
    bounds: Option<Bounds>,
    materials: &[String],
    max_bytes: usize,
) -> Result<Vec<u8>, Error> {
    let length = OUTPUT_HEADER_BYTES
        .checked_add(
            items
                .len()
                .checked_mul(OUTPUT_RECORD_BYTES)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::BoundExceeded,
                        "particle-output",
                        0,
                        "output length overflowed",
                    )
                })?,
        )
        .ok_or_else(|| {
            Error::new(
                ErrorCode::BoundExceeded,
                "particle-output",
                0,
                "output length overflowed",
            )
        })?;
    if length > max_bytes || items.len() > u32::MAX as usize {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-output",
            0,
            "render output exceeds its byte or count limit",
        ));
    }
    if materials
        .len()
        .checked_sub(1)
        .is_some_and(|index| u32::try_from(index).is_err())
    {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-output",
            0,
            "material index exceeds u32",
        ));
    }
    let sorted_materials = materials.windows(2).all(|pair| pair[0] <= pair[1]);
    let mut bytes = vec![0; length];
    bytes[0..4].copy_from_slice(&0x5250_5350_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&4_u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&(items.len() as u32).to_le_bytes());
    if let Some(bounds) = bounds {
        if !finite(&bounds.minimum)
            || !finite(&bounds.maximum)
            || (0..3).any(|component| bounds.minimum[component] > bounds.maximum[component])
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "particle-output",
                0,
                "particle bounds are invalid",
            ));
        }
        bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        put_vector(&mut bytes, 16, bounds.minimum);
        put_vector(&mut bytes, 28, bounds.maximum);
    }
    for (index, item) in items.iter().enumerate() {
        if !finite(&item.position)
            || !finite(&item.previous_position)
            || !finite(&item.trail_end_position)
            || !finite(&[
                item.radius,
                item.roll_radians,
                item.yaw_radians,
                item.opacity,
                item.trail_length_scale,
                item.trail_length,
                item.trail_width,
                item.sort_key,
                item.age_seconds,
                item.lifetime_seconds,
                item.animation_rate,
                item.secondary_animation_rate,
                item.step_seconds,
                item.trail_min_length,
                item.trail_max_length,
                item.trail_fade_in_seconds,
            ])
            || item.radius < 0.0
            || !(0.0..=1.0).contains(&item.opacity)
            || item.trail_length_scale < 0.0
            || item.trail_length < 0.0
            || item.trail_width < 0.0
            || item.age_seconds < 0.0
            || item.lifetime_seconds <= 0.0
            || item.step_seconds <= 0.0
            || item.trail_min_length < 0.0
            || item.trail_max_length < 0.0
            || item.trail_fade_in_seconds < 0.0
            || !valid_sheet_sample(item.primary_sheet.as_ref())
            || !valid_sheet_sample(item.secondary_sheet.as_ref())
            || item.primary_sheet.is_none()
            || item.material_state.is_none()
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "particle-output",
                index,
                "render item contains an invalid scalar",
            ));
        }
        let material = if sorted_materials {
            materials
                .partition_point(|identity| identity.as_str() <= item.material.as_str())
                .checked_sub(1)
                .filter(|index| materials[*index] == item.material)
        } else {
            materials
                .iter()
                .rposition(|identity| identity == &item.material)
        }
        .ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDependency,
                "particle-output",
                index,
                "render item material is absent from the supplied registry",
            )
        })? as u32;
        let offset = OUTPUT_HEADER_BYTES + index * OUTPUT_RECORD_BYTES;
        bytes[offset..offset + 4].copy_from_slice(&(index as u32 + 1).to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&item.effect_identity.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&item.particle_identity.to_le_bytes());
        bytes[offset + 12..offset + 14].copy_from_slice(&item.renderer_index.to_le_bytes());
        bytes[offset + 14] = match item.primitive {
            Primitive::Sprite => 0,
            Primitive::Trail => 1,
        };
        bytes[offset + 15] = u8::from(item.sky);
        bytes[offset + 16..offset + 32].copy_from_slice(&item.system_uuid);
        bytes[offset + 32..offset + 36].copy_from_slice(&material.to_le_bytes());
        put_vector(&mut bytes, offset + 36, item.position);
        put_vector(&mut bytes, offset + 48, item.previous_position);
        bytes[offset + 60..offset + 64].copy_from_slice(&item.radius.to_le_bytes());
        bytes[offset + 64..offset + 68].copy_from_slice(&item.roll_radians.to_le_bytes());
        let color = u32::from(item.color[0]) << 16
            | u32::from(item.color[1]) << 8
            | u32::from(item.color[2]);
        bytes[offset + 68..offset + 72].copy_from_slice(&color.to_le_bytes());
        bytes[offset + 72..offset + 76].copy_from_slice(&item.opacity.to_le_bytes());
        bytes[offset + 76..offset + 80].copy_from_slice(&item.sequence.to_le_bytes());
        bytes[offset + 80..offset + 84].copy_from_slice(&item.trail_length.to_le_bytes());
        bytes[offset + 84..offset + 88].copy_from_slice(&item.sort_key.to_le_bytes());
        bytes[offset + 88..offset + 92].copy_from_slice(&item.age_seconds.to_le_bytes());
        bytes[offset + 92..offset + 96].copy_from_slice(&item.lifetime_seconds.to_le_bytes());
        bytes[offset + 96..offset + 100].copy_from_slice(&item.animation_rate.to_le_bytes());
        bytes[offset + 100..offset + 104].copy_from_slice(&item.trail_min_length.to_le_bytes());
        bytes[offset + 104..offset + 108].copy_from_slice(&item.trail_max_length.to_le_bytes());
        bytes[offset + 108..offset + 112]
            .copy_from_slice(&item.trail_fade_in_seconds.to_le_bytes());
        bytes[offset + 112..offset + 116].copy_from_slice(&item.orientation_type.to_le_bytes());
        let flags =
            u32::from(item.animation_fit_lifetime) | (u32::from(item.animation_rate_as_fps) << 1);
        bytes[offset + 116..offset + 120].copy_from_slice(&flags.to_le_bytes());
        bytes[offset + 120..offset + 124].copy_from_slice(&item.secondary_sequence.to_le_bytes());
        let sheet_flags = u32::from(item.primary_sheet.is_some())
            | (u32::from(item.secondary_sheet.is_some()) << 1);
        bytes[offset + 124..offset + 128].copy_from_slice(&sheet_flags.to_le_bytes());
        if let Some(sample) = &item.primary_sheet {
            bytes[offset + 128..offset + 132].copy_from_slice(&sample.blend.to_le_bytes());
            put_sheet_images(&mut bytes, offset + 132, sample.current);
            put_sheet_images(&mut bytes, offset + 196, sample.next);
        }
        if let Some(sample) = &item.secondary_sheet {
            bytes[offset + 260..offset + 264].copy_from_slice(&sample.blend.to_le_bytes());
            put_sheet_images(&mut bytes, offset + 264, sample.current);
            put_sheet_images(&mut bytes, offset + 328, sample.next);
        }
        let material_state = item.material_state.expect("validated material state");
        bytes[offset + 392] = match material_state.shader {
            ParticleMaterialShader::SpriteCard => 0,
            ParticleMaterialShader::MeshSprite => 1,
        };
        bytes[offset + 393] = match material_state.color_space {
            ParticleColorSpace::SrgbTextureLinearTint => 0,
        };
        bytes[offset + 394] = material_state.blend.source as u8;
        bytes[offset + 395] = material_state.blend.destination as u8;
        bytes[offset + 396..offset + 400]
            .copy_from_slice(&item.secondary_animation_rate.to_le_bytes());
        bytes[offset + 400..offset + 404].copy_from_slice(&item.step_seconds.to_le_bytes());
        put_vector(&mut bytes, offset + 404, item.trail_end_position);
        bytes[offset + 416..offset + 420].copy_from_slice(&item.trail_width.to_le_bytes());
        bytes[offset + 420..offset + 424].copy_from_slice(&item.trail_length_scale.to_le_bytes());
        bytes[offset + 424..offset + 432].copy_from_slice(&item.stable_tie_identity.to_le_bytes());
        bytes[offset + 432..offset + 436].copy_from_slice(&item.yaw_radians.to_le_bytes());
    }
    Ok(bytes)
}

fn valid_sheet_sample(sample: Option<&SheetSample>) -> bool {
    sample.is_none_or(|sample| {
        sample.blend.is_finite()
            && (0.0..=1.0).contains(&sample.blend)
            && sample
                .current
                .iter()
                .flatten()
                .all(|value| value.is_finite())
            && sample.next.iter().flatten().all(|value| value.is_finite())
    })
}

fn put_sheet_images(bytes: &mut [u8], offset: usize, images: [[f32; 4]; 4]) {
    for (index, value) in images.into_iter().flatten().enumerate() {
        let start = offset + index * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceRequest {
    pub identity: u64,
    pub effect_identity: u32,
    pub system_uuid: [u8; 16],
    pub particle_identity: u32,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
    pub brush_only: bool,
    pub collision_group: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionResult {
    pub identity: u64,
    pub fraction: f32,
    pub start_solid: bool,
    pub normal: [f32; 3],
}

pub trait CollisionQuery {
    fn trace_batch(&mut self, requests: &[TraceRequest]) -> Result<Vec<CollisionResult>, Error>;

    fn lighting_at(&mut self, position: [f32; 3]) -> Result<[u8; 3], Error> {
        Err(Error::new(
            ErrorCode::MissingQuery,
            "particle-world",
            0,
            format!("lighting query at {position:?} is missing"),
        ))
    }
}

#[derive(Default)]
pub struct CollisionBatch {
    pub results: BTreeMap<u64, CollisionResult>,
}

impl CollisionQuery for CollisionBatch {
    fn trace_batch(&mut self, requests: &[TraceRequest]) -> Result<Vec<CollisionResult>, Error> {
        requests
            .iter()
            .map(|request| {
                self.results.get(&request.identity).cloned().ok_or_else(|| {
                    Error::new(
                        ErrorCode::MissingQuery,
                        "particle-world",
                        0,
                        format!("collision result {} is missing", request.identity),
                    )
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleWorld {
    registry: Arc<Registry>,
    sequence_spans: Arc<BTreeMap<String, Arc<BTreeMap<i32, f32>>>>,
    limits: WorldLimits,
    time: f32,
    effects: Vec<Effect>,
    event_identities: Arc<BTreeSet<u64>>,
    simulation_random: SimdRandom,
}

#[derive(Clone, Debug, PartialEq)]
struct Effect {
    identity: u32,
    owner_identity: Option<u32>,
    root: System,
    collision_cache: CollisionPlaneCache,
}

#[derive(Clone, Debug, PartialEq)]
struct System {
    definition_uuid: [u8; 16],
    definition_index: usize,
    sequence_spans: Option<Arc<BTreeMap<i32, f32>>>,
    path_identity: u64,
    start_seconds: f32,
    delay_seconds: f32,
    emission_active: bool,
    dormant: bool,
    first_frame: bool,
    local_time: f32,
    current_step: f32,
    previous_step: f32,
    simulated_frames: u32,
    random_seed: i32,
    random_query_count: i32,
    unique_particle_identity: i32,
    emitter_contexts: Vec<EmitterContext>,
    operator_contexts: Vec<OperatorContext>,
    controls: Arc<Vec<Option<ControlPoint>>>,
    local_lighting: Option<(i32, [f32; 3], [u8; 3])>,
    target_control_point: u8,
    particles: Vec<Particle>,
    children: Vec<System>,
}

#[derive(Clone, Debug, PartialEq)]
struct Particle {
    identity: u32,
    creation_seconds: f32,
    lifetime_seconds: f32,
    position: [f32; 3],
    previous_position: [f32; 3],
    initial_radius: f32,
    radius: f32,
    initial_roll: f32,
    roll: f32,
    yaw: f32,
    roll_speed: f32,
    initial_color: [f32; 3],
    color: [f32; 3],
    initial_alpha: f32,
    alpha: f32,
    sequence: i32,
    secondary_sequence: i32,
    trail_length: f32,
    target_control_point: u8,
}

#[derive(Clone, Debug, PartialEq)]
enum EmitterContext {
    Continuous {
        total_actual: f32,
        emitted: i32,
        time_offset: f32,
        stopped: bool,
    },
    Instantaneous {
        remaining: i32,
        actual: i32,
        start_time: f32,
        time_offset: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum OperatorContext {
    None,
    PositionLock { previous_position: [f32; 3] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CollisionPlane {
    point: [f32; 3],
    normal: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
struct CollisionPlaneCache {
    initialized: bool,
    last_update_seconds: f32,
    last_origin: [f32; 3],
    planes: Vec<CollisionPlane>,
}

impl Default for CollisionPlaneCache {
    fn default() -> Self {
        Self {
            initialized: false,
            last_update_seconds: -1.0,
            last_origin: [0.0; 3],
            planes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SimdRandom {
    values: [[f32; 4]; 55],
    first: usize,
    second: usize,
}

impl SimdRandom {
    fn new(mut seed: u32) -> Self {
        let mut values = [[0.0; 4]; 55];
        for value in &mut values {
            for lane in value {
                *lane = (seed >> 16) as f32 / 65_536.0;
                seed = seed.wrapping_add(1).wrapping_mul(3_141_592_621);
            }
        }
        Self {
            values,
            first: 23,
            second: 54,
        }
    }

    fn sample4(&mut self) -> [f32; 4] {
        let mut result = [0.0; 4];
        for (lane, output) in result.iter_mut().enumerate() {
            let value = self.values[self.first][lane] + self.values[self.second][lane];
            *output = if value >= 1.0 { value - 1.0 } else { value };
        }
        self.values[self.second] = result;
        self.first = self.first.checked_sub(1).unwrap_or(54);
        self.second = self.second.checked_sub(1).unwrap_or(54);
        result
    }
}

impl ParticleWorld {
    pub fn new(registry: &Registry, materials: &BTreeMap<String, ParticleMaterial>, limits: WorldLimits) -> Result<Self, Error> {
        validate_limits(limits)?;
        Ok(Self {
            registry: Arc::new(registry.clone()),
            sequence_spans: Arc::new(materials.iter().map(|(name, material)| {
                (name.clone(), Arc::new(material.sheet.sequences.iter().map(|(index, sequence)| {
                    (*index, sequence.duration_seconds)
                }).collect()))
            }).collect()),
            limits,
            time: 0.0,
            effects: Vec::new(),
            event_identities: Arc::new(BTreeSet::new()),
            simulation_random: SimdRandom::new(12_345_678),
        })
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn independent(&self) -> Self {
        Self { registry: Arc::clone(&self.registry), sequence_spans: Arc::clone(&self.sequence_spans),
            limits: self.limits, time: 0.0, effects: Vec::new(), event_identities: Arc::new(BTreeSet::new()),
            simulation_random: SimdRandom::new(12_345_678) }
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn event_identity_count(&self) -> usize {
        self.event_identities.len()
    }

    pub fn has_effect(&self, identity: u32) -> bool {
        self.effects.iter().any(|effect| effect.identity == identity)
    }

    pub fn advance(
        &mut self,
        events: &[Event],
        request: AdvanceRequest,
        collision: &mut impl CollisionQuery,
    ) -> Result<(Vec<RenderItem>, Option<Bounds>), Error> {
        self.transact(events, &[], request, collision, |items, bounds| {
            Ok((items, bounds))
        })
    }

    pub fn transact_render_output(
        &mut self,
        events: &[Event],
        attached_controls: &[(u32, ControlPoint)],
        request: AdvanceRequest,
        collision: &mut impl CollisionQuery,
        materials: &BTreeMap<String, ParticleMaterial>,
        material_identities: &[String],
        maximum_output_bytes: usize,
    ) -> Result<Vec<u8>, Error> {
        self.transact(events, attached_controls, request, collision, |items, bounds| {
            let resolved = resolve_render_output(items, materials)?;
            encode_render_output(&resolved, bounds, material_identities, maximum_output_bytes)
        })
    }

    /// Compose independent collections under one atomic presentation admission.
    /// State advances only after the caller accepts the complete render output.
    pub fn transact<T>(
        &mut self,
        events: &[Event],
        attached_controls: &[(u32, ControlPoint)],
        request: AdvanceRequest,
        collision: &mut impl CollisionQuery,
        complete: impl FnOnce(Vec<RenderItem>, Option<Bounds>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.transact_views(events, attached_controls, request, |_| request.camera_position, collision, complete)
    }

    /// Each collection is sorted using the camera of its owning render pass.
    pub fn transact_views<T>(
        &mut self,
        events: &[Event],
        attached_controls: &[(u32, ControlPoint)],
        request: AdvanceRequest,
        camera: impl Fn(u32) -> [f32; 3],
        collision: &mut impl CollisionQuery,
        complete: impl FnOnce(Vec<RenderItem>, Option<Bounds>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        validate_advance(self.time, events, request, self.limits)?;
        let mut candidate = self.clone();
        for (identity, control) in attached_controls {
            validate_control_point(control, self.limits)?;
            set_control(&mut effect_mut(&mut candidate.effects, *identity)?.root, control.clone());
        }
        let mut substeps = 0;
        let mut queries = 0;
        let mut cursor = request.from_seconds;
        for event in events {
            candidate.advance_interval(
                cursor,
                event.timestamp_seconds,
                request.maximum_step_seconds,
                &mut substeps,
                &mut queries,
                collision,
            )?;
            candidate.apply_event(event, collision)?;
            cursor = event.timestamp_seconds;
        }
        candidate.advance_interval(
            cursor,
            request.to_seconds,
            request.maximum_step_seconds,
            &mut substeps,
            &mut queries,
            collision,
        )?;
        candidate.time = request.to_seconds;
        candidate
            .effects
            .retain(|effect| !finished(&effect.root, &candidate.registry, request.to_seconds));
        let (items, bounds) = candidate.render(camera)?;
        let output = complete(items, bounds)?;
        *self = candidate;
        Ok(output)
    }

    fn advance_interval(
        &mut self,
        mut from: f32,
        to: f32,
        maximum_step: f32,
        substeps: &mut usize,
        queries: &mut usize,
        collision: &mut impl CollisionQuery,
    ) -> Result<(), Error> {
        while from < to {
            if self.effects.is_empty() {
                break;
            }
            *substeps += 1;
            if *substeps > self.limits.max_substeps {
                return Err(Error::new(
                    ErrorCode::BoundExceeded,
                    "particle-world",
                    0,
                    "advance exceeds max_substeps",
                ));
            }
            let definition_step = self
                .effects
                .iter()
                .map(|effect| maximum_system_step(&effect.root, &self.registry))
                .fold(0.1_f32, f32::min);
            let step = maximum_step.min(definition_step);
            let remaining_time = to - from;
            let next = if remaining_time <= step + step.abs().max(1.0) * 1.0e-6 {
                to
            } else {
                from + step
            };
            let total_before = particle_count(&self.effects);
            let mut remaining = self.limits.max_particles_total.saturating_sub(total_before);
            for effect in &mut self.effects {
                advance_system(
                    &mut effect.root,
                    &self.registry,
                    effect.identity,
                    from,
                    next,
                    self.limits,
                    &mut remaining,
                    queries,
                    collision,
                    &mut self.simulation_random,
                    &mut effect.collision_cache,
                )?;
            }
            from = next;
        }
        Ok(())
    }

    fn apply_event(
        &mut self,
        event: &Event,
        collision: &mut impl CollisionQuery,
    ) -> Result<(), Error> {
        if !Arc::make_mut(&mut self.event_identities).insert(event.identity) {
            return Err(Error::new(
                ErrorCode::InvalidEvent,
                "particle-world",
                0,
                "event identity is duplicated",
            ));
        }
        match &event.command {
            EventCommand::Create {
                effect_identity,
                definition,
                seed,
                owner_identity,
                control_points,
            } => {
                if self
                    .effects
                    .iter()
                    .any(|effect| effect.identity == *effect_identity)
                {
                    return Err(invalid_state("create targets an existing effect"));
                }
                self.create_effect(
                    *effect_identity,
                    definition,
                    *seed,
                    *owner_identity,
                    control_points,
                    event.timestamp_seconds,
                    collision,
                )?;
            }
            EventCommand::Replace {
                effect_identity,
                definition,
                seed,
                owner_identity,
                control_points,
            } => {
                let Some(index) = self
                    .effects
                    .iter()
                    .position(|effect| effect.identity == *effect_identity)
                else {
                    return Err(invalid_state("replace targets a missing effect"));
                };
                let replacement = self.build_effect(
                    *effect_identity,
                    definition,
                    *seed,
                    *owner_identity,
                    control_points,
                    event.timestamp_seconds,
                )?;
                let retained_particles = particle_count(&self.effects)
                    .saturating_sub(system_particle_count(&self.effects[index].root));
                let mut replacement = replacement;
                let mut remaining = self
                    .limits
                    .max_particles_total
                    .saturating_sub(retained_particles);
                initialize_first_frame(
                    &mut replacement.root,
                    &self.registry,
                    self.limits,
                    &mut remaining,
                    collision,
                )?;
                self.effects[index] = replacement;
            }
            EventCommand::SetControlPoint {
                effect_identity,
                control_point,
            } => {
                validate_control_point(control_point, self.limits)?;
                let effect = effect_mut(&mut self.effects, *effect_identity)?;
                set_control(&mut effect.root, control_point.clone());
            }
            EventCommand::StartEmission { effect_identity } => {
                let effect = effect_mut(&mut self.effects, *effect_identity)?;
                set_emission(&mut effect.root, true, false);
            }
            EventCommand::StopEmission {
                effect_identity,
                mode,
            } => {
                let index = self
                    .effects
                    .iter()
                    .position(|effect| effect.identity == *effect_identity)
                    .ok_or_else(|| invalid_state("stop targets a missing effect"))?;
                if *mode == StopMode::Immediate {
                    self.effects.remove(index);
                } else {
                    set_emission(&mut self.effects[index].root, false, false);
                }
            }
            EventCommand::Restart { effect_identity } => {
                let effect = effect_mut(&mut self.effects, *effect_identity)?;
                restart_system(&mut effect.root, &self.registry);
            }
            EventCommand::SetDormant {
                effect_identity,
                dormant,
            } => {
                let effect = effect_mut(&mut self.effects, *effect_identity)?;
                set_dormant(&mut effect.root, *dormant);
            }
            EventCommand::Destroy { effect_identity } => {
                let Some(index) = self
                    .effects
                    .iter()
                    .position(|effect| effect.identity == *effect_identity)
                else {
                    return Err(invalid_state("destroy targets a missing effect"));
                };
                self.effects.remove(index);
            }
            EventCommand::Reset => {
                self.effects.clear();
                let identities = Arc::make_mut(&mut self.event_identities);
                identities.clear();
                identities.insert(event.identity);
            }
        }
        Ok(())
    }

    fn create_effect(
        &mut self,
        identity: u32,
        definition: &str,
        seed: u64,
        owner_identity: Option<u32>,
        control_points: &[ControlPoint],
        timestamp: f32,
        collision: &mut impl CollisionQuery,
    ) -> Result<(), Error> {
        if self.effects.len() >= self.limits.max_effects {
            return Err(Error::new(
                ErrorCode::BoundExceeded,
                "particle-world",
                0,
                "effect count exceeds max_effects",
            ));
        }
        let mut effect = self.build_effect(
            identity,
            definition,
            seed,
            owner_identity,
            control_points,
            timestamp,
        )?;
        let mut remaining = self
            .limits
            .max_particles_total
            .saturating_sub(particle_count(&self.effects));
        initialize_first_frame(
            &mut effect.root,
            &self.registry,
            self.limits,
            &mut remaining,
            collision,
        )?;
        self.effects.push(effect);
        Ok(())
    }

    fn build_effect(
        &self,
        identity: u32,
        definition: &str,
        seed: u64,
        owner_identity: Option<u32>,
        control_points: &[ControlPoint],
        timestamp: f32,
    ) -> Result<Effect, Error> {
        if identity == 0 || definition.is_empty() {
            return Err(Error::new(
                ErrorCode::InvalidEvent,
                "particle-world",
                0,
                "effect identity and definition must be nonzero/nonempty",
            ));
        }
        validate_controls(control_points, self.limits)?;
        let root_definition = self
            .registry
            .definition(DefinitionLookup::Name(definition))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::MissingDefinition,
                    "particle-world",
                    0,
                    format!("particle definition {definition} is missing"),
                )
            })?;
        self.registry
            .target_closure(&[DefinitionLookup::Name(definition)])?;
        let mut system_count = 0;
        let mut controls = vec![None; self.limits.max_control_points];
        for control in control_points {
            controls[control.index as usize] = Some(control.clone());
        }
        let controls = Arc::new(controls);
        let mut instantiate_state = InstantiateState {
            controls: &controls,
            sequence_spans: &self.sequence_spans,
            system_count: &mut system_count,
            limits: self.limits,
        };
        let random_seed = normalize_seed(seed);
        let root = instantiate(
            &self.registry,
            root_definition,
            timestamp,
            0.0,
            hash(seed, identity as u64),
            random_seed,
            &mut instantiate_state,
        )?;
        Ok(Effect {
            identity,
            owner_identity,
            root,
            collision_cache: CollisionPlaneCache::default(),
        })
    }

    fn render(&self, camera: impl Fn(u32) -> [f32; 3]) -> Result<(Vec<RenderItem>, Option<Bounds>), Error> {
        let mut items = Vec::with_capacity(particle_count(&self.effects));
        let mut bounds: Option<Bounds> = None;
        for effect in &self.effects {
            let camera = camera(effect.identity);
            if !finite(&camera) { return Err(Error::new(ErrorCode::NonFinite, "particle-world", 0, "camera position is non-finite")); }
            if let Some(effect_bounds) = system_bounds(&effect.root, &self.registry) {
                merge_bounds(&mut bounds, effect_bounds);
            }
            render_system(
                &effect.root,
                &self.registry,
                effect.identity,
                camera,
                &mut items,
                self.limits.max_render_items,
            )?;
        }
        Ok((items, bounds))
    }
}

fn initialize_first_frame(
    system: &mut System,
    registry: &Registry,
    limits: WorldLimits,
    remaining_particles: &mut usize,
    collision: &mut impl CollisionQuery,
) -> Result<(), Error> {
    if system.delay_seconds > 0.0 {
        return Ok(());
    }
    initialize_system_first_frame(system, registry, limits, remaining_particles, collision)?;
    for child in &mut system.children {
        initialize_first_frame(child, registry, limits, remaining_particles, collision)?;
    }
    Ok(())
}

fn initialize_system_first_frame(
    system: &mut System,
    registry: &Registry,
    limits: WorldLimits,
    remaining_particles: &mut usize,
    collision: &mut impl CollisionQuery,
) -> Result<(), Error> {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    operate_before_emitters(system, definition)?;
    let initial = (integer_attribute(definition, &["initial_particles"], 0).max(0) as usize)
        .min(authored_remaining(system, definition));
    let capacity = caller_remaining(system, limits, *remaining_particles);
    if initial > capacity {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            &definition.source,
            0,
            "initial particle count exceeds the active system capacity",
        ));
    }
    for _ in 0..initial {
        let particle = initialize_particle(system, definition, 0.0, collision)?;
        system.particles.push(particle);
        *remaining_particles -= 1;
    }
    system.first_frame = false;
    system.simulated_frames = 1;
    Ok(())
}

struct InstantiateState<'a> {
    controls: &'a Arc<Vec<Option<ControlPoint>>>,
    sequence_spans: &'a BTreeMap<String, Arc<BTreeMap<i32, f32>>>,
    system_count: &'a mut usize,
    limits: WorldLimits,
}
fn instantiate(
    registry: &Registry,
    definition: &Definition,
    start: f32,
    delay: f32,
    path_identity: u64,
    random_seed: i32,
    state: &mut InstantiateState<'_>,
) -> Result<System, Error> {
    *state.system_count += 1;
    if *state.system_count > state.limits.max_systems {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-world",
            0,
            "system count exceeds max_systems",
        ));
    }
    let emitter_contexts = definition
        .functions(FunctionCategory::Emitter)
        .map(|emitter| {
            if emitter
                .identity
                .eq_ignore_ascii_case("emit_instantaneously")
            {
                let maximum = integer_parameter(emitter, "num_to_emit", 100);
                let minimum = integer_parameter(emitter, "num_to_emit_minimum", -1);
                let actual = if minimum >= 0 {
                    source_random_int(random_seed, 0, minimum.min(maximum), minimum.max(maximum))
                } else {
                    maximum
                }
                .max(0);
                EmitterContext::Instantaneous {
                    remaining: actual,
                    actual,
                    start_time: float_parameter(emitter, "emission_start_time", 0.0),
                    time_offset: 0.0,
                }
            } else {
                EmitterContext::Continuous {
                    total_actual: 0.0,
                    emitted: 0,
                    time_offset: 0.0,
                    stopped: false,
                }
            }
        })
        .collect();
    let operator_contexts = definition
        .functions(FunctionCategory::Operator)
        .map(|operator| {
            if operator
                .identity
                .eq_ignore_ascii_case("Movement Lock to Control Point")
            {
                OperatorContext::PositionLock {
                    previous_position: [0.0; 3],
                }
            } else {
                OperatorContext::None
            }
        })
        .collect();
    let mut children = Vec::with_capacity(definition.children.len());
    let mut child_seed = random_seed;
    for (index, child) in definition.children.iter().enumerate() {
        let child_definition = registry.child_definition(child).ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDefinition,
                &definition.source,
                0,
                "child definition is missing",
            )
        })?;
        if child_seed != 0 {
            child_seed = child_seed.wrapping_add(129);
        }
        children.push(instantiate(
            registry,
            child_definition,
            start,
            delay + child.delay_seconds,
            hash(path_identity, index as u64 + 1),
            child_seed,
            state,
        )?);
    }
    Ok(System {
        definition_uuid: definition.uuid,
        definition_index: definition.registry_index,
        sequence_spans: state.sequence_spans.get(&definition.material).cloned(),
        path_identity,
        start_seconds: start,
        delay_seconds: delay,
        emission_active: true,
        dormant: false,
        first_frame: true,
        local_time: 0.0,
        current_step: 0.0,
        previous_step: DEFAULT_STEP_SECONDS,
        simulated_frames: 0,
        random_seed,
        random_query_count: 0,
        unique_particle_identity: 0,
        emitter_contexts,
        operator_contexts,
        controls: Arc::clone(state.controls),
        local_lighting: None,
        target_control_point: state
            .controls
            .iter()
            .rposition(Option::is_some)
            .unwrap_or(0) as u8,
        particles: Vec::new(),
        children,
    })
}

#[allow(clippy::too_many_arguments)]
fn advance_system(
    system: &mut System,
    registry: &Registry,
    effect_identity: u32,
    from: f32,
    to: f32,
    limits: WorldLimits,
    remaining_particles: &mut usize,
    queries: &mut usize,
    collision: &mut impl CollisionQuery,
    simulation_random: &mut SimdRandom,
    collision_cache: &mut CollisionPlaneCache,
) -> Result<(), Error> {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    let local_from = (from - system.start_seconds - system.delay_seconds).max(0.0);
    let local_to = (to - system.start_seconds - system.delay_seconds).max(0.0);
    if to < system.start_seconds + system.delay_seconds {
        return Ok(());
    }
    if system.first_frame {
        initialize_system_first_frame(system, registry, limits, remaining_particles, collision)?;
    }
    let dt = (local_to - local_from).max(0.0);
    system.current_step = dt;
    system.local_time = local_to;
    operate_before_emitters(system, definition)?;
    emit(
        system,
        definition,
        local_from,
        local_to,
        limits,
        remaining_particles,
        collision,
    )?;
    if dt > 0.0 {
        system.simulated_frames = system.simulated_frames.saturating_add(1);
        operate(system, definition, local_to, dt, simulation_random);
        constrain(
            system,
            definition,
            effect_identity,
            limits,
            queries,
            collision,
            collision_cache,
        )?;
        system
            .particles
            .retain(|particle| local_to - particle.creation_seconds < particle.lifetime_seconds);
        system.previous_step = dt;
    }
    system.first_frame = false;
    update_control_history(system);
    for child in &mut system.children {
        advance_system(
            child,
            registry,
            effect_identity,
            from,
            to,
            limits,
            remaining_particles,
            queries,
            collision,
            simulation_random,
            collision_cache,
        )?;
    }
    Ok(())
}

fn operate_before_emitters(system: &mut System, definition: &Definition) -> Result<(), Error> {
    for operator in definition.functions(FunctionCategory::Operator) {
        if !operator.identity.eq_ignore_ascii_case("Set Control Point Positions")
            || operator_strength(operator, system.local_time) == 0.0 { continue; }
        let reference = integer_parameter(operator, "Control Point to offset positions from", 0);
        if reference < 0 || reference as usize >= system.controls.len() {
            return Err(Error::new(ErrorCode::BoundExceeded, &definition.source, 0, "control point operator reference exceeds the collection's controls"));
        }
        let origin = control_at_time(system, reference, system.local_time);
        let orientation = control_orientation(system, reference).unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let world_space = bool_parameter(operator, "Set positions in world space", false);
        for (ordinal, number_key, parent_key, location_key, location) in [
            (1, "First Control Point Number", "First Control Point Parent", "First Control Point Location", [128.0, 0.0, 0.0]),
            (2, "Second Control Point Number", "Second Control Point Parent", "Second Control Point Location", [0.0, 128.0, 0.0]),
            (3, "Third Control Point Number", "Third Control Point Parent", "Third Control Point Location", [-128.0, 0.0, 0.0]),
            (4, "Fourth Control Point Number", "Fourth Control Point Parent", "Fourth Control Point Location", [0.0, -128.0, 0.0]),
        ] {
            let index = integer_parameter(operator, number_key, ordinal);
            let parent = integer_parameter(operator, parent_key, 0);
            if index < 0 || index as usize >= system.controls.len() || parent < 0 || parent as usize >= system.controls.len() {
                return Err(Error::new(ErrorCode::BoundExceeded, &definition.source, 0, "control point operator exceeds the collection's controls"));
            }
            let location = vector_parameter(operator, location_key, location);
            let position = if world_space { location } else { add(origin, rotate(orientation, location)) };
            let mut control = system.controls[index as usize].clone().unwrap_or(ControlPoint {
                index: index as u8, position, previous_position: position, orientation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0; 3], radius: 0.0, density: 1.0, duration: 0.0, parent: None, object_identity: None,
            });
            control.position = position;
            control.parent = (index != 0 || parent != 0).then_some(parent as u8);
            set_control(system, control);
        }
    }
    Ok(())
}

fn emit(
    system: &mut System,
    definition: &Definition,
    from: f32,
    to: f32,
    limits: WorldLimits,
    remaining_particles: &mut usize,
    collision: &mut impl CollisionQuery,
) -> Result<(), Error> {
    if !system.emission_active {
        return Ok(());
    }
    let emitters: Vec<&Function> = definition.functions(FunctionCategory::Emitter).collect();
    for (emitter_index, emitter) in emitters.iter().enumerate() {
        let strength = operator_strength(emitter, to);
        let mut count = 0_usize;
        let mut creation_start = from;
        let mut creation_end = to;
        if emitter
            .identity
            .eq_ignore_ascii_case("emit_instantaneously")
        {
            let EmitterContext::Instantaneous {
                remaining,
                actual,
                time_offset,
                ..
            } = &mut system.emitter_contexts[emitter_index]
            else {
                unreachable!("validated emitter context")
            };
            let start = float_parameter(emitter, "emission_start_time", 0.0) + *time_offset;
            if *remaining > 0 && to >= start && *actual > 0 && strength > 0.0 {
                let per_frame = integer_parameter(emitter, "maximum emission per frame", -1);
                count = if per_frame < 0 {
                    *remaining as usize
                } else {
                    (*remaining as usize).min(per_frame.max(0) as usize)
                };
                *remaining -= count as i32;
                creation_start = start;
                creation_end = start;
            }
        } else {
            let EmitterContext::Continuous {
                total_actual,
                emitted,
                time_offset,
                stopped,
            } = &mut system.emitter_contexts[emitter_index]
            else {
                unreachable!("validated emitter context")
            };
            if *stopped || strength <= 0.0 {
                continue;
            }
            let mut rate = float_parameter(emitter, "emission_rate", 100.0).max(0.0) * strength;
            let scale = system.controls.iter().rposition(Option::is_some).unwrap_or(0) as f32
                * float_parameter(emitter, "scale emission to used control points", 0.0);
            if scale != 0.0 { rate *= scale; }
            let duration = float_parameter(emitter, "emission_duration", 0.0);
            let start = float_parameter(emitter, "emission_start_time", 0.0) + *time_offset;
            creation_start = from.max(start);
            creation_end = if duration > 0.0 {
                to.min(start + duration)
            } else {
                to
            };
            let active_duration = (creation_end - creation_start).max(0.0);
            *total_actual += rate * active_duration;
            let target = total_actual.floor() as i32;
            count = target.saturating_sub(*emitted).max(0) as usize;
            *emitted = target;
        }
        count = count.min(authored_remaining(system, definition));
        let caller_capacity = caller_remaining(system, limits, *remaining_particles);
        if count > caller_capacity {
            return Err(Error::new(
                ErrorCode::BoundExceeded,
                &definition.source,
                0,
                format!(
                    "definition {} emission of {} exceeds remaining capacity {}",
                    definition.name, count, caller_capacity
                ),
            ));
        }
        for ordinal in 0..count {
            let creation = if creation_end > creation_start {
                creation_start
                    + (creation_end - creation_start) * (ordinal + 1) as f32 / count as f32
            } else {
                creation_start
            };
            let particle = initialize_particle(system, definition, creation.min(to), collision)?;
            system.particles.push(particle);
            *remaining_particles -= 1;
        }
    }
    Ok(())
}

fn initialize_particle(
    system: &mut System,
    definition: &Definition,
    creation: f32,
    collision: &mut impl CollisionQuery,
) -> Result<Particle, Error> {
    let constant_color = color_attribute(definition, "color", [255; 4]);
    let mut particle = Particle {
        identity: 0,
        creation_seconds: creation,
        lifetime_seconds: 1.0,
        position: [0.0; 3],
        previous_position: [0.0; 3],
        initial_radius: float_attribute(definition, "radius", 5.0),
        radius: float_attribute(definition, "radius", 5.0),
        initial_roll: float_attribute(definition, "rotation", 0.0),
        roll: float_attribute(definition, "rotation", 0.0),
        yaw: 0.0,
        roll_speed: float_attribute(definition, "rotation_speed", 0.0),
        initial_color: [
            constant_color[0] as f32 / 255.0,
            constant_color[1] as f32 / 255.0,
            constant_color[2] as f32 / 255.0,
        ],
        color: [
            constant_color[0] as f32 / 255.0,
            constant_color[1] as f32 / 255.0,
            constant_color[2] as f32 / 255.0,
        ],
        initial_alpha: constant_color[3] as f32 / 255.0,
        alpha: constant_color[3] as f32 / 255.0,
        sequence: integer_attribute(definition, &["sequence_number"], 0),
        secondary_sequence: integer_attribute(definition, &["sequence_number 1"], 0),
        trail_length: 0.1,
        target_control_point: 0,
    };
    let mut velocity = [0.0; 3];
    let mut claimed = BTreeSet::new();
    for initializer in definition.functions(FunctionCategory::Initializer) {
        if initializer
            .identity
            .eq_ignore_ascii_case("Position Modify Offset Random")
            || initializer
                .identity
                .eq_ignore_ascii_case("Rotation Speed Random")
            || initializer
                .identity
                .eq_ignore_ascii_case("Remap Initial Scalar")
            || initializer
                .identity
                .eq_ignore_ascii_case("Remap Scalar to Vector")
            || initializer
                .identity
                .eq_ignore_ascii_case("Move Particles Between 2 Control Points")
            || initializer.identity.eq_ignore_ascii_case("Velocity Noise")
            || initializer
                .identity
                .eq_ignore_ascii_case("Remap Control Point to Vector")
            || initializer
                .identity
                .eq_ignore_ascii_case("Lifetime Pre-Age Noise")
            || initializer.identity.eq_ignore_ascii_case("Lifetime From Sequence")
        {
            continue;
        }
        let attribute = initializer_attribute(&initializer.identity);
        if attribute.is_some_and(|attribute| !claimed.insert(attribute)) {
            continue;
        }
        if initializer
            .identity
            .eq_ignore_ascii_case("Assign target CP")
        {
            let minimum = integer_parameter(initializer, "starting control point", 0);
            let maximum = integer_parameter(initializer, "maximum end control point", 0);
            particle.target_control_point = if minimum <= maximum {
                source_clamp(i32::from(system.target_control_point), minimum, maximum) as u8
            } else {
                maximum as u8
            };
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Lifetime From Control Point Life Time")
        {
            let minimum = integer_parameter(initializer, "starting control point", 0);
            let maximum = integer_parameter(initializer, "maximum end control point", 0);
            let target = source_clamp(i32::from(particle.target_control_point), minimum, maximum);
            particle.lifetime_seconds = system
                .controls
                .get(target as usize)
                .and_then(Option::as_ref)
                .map_or(0.0, |control| control.duration);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Rotation Yaw Flip Random")
        {
            if next_random(system) < float_parameter(initializer, "Flip Percentage", 0.5) {
                particle.yaw += std::f32::consts::PI;
            }
        } else if initializer.identity.eq_ignore_ascii_case("Lifetime Random") {
            particle.lifetime_seconds = ranged(
                float_parameter(initializer, "lifetime_min", 0.0),
                float_parameter(initializer, "lifetime_max", 0.0),
                float_parameter(initializer, "lifetime_random_exponent", 1.0),
                next_random(system),
            );
        } else if initializer.identity.eq_ignore_ascii_case("Radius Random") {
            particle.radius = ranged(
                float_parameter(initializer, "radius_min", 1.0),
                float_parameter(initializer, "radius_max", 1.0),
                float_parameter(initializer, "radius_random_exponent", 1.0),
                next_random(system),
            )
            .max(0.0);
        } else if initializer.identity.eq_ignore_ascii_case("Alpha Random") {
            let minimum = integer_parameter(initializer, "alpha_min", 255) as f32 / 255.0;
            let maximum = integer_parameter(initializer, "alpha_max", 255) as f32 / 255.0;
            particle.alpha = ranged(
                minimum,
                maximum,
                float_parameter(initializer, "alpha_random_exponent", 1.0),
                next_random(system),
            )
            .clamp(0.0, 1.0);
        } else if initializer.identity.eq_ignore_ascii_case("Color Random") {
            let first = color_parameter(initializer, "color1", [255; 4]);
            let second = color_parameter(initializer, "color2", first);
            let tint_percentage = float_parameter(initializer, "tint_perc", 0.0);
            let tint = if tint_percentage != 0.0 {
                let point = integer_parameter(initializer, "tint control point", 0);
                let origin = control_at_time(system, point, system.local_time);
                let threshold =
                    float_parameter(initializer, "tint update movement threshold", 32.0);
                let value = match system.local_lighting {
                    Some((cached_point, previous, value))
                        if cached_point == point
                            && length_squared(sub(previous, origin)) < threshold * threshold =>
                    {
                        value
                    }
                    _ => {
                        let value = collision.lighting_at(origin)?;
                        system.local_lighting = Some((point, origin, value));
                        value
                    }
                };
                let minimum = color_parameter(initializer, "tint clamp min", [0; 4]);
                let maximum = color_parameter(initializer, "tint clamp max", [255; 4]);
                Some(std::array::from_fn::<_, 3, _>(|index| {
                    source_clamp(value[index], minimum[index], maximum[index]) as f32 / 255.0
                }))
            } else {
                None
            };
            let random = next_random(system);
            for component in 0..3 {
                particle.color[component] = (first[component] as f32
                    + (second[component] as f32 - first[component] as f32) * random)
                    / 255.0;
                if let Some(value) = tint {
                    particle.color[component] =
                        mix(particle.color[component], value[component], tint_percentage);
                }
            }
        } else if initializer.identity.eq_ignore_ascii_case("Rotation Random") {
            particle.roll = ranged(
                float_parameter(initializer, "rotation_offset_min", 0.0),
                float_parameter(initializer, "rotation_offset_max", 360.0),
                float_parameter(initializer, "rotation_random_exponent", 1.0),
                next_random(system),
            )
            .to_radians()
                + float_parameter(initializer, "rotation_initial", 0.0).to_radians();
        } else if initializer.identity.eq_ignore_ascii_case("Sequence Random") {
            let minimum = integer_parameter(initializer, "sequence_min", 0);
            let maximum = integer_parameter(initializer, "sequence_max", 0);
            particle.sequence = next_random_int(system, minimum, maximum);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Trail Length Random")
        {
            particle.trail_length = ranged(
                float_parameter(initializer, "length_min", 0.1),
                float_parameter(initializer, "length_max", 0.1),
                float_parameter(initializer, "length_random_exponent", 1.0),
                next_random(system),
            )
            .max(0.0);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Position Along Path Random")
        {
            let (start, midpoint, end) = particle_path(system, initializer, creation, "bulge");
            let fraction = next_random(system);
            let first = add(start, mul(sub(midpoint, start), fraction));
            let second = add(midpoint, mul(sub(end, midpoint), fraction));
            let maximum = float_parameter(initializer, "maximum distance", 0.0);
            let offset = next_random_vector(system, [-maximum; 3], [maximum; 3]);
            particle.position = add(add(first, mul(sub(second, first), fraction)), offset);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Position Within Box Random")
        {
            let minimum = vector_parameter(initializer, "min", [0.0; 3]);
            let maximum = vector_parameter(initializer, "max", [0.0; 3]);
            let cp = control_at_time(
                system,
                integer_parameter(initializer, "control point number", 0),
                creation,
            );
            let random = next_random_vector(system, minimum, maximum);
            for component in 0..3 {
                particle.position[component] = cp[component] + random[component];
            }
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Position Within Sphere Random")
        {
            let mut cp_index = integer_parameter(initializer, "control_point_number", 0);
            if bool_parameter(initializer, "randomly distribute to highest supplied Control Point", false) {
                let growth = float_parameter(initializer, "randomly distribution growth time", 0.0);
                let strength = if growth == 0.0 { 1.0 } else { system.local_time.min(growth) / growth };
                let highest = system.controls.iter().rposition(Option::is_some).unwrap_or(0);
                cp_index = next_random_int(system, cp_index, (highest as f32 * strength).floor() as i32);
            }
            let cp = control_at_time(system, cp_index, creation);
            let (mut direction, unit_radius) = next_random_in_unit_sphere(system);
            let distance_bias = vector_parameter(initializer, "distance_bias", [1.0; 3]);
            let absolute = vector_parameter(initializer, "distance_bias_absolute_value", [0.0; 3]);
            for component in 0..3 {
                if absolute[component] != 0.0 {
                    direction[component] = direction[component].abs();
                }
                direction[component] *= distance_bias[component];
            }
            direction = normalize(direction).unwrap_or([1.0, 0.0, 0.0]);
            let distance = mix(
                float_parameter(initializer, "distance_min", 0.0),
                float_parameter(initializer, "distance_max", 0.0),
                unit_radius,
            );
            let offset = mul(direction, distance);
            let offset = if bool_parameter(initializer, "bias in local system", false)
                && distance_bias != [1.0; 3]
            {
                control_orientation(system, cp_index)
                    .map_or(offset, |orientation| rotate(orientation, offset))
            } else {
                offset
            };
            particle.position = add(cp, offset);
            velocity = [0.0; 3];
            let speed_maximum = float_parameter(initializer, "speed_max", 0.0);
            if speed_maximum > 0.0 {
                let speed = ranged(
                    float_parameter(initializer, "speed_min", 0.0),
                    speed_maximum,
                    float_parameter(initializer, "speed_random_exponent", 1.0),
                    next_random(system),
                );
                velocity = mul(direction, speed);
            }
            let speed_min = vector_parameter(
                initializer,
                "speed_in_local_coordinate_system_min",
                [0.0; 3],
            );
            let speed_max = vector_parameter(
                initializer,
                "speed_in_local_coordinate_system_max",
                [0.0; 3],
            );
            let local_velocity = [
                mix(speed_min[0], speed_max[0], next_random(system)),
                mix(speed_min[1], speed_max[1], next_random(system)),
                mix(speed_min[2], speed_max[2], next_random(system)),
            ];
            let local_velocity =
                control_orientation(system, cp_index).map_or(local_velocity, |orientation| {
                    rotate(
                        orientation,
                        [local_velocity[0], -local_velocity[1], local_velocity[2]],
                    )
                });
            velocity = add(velocity, local_velocity);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Remap Initial Scalar")
        {
            let start = float_parameter(initializer, "emitter lifetime start time (seconds)", -1.0);
            let end = float_parameter(initializer, "emitter lifetime end time (seconds)", -1.0);
            if start != -1.0 && end != -1.0 && (creation < start || creation >= end) {
                continue;
            }
            let minimum = float_parameter(initializer, "input minimum", 0.0);
            let maximum = float_parameter(initializer, "input maximum", 1.0);
            if bool_parameter(
                initializer,
                "only active within specified input range",
                false,
            ) && (creation < minimum || creation > maximum)
            {
                continue;
            }
            let field = integer_parameter(initializer, "output field", 3);
            let mut low = float_parameter(initializer, "output minimum", 0.0);
            let mut high = float_parameter(initializer, "output maximum", 1.0);
            if field == 7 {
                low = low.clamp(0.0, 1.0);
                high = high.clamp(0.0, 1.0);
            }
            let mut value = mix(low, high, remap(creation, minimum, maximum));
            let destination = match field {
                1 => &mut particle.lifetime_seconds,
                3 => &mut particle.radius,
                4 => &mut particle.roll,
                7 => &mut particle.alpha,
                _ => unreachable!("validated particle scalar output field"),
            };
            if bool_parameter(
                initializer,
                "output is scalar of initial random range",
                false,
            ) {
                value *= *destination;
            }
            *destination = value;
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Remap Scalar to Vector")
        {
            let start = float_parameter(initializer, "emitter lifetime start time (seconds)", -1.0);
            let end = float_parameter(initializer, "emitter lifetime end time (seconds)", -1.0);
            if start != -1.0 && end != -1.0 && (creation < start || creation >= end) {
                continue;
            }
            let minimum = vector_parameter(initializer, "output minimum", [0.0; 3]);
            let maximum = vector_parameter(initializer, "output maximum", [1.0; 3]);
            let fraction = remap(
                creation,
                float_parameter(initializer, "input minimum", 0.0),
                float_parameter(initializer, "input maximum", 1.0),
            );
            let mut output =
                std::array::from_fn(|index| mix(minimum[index], maximum[index], fraction));
            let control = integer_parameter(initializer, "control_point_number", 0);
            if bool_parameter(initializer, "use local system", true)
                && let Some(orientation) = control_orientation(system, control)
            {
                output = rotate(orientation, output);
            }
            output = add(control_at_time(system, control, creation), output);
            if bool_parameter(
                initializer,
                "output is scalar of initial random range",
                false,
            ) {
                for (component, prior) in output.iter_mut().zip(particle.position) {
                    *component *= prior;
                }
            }
            particle.position = output;
            velocity = [0.0; 3];
        }
    }
    for initializer in definition.functions(FunctionCategory::Initializer) {
        if initializer
            .identity
            .eq_ignore_ascii_case("Lifetime From Sequence")
        {
            let rate = float_parameter(initializer, "Frames Per Second", 30.0);
            if rate != 0.0 && let Some(spans) = &system.sequence_spans {
                let span = spans.get(&particle.sequence)
                    .or_else(|| spans.first_key_value().map(|(_, span)| span))
                    .copied().unwrap_or(0.0);
                particle.lifetime_seconds = if span == 0.0 { 1.0 } else { span / rate };
            }
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Position Modify Offset Random")
        {
            let minimum = vector_parameter(initializer, "offset min", [0.0; 3]);
            let maximum = vector_parameter(initializer, "offset max", [0.0; 3]);
            let radius_scale =
                if bool_parameter(initializer, "offset proportional to radius 0/1", false) {
                    particle.radius
                } else {
                    1.0
                };
            let mut offset = next_random_vector(
                system,
                mul(minimum, radius_scale),
                mul(maximum, radius_scale),
            );
            if bool_parameter(initializer, "offset in local space 0/1", false) {
                let cp = integer_parameter(initializer, "control_point_number", 0);
                if let Some(orientation) = control_orientation(system, cp) {
                    offset = rotate(orientation, offset);
                }
            }
            particle.position = add(particle.position, offset);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Rotation Speed Random")
        {
            let mut speed = float_parameter(initializer, "rotation_speed_constant", 0.0)
                + ranged(
                    float_parameter(initializer, "rotation_speed_random_min", 0.0),
                    float_parameter(initializer, "rotation_speed_random_max", 360.0),
                    float_parameter(initializer, "rotation_speed_random_exponent", 1.0),
                    next_random(system),
                );
            if bool_parameter(initializer, "randomly_flip_direction", true)
                && next_random(system) >= 0.5
            {
                speed = -speed;
            }
            particle.roll_speed += speed.to_radians();
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Remap Initial Scalar")
        {
            let start = float_parameter(initializer, "emitter lifetime start time (seconds)", -1.0);
            let end = float_parameter(initializer, "emitter lifetime end time (seconds)", -1.0);
            if start != -1.0 && end != -1.0 && (creation < start || creation >= end) {
                continue;
            }
            let input_field = integer_parameter(initializer, "input field", 8);
            let Some(input) = particle_scalar(&particle, input_field) else {
                continue;
            };
            let minimum = float_parameter(initializer, "input minimum", 0.0);
            let maximum = float_parameter(initializer, "input maximum", 1.0);
            if bool_parameter(
                initializer,
                "only active within specified input range",
                false,
            ) && (input < minimum || input > maximum)
            {
                continue;
            }
            let field = integer_parameter(initializer, "output field", 3);
            let output_min = float_parameter(initializer, "output minimum", 0.0);
            let output_max = float_parameter(initializer, "output maximum", 1.0);
            let output_min = if field == 7 {
                output_min.clamp(0.0, 1.0)
            } else {
                output_min
            };
            let output_max = if field == 7 {
                output_max.clamp(0.0, 1.0)
            } else {
                output_max
            };
            let mut output = mix(output_min, output_max, remap(input, minimum, maximum));
            if bool_parameter(
                initializer,
                "output is scalar of initial random range",
                false,
            ) {
                output *= particle_scalar(&particle, field).unwrap_or(0.0);
            }
            set_particle_scalar(&mut particle, field, output);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Remap Scalar to Vector")
        {
            let start = float_parameter(initializer, "emitter lifetime start time (seconds)", -1.0);
            let end = float_parameter(initializer, "emitter lifetime end time (seconds)", -1.0);
            if start != -1.0 && end != -1.0 && (creation < start || creation >= end) {
                continue;
            }
            let Some(input) =
                particle_scalar(&particle, integer_parameter(initializer, "input field", 8))
            else {
                continue;
            };
            let fraction = remap(
                input,
                float_parameter(initializer, "input minimum", 0.0),
                float_parameter(initializer, "input maximum", 1.0),
            );
            let minimum = vector_parameter(initializer, "output minimum", [0.0; 3]);
            let maximum = vector_parameter(initializer, "output maximum", [1.0; 3]);
            let mut output = [
                mix(minimum[0], maximum[0], fraction),
                mix(minimum[1], maximum[1], fraction),
                mix(minimum[2], maximum[2], fraction),
            ];
            let field = integer_parameter(initializer, "output field", 0);
            if field == 0 {
                let cp = integer_parameter(initializer, "control_point_number", 0);
                if bool_parameter(initializer, "use local system", true) {
                    output = control_orientation(system, cp)
                        .map_or(output, |orientation| rotate(orientation, output));
                }
                output = add(control_at_time(system, cp, creation), output);
            }
            if bool_parameter(
                initializer,
                "output is scalar of initial random range",
                false,
            ) {
                let initial = particle_vector(&particle, field).unwrap_or([0.0; 3]);
                output = [
                    output[0] * initial[0],
                    output[1] * initial[1],
                    output[2] * initial[2],
                ];
            }
            set_particle_vector(&mut particle, field, output);
            if field == 0 {
                velocity = [0.0; 3];
            }
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Move Particles Between 2 Control Points")
        {
            let point = integer_parameter(initializer, "end control point", 1);
            let mut destination = control_at_time(system, point, creation);
            let spread = float_parameter(initializer, "end spread", 0.0);
            if spread > 0.0 {
                let (direction, radius) = next_random_in_unit_sphere(system);
                destination = add(destination, mul(direction, radius * spread));
            }
            let mut delta = sub(destination, particle.position);
            let mut distance = length_squared(delta).sqrt();
            let offset = float_parameter(initializer, "start offset", 0.0);
            if offset > 0.0 {
                particle.position = add(
                    particle.position,
                    mul(delta, offset / (distance + f32::EPSILON)),
                );
                delta = sub(destination, particle.position);
                distance = length_squared(delta).sqrt();
            }
            let speed = mix(
                float_parameter(initializer, "minimum speed", 1.0),
                float_parameter(initializer, "maximum speed", 1.0),
                next_random(system),
            );
            particle.lifetime_seconds = distance / (speed + f32::EPSILON);
            velocity = if distance > 0.0 {
                mul(delta, speed / distance)
            } else {
                [0.0; 3]
            };
        } else if initializer.identity.eq_ignore_ascii_case("Velocity Noise") {
            let temporal = (particle.creation_seconds
                + float_parameter(initializer, "time coordinate offset", 0.0))
                * float_parameter(initializer, "time noise coordinate scale", 1.0);
            let spatial = mul(
                add(
                    particle.position,
                    vector_parameter(initializer, "spatial coordinate offset", [0.0; 3]),
                ),
                float_parameter(initializer, "spatial noise coordinate scale", 0.01),
            );
            let coordinate = spatial.map(|value| value + temporal);
            let offsets = [
                [0.0, 0.0, 0.0],
                [100_000.5, 300_000.25, 9_000_000.75],
                [110_000.25, 310_000.75, 9_100_000.5],
            ];
            let absolute = vector_parameter(initializer, "absolute value", [0.0; 3]);
            let invert = vector_parameter(initializer, "invert abs value", [0.0; 3]);
            let minimum = vector_parameter(initializer, "output minimum", [0.0; 3]);
            let maximum = vector_parameter(initializer, "output maximum", [1.0; 3]);
            let mut added = std::array::from_fn(|index| {
                let mut noise = crate::source_noise::sample(add(coordinate, offsets[index]));
                let scale = if absolute[index] != 0.0 {
                    noise = noise.abs();
                    1.0
                } else {
                    0.5
                };
                if invert[index] != 0.0 {
                    noise = 1.0 - noise;
                }
                minimum[index]
                    + (1.0 - scale) * (maximum[index] - minimum[index])
                    + scale * (maximum[index] - minimum[index]) * noise
            });
            if bool_parameter(initializer, "apply velocity in local space (0/1)", false) {
                let point = integer_parameter(initializer, "control point number", 0);
                if let Some(orientation) = control_orientation(system, point) {
                    added = rotate(orientation, added);
                }
            }
            velocity = add(velocity, added);
        } else if initializer.identity.eq_ignore_ascii_case("Remap Noise to Scalar") {
            let temporal = (particle.creation_seconds + float_parameter(initializer, "time coordinate offset", 0.0))
                * float_parameter(initializer, "time noise coordinate scale", 0.1);
            let spatial = mul(add(particle.position, vector_parameter(initializer, "spatial coordinate offset", [0.0; 3])),
                float_parameter(initializer, "spatial noise coordinate scale", 0.001));
            let mut noise = crate::source_noise::sample(spatial.map(|value| value + temporal));
            let absolute = bool_parameter(initializer, "absolute value", false);
            if absolute { noise = noise.abs(); }
            if bool_parameter(initializer, "invert absolute value", false) { noise = 1.0 - noise; }
            let field = integer_parameter(initializer, "output field", 3);
            let mut minimum = float_parameter(initializer, "output minimum", 0.0);
            let mut maximum = float_parameter(initializer, "output maximum", 1.0);
            if matches!(field, 4 | 5 | 12) { minimum = minimum.to_radians(); maximum = maximum.to_radians(); }
            let scale = if absolute { 1.0 } else { 0.5 };
            let value = minimum + (1.0 - scale) * (maximum - minimum) + scale * (maximum - minimum) * noise;
            set_particle_scalar(&mut particle, field, value);
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Lifetime Pre-Age Noise")
        {
            let temporal = (particle.creation_seconds
                + float_parameter(initializer, "time coordinate offset", 0.0))
                * float_parameter(initializer, "time noise coordinate scale", 1.0);
            let spatial = mul(
                add(
                    particle.position,
                    vector_parameter(initializer, "spatial coordinate offset", [0.0; 3]),
                ),
                float_parameter(initializer, "spatial noise coordinate scale", 1.0),
            );
            let mut noise = crate::source_noise::sample(spatial.map(|value| value + temporal));
            let absolute = bool_parameter(initializer, "absolute value", false);
            let scale = if absolute {
                noise = noise.abs();
                1.0
            } else {
                0.5
            };
            if bool_parameter(initializer, "invert absolute value", false) {
                noise = 1.0 - noise;
            }
            let minimum = float_parameter(initializer, "start age minimum", 0.0);
            let maximum = float_parameter(initializer, "start age maximum", 1.0);
            let age = (minimum
                + (1.0 - scale) * (maximum - minimum)
                + scale * (maximum - minimum) * noise)
                .clamp(0.0, 1.0);
            particle.creation_seconds -= age * particle.lifetime_seconds;
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Remap Control Point to Vector")
        {
            let start = float_parameter(initializer, "emitter lifetime start time (seconds)", -1.0);
            let end = float_parameter(initializer, "emitter lifetime end time (seconds)", -1.0);
            if start != -1.0 && end != -1.0 && (creation < start || creation >= end) {
                continue;
            }
            let input = control_at_time(
                system,
                integer_parameter(initializer, "input control point number", 0),
                system.local_time,
            );
            let input_minimum = vector_parameter(initializer, "input minimum", [0.0; 3]);
            let input_maximum = vector_parameter(initializer, "input maximum", [0.0; 3]);
            let mut minimum = vector_parameter(initializer, "output minimum", [0.0; 3]);
            let mut maximum = vector_parameter(initializer, "output maximum", [0.0; 3]);
            let local = integer_parameter(initializer, "local space cp", -1);
            if local >= 0
                && let Some(orientation) = control_orientation(system, local)
            {
                minimum = rotate(orientation, minimum);
                maximum = rotate(orientation, maximum);
            }
            let field = integer_parameter(initializer, "output field", 0);
            let mut output = std::array::from_fn(|index| {
                mix(
                    minimum[index],
                    maximum[index],
                    remap(input[index], input_minimum[index], input_maximum[index]),
                )
            });
            if bool_parameter(
                initializer,
                "output is scalar of initial random range",
                false,
            ) {
                let initial = particle_vector(&particle, field).unwrap_or([0.0; 3]);
                output = std::array::from_fn(|index| output[index] * initial[index]);
            }
            if field == 6 {
                set_particle_vector(&mut particle, field, output);
            } else {
                let offset = bool_parameter(initializer, "offset position", false);
                let accelerate = bool_parameter(initializer, "accelerate position", false);
                if offset {
                    output = add(
                        output,
                        particle_vector(&particle, field).unwrap_or([0.0; 3]),
                    );
                }
                if accelerate {
                    output = if offset {
                        add(output, mul(output, system.current_step))
                    } else {
                        mul(output, system.current_step)
                    };
                }
                set_particle_vector(&mut particle, field, output);
                if field == 0 && !accelerate {
                    velocity = [0.0; 3];
                }
            }
        }
    }
    particle.previous_position = sub(particle.position, mul(velocity, system.previous_step));
    particle.identity = source_particle_identity(system);
    particle.initial_radius = particle.radius;
    particle.initial_roll = particle.roll;
    particle.initial_color = particle.color;
    particle.initial_alpha = particle.alpha;
    Ok(particle)
}

fn operate(
    system: &mut System,
    definition: &Definition,
    time: f32,
    dt: f32,
    simulation_random: &mut SimdRandom,
) {
    let random_seed = system.random_seed;
    for (function_index, operator) in definition.functions(FunctionCategory::Operator).enumerate() {
        let strength = operator_strength(operator, time);
        if strength <= 0.0 {
            continue;
        }
        if operator.identity.eq_ignore_ascii_case("Movement Basic") {
            operate_movement(system, definition, operator, time, dt, simulation_random);
            continue;
        }
        let position_lock = if operator
            .identity
            .eq_ignore_ascii_case("Movement Lock to Control Point")
        {
            let control_index = integer_parameter(operator, "control_point_number", 0);
            let current = control_at(system, control_index);
            let snapshot_previous = system
                .controls
                .get(control_index.max(0) as usize)
                .and_then(Option::as_ref)
                .map_or(current, |control| control.previous_position);
            let previous = match &mut system.operator_contexts[function_index] {
                OperatorContext::PositionLock { previous_position } => {
                    let value = if *previous_position == [0.0; 3] {
                        current
                    } else {
                        *previous_position
                    };
                    *previous_position = current;
                    value
                }
                OperatorContext::None => current,
            };
            Some((current, previous, snapshot_previous))
        } else {
            None
        };
        let random_offset = (function_index as i32).wrapping_mul(17);
        let controls = &system.controls;
        for particle in &mut system.particles {
            let age = (time - particle.creation_seconds).max(0.0);
            let life = if particle.lifetime_seconds > 0.0 {
                (age / particle.lifetime_seconds).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if operator
                .identity
                .eq_ignore_ascii_case("Alpha Fade and Decay")
            {
                if age >= particle.lifetime_seconds {
                    particle.lifetime_seconds = 0.0;
                    continue;
                }
                let (fade_in_start, fade_in_end, fade_out_start, fade_out_end) =
                    fade_windows(operator);
                let start = float_parameter(operator, "start_alpha", 1.0);
                let end = float_parameter(operator, "end_alpha", 0.0);
                if life >= fade_in_start && life < fade_in_end {
                    let value = spline(remap(life, fade_in_start, fade_in_end));
                    particle.alpha = mix(
                        particle.initial_alpha * start,
                        particle.initial_alpha,
                        value,
                    );
                }
                if life >= fade_out_start && life < fade_out_end {
                    let value = spline(remap(life, fade_out_start, fade_out_end));
                    particle.alpha =
                        mix(particle.initial_alpha, particle.initial_alpha * end, value);
                }
                particle.alpha = particle.alpha.clamp(0.0, 1.0);
            } else if operator
                .identity
                .eq_ignore_ascii_case("Alpha Fade Out Random")
            {
                let duration = source_random_exp(
                    random_seed,
                    particle.identity as i32 + random_offset,
                    float_parameter(operator, "fade out time min", 0.25),
                    float_parameter(operator, "fade out time max", 0.25),
                    float_parameter(operator, "fade out time exponent", 1.0),
                )
                .max(f32::EPSILON);
                let proportional = bool_parameter(operator, "proportional 0/1", true);
                let (elapsed, fade_start) = if proportional {
                    (life, 1.0 - duration)
                } else {
                    (age, particle.lifetime_seconds - duration)
                };
                if elapsed > fade_start {
                    let mut fraction = remap(elapsed, fade_start, fade_start + duration);
                    if bool_parameter(operator, "ease in and out", true) {
                        fraction = spline(fraction);
                    } else {
                        fraction = bias(fraction, float_parameter(operator, "fade bias", 0.5));
                    }
                    particle.alpha = (particle.initial_alpha * (1.0 - fraction)).max(0.0);
                }
            } else if operator.identity.eq_ignore_ascii_case("Lifespan Decay") {
                if age >= particle.lifetime_seconds {
                    particle.lifetime_seconds = 0.0;
                }
            } else if operator.identity.eq_ignore_ascii_case("Remap Scalar") {
                let input_field = integer_parameter(operator, "input field", 7);
                let input = if input_field == 8 {
                    age
                } else {
                    particle_scalar(particle, input_field).unwrap_or(0.0)
                };
                let mapped = mix(
                    float_parameter(operator, "output minimum", 0.0),
                    float_parameter(operator, "output maximum", 1.0),
                    remap(
                        input,
                        float_parameter(operator, "input minimum", 0.0),
                        float_parameter(operator, "input maximum", 1.0),
                    ),
                );
                let output = integer_parameter(operator, "output field", 3);
                let previous = particle_scalar(particle, output).unwrap_or(0.0);
                set_particle_scalar(particle, output, mix(previous, mapped, strength));
            } else if operator.identity.eq_ignore_ascii_case("Radius Scale") {
                let start_time = float_parameter(operator, "start_time", 0.0);
                let end_time = float_parameter(operator, "end_time", 1.0);
                if end_time <= start_time || life < start_time || life >= end_time {
                    continue;
                }
                let mut fraction = remap(life, start_time, end_time);
                if bool_parameter(operator, "ease_in_and_out", false) {
                    fraction = spline(fraction);
                } else {
                    fraction = bias(fraction, float_parameter(operator, "scale_bias", 0.5));
                }
                particle.radius = particle.initial_radius
                    * mix(
                        float_parameter(operator, "radius_start_scale", 1.0),
                        float_parameter(operator, "radius_end_scale", 1.0),
                        fraction,
                    )
                    .max(0.0);
            } else if operator.identity.eq_ignore_ascii_case("Color Fade") {
                let target = color_parameter(operator, "color_fade", [255; 4]);
                let mut fraction = remap(
                    life,
                    float_parameter(operator, "fade_start_time", 0.0),
                    float_parameter(operator, "fade_end_time", 1.0),
                );
                if bool_parameter(operator, "ease_in_and_out", true) {
                    fraction = spline(fraction);
                }
                for ((color, initial), target) in particle
                    .color
                    .iter_mut()
                    .zip(particle.initial_color)
                    .zip(target)
                {
                    *color = mix(initial, target as f32 / 255.0, fraction);
                }
            } else if operator.identity.eq_ignore_ascii_case("Rotation Basic") {
                particle.roll += particle.roll_speed * dt.min(age) * strength;
            } else if operator.identity.eq_ignore_ascii_case("Rotation Spin Roll") {
                let stop = float_parameter(operator, "spin_stop_time", 0.0);
                let rate = integer_parameter(operator, "spin_rate_degrees", 0) as f32
                    * std::f32::consts::PI
                    / 180.0
                    * std::f32::consts::TAU
                    * strength;
                let minimum = integer_parameter(operator, "spin_rate_min", 0) as f32
                    * std::f32::consts::PI
                    / 180.0
                    * std::f32::consts::TAU;
                let fade = if stop > 0.0 {
                    (1.0 - age / (particle.lifetime_seconds * stop)).max(0.0)
                } else {
                    1.0
                };
                let delta = (rate * dt * fade).max(minimum * dt);
                particle.roll = wrap_angle(particle.roll + delta);
            } else if operator
                .identity
                .eq_ignore_ascii_case("Movement Lock to Control Point")
            {
                let (current_control, previous_control, snapshot_previous) =
                    position_lock.expect("position lock context");
                let particle_previous_control = if particle.creation_seconds >= time - dt {
                    interpolate_control_position(
                        snapshot_previous,
                        current_control,
                        time,
                        dt,
                        particle.creation_seconds,
                    )
                } else {
                    previous_control
                };
                let delta = sub(current_control, particle_previous_control);
                let start = source_random_exp(
                    random_seed,
                    particle.identity as i32 + random_offset + 9,
                    float_parameter(operator, "start_fadeout_min", 1.0),
                    float_parameter(operator, "start_fadeout_max", 1.0),
                    float_parameter(operator, "start_fadeout_exponent", 1.0),
                );
                let end = source_random_exp(
                    random_seed,
                    particle.identity as i32 + random_offset + 10,
                    float_parameter(operator, "end_fadeout_min", 1.0),
                    float_parameter(operator, "end_fadeout_max", 1.0),
                    float_parameter(operator, "end_fadeout_exponent", 1.0),
                );
                let lock = 1.0 - spline(remap(life, start, end));
                let creation_bias = if dt > 0.0 { age.min(dt) / dt } else { 1.0 };
                let movement = mul(delta, strength * lock * creation_bias);
                particle.position = add(particle.position, movement);
                particle.previous_position = add(particle.previous_position, movement);
            } else if operator.identity.eq_ignore_ascii_case("Movement Follow CP") {
                let minimum = integer_parameter(operator, "starting control point", 0);
                let maximum = integer_parameter(operator, "maximum end control point", 0);
                let index = source_clamp(i32::from(particle.target_control_point), minimum, maximum);
                let Some(control) = controls.get(index as usize).and_then(Option::as_ref) else {
                    continue;
                };
                let delta = sub(control.position, particle.position);
                let distance = length_squared(delta).sqrt();
                if distance > 0.0 {
                    let velocity = length_squared(control.velocity).sqrt();
                    let catch_up = float_parameter(operator, "catch up speed", 0.0);
                    let speed = if dot(control.velocity, delta) > 1.0 {
                        velocity + catch_up
                    } else {
                        velocity
                    };
                    let movement = mul(delta, speed * dt * strength / distance);
                    particle.previous_position = particle.position;
                    particle.position = add(particle.position, movement);
                }
                let radius_speed = float_parameter(operator, "lerp to CP radius speed", 0.0);
                if radius_speed > 0.0 {
                    let difference = control.radius - particle.radius;
                    let step = radius_speed * dt;
                    particle.radius += source_clamp(difference, -step, step);
                }
                if bool_parameter(operator, "update particle life time", false) {
                    particle.lifetime_seconds = control.duration;
                }
            } else if operator
                .identity
                .eq_ignore_ascii_case("Remap Distance to Control Point to Scalar")
            {
                let cp =
                    control_at_slice(controls, integer_parameter(operator, "control point", 0));
                let distance = length_squared(sub(cp, particle.position)).sqrt();
                let minimum = float_parameter(operator, "distance minimum", 0.0);
                let maximum = float_parameter(operator, "distance maximum", 128.0);
                if bool_parameter(operator, "only active within specified distance", false)
                    && (distance < minimum || distance > maximum)
                {
                    continue;
                }
                let field = integer_parameter(operator, "output field", 3);
                let min_output = float_parameter(operator, "output minimum", 0.0);
                let max_output = float_parameter(operator, "output maximum", 1.0);
                let mut output = mix(min_output, max_output, remap(distance, minimum, maximum));
                if bool_parameter(operator, "output is scalar of initial random range", false) {
                    output *= match field {
                        3 => particle.initial_radius,
                        4 => particle.initial_roll,
                        7 => particle.initial_alpha,
                        _ => particle_scalar(particle, field).unwrap_or(0.0),
                    };
                }
                let current = particle_scalar(particle, field).unwrap_or(0.0);
                set_particle_scalar(particle, field, mix(current, output, strength));
            } else if operator
                .identity
                .eq_ignore_ascii_case("Remap Distance to Control Point to Vector")
            {
                let cp =
                    control_at_slice(controls, integer_parameter(operator, "control point", 0));
                let distance = length_squared(sub(cp, particle.position)).sqrt();
                let minimum = float_parameter(operator, "distance minimum", 0.0);
                let maximum = float_parameter(operator, "distance maximum", 128.0);
                if bool_parameter(operator, "only active within specified distance", false)
                    && (distance < minimum || distance > maximum)
                {
                    continue;
                }
                let mut low = vector_parameter(operator, "output minimum", [0.0; 3]);
                let mut high = vector_parameter(operator, "output maximum", [1.0; 3]);
                let local = integer_parameter(operator, "local space CP", -1);
                if local >= 0
                    && let Some(orientation) = controls
                        .get(local as usize)
                        .and_then(Option::as_ref)
                        .map(|control| control.orientation)
                {
                    low = rotate(orientation, low);
                    high = rotate(orientation, high);
                }
                let fraction = remap(distance, minimum, maximum);
                let output = [
                    mix(low[0], high[0], fraction),
                    mix(low[1], high[1], fraction),
                    mix(low[2], high[2], fraction),
                ];
                let field = integer_parameter(operator, "output field", 6);
                let current = particle_vector(particle, field).unwrap_or([0.0; 3]);
                set_particle_vector(
                    particle,
                    field,
                    [
                        mix(current[0], output[0], strength),
                        mix(current[1], output[1], strength),
                        mix(current[2], output[2], strength),
                    ],
                );
            } else if operator.identity.eq_ignore_ascii_case("Oscillate Vector") {
                let random = |ordinal: i32| {
                    source_random_at(
                        random_seed,
                        particle.identity as i32 + random_offset + ordinal,
                    )
                };
                let operation_time = if bool_parameter(operator, "start/end proportional", true) {
                    life
                } else {
                    age
                };
                let start = mix(
                    float_parameter(operator, "start time min", 0.0),
                    float_parameter(operator, "start time max", 0.0),
                    random(11),
                );
                let end = mix(
                    float_parameter(operator, "end time min", 1.0),
                    float_parameter(operator, "end time max", 1.0),
                    random(12),
                );
                if operation_time < start || operation_time >= end {
                    continue;
                }
                let rate_min = vector_parameter(operator, "oscillation rate min", [0.0; 3]);
                let rate_max = vector_parameter(operator, "oscillation rate max", [0.0; 3]);
                let frequency_min =
                    vector_parameter(operator, "oscillation frequency min", [1.0; 3]);
                let frequency_max =
                    vector_parameter(operator, "oscillation frequency max", [1.0; 3]);
                let field = integer_parameter(operator, "oscillation field", 0);
                let Some(mut output) = particle_vector(particle, field) else {
                    continue;
                };
                for (component, (rate_ordinal, frequency_ordinal)) in
                    [(3, 8), (7, 12), (9, 15)].into_iter().enumerate()
                {
                    let rate = mix(
                        rate_min[component],
                        rate_max[component],
                        random(rate_ordinal),
                    );
                    let frequency = mix(
                        frequency_min[component],
                        frequency_max[component],
                        random(frequency_ordinal),
                    );
                    let multiplier = float_parameter(operator, "oscillation multiplier", 2.0);
                    let offset = float_parameter(operator, "oscillation start phase", 0.5);
                    let phase = if bool_parameter(operator, "proportional 0/1", true) {
                        multiplier * life * frequency + offset
                    } else {
                        (multiplier * time + offset) * frequency
                    };
                    output[component] += sin_estimate_cycles(phase) * rate * dt * strength;
                }
                set_particle_vector(particle, field, output);
            } else if operator.identity.eq_ignore_ascii_case("Oscillate Scalar") {
                let random = |ordinal: i32| {
                    source_random_at(
                        random_seed,
                        particle.identity as i32 + random_offset + ordinal,
                    )
                };
                let operation_time = if bool_parameter(operator, "start/end proportional", true) {
                    life
                } else {
                    age
                };
                let start = mix(
                    float_parameter(operator, "start time min", 0.0),
                    float_parameter(operator, "start time max", 0.0),
                    random(11),
                );
                let end = mix(
                    float_parameter(operator, "end time min", 1.0),
                    float_parameter(operator, "end time max", 1.0),
                    random(12),
                );
                if operation_time < start || operation_time >= end {
                    continue;
                }
                let field = integer_parameter(operator, "oscillation field", 7);
                let rate = mix(
                    float_parameter(operator, "oscillation rate min", 0.0),
                    float_parameter(operator, "oscillation rate max", 0.0),
                    random(1),
                );
                let frequency = mix(
                    float_parameter(operator, "oscillation frequency min", 1.0),
                    float_parameter(operator, "oscillation frequency max", 1.0),
                    random(0),
                );
                let proportional = bool_parameter(operator, "proportional 0/1", true);
                let multiplier = float_parameter(operator, "oscillation multiplier", 2.0);
                let phase_start = float_parameter(operator, "oscillation start phase", 0.5);
                let phase = if proportional {
                    multiplier * life * frequency + phase_start
                } else {
                    (multiplier * time + phase_start) * frequency
                };
                let value = sin_estimate_cycles(phase) * rate * dt * strength;
                match field {
                    3 => particle.radius = (particle.radius + value).max(0.0),
                    4 => particle.roll += value,
                    7 => particle.alpha = (particle.alpha + value).clamp(0.0, 1.0),
                    _ => {}
                }
            }
        }
        system
            .particles
            .retain(|particle| particle.lifetime_seconds > 0.0);
    }
}

fn operate_movement(
    system: &mut System,
    definition: &Definition,
    operator: &Function,
    time: f32,
    dt: f32,
    simulation_random: &mut SimdRandom,
) {
    let gravity = vector_parameter(operator, "gravity", [0.0; 3]);
    let mut accelerations = vec![gravity; system.particles.len()];
    for force in definition.functions(FunctionCategory::Force) {
        let strength = operator_strength(force, time);
        if strength <= 0.0 {
            continue;
        }
        if force.identity.eq_ignore_ascii_case("twist around axis") {
            let mut axis = vector_parameter(force, "twist axis", [0.0, 0.0, 1.0]);
            if bool_parameter(force, "object local space axis 0/1", false)
                && let Some(orientation) = control_orientation(system, 0)
            {
                axis = rotate(orientation, axis);
            }
            let Some(axis) = normalize(axis) else {
                continue;
            };
            let origin = control_at(system, 0);
            let amount = float_parameter(force, "amount of force", 0.0) * strength;
            for (particle, acceleration) in system.particles.iter().zip(&mut accelerations) {
                let Some(offset) = normalize(sub(particle.position, origin)) else {
                    continue;
                };
                let Some(radial) = normalize(sub(offset, mul(axis, dot(offset, axis)))) else {
                    continue;
                };
                *acceleration = add(*acceleration, mul(cross(radial, axis), amount));
            }
            continue;
        }
        if !force.identity.eq_ignore_ascii_case("random force") {
            continue;
        }
        let minimum = mul(vector_parameter(force, "min force", [0.0; 3]), strength);
        let maximum = mul(vector_parameter(force, "max force", [0.0; 3]), strength);
        for chunk in accelerations.chunks_mut(4) {
            let x = simulation_random.sample4();
            let y = simulation_random.sample4();
            let z = simulation_random.sample4();
            for (lane, acceleration) in chunk.iter_mut().enumerate() {
                acceleration[0] += mix(minimum[0], maximum[0], x[lane]);
                acceleration[1] += mix(minimum[1], maximum[1], y[lane]);
                acceleration[2] += mix(minimum[2], maximum[2], z[lane]);
            }
        }
    }
    let drag = float_parameter(operator, "drag", 0.0).max(0.0);
    let damping = (1.0 - drag).max(0.0).powf(dt * 30.0);
    let ratio = dt / system.previous_step.max(f32::EPSILON);
    for (particle, acceleration) in system.particles.iter_mut().zip(accelerations) {
        let previous = particle.position;
        let delta = mul(
            sub(particle.position, particle.previous_position),
            ratio * damping,
        );
        particle.position = add(add(particle.position, delta), mul(acceleration, dt * dt));
        particle.previous_position = previous;
    }
}

fn constrain(
    system: &mut System,
    definition: &Definition,
    effect_identity: u32,
    limits: WorldLimits,
    queries: &mut usize,
    collision: &mut impl CollisionQuery,
    collision_cache: &mut CollisionPlaneCache,
) -> Result<(), Error> {
    for (constraint_index, constraint) in definition
        .functions(FunctionCategory::Constraint)
        .enumerate()
    {
        if constraint
            .identity
            .eq_ignore_ascii_case("Constrain distance to path between two control points")
        {
            let (start, midpoint, end) =
                particle_path(system, constraint, system.local_time, "random bulge");
            let travel = float_parameter(constraint, "travel time", 10.0).max(0.001);
            let minimum = float_parameter(constraint, "minimum distance", 0.0);
            let beginning = float_parameter(constraint, "maximum distance", 100.0);
            let authored_middle = float_parameter(constraint, "maximum distance middle", -1.0);
            let middle = if authored_middle < 0.0 {
                beginning
            } else {
                authored_middle
            };
            let authored_end = float_parameter(constraint, "maximum distance end", -1.0);
            let ending = if authored_end < 0.0 {
                middle
            } else {
                authored_end
            };
            for particle in &mut system.particles {
                let fraction = ((system.local_time - particle.creation_seconds) / travel).min(1.0);
                let first = add(start, mul(sub(midpoint, start), fraction));
                let second = add(midpoint, mul(sub(end, midpoint), fraction));
                let center = add(first, mul(sub(second, first), fraction));
                let maximum = mix(
                    mix(beginning, middle, fraction),
                    mix(middle, ending, fraction),
                    fraction,
                );
                let offset = sub(particle.position, center);
                let distance = dot(offset, offset).sqrt();
                if distance > maximum || distance < minimum {
                    if let Some(direction) = normalize(offset) {
                        particle.position =
                            add(center, mul(direction, source_clamp(distance, minimum, maximum)));
                    }
                }
            }
            continue;
        }
        if !constraint
            .identity
            .eq_ignore_ascii_case("Collision via traces")
        {
            continue;
        }
        let mode = integer_parameter(constraint, "collision mode", 0);
        if mode != 0 {
            update_collision_planes(
                system,
                constraint,
                constraint_index,
                effect_identity,
                limits,
                queries,
                collision,
                collision_cache,
            )?;
        }
        let radius_scale = float_parameter(constraint, "radius scale", 1.0);
        let bounce = float_parameter(constraint, "amount of bounce", 0.0);
        let slide_scale = float_parameter(constraint, "amount of slide", 0.0);
        let kill = bool_parameter(constraint, "kill particle on collision", false);
        for particle in &mut system.particles {
            let movement = sub(particle.position, particle.previous_position);
            let Some(direction) = normalize(movement) else {
                continue;
            };
            let end = add(
                particle.position,
                mul(direction, particle.radius * radius_scale),
            );
            let mut intersection = 2.0_f32;
            let mut normal = [0.0; 3];
            if mode == 0 {
                *queries += 1;
                if *queries > limits.max_queries_per_advance {
                    return Err(Error::new(
                        ErrorCode::BoundExceeded,
                        &definition.source,
                        0,
                        "particle collision queries exceed the configured limit",
                    ));
                }
                let identity = hash(
                    system.path_identity,
                    hash(particle.identity as u64, constraint_index as u64),
                );
                let request = TraceRequest {
                    identity,
                    effect_identity,
                    system_uuid: system.definition_uuid,
                    particle_identity: particle.identity,
                    start: particle.previous_position,
                    end,
                    radius: 0.0,
                    brush_only: bool_parameter(constraint, "brush only", false),
                    collision_group: string_parameter(constraint, "collision group", "NONE")
                        .to_owned(),
                };
                let results = collision.trace_batch(&[request])?;
                let result = results.first().ok_or_else(|| {
                    Error::new(
                        ErrorCode::MissingQuery,
                        &definition.source,
                        0,
                        "particle trace is absent",
                    )
                })?;
                intersection = result.fraction;
                normal = result.normal;
            } else {
                for plane in &collision_cache.planes {
                    let start_distance =
                        dot(sub(particle.previous_position, plane.point), plane.normal);
                    let end_distance = dot(sub(end, plane.point), plane.normal);
                    if start_distance >= 0.0 && end_distance < 0.0 {
                        let fraction = start_distance / (start_distance - end_distance);
                        if fraction < intersection {
                            intersection = fraction;
                            normal = plane.normal;
                        }
                    }
                }
            }
            if intersection >= 1.0 {
                continue;
            }
            if kill {
                particle.lifetime_seconds = 0.0;
                continue;
            }
            let mut point = add(particle.previous_position, mul(movement, intersection));
            if bounce != 0.0 || slide_scale != 0.0 {
                let reflected = sub(direction, mul(normal, 2.0 * dot(direction, normal)));
                let slide = sub(movement, mul(normal, dot(movement, normal)));
                let velocity = add(mul(reflected, bounce), mul(slide, slide_scale));
                point = add(point, mul(velocity, 1.0 - intersection));
                particle.previous_position = sub(point, velocity);
            }
            particle.position = point;
        }
        system
            .particles
            .retain(|particle| particle.lifetime_seconds > 0.0);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn update_collision_planes(
    system: &System,
    constraint: &Function,
    constraint_index: usize,
    effect_identity: u32,
    limits: WorldLimits,
    queries: &mut usize,
    collision: &mut impl CollisionQuery,
    cache: &mut CollisionPlaneCache,
) -> Result<(), Error> {
    let origin = add(
        control_at(system, 0),
        vector_parameter(
            constraint,
            "control point offset for fast collisions",
            [0.0; 3],
        ),
    );
    let tolerance = float_parameter(constraint, "control point movement distance tolerance", 5.0);
    if cache.initialized
        && cache.last_update_seconds > 0.0
        && length_squared(sub(origin, cache.last_origin)) < tolerance * tolerance
    {
        cache.last_update_seconds = system.local_time;
        return Ok(());
    }
    let mut requests = Vec::with_capacity(52);
    let collision_group = string_parameter(constraint, "collision group", "NONE");
    for pass in 0..2_u64 {
        for x in -1..=1_i32 {
            for y in -1..=1_i32 {
                for z in -1..=1_i32 {
                    if x == 0 && y == 0 && z == 0 {
                        continue;
                    }
                    let ordinal = requests.len() as u64;
                    let mut identity = hash(
                        system.path_identity,
                        ((constraint_index as u64) << 56) | (pass << 48) | ordinal,
                    );
                    for value in origin {
                        identity = hash(identity, value.to_bits() as u64);
                    }
                    requests.push(TraceRequest {
                        identity,
                        effect_identity,
                        system_uuid: system.definition_uuid,
                        particle_identity: 0,
                        start: origin,
                        end: add(
                            origin,
                            [x as f32 * 1_000.0, y as f32 * 1_000.0, z as f32 * 1_000.0],
                        ),
                        radius: 0.0,
                        brush_only: false,
                        collision_group: collision_group.clone(),
                    });
                }
            }
        }
    }
    *queries = queries.checked_add(requests.len()).ok_or_else(|| {
        Error::new(
            ErrorCode::BoundExceeded,
            "particle-world",
            0,
            "query count overflowed",
        )
    })?;
    if *queries > limits.max_queries_per_advance {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-world",
            0,
            "collision query count exceeds max_queries_per_advance",
        ));
    }
    let results = collision.trace_batch(&requests)?;
    if results.len() != requests.len() {
        return Err(Error::new(
            ErrorCode::MissingQuery,
            "particle-world",
            0,
            "collision batch result count does not match its requests",
        ));
    }
    for (request, result) in requests.iter().zip(&results) {
        validate_collision_result(request, result)?;
    }
    cache.planes.clear();
    for (request, result) in requests[26..].iter().zip(&results[26..]) {
        if result.fraction < 1.0 {
            cache.planes.push(CollisionPlane {
                point: add(
                    request.start,
                    mul(sub(request.end, request.start), result.fraction),
                ),
                normal: result.normal,
            });
        }
    }
    cache.last_origin = origin;
    cache.last_update_seconds = system.local_time;
    cache.initialized = true;
    Ok(())
}

fn render_system(
    system: &System,
    registry: &Registry,
    effect_identity: u32,
    camera: [f32; 3],
    output: &mut Vec<RenderItem>,
    limit: usize,
) -> Result<(), Error> {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    let mut particles: Vec<&Particle> = system.particles.iter().collect();
    if bool_attribute(definition, "sort particles", false) {
        particles.sort_by(|left, right| {
            let left_distance = length_squared(sub(left.position, camera));
            let right_distance = length_squared(sub(right.position, camera));
            right_distance
                .total_cmp(&left_distance)
                .then(left.identity.cmp(&right.identity))
        });
    } else {
        particles.reverse();
    }
    for (renderer_index, renderer) in definition.functions(FunctionCategory::Renderer).enumerate() {
        let primitive = if renderer
            .identity
            .eq_ignore_ascii_case("render_sprite_trail")
        {
            Primitive::Trail
        } else {
            Primitive::Sprite
        };
        for particle in &particles {
            if output.len() >= limit {
                return Err(Error::new(
                    ErrorCode::BoundExceeded,
                    "particle-world",
                    0,
                    "render item count exceeds max_render_items",
                ));
            }
            let distance = length_squared(sub(particle.position, camera));
            let item = RenderItem {
                effect_identity,
                system_uuid: system.definition_uuid,
                sky: false,
                particle_identity: particle.identity,
                renderer_index: renderer_index as u16,
                primitive,
                material: definition.material.clone(),
                position: particle.position,
                previous_position: particle.previous_position,
                radius: particle.radius,
                roll_radians: particle.roll,
                yaw_radians: particle.yaw,
                color: particle
                    .color
                    .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8),
                opacity: (particle.alpha.clamp(0.0, 1.0) * 255.0).round() / 255.0,
                sequence: particle.sequence,
                secondary_sequence: particle.secondary_sequence,
                trail_length_scale: particle.trail_length,
                sort_key: distance,
                age_seconds: (system.local_time - particle.creation_seconds).max(0.0),
                lifetime_seconds: particle.lifetime_seconds,
                animation_rate: float_parameter(renderer, "animation rate", 0.1),
                secondary_animation_rate: float_parameter(
                    renderer,
                    "second sequence animation rate",
                    0.0,
                ),
                step_seconds: if system.current_step > 0.0 {
                    system.current_step
                } else {
                    1.0
                },
                trail_min_length: float_parameter(renderer, "min length", 0.0).max(0.0),
                trail_max_length: float_parameter(renderer, "max length", 2_000.0).max(0.0),
                trail_fade_in_seconds: float_parameter(renderer, "length fade in time", 0.0)
                    .max(0.0),
                orientation_type: integer_parameter(renderer, "orientation_type", 0),
                animation_fit_lifetime: bool_parameter(renderer, "animation_fit_lifetime", false),
                animation_rate_as_fps: bool_parameter(renderer, "use animation rate as FPS", false),
                primary_sheet: None,
                secondary_sheet: None,
                trail_end_position: particle.position,
                trail_width: 0.0,
                trail_length: 0.0,
                material_state: None,
                stable_tie_identity: hash(system.path_identity, particle.identity as u64),
            };
            output.push(item);
        }
    }
    for child in &system.children {
        render_system(child, registry, effect_identity, camera, output, limit)?;
    }
    Ok(())
}

fn finished(system: &System, registry: &Registry, time: f32) -> bool {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    let local = (time - system.start_seconds - system.delay_seconds).max(0.0);
    let emitters_finished = definition
        .functions(FunctionCategory::Emitter)
        .zip(&system.emitter_contexts)
        .all(|(emitter, context)| match context {
            EmitterContext::Instantaneous { remaining, .. } => *remaining <= 0,
            EmitterContext::Continuous {
                time_offset,
                stopped,
                ..
            } => {
                let duration = float_parameter(emitter, "emission_duration", 0.0);
                *stopped
                    || float_parameter(emitter, "emission_rate", 100.0) <= 0.0
                    || (duration > 0.0
                        && local
                            >= float_parameter(emitter, "emission_start_time", 0.0)
                                + *time_offset
                                + duration)
            }
        });
    !system.dormant
        && !system.first_frame
        && emitters_finished
        && system.particles.is_empty()
        && system
            .children
            .iter()
            .all(|child| finished(child, registry, time))
}

fn validate_advance(
    current: f32,
    events: &[Event],
    request: AdvanceRequest,
    limits: WorldLimits,
) -> Result<(), Error> {
    if events.len() > limits.max_events_per_advance {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-world",
            0,
            "event count exceeds max_events_per_advance",
        ));
    }
    if !finite(&[
        request.from_seconds,
        request.to_seconds,
        request.maximum_step_seconds,
        request.camera_position[0],
        request.camera_position[1],
        request.camera_position[2],
    ]) || request.maximum_step_seconds <= 0.0
        || request.from_seconds < 0.0
        || request.to_seconds < request.from_seconds
        || request.from_seconds != current
    {
        return Err(Error::new(
            ErrorCode::TimeReversed,
            "particle-world",
            0,
            "advance range is invalid or does not begin at current time",
        ));
    }
    let mut previous = (request.from_seconds, 0_u32);
    for (index, event) in events.iter().enumerate() {
        if event.identity == 0
            || !event.timestamp_seconds.is_finite()
            || event.timestamp_seconds < request.from_seconds
            || event.timestamp_seconds > request.to_seconds
            || (index > 0 && (event.timestamp_seconds, event.source_order) < previous)
        {
            return Err(Error::new(
                ErrorCode::InvalidEvent,
                "particle-world",
                index,
                "event ordering or timestamp is invalid",
            ));
        }
        previous = (event.timestamp_seconds, event.source_order);
    }
    Ok(())
}

fn validate_limits(limits: WorldLimits) -> Result<(), Error> {
    if [
        limits.max_effects,
        limits.max_systems,
        limits.max_particles_per_system,
        limits.max_particles_total,
        limits.max_control_points,
        limits.max_events_per_advance,
        limits.max_substeps,
        limits.max_queries_per_advance,
        limits.max_render_items,
    ]
    .contains(&0)
        || limits.max_particles_per_system > 5_000
        || limits.max_control_points > CONTROL_POINT_LIMIT
    {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-world",
            0,
            "world limits are zero or exceed Source PC limits",
        ));
    }
    Ok(())
}

fn validate_controls(controls: &[ControlPoint], limits: WorldLimits) -> Result<(), Error> {
    if controls.len() > limits.max_control_points {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            "particle-world",
            0,
            "control point count exceeds max_control_points",
        ));
    }
    let mut indexes = BTreeSet::new();
    for control in controls {
        validate_control_point(control, limits)?;
        if !indexes.insert(control.index) {
            return Err(Error::new(
                ErrorCode::InvalidEvent,
                "particle-world",
                0,
                "control point index is duplicated",
            ));
        }
    }
    for control in controls {
        if let Some(parent) = control.parent {
            if parent == control.index || !indexes.contains(&parent) {
                return Err(Error::new(
                    ErrorCode::InvalidEvent,
                    "particle-world",
                    0,
                    "control point parent is missing or self-referential",
                ));
            }
            let mut cursor = parent;
            let mut depth = 0;
            loop {
                depth += 1;
                if depth > controls.len() {
                    return Err(Error::new(
                        ErrorCode::InvalidEvent,
                        "particle-world",
                        0,
                        "control point parent graph contains a cycle",
                    ));
                }
                let next = controls
                    .iter()
                    .find(|candidate| candidate.index == cursor)
                    .and_then(|candidate| candidate.parent);
                match next {
                    Some(value) if value == control.index => {
                        return Err(Error::new(
                            ErrorCode::InvalidEvent,
                            "particle-world",
                            0,
                            "control point parent graph contains a cycle",
                        ));
                    }
                    Some(value) => cursor = value,
                    None => break,
                }
            }
        }
    }
    Ok(())
}

fn validate_control_point(control: &ControlPoint, limits: WorldLimits) -> Result<(), Error> {
    if control.index as usize >= limits.max_control_points
        || control
            .parent
            .is_some_and(|parent| parent as usize >= limits.max_control_points)
        || !finite(&control.position)
        || !finite(&control.previous_position)
        || !finite(&control.orientation)
        || !finite(&control.velocity)
        || !control.radius.is_finite()
        || !control.density.is_finite()
        || !control.duration.is_finite()
        || control.radius < 0.0
        || control.density < 0.0
        || control.duration < 0.0
        || (length_squared4(control.orientation) - 1.0).abs() > 1.0e-4
    {
        return Err(Error::new(
            ErrorCode::InvalidEvent,
            "particle-world",
            0,
            "control point is invalid",
        ));
    }
    Ok(())
}

fn validate_collision_result(
    request: &TraceRequest,
    result: &CollisionResult,
) -> Result<(), Error> {
    if result.identity != request.identity
        || !result.fraction.is_finite()
        || !(0.0..=1.0).contains(&result.fraction)
        || !finite(&result.normal)
        || (result.fraction < 1.0
            && !result.start_solid
            && (length_squared(result.normal) - 1.0).abs() > 1.0e-4)
    {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-world",
            0,
            "collision result is malformed or does not match its request",
        ));
    }
    Ok(())
}

fn effect_mut(effects: &mut [Effect], identity: u32) -> Result<&mut Effect, Error> {
    effects
        .iter_mut()
        .find(|effect| effect.identity == identity)
        .ok_or_else(|| invalid_state("event targets a missing effect"))
}

fn invalid_state(detail: &str) -> Error {
    Error::new(ErrorCode::InvalidState, "particle-world", 0, detail)
}

fn set_control(system: &mut System, control: ControlPoint) {
    if control.index != 0 {
        system.target_control_point = control.index;
    }
    if system.controls[control.index as usize].as_ref() != Some(&control) {
        Arc::make_mut(&mut system.controls)[control.index as usize] = Some(control.clone());
    }
    for child in &mut system.children {
        set_control(child, control.clone());
    }
}

fn set_emission(system: &mut System, active: bool, clear: bool) {
    if !active {
        system.dormant = false;
    }
    system.emission_active = active;
    for context in &mut system.emitter_contexts {
        match context {
            EmitterContext::Continuous { stopped, .. } => *stopped = !active,
            EmitterContext::Instantaneous {
                remaining,
                actual,
                start_time,
                time_offset,
            } => {
                if active {
                    *remaining = *actual;
                    if system.local_time > *start_time + *time_offset + 2.0 {
                        *remaining = 0;
                    }
                } else {
                    *remaining = 0;
                }
            }
        }
    }
    if clear {
        system.particles.clear();
    }
    for child in &mut system.children {
        set_emission(child, active, clear);
    }
}

fn set_dormant(system: &mut System, dormant: bool) {
    if dormant == system.dormant || (dormant && !system.emission_active) {
        return;
    }
    set_emission(system, !dormant, false);
    system.dormant = dormant;
}

fn restart_system(system: &mut System, registry: &Registry) {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    for (emitter, context) in definition
        .functions(FunctionCategory::Emitter)
        .zip(&mut system.emitter_contexts)
    {
        match context {
            EmitterContext::Continuous {
                total_actual,
                emitted,
                time_offset,
                ..
            } => {
                if float_parameter(emitter, "emission_duration", 0.0) > 0.0 {
                    *time_offset = system.local_time;
                    *total_actual = 0.0;
                    *emitted = 0;
                }
            }
            EmitterContext::Instantaneous {
                remaining,
                actual,
                time_offset,
                ..
            } => {
                *remaining = *actual;
                *time_offset = system.local_time;
            }
        }
    }
    for child in &mut system.children {
        restart_system(child, registry);
    }
}

fn update_control_history(system: &mut System) {
    if system
        .controls
        .iter()
        .flatten()
        .any(|control| control.previous_position != control.position)
    {
        for control in Arc::make_mut(&mut system.controls).iter_mut().flatten() {
            control.previous_position = control.position;
        }
    }
}

fn particle_count(effects: &[Effect]) -> usize {
    effects
        .iter()
        .map(|effect| system_particle_count(&effect.root))
        .sum()
}

fn system_particle_count(system: &System) -> usize {
    system.particles.len()
        + system
            .children
            .iter()
            .map(system_particle_count)
            .sum::<usize>()
}

fn maximum_system_step(system: &System, registry: &Registry) -> f32 {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    let own = float_attribute(definition, "maximum time step", 0.1);
    let own = if own > 0.0 { own } else { 0.1 };
    system
        .children
        .iter()
        .map(|child| maximum_system_step(child, registry))
        .fold(own, f32::min)
}

fn control_at(system: &System, index: i32) -> [f32; 3] {
    control_at_slice(&system.controls, index)
}

fn control_at_slice(controls: &[Option<ControlPoint>], index: i32) -> [f32; 3] {
    controls
        .get(index.max(0) as usize)
        .and_then(Option::as_ref)
        .map_or([0.0; 3], |control| control.position)
}

fn control_at_time(system: &System, index: i32, time: f32) -> [f32; 3] {
    let Some(control) = system
        .controls
        .get(index.max(0) as usize)
        .and_then(Option::as_ref)
    else {
        return [0.0; 3];
    };
    interpolate_control_position(
        control.previous_position,
        control.position,
        system.local_time,
        system.current_step,
        time,
    )
}

fn interpolate_control_position(
    previous: [f32; 3],
    current: [f32; 3],
    current_time: f32,
    step: f32,
    sample_time: f32,
) -> [f32; 3] {
    if step <= 0.0 {
        return current;
    }
    let how_long_ago = current_time - sample_time;
    let fraction = ((step - how_long_ago) / step).max(0.0);
    [
        mix(previous[0], current[0], fraction),
        mix(previous[1], current[1], fraction),
        mix(previous[2], current[2], fraction),
    ]
}

fn control_orientation(system: &System, index: i32) -> Option<[f32; 4]> {
    system
        .controls
        .get(index.max(0) as usize)
        .and_then(Option::as_ref)
        .map(|control| control.orientation)
}

fn integer_attribute(definition: &Definition, names: &[&str], default: i32) -> i32 {
    names
        .iter()
        .find_map(|name| match definition.attribute(name) {
            Some(Value::Int(value)) => Some(*value),
            _ => None,
        })
        .unwrap_or(default)
}

fn float_attribute(definition: &Definition, name: &str, default: f32) -> f32 {
    match definition.attribute(name) {
        Some(Value::Float(value)) => *value,
        _ => default,
    }
}

fn vector_attribute(definition: &Definition, name: &str, default: [f32; 3]) -> [f32; 3] {
    match definition.attribute(name) {
        Some(Value::Vector3(value)) => *value,
        _ => default,
    }
}

fn color_attribute(definition: &Definition, name: &str, default: [u8; 4]) -> [u8; 4] {
    match definition.attribute(name) {
        Some(Value::Color(value)) => *value,
        _ => default,
    }
}

fn bool_attribute(definition: &Definition, name: &str, default: bool) -> bool {
    match definition.attribute(name) {
        Some(Value::Bool(value)) => *value,
        _ => default,
    }
}

fn float_parameter(function: &Function, name: &str, default: f32) -> f32 {
    match function.parameter(name) {
        Some(Value::Float(value)) => *value,
        _ => default,
    }
}

fn integer_parameter(function: &Function, name: &str, default: i32) -> i32 {
    match function.parameter(name) {
        Some(Value::Int(value)) => *value,
        _ => default,
    }
}

fn bool_parameter(function: &Function, name: &str, default: bool) -> bool {
    match function.parameter(name) {
        Some(Value::Bool(value)) => *value,
        _ => default,
    }
}

fn vector_parameter(function: &Function, name: &str, default: [f32; 3]) -> [f32; 3] {
    match function.parameter(name) {
        Some(Value::Vector3(value)) => *value,
        _ => default,
    }
}

fn color_parameter(function: &Function, name: &str, default: [u8; 4]) -> [u8; 4] {
    match function.parameter(name) {
        Some(Value::Color(value)) => *value,
        _ => default,
    }
}

fn string_parameter(function: &Function, name: &str, default: &str) -> String {
    match function.parameter(name) {
        Some(Value::String(value)) => value.clone(),
        _ => default.to_owned(),
    }
}

fn system_bounds(system: &System, registry: &Registry) -> Option<Bounds> {
    let definition = registry
        .definition_at(system.definition_index)
        .expect("instantiated definition");
    let mut bounds = if system.particles.is_empty() {
        None
    } else {
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        for particle in &system.particles {
            for component in 0..3 {
                minimum[component] = minimum[component].min(particle.position[component]);
                maximum[component] = maximum[component].max(particle.position[component]);
            }
        }
        let expansion_min = vector_attribute(definition, "bounding_box_min", [-10.0; 3]);
        let expansion_max = vector_attribute(definition, "bounding_box_max", [10.0; 3]);
        for component in 0..3 {
            minimum[component] += expansion_min[component];
            maximum[component] += expansion_max[component];
        }
        Some(Bounds { minimum, maximum })
    };
    if system.simulated_frames > 2 && definition_reads_control_zero(definition) {
        let control = control_at(system, 0);
        match &mut bounds {
            Some(bounds) => {
                for (component, value) in control.into_iter().enumerate() {
                    bounds.minimum[component] = bounds.minimum[component].min(value);
                    bounds.maximum[component] = bounds.maximum[component].max(value);
                }
            }
            None => {
                bounds = Some(Bounds {
                    minimum: control,
                    maximum: control,
                });
            }
        }
    }
    for child in &system.children {
        if let Some(child_bounds) = system_bounds(child, registry) {
            merge_bounds(&mut bounds, child_bounds);
        }
    }
    bounds
}

fn definition_reads_control_zero(definition: &Definition) -> bool {
    definition.functions.iter().any(|function| {
        function.identity.eq_ignore_ascii_case("Color Random")
            || function
                .identity
                .eq_ignore_ascii_case("Collision via traces")
            || ((function
                .identity
                .eq_ignore_ascii_case("Position Within Sphere Random")
                || function
                    .identity
                    .eq_ignore_ascii_case("Position Modify Offset Random")
                || function
                    .identity
                    .eq_ignore_ascii_case("Movement Lock to Control Point"))
                && integer_parameter(function, "control_point_number", 0) == 0)
            || (function
                .identity
                .eq_ignore_ascii_case("Position Within Box Random")
                && integer_parameter(function, "control point number", 0) == 0)
    })
}

fn merge_bounds(bounds: &mut Option<Bounds>, incoming: Bounds) {
    match bounds {
        Some(bounds) => {
            for component in 0..3 {
                bounds.minimum[component] =
                    bounds.minimum[component].min(incoming.minimum[component]);
                bounds.maximum[component] =
                    bounds.maximum[component].max(incoming.maximum[component]);
            }
        }
        None => *bounds = Some(incoming),
    }
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|value| value.is_finite())
}

fn hash(left: u64, right: u64) -> u64 {
    let mut value = left ^ right.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// SDK mathlib::clamp returns the upper bound for reversed authored ranges.
fn source_clamp<T: PartialOrd + Copy>(value: T, minimum: T, maximum: T) -> T {
    if maximum < minimum { maximum }
    else if value < minimum { minimum }
    else if value > maximum { maximum }
    else { value }
}

#[test]
fn reversed_authored_bounds_match_sdk_clamp_without_panicking() {
    for value in [-1.0_f32, 21.0, 23.0] { assert_eq!(source_clamp(value, 22.0, 20.0), 20.0); }
    assert_eq!(source_clamp(5, 3, 1), 1);
    assert_eq!(source_clamp(0.5_f32, 0.0, 1.0), 0.5);
}

fn normalize_seed(seed: u64) -> i32 {
    let value = seed as u32 as i32;
    if value == 0 { 1 } else { value }
}

fn source_random_at(seed: i32, sample_identity: i32) -> f32 {
    let index = seed.wrapping_add(sample_identity) as usize & 4_095;
    f32::from_bits(SOURCE_RANDOM_FLOAT_BITS[index])
}

fn source_random_int(seed: i32, sample_identity: i32, minimum: i32, maximum: i32) -> i32 {
    minimum
        + (source_random_at(seed, sample_identity)
            * maximum.wrapping_add(1).wrapping_sub(minimum) as f32) as i32
}

fn source_random_exp(
    seed: i32,
    sample_identity: i32,
    minimum: f32,
    maximum: f32,
    exponent: f32,
) -> f32 {
    mix(
        minimum,
        maximum,
        source_random_at(seed, sample_identity).powf(exponent),
    )
}

fn next_random(system: &mut System) -> f32 {
    let identity = system.random_query_count;
    system.random_query_count = system.random_query_count.wrapping_add(1);
    source_random_at(system.random_seed, identity)
}

fn next_random_int(system: &mut System, minimum: i32, maximum: i32) -> i32 {
    let identity = system.random_query_count;
    system.random_query_count = system.random_query_count.wrapping_add(1);
    source_random_int(system.random_seed, identity, minimum, maximum)
}

fn next_random_vector(system: &mut System, minimum: [f32; 3], maximum: [f32; 3]) -> [f32; 3] {
    let identity = system.random_query_count;
    system.random_query_count = system.random_query_count.wrapping_add(1);
    let base = system.random_seed.wrapping_add(identity);
    [
        mix(
            minimum[0],
            maximum[0],
            source_random_at(system.random_seed, base),
        ),
        mix(
            minimum[1],
            maximum[1],
            source_random_at(system.random_seed, base.wrapping_add(1)),
        ),
        mix(
            minimum[2],
            maximum[2],
            source_random_at(system.random_seed, base.wrapping_add(2)),
        ),
    ]
}

fn next_random_in_unit_sphere(system: &mut System) -> ([f32; 3], f32) {
    let identity = system.random_query_count;
    system.random_query_count = system.random_query_count.wrapping_add(1);
    let range = |value: f32| 0.0001 + value * 0.9999;
    let u = range(source_random_at(system.random_seed, identity));
    let v = range(source_random_at(
        system.random_seed,
        identity.wrapping_add(1),
    ));
    let w = range(source_random_at(
        system.random_seed,
        identity.wrapping_add(2),
    ));
    let phi = (1.0 - 2.0 * u).acos();
    let theta = std::f32::consts::TAU * v;
    let radius = w.powf(1.0 / 3.0);
    let direction = [phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos()];
    (mul(direction, radius), radius)
}

fn source_particle_identity(system: &mut System) -> u32 {
    let identity = system
        .random_seed
        .wrapping_add(system.unique_particle_identity) as u32
        & 4_095;
    system.unique_particle_identity = system.unique_particle_identity.wrapping_add(1);
    identity
}

fn ranged(minimum: f32, maximum: f32, exponent: f32, sample: f32) -> f32 {
    mix(minimum, maximum, sample.powf(exponent))
}

fn authored_remaining(system: &System, definition: &Definition) -> usize {
    let authored =
        integer_attribute(definition, &["max_particles"], 1_000).clamp(1, 5_000) as usize;
    authored.saturating_sub(system.particles.len())
}

fn caller_remaining(system: &System, limits: WorldLimits, remaining_total: usize) -> usize {
    limits
        .max_particles_per_system
        .saturating_sub(system.particles.len())
        .min(remaining_total)
}

fn particle_path(
    system: &mut System,
    function: &Function,
    time: f32,
    bulge_name: &str,
) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let start_index = integer_parameter(function, "start control point number", 0);
    let end_index = integer_parameter(function, "end control point number", 0);
    let start = control_at_time(system, start_index, time);
    let end = control_at_time(system, end_index, time);
    let mut midpoint = add(
        start,
        mul(
            sub(end, start),
            float_parameter(function, "mid point position", 0.5),
        ),
    );
    let bulge = float_parameter(function, bulge_name, 0.0);
    let control = integer_parameter(
        function,
        "bulge control 0=random 1=orientation of start pnt 2=orientation of end point",
        0,
    );
    if control == 0 {
        midpoint = add(
            midpoint,
            next_random_vector(system, [-bulge; 3], [bulge; 3]),
        );
    } else {
        let index = if control == 2 { end_index } else { start_index };
        let forward = control_orientation(system, index).map_or([1.0, 0.0, 0.0], |orientation| {
            rotate(orientation, [1.0, 0.0, 0.0])
        });
        let delta = sub(end, start);
        let length = dot(delta, delta).sqrt();
        if length > 1e-6 {
            let direction = mul(delta, 1.0 / length);
            let scale = 1.0 - dot(direction, forward).abs();
            if let Some(forward) = normalize(forward) {
                midpoint = add(midpoint, mul(forward, bulge * length * scale));
            }
        }
    }
    (start, midpoint, end)
}

fn particle_scalar(particle: &Particle, field: i32) -> Option<f32> {
    Some(match field {
        1 => particle.lifetime_seconds,
        3 => particle.radius,
        4 => particle.roll,
        5 => particle.roll_speed,
        7 => particle.alpha,
        8 => particle.creation_seconds,
        9 => particle.sequence as f32,
        10 => particle.trail_length,
        11 => particle.identity as f32,
        12 => particle.yaw,
        13 => particle.secondary_sequence as f32,
        21 => f32::from(particle.target_control_point),
        _ => return None,
    })
}

fn set_particle_scalar(particle: &mut Particle, field: i32, value: f32) {
    match field {
        1 => particle.lifetime_seconds = value,
        3 => particle.radius = value.max(0.0),
        4 => particle.roll = value,
        5 => particle.roll_speed = value,
        7 => particle.alpha = value.clamp(0.0, 1.0),
        9 => particle.sequence = value as i32,
        10 => particle.trail_length = value.max(0.0),
        12 => particle.yaw = value,
        13 => particle.secondary_sequence = value as i32,
        21 => particle.target_control_point = value as u8,
        _ => {}
    }
}

fn particle_vector(particle: &Particle, field: i32) -> Option<[f32; 3]> {
    match field {
        0 => Some(particle.position),
        2 => Some(particle.previous_position),
        6 => Some(particle.color),
        _ => None,
    }
}

fn set_particle_vector(particle: &mut Particle, field: i32, value: [f32; 3]) {
    match field {
        0 => particle.position = value,
        2 => particle.previous_position = value,
        6 => particle.color = value.map(|component| component.clamp(0.0, 1.0)),
        _ => {}
    }
}

fn initializer_attribute(identity: &str) -> Option<&'static str> {
    if identity.eq_ignore_ascii_case("Lifetime Random")
        || identity.eq_ignore_ascii_case("Lifetime From Control Point Life Time")
    {
        Some("lifetime")
    } else if identity.eq_ignore_ascii_case("Radius Random") {
        Some("radius")
    } else if identity.eq_ignore_ascii_case("Alpha Random") {
        Some("alpha")
    } else if identity.eq_ignore_ascii_case("Color Random") {
        Some("color")
    } else if identity.eq_ignore_ascii_case("Rotation Random") {
        Some("rotation")
    } else if identity.eq_ignore_ascii_case("Sequence Random") {
        Some("sequence")
    } else if identity.eq_ignore_ascii_case("Trail Length Random") {
        Some("trail")
    } else if identity.eq_ignore_ascii_case("Position Within Box Random")
        || identity.eq_ignore_ascii_case("Position Within Sphere Random")
        || identity.eq_ignore_ascii_case("Position Along Path Random")
    {
        Some("position")
    } else {
        None
    }
}

fn operator_strength(function: &Function, system_time: f32) -> f32 {
    let period = float_parameter(function, "operator fade oscillate", 0.0);
    let time = if period > 0.0 {
        (system_time / period) % 1.0
    } else {
        system_time
    };
    let start_in = float_parameter(function, "operator start fadein", 0.0);
    let end_in = float_parameter(function, "operator end fadein", 0.0).max(start_in);
    let start_out = float_parameter(function, "operator start fadeout", 0.0).max(end_in);
    let end_out = float_parameter(function, "operator end fadeout", 0.0).max(start_out);
    if time < start_in || (end_out > 0.0 && time > end_out) {
        return 0.0;
    }
    let mut strength: f32 = 1.0;
    if time < end_in && end_in > start_in {
        strength = strength.min((time - start_in) / (end_in - start_in));
    }
    if time > start_out && end_out > start_out {
        strength = strength.min((end_out - time) / (end_out - start_out));
    }
    strength
}

fn fade_windows(function: &Function) -> (f32, f32, f32, f32) {
    let mut start_in = float_parameter(function, "start_fade_in_time", 0.0);
    let mut end_in = float_parameter(function, "end_fade_in_time", 0.5).max(start_in);
    let mut start_out = float_parameter(function, "start_fade_out_time", 0.5);
    let mut end_out = float_parameter(function, "end_fade_out_time", 1.0).max(start_out);
    if start_out < start_in {
        std::mem::swap(&mut start_in, &mut start_out);
    }
    if end_out < end_in {
        std::mem::swap(&mut end_in, &mut end_out);
    }
    (start_in, end_in, start_out, end_out)
}

fn sin_estimate_cycles(value: f32) -> f32 {
    let absolute = value.abs();
    let reduced = absolute % 2.0;
    let odd = reduced >= 1.0;
    let phase = reduced - if odd { 1.0 } else { 0.0 };
    let estimate = phase * (4.0 - phase * 4.0);
    if value.is_sign_negative() != odd {
        -estimate
    } else {
        estimate
    }
}

fn wrap_angle(value: f32) -> f32 {
    if value >= std::f32::consts::TAU {
        value - std::f32::consts::TAU
    } else if value <= -std::f32::consts::TAU {
        value + std::f32::consts::TAU
    } else {
        value
    }
}

fn remap(value: f32, minimum: f32, maximum: f32) -> f32 {
    if maximum <= minimum {
        return f32::from(value >= maximum);
    }
    ((value - minimum) / (maximum - minimum)).clamp(0.0, 1.0)
}

fn spline(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn bias(value: f32, amount: f32) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }
    if amount >= 1.0 {
        return 1.0;
    }
    value / (((1.0 / amount) - 2.0) * (1.0 - value) + 1.0)
}

fn mix(left: f32, right: f32, fraction: f32) -> f32 {
    left + (right - left) * fraction
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn mul(value: [f32; 3], scale: f32) -> [f32; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn length_squared(value: [f32; 3]) -> f32 {
    dot(value, value)
}

fn length_squared4(value: [f32; 4]) -> f32 {
    value.iter().map(|component| component * component).sum()
}

fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = length_squared(value).sqrt();
    (length > f32::EPSILON).then(|| mul(value, length.recip()))
}

fn rotate(quaternion: [f32; 4], vector: [f32; 3]) -> [f32; 3] {
    let q = [quaternion[0], quaternion[1], quaternion[2]];
    let uv = cross(q, vector);
    let uuv = cross(q, uv);
    add(vector, add(mul(uv, 2.0 * quaternion[3]), mul(uuv, 2.0)))
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn put_vector(bytes: &mut [u8], offset: usize, value: [f32; 3]) {
    for (component, value) in value.into_iter().enumerate() {
        let start = offset + component * 4;
        bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
    }
}
