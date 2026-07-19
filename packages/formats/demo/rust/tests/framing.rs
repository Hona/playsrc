use playsrc_demo::{
    CommandBody, ErrorCode, Limits, Profile, SourceIdentity, TerminalTick, parse, parse_chunks,
};

fn limits(bytes: usize) -> Limits {
    Limits {
        max_input_bytes: bytes,
        max_command_records: 64,
        max_encoded_bytes_per_command: bytes,
        max_total_encoded_command_bytes: bytes,
        max_retained_diagnostic_bytes: 64,
        max_streaming_buffer_bytes: bytes,
    }
}

fn identity(bytes: usize) -> SourceIdentity {
    SourceIdentity {
        name: "fixture".into(),
        byte_length: bytes as u64,
        sha256: [0x5a; 32],
    }
}

fn header(signon_length: i32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 1_072];
    bytes[0..8].copy_from_slice(b"HL2DEMO\0");
    bytes[8..12].copy_from_slice(&3_i32.to_le_bytes());
    bytes[12..16].copy_from_slice(&24_i32.to_le_bytes());
    bytes[16..23].copy_from_slice(b"server\0");
    bytes[276..283].copy_from_slice(b"client\0");
    bytes[536..540].copy_from_slice(b"map\0");
    bytes[796..799].copy_from_slice(b"tf\0");
    bytes[1_056..1_060].copy_from_slice(&1.5_f32.to_bits().to_le_bytes());
    bytes[1_060..1_064].copy_from_slice(&17_i32.to_le_bytes());
    bytes[1_064..1_068].copy_from_slice(&11_i32.to_le_bytes());
    bytes[1_068..1_072].copy_from_slice(&signon_length.to_le_bytes());
    bytes
}

fn command_header(bytes: &mut Vec<u8>, command: u8, tick: i32) {
    bytes.push(command);
    bytes.extend_from_slice(&tick.to_le_bytes());
}

fn length_payload(bytes: &mut Vec<u8>, command: u8, tick: i32, payload: &[u8]) {
    command_header(bytes, command, tick);
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(payload);
}

fn complete_fixture(short_stop: bool) -> Vec<u8> {
    let mut first = Vec::new();
    command_header(&mut first, 1, -1);
    first.extend_from_slice(&5_i32.to_le_bytes());
    for value in 0_u32..18 {
        first.extend_from_slice(&(0x3f00_0000 + value).to_le_bytes());
    }
    first.extend_from_slice(&7_i32.to_le_bytes());
    first.extend_from_slice(&6_i32.to_le_bytes());
    first.extend_from_slice(&3_i32.to_le_bytes());
    first.extend_from_slice(&[0xaa, 0xbb, 0xcc]);

    let mut bytes = header(first.len() as i32);
    bytes.extend_from_slice(&first);
    command_header(&mut bytes, 3, 0);
    length_payload(&mut bytes, 4, 1, b"cmd\0");
    command_header(&mut bytes, 5, 2);
    bytes.extend_from_slice(&9_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    bytes.extend_from_slice(&[0x10, 0x20]);
    length_payload(&mut bytes, 6, 3, &[1, 2, 3, 4]);
    length_payload(&mut bytes, 8, 4, &[5, 6]);
    if short_stop {
        bytes.extend_from_slice(&[7, 0x34, 0x12, 0x00]);
    } else {
        command_header(&mut bytes, 7, 0x1234);
    }
    bytes
}

#[test]
fn lossless_all_protocol_three_commands_and_chunk_schedules() {
    let bytes = complete_fixture(false);
    let expected = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        limits(bytes.len()),
    )
    .unwrap();

    assert_eq!(expected.source_bytes(), bytes);
    assert_eq!(expected.header.playback_time_bits, 1.5_f32.to_bits());
    assert_eq!(expected.summary.signon_end, 1_072 + 96);
    assert_eq!(expected.summary.command_counts[1], 1);
    assert_eq!(expected.summary.command_counts[3], 1);
    assert_eq!(expected.summary.command_counts[4], 1);
    assert_eq!(expected.summary.command_counts[5], 1);
    assert_eq!(expected.summary.command_counts[6], 1);
    assert_eq!(expected.summary.command_counts[7], 1);
    assert_eq!(expected.summary.command_counts[8], 1);
    assert_eq!(expected.summary.total_encoded_payload_bytes, 15);

    let mut reconstructed = Vec::new();
    reconstructed.extend_from_slice(&bytes[..1_072]);
    for command in &expected.commands {
        reconstructed.extend_from_slice(expected.command_bytes(command));
    }
    assert_eq!(reconstructed, bytes);

    for chunk_size in 1..=bytes.len().min(97) {
        let actual = parse_chunks(
            bytes.chunks(chunk_size),
            identity(bytes.len()),
            Profile::Tf2Demo3Net24,
            limits(bytes.len()),
        )
        .unwrap();
        assert_eq!(actual, expected, "chunk size {chunk_size}");
    }
}

