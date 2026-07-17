use std::collections::BTreeMap;

use super::{Connection, ConnectionError, Entity, Graph};

const MAX_LIVE_ENTITIES: usize = 8_192;
const BUTTON_TOGGLE: i32 = 32;
const BUTTON_LOCKED: i32 = 2_048;
const DOOR_START_OPEN: i32 = 1;
const DOOR_NO_AUTO_RETURN: i32 = 32;
const DOOR_LOCKED: i32 = 2_048;
const RELAY_REMOVE_ON_FIRE: i32 = 1;
const RELAY_FAST_RETRIGGER: i32 = 2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EntityHandle {
    pub slot: u16,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MapLoadKind {
    NewGame,
    LoadGame,
    Transition,
    Background,
    MultiplayerNewMap,
    MultiplayerNewRound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub max_entities: usize,
    pub max_hierarchy_depth: usize,
    pub max_output_actions: usize,
    pub max_queued_events: usize,
    pub max_events_per_tick: usize,
    pub max_transitions_per_phase: usize,
    pub max_trigger_contacts: usize,
    pub max_snapshot_bytes: usize,
    pub max_diagnostics: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_entities: MAX_LIVE_ENTITIES,
            max_hierarchy_depth: 64,
            max_output_actions: 262_144,
            max_queued_events: 65_536,
            max_events_per_tick: 16_384,
            max_transitions_per_phase: 131_072,
            max_trigger_contacts: 65_536,
            max_snapshot_bytes: 64 * 1024 * 1024,
            max_diagnostics: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelBounds {
    pub model: usize,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntityWorldConfig {
    pub tick_interval: f32,
    pub source_identity: u64,
    pub registry_identity: u64,
    pub load_kind: MapLoadKind,
    pub random_seed: u64,
    pub model_bounds: Vec<ModelBounds>,
    pub limits: RuntimeLimits,
}

impl Default for EntityWorldConfig {
    fn default() -> Self {
        Self {
            tick_interval: 0.015,
            source_identity: 0,
            registry_identity: 0,
            load_kind: MapLoadKind::NewGame,
            random_seed: 0x243f_6a88_85a3_08d3,
            model_bounds: Vec::new(),
            limits: RuntimeLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    Handled,
    IntentionallyInert,
    Unsupported,
    Unknown,
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Created,
    Spawned,
    Activated,
    PendingRemoval,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
}

impl Transform {
    pub const IDENTITY: Self = Self {
        origin: [0.0; 3],
        angles: [0.0; 3],
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Variant {
    Void,
    Bool(bool),
    Integer(i32),
    Float(u32),
    String(Vec<u8>),
    Vector([u32; 3]),
    Handle(EntityHandle),
}

impl Variant {
    pub fn float(value: f32) -> Self {
        Self::Float(value.to_bits())
    }

    pub fn vector(value: [f32; 3]) -> Self {
        Self::Vector(value.map(f32::to_bits))
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Integer(value) => Some(*value as f32),
            Self::Float(bits) => Some(f32::from_bits(*bits)),
            Self::Bool(value) => Some(i32::from(*value) as f32),
            Self::String(value) => parse_f32(value),
            _ => None,
        }
    }

    fn as_string(&self) -> Vec<u8> {
        match self {
            Self::Void => Vec::new(),
            Self::Bool(value) => if *value { b"1" } else { b"0" }.to_vec(),
            Self::Integer(value) => value.to_string().into_bytes(),
            Self::Float(bits) => f32::from_bits(*bits).to_string().into_bytes(),
            Self::String(value) => value.clone(),
            Self::Vector(bits) => format!(
                "{} {} {}",
                f32::from_bits(bits[0]),
                f32::from_bits(bits[1]),
                f32::from_bits(bits[2])
            )
            .into_bytes(),
            Self::Handle(handle) => format!("{}:{}", handle.slot, handle.generation).into_bytes(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputActionState {
    pub id: u64,
    pub declaration_order: usize,
    pub output: Vec<u8>,
    pub target: Vec<u8>,
    pub input: Vec<u8>,
    pub parameter: Vec<u8>,
    pub delay_bits: u32,
    pub remaining_fires: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoverKind {
    Button,
    Door,
    Linear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoverPosition {
    Closed,
    Opening,
    Open,
    Closing,
    Positioned(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingMove {
    pub request_id: u64,
    pub local_destination: [f32; 3],
    pub world_destination: [f32; 3],
    pub opening: bool,
    pub activator: Option<EntityHandle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoverState {
    pub kind: MoverKind,
    pub closed: [f32; 3],
    pub open: [f32; 3],
    pub speed: f32,
    pub wait_ticks: Option<u64>,
    pub position: MoverPosition,
    pub pending: Option<PendingMove>,
    pub locked: bool,
    pub toggle: bool,
    pub force_closed: bool,
    pub no_auto_return: bool,
    pub outputs_reversed: bool,
    pub block_damage_bits: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrushSolidity {
    Toggle,
    Never,
    Always,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrushState {
    pub enabled: bool,
    pub solidity: BrushSolidity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayState {
    pub enabled: bool,
    pub waiting_for_refire: bool,
    pub remove_on_fire: bool,
    pub fast_retrigger: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterState {
    pub value_bits: u32,
    pub min_bits: u32,
    pub max_bits: u32,
    pub hit_min: bool,
    pub hit_max: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseState {
    pub cases: Vec<Option<Vec<u8>>>,
    pub shuffle: Vec<usize>,
    pub last_shuffle: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterPredicate {
    Name(Vec<u8>),
    Class(Vec<u8>),
    All(Vec<Vec<u8>>),
    Any(Vec<Vec<u8>>),
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterState {
    pub negated: bool,
    pub predicate: FilterPredicate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerKind {
    Multiple,
    Hurt,
    Push,
    Catapult,
    Teleport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerState {
    pub kind: TriggerKind,
    pub enabled: bool,
    pub filter_name: Option<Vec<u8>>,
    pub contacts: Vec<EntityHandle>,
    pub wait_ticks: u64,
    pub next_fire_tick: u64,
    pub mutable_value_bits: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BehaviorState {
    Inert,
    Brush(BrushState),
    Mover(MoverState),
    LogicAuto,
    Relay(RelayState),
    Counter(CounterState),
    Case(CaseState),
    Filter(FilterState),
    Trigger(TriggerState),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEntity {
    pub handle: EntityHandle,
    pub source_index: usize,
    pub classname: Vec<u8>,
    pub targetname: Option<Vec<u8>>,
    pub lifecycle: Lifecycle,
    pub coverage: Coverage,
    pub local_transform: Transform,
    pub world_transform: Transform,
    pub parent: Option<EntityHandle>,
    pub parent_attachment: Option<Vec<u8>>,
    pub children: Vec<EntityHandle>,
    pub attachments: BTreeMap<Vec<u8>, Transform>,
    pub outputs: Vec<OutputActionState>,
    pub malformed_outputs: Vec<(usize, ConnectionError)>,
    pub definition: Entity,
    pub behavior: BehaviorState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventTarget {
    Expression(Vec<u8>),
    Direct(EntityHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputRecord {
    pub target: EventTarget,
    pub input: Vec<u8>,
    pub value: Variant,
    pub activator: Option<EntityHandle>,
    pub caller: Option<EntityHandle>,
    pub output_action: Option<u64>,
    pub producer_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactKind {
    Enter,
    Stay,
    Exit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactRecord {
    pub trigger: EntityHandle,
    pub subject: EntityHandle,
    pub kind: ContactKind,
    pub external_filter_result: Option<bool>,
    pub producer_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentMode {
    MaintainWorld,
    SnapToAttachment,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParentRequest {
    pub child: EntityHandle,
    pub parent: Option<EntityHandle>,
    pub attachment: Option<Vec<u8>>,
    pub mode: ParentMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockContactKind {
    Start,
    Stay,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldCommand {
    Spawn(Entity),
    Input(InputRecord),
    Contact(ContactRecord),
    Remove(EntityHandle),
    SetParent(ParentRequest),
    SetWorldTransform {
        entity: EntityHandle,
        transform: Transform,
    },
    SetAttachmentTransform {
        parent: EntityHandle,
        attachment: Vec<u8>,
        parent_space_transform: Transform,
    },
    MoverCompleted {
        entity: EntityHandle,
        request_id: u64,
    },
    MoverBlocked {
        entity: EntityHandle,
        request_id: u64,
        blocker: EntityHandle,
        kind: BlockContactKind,
    },
    CancelCaller(EntityHandle),
    CancelDirectInput {
        target: EntityHandle,
        input_prefix: Vec<u8>,
    },
    EmitOutput {
        entity: EntityHandle,
        output: Vec<u8>,
        value: Variant,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
        delay: f32,
    },
    SetTargetname {
        entity: EntityHandle,
        targetname: Option<Vec<u8>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticCode {
    MissingTarget,
    StaleHandle,
    UnknownInput,
    BadParameter,
    MissingParent,
    AmbiguousParent,
    ParentCycle,
    MissingAttachment,
    FilterRejected,
    ExternalFilterRequired,
    DuplicateContact,
    MissingContact,
    MoverRequestMismatch,
    UnsupportedSelector,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeRequest {
    Mover {
        request_id: u64,
        entity: EntityHandle,
        kind: MoverKind,
        local_destination: [f32; 3],
        world_destination: [f32; 3],
        speed: f32,
        opening: bool,
    },
    BrushState {
        entity: EntityHandle,
        enabled: bool,
        solid: bool,
    },
    TriggerEffect {
        trigger: EntityHandle,
        subject: EntityHandle,
        kind: TriggerKind,
        contact: ContactKind,
    },
    BlockDamage {
        mover: EntityHandle,
        blocker: EntityHandle,
        damage_bits: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Transition {
    Lifecycle {
        entity: EntityHandle,
        state: Lifecycle,
    },
    Input {
        target: EntityHandle,
        input: Vec<u8>,
        accepted: bool,
        producer_sequence: u64,
    },
    Output {
        caller: EntityHandle,
        output: Vec<u8>,
        action_id: u64,
    },
    Scheduled {
        event_id: u64,
        due_tick: u64,
    },
    Cancelled {
        event_id: u64,
    },
    ParentChanged {
        child: EntityHandle,
        parent: Option<EntityHandle>,
        attachment: Option<Vec<u8>>,
    },
    TransformChanged {
        entity: EntityHandle,
        local: Transform,
        world: Transform,
    },
    Contact {
        trigger: EntityHandle,
        subject: EntityHandle,
        kind: ContactKind,
        accepted: bool,
        producer_sequence: u64,
    },
    Request(RuntimeRequest),
    Diagnostic {
        code: DiagnosticCode,
        entity: Option<EntityHandle>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransitionRecord {
    pub sequence: u64,
    pub transition: Transition,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransitionBatch {
    pub records: Vec<TransitionRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFailureCode {
    InvalidConfiguration,
    EntityLimit,
    OutputLimit,
    QueueLimit,
    EventPhaseLimit,
    TransitionLimit,
    ContactLimit,
    HierarchyLimit,
    SnapshotLimit,
    DiagnosticLimit,
    TickRegression,
    InvalidField,
    SnapshotIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFailure {
    pub code: RuntimeFailureCode,
    pub entity: Option<usize>,
    pub limit: Option<usize>,
    pub actual: Option<usize>,
}

impl std::fmt::Display for RuntimeFailure {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(output, "{:?}", self.code)
    }
}

impl std::error::Error for RuntimeFailure {}

#[derive(Clone, Debug)]
struct QueuedEvent {
    id: u64,
    due_tick: u64,
    enqueue_sequence: u64,
    target: EventTarget,
    input: Vec<u8>,
    value: Variant,
    activator: Option<EntityHandle>,
    caller: Option<EntityHandle>,
    caller_name: Option<Vec<u8>>,
    caller_class: Option<Vec<u8>>,
    output_action: Option<u64>,
}

#[derive(Clone, Debug)]
struct Slot {
    generation: u32,
    entity: Option<RuntimeEntity>,
}

#[derive(Clone, Debug)]
struct WorldState {
    current_tick: u64,
    next_creation_order: u64,
    next_output_id: u64,
    next_event_id: u64,
    next_enqueue_sequence: u64,
    next_transition_sequence: u64,
    next_mover_request_id: u64,
    diagnostics: usize,
    random_state: u64,
    slots: Vec<Slot>,
    creation_order: Vec<EntityHandle>,
    targetname_index: BTreeMap<Vec<u8>, Vec<EntityHandle>>,
    classname_index: BTreeMap<Vec<u8>, Vec<EntityHandle>>,
    queue: Vec<QueuedEvent>,
}

#[derive(Clone, Debug)]
pub struct EntityWorld {
    config: EntityWorldConfig,
    state: WorldState,
}

#[derive(Clone, Debug)]
pub struct EntitySnapshot {
    source_identity: u64,
    registry_identity: u64,
    bytes: Vec<u8>,
    state: WorldState,
}

impl EntitySnapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn source_identity(&self) -> u64 {
        self.source_identity
    }

    pub fn registry_identity(&self) -> u64 {
        self.registry_identity
    }
}

impl EntityWorld {
    pub fn compile(
        graph: &Graph,
        config: EntityWorldConfig,
    ) -> Result<(Self, TransitionBatch), RuntimeFailure> {
        validate_config(&config)?;
        if graph.entities.len() > config.limits.max_entities {
            return Err(failure(
                RuntimeFailureCode::EntityLimit,
                None,
                config.limits.max_entities,
                graph.entities.len(),
            ));
        }
        let mut world = Self {
            state: WorldState {
                current_tick: 0,
                next_creation_order: 0,
                next_output_id: 1,
                next_event_id: 1,
                next_enqueue_sequence: 1,
                next_transition_sequence: 1,
                next_mover_request_id: 1,
                diagnostics: 0,
                random_state: config.random_seed.max(1),
                slots: Vec::new(),
                creation_order: Vec::new(),
                targetname_index: BTreeMap::new(),
                classname_index: BTreeMap::new(),
                queue: Vec::new(),
            },
            config,
        };
        let mut batch = TransitionBatch::default();
        for definition in &graph.entities {
            world.spawn_definition(definition.clone(), &mut batch)?;
        }
        world.resolve_initial_parents(&mut batch)?;
        let handles = world.state.creation_order.clone();
        for handle in handles {
            if let Some(entity) = world.entity_mut(handle) {
                entity.lifecycle = Lifecycle::Activated;
            }
            world.push_transition(
                &mut batch,
                Transition::Lifecycle {
                    entity: handle,
                    state: Lifecycle::Activated,
                },
            )?;
            world.schedule_activation(handle, &mut batch)?;
        }
        Ok((world, batch))
    }

    pub fn current_tick(&self) -> u64 {
        self.state.current_tick
    }

    pub fn entity(&self, handle: EntityHandle) -> Option<&RuntimeEntity> {
        let slot = self.state.slots.get(usize::from(handle.slot))?;
        (slot.generation == handle.generation)
            .then_some(slot.entity.as_ref())
            .flatten()
    }

    pub fn live_handles(&self) -> Vec<EntityHandle> {
        self.state
            .creation_order
            .iter()
            .copied()
            .filter(|handle| self.is_resolvable(*handle))
            .collect()
    }

    pub fn resolve(
        &self,
        expression: &[u8],
        searching: Option<EntityHandle>,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
    ) -> Vec<EntityHandle> {
        self.resolve_target(expression, searching, activator, caller)
            .unwrap_or_default()
    }

    pub fn has_pending(&self, target: EntityHandle, input_prefix: &[u8]) -> bool {
        self.state.queue.iter().any(|event| {
            matches!(event.target, EventTarget::Direct(value) if value == target)
                && event.input.starts_with(input_prefix)
        })
    }

    pub fn phase(
        &mut self,
        tick: u64,
        commands: &[WorldCommand],
    ) -> Result<TransitionBatch, RuntimeFailure> {
        if tick < self.state.current_tick {
            return Err(failure(
                RuntimeFailureCode::TickRegression,
                None,
                self.state.current_tick as usize,
                tick as usize,
            ));
        }
        self.state.current_tick = tick;
        let mut batch = TransitionBatch::default();
        for command in commands {
            let prior = self.state.clone();
            let batch_len = batch.records.len();
            if let Err(error) = self.apply_command(command.clone(), &mut batch) {
                self.state = prior;
                batch.records.truncate(batch_len);
                return Err(error);
            }
        }
        self.drain_due_events(&mut batch)?;
        self.commit_removals();
        Ok(batch)
    }

    pub fn snapshot(&self) -> Result<EntitySnapshot, RuntimeFailure> {
        let bytes = encode_state(&self.state, &self.config)?;
        Ok(EntitySnapshot {
            source_identity: self.config.source_identity,
            registry_identity: self.config.registry_identity,
            bytes,
            state: self.state.clone(),
        })
    }

    pub fn restore(&mut self, snapshot: &EntitySnapshot) -> Result<(), RuntimeFailure> {
        if snapshot.source_identity != self.config.source_identity
            || snapshot.registry_identity != self.config.registry_identity
        {
            return Err(failure(RuntimeFailureCode::SnapshotIdentity, None, 0, 0));
        }
        if snapshot.bytes.len() > self.config.limits.max_snapshot_bytes {
            return Err(failure(
                RuntimeFailureCode::SnapshotLimit,
                None,
                self.config.limits.max_snapshot_bytes,
                snapshot.bytes.len(),
            ));
        }
        let bytes = encode_state(&snapshot.state, &self.config)?;
        if bytes != snapshot.bytes {
            return Err(failure(
                RuntimeFailureCode::SnapshotIdentity,
                None,
                snapshot.bytes.len(),
                bytes.len(),
            ));
        }
        self.state = snapshot.state.clone();
        Ok(())
    }

    fn entity_mut(&mut self, handle: EntityHandle) -> Option<&mut RuntimeEntity> {
        let slot = self.state.slots.get_mut(usize::from(handle.slot))?;
        (slot.generation == handle.generation)
            .then_some(slot.entity.as_mut())
            .flatten()
    }

    fn is_resolvable(&self, handle: EntityHandle) -> bool {
        self.entity(handle)
            .is_some_and(|entity| entity.lifecycle != Lifecycle::PendingRemoval)
    }

    fn spawn_definition(
        &mut self,
        definition: Entity,
        batch: &mut TransitionBatch,
    ) -> Result<EntityHandle, RuntimeFailure> {
        let live = self
            .state
            .slots
            .iter()
            .filter(|slot| slot.entity.is_some())
            .count();
        if live >= self.config.limits.max_entities {
            return Err(failure(
                RuntimeFailureCode::EntityLimit,
                Some(definition.index),
                self.config.limits.max_entities,
                live + 1,
            ));
        }
        let output_count = self
            .state
            .slots
            .iter()
            .filter_map(|slot| slot.entity.as_ref())
            .map(|entity| entity.outputs.len())
            .sum::<usize>()
            + definition
                .connections
                .iter()
                .filter(|connection| matches!(connection, Connection::Parsed { .. }))
                .count();
        if output_count > self.config.limits.max_output_actions {
            return Err(failure(
                RuntimeFailureCode::OutputLimit,
                Some(definition.index),
                self.config.limits.max_output_actions,
                output_count,
            ));
        }

        let slot_index = self
            .state
            .slots
            .iter()
            .position(|slot| slot.entity.is_none())
            .unwrap_or(self.state.slots.len());
        if slot_index > u16::MAX as usize {
            return Err(failure(
                RuntimeFailureCode::EntityLimit,
                Some(definition.index),
                u16::MAX as usize,
                slot_index,
            ));
        }
        if slot_index == self.state.slots.len() {
            self.state.slots.push(Slot {
                generation: 1,
                entity: None,
            });
        }
        let handle = EntityHandle {
            slot: slot_index as u16,
            generation: self.state.slots[slot_index].generation,
        };
        let classname = definition.classname.clone().unwrap_or_default();
        let targetname = definition
            .targetname
            .clone()
            .filter(|name| !name.is_empty());
        let local_transform = Transform {
            origin: field_vector(&definition, b"origin", [0.0; 3])?,
            angles: field_vector(&definition, b"angles", [0.0; 3])?,
        };
        let (behavior, coverage) = self.behavior_for(&definition, local_transform)?;
        let mut outputs = Vec::new();
        let mut malformed_outputs = Vec::new();
        for connection in &definition.connections {
            match connection {
                Connection::Parsed {
                    order,
                    output,
                    target,
                    input,
                    parameter,
                    delay_bits,
                    max_fires,
                } => {
                    outputs.push(OutputActionState {
                        id: self.state.next_output_id,
                        declaration_order: *order,
                        output: output.clone(),
                        target: target.clone(),
                        input: input.clone(),
                        parameter: parameter.clone(),
                        delay_bits: *delay_bits,
                        remaining_fires: *max_fires,
                    });
                    self.state.next_output_id += 1;
                }
                Connection::Malformed { order, error, .. } => {
                    malformed_outputs.push((*order, *error));
                }
            }
        }
        let mut runtime_entity = RuntimeEntity {
            handle,
            source_index: definition.index,
            classname: classname.clone(),
            targetname: targetname.clone(),
            lifecycle: Lifecycle::Created,
            coverage,
            local_transform,
            world_transform: local_transform,
            parent: None,
            parent_attachment: None,
            children: Vec::new(),
            attachments: BTreeMap::new(),
            outputs,
            malformed_outputs,
            definition,
            behavior,
        };
        if let BehaviorState::Mover(MoverState {
            position: MoverPosition::Open,
            open,
            ..
        }) = &runtime_entity.behavior
        {
            runtime_entity.local_transform.origin = *open;
            runtime_entity.world_transform.origin = *open;
        }
        self.state.slots[slot_index].entity = Some(runtime_entity);
        self.state.creation_order.push(handle);
        self.state.next_creation_order += 1;
        self.index_insert(&classname, targetname.as_deref(), handle);
        self.push_transition(
            batch,
            Transition::Lifecycle {
                entity: handle,
                state: Lifecycle::Created,
            },
        )?;
        self.entity_mut(handle)
            .expect("new handle must resolve")
            .lifecycle = Lifecycle::Spawned;
        self.push_transition(
            batch,
            Transition::Lifecycle {
                entity: handle,
                state: Lifecycle::Spawned,
            },
        )?;
        Ok(handle)
    }

    fn behavior_for(
        &self,
        definition: &Entity,
        transform: Transform,
    ) -> Result<(BehaviorState, Coverage), RuntimeFailure> {
        let classname = definition.classname.as_deref().unwrap_or_default();
        let flags = field_i32(definition, b"spawnflags", 0)?;
        if classname.eq_ignore_ascii_case(b"func_brush") {
            let solidity = match field_i32(definition, b"Solidity", 0)? {
                0 => BrushSolidity::Toggle,
                1 => BrushSolidity::Never,
                2 => BrushSolidity::Always,
                _ => {
                    return Err(failure(
                        RuntimeFailureCode::InvalidField,
                        Some(definition.index),
                        2,
                        3,
                    ));
                }
            };
            return Ok((
                BehaviorState::Brush(BrushState {
                    enabled: field_i32(definition, b"StartDisabled", 0)? == 0,
                    solidity,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_button") {
            let speed = {
                let speed = field_f32(definition, b"speed", 0.0)?;
                if speed == 0.0 { 40.0 } else { speed }
            };
            let wait = {
                let wait = field_f32(definition, b"wait", 0.0)?;
                if wait == 0.0 { 1.0 } else { wait }
            };
            let lip = {
                let lip = field_f32(definition, b"lip", 0.0)?;
                if lip == 0.0 { 4.0 } else { lip }
            };
            let direction = direction_from_angles(field_vector(definition, b"movedir", [0.0; 3])?);
            let distance = self.brush_travel(definition, direction, lip).max(0.0);
            let open = add(transform.origin, scale(direction, distance));
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Button,
                    closed: transform.origin,
                    open,
                    speed,
                    wait_ticks: (wait >= 0.0).then(|| delay_ticks(wait, self.config.tick_interval)),
                    position: MoverPosition::Closed,
                    pending: None,
                    locked: flags & BUTTON_LOCKED != 0,
                    toggle: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    force_closed: false,
                    no_auto_return: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    outputs_reversed: false,
                    block_damage_bits: 0.0f32.to_bits(),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_door") {
            let speed = {
                let speed = field_f32(definition, b"speed", 0.0)?;
                if speed == 0.0 { 100.0 } else { speed }
            };
            let direction = direction_from_angles(field_vector(definition, b"movedir", [0.0; 3])?);
            let lip = field_f32(definition, b"lip", 0.0)?;
            let distance = self.brush_travel(definition, direction, lip).max(0.0);
            let open = add(transform.origin, scale(direction, distance));
            let starts_open =
                field_i32(definition, b"spawnpos", 0)? == 1 || flags & DOOR_START_OPEN != 0;
            let wait = field_f32(definition, b"wait", 0.0)?;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Door,
                    closed: transform.origin,
                    open,
                    speed,
                    wait_ticks: (wait >= 0.0).then(|| delay_ticks(wait, self.config.tick_interval)),
                    position: if starts_open {
                        MoverPosition::Open
                    } else {
                        MoverPosition::Closed
                    },
                    pending: None,
                    locked: flags & DOOR_LOCKED != 0,
                    toggle: flags & DOOR_NO_AUTO_RETURN != 0,
                    force_closed: field_i32(definition, b"forceclosed", 0)? != 0,
                    no_auto_return: flags & DOOR_NO_AUTO_RETURN != 0,
                    outputs_reversed: starts_open,
                    block_damage_bits: field_f32(definition, b"dmg", 0.0)?.to_bits(),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_movelinear") {
            let direction = direction_from_angles(field_vector(definition, b"movedir", [0.0; 3])?);
            let mut distance = field_f32(definition, b"MoveDistance", 0.0)?;
            if distance <= 0.0 {
                distance =
                    self.brush_travel(definition, direction, field_f32(definition, b"lip", 0.0)?);
            }
            let start = field_f32(definition, b"StartPosition", 0.0)?;
            let closed = sub(transform.origin, scale(direction, distance * start));
            let open = add(closed, scale(direction, distance));
            let speed = field_f32(definition, b"speed", 0.0)?;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Linear,
                    closed,
                    open,
                    speed: if speed <= 0.0 { 100.0 } else { speed },
                    wait_ticks: None,
                    position: MoverPosition::Positioned(start.to_bits()),
                    pending: None,
                    locked: false,
                    toggle: false,
                    force_closed: false,
                    no_auto_return: true,
                    outputs_reversed: false,
                    block_damage_bits: field_f32(definition, b"BlockDamage", 0.0)?.to_bits(),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"logic_auto") {
            return Ok((BehaviorState::LogicAuto, Coverage::Handled));
        }
        if classname.eq_ignore_ascii_case(b"logic_relay") {
            return Ok((
                BehaviorState::Relay(RelayState {
                    enabled: field_i32(definition, b"StartDisabled", 0)? == 0,
                    waiting_for_refire: false,
                    remove_on_fire: flags & RELAY_REMOVE_ON_FIRE != 0,
                    fast_retrigger: flags & RELAY_FAST_RETRIGGER != 0,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"math_counter") {
            let mut min = field_f32(definition, b"min", 0.0)?;
            let mut max = field_f32(definition, b"max", 0.0)?;
            if min > max {
                std::mem::swap(&mut min, &mut max);
            }
            let mut value = field_f32(definition, b"startvalue", 0.0)?;
            if min != 0.0 || max != 0.0 {
                value = value.clamp(min, max);
            }
            return Ok((
                BehaviorState::Counter(CounterState {
                    value_bits: value.to_bits(),
                    min_bits: min.to_bits(),
                    max_bits: max.to_bits(),
                    hit_min: false,
                    hit_max: false,
                    enabled: field_i32(definition, b"StartDisabled", 0)? == 0,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"logic_case") {
            let mut cases = Vec::with_capacity(16);
            for index in 1..=16 {
                let key = format!("Case{index:02}");
                cases.push(field(definition, key.as_bytes()).map(<[u8]>::to_vec));
            }
            return Ok((
                BehaviorState::Case(CaseState {
                    cases,
                    shuffle: Vec::new(),
                    last_shuffle: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"filter_activator_name") {
            return Ok((
                BehaviorState::Filter(FilterState {
                    negated: field_i32(definition, b"Negated", 0)? != 0,
                    predicate: FilterPredicate::Name(
                        field(definition, b"filtername")
                            .unwrap_or_default()
                            .to_vec(),
                    ),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"filter_activator_class") {
            return Ok((
                BehaviorState::Filter(FilterState {
                    negated: field_i32(definition, b"Negated", 0)? != 0,
                    predicate: FilterPredicate::Class(
                        field(definition, b"filterclass")
                            .unwrap_or_default()
                            .to_vec(),
                    ),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"filter_multi") {
            let mut children = Vec::new();
            for index in 1..=5 {
                let key = format!("Filter{index:02}");
                if let Some(name) =
                    field(definition, key.as_bytes()).filter(|value| !value.is_empty())
                {
                    children.push(name.to_vec());
                }
            }
            let predicate = if field_i32(definition, b"FilterType", 0)? == 0 {
                FilterPredicate::All(children)
            } else {
                FilterPredicate::Any(children)
            };
            return Ok((
                BehaviorState::Filter(FilterState {
                    negated: field_i32(definition, b"Negated", 0)? != 0,
                    predicate,
                }),
                Coverage::Handled,
            ));
        }
        if classname.starts_with(b"filter_") {
            return Ok((
                BehaviorState::Filter(FilterState {
                    negated: field_i32(definition, b"Negated", 0)? != 0,
                    predicate: FilterPredicate::External,
                }),
                Coverage::Unsupported,
            ));
        }
        let trigger_kind = if classname.eq_ignore_ascii_case(b"trigger_multiple") {
            Some(TriggerKind::Multiple)
        } else if classname.eq_ignore_ascii_case(b"trigger_hurt") {
            Some(TriggerKind::Hurt)
        } else if classname.eq_ignore_ascii_case(b"trigger_push") {
            Some(TriggerKind::Push)
        } else if classname.eq_ignore_ascii_case(b"trigger_catapult") {
            Some(TriggerKind::Catapult)
        } else if classname.eq_ignore_ascii_case(b"trigger_teleport") {
            Some(TriggerKind::Teleport)
        } else {
            None
        };
        if let Some(kind) = trigger_kind {
            let wait = if kind == TriggerKind::Multiple {
                let wait = field_f32(definition, b"wait", 0.0)?;
                if wait == 0.0 { 0.2 } else { wait }
            } else {
                0.0
            };
            return Ok((
                BehaviorState::Trigger(TriggerState {
                    kind,
                    enabled: field_i32(definition, b"StartDisabled", 0)? == 0,
                    filter_name: field(definition, b"filtername")
                        .filter(|value| !value.is_empty())
                        .map(<[u8]>::to_vec),
                    contacts: Vec::new(),
                    wait_ticks: delay_ticks(wait, self.config.tick_interval),
                    next_fire_tick: 0,
                    mutable_value_bits: field_f32(definition, b"damage", 0.0)?.to_bits(),
                }),
                Coverage::Handled,
            ));
        }
        let coverage = if classname.eq_ignore_ascii_case(b"worldspawn")
            || classname.eq_ignore_ascii_case(b"info_teleport_destination")
        {
            Coverage::IntentionallyInert
        } else if classname.is_empty() {
            Coverage::Malformed
        } else {
            Coverage::Unknown
        };
        Ok((BehaviorState::Inert, coverage))
    }

    fn brush_travel(&self, definition: &Entity, direction: [f32; 3], lip: f32) -> f32 {
        let Some(model) = definition.bsp_model_index else {
            return 0.0;
        };
        let Some(bounds) = self
            .config
            .model_bounds
            .iter()
            .find(|bounds| bounds.model == model)
        else {
            return 0.0;
        };
        let size = [
            (bounds.maxs[0] - bounds.mins[0] - 2.0).max(0.0),
            (bounds.maxs[1] - bounds.mins[1] - 2.0).max(0.0),
            (bounds.maxs[2] - bounds.mins[2] - 2.0).max(0.0),
        ];
        dot_abs(direction, size) - lip
    }

    fn index_insert(&mut self, classname: &[u8], targetname: Option<&[u8]>, handle: EntityHandle) {
        self.state
            .classname_index
            .entry(ascii_key(classname))
            .or_default()
            .push(handle);
        if let Some(name) = targetname {
            self.state
                .targetname_index
                .entry(ascii_key(name))
                .or_default()
                .push(handle);
        }
    }

    fn index_remove(&mut self, handle: EntityHandle) {
        for values in self.state.classname_index.values_mut() {
            values.retain(|value| *value != handle);
        }
        for values in self.state.targetname_index.values_mut() {
            values.retain(|value| *value != handle);
        }
        self.state
            .classname_index
            .retain(|_, values| !values.is_empty());
        self.state
            .targetname_index
            .retain(|_, values| !values.is_empty());
    }

    fn push_transition(
        &mut self,
        batch: &mut TransitionBatch,
        transition: Transition,
    ) -> Result<(), RuntimeFailure> {
        if batch.records.len() >= self.config.limits.max_transitions_per_phase {
            return Err(failure(
                RuntimeFailureCode::TransitionLimit,
                None,
                self.config.limits.max_transitions_per_phase,
                batch.records.len() + 1,
            ));
        }
        if matches!(transition, Transition::Diagnostic { .. }) {
            if self.state.diagnostics >= self.config.limits.max_diagnostics {
                return Err(failure(
                    RuntimeFailureCode::DiagnosticLimit,
                    None,
                    self.config.limits.max_diagnostics,
                    self.state.diagnostics + 1,
                ));
            }
            self.state.diagnostics += 1;
        }
        let sequence = self.state.next_transition_sequence;
        self.state.next_transition_sequence += 1;
        batch.records.push(TransitionRecord {
            sequence,
            transition,
        });
        Ok(())
    }

    fn resolve_initial_parents(
        &mut self,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let requests: Vec<_> = self
            .state
            .creation_order
            .iter()
            .filter_map(|handle| {
                let entity = self.entity(*handle)?;
                entity
                    .definition
                    .parentname
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .map(|value| (*handle, value.clone()))
            })
            .collect();
        for (child, raw) in requests {
            let mut fields = raw.splitn(2, |byte| *byte == b',');
            let name = fields.next().unwrap_or_default();
            let attachment = fields
                .next()
                .filter(|value| !value.is_empty())
                .map(<[u8]>::to_vec);
            let matches = self.resolve_target(name, None, None, None)?;
            if matches.is_empty() {
                self.push_transition(
                    batch,
                    Transition::Diagnostic {
                        code: DiagnosticCode::MissingParent,
                        entity: Some(child),
                    },
                )?;
                continue;
            }
            if matches.len() > 1 {
                self.push_transition(
                    batch,
                    Transition::Diagnostic {
                        code: DiagnosticCode::AmbiguousParent,
                        entity: Some(child),
                    },
                )?;
            }
            self.set_parent(
                ParentRequest {
                    child,
                    parent: Some(matches[0]),
                    attachment,
                    mode: ParentMode::MaintainWorld,
                },
                batch,
            )?;
        }
        Ok(())
    }

    fn schedule_activation(
        &mut self,
        handle: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let behavior = self.entity(handle).map(|entity| entity.behavior.clone());
        match behavior {
            Some(BehaviorState::LogicAuto) => {
                self.schedule_event(
                    EventTarget::Direct(handle),
                    b"__logic_auto_fire".to_vec(),
                    Variant::Void,
                    0.2,
                    None,
                    Some(handle),
                    None,
                    batch,
                )?;
            }
            Some(BehaviorState::Relay(_))
                if self
                    .entity(handle)
                    .is_some_and(|entity| output_connected(entity, b"OnSpawn")) =>
            {
                self.schedule_event(
                    EventTarget::Direct(handle),
                    b"__relay_spawn".to_vec(),
                    Variant::Void,
                    0.01,
                    Some(handle),
                    Some(handle),
                    None,
                    batch,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_target(
        &self,
        expression: &[u8],
        searching: Option<EntityHandle>,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
    ) -> Result<Vec<EntityHandle>, RuntimeFailure> {
        if expression.is_empty() {
            return Ok(Vec::new());
        }
        if expression[0] == b'!' {
            let selected = if expression.eq_ignore_ascii_case(b"!self") {
                searching
            } else if expression.eq_ignore_ascii_case(b"!activator") {
                activator
            } else if expression.eq_ignore_ascii_case(b"!caller") {
                caller
            } else {
                None
            };
            return Ok(selected
                .filter(|handle| self.is_resolvable(*handle))
                .into_iter()
                .collect());
        }
        let wildcard =
            expression.ends_with(b"*") && !expression[..expression.len() - 1].contains(&b'*');
        let has_other_wildcard = expression[..expression.len().saturating_sub(1)].contains(&b'*');
        if has_other_wildcard {
            return Ok(Vec::new());
        }
        let matches_name = |entity: &RuntimeEntity| {
            entity.targetname.as_deref().is_some_and(|name| {
                if wildcard {
                    ascii_starts_with(name, &expression[..expression.len() - 1])
                } else {
                    name.eq_ignore_ascii_case(expression)
                }
            })
        };
        let mut found: Vec<_> = self
            .state
            .creation_order
            .iter()
            .copied()
            .filter(|handle| self.is_resolvable(*handle))
            .filter(|handle| self.entity(*handle).is_some_and(&matches_name))
            .collect();
        if !found.is_empty() {
            return Ok(found);
        }
        found.extend(
            self.state
                .creation_order
                .iter()
                .copied()
                .filter(|handle| self.is_resolvable(*handle))
                .filter(|handle| {
                    self.entity(*handle).is_some_and(|entity| {
                        if wildcard {
                            ascii_starts_with(
                                &entity.classname,
                                &expression[..expression.len() - 1],
                            )
                        } else {
                            entity.classname.eq_ignore_ascii_case(expression)
                        }
                    })
                }),
        );
        Ok(found)
    }

    fn set_parent(
        &mut self,
        request: ParentRequest,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(request.child) {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(request.child));
        }
        if let Some(parent) = request.parent {
            if !self.is_resolvable(parent) {
                return self.diagnostic(batch, DiagnosticCode::MissingParent, Some(request.child));
            }
            if parent == request.child || self.is_descendant(request.child, parent)? {
                return self.diagnostic(batch, DiagnosticCode::ParentCycle, Some(request.child));
            }
            if let Some(attachment) = request.attachment.as_ref()
                && !self
                    .entity(parent)
                    .is_some_and(|entity| entity.attachments.contains_key(attachment))
            {
                return self.diagnostic(
                    batch,
                    DiagnosticCode::MissingAttachment,
                    Some(request.child),
                );
            }
        }
        let world = self
            .entity(request.child)
            .expect("validated child")
            .world_transform;
        let old_parent = self.entity(request.child).and_then(|entity| entity.parent);
        let old_attachment = self
            .entity(request.child)
            .and_then(|entity| entity.parent_attachment.clone());
        let old_basis = match old_parent {
            Some(parent) => self.parent_basis(parent, old_attachment.as_deref())?,
            None => Transform::IDENTITY,
        };
        let mover_world_endpoints = self.entity(request.child).and_then(|entity| {
            let BehaviorState::Mover(mover) = &entity.behavior else {
                return None;
            };
            let transform_point = |point| {
                if old_parent.is_some() {
                    compose_transform(
                        old_basis,
                        Transform {
                            origin: point,
                            angles: [0.0; 3],
                        },
                    )
                    .origin
                } else {
                    point
                }
            };
            Some((transform_point(mover.closed), transform_point(mover.open)))
        });
        if let Some(old_parent) = old_parent
            && let Some(parent) = self.entity_mut(old_parent)
        {
            parent.children.retain(|child| *child != request.child);
        }
        let local = match request.parent {
            Some(parent) => {
                let basis = self.parent_basis(parent, request.attachment.as_deref())?;
                match request.mode {
                    ParentMode::MaintainWorld => relative_transform(basis, world),
                    ParentMode::SnapToAttachment => Transform::IDENTITY,
                }
            }
            None => world,
        };
        let new_basis = match request.parent {
            Some(parent) => self.parent_basis(parent, request.attachment.as_deref())?,
            None => Transform::IDENTITY,
        };
        if let Some(entity) = self.entity_mut(request.child) {
            entity.parent = request.parent;
            entity.parent_attachment = request.attachment.clone();
            entity.local_transform = local;
            if let (Some((closed, open)), BehaviorState::Mover(mover)) =
                (mover_world_endpoints, &mut entity.behavior)
            {
                let local_point = |point| {
                    if request.parent.is_some() {
                        relative_transform(
                            new_basis,
                            Transform {
                                origin: point,
                                angles: [0.0; 3],
                            },
                        )
                        .origin
                    } else {
                        point
                    }
                };
                mover.closed = local_point(closed);
                mover.open = local_point(open);
            }
        }
        if let Some(parent_handle) = request.parent
            && let Some(parent) = self.entity_mut(parent_handle)
        {
            parent.children.push(request.child);
        }
        self.recompute_subtree(request.child, 0)?;
        self.push_transition(
            batch,
            Transition::ParentChanged {
                child: request.child,
                parent: request.parent,
                attachment: request.attachment,
            },
        )?;
        let entity = self.entity(request.child).expect("validated child");
        self.push_transition(
            batch,
            Transition::TransformChanged {
                entity: request.child,
                local: entity.local_transform,
                world: entity.world_transform,
            },
        )
    }

    fn is_descendant(
        &self,
        ancestor: EntityHandle,
        candidate: EntityHandle,
    ) -> Result<bool, RuntimeFailure> {
        let mut at = Some(candidate);
        let mut depth = 0;
        while let Some(handle) = at {
            if handle == ancestor {
                return Ok(true);
            }
            depth += 1;
            if depth > self.config.limits.max_hierarchy_depth {
                return Err(failure(
                    RuntimeFailureCode::HierarchyLimit,
                    self.entity(candidate).map(|entity| entity.source_index),
                    self.config.limits.max_hierarchy_depth,
                    depth,
                ));
            }
            at = self.entity(handle).and_then(|entity| entity.parent);
        }
        Ok(false)
    }

    fn parent_basis(
        &self,
        parent: EntityHandle,
        attachment: Option<&[u8]>,
    ) -> Result<Transform, RuntimeFailure> {
        let entity = self
            .entity(parent)
            .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
        Ok(match attachment {
            Some(name) => compose_transform(
                entity.world_transform,
                *entity.attachments.get(name).ok_or_else(|| {
                    failure(
                        RuntimeFailureCode::InvalidField,
                        Some(entity.source_index),
                        0,
                        0,
                    )
                })?,
            ),
            None => entity.world_transform,
        })
    }

    fn recompute_subtree(
        &mut self,
        handle: EntityHandle,
        depth: usize,
    ) -> Result<(), RuntimeFailure> {
        if depth > self.config.limits.max_hierarchy_depth {
            return Err(failure(
                RuntimeFailureCode::HierarchyLimit,
                self.entity(handle).map(|entity| entity.source_index),
                self.config.limits.max_hierarchy_depth,
                depth,
            ));
        }
        let (parent, attachment, local, children) = {
            let entity = self
                .entity(handle)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            (
                entity.parent,
                entity.parent_attachment.clone(),
                entity.local_transform,
                entity.children.clone(),
            )
        };
        let world = match parent {
            Some(parent) => {
                compose_transform(self.parent_basis(parent, attachment.as_deref())?, local)
            }
            None => local,
        };
        if let Some(entity) = self.entity_mut(handle) {
            entity.world_transform = world;
        }
        for child in children {
            self.recompute_subtree(child, depth + 1)?;
        }
        Ok(())
    }

    fn diagnostic(
        &mut self,
        batch: &mut TransitionBatch,
        code: DiagnosticCode,
        entity: Option<EntityHandle>,
    ) -> Result<(), RuntimeFailure> {
        self.push_transition(batch, Transition::Diagnostic { code, entity })
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_event(
        &mut self,
        target: EventTarget,
        input: Vec<u8>,
        value: Variant,
        delay: f32,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
        output_action: Option<u64>,
        batch: &mut TransitionBatch,
    ) -> Result<u64, RuntimeFailure> {
        if self.state.queue.len() >= self.config.limits.max_queued_events {
            return Err(failure(
                RuntimeFailureCode::QueueLimit,
                caller.and_then(|handle| self.entity(handle).map(|entity| entity.source_index)),
                self.config.limits.max_queued_events,
                self.state.queue.len() + 1,
            ));
        }
        let event_id = self.state.next_event_id;
        self.state.next_event_id += 1;
        let due_tick = self
            .state
            .current_tick
            .saturating_add(delay_ticks(delay, self.config.tick_interval));
        let (caller_name, caller_class) = caller
            .and_then(|handle| self.entity(handle))
            .map(|entity| (entity.targetname.clone(), Some(entity.classname.clone())))
            .unwrap_or((None, None));
        let event = QueuedEvent {
            id: event_id,
            due_tick,
            enqueue_sequence: self.state.next_enqueue_sequence,
            target,
            input,
            value,
            activator,
            caller,
            caller_name,
            caller_class,
            output_action,
        };
        self.state.next_enqueue_sequence += 1;
        self.state.queue.push(event);
        self.state
            .queue
            .sort_by_key(|event| (event.due_tick, event.enqueue_sequence));
        self.push_transition(batch, Transition::Scheduled { event_id, due_tick })?;
        Ok(event_id)
    }

    fn apply_command(
        &mut self,
        command: WorldCommand,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        match command {
            WorldCommand::Spawn(definition) => {
                let handle = self.spawn_definition(definition, batch)?;
                self.entity_mut(handle).expect("spawned handle").lifecycle = Lifecycle::Activated;
                self.push_transition(
                    batch,
                    Transition::Lifecycle {
                        entity: handle,
                        state: Lifecycle::Activated,
                    },
                )?;
                self.schedule_activation(handle, batch)
            }
            WorldCommand::Input(input) => self.dispatch_record(input, batch),
            WorldCommand::Contact(contact) => self.apply_contact(contact, batch),
            WorldCommand::Remove(handle) => self.mark_removal(handle, batch),
            WorldCommand::SetParent(request) => self.set_parent(request, batch),
            WorldCommand::SetWorldTransform { entity, transform } => {
                if !self.is_resolvable(entity) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
                }
                let local = match self.entity(entity).and_then(|value| value.parent) {
                    Some(parent) => relative_transform(
                        self.parent_basis(
                            parent,
                            self.entity(entity)
                                .and_then(|value| value.parent_attachment.as_deref()),
                        )?,
                        transform,
                    ),
                    None => transform,
                };
                self.entity_mut(entity).expect("validated").local_transform = local;
                self.recompute_subtree(entity, 0)?;
                let entity_state = self.entity(entity).expect("validated");
                self.push_transition(
                    batch,
                    Transition::TransformChanged {
                        entity,
                        local: entity_state.local_transform,
                        world: entity_state.world_transform,
                    },
                )
            }
            WorldCommand::SetAttachmentTransform {
                parent,
                attachment,
                parent_space_transform,
            } => {
                if !self.is_resolvable(parent) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(parent));
                }
                let children = self.entity(parent).expect("validated").children.clone();
                self.entity_mut(parent)
                    .expect("validated")
                    .attachments
                    .insert(attachment, parent_space_transform);
                for child in children {
                    self.recompute_subtree(child, 0)?;
                }
                Ok(())
            }
            WorldCommand::MoverCompleted { entity, request_id } => {
                self.complete_mover(entity, request_id, batch)
            }
            WorldCommand::MoverBlocked {
                entity,
                request_id,
                blocker,
                kind,
            } => self.block_mover(entity, request_id, blocker, kind, batch),
            WorldCommand::CancelCaller(caller) => self.cancel_caller(caller, batch),
            WorldCommand::CancelDirectInput {
                target,
                input_prefix,
            } => self.cancel_direct(target, &input_prefix, batch),
            WorldCommand::EmitOutput {
                entity,
                output,
                value,
                activator,
                caller,
                delay,
            } => {
                if !delay.is_finite() {
                    return Err(failure(
                        RuntimeFailureCode::InvalidField,
                        self.entity(entity).map(|entity| entity.source_index),
                        0,
                        0,
                    ));
                }
                self.fire_output(entity, &output, value, activator, caller, delay, batch)
            }
            WorldCommand::SetTargetname { entity, targetname } => {
                if !self.is_resolvable(entity) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
                }
                self.index_remove(entity);
                let classname = self.entity(entity).expect("validated").classname.clone();
                let targetname = targetname.filter(|name| !name.is_empty());
                self.entity_mut(entity).expect("validated").targetname = targetname.clone();
                self.index_insert(&classname, targetname.as_deref(), entity);
                Ok(())
            }
        }
    }

    fn drain_due_events(&mut self, batch: &mut TransitionBatch) -> Result<(), RuntimeFailure> {
        let mut serviced = 0usize;
        loop {
            self.state
                .queue
                .sort_by_key(|event| (event.due_tick, event.enqueue_sequence));
            let Some(event) = self.state.queue.first().cloned() else {
                break;
            };
            if event.due_tick > self.state.current_tick {
                break;
            }
            if serviced >= self.config.limits.max_events_per_tick {
                return Err(failure(
                    RuntimeFailureCode::EventPhaseLimit,
                    None,
                    self.config.limits.max_events_per_tick,
                    serviced + 1,
                ));
            }
            let prior = self.state.clone();
            let batch_len = batch.records.len();
            self.state.queue.remove(0);
            let record = InputRecord {
                target: event.target,
                input: event.input,
                value: event.value,
                activator: event.activator.filter(|handle| self.is_resolvable(*handle)),
                caller: event.caller.filter(|handle| self.is_resolvable(*handle)),
                output_action: event.output_action,
                producer_sequence: event.enqueue_sequence,
            };
            if let Err(error) = self.dispatch_record(record, batch) {
                self.state = prior;
                batch.records.truncate(batch_len);
                return Err(error);
            }
            serviced += 1;
        }
        Ok(())
    }

    fn cancel_caller(
        &mut self,
        caller: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(entity) = self.entity(caller) else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(caller));
        };
        let name = entity.targetname.clone();
        let class = entity.classname.clone();
        let mut cancelled = Vec::new();
        self.state.queue.retain(|event| {
            let matches = event.caller == Some(caller)
                && event.caller_name == name
                && event.caller_class.as_deref() == Some(class.as_slice());
            if matches {
                cancelled.push(event.id);
            }
            !matches
        });
        for event_id in cancelled {
            self.push_transition(batch, Transition::Cancelled { event_id })?;
        }
        Ok(())
    }

    fn cancel_direct(
        &mut self,
        target: EntityHandle,
        input_prefix: &[u8],
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let mut cancelled = Vec::new();
        self.state.queue.retain(|event| {
            let matches = matches!(event.target, EventTarget::Direct(handle) if handle == target)
                && event.input.starts_with(input_prefix);
            if matches {
                cancelled.push(event.id);
            }
            !matches
        });
        for event_id in cancelled {
            self.push_transition(batch, Transition::Cancelled { event_id })?;
        }
        Ok(())
    }

    fn dispatch_record(
        &mut self,
        record: InputRecord,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let targets = match &record.target {
            EventTarget::Direct(handle) => {
                if self.is_resolvable(*handle) {
                    vec![*handle]
                } else {
                    self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(*handle))?;
                    Vec::new()
                }
            }
            EventTarget::Expression(expression) => {
                if expression.starts_with(b"!")
                    && !expression.eq_ignore_ascii_case(b"!self")
                    && !expression.eq_ignore_ascii_case(b"!caller")
                    && !expression.eq_ignore_ascii_case(b"!activator")
                {
                    self.diagnostic(batch, DiagnosticCode::UnsupportedSelector, record.caller)?;
                    Vec::new()
                } else {
                    self.resolve_target(expression, record.caller, record.activator, record.caller)?
                }
            }
        };
        if targets.is_empty() {
            self.diagnostic(batch, DiagnosticCode::MissingTarget, record.caller)?;
            return Ok(());
        }
        for target in targets {
            self.dispatch_input(target, &record, batch)?;
        }
        Ok(())
    }

    fn dispatch_input(
        &mut self,
        target: EntityHandle,
        record: &InputRecord,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(target) {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(target));
        }
        let input = ascii_key(&record.input);
        let behavior = self
            .entity(target)
            .expect("validated target")
            .behavior
            .clone();
        let mut accepted = true;
        let mut outputs: Vec<(Vec<u8>, Variant, Option<EntityHandle>)> = Vec::new();
        let mut mover: Option<(f32, bool)> = None;
        let mut remove = false;
        let mut parent: Option<ParentRequest> = None;
        let mut cancel_caller = false;
        let mut relay_refire_delay = None;

        if input == b"kill" {
            remove = true;
        } else if input == b"clearparent" {
            parent = Some(ParentRequest {
                child: target,
                parent: None,
                attachment: None,
                mode: ParentMode::MaintainWorld,
            });
        } else if input == b"setparent" {
            let raw = record.value.as_string();
            let mut fields = raw.splitn(2, |byte| *byte == b',');
            let name = fields.next().unwrap_or_default();
            let attachment = fields
                .next()
                .filter(|value| !value.is_empty())
                .map(<[u8]>::to_vec);
            let matches =
                self.resolve_target(name, record.caller, record.activator, record.caller)?;
            if let Some(new_parent) = matches.first() {
                parent = Some(ParentRequest {
                    child: target,
                    parent: Some(*new_parent),
                    attachment,
                    mode: ParentMode::MaintainWorld,
                });
            } else {
                accepted = false;
            }
        } else if let Some(user) = input.strip_prefix(b"fireuser") {
            if matches!(user, b"1" | b"2" | b"3" | b"4") {
                outputs.push((
                    [b"OnUser".as_slice(), user].concat(),
                    Variant::Void,
                    record.activator,
                ));
            } else {
                accepted = false;
            }
        } else {
            match behavior {
                BehaviorState::Inert => accepted = false,
                BehaviorState::Brush(mut state) => {
                    match input.as_slice() {
                        b"enable" => state.enabled = true,
                        b"disable" => state.enabled = false,
                        b"toggle" => state.enabled = !state.enabled,
                        _ => accepted = false,
                    }
                    if accepted {
                        let solid = match state.solidity {
                            BrushSolidity::Never => false,
                            BrushSolidity::Always => true,
                            BrushSolidity::Toggle => state.enabled,
                        };
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Brush(state.clone());
                        self.push_transition(
                            batch,
                            Transition::Request(RuntimeRequest::BrushState {
                                entity: target,
                                enabled: state.enabled,
                                solid,
                            }),
                        )?;
                    }
                }
                BehaviorState::Mover(mut state) => {
                    match state.kind {
                        MoverKind::Door => match input.as_slice() {
                            b"open" => {
                                if !state.locked
                                    && !matches!(
                                        state.position,
                                        MoverPosition::Open | MoverPosition::Opening
                                    )
                                {
                                    mover = Some((1.0, true));
                                }
                            }
                            b"close" => {
                                if state.position != MoverPosition::Closed {
                                    mover = Some((0.0, false));
                                }
                            }
                            b"toggle" | b"use" => {
                                if !state.locked {
                                    match state.position {
                                        MoverPosition::Closed | MoverPosition::Closing => {
                                            mover = Some((1.0, true));
                                        }
                                        MoverPosition::Open | MoverPosition::Opening => {
                                            mover = Some((0.0, false));
                                        }
                                        MoverPosition::Positioned(_) => {}
                                    }
                                }
                            }
                            b"lock" => state.locked = true,
                            b"unlock" => state.locked = false,
                            b"setspeed" => match record.value.as_float() {
                                Some(speed) if speed.is_finite() => state.speed = speed,
                                _ => accepted = false,
                            },
                            _ => accepted = false,
                        },
                        MoverKind::Button => match input.as_slice() {
                            b"press" | b"use" => {
                                if !state.locked {
                                    match state.position {
                                        MoverPosition::Closed => mover = Some((1.0, true)),
                                        MoverPosition::Open if state.toggle => {
                                            mover = Some((0.0, false));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            b"pressin" => {
                                if !state.locked
                                    && matches!(
                                        state.position,
                                        MoverPosition::Closed | MoverPosition::Closing
                                    )
                                {
                                    mover = Some((1.0, true));
                                }
                            }
                            b"pressout" => {
                                if !state.locked
                                    && matches!(
                                        state.position,
                                        MoverPosition::Open | MoverPosition::Opening
                                    )
                                {
                                    mover = Some((0.0, false));
                                }
                            }
                            b"lock" => state.locked = true,
                            b"unlock" => state.locked = false,
                            _ => accepted = false,
                        },
                        MoverKind::Linear => match input.as_slice() {
                            b"open" => mover = Some((1.0, true)),
                            b"close" => mover = Some((0.0, false)),
                            b"setposition" => match record.value.as_float() {
                                Some(value) if value.is_finite() => {
                                    mover = Some((value, value >= 0.5));
                                }
                                _ => accepted = false,
                            },
                            b"setspeed" => match record.value.as_float() {
                                Some(speed) if speed.is_finite() => {
                                    state.speed = speed;
                                    if let Some(pending) = &state.pending {
                                        let travel = sub(state.open, state.closed);
                                        let denominator = dot(travel, travel);
                                        if denominator > f32::EPSILON {
                                            mover = Some((
                                                dot(
                                                    sub(pending.local_destination, state.closed),
                                                    travel,
                                                ) / denominator,
                                                pending.opening,
                                            ));
                                        }
                                    }
                                }
                                _ => accepted = false,
                            },
                            _ => accepted = false,
                        },
                    }
                    if accepted {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Mover(state);
                    }
                }
                BehaviorState::LogicAuto => {
                    if input == b"__logic_auto_fire" {
                        let load_output = match self.config.load_kind {
                            MapLoadKind::NewGame => b"OnNewGame".as_slice(),
                            MapLoadKind::LoadGame => b"OnLoadGame".as_slice(),
                            MapLoadKind::Transition => b"OnMapTransition".as_slice(),
                            MapLoadKind::Background => b"OnBackgroundMap".as_slice(),
                            MapLoadKind::MultiplayerNewMap => b"OnMultiNewMap".as_slice(),
                            MapLoadKind::MultiplayerNewRound => b"OnMultiNewRound".as_slice(),
                        };
                        outputs.push((load_output.to_vec(), Variant::Void, None));
                        outputs.push((b"OnMapSpawn".to_vec(), Variant::Void, None));
                        let flags = field_i32(
                            &self.entity(target).expect("validated").definition,
                            b"spawnflags",
                            0,
                        )?;
                        remove = flags & 1 != 0;
                    } else {
                        accepted = false;
                    }
                }
                BehaviorState::Relay(mut state) => {
                    match input.as_slice() {
                        b"enable" => state.enabled = true,
                        b"disable" => state.enabled = false,
                        b"toggle" => state.enabled = !state.enabled,
                        b"enablerefire" => state.waiting_for_refire = false,
                        b"cancelpending" => {
                            cancel_caller = true;
                            state.waiting_for_refire = false;
                        }
                        b"trigger" => {
                            if state.enabled && !state.waiting_for_refire {
                                outputs.push((
                                    b"OnTrigger".to_vec(),
                                    Variant::Void,
                                    record.activator,
                                ));
                                if state.remove_on_fire {
                                    remove = true;
                                } else if !state.fast_retrigger {
                                    state.waiting_for_refire = true;
                                    relay_refire_delay =
                                        Some(self.max_output_delay(target, b"OnTrigger") + 0.001);
                                }
                            }
                        }
                        b"__relay_spawn" => {
                            outputs.push((b"OnSpawn".to_vec(), Variant::Void, Some(target)));
                            remove = state.remove_on_fire;
                        }
                        _ => accepted = false,
                    }
                    if accepted {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Relay(state.clone());
                    }
                }
                BehaviorState::Counter(state) => {
                    accepted = self.counter_input(target, state, &input, record, &mut outputs)?;
                }
                BehaviorState::Case(state) => {
                    accepted = self.case_input(target, state, &input, record, &mut outputs)?;
                }
                BehaviorState::Filter(_) => {
                    if input == b"testactivator" {
                        let passed = record.activator.is_some_and(|activator| {
                            self.evaluate_filter(target, activator, None, 0, &mut Vec::new())
                                .unwrap_or(false)
                        });
                        outputs.push((
                            if passed { b"OnPass" } else { b"OnFail" }.to_vec(),
                            Variant::Void,
                            record.activator,
                        ));
                    } else {
                        accepted = false;
                    }
                }
                BehaviorState::Trigger(mut state) => {
                    let mut write_state = true;
                    match input.as_slice() {
                        b"enable" => state.enabled = true,
                        b"disable" => state.enabled = false,
                        b"toggle" => state.enabled = !state.enabled,
                        b"disableandendtouch" => {
                            let contacts = state.contacts.clone();
                            for subject in contacts.into_iter().rev() {
                                self.end_contact(target, subject, record.producer_sequence, batch)?;
                            }
                            state = match &self.entity(target).expect("validated").behavior {
                                BehaviorState::Trigger(state) => state.clone(),
                                _ => unreachable!(),
                            };
                            state.enabled = false;
                        }
                        b"touchtest" => {
                            if state.enabled {
                                outputs.push((
                                    if state.contacts.is_empty() {
                                        b"OnNotTouching".as_slice()
                                    } else {
                                        b"OnTouching".as_slice()
                                    }
                                    .to_vec(),
                                    Variant::Void,
                                    Some(target),
                                ));
                            }
                        }
                        b"starttouch" => {
                            write_state = false;
                            if let Some(subject) = record.caller {
                                self.apply_contact(
                                    ContactRecord {
                                        trigger: target,
                                        subject,
                                        kind: ContactKind::Enter,
                                        external_filter_result: None,
                                        producer_sequence: record.producer_sequence,
                                    },
                                    batch,
                                )?;
                            }
                        }
                        b"endtouch" => {
                            write_state = false;
                            if let Some(subject) = record.caller {
                                self.end_contact(target, subject, record.producer_sequence, batch)?;
                            }
                        }
                        b"setdamage" if state.kind == TriggerKind::Hurt => {
                            write_state = false;
                            let Some(value) = record.value.as_float() else {
                                accepted = false;
                                self.push_transition(
                                    batch,
                                    Transition::Input {
                                        target,
                                        input: record.input.clone(),
                                        accepted,
                                        producer_sequence: record.producer_sequence,
                                    },
                                )?;
                                self.diagnostic(batch, DiagnosticCode::BadParameter, Some(target))?;
                                return Ok(());
                            };
                            if let BehaviorState::Trigger(current) =
                                &mut self.entity_mut(target).expect("validated").behavior
                            {
                                current.mutable_value_bits = value.to_bits();
                            }
                        }
                        _ => accepted = false,
                    }
                    if accepted && write_state {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Trigger(state);
                    }
                }
            }
        }

        self.push_transition(
            batch,
            Transition::Input {
                target,
                input: record.input.clone(),
                accepted,
                producer_sequence: record.producer_sequence,
            },
        )?;
        if !accepted {
            self.diagnostic(batch, DiagnosticCode::UnknownInput, Some(target))?;
            return Ok(());
        }
        if cancel_caller {
            self.cancel_caller(target, batch)?;
        }
        if let Some(parent) = parent {
            self.set_parent(parent, batch)?;
        }
        if let Some((position, opening)) = mover {
            if self.entity(target).is_some_and(|entity| {
                matches!(
                    entity.behavior,
                    BehaviorState::Mover(MoverState {
                        kind: MoverKind::Button,
                        ..
                    })
                )
            }) {
                self.fire_output(
                    target,
                    b"OnPressed",
                    Variant::Void,
                    record.activator,
                    Some(target),
                    0.0,
                    batch,
                )?;
            }
            self.start_mover(target, position, opening, record.activator, batch)?;
        }
        for (output, value, activator) in outputs {
            self.fire_output(target, &output, value, activator, target.into(), 0.0, batch)?;
        }
        if let Some(delay) = relay_refire_delay {
            self.schedule_event(
                EventTarget::Direct(target),
                b"EnableRefire".to_vec(),
                Variant::Void,
                delay,
                Some(target),
                Some(target),
                None,
                batch,
            )?;
        }
        if remove {
            self.mark_removal(target, batch)?;
        }
        Ok(())
    }

    fn counter_input(
        &mut self,
        target: EntityHandle,
        mut state: CounterState,
        input: &[u8],
        record: &InputRecord,
        outputs: &mut Vec<(Vec<u8>, Variant, Option<EntityHandle>)>,
    ) -> Result<bool, RuntimeFailure> {
        match input {
            b"enable" => state.enabled = true,
            b"disable" => state.enabled = false,
            b"getvalue" => outputs.push((
                b"OnGetValue".to_vec(),
                Variant::Float(state.value_bits),
                record.activator,
            )),
            b"add" | b"subtract" | b"multiply" | b"divide" | b"setvalue" | b"setvaluenofire"
            | b"sethitmax" | b"sethitmin" => {
                if !state.enabled {
                    self.entity_mut(target).expect("validated").behavior =
                        BehaviorState::Counter(state);
                    return Ok(true);
                }
                let Some(operand) = record.value.as_float().filter(|value| value.is_finite())
                else {
                    return Ok(false);
                };
                let mut value = f32::from_bits(state.value_bits);
                let mut min = f32::from_bits(state.min_bits);
                let mut max = f32::from_bits(state.max_bits);
                if input == b"sethitmax" {
                    max = operand;
                    if max < min {
                        min = max;
                    }
                    state.min_bits = min.to_bits();
                    state.max_bits = max.to_bits();
                } else if input == b"sethitmin" {
                    min = operand;
                    if max < min {
                        max = min;
                    }
                    state.min_bits = min.to_bits();
                    state.max_bits = max.to_bits();
                } else {
                    value = match input {
                        b"add" => value + operand,
                        b"subtract" => value - operand,
                        b"multiply" => value * operand,
                        b"divide" if operand != 0.0 => value / operand,
                        b"divide" => value,
                        b"setvalue" | b"setvaluenofire" => operand,
                        _ => unreachable!(),
                    };
                }
                if !value.is_finite() {
                    return Ok(false);
                }
                let clamped = min != 0.0 || max != 0.0;
                let silent = input == b"setvaluenofire";
                if clamped && !silent {
                    if value >= max {
                        if !state.hit_max {
                            state.hit_max = true;
                            outputs.push((b"OnHitMax".to_vec(), Variant::Void, record.activator));
                        }
                    } else {
                        state.hit_max = false;
                    }
                    if value <= min {
                        if !state.hit_min {
                            state.hit_min = true;
                            outputs.push((b"OnHitMin".to_vec(), Variant::Void, record.activator));
                        }
                    } else {
                        state.hit_min = false;
                    }
                    value = value.clamp(min, max);
                } else if clamped {
                    value = value.clamp(min, max);
                }
                state.value_bits = value.to_bits();
                if !silent {
                    outputs.push((
                        b"OutValue".to_vec(),
                        Variant::float(value),
                        record.activator,
                    ));
                }
            }
            _ => return Ok(false),
        }
        self.entity_mut(target).expect("validated").behavior = BehaviorState::Counter(state);
        Ok(true)
    }

    fn case_input(
        &mut self,
        target: EntityHandle,
        mut state: CaseState,
        input: &[u8],
        record: &InputRecord,
        outputs: &mut Vec<(Vec<u8>, Variant, Option<EntityHandle>)>,
    ) -> Result<bool, RuntimeFailure> {
        let selected = if input == b"invalue" {
            let value = record.value.as_string();
            state.cases.iter().position(|case| {
                case.as_deref()
                    .is_some_and(|case| case.eq_ignore_ascii_case(&value))
            })
        } else if input == b"pickrandom" {
            let connected = self.connected_cases(target);
            (!connected.is_empty()).then(|| {
                let at = self.random_index(connected.len());
                connected[at]
            })
        } else if input == b"pickrandomshuffle" {
            let mut avoid = None;
            if state.shuffle.is_empty() {
                state.shuffle = self.connected_cases(target);
                if state.shuffle.len() > 1 {
                    avoid = state.last_shuffle;
                }
            }
            if state.shuffle.is_empty() {
                None
            } else {
                let candidates: Vec<_> = state
                    .shuffle
                    .iter()
                    .enumerate()
                    .filter_map(|(at, case)| (Some(*case) != avoid).then_some(at))
                    .collect();
                let at = candidates[self.random_index(candidates.len())];
                let selected = state.shuffle.swap_remove(at);
                state.last_shuffle = Some(selected);
                Some(selected)
            }
        } else {
            return Ok(false);
        };
        if let Some(case) = selected {
            outputs.push((
                format!("OnCase{:02}", case + 1).into_bytes(),
                Variant::Void,
                record.activator,
            ));
        } else if input == b"invalue" {
            outputs.push((
                b"OnDefault".to_vec(),
                record.value.clone(),
                record.activator,
            ));
        }
        self.entity_mut(target).expect("validated").behavior = BehaviorState::Case(state);
        Ok(true)
    }

    fn connected_cases(&self, target: EntityHandle) -> Vec<usize> {
        (0..16)
            .filter(|index| {
                let name = format!("OnCase{:02}", index + 1);
                self.entity(target)
                    .is_some_and(|entity| output_connected(entity, name.as_bytes()))
            })
            .collect()
    }

    fn random_index(&mut self, count: usize) -> usize {
        let mut value = self.state.random_state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state.random_state = value;
        (value as usize) % count
    }

    fn max_output_delay(&self, caller: EntityHandle, output: &[u8]) -> f32 {
        self.entity(caller)
            .into_iter()
            .flat_map(|entity| &entity.outputs)
            .filter(|action| {
                action.remaining_fires != 0 && action.output.eq_ignore_ascii_case(output)
            })
            .map(|action| f32::from_bits(action.delay_bits))
            .fold(0.0, f32::max)
    }

    #[allow(clippy::too_many_arguments)]
    fn fire_output(
        &mut self,
        caller: EntityHandle,
        output: &[u8],
        value: Variant,
        activator: Option<EntityHandle>,
        event_caller: Option<EntityHandle>,
        call_delay: f32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let matching = self
            .entity(caller)
            .map(|entity| {
                entity
                    .outputs
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, action)| {
                        action.remaining_fires != 0 && action.output.eq_ignore_ascii_case(output)
                    })
                    .map(|(index, _)| index)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if self.state.queue.len() + matching.len() > self.config.limits.max_queued_events {
            return Err(failure(
                RuntimeFailureCode::QueueLimit,
                self.entity(caller).map(|entity| entity.source_index),
                self.config.limits.max_queued_events,
                self.state.queue.len() + matching.len(),
            ));
        }
        let mut actions = Vec::new();
        {
            let entity = self
                .entity_mut(caller)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            for index in matching {
                let action = &mut entity.outputs[index];
                let inherited = action.parameter.is_empty();
                actions.push((
                    action.id,
                    action.target.clone(),
                    action.input.clone(),
                    if inherited {
                        value.clone()
                    } else {
                        Variant::String(action.parameter.clone())
                    },
                    f32::from_bits(action.delay_bits) + if inherited { call_delay } else { 0.0 },
                ));
                if action.remaining_fires > 0 {
                    action.remaining_fires -= 1;
                }
            }
            entity.outputs.retain(|action| action.remaining_fires != 0);
        }
        for (action_id, target, input, value, delay) in actions {
            self.push_transition(
                batch,
                Transition::Output {
                    caller,
                    output: output.to_vec(),
                    action_id,
                },
            )?;
            self.schedule_event(
                EventTarget::Expression(target),
                input,
                value,
                delay,
                activator,
                event_caller,
                Some(action_id),
                batch,
            )?;
        }
        Ok(())
    }

    fn apply_contact(
        &mut self,
        record: ContactRecord,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(record.trigger) || !self.is_resolvable(record.subject) {
            self.push_transition(
                batch,
                Transition::Contact {
                    trigger: record.trigger,
                    subject: record.subject,
                    kind: record.kind,
                    accepted: false,
                    producer_sequence: record.producer_sequence,
                },
            )?;
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(record.trigger));
        }
        if record.kind == ContactKind::Exit {
            return self.end_contact(
                record.trigger,
                record.subject,
                record.producer_sequence,
                batch,
            );
        }
        let trigger_state = match &self.entity(record.trigger).expect("validated").behavior {
            BehaviorState::Trigger(state) => state.clone(),
            _ => {
                self.push_transition(
                    batch,
                    Transition::Contact {
                        trigger: record.trigger,
                        subject: record.subject,
                        kind: record.kind,
                        accepted: false,
                        producer_sequence: record.producer_sequence,
                    },
                )?;
                return self.diagnostic(
                    batch,
                    DiagnosticCode::FilterRejected,
                    Some(record.trigger),
                );
            }
        };
        let already = trigger_state.contacts.contains(&record.subject);
        if record.kind == ContactKind::Stay && !already {
            self.push_transition(
                batch,
                Transition::Contact {
                    trigger: record.trigger,
                    subject: record.subject,
                    kind: record.kind,
                    accepted: false,
                    producer_sequence: record.producer_sequence,
                },
            )?;
            return self.diagnostic(batch, DiagnosticCode::MissingContact, Some(record.trigger));
        }
        if record.kind == ContactKind::Enter && already {
            self.push_transition(
                batch,
                Transition::Contact {
                    trigger: record.trigger,
                    subject: record.subject,
                    kind: record.kind,
                    accepted: false,
                    producer_sequence: record.producer_sequence,
                },
            )?;
            return self.diagnostic(
                batch,
                DiagnosticCode::DuplicateContact,
                Some(record.trigger),
            );
        }
        if !trigger_state.enabled {
            self.push_transition(
                batch,
                Transition::Contact {
                    trigger: record.trigger,
                    subject: record.subject,
                    kind: record.kind,
                    accepted: false,
                    producer_sequence: record.producer_sequence,
                },
            )?;
            return Ok(());
        }
        let accepted = self.trigger_filter_result(
            &trigger_state,
            record.trigger,
            record.subject,
            record.external_filter_result,
            batch,
        )?;
        self.push_transition(
            batch,
            Transition::Contact {
                trigger: record.trigger,
                subject: record.subject,
                kind: record.kind,
                accepted,
                producer_sequence: record.producer_sequence,
            },
        )?;
        if !accepted {
            return Ok(());
        }
        if record.kind == ContactKind::Enter {
            let total_contacts: usize = self
                .state
                .slots
                .iter()
                .filter_map(|slot| slot.entity.as_ref())
                .filter_map(|entity| match &entity.behavior {
                    BehaviorState::Trigger(state) => Some(state.contacts.len()),
                    _ => None,
                })
                .sum();
            if total_contacts >= self.config.limits.max_trigger_contacts {
                return Err(failure(
                    RuntimeFailureCode::ContactLimit,
                    self.entity(record.trigger)
                        .map(|entity| entity.source_index),
                    self.config.limits.max_trigger_contacts,
                    total_contacts + 1,
                ));
            }
            let was_empty = trigger_state.contacts.is_empty();
            if let BehaviorState::Trigger(state) =
                &mut self.entity_mut(record.trigger).expect("validated").behavior
            {
                state.contacts.push(record.subject);
            }
            self.fire_output(
                record.trigger,
                b"OnStartTouch",
                Variant::Void,
                Some(record.subject),
                Some(record.trigger),
                0.0,
                batch,
            )?;
            if was_empty {
                self.fire_output(
                    record.trigger,
                    b"OnStartTouchAll",
                    Variant::Void,
                    Some(record.subject),
                    Some(record.trigger),
                    0.0,
                    batch,
                )?;
            }
        }
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::TriggerEffect {
                trigger: record.trigger,
                subject: record.subject,
                kind: trigger_state.kind,
                contact: record.kind,
            }),
        )?;
        if trigger_state.kind == TriggerKind::Multiple
            && self.state.current_tick >= trigger_state.next_fire_tick
        {
            self.fire_output(
                record.trigger,
                b"OnTrigger",
                Variant::Void,
                Some(record.subject),
                Some(record.trigger),
                0.0,
                batch,
            )?;
            let tick = self.state.current_tick;
            if let BehaviorState::Trigger(state) =
                &mut self.entity_mut(record.trigger).expect("validated").behavior
            {
                state.next_fire_tick = tick.saturating_add(state.wait_ticks);
            }
        }
        Ok(())
    }

    fn trigger_filter_result(
        &mut self,
        trigger: &TriggerState,
        trigger_handle: EntityHandle,
        subject: EntityHandle,
        external: Option<bool>,
        batch: &mut TransitionBatch,
    ) -> Result<bool, RuntimeFailure> {
        if external == Some(false) {
            return Ok(false);
        }
        let Some(filter_name) = trigger.filter_name.as_ref() else {
            return Ok(true);
        };
        let filters =
            self.resolve_target(filter_name, Some(trigger_handle), Some(subject), None)?;
        let Some(filter) = filters.first().copied() else {
            self.diagnostic(batch, DiagnosticCode::FilterRejected, Some(trigger_handle))?;
            return Ok(false);
        };
        match self.entity(filter).map(|entity| &entity.behavior) {
            Some(BehaviorState::Filter(FilterState {
                predicate: FilterPredicate::External,
                ..
            })) => match external {
                Some(result) => Ok(result),
                None => {
                    self.diagnostic(
                        batch,
                        DiagnosticCode::ExternalFilterRequired,
                        Some(trigger_handle),
                    )?;
                    Ok(false)
                }
            },
            Some(BehaviorState::Filter(_)) => {
                self.evaluate_filter(filter, subject, external, 0, &mut Vec::new())
            }
            _ => {
                self.diagnostic(batch, DiagnosticCode::FilterRejected, Some(trigger_handle))?;
                Ok(false)
            }
        }
    }

    fn evaluate_filter(
        &self,
        filter: EntityHandle,
        subject: EntityHandle,
        external: Option<bool>,
        depth: usize,
        stack: &mut Vec<EntityHandle>,
    ) -> Result<bool, RuntimeFailure> {
        if depth > self.config.limits.max_hierarchy_depth || stack.contains(&filter) {
            return Ok(false);
        }
        let Some(filter_entity) = self.entity(filter) else {
            return Ok(false);
        };
        let BehaviorState::Filter(state) = &filter_entity.behavior else {
            return Ok(false);
        };
        let Some(subject_entity) = self.entity(subject) else {
            return Ok(false);
        };
        stack.push(filter);
        let base = match &state.predicate {
            FilterPredicate::Name(name) if name.eq_ignore_ascii_case(b"!player") => {
                external.unwrap_or(false)
            }
            FilterPredicate::Name(name) => subject_entity
                .targetname
                .as_deref()
                .is_some_and(|target| wildcard_match(target, name)),
            FilterPredicate::Class(classname) => {
                wildcard_match(&subject_entity.classname, classname)
            }
            FilterPredicate::External => external.unwrap_or(false),
            FilterPredicate::All(children) => {
                let mut result = true;
                for name in children {
                    let Some(child) = self
                        .resolve_target(name, Some(filter), Some(subject), None)?
                        .first()
                        .copied()
                    else {
                        result = false;
                        break;
                    };
                    if !self.evaluate_filter(child, subject, external, depth + 1, stack)? {
                        result = false;
                        break;
                    }
                }
                result
            }
            FilterPredicate::Any(children) => {
                let mut result = false;
                for name in children {
                    let Some(child) = self
                        .resolve_target(name, Some(filter), Some(subject), None)?
                        .first()
                        .copied()
                    else {
                        continue;
                    };
                    if self.evaluate_filter(child, subject, external, depth + 1, stack)? {
                        result = true;
                        break;
                    }
                }
                result
            }
        };
        stack.pop();
        Ok(if state.negated { !base } else { base })
    }

    fn end_contact(
        &mut self,
        trigger: EntityHandle,
        subject: EntityHandle,
        producer_sequence: u64,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Trigger(state)) =
            self.entity(trigger).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(trigger));
        };
        let Some(at) = state
            .contacts
            .iter()
            .position(|contact| *contact == subject)
        else {
            self.push_transition(
                batch,
                Transition::Contact {
                    trigger,
                    subject,
                    kind: ContactKind::Exit,
                    accepted: false,
                    producer_sequence,
                },
            )?;
            return self.diagnostic(batch, DiagnosticCode::MissingContact, Some(trigger));
        };
        let becomes_empty = state.contacts.len() == 1;
        if let BehaviorState::Trigger(state) =
            &mut self.entity_mut(trigger).expect("validated").behavior
        {
            state.contacts.remove(at);
        }
        self.push_transition(
            batch,
            Transition::Contact {
                trigger,
                subject,
                kind: ContactKind::Exit,
                accepted: true,
                producer_sequence,
            },
        )?;
        self.fire_output(
            trigger,
            b"OnEndTouch",
            Variant::Void,
            Some(subject),
            Some(trigger),
            0.0,
            batch,
        )?;
        if becomes_empty {
            self.fire_output(
                trigger,
                b"OnEndTouchAll",
                Variant::Void,
                Some(subject),
                Some(trigger),
                0.0,
                batch,
            )?;
        }
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::TriggerEffect {
                trigger,
                subject,
                kind: state.kind,
                contact: ContactKind::Exit,
            }),
        )
    }

    fn start_mover(
        &mut self,
        entity: EntityHandle,
        position: f32,
        opening: bool,
        activator: Option<EntityHandle>,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (kind, speed, local, parent, attachment) = {
            let entity_state = self
                .entity(entity)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            let BehaviorState::Mover(mover) = &entity_state.behavior else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            (
                mover.kind,
                mover.speed,
                lerp(mover.closed, mover.open, position),
                entity_state.parent,
                entity_state.parent_attachment.clone(),
            )
        };
        let basis = match parent {
            Some(parent) => self.parent_basis(parent, attachment.as_deref())?,
            None => Transform::IDENTITY,
        };
        let world = if parent.is_some() {
            compose_transform(
                basis,
                Transform {
                    origin: local,
                    angles: self
                        .entity(entity)
                        .expect("validated")
                        .local_transform
                        .angles,
                },
            )
            .origin
        } else {
            local
        };
        let request_id = self.state.next_mover_request_id;
        self.state.next_mover_request_id += 1;
        if let BehaviorState::Mover(mover) =
            &mut self.entity_mut(entity).expect("validated").behavior
        {
            mover.pending = Some(PendingMove {
                request_id,
                local_destination: local,
                world_destination: world,
                opening,
                activator,
            });
            mover.position = if opening {
                MoverPosition::Opening
            } else {
                MoverPosition::Closing
            };
        }
        if kind == MoverKind::Door {
            self.fire_output(
                entity,
                if opening { b"OnOpen" } else { b"OnClose" },
                Variant::Void,
                Some(entity),
                Some(entity),
                0.0,
                batch,
            )?;
        }
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::Mover {
                request_id,
                entity,
                kind,
                local_destination: local,
                world_destination: world,
                speed,
                opening,
            }),
        )
    }

    fn complete_mover(
        &mut self,
        entity: EntityHandle,
        request_id: u64,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Mover(mover)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
        };
        let Some(pending) = mover
            .pending
            .filter(|pending| pending.request_id == request_id)
        else {
            return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
        };
        if let Some(entity_state) = self.entity_mut(entity) {
            entity_state.local_transform.origin = pending.local_destination;
            if let BehaviorState::Mover(state) = &mut entity_state.behavior {
                state.pending = None;
                state.position = if pending.opening {
                    MoverPosition::Open
                } else {
                    MoverPosition::Closed
                };
            }
        }
        self.recompute_subtree(entity, 0)?;
        let transformed = self.entity(entity).expect("validated");
        self.push_transition(
            batch,
            Transition::TransformChanged {
                entity,
                local: transformed.local_transform,
                world: transformed.world_transform,
            },
        )?;
        let endpoint_open = pending.opening ^ mover.outputs_reversed;
        let output = match (mover.kind, endpoint_open) {
            (MoverKind::Button, true) => b"OnIn".as_slice(),
            (MoverKind::Button, false) => b"OnOut".as_slice(),
            (_, true) => b"OnFullyOpen".as_slice(),
            (_, false) => b"OnFullyClosed".as_slice(),
        };
        self.fire_output(
            entity,
            output,
            Variant::Void,
            pending.activator.or(Some(entity)),
            Some(entity),
            0.0,
            batch,
        )?;
        if pending.opening
            && matches!(mover.kind, MoverKind::Button | MoverKind::Door)
            && !mover.no_auto_return
            && let Some(wait) = mover.wait_ticks
        {
            self.schedule_event_ticks(
                EventTarget::Direct(entity),
                if mover.kind == MoverKind::Button {
                    b"PressOut".to_vec()
                } else {
                    b"Close".to_vec()
                },
                Variant::Void,
                wait,
                pending.activator.or(Some(entity)),
                Some(entity),
                None,
                batch,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_event_ticks(
        &mut self,
        target: EventTarget,
        input: Vec<u8>,
        value: Variant,
        delay_ticks: u64,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
        output_action: Option<u64>,
        batch: &mut TransitionBatch,
    ) -> Result<u64, RuntimeFailure> {
        let event_id = self.schedule_event(
            target,
            input,
            value,
            0.0,
            activator,
            caller,
            output_action,
            batch,
        )?;
        if let Some(event) = self
            .state
            .queue
            .iter_mut()
            .find(|event| event.id == event_id)
        {
            event.due_tick = self.state.current_tick.saturating_add(delay_ticks);
        }
        if let Some(record) = batch.records.iter_mut().rev().find(|record| {
            matches!(record.transition, Transition::Scheduled { event_id: id, .. } if id == event_id)
        }) {
            record.transition = Transition::Scheduled {
                event_id,
                due_tick: self.state.current_tick.saturating_add(delay_ticks),
            };
        }
        self.state
            .queue
            .sort_by_key(|event| (event.due_tick, event.enqueue_sequence));
        Ok(event_id)
    }

    fn block_mover(
        &mut self,
        entity: EntityHandle,
        request_id: u64,
        blocker: EntityHandle,
        kind: BlockContactKind,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Mover(mover)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
        };
        let Some(pending) = mover
            .pending
            .filter(|pending| pending.request_id == request_id)
        else {
            return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
        };
        if f32::from_bits(mover.block_damage_bits) != 0.0 && kind != BlockContactKind::End {
            self.push_transition(
                batch,
                Transition::Request(RuntimeRequest::BlockDamage {
                    mover: entity,
                    blocker,
                    damage_bits: mover.block_damage_bits,
                }),
            )?;
        }
        if mover.kind == MoverKind::Door {
            match kind {
                BlockContactKind::Start => self.fire_output(
                    entity,
                    if pending.opening {
                        b"OnBlockedOpening"
                    } else {
                        b"OnBlockedClosing"
                    },
                    Variant::Void,
                    Some(blocker),
                    Some(entity),
                    0.0,
                    batch,
                )?,
                BlockContactKind::End => self.fire_output(
                    entity,
                    if pending.opening {
                        b"OnUnblockedOpening"
                    } else {
                        b"OnUnblockedClosing"
                    },
                    Variant::Void,
                    Some(entity),
                    Some(entity),
                    0.0,
                    batch,
                )?,
                BlockContactKind::Stay if !mover.force_closed && mover.wait_ticks.is_some() => {
                    self.start_mover(
                        entity,
                        if pending.opening { 0.0 } else { 1.0 },
                        !pending.opening,
                        pending.activator,
                        batch,
                    )?;
                }
                BlockContactKind::Stay => {}
            }
        }
        Ok(())
    }

    fn mark_removal(
        &mut self,
        handle: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(handle) {
            return Ok(());
        }
        let descendants = self.removal_order(handle)?;
        for removing in descendants {
            let touching: Vec<_> = self
                .state
                .creation_order
                .iter()
                .copied()
                .filter(|trigger| {
                    matches!(
                        self.entity(*trigger).map(|entity| &entity.behavior),
                        Some(BehaviorState::Trigger(state)) if state.contacts.contains(&removing)
                    )
                })
                .collect();
            for trigger in touching {
                self.end_contact(
                    trigger,
                    removing,
                    self.state.next_transition_sequence,
                    batch,
                )?;
            }
            if let Some(BehaviorState::Trigger(state)) =
                self.entity(removing).map(|entity| entity.behavior.clone())
            {
                for subject in state.contacts.into_iter().rev() {
                    self.end_contact(
                        removing,
                        subject,
                        self.state.next_transition_sequence,
                        batch,
                    )?;
                }
            }
            let mut cancelled = Vec::new();
            self.state.queue.retain(|event| {
                let internal = matches!(event.target, EventTarget::Direct(target) if target == removing)
                    && event.caller == Some(removing);
                if internal {
                    cancelled.push(event.id);
                }
                !internal
            });
            for event_id in cancelled {
                self.push_transition(batch, Transition::Cancelled { event_id })?;
            }
            self.index_remove(removing);
            if let Some(entity) = self.entity_mut(removing) {
                entity.lifecycle = Lifecycle::PendingRemoval;
            }
            self.push_transition(
                batch,
                Transition::Lifecycle {
                    entity: removing,
                    state: Lifecycle::PendingRemoval,
                },
            )?;
        }
        Ok(())
    }

    fn removal_order(&self, root: EntityHandle) -> Result<Vec<EntityHandle>, RuntimeFailure> {
        fn append(
            world: &EntityWorld,
            handle: EntityHandle,
            depth: usize,
            output: &mut Vec<EntityHandle>,
        ) -> Result<(), RuntimeFailure> {
            if depth > world.config.limits.max_hierarchy_depth {
                return Err(failure(
                    RuntimeFailureCode::HierarchyLimit,
                    world.entity(handle).map(|entity| entity.source_index),
                    world.config.limits.max_hierarchy_depth,
                    depth,
                ));
            }
            output.push(handle);
            if let Some(entity) = world.entity(handle) {
                for child in &entity.children {
                    append(world, *child, depth + 1, output)?;
                }
            }
            Ok(())
        }
        let mut output = Vec::new();
        append(self, root, 0, &mut output)?;
        Ok(output)
    }

    fn commit_removals(&mut self) {
        let removed: Vec<_> = self
            .state
            .creation_order
            .iter()
            .copied()
            .filter(|handle| {
                self.entity(*handle)
                    .is_some_and(|entity| entity.lifecycle == Lifecycle::PendingRemoval)
            })
            .collect();
        for handle in &removed {
            if let Some(parent) = self.entity(*handle).and_then(|entity| entity.parent)
                && let Some(parent) = self.entity_mut(parent)
            {
                parent.children.retain(|child| child != handle);
            }
        }
        for handle in &removed {
            if let Some(slot) = self.state.slots.get_mut(usize::from(handle.slot)) {
                slot.entity = None;
                slot.generation = slot.generation.wrapping_add(1).max(1);
            }
        }
        self.state
            .creation_order
            .retain(|handle| !removed.contains(handle));
    }
}

fn validate_config(config: &EntityWorldConfig) -> Result<(), RuntimeFailure> {
    if !config.tick_interval.is_finite()
        || config.tick_interval <= 0.0
        || config.limits.max_entities > MAX_LIVE_ENTITIES
        || config.limits.max_entities == 0
        || config.limits.max_hierarchy_depth == 0
        || config.limits.max_queued_events == 0
        || config.limits.max_events_per_tick == 0
        || config.limits.max_transitions_per_phase == 0
        || config.limits.max_snapshot_bytes == 0
    {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            0,
            0,
        ));
    }
    Ok(())
}

fn failure(
    code: RuntimeFailureCode,
    entity: Option<usize>,
    limit: usize,
    actual: usize,
) -> RuntimeFailure {
    RuntimeFailure {
        code,
        entity,
        limit: Some(limit),
        actual: Some(actual),
    }
}

fn ascii_key(value: &[u8]) -> Vec<u8> {
    value.iter().map(u8::to_ascii_lowercase).collect()
}

fn parse_i32(value: &[u8]) -> Option<i32> {
    std::str::from_utf8(value).ok()?.trim().parse().ok()
}

fn parse_f32(value: &[u8]) -> Option<f32> {
    let value: f32 = std::str::from_utf8(value).ok()?.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

fn parse_vector(value: &[u8]) -> Option<[f32; 3]> {
    let values = std::str::from_utf8(value)
        .ok()?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2]])
}

fn field<'a>(entity: &'a Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn field_i32(entity: &Entity, key: &[u8], default: i32) -> Result<i32, RuntimeFailure> {
    match field(entity, key) {
        Some(value) => parse_i32(value)
            .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, Some(entity.index), 0, 0)),
        None => Ok(default),
    }
}

fn field_f32(entity: &Entity, key: &[u8], default: f32) -> Result<f32, RuntimeFailure> {
    match field(entity, key) {
        Some(value) => parse_f32(value)
            .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, Some(entity.index), 0, 0)),
        None => Ok(default),
    }
}

fn field_vector(
    entity: &Entity,
    key: &[u8],
    default: [f32; 3],
) -> Result<[f32; 3], RuntimeFailure> {
    match field(entity, key) {
        Some(value) => parse_vector(value)
            .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, Some(entity.index), 0, 0)),
        None => Ok(default),
    }
}

fn delay_ticks(delay: f32, tick_interval: f32) -> u64 {
    if delay <= 0.0 {
        0
    } else {
        (f64::from(delay) / f64::from(tick_interval)).ceil() as u64
    }
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

fn lerp(a: [f32; 3], b: [f32; 3], value: f32) -> [f32; 3] {
    add(a, scale(sub(b, a), value))
}

fn dot_abs(direction: [f32; 3], size: [f32; 3]) -> f32 {
    direction[0].abs() * size[0] + direction[1].abs() * size[1] + direction[2].abs() * size[2]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn ascii_starts_with(value: &[u8], prefix: &[u8]) -> bool {
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn wildcard_match(value: &[u8], pattern: &[u8]) -> bool {
    if let Some(prefix) = pattern.strip_suffix(b"*") {
        !prefix.contains(&b'*') && ascii_starts_with(value, prefix)
    } else {
        value.eq_ignore_ascii_case(pattern)
    }
}

fn output_connected(entity: &RuntimeEntity, output: &[u8]) -> bool {
    entity
        .outputs
        .iter()
        .any(|action| action.remaining_fires != 0 && action.output.eq_ignore_ascii_case(output))
}

fn quat_from_angles(angles: [f32; 3]) -> [f64; 4] {
    let pitch = f64::from(angles[0]).to_radians() * 0.5;
    let yaw = f64::from(angles[1]).to_radians() * 0.5;
    let roll = f64::from(angles[2]).to_radians() * 0.5;
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let (sr, cr) = roll.sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

fn quat_mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quat_inverse(value: [f64; 4]) -> [f64; 4] {
    [-value[0], -value[1], -value[2], value[3]]
}

fn quat_rotate(quat: [f64; 4], value: [f32; 3]) -> [f32; 3] {
    let vector = [
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
        0.0,
    ];
    let result = quat_mul(quat_mul(quat, vector), quat_inverse(quat));
    [result[0] as f32, result[1] as f32, result[2] as f32]
}

fn angles_from_quat(quat: [f64; 4]) -> [f32; 3] {
    let sin_roll_cos_pitch = 2.0 * (quat[3] * quat[0] + quat[1] * quat[2]);
    let cos_roll_cos_pitch = 1.0 - 2.0 * (quat[0] * quat[0] + quat[1] * quat[1]);
    let roll = sin_roll_cos_pitch.atan2(cos_roll_cos_pitch);
    let sin_pitch = 2.0 * (quat[3] * quat[1] - quat[2] * quat[0]);
    let pitch = if sin_pitch.abs() >= 1.0 {
        std::f64::consts::FRAC_PI_2.copysign(sin_pitch)
    } else {
        sin_pitch.asin()
    };
    let sin_yaw_cos_pitch = 2.0 * (quat[3] * quat[2] + quat[0] * quat[1]);
    let cos_yaw_cos_pitch = 1.0 - 2.0 * (quat[1] * quat[1] + quat[2] * quat[2]);
    let yaw = sin_yaw_cos_pitch.atan2(cos_yaw_cos_pitch);
    [
        pitch.to_degrees() as f32,
        yaw.to_degrees() as f32,
        roll.to_degrees() as f32,
    ]
}

fn compose_transform(parent: Transform, local: Transform) -> Transform {
    if parent.angles == [0.0; 3] && local.angles == [0.0; 3] {
        return Transform {
            origin: add(parent.origin, local.origin),
            angles: [0.0; 3],
        };
    }
    let parent_rotation = quat_from_angles(parent.angles);
    Transform {
        origin: add(parent.origin, quat_rotate(parent_rotation, local.origin)),
        angles: angles_from_quat(quat_mul(parent_rotation, quat_from_angles(local.angles))),
    }
}

fn relative_transform(parent: Transform, world: Transform) -> Transform {
    if parent.angles == [0.0; 3] && world.angles == [0.0; 3] {
        return Transform {
            origin: sub(world.origin, parent.origin),
            angles: [0.0; 3],
        };
    }
    let inverse = quat_inverse(quat_from_angles(parent.angles));
    Transform {
        origin: quat_rotate(inverse, sub(world.origin, parent.origin)),
        angles: angles_from_quat(quat_mul(inverse, quat_from_angles(world.angles))),
    }
}

struct SnapshotWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl SnapshotWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), RuntimeFailure> {
        let actual = self.bytes.len().saturating_add(value.len());
        if actual > self.limit {
            return Err(failure(
                RuntimeFailureCode::SnapshotLimit,
                None,
                self.limit,
                actual,
            ));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), RuntimeFailure> {
        self.extend(&[value])
    }

    fn bool(&mut self, value: bool) -> Result<(), RuntimeFailure> {
        self.u8(u8::from(value))
    }

    fn u32(&mut self, value: u32) -> Result<(), RuntimeFailure> {
        self.extend(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), RuntimeFailure> {
        self.extend(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), RuntimeFailure> {
        self.extend(&value.to_le_bytes())
    }

    fn usize(&mut self, value: usize) -> Result<(), RuntimeFailure> {
        self.u64(value as u64)
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), RuntimeFailure> {
        self.usize(value.len())?;
        self.extend(value)
    }

    fn optional_bytes(&mut self, value: Option<&[u8]>) -> Result<(), RuntimeFailure> {
        self.bool(value.is_some())?;
        if let Some(value) = value {
            self.bytes(value)?;
        }
        Ok(())
    }

    fn handle(&mut self, handle: EntityHandle) -> Result<(), RuntimeFailure> {
        self.u32(u32::from(handle.slot))?;
        self.u32(handle.generation)
    }

    fn optional_handle(&mut self, handle: Option<EntityHandle>) -> Result<(), RuntimeFailure> {
        self.bool(handle.is_some())?;
        if let Some(handle) = handle {
            self.handle(handle)?;
        }
        Ok(())
    }

    fn transform(&mut self, transform: Transform) -> Result<(), RuntimeFailure> {
        for value in transform.origin.into_iter().chain(transform.angles) {
            self.u32(value.to_bits())?;
        }
        Ok(())
    }

    fn variant(&mut self, value: &Variant) -> Result<(), RuntimeFailure> {
        match value {
            Variant::Void => self.u8(0),
            Variant::Bool(value) => {
                self.u8(1)?;
                self.bool(*value)
            }
            Variant::Integer(value) => {
                self.u8(2)?;
                self.i32(*value)
            }
            Variant::Float(value) => {
                self.u8(3)?;
                self.u32(*value)
            }
            Variant::String(value) => {
                self.u8(4)?;
                self.bytes(value)
            }
            Variant::Vector(value) => {
                self.u8(5)?;
                for bits in value {
                    self.u32(*bits)?;
                }
                Ok(())
            }
            Variant::Handle(value) => {
                self.u8(6)?;
                self.handle(*value)
            }
        }
    }
}

fn encode_state(state: &WorldState, config: &EntityWorldConfig) -> Result<Vec<u8>, RuntimeFailure> {
    let mut output = SnapshotWriter::new(config.limits.max_snapshot_bytes);
    output.extend(b"PSEN")?;
    output.u32(1)?;
    output.u64(config.source_identity)?;
    output.u64(config.registry_identity)?;
    output.u32(config.tick_interval.to_bits())?;
    output.u8(match config.load_kind {
        MapLoadKind::NewGame => 0,
        MapLoadKind::LoadGame => 1,
        MapLoadKind::Transition => 2,
        MapLoadKind::Background => 3,
        MapLoadKind::MultiplayerNewMap => 4,
        MapLoadKind::MultiplayerNewRound => 5,
    })?;
    output.u64(state.current_tick)?;
    output.u64(state.next_creation_order)?;
    output.u64(state.next_output_id)?;
    output.u64(state.next_event_id)?;
    output.u64(state.next_enqueue_sequence)?;
    output.u64(state.next_transition_sequence)?;
    output.u64(state.next_mover_request_id)?;
    output.usize(state.diagnostics)?;
    output.u64(state.random_state)?;
    output.usize(state.slots.len())?;
    for slot in &state.slots {
        output.u32(slot.generation)?;
        output.bool(slot.entity.is_some())?;
        if let Some(entity) = &slot.entity {
            encode_entity(&mut output, entity)?;
        }
    }
    output.usize(state.creation_order.len())?;
    for handle in &state.creation_order {
        output.handle(*handle)?;
    }
    encode_index(&mut output, &state.targetname_index)?;
    encode_index(&mut output, &state.classname_index)?;
    output.usize(state.queue.len())?;
    for event in &state.queue {
        output.u64(event.id)?;
        output.u64(event.due_tick)?;
        output.u64(event.enqueue_sequence)?;
        match &event.target {
            EventTarget::Expression(value) => {
                output.u8(0)?;
                output.bytes(value)?;
            }
            EventTarget::Direct(value) => {
                output.u8(1)?;
                output.handle(*value)?;
            }
        }
        output.bytes(&event.input)?;
        output.variant(&event.value)?;
        output.optional_handle(event.activator)?;
        output.optional_handle(event.caller)?;
        output.optional_bytes(event.caller_name.as_deref())?;
        output.optional_bytes(event.caller_class.as_deref())?;
        output.bool(event.output_action.is_some())?;
        if let Some(value) = event.output_action {
            output.u64(value)?;
        }
    }
    Ok(output.bytes)
}

fn encode_index(
    output: &mut SnapshotWriter,
    index: &BTreeMap<Vec<u8>, Vec<EntityHandle>>,
) -> Result<(), RuntimeFailure> {
    output.usize(index.len())?;
    for (key, values) in index {
        output.bytes(key)?;
        output.usize(values.len())?;
        for handle in values {
            output.handle(*handle)?;
        }
    }
    Ok(())
}

fn encode_entity(
    output: &mut SnapshotWriter,
    entity: &RuntimeEntity,
) -> Result<(), RuntimeFailure> {
    output.handle(entity.handle)?;
    output.usize(entity.source_index)?;
    output.bytes(&entity.classname)?;
    output.optional_bytes(entity.targetname.as_deref())?;
    output.u8(match entity.lifecycle {
        Lifecycle::Created => 0,
        Lifecycle::Spawned => 1,
        Lifecycle::Activated => 2,
        Lifecycle::PendingRemoval => 3,
    })?;
    output.u8(match entity.coverage {
        Coverage::Handled => 0,
        Coverage::IntentionallyInert => 1,
        Coverage::Unsupported => 2,
        Coverage::Unknown => 3,
        Coverage::Malformed => 4,
    })?;
    output.transform(entity.local_transform)?;
    output.transform(entity.world_transform)?;
    output.optional_handle(entity.parent)?;
    output.optional_bytes(entity.parent_attachment.as_deref())?;
    output.usize(entity.children.len())?;
    for child in &entity.children {
        output.handle(*child)?;
    }
    output.usize(entity.attachments.len())?;
    for (name, transform) in &entity.attachments {
        output.bytes(name)?;
        output.transform(*transform)?;
    }
    output.usize(entity.outputs.len())?;
    for action in &entity.outputs {
        output.u64(action.id)?;
        output.usize(action.declaration_order)?;
        output.bytes(&action.output)?;
        output.bytes(&action.target)?;
        output.bytes(&action.input)?;
        output.bytes(&action.parameter)?;
        output.u32(action.delay_bits)?;
        output.i32(action.remaining_fires)?;
    }
    output.usize(entity.malformed_outputs.len())?;
    for (order, error) in &entity.malformed_outputs {
        output.usize(*order)?;
        output.u8(match error {
            ConnectionError::FieldCount => 0,
            ConnectionError::FieldLimit => 1,
            ConnectionError::EmptyTarget => 2,
            ConnectionError::EmptyInput => 3,
            ConnectionError::InvalidDelay => 4,
            ConnectionError::InvalidMaxFires => 5,
        })?;
    }
    output.usize(entity.definition.pairs.len())?;
    for pair in &entity.definition.pairs {
        output.bytes(&pair.key)?;
        output.bytes(&pair.value)?;
        output.usize(pair.key_range.start)?;
        output.usize(pair.key_range.end)?;
        output.usize(pair.value_range.start)?;
        output.usize(pair.value_range.end)?;
    }
    encode_behavior(output, &entity.behavior)
}

fn encode_behavior(
    output: &mut SnapshotWriter,
    behavior: &BehaviorState,
) -> Result<(), RuntimeFailure> {
    match behavior {
        BehaviorState::Inert => output.u8(0),
        BehaviorState::Brush(state) => {
            output.u8(1)?;
            output.bool(state.enabled)?;
            output.u8(match state.solidity {
                BrushSolidity::Toggle => 0,
                BrushSolidity::Never => 1,
                BrushSolidity::Always => 2,
            })
        }
        BehaviorState::Mover(state) => {
            output.u8(2)?;
            output.u8(match state.kind {
                MoverKind::Button => 0,
                MoverKind::Door => 1,
                MoverKind::Linear => 2,
            })?;
            for value in state.closed.into_iter().chain(state.open) {
                output.u32(value.to_bits())?;
            }
            output.u32(state.speed.to_bits())?;
            output.bool(state.wait_ticks.is_some())?;
            if let Some(wait) = state.wait_ticks {
                output.u64(wait)?;
            }
            match state.position {
                MoverPosition::Closed => output.u8(0)?,
                MoverPosition::Opening => output.u8(1)?,
                MoverPosition::Open => output.u8(2)?,
                MoverPosition::Closing => output.u8(3)?,
                MoverPosition::Positioned(value) => {
                    output.u8(4)?;
                    output.u32(value)?;
                }
            }
            output.bool(state.pending.is_some())?;
            if let Some(pending) = &state.pending {
                output.u64(pending.request_id)?;
                for value in pending
                    .local_destination
                    .into_iter()
                    .chain(pending.world_destination)
                {
                    output.u32(value.to_bits())?;
                }
                output.bool(pending.opening)?;
                output.optional_handle(pending.activator)?;
            }
            output.bool(state.locked)?;
            output.bool(state.toggle)?;
            output.bool(state.force_closed)?;
            output.bool(state.no_auto_return)?;
            output.bool(state.outputs_reversed)?;
            output.u32(state.block_damage_bits)
        }
        BehaviorState::LogicAuto => output.u8(3),
        BehaviorState::Relay(state) => {
            output.u8(4)?;
            output.bool(state.enabled)?;
            output.bool(state.waiting_for_refire)?;
            output.bool(state.remove_on_fire)?;
            output.bool(state.fast_retrigger)
        }
        BehaviorState::Counter(state) => {
            output.u8(5)?;
            output.u32(state.value_bits)?;
            output.u32(state.min_bits)?;
            output.u32(state.max_bits)?;
            output.bool(state.hit_min)?;
            output.bool(state.hit_max)?;
            output.bool(state.enabled)
        }
        BehaviorState::Case(state) => {
            output.u8(6)?;
            output.usize(state.cases.len())?;
            for case in &state.cases {
                output.optional_bytes(case.as_deref())?;
            }
            output.usize(state.shuffle.len())?;
            for case in &state.shuffle {
                output.usize(*case)?;
            }
            output.bool(state.last_shuffle.is_some())?;
            if let Some(case) = state.last_shuffle {
                output.usize(case)?;
            }
            Ok(())
        }
        BehaviorState::Filter(state) => {
            output.u8(7)?;
            output.bool(state.negated)?;
            match &state.predicate {
                FilterPredicate::Name(value) => {
                    output.u8(0)?;
                    output.bytes(value)
                }
                FilterPredicate::Class(value) => {
                    output.u8(1)?;
                    output.bytes(value)
                }
                FilterPredicate::All(values) | FilterPredicate::Any(values) => {
                    output.u8(if matches!(state.predicate, FilterPredicate::All(_)) {
                        2
                    } else {
                        3
                    })?;
                    output.usize(values.len())?;
                    for value in values {
                        output.bytes(value)?;
                    }
                    Ok(())
                }
                FilterPredicate::External => output.u8(4),
            }
        }
        BehaviorState::Trigger(state) => {
            output.u8(8)?;
            output.u8(match state.kind {
                TriggerKind::Multiple => 0,
                TriggerKind::Hurt => 1,
                TriggerKind::Push => 2,
                TriggerKind::Catapult => 3,
                TriggerKind::Teleport => 4,
            })?;
            output.bool(state.enabled)?;
            output.optional_bytes(state.filter_name.as_deref())?;
            output.usize(state.contacts.len())?;
            for contact in &state.contacts {
                output.handle(*contact)?;
            }
            output.u64(state.wait_ticks)?;
            output.u64(state.next_fire_tick)?;
            output.u32(state.mutable_value_bits)
        }
    }
}
