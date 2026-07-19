//! Bounded, lossless framing for Source 1 DEM containers.
//!
//! The current profile is the protocol-3, network-protocol-24 Team Fortress 2
//! contract declared by Valve Source SDK 2013 in
//! `src/public/demofile/demoformat.h` and `src/common/proto_version.h`.

use std::{fmt, ops::Range, sync::Arc};

pub const HEADER_BYTES: usize = 1_072;
pub const COMMAND_INFO_BYTES: usize = 76;
pub const DEMO_STAMP: [u8; 8] = *b"HL2DEMO\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Tf2Demo3Net24,
}

impl Profile {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::Tf2Demo3Net24 => "tf2-demo3-net24",
        }
    }

    const fn demo_protocol(self) -> i32 {
        3
    }

    const fn network_protocol(self) -> i32 {
        24
    }

    const fn game_directory(self) -> &'static [u8] {
        b"tf"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIdentity {
    pub name: String,
    pub byte_length: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_command_records: usize,
    pub max_encoded_bytes_per_command: usize,
    pub max_total_encoded_command_bytes: usize,
    pub max_retained_diagnostic_bytes: usize,
    pub max_streaming_buffer_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coverage {
    Handled,
    IntentionallyInert,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameField {
    pub raw: [u8; 260],
    pub first_nul: Option<usize>,
}

impl NameField {
    pub fn prefix(&self) -> &[u8] {
        &self.raw[..self.first_nul.unwrap_or(self.raw.len())]
    }

    pub fn is_nul_terminated(&self) -> bool {
        self.first_nul.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    pub range: Range<usize>,
    pub stamp: [u8; 8],
    pub demo_protocol: i32,
    pub network_protocol: i32,
    pub server_name: NameField,
    pub client_name: NameField,
    pub map_name: NameField,
    pub game_directory: NameField,
    pub playback_time_bits: u32,
    pub playback_ticks: i32,
    pub playback_frames: i32,
    pub signon_length: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInfo {
    pub range: Range<usize>,
    pub flags: i32,
    pub vectors: [[u32; 3]; 6],
    pub unknown_flag_bits: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketKind {
    Signon,
    Packet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketRecord {
    pub kind: PacketKind,
    pub command_info: CommandInfo,
    pub incoming_sequence: i32,
    pub outgoing_acknowledged_sequence: i32,
    pub declared_payload_bytes: i32,
    pub payload_range: Range<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LengthDelimitedKind {
    ConsoleCommand,
    DataTables,
    StringTables,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LengthDelimitedRecord {
    pub kind: LengthDelimitedKind,
    pub declared_payload_bytes: i32,
    pub payload_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserCommandRecord {
    pub command_sequence: i32,
    pub declared_payload_bytes: i32,
    pub payload_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalTick {
    Complete(i32),
    SourceTvStreamFlush { encoded_low_bytes: [u8; 3] },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandBody {
    Packet(PacketRecord),
    SyncTick,
    LengthDelimited(LengthDelimitedRecord),
    UserCommand(UserCommandRecord),
    Stop { tick: TerminalTick },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub ordinal: usize,
    pub range: Range<usize>,
    pub header_range: Range<usize>,
    pub encoded_id: u8,
    pub tick: Option<i32>,
    pub profile: Profile,
    pub body: CommandBody,
    pub coverage: Coverage,
}

impl Command {
    pub fn payload_range(&self) -> Option<&Range<usize>> {
        match &self.body {
            CommandBody::Packet(value) => Some(&value.payload_range),
            CommandBody::LengthDelimited(value) => Some(&value.payload_range),
            CommandBody::UserCommand(value) => Some(&value.payload_range),
            CommandBody::SyncTick | CommandBody::Stop { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    pub command_counts: [usize; 9],
    pub packet_count: usize,
    pub signon_end: usize,
    pub terminal_stop_range: Range<usize>,
    pub total_encoded_payload_bytes: usize,
    pub non_handled_commands: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Demo {
    pub source_identity: SourceIdentity,
    pub profile: Profile,
    pub header: Header,
    pub commands: Vec<Command>,
    pub summary: Summary,
    source: Arc<[u8]>,
}

impl Demo {
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }

    pub fn command_bytes(&self, command: &Command) -> &[u8] {
        &self.source[command.range.clone()]
    }

    pub fn payload_bytes(&self, command: &Command) -> Option<&[u8]> {
        command
            .payload_range()
            .map(|range| &self.source[range.clone()])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Malformed,
    Unsupported,
    Unknown,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    StreamingBufferLimit,
    SourceLengthMismatch,
    DiagnosticLimit,
    TruncatedHeader,
    InvalidStamp,
    ProfileMismatch,
    UnterminatedGameDirectory,
    NegativeSignonLength,
    SignonRange,
    SignonBoundary,
    CommandLimit,
    TruncatedCommandHeader,
    UnknownCommand,
    TruncatedCommandInfo,
    UnknownCommandInfoFlags,
    NegativePayloadLength,
    PayloadRangeOverflow,
    TruncatedPayload,
    CommandPayloadLimit,
    TotalPayloadLimit,
    MissingStop,
    TrailingBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub classification: Classification,
    pub code: ErrorCode,
    pub range: Range<usize>,
    pub profile: Profile,
    pub command_ordinal: Option<usize>,
    pub field: &'static str,
    pub declared: Option<i64>,
    pub available: Option<u64>,
    pub limit: Option<u64>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?} {:?} at {}..{} ({})",
            self.classification, self.code, self.range.start, self.range.end, self.field
        )
    }
}

impl std::error::Error for ParseError {}

pub fn parse(
    source: &[u8],
    source_identity: SourceIdentity,
    profile: Profile,
    limits: Limits,
) -> Result<Demo, ParseError> {
    if source.len() > limits.max_input_bytes {
        return Err(error(
            profile,
            ErrorCode::InputLimit,
            Classification::Malformed,
            source.len()..source.len(),
            None,
            "source",
            Some(source.len() as i64),
            None,
            Some(limits.max_input_bytes as u64),
        ));
    }
    if source_identity.name.len() > limits.max_retained_diagnostic_bytes {
        return Err(error(
            profile,
            ErrorCode::DiagnosticLimit,
            Classification::Malformed,
            0..0,
            None,
            "source_identity.name",
            Some(source_identity.name.len() as i64),
            None,
            Some(limits.max_retained_diagnostic_bytes as u64),
        ));
    }
    if source_identity.byte_length != source.len() as u64 {
        return Err(error(
            profile,
            ErrorCode::SourceLengthMismatch,
            Classification::Malformed,
            0..source.len(),
            None,
            "source_identity.byte_length",
            Some(source_identity.byte_length as i64),
            Some(source.len() as u64),
            None,
        ));
    }
    if source.len() < HEADER_BYTES {
        return Err(error(
            profile,
            ErrorCode::TruncatedHeader,
            Classification::Malformed,
            source.len()..HEADER_BYTES,
            None,
            "header",
            Some(HEADER_BYTES as i64),
            Some(source.len() as u64),
            None,
        ));
    }

    let header = parse_header(source, profile)?;
    let signon_length = usize::try_from(header.signon_length).map_err(|_| {
        error(
            profile,
            ErrorCode::NegativeSignonLength,
            Classification::Malformed,
            1_068..1_072,
            None,
            "header.signon_length",
            Some(header.signon_length as i64),
            None,
            None,
        )
    })?;
    let signon_end = HEADER_BYTES.checked_add(signon_length).ok_or_else(|| {
        error(
            profile,
            ErrorCode::SignonRange,
            Classification::Malformed,
            1_068..1_072,
            None,
            "header.signon_length",
            Some(header.signon_length as i64),
            Some(source.len() as u64),
            None,
        )
    })?;
    if signon_end > source.len() {
        return Err(error(
            profile,
            ErrorCode::SignonRange,
            Classification::Malformed,
            1_068..1_072,
            None,
            "header.signon_length",
            Some(signon_end as i64),
            Some(source.len() as u64),
            None,
        ));
    }

    let mut cursor = HEADER_BYTES;
    let mut commands = Vec::new();
    let mut command_counts = [0_usize; 9];
    let mut total_payload = 0_usize;
    let mut boundaries = vec![HEADER_BYTES];
    let mut terminal = None;
    while cursor < source.len() {
        if commands.len() >= limits.max_command_records {
            return Err(error(
                profile,
                ErrorCode::CommandLimit,
                Classification::Malformed,
                cursor..cursor,
                Some(commands.len()),
                "commands",
                Some((commands.len() + 1) as i64),
                None,
                Some(limits.max_command_records as u64),
            ));
        }
        let ordinal = commands.len();
        let start = cursor;
        let remaining = source.len() - cursor;

        if remaining == 4 && source[cursor] == 7 {
            let encoded_low_bytes = source[cursor + 1..cursor + 4]
                .try_into()
                .expect("fixed terminal range");
            cursor += 4;
            let command = Command {
                ordinal,
                range: start..cursor,
                header_range: start..cursor,
                encoded_id: 7,
                tick: None,
                profile,
                body: CommandBody::Stop {
                    tick: TerminalTick::SourceTvStreamFlush { encoded_low_bytes },
                },
                coverage: Coverage::Handled,
            };
            command_counts[7] += 1;
            terminal = Some(command.range.clone());
            boundaries.push(cursor);
            commands.push(command);
            break;
        }

        if remaining < 5 {
            return Err(error(
                profile,
                ErrorCode::TruncatedCommandHeader,
                Classification::Malformed,
                cursor..source.len(),
                Some(ordinal),
                "command.header",
                Some(5),
                Some(remaining as u64),
                None,
            ));
        }
        let encoded_id = source[cursor];
        let tick = i32_at(source, cursor + 1);
        cursor += 5;
        let header_range = start..cursor;
        if !(1..=8).contains(&encoded_id) {
            return Err(error(
                profile,
                ErrorCode::UnknownCommand,
                Classification::Unknown,
                start..start + 1,
                Some(ordinal),
                "command.id",
                Some(encoded_id as i64),
                None,
                None,
            ));
        }

        let (body, coverage) = match encoded_id {
            1 | 2 => {
                let kind = if encoded_id == 1 {
                    PacketKind::Signon
                } else {
                    PacketKind::Packet
                };
                let (record, next, command_coverage) = parse_packet(
                    source,
                    cursor,
                    ordinal,
                    kind,
                    profile,
                    limits,
                    &mut total_payload,
                )?;
                cursor = next;
                (CommandBody::Packet(record), command_coverage)
            }
            3 => (CommandBody::SyncTick, Coverage::IntentionallyInert),
            4 | 6 | 8 => {
                let kind = match encoded_id {
                    4 => LengthDelimitedKind::ConsoleCommand,
                    6 => LengthDelimitedKind::DataTables,
                    8 => LengthDelimitedKind::StringTables,
                    _ => unreachable!(),
                };
                let (record, next) = parse_length_delimited(
                    source,
                    cursor,
                    ordinal,
                    kind,
                    profile,
                    limits,
                    &mut total_payload,
                )?;
                cursor = next;
                (CommandBody::LengthDelimited(record), Coverage::Handled)
            }
            5 => {
                let sequence_range = require_range(
                    source,
                    cursor,
                    4,
                    ordinal,
                    profile,
                    "command.user_command.sequence",
                )?;
                let command_sequence = i32_at(source, sequence_range.start);
                cursor = sequence_range.end;
                let (payload, next) = parse_payload(
                    source,
                    cursor,
                    ordinal,
                    profile,
                    limits,
                    &mut total_payload,
                    "command.user_command.payload",
                )?;
                cursor = next;
                (
                    CommandBody::UserCommand(UserCommandRecord {
                        command_sequence,
                        declared_payload_bytes: payload.declared,
                        payload_range: payload.range,
                    }),
                    Coverage::Handled,
                )
            }
            7 => (
                CommandBody::Stop {
                    tick: TerminalTick::Complete(tick),
                },
                Coverage::Handled,
            ),
            _ => unreachable!(),
        };

        let command = Command {
            ordinal,
            range: start..cursor,
            header_range,
            encoded_id,
            tick: Some(tick),
            profile,
            body,
            coverage,
        };
        command_counts[encoded_id as usize] += 1;
        boundaries.push(cursor);
        if encoded_id == 7 {
            terminal = Some(command.range.clone());
            commands.push(command);
            if cursor != source.len() {
                return Err(error(
                    profile,
                    ErrorCode::TrailingBytes,
                    Classification::Malformed,
                    cursor..source.len(),
                    Some(ordinal),
                    "command.stop.trailing_bytes",
                    Some((source.len() - cursor) as i64),
                    None,
                    None,
                ));
            }
            break;
        }
        commands.push(command);
    }

    let terminal_stop_range = terminal.ok_or_else(|| {
        error(
            profile,
            ErrorCode::MissingStop,
            Classification::Missing,
            source.len()..source.len(),
            Some(commands.len()),
            "command.stop",
            None,
            None,
            None,
        )
    })?;
    if !boundaries.contains(&signon_end) {
        return Err(error(
            profile,
            ErrorCode::SignonBoundary,
            Classification::Malformed,
            signon_end..signon_end.saturating_add(1).min(source.len()),
            None,
            "header.signon_length",
            Some(signon_end as i64),
            None,
            None,
        ));
    }

    let packet_count = command_counts[1] + command_counts[2];
    let non_handled_commands = commands
        .iter()
        .filter(|command| command.coverage != Coverage::Handled)
        .map(|command| command.ordinal)
        .collect();
    Ok(Demo {
        source_identity,
        profile,
        header,
        commands,
        summary: Summary {
            command_counts,
            packet_count,
            signon_end,
            terminal_stop_range,
            total_encoded_payload_bytes: total_payload,
            non_handled_commands,
        },
        source: Arc::from(source),
    })
}

pub fn parse_chunks<I, B>(
    chunks: I,
    source_identity: SourceIdentity,
    profile: Profile,
    limits: Limits,
) -> Result<Demo, ParseError>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut source = Vec::new();
    for chunk in chunks {
        let chunk = chunk.as_ref();
        let next = source.len().checked_add(chunk.len()).ok_or_else(|| {
            error(
                profile,
                ErrorCode::StreamingBufferLimit,
                Classification::Malformed,
                source.len()..source.len(),
                None,
                "stream.buffer",
                None,
                None,
                Some(limits.max_streaming_buffer_bytes as u64),
            )
        })?;
        if next > limits.max_streaming_buffer_bytes || next > limits.max_input_bytes {
            return Err(error(
                profile,
                ErrorCode::StreamingBufferLimit,
                Classification::Malformed,
                source.len()..next,
                None,
                "stream.buffer",
                Some(next as i64),
                None,
                Some(
                    limits
                        .max_streaming_buffer_bytes
                        .min(limits.max_input_bytes) as u64,
                ),
            ));
        }
        source.extend_from_slice(chunk);
    }
    parse(&source, source_identity, profile, limits)
}

fn parse_header(source: &[u8], profile: Profile) -> Result<Header, ParseError> {
    let stamp = source[0..8].try_into().expect("validated header range");
    if stamp != DEMO_STAMP {
        return Err(error(
            profile,
            ErrorCode::InvalidStamp,
            Classification::Malformed,
            0..8,
            None,
            "header.stamp",
            None,
            None,
            None,
        ));
    }
    let demo_protocol = i32_at(source, 8);
    let network_protocol = i32_at(source, 12);
    if demo_protocol != profile.demo_protocol() || network_protocol != profile.network_protocol() {
        return Err(error(
            profile,
            ErrorCode::ProfileMismatch,
            Classification::Unsupported,
            8..16,
            None,
            "header.protocol",
            Some(((demo_protocol as i64 as u64) << 32 | network_protocol as u32 as u64) as i64),
            None,
            None,
        ));
    }
    let server_name = name_field(source, 16);
    let client_name = name_field(source, 276);
    let map_name = name_field(source, 536);
    let game_directory = name_field(source, 796);
    if !game_directory.is_nul_terminated() {
        return Err(error(
            profile,
            ErrorCode::UnterminatedGameDirectory,
            Classification::Malformed,
            796..1_056,
            None,
            "header.game_directory",
            None,
            None,
            None,
        ));
    }
    if game_directory.prefix() != profile.game_directory() {
        return Err(error(
            profile,
            ErrorCode::ProfileMismatch,
            Classification::Unsupported,
            796..796 + game_directory.first_nul.unwrap_or(260),
            None,
            "header.game_directory",
            None,
            None,
            None,
        ));
    }
    Ok(Header {
        range: 0..HEADER_BYTES,
        stamp,
        demo_protocol,
        network_protocol,
        server_name,
        client_name,
        map_name,
        game_directory,
        playback_time_bits: u32_at(source, 1_056),
        playback_ticks: i32_at(source, 1_060),
        playback_frames: i32_at(source, 1_064),
        signon_length: i32_at(source, 1_068),
    })
}

fn parse_packet(
    source: &[u8],
    cursor: usize,
    ordinal: usize,
    kind: PacketKind,
    profile: Profile,
    limits: Limits,
    total_payload: &mut usize,
) -> Result<(PacketRecord, usize, Coverage), ParseError> {
    let info_range = require_range(
        source,
        cursor,
        COMMAND_INFO_BYTES,
        ordinal,
        profile,
        "command.packet.info",
    )?;
    let flags = i32_at(source, info_range.start);
    let mut vectors = [[0_u32; 3]; 6];
    let mut vector_cursor = info_range.start + 4;
    for vector in &mut vectors {
        for component in vector {
            *component = u32_at(source, vector_cursor);
            vector_cursor += 4;
        }
    }
    let unknown_flag_bits = flags as u32 & !0b111;
    let sequence_range = require_range(
        source,
        info_range.end,
        8,
        ordinal,
        profile,
        "command.packet.sequences",
    )?;
    let incoming_sequence = i32_at(source, sequence_range.start);
    let outgoing_acknowledged_sequence = i32_at(source, sequence_range.start + 4);
    let (payload, next) = parse_payload(
        source,
        sequence_range.end,
        ordinal,
        profile,
        limits,
        total_payload,
        "command.packet.payload",
    )?;
    let coverage = if unknown_flag_bits == 0 {
        Coverage::Handled
    } else {
        Coverage::Unknown
    };
    Ok((
        PacketRecord {
            kind,
            command_info: CommandInfo {
                range: info_range,
                flags,
                vectors,
                unknown_flag_bits,
            },
            incoming_sequence,
            outgoing_acknowledged_sequence,
            declared_payload_bytes: payload.declared,
            payload_range: payload.range,
        },
        next,
        coverage,
    ))
}

fn parse_length_delimited(
    source: &[u8],
    cursor: usize,
    ordinal: usize,
    kind: LengthDelimitedKind,
    profile: Profile,
    limits: Limits,
    total_payload: &mut usize,
) -> Result<(LengthDelimitedRecord, usize), ParseError> {
    let (payload, next) = parse_payload(
        source,
        cursor,
        ordinal,
        profile,
        limits,
        total_payload,
        "command.length_delimited.payload",
    )?;
    Ok((
        LengthDelimitedRecord {
            kind,
            declared_payload_bytes: payload.declared,
            payload_range: payload.range,
        },
        next,
    ))
}

struct Payload {
    declared: i32,
    range: Range<usize>,
}

#[allow(clippy::too_many_arguments)]
fn parse_payload(
    source: &[u8],
    cursor: usize,
    ordinal: usize,
    profile: Profile,
    limits: Limits,
    total_payload: &mut usize,
    field: &'static str,
) -> Result<(Payload, usize), ParseError> {
    let length_range = require_range(source, cursor, 4, ordinal, profile, field)?;
    let declared = i32_at(source, length_range.start);
    let length = usize::try_from(declared).map_err(|_| {
        error(
            profile,
            ErrorCode::NegativePayloadLength,
            Classification::Malformed,
            length_range.clone(),
            Some(ordinal),
            field,
            Some(declared as i64),
            None,
            None,
        )
    })?;
    if length > limits.max_encoded_bytes_per_command {
        return Err(error(
            profile,
            ErrorCode::CommandPayloadLimit,
            Classification::Malformed,
            length_range.clone(),
            Some(ordinal),
            field,
            Some(length as i64),
            None,
            Some(limits.max_encoded_bytes_per_command as u64),
        ));
    }
    let start = length_range.end;
    let end = start.checked_add(length).ok_or_else(|| {
        error(
            profile,
            ErrorCode::PayloadRangeOverflow,
            Classification::Malformed,
            length_range.clone(),
            Some(ordinal),
            field,
            Some(length as i64),
            Some(source.len().saturating_sub(start) as u64),
            None,
        )
    })?;
    if end > source.len() {
        return Err(error(
            profile,
            ErrorCode::TruncatedPayload,
            Classification::Malformed,
            start..end,
            Some(ordinal),
            field,
            Some(length as i64),
            Some(source.len().saturating_sub(start) as u64),
            None,
        ));
    }
    let next_total = total_payload.checked_add(length).ok_or_else(|| {
        error(
            profile,
            ErrorCode::TotalPayloadLimit,
            Classification::Malformed,
            start..end,
            Some(ordinal),
            field,
            None,
            None,
            Some(limits.max_total_encoded_command_bytes as u64),
        )
    })?;
    if next_total > limits.max_total_encoded_command_bytes {
        return Err(error(
            profile,
            ErrorCode::TotalPayloadLimit,
            Classification::Malformed,
            start..end,
            Some(ordinal),
            field,
            Some(next_total as i64),
            None,
            Some(limits.max_total_encoded_command_bytes as u64),
        ));
    }
    *total_payload = next_total;
    Ok((
        Payload {
            declared,
            range: start..end,
        },
        end,
    ))
}

fn require_range(
    source: &[u8],
    start: usize,
    length: usize,
    ordinal: usize,
    profile: Profile,
    field: &'static str,
) -> Result<Range<usize>, ParseError> {
    let end = start.checked_add(length).ok_or_else(|| {
        error(
            profile,
            ErrorCode::PayloadRangeOverflow,
            Classification::Malformed,
            start..start,
            Some(ordinal),
            field,
            Some(length as i64),
            None,
            None,
        )
    })?;
    if end > source.len() {
        let code = if field == "command.packet.info" {
            ErrorCode::TruncatedCommandInfo
        } else {
            ErrorCode::TruncatedPayload
        };
        return Err(error(
            profile,
            code,
            Classification::Malformed,
            start..end,
            Some(ordinal),
            field,
            Some(length as i64),
            Some(source.len().saturating_sub(start) as u64),
            None,
        ));
    }
    Ok(start..end)
}

fn name_field(source: &[u8], start: usize) -> NameField {
    let raw: [u8; 260] = source[start..start + 260]
        .try_into()
        .expect("validated header range");
    NameField {
        first_nul: raw.iter().position(|byte| *byte == 0),
        raw,
    }
}

fn i32_at(source: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        source[offset..offset + 4]
            .try_into()
            .expect("validated integer range"),
    )
}

fn u32_at(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        source[offset..offset + 4]
            .try_into()
            .expect("validated integer range"),
    )
}

#[allow(clippy::too_many_arguments)]
fn error(
    profile: Profile,
    code: ErrorCode,
    classification: Classification,
    range: Range<usize>,
    command_ordinal: Option<usize>,
    field: &'static str,
    declared: Option<i64>,
    available: Option<u64>,
    limit: Option<u64>,
) -> ParseError {
    ParseError {
        classification,
        code,
        range,
        profile,
        command_ordinal,
        field,
        declared,
        available,
        limit,
    }
}
