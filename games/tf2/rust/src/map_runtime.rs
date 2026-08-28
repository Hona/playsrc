use std::collections::BTreeMap;

use playsrc_collision::Hull;
use playsrc_entity::{
    BehaviorState, BrushModelPresentation, ContactKind, ContactRecord, EntityHandle, EntityWorld,
    EntityWorldConfig, EventTarget, ExternalBrushModelBinding, ExternalBrushModelVisibility,
    InputRecord, ModelBounds, RuntimeFailure, RuntimeRequest, Transform, Transition,
    TransitionBatch, TriggerKind, Variant, WorldCommand,
};
use playsrc_movement::{Error as MoveError, Tracer};

use crate::pickup::{
    ITEM_PICKUP_BOX_BLOAT, MAP_PICKUP_RESPAWN_SECONDS, MapPickupDefinition, MapPickupKind,
    PickupSize, map_pickup_definition,
};

pub const CONTENTS_RED_TEAM: u32 = 0x800;
pub const CONTENTS_BLUE_TEAM: u32 = 0x1000;

/// CFuncRegenerate selects the first named entity, then tests prop_dynamic.
/// Presentation preparation shares this join with actual regeneration.
pub fn regenerate_associated_model<'a>(
    graph: &'a playsrc_entity::Graph,
    entity: &playsrc_entity::Entity,
) -> Option<&'a playsrc_entity::Entity> {
    if !class(entity, b"func_regenerate") {
        return None;
    }
    field(entity, b"associatedmodel")
        .filter(|name| !name.is_empty())
        .and_then(|name| graph.entities.iter().find(|candidate| {
            candidate.targetname.as_deref().is_some_and(|target| target.eq_ignore_ascii_case(name))
        }))
        .filter(|candidate| class(candidate, b"prop_dynamic"))
}

