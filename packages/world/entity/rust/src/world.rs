use std::{collections::BTreeMap, sync::Arc};

use super::{
    Connection, ConnectionError, Entity, FieldType, Graph, Pair, ValueConversionError,
    source_random::SourceRandom,
    value::{convert_value, project_string, source_float, source_integer, source_vector},
};

const MAX_LIVE_ENTITIES: usize = 8_192;
const BUTTON_TOGGLE: i32 = 32;
const BUTTON_DAMAGE_ACTIVATES: i32 = 512;
const BUTTON_LOCKED: i32 = 2_048;
const BUTTON_DONT_MOVE: i32 = 1;
const DOOR_START_OPEN: i32 = 1;
const DOOR_NO_AUTO_RETURN: i32 = 32;
const DOOR_LOCKED: i32 = 2_048;
const RELAY_REMOVE_ON_FIRE: i32 = 1;
const RELAY_FAST_RETRIGGER: i32 = 2;
const EF_NOSHADOW: u16 = 0x010;
const EF_NODRAW: u16 = 0x020;
const RENDER_MODE_NONE: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EntityHandle {
    pub slot: u16,
    pub generation: u32,
}

impl EntityHandle {
    pub const NULL: Self = Self {
        slot: u16::MAX,
        generation: 0,
    };
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
    pub max_template_instances: usize,
    pub max_template_members: usize,
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
            max_template_instances: 65_536,
            max_template_members: 8_192,
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
    pub external_classes: Vec<ExternalClassBinding>,
    pub external_brush_models: Vec<ExternalBrushModelBinding>,
    pub field_bindings: Vec<ClassFieldBinding>,
    pub initial_attachments: Vec<InitialAttachmentBinding>,
    pub pickup_classes: Vec<Vec<u8>>,
    pub spawn_priorities: Vec<ClassSpawnPriority>,
    pub class_dispositions: Vec<ClassDispositionBinding>,
    pub limits: RuntimeLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassFieldBinding {
    pub classname: Vec<u8>,
    pub fields: Vec<FieldBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldBinding {
    pub key: Vec<u8>,
    pub field_type: FieldType,
    pub writable_input: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InitialAttachmentBinding {
    pub parent_source_index: usize,
    pub attachment: Vec<u8>,
    pub parent_space_transform: Transform,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassSpawnPriority {
    pub classname: Vec<u8>,
    pub priority: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassDispositionBinding {
    pub classname: Vec<u8>,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalClassBinding {
    pub classname: Vec<u8>,
    pub inputs: Vec<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalBrushModelVisibility {
    BaseEntity,
    Hidden,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalBrushModelBinding {
    pub classname: Vec<u8>,
    pub initial_visibility: ExternalBrushModelVisibility,
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
            external_classes: Vec::new(),
            external_brush_models: Vec::new(),
            field_bindings: Vec::new(),
            initial_attachments: Vec::new(),
            pickup_classes: Vec::new(),
            spawn_priorities: Vec::new(),
            class_dispositions: Vec::new(),
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
    Missing,
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
    PositionVector([u32; 3]),
    Handle(EntityHandle),
    Color([u8; 4]),
}

impl Variant {
    pub fn float(value: f32) -> Self {
        Self::Float(value.to_bits())
    }

    pub fn vector(value: [f32; 3]) -> Self {
        Self::Vector(value.map(f32::to_bits))
    }

    pub fn position_vector(value: [f32; 3]) -> Self {
        Self::PositionVector(value.map(f32::to_bits))
    }

    pub fn field_type(&self) -> FieldType {
        match self {
            Self::Void => FieldType::Void,
            Self::Bool(_) => FieldType::Boolean,
            Self::Integer(_) => FieldType::Integer,
            Self::Float(_) => FieldType::Float,
            Self::String(_) => FieldType::String,
            Self::Vector(_) => FieldType::Vector,
            Self::PositionVector(_) => FieldType::PositionVector,
            Self::Handle(_) => FieldType::Handle,
            Self::Color(_) => FieldType::Color,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Integer(value) => Some(*value as f32),
            Self::Float(bits) => Some(f32::from_bits(*bits)),
            Self::String(value) => {
                let value = source_float(value);
                value.is_finite().then_some(value)
            }
            _ => None,
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
pub enum MoverClass {
    Button,
    RotatingButton,
    MomentaryRotatingButton,
    Door,
    RotatingDoor,
    Linear,
    Rotating,
    Platform,
    RotatingPlatform,
    Train,
    TrackTrain,
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
    pub local_angles_destination: [f32; 3],
    pub world_angles_destination: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub continuous: bool,
    pub path_destination: Option<EntityHandle>,
    pub opening: bool,
    pub activator: Option<EntityHandle>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MoverState {
    pub kind: MoverKind,
    pub class: MoverClass,
    pub closed: [f32; 3],
    pub open: [f32; 3],
    pub closed_angles: [f32; 3],
    pub open_angles: [f32; 3],
    pub speed: f32,
    pub wait_ticks: Option<u64>,
    pub position: MoverPosition,
    pub pending: Option<PendingMove>,
    pub locked: bool,
    pub toggle: bool,
    pub force_closed: bool,
    pub no_auto_return: bool,
    pub stay_pushed: bool,
    pub outputs_reversed: bool,
    pub block_damage_bits: u32,
    pub damage_activates: bool,
    pub dont_move: bool,
    pub activator: Option<EntityHandle>,
    pub continuous_speed_bits: u32,
    pub solid: bool,
    pub path: Option<TrainPathState>,
    pub rotator: Option<RotatorState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotatorState {
    pub target_speed_bits: u32,
    pub friction_bits: u32,
    pub accelerate: bool,
    pub reversed: bool,
    pub stop_at_start: bool,
    pub start_angles: [u32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainPathState {
    pub track: bool,
    pub current: Option<EntityHandle>,
    pub target_name: Vec<u8>,
    pub running: bool,
    pub forward: bool,
    pub height_bits: u32,
    pub length_bits: u32,
    pub maximum_speed_bits: u32,
    pub current_speed_bits: u32,
    pub desired_speed_bits: u32,
    pub unmodified_desired_speed_bits: u32,
    pub old_speed_bits: u32,
    pub forward_modifier_bits: u32,
    pub acceleration_bits: u32,
    pub deceleration_bits: u32,
    pub bank_bits: u32,
    pub velocity_type: i32,
    pub orientation_type: i32,
    pub manual_speed_changes: bool,
    pub accelerating: bool,
    pub controls_disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathNodeState {
    pub track: bool,
    pub next_name: Vec<u8>,
    pub alternate_name: Vec<u8>,
    pub next: Option<EntityHandle>,
    pub previous: Option<EntityHandle>,
    pub alternate: Option<EntityHandle>,
    pub flags: i32,
    pub wait_bits: u32,
    pub speed_bits: u32,
    pub orientation: i32,
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
pub struct BreakableState {
    pub broken: bool,
    pub can_break: bool,
    pub health: i32,
    pub maximum_health: i32,
    pub minimum_damage: i32,
    pub trigger_only: bool,
    pub breaker: Option<EntityHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicPropState {
    pub skin: i32,
    pub visible: bool,
    pub collision_enabled: bool,
    pub default_animation: Vec<u8>,
    pub requested_animation: Option<Vec<u8>>,
    pub current_animation: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickupState {
    pub touchable: bool,
    pub visible: bool,
    pub original_transform: Transform,
    pub pending_subject: Option<EntityHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityRenderState {
    pub brush_model: Option<usize>,
    pub mode: u8,
    pub color: [u8; 4],
    pub fx: u8,
    pub effects: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayState {
    pub enabled: bool,
    pub waiting_for_refire: bool,
    pub remove_on_fire: bool,
    pub fast_retrigger: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerState {
    pub enabled: bool,
    pub interval_bits: u32,
    pub alternating: bool,
    pub high_next: bool,
    pub use_random: bool,
    pub lower_bound_bits: u32,
    pub upper_bound_bits: u32,
    pub next_fire_tick: Option<u64>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateMemberState {
    pub definition: Entity,
    pub relative_transform: Transform,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateState {
    pub requested_names: Vec<Vec<u8>>,
    pub members: Vec<TemplateMemberState>,
    pub preserve_names: bool,
    pub keep_prototypes: bool,
    pub instances: usize,
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
    Soundscape,
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
    pub damage_cap_bits: u32,
    pub damage_model: i32,
    pub damage_type: i32,
    pub no_damage_force: bool,
    pub original_damage_bits: u32,
    pub next_hurt_tick: Option<u64>,
    pub hurt_last_cycle: Vec<EntityHandle>,
    pub direction: [u32; 3],
    pub speed_bits: u32,
    pub push_once: bool,
    pub target_name: Vec<u8>,
    pub landmark_name: Vec<u8>,
    pub preserve_angles: bool,
    pub catapult_physics_speed_bits: u32,
    pub catapult_exact_velocity: bool,
    pub catapult_exact_choice: i32,
    pub catapult_threshold: bool,
    pub catapult_lower_bits: u32,
    pub catapult_upper_bits: u32,
    pub catapult_cooldowns: Vec<(EntityHandle, u64)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PushMode {
    BaseVelocity,
    ImpulseAndRemove,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TriggerEffectData {
    Multiple,
    Hurt {
        damage_bits: u32,
        damage_type: i32,
        no_force: bool,
    },
    Push {
        velocity: [f32; 3],
        mode: PushMode,
    },
    Catapult {
        direction: [f32; 3],
        player_speed_bits: u32,
        physics_speed_bits: u32,
        target: Option<EntityHandle>,
        exact_velocity: bool,
        exact_choice: i32,
        threshold: bool,
        lower_bits: u32,
        upper_bits: u32,
    },
    Teleport {
        destination: Option<EntityHandle>,
        landmark: Option<EntityHandle>,
        preserve_angles: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum BehaviorState {
    Inert,
    Brush(BrushState),
    Mover(MoverState),
    LogicAuto,
    Relay(RelayState),
    Timer(TimerState),
    Template(TemplateState),
    PathNode(PathNodeState),
    Counter(CounterState),
    Case(CaseState),
    Filter(FilterState),
    Trigger(TriggerState),
    External,
    Breakable(BreakableState),
    DynamicProp(DynamicPropState),
    Pickup(PickupState),
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
    pub fields: Vec<TypedFieldState>,
    // Installed authored data is read-only. Runtime fields/outputs detach with
    // the entity; template prototypes remain owned and are fixed up before spawn.
    pub definition: Arc<Entity>,
    pub behavior: BehaviorState,
    pub render: EntityRenderState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedFieldState {
    pub key: Vec<u8>,
    pub field_type: FieldType,
    pub writable_input: Option<Vec<u8>>,
    pub source_pair: Option<usize>,
    pub value: Option<Variant>,
    pub coverage: Coverage,
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
    /// Map loading/recreation creates the complete hierarchy before activation.
    SpawnMapEntities(Vec<Entity>),
    Input(InputRecord),
    QueueInput { input: InputRecord, delay: f32 },
    Contact(ContactRecord),
    Damage {
        entity: EntityHandle,
        attacker: Option<EntityHandle>,
    },
    DamageValue {
        entity: EntityHandle,
        attacker: Option<EntityHandle>,
        damage: i32,
    },
    DynamicPropAnimationStarted {
        entity: EntityHandle,
        accepted: bool,
    },
    DynamicPropAnimationCompleted {
        entity: EntityHandle,
    },
    PickupContact {
        entity: EntityHandle,
        subject: EntityHandle,
        unobstructed: bool,
    },
    PickupResult {
        entity: EntityHandle,
        subject: EntityHandle,
        accepted: bool,
        respawn_ticks: Option<u64>,
        respawn_transform: Option<Transform>,
    },
    Remove(EntityHandle),
    SetParent(ParentRequest),
    SetWorldTransform {
        entity: EntityHandle,
        transform: Transform,
    },
    SetBrushModel {
        entity: EntityHandle,
        model: Option<usize>,
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
        local_angles_destination: [f32; 3],
        world_angles_destination: [f32; 3],
        angular_velocity: [f32; 3],
        continuous: bool,
        solid: bool,
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
        effect: TriggerEffectData,
    },
    BlockDamage {
        mover: EntityHandle,
        blocker: EntityHandle,
        damage_bits: u32,
    },
    ExternalInput {
        entity: EntityHandle,
        input: Vec<u8>,
        value: Variant,
        activator: Option<EntityHandle>,
        caller: Option<EntityHandle>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Transition {
    PathTrackPassed { node: EntityHandle },
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
    TemplateLimit,
    HierarchyLimit,
    SnapshotLimit,
    DiagnosticLimit,
    TickRegression,
    InvalidField,
    SnapshotIdentity,
    RevisionMismatch,
    RevisionOverflow,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimerAction {
    Reset,
    Disable,
    Add(u64),
    Subtract(u64),
}

#[derive(Clone, Debug, PartialEq)]
enum TrainAction {
    Find,
    Think,
    Start,
    Stop,
    Resume,
    Reverse,
    SetSpeed { speed: f32, accelerate: bool },
    SetForwardModifier(f32),
    Teleport(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PathLookAhead {
    current: EntityHandle,
    next: Option<EntityHandle>,
    position: [f32; 3],
    dead_end: bool,
}

#[derive(Clone, Debug)]
struct Slot {
    generation: u32,
    entity: Option<Arc<RuntimeEntity>>,
}

#[derive(Clone, Debug)]
struct WorldState {
    current_tick: u64,
    revision: u64,
    next_creation_order: u64,
    next_output_id: u64,
    next_event_id: u64,
    next_enqueue_sequence: u64,
    next_transition_sequence: u64,
    next_mover_request_id: u64,
    next_template_instance: u32,
    diagnostics: usize,
    random: SourceRandom,
    slots: Vec<Slot>,
    creation_order: Vec<EntityHandle>,
    // Tick/phase checkpoints retain these tables unchanged. Detach only at
    // entity admission, rename, or removal, just like the retained entities.
    targetname_index: Arc<BTreeMap<Vec<u8>, Vec<EntityHandle>>>,
    classname_index: Arc<BTreeMap<Vec<u8>, Vec<EntityHandle>>>,
    queue: Vec<QueuedEvent>,
}

#[derive(Clone, Debug)]
pub struct EntityWorld {
    config: Arc<EntityWorldConfig>,
    state: WorldState,
}

#[derive(Clone, Debug)]
pub struct EntitySnapshot {
    source_identity: u64,
    registry_identity: u64,
    bytes: Vec<u8>,
    state: WorldState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushMoverPresentation {
    pub kind: MoverKind,
    pub class: MoverClass,
    pub position: MoverPosition,
    pub progress_bits: u32,
    pub request_id: Option<u64>,
    pub local_destination: Option<[f32; 3]>,
    pub world_destination: Option<[f32; 3]>,
    pub local_angles_destination: Option<[f32; 3]>,
    pub world_angles_destination: Option<[f32; 3]>,
    pub angular_velocity: Option<[f32; 3]>,
    pub continuous: Option<bool>,
    pub opening: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushModelDrawState {
    pub handle: EntityHandle,
    pub source_index: usize,
    pub model: usize,
    pub local_transform: Transform,
    pub world_transform: Transform,
    pub parent: Option<EntityHandle>,
    pub render_mode: u8,
    pub color: [u8; 4],
    pub render_fx: u8,
    pub effects: u16,
    pub draw: bool,
    pub mover: Option<BrushMoverPresentation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StudioModelDrawState {
    pub source_index: usize,
    pub skin: i32,
    pub world_transform: Transform,
    pub draw: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushModelPresentation {
    pub source_identity: u64,
    pub registry_identity: u64,
    pub tick: u64,
    pub revision: u64,
    pub models: Vec<BrushModelDrawState>,
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

    pub fn revision(&self) -> u64 {
        self.state.revision
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
                revision: 0,
                next_creation_order: 0,
                next_output_id: 1,
                next_event_id: 1,
                next_enqueue_sequence: 1,
                next_transition_sequence: 1,
                next_mover_request_id: 1,
                next_template_instance: 0,
                diagnostics: 0,
                random: SourceRandom::new(config.random_seed as i32),
                slots: Vec::new(),
                creation_order: Vec::new(),
                targetname_index: Arc::default(),
                classname_index: Arc::default(),
                queue: Vec::new(),
            },
            config: Arc::new(config),
        };
        let mut batch = TransitionBatch::default();
        let skipped = template_prototype_indices(graph)?;
        for definition in ordered_definitions(&graph.entities, &skipped, &world.config.spawn_priorities) {
            world.spawn_definition(definition.clone(), &mut batch)?;
        }
        world.refresh_projected_fields();
        world.install_initial_attachments(None)?;
        world.resolve_initial_parents(&world.state.creation_order.clone(), &mut batch)?;
        world.capture_templates(graph)?;
        world.link_path_nodes()?;
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
        world.state.revision = 1;
        Ok((world, batch))
    }

    pub fn current_tick(&self) -> u64 {
        self.state.current_tick
    }

    /// Whether a game-owned entity subscribes to a broadcast input. Broadcasts
    /// ignore non-subscribers rather than treating them as failed direct I/O.
    pub fn accepts_external_input(&self, handle: EntityHandle, input: &[u8]) -> bool {
        self.entity(handle).is_some_and(|entity| {
            self.config.external_classes.iter().any(|binding| {
                entity.definition.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(&binding.classname))
                    && binding.inputs.iter().any(|name| name.eq_ignore_ascii_case(input))
            })
        })
    }

    /// Game round cleanup clears scheduled map I/O before recreating map entities.
    pub fn clear_event_queue(&mut self) {
        self.state.queue.clear();
    }

    pub fn set_map_load_kind(&mut self, kind: MapLoadKind) {
        Arc::make_mut(&mut self.config).load_kind = kind;
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }

    pub fn brush_model_presentation(
        &self,
        expected_revision: u64,
    ) -> Result<BrushModelPresentation, RuntimeFailure> {
        if expected_revision != self.state.revision {
            return Err(failure(
                RuntimeFailureCode::RevisionMismatch,
                None,
                usize::try_from(self.state.revision).unwrap_or(usize::MAX),
                usize::try_from(expected_revision).unwrap_or(usize::MAX),
            ));
        }
        let mut models = Vec::new();
        for handle in &self.state.creation_order {
            let Some(entity) = self.entity(*handle) else {
                continue;
            };
            let Some(model) = entity.render.brush_model.filter(|model| *model != 0) else {
                continue;
            };
            if models.len() >= self.config.limits.max_entities {
                return Err(failure(
                    RuntimeFailureCode::EntityLimit,
                    Some(entity.source_index),
                    self.config.limits.max_entities,
                    models.len() + 1,
                ));
            }
            let draw =
                entity.render.mode != RENDER_MODE_NONE && entity.render.effects & EF_NODRAW == 0;
            models.push(BrushModelDrawState {
                handle: entity.handle,
                source_index: entity.source_index,
                model,
                local_transform: entity.local_transform,
                world_transform: entity.world_transform,
                parent: entity.parent,
                render_mode: entity.render.mode,
                color: entity.render.color,
                render_fx: entity.render.fx,
                effects: entity.render.effects,
                draw,
                mover: match &entity.behavior {
                    BehaviorState::Mover(mover) => Some(mover_presentation(entity, mover)),
                    _ => None,
                },
            });
        }
        models.sort_by_key(|model| (model.source_index, model.handle));
        Ok(BrushModelPresentation {
            source_identity: self.config.source_identity,
            registry_identity: self.config.registry_identity,
            tick: self.state.current_tick,
            revision: self.state.revision,
            models,
        })
    }

    pub fn studio_model_presentation(
        &self,
        expected_revision: u64,
    ) -> Result<Vec<StudioModelDrawState>, RuntimeFailure> {
        if expected_revision != self.state.revision {
            return Err(failure(
                RuntimeFailureCode::RevisionMismatch,
                None,
                usize::try_from(self.state.revision).unwrap_or(usize::MAX),
                usize::try_from(expected_revision).unwrap_or(usize::MAX),
            ));
        }
        let mut models = Vec::new();
        for handle in &self.state.creation_order {
            let Some(entity) = self.entity(*handle) else {
                continue;
            };
            let BehaviorState::DynamicProp(state) = &entity.behavior else { continue; };
            models.push(StudioModelDrawState {
                source_index: entity.source_index,
                skin: state.skin,
                world_transform: entity.world_transform,
                draw: entity.render.mode != RENDER_MODE_NONE
                    && entity.render.effects & EF_NODRAW == 0,
            });
        }
        models.sort_by_key(|model| model.source_index);
        Ok(models)
    }

    pub fn entity(&self, handle: EntityHandle) -> Option<&RuntimeEntity> {
        let slot = self.state.slots.get(usize::from(handle.slot))?;
        (slot.generation == handle.generation)
            .then_some(slot.entity.as_deref())
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

    pub fn convert_variant(
        &self,
        value: &Variant,
        target: FieldType,
    ) -> Result<Variant, ValueConversionError> {
        convert_value(
            value,
            target,
            |handle| {
                self.entity(handle)
                    .and_then(|entity| entity.targetname.clone())
            },
            |name| self.resolve(name, None, None, None).first().copied(),
        )
    }

    pub fn phase(
        &mut self,
        tick: u64,
        commands: &[WorldCommand],
    ) -> Result<TransitionBatch, RuntimeFailure> {
        self.command_transaction(tick, commands, true)
    }

    /// CEventQueue::AddEvent does not service events. Console inputs are queued
    /// between simulation phases, including zero-delay inputs.
    pub fn enqueue_input(&mut self, tick: u64, input: InputRecord, delay: f32) -> Result<TransitionBatch, RuntimeFailure> {
        self.command_transaction(tick, &[WorldCommand::QueueInput { input, delay }], false)
    }

    fn command_transaction(&mut self, tick: u64, commands: &[WorldCommand], service_events: bool) -> Result<TransitionBatch, RuntimeFailure> {
        if tick < self.state.current_tick {
            return Err(failure(
                RuntimeFailureCode::TickRegression,
                None,
                self.state.current_tick as usize,
                tick as usize,
            ));
        }
        let phase_start = self.state.clone();
        self.state.current_tick = tick;
        let mut batch = TransitionBatch::default();
        for command in commands {
            if let Err(error) = self.apply_command(command.clone(), &mut batch) {
                self.state = phase_start;
                return Err(error);
            }
        }
        if service_events {
            if let Err(error) = self.drain_due_events(&mut batch) {
                self.state = phase_start;
                return Err(error);
            }
        }
        self.commit_removals();
        let Some(revision) = self.state.revision.checked_add(1) else {
            self.state = phase_start;
            return Err(failure(
                RuntimeFailureCode::RevisionOverflow,
                None,
                usize::MAX,
                usize::MAX,
            ));
        };
        self.state.revision = revision;
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
            .then_some(slot.entity.as_mut().map(Arc::make_mut))
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
            angles: crate::sprite::spawn_angles(&classname,field_vector(&definition, b"angles", [0.0; 3])?),
        };
        let (behavior, coverage) = self.behavior_for(&definition, local_transform)?;
        let mut render = render_state(&definition)?;
        if let Some(model) = render.brush_model.filter(|model| *model != 0)
            && !self
                .config
                .model_bounds
                .iter()
                .any(|bounds| bounds.model == model)
        {
            return Err(failure(
                RuntimeFailureCode::InvalidField,
                Some(definition.index),
                self.config.model_bounds.len(),
                model,
            ));
        }
        if matches!(&behavior, BehaviorState::Trigger(_))
            || matches!(
                &behavior,
                BehaviorState::Brush(BrushState { enabled: false, .. })
            )
            || matches!(&behavior, BehaviorState::DynamicProp(state) if !state.visible)
            || matches!(&behavior, BehaviorState::Pickup(state) if !state.visible)
            || self.config.external_brush_models.iter().any(|binding| {
                binding.classname.eq_ignore_ascii_case(&classname)
                    && binding.initial_visibility == ExternalBrushModelVisibility::Hidden
            })
        {
            render.effects |= EF_NODRAW;
        }
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
        let fields = self.project_fields(&definition, &classname);
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
            fields,
            definition: Arc::new(definition),
            behavior,
            render,
        };
        if let BehaviorState::Mover(mover) = &runtime_entity.behavior {
            let endpoint = match mover.position {
                MoverPosition::Closed => Some((mover.closed, mover.closed_angles)),
                MoverPosition::Open => Some((mover.open, mover.open_angles)),
                MoverPosition::Positioned(bits) => {
                    let position = f32::from_bits(bits);
                    Some((
                        lerp(mover.closed, mover.open, position),
                        lerp(mover.closed_angles, mover.open_angles, position),
                    ))
                }
                MoverPosition::Opening | MoverPosition::Closing => None,
            };
            if let Some((origin, angles)) = endpoint {
                runtime_entity.local_transform.origin = origin;
                runtime_entity.local_transform.angles = angles;
                runtime_entity.world_transform.origin = origin;
                runtime_entity.world_transform.angles = angles;
            }
        }
        self.state.slots[slot_index].entity = Some(Arc::new(runtime_entity));
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

    fn project_fields(&self, definition: &Entity, classname: &[u8]) -> Vec<TypedFieldState> {
        let Some(binding) = self
            .config
            .field_bindings
            .iter()
            .find(|binding| binding.classname.eq_ignore_ascii_case(classname))
        else {
            return Vec::new();
        };
        binding
            .fields
            .iter()
            .map(|binding| {
                let source_pair = definition
                    .pairs
                    .iter()
                    .position(|pair| pair.key.eq_ignore_ascii_case(&binding.key));
                let projected = source_pair.map(|index| {
                    project_string(&definition.pairs[index].value, binding.field_type, |name| {
                        self.resolve(name, None, None, None).first().copied()
                    })
                });
                let (value, coverage) = match projected {
                    Some(Ok(value)) => (Some(value), Coverage::Handled),
                    Some(Err(_)) => (None, Coverage::Malformed),
                    None => (None, Coverage::Missing),
                };
                TypedFieldState {
                    key: binding.key.clone(),
                    field_type: binding.field_type,
                    writable_input: binding.writable_input.clone(),
                    source_pair,
                    value,
                    coverage,
                }
            })
            .collect()
    }

    fn install_initial_attachments(&mut self, scope: Option<&[EntityHandle]>) -> Result<(), RuntimeFailure> {
        for binding in self.config.initial_attachments.clone() {
            if scope.is_some_and(|handles| !handles.iter().any(|handle| self.entity(*handle).is_some_and(|entity| entity.source_index == binding.parent_source_index))) { continue; }
            if binding.attachment.is_empty()
                || binding
                    .parent_space_transform
                    .origin
                    .into_iter()
                    .chain(binding.parent_space_transform.angles)
                    .any(|value| !value.is_finite())
            {
                return Err(failure(
                    RuntimeFailureCode::InvalidConfiguration,
                    Some(binding.parent_source_index),
                    0,
                    binding.attachment.len(),
                ));
            }
            let Some(parent) = self.state.creation_order.iter().copied().find(|handle| {
                self.entity(*handle)
                    .is_some_and(|entity| entity.source_index == binding.parent_source_index)
            }) else {
                return Err(failure(
                    RuntimeFailureCode::InvalidConfiguration,
                    Some(binding.parent_source_index),
                    self.state.creation_order.len(),
                    binding.parent_source_index,
                ));
            };
            let replaced = self
                .entity_mut(parent)
                .expect("resolved initial attachment parent")
                .attachments
                .insert(binding.attachment, binding.parent_space_transform);
            if replaced.is_some() {
                return Err(failure(
                    RuntimeFailureCode::InvalidConfiguration,
                    Some(binding.parent_source_index),
                    1,
                    2,
                ));
            }
        }
        Ok(())
    }

    fn refresh_projected_fields(&mut self) {
        let handles = self.state.creation_order.clone();
        for handle in handles {
            let Some(entity) = self.entity(handle) else {
                continue;
            };
            let fields = self.project_fields(&entity.definition, &entity.classname);
            self.entity_mut(handle)
                .expect("live projected entity")
                .fields = fields;
        }
    }

    fn capture_templates(&mut self, graph: &Graph) -> Result<(), RuntimeFailure> {
        let templates = self
            .state
            .creation_order
            .iter()
            .copied()
            .filter(|handle| {
                matches!(
                    self.entity(*handle).map(|entity| &entity.behavior),
                    Some(BehaviorState::Template(_))
                )
            })
            .collect::<Vec<_>>();
        for handle in templates {
            let (transform, names) = {
                let entity = self.entity(handle).expect("template handle");
                let BehaviorState::Template(state) = &entity.behavior else {
                    unreachable!()
                };
                (entity.world_transform, state.requested_names.clone())
            };
            let mut members = Vec::new();
            for name in names {
                for definition in graph.entities.iter().filter(|definition| {
                    definition
                        .targetname
                        .as_deref()
                        .is_some_and(|target| target.eq_ignore_ascii_case(&name))
                }) {
                    if members.len() >= self.config.limits.max_template_members {
                        return Err(failure(
                            RuntimeFailureCode::TemplateLimit,
                            self.entity(handle).map(|entity| entity.source_index),
                            self.config.limits.max_template_members,
                            members.len() + 1,
                        ));
                    }
                    let member_transform = Transform {
                        origin: field_vector(definition, b"origin", [0.0; 3])?,
                        angles: field_vector(definition, b"angles", [0.0; 3])?,
                    };
                    members.push(TemplateMemberState {
                        definition: definition.clone(),
                        relative_transform: relative_transform(transform, member_transform),
                    });
                }
            }
            if let BehaviorState::Template(state) =
                &mut self.entity_mut(handle).expect("template handle").behavior
            {
                state.members = members;
            }
        }
        Ok(())
    }

    fn link_path_nodes(&mut self) -> Result<(), RuntimeFailure> {
        let handles = self.state.creation_order.clone();
        for handle in &handles {
            let Some(BehaviorState::PathNode(state)) =
                self.entity(*handle).map(|entity| entity.behavior.clone())
            else {
                continue;
            };
            let next = self
                .resolve(&state.next_name, Some(*handle), None, None)
                .into_iter()
                .find(|candidate| {
                    matches!(
                        self.entity(*candidate).map(|entity| &entity.behavior),
                        Some(BehaviorState::PathNode(_))
                    )
                });
            let alternate = self
                .resolve(&state.alternate_name, Some(*handle), None, None)
                .into_iter()
                .find(|candidate| {
                    matches!(
                        self.entity(*candidate).map(|entity| &entity.behavior),
                        Some(BehaviorState::PathNode(_))
                    )
                });
            if let BehaviorState::PathNode(node) =
                &mut self.entity_mut(*handle).expect("path node").behavior
            {
                node.next = next;
                node.alternate = alternate;
            }
            for candidate in [next, alternate].into_iter().flatten() {
                let is_own_alternate = self
                    .entity(candidate)
                    .and_then(|entity| match &entity.behavior {
                        BehaviorState::PathNode(node) => Some(node.alternate_name.as_slice()),
                        _ => None,
                    })
                    .zip(
                        self.entity(*handle)
                            .and_then(|entity| entity.targetname.as_deref()),
                    )
                    .is_some_and(|(alternate_name, previous_name)| {
                        alternate_name.eq_ignore_ascii_case(previous_name)
                    });
                if !is_own_alternate
                    && let BehaviorState::PathNode(node) = &mut self
                        .entity_mut(candidate)
                        .expect("linked path node")
                        .behavior
                {
                    node.previous = Some(*handle);
                }
            }
        }
        Ok(())
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
        if classname.eq_ignore_ascii_case(b"func_rot_button") {
            let speed = field_f32(definition, b"speed", 0.0)?;
            let wait = field_f32(definition, b"wait", 0.0)?;
            let axis = toggle_rotation_axis(flags);
            let distance = field_f32(definition, b"distance", 0.0)?;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Button,
                    class: MoverClass::RotatingButton,
                    closed: transform.origin,
                    open: transform.origin,
                    closed_angles: transform.angles,
                    open_angles: add(transform.angles, scale(axis, distance)),
                    speed: if speed == 0.0 { 40.0 } else { speed },
                    wait_ticks: (wait >= 0.0).then(|| {
                        delay_ticks(
                            if wait == 0.0 { 1.0 } else { wait },
                            self.config.tick_interval,
                        )
                    }),
                    position: MoverPosition::Closed,
                    pending: None,
                    locked: flags & BUTTON_LOCKED != 0,
                    toggle: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    force_closed: false,
                    no_auto_return: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    stay_pushed: wait == -1.0,
                    outputs_reversed: false,
                    block_damage_bits: 0.0_f32.to_bits(),
                    damage_activates: flags & BUTTON_DAMAGE_ACTIVATES != 0
                        || field_i32(definition, b"health", 0)? != 0,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 1 == 0,
                    path: None,
                    rotator: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"momentary_rot_button") {
            let mut axis = toggle_rotation_axis(flags);
            let mut distance = field_f32(definition, b"distance", 0.0)?;
            if distance < 0.0 {
                axis = scale(axis, -1.0);
                distance = -distance;
            }
            let start = field_f32(definition, b"StartPosition", 0.0)?.clamp(0.0, 1.0);
            let speed = field_f32(definition, b"speed", 0.0)?;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Linear,
                    class: MoverClass::MomentaryRotatingButton,
                    closed: transform.origin,
                    open: transform.origin,
                    closed_angles: sub(transform.angles, scale(axis, distance * start)),
                    open_angles: add(transform.angles, scale(axis, distance * (1.0 - start))),
                    speed: if speed == 0.0 { 100.0 } else { speed },
                    wait_ticks: None,
                    position: MoverPosition::Positioned(start.to_bits()),
                    pending: None,
                    locked: flags & BUTTON_LOCKED != 0,
                    toggle: false,
                    force_closed: false,
                    no_auto_return: true,
                    stay_pushed: false,
                    outputs_reversed: false,
                    block_damage_bits: 0.0_f32.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 1 == 0,
                    path: None,
                    rotator: None,
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
            let mut distance = self.brush_travel(definition, direction, lip).max(0.0);
            if distance < 1.0 || flags & BUTTON_DONT_MOVE != 0 {
                distance = 0.0;
            }
            let open = add(transform.origin, scale(direction, distance));
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Button,
                    class: MoverClass::Button,
                    closed: transform.origin,
                    open,
                    closed_angles: transform.angles,
                    open_angles: transform.angles,
                    speed,
                    wait_ticks: (wait >= 0.0).then(|| delay_ticks(wait, self.config.tick_interval)),
                    position: MoverPosition::Closed,
                    pending: None,
                    locked: flags & BUTTON_LOCKED != 0,
                    toggle: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    force_closed: false,
                    no_auto_return: flags & BUTTON_TOGGLE != 0 || wait == -1.0,
                    stay_pushed: wait == -1.0,
                    outputs_reversed: false,
                    block_damage_bits: 0.0f32.to_bits(),
                    damage_activates: flags & BUTTON_DAMAGE_ACTIVATES != 0
                        || field_i32(definition, b"health", 0)? != 0,
                    dont_move: flags & BUTTON_DONT_MOVE != 0,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: true,
                    path: None,
                    rotator: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_door_rotating") {
            let speed = field_f32(definition, b"speed", 0.0)?;
            let wait = field_f32(definition, b"wait", 0.0)?;
            let distance = field_f32(definition, b"distance", 0.0)?;
            let axis = toggle_rotation_axis(flags);
            let mut closed_angles = transform.angles;
            let mut open_angles = add(transform.angles, scale(axis, distance));
            let obsolete_start_open = flags & DOOR_START_OPEN != 0;
            if obsolete_start_open {
                std::mem::swap(&mut closed_angles, &mut open_angles);
            }
            let starts_open = field_i32(definition, b"spawnpos", 0)? == 1;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Door,
                    class: MoverClass::RotatingDoor,
                    closed: transform.origin,
                    open: transform.origin,
                    closed_angles,
                    open_angles,
                    speed: if speed == 0.0 { 100.0 } else { speed },
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
                    stay_pushed: false,
                    outputs_reversed: obsolete_start_open,
                    block_damage_bits: field_f32(definition, b"dmg", 0.0)?.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 8 == 0,
                    path: None,
                    rotator: None,
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
                    class: MoverClass::Door,
                    closed: transform.origin,
                    open,
                    closed_angles: transform.angles,
                    open_angles: transform.angles,
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
                    stay_pushed: false,
                    outputs_reversed: flags & DOOR_START_OPEN != 0,
                    block_damage_bits: field_f32(definition, b"dmg", 0.0)?.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 8 == 0,
                    path: None,
                    rotator: None,
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
                    class: MoverClass::Linear,
                    closed,
                    open,
                    closed_angles: transform.angles,
                    open_angles: transform.angles,
                    speed: if speed <= 0.0 { 100.0 } else { speed },
                    wait_ticks: None,
                    position: MoverPosition::Positioned(start.to_bits()),
                    pending: None,
                    locked: false,
                    toggle: false,
                    force_closed: false,
                    no_auto_return: true,
                    stay_pushed: false,
                    outputs_reversed: false,
                    block_damage_bits: field_f32(definition, b"BlockDamage", 0.0)?.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 8 == 0,
                    path: None,
                    rotator: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_plat")
            || classname.eq_ignore_ascii_case(b"func_platrot")
        {
            let height = {
                let configured = field_f32(definition, b"height", 0.0)?;
                if configured != 0.0 {
                    configured
                } else {
                    definition
                        .bsp_model_index
                        .and_then(|model| {
                            self.config
                                .model_bounds
                                .iter()
                                .find(|item| item.model == model)
                        })
                        .map_or(0.0, |bounds| bounds.maxs[2] - bounds.mins[2] - 8.0)
                }
            };
            let speed = field_f32(definition, b"speed", 0.0)?;
            let rotating = classname.eq_ignore_ascii_case(b"func_platrot");
            let axis = toggle_rotation_axis(flags);
            let rotation = if rotating {
                field_f32(definition, b"rotation", 0.0)?
            } else {
                0.0
            };
            let named = definition
                .targetname
                .as_ref()
                .is_some_and(|name| !name.is_empty());
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Linear,
                    class: if rotating {
                        MoverClass::RotatingPlatform
                    } else {
                        MoverClass::Platform
                    },
                    closed: sub(transform.origin, [0.0, 0.0, height]),
                    open: transform.origin,
                    closed_angles: transform.angles,
                    open_angles: add(transform.angles, scale(axis, rotation)),
                    speed: if speed == 0.0 { 150.0 } else { speed },
                    wait_ticks: Some(delay_ticks(3.0, self.config.tick_interval)),
                    position: if named {
                        MoverPosition::Open
                    } else {
                        MoverPosition::Closed
                    },
                    pending: None,
                    locked: false,
                    toggle: flags & 1 != 0,
                    force_closed: false,
                    no_auto_return: flags & 1 != 0,
                    stay_pushed: false,
                    outputs_reversed: false,
                    block_damage_bits: 1.0_f32.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: true,
                    path: None,
                    rotator: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_rotating") {
            let max_speed = field_f32(definition, b"maxspeed", 0.0)?.abs();
            let max_speed = if max_speed == 0.0 { 100.0 } else { max_speed };
            let axis = rotating_axis(flags);
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Linear,
                    class: MoverClass::Rotating,
                    closed: transform.origin,
                    open: transform.origin,
                    closed_angles: transform.angles,
                    open_angles: add(transform.angles, scale(axis, 360.0)),
                    speed: max_speed,
                    wait_ticks: None,
                    position: MoverPosition::Positioned(0.0_f32.to_bits()),
                    pending: None,
                    locked: false,
                    toggle: false,
                    force_closed: false,
                    no_auto_return: true,
                    stay_pushed: false,
                    outputs_reversed: false,
                    block_damage_bits: field_f32(definition, b"dmg", 0.0)?.to_bits(),
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 64 == 0,
                    path: None,
                    rotator: Some(RotatorState {
                        target_speed_bits: if flags & 1 != 0 {
                            max_speed.to_bits()
                        } else {
                            0.0_f32.to_bits()
                        },
                        friction_bits: {
                            let friction = field_f32(definition, b"fanfriction", 0.0)? / 100.0;
                            (if friction == 0.0 { 1.0 } else { friction }).to_bits()
                        },
                        accelerate: flags & 16 != 0,
                        reversed: false,
                        stop_at_start: false,
                        start_angles: transform.angles.map(f32::to_bits),
                    }),
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"path_corner")
            || classname.eq_ignore_ascii_case(b"path_track")
        {
            return Ok((
                BehaviorState::PathNode(PathNodeState {
                    track: classname.eq_ignore_ascii_case(b"path_track"),
                    next_name: field(definition, b"target").unwrap_or_default().to_vec(),
                    alternate_name: field(definition, b"altpath").unwrap_or_default().to_vec(),
                    next: None,
                    previous: None,
                    alternate: None,
                    flags,
                    wait_bits: field_f32(definition, b"wait", 0.0)?.to_bits(),
                    speed_bits: field_f32(definition, b"speed", 0.0)?.to_bits(),
                    orientation: field_i32(
                        definition,
                        b"orientationtype",
                        if classname.eq_ignore_ascii_case(b"path_track") {
                            1
                        } else {
                            0
                        },
                    )?,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_train")
            || classname.eq_ignore_ascii_case(b"func_tracktrain")
        {
            let track = classname.eq_ignore_ascii_case(b"func_tracktrain");
            let configured_speed = field_f32(definition, b"speed", 0.0)?;
            let maximum_speed = if track {
                let value = field_f32(definition, b"startspeed", 0.0)?;
                if value == 0.0 {
                    if configured_speed == 0.0 {
                        100.0
                    } else {
                        configured_speed
                    }
                } else {
                    value
                }
            } else if configured_speed == 0.0 {
                100.0
            } else {
                configured_speed
            };
            let current_speed = if track {
                configured_speed
            } else {
                maximum_speed
            };
            let block_damage = field_f32(definition, b"dmg", 0.0)?;
            return Ok((
                BehaviorState::Mover(MoverState {
                    kind: MoverKind::Linear,
                    class: if track {
                        MoverClass::TrackTrain
                    } else {
                        MoverClass::Train
                    },
                    closed: transform.origin,
                    open: transform.origin,
                    closed_angles: transform.angles,
                    open_angles: transform.angles,
                    speed: maximum_speed.abs(),
                    wait_ticks: None,
                    position: MoverPosition::Positioned(0.0_f32.to_bits()),
                    pending: None,
                    locked: false,
                    toggle: false,
                    force_closed: false,
                    no_auto_return: true,
                    stay_pushed: false,
                    outputs_reversed: false,
                    block_damage_bits: if block_damage == 0.0 && !track {
                        2.0_f32.to_bits()
                    } else {
                        block_damage.to_bits()
                    },
                    damage_activates: false,
                    dont_move: false,
                    activator: None,
                    continuous_speed_bits: 0.0_f32.to_bits(),
                    solid: flags & 8 == 0,
                    path: Some(TrainPathState {
                        track,
                        current: None,
                        target_name: field(definition, b"target").unwrap_or_default().to_vec(),
                        running: track && current_speed != 0.0,
                        forward: true,
                        height_bits: field_f32(definition, b"height", 0.0)?.to_bits(),
                        length_bits: field_f32(definition, b"wheels", 0.0)?.to_bits(),
                        maximum_speed_bits: maximum_speed.to_bits(),
                        current_speed_bits: current_speed.to_bits(),
                        desired_speed_bits: 0.0_f32.to_bits(),
                        unmodified_desired_speed_bits: 0.0_f32.to_bits(),
                        old_speed_bits: 0.0_f32.to_bits(),
                        forward_modifier_bits: 1.0_f32.to_bits(),
                        acceleration_bits: field_f32(definition, b"ManualAccelSpeed", 0.0)?
                            .to_bits(),
                        deceleration_bits: field_f32(definition, b"ManualDecelSpeed", 0.0)?
                            .to_bits(),
                        bank_bits: field_f32(definition, b"bank", 0.0)?.to_bits(),
                        velocity_type: field_i32(definition, b"velocitytype", 0)?,
                        orientation_type: field_i32(definition, b"orientationtype", 1)?,
                        manual_speed_changes: field_i32(definition, b"ManualSpeedChanges", 0)? != 0,
                        accelerating: false,
                        controls_disabled: flags & 2 != 0,
                    }),
                    rotator: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"func_breakable") {
            let material = match field(definition, b"material") {
                Some(value) => source_i32(value).ok_or_else(|| {
                    failure(
                        RuntimeFailureCode::InvalidField,
                        Some(definition.index),
                        0,
                        value.len(),
                    )
                })?,
                None => 0,
            };
            let health = field_i32(definition, b"health", 0)?;
            return Ok((
                BehaviorState::Breakable(BreakableState {
                    broken: false,
                    can_break: material != 7,
                    health,
                    maximum_health: health.max(1),
                    minimum_damage: field_i32(definition, b"minhealthdmg", 0)?,
                    trigger_only: flags & 1 != 0,
                    breaker: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"prop_dynamic")
            || classname.eq_ignore_ascii_case(b"prop_dynamic_override")
            || classname.eq_ignore_ascii_case(b"dynamic_prop")
        {
            return Ok((
                BehaviorState::DynamicProp(DynamicPropState {
                    skin: field_i32(definition, b"skin", 0)?,
                    visible: field_i32(definition, b"StartDisabled", 0)? == 0,
                    collision_enabled: flags & 256 == 0,
                    default_animation: field(definition, b"DefaultAnim")
                        .unwrap_or_default()
                        .to_vec(),
                    requested_animation: None,
                    current_animation: None,
                }),
                Coverage::Handled,
            ));
        }
        if self
            .config
            .pickup_classes
            .iter()
            .any(|class| class.eq_ignore_ascii_case(classname))
        {
            return Ok((
                BehaviorState::Pickup(PickupState {
                    touchable: field_i32(definition, b"StartDisabled", 0)? == 0,
                    visible: field_i32(definition, b"StartDisabled", 0)? == 0,
                    original_transform: transform,
                    pending_subject: None,
                }),
                Coverage::Handled,
            ));
        }
        if classname.eq_ignore_ascii_case(b"logic_auto") {
            return Ok((BehaviorState::LogicAuto, Coverage::Handled));
        }
        if classname.eq_ignore_ascii_case(b"point_template") {
            let requested_names = (1..=16)
                .filter_map(|index| {
                    let key = format!("Template{index:02}");
                    field(definition, key.as_bytes())
                        .filter(|value| !value.is_empty())
                        .map(<[u8]>::to_vec)
                })
                .collect();
            return Ok((
                BehaviorState::Template(TemplateState {
                    requested_names,
                    members: Vec::new(),
                    preserve_names: flags & 2 != 0,
                    keep_prototypes: flags & 1 != 0,
                    instances: 0,
                }),
                Coverage::Handled,
            ));
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
        if classname.eq_ignore_ascii_case(b"logic_timer") {
            let use_random = field_i32(definition, b"UseRandomTime", 0)? != 0;
            let mut interval = field_f32(definition, b"RefireTime", 0.0)?;
            if !use_random && interval < 0.01 {
                interval = 0.01;
            }
            return Ok((
                BehaviorState::Timer(TimerState {
                    enabled: field_i32(definition, b"StartDisabled", 0)? == 0
                        && (interval > 0.0 || use_random),
                    interval_bits: interval.to_bits(),
                    alternating: flags & 1 != 0,
                    high_next: false,
                    use_random,
                    lower_bound_bits: field_f32(definition, b"LowerRandomBound", 0.0)?.to_bits(),
                    upper_bound_bits: field_f32(definition, b"UpperRandomBound", 0.0)?.to_bits(),
                    next_fire_tick: None,
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
            let coverage = if self
                .config
                .external_classes
                .iter()
                .any(|binding| binding.classname.eq_ignore_ascii_case(classname))
            {
                Coverage::Handled
            } else {
                Coverage::Unsupported
            };
            return Ok((
                BehaviorState::Filter(FilterState {
                    negated: field_i32(definition, b"Negated", 0)? != 0,
                    predicate: FilterPredicate::External,
                }),
                coverage,
            ));
        }
        let trigger_kind = if classname.eq_ignore_ascii_case(b"trigger_soundscape") {
            Some(TriggerKind::Soundscape)
        } else if classname.eq_ignore_ascii_case(b"trigger_multiple") {
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
                    damage_cap_bits: field_f32(definition, b"damagecap", 20.0)?.to_bits(),
                    damage_model: field_i32(definition, b"damagemodel", 0)?,
                    damage_type: field_i32(definition, b"damagetype", 0)?,
                    no_damage_force: field_i32(definition, b"nodmgforce", 0)? != 0,
                    original_damage_bits: field_f32(definition, b"damage", 0.0)?.to_bits(),
                    next_hurt_tick: None,
                    hurt_last_cycle: Vec::new(),
                    direction: direction_from_angles(if kind == TriggerKind::Catapult {
                        field_vector(definition, b"launchDirection", [0.0; 3])?
                    } else {
                        field_vector(definition, b"pushdir", [0.0; 3])?
                    })
                    .map(f32::to_bits),
                    speed_bits: {
                        let speed = if kind == TriggerKind::Catapult {
                            field_f32(definition, b"playerSpeed", 0.0)?
                        } else {
                            field_f32(definition, b"speed", 0.0)?
                        };
                        (if kind == TriggerKind::Push && speed == 0.0 {
                            100.0
                        } else {
                            speed
                        })
                        .to_bits()
                    },
                    push_once: kind == TriggerKind::Push && flags & 128 != 0,
                    target_name: field(
                        definition,
                        if kind == TriggerKind::Catapult {
                            b"launchTarget".as_slice()
                        } else {
                            b"target".as_slice()
                        },
                    )
                    .unwrap_or_default()
                    .to_vec(),
                    landmark_name: field(definition, b"landmark").unwrap_or_default().to_vec(),
                    preserve_angles: flags & 0x20 != 0,
                    catapult_physics_speed_bits: field_f32(definition, b"physicsSpeed", 0.0)?
                        .to_bits(),
                    catapult_exact_velocity: field_i32(definition, b"useExactVelocity", 0)? != 0,
                    catapult_exact_choice: field_i32(definition, b"exactVelocityChoiceType", 0)?,
                    catapult_threshold: field_i32(definition, b"useThresholdCheck", 0)? != 0,
                    catapult_lower_bits: field_f32(definition, b"lowerThreshold", 0.0)?
                        .clamp(0.0, 1.0)
                        .to_bits(),
                    catapult_upper_bits: field_f32(definition, b"upperThreshold", 0.0)?
                        .clamp(0.0, 1.0)
                        .to_bits(),
                    catapult_cooldowns: Vec::new(),
                }),
                Coverage::Handled,
            ));
        }
        if self
            .config
            .external_classes
            .iter()
            .any(|binding| binding.classname.eq_ignore_ascii_case(classname))
        {
            return Ok((BehaviorState::External, Coverage::Handled));
        }
        if let Some(disposition) = self
            .config
            .class_dispositions
            .iter()
            .find(|binding| binding.classname.eq_ignore_ascii_case(classname))
        {
            return Ok((BehaviorState::Inert, disposition.coverage));
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
        Arc::make_mut(&mut self.state.classname_index)
            .entry(ascii_key(classname))
            .or_default()
            .push(handle);
        if let Some(name) = targetname {
            Arc::make_mut(&mut self.state.targetname_index)
                .entry(ascii_key(name))
                .or_default()
                .push(handle);
        }
    }

    fn index_remove(&mut self, handle: EntityHandle) {
        let classes = Arc::make_mut(&mut self.state.classname_index);
        for values in classes.values_mut() {
            values.retain(|value| *value != handle);
        }
        let names = Arc::make_mut(&mut self.state.targetname_index);
        for values in names.values_mut() {
            values.retain(|value| *value != handle);
        }
        classes.retain(|_, values| !values.is_empty());
        names.retain(|_, values| !values.is_empty());
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
        handles: &[EntityHandle],
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let requests: Vec<_> = handles.iter()
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
            Some(BehaviorState::Timer(state)) if state.enabled => {
                self.reset_timer(handle, batch)?;
            }
            Some(BehaviorState::Mover(state))
                if state.class == MoverClass::Rotating
                    && state.rotator.as_ref().is_some_and(|rotator| {
                        f32::from_bits(rotator.target_speed_bits) != 0.0
                    }) =>
            {
                self.schedule_event(
                    EventTarget::Direct(handle),
                    b"__rotating_start".to_vec(),
                    Variant::Void,
                    0.2,
                    Some(handle),
                    Some(handle),
                    None,
                    batch,
                )?;
            }
            Some(BehaviorState::Mover(state)) if state.class == MoverClass::Train => {
                self.initialize_train(handle, batch)?;
                if self
                    .entity(handle)
                    .is_some_and(|entity| entity.targetname.is_none())
                {
                    self.schedule_event(
                        EventTarget::Direct(handle),
                        b"__train_start".to_vec(),
                        Variant::Void,
                        0.1,
                        Some(handle),
                        Some(handle),
                        None,
                        batch,
                    )?;
                }
            }
            Some(BehaviorState::Mover(state)) if state.class == MoverClass::TrackTrain => {
                self.schedule_event(
                    EventTarget::Direct(handle),
                    b"__train_find".to_vec(),
                    Variant::Void,
                    0.0,
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
            let transform_endpoint = |origin, angles| {
                let endpoint = Transform { origin, angles };
                if old_parent.is_some() {
                    compose_transform(old_basis, endpoint)
                } else {
                    endpoint
                }
            };
            Some((
                transform_endpoint(mover.closed, mover.closed_angles),
                transform_endpoint(mover.open, mover.open_angles),
            ))
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
                let local_endpoint = |endpoint| {
                    if request.parent.is_some() {
                        relative_transform(new_basis, endpoint)
                    } else {
                        endpoint
                    }
                };
                let closed = local_endpoint(closed);
                let open = local_endpoint(open);
                mover.closed = closed.origin;
                mover.closed_angles = closed.angles;
                mover.open = open.origin;
                mover.open_angles = open.angles;
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
            WorldCommand::SpawnMapEntities(definitions) => {
                let mut handles = Vec::with_capacity(definitions.len());
                for definition in ordered_definitions(&definitions, &Default::default(), &self.config.spawn_priorities) {
                    handles.push(self.spawn_definition(definition.clone(), batch)?);
                }
                self.refresh_projected_fields();
                self.install_initial_attachments(Some(&handles))?;
                self.resolve_initial_parents(&handles, batch)?;
                self.link_path_nodes()?;
                for handle in handles {
                    self.entity_mut(handle).expect("spawned map handle").lifecycle = Lifecycle::Activated;
                    self.push_transition(batch, Transition::Lifecycle { entity: handle, state: Lifecycle::Activated })?;
                    self.schedule_activation(handle, batch)?;
                }
                Ok(())
            }
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
            WorldCommand::QueueInput { input, delay } => self.schedule_event(input.target, input.input, input.value, delay, input.activator, input.caller, input.output_action, batch).map(|_| ()),
            WorldCommand::Contact(contact) => self.apply_contact(contact, batch),
            WorldCommand::Damage { entity, attacker } => {
                self.damage_entity(entity, attacker, batch)
            }
            WorldCommand::DamageValue {
                entity,
                attacker,
                damage,
            } => self.damage_breakable(entity, attacker, damage, batch),
            WorldCommand::DynamicPropAnimationStarted { entity, accepted } => {
                self.dynamic_prop_animation_started(entity, accepted, batch)
            }
            WorldCommand::DynamicPropAnimationCompleted { entity } => {
                self.dynamic_prop_animation_completed(entity, batch)
            }
            WorldCommand::PickupContact {
                entity,
                subject,
                unobstructed,
            } => self.pickup_contact(entity, subject, unobstructed, batch),
            WorldCommand::PickupResult {
                entity,
                subject,
                accepted,
                respawn_ticks,
                respawn_transform,
            } => self.pickup_result(
                entity,
                subject,
                accepted,
                respawn_ticks,
                respawn_transform,
                batch,
            ),
            WorldCommand::Remove(handle) => self.mark_removal(handle, batch),
            WorldCommand::SetParent(request) => self.set_parent(request, batch),
            WorldCommand::SetWorldTransform { entity, transform } => {
                if !self.is_resolvable(entity) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
                }
                if transform
                    .origin
                    .into_iter()
                    .chain(transform.angles)
                    .any(|value| !value.is_finite())
                {
                    return Err(failure(
                        RuntimeFailureCode::InvalidField,
                        self.entity(entity).map(|entity| entity.source_index),
                        0,
                        0,
                    ));
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
            WorldCommand::SetBrushModel { entity, model } => {
                if !self.is_resolvable(entity) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
                }
                if let Some(model) = model.filter(|model| *model != 0)
                    && !self
                        .config
                        .model_bounds
                        .iter()
                        .any(|bounds| bounds.model == model)
                {
                    return Err(failure(
                        RuntimeFailureCode::InvalidField,
                        self.entity(entity).map(|entity| entity.source_index),
                        self.config.model_bounds.len(),
                        model,
                    ));
                }
                self.entity_mut(entity)
                    .expect("validated")
                    .render
                    .brush_model = model;
                Ok(())
            }
            WorldCommand::SetAttachmentTransform {
                parent,
                attachment,
                parent_space_transform,
            } => {
                if !self.is_resolvable(parent) {
                    return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(parent));
                }
                if parent_space_transform
                    .origin
                    .into_iter()
                    .chain(parent_space_transform.angles)
                    .any(|value| !value.is_finite())
                {
                    return Err(failure(
                        RuntimeFailureCode::InvalidField,
                        self.entity(parent).map(|entity| entity.source_index),
                        0,
                        0,
                    ));
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

    fn damage_entity(
        &mut self,
        target: EntityHandle,
        attacker: Option<EntityHandle>,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(target) {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(target));
        }
        let Some(BehaviorState::Mover(mut state)) =
            self.entity(target).map(|entity| entity.behavior.clone())
        else {
            return Ok(());
        };
        if state.kind != MoverKind::Button {
            return Ok(());
        }
        self.fire_output(
            target,
            b"OnDamaged",
            Variant::Void,
            state.activator,
            Some(target),
            0.0,
            batch,
        )?;
        if !state.damage_activates || state.locked {
            return Ok(());
        }
        let opening = match state.position {
            MoverPosition::Closed => Some(true),
            MoverPosition::Open if state.toggle && !state.stay_pushed => Some(false),
            MoverPosition::Opening | MoverPosition::Closing | MoverPosition::Positioned(_) => None,
            MoverPosition::Open => None,
        };
        let Some(opening) = opening else {
            return Ok(());
        };
        let Some(attacker) = attacker else {
            return Ok(());
        };
        state.activator = Some(attacker);
        self.entity_mut(target).expect("validated").behavior = BehaviorState::Mover(state);
        self.fire_output(
            target,
            b"OnPressed",
            Variant::Void,
            Some(attacker),
            Some(target),
            0.0,
            batch,
        )?;
        self.start_mover(
            target,
            if opening { 1.0 } else { 0.0 },
            opening,
            Some(attacker),
            batch,
        )
    }

    fn damage_breakable(
        &mut self,
        target: EntityHandle,
        attacker: Option<EntityHandle>,
        damage: i32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Breakable(state)) =
            self.entity(target).map(|entity| entity.behavior.clone())
        else {
            return Ok(());
        };
        if state.broken
            || !state.can_break
            || state.trigger_only
            || damage < state.minimum_damage
            || state.health <= 0
        {
            return Ok(());
        }
        self.dispatch_input(
            target,
            &InputRecord {
                target: EventTarget::Direct(target),
                input: b"RemoveHealth".to_vec(),
                value: Variant::Integer(damage),
                activator: attacker,
                caller: attacker,
                output_action: None,
                producer_sequence: self.state.next_transition_sequence,
            },
            batch,
        )
    }

    fn dynamic_prop_animation_started(
        &mut self,
        entity: EntityHandle,
        accepted: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::DynamicProp(mut state)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
        };
        let requested = state.requested_animation.take();
        if accepted {
            state.current_animation = requested;
            self.entity_mut(entity).expect("dynamic prop").behavior =
                BehaviorState::DynamicProp(state);
            self.fire_output(
                entity,
                b"OnAnimationBegun",
                Variant::Void,
                None,
                Some(entity),
                0.0,
                batch,
            )
        } else {
            self.entity_mut(entity).expect("dynamic prop").behavior =
                BehaviorState::DynamicProp(state);
            Ok(())
        }
    }

    fn dynamic_prop_animation_completed(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::DynamicProp(mut state)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
        };
        state.current_animation = None;
        let default_animation = state.default_animation.clone();
        if !default_animation.is_empty() {
            state.requested_animation = Some(default_animation.clone());
        }
        self.entity_mut(entity).expect("dynamic prop").behavior = BehaviorState::DynamicProp(state);
        self.fire_output(
            entity,
            b"OnAnimationDone",
            Variant::Void,
            None,
            Some(entity),
            0.0,
            batch,
        )?;
        if !default_animation.is_empty() {
            self.push_transition(
                batch,
                Transition::Request(RuntimeRequest::ExternalInput {
                    entity,
                    input: b"SetAnimation".to_vec(),
                    value: Variant::String(default_animation),
                    activator: None,
                    caller: Some(entity),
                }),
            )?;
        }
        Ok(())
    }

    fn pickup_contact(
        &mut self,
        entity: EntityHandle,
        subject: EntityHandle,
        unobstructed: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Pickup(mut state)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
        };
        if !unobstructed || !state.touchable || !state.visible || !self.is_resolvable(subject) {
            return Ok(());
        }
        self.fire_output(
            entity,
            b"OnCacheInteraction",
            Variant::Void,
            Some(subject),
            Some(entity),
            0.0,
            batch,
        )?;
        state.pending_subject = Some(subject);
        self.entity_mut(entity).expect("pickup").behavior = BehaviorState::Pickup(state);
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::ExternalInput {
                entity,
                input: b"Pickup".to_vec(),
                value: Variant::Handle(subject),
                activator: Some(subject),
                caller: Some(entity),
            }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn pickup_result(
        &mut self,
        entity: EntityHandle,
        subject: EntityHandle,
        accepted: bool,
        respawn_ticks: Option<u64>,
        respawn_transform: Option<Transform>,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Pickup(mut state)) =
            self.entity(entity).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(entity));
        };
        if state.pending_subject != Some(subject) {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(subject));
        }
        state.pending_subject = None;
        if !accepted {
            self.entity_mut(entity).expect("pickup").behavior = BehaviorState::Pickup(state);
            return Ok(());
        }
        self.fire_output(
            entity,
            b"OnPlayerTouch",
            Variant::Void,
            Some(subject),
            Some(entity),
            0.0,
            batch,
        )?;
        if let Some(ticks) = respawn_ticks {
            let transform = respawn_transform.unwrap_or(state.original_transform);
            state.touchable = false;
            state.visible = false;
            self.entity_mut(entity).expect("pickup").behavior = BehaviorState::Pickup(state);
            self.entity_mut(entity).expect("pickup").render.effects |= EF_NODRAW;
            self.set_entity_world_transform(entity, transform, batch)?;
            self.schedule_event_ticks(
                EventTarget::Direct(entity),
                b"__pickup_materialize".to_vec(),
                Variant::Void,
                ticks,
                Some(entity),
                Some(entity),
                None,
                batch,
            )?;
            Ok(())
        } else {
            self.mark_removal(entity, batch)
        }
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
        if let Some(result) = self.assign_input_field(target, &input, &record.value) {
            let accepted = result.is_ok();
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
                self.diagnostic(batch, DiagnosticCode::BadParameter, Some(target))?;
            }
            return Ok(());
        }
        let behavior = self
            .entity(target)
            .expect("validated target")
            .behavior
            .clone();
        let mut accepted = true;
        let mut outputs: Vec<(Vec<u8>, Variant, Option<EntityHandle>)> = Vec::new();
        let mut mover: Option<(f32, bool)> = None;
        let mut rotator_target = None;
        let mut rotator_adjust = false;
        let mut immediate_mover_position = None;
        let mut train_action = None;
        let mut mover_activator = record.activator;
        let mut remove = false;
        let mut remove_child_first = false;
        let mut parent: Option<ParentRequest> = None;
        let mut cancel_caller = false;
        let mut relay_refire_delay = None;
        let mut timer_action = None;
        let mut breakable_remove_delay = None;
        let mut breakable_became_nonsolid = false;
        let mut breakable_health_output = None;
        let mut template_spawn = false;
        let mut trigger_hurt_tick = false;

        if input == b"kill" {
            remove = true;
        } else if input == b"killhierarchy" {
            remove_child_first = true;
        } else if input == b"alpha" {
            match variant_i32(&record.value) {
                Some(alpha) => {
                    self.entity_mut(target).expect("validated").render.color[3] =
                        alpha.clamp(0, 255) as u8;
                }
                None => accepted = false,
            }
        } else if input == b"color" {
            match variant_color(&record.value) {
                Some(color) => self.entity_mut(target).expect("validated").render.color[..3]
                    .copy_from_slice(&color[..3]),
                None => accepted = false,
            }
        } else if input == b"disableshadow" {
            self.entity_mut(target).expect("validated").render.effects |= EF_NOSHADOW;
        } else if input == b"enableshadow" {
            self.entity_mut(target).expect("validated").render.effects &= !EF_NOSHADOW;
        } else if input == b"addoutput" {
            accepted = match self.variant_string(&record.value) {
                Some(value) => self.apply_add_output(target, &value)?,
                None => false,
            };
        } else if input == b"clearparent" {
            parent = Some(ParentRequest {
                child: target,
                parent: None,
                attachment: None,
                mode: ParentMode::MaintainWorld,
            });
        } else if input == b"setparent" {
            let Some(raw) = self.variant_string(&record.value) else {
                self.push_transition(
                    batch,
                    Transition::Input {
                        target,
                        input: record.input.clone(),
                        accepted: false,
                        producer_sequence: record.producer_sequence,
                    },
                )?;
                self.diagnostic(batch, DiagnosticCode::BadParameter, Some(target))?;
                return Ok(());
            };
            let matches =
                self.resolve_target(&raw, record.caller, record.activator, record.caller)?;
            if let Some(new_parent) = matches.first() {
                parent = Some(ParentRequest {
                    child: target,
                    parent: Some(*new_parent),
                    attachment: None,
                    mode: ParentMode::MaintainWorld,
                });
            } else {
                accepted = false;
            }
        } else if input == b"setparentattachment" || input == b"setparentattachmentmaintainoffset" {
            let attachment = self.variant_string(&record.value).unwrap_or_else(|| {
                accepted = false;
                Vec::new()
            });
            let current_parent = self.entity(target).and_then(|entity| entity.parent);
            if current_parent.is_some() && !attachment.is_empty() {
                parent = Some(ParentRequest {
                    child: target,
                    parent: current_parent,
                    attachment: Some(attachment),
                    mode: if input == b"setparentattachment" {
                        ParentMode::SnapToAttachment
                    } else {
                        ParentMode::MaintainWorld
                    },
                });
            } else {
                accepted = false;
            }
        } else if let Some(user) = input.strip_prefix(b"fireuser") {
            if matches!(user, b"1" | b"2" | b"3" | b"4")
                && self.variant_string(&record.value).is_some()
            {
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
                        if state.enabled {
                            self.entity_mut(target).expect("validated").render.effects &=
                                !EF_NODRAW;
                        } else {
                            self.entity_mut(target).expect("validated").render.effects |= EF_NODRAW;
                        }
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
                    if matches!(state.class, MoverClass::Train | MoverClass::TrackTrain) {
                        let Some(path) = state.path.as_mut() else {
                            accepted = false;
                            self.entity_mut(target).expect("validated").behavior =
                                BehaviorState::Mover(state);
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
                        match input.as_slice() {
                            b"__train_find" => train_action = Some(TrainAction::Find),
                            b"__tracktrain_next" if path.track => {
                                train_action = Some(TrainAction::Think);
                            }
                            b"__train_start" | b"__train_next" | b"start" => {
                                path.running = true;
                                train_action = Some(TrainAction::Start);
                            }
                            b"resume" => {
                                train_action = Some(if path.track {
                                    TrainAction::Resume
                                } else {
                                    path.running = true;
                                    TrainAction::Start
                                });
                            }
                            b"stop" => train_action = Some(TrainAction::Stop),
                            b"toggle" if path.track => {
                                let speed = if f32::from_bits(path.current_speed_bits) == 0.0 {
                                    f32::from_bits(path.maximum_speed_bits)
                                } else {
                                    0.0
                                };
                                train_action = Some(TrainAction::SetSpeed {
                                    speed,
                                    accelerate: false,
                                });
                            }
                            b"toggle" => {
                                path.running = !path.running;
                                train_action = Some(if path.running {
                                    TrainAction::Start
                                } else {
                                    TrainAction::Stop
                                });
                            }
                            b"reverse" => {
                                self.set_train_direction(path, !path.forward);
                                train_action = Some(TrainAction::Reverse);
                            }
                            b"startforward" | b"startbackward" => {
                                self.set_train_direction(path, input == b"startforward");
                                train_action = Some(TrainAction::SetSpeed {
                                    speed: f32::from_bits(path.maximum_speed_bits),
                                    accelerate: false,
                                });
                            }
                            b"setspeed" | b"setspeeddir" | b"setspeedreal"
                            | b"setspeeddiraccel" => {
                                match record.value.as_float().filter(|value| value.is_finite()) {
                                    Some(value) => {
                                        let maximum = f32::from_bits(path.maximum_speed_bits).abs();
                                        if input == b"setspeeddir" || input == b"setspeeddiraccel" {
                                            self.set_train_direction(path, value >= 0.0);
                                        }
                                        let speed = if input == b"setspeedreal" {
                                            value.clamp(0.0, maximum)
                                        } else if input == b"setspeed" {
                                            value.clamp(0.0, 1.0) * maximum
                                        } else {
                                            value.abs().clamp(0.0, 1.0) * maximum
                                        };
                                        train_action = Some(TrainAction::SetSpeed {
                                            speed,
                                            accelerate: input == b"setspeeddiraccel",
                                        });
                                    }
                                    None => accepted = false,
                                }
                            }
                            b"setspeedforwardmodifier" if path.track => {
                                match record.value.as_float().filter(|value| value.is_finite()) {
                                    Some(value) => {
                                        train_action = Some(TrainAction::SetForwardModifier(value));
                                    }
                                    None => accepted = false,
                                }
                            }
                            b"teleporttopathtrack" if path.track => {
                                match self.variant_string(&record.value) {
                                    Some(value) => {
                                        train_action = Some(TrainAction::Teleport(value))
                                    }
                                    None => accepted = false,
                                }
                            }
                            _ => accepted = false,
                        }
                    } else if state.class == MoverClass::Rotating {
                        let current = f32::from_bits(state.continuous_speed_bits);
                        let rotator = state.rotator.as_mut().expect("rotator state");
                        let requested = match input.as_slice() {
                            b"__rotating_start" => Some(f32::from_bits(rotator.target_speed_bits)),
                            b"__rotator_adjust" => {
                                rotator_adjust = true;
                                None
                            }
                            b"start" => Some(if rotator.reversed {
                                -state.speed
                            } else {
                                state.speed
                            }),
                            b"startforward" => {
                                rotator.reversed = false;
                                Some(state.speed)
                            }
                            b"startbackward" => {
                                rotator.reversed = true;
                                Some(-state.speed)
                            }
                            b"stop" => Some(0.0),
                            b"toggle" => Some(if current == 0.0 { state.speed } else { 0.0 }),
                            b"reverse" => {
                                rotator.stop_at_start = false;
                                rotator.reversed = !rotator.reversed;
                                Some(if rotator.reversed {
                                    -current.abs()
                                } else {
                                    current.abs()
                                })
                            }
                            b"setspeed" => record
                                .value
                                .as_float()
                                .filter(|value| value.is_finite())
                                .map(|value| {
                                    rotator.stop_at_start = false;
                                    rotator.reversed = value < 0.0;
                                    value.abs().clamp(0.0, 1.0)
                                        * state.speed
                                        * if rotator.reversed { -1.0 } else { 1.0 }
                                }),
                            b"stopatstartpos" => {
                                rotator.stop_at_start = true;
                                Some(0.0)
                            }
                            _ => None,
                        };
                        if let Some(target_speed) = requested {
                            rotator.target_speed_bits = target_speed.to_bits();
                            rotator_target = Some(target_speed);
                        } else if !rotator_adjust {
                            accepted = false;
                        }
                    } else {
                        match state.kind {
                            MoverKind::Door => match input.as_slice() {
                                b"open" => {
                                    mover_activator = state.activator;
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
                                    mover_activator = state.activator;
                                    if state.position != MoverPosition::Closed {
                                        mover = Some((0.0, false));
                                    }
                                }
                                b"toggle" => {
                                    mover_activator = state.activator;
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
                                b"use" => {
                                    state.activator = record.activator;
                                    mover_activator = record.activator;
                                    if !state.locked {
                                        match state.position {
                                            MoverPosition::Closed => mover = Some((1.0, true)),
                                            MoverPosition::Open if state.no_auto_return => {
                                                mover = Some((0.0, false));
                                            }
                                            _ => {}
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
                                b"press" => {
                                    state.activator = record.activator;
                                    mover_activator = record.activator;
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
                                b"use" => {
                                    state.activator = record.activator;
                                    mover_activator = record.activator;
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
                                    mover_activator = state.activator;
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
                                    mover_activator = state.activator;
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
                                b"goup"
                                    if matches!(
                                        state.class,
                                        MoverClass::Platform | MoverClass::RotatingPlatform
                                    ) =>
                                {
                                    mover = Some((1.0, true));
                                }
                                b"godown"
                                    if matches!(
                                        state.class,
                                        MoverClass::Platform | MoverClass::RotatingPlatform
                                    ) =>
                                {
                                    mover = Some((0.0, false));
                                }
                                b"toggle"
                                    if matches!(
                                        state.class,
                                        MoverClass::Platform | MoverClass::RotatingPlatform
                                    ) =>
                                {
                                    match state.position {
                                        MoverPosition::Closed => mover = Some((1.0, true)),
                                        MoverPosition::Open => mover = Some((0.0, false)),
                                        _ => {}
                                    }
                                }
                                b"setposition" => match record.value.as_float() {
                                    Some(value) if value.is_finite() => {
                                        mover = Some((value, value >= 0.5));
                                    }
                                    _ => accepted = false,
                                },
                                b"setpositionimmediately"
                                    if state.class == MoverClass::MomentaryRotatingButton =>
                                {
                                    match record.value.as_float() {
                                        Some(value) if value.is_finite() => {
                                            immediate_mover_position = Some(value);
                                            state.position =
                                                MoverPosition::Positioned(value.to_bits());
                                            state.pending = None;
                                        }
                                        _ => accepted = false,
                                    }
                                }
                                b"setspeed" => match record.value.as_float() {
                                    Some(speed) if speed.is_finite() => {
                                        state.speed = speed;
                                        if let Some(pending) = &state.pending {
                                            let travel = sub(state.open, state.closed);
                                            let denominator = dot(travel, travel);
                                            if denominator > f32::EPSILON {
                                                mover = Some((
                                                    dot(
                                                        sub(
                                                            pending.local_destination,
                                                            state.closed,
                                                        ),
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
                BehaviorState::Timer(mut state) => {
                    match input.as_slice() {
                        b"enable" => {
                            state.enabled = true;
                            timer_action = Some(TimerAction::Reset);
                        }
                        b"disable" => {
                            state.enabled = false;
                            state.next_fire_tick = None;
                            timer_action = Some(TimerAction::Disable);
                        }
                        b"toggle" => {
                            state.enabled = !state.enabled;
                            state.next_fire_tick = None;
                            timer_action = Some(if state.enabled {
                                TimerAction::Reset
                            } else {
                                TimerAction::Disable
                            });
                        }
                        b"firetimer" | b"__timer_fire" => {
                            if state.enabled {
                                outputs.push((
                                    if state.alternating {
                                        if state.high_next {
                                            b"OnTimerHigh".as_slice()
                                        } else {
                                            b"OnTimerLow".as_slice()
                                        }
                                    } else {
                                        b"OnTimer".as_slice()
                                    }
                                    .to_vec(),
                                    Variant::Void,
                                    Some(target),
                                ));
                                if state.alternating {
                                    state.high_next = !state.high_next;
                                }
                                timer_action = Some(TimerAction::Reset);
                            }
                        }
                        b"refiretime" => match record.value.as_float() {
                            Some(value) if value.is_finite() => {
                                let interval = value.max(0.01);
                                if interval.to_bits() != state.interval_bits {
                                    state.interval_bits = interval.to_bits();
                                    if state.enabled {
                                        timer_action = Some(TimerAction::Reset);
                                    }
                                }
                            }
                            _ => accepted = false,
                        },
                        b"resettimer" => {
                            if state.enabled {
                                timer_action = Some(TimerAction::Reset);
                            }
                        }
                        b"addtotimer" | b"subtractfromtimer" => match record.value.as_float() {
                            Some(value) if value.is_finite() => {
                                if state.enabled {
                                    let ticks =
                                        delay_ticks(value.max(0.0), self.config.tick_interval);
                                    timer_action = Some(if input == b"addtotimer" {
                                        TimerAction::Add(ticks)
                                    } else {
                                        TimerAction::Subtract(ticks)
                                    });
                                }
                            }
                            _ => accepted = false,
                        },
                        b"userandomtime" => match variant_i32(&record.value) {
                            Some(value) => state.use_random = value != 0,
                            None => accepted = false,
                        },
                        b"lowerrandombound" => match record.value.as_float() {
                            Some(value) if value.is_finite() => {
                                state.lower_bound_bits = value.to_bits();
                            }
                            _ => accepted = false,
                        },
                        b"upperrandombound" => match record.value.as_float() {
                            Some(value) if value.is_finite() => {
                                state.upper_bound_bits = value.to_bits();
                            }
                            _ => accepted = false,
                        },
                        _ => accepted = false,
                    }
                    if accepted {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Timer(state);
                    }
                }
                BehaviorState::Template(_) => {
                    if input == b"forcespawn" {
                        template_spawn = true;
                    } else {
                        accepted = false;
                    }
                }
                BehaviorState::PathNode(mut state) => {
                    match input.as_slice() {
                        b"inpass" => {
                            outputs.push((b"OnPass".to_vec(), Variant::Void, record.activator))
                        }
                        b"inteleport" if state.track => {
                            outputs.push((b"OnTeleport".to_vec(), Variant::Void, record.activator))
                        }
                        b"setnextpathcorner" if !state.track => {
                            if let Some(value) = self.variant_string(&record.value) {
                                state.next_name = value;
                            } else {
                                accepted = false;
                            }
                            state.next = self
                                .resolve(
                                    &state.next_name,
                                    Some(target),
                                    record.activator,
                                    record.caller,
                                )
                                .into_iter()
                                .find(|handle| {
                                    matches!(
                                        self.entity(*handle).map(|entity| &entity.behavior),
                                        Some(BehaviorState::PathNode(_))
                                    )
                                });
                        }
                        b"enablealternatepath" if state.track && state.alternate.is_some() => {
                            state.flags |= 0x8000;
                        }
                        b"disablealternatepath" if state.track && state.alternate.is_some() => {
                            state.flags &= !0x8000;
                        }
                        b"togglealternatepath" if state.track && state.alternate.is_some() => {
                            state.flags ^= 0x8000;
                        }
                        b"enablepath" if state.track => state.flags &= !1,
                        b"disablepath" if state.track => state.flags |= 1,
                        b"togglepath" if state.track => state.flags ^= 1,
                        _ => accepted = false,
                    }
                    if accepted {
                        self.entity_mut(target).expect("path node").behavior =
                            BehaviorState::PathNode(state);
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
                        b"__trigger_hurt_tick" if state.kind == TriggerKind::Hurt => {
                            trigger_hurt_tick = true;
                        }
                        b"__trigger_hurt_reset" if state.kind == TriggerKind::Hurt => {
                            if state.contacts.is_empty() {
                                state.mutable_value_bits = state.original_damage_bits;
                            }
                        }
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
                        b"setplayerspeed" if state.kind == TriggerKind::Catapult => {
                            match record.value.as_float().filter(|value| value.is_finite()) {
                                Some(value) => state.speed_bits = value.to_bits(),
                                None => accepted = false,
                            }
                        }
                        b"setphysicsspeed" if state.kind == TriggerKind::Catapult => {
                            match record.value.as_float().filter(|value| value.is_finite()) {
                                Some(value) => state.catapult_physics_speed_bits = value.to_bits(),
                                None => accepted = false,
                            }
                        }
                        b"setlaunchtarget" if state.kind == TriggerKind::Catapult => {
                            if let Some(value) = self.variant_string(&record.value) {
                                state.target_name = value;
                            } else {
                                accepted = false;
                            }
                        }
                        b"setexactvelocitychoicetype" if state.kind == TriggerKind::Catapult => {
                            match variant_i32(&record.value) {
                                Some(value) => state.catapult_exact_choice = value,
                                None => accepted = false,
                            }
                        }
                        _ => accepted = false,
                    }
                    if accepted && write_state {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Trigger(state);
                    }
                }
                BehaviorState::External => {
                    let classname = &self.entity(target).expect("validated").classname;
                    accepted = self.config.external_classes.iter().any(|binding| {
                        binding.classname.eq_ignore_ascii_case(classname)
                            && binding
                                .inputs
                                .iter()
                                .any(|declared| declared.eq_ignore_ascii_case(&record.input))
                    });
                    if accepted {
                        self.push_transition(
                            batch,
                            Transition::Request(RuntimeRequest::ExternalInput {
                                entity: target,
                                input: record.input.clone(),
                                value: record.value.clone(),
                                activator: record.activator,
                                caller: record.caller,
                            }),
                        )?;
                    }
                }
                BehaviorState::Breakable(mut state) => {
                    match input.as_slice() {
                        b"break" if state.can_break && !state.broken => {
                            state.broken = true;
                            state.breaker = record.activator;
                            breakable_became_nonsolid = true;
                            breakable_remove_delay = Some(0.1);
                            outputs.push((b"OnBreak".to_vec(), Variant::Void, record.activator));
                        }
                        b"break" => {}
                        b"sethealth" | b"addhealth" | b"removehealth" if !state.broken => {
                            match variant_i32(&record.value) {
                                Some(value) => {
                                    let health = match input.as_slice() {
                                        b"sethealth" => value,
                                        b"addhealth" => state.health.saturating_add(value),
                                        b"removehealth" => state.health.saturating_sub(value),
                                        _ => unreachable!(),
                                    };
                                    if health != state.health {
                                        state.health = health;
                                        let ratio = (state.health as f32
                                            / state.maximum_health as f32)
                                            .clamp(0.0, 1.0);
                                        breakable_health_output = Some(ratio);
                                        if state.health <= 0 && state.can_break {
                                            state.broken = true;
                                            state.breaker = record.activator;
                                            breakable_became_nonsolid = true;
                                            breakable_remove_delay = Some(0.1);
                                            outputs.push((
                                                b"OnBreak".to_vec(),
                                                Variant::Void,
                                                record.activator,
                                            ));
                                        }
                                    }
                                }
                                None => accepted = false,
                            }
                        }
                        b"__breakable_remove" => remove = true,
                        _ => accepted = false,
                    }
                    if accepted {
                        self.entity_mut(target).expect("validated").behavior =
                            BehaviorState::Breakable(state);
                    }
                }
                BehaviorState::DynamicProp(mut state) => {
                    match input.as_slice() {
                        b"skin" => match variant_i32(&record.value) { Some(value) => state.skin = value, None => accepted = false },
                        b"setanimation" => {
                            let animation = self.variant_string(&record.value).unwrap_or_default();
                            if animation.is_empty() {
                                accepted = false;
                            } else {
                                state.requested_animation = Some(animation);
                            }
                        }
                        b"setdefaultanimation" => {
                            if let Some(value) = self.variant_string(&record.value) {
                                state.default_animation = value;
                            } else {
                                accepted = false;
                            }
                        }
                        b"turnon" | b"enable" => {
                            state.visible = true;
                            self.entity_mut(target)
                                .expect("dynamic prop")
                                .render
                                .effects &= !EF_NODRAW;
                        }
                        b"turnoff" | b"disable" => {
                            state.visible = false;
                            self.entity_mut(target)
                                .expect("dynamic prop")
                                .render
                                .effects |= EF_NODRAW;
                        }
                        b"enablecollision" => state.collision_enabled = true,
                        b"disablecollision" => state.collision_enabled = false,
                        _ => accepted = false,
                    }
                    if accepted {
                        let request_input = record.input.clone();
                        let request_value = record.value.clone();
                        self.entity_mut(target).expect("dynamic prop").behavior =
                            BehaviorState::DynamicProp(state);
                        if matches!(
                            input.as_slice(),
                            b"setanimation" | b"enablecollision" | b"disablecollision"
                        ) {
                            self.push_transition(
                                batch,
                                Transition::Request(RuntimeRequest::ExternalInput {
                                    entity: target,
                                    input: request_input,
                                    value: request_value,
                                    activator: record.activator,
                                    caller: record.caller,
                                }),
                            )?;
                        }
                    }
                }
                BehaviorState::Pickup(mut state) => {
                    let auto_materialize = field_i32(
                        &self.entity(target).expect("pickup").definition,
                        b"AutoMaterialize",
                        1,
                    )? != 0;
                    let pending_materialize = self.state.queue.iter().any(|event| {
                        event.target == EventTarget::Direct(target)
                            && event.input.eq_ignore_ascii_case(b"__pickup_materialize")
                    });
                    match input.as_slice() {
                        b"__pickup_materialize" if auto_materialize => {
                            state.touchable = true;
                            state.visible = true;
                            state.pending_subject = None;
                        }
                        b"__pickup_materialize" => {}
                        b"enable" => {
                            if !pending_materialize || !auto_materialize {
                                state.touchable = true;
                                state.visible = true;
                            }
                        }
                        b"disable" => {
                            state.touchable = false;
                            state.visible = false;
                        }
                        b"toggle" => {
                            state.visible = !state.visible;
                            state.touchable = state.visible;
                        }
                        _ => accepted = false,
                    }
                    if accepted {
                        let entity = self.entity_mut(target).expect("pickup");
                        if state.visible {
                            entity.render.effects &= !EF_NODRAW;
                        } else {
                            entity.render.effects |= EF_NODRAW;
                        }
                        entity.behavior = BehaviorState::Pickup(state);
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
            self.start_mover(target, position, opening, mover_activator, batch)?;
        }
        if let Some(position) = immediate_mover_position {
            self.set_mover_position_immediately(target, position, batch)?;
        }
        if let Some(speed) = rotator_target {
            self.apply_rotator_target(target, speed, batch)?;
        }
        if rotator_adjust {
            self.adjust_rotator(target, batch)?;
        }
        if let Some(action) = train_action {
            self.apply_train_action(target, action, batch)?;
        }
        if let Some(ratio) = breakable_health_output {
            self.fire_output(
                target,
                b"OnHealthChanged",
                Variant::float(ratio),
                record.activator,
                Some(target),
                0.0,
                batch,
            )?;
        }
        for (output, value, activator) in outputs {
            self.fire_output(target, &output, value, activator, target.into(), 0.0, batch)?;
        }
        if record.input.eq_ignore_ascii_case(b"InPass") && self.entity(target).is_some_and(|entity| matches!(&entity.behavior, BehaviorState::PathNode(path) if path.track)) {
            self.push_transition(batch, Transition::PathTrackPassed { node: target })?;
        }
        if breakable_became_nonsolid {
            self.push_transition(
                batch,
                Transition::Request(RuntimeRequest::BrushState {
                    entity: target,
                    enabled: true,
                    solid: false,
                }),
            )?;
        }
        if let Some(delay) = breakable_remove_delay {
            self.schedule_event(
                EventTarget::Direct(target),
                b"__breakable_remove".to_vec(),
                Variant::Void,
                delay,
                record.activator,
                Some(target),
                None,
                batch,
            )?;
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
        if let Some(action) = timer_action {
            self.apply_timer_action(target, action, batch)?;
        }
        if template_spawn {
            self.spawn_template_instance(target, batch)?;
        }
        if trigger_hurt_tick {
            self.hurt_trigger_tick(target, batch)?;
        }
        if remove {
            self.mark_removal(target, batch)?;
        }
        if remove_child_first {
            self.mark_removal_child_first(target, batch)?;
        }
        Ok(())
    }

    fn assign_input_field(
        &mut self,
        target: EntityHandle,
        input: &[u8],
        value: &Variant,
    ) -> Option<Result<(), ValueConversionError>> {
        let (index, field_type) =
            self.entity(target)?
                .fields
                .iter()
                .enumerate()
                .find_map(|(index, field)| {
                    field
                        .writable_input
                        .as_deref()
                        .is_some_and(|declared| declared.eq_ignore_ascii_case(input))
                        .then_some((index, field.field_type))
                })?;
        let converted = self.convert_variant(value, field_type);
        Some(converted.map(|converted| {
            let field = &mut self
                .entity_mut(target)
                .expect("validated field target")
                .fields[index];
            field.value = Some(converted);
            field.coverage = Coverage::Handled;
        }))
    }

    fn variant_string(&self, value: &Variant) -> Option<Vec<u8>> {
        match self.convert_variant(value, FieldType::String).ok()? {
            Variant::String(value) => Some(value),
            _ => None,
        }
    }

    fn apply_add_output(
        &mut self,
        target: EntityHandle,
        value: &[u8],
    ) -> Result<bool, RuntimeFailure> {
        let bounded = &value[..value.len().min(259)];
        let Some(split) = bounded.iter().position(|byte| *byte == b' ') else {
            return Ok(false);
        };
        let key = &bounded[..split];
        let mut raw = bounded[split + 1..].to_vec();
        for byte in &mut raw {
            if *byte == b':' {
                *byte = b',';
            }
        }
        let pair = Pair {
            key: key.to_vec(),
            value: raw.clone(),
            key_range: 0..key.len(),
            value_range: split + 1..bounded.len(),
        };
        if (key.len() >= 2 && key[..2].eq_ignore_ascii_case(b"on"))
            || (key.len() >= 3 && key[..3].eq_ignore_ascii_case(b"out"))
        {
            let Some(connection) = super::connection(usize::MAX, &pair) else {
                return Ok(false);
            };
            let Connection::Parsed {
                output,
                target: action_target,
                input,
                parameter,
                delay_bits,
                max_fires,
                ..
            } = connection
            else {
                return Ok(false);
            };
            let output_count = self
                .state
                .slots
                .iter()
                .filter_map(|slot| slot.entity.as_ref())
                .map(|entity| entity.outputs.len())
                .sum::<usize>();
            if output_count >= self.config.limits.max_output_actions {
                return Err(failure(
                    RuntimeFailureCode::OutputLimit,
                    self.entity(target).map(|entity| entity.source_index),
                    self.config.limits.max_output_actions,
                    output_count + 1,
                ));
            }
            let id = self.state.next_output_id;
            self.state.next_output_id += 1;
            self.entity_mut(target)
                .expect("add-output target")
                .outputs
                .push(OutputActionState {
                    id,
                    declaration_order: usize::MAX,
                    output,
                    target: action_target,
                    input,
                    parameter,
                    delay_bits,
                    remaining_fires: max_fires,
                });
            return Ok(true);
        }
        if key.eq_ignore_ascii_case(b"targetname") {
            self.index_remove(target);
            let classname = self
                .entity(target)
                .expect("add-output target")
                .classname
                .clone();
            let targetname = (!raw.is_empty()).then_some(raw);
            self.entity_mut(target)
                .expect("add-output target")
                .targetname = targetname.clone();
            self.index_insert(&classname, targetname.as_deref(), target);
            return Ok(true);
        }
        if apply_render_key(
            &mut self.entity_mut(target).expect("add-output target").render,
            bounded,
        ) {
            return Ok(true);
        }
        let field = self.entity(target).and_then(|entity| {
            entity
                .fields
                .iter()
                .enumerate()
                .find(|(_, field)| field.key.eq_ignore_ascii_case(key))
                .map(|(index, field)| (index, field.field_type))
        });
        let Some((index, field_type)) = field else {
            return Ok(false);
        };
        let projected = project_string(&raw, field_type, |name| {
            self.resolve(name, Some(target), None, Some(target))
                .first()
                .copied()
        });
        let Ok(projected) = projected else {
            return Ok(false);
        };
        let field = &mut self.entity_mut(target).expect("add-output target").fields[index];
        field.value = Some(projected);
        field.coverage = Coverage::Handled;
        Ok(true)
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
            let Some(value) = self.variant_string(&record.value) else {
                return Ok(false);
            };
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
        self.state.random.integer(0, count.saturating_sub(1) as i32) as usize
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

    fn reset_timer(
        &mut self,
        target: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        self.cancel_direct(target, b"__timer_fire", batch)?;
        let Some(BehaviorState::Timer(mut state)) =
            self.entity(target).map(|entity| entity.behavior.clone())
        else {
            return Ok(());
        };
        if !state.enabled {
            state.next_fire_tick = None;
            self.entity_mut(target).expect("timer target").behavior = BehaviorState::Timer(state);
            return Ok(());
        }
        let interval = if state.use_random {
            let lower = f32::from_bits(state.lower_bound_bits);
            let upper = f32::from_bits(state.upper_bound_bits);
            let value = self.state.random.float(lower, upper);
            state.interval_bits = value.to_bits();
            value
        } else {
            f32::from_bits(state.interval_bits)
        };
        let ticks = delay_ticks(interval.max(0.0), self.config.tick_interval);
        let due_tick = self.state.current_tick.saturating_add(ticks);
        state.next_fire_tick = Some(due_tick);
        self.entity_mut(target).expect("timer target").behavior = BehaviorState::Timer(state);
        self.schedule_event_ticks(
            EventTarget::Direct(target),
            b"__timer_fire".to_vec(),
            Variant::Void,
            ticks,
            Some(target),
            Some(target),
            None,
            batch,
        )?;
        Ok(())
    }

    fn apply_timer_action(
        &mut self,
        target: EntityHandle,
        action: TimerAction,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        match action {
            TimerAction::Reset => self.reset_timer(target, batch),
            TimerAction::Disable => {
                self.cancel_direct(target, b"__timer_fire", batch)?;
                Ok(())
            }
            TimerAction::Add(ticks) | TimerAction::Subtract(ticks) => {
                let current_tick = self.state.current_tick;
                let mut due = None;
                if let Some(event) = self.state.queue.iter_mut().find(|event| {
                    matches!(event.target, EventTarget::Direct(handle) if handle == target)
                        && event.input == b"__timer_fire"
                }) {
                    event.due_tick = if matches!(action, TimerAction::Add(_)) {
                        event.due_tick.saturating_add(ticks)
                    } else {
                        event.due_tick.saturating_sub(ticks).max(current_tick)
                    };
                    due = Some(event.due_tick);
                }
                if let Some(BehaviorState::Timer(state)) =
                    self.entity_mut(target).map(|entity| &mut entity.behavior)
                {
                    state.next_fire_tick = due;
                }
                self.state
                    .queue
                    .sort_by_key(|event| (event.due_tick, event.enqueue_sequence));
                Ok(())
            }
        }
    }

    fn spawn_template_instance(
        &mut self,
        template: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<Vec<EntityHandle>, RuntimeFailure> {
        let (template_transform, state) = {
            let entity = self
                .entity(template)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            let BehaviorState::Template(state) = &entity.behavior else {
                return Err(failure(
                    RuntimeFailureCode::InvalidField,
                    Some(entity.source_index),
                    0,
                    0,
                ));
            };
            (entity.world_transform, state.clone())
        };
        if state.instances >= self.config.limits.max_template_instances {
            return Err(failure(
                RuntimeFailureCode::TemplateLimit,
                self.entity(template).map(|entity| entity.source_index),
                self.config.limits.max_template_instances,
                state.instances + 1,
            ));
        }
        if self
            .live_handles()
            .len()
            .saturating_add(state.members.len())
            > self.config.limits.max_entities
        {
            return Err(failure(
                RuntimeFailureCode::EntityLimit,
                self.entity(template).map(|entity| entity.source_index),
                self.config.limits.max_entities,
                self.live_handles()
                    .len()
                    .saturating_add(state.members.len()),
            ));
        }
        self.state.next_template_instance = (self.state.next_template_instance + 1) % 10_000;
        let suffix = format!("&{:04}", self.state.next_template_instance).into_bytes();
        let mut members = state.members;
        if !state.preserve_names {
            fixup_template_definitions(&mut members, &suffix);
        }

        let mut created = Vec::with_capacity(members.len());
        for member in &members {
            let handle = self.spawn_definition(member.definition.clone(), batch)?;
            let transform = compose_transform(template_transform, member.relative_transform);
            self.apply_command(
                WorldCommand::SetWorldTransform {
                    entity: handle,
                    transform,
                },
                batch,
            )?;
            created.push(handle);
        }
        self.refresh_projected_fields();
        for handle in &created {
            let Some(raw) = self
                .entity(*handle)
                .and_then(|entity| entity.definition.parentname.clone())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let mut fields = raw.splitn(2, |byte| *byte == b',');
            let name = fields.next().unwrap_or_default();
            let attachment = fields
                .next()
                .filter(|value| !value.is_empty())
                .map(<[u8]>::to_vec);
            let Some(parent) = self
                .resolve(name, Some(*handle), None, None)
                .first()
                .copied()
            else {
                continue;
            };
            self.set_parent(
                ParentRequest {
                    child: *handle,
                    parent: Some(parent),
                    attachment,
                    mode: ParentMode::MaintainWorld,
                },
                batch,
            )?;
        }
        for handle in &created {
            self.entity_mut(*handle).expect("template member").lifecycle = Lifecycle::Activated;
            self.push_transition(
                batch,
                Transition::Lifecycle {
                    entity: *handle,
                    state: Lifecycle::Activated,
                },
            )?;
            self.schedule_activation(*handle, batch)?;
        }
        if let BehaviorState::Template(state) =
            &mut self.entity_mut(template).expect("template").behavior
        {
            state.instances += 1;
        }
        self.fire_output(
            template,
            b"OnEntitySpawned",
            Variant::Void,
            Some(template),
            Some(template),
            0.0,
            batch,
        )?;
        Ok(created)
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
        match trigger_state.kind {
            TriggerKind::Soundscape => {}
            TriggerKind::Multiple => self.emit_trigger_effect(
                record.trigger,
                record.subject,
                record.kind,
                &trigger_state,
                batch,
            )?,
            TriggerKind::Hurt => {
                if record.kind == ContactKind::Enter && trigger_state.next_hurt_tick.is_none() {
                    let current_tick = self.state.current_tick;
                    if let BehaviorState::Trigger(state) = &mut self
                        .entity_mut(record.trigger)
                        .expect("hurt trigger")
                        .behavior
                    {
                        state.next_hurt_tick = Some(current_tick);
                    }
                    self.schedule_event_ticks(
                        EventTarget::Direct(record.trigger),
                        b"__trigger_hurt_tick".to_vec(),
                        Variant::Void,
                        0,
                        Some(record.subject),
                        Some(record.trigger),
                        None,
                        batch,
                    )?;
                }
            }
            TriggerKind::Push => {
                self.emit_trigger_effect(
                    record.trigger,
                    record.subject,
                    record.kind,
                    &trigger_state,
                    batch,
                )?;
                if trigger_state.push_once && record.kind == ContactKind::Enter {
                    self.mark_removal(record.trigger, batch)?;
                }
            }
            TriggerKind::Catapult => {
                let due = trigger_state
                    .catapult_cooldowns
                    .iter()
                    .find_map(|(subject, tick)| (*subject == record.subject).then_some(*tick))
                    .unwrap_or(0);
                if self.state.current_tick >= due {
                    self.emit_trigger_effect(
                        record.trigger,
                        record.subject,
                        record.kind,
                        &trigger_state,
                        batch,
                    )?;
                    let next = self
                        .state
                        .current_tick
                        .saturating_add(delay_ticks(0.5, self.config.tick_interval));
                    if let BehaviorState::Trigger(state) =
                        &mut self.entity_mut(record.trigger).expect("catapult").behavior
                    {
                        if let Some((_, tick)) = state
                            .catapult_cooldowns
                            .iter_mut()
                            .find(|(subject, _)| *subject == record.subject)
                        {
                            *tick = next;
                        } else {
                            state.catapult_cooldowns.push((record.subject, next));
                        }
                    }
                }
            }
            TriggerKind::Teleport => self.emit_trigger_effect(
                record.trigger,
                record.subject,
                record.kind,
                &trigger_state,
                batch,
            )?,
        }
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
            if trigger_state.wait_ticks == 0 {
                self.schedule_event(
                    EventTarget::Direct(record.trigger),
                    b"Kill".to_vec(),
                    Variant::Void,
                    0.1,
                    Some(record.subject),
                    Some(record.trigger),
                    None,
                    batch,
                )?;
            }
        }
        Ok(())
    }

    fn emit_trigger_effect(
        &mut self,
        trigger: EntityHandle,
        subject: EntityHandle,
        contact: ContactKind,
        state: &TriggerState,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let local_direction = state.direction.map(f32::from_bits);
        let world_direction = quat_rotate(
            quat_from_angles(
                self.entity(trigger)
                    .map_or([0.0; 3], |entity| entity.world_transform.angles),
            ),
            local_direction,
        );
        let effect = match state.kind {
            TriggerKind::Soundscape => return Ok(()),
            TriggerKind::Multiple => TriggerEffectData::Multiple,
            TriggerKind::Hurt => TriggerEffectData::Hurt {
                damage_bits: state.mutable_value_bits,
                damage_type: state.damage_type,
                no_force: state.no_damage_force,
            },
            TriggerKind::Push => TriggerEffectData::Push {
                velocity: scale(world_direction, f32::from_bits(state.speed_bits)),
                mode: if state.push_once {
                    PushMode::ImpulseAndRemove
                } else {
                    PushMode::BaseVelocity
                },
            },
            TriggerKind::Catapult => TriggerEffectData::Catapult {
                direction: world_direction,
                player_speed_bits: state.speed_bits,
                physics_speed_bits: state.catapult_physics_speed_bits,
                target: self
                    .resolve(
                        &state.target_name,
                        Some(trigger),
                        Some(subject),
                        Some(subject),
                    )
                    .first()
                    .copied(),
                exact_velocity: state.catapult_exact_velocity,
                exact_choice: state.catapult_exact_choice,
                threshold: state.catapult_threshold,
                lower_bits: state.catapult_lower_bits,
                upper_bits: state.catapult_upper_bits,
            },
            TriggerKind::Teleport => TriggerEffectData::Teleport {
                destination: self
                    .resolve(
                        &state.target_name,
                        Some(trigger),
                        Some(subject),
                        Some(subject),
                    )
                    .first()
                    .copied(),
                landmark: self
                    .resolve(
                        &state.landmark_name,
                        Some(trigger),
                        Some(subject),
                        Some(subject),
                    )
                    .first()
                    .copied(),
                preserve_angles: state.preserve_angles,
            },
        };
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::TriggerEffect {
                trigger,
                subject,
                kind: state.kind,
                contact,
                effect,
            }),
        )
    }

    fn hurt_trigger_tick(
        &mut self,
        trigger: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let Some(BehaviorState::Trigger(mut state)) =
            self.entity(trigger).map(|entity| entity.behavior.clone())
        else {
            return self.diagnostic(batch, DiagnosticCode::StaleHandle, Some(trigger));
        };
        state
            .contacts
            .retain(|subject| self.is_resolvable(*subject));
        state.hurt_last_cycle.clear();
        let damage = f32::from_bits(state.mutable_value_bits) * 0.5;
        let mut effect_state = state.clone();
        effect_state.mutable_value_bits = damage.to_bits();
        for subject in &state.contacts {
            self.emit_trigger_effect(trigger, *subject, ContactKind::Stay, &effect_state, batch)?;
            state.hurt_last_cycle.push(*subject);
        }
        if state.damage_model == 1 && !state.contacts.is_empty() {
            state.mutable_value_bits = (f32::from_bits(state.mutable_value_bits) * 2.0)
                .min(f32::from_bits(state.damage_cap_bits))
                .to_bits();
        }
        if state.contacts.is_empty() {
            state.next_hurt_tick = None;
        } else {
            let ticks = delay_ticks(0.5, self.config.tick_interval);
            state.next_hurt_tick = Some(self.state.current_tick.saturating_add(ticks));
            self.schedule_event_ticks(
                EventTarget::Direct(trigger),
                b"__trigger_hurt_tick".to_vec(),
                Variant::Void,
                ticks,
                Some(trigger),
                Some(trigger),
                None,
                batch,
            )?;
        }
        self.entity_mut(trigger).expect("hurt trigger").behavior = BehaviorState::Trigger(state);
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
        if state.kind == TriggerKind::Hurt && !state.hurt_last_cycle.contains(&subject) {
            let mut effect_state = state.clone();
            effect_state.mutable_value_bits =
                (f32::from_bits(state.mutable_value_bits) * 0.5).to_bits();
            self.emit_trigger_effect(trigger, subject, ContactKind::Exit, &effect_state, batch)?;
        }
        if let BehaviorState::Trigger(state) =
            &mut self.entity_mut(trigger).expect("validated").behavior
        {
            state.contacts.remove(at);
            state.hurt_last_cycle.retain(|handle| *handle != subject);
            state
                .catapult_cooldowns
                .retain(|(handle, _)| *handle != subject);
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
            if state.kind == TriggerKind::Hurt && state.damage_model == 1 {
                self.schedule_event(
                    EventTarget::Direct(trigger),
                    b"__trigger_hurt_reset".to_vec(),
                    Variant::Void,
                    3.0,
                    Some(trigger),
                    Some(trigger),
                    None,
                    batch,
                )?;
            }
        }
        Ok(())
    }

    fn start_mover(
        &mut self,
        entity: EntityHandle,
        position: f32,
        opening: bool,
        activator: Option<EntityHandle>,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (kind, speed, solid, local, local_angles, parent, attachment) = {
            let entity_state = self
                .entity(entity)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            let BehaviorState::Mover(mover) = &entity_state.behavior else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            (
                mover.kind,
                mover.speed,
                mover.solid,
                lerp(mover.closed, mover.open, position),
                lerp(mover.closed_angles, mover.open_angles, position),
                entity_state.parent,
                entity_state.parent_attachment.clone(),
            )
        };
        let basis = match parent {
            Some(parent) => self.parent_basis(parent, attachment.as_deref())?,
            None => Transform::IDENTITY,
        };
        let world_transform = if parent.is_some() {
            compose_transform(
                basis,
                Transform {
                    origin: local,
                    angles: local_angles,
                },
            )
        } else {
            Transform {
                origin: local,
                angles: local_angles,
            }
        };
        let current = self.entity(entity).expect("validated").world_transform;
        let linear_distance = length(sub(world_transform.origin, current.origin));
        let angular_delta = sub(world_transform.angles, current.angles);
        let angular_distance = length(angular_delta);
        let travel_time = if linear_distance > f32::EPSILON {
            linear_distance / speed.abs().max(f32::EPSILON)
        } else {
            angular_distance / speed.abs().max(f32::EPSILON)
        };
        let angular_velocity = if travel_time > f32::EPSILON {
            scale(angular_delta, 1.0 / travel_time)
        } else {
            [0.0; 3]
        };
        let request_id = self.state.next_mover_request_id;
        self.state.next_mover_request_id += 1;
        if let BehaviorState::Mover(mover) =
            &mut self.entity_mut(entity).expect("validated").behavior
        {
            mover.pending = Some(PendingMove {
                request_id,
                local_destination: local,
                world_destination: world_transform.origin,
                local_angles_destination: local_angles,
                world_angles_destination: world_transform.angles,
                angular_velocity,
                continuous: false,
                path_destination: None,
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
                world_destination: world_transform.origin,
                local_angles_destination: local_angles,
                world_angles_destination: world_transform.angles,
                angular_velocity,
                continuous: false,
                solid,
                speed,
                opening,
            }),
        )
    }

    fn set_mover_position_immediately(
        &mut self,
        entity: EntityHandle,
        position: f32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (origin, angles) = {
            let Some(BehaviorState::Mover(state)) =
                self.entity(entity).map(|entity| &entity.behavior)
            else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            (
                lerp(state.closed, state.open, position),
                lerp(state.closed_angles, state.open_angles, position),
            )
        };
        let entity_state = self.entity_mut(entity).expect("immediate mover");
        entity_state.local_transform.origin = origin;
        entity_state.local_transform.angles = angles;
        self.recompute_subtree(entity, 0)?;
        let state = self.entity(entity).expect("immediate mover");
        self.push_transition(
            batch,
            Transition::TransformChanged {
                entity,
                local: state.local_transform,
                world: state.world_transform,
            },
        )?;
        self.fire_output(
            entity,
            b"OnReachedPosition",
            Variant::Void,
            Some(entity),
            Some(entity),
            0.0,
            batch,
        )
    }

    fn start_continuous_rotator(
        &mut self,
        entity: EntityHandle,
        speed: f32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (kind, solid, axis, local, world) = {
            let entity_state = self
                .entity(entity)
                .ok_or_else(|| failure(RuntimeFailureCode::InvalidField, None, 0, usize::MAX))?;
            let BehaviorState::Mover(state) = &entity_state.behavior else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            let angle_delta = sub(state.open_angles, state.closed_angles);
            let axis = if length(angle_delta) > f32::EPSILON {
                scale(angle_delta, 1.0 / length(angle_delta))
            } else {
                [0.0; 3]
            };
            (
                state.kind,
                state.solid,
                axis,
                entity_state.local_transform,
                entity_state.world_transform,
            )
        };
        let angular_velocity = scale(axis, speed);
        let request_id = self.state.next_mover_request_id;
        self.state.next_mover_request_id += 1;
        if let BehaviorState::Mover(state) =
            &mut self.entity_mut(entity).expect("continuous mover").behavior
        {
            state.pending = (speed != 0.0).then_some(PendingMove {
                request_id,
                local_destination: local.origin,
                world_destination: world.origin,
                local_angles_destination: local.angles,
                world_angles_destination: world.angles,
                angular_velocity,
                continuous: true,
                path_destination: None,
                opening: speed >= 0.0,
                activator: Some(entity),
            });
        }
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::Mover {
                request_id,
                entity,
                kind,
                local_destination: local.origin,
                world_destination: world.origin,
                local_angles_destination: local.angles,
                world_angles_destination: world.angles,
                angular_velocity,
                continuous: true,
                solid,
                speed: speed.abs(),
                opening: speed >= 0.0,
            }),
        )
    }

    fn apply_rotator_target(
        &mut self,
        entity: EntityHandle,
        target_speed: f32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let accelerate = self.entity(entity).is_some_and(|entity| {
            matches!(&entity.behavior, BehaviorState::Mover(mover) if mover.rotator.as_ref().is_some_and(|rotator| rotator.accelerate))
        });
        self.cancel_direct(entity, b"__rotator_adjust", batch)?;
        if !accelerate {
            if let BehaviorState::Mover(mover) =
                &mut self.entity_mut(entity).expect("rotator").behavior
            {
                mover.continuous_speed_bits = target_speed.to_bits();
            }
            return self.start_continuous_rotator(entity, target_speed, batch);
        }
        self.schedule_event(
            EventTarget::Direct(entity),
            b"__rotator_adjust".to_vec(),
            Variant::Void,
            0.1,
            Some(entity),
            Some(entity),
            None,
            batch,
        )?;
        Ok(())
    }

    fn adjust_rotator(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (current, target, maximum, friction, stop_at_start, start_angles) = {
            let BehaviorState::Mover(mover) = &self.entity(entity).expect("rotator").behavior
            else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            let rotator = mover.rotator.as_ref().expect("rotator state");
            (
                f32::from_bits(mover.continuous_speed_bits),
                f32::from_bits(rotator.target_speed_bits),
                mover.speed,
                f32::from_bits(rotator.friction_bits),
                rotator.stop_at_start,
                rotator.start_angles.map(f32::from_bits),
            )
        };
        let reversing = current != 0.0 && target != 0.0 && current.signum() != target.signum();
        let mut next = if reversing || current.abs() > target.abs() {
            (current.abs() - 0.1 * maximum * friction).max(0.0) * current.signum()
        } else if current.abs() < target.abs() {
            let value = current.abs() + 0.2 * maximum * friction;
            value.min(target.abs()) * target.signum()
        } else {
            target
        };
        if reversing && next == 0.0 {
            next = 0.0;
        } else if !reversing && next.abs() <= target.abs() && current.abs() > target.abs() {
            next = target;
        }
        let mut snapped = false;
        if stop_at_start && target == 0.0 && next.abs() <= 25.0 {
            let angles = self.entity(entity).expect("rotator").local_transform.angles;
            let delta = sub(angles, start_angles)
                .into_iter()
                .find(|value| value.abs() > f32::EPSILON)
                .unwrap_or(0.0)
                .rem_euclid(360.0);
            let delta = if delta > 180.0 { delta - 360.0 } else { delta };
            if delta.abs() < 1.0 {
                next = 0.0;
                snapped = true;
                let mut transform = self.entity(entity).expect("rotator").world_transform;
                transform.angles = start_angles;
                self.set_entity_world_transform(entity, transform, batch)?;
            }
        }
        if let BehaviorState::Mover(mover) = &mut self.entity_mut(entity).expect("rotator").behavior
        {
            mover.continuous_speed_bits = next.to_bits();
            if snapped {
                let rotator = mover.rotator.as_mut().expect("rotator state");
                rotator.stop_at_start = false;
                rotator.target_speed_bits = 0.0_f32.to_bits();
            }
        }
        self.start_continuous_rotator(entity, next, batch)?;
        if next.to_bits() != target.to_bits() || stop_at_start && !snapped {
            self.schedule_event(
                EventTarget::Direct(entity),
                b"__rotator_adjust".to_vec(),
                Variant::Void,
                0.1,
                Some(entity),
                Some(entity),
                None,
                batch,
            )?;
        }
        Ok(())
    }

    fn set_train_direction(&self, path: &mut TrainPathState, forward: bool) {
        if path.forward == forward {
            return;
        }
        if path.track
            && let Some(current) = path.current
            && let Some(adjusted) = self.path_next(current, !forward, false)
        {
            path.current = Some(adjusted);
        }
        path.forward = forward;
    }

    fn initialize_train(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (track, target_name) = {
            let Some(BehaviorState::Mover(mover)) =
                self.entity(entity).map(|entity| &entity.behavior)
            else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            let path = mover.path.as_ref().expect("train path state");
            (path.track, path.target_name.clone())
        };
        let path = self
            .resolve(&target_name, Some(entity), None, Some(entity))
            .into_iter()
            .find(|handle| {
                matches!(
                    self.entity(*handle).map(|entity| &entity.behavior),
                    Some(BehaviorState::PathNode(node)) if node.track == track
                )
            });
        let Some(path) = path else {
            return self.diagnostic(batch, DiagnosticCode::MissingTarget, Some(entity));
        };
        let node = self.entity(path).expect("train path").clone();
        let (next_name, height, class) = {
            let BehaviorState::PathNode(node_state) = &node.behavior else {
                unreachable!()
            };
            let mover = match &self.entity(entity).expect("train").behavior {
                BehaviorState::Mover(mover) => mover,
                _ => unreachable!(),
            };
            (
                node_state.next_name.clone(),
                f32::from_bits(mover.path.as_ref().expect("train path").height_bits),
                mover.class,
            )
        };
        let mut transform = node.world_transform;
        if class == MoverClass::Train {
            transform.origin = sub(transform.origin, self.mover_model_center(entity));
        } else {
            let authored_angles = self
                .entity(entity)
                .expect("track train")
                .world_transform
                .angles;
            let wheels = match &self.entity(entity).expect("track train").behavior {
                BehaviorState::Mover(mover) => mover
                    .path
                    .as_ref()
                    .map(|path| f32::from_bits(path.length_bits))
                    .unwrap_or(0.0),
                _ => 0.0,
            };
            let look = self.path_look_ahead(path, transform.origin, wheels, false)?;
            let flags = self
                .entity(entity)
                .and_then(|train| field(&train.definition, b"spawnflags"))
                .and_then(parse_i32)
                .unwrap_or(0);
            transform.origin[2] += height;
            transform.angles = if flags & 0x10 != 0 {
                authored_angles
            } else {
                let mut angles = vector_angles(sub(look.position, node.world_transform.origin));
                if flags & 1 != 0 {
                    angles[0] = 0.0;
                }
                angles
            };
        }
        self.set_entity_world_transform(entity, transform, batch)?;
        if let BehaviorState::Mover(mover) = &mut self.entity_mut(entity).expect("train").behavior {
            let path_state = mover.path.as_mut().expect("train path");
            path_state.current = Some(path);
            if class == MoverClass::Train {
                path_state.target_name = next_name;
            }
            mover.closed = transform.origin;
            mover.open = transform.origin;
            mover.closed_angles = transform.angles;
            mover.open_angles = transform.angles;
        }
        if class == MoverClass::TrackTrain {
            self.arrive_track_node(entity, path, batch)?;
        }
        Ok(())
    }

    fn apply_train_action(
        &mut self,
        entity: EntityHandle,
        action: TrainAction,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        match action {
            TrainAction::Find => {
                self.initialize_train(entity, batch)?;
                let running = self.entity(entity).is_some_and(|entity| {
                    matches!(&entity.behavior, BehaviorState::Mover(mover) if mover.path.as_ref().is_some_and(|path| path.running))
                });
                if running {
                    self.schedule_track_train_think(entity, 0.1, batch)?;
                }
                Ok(())
            }
            TrainAction::Think => self.start_train_segment(entity, batch),
            TrainAction::Stop => self.stop_train(entity, batch),
            TrainAction::Resume => {
                let old_speed = self
                    .entity(entity)
                    .and_then(|entity| match &entity.behavior {
                        BehaviorState::Mover(mover) => mover
                            .path
                            .as_ref()
                            .map(|path| f32::from_bits(path.old_speed_bits)),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                if let Some(BehaviorState::Mover(mover)) =
                    self.entity_mut(entity).map(|entity| &mut entity.behavior)
                {
                    let path = mover.path.as_mut().expect("track train path");
                    path.current_speed_bits = old_speed.to_bits();
                    path.running = old_speed != 0.0;
                }
                self.start_track_train(entity, batch)
            }
            TrainAction::Reverse => {
                let speed = self
                    .entity(entity)
                    .and_then(|entity| match &entity.behavior {
                        BehaviorState::Mover(mover) => mover
                            .path
                            .as_ref()
                            .map(|path| f32::from_bits(path.current_speed_bits)),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                self.set_track_train_speed(entity, speed, false, batch)
            }
            TrainAction::SetSpeed { speed, accelerate } => {
                self.set_track_train_speed(entity, speed, accelerate, batch)
            }
            TrainAction::SetForwardModifier(modifier) => {
                let unmodified = if let Some(BehaviorState::Mover(mover)) =
                    self.entity_mut(entity).map(|entity| &mut entity.behavior)
                {
                    let path = mover.path.as_mut().expect("track train path");
                    path.forward_modifier_bits = modifier.abs().clamp(0.0, 1.0).to_bits();
                    f32::from_bits(path.unmodified_desired_speed_bits)
                } else {
                    return self.diagnostic(
                        batch,
                        DiagnosticCode::MoverRequestMismatch,
                        Some(entity),
                    );
                };
                self.set_track_train_speed(entity, unmodified, true, batch)
            }
            TrainAction::Start => self.start_track_train(entity, batch),
            TrainAction::Teleport(name) => {
                let destination = self
                    .resolve(&name, Some(entity), Some(entity), Some(entity))
                    .into_iter()
                    .find(|handle| {
                        matches!(
                            self.entity(*handle).map(|entity| &entity.behavior),
                            Some(BehaviorState::PathNode(node)) if node.track
                        )
                    });
                let Some(destination) = destination else {
                    return self.diagnostic(batch, DiagnosticCode::MissingTarget, Some(entity));
                };
                if let BehaviorState::Mover(mover) =
                    &mut self.entity_mut(entity).expect("train").behavior
                {
                    mover.path.as_mut().expect("train path").target_name = name;
                }
                self.teleport_track_train(entity, destination, true, batch)
            }
        }
    }

    fn schedule_track_train_think(
        &mut self,
        entity: EntityHandle,
        delay: f32,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.has_pending(entity, b"__tracktrain_next") {
            self.schedule_event(
                EventTarget::Direct(entity),
                b"__tracktrain_next".to_vec(),
                Variant::Void,
                delay,
                Some(entity),
                Some(entity),
                None,
                batch,
            )?;
        }
        Ok(())
    }

    fn start_track_train(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let track = self.entity(entity).is_some_and(|entity| {
            matches!(&entity.behavior, BehaviorState::Mover(mover)
                if mover.path.as_ref().is_some_and(|path| path.track))
        });
        if track {
            self.fire_output(
                entity,
                b"OnStart",
                Variant::Void,
                Some(entity),
                Some(entity),
                0.0,
                batch,
            )?;
        }
        self.start_train_segment(entity, batch)
    }

    fn set_track_train_speed(
        &mut self,
        entity: EntityHandle,
        requested: f32,
        accelerate: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let (old_speed, new_speed, track) = {
            let Some(BehaviorState::Mover(mover)) =
                self.entity_mut(entity).map(|entity| &mut entity.behavior)
            else {
                return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity));
            };
            let path = mover.path.as_mut().expect("train path");
            let old_speed = f32::from_bits(path.current_speed_bits);
            path.unmodified_desired_speed_bits = requested.to_bits();
            let modifier = if path.forward {
                f32::from_bits(path.forward_modifier_bits)
            } else {
                1.0
            };
            let signed = requested.abs() * modifier * if path.forward { 1.0 } else { -1.0 };
            path.accelerating = accelerate;
            if accelerate {
                path.desired_speed_bits = signed.to_bits();
                if old_speed == 0.0 && signed != 0.0 {
                    path.current_speed_bits = 0.1_f32.to_bits();
                }
                path.running = f32::from_bits(path.current_speed_bits) != 0.0;
                (
                    old_speed,
                    f32::from_bits(path.current_speed_bits),
                    path.track,
                )
            } else {
                path.current_speed_bits = signed.to_bits();
                path.running = signed != 0.0;
                (old_speed, signed, path.track)
            }
        };
        if accelerate {
            return self.start_track_train(entity, batch);
        }
        if old_speed.to_bits() == new_speed.to_bits() {
            return Ok(());
        }
        if new_speed == 0.0 {
            self.stop_train(entity, batch)
        } else if track && old_speed == 0.0 {
            self.start_track_train(entity, batch)
        } else {
            self.start_train_segment(entity, batch)
        }
    }

    fn arrive_track_node(
        &mut self,
        train: EntityHandle,
        node: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        self.pass_path_node(train, node, true, batch)?;
        let Some(BehaviorState::PathNode(path_node)) =
            self.entity(node).map(|entity| entity.behavior.clone())
        else {
            return Ok(());
        };
        let node_speed = f32::from_bits(path_node.speed_bits);
        let apply_speed = if let Some(BehaviorState::Mover(mover)) =
            self.entity_mut(train).map(|entity| &mut entity.behavior)
        {
            let path = mover.path.as_mut().expect("track train path");
            if path_node.flags & 8 != 0 {
                path.controls_disabled = true;
            }
            path.controls_disabled && node_speed != 0.0
        } else {
            false
        };
        if apply_speed {
            self.set_track_train_speed(train, node_speed, false, batch)?;
        }
        Ok(())
    }

    fn start_train_segment(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let mover = match &self.entity(entity).expect("train").behavior {
            BehaviorState::Mover(mover) => mover.clone(),
            _ => return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity)),
        };
        let path = mover.path.as_ref().expect("train path");
        if !path.running {
            return Ok(());
        }
        if path.track {
            return self.start_track_train_segment(entity, &mover, batch);
        }
        let mut target_name = path.target_name.clone();
        for _ in 0..=self.config.limits.max_hierarchy_depth {
            let destination = self
                .resolve(&target_name, Some(entity), Some(entity), Some(entity))
                .into_iter()
                .find(|handle| {
                    matches!(
                        self.entity(*handle).map(|entity| &entity.behavior),
                        Some(BehaviorState::PathNode(node)) if !node.track
                    )
                });
            let Some(destination) = destination else {
                return self.stop_train(entity, batch);
            };
            let node = self.entity(destination).expect("path corner").clone();
            let BehaviorState::PathNode(node_state) = &node.behavior else {
                unreachable!()
            };
            let mut speed = mover.speed;
            let node_speed = f32::from_bits(node_state.speed_bits);
            if node_speed != 0.0 {
                speed = node_speed;
            }
            if let BehaviorState::Mover(current) =
                &mut self.entity_mut(entity).expect("train").behavior
            {
                current.speed = speed;
                let path = current.path.as_mut().expect("train path");
                path.current = Some(destination);
                path.target_name = node_state.next_name.clone();
            }
            let transform = Transform {
                origin: sub(node.world_transform.origin, self.mover_model_center(entity)),
                angles: self.entity(entity).expect("train").world_transform.angles,
            };
            if node_state.flags & 2 != 0 {
                self.set_entity_world_transform(entity, transform, batch)?;
                self.pass_path_node(entity, destination, false, batch)?;
                target_name = node_state.next_name.clone();
                continue;
            }
            return self.start_path_request(
                entity,
                transform,
                speed.abs(),
                Some(destination),
                None,
                batch,
            );
        }
        Err(failure(
            RuntimeFailureCode::HierarchyLimit,
            self.entity(entity).map(|entity| entity.source_index),
            self.config.limits.max_hierarchy_depth,
            self.config.limits.max_hierarchy_depth + 1,
        ))
    }

    fn start_track_train_segment(
        &mut self,
        entity: EntityHandle,
        mover: &MoverState,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let path = mover.path.as_ref().expect("track train path");
        let Some(current) = path.current else {
            return self.stop_train(entity, batch);
        };
        let speed = f32::from_bits(path.current_speed_bits);
        if speed == 0.0 {
            return Ok(());
        }
        let transform = self.entity(entity).expect("track train").world_transform;
        let height = f32::from_bits(path.height_bits);
        let start = sub(transform.origin, [0.0, 0.0, height]);
        let look_ahead = self.path_look_ahead(current, start, speed * 0.1, true)?;
        if look_ahead.dead_end {
            let endpoint = add(look_ahead.position, [0.0, 0.0, height]);
            let remaining = length(sub(endpoint, transform.origin));
            if let BehaviorState::Mover(current_mover) =
                &mut self.entity_mut(entity).expect("track train").behavior
            {
                let state = current_mover.path.as_mut().expect("track train path");
                state.old_speed_bits = speed.to_bits();
                state.current_speed_bits = 0.0_f32.to_bits();
                state.running = false;
            }
            if remaining == 0.0 {
                self.pass_path_node(entity, look_ahead.current, true, batch)?;
                return self.stop_train(entity, batch);
            }
            return self.start_path_request(
                entity,
                Transform {
                    origin: endpoint,
                    angles: transform.angles,
                },
                speed.abs(),
                Some(look_ahead.current),
                None,
                batch,
            );
        }

        let next_speed = self.track_train_velocity(entity, path, &look_ahead, transform.origin);
        if let BehaviorState::Mover(current_mover) =
            &mut self.entity_mut(entity).expect("track train").behavior
        {
            current_mover
                .path
                .as_mut()
                .expect("track train path")
                .current_speed_bits = next_speed.to_bits();
        }
        let (angles, angular_velocity) =
            self.track_train_motion_orientation(entity, path, current, &look_ahead, transform)?;
        let direction = sub(
            add(look_ahead.position, [0.0, 0.0, height]),
            transform.origin,
        );
        let distance = length(direction);
        let travel = distance.max(next_speed.abs() * self.config.tick_interval);
        let destination = Transform {
            origin: if distance == 0.0 {
                transform.origin
            } else {
                add(transform.origin, scale(direction, travel / distance))
            },
            angles,
        };

        if look_ahead.current != current {
            if let BehaviorState::Mover(current_mover) =
                &mut self.entity_mut(entity).expect("track train").behavior
            {
                current_mover
                    .path
                    .as_mut()
                    .expect("track train path")
                    .current = Some(look_ahead.current);
            }
            self.arrive_track_node(entity, look_ahead.current, batch)?;
            if let Some(teleport) = self.path_next(look_ahead.current, true, false)
                && self.entity(teleport).is_some_and(|node| {
                    matches!(&node.behavior, BehaviorState::PathNode(state) if state.flags & 0x10 != 0)
                })
            {
                self.teleport_track_train(entity, teleport, false, batch)?;
            }
        }
        self.fire_output(
            entity,
            b"OnNextPoint",
            Variant::Void,
            Some(look_ahead.current),
            Some(entity),
            0.0,
            batch,
        )?;
        if next_speed == 0.0 {
            return self.stop_train(entity, batch);
        }
        if distance != 0.0 {
            self.start_path_request(
                entity,
                destination,
                next_speed.abs(),
                None,
                Some(angular_velocity),
                batch,
            )?;
            self.schedule_track_train_think(entity, self.config.tick_interval, batch)?;
        }
        Ok(())
    }

    fn track_train_velocity(
        &self,
        _entity: EntityHandle,
        path: &TrainPathState,
        look_ahead: &PathLookAhead,
        origin: [f32; 3],
    ) -> f32 {
        let current = f32::from_bits(path.current_speed_bits);
        if !matches!(path.velocity_type, 1 | 2) {
            return current;
        }
        if path.accelerating {
            let desired = f32::from_bits(path.desired_speed_bits);
            let rate = if desired.abs() > current.abs() {
                f32::from_bits(path.acceleration_bits)
            } else {
                f32::from_bits(path.deceleration_bits)
            };
            return approach(desired, current, rate * self.config.tick_interval);
        }
        let Some(next) = look_ahead.next else {
            return current;
        };
        let node_speed = |handle| {
            self.entity(handle)
                .and_then(|entity| match &entity.behavior {
                    BehaviorState::PathNode(node) => Some(f32::from_bits(node.speed_bits)),
                    _ => None,
                })
        };
        let previous_speed = node_speed(look_ahead.current)
            .filter(|value| *value != 0.0)
            .unwrap_or(current.abs());
        let next_speed = node_speed(next)
            .filter(|value| *value != 0.0)
            .unwrap_or(previous_speed);
        let blended = if previous_speed == next_speed {
            previous_speed
        } else {
            let previous_origin = self
                .entity(look_ahead.current)
                .expect("previous track node")
                .world_transform
                .origin;
            let next_origin = self
                .entity(next)
                .expect("next track node")
                .world_transform
                .origin;
            let distance = length(sub(next_origin, previous_origin));
            if distance == 0.0 {
                current.abs()
            } else {
                let mut fraction = length(sub(origin, previous_origin)) / distance;
                if path.velocity_type == 2 {
                    fraction = simple_spline(fraction);
                }
                previous_speed * (1.0 - fraction) + next_speed * fraction
            }
        };
        blended * if path.forward { 1.0 } else { -1.0 }
    }

    fn track_train_motion_orientation(
        &self,
        entity: EntityHandle,
        path: &TrainPathState,
        current: EntityHandle,
        look_ahead: &PathLookAhead,
        transform: Transform,
    ) -> Result<([f32; 3], [f32; 3]), RuntimeFailure> {
        let flags = self
            .entity(entity)
            .and_then(|entity| field(&entity.definition, b"spawnflags"))
            .and_then(parse_i32)
            .unwrap_or(0);
        if flags & 0x10 != 0 || path.orientation_type == 0 {
            return Ok((transform.angles, [0.0; 3]));
        }
        let mut desired = match path.orientation_type {
            1 => {
                let height = f32::from_bits(path.height_bits);
                let wheels = f32::from_bits(path.length_bits);
                let distance = if wheels > 0.0 { wheels } else { 100.0 }
                    * if path.forward { 1.0 } else { -1.0 };
                let front = self.path_look_ahead(
                    current,
                    sub(transform.origin, [0.0, 0.0, height]),
                    distance,
                    false,
                )?;
                let mut face = sub(add(front.position, [0.0, 0.0, height]), transform.origin);
                if !path.forward {
                    face = scale(face, -1.0);
                }
                let mut angles = if face[0] == 0.0 && face[1] == 0.0 {
                    transform.angles
                } else {
                    vector_angles(face)
                };
                if path.manual_speed_changes
                    && let Some(next) = front.next
                    && self.entity(next).is_some_and(|node| {
                        matches!(&node.behavior, BehaviorState::PathNode(state) if state.orientation == 2)
                    })
                {
                    angles = self.track_orientation(next, path.forward, transform.angles);
                }
                angles
            }
            2 | 3 => {
                let previous =
                    self.track_orientation(look_ahead.current, path.forward, transform.angles);
                let mut next = look_ahead.next.map_or(previous, |node| {
                    self.track_orientation(node, path.forward, previous)
                });
                if flags & 1 != 0 {
                    next[0] = previous[0];
                }
                let fraction = if previous != next
                    && let Some(next_node) = look_ahead.next
                {
                    let previous_origin = self
                        .entity(look_ahead.current)
                        .expect("previous track node")
                        .world_transform
                        .origin;
                    let next_origin = self
                        .entity(next_node)
                        .expect("next track node")
                        .world_transform
                        .origin;
                    let segment_length = length(sub(next_origin, previous_origin));
                    if segment_length == 0.0 {
                        0.0
                    } else {
                        let value = length(sub(transform.origin, previous_origin)) / segment_length;
                        if path.orientation_type == 3 {
                            simple_spline(value)
                        } else {
                            value
                        }
                    }
                } else {
                    0.0
                };
                let mut result = angles_from_quat(quat_slerp(
                    quat_from_angles(previous),
                    quat_from_angles(next),
                    f64::from(fraction),
                ));
                if flags & 1 != 0 {
                    result[0] = previous[0];
                }
                result
            }
            _ => transform.angles,
        };
        if flags & 1 != 0 {
            desired[0] = transform.angles[0];
        }
        let mut pitch = angle_distance(desired[0], transform.angles[0]);
        let mut yaw = angle_distance(desired[1], transform.angles[1]);
        if pitch.abs() < 0.1 {
            pitch = 0.0;
        }
        if yaw.abs() < 0.1 {
            yaw = 0.0;
        }
        let interval = self.config.tick_interval;
        let mut angular_velocity = [pitch / interval, yaw / interval, 0.0];
        let bank = f32::from_bits(path.bank_bits);
        if bank != 0.0 {
            let target = if angular_velocity[1] < -5.0 {
                -bank
            } else if angular_velocity[1] > 5.0 {
                bank
            } else {
                0.0
            };
            let rate = if target == 0.0 {
                bank * 4.0
            } else {
                bank * 2.0
            };
            angular_velocity[2] = angle_distance(
                approach_angle(target, transform.angles[2], rate),
                transform.angles[2],
            ) * if target == 0.0 { 4.0 } else { 1.0 };
        }
        Ok((
            add(transform.angles, scale(angular_velocity, interval)),
            angular_velocity,
        ))
    }

    fn teleport_track_train(
        &mut self,
        entity: EntityHandle,
        destination: EntityHandle,
        update_current: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let node = self
            .entity(destination)
            .expect("track destination")
            .world_transform;
        let (forward, current_angles) = self
            .entity(entity)
            .and_then(|train| match &train.behavior {
                BehaviorState::Mover(mover) => mover
                    .path
                    .as_ref()
                    .map(|path| (path.forward, train.world_transform.angles)),
                _ => None,
            })
            .unwrap_or((true, node.angles));
        self.set_entity_world_transform(
            entity,
            Transform {
                origin: node.origin,
                angles: self.track_train_orientation(entity, destination, forward, current_angles),
            },
            batch,
        )?;
        if update_current
            && let BehaviorState::Mover(mover) =
                &mut self.entity_mut(entity).expect("track train").behavior
        {
            mover.path.as_mut().expect("track train path").current = Some(destination);
        }
        self.dispatch_input(
            destination,
            &InputRecord {
                target: EventTarget::Direct(destination),
                input: b"InTeleport".to_vec(),
                value: Variant::Void,
                activator: Some(entity),
                caller: Some(entity),
                output_action: None,
                producer_sequence: self.state.next_transition_sequence,
            },
            batch,
        )
    }

    fn start_path_request(
        &mut self,
        entity: EntityHandle,
        destination: Transform,
        speed: f32,
        path_destination: Option<EntityHandle>,
        requested_angular_velocity: Option<[f32; 3]>,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let current = self.entity(entity).expect("path mover").world_transform;
        let angular_delta = sub(destination.angles, current.angles);
        let linear_distance = length(sub(destination.origin, current.origin));
        let travel_time = if linear_distance > f32::EPSILON {
            linear_distance / speed.max(f32::EPSILON)
        } else {
            length(angular_delta) / speed.max(f32::EPSILON)
        };
        let angular_velocity = requested_angular_velocity.unwrap_or_else(|| {
            if travel_time > f32::EPSILON {
                scale(angular_delta, 1.0 / travel_time)
            } else {
                [0.0; 3]
            }
        });
        let request_id = self.state.next_mover_request_id;
        self.state.next_mover_request_id += 1;
        let solid = match &mut self.entity_mut(entity).expect("path mover").behavior {
            BehaviorState::Mover(mover) => {
                mover.pending = Some(PendingMove {
                    request_id,
                    local_destination: destination.origin,
                    world_destination: destination.origin,
                    local_angles_destination: destination.angles,
                    world_angles_destination: destination.angles,
                    angular_velocity,
                    continuous: false,
                    path_destination,
                    opening: true,
                    activator: Some(entity),
                });
                mover.position = MoverPosition::Opening;
                mover.solid
            }
            _ => unreachable!(),
        };
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::Mover {
                request_id,
                entity,
                kind: MoverKind::Linear,
                local_destination: destination.origin,
                world_destination: destination.origin,
                local_angles_destination: destination.angles,
                world_angles_destination: destination.angles,
                angular_velocity,
                continuous: false,
                solid,
                speed,
                opening: true,
            }),
        )
    }

    fn stop_train(
        &mut self,
        entity: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let transform = self.entity(entity).expect("train").world_transform;
        let (solid, track) = match &mut self.entity_mut(entity).expect("train").behavior {
            BehaviorState::Mover(mover) => {
                mover.pending = None;
                mover.position = MoverPosition::Positioned(0.0_f32.to_bits());
                let path = mover.path.as_mut().expect("train path");
                path.old_speed_bits = path.current_speed_bits;
                path.current_speed_bits = 0.0_f32.to_bits();
                path.running = false;
                (mover.solid, path.track)
            }
            _ => return self.diagnostic(batch, DiagnosticCode::MoverRequestMismatch, Some(entity)),
        };
        if track {
            self.cancel_direct(entity, b"__tracktrain_next", batch)?;
        }
        let request_id = self.state.next_mover_request_id;
        self.state.next_mover_request_id += 1;
        self.push_transition(
            batch,
            Transition::Request(RuntimeRequest::Mover {
                request_id,
                entity,
                kind: MoverKind::Linear,
                local_destination: transform.origin,
                world_destination: transform.origin,
                local_angles_destination: transform.angles,
                world_angles_destination: transform.angles,
                angular_velocity: [0.0; 3],
                continuous: track,
                solid,
                speed: 0.0,
                opening: true,
            }),
        )
    }

    fn pass_path_node(
        &mut self,
        train: EntityHandle,
        node: EntityHandle,
        _track: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        self.dispatch_input(
            node,
            &InputRecord {
                target: EventTarget::Direct(node),
                input: b"InPass".to_vec(),
                value: Variant::Void,
                activator: Some(train),
                caller: Some(train),
                output_action: None,
                producer_sequence: self.state.next_transition_sequence,
            },
            batch,
        )?;
        Ok(())
    }

    fn mover_model_center(&self, entity: EntityHandle) -> [f32; 3] {
        self.entity(entity)
            .and_then(|entity| entity.definition.bsp_model_index)
            .and_then(|model| {
                self.config
                    .model_bounds
                    .iter()
                    .find(|bounds| bounds.model == model)
            })
            .map_or([0.0; 3], |bounds| scale(add(bounds.mins, bounds.maxs), 0.5))
    }

    fn set_entity_world_transform(
        &mut self,
        entity: EntityHandle,
        transform: Transform,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        let local = match self.entity(entity).and_then(|entity| entity.parent) {
            Some(parent) => relative_transform(
                self.parent_basis(
                    parent,
                    self.entity(entity)
                        .and_then(|entity| entity.parent_attachment.as_deref()),
                )?,
                transform,
            ),
            None => transform,
        };
        self.entity_mut(entity)
            .expect("transform entity")
            .local_transform = local;
        self.recompute_subtree(entity, 0)?;
        let state = self.entity(entity).expect("transform entity");
        self.push_transition(
            batch,
            Transition::TransformChanged {
                entity,
                local: state.local_transform,
                world: state.world_transform,
            },
        )
    }

    pub fn path_next(
        &self,
        node: EntityHandle,
        forward: bool,
        test_disabled: bool,
    ) -> Option<EntityHandle> {
        let BehaviorState::PathNode(state) = &self.entity(node)?.behavior else {
            return None;
        };
        let next = if forward {
            if state.flags & 0x8000 != 0 && state.flags & 4 == 0 {
                state.alternate
            } else {
                state.next
            }
        } else if state.flags & 0x8000 != 0 && state.flags & 4 != 0 {
            state.alternate
        } else {
            state.previous
        }?;
        if test_disabled
            && matches!(&self.entity(next)?.behavior, BehaviorState::PathNode(state) if state.flags & 1 != 0)
        {
            None
        } else {
            Some(next)
        }
    }

    fn path_look_ahead(
        &self,
        mut current: EntityHandle,
        mut origin: [f32; 3],
        distance: f32,
        move_path: bool,
    ) -> Result<PathLookAhead, RuntimeFailure> {
        let forward = distance >= 0.0;
        let mut remaining = distance.abs();
        let original = remaining;
        let mut current_position = origin;
        let mut visits = 0;
        while remaining > 0.0 {
            visits += 1;
            if visits > self.config.limits.max_entities {
                return Err(failure(
                    RuntimeFailureCode::HierarchyLimit,
                    self.entity(current).map(|entity| entity.source_index),
                    self.config.limits.max_entities,
                    visits,
                ));
            }
            let Some(next) = self.path_next(current, forward, move_path) else {
                if !move_path && let Some(previous) = self.path_next(current, !forward, false) {
                    let from = self
                        .entity(previous)
                        .expect("previous track node")
                        .world_transform
                        .origin;
                    let to = self
                        .entity(current)
                        .expect("current track node")
                        .world_transform
                        .origin;
                    let direction = sub(to, from);
                    let segment = length(direction);
                    if segment != 0.0 {
                        origin = add(to, scale(direction, remaining / segment));
                    }
                }
                return Ok(PathLookAhead {
                    current,
                    next: None,
                    position: origin,
                    dead_end: true,
                });
            };
            let next_position = self.entity(next).expect("next path").world_transform.origin;
            let delta = sub(next_position, current_position);
            let segment = length(delta);
            if segment == 0.0 && self.path_next(next, forward, move_path).is_none() {
                return Ok(PathLookAhead {
                    current: if remaining == original { current } else { next },
                    next: None,
                    position: origin,
                    dead_end: remaining == original,
                });
            }
            if segment > remaining {
                origin = add(current_position, scale(delta, remaining / segment));
                return Ok(PathLookAhead {
                    current,
                    next: Some(next),
                    position: origin,
                    dead_end: false,
                });
            }
            remaining -= segment;
            current_position = next_position;
            current = next;
            origin = current_position;
        }
        Ok(PathLookAhead {
            current,
            next: self.path_next(current, forward, move_path),
            position: origin,
            dead_end: false,
        })
    }

    fn track_orientation(&self, node: EntityHandle, forward: bool, fallback: [f32; 3]) -> [f32; 3] {
        let Some(entity) = self.entity(node) else {
            return fallback;
        };
        let BehaviorState::PathNode(state) = &entity.behavior else {
            return fallback;
        };
        if state.orientation == 2 {
            return entity.world_transform.angles;
        }
        let (from, to) = if let Some(next) = self.path_next(node, forward, false) {
            (
                entity.world_transform.origin,
                self.entity(next)
                    .expect("track next")
                    .world_transform
                    .origin,
            )
        } else if let Some(previous) = self.path_next(node, !forward, false) {
            (
                self.entity(previous)
                    .expect("track previous")
                    .world_transform
                    .origin,
                entity.world_transform.origin,
            )
        } else {
            return fallback;
        };
        vector_angles(sub(to, from))
    }

    fn track_train_orientation(
        &self,
        train: EntityHandle,
        node: EntityHandle,
        forward: bool,
        fallback: [f32; 3],
    ) -> [f32; 3] {
        let flags = self
            .entity(train)
            .and_then(|entity| field(&entity.definition, b"spawnflags"))
            .and_then(parse_i32)
            .unwrap_or(0);
        if flags & 0x10 != 0 {
            return fallback;
        }
        let mut angles = self.track_orientation(node, forward, fallback);
        if flags & 1 != 0 {
            angles[0] = fallback[0];
        }
        angles
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
            entity_state.local_transform.angles = pending.local_angles_destination;
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
        if let Some(path) = mover.path.as_ref() {
            if let Some(node) = pending.path_destination {
                self.pass_path_node(entity, node, path.track, batch)?;
            }
            if path.track {
                return Ok(());
            }
            let node_state = pending.path_destination.and_then(|node| {
                self.entity(node).and_then(|entity| match &entity.behavior {
                    BehaviorState::PathNode(state) => Some(state.clone()),
                    _ => None,
                })
            });
            if let Some(node) = node_state {
                let wait = f32::from_bits(node.wait_bits);
                if node.flags & 1 != 0 || wait < 0.0 {
                    return self.stop_train(entity, batch);
                }
                if wait == 0.0 {
                    return self.start_train_segment(entity, batch);
                }
                self.schedule_event(
                    EventTarget::Direct(entity),
                    b"__train_next".to_vec(),
                    Variant::Void,
                    wait,
                    Some(entity),
                    Some(entity),
                    None,
                    batch,
                )?;
            }
            return Ok(());
        }
        let endpoint_open = pending.opening ^ mover.outputs_reversed;
        let output = match (mover.kind, endpoint_open) {
            (MoverKind::Button, true) => b"OnIn".as_slice(),
            (MoverKind::Button, false) => b"OnOut".as_slice(),
            (_, true) => b"OnFullyOpen".as_slice(),
            (_, false) => b"OnFullyClosed".as_slice(),
        };
        let activator = match mover.kind {
            MoverKind::Button => pending.activator,
            MoverKind::Door if pending.opening => Some(entity),
            MoverKind::Door => pending.activator,
            MoverKind::Linear => Some(entity),
        };
        self.fire_output(
            entity,
            output,
            Variant::Void,
            activator,
            Some(entity),
            0.0,
            batch,
        )?;
        if pending.opening
            && (matches!(mover.kind, MoverKind::Button | MoverKind::Door)
                || matches!(
                    mover.class,
                    MoverClass::Platform | MoverClass::RotatingPlatform
                ))
            && !mover.no_auto_return
            && let Some(wait) = mover.wait_ticks
        {
            self.schedule_event_ticks(
                EventTarget::Direct(entity),
                if mover.kind == MoverKind::Button {
                    b"PressOut".to_vec()
                } else if matches!(
                    mover.class,
                    MoverClass::Platform | MoverClass::RotatingPlatform
                ) {
                    b"GoDown".to_vec()
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
        } else if matches!(
            mover.class,
            MoverClass::Platform | MoverClass::RotatingPlatform
        ) && kind == BlockContactKind::Stay
        {
            self.start_mover(
                entity,
                if pending.opening { 0.0 } else { 1.0 },
                !pending.opening,
                pending.activator,
                batch,
            )?;
        }
        Ok(())
    }

    fn mark_removal(
        &mut self,
        handle: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        self.mark_removal_ordered(handle, false, batch)
    }

    fn mark_removal_child_first(
        &mut self,
        handle: EntityHandle,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        self.mark_removal_ordered(handle, true, batch)
    }

    fn mark_removal_ordered(
        &mut self,
        handle: EntityHandle,
        child_first: bool,
        batch: &mut TransitionBatch,
    ) -> Result<(), RuntimeFailure> {
        if !self.is_resolvable(handle) {
            return Ok(());
        }
        let descendants = self.removal_order(handle, child_first)?;
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

    fn removal_order(
        &self,
        root: EntityHandle,
        child_first: bool,
    ) -> Result<Vec<EntityHandle>, RuntimeFailure> {
        fn append(
            world: &EntityWorld,
            handle: EntityHandle,
            depth: usize,
            output: &mut Vec<EntityHandle>,
            child_first: bool,
        ) -> Result<(), RuntimeFailure> {
            if depth > world.config.limits.max_hierarchy_depth {
                return Err(failure(
                    RuntimeFailureCode::HierarchyLimit,
                    world.entity(handle).map(|entity| entity.source_index),
                    world.config.limits.max_hierarchy_depth,
                    depth,
                ));
            }
            if !child_first {
                output.push(handle);
            }
            if let Some(entity) = world.entity(handle) {
                for child in &entity.children {
                    append(world, *child, depth + 1, output, child_first)?;
                }
            }
            if child_first {
                output.push(handle);
            }
            Ok(())
        }
        let mut output = Vec::new();
        append(self, root, 0, &mut output, child_first)?;
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

fn fixup_template_definitions(members: &mut [TemplateMemberState], suffix: &[u8]) {
    let names = members
        .iter()
        .filter_map(|member| member.definition.targetname.clone())
        .collect::<Vec<_>>();
    let referenced = names
        .iter()
        .filter(|name| {
            members.iter().any(|member| {
                member.definition.pairs.iter().any(|pair| {
                    if pair.key.eq_ignore_ascii_case(b"targetname") {
                        return false;
                    }
                    if pair.value.eq_ignore_ascii_case(name) {
                        return true;
                    }
                    let delimiter = if pair.value.contains(&0x1b) {
                        0x1b
                    } else {
                        b','
                    };
                    pair.value
                        .split(|byte| *byte == delimiter)
                        .next()
                        .is_some_and(|target| target.eq_ignore_ascii_case(name))
                })
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let fixed = referenced
        .iter()
        .map(|name| (name.clone(), [name.as_slice(), suffix].concat()))
        .collect::<Vec<_>>();
    let replace_exact = |value: &[u8]| {
        fixed
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(value))
            .map(|(_, replacement)| replacement.clone())
    };
    for member in members {
        for pair in &mut member.definition.pairs {
            if pair.key.eq_ignore_ascii_case(b"targetname") {
                if let Some(value) = replace_exact(&pair.value) {
                    pair.value = value;
                }
                continue;
            }
            if let Some(value) = replace_exact(&pair.value) {
                pair.value = value;
                continue;
            }
            let delimiter = if pair.value.contains(&0x1b) {
                0x1b
            } else {
                b','
            };
            if let Some(split) = pair.value.iter().position(|byte| *byte == delimiter)
                && let Some(target) = replace_exact(&pair.value[..split])
            {
                pair.value = [target.as_slice(), &pair.value[split..]].concat();
                continue;
            }
            if pair.key.eq_ignore_ascii_case(b"parentname")
                && let Some(split) = pair.value.iter().position(|byte| *byte == b',')
                && let Some(parent) = replace_exact(&pair.value[..split])
            {
                pair.value = [parent.as_slice(), &pair.value[split..]].concat();
            }
        }
        member.definition.targetname = member
            .definition
            .targetname
            .as_deref()
            .and_then(&replace_exact)
            .or_else(|| member.definition.targetname.clone());
        member.definition.parentname = member.definition.parentname.as_ref().map(|value| {
            let split = value.iter().position(|byte| *byte == b',');
            let name = split.map_or(value.as_slice(), |split| &value[..split]);
            replace_exact(name).map_or_else(
                || value.clone(),
                |parent| {
                    split.map_or(parent.clone(), |split| {
                        [parent.as_slice(), &value[split..]].concat()
                    })
                },
            )
        });
        for connection in &mut member.definition.connections {
            if let Connection::Parsed { target, .. } = connection
                && let Some(value) = replace_exact(target)
            {
                *target = value;
            }
        }
    }
}

fn template_prototype_indices(
    graph: &Graph,
) -> Result<std::collections::BTreeSet<usize>, RuntimeFailure> {
    let mut output = std::collections::BTreeSet::new();
    for template in graph.entities.iter().filter(|entity| {
        entity
            .classname
            .as_deref()
            .is_some_and(|class| class.eq_ignore_ascii_case(b"point_template"))
    }) {
        if field_i32(template, b"spawnflags", 0)? & 1 != 0 {
            continue;
        }
        for index in 1..=16 {
            let key = format!("Template{index:02}");
            let Some(name) = field(template, key.as_bytes()).filter(|name| !name.is_empty()) else {
                continue;
            };
            output.extend(graph.entities.iter().filter_map(|definition| {
                definition
                    .targetname
                    .as_deref()
                    .is_some_and(|target| target.eq_ignore_ascii_case(name))
                    .then_some(definition.index)
            }));
        }
    }
    Ok(output)
}

fn ordered_definitions<'a>(
    entities: &'a [Entity],
    skipped: &std::collections::BTreeSet<usize>,
    priorities: &[ClassSpawnPriority],
) -> Vec<&'a Entity> {
    fn depth(
        definition: &Entity,
        entities: &[Entity],
        stack: &mut Vec<usize>,
        memo: &mut BTreeMap<usize, usize>,
    ) -> usize {
        if let Some(value) = memo.get(&definition.index) {
            return *value;
        }
        if stack.contains(&definition.index) {
            return 0;
        }
        stack.push(definition.index);
        let parent_name = definition
            .parentname
            .as_deref()
            .and_then(|value| value.split(|byte| *byte == b',').next())
            .filter(|value| !value.is_empty());
        let value = parent_name
            .and_then(|name| {
                entities.iter().find(|candidate| {
                    candidate
                        .targetname
                        .as_deref()
                        .is_some_and(|target| target.eq_ignore_ascii_case(name))
                })
            })
            .map_or(0, |parent| 1 + depth(parent, entities, stack, memo));
        stack.pop();
        memo.insert(definition.index, value);
        value
    }

    let mut memo = BTreeMap::new();
    let mut output = entities.iter()
        .filter(|definition| !skipped.contains(&definition.index))
        .collect::<Vec<_>>();
    output.sort_by_key(|definition| {
        let class = definition.classname.as_deref().unwrap_or_default();
        let group = if class.eq_ignore_ascii_case(b"worldspawn") {
            0
        } else if class.eq_ignore_ascii_case(b"point_template") {
            1
        } else {
            2
        };
        let hierarchy_depth = if group == 2 {
            depth(definition, entities, &mut Vec::new(), &mut memo)
        } else {
            0
        };
        let priority = priorities
            .iter()
            .find(|binding| {
                definition
                    .classname
                    .as_deref()
                    .is_some_and(|class| class.eq_ignore_ascii_case(&binding.classname))
            })
            .map_or(0, |binding| binding.priority);
        (
            group,
            hierarchy_depth,
            std::cmp::Reverse(priority),
            definition.index,
        )
    });
    output
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
        || config.limits.max_template_instances == 0
        || config.limits.max_template_members == 0
        || config.limits.max_snapshot_bytes == 0
    {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            0,
            0,
        ));
    }
    let mut classes = std::collections::BTreeSet::new();
    for binding in &config.external_classes {
        let class = ascii_key(&binding.classname);
        let mut inputs = std::collections::BTreeSet::new();
        if class.is_empty()
            || !classes.insert(class)
            || binding.inputs.is_empty()
            || binding
                .inputs
                .iter()
                .any(|input| input.is_empty() || !inputs.insert(ascii_key(input)))
        {
            return Err(failure(
                RuntimeFailureCode::InvalidConfiguration,
                None,
                0,
                0,
            ));
        }
    }
    let mut field_classes = std::collections::BTreeSet::new();
    for binding in &config.field_bindings {
        let mut keys = std::collections::BTreeSet::new();
        let mut inputs = std::collections::BTreeSet::new();
        if binding.classname.is_empty()
            || !field_classes.insert(ascii_key(&binding.classname))
            || binding.fields.iter().any(|field| {
                field.key.is_empty()
                    || !keys.insert(ascii_key(&field.key))
                    || field
                        .writable_input
                        .as_ref()
                        .is_some_and(|input| input.is_empty() || !inputs.insert(ascii_key(input)))
            })
        {
            return Err(failure(
                RuntimeFailureCode::InvalidConfiguration,
                None,
                config.field_bindings.len(),
                field_classes.len(),
            ));
        }
    }
    let mut pickup_classes = std::collections::BTreeSet::new();
    if config
        .pickup_classes
        .iter()
        .any(|class| class.is_empty() || !pickup_classes.insert(ascii_key(class)))
    {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            config.pickup_classes.len(),
            pickup_classes.len(),
        ));
    }
    let mut priority_classes = std::collections::BTreeSet::new();
    if config.spawn_priorities.iter().any(|binding| {
        binding.classname.is_empty() || !priority_classes.insert(ascii_key(&binding.classname))
    }) {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            config.spawn_priorities.len(),
            priority_classes.len(),
        ));
    }
    let mut disposition_classes = std::collections::BTreeSet::new();
    if config.class_dispositions.iter().any(|binding| {
        binding.classname.is_empty()
            || !matches!(
                binding.coverage,
                Coverage::IntentionallyInert | Coverage::Unsupported | Coverage::Unknown
            )
            || classes.contains(&ascii_key(&binding.classname))
            || !disposition_classes.insert(ascii_key(&binding.classname))
    }) {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            config.class_dispositions.len(),
            disposition_classes.len(),
        ));
    }
    let mut attachments = std::collections::BTreeSet::new();
    if config.initial_attachments.iter().any(|binding| {
        binding.attachment.is_empty()
            || !attachments.insert((binding.parent_source_index, ascii_key(&binding.attachment)))
            || binding
                .parent_space_transform
                .origin
                .into_iter()
                .chain(binding.parent_space_transform.angles)
                .any(|value| !value.is_finite())
    }) {
        return Err(failure(
            RuntimeFailureCode::InvalidConfiguration,
            None,
            config.initial_attachments.len(),
            attachments.len(),
        ));
    }
    let mut models = std::collections::BTreeSet::new();
    for bounds in &config.model_bounds {
        if !models.insert(bounds.model)
            || bounds
                .mins
                .iter()
                .chain(bounds.maxs.iter())
                .any(|value| !value.is_finite())
            || (0..3).any(|axis| bounds.mins[axis] > bounds.maxs[axis])
        {
            return Err(failure(
                RuntimeFailureCode::InvalidConfiguration,
                None,
                config.model_bounds.len(),
                bounds.model,
            ));
        }
    }
    let mut external_brush_models = std::collections::BTreeSet::new();
    for binding in &config.external_brush_models {
        if binding.classname.is_empty()
            || !external_brush_models.insert(ascii_key(&binding.classname))
        {
            return Err(failure(
                RuntimeFailureCode::InvalidConfiguration,
                None,
                config.external_brush_models.len(),
                external_brush_models.len(),
            ));
        }
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
    Some(source_integer(value))
}

fn parse_f32(value: &[u8]) -> Option<f32> {
    let value = source_float(value);
    value.is_finite().then_some(value)
}

fn parse_vector(value: &[u8]) -> Option<[f32; 3]> {
    let values = source_vector(value);
    values
        .iter()
        .all(|value| value.is_finite())
        .then_some(values)
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

fn render_state(entity: &Entity) -> Result<EntityRenderState, RuntimeFailure> {
    let mut color = [255; 4];
    if entity.classname.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(b"env_smokestack")) {
        color[..3].fill(0);
    }
    if let Some(value) = field(entity, b"rendercolor").or_else(|| field(entity, b"rendercolor32")) {
        let parsed = source_color(value).ok_or_else(|| {
            failure(
                RuntimeFailureCode::InvalidField,
                Some(entity.index),
                4,
                value.len(),
            )
        })?;
        color[..3].copy_from_slice(&parsed[..3]);
    }
    if let Some(value) = field(entity, b"renderamt") {
        color[3] = source_i32(value).ok_or_else(|| {
            failure(
                RuntimeFailureCode::InvalidField,
                Some(entity.index),
                0,
                value.len(),
            )
        })? as u8;
    }
    let mut effects = match field(entity, b"effects") {
        Some(value) => source_i32(value).ok_or_else(|| {
            failure(
                RuntimeFailureCode::InvalidField,
                Some(entity.index),
                0,
                value.len(),
            )
        })?,
        None => 0,
    } as u16
        & 0x03ff;
    if let Some(value) = field(entity, b"disableshadows") {
        let disable = source_i32(value).ok_or_else(|| {
            failure(
                RuntimeFailureCode::InvalidField,
                Some(entity.index),
                0,
                value.len(),
            )
        })?;
        if disable != 0 {
            effects |= EF_NOSHADOW;
        }
    }
    let byte_field = |key: &[u8]| -> Result<u8, RuntimeFailure> {
        match field(entity, key) {
            Some(value) => source_i32(value).map(|value| value as u8).ok_or_else(|| {
                failure(
                    RuntimeFailureCode::InvalidField,
                    Some(entity.index),
                    0,
                    value.len(),
                )
            }),
            None => Ok(0),
        }
    };
    Ok(EntityRenderState {
        brush_model: entity.bsp_model_index,
        mode: byte_field(b"rendermode")?,
        color,
        fx: byte_field(b"renderfx")?,
        effects,
    })
}

fn apply_render_key(render: &mut EntityRenderState, value: &[u8]) -> bool {
    let Some(split) = value.iter().position(u8::is_ascii_whitespace) else {
        return false;
    };
    let key = &value[..split];
    let raw = value[split..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|offset| &value[split + offset..])
        .unwrap_or_default();
    if key.eq_ignore_ascii_case(b"rendermode") {
        let Some(mode) = source_i32(raw) else {
            return false;
        };
        render.mode = mode as u8;
    } else if key.eq_ignore_ascii_case(b"renderfx") {
        let Some(fx) = source_i32(raw) else {
            return false;
        };
        render.fx = fx as u8;
    } else if key.eq_ignore_ascii_case(b"effects") {
        let Some(effects) = source_i32(raw) else {
            return false;
        };
        render.effects = effects as u16 & 0x03ff;
    } else if key.eq_ignore_ascii_case(b"renderamt") {
        let Some(alpha) = source_i32(raw) else {
            return false;
        };
        render.color[3] = alpha as u8;
    } else if key.eq_ignore_ascii_case(b"rendercolor") || key.eq_ignore_ascii_case(b"rendercolor32")
    {
        let Some(color) = source_color(raw) else {
            return false;
        };
        render.color[..3].copy_from_slice(&color[..3]);
    } else if key.eq_ignore_ascii_case(b"disableshadows") {
        let Some(disable) = source_i32(raw) else {
            return false;
        };
        if disable != 0 {
            render.effects |= EF_NOSHADOW;
        }
    } else {
        return false;
    }
    true
}

fn source_i32(value: &[u8]) -> Option<i32> {
    let value = std::str::from_utf8(value).ok()?.trim_start();
    let (negative, digits) = if let Some(value) = value.strip_prefix('-') {
        (true, value)
    } else if let Some(value) = value.strip_prefix('+') {
        (false, value)
    } else {
        (false, value)
    };
    let digits = digits
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .copied()
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return Some(0);
    }
    let magnitude = digits.iter().try_fold(0_i64, |value, digit| {
        value.checked_mul(10)?.checked_add(i64::from(*digit - b'0'))
    })?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

fn source_color(value: &[u8]) -> Option<[u8; 4]> {
    let mut output = [0; 4];
    for (index, field) in value
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|v| !v.is_empty())
        .take(4)
        .enumerate()
    {
        output[index] = source_i32(field)? as u8;
    }
    Some(output)
}

fn variant_i32(value: &Variant) -> Option<i32> {
    match value {
        Variant::Integer(value) => Some(*value),
        Variant::Float(bits) => {
            let value = f32::from_bits(*bits);
            let widened = f64::from(value);
            (value.is_finite() && widened >= f64::from(i32::MIN) && widened <= f64::from(i32::MAX))
                .then_some(value as i32)
        }
        Variant::String(value) => source_i32(value),
        _ => None,
    }
}

fn variant_color(value: &Variant) -> Option<[u8; 4]> {
    match value {
        Variant::Color(value) => Some(*value),
        Variant::String(value) => source_color(value),
        _ => None,
    }
}

fn mover_presentation(entity: &RuntimeEntity, mover: &MoverState) -> BrushMoverPresentation {
    let travel = sub(mover.open, mover.closed);
    let angular_travel = sub(mover.open_angles, mover.closed_angles);
    let (travel, current) = if dot(travel, travel) > f32::EPSILON {
        (travel, sub(entity.local_transform.origin, mover.closed))
    } else {
        (
            angular_travel,
            sub(entity.local_transform.angles, mover.closed_angles),
        )
    };
    let length_squared = dot(travel, travel);
    let progress = if length_squared > 0.0 {
        dot(current, travel) / length_squared
    } else {
        match mover.position {
            MoverPosition::Closed => 0.0,
            MoverPosition::Open => 1.0,
            MoverPosition::Positioned(bits) => f32::from_bits(bits),
            MoverPosition::Opening | MoverPosition::Closing => mover
                .pending
                .as_ref()
                .map_or(0.0, |pending| if pending.opening { 1.0 } else { 0.0 }),
        }
    };
    BrushMoverPresentation {
        kind: mover.kind,
        class: mover.class,
        position: mover.position,
        progress_bits: progress.to_bits(),
        request_id: mover.pending.as_ref().map(|pending| pending.request_id),
        local_destination: mover
            .pending
            .as_ref()
            .map(|pending| pending.local_destination),
        world_destination: mover
            .pending
            .as_ref()
            .map(|pending| pending.world_destination),
        local_angles_destination: mover
            .pending
            .as_ref()
            .map(|pending| pending.local_angles_destination),
        world_angles_destination: mover
            .pending
            .as_ref()
            .map(|pending| pending.world_angles_destination),
        angular_velocity: mover
            .pending
            .as_ref()
            .map(|pending| pending.angular_velocity),
        continuous: mover.pending.as_ref().map(|pending| pending.continuous),
        opening: mover.pending.as_ref().map(|pending| pending.opening),
    }
}

fn delay_ticks(delay: f32, tick_interval: f32) -> u64 {
    if delay <= 0.0 {
        0
    } else {
        (f64::from(delay) / f64::from(tick_interval)).ceil() as u64
    }
}

fn approach(target: f32, value: f32, speed: f32) -> f32 {
    let delta = target - value;
    if delta > speed {
        value + speed
    } else if delta < -speed {
        value - speed
    } else {
        target
    }
}

fn simple_spline(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

fn angle_distance(target: f32, value: f32) -> f32 {
    let mut delta = (target - value) % 360.0;
    if delta > 180.0 {
        delta -= 360.0;
    } else if delta < -180.0 {
        delta += 360.0;
    }
    delta
}

fn approach_angle(target: f32, value: f32, speed: f32) -> f32 {
    value + angle_distance(target, value).clamp(-speed.abs(), speed.abs())
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

fn toggle_rotation_axis(flags: i32) -> [f32; 3] {
    let mut axis = if flags & 64 != 0 {
        [0.0, 0.0, 1.0]
    } else if flags & 128 != 0 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    if flags & 2 != 0 {
        axis = scale(axis, -1.0);
    }
    axis
}

fn rotating_axis(flags: i32) -> [f32; 3] {
    let mut axis = if flags & 4 != 0 {
        [0.0, 0.0, 1.0]
    } else if flags & 8 != 0 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    if flags & 2 != 0 {
        axis = scale(axis, -1.0);
    }
    axis
}

fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

fn vector_angles(direction: [f32; 3]) -> [f32; 3] {
    if direction[0] == 0.0 && direction[1] == 0.0 {
        [if direction[2] > 0.0 { 270.0 } else { 90.0 }, 0.0, 0.0]
    } else {
        let yaw = direction[1]
            .atan2(direction[0])
            .to_degrees()
            .rem_euclid(360.0);
        let forward = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        let pitch = (-direction[2])
            .atan2(forward)
            .to_degrees()
            .rem_euclid(360.0);
        [pitch, yaw, 0.0]
    }
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

fn quat_slerp(from: [f64; 4], mut to: [f64; 4], fraction: f64) -> [f64; 4] {
    let mut cosine = from.into_iter().zip(to).map(|(a, b)| a * b).sum::<f64>();
    if cosine < 0.0 {
        cosine = -cosine;
        to = to.map(|value| -value);
    }
    let (first, second) = if cosine > 0.999_999 {
        (1.0 - fraction, fraction)
    } else {
        let angle = cosine.clamp(-1.0, 1.0).acos();
        let sine = angle.sin();
        (
            ((1.0 - fraction) * angle).sin() / sine,
            (fraction * angle).sin() / sine,
        )
    };
    std::array::from_fn(|index| from[index] * first + to[index] * second)
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

pub(crate) struct SnapshotWriter {
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

    fn u16(&mut self, value: u16) -> Result<(), RuntimeFailure> {
        self.extend(&value.to_le_bytes())
    }

    fn bool(&mut self, value: bool) -> Result<(), RuntimeFailure> {
        self.u8(u8::from(value))
    }

    fn u32(&mut self, value: u32) -> Result<(), RuntimeFailure> {
        self.extend(&value.to_le_bytes())
    }

    pub(crate) fn i32(&mut self, value: i32) -> Result<(), RuntimeFailure> {
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
            Variant::Color(value) => {
                self.u8(7)?;
                self.extend(value)
            }
            Variant::PositionVector(value) => {
                self.u8(8)?;
                for bits in value {
                    self.u32(*bits)?;
                }
                Ok(())
            }
        }
    }
}

fn encode_state(state: &WorldState, config: &EntityWorldConfig) -> Result<Vec<u8>, RuntimeFailure> {
    let mut output = SnapshotWriter::new(config.limits.max_snapshot_bytes);
    output.extend(b"PSEN")?;
    output.u32(5)?;
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
    output.u64(state.revision)?;
    output.u64(state.next_creation_order)?;
    output.u64(state.next_output_id)?;
    output.u64(state.next_event_id)?;
    output.u64(state.next_enqueue_sequence)?;
    output.u64(state.next_transition_sequence)?;
    output.u64(state.next_mover_request_id)?;
    output.u32(state.next_template_instance)?;
    output.usize(state.diagnostics)?;
    state.random.encode(&mut output)?;
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
        Coverage::Missing => 5,
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
    output.usize(entity.fields.len())?;
    for field in &entity.fields {
        output.bytes(&field.key)?;
        output.u8(match field.field_type {
            FieldType::Void => 0,
            FieldType::Boolean => 1,
            FieldType::Integer => 2,
            FieldType::Float => 3,
            FieldType::String => 4,
            FieldType::Vector => 5,
            FieldType::PositionVector => 6,
            FieldType::Color => 7,
            FieldType::Handle => 8,
            FieldType::Variant => 9,
        })?;
        output.optional_bytes(field.writable_input.as_deref())?;
        output.bool(field.source_pair.is_some())?;
        if let Some(pair) = field.source_pair {
            output.usize(pair)?;
        }
        output.bool(field.value.is_some())?;
        if let Some(value) = &field.value {
            output.variant(value)?;
        }
        output.u8(match field.coverage {
            Coverage::Handled => 0,
            Coverage::IntentionallyInert => 1,
            Coverage::Unsupported => 2,
            Coverage::Unknown => 3,
            Coverage::Malformed => 4,
            Coverage::Missing => 5,
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
    output.bool(entity.render.brush_model.is_some())?;
    if let Some(model) = entity.render.brush_model {
        output.usize(model)?;
    }
    output.u8(entity.render.mode)?;
    output.extend(&entity.render.color)?;
    output.u8(entity.render.fx)?;
    output.u16(entity.render.effects)?;
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
            output.u8(match state.class {
                MoverClass::Button => 0,
                MoverClass::RotatingButton => 1,
                MoverClass::MomentaryRotatingButton => 2,
                MoverClass::Door => 3,
                MoverClass::RotatingDoor => 4,
                MoverClass::Linear => 5,
                MoverClass::Rotating => 6,
                MoverClass::Platform => 7,
                MoverClass::RotatingPlatform => 8,
                MoverClass::Train => 9,
                MoverClass::TrackTrain => 10,
            })?;
            for value in state
                .closed
                .into_iter()
                .chain(state.open)
                .chain(state.closed_angles)
                .chain(state.open_angles)
            {
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
                    .chain(pending.local_angles_destination)
                    .chain(pending.world_angles_destination)
                    .chain(pending.angular_velocity)
                {
                    output.u32(value.to_bits())?;
                }
                output.bool(pending.opening)?;
                output.bool(pending.continuous)?;
                output.optional_handle(pending.path_destination)?;
                output.optional_handle(pending.activator)?;
            }
            output.bool(state.locked)?;
            output.bool(state.toggle)?;
            output.bool(state.force_closed)?;
            output.bool(state.no_auto_return)?;
            output.bool(state.stay_pushed)?;
            output.bool(state.outputs_reversed)?;
            output.u32(state.block_damage_bits)?;
            output.bool(state.damage_activates)?;
            output.bool(state.dont_move)?;
            output.optional_handle(state.activator)?;
            output.u32(state.continuous_speed_bits)?;
            output.bool(state.solid)?;
            output.bool(state.path.is_some())?;
            if let Some(path) = &state.path {
                output.bool(path.track)?;
                output.optional_handle(path.current)?;
                output.bytes(&path.target_name)?;
                output.bool(path.running)?;
                output.bool(path.forward)?;
                output.u32(path.height_bits)?;
                output.u32(path.length_bits)?;
                output.u32(path.maximum_speed_bits)?;
                output.u32(path.current_speed_bits)?;
                output.u32(path.desired_speed_bits)?;
                output.u32(path.unmodified_desired_speed_bits)?;
                output.u32(path.old_speed_bits)?;
                output.u32(path.forward_modifier_bits)?;
                output.u32(path.acceleration_bits)?;
                output.u32(path.deceleration_bits)?;
                output.u32(path.bank_bits)?;
                output.i32(path.velocity_type)?;
                output.i32(path.orientation_type)?;
                output.bool(path.manual_speed_changes)?;
                output.bool(path.accelerating)?;
                output.bool(path.controls_disabled)?;
            }
            output.bool(state.rotator.is_some())?;
            if let Some(rotator) = &state.rotator {
                output.u32(rotator.target_speed_bits)?;
                output.u32(rotator.friction_bits)?;
                output.bool(rotator.accelerate)?;
                output.bool(rotator.reversed)?;
                output.bool(rotator.stop_at_start)?;
                for bits in rotator.start_angles {
                    output.u32(bits)?;
                }
            }
            Ok(())
        }
        BehaviorState::LogicAuto => output.u8(3),
        BehaviorState::Relay(state) => {
            output.u8(4)?;
            output.bool(state.enabled)?;
            output.bool(state.waiting_for_refire)?;
            output.bool(state.remove_on_fire)?;
            output.bool(state.fast_retrigger)
        }
        BehaviorState::Timer(state) => {
            output.u8(11)?;
            output.bool(state.enabled)?;
            output.u32(state.interval_bits)?;
            output.bool(state.alternating)?;
            output.bool(state.high_next)?;
            output.bool(state.use_random)?;
            output.u32(state.lower_bound_bits)?;
            output.u32(state.upper_bound_bits)?;
            output.bool(state.next_fire_tick.is_some())?;
            if let Some(tick) = state.next_fire_tick {
                output.u64(tick)?;
            }
            Ok(())
        }
        BehaviorState::Template(state) => {
            output.u8(12)?;
            output.usize(state.requested_names.len())?;
            for name in &state.requested_names {
                output.bytes(name)?;
            }
            output.usize(state.members.len())?;
            for member in &state.members {
                output.usize(member.definition.index)?;
                output.usize(member.definition.pairs.len())?;
                for pair in &member.definition.pairs {
                    output.bytes(&pair.key)?;
                    output.bytes(&pair.value)?;
                    output.usize(pair.key_range.start)?;
                    output.usize(pair.key_range.end)?;
                    output.usize(pair.value_range.start)?;
                    output.usize(pair.value_range.end)?;
                }
                output.transform(member.relative_transform)?;
            }
            output.bool(state.preserve_names)?;
            output.bool(state.keep_prototypes)?;
            output.usize(state.instances)
        }
        BehaviorState::PathNode(state) => {
            output.u8(13)?;
            output.bool(state.track)?;
            output.bytes(&state.next_name)?;
            output.bytes(&state.alternate_name)?;
            output.optional_handle(state.next)?;
            output.optional_handle(state.previous)?;
            output.optional_handle(state.alternate)?;
            output.i32(state.flags)?;
            output.u32(state.wait_bits)?;
            output.u32(state.speed_bits)?;
            output.i32(state.orientation)
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
                TriggerKind::Soundscape => 5,
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
            output.u32(state.mutable_value_bits)?;
            output.u32(state.damage_cap_bits)?;
            output.i32(state.damage_model)?;
            output.i32(state.damage_type)?;
            output.bool(state.no_damage_force)?;
            output.u32(state.original_damage_bits)?;
            output.bool(state.next_hurt_tick.is_some())?;
            if let Some(tick) = state.next_hurt_tick {
                output.u64(tick)?;
            }
            output.usize(state.hurt_last_cycle.len())?;
            for subject in &state.hurt_last_cycle {
                output.handle(*subject)?;
            }
            for bits in state.direction {
                output.u32(bits)?;
            }
            output.u32(state.speed_bits)?;
            output.bool(state.push_once)?;
            output.bytes(&state.target_name)?;
            output.bytes(&state.landmark_name)?;
            output.bool(state.preserve_angles)?;
            output.u32(state.catapult_physics_speed_bits)?;
            output.bool(state.catapult_exact_velocity)?;
            output.i32(state.catapult_exact_choice)?;
            output.bool(state.catapult_threshold)?;
            output.u32(state.catapult_lower_bits)?;
            output.u32(state.catapult_upper_bits)?;
            output.usize(state.catapult_cooldowns.len())?;
            for (subject, tick) in &state.catapult_cooldowns {
                output.handle(*subject)?;
                output.u64(*tick)?;
            }
            Ok(())
        }
        BehaviorState::External => output.u8(9),
        BehaviorState::Breakable(state) => {
            output.u8(10)?;
            output.bool(state.broken)?;
            output.bool(state.can_break)?;
            output.i32(state.health)?;
            output.i32(state.maximum_health)?;
            output.i32(state.minimum_damage)?;
            output.bool(state.trigger_only)?;
            output.optional_handle(state.breaker)
        }
        BehaviorState::DynamicProp(state) => {
            output.u8(14)?;
            output.i32(state.skin)?;
            output.bool(state.visible)?;
            output.bool(state.collision_enabled)?;
            output.bytes(&state.default_animation)?;
            output.optional_bytes(state.requested_animation.as_deref())?;
            output.optional_bytes(state.current_animation.as_deref())
        }
        BehaviorState::Pickup(state) => {
            output.u8(15)?;
            output.bool(state.touchable)?;
            output.bool(state.visible)?;
            output.transform(state.original_transform)?;
            output.optional_handle(state.pending_subject)
        }
    }
}
