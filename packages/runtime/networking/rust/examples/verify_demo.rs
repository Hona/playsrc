use std::{env, fs, process::ExitCode};

use playsrc_demo::{CommandBody, Limits as DemoLimits, Profile, SourceIdentity, parse};
use playsrc_networking::{Limits, NetworkOperation, RecordedStateCodec};

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, String> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("missing internal demo path")?;
    if args.next().is_some() {
        return Err("unexpected verifier argument".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("cannot read capture: {error}"))?;
    let demo = parse(
        &bytes,
        SourceIdentity {
            name: "controlled".into(),
            byte_length: bytes.len() as u64,
            sha256: [0; 32],
        },
        Profile::Tf2Demo3Net24,
        DemoLimits {
            max_input_bytes: 64 * 1024 * 1024,
            max_command_records: 2_000_000,
            max_encoded_bytes_per_command: 16 * 1024 * 1024,
            max_total_encoded_command_bytes: 64 * 1024 * 1024,
            max_retained_diagnostic_bytes: 256,
            max_streaming_buffer_bytes: 64 * 1024 * 1024,
        },
    )
    .map_err(|error| error.to_string())?;
    let mut codec = RecordedStateCodec::new(Limits {
        max_payload_bits: 16 * 1024 * 1024 * 8,
        max_messages_per_payload: 65_536,
        max_string_bytes: 4_096,
        max_data_tables: 1_024,
        max_properties_per_table: 1_024,
        max_flat_properties: 4_096,
        max_classes: 512,
        max_string_tables: 32,
        max_entries_per_string_table: 65_536,
        max_string_user_data_bytes: 16_384,
        max_event_schemas: 512,
        max_fields_per_event: 512,
        max_entities: 2_048,
        max_fields_per_entity: 4_096,
        max_snapshot_history: 128,
        max_decompressed_bytes: 64 * 1024 * 1024,
    });
    let mut packets = 0_usize;
    let mut messages = 0_usize;
    let mut snapshots = 0_usize;
    let mut events = 0_usize;
    let mut user_messages = 0_usize;
    for command in &demo.commands {
        match &command.body {
            CommandBody::LengthDelimited(record)
                if record.kind == playsrc_demo::LengthDelimitedKind::DataTables =>
            {
                let payload = demo.payload_bytes(command).expect("data-table payload");
                codec
                    .replace_demo_data_tables(payload, payload.len() * 8)
                    .map_err(|error| format!("record {} data tables: {error}", command.ordinal))?;
            }
            CommandBody::LengthDelimited(record)
                if record.kind == playsrc_demo::LengthDelimitedKind::StringTables =>
            {
                let payload = demo.payload_bytes(command).expect("string-table payload");
                codec
                    .replace_demo_string_tables(payload, payload.len() * 8)
                    .map_err(|error| {
                        format!("record {} string tables: {error}", command.ordinal)
                    })?;
            }
            CommandBody::Packet(_) => {
                let payload = demo.payload_bytes(command).expect("packet payload");
                let packet = codec
                    .decode_packet(payload, payload.len() * 8)
                    .map_err(|error| format!("record {} packet: {error}", command.ordinal))?;
                packets += 1;
                messages += packet.messages.len();
                for operation in packet.operations {
                    match operation {
                        NetworkOperation::EntitySnapshot { .. } => snapshots += 1,
                        NetworkOperation::Event(_) => events += 1,
                        NetworkOperation::UserMessage(_) => user_messages += 1,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let entities = codec
        .state()
        .snapshots
        .last_key_value()
        .map_or(0, |(_, snapshot)| snapshot.entities.len());
    Ok(format!(
        "{{\"packets\":{packets},\"messages\":{messages},\"snapshots\":{snapshots},\"events\":{events},\"userMessages\":{user_messages},\"tables\":{},\"classes\":{},\"stringTables\":{},\"entities\":{entities}}}",
        codec.state().schema.tables.len(),
        codec.state().schema.classes.len(),
        codec.state().string_tables.len(),
    ))
}
