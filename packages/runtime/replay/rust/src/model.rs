use std::{collections::BTreeMap, fmt, ops::Range, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayIdentity {
    pub source_name: String,
    pub source_bytes: u64,
    pub source_sha256: [u8; 32],
    pub profile: String,
    pub game: String,
    pub codec: String,
    pub decoder: String,
    pub index: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Coverage {
    Handled,
    IntentionallyInert,
    Unsupported,
    Unknown,
    Malformed,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SetupFamily {
    DataTables,
    StringTables,
    Baselines,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpolationPolicy {
    Discrete,
    Linear,
    Angular,
    NoInterpolation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedField {
    pub bytes: Arc<[u8]>,
    pub interpolation: InterpolationPolicy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EntityIdentity {
    pub index: u16,
    pub generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedEntity {
    pub identity: EntityIdentity,
    pub class_id: u16,
    pub fields: BTreeMap<u32, RecordedField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayEventKind {
    Game,
    Audio,
    Particle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedEvent {
    pub kind: ReplayEventKind,
    pub selected_game_identity: u32,
    pub target: Option<EntityIdentity>,
    pub action: u32,
    pub payload: Arc<[u8]>,
    pub coverage: Coverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordedOperation {
    SignonState(u8),
    ReplaceSetup {
        family: SetupFamily,
        version: u64,
        bytes: Arc<[u8]>,
    },
    FullState {
        entities: BTreeMap<u16, RecordedEntity>,
        decoder_state: Arc<[u8]>,
    },
    CreateEntity(RecordedEntity),
    PatchEntity {
        identity: EntityIdentity,
        fields: BTreeMap<u32, RecordedField>,
    },
    DeleteEntity(EntityIdentity),
    ReplaceDecoderState(Arc<[u8]>),
    Event(RecordedEvent),
    IntentionallyInert,
    Terminal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRecord {
    pub ordinal: usize,
    pub source_range: Range<u64>,
    pub command_tick: Option<i32>,
    pub server_tick: Option<i32>,
    pub no_interpolation: bool,
    pub coverage: Coverage,
    pub operations: Vec<RecordedOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaySource {
    pub identity: ReplayIdentity,
    pub tick_interval_ns: u64,
    pub records: Arc<[RecordedRecord]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayLimits {
    pub max_resident_bytes: usize,
    pub max_index_entries: usize,
    pub max_index_bytes: usize,
    pub max_checkpoint_count: usize,
    pub max_checkpoint_bytes: usize,
    pub checkpoint_interval_ticks: usize,
    pub max_seek_records: usize,
    pub max_entities: usize,
    pub max_fields_per_entity: usize,
    pub max_events: usize,
    pub max_events_per_tick: usize,
    pub max_event_payload_bytes: usize,
    pub max_snapshot_bytes: usize,
    pub max_diagnostics: usize,
    pub max_step_ticks: usize,
    pub max_rate_numerator: u32,
    pub max_rate_denominator: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReplayCursor {
    pub server_tick: Option<i32>,
    pub occurrence: u32,
    pub last_applied_record_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Opening,
    Signon,
    Ready,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackRate {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupValue {
    pub version: u64,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoritativeState {
    pub cursor: ReplayCursor,
    pub setup: BTreeMap<SetupFamily, SetupValue>,
    pub entities: BTreeMap<u16, RecordedEntity>,
    pub decoder_state: Arc<[u8]>,
    pub latest_command_tick: Option<i32>,
    pub latest_server_tick: Option<i32>,
    pub latest_tick_occurrence: u32,
    pub event_cursor: usize,
    pub events_at_latest_tick: usize,
    pub signon_state: Option<u8>,
    pub has_full_state: bool,
    pub discontinuity: bool,
}

impl Default for AuthoritativeState {
    fn default() -> Self {
        Self {
            cursor: ReplayCursor {
                server_tick: None,
                occurrence: 0,
                last_applied_record_ordinal: None,
            },
            setup: BTreeMap::new(),
            entities: BTreeMap::new(),
            decoder_state: Arc::from([]),
            latest_command_tick: None,
            latest_server_tick: None,
            latest_tick_occurrence: 0,
            event_cursor: 0,
            events_at_latest_tick: 0,
            signon_state: None,
            has_full_state: false,
            discontinuity: true,
        }
    }
}

impl AuthoritativeState {
    pub fn is_ready(&self) -> bool {
        self.has_full_state
            && self.setup.contains_key(&SetupFamily::DataTables)
            && self.setup.contains_key(&SetupFamily::StringTables)
            && self.setup.contains_key(&SetupFamily::Baselines)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        encode_cursor(&mut output, self.cursor);
        write_u64(&mut output, self.setup.len() as u64);
        for (family, value) in &self.setup {
            output.push(match family {
                SetupFamily::DataTables => 0,
                SetupFamily::StringTables => 1,
                SetupFamily::Baselines => 2,
            });
            write_u64(&mut output, value.version);
            write_bytes(&mut output, &value.bytes);
        }
        write_u64(&mut output, self.entities.len() as u64);
        for entity in self.entities.values() {
            write_u16(&mut output, entity.identity.index);
            write_u32(&mut output, entity.identity.generation);
            write_u16(&mut output, entity.class_id);
            write_u64(&mut output, entity.fields.len() as u64);
            for (field_id, field) in &entity.fields {
                write_u32(&mut output, *field_id);
                output.push(match field.interpolation {
                    InterpolationPolicy::Discrete => 0,
                    InterpolationPolicy::Linear => 1,
                    InterpolationPolicy::Angular => 2,
                    InterpolationPolicy::NoInterpolation => 3,
                });
                write_bytes(&mut output, &field.bytes);
            }
        }
        write_bytes(&mut output, &self.decoder_state);
        write_option_i32(&mut output, self.latest_command_tick);
        write_option_i32(&mut output, self.latest_server_tick);
        write_u32(&mut output, self.latest_tick_occurrence);
        write_u64(&mut output, self.event_cursor as u64);
        write_u64(&mut output, self.events_at_latest_tick as u64);
        output.push(self.signon_state.unwrap_or(u8::MAX));
        output.push(u8::from(self.has_full_state));
        output.push(u8::from(self.discontinuity));
        output
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayEvent {
    pub ordinal: usize,
    pub cursor: ReplayCursor,
    pub record_ordinal: usize,
    pub operation_ordinal: usize,
    pub event: RecordedEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordIndexEntry {
    pub record_ordinal: usize,
    pub source_range: Range<u64>,
    pub command_tick: Option<i32>,
    pub server_tick: Option<i32>,
    pub cursor: ReplayCursor,
    pub first_event_ordinal: usize,
    pub next_event_ordinal: usize,
    pub setup: bool,
    pub state_bearing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaySnapshot {
    pub identity: ReplayIdentity,
    pub limits: ReplayLimits,
    pub lifecycle: Lifecycle,
    pub playback: PlaybackState,
    pub rate: PlaybackRate,
    pub rate_remainder_numerator: u128,
    pub rate_remainder_denominator: u128,
    pub accumulated_source_ns: u128,
    pub position_record: Option<usize>,
    pub next_record: usize,
    pub delivered_event_cursor: usize,
    pub state: AuthoritativeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayPresentationSnapshot {
    pub prior: Arc<AuthoritativeState>,
    pub next: Arc<AuthoritativeState>,
    pub fraction_numerator: u128,
    pub fraction_denominator: u128,
    pub event_range: Range<usize>,
    pub playback: PlaybackState,
    pub discontinuity: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReport {
    pub cursor: ReplayCursor,
    pub event_range: Range<usize>,
    pub discontinuity: bool,
    pub replayed_records: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthorityAudit {
    pub simulation_calls: u64,
    pub applied_records: u64,
    pub restored_checkpoints: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayErrorCode {
    InvalidLimits,
    SourceLimit,
    RecordOrder,
    SourceRange,
    Coverage,
    MissingSetup,
    MissingFullState,
    MissingTerminal,
    OperationAfterTerminal,
    EntityLimit,
    FieldLimit,
    EntityExists,
    MissingEntity,
    EntityGeneration,
    EventLimit,
    EventPayloadLimit,
    EventTickLimit,
    IndexLimit,
    CheckpointLimit,
    SnapshotLimit,
    ResidentLimit,
    NotReady,
    NotPaused,
    InvalidRate,
    ElapsedOverflow,
    StepLimit,
    MissingCursor,
    SeekWorkLimit,
    IdentityMismatch,
    RestoreState,
    ConcurrentMutation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayError {
    pub code: ReplayErrorCode,
    pub record_ordinal: Option<usize>,
    pub operation_ordinal: Option<usize>,
    pub cursor: Option<Box<ReplayCursor>>,
    pub field: &'static str,
    pub required: Option<u64>,
    pub available: Option<u64>,
    pub limit: Option<u64>,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?} ({})", self.code, self.field)
    }
}

impl std::error::Error for ReplayError {}

pub(crate) fn error(code: ReplayErrorCode, field: &'static str) -> ReplayError {
    ReplayError {
        code,
        record_ordinal: None,
        operation_ordinal: None,
        cursor: None,
        field,
        required: None,
        available: None,
        limit: None,
    }
}

fn encode_cursor(output: &mut Vec<u8>, cursor: ReplayCursor) {
    write_option_i32(output, cursor.server_tick);
    write_u32(output, cursor.occurrence);
    match cursor.last_applied_record_ordinal {
        Some(value) => {
            output.push(1);
            write_u64(output, value as u64);
        }
        None => output.push(0),
    }
}

fn write_option_i32(output: &mut Vec<u8>, value: Option<i32>) {
    match value {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_le_bytes());
        }
        None => output.push(0),
    }
}

fn write_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    write_u64(output, bytes.len() as u64);
    output.extend_from_slice(bytes);
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}
