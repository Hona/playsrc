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
const OUTPUT_HEADER_BYTES: usize = 12;
const OUTPUT_RECORD_BYTES: usize = 392;

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
        seed: u64,
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

#[derive(Clone, Debug, PartialEq)]
pub struct RenderItem {
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
    pub color: [u8; 3],
    pub opacity: f32,
    pub sequence: i32,
    pub secondary_sequence: i32,
    pub trail_length: f32,
    pub sort_key: f32,
    pub age_seconds: f32,
    pub lifetime_seconds: f32,
    pub animation_rate: f32,
    pub trail_min_length: f32,
    pub trail_max_length: f32,
    pub trail_fade_in_seconds: f32,
    pub orientation_type: i32,
    pub animation_fit_lifetime: bool,
    pub animation_rate_as_fps: bool,
    pub primary_sheet: Option<SheetSample>,
    pub secondary_sheet: Option<SheetSample>,
    pub stable_tie_identity: u64,
}

pub fn resolve_render_sheets(
    items: &mut [RenderItem],
    sheets: &BTreeMap<String, ParticleSheet>,
) -> Result<(), Error> {
    for item in items {
        let sheet = sheets.get(&item.material).ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDependency,
                "particle-output",
                0,
                format!("particle sheet for material {} is missing", item.material),
            )
        })?;
        item.primary_sheet = Some(sample_sheet(
            sheet,
            SheetSampleRequest {
                sequence: item.sequence,
                age_seconds: item.age_seconds,
                lifetime_seconds: item.lifetime_seconds,
                animation_rate: item.animation_rate,
                fit_lifetime: item.animation_fit_lifetime,
                animation_rate_as_fps: item.animation_rate_as_fps,
            },
        )?);
        item.secondary_sheet = Some(sample_sheet(
            sheet,
            SheetSampleRequest {
                sequence: item.secondary_sequence,
                age_seconds: item.age_seconds,
                lifetime_seconds: item.lifetime_seconds,
                animation_rate: 0.0,
                fit_lifetime: item.animation_fit_lifetime,
                animation_rate_as_fps: item.animation_rate_as_fps,
            },
        )?);
    }
    Ok(())
}

pub fn encode_render_output(
    items: &[RenderItem],
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
    let material_indexes: BTreeMap<&str, u32> = materials
        .iter()
        .enumerate()
        .map(|(index, material)| {
            u32::try_from(index)
                .map(|index| (material.as_str(), index))
                .map_err(|_| {
                    Error::new(
                        ErrorCode::BoundExceeded,
                        "particle-output",
                        0,
                        "material index exceeds u32",
                    )
                })
        })
        .collect::<Result<_, _>>()?;
    let mut bytes = vec![0; length];
    bytes[0..4].copy_from_slice(&0x5250_5350_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&(items.len() as u32).to_le_bytes());
    for (index, item) in items.iter().enumerate() {
        if !finite(&item.position)
            || !finite(&item.previous_position)
            || !finite(&[
                item.radius,
                item.roll_radians,
                item.opacity,
                item.trail_length,
                item.sort_key,
                item.age_seconds,
                item.lifetime_seconds,
                item.animation_rate,
                item.trail_min_length,
                item.trail_max_length,
                item.trail_fade_in_seconds,
            ])
            || item.radius < 0.0
            || !(0.0..=1.0).contains(&item.opacity)
            || item.trail_length < 0.0
            || item.age_seconds < 0.0
            || item.lifetime_seconds <= 0.0
            || item.trail_min_length < 0.0
            || item.trail_max_length < item.trail_min_length
            || item.trail_fade_in_seconds < 0.0
            || !valid_sheet_sample(item.primary_sheet.as_ref())
            || !valid_sheet_sample(item.secondary_sheet.as_ref())
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "particle-output",
                index,
                "render item contains an invalid scalar",
            ));
        }
        let material = material_indexes
            .get(item.material.as_str())
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::MissingDependency,
                    "particle-output",
                    index,
                    "render item material is absent from the supplied registry",
                )
            })?;
        let offset = OUTPUT_HEADER_BYTES + index * OUTPUT_RECORD_BYTES;
        bytes[offset..offset + 4].copy_from_slice(&(index as u32 + 1).to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&item.effect_identity.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&item.particle_identity.to_le_bytes());
        bytes[offset + 12..offset + 14].copy_from_slice(&item.renderer_index.to_le_bytes());
        bytes[offset + 14] = match item.primitive {
            Primitive::Sprite => 0,
            Primitive::Trail => 1,
        };
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
    limits: WorldLimits,
    time: f32,
    effects: Vec<Effect>,
    event_identities: BTreeSet<u64>,
    simulation_random: SimdRandom,
}

