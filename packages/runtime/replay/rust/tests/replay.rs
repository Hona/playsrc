use std::{collections::BTreeMap, sync::Arc};

use playsrc_replay::{
    Coverage, EntityIdentity, InterpolationPolicy, Lifecycle, PlaybackRate, PlaybackState,
    RecordedEntity, RecordedEvent, RecordedField, RecordedOperation, RecordedRecord,
    ReplayErrorCode, ReplayEventKind, ReplayIdentity, ReplayLimits, ReplaySession, ReplaySource,
    SetupFamily,
};

fn limits() -> ReplayLimits {
    ReplayLimits {
        max_resident_bytes: 8 * 1024 * 1024,
        max_index_entries: 128,
        max_index_bytes: 128 * 1024,
        max_checkpoint_count: 128,
        max_checkpoint_bytes: 8 * 1024 * 1024,
        checkpoint_interval_ticks: 1,
        max_seek_records: 8,
        max_entities: 128,
        max_fields_per_entity: 128,
        max_events: 128,
        max_events_per_tick: 16,
        max_event_payload_bytes: 1_024,
        max_snapshot_bytes: 128 * 1024,
        max_diagnostics: 16,
        max_step_ticks: 8,
        max_rate_numerator: 16,
        max_rate_denominator: 16,
    }
}

fn identity() -> ReplayIdentity {
    ReplayIdentity {
        source_name: "fixture".into(),
        source_bytes: 1_000,
        source_sha256: [7; 32],
        profile: "tf2-demo3-net24".into(),
        game: "tf2".into(),
        codec: "protocol24".into(),
        decoder: "fixture-decoder".into(),
        index: "fixture-index".into(),
    }
}

fn field(value: u8) -> RecordedField {
    RecordedField {
        bytes: Arc::from([value]),
        interpolation: InterpolationPolicy::Linear,
    }
}

fn entity(value: u8) -> RecordedEntity {
    RecordedEntity {
        identity: EntityIdentity {
            index: 1,
            generation: 3,
        },
        class_id: 9,
        fields: BTreeMap::from([(4, field(value))]),
    }
}

fn record(
    ordinal: usize,
    command_tick: i32,
    server_tick: Option<i32>,
    operations: Vec<RecordedOperation>,
) -> RecordedRecord {
    RecordedRecord {
        ordinal,
        source_range: (ordinal as u64 * 10)..(ordinal as u64 * 10 + 10),
        command_tick: Some(command_tick),
        server_tick,
        no_interpolation: false,
        coverage: Coverage::Handled,
        operations,
    }
}

fn source() -> ReplaySource {
    let event = RecordedEvent {
        kind: ReplayEventKind::Game,
        selected_game_identity: 5,
        target: Some(entity(0).identity),
        action: 8,
        payload: Arc::from([1, 2, 3]),
        coverage: Coverage::Handled,
    };
    ReplaySource {
        identity: identity(),
        tick_interval_ns: 100,
        records: Arc::from([
            record(
                0,
                -1,
                None,
                vec![RecordedOperation::ReplaceSetup {
                    family: SetupFamily::DataTables,
                    version: 1,
                    bytes: Arc::from([1]),
                }],
            ),
            record(
                1,
                -1,
                None,
                vec![RecordedOperation::ReplaceSetup {
                    family: SetupFamily::StringTables,
                    version: 1,
                    bytes: Arc::from([2]),
                }],
            ),
            record(
                2,
                10,
                Some(10),
                vec![RecordedOperation::ReplaceSetup {
                    family: SetupFamily::Baselines,
                    version: 1,
                    bytes: Arc::from([3]),
                }],
            ),
            record(
                3,
                10,
                Some(10),
                vec![RecordedOperation::FullState {
                    entities: BTreeMap::from([(1, entity(10))]),
                    decoder_state: Arc::from([10]),
                }],
            ),
            record(
                4,
                11,
                Some(11),
                vec![
                    RecordedOperation::PatchEntity {
                        identity: entity(0).identity,
                        fields: BTreeMap::from([(4, field(11))]),
                    },
                    RecordedOperation::Event(event),
                    RecordedOperation::ReplaceDecoderState(Arc::from([11])),
                ],
            ),
            record(
                5,
                12,
                Some(12),
                vec![
                    RecordedOperation::PatchEntity {
                        identity: entity(0).identity,
                        fields: BTreeMap::from([(4, field(12))]),
                    },
                    RecordedOperation::ReplaceDecoderState(Arc::from([12])),
                ],
            ),
            record(6, 12, None, vec![RecordedOperation::Terminal]),
        ]),
    }
}

fn value(session: &ReplaySession) -> u8 {
    session.authoritative_state().entities[&1].fields[&4].bytes[0]
}