#[test]
fn accepts_lossless_source_tv_terminal_encoding_only_at_eof() {
    let bytes = complete_fixture(true);
    let demo = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        limits(bytes.len()),
    )
    .unwrap();
    let last = demo.commands.last().unwrap();
    assert_eq!(last.tick, None);
    assert_eq!(last.range.end - last.range.start, 4);
    assert_eq!(
        last.body,
        CommandBody::Stop {
            tick: TerminalTick::SourceTvStreamFlush {
                encoded_low_bytes: [0x34, 0x12, 0x00]
            }
        }
    );

    let mut trailing = bytes;
    trailing.extend_from_slice(&[0, 0]);
    let error = parse(
        &trailing,
        identity(trailing.len()),
        Profile::Tf2Demo3Net24,
        limits(trailing.len()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::TrailingBytes);
}

#[test]
fn malformed_lengths_and_bounds_are_atomic_and_exact() {
    let mut negative = header(0);
    command_header(&mut negative, 4, 0);
    negative.extend_from_slice(&(-1_i32).to_le_bytes());
    command_header(&mut negative, 7, 0);
    let error = parse(
        &negative,
        identity(negative.len()),
        Profile::Tf2Demo3Net24,
        limits(negative.len()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::NegativePayloadLength);
    assert_eq!(error.range, 1_077..1_081);

    let bytes = complete_fixture(false);
    let mut bounded = limits(bytes.len());
    bounded.max_encoded_bytes_per_command = 2;
    let error = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        bounded,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CommandPayloadLimit);
    assert_eq!(error.command_ordinal, Some(0));

    let mut bounded = limits(bytes.len());
    bounded.max_command_records = 1;
    let error = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        bounded,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CommandLimit);
}

#[test]
fn every_truncation_is_rejected_without_resynchronization() {
    let bytes = complete_fixture(false);
    for length in 0..bytes.len() - 1 {
        let truncated = &bytes[..length];
        let result = parse(
            truncated,
            identity(truncated.len()),
            Profile::Tf2Demo3Net24,
            limits(bytes.len()),
        );
        assert!(result.is_err(), "accepted truncation at {length}");
    }

    let source_tv = &bytes[..bytes.len() - 1];
    assert!(
        parse(
            source_tv,
            identity(source_tv.len()),
            Profile::Tf2Demo3Net24,
            limits(bytes.len()),
        )
        .is_ok()
    );
}

#[test]
fn profile_and_signon_boundary_mismatches_are_explicit() {
    let mut bytes = complete_fixture(false);
    bytes[12..16].copy_from_slice(&23_i32.to_le_bytes());
    let error = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        limits(bytes.len()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ProfileMismatch);

    let mut bytes = complete_fixture(false);
    bytes[1_068..1_072].copy_from_slice(&1_i32.to_le_bytes());
    let error = parse(
        &bytes,
        identity(bytes.len()),
        Profile::Tf2Demo3Net24,
        limits(bytes.len()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::SignonBoundary);
    assert_eq!(error.range, 1_073..1_074);
}