#[derive(Clone, Debug, PartialEq)]
struct Effect {
    identity: u32,
    owner_identity: Option<u32>,
    root: System,
}

#[derive(Clone, Debug, PartialEq)]
struct System {
    definition_uuid: [u8; 16],
    path_identity: u64,
    start_seconds: f32,
    delay_seconds: f32,
    emission_active: bool,
    dormant: bool,
    first_frame: bool,
    local_time: f32,
    previous_step: f32,
    random_seed: i32,
    random_query_count: i32,
    unique_particle_identity: i32,
    emitter_contexts: Vec<EmitterContext>,
    controls: Vec<Option<ControlPoint>>,
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
    roll_speed: f32,
    initial_color: [f32; 3],
    color: [f32; 3],
    initial_alpha: f32,
    alpha: f32,
    sequence: i32,
    secondary_sequence: i32,
    trail_length: f32,
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
        time_offset: f32,
    },
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
    pub fn new(registry: &Registry, limits: WorldLimits) -> Result<Self, Error> {
        validate_limits(limits)?;
        Ok(Self {
            registry: Arc::new(registry.clone()),
            limits,
            time: 0.0,
            effects: Vec::new(),
            event_identities: BTreeSet::new(),
            simulation_random: SimdRandom::new(12_345_678),
        })
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn advance(
        &mut self,
        events: &[Event],
        request: AdvanceRequest,
        collision: &mut impl CollisionQuery,
    ) -> Result<(Vec<RenderItem>, Option<Bounds>), Error> {
        validate_advance(self.time, events, request, self.limits)?;
        let mut candidate = self.clone();
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
            candidate.apply_event(event)?;
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
        let (render, bounds) = candidate.render(request.camera_position)?;
        *self = candidate;
        Ok((render, bounds))
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
            *substeps += 1;
            if *substeps > self.limits.max_substeps {
                return Err(Error::new(
                    ErrorCode::BoundExceeded,
                    "particle-world",
                    0,
                    "advance exceeds max_substeps",
                ));
            }
            let next = (from + maximum_step).min(to);
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
                )?;
            }
            from = next;
        }
        Ok(())
    }

    fn apply_event(&mut self, event: &Event) -> Result<(), Error> {
        if !self.event_identities.insert(event.identity) {
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
            EventCommand::Restart {
                effect_identity,
                seed,
            } => {
                let effect = effect_mut(&mut self.effects, *effect_identity)?;
                restart_system(
                    &mut effect.root,
                    event.timestamp_seconds,
                    normalize_seed(*seed),
                );
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
                self.event_identities.clear();
                self.event_identities.insert(event.identity);
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
            .target_closure(&[DefinitionLookup::Uuid(root_definition.uuid)])?;
        let mut system_count = 0;
        let mut controls = vec![None; self.limits.max_control_points];
        for control in control_points {
            controls[control.index as usize] = Some(control.clone());
        }
        let mut instantiate_state = InstantiateState {
            controls: &controls,
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
        })
    }

    fn render(&self, camera: [f32; 3]) -> Result<(Vec<RenderItem>, Option<Bounds>), Error> {
        if !finite(&camera) {
            return Err(Error::new(
                ErrorCode::NonFinite,
                "particle-world",
                0,
                "camera position is non-finite",
            ));
        }
        let mut items = Vec::new();
        let mut bounds: Option<Bounds> = None;
        for effect in &self.effects {
            render_system(
                &effect.root,
                &self.registry,
                effect.identity,
                camera,
                &mut items,
                &mut bounds,
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
) -> Result<(), Error> {
    if system.delay_seconds > 0.0 {
        return Ok(());
    }
    initialize_system_first_frame(system, registry, limits, remaining_particles)?;
    for child in &mut system.children {
        initialize_first_frame(child, registry, limits, remaining_particles)?;
    }
    Ok(())
}

fn initialize_system_first_frame(
    system: &mut System,
    registry: &Registry,
    limits: WorldLimits,
    remaining_particles: &mut usize,
) -> Result<(), Error> {
    let definition = registry
        .definition_by_uuid(system.definition_uuid)
        .expect("instantiated definition");
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
        let particle = initialize_particle(system, definition, 0.0);
        system.particles.push(particle);
        *remaining_particles -= 1;
    }
    system.first_frame = false;
    Ok(())
}

struct InstantiateState<'a> {
    controls: &'a [Option<ControlPoint>],
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
    let mut children = Vec::with_capacity(definition.children.len());
    let mut child_seed = random_seed;
    for (index, child) in definition.children.iter().enumerate() {
        let child_definition = registry
            .definition_by_uuid(child.definition_uuid)
            .ok_or_else(|| {
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
        path_identity,
        start_seconds: start,
        delay_seconds: delay,
        emission_active: true,
        dormant: false,
        first_frame: true,
        local_time: 0.0,
        previous_step: DEFAULT_STEP_SECONDS,
        random_seed,
        random_query_count: 0,
        unique_particle_identity: 0,
        emitter_contexts,
        controls: state.controls.to_vec(),
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
) -> Result<(), Error> {
    let definition = registry
        .definition_by_uuid(system.definition_uuid)
        .expect("instantiated definition");
    let local_from = (from - system.start_seconds - system.delay_seconds).max(0.0);
    let local_to = (to - system.start_seconds - system.delay_seconds).max(0.0);
    if to < system.start_seconds + system.delay_seconds || system.dormant {
        return Ok(());
    }
    if system.first_frame {
        initialize_system_first_frame(system, registry, limits, remaining_particles)?;
    }
    let dt = (local_to - local_from).max(0.0);
    emit(
        system,
        definition,
        local_from,
        local_to,
        limits,
        remaining_particles,
    )?;
    if dt > 0.0 {
        operate(system, definition, local_to, dt, simulation_random);
        constrain(
            system,
            definition,
            effect_identity,
            limits,
            queries,
            collision,
        )?;
        system
            .particles
            .retain(|particle| local_to - particle.creation_seconds < particle.lifetime_seconds);
        system.previous_step = dt;
    }
    system.first_frame = false;
    system.local_time = local_to;
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
        )?;
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
            let rate = float_parameter(emitter, "emission_rate", 100.0).max(0.0) * strength;
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
            let particle = initialize_particle(system, definition, creation.min(to));
            system.particles.push(particle);
            *remaining_particles -= 1;
        }
    }
    Ok(())
}

fn initialize_particle(system: &mut System, definition: &Definition, creation: f32) -> Particle {
    let origin = control_at(system, 0);
    let constant_color = color_attribute(definition, "color", [255; 4]);
    let mut particle = Particle {
        identity: 0,
        creation_seconds: creation,
        lifetime_seconds: 1.0,
        position: origin,
        previous_position: origin,
        initial_radius: float_attribute(definition, "radius", 5.0),
        radius: float_attribute(definition, "radius", 5.0),
        initial_roll: float_attribute(definition, "rotation", 0.0),
        roll: float_attribute(definition, "rotation", 0.0),
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
        {
            continue;
        }
        let attribute = initializer_attribute(&initializer.identity);
        if attribute.is_some_and(|attribute| !claimed.insert(attribute)) {
            continue;
        }
        if initializer.identity.eq_ignore_ascii_case("Lifetime Random") {
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
            let random = next_random(system);
            for component in 0..3 {
                particle.color[component] = (first[component] as f32
                    + (second[component] as f32 - first[component] as f32) * random)
                    / 255.0;
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
            .eq_ignore_ascii_case("Position Within Box Random")
        {
            let minimum = vector_parameter(initializer, "min", [0.0; 3]);
            let maximum = vector_parameter(initializer, "max", [0.0; 3]);
            let cp = control_at(
                system,
                integer_parameter(initializer, "control point number", 0),
            );
            let random = next_random_vector(system, minimum, maximum);
            for component in 0..3 {
                particle.position[component] = cp[component] + random[component];
            }
        } else if initializer
            .identity
            .eq_ignore_ascii_case("Position Within Sphere Random")
        {
            let cp_index = integer_parameter(initializer, "control_point_number", 0);
            let cp = control_at(system, cp_index);
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
            for component in 0..3 {
                particle.position[component] = cp[component] + direction[component] * distance;
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
            velocity = control_orientation(system, cp_index)
                .map_or(local_velocity, |orientation| {
                    rotate(orientation, local_velocity)
                });
            let speed_maximum = float_parameter(initializer, "speed_max", 0.0);
            if speed_maximum > 0.0 {
                let speed = ranged(
                    float_parameter(initializer, "speed_min", 0.0),
                    speed_maximum,
                    float_parameter(initializer, "speed_random_exponent", 1.0),
                    next_random(system),
                );
                let radial = mul(direction, speed);
                velocity = add(velocity, radial);
            }
        }
    }
    for initializer in definition.functions(FunctionCategory::Initializer) {
        if initializer
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
        }
    }
    particle.previous_position = sub(particle.position, mul(velocity, system.previous_step));
    particle.identity = source_particle_identity(system);
    particle.initial_radius = particle.radius;
    particle.initial_roll = particle.roll;
    particle.initial_color = particle.color;
    particle.initial_alpha = particle.alpha;
    particle
}

fn operate(
    system: &mut System,
    definition: &Definition,
    time: f32,
    dt: f32,
    simulation_random: &mut SimdRandom,
) {
    let control_delta = system
        .controls
        .first()
        .and_then(Option::as_ref)
        .map_or([0.0; 3], |control| {
            sub(control.position, control.previous_position)
        });
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
        let random_offset = (function_index as i32).wrapping_mul(17);
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
                let control_index = integer_parameter(operator, "control_point_number", 0);
                let delta = if control_index == 0 {
                    control_delta
                } else {
                    system
                        .controls
                        .get(control_index.max(0) as usize)
                        .and_then(Option::as_ref)
                        .map_or([0.0; 3], |control| {
                            sub(control.position, control.previous_position)
                        })
                };
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
                let movement = mul(delta, strength * lock);
                particle.position = add(particle.position, movement);
                particle.previous_position = add(particle.previous_position, movement);
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
                let oscillator_time = if bool_parameter(operator, "proportional 0/1", true) {
                    life
                } else {
                    time
                };
                let phase = (float_parameter(operator, "oscillation multiplier", 2.0)
                    * oscillator_time
                    + float_parameter(operator, "oscillation start phase", 0.5))
                    * frequency;
                let value = sin_estimate_cycles(phase) * rate * dt * strength;
                match field {
                    3 => particle.radius = (particle.radius + value).max(0.0),
                    4 => particle.roll += value,
                    7 => particle.alpha = (particle.alpha + value).clamp(0.0, 1.0),
                    _ => {}
                }
            }
        }
    }
    system
        .particles
        .retain(|particle| particle.lifetime_seconds > 0.0);
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
        if !force.identity.eq_ignore_ascii_case("random force") {
            continue;
        }
        let strength = operator_strength(force, time);
        if strength <= 0.0 {
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
) -> Result<(), Error> {
    for (constraint_index, constraint) in definition
        .functions(FunctionCategory::Constraint)
        .enumerate()
    {
        if !constraint
            .identity
            .eq_ignore_ascii_case("Collision via traces")
        {
            continue;
        }
        let mut requests = Vec::new();
        for particle in &system.particles {
            if length_squared(sub(particle.position, particle.previous_position)) <= f32::EPSILON {
                continue;
            }
            let mut identity = hash(
                system.path_identity,
                ((constraint_index as u64) << 32) | particle.identity as u64,
            );
            for component in particle
                .previous_position
                .into_iter()
                .chain(particle.position)
            {
                identity = hash(identity, component.to_bits() as u64);
            }
            requests.push(TraceRequest {
                identity,
                effect_identity,
                system_uuid: system.definition_uuid,
                particle_identity: particle.identity,
                start: particle.previous_position,
                end: particle.position,
                radius: particle.radius * float_parameter(constraint, "radius scale", 1.0),
                brush_only: bool_parameter(constraint, "brush only", false),
                collision_group: string_parameter(constraint, "collision group", "NONE"),
            });
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
        if requests.is_empty() {
            continue;
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
        for (request, result) in requests.iter().zip(results) {
            validate_collision_result(request, &result)?;
            if result.fraction >= 1.0 {
                continue;
            }
            let particle = system
                .particles
                .iter_mut()
                .find(|particle| particle.identity == request.particle_identity)
                .expect("requested particle");
            if bool_parameter(constraint, "kill particle on collision", false) {
                particle.lifetime_seconds = 0.0;
                continue;
            }
            if result.start_solid {
                particle.position = particle.previous_position;
                continue;
            }
            let movement = sub(request.end, request.start);
            let impact = add(request.start, mul(movement, result.fraction));
            let leftover = 1.0 - result.fraction;
            let direction = normalize(movement).unwrap_or([0.0; 3]);
            let reflected = sub(
                direction,
                mul(result.normal, 2.0 * dot(direction, result.normal)),
            );
            let slide = sub(movement, mul(result.normal, dot(movement, result.normal)));
            let new_velocity = add(
                mul(
                    reflected,
                    float_parameter(constraint, "amount of bounce", 0.0),
                ),
                mul(slide, float_parameter(constraint, "amount of slide", 0.0)),
            );
            particle.position = add(impact, mul(new_velocity, leftover));
            particle.previous_position = sub(particle.position, new_velocity);
        }
    }
    Ok(())
}

fn render_system(
    system: &System,
    registry: &Registry,
    effect_identity: u32,
    camera: [f32; 3],
    output: &mut Vec<RenderItem>,
    bounds: &mut Option<Bounds>,
    limit: usize,
) -> Result<(), Error> {
    let definition = registry
        .definition_by_uuid(system.definition_uuid)
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
                particle_identity: particle.identity,
                renderer_index: renderer_index as u16,
                primitive,
                material: definition.material.clone(),
                position: particle.position,
                previous_position: particle.previous_position,
                radius: particle.radius,
                roll_radians: particle.roll,
                color: particle
                    .color
                    .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8),
                opacity: particle.alpha.clamp(0.0, 1.0),
                sequence: particle.sequence,
                secondary_sequence: particle.secondary_sequence,
                trail_length: particle.trail_length,
                sort_key: distance,
                age_seconds: (system.local_time - particle.creation_seconds).max(0.0),
                lifetime_seconds: particle.lifetime_seconds,
                animation_rate: float_parameter(renderer, "animation rate", 0.1),
                trail_min_length: float_parameter(renderer, "min length", 0.0).max(0.0),
                trail_max_length: float_parameter(renderer, "max length", 2_000.0).max(0.0),
                trail_fade_in_seconds: float_parameter(renderer, "length fade in time", 0.0)
                    .max(0.0),
                orientation_type: integer_parameter(renderer, "orientation_type", 0),
                animation_fit_lifetime: bool_parameter(renderer, "animation_fit_lifetime", false),
                animation_rate_as_fps: bool_parameter(renderer, "use animation rate as FPS", false),
                primary_sheet: None,
                secondary_sheet: None,
                stable_tie_identity: hash(system.path_identity, particle.identity as u64),
            };
            extend_bounds(bounds, item.position, item.radius);
            output.push(item);
        }
    }
    for child in &system.children {
        render_system(
            child,
            registry,
            effect_identity,
            camera,
            output,
            bounds,
            limit,
        )?;
    }
    Ok(())
}