#[test]
fn lifecycle_clock_events_and_interpolation_are_recorded_only() {
    let mut replay = ReplaySession::open(source(), limits()).unwrap();
    assert_eq!(replay.lifecycle(), Lifecycle::Ready);
    assert_eq!(replay.playback_state(), PlaybackState::Playing);
    assert_eq!(value(&replay), 10);
    assert_eq!(replay.audit().simulation_calls, 0);

    let report = replay.advance(50).unwrap();
    assert!(report.event_range.is_empty());
    assert_eq!(value(&replay), 10);
    let presentation = replay.presentation_snapshot().unwrap();
    assert_eq!(presentation.fraction_numerator, 50);
    assert_eq!(presentation.fraction_denominator, 100);
    assert_eq!(presentation.next.entities[&1].fields[&4].bytes[0], 11);

    let report = replay.advance(50).unwrap();
    assert_eq!(report.event_range, 0..1);
    assert_eq!(value(&replay), 11);
    assert_eq!(replay.events()[0].event.payload.as_ref(), &[1, 2, 3]);
    assert_eq!(replay.audit().simulation_calls, 0);

    replay.pause();
    let cursor = replay.cursor();
    replay.advance(10_000).unwrap();
    assert_eq!(replay.cursor(), cursor);
    assert_eq!(replay.audit().simulation_calls, 0);
}

#[test]
fn seek_step_snapshot_restore_and_continuation_are_identical() {
    let mut uninterrupted = ReplaySession::open(source(), limits()).unwrap();
    uninterrupted.advance(200).unwrap();
    assert_eq!(uninterrupted.lifecycle(), Lifecycle::Ended);
    let expected = uninterrupted.authoritative_state().canonical_bytes();

    let mut replay = ReplaySession::open(source(), limits()).unwrap();
    replay.pause();
    let step = replay.step(1).unwrap();
    assert!(step.discontinuity);
    assert!(step.event_range.is_empty());
    assert_eq!(value(&replay), 11);
    replay.step(-1).unwrap();
    assert_eq!(value(&replay), 10);
    replay.step(1).unwrap();

    let snapshot = replay.snapshot().unwrap();
    let mut restored = ReplaySession::open(source(), limits()).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(
        replay.authoritative_state().canonical_bytes(),
        restored.authoritative_state().canonical_bytes()
    );
    replay.resume();
    restored.resume();
    replay.advance(100).unwrap();
    restored.advance(100).unwrap();
    assert_eq!(replay.authoritative_state().canonical_bytes(), expected);
    assert_eq!(restored.authoritative_state().canonical_bytes(), expected);
    assert_eq!(restored.audit().simulation_calls, 0);
}

#[test]
fn rational_rate_is_chunk_independent_and_preserves_remainder() {
    let mut one = ReplaySession::open(source(), limits()).unwrap();
    let mut split = ReplaySession::open(source(), limits()).unwrap();
    let rate = PlaybackRate {
        numerator: 3,
        denominator: 2,
    };
    one.set_rate(rate).unwrap();
    split.set_rate(rate).unwrap();
    one.advance(100).unwrap();
    split.advance(33).unwrap();
    split.advance(67).unwrap();
    assert_eq!(
        one.authoritative_state().canonical_bytes(),
        split.authoritative_state().canonical_bytes()
    );
    assert_eq!(
        one.presentation_snapshot().unwrap(),
        split.presentation_snapshot().unwrap()
    );
}

#[test]
fn equal_cursor_seek_histories_are_byte_identical() {
    let mut first = ReplaySession::open(source(), limits()).unwrap();
    let mut second = ReplaySession::open(source(), limits()).unwrap();
    first.advance(100).unwrap();
    let target = first.cursor();
    first.advance(100).unwrap();
    first.seek(target).unwrap();
    second.seek(target).unwrap();
    assert_eq!(
        first.authoritative_state().canonical_bytes(),
        second.authoritative_state().canonical_bytes()
    );
    assert_eq!(first.audit().simulation_calls, 0);
    assert_eq!(second.audit().simulation_calls, 0);
}

#[test]
fn restore_identity_and_bounds_fail_atomically() {
    let mut replay = ReplaySession::open(source(), limits()).unwrap();
    let before = replay.authoritative_state().canonical_bytes();
    let mut snapshot = replay.snapshot().unwrap();
    snapshot.identity.decoder = "wrong".into();
    let error = replay.restore(&snapshot).unwrap_err();
    assert_eq!(error.code, ReplayErrorCode::IdentityMismatch);
    assert_eq!(replay.authoritative_state().canonical_bytes(), before);

    replay.pause();
    let error = replay.step(9).unwrap_err();
    assert_eq!(error.code, ReplayErrorCode::StepLimit);
    assert_eq!(replay.authoritative_state().canonical_bytes(), before);
}