pub fn respawn_barrier_collides(
    barrier_team: Option<u8>,
    player_movement: bool,
    contents_mask: u32,
    team_win: bool,
) -> bool {
    if team_win || !player_movement {
        return false;
    }
    match barrier_team {
        Some(2) => contents_mask & CONTENTS_RED_TEAM != 0,
        Some(3) => contents_mask & CONTENTS_BLUE_TEAM != 0,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatPlayerFacts {
    pub team: crate::PlayerTeam,
    pub health: i32,
    pub world_center: [f32; 3],
    pub eye_forward: [f32; 3],
    pub backstab_immune: bool,
}

pub trait GameplayWorld: Tracer {
    /// Host monotonic work clock for NextBot's frame budget. Deterministic
    /// fixture worlds report zero work; runtime adapters supply real elapsed time.
    fn bot_update_milliseconds(&self)->f64{0.0}
    fn has_player_hitbox_models(&self) -> bool { false }

    fn pose_player_hitboxes(&self, _actors: &[crate::PlayerHitboxPose], _tick: u64, _interval: f32) -> Result<Vec<crate::PosedPlayerHitbox>, MoveError> {
        Ok(Vec::new())
    }

    fn combat_player(&self, _identity: u32) -> Option<CombatPlayerFacts> {
        None
    }

    fn collision_snapshot_revision(&self) -> Option<u64> {
        None
    }

    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, MoveError>;

    fn overlaps_transformed_model_hull(&self, model:usize, transform:Transform, position:[f32;3], hull:Hull) -> Result<bool,MoveError> {
        if transform.angles != [0.0;3] { return Err(MoveError::new(playsrc_movement::Operation::Trace,playsrc_movement::FailureKind::Missing,"rotated capture-area collision")); }
        self.overlaps_model_hull(model,transform.origin,position,hull)
    }
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
    pub respawn_rooms: u32,
    pub capture_flags: u32,
    pub capture_zones: u32,
    pub health_pickups: u32,
    pub ammo_pickups: u32,
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
        angles: Option<[f32; 3]>,
    },
    Hurt {
        trigger: u32,
        damage_per_second: f32,
        damage_type: u32,
        contact: ContactKind,
    },
    Push {
        trigger: u32,
        velocity: [f32; 3],
        replace: bool,
    },
    Regenerate {
        entity: u32,
        team: Option<u8>,
        associated_model: Option<u32>,
    },
    RespawnRoom {
        entity: u32,
        team: Option<u8>,
        contact: ContactKind,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPickupSnapshot {
    pub identity: u32,
    pub kind: MapPickupKind,
    pub size: PickupSize,
    pub team: Option<u8>,
    pub available: bool,
    pub disabled: bool,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub respawn_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PickupContactCandidate {
    pub identity: u32,
    pub definition: MapPickupDefinition,
    pub team: Option<u8>,
    pub origin: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegenerateContact {
    pub sequence: u64,
    pub entity: u32,
    pub kind: ContactKind,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerContactFacts {
    pub team: u8,
    pub class: u8,
    pub observer: bool,
    pub conditions: [u32; 5],
    pub winning_team: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorContact {
    pub identity: u32,
    pub position: [f32; 3],
    pub hull: Hull,
    pub facts: PlayerContactFacts,
    pub alive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GameFilter {
    Team { team: u8, negated: bool },
    Class { class: u8, negated: bool },
    Condition { condition: u8, negated: bool },
}

impl GameFilter {
    fn passes(&self, player: PlayerContactFacts) -> bool {
        match *self {
            Self::Team { team, negated } => {
                if player.winning_team == Some(player.team) {
                    true
                } else {
                    (player.team == team) != negated
                }
            }
            Self::Class { class, negated } => {
                (!player.observer && player.class == class) != negated
            }
            Self::Condition { condition, negated } => {
                let value = usize::from(condition);
                let present =
                    value < 160 && player.conditions[value / 32] & (1_u32 << (value % 32)) != 0;
                present != negated
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerContact {
    pub sequence: u64,
    pub trigger_entity: u32,
    pub kind: ContactKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapPhase {
    pub payload_events: Vec<crate::payload::Event>,
    pub events: Vec<EntityEvent>,
    pub control_point_events: Vec<crate::control_point::Event>,
    pub effects: Vec<Effect>,
    pub contacts: Vec<TriggerContact>,
    pub regenerate_contacts: Vec<RegenerateContact>,
    pub mover_requests: Vec<MoverRequest>,
    pub carry: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverRequest {
    pub request_id: u64,
    pub entity: u32,
    pub model: Option<u32>,
    pub start: [f32; 3],
    pub start_angles: [f32; 3],
    pub destination: [f32; 3],
    pub destination_angles: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub speed: f32,
    pub opening: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoverResultKind {
    Progress,
    Completed,
    BlockedStart,
    BlockedStay,
    BlockedEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoverResult {
    pub request_id: u64,
    pub entity: u32,
    pub kind: MoverResultKind,
    pub transform: Transform,
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
        self.payload_events.append(&mut other.payload_events);
        self.events.append(&mut other.events);
        self.control_point_events.append(&mut other.control_point_events);
        self.effects.append(&mut other.effects);
        self.contacts.append(&mut other.contacts);
        self.regenerate_contacts
            .append(&mut other.regenerate_contacts);
        self.mover_requests.append(&mut other.mover_requests);
        self.carry = add(self.carry, other.carry);
    }
}

#[derive(Clone, Debug)]
struct Volume {
    source: u32,
    handle: Option<EntityHandle>,
    model: usize,
    origin: [f32; 3],
    bounds: Option<([f32; 3], [f32; 3])>,
    kind: VolumeKind,
    touching: bool,
}

#[derive(Clone, Debug)]
enum VolumeKind {
    Generic,
    Soundscape { spectator_next: f32, spectator_touching: bool },
    Regenerate {
        enabled: bool,
        team: Option<u8>,
        associated_model: Option<u32>,
        associated_body: Option<i32>,
    },
    RespawnRoom {
        enabled: bool,
        team: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug)]
struct TeleportDestination {
    source: u32,
    position: [f32; 3],
    angles: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
struct TeleportLink {
    destination: TeleportDestination,
    landmark: Option<TeleportDestination>,
    preserve_angles: bool,
}

#[derive(Clone, Copy, Debug)]
struct ActiveMover {
    request_id: u64,
}

#[derive(Clone, Debug)]
struct MapPickup {
    source: u32,
    handle: EntityHandle,
    definition: MapPickupDefinition,
    team: Option<u8>,
    disabled: bool,
    auto_materialize: bool,
    origin: [f32; 3],
    angles: [f32; 3],
    respawn_tick: Option<u64>,
}

#[derive(Clone, Debug)]
struct BuildingExclusion {
    source: u32,
    model: usize,
    origin: [f32; 3],
    bounds: Option<([f32; 3], [f32; 3])>,
    team: Option<u8>,
    enabled: bool,
    allow_sentry: bool,
    allow_dispenser: bool,
    allow_teleporters: bool,
}

#[derive(Clone, Debug)]
pub struct MapRuntime {
    world: EntityWorld,
    soundscapes: playsrc_entity::soundscape::Systems,
    soundscape_player: playsrc_entity::soundscape::Player,
    player: EntityHandle,
    actor_handles: BTreeMap<u32, EntityHandle>,
    source_handles: BTreeMap<u32, EntityHandle>,
    volumes: Vec<Volume>,
    pickups: Vec<MapPickup>,
    building_exclusions: Vec<BuildingExclusion>,
    team_spawns: [Option<crate::PlayerSpawn>; 2],
    teleports: BTreeMap<EntityHandle, TeleportLink>,
    movers: BTreeMap<EntityHandle, ActiveMover>,
    prop_animations: BTreeMap<EntityHandle, crate::dynamic_prop::Animation>,
    particle_systems: playsrc_entity::particle_system::Systems,
    smokestacks: playsrc_entity::smokestack::Systems,
    sprites: playsrc_entity::sprite::Sprites,
    suns:playsrc_entity::sun::Suns,
    ropes:playsrc_entity::rope::Ropes,
    spotlights:Vec<playsrc_entity::spotlight::Beam>,
    spotlight_collision:Option<crate::spotlight::Collision>,
    sprite_models: std::sync::Arc<BTreeMap<Vec<u8>,u32>>,
    game_filters: BTreeMap<Vec<u8>, GameFilter>,
    objectives: Option<crate::ctf::World>,
    control_points: Option<crate::control_point::World>,
    control_point_facts: crate::control_point::Facts,
    restart_definitions: Option<std::sync::Arc<Vec<playsrc_entity::Entity>>>,
    restart_models: Option<std::sync::Arc<BTreeMap<String, std::sync::Arc<playsrc_studio_model::PresentationModel>>>>,
    model_skin_counts: std::sync::Arc<BTreeMap<usize, usize>>,
    round_configuration: crate::round::Configuration,
    round_inputs: Vec<(u32, Vec<u8>, Variant)>,
    counts: MapCounts,
    payload_constraint_blocked: bool,
    payload_watchers: Vec<crate::payload::Watcher>,
    tick_interval: f32,
    next_producer_sequence: u64,
    last_player_position: [f32; 3],
}

impl MapRuntime {
    pub(crate) fn is_brush_model(&self, source: u32) -> bool {
        self.source_handles.get(&source).and_then(|handle| self.world.entity(*handle)).is_some_and(|entity| entity.render.brush_model.is_some())
    }

    pub fn initialize_soundscapes(&mut self, registry: &playsrc_audio::soundscape::Registry) {
        self.soundscapes = playsrc_entity::soundscape::Systems::from_world(&self.world, |name| registry.find(name));
    }

    pub fn soundscape_zones(&self) -> Vec<([f32; 3], f32)> {
        self.soundscapes.zones().iter().map(|zone| {
            (self.world.entity(zone.entity).map_or([0.0; 3], |entity| entity.world_transform.origin), zone.radius)
        }).collect()
    }

    pub fn soundscape_selection(&self) -> playsrc_entity::soundscape::Selection { self.soundscape_player.selection }

    pub fn update_soundscape(&mut self, ear: [f32; 3], candidates: &[usize],
        trace: impl FnMut([f32; 3], [f32; 3]) -> playsrc_entity::soundscape::Trace) -> Result<MapPhase, RuntimeFailure> {
        let mut played = Vec::new();
        self.soundscapes.update(&self.world, &mut self.soundscape_player, ear, candidates, trace, &mut played);
        if played.is_empty() { return Ok(MapPhase::default()); }
        let commands = played.into_iter().map(soundscape_output).collect::<Vec<_>>();
        let batch = self.world.phase(self.world.current_tick(), &commands)?;
        self.consume(batch)
    }

    pub fn compile(
        graph: &playsrc_entity::Graph,
        tick_interval: f32,
        source_identity: u64,
        model_bounds: Vec<ModelBounds>,
    ) -> Result<Self, RuntimeFailure> {
        let volume_bounds = model_bounds
            .iter()
            .map(|bounds| (bounds.model, (bounds.mins, bounds.maxs)))
            .collect::<BTreeMap<_, _>>();
        let mut round_configuration =
            crate::round::Configuration::from_graph(graph).map_err(|_| invalid(0))?;
        let mut control_points = crate::control_point::World::from_graph(graph).map_err(|_| invalid(0))?;
        if let Some(points) = &mut control_points { points.set_model_bounds(&model_bounds); round_configuration.score_per_round = !points.master().score_per_capture; }
        let mut objectives =
            crate::ctf::World::compile(graph, crate::ctf::Configuration::default()).map_err(
                |error| match error {
                    crate::ctf::Error::InvalidEntity(entity) => invalid(entity as usize),
                    _ => invalid(0),
                },
            )?;
        if let Some(objectives) = &mut objectives {
            objectives.set_model_bounds(&model_bounds);
        }
        let mut config = EntityWorldConfig {
            tick_interval,
            load_kind: playsrc_entity::MapLoadKind::MultiplayerNewMap,
            source_identity,
            registry_identity: 0x5446_325f_454e_5434,
            model_bounds,
            external_classes: vec![
                playsrc_entity::particle_system::binding(),
                playsrc_entity::smokestack::binding(),
                playsrc_entity::ExternalClassBinding {
                    classname: b"info_player_teamspawn".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable", b"RoundSpawn", b"RoundActivate"].into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"team_control_point".to_vec(),
                    inputs: [b"SetOwner".as_slice(), b"ShowModel", b"HideModel", b"RoundActivate", b"SetLocked", b"SetUnlockTime"]
                        .into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"team_control_point_master".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable", b"SetWinner", b"SetWinnerAndForceCaps", b"RoundSpawn", b"RoundActivate", b"SetCapLayout", b"SetCapLayoutCustomPositionX", b"SetCapLayoutCustomPositionY"]
                        .into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"team_control_point_round".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable", b"RoundSpawn"].into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"trigger_capture_area".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable", b"Toggle", b"RoundSpawn", b"SetTeamCanCap", b"SetControlPoint", b"CaptureCurrentCP"]
                        .into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"func_regenerate".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable", b"Toggle"]
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"func_respawnroom".to_vec(),
                    inputs: [
                        b"SetActive".as_slice(),
                        b"SetInactive",
                        b"ToggleActive",
                        b"RoundActivate",
                    ]
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"item_teamflag".to_vec(),
                    inputs: [
                        b"Enable".as_slice(),
                        b"Disable",
                        b"RoundActivate",
                        b"ForceDrop",
                        b"ForceReset",
                        b"ForceResetSilent",
                        b"ForceResetAndDisableSilent",
                        b"SetReturnTime",
                        b"ShowTimer",
                        b"ForceGlowDisabled",
                    ]
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"func_capturezone".to_vec(),
                    inputs: [b"Enable".as_slice(), b"Disable"]
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"func_nobuild".to_vec(),
                    inputs: [b"SetActive".as_slice(), b"SetInactive", b"ToggleActive"]
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"team_train_watcher".to_vec(),
                    inputs: crate::payload::INPUTS.iter().map(|input| input.to_vec()).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"team_round_timer".to_vec(),
                    inputs: [
                        b"Enable".as_slice(),
                        b"Disable",
                        b"Pause",
                        b"Resume",
                        b"SetTime",
                        b"AddTime",
                        b"AddTeamTime",
                        b"ShowInHUD",
                        b"RoundSpawn",
                    ]
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"tf_logic_koth".to_vec(),
                    inputs: [b"RoundSpawn".as_slice(), b"RoundActivate", b"SetRedTimer", b"SetBlueTimer", b"AddRedTimer", b"AddBlueTimer"]
                        .into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"tf_gamerules".to_vec(),
                    inputs: [b"SetRedKothClockActive".as_slice(), b"SetBlueKothClockActive", b"SetRedTeamRespawnWaveTime", b"SetBlueTeamRespawnWaveTime"]
                        .into_iter().map(<[u8]>::to_vec).collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"game_round_win".to_vec(),
                    inputs: [b"RoundWin".as_slice(), b"SetTeam"]
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"filter_activator_tfteam".to_vec(),
                    inputs: [b"TestActivator".as_slice(), b"RoundSpawn", b"RoundActivate"]
                        .into_iter()
                        .map(<[u8]>::to_vec)
                        .collect(),
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"filter_tf_class".to_vec(),
                    inputs: vec![b"TestActivator".to_vec()],
                },
                playsrc_entity::ExternalClassBinding {
                    classname: b"filter_tf_condition".to_vec(),
                    inputs: vec![b"TestActivator".to_vec()],
                },
            ],
            pickup_classes: [
                b"item_healthkit_small".as_slice(),
                b"item_healthkit_medium",
                b"item_healthkit_full",
                b"item_ammopack_small",
                b"item_ammopack_medium",
                b"item_ammopack_full",
            ]
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect(),
            external_brush_models: [
                b"func_regenerate".as_slice(),
                b"func_respawnroom",
                b"func_nobuild",
                b"func_capturezone",
            ]
            .into_iter()
            .map(|classname| ExternalBrushModelBinding {
                classname: classname.to_vec(),
                initial_visibility: ExternalBrushModelVisibility::Hidden,
            })
            .collect(),
            ..EntityWorldConfig::default()
        };
        // CTFGameRules::CreateStandardEntities provides the named rules proxy
        // even when a map has no authored tf_gamerules entity.
        let mut standard_graph;
        let entity_graph = if !graph.entities.iter().any(|e| class(e, b"tf_gamerules")) {
            standard_graph = graph.clone();
            let mut proxy = playsrc_entity::parse(b"{\"classname\"\"tf_gamerules\"\"targetname\"\"tf_gamerules\"}\0", Default::default()).expect("standard rules proxy").entities.remove(0);
            // KOTH's two generated timer identities immediately follow the BSP.
            proxy.index = graph.entities.iter().map(|e| e.index).max().unwrap_or(0) + 3;
            standard_graph.entities.push(proxy);
            &standard_graph
        } else { graph };
        config.external_classes.extend(playsrc_entity::soundscape::bindings());
        config.external_classes.extend(playsrc_entity::sprite::bindings());
        config.external_classes.push(playsrc_entity::sun::binding());
        config.external_classes.extend(playsrc_entity::rope::bindings());
        config.external_classes.extend(playsrc_entity::spotlight::bindings());
        let (mut world, _) = EntityWorld::compile(entity_graph, config)?;
        if let Some(koth) = round_configuration.koth {
            for (identity, name) in [(koth.blue_timer, "zz_blue_koth_timer"), (koth.red_timer, "zz_red_koth_timer")] {
                let mut definition = playsrc_entity::parse(
                    format!("{{\"classname\"\"team_round_timer\"\"targetname\"\"{name}\"}}\0").as_bytes(),
                    playsrc_entity::Limits::default(),
                ).expect("generated KOTH timer").entities.remove(0);
                definition.index = identity as usize;
                world.phase(0, &[WorldCommand::Spawn(definition)])?;
            }
        }
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

        let mut game_filters = BTreeMap::new();
        for entity in &graph.entities {
            let Some(name) = entity.targetname.as_ref().filter(|name| !name.is_empty()) else {
                continue;
            };
            let negated = boolean(entity, b"Negated", false);
            let filter = if class(entity, b"filter_activator_tfteam") {
                let team = integer(entity, b"TeamNum", 0);
                if !(0..=u8::MAX as i32).contains(&team) {
                    return Err(invalid(entity.index));
                }
                Some(GameFilter::Team {
                    team: team as u8,
                    negated,
                })
            } else if class(entity, b"filter_tf_class") {
                let class = integer(entity, b"tfclass", 0);
                if !(0..=u8::MAX as i32).contains(&class) {
                    return Err(invalid(entity.index));
                }
                Some(GameFilter::Class {
                    class: class as u8,
                    negated,
                })
            } else if class(entity, b"filter_tf_condition") {
                let condition = integer(entity, b"condition", 0);
                if !(0..160).contains(&condition) {
                    return Err(invalid(entity.index));
                }
                Some(GameFilter::Condition {
                    condition: condition as u8,
                    negated,
                })
            } else {
                None
            };
            if let Some(filter) = filter
                && game_filters.insert(ascii_lower(name), filter).is_some()
            {
                return Err(invalid(entity.index));
            }
        }

        let mut points = BTreeMap::<Vec<u8>, TeleportDestination>::new();
        let mut destination_count = 0_u32;
        for entity in &graph.entities {
            if let Some(name) = entity.targetname.clone().filter(|value| !value.is_empty()) {
                let position = vector(entity, b"origin", Some([0.0; 3]))?;
                let angles = vector(entity, b"angles", Some([0.0; 3]))?;
                points
                    .entry(ascii_lower(&name))
                    .or_insert(TeleportDestination {
                        source: u32::try_from(entity.index).map_err(|_| invalid(entity.index))?,
                        position,
                        angles,
                    });
            }
            if class(entity, b"info_teleport_destination") {
                if entity.targetname.as_ref().is_none_or(Vec::is_empty) {
                    return Err(invalid(entity.index));
                }
                destination_count = destination_count.saturating_add(1);
            }
        }

        let mut counts = MapCounts {
            teleport_destinations: destination_count,
            capture_flags: objectives
                .as_ref()
                .map_or(0, |objectives| objectives.flags().count() as u32),
            capture_zones: objectives
                .as_ref()
                .map_or(0, |objectives| objectives.zones().count() as u32),
            ..MapCounts::default()
        };
        let mut volumes = Vec::new();
        let mut pickups = Vec::new();
        let mut building_exclusions = Vec::new();
        let mut team_spawns = [None; 2];
        let mut teleports = BTreeMap::new();
        for entity in &graph.entities {
            if class(entity, b"info_player_teamspawn") && !boolean(entity, b"StartDisabled", false)
            {
                if let Some(team) = source_team(entity)? {
                    let index = usize::from(team == 3);
                    if team_spawns[index].is_none() {
                        team_spawns[index] = Some(crate::PlayerSpawn { position: vector(entity, b"origin", None)?, angles: vector(entity, b"angles", Some([0.0; 3]))? });
                    }
                }
            }
            if let Some(definition) = entity.classname.as_deref().and_then(map_pickup_definition) {
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                match definition.kind {
                    MapPickupKind::Health => counts.health_pickups += 1,
                    MapPickupKind::Ammo => counts.ammo_pickups += 1,
                }
                pickups.push(MapPickup {
                    source,
                    handle: *source_handles
                        .get(&source)
                        .ok_or_else(|| invalid(entity.index))?,
                    definition,
                    team: source_team(entity)?,
                    disabled: boolean(entity, b"StartDisabled", false),
                    auto_materialize: boolean(entity, b"AutoMaterialize", true),
                    origin: vector(entity, b"origin", Some([0.0; 3]))?,
                    angles: vector(entity, b"angles", Some([0.0; 3]))?,
                    respawn_tick: None,
                });
            }
            counts.buttons += u32::from(class(entity, b"func_button"));
            counts.doors += u32::from(class(entity, b"func_door"));
            counts.linear_movers += u32::from(class(entity, b"func_movelinear"));
            let generic_kind = if class(entity, b"trigger_soundscape") {
                Some(TriggerKind::Soundscape)
            } else if class(entity, b"trigger_multiple") {
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
                    bounds: volume_bounds.get(&model).copied(),
                    kind: if kind == TriggerKind::Soundscape { VolumeKind::Soundscape { spectator_next: 0.2, spectator_touching: false } } else { VolumeKind::Generic },
                    touching: false,
                });
                if kind == TriggerKind::Teleport {
                    let target = field(entity, b"target")
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| invalid(entity.index))?;
                    let destination = *points
                        .get(&ascii_lower(target))
                        .ok_or_else(|| invalid(entity.index))?;
                    let landmark = field(entity, b"landmark")
                        .filter(|value| !value.is_empty())
                        .and_then(|name| points.get(&ascii_lower(name)))
                        .copied();
                    let flags = integer(entity, b"spawnflags", 0);
                    teleports.insert(
                        handle,
                        TeleportLink {
                            destination,
                            landmark,
                            preserve_angles: flags & 0x20 != 0,
                        },
                    );
                }
            } else if class(entity, b"func_nobuild") {
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                let model = entity
                    .bsp_model_index
                    .ok_or_else(|| invalid(entity.index))?;
                building_exclusions.push(BuildingExclusion {
                    source,
                    model,
                    origin: vector(entity, b"origin", Some([0.0; 3]))?,
                    bounds: volume_bounds.get(&model).copied(),
                    team: source_team(entity)?,
                    enabled: !boolean(entity, b"StartDisabled", false),
                    allow_sentry: boolean(entity, b"AllowSentry", false),
                    allow_dispenser: boolean(entity, b"AllowDispenser", false),
                    allow_teleporters: boolean(entity, b"AllowTeleporters", false),
                });
            } else if class(entity, b"func_regenerate") {
                counts.regenerate_zones += 1;
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                let team = source_team(entity)?;
                let associated = regenerate_associated_model(graph, entity);
                let associated_model =
                    associated.and_then(|candidate| u32::try_from(candidate.index).ok());
                let associated_body =
                    associated.map(|candidate| integer(candidate, b"SetBodyGroup", 0));
                volumes.push(Volume {
                    source,
                    handle: source_handles.get(&source).copied(),
                    model: entity
                        .bsp_model_index
                        .ok_or_else(|| invalid(entity.index))?,
                    origin: vector(entity, b"origin", Some([0.0; 3]))?,
                    bounds: entity
                        .bsp_model_index
                        .and_then(|model| volume_bounds.get(&model).copied()),
                    kind: VolumeKind::Regenerate {
                        enabled: !boolean(entity, b"StartDisabled", false),
                        team,
                        associated_model,
                        associated_body,
                    },
                    touching: false,
                });
            } else if class(entity, b"func_respawnroom") {
                counts.respawn_rooms += 1;
                let source = u32::try_from(entity.index).map_err(|_| invalid(entity.index))?;
                let team = source_team(entity)?;
                volumes.push(Volume {
                    source,
                    handle: source_handles.get(&source).copied(),
                    model: entity
                        .bsp_model_index
                        .ok_or_else(|| invalid(entity.index))?,
                    origin: vector(entity, b"origin", Some([0.0; 3]))?,
                    bounds: entity
                        .bsp_model_index
                        .and_then(|model| volume_bounds.get(&model).copied()),
                    kind: VolumeKind::RespawnRoom {
                        enabled: !boolean(entity, b"StartDisabled", false),
                        team,
                    },
                    touching: false,
                });
            }
        }
        volumes.sort_by_key(|volume| volume.source);
        let payload_constraint_blocked = graph.entities.iter().any(|constraint| {
            if !class(constraint, b"phys_constraint") {
                return false;
            }
            let Some(first) = field(constraint, b"attach1") else {
                return false;
            };
            let Some(second) = field(constraint, b"attach2") else {
                return false;
            };
            let attached = |name: &[u8], expected: &[u8]| {
                graph.entities.iter().any(|candidate| {
                    class(candidate, expected)
                        && candidate
                            .targetname
                            .as_deref()
                            .is_some_and(|value| value.eq_ignore_ascii_case(name))
                })
            };
            attached(first, b"func_tracktrain")
                && (attached(second, b"prop_physics") || attached(second, b"prop_physics_override"))
                || attached(second, b"func_tracktrain")
                    && (attached(first, b"prop_physics")
                        || attached(first, b"prop_physics_override"))
        });
        let restart_definitions = control_points.as_ref().map(|_| std::sync::Arc::new(source_handles.values().filter_map(|handle| world.entity(*handle).map(|e| e.definition.as_ref().clone())).collect()));
        let particle_systems = playsrc_entity::particle_system::Systems::from_world(&world, 0.0);
        let suns=playsrc_entity::sun::Suns::from_world(&world).map_err(|source|invalid(source as usize))?;
        let ropes=playsrc_entity::rope::Ropes::from_world(&world).map_err(|source|invalid(source as usize))?;
        let mut smokestacks = playsrc_entity::smokestack::Systems::default();
        smokestacks.synchronize(&world).map_err(|_| invalid(0))?;
        Ok(Self {
            particle_systems,
            smokestacks,
            soundscapes: playsrc_entity::soundscape::Systems::default(),
            soundscape_player: playsrc_entity::soundscape::Player::default(),
            sprites: playsrc_entity::sprite::Sprites::default(),
            suns,
            ropes,
            spotlights:Vec::new(),spotlight_collision:None,
            sprite_models: std::sync::Arc::default(),
            world,
            player,
            actor_handles: BTreeMap::new(),
            source_handles,
            volumes,
            pickups,
            building_exclusions,
            team_spawns,
            teleports,
            movers: BTreeMap::new(),
            prop_animations: BTreeMap::new(),
            game_filters,
            objectives,
            control_points,
            control_point_facts: crate::control_point::Facts::default(),
            restart_definitions,
            restart_models: None,
            model_skin_counts: std::sync::Arc::default(),
            round_configuration,
            round_inputs: Vec::new(),
            counts,
            payload_constraint_blocked,
            payload_watchers: crate::payload::Watcher::from_entities(&graph.entities),
            tick_interval,
            next_producer_sequence: 1,
            last_player_position: [0.0; 3],
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

    pub fn payload_constraint_blocked(&self) -> bool {
        self.payload_constraint_blocked
    }

    pub fn payload_watchers(&self) -> &[crate::payload::Watcher] { &self.payload_watchers }

    pub fn payload_timer_may_expire(&self) -> bool {
        self.payload_watchers.iter().all(crate::payload::Watcher::timer_may_expire)
    }

    pub fn advance_payload(&mut self, tick:u64, running:bool, overtime:bool) -> Result<MapPhase,RuntimeFailure> {
        let mut events=Vec::new();
        if let Some(points)=&self.control_points {
            for watcher in &mut self.payload_watchers { watcher.think(&self.world,points,tick as f32*self.tick_interval,running,overtime,&mut events); }
        }
        self.apply_payload_events(tick,events)
    }

    pub fn payload_overtime_started(&mut self,tick:u64) -> Result<MapPhase,RuntimeFailure> {
        let mut events=Vec::new();
        if let Some(points)=&self.control_points {
            for watcher in &mut self.payload_watchers { watcher.input(b"OnStartOvertime",&Variant::Void,None,&self.world,points,tick as f32*self.tick_interval,true,&mut events); }
        }
        self.apply_payload_events(tick,events)
    }

    fn apply_payload_events(&mut self,tick:u64,events:Vec<crate::payload::Event>) -> Result<MapPhase,RuntimeFailure> {
        use crate::{payload::Event,control_point::Event as PointEvent,AudioAction,SoundDefinition};
        let mut phase=MapPhase::default();
        for event in &events {
            let mut commands=Vec::new();
            match event {
                Event::TrainInput { train,input,value } => {
                    // This is a policy request, not a replacement cart solver.
                    // Never advance a train away from its unimplemented cart body.
                    if !self.payload_constraint_blocked || *input!=b"SetSpeedDirAccel" || value.as_float()==Some(0.0) {
                        commands.push(WorldCommand::Input(InputRecord {target:EventTarget::Direct(*train),input:input.to_vec(),value:value.clone(),activator:None,caller:None,output_action:None,producer_sequence:self.next_producer_sequence}));
                        self.next_producer_sequence+=1;
                    }
                }
                Event::StartRecede { watcher } => if let Some(entity)=self.source_handle(*watcher) { commands.push(WorldCommand::EmitOutput {entity,output:b"OnTrainStartRecede".to_vec(),value:Variant::Void,activator:Some(entity),caller:Some(entity),delay:0.0}); },
                Event::Sparks { name,active } => if !name.is_empty() && !self.payload_constraint_blocked {
                    commands.push(WorldCommand::Input(InputRecord {target:EventTarget::Expression(name.clone()),input:if *active {b"StartSpark".to_vec()} else {b"StopSpark".to_vec()},value:Variant::Void,activator:None,caller:None,output_action:None,producer_sequence:self.next_producer_sequence}));self.next_producer_sequence+=1;
                }
                Event::CaptureAlert { point,final_point } => phase.control_point_events.push(PointEvent::Sound {point:*point,recipient:None,definition:if *final_point{SoundDefinition::CartFinalWarning}else{SoundDefinition::CartWarning},action:AudioAction::Play}),
                Event::AlarmStart {point} | Event::AlarmSingle {point} | Event::AlarmStop {point} => phase.control_point_events.push(PointEvent::Sound {point:*point,recipient:None,definition:if matches!(event,Event::AlarmSingle{..}){SoundDefinition::CartAlarmSingle}else{SoundDefinition::CartAlarm},action:if matches!(event,Event::AlarmStop{..}){AudioAction::Stop}else{AudioAction::Play}}),
                Event::Speak {..} | Event::Pushed {..} => {},
            }
            if !commands.is_empty(){let batch=self.world.phase(tick,&commands)?;phase.append(self.consume(batch)?);}
        }
        phase.payload_events.extend(events);
        Ok(phase)
    }

    pub fn pickups(&self) -> Vec<MapPickupSnapshot> {
        self.pickups
            .iter()
            .map(|pickup| MapPickupSnapshot {
                identity: pickup.source,
                kind: pickup.definition.kind,
                size: pickup.definition.size,
                team: pickup.team,
                available: !pickup.disabled
                    && self.world.entity(pickup.handle).is_some_and(|entity| {
                        matches!(&entity.behavior, BehaviorState::Pickup(state) if state.visible && state.touchable)
                    }),
                disabled: pickup.disabled,
                origin: pickup.origin,
                angles: pickup.angles,
                respawn_tick: pickup.respawn_tick,
            })
            .collect()
    }

    pub(crate) fn supply_targets(&self) -> Vec<crate::bot::SupplyTarget> {
        let mut targets = self
            .pickups()
            .into_iter()
            .filter(|pickup| pickup.available)
            .map(|pickup| crate::bot::SupplyTarget {
                identity: pickup.identity,
                kind: Some(pickup.kind),
                team: pickup.team.and_then(|team| match team {
                    2 => Some(crate::PlayerTeam::Red),
                    3 => Some(crate::PlayerTeam::Blue),
                    _ => None,
                }),
                position: pickup.origin,
            })
            .collect::<Vec<_>>();
        for volume in &self.volumes {
            if let VolumeKind::Regenerate {
                enabled: true,
                team,
                associated_model,
                ..
            } = volume.kind
            {
                let position = associated_model
                    .and_then(|identity| self.source_handle(identity))
                    .and_then(|handle| self.world.entity(handle))
                    .map_or(volume.origin, |entity| entity.world_transform.origin);
                targets.push(crate::bot::SupplyTarget {
                    identity: volume.source,
                    kind: None,
                    team: team.and_then(|value| match value {
                        2 => Some(crate::PlayerTeam::Red),
                        3 => Some(crate::PlayerTeam::Blue),
                        _ => None,
                    }),
                    position,
                });
            }
        }
        targets
    }

    pub(crate) fn bot_regenerate_zones<W: GameplayWorld>(
        &self,
        collision: &W,
        position: [f32; 3],
        hull: Hull,
        team: crate::PlayerTeam,
    ) -> Result<Vec<u32>, MapError> {
        let mut zones = Vec::new();
        for volume in &self.volumes {
            if let VolumeKind::Regenerate {
                enabled: true,
                team: required,
                ..
            } = volume.kind
                && required.is_none_or(|value| value == team.source_number())
                && collision.overlaps_model_hull(volume.model, volume.origin, position, hull)?
            {
                zones.push(volume.source);
            }
        }
        Ok(zones)
    }

    pub(crate) fn pickup_candidates<W: GameplayWorld>(
        &self,
        collision: &W,
        position: [f32; 3],
        hull: Hull,
        eye_height: f32,
    ) -> Result<Vec<PickupContactCandidate>, MapError> {
        let mut candidates = Vec::new();
        for pickup in &self.pickups {
            if pickup.disabled
                || !self.world.entity(pickup.handle).is_some_and(|entity| {
                    matches!(&entity.behavior, BehaviorState::Pickup(state) if state.visible && state.touchable)
                })
            {
                continue;
            }
            let minimum = [
                pickup.origin[0] + pickup.definition.minimum[0] - ITEM_PICKUP_BOX_BLOAT,
                pickup.origin[1] + pickup.definition.minimum[1] - ITEM_PICKUP_BOX_BLOAT,
                pickup.origin[2] + pickup.definition.minimum[2],
            ];
            let maximum = [
                pickup.origin[0] + pickup.definition.maximum[0] + ITEM_PICKUP_BOX_BLOAT,
                pickup.origin[1] + pickup.definition.maximum[1] + ITEM_PICKUP_BOX_BLOAT,
                pickup.origin[2] + pickup.definition.maximum[2] + ITEM_PICKUP_BOX_BLOAT * 0.5,
            ];
            if (0..3).any(|axis| {
                position[axis] + hull.maxs[axis] < minimum[axis]
                    || position[axis] + hull.mins[axis] > maximum[axis]
            }) {
                continue;
            }
            let center = std::array::from_fn(|axis| {
                pickup.origin[axis]
                    + (pickup.definition.minimum[axis] + pickup.definition.maximum[axis]) * 0.5
            });
            let eye = [position[0], position[1], position[2] + eye_height];
            let trace = collision
                .trace(
                    center,
                    eye,
                    Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    crate::MASK_SOLID,
                )
                .map_err(MapError::PickupTrace)?;
            if trace.fraction < 1.0 {
                continue;
            }
            candidates.push(PickupContactCandidate {
                identity: pickup.source,
                definition: pickup.definition,
                team: pickup.team,
                origin: pickup.origin,
            });
        }
        Ok(candidates)
    }

    pub(crate) fn begin_pickup(
        &mut self,
        tick: u64,
        source: u32,
        actor: u32,
    ) -> Result<MapPhase, MapError> {
        let pickup = self
            .pickups
            .iter()
            .find(|pickup| pickup.source == source)
            .ok_or(MapError::MissingEntity(source))?;
        let handle = pickup.handle;
        let subject = self.actor_handle(tick, actor)?;
        let batch = self.world.phase(
            tick,
            &[WorldCommand::PickupContact {
                entity: handle,
                subject,
                unobstructed: true,
            }],
        )?;
        self.consume(batch).map_err(MapError::from)
    }

    pub(crate) fn finish_pickup(
        &mut self,
        tick: u64,
        source: u32,
        actor: u32,
        accepted: bool,
    ) -> Result<MapPhase, MapError> {
        let index = self
            .pickups
            .iter()
            .position(|pickup| pickup.source == source)
            .ok_or(MapError::MissingEntity(source))?;
        let handle = self.pickups[index].handle;
        let subject = self.actor_handle(tick, actor)?;
        let respawn = crate::ticks(MAP_PICKUP_RESPAWN_SECONDS, self.tick_interval);
        if accepted {
            self.pickups[index].respawn_tick = Some(tick + respawn);
        }
        let batch = self.world.phase(
            tick,
            &[WorldCommand::PickupResult {
                entity: handle,
                subject,
                accepted,
                respawn_ticks: accepted.then_some(respawn),
                respawn_transform: None,
            }],
        )?;
        self.consume(batch).map_err(MapError::from)
    }

    fn actor_handle(&mut self, tick: u64, actor: u32) -> Result<EntityHandle, MapError> {
        if actor == crate::PLAYER_IDENTITY {
            return Ok(self.player);
        }
        if let Some(handle) = self.actor_handles.get(&actor).copied() {
            return Ok(handle);
        }
        let definition = playsrc_entity::parse(
            format!("{{\"classname\"\"player\"\"targetname\"\"bot_{actor}\"}}\0").as_bytes(),
            playsrc_entity::Limits::default(),
        )
        .map_err(|_| MapError::MissingEntity(actor))?
        .entities
        .into_iter()
        .next()
        .ok_or(MapError::MissingEntity(actor))?;
        let batch = self.world.phase(tick, &[WorldCommand::Spawn(definition)])?;
        let handle = batch
            .records
            .iter()
            .find_map(|record| match record.transition {
                Transition::Lifecycle { entity, .. } => Some(entity),
                _ => None,
            })
            .ok_or(MapError::MissingEntity(actor))?;
        self.actor_handles.insert(actor, handle);
        Ok(handle)
    }

    pub(crate) fn building_position_allowed<W: GameplayWorld>(
        &self,
        world: &W,
        object: crate::building::Object,
        team: crate::PlayerTeam,
        position: [f32; 3],
    ) -> Result<bool, MoveError> {
        let point = Hull {
            mins: [0.0; 3],
            maxs: [0.0; 3],
        };
        let center = [
            position[0],
            position[1],
            position[2] + object.hull().maxs[2] * 0.5,
        ];
        for value in &self.building_exclusions {
            if !value.enabled
                || value
                    .team
                    .is_some_and(|number| number != team.source_number())
            {
                continue;
            }
            let allowed = match object.kind {
                crate::building::Kind::Sentry => value.allow_sentry,
                crate::building::Kind::Dispenser => value.allow_dispenser,
                crate::building::Kind::Teleporter => value.allow_teleporters,
            };
            if !allowed {
                for test in [position, center] {
                    if bounds_may_overlap(value.bounds, value.origin, test, point)
                        && world.overlaps_model_hull(value.model, value.origin, test, point)?
                    {
                        return Ok(false);
                    }
                }
            }
        }
        for volume in &self.volumes {
            if !matches!(volume.kind, VolumeKind::RespawnRoom { enabled: true, .. }) {
                continue;
            }
            for test in [position, center] {
                if volume_may_overlap(volume, volume.origin, test, point)
                    && world.overlaps_model_hull(volume.model, volume.origin, test, point)?
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn team_spawn(&self, team: crate::PlayerTeam) -> Option<crate::PlayerSpawn> {
        if let Some(points) = &self.control_points { return points.spawns().iter().find(|s| s.team == team && !s.disabled).map(|s| crate::PlayerSpawn { position: s.position, angles: s.angles }); }
        match team {
            crate::PlayerTeam::Red => self.team_spawns[0],
            crate::PlayerTeam::Blue => self.team_spawns[1],
            _ => None,
        }
    }

    pub(crate) fn point_in_friendly_respawn_room<W: crate::GameplayWorld>(&self, world: &W, team: crate::PlayerTeam, position: [f32; 3]) -> Result<bool, MoveError> {
        let point = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        for volume in &self.volumes {
            if let VolumeKind::RespawnRoom { enabled: true, team: owner } = volume.kind
                && owner.is_none_or(|owner| owner == team.source_number())
                && volume_may_overlap(volume, volume.origin, position, point)
                && world.overlaps_model_hull(volume.model, volume.origin, position, point)?
            { return Ok(true); }
        }
        Ok(false)
    }

    pub(crate) fn entity_revision(&self) -> u64 {
        self.world.revision()
    }

    pub(crate) fn studio_model_presentation(
        &self,
        expected_revision: u64,
    ) -> Result<Vec<playsrc_entity::StudioModelDrawState>, RuntimeFailure> {
        let mut models = self.world.studio_model_presentation(expected_revision)?;
        for model in &mut models {
            if let Some(count) = self.model_skin_counts.get(&model.source_index) { model.skin = playsrc_studio_model::source_skin_family(model.skin, *count) as i32; }
        }
        Ok(models)
    }

    pub fn install_studio_models(
        &mut self,
        models: &BTreeMap<String, std::sync::Arc<playsrc_studio_model::PresentationModel>>,
    ) -> Result<(), RuntimeFailure> {
        let mut definitions = BTreeMap::new();
        let mut skin_counts = BTreeMap::new();
        if self.restart_definitions.is_some() { self.restart_models = Some(std::sync::Arc::new(models.clone())); }
        if let Some(points) = &mut self.control_points { points.install_models(models); }
        for handle in self.source_handles.values().copied() {
            let Some(entity) = self.world.entity(handle) else {
                continue;
            };
            let BehaviorState::DynamicProp(_) = &entity.behavior else {
                continue;
            };
            let Some(path) = field(&entity.definition, b"model") else {
                continue;
            };
            let Some(model) = models.get(&String::from_utf8_lossy(path).to_lowercase()) else {
                continue;
            };
            skin_counts.insert(entity.source_index, model.skins.len());
            let definition = if let Some(definition) = definitions.get(&model.identity) {
                std::sync::Arc::clone(definition)
            } else {
                let definition = std::sync::Arc::new(crate::dynamic_prop::Definition::compile(model));
                definitions.insert(model.identity.clone(), definition.clone());
                definition
            };
            // Leave the admitted authored occurrence alone until Entity emits
            // an animation request. Publication must not activate unrelated
            // map models or add per-frame pose work for fixed occurrences.
            self.prop_animations
                .insert(handle, crate::dynamic_prop::Animation::new(definition));
        }
        self.model_skin_counts = std::sync::Arc::new(skin_counts);
        Ok(())
    }

    pub(crate) fn studio_animation_presentation(
        &self,
    ) -> Vec<crate::dynamic_prop::AnimationPresentation> {
        let now = self.world.current_tick() as f32 * self.tick_interval;
        self.prop_animations
            .iter()
            .filter_map(|(handle, animation)| {
                self.world
                    .entity(*handle)
                    .and_then(|entity| animation.presentation(entity.source_index as u32, now))
            })
            .collect()
    }

    pub(crate) fn brush_model_presentation(
        &self,
        expected_revision: u64,
    ) -> Result<BrushModelPresentation, RuntimeFailure> {
        self.world.brush_model_presentation(expected_revision)
    }

    pub fn source_handle(&self, source: u32) -> Option<EntityHandle> {
        self.source_handles.get(&source).copied()
    }

    pub fn visual_entity(&self, source: u32) -> Option<(EntityHandle, playsrc_entity::Transform, playsrc_entity::EntityRenderState)> {
        let entity = self.world.entity(self.source_handle(source)?)?;
        Some((entity.handle, entity.world_transform, entity.render.clone()))
    }

    pub fn install_sprite_models(&mut self, models:BTreeMap<Vec<u8>,u32>) -> Result<(),RuntimeFailure> {
        let sprites=playsrc_entity::sprite::Sprites::from_world(&self.world,self.world.current_tick() as f32*self.tick_interval,|name|models.get(name).copied())
            .map_err(|source|invalid(source as usize))?;
        self.sprites=sprites;self.sprite_models=std::sync::Arc::new(models);Ok(())
    }

    pub fn sprite_state(&self,source:u32)->Option<playsrc_entity::sprite::Presentation> { self.sprites.presentation(&self.world,source) }
    pub fn sun_state(&self,source:u32)->Option<playsrc_entity::sun::Presentation>{self.suns.get(&self.world,source)}
    pub fn rope_state(&self,source:u32)->Option<(playsrc_entity::rope::Definition,[Option<[f32;3]>;2])>{self.ropes.get(&self.world,source)}
    pub fn install_spotlights(&mut self,world:std::sync::Arc<playsrc_collision::World>,inputs:Vec<playsrc_collision::ObjectInput>)->Result<MapPhase,RuntimeFailure>{
        let collision=crate::spotlight::Collision::new(world,inputs);
        let (seeds,commands)=collision.prepare(&self.world).map_err(|source|invalid(source as usize))?;
        let batch=self.world.phase(self.world.current_tick(),&commands)?;
        self.spotlights=playsrc_entity::spotlight::bind(&self.world,seeds).map_err(|source|invalid(source as usize))?;
        self.spotlight_collision=Some(collision);self.consume(batch)
    }
    pub fn spotlight_state(&self,source:u32)->Option<(playsrc_entity::spotlight::Beam,playsrc_entity::EntityRenderState)>{
        playsrc_entity::spotlight::presentation(&self.world,self.spotlights.iter().find(|beam|beam.source==source)?)
    }

    pub fn collision_entity(&self,source:u32)->Option<playsrc_entity::EntityCollisionState>{
        self.world.collision_state(*self.source_handles.get(&source)?)
    }

    pub fn mover_hierarchy(&self,source:u32)->Vec<(u32,playsrc_entity::EntityCollisionState,Vec<playsrc_entity::Transform>)>{
        let Some(root_handle)=self.source_handles.get(&source).copied()else{return Vec::new();};
        let Some(root)=self.world.entity(root_handle)else{return Vec::new();};
        let mut pending=root.children.iter().rev().copied().collect::<Vec<_>>();let mut output=Vec::new();
        while let Some(handle)=pending.pop(){
            let Some(entity)=self.world.entity(handle)else{continue;};
            if let Ok(source)=u32::try_from(entity.source_index){output.push((source,self.world.collision_state(handle).expect("live hierarchy member"),self.world.descendant_local_chain(root_handle,handle).expect("descendant chain")));}
            pending.extend(entity.children.iter().rev().copied());
        }
        output
    }

    pub fn round_configuration(&self) -> crate::round::Configuration {
        self.round_configuration.clone()
    }

    pub fn apply_round_inputs(&mut self, rules: &mut crate::round::Rules, now: f32) {
        for (entity, input, value) in self.round_inputs.drain(..) {
            if input.eq_ignore_ascii_case(b"AddTeamTime") {
                if let Variant::String(value) = &value { rules.add_team_time(entity, value, now); }
                continue;
            }
            let number = value.as_float().unwrap_or(0.0);
            let integer = match &value {
                Variant::Integer(value) => *value,
                Variant::String(value) => playsrc_entity::source_integer(value),
                Variant::Bool(value) => i32::from(*value),
                _ => number as i32,
            };
            if input.eq_ignore_ascii_case(b"SetRedTeamRespawnWaveTime") { rules.set_respawn_wave(crate::PlayerTeam::Red, number); }
            else if input.eq_ignore_ascii_case(b"SetBlueTeamRespawnWaveTime") { rules.set_respawn_wave(crate::PlayerTeam::Blue, number); }
            else { rules.apply_input(entity, &input, integer, now); }
        }
    }

    pub fn objectives(&self) -> Option<&crate::ctf::World> {
        self.objectives.as_ref()
    }

    pub fn objectives_mut(&mut self) -> Option<&mut crate::ctf::World> {
        self.objectives.as_mut()
    }

    pub fn control_points(&self) -> Option<&crate::control_point::World> {
        self.control_points.as_ref()
    }

    pub fn control_points_mut(&mut self) -> Option<&mut crate::control_point::World> {
        self.control_points.as_mut()
    }

    pub fn particle_systems(&self) -> Vec<playsrc_entity::particle_system::Presentation> {
        self.particle_systems.presentation(&self.world)
    }

    pub fn smokestacks(&self) -> Vec<playsrc_entity::smokestack::Presentation> {
        self.smokestacks.presentation(&self.world)
    }

    pub fn restart_control_point_map(&mut self, tick: u64) -> Result<MapPhase, RuntimeFailure> {
        let Some(definitions) = self.restart_definitions.clone() else { return Ok(MapPhase::default()); };
        let mut payload_events=Vec::new();
        for watcher in &mut self.payload_watchers { watcher.stop_alarm(&mut payload_events); }
        let mut payload_phase=self.apply_payload_events(tick,payload_events)?;
        let actors: std::collections::BTreeSet<_> = self.actor_handles.values().copied().chain([self.player]).collect();
        let removals: Vec<_> = self.world.live_handles().into_iter().filter(|handle| !actors.contains(handle)
            && self.world.entity(*handle).is_some_and(|entity| !preserved_on_round_restart(&entity.definition))).map(WorldCommand::Remove).collect();
        self.world.clear_event_queue();
        self.world.set_map_load_kind(playsrc_entity::MapLoadKind::MultiplayerNewRound);
        let removed = self.world.phase(tick, &removals)?;
        let mut result = self.consume(removed)?;
        result.append(std::mem::take(&mut payload_phase));
        self.world.clear_event_queue();
        let excluded_sources=definitions.iter().filter(|entity|preserved_on_round_restart(entity)).map(|entity|entity.index).collect();
        let spawned=self.world.phase(tick,&[WorldCommand::SpawnMapEntities{definitions:definitions.to_vec(),excluded_sources}])?;
        self.source_handles.clear();
        for handle in self.world.live_handles().into_iter().filter(|handle| !actors.contains(handle)) {
            if let Some(entity) = self.world.entity(handle) { self.source_handles.insert(entity.source_index as u32, handle); }
        }
        self.movers.clear();
        self.round_inputs.clear();
        for volume in &mut self.volumes {
            if volume.handle.is_some() { volume.handle = self.source_handles.get(&volume.source).copied(); }
            if !matches!(volume.kind, VolumeKind::Soundscape { .. }) { volume.touching = false; }
            let enabled = definitions.iter().find(|e| e.index == volume.source as usize).is_none_or(|e| !boolean(e, b"StartDisabled", false));
            match &mut volume.kind {
                VolumeKind::Regenerate { enabled: state, .. } | VolumeKind::RespawnRoom { enabled: state, .. } => *state = enabled,
                VolumeKind::Generic | VolumeKind::Soundscape { .. } => {}
            }
        }
        for pickup in &mut self.pickups {
            if let Some(handle) = self.source_handles.get(&pickup.source) { pickup.handle = *handle; }
            pickup.respawn_tick = None;
            pickup.disabled = definitions.iter().find(|e| e.index == pickup.source as usize).is_some_and(|e| boolean(e, b"StartDisabled", false));
        }
        for exclusion in &mut self.building_exclusions {
            exclusion.enabled = definitions.iter().find(|e| e.index == exclusion.source as usize).is_none_or(|e| !boolean(e, b"StartDisabled", false));
        }
        self.prop_animations.clear();
        self.payload_watchers=crate::payload::Watcher::from_entities(&definitions);
        self.particle_systems = playsrc_entity::particle_system::Systems::from_world(&self.world, tick as f32 * self.tick_interval);
        self.sprites.reconcile(&self.world,tick as f32*self.tick_interval,|name|self.sprite_models.get(name).copied()).map_err(|source|invalid(source as usize))?;
        self.suns.reconcile(&self.world).map_err(|source|invalid(source as usize))?;
        self.ropes.reconcile(&self.world).map_err(|source|invalid(source as usize))?;
        let (seeds,commands)=self.spotlight_collision.as_ref().map(|collision|collision.prepare(&self.world)).transpose().map_err(|source|invalid(source as usize))?.unwrap_or_default();
        let beams=self.world.phase(tick,&commands)?;
        self.spotlights=playsrc_entity::spotlight::bind(&self.world,seeds).map_err(|source|invalid(source as usize))?;
        result.append(self.consume(beams)?);
        if let Some(models) = self.restart_models.clone() { self.install_studio_models(&models)?; }
        result.append(self.consume(spawned)?);
        Ok(result)
    }

    pub fn set_control_point_facts(&mut self, facts: crate::control_point::Facts) {
        self.control_point_facts = facts;
    }

    pub fn sync_capture_area_transforms(&mut self) {
        if let Some(points)=&mut self.control_points {
            points.update_area_transforms(|source|self.world.entity(*self.source_handles.get(&source)?).map(|entity|entity.world_transform));
        }
    }

    pub fn activate_control_point_round(&mut self, tick: u64, facts: crate::control_point::Facts, random: &mut crate::UniformRandomStream) -> Result<MapPhase, RuntimeFailure> {
        self.control_point_facts = facts;
        let sources: Vec<_> = self.source_handles.iter().map(|(source, handle)| (*source, *handle)).collect();
        let mut result = MapPhase::default();
        for input in [b"RoundSpawn".as_slice(), b"RoundActivate"] {
            for (source, handle) in &sources {
                if !self.world.accepts_external_input(*handle, input) { continue; }
                if input == b"RoundActivate" && self.control_points.as_ref().is_some_and(|p| p.master().identity == *source) {
                    let mut events = Vec::new();
                    self.control_points.as_mut().unwrap().select_round(None, random, &mut events);
                    result.append(self.emit_control_point_outputs(tick, &events)?);
                    result.control_point_events.extend(events);
                }
                let batch = self.world.phase(tick, &[WorldCommand::Input(InputRecord { target: EventTarget::Direct(*handle), input: input.to_vec(), value: Variant::Void, activator: None, caller: None, output_action: None, producer_sequence: self.next_producer_sequence })])?;
                self.next_producer_sequence += 1;
                result.append(self.consume(batch)?);
                if input == b"RoundActivate" {
                    if let Some(koth) = self.round_configuration.koth.filter(|koth| koth.identity == *source) {
                        let mut events = Vec::new();
                        if let Some(points) = &mut self.control_points { koth.round_activate(points, tick as f32 * self.tick_interval, facts, &mut events); }
                        result.append(self.emit_control_point_outputs(tick, &events)?);
                        result.control_point_events.extend(events);
                    }
                }
            }
        }
        Ok(result)
    }

    pub fn emit_control_point_outputs(&mut self, tick: u64, events: &[crate::control_point::Event]) -> Result<MapPhase, RuntimeFailure> {
        let mut commands = Vec::new();
        let mut payload_events=Vec::new();
        for event in events {
            let crate::control_point::Event::MapOutput { entity, output, value } = event else { continue; };
            let handle = self.source_handle(*entity).ok_or_else(|| invalid(*entity as usize))?;
            commands.push(WorldCommand::EmitOutput { entity: handle, output: output.as_bytes().to_vec(), value: value.clone(), activator: Some(handle), caller: Some(handle), delay: 0.0 });
            if *output=="OnNumCappersChanged2" && let Variant::Integer(count)=value {
                let blocked=self.control_points.as_ref().and_then(|points|points.areas().iter().find(|area|area.identity==*entity)).is_some_and(|area|area.blocked);
                for watcher in &mut self.payload_watchers {
                    if watcher.capture_area==Some(*entity) { watcher.set_cappers(*count,Some((*entity,blocked)),tick as f32*self.tick_interval,self.control_point_facts.in_overtime,&mut payload_events); }
                }
            }
        }
        if commands.is_empty() { return Ok(MapPhase::default()); }
        let batch = self.world.phase(tick, &commands)?;
        let mut phase=self.consume(batch)?;
        phase.append(self.apply_payload_events(tick,payload_events)?);
        Ok(phase)
    }

    pub fn emit_objective_outputs(
        &mut self,
        tick: u64,
        events: &[crate::ctf::Event],
    ) -> Result<MapPhase, MapError> {
        let mut commands = Vec::new();
        for event in events {
            let crate::ctf::Event::MapOutput {
                entity,
                output,
                activator,
            } = event
            else {
                continue;
            };
            let Some(handle) = self.source_handle(*entity) else {
                return Err(MapError::MissingEntity(*entity));
            };
            let activator = match activator {
                Some(identity) => Some(self.actor_handle(tick, *identity)?),
                None => Some(handle),
            };
            commands.push(WorldCommand::EmitOutput {
                entity: handle,
                output: output.as_bytes().to_vec(),
                value: Variant::Void,
                activator,
                caller: Some(handle),
                delay: 0.0,
            });
        }
        if commands.is_empty() {
            return Ok(MapPhase::default());
        }
        let batch = self.world.phase(tick, &commands)?;
        self.consume(batch).map_err(MapError::from)
    }

    pub fn emit_round_outputs(
        &mut self,
        tick: u64,
        events: &[crate::round::Event],
    ) -> Result<MapPhase, MapError> {
        let mut commands = Vec::new();
        for event in events {
            let (source, output) = match *event {
                crate::round::Event::SetupFinished { timer } => {
                    (timer, b"OnSetupFinished".as_slice())
                }
                crate::round::Event::TimerFinished { timer } => (timer, b"OnFinished".as_slice()),
                crate::round::Event::MapRoundWin { entity } => (entity, b"OnRoundWin".as_slice()),
                crate::round::Event::TimerThreshold { timer, seconds } => (timer, match seconds {
                    300 => b"On5MinRemain".as_slice(), 240 => b"On4MinRemain", 180 => b"On3MinRemain",
                    120 => b"On2MinRemain", 60 => b"On1MinRemain", 30 => b"On30SecRemain",
                    10 => b"On10SecRemain", 5 => b"On5SecRemain", 4 => b"On4SecRemain",
                    3 => b"On3SecRemain", 2 => b"On2SecRemain", 1 => b"On1SecRemain", _ => continue,
                }),
                _ => continue,
            };
            let handle = self
                .source_handle(source)
                .ok_or(MapError::MissingEntity(source))?;
            commands.push(WorldCommand::EmitOutput {
                entity: handle,
                output: output.to_vec(),
                value: Variant::Void,
                activator: Some(handle),
                caller: Some(handle),
                delay: 0.0,
            });
        }
        if commands.is_empty() {
            return Ok(MapPhase::default());
        }
        let batch = self.world.phase(tick, &commands)?;
        self.consume(batch).map_err(MapError::from)
    }

    pub fn input(
        &mut self,
        tick: u64,
        source: u32,
        input: &[u8],
        value: Variant,
    ) -> Result<MapPhase, MapError> {
        let handle = self
            .source_handle(source)
            .ok_or(MapError::MissingEntity(source))?;
        let batch = self.world.phase(
            tick,
            &[WorldCommand::Input(InputRecord {
                target: EventTarget::Direct(handle),
                input: input.to_vec(),
                value,
                activator: Some(self.player),
                caller: Some(self.player),
                output_action: None,
                producer_sequence: self.next_producer_sequence,
            })],
        )?;
        self.next_producer_sequence += 1;
        self.consume(batch).map_err(MapError::from)
    }

    pub fn fire_input(&mut self, tick: u64, target: &[u8], input: &[u8], value: &[u8], delay: f32) -> Result<MapPhase, MapError> {
        let record = InputRecord { target: EventTarget::Expression(target.to_vec()), input: input.to_vec(), value: Variant::String(value.to_vec()),
            activator: Some(self.player), caller: Some(self.player), output_action: None, producer_sequence: self.next_producer_sequence };
        self.next_producer_sequence += 1;
        let batch = self.world.enqueue_input(tick, record, delay)?;
        self.consume(batch).map_err(MapError::from)
    }

    pub fn damage(&mut self, tick: u64, source: u32) -> Result<MapPhase, MapError> {
        let handle = self
            .source_handle(source)
            .ok_or(MapError::MissingEntity(source))?;
        let batch = self.world.phase(
            tick,
            &[WorldCommand::Damage {
                entity: handle,
                attacker: Some(self.player),
            }],
        )?;
        self.consume(batch).map_err(MapError::from)
    }

    pub fn regenerate_associated_body(&self, source: u32) -> Option<i32> {
        self.volumes.iter().find_map(|volume| {
            if volume.source != source {
                return None;
            }
            match volume.kind {
                VolumeKind::Regenerate {
                    associated_body, ..
                } => associated_body,
                _ => None,
            }
        })
    }

    pub fn accepts_course_trigger(&self, source: u32) -> bool {
        self.volumes
            .iter()
            .any(|volume| volume.source == source && matches!(volume.kind, VolumeKind::Generic))
    }

    pub fn begin_tick<W: GameplayWorld>(
        &mut self,
        _collision: &W,
        input: BeginTickInput,
    ) -> Result<MapPhase, MapError> {
        self.last_player_position = input.player_position;
        let mut commands = Vec::new();
        for (handle, animation) in &mut self.prop_animations {
            if animation
                .think(input.tick as f32 * self.tick_interval)
                .map_err(|_| invalid(usize::from(handle.slot)))?
            {
                commands.push(WorldCommand::DynamicPropAnimationCompleted { entity: *handle });
            }
        }
        if let Some(source) = input.activate_entity {
            let handle = self
                .source_handle(source)
                .ok_or(MapError::MissingEntity(source))?;
            commands.push(WorldCommand::Damage {
                entity: handle,
                attacker: Some(self.player),
            });
        }
        let batch = self.world.phase(input.tick, &commands)?;
        for pickup in &mut self.pickups {
            if pickup.auto_materialize
                && pickup.respawn_tick.is_some_and(|due| input.tick >= due)
                && self.world.entity(pickup.handle).is_some_and(|entity| {
                    matches!(&entity.behavior, BehaviorState::Pickup(state) if state.visible)
                })
            {
                pickup.respawn_tick = None;
            }
        }
        let mut output=self.consume(batch).map_err(MapError::from)?;
        let commands=self.sprites.advance(&self.world,input.tick as f32*self.tick_interval);
        if !commands.is_empty() {let batch=self.world.phase(input.tick,&commands)?;output.append(self.consume(batch)?);}
        Ok(output)
    }

    pub fn apply_mover_results(
        &mut self,
        tick: u64,
        results: &[MoverResult],
    ) -> Result<MapPhase, MapError> {
        let mut commands = Vec::new();
        let mut carry = [0.0; 3];
        let mut seen = std::collections::BTreeSet::new();
        for result in results {
            if !seen.insert(result.entity)
                || result
                    .transform
                    .origin
                    .into_iter()
                    .chain(result.transform.angles)
                    .chain(result.carry)
                    .any(|value| !value.is_finite())
            {
                return Err(MapError::MissingEntity(result.entity));
            }
            let handle = self
                .source_handle(result.entity)
                .ok_or(MapError::MissingEntity(result.entity))?;
            let active = self
                .movers
                .get(&handle)
                .copied()
                .filter(|active| active.request_id == result.request_id)
                .ok_or(MapError::MissingEntity(result.entity))?;
            match result.kind {
                MoverResultKind::Progress | MoverResultKind::Completed => {
                    commands.push(WorldCommand::SetWorldTransform {
                        entity: handle,
                        transform: result.transform,
                    });
                    carry = add(carry, result.carry);
                    if result.kind == MoverResultKind::Completed {
                        commands.push(WorldCommand::MoverCompleted {
                            entity: handle,
                            request_id: result.request_id,
                        });
                        self.movers.remove(&handle);
                    }
                }
                MoverResultKind::BlockedStart
                | MoverResultKind::BlockedStay
                | MoverResultKind::BlockedEnd => commands.push(WorldCommand::MoverBlocked {
                    entity: handle,
                    request_id: active.request_id,
                    blocker: self.player,
                    kind: match result.kind {
                        MoverResultKind::BlockedStart => playsrc_entity::BlockContactKind::Start,
                        MoverResultKind::BlockedStay => playsrc_entity::BlockContactKind::Stay,
                        MoverResultKind::BlockedEnd => playsrc_entity::BlockContactKind::End,
                        _ => unreachable!(),
                    },
                }),
            }
        }
        let batch = self.world.phase(tick, &commands)?;
        let mut output = self.consume(batch)?;
        output.carry = carry;
        Ok(output)
    }

    pub fn contact_phase<W: GameplayWorld>(
        &mut self,
        collision: &W,
        tick: u64,
        position: [f32; 3],
        hull: Hull,
        player: PlayerContactFacts,
        actors: &[ActorContact],
    ) -> Result<MapPhase, MapError> {
        self.last_player_position = position;
        let mut commands = Vec::new();
        let mut effects = Vec::new();
        let mut regenerate_contacts = Vec::new();
        for volume in &mut self.volumes {
            let origin = volume
                .handle
                .and_then(|handle| self.world.entity(handle))
                .map_or(volume.origin, |entity| entity.world_transform.origin);
            let overlap = collision.overlaps_model_hull(volume.model, origin, position, hull)?;
            match &mut volume.kind {
                VolumeKind::Soundscape { spectator_next, spectator_touching } => {
                    let handle = volume.handle.expect("soundscape trigger handle");
                    let Some(entity) = self.world.entity(handle) else { continue; };
                    let enabled = matches!(&entity.behavior, BehaviorState::Trigger(state) if state.enabled);
                    let now = tick as f32 * self.tick_interval;
                    let contact = if player.observer {
                        if now < *spectator_next { continue; }
                        *spectator_next = now + 0.2;
                        let previous = *spectator_touching;
                        *spectator_touching = overlap;
                        match (overlap, previous) { (true, false) => Some(ContactKind::Enter), (false, true) => Some(ContactKind::Exit), _ => None }
                    } else {
                        let current = overlap && enabled;
                        let previous = volume.touching;
                        volume.touching = current;
                        match (current, previous) { (true, false) => Some(ContactKind::Enter), (false, true) => Some(ContactKind::Exit), _ => None }
                    };
                    if let Some(kind) = contact {
                        let mut played = Vec::new();
                        self.soundscapes.touch(&self.world, handle, kind == ContactKind::Enter, &mut self.soundscape_player, &mut played);
                        commands.extend(played.into_iter().map(soundscape_output));
                        commands.push(WorldCommand::Contact(ContactRecord { trigger: handle, subject: self.player, kind,
                            external_filter_result: None, producer_sequence: self.next_producer_sequence }));
                        self.next_producer_sequence += 1;
                    }
                }
                VolumeKind::Regenerate {
                    enabled,
                    team,
                    associated_model,
                    ..
                } => {
                    let contact = match (overlap, volume.touching) {
                        (true, false) => Some(ContactKind::Enter),
                        (true, true) => Some(ContactKind::Stay),
                        (false, true) => Some(ContactKind::Exit),
                        (false, false) => None,
                    };
                    volume.touching = overlap;
                    if let Some(kind) = contact {
                        regenerate_contacts.push(RegenerateContact {
                            sequence: self.next_producer_sequence,
                            entity: volume.source,
                            kind,
                            enabled: *enabled,
                        });
                        self.next_producer_sequence += 1;
                    }
                    if overlap && *enabled {
                        effects.push(Effect::Regenerate {
                            entity: volume.source,
                            team: *team,
                            associated_model: *associated_model,
                        });
                    }
                }
                VolumeKind::RespawnRoom { enabled, team } => {
                    let current = overlap && *enabled;
                    let contact = match (current, volume.touching) {
                        (true, false) => Some(ContactKind::Enter),
                        (true, true) => Some(ContactKind::Stay),
                        (false, true) => Some(ContactKind::Exit),
                        (false, false) => None,
                    };
                    volume.touching = current;
                    if let Some(contact) = contact {
                        effects.push(Effect::RespawnRoom {
                            entity: volume.source,
                            team: *team,
                            contact,
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
                        let external_filter_result = self.world.entity(handle).and_then(|entity| {
                            field(&entity.definition, b"filtername")
                                .filter(|name| !name.is_empty())
                                .and_then(|name| self.game_filters.get(&ascii_lower(name)))
                                .map(|filter| filter.passes(player))
                        });
                        commands.push(WorldCommand::Contact(ContactRecord {
                            trigger: handle,
                            subject: self.player,
                            kind,
                            external_filter_result,
                            producer_sequence: self.next_producer_sequence,
                        }));
                        self.next_producer_sequence += 1;
                    }
                }
            }
        }
        let (mut actor_commands, mut output) =
            self.bot_contact_commands(collision, tick, actors)?;
        commands.append(&mut actor_commands);
        let batch = self.world.phase(tick, &commands)?;
        output.append(self.consume(batch)?);
        output.effects.extend(effects);
        output.regenerate_contacts.extend(regenerate_contacts);
        Ok(output)
    }

    fn bot_contact_commands<W: GameplayWorld>(
        &mut self,
        collision: &W,
        tick: u64,
        actors: &[ActorContact],
    ) -> Result<(Vec<WorldCommand>, MapPhase), MapError> {
        let removed = self
            .actor_handles
            .iter()
            .filter(|(identity, _)| !actors.iter().any(|actor| actor.identity == **identity))
            .map(|(identity, handle)| (*identity, *handle))
            .collect::<Vec<_>>();
        let mut output = MapPhase::default();
        if !removed.is_empty() {
            let commands = removed
                .iter()
                .map(|(_, handle)| WorldCommand::Remove(*handle))
                .collect::<Vec<_>>();
            let batch = self.world.phase(tick, &commands)?;
            output.append(self.consume(batch)?);
            for (identity, _) in removed {
                self.actor_handles.remove(&identity);
            }
        }

        for actor in actors.iter().filter(|actor| actor.alive) {
            self.actor_handle(tick, actor.identity)?;
        }

        let mut commands = Vec::new();
        for actor in actors {
            let Some(subject) = self.actor_handles.get(&actor.identity).copied() else {
                continue;
            };
            for volume in &self.volumes {
                if !matches!(volume.kind, VolumeKind::Generic) {
                    continue;
                }
                let trigger = volume.handle.expect("generic trigger handle");
                let accepted = self.world.entity(trigger).is_some_and(|entity| {
                    matches!(
                        &entity.behavior,
                        BehaviorState::Trigger(state) if state.contacts.contains(&subject)
                    )
                });
                let origin = self
                    .world
                    .entity(trigger)
                    .map_or(volume.origin, |entity| entity.world_transform.origin);
                let overlap = actor.alive
                    && volume_may_overlap(volume, origin, actor.position, actor.hull)
                    && collision.overlaps_model_hull(
                        volume.model,
                        origin,
                        actor.position,
                        actor.hull,
                    )?;
                let kind = match (overlap, accepted) {
                    (true, true) => Some(ContactKind::Stay),
                    (true, false) => Some(ContactKind::Enter),
                    (false, true) => Some(ContactKind::Exit),
                    (false, false) => None,
                };
                let Some(kind) = kind else {
                    continue;
                };
                let external_filter_result = self.world.entity(trigger).and_then(|entity| {
                    field(&entity.definition, b"filtername")
                        .filter(|name| !name.is_empty())
                        .and_then(|name| self.game_filters.get(&ascii_lower(name)))
                        .map(|filter| filter.passes(actor.facts))
                });
                commands.push(WorldCommand::Contact(ContactRecord {
                    trigger,
                    subject,
                    kind,
                    external_filter_result,
                    producer_sequence: self.next_producer_sequence,
                }));
                self.next_producer_sequence += 1;
            }
        }
        Ok((commands, output))
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

    pub fn entity_world_transform(&self, identity: u32) -> Option<Transform> {
        self.world.entity(*self.source_handles.get(&identity)?).map(|entity| entity.world_transform)
    }

    pub fn entity_descends_from(&self, identity: u32, ancestor: u32) -> bool {
        let Some(handle) = self.source_handles.get(&identity) else { return false; };
        let Some(ancestor) = self.source_handles.get(&ancestor) else { return false; };
        let mut parent = self.world.entity(*handle).and_then(|entity| entity.parent);
        while let Some(handle) = parent {
            if handle == *ancestor { return true; }
            parent = self.world.entity(handle).and_then(|entity| entity.parent);
        }
        false
    }

    fn consume(&mut self, batch: TransitionBatch) -> Result<MapPhase, RuntimeFailure> {
        if batch.records.iter().any(|record| matches!(record.transition, Transition::Lifecycle { .. })) {
            self.smokestacks.synchronize(&self.world).map_err(|_| invalid(0))?;
        }
        let mut output = MapPhase::default();
        for record in batch.records {
            match record.transition {
                Transition::PathTrackPassed {node} => {
                    let mut events=Vec::new();
                    for watcher in &mut self.payload_watchers { watcher.path_passed(node,&self.world,&mut events); }
                    output.append(self.apply_payload_events(self.world.current_tick(),events)?);
                }
                Transition::Input {
                    target,
                    input,
                    accepted,
                    ..
                } => {
                    let source = self.source(target);
                    if accepted
                        && let Some(pickup) = self
                            .pickups
                            .iter_mut()
                            .find(|pickup| pickup.source == source)
                    {
                        if input.eq_ignore_ascii_case(b"Enable") {
                            pickup.disabled = false;
                            if !pickup.auto_materialize {
                                pickup.respawn_tick = None;
                            }
                        } else if input.eq_ignore_ascii_case(b"Disable") {
                            pickup.disabled = true;
                        } else if input.eq_ignore_ascii_case(b"Toggle") {
                            pickup.disabled = !pickup.disabled;
                            if !pickup.disabled && !pickup.auto_materialize {
                                pickup.respawn_tick = None;
                            }
                        }
                    }
                    output.events.push(EntityEvent {
                        sequence: record.sequence,
                        kind: EntityEventKind::Input,
                        entity: source,
                        subject: None,
                        accepted,
                        contact: None,
                        name: input,
                    });
                }
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
                    if accepted && subject == self.player {
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
                        world_angles_destination,
                        angular_velocity,
                        speed,
                        opening,
                        ..
                    } => {
                        let current = self
                            .world
                            .entity(entity)
                            .map_or(Transform::IDENTITY, |value| value.world_transform);
                        let model = self
                            .world
                            .entity(entity)
                            .and_then(|value| value.definition.bsp_model_index);
                        if speed == 0.0 && angular_velocity == [0.0; 3] {
                            self.movers.remove(&entity);
                        } else {
                            self.movers.insert(entity, ActiveMover { request_id });
                        }
                        output.mover_requests.push(MoverRequest {
                            request_id,
                            entity: self.source(entity),
                            model: model.and_then(|value| u32::try_from(value).ok()),
                            start: current.origin,
                            start_angles: current.angles,
                            destination: world_destination,
                            destination_angles: world_angles_destination,
                            angular_velocity,
                            speed,
                            opening,
                        });
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
                        subject,
                        kind,
                        contact,
                        ..
                    } => {
                        if subject != self.player {
                            continue;
                        }
                        let source = self.source(trigger);
                        let Some(entity) = self.world.entity(trigger) else {
                            continue;
                        };
                        match kind {
                            TriggerKind::Teleport => {
                                if contact != ContactKind::Exit
                                    && let Some(link) = self.teleports.get(&trigger)
                                {
                                    let position = match link.landmark {
                                        Some(landmark) => add(
                                            link.destination.position,
                                            [
                                                self.last_player_position[0] - landmark.position[0],
                                                self.last_player_position[1] - landmark.position[1],
                                                self.last_player_position[2] - landmark.position[2],
                                            ],
                                        ),
                                        None => link.destination.position,
                                    };
                                    output.effects.push(Effect::Teleport {
                                        trigger: source,
                                        destination: link.destination.source,
                                        position,
                                        angles: (link.landmark.is_none() && !link.preserve_angles)
                                            .then_some(link.destination.angles),
                                    });
                                }
                            }
                            TriggerKind::Hurt => output.effects.push(Effect::Hurt {
                                trigger: source,
                                damage_type: match &entity.behavior {
                                    BehaviorState::Trigger(state) => state.damage_type as u32,
                                    _ => 0,
                                },
                                damage_per_second: f32::from_bits(match &entity.behavior {
                                    BehaviorState::Trigger(state) => state.mutable_value_bits,
                                    _ => 0,
                                }),
                                contact,
                            }),
                            TriggerKind::Push => {
                                if contact == ContactKind::Exit {
                                    continue;
                                }
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
                                if contact == ContactKind::Exit {
                                    continue;
                                }
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
                            TriggerKind::Multiple | TriggerKind::Soundscape => {}
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
                    RuntimeRequest::ExternalInput {
                        entity,
                        input,
                        value,
                        caller,
                        ..
                    } => {
                        self.soundscapes.input(entity, &input);
                        let source = self.source(entity);
                        if let Some(index)=self.payload_watchers.iter().position(|watcher|watcher.identity==source) {
                            let area=caller.and_then(|caller|self.control_points.as_ref()?.areas().iter().find(|area|area.identity==self.source(caller)).map(|area|(area.identity,area.blocked)));
                            let mut events=Vec::new();
                            if let Some(points)=&self.control_points { self.payload_watchers[index].input(&input,&value,area,&self.world,points,self.world.current_tick() as f32*self.tick_interval,self.control_point_facts.in_overtime,&mut events); }
                            output.append(self.apply_payload_events(self.world.current_tick(),events)?);
                        }
                        self.particle_systems.input(&self.world, entity, &input, self.world.current_tick() as f32 * self.tick_interval);
                        self.smokestacks.input(entity, &input, &value);
                        self.ropes.input(&self.world,entity,&input,&value).map_err(|source|invalid(source as usize))?;
                        if input.eq_ignore_ascii_case(b"Width")&&let Some(beam)=self.spotlights.iter_mut().find(|beam|beam.entity==entity)&&let Some(width)=value.as_float(){beam.width=width.min(102.3);beam.end_width=beam.width;}
                        if let Some(color)=self.suns.input(&self.world,entity,&input,&value){
                            let changed=self.world.phase(self.world.current_tick(),&[WorldCommand::Input(InputRecord{target:EventTarget::Direct(entity),input:b"Color".to_vec(),value:color,activator:None,caller:Some(entity),output_action:None,producer_sequence:self.next_producer_sequence})])?;
                            self.next_producer_sequence+=1;output.append(self.consume(changed)?);
                        }
                        if let Some(change)=self.sprites.input(&self.world,entity,&input,&value,self.world.current_tick() as f32*self.tick_interval) {
                            let command=match change {
                                playsrc_entity::sprite::Change::Effects(effects)=>WorldCommand::SetRenderEffects{entity,effects},
                                playsrc_entity::sprite::Change::Color(color)=>WorldCommand::Input(InputRecord{
                                    target:EventTarget::Direct(entity),input:b"Color".to_vec(),value:Variant::Color(color),activator:None,caller:Some(entity),output_action:None,producer_sequence:self.next_producer_sequence,
                                }),
                            };
                            let changed=self.world.phase(self.world.current_tick(),&[command])?;
                            self.next_producer_sequence+=1;output.append(self.consume(changed)?);
                        }
                        let mut point_events = Vec::new();
                        if input.eq_ignore_ascii_case(b"RoundActivate")
                            && let Some(koth) = self.round_configuration.koth
                            && source == koth.identity
                            && let Some(points) = &mut self.control_points
                        {
                            koth.round_activate(points, self.world.current_tick() as f32 * self.tick_interval, self.control_point_facts, &mut point_events);
                        }
                        if input.eq_ignore_ascii_case(b"SetControlPoint") {
                            let name=match &value {Variant::String(name)=>Some(name.as_slice()),Variant::Void=>Some(b"".as_slice()),_=>None};
                            if let Some(name)=name {
                                let name=&name[..name.iter().position(|byte|*byte==0).unwrap_or(name.len()).min(254)];
                                let point=self.world.resolve(name,None,None,None).first().and_then(|handle|self.world.entity(*handle)).filter(|entity|class(&entity.definition,b"team_control_point")).map(|entity|entity.source_index as u32);
                                if let Some(points)=&mut self.control_points{points.retarget_area(source,point,self.world.current_tick() as f32*self.tick_interval,self.control_point_facts,&mut point_events);}
                            }
                        } else if let Some(points) = &mut self.control_points {
                            points.apply_input(source, &input, &value, self.world.current_tick() as f32 * self.tick_interval, self.control_point_facts, &mut point_events);
                        }
                        if !point_events.is_empty() {
                            output.append(self.emit_control_point_outputs(self.world.current_tick(), &point_events)?);
                            output.control_point_events.extend(point_events);
                        }
                        if self.world.entity(entity).is_some_and(|entity| {
                            class(&entity.definition, b"team_round_timer") || class(&entity.definition, b"tf_logic_koth") || class(&entity.definition, b"tf_gamerules") || class(&entity.definition, b"game_round_win")
                        }) {
                            self.round_inputs.push((source, input.clone(), value.clone()));
                        }
                        if input.eq_ignore_ascii_case(b"SetAnimation") {
                            if let (Some(animation), Variant::String(name)) =
                                (self.prop_animations.get_mut(&entity), &value)
                            {
                                let accepted = animation
                                    .set(
                                        name,
                                        self.world.current_tick() as f32 * self.tick_interval,
                                    )
                                    .map_err(|_| invalid(source as usize))?;
                                let acknowledged = self.world.phase(
                                    self.world.current_tick(),
                                    &[WorldCommand::DynamicPropAnimationStarted {
                                        entity,
                                        accepted,
                                    }],
                                )?;
                                output.append(self.consume(acknowledged)?);
                            }
                        }
                        if let Some(objectives) = &mut self.objectives {
                            if objectives.flag(source).is_some() {
                                if input.eq_ignore_ascii_case(b"Enable") {
                                    let _ = objectives.set_flag_disabled(source, false);
                                } else if input.eq_ignore_ascii_case(b"Disable") {
                                    let _ = objectives.set_flag_disabled(source, true);
                                }
                            } else if objectives.zones().any(|zone| zone.identity == source) {
                                if input.eq_ignore_ascii_case(b"Enable") {
                                    let _ = objectives.set_zone_disabled(source, false);
                                } else if input.eq_ignore_ascii_case(b"Disable") {
                                    let _ = objectives.set_zone_disabled(source, true);
                                }
                            }
                        }
                        if let Some(volume) = self
                            .volumes
                            .iter_mut()
                            .find(|volume| volume.source == source)
                        {
                            match &mut volume.kind {
                                VolumeKind::Regenerate { enabled, .. } => {
                                    if input.eq_ignore_ascii_case(b"Enable") {
                                        *enabled = true;
                                    } else if input.eq_ignore_ascii_case(b"Disable") {
                                        *enabled = false;
                                    } else if input.eq_ignore_ascii_case(b"Toggle") {
                                        *enabled = !*enabled;
                                    }
                                }
                                VolumeKind::RespawnRoom { enabled, .. } => {
                                    if input.eq_ignore_ascii_case(b"SetActive") {
                                        *enabled = true;
                                    } else if input.eq_ignore_ascii_case(b"SetInactive") {
                                        *enabled = false;
                                    } else if input.eq_ignore_ascii_case(b"ToggleActive") {
                                        *enabled = !*enabled;
                                    }
                                }
                                VolumeKind::Generic | VolumeKind::Soundscape { .. } => {}
                            }
                        }
                        if let Some(exclusion) = self
                            .building_exclusions
                            .iter_mut()
                            .find(|exclusion| exclusion.source == source)
                        {
                            if input.eq_ignore_ascii_case(b"SetActive") {
                                exclusion.enabled = true;
                            } else if input.eq_ignore_ascii_case(b"SetInactive") {
                                exclusion.enabled = false;
                            } else if input.eq_ignore_ascii_case(b"ToggleActive") {
                                exclusion.enabled = !exclusion.enabled;
                            }
                        }
                    }
                    RuntimeRequest::BrushState { .. } => {}
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
                Transition::Lifecycle {entity,..} => {
                    if self.world.entity(entity).is_none() {
                        let source=self.source(entity);let mut events=Vec::new();
                        for watcher in &mut self.payload_watchers{watcher.entity_removed(entity,source,&mut events);}
                        output.append(self.apply_payload_events(self.world.current_tick(),events)?);
                    }
                }
                Transition::Scheduled { .. }
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
        if let Some((&identity, _)) = self
            .actor_handles
            .iter()
            .find(|(_, actor)| **actor == handle)
        {
            return identity;
        }
        self.world
            .entity(handle)
            .and_then(|entity| u32::try_from(entity.source_index).ok())
            .unwrap_or(u32::MAX)
    }
}

fn volume_may_overlap(volume: &Volume, origin: [f32; 3], position: [f32; 3], hull: Hull) -> bool {
    bounds_may_overlap(volume.bounds, origin, position, hull)
}

fn bounds_may_overlap(
    bounds: Option<([f32; 3], [f32; 3])>,
    origin: [f32; 3],
    position: [f32; 3],
    hull: Hull,
) -> bool {
    bounds.is_none_or(|(minimum, maximum)| {
        (0..3).all(|axis| {
            position[axis] + hull.maxs[axis] >= origin[axis] + minimum[axis]
                && position[axis] + hull.mins[axis] <= origin[axis] + maximum[axis]
        })
    })
}

#[derive(Debug)]
pub enum MapError {
    Entity(RuntimeFailure),
    Movement(MoveError),
    PickupTrace(MoveError),
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

fn soundscape_output(entity: EntityHandle) -> WorldCommand {
    WorldCommand::EmitOutput { entity, output: b"OnPlay".to_vec(), value: Variant::Void,
        activator: Some(entity), caller: Some(entity), delay: 0.0 }
}

fn preserved_on_round_restart(entity: &playsrc_entity::Entity) -> bool {
    // CTeamplayRoundBasedRules / CTFGameRules round-cleanup preserve lists.
    const CLASSES: &[&[u8]] = &[
        b"player", b"viewmodel", b"worldspawn", b"soundent", b"ai_network", b"ai_hint",
        b"env_soundscape", b"env_soundscape_proxy", b"env_soundscape_triggerable", b"env_sprite",
        b"env_sun", b"env_wind", b"env_fog_controller", b"func_wall", b"func_illusionary",
        b"info_node", b"info_target", b"info_node_hint", b"point_commentary_node", b"point_viewcontrol",
        b"func_precipitation", b"func_team_wall", b"shadow_control", b"sky_camera", b"scene_manager",
        b"trigger_soundscape", b"commentary_auto", b"point_commentary_viewpoint", b"bot_roster", b"info_populator",
        b"tf_gamerules", b"tf_team_manager", b"tf_player_manager", b"tf_team", b"tf_objective_resource",
        b"keyframe_rope", b"move_rope", b"tf_viewmodel", b"tf_logic_training", b"tf_logic_training_mode",
        b"tf_powerup_bottle", b"tf_mann_vs_machine_stats", b"tf_wearable", b"tf_wearable_demoshield",
        b"tf_wearable_robot_arm", b"tf_wearable_vm", b"tf_logic_bonusround", b"vote_controller",
        b"monster_resource", b"tf_logic_medieval", b"tf_logic_cp_timer", b"tf_logic_tower_defense",
        b"tf_logic_mann_vs_machine", b"func_upgradestation", b"entity_rocket", b"entity_carrier",
        b"entity_sign", b"entity_saucer", b"tf_halloween_gift_pickup", b"tf_logic_competitive",
        b"tf_wearable_razorback", b"entity_soldier_statue",
    ];
    CLASSES.iter().any(|name| class(entity, name))
}

fn field<'a>(entity: &'a playsrc_entity::Entity, name: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_slice())
}

fn integer(entity: &playsrc_entity::Entity, name: &[u8], default: i32) -> i32 {
    field(entity, name).map_or(default, |value| {
        playsrc_keyvalues::NumericValue::Bytes(value).get_int()
    })
}

fn boolean(entity: &playsrc_entity::Entity, name: &[u8], default: bool) -> bool {
    field(entity, name).map_or(default, |value| {
        playsrc_keyvalues::NumericValue::Bytes(value).get_bool()
    })
}

fn source_team(entity: &playsrc_entity::Entity) -> Result<Option<u8>, RuntimeFailure> {
    match integer(entity, b"TeamNum", 0) {
        0 => Ok(None),
        2 => Ok(Some(2)),
        3 => Ok(Some(3)),
        _ => Err(invalid(entity.index)),
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
            let value = Some(playsrc_keyvalues::NumericValue::Bytes(value).get_float())
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
    vector_bytes(value).ok_or_else(|| invalid(entity.index))
}

fn vector_bytes(value: &[u8]) -> Option<[f32; 3]> {
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
        .filter(|values| values.len() == 3 && values.iter().all(|value| value.is_finite()))?;
    Some([values[0], values[1], values[2]])
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

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::{Error as MoveError, Trace, Tracer};

    #[test]
    fn soundscape_touch_delegates_before_base_outputs_and_retains_zone_on_round_restart() {
        let graph = playsrc_entity::parse(br#"
            {"classname" "team_control_point_master"}
            {"classname" "team_control_point" "targetname" "point"}
            {"classname" "env_soundscape_triggerable" "targetname" "zone" "soundscape" "inside" "OnPlay" "relay,Trigger,,0,-1"}
            {"classname" "trigger_soundscape" "model" "*1" "soundscape" "zone" "spawnflags" "1" "OnStartTouch" "relay,Trigger,,0,-1"}
            {"classname" "logic_relay" "targetname" "relay"}
        "#, Default::default()).unwrap();
        let mut map = MapRuntime::compile(&graph, 0.015, 1, vec![ModelBounds { model: 1, mins: [-64.0;3], maxs: [64.0;3] }]).unwrap();
        let mut registry = playsrc_audio::soundscape::Registry::default();
        registry.append(&playsrc_keyvalues::parse_text(b"inside { dsp 1 }", playsrc_keyvalues::EscapeMode::LiteralBackslash, Default::default()).unwrap().roots);
        map.initialize_soundscapes(&registry);
        let phase = map.contact_phase(&AlwaysOverlap, 1, [0.0;3], Hull { mins: [-24.0,-24.0,0.0], maxs: [24.0,24.0,82.0] }, PlayerContactFacts::default(), &[]).unwrap();
        assert_eq!(map.soundscape_selection().soundscape, 0);
        let output_names = phase.events.iter().filter(|event| event.kind == EntityEventKind::Output).map(|event| event.name.as_slice()).collect::<Vec<_>>();
        assert_eq!(output_names[0], b"OnPlay");
        assert!(output_names.iter().position(|name| *name == b"OnStartTouch").is_some_and(|index| index > 0));
        let selected = map.soundscape_selection();
        map.restart_control_point_map(2).unwrap();
        assert_eq!(map.soundscape_selection(), selected);
        assert!(map.world.entity(map.soundscapes.zones()[0].entity).is_some());
    }

    #[test]
    fn control_point_round_cleanup_recreates_doors_and_clears_old_io_without_replacing_players() {
        let graph = playsrc_entity::parse(br#"
            {"classname" "team_control_point_master"}
            {"classname" "team_control_point" "targetname" "point"}
            {"classname" "info_target" "targetname" "preserved" "origin" "1 2 3"}
            {"classname" "func_door" "targetname" "door" "model" "*1" "origin" "10 0 0" "movedir" "0 0 0" "speed" "100"}
            {"classname" "logic_relay" "targetname" "old_relay" "OnTrigger" "door,Open,,0,-1"}
            {"classname" "prop_dynamic" "targetname" "gate_model" "parentname" "door" "model" "models/gate.mdl" "origin" "12 0 0"}
        "#, playsrc_entity::Limits::default()).unwrap();
        let mut map = MapRuntime::compile(&graph, 0.015, 7, vec![ModelBounds { model: 1, mins: [-8.0;3], maxs: [8.0;3] }]).unwrap();
        let door = map.source_handle(3).unwrap();
        let preserved = map.source_handle(2).unwrap();
        let player = map.player;
        map.world.phase(1, &[WorldCommand::SetWorldTransform { entity: door, transform: Transform { origin: [74.0,0.0,0.0], angles: [0.0;3] } }]).unwrap();
        map.world.phase(1, &[WorldCommand::QueueInput { input: InputRecord { target: EventTarget::Direct(map.source_handle(4).unwrap()), input: b"Trigger".to_vec(), value: Variant::Void, activator: Some(player), caller: Some(player), output_action: None, producer_sequence: 1 }, delay: 5.0 }]).unwrap();
        map.restart_control_point_map(2).unwrap();
        assert_ne!(map.source_handle(3), Some(door));
        assert_eq!(map.source_handle(2), Some(preserved));
        assert_eq!(map.player, player);
        assert_eq!(map.world.entity(map.source_handle(3).unwrap()).unwrap().world_transform.origin, [10.0,0.0,0.0]);
        let child = map.world.entity(map.source_handle(5).unwrap()).unwrap();
        assert_eq!(child.parent, map.source_handle(3), "round recreation must resolve the authored hierarchy again");
        assert_eq!(child.world_transform.origin, [12.0,0.0,0.0]);
        map.world.phase(3, &[WorldCommand::SetWorldTransform { entity: map.source_handle(3).unwrap(), transform: Transform { origin: [74.0,0.0,0.0], angles: [0.0;3] } }]).unwrap();
        assert_eq!(map.entity_world_transform(5).unwrap().origin, [76.0,0.0,0.0]);
        map.restart_control_point_map(4).unwrap();
        assert_eq!(map.world.entity(map.source_handle(5).unwrap()).unwrap().parent, map.source_handle(3));
        assert_eq!(map.entity_world_transform(5).unwrap().origin, [12.0,0.0,0.0]);
        let later = map.world.phase(600, &[]).unwrap();
        assert!(!later.records.iter().any(|record| matches!(&record.transition, Transition::Output { output, .. } if output == b"OnTrigger")));
    }

    #[test]
    fn repeated_round_broadcasts_ignore_non_subscribers_without_spending_direct_io_diagnostics() {
        let mut text = String::from(r#"{"classname" "team_control_point_master"}{"classname" "team_control_point" "targetname" "point"}"#);
        for _ in 0..200 { text += r#"{"classname" "logic_relay"}"#; }
        let graph = playsrc_entity::parse(text.as_bytes(), Default::default()).unwrap();
        let mut map = MapRuntime::compile(&graph, 0.015, 42, Vec::new()).unwrap();
        let mut random = crate::UniformRandomStream::from_seed(1).unwrap();
        for tick in 1..100 {
            map.activate_control_point_round(tick, crate::control_point::Facts::default(), &mut random).unwrap();
        }
    }

    #[derive(Clone)]
    struct AlwaysOverlap;

    impl Tracer for AlwaysOverlap {
        fn trace(
            &self,
            _start: [f32; 3],
            end: [f32; 3],
            _hull: Hull,
            _mask: u32,
        ) -> Result<Trace, MoveError> {
            Ok(Trace {
                fraction: 1.0,
                start_solid: false,
                all_solid: false,
                end,
                normal: None,
                hit: None,
                contents: 0,
            })
        }
    }

    impl GameplayWorld for AlwaysOverlap {
        fn overlaps_model_hull(
            &self,
            _model: usize,
            _origin: [f32; 3],
            _position: [f32; 3],
            _hull: Hull,
        ) -> Result<bool, MoveError> {
            Ok(true)
        }
    }

    #[test]
    fn authored_no_build_zones_preserve_team_object_points_and_disabled_input_edges() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "func_nobuild" "targetname" "red_sentry" "model" "*1" "TeamNum" "2" "AllowDispenser" "1" "AllowTeleporters" "1"}
{"classname" "func_nobuild" "targetname" "disabled" "model" "*2" "StartDisabled" "1"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![
                ModelBounds {
                    model: 1,
                    mins: [0.0, 0.0, 0.0],
                    maxs: [100.0, 100.0, 32.0],
                },
                ModelBounds {
                    model: 2,
                    mins: [200.0, 0.0, 0.0],
                    maxs: [300.0, 100.0, 100.0],
                },
            ],
        )
        .unwrap();
        let collision = AlwaysOverlap;
        let sentry = crate::building::Object::SENTRY;
        assert!(
            !map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [50.0, 50.0, 0.0]
            )
            .unwrap()
        );
        assert!(
            map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Blue,
                [50.0, 50.0, 0.0]
            )
            .unwrap()
        );
        for object in [
            crate::building::Object::DISPENSER,
            crate::building::Object::ENTRANCE,
            crate::building::Object::EXIT,
        ] {
            assert!(
                map.building_position_allowed(
                    &collision,
                    object,
                    crate::PlayerTeam::Red,
                    [50.0, 50.0, 0.0]
                )
                .unwrap()
            );
        }
        assert!(
            map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [100.01, 50.0, 0.0]
            )
            .unwrap()
        );
        assert!(
            !map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [50.0, 50.0, -33.0]
            )
            .unwrap()
        );
        assert!(
            map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [250.0, 50.0, 0.0]
            )
            .unwrap()
        );

        let disabled = graph.entities[1].index as u32;
        map.input(1, disabled, b"SetActive", Variant::Void).unwrap();
        assert!(
            !map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [250.0, 50.0, 0.0]
            )
            .unwrap()
        );
        map.input(2, disabled, b"ToggleActive", Variant::Void)
            .unwrap();
        assert!(
            map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [250.0, 50.0, 0.0]
            )
            .unwrap()
        );
        map.input(3, disabled, b"SetActive", Variant::Void).unwrap();
        map.input(4, disabled, b"SetInactive", Variant::Void)
            .unwrap();
        assert!(
            map.building_position_allowed(
                &collision,
                sentry,
                crate::PlayerTeam::Red,
                [250.0, 50.0, 0.0]
            )
            .unwrap()
        );
    }

    #[test]
    fn active_respawn_rooms_reject_both_build_points_for_either_team_only_inside_bounds() {
        let graph = playsrc_entity::parse(
            br#"{"classname" "func_respawnroom" "targetname" "red_spawn" "model" "*1" "TeamNum" "2"}"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [10.0, 10.0, 20.0],
                maxs: [90.0, 90.0, 30.0],
            }],
        )
        .unwrap();
        for team in [crate::PlayerTeam::Red, crate::PlayerTeam::Blue] {
            assert!(
                !map.building_position_allowed(
                    &AlwaysOverlap,
                    crate::building::Object::SENTRY,
                    team,
                    [50.0, 50.0, -8.0],
                )
                .unwrap()
            );
            assert!(
                map.building_position_allowed(
                    &AlwaysOverlap,
                    crate::building::Object::SENTRY,
                    team,
                    [90.01, 50.0, 20.0],
                )
                .unwrap()
            );
        }
        map.input(
            1,
            graph.entities[0].index as u32,
            b"SetInactive",
            Variant::Void,
        )
        .unwrap();
        assert!(
            map.building_position_allowed(
                &AlwaysOverlap,
                crate::building::Object::SENTRY,
                crate::PlayerTeam::Red,
                [50.0, 50.0, 20.0],
            )
            .unwrap()
        );
    }

    #[test]
    #[ignore = "requires playsrc.local.json and the exact configured ctf_2fort and pl_upward BSPs"]
    fn configured_build_regions_preserve_exact_authored_brush_boundaries_and_disabled_zones() {
        struct AuthoredBrushes(playsrc_collision::World);

        impl Tracer for AuthoredBrushes {
            fn trace(
                &self,
                _start: [f32; 3],
                end: [f32; 3],
                _hull: Hull,
                _mask: u32,
            ) -> Result<Trace, MoveError> {
                Ok(Trace {
                    fraction: 1.0,
                    start_solid: false,
                    all_solid: false,
                    end,
                    normal: None,
                    hit: None,
                    contents: 0,
                })
            }
        }

        impl GameplayWorld for AuthoredBrushes {
            fn overlaps_model_hull(
                &self,
                model: usize,
                origin: [f32; 3],
                position: [f32; 3],
                hull: Hull,
            ) -> Result<bool, MoveError> {
                self.0
                    .overlaps_model_hull(model, origin, position, hull)
                    .map_err(|_| {
                        MoveError::new(
                            playsrc_movement::Operation::Trace,
                            playsrc_movement::FailureKind::Malformed,
                            "authored no-build brush",
                        )
                    })
            }
        }

        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let config = std::fs::read_to_string(root.join("playsrc.local.json")).unwrap();
        let marker = "\"tf2Dir\"";
        let value = &config[config.find(marker).unwrap() + marker.len()..];
        let value = value[value.find(':').unwrap() + 1..].trim_start();
        let tf2 = std::path::PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);

        for identity in ["ctf_2fort", "pl_upward"] {
            let bytes = std::fs::read(tf2.join("maps").join(format!("{identity}.bsp"))).unwrap();
            let bsp = playsrc_bsp::parse(
                &bytes,
                playsrc_bsp::Profile::Source2013V20,
                playsrc_bsp::Limits::default(),
            )
            .unwrap();
            let graph = playsrc_entity::parse(
                bsp.lump(0).unwrap().bytes(&bsp),
                playsrc_entity::Limits::default(),
            )
            .unwrap();
            let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else {
                panic!("configured model lump is absent");
            };
            let bounds = models
                .iter()
                .enumerate()
                .map(|(model, value)| ModelBounds {
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
            let mut map = MapRuntime::compile(&graph, 0.015, 1, bounds).unwrap();
            let collision = AuthoredBrushes(playsrc_collision::compile(&bsp).unwrap());
            let (prohibited, valid) = if identity == "ctf_2fort" {
                assert_eq!(map.building_exclusions.len(), 2);
                ([900.0, 1400.0, 280.0], [880.0, 1400.0, 280.0])
            } else {
                assert_eq!(map.building_exclusions.len(), 16);
                ([760.0, 540.0, 600.0], [800.0, 447.0, 592.0])
            };
            for team in [crate::PlayerTeam::Red, crate::PlayerTeam::Blue] {
                for object in crate::building::Object::MENU {
                    assert!(
                        !map.building_position_allowed(&collision, object, team, prohibited)
                            .unwrap(),
                        "{identity}: {team:?} {object:?} prohibited"
                    );
                    assert!(
                        map.building_position_allowed(&collision, object, team, valid)
                            .unwrap(),
                        "{identity}: {team:?} {object:?} valid"
                    );
                }
            }
            if identity == "pl_upward" {
                let disabled = graph
                    .entities
                    .iter()
                    .find(|entity| {
                        entity.targetname.as_deref() == Some(b"func_nobuild_exitC4".as_slice())
                    })
                    .unwrap()
                    .index as u32;
                let point = [-220.0, -60.0, 750.0];
                assert!(
                    map.building_position_allowed(
                        &collision,
                        crate::building::Object::SENTRY,
                        crate::PlayerTeam::Red,
                        point,
                    )
                    .unwrap()
                );
                map.input(1, disabled, b"SetActive", Variant::Void).unwrap();
                assert!(
                    !map.building_position_allowed(
                        &collision,
                        crate::building::Object::SENTRY,
                        crate::PlayerTeam::Red,
                        point,
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn tf_filter_matrix_applies_team_win_negation_class_observer_and_conditions() {
        let red_soldier = PlayerContactFacts {
            team: 2,
            class: 3,
            observer: false,
            conditions: [1 << 22, 0, 0, 0, 0],
            winning_team: None,
        };
        assert!(
            GameFilter::Team {
                team: 2,
                negated: false
            }
            .passes(red_soldier)
        );
        assert!(
            !GameFilter::Team {
                team: 3,
                negated: false
            }
            .passes(red_soldier)
        );
        assert!(
            GameFilter::Class {
                class: 3,
                negated: false
            }
            .passes(red_soldier)
        );
        assert!(
            GameFilter::Condition {
                condition: 22,
                negated: false
            }
            .passes(red_soldier)
        );

        let observer = PlayerContactFacts {
            observer: true,
            ..red_soldier
        };
        assert!(
            !GameFilter::Class {
                class: 3,
                negated: false
            }
            .passes(observer)
        );

        let winner = PlayerContactFacts {
            winning_team: Some(2),
            ..red_soldier
        };
        assert!(
            GameFilter::Team {
                team: 3,
                negated: true
            }
            .passes(winner)
        );
    }

    #[test]
    fn tf_filter_uses_source_numeric_prefix_accessors_without_rewriting_entity_values() {
        let raw = b"Allow entities that match criteria";
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"filter_activator_tfteam\"\"targetname\"\"filter_team_blue\"\"TeamNum\"\"3 blue\"\"Negated\"\"Allow entities that match criteria\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        assert_eq!(field(&graph.entities[0], b"Negated"), Some(raw.as_slice()));

        let runtime = MapRuntime::compile(&graph, 0.015, 1, Vec::new()).unwrap();
        assert_eq!(
            runtime.game_filters.get(b"filter_team_blue".as_slice()),
            Some(&GameFilter::Team {
                team: 3,
                negated: false,
            })
        );
        assert_eq!(field(&graph.entities[0], b"Negated"), Some(raw.as_slice()));
    }

    #[test]
    fn living_bots_open_only_their_authored_team_door_and_close_contacts_on_death() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "filter_activator_tfteam" "targetname" "red_only" "TeamNum" "2"}
{"classname" "trigger_multiple" "model" "*1" "spawnflags" "1" "filtername" "red_only" "OnStartTouch" "spawn_door,Open,,0,-1" "OnEndTouchAll" "spawn_door,Close,,0,-1"}
{"classname" "func_door" "targetname" "spawn_door" "model" "*2" "movedir" "0 0 0" "speed" "100" "wait" "-1"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![
                ModelBounds {
                    model: 1,
                    mins: [-64.0; 3],
                    maxs: [64.0; 3],
                },
                ModelBounds {
                    model: 2,
                    mins: [-8.0; 3],
                    maxs: [8.0; 3],
                },
            ],
        )
        .unwrap();
        let actor = |identity, team, alive: bool| ActorContact {
            identity,
            position: [0.0; 3],
            hull: Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
            facts: PlayerContactFacts {
                team,
                class: 3,
                observer: !alive,
                ..PlayerContactFacts::default()
            },
            alive,
        };

        let run = |map: &mut MapRuntime, tick, actors: &[ActorContact]| {
            map.contact_phase(
                &AlwaysOverlap,
                tick,
                [0.0; 3],
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                PlayerContactFacts::default(),
                actors,
            )
            .unwrap()
        };
        let rejected = run(&mut map, 0, &[actor(2, 3, true)]);
        assert!(rejected.mover_requests.is_empty());
        assert!(rejected.events.iter().any(|event| {
            event.kind == EntityEventKind::Contact && event.subject == Some(2) && !event.accepted
        }));

        let opened = run(&mut map, 1, &[actor(2, 3, true), actor(3, 2, true)]);
        assert!(opened.events.iter().any(|event| {
            event.kind == EntityEventKind::Contact
                && event.subject == Some(3)
                && event.contact == Some(ContactKind::Enter)
                && event.accepted
        }));
        assert!(opened.mover_requests.iter().any(|request| request.opening));
        assert!(opened.contacts.is_empty());

        let closed = run(&mut map, 2, &[actor(2, 3, true), actor(3, 2, false)]);
        assert!(closed.events.iter().any(|event| {
            event.kind == EntityEventKind::Contact
                && event.subject == Some(3)
                && event.contact == Some(ContactKind::Exit)
        }));
    }

    #[test]
    fn tracktrain_requests_preserve_authoritative_linear_and_angular_trajectory() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "path_track" "targetname" "first" "target" "corner" "origin" "0 0 0"}
{"classname" "path_track" "targetname" "corner" "target" "end" "origin" "8 0 0"}
{"classname" "path_track" "targetname" "end" "origin" "8 128 0"}
{"classname" "func_tracktrain" "targetname" "cart" "target" "first" "model" "*1"
 "wheels" "4" "startspeed" "90" "velocitytype" "1" "orientationtype" "1"
 "ManualSpeedChanges" "1" "ManualAccelSpeed" "70" "ManualDecelSpeed" "150"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [-8.0; 3],
                maxs: [8.0; 3],
            }],
        )
        .unwrap();
        let phase = map.input(0, 3, b"StartForward", Variant::Void).unwrap();
        let request = phase.mover_requests[0];
        assert_eq!(request.start, [0.0; 3]);
        assert_eq!(request.start_angles, [0.0; 3]);
        assert!(request.destination[0] > 0.0);
        assert_eq!(request.speed, 90.0);
        assert!(request.destination_angles[1] >= 0.0);
        assert_eq!(
            request.angular_velocity[1],
            request.destination_angles[1] / 0.015
        );
    }

    #[test]
    fn authored_physics_cart_constraint_is_reported_without_inventing_parenting() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "path_track" "targetname" "first" "target" "second" "origin" "0 0 0"}
{"classname" "path_track" "targetname" "second" "origin" "100 0 0"}
{"classname" "func_tracktrain" "targetname" "cart_train" "target" "first" "model" "*1" "startspeed" "90"}
{"classname" "prop_physics_override" "targetname" "cart_model" "model" "models/props_trainyard/bomb_cart.mdl"}
{"classname" "phys_constraint" "targetname" "cart_constraint" "attach1" "cart_train" "attach2" "cart_model"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [-8.0; 3],
                maxs: [8.0; 3],
            }],
        )
        .unwrap();
        assert!(map.payload_constraint_blocked());
        let prop = map.source_handle(3).unwrap();
        assert_eq!(map.world.entity(prop).unwrap().parent, None);
        assert!(!MapRuntime::empty(0.015).payload_constraint_blocked());
    }

    #[test]
    fn landmark_teleport_preserves_angles_and_applies_relative_offset() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"info_target\"\"targetname\"\"land\"\"origin\"\"10 0 0\"}{\"classname\"\"info_teleport_destination\"\"targetname\"\"dest\"\"origin\"\"100 5 6\"\"angles\"\"1 2 3\"}{\"classname\"\"trigger_teleport\"\"model\"\"*1\"\"target\"\"dest\"\"landmark\"\"land\"\"spawnflags\"\"1\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [-8.0; 3],
                maxs: [8.0; 3],
            }],
        )
        .unwrap();
        let phase = map
            .contact_phase(
                &AlwaysOverlap,
                0,
                [20.0, 4.0, 2.0],
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                PlayerContactFacts::default(),
                &[],
            )
            .unwrap();
        assert!(phase.effects.iter().any(|effect| matches!(
            effect,
            Effect::Teleport {
                position: [110.0, 9.0, 8.0],
                angles: None,
                ..
            }
        )));
    }

    #[test]
    fn respawn_barrier_policy_uses_team_contents_and_opens_on_team_win() {
        assert!(respawn_barrier_collides(
            Some(2),
            true,
            CONTENTS_RED_TEAM,
            false
        ));
        assert!(!respawn_barrier_collides(
            Some(2),
            true,
            CONTENTS_BLUE_TEAM,
            false
        ));
        assert!(!respawn_barrier_collides(
            Some(2),
            true,
            CONTENTS_RED_TEAM,
            true
        ));
        assert!(!respawn_barrier_collides(None, true, u32::MAX, false));
        assert!(!respawn_barrier_collides(
            Some(3),
            false,
            CONTENTS_BLUE_TEAM,
            false
        ));
    }

    #[test]
    fn regenerate_association_uses_only_the_first_named_dynamic_prop() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"info_target\"\"targetname\"\"locker\"}{\"classname\"\"prop_dynamic\"\"targetname\"\"locker\"\"SetBodyGroup\"\"3\"}{\"classname\"\"func_regenerate\"\"model\"\"*1\"\"associatedmodel\"\"locker\"}",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            5,
            vec![ModelBounds {
                model: 1,
                mins: [-8.0; 3],
                maxs: [8.0; 3],
            }],
        )
        .unwrap();
        let phase = map
            .contact_phase(
                &AlwaysOverlap,
                0,
                [0.0; 3],
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                PlayerContactFacts::default(),
                &[],
            )
            .unwrap();
        assert!(matches!(
            phase.effects.as_slice(),
            [Effect::Regenerate {
                associated_model: None,
                ..
            }]
        ));
        assert_eq!(map.regenerate_associated_body(2), None);
    }

    #[test]
    fn presentation_delegates_complete_entity_state_and_survives_restore_and_rollback() {
        let graph = playsrc_entity::parse(
            br#"
{"classname" "func_brush" "targetname" "carrier" "model" "*1" "origin" "10 0 0"}
{"classname" "func_movelinear" "targetname" "child" "model" "*2" "parentname" "carrier"
 "origin" "15 0 0" "angles" "0 90 0" "MoveDistance" "10" "StartPosition" "0"
 "rendermode" "2" "rendercolor" "10 20 30 99" "renderamt" "128" "renderfx" "3" "effects" "64"}
{"classname" "func_brush" "targetname" "hidden" "model" "*3" "StartDisabled" "1"}
{"classname" "func_regenerate" "targetname" "resupply" "model" "*4"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            0x1234,
            (1..=4)
                .map(|model| ModelBounds {
                    model,
                    mins: [0.0; 3],
                    maxs: [12.0, 4.0, 4.0],
                })
                .collect(),
        )
        .unwrap();

        let revision = map.entity_revision();
        let initial = map.brush_model_presentation(revision).unwrap();
        assert_eq!(initial.source_identity, 0x1234);
        assert_eq!(initial.registry_identity, 0x5446_325f_454e_5434);
        assert_eq!(initial.tick, 0);
        assert_eq!(initial.revision, revision);
        assert_eq!(
            initial
                .models
                .iter()
                .map(|model| model.source_index)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let carrier = &initial.models[0];
        let child = &initial.models[1];
        assert!(carrier.draw);
        assert_eq!(child.model, 2);
        assert_eq!(child.local_transform.origin, [5.0, 0.0, 0.0]);
        assert_eq!(child.world_transform.origin, [15.0, 0.0, 0.0]);
        assert_eq!(child.parent, Some(carrier.handle));
        assert_eq!(child.render_mode, 2);
        assert_eq!(child.color, [10, 20, 30, 128]);
        assert_eq!(child.render_fx, 3);
        assert_eq!(child.effects, 64);
        assert!(child.draw);
        assert!(child.mover.is_some());
        assert!(!initial.models[2].draw);
        assert!(!initial.models[3].draw);

        let snapshot = map.world.snapshot().unwrap();
        let child_handle = map.source_handle(1).unwrap();
        map.world
            .phase(
                1,
                &[WorldCommand::SetBrushModel {
                    entity: child_handle,
                    model: Some(3),
                }],
            )
            .unwrap();
        assert_eq!(
            map.brush_model_presentation(map.entity_revision())
                .unwrap()
                .models[1]
                .model,
            3
        );
        map.world.restore(&snapshot).unwrap();
        assert_eq!(map.brush_model_presentation(revision).unwrap(), initial);

        let failed = map.world.phase(
            1,
            &[
                WorldCommand::SetWorldTransform {
                    entity: child_handle,
                    transform: Transform {
                        origin: [7.0, 0.0, 0.0],
                        angles: [0.0; 3],
                    },
                },
                WorldCommand::SetBrushModel {
                    entity: child_handle,
                    model: Some(999),
                },
            ],
        );
        assert_eq!(
            failed.unwrap_err().code,
            playsrc_entity::RuntimeFailureCode::InvalidField
        );
        assert_eq!(map.entity_revision(), revision);
        assert_eq!(map.brush_model_presentation(revision).unwrap(), initial);
    }

    #[test]
    fn presentation_tracks_mover_progress_block_reversal_and_completion() {
        let graph = playsrc_entity::parse(
            br#"{"classname" "func_door" "targetname" "door" "model" "*1" "movedir" "0 0 0" "wait" "1"}
            {"classname" "prop_dynamic" "parentname" "door" "origin" "1 2 3" "model" "models/door.mdl"}"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut map = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [0.0; 3],
                maxs: [12.0, 2.0, 2.0],
            }],
        )
        .unwrap();
        let opened = map.input(0, 0, b"Open", Variant::Void).unwrap();
        let request = opened.mover_requests[0];
        let moving = map.brush_model_presentation(map.entity_revision()).unwrap();
        assert_eq!(
            moving.models[0].mover.as_ref().unwrap().request_id,
            Some(request.request_id)
        );
        assert_eq!(moving.models[0].mover.as_ref().unwrap().opening, Some(true));

        map.apply_mover_results(
            1,
            &[MoverResult {
                request_id: request.request_id,
                entity: 0,
                kind: MoverResultKind::Progress,
                transform: Transform {
                    origin: [5.0, 0.0, 0.0],
                    angles: [0.0; 3],
                },
                carry: [0.0; 3],
            }],
        )
        .unwrap();
        let progress = map.brush_model_presentation(map.entity_revision()).unwrap();
        let mover = progress.models[0].mover.as_ref().unwrap();
        assert_eq!(f32::from_bits(mover.progress_bits), 0.5);
        assert_eq!(progress.models[0].world_transform.origin, [5.0, 0.0, 0.0]);
        assert_eq!(
            map.studio_model_presentation(map.entity_revision())
                .unwrap()[0]
                .world_transform
                .origin,
            [6.0, 2.0, 3.0]
        );

        let blocked = map
            .apply_mover_results(
                2,
                &[MoverResult {
                    request_id: request.request_id,
                    entity: 0,
                    kind: MoverResultKind::BlockedStay,
                    transform: Transform {
                        origin: [5.0, 0.0, 0.0],
                        angles: [0.0; 3],
                    },
                    carry: [0.0; 3],
                }],
            )
            .unwrap();
        let reversal = blocked.mover_requests[0];
        let reversing = map.brush_model_presentation(map.entity_revision()).unwrap();
        let mover = reversing.models[0].mover.as_ref().unwrap();
        assert_eq!(mover.request_id, Some(reversal.request_id));
        assert_eq!(mover.opening, Some(false));
        assert_eq!(f32::from_bits(mover.progress_bits), 0.5);

        map.apply_mover_results(
            3,
            &[MoverResult {
                request_id: reversal.request_id,
                entity: 0,
                kind: MoverResultKind::Completed,
                transform: Transform::IDENTITY,
                carry: [0.0; 3],
            }],
        )
        .unwrap();
        let completed = map.brush_model_presentation(map.entity_revision()).unwrap();
        let mover = completed.models[0].mover.as_ref().unwrap();
        assert_eq!(mover.request_id, None);
        assert_eq!(mover.position, playsrc_entity::MoverPosition::Closed);
        assert_eq!(f32::from_bits(mover.progress_bits), 0.0);
        assert_eq!(
            map.studio_model_presentation(map.entity_revision())
                .unwrap()[0]
                .world_transform
                .origin,
            [1.0, 2.0, 3.0]
        );
        let retriggered = map.input(4, 0, b"Open", Variant::Void).unwrap();
        assert_ne!(retriggered.mover_requests[0].request_id, request.request_id);
    }

    #[test]
    fn selected_game_entity_limit_rejects_before_a_partial_presentation_exists() {
        let mut bytes = Vec::new();
        for _ in 0..playsrc_entity::Limits::default().max_entities {
            bytes.extend_from_slice(br#"{"classname" "func_brush" "model" "*1"}"#);
        }
        let graph = playsrc_entity::parse(&bytes, playsrc_entity::Limits::default()).unwrap();
        let failure = MapRuntime::compile(
            &graph,
            0.015,
            1,
            vec![ModelBounds {
                model: 1,
                mins: [0.0; 3],
                maxs: [1.0; 3],
            }],
        )
        .unwrap_err();
        assert_eq!(
            failure.code,
            playsrc_entity::RuntimeFailureCode::EntityLimit
        );
    }
}