fn finished(system: &System, registry: &Registry, time: f32) -> bool {
    let definition = registry
        .definition_by_uuid(system.definition_uuid)
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
    system.controls[control.index as usize] = Some(control.clone());
    for child in &mut system.children {
        set_control(child, control.clone());
    }
}

fn set_emission(system: &mut System, active: bool, clear: bool) {
    system.emission_active = active;
    for context in &mut system.emitter_contexts {
        match context {
            EmitterContext::Continuous { stopped, .. } => *stopped = !active,
            EmitterContext::Instantaneous {
                remaining, actual, ..
            } => {
                if active {
                    *remaining = *actual;
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
    system.dormant = dormant;
    for child in &mut system.children {
        set_dormant(child, dormant);
    }
}

fn restart_system(system: &mut System, _timestamp: f32, random_seed: i32) {
    system.emission_active = true;
    system.dormant = false;
    system.random_seed = random_seed;
    system.random_query_count = 0;
    for context in &mut system.emitter_contexts {
        match context {
            EmitterContext::Continuous {
                total_actual,
                emitted,
                time_offset,
                stopped,
            } => {
                *stopped = false;
                *total_actual = 0.0;
                *emitted = 0;
                *time_offset = system.local_time;
            }
            EmitterContext::Instantaneous {
                remaining,
                actual,
                time_offset,
            } => {
                *remaining = *actual;
                *time_offset = system.local_time;
            }
        }
    }
    let mut child_seed = random_seed;
    for child in &mut system.children {
        if child_seed != 0 {
            child_seed = child_seed.wrapping_add(129);
        }
        restart_system(child, _timestamp, child_seed);
    }
}

fn update_control_history(system: &mut System) {
    for control in system.controls.iter_mut().flatten() {
        control.previous_position = control.position;
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

fn control_at(system: &System, index: i32) -> [f32; 3] {
    system
        .controls
        .get(index.max(0) as usize)
        .and_then(Option::as_ref)
        .map_or([0.0; 3], |control| control.position)
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

fn extend_bounds(bounds: &mut Option<Bounds>, position: [f32; 3], radius: f32) {
    let minimum = position.map(|value| value - radius);
    let maximum = position.map(|value| value + radius);
    match bounds {
        Some(bounds) => {
            for component in 0..3 {
                bounds.minimum[component] = bounds.minimum[component].min(minimum[component]);
                bounds.maximum[component] = bounds.maximum[component].max(maximum[component]);
            }
        }
        None => *bounds = Some(Bounds { minimum, maximum }),
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
    let authored = integer_attribute(definition, &["max_particles"], 1_000).max(0) as usize;
    authored.saturating_sub(system.particles.len())
}

fn caller_remaining(system: &System, limits: WorldLimits, remaining_total: usize) -> usize {
    limits
        .max_particles_per_system
        .saturating_sub(system.particles.len())
        .min(remaining_total)
}

fn initializer_attribute(identity: &str) -> Option<&'static str> {
    if identity.eq_ignore_ascii_case("Lifetime Random") {
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
