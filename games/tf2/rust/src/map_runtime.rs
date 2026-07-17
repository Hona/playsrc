use std::collections::BTreeMap;

use playsrc_collision::Hull;
use playsrc_entity::{
    BehaviorState, ContactKind, ContactRecord, EntityHandle, EntityWorld, EntityWorldConfig,
    ModelBounds, RuntimeFailure, RuntimeRequest, Transform, Transition, TransitionBatch,
    TriggerKind, Variant, WorldCommand,
};
use playsrc_movement::{Error as MoveError, Tracer};

pub trait GameplayWorld: Tracer {
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, MoveError>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MapCounts {
    pub buttons: u32,
    pub doors: u32,
    pub linear_movers: u32,
    pub multiple_triggers: u32,
    pub hurt_triggers: u32,
    pub push_triggers: u32,
    pub catapult_triggers: u32,
    pub teleport_triggers: u32,
    pub teleport_destinations: u32,
    pub regenerate_zones: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EntityEventKind {
    Input = 1,
    Output = 2,
    Contact = 3,
    MoverStarted = 4,
    MoverCompleted = 5,
    MoverBlocked = 6,
    Transform = 7,
    Diagnostic = 8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityEvent {
    pub sequence: u64,
    pub kind: EntityEventKind,
    pub entity: u32,
    pub subject: Option<u32>,
    pub accepted: bool,
    pub contact: Option<ContactKind>,
    pub name: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityTransform {
    pub identity: u32,
    pub model: u32,
    pub position: [f32; 3],
    pub angles: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Teleport {
        trigger: u32,
        destination: u32,
        position: [f32; 3],
        yaw_degrees: Option<f32>,
    },
    Hurt {
        trigger: u32,
        damage_per_second: f32,
    },
    Push {
        trigger: u32,
        velocity: [f32; 3],
        replace: bool,
    },
    Regenerate {
        entity: u32,
        team: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerContact {
    pub sequence: u64,
    pub trigger_entity: u32,
    pub kind: ContactKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapPhase {
    pub events: Vec<EntityEvent>,
    pub effects: Vec<Effect>,
    pub contacts: Vec<TriggerContact>,
    pub carry: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeginTickInput {
    pub tick: u64,
    pub tick_interval: f32,
    pub activate_entity: Option<u32>,
    pub player_position: [f32; 3],
    pub player_hull: Hull,
    pub grounded: bool,
}

impl MapPhase {
    pub fn append(&mut self, mut other: Self) {
        self.events.append(&mut other.events);
        self.effects.append(&mut other.effects);
        self.contacts.append(&mut other.contacts);
        self.carry = add(self.carry, other.carry);
    }
}

#[derive(Clone, Debug)]
struct Volume {
    source: u32,
    handle: Option<EntityHandle>,
    model: usize,
    origin: [f32; 3],
    kind: VolumeKind,
}

#[derive(Clone, Debug)]
enum VolumeKind {
    Generic,
    Regenerate { enabled: bool, team: Option<u8> },
}

#[derive(Clone, Copy, Debug)]
struct TeleportDestination {
    source: u32,
    position: [f32; 3],
    yaw_degrees: f32,
}

#[derive(Clone, Copy, Debug)]
struct TeleportLink {
    destination: TeleportDestination,
    preserve_angles: bool,
}

#[derive(Clone, Copy, Debug)]
struct ActiveMover {
    request_id: u64,
    entity: EntityHandle,
    model: Option<usize>,
    position: [f32; 3],
    destination: [f32; 3],
    speed: f32,
}

#[derive(Clone, Debug)]
pub struct MapRuntime {
    world: EntityWorld,
    player: EntityHandle,
    source_handles: BTreeMap<u32, EntityHandle>,
    volumes: Vec<Volume>,
    teleports: BTreeMap<EntityHandle, TeleportLink>,
    movers: BTreeMap<EntityHandle, ActiveMover>,
    counts: MapCounts,
    next_producer_sequence: u64,
}

impl MapRuntime {
    pub fn compile(
        graph: &playsrc_entity::Graph,
        tick_interval: f32,
        source_identity: u64,
        model_bounds: Vec<ModelBounds>,
    ) -> Result<Self, RuntimeFailure> {
        let config = EntityWorldConfig {
            tick_interval,
            source_identity,
            registry_identity: 0x5446_325f_454e_5432,
            model_bounds,
            ..EntityWorldConfig::default()
        };
        let (mut world, _) = EntityWorld::compile(graph, config)?;
        let player_definition = playsrc_entity::parse(
            b"{\"classname\"\"player\"\"targetname\"\"!player\"}\0",
            playsrc_entity::Limits::default(),
        )
        .expect("static player entity definition")
        .entities
        .into_iter()
        .next()
        .expect("static player entity");
        let spawned = world.phase(0, &[WorldCommand::Spawn(player_definition)])?;
        let player = spawned
            .records
            .iter()
            .find_map(|record| match record.transition {
                Transition::Lifecycle { entity, .. } => Some(entity),
                _ => None,
            })
            .expect("spawn transition identifies player");

        let mut source_handles = BTreeMap::new();
        for handle in world.live_handles() {
            if handle != player
                && let Some(entity) = world.entity(handle)
                && let Ok(source) = u32::try_from(entity.source_index)
            {
                source_handles.insert(source, handle);
            }
        }

        let mut destinations = BTreeMap::<Vec<u8>, TeleportDestination>::new();
        for entity in &graph.entities {
            if class(entity, b"info_teleport_destination") {
                let Some(name) = entity.targetname.clone().filter(|value| !value.is_empty()) else {
                    return Err(invalid(entity.index));
                };
                let position = vector(entity, b"origin", None)?;
                let angles = vector(entity, b"angles", Some([0.0; 3]))?;
                destinations.insert(
                    ascii_lower(&name),
                    TeleportDestination {
                        source: u32::try_from(entity.index).map_err(|_| invalid(entity.index))?,
                        position,
                        yaw_degrees: angles[1],
                    },
                );
            }
        }

        let mut counts = MapCounts {
            teleport_destinations: u32::try_from(destinations.len()).unwrap_or(u32::MAX),
            ..MapCounts::default()
        };
        let mut volumes = Vec::new();
        let mut teleports = BTreeMap::new();
        for entity in &graph.entities {
            counts.buttons += u32::from(class(entity, b"func_button"));
            counts.doors += u32::from(class(entity, b"func_door"));
            counts.linear_movers += u32::from(class(entity, b"func_movelinear"));
            let generic_kind = if class(entity, b"trigger_multiple") {
                counts.multiple_triggers += 1;
                Some(TriggerKind::Multiple)
            } else if class(entity, b"trigger_hurt") {
                counts.hurt_triggers += 1;
                Some(TriggerKind::Hurt)
            } else if class(entity, b"trigger_push") {
                counts.push_triggers += 1;
                Some(TriggerKind::Push)
            } else if class(entity, b"trigger_catapult") {
                counts.catapult_triggers += 1;
                Some(TriggerKind::Catapult)
            } else if class(entity, b"trigger_teleport") {
                counts.teleport_triggers += 1;
                Some(TriggerKind::Teleport)
            } else {
                None
            };
            if let Some(kind) = generic_kind {
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                let handle = *source_handles
                    .get(&source)
                    .ok_or_else(|| invalid(entity.index))?;
                let model = entity
                    .bsp_model_index
                    .ok_or_else(|| invalid(entity.index))?;
                let origin = vector(entity, b"origin", Some([0.0; 3]))?;
                volumes.push(Volume {
                    source,
                    handle: Some(handle),
                    model,
                    origin,
                    kind: VolumeKind::Generic,
                });
                if kind == TriggerKind::Teleport {
                    let target = field(entity, b"target")
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| invalid(entity.index))?;
                    let destination = *destinations
                        .get(&ascii_lower(target))
                        .ok_or_else(|| invalid(entity.index))?;
                    let flags = integer(entity, b"spawnflags", 0)?;
                    teleports.insert(
                        handle,
                        TeleportLink {
                            destination,
                            preserve_angles: flags & 0x20 != 0,
                        },
                    );
                }
            } else if class(entity, b"func_regenerate") {
                counts.regenerate_zones += 1;
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                let team = match integer(entity, b"TeamNum", 0)? {
                    0 => None,
                    2 => Some(2),
                    3 => Some(3),
                    _ => return Err(invalid(entity.index)),
                };
                volumes.push(Volume {
                    source,
                    handle: None,
                    model: entity
                        .bsp_model_index
                        .ok_or_else(|| invalid(entity.index))?,
                    origin: vector(entity, b"origin", Some([0.0; 3]))?,
                    kind: VolumeKind::Regenerate {
                        enabled: integer(entity, b"StartDisabled", 0)? == 0,
                        team,
                    },
                });
            }
        }
        volumes.sort_by_key(|volume| volume.source);
        Ok(Self {
            world,
            player,
            source_handles,
            volumes,
            teleports,
            movers: BTreeMap::new(),
            counts,
            next_producer_sequence: 1,
        })
    }

    pub fn empty(tick_interval: f32) -> Self {
        let graph =
            playsrc_entity::parse(b"", playsrc_entity::Limits::default()).expect("empty graph");
        Self::compile(&graph, tick_interval, 0, Vec::new()).expect("empty map runtime")
    }

    pub fn counts(&self) -> MapCounts {
        self.counts
    }

    pub fn source_handle(&self, source: u32) -> Option<EntityHandle> {
        self.source_handles.get(&source).copied()
    }

    pub fn accepts_course_trigger(&self, source: u32) -> bool {
        self.volumes
            .iter()
            .any(|volume| volume.source == source && matches!(volume.kind, VolumeKind::Generic))
    }

    pub fn begin_tick<W: GameplayWorld>(
        &mut self,
        collision: &W,
        input: BeginTickInput,
    ) -> Result<MapPhase, MapError> {
        let mut commands = Vec::new();
        if let Some(source) = input.activate_entity {
            let handle = self
                .source_handle(source)
                .ok_or(MapError::MissingEntity(source))?;
            commands.push(WorldCommand::EmitOutput {
                entity: handle,
                output: b"OnDamaged".to_vec(),
                value: Variant::Void,
                activator: Some(self.player),
                caller: Some(self.player),
                delay: 0.0,
            });
        }
        let batch = self.world.phase(input.tick, &commands)?;
        let mut output = self.consume(batch)?;

        let movers: Vec<_> = self.movers.values().copied().collect();
        let mut mover_commands = Vec::new();
        let mut completed = Vec::new();
        for mover in movers {
            let delta = sub(mover.destination, mover.position);
            let distance = length(delta);
            let step = (mover.speed * input.tick_interval).max(0.0);
            let next = if distance <= step || distance <= f32::EPSILON {
                completed.push(mover.entity);
                mover.destination
            } else {
                add(mover.position, scale(delta, step / distance))
            };
            if let Some(active) = self.movers.get_mut(&mover.entity) {
                active.position = next;
            }
            let movement = sub(next, mover.position);
            if input.grounded
                && movement != [0.0; 3]
                && let Some(model) = mover.model
                && collision.overlaps_model_hull(
                    model,
                    mover.position,
                    [
                        input.player_position[0],
                        input.player_position[1],
                        input.player_position[2] - 2.0,
                    ],
                    input.player_hull,
                )?
            {
                output.carry = add(output.carry, movement);
            }
            let angles = self
                .world
                .entity(mover.entity)
                .map_or([0.0; 3], |entity| entity.world_transform.angles);
            mover_commands.push(WorldCommand::SetWorldTransform {
                entity: mover.entity,
                transform: Transform {
                    origin: next,
                    angles,
                },
            });
            if completed.contains(&mover.entity) {
                mover_commands.push(WorldCommand::MoverCompleted {
                    entity: mover.entity,
                    request_id: mover.request_id,
                });
            }
        }
        for handle in completed {
            self.movers.remove(&handle);
        }
        if !mover_commands.is_empty() {
            let batch = self.world.phase(input.tick, &mover_commands)?;
            output.append(self.consume(batch)?);
        }
        Ok(output)
    }

    pub fn contact_phase<W: GameplayWorld>(
        &mut self,
        collision: &W,
        tick: u64,
        position: [f32; 3],
        hull: Hull,
    ) -> Result<MapPhase, MapError> {
        let mut commands = Vec::new();
        let mut regenerate_effects = Vec::new();
        for volume in &self.volumes {
            let origin = volume
                .handle
                .and_then(|handle| self.world.entity(handle))
                .map_or(volume.origin, |entity| entity.world_transform.origin);
            let overlap = collision.overlaps_model_hull(volume.model, origin, position, hull)?;
            match &volume.kind {
                VolumeKind::Regenerate { enabled, team } => {
                    if overlap && *enabled {
                        regenerate_effects.push(Effect::Regenerate {
                            entity: volume.source,
                            team: *team,
                        });
                    }
                }
                VolumeKind::Generic => {
                    let handle = volume.handle.expect("generic trigger handle");
                    let accepted_contact = self.world.entity(handle).is_some_and(|entity| {
                        matches!(
                            &entity.behavior,
                            BehaviorState::Trigger(state) if state.contacts.contains(&self.player)
                        )
                    });
                    let kind = match (overlap, accepted_contact) {
                        (true, true) => Some(ContactKind::Stay),
                        (true, false) => Some(ContactKind::Enter),
                        (false, true) => Some(ContactKind::Exit),
                        (false, false) => None,
                    };
                    if let Some(kind) = kind {
                        commands.push(WorldCommand::Contact(ContactRecord {
                            trigger: handle,
                            subject: self.player,
                            kind,
                            external_filter_result: Some(true),
                            producer_sequence: self.next_producer_sequence,
                        }));
                        self.next_producer_sequence += 1;
                    }
                }
            }
        }
        let batch = self.world.phase(tick, &commands)?;
        let mut output = self.consume(batch)?;
        output.effects.extend(regenerate_effects);
        Ok(output)
    }

    pub fn transforms(&self) -> Vec<EntityTransform> {
        self.world
            .live_handles()
            .into_iter()
            .filter(|handle| *handle != self.player)
            .filter_map(|handle| {
                let entity = self.world.entity(handle)?;
                let model = u32::try_from(entity.definition.bsp_model_index?).ok()?;
                let identity = u32::try_from(entity.source_index).ok()?;
                Some(EntityTransform {
                    identity,
                    model,
                    position: entity.world_transform.origin,
                    angles: entity.world_transform.angles,
                })
            })
            .collect()
    }

    fn consume(&mut self, batch: TransitionBatch) -> Result<MapPhase, RuntimeFailure> {
        let mut output = MapPhase::default();
        for record in batch.records {
            match record.transition {
                Transition::Input {
                    target,
                    input,
                    accepted,
                    ..
                } => output.events.push(EntityEvent {
                    sequence: record.sequence,
                    kind: EntityEventKind::Input,
                    entity: self.source(target),
                    subject: None,
                    accepted,
                    contact: None,
                    name: input,
                }),
                Transition::Output {
                    caller,
                    output: name,
                    ..
                } => output.events.push(EntityEvent {
                    sequence: record.sequence,
                    kind: EntityEventKind::Output,
                    entity: self.source(caller),
                    subject: None,
                    accepted: true,
                    contact: None,
                    name,
                }),
                Transition::Contact {
                    trigger,
                    subject,
                    kind,
                    accepted,
                    ..
                } => {
                    let trigger_entity = self.source(trigger);
                    output.events.push(EntityEvent {
                        sequence: record.sequence,
                        kind: EntityEventKind::Contact,
                        entity: trigger_entity,
                        subject: Some(self.source(subject)),
                        accepted,
                        contact: Some(kind),
                        name: Vec::new(),
                    });
                    if accepted {
                        output.contacts.push(TriggerContact {
                            sequence: record.sequence,
                            trigger_entity,
                            kind,
                        });
                    }
                }
                Transition::TransformChanged { entity, .. } => output.events.push(EntityEvent {
                    sequence: record.sequence,
                    kind: EntityEventKind::Transform,
                    entity: self.source(entity),
                    subject: None,
                    accepted: true,
                    contact: None,
                    name: Vec::new(),
                }),
                Transition::Request(request) => match request {
                    RuntimeRequest::Mover {
                        request_id,
                        entity,
                        world_destination,
                        speed,
                        ..
                    } => {
                        let current = self
                            .world
                            .entity(entity)
                            .map_or([0.0; 3], |value| value.world_transform.origin);
                        let model = self
                            .world
                            .entity(entity)
                            .and_then(|value| value.definition.bsp_model_index);
                        self.movers.insert(
                            entity,
                            ActiveMover {
                                request_id,
                                entity,
                                model,
                                position: current,
                                destination: world_destination,
                                speed,
                            },
                        );
                        output.events.push(EntityEvent {
                            sequence: record.sequence,
                            kind: EntityEventKind::MoverStarted,
                            entity: self.source(entity),
                            subject: None,
                            accepted: true,
                            contact: None,
                            name: Vec::new(),
                        });
                    }
                    RuntimeRequest::TriggerEffect {
                        trigger,
                        kind,
                        contact,
                        ..
                    } if contact != ContactKind::Exit => {
                        let source = self.source(trigger);
                        let Some(entity) = self.world.entity(trigger) else {
                            continue;
                        };
                        match kind {
                            TriggerKind::Teleport => {
                                if let Some(link) = self.teleports.get(&trigger) {
                                    output.effects.push(Effect::Teleport {
                                        trigger: source,
                                        destination: link.destination.source,
                                        position: link.destination.position,
                                        yaw_degrees: (!link.preserve_angles)
                                            .then_some(link.destination.yaw_degrees),
                                    });
                                }
                            }
                            TriggerKind::Hurt => output.effects.push(Effect::Hurt {
                                trigger: source,
                                damage_per_second: f32::from_bits(match &entity.behavior {
                                    BehaviorState::Trigger(state) => state.mutable_value_bits,
                                    _ => 0,
                                }),
                            }),
                            TriggerKind::Push => {
                                let direction = direction_from_angles(vector(
                                    &entity.definition,
                                    b"pushdir",
                                    Some([0.0; 3]),
                                )?);
                                let speed = number(&entity.definition, b"speed", 0.0)?;
                                output.effects.push(Effect::Push {
                                    trigger: source,
                                    velocity: scale(direction, speed),
                                    replace: false,
                                });
                            }
                            TriggerKind::Catapult => {
                                let direction = direction_from_angles(vector(
                                    &entity.definition,
                                    b"launchDirection",
                                    Some([0.0; 3]),
                                )?);
                                let speed = number(&entity.definition, b"playerSpeed", 0.0)?;
                                output.effects.push(Effect::Push {
                                    trigger: source,
                                    velocity: scale(direction, speed),
                                    replace: true,
                                });
                            }
                            TriggerKind::Multiple => {}
                        }
                    }
                    RuntimeRequest::BlockDamage { mover, blocker, .. } => {
                        output.events.push(EntityEvent {
                            sequence: record.sequence,
                            kind: EntityEventKind::MoverBlocked,
                            entity: self.source(mover),
                            subject: Some(self.source(blocker)),
                            accepted: true,
                            contact: None,
                            name: Vec::new(),
                        })
                    }
                    RuntimeRequest::BrushState { .. } | RuntimeRequest::TriggerEffect { .. } => {}
                },
                Transition::Diagnostic { entity, .. } => output.events.push(EntityEvent {
                    sequence: record.sequence,
                    kind: EntityEventKind::Diagnostic,
                    entity: entity.map_or(u32::MAX, |handle| self.source(handle)),
                    subject: None,
                    accepted: false,
                    contact: None,
                    name: Vec::new(),
                }),
                Transition::Lifecycle { .. }
                | Transition::Scheduled { .. }
                | Transition::Cancelled { .. }
                | Transition::ParentChanged { .. } => {}
            }
        }
        Ok(output)
    }

    fn source(&self, handle: EntityHandle) -> u32 {
        if handle == self.player {
            return u32::MAX;
        }
        self.world
            .entity(handle)
            .and_then(|entity| u32::try_from(entity.source_index).ok())
            .unwrap_or(u32::MAX)
    }
}

#[derive(Debug)]
pub enum MapError {
    Entity(RuntimeFailure),
    Movement(MoveError),
    MissingEntity(u32),
}

impl From<RuntimeFailure> for MapError {
    fn from(value: RuntimeFailure) -> Self {
        Self::Entity(value)
    }
}

impl From<MoveError> for MapError {
    fn from(value: MoveError) -> Self {
        Self::Movement(value)
    }
}

fn invalid(entity: usize) -> RuntimeFailure {
    RuntimeFailure {
        code: playsrc_entity::RuntimeFailureCode::InvalidField,
        entity: Some(entity),
        limit: None,
        actual: None,
    }
}

fn class(entity: &playsrc_entity::Entity, expected: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn field<'a>(entity: &'a playsrc_entity::Entity, name: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_slice())
}

fn integer(
    entity: &playsrc_entity::Entity,
    name: &[u8],
    default: i32,
) -> Result<i32, RuntimeFailure> {
    match field(entity, name) {
        None => Ok(default),
        Some(value) => std::str::from_utf8(value)
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(|| invalid(entity.index)),
    }
}

fn number(
    entity: &playsrc_entity::Entity,
    name: &[u8],
    default: f32,
) -> Result<f32, RuntimeFailure> {
    match field(entity, name) {
        None => Ok(default),
        Some(value) => {
            let value = std::str::from_utf8(value)
                .ok()
                .and_then(|value| value.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(entity.index))?;
            Ok(value)
        }
    }
}

fn vector(
    entity: &playsrc_entity::Entity,
    name: &[u8],
    default: Option<[f32; 3]>,
) -> Result<[f32; 3], RuntimeFailure> {
    let Some(value) = field(entity, name) else {
        return default.ok_or_else(|| invalid(entity.index));
    };
    let values = std::str::from_utf8(value)
        .ok()
        .map(|value| {
            value
                .split_ascii_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .ok()
        .flatten()
        .filter(|values| values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .ok_or_else(|| invalid(entity.index))?;
    Ok([values[0], values[1], values[2]])
}

fn ascii_lower(value: &[u8]) -> Vec<u8> {
    value.iter().map(u8::to_ascii_lowercase).collect()
}

fn direction_from_angles(angles: [f32; 3]) -> [f32; 3] {
    let pitch = angles[0].to_radians();
    let yaw = angles[1].to_radians();
    [
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        -pitch.sin(),
    ]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}
