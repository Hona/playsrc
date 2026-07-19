use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use playsrc_networking::{
    Class, CodecState, EntitySnapshot, EventValue, FLAG_UNSIGNED, FieldValue, FlatProperty, Limits,
    MessageBody, NetworkOperation, Property, PropertyKind, RecordedStateCodec, SchemaRegistry,
    SendTable, parse_demo_data_tables,
};

fn limits() -> Limits {
    Limits {
        max_payload_bits: 8 * 1024 * 1024,
        max_messages_per_payload: 1_024,
        max_string_bytes: 1_024,
        max_data_tables: 1_024,
        max_properties_per_table: 1_024,
        max_flat_properties: 4_096,
        max_classes: 512,
        max_string_tables: 32,
        max_entries_per_string_table: 65_536,
        max_string_user_data_bytes: 16_384,
        max_event_schemas: 512,
        max_fields_per_event: 128,
        max_entities: 2_048,
        max_fields_per_entity: 4_096,
        max_snapshot_history: 64,
        max_decompressed_bytes: 16 * 1024 * 1024,
    }
}

#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    len: usize,
}

impl Bits {
    fn bit(&mut self, value: bool) {
        self.unsigned(u32::from(value), 1);
    }

    fn unsigned(&mut self, value: u32, width: usize) {
        for bit in 0..width {
            if self.len.is_multiple_of(8) {
                self.bytes.push(0);
            }
            self.bytes[self.len / 8] |= (((value >> bit) & 1) as u8) << (self.len % 8);
            self.len += 1;
        }
    }

    fn i16(&mut self, value: i16) {
        self.unsigned(value as u16 as u32, 16);
    }

    fn i32(&mut self, value: i32) {
        self.unsigned(value as u32, 32);
    }

    fn string(&mut self, value: &[u8]) {
        for byte in value {
            self.unsigned(u32::from(*byte), 8);
        }
        self.unsigned(0, 8);
    }

    fn ubit_var(&mut self, value: u32) {
        if value < 16 {
            self.unsigned(value << 2, 6);
        } else if value < 256 {
            self.unsigned((value << 2) | 1, 10);
        } else if value < 4_096 {
            self.unsigned((value << 2) | 2, 14);
        } else {
            self.unsigned((value << 2) | 3, 34);
        }
    }

    fn payload(&mut self, payload: &Bits) {
        for bit in 0..payload.len {
            let value = (payload.bytes[bit / 8] >> (bit % 8)) & 1;
            self.bit(value != 0);
        }
    }
}

fn integer_property(name: &[u8]) -> Property {
    Property {
        name: name.to_vec(),
        kind: PropertyKind::Integer,
        flags: FLAG_UNSIGNED,
        table_name: None,
        array_elements: None,
        low_value_bits: Some(0),
        high_value_bits: Some(0),
        bit_count: Some(8),
    }
}

fn schema() -> SchemaRegistry {
    let table = SendTable {
        name: b"DT_Test".to_vec(),
        needs_decoder: true,
        properties: vec![integer_property(b"value")],
    };
    let mut schema = SchemaRegistry::default();
    schema
        .replace(
            vec![table],
            vec![Class {
                id: 0,
                name: b"CTest".to_vec(),
                table_name: b"DT_Test".to_vec(),
            }],
            limits(),
        )
        .unwrap();
    schema
}

fn baseline(value: u8) -> BTreeMap<u16, FieldValue> {
    BTreeMap::from([(0, FieldValue::UnsignedInteger(u32::from(value)))])
}

fn entity_payload(value: Option<u8>, delta: bool) -> Bits {
    let mut bits = Bits::default();
    bits.ubit_var(0);
    bits.bit(false);
    bits.bit(!delta);
    if !delta {
        bits.unsigned(0, 1);
        bits.unsigned(3, 10);
    }
    if let Some(value) = value {
        bits.bit(true);
        bits.ubit_var(0);
        bits.unsigned(u32::from(value), 8);
    }
    bits.bit(false);
    if delta {
        bits.bit(false);
    }
    bits
}

fn state_packet(tick: i32, delta_from: Option<i32>, value: Option<u8>) -> Bits {
    let mut packet = Bits::default();
    packet.unsigned(3, 6);
    packet.i32(tick);
    packet.unsigned(1_500, 16);
    packet.unsigned(10, 16);

    let payload = entity_payload(value, delta_from.is_some());
    packet.unsigned(26, 6);
    packet.unsigned(2_047, 11);
    packet.bit(delta_from.is_some());
    if let Some(base) = delta_from {
        packet.i32(base);
    }
    packet.bit(false);
    packet.unsigned(1, 11);
    packet.unsigned(payload.len as u32, 20);
    packet.bit(false);
    packet.payload(&payload);
    packet
}

fn codec_with_schema() -> RecordedStateCodec {
    let mut codec = RecordedStateCodec::new(limits());
    let state = CodecState {
        schema: Arc::new(schema()),
        class_baselines: Arc::new(BTreeMap::from([(0, baseline(5))])),
        ..CodecState::default()
    };
    codec.restore(state).unwrap();
    codec
}

#[test]
fn demo_data_tables_and_flattened_schema_are_exact() {
    let mut bits = Bits::default();
    bits.bit(true);
    bits.bit(true);
    bits.string(b"DT_Test");
    bits.unsigned(1, 10);
    bits.unsigned(0, 5);
    bits.string(b"value");
    bits.unsigned(u32::from(FLAG_UNSIGNED), 16);
    bits.unsigned(0, 32);
    bits.unsigned(0, 32);
    bits.unsigned(8, 7);
    bits.bit(false);
    bits.i16(1);
    bits.i16(0);
    bits.string(b"CTest");
    bits.string(b"DT_Test");

    let parsed = parse_demo_data_tables(&bits.bytes, bits.len, limits()).unwrap();
    assert_eq!(parsed, schema());
    assert_eq!(parsed.class_id_bits, 1);
    assert_eq!(parsed.flat_properties(0).unwrap().len(), 1);
}

#[test]
fn full_delta_and_full_state_converge_byte_for_byte() {
    let mut delta_codec = codec_with_schema();
    let full = state_packet(100, None, None);
    delta_codec.decode_packet(&full.bytes, full.len).unwrap();
    assert_eq!(
        delta_codec.state().snapshots[&100].entities[&0].fields,
        baseline(5)
    );

    let delta = state_packet(101, Some(100), Some(9));
    let decoded = delta_codec.decode_packet(&delta.bytes, delta.len).unwrap();
    assert!(decoded.operations.iter().any(|operation| matches!(
        operation,
        NetworkOperation::EntitySnapshot {
            delta_from: Some(100),
            ..
        }
    )));

    let mut full_codec = codec_with_schema();
    let full = state_packet(101, None, Some(9));
    full_codec.decode_packet(&full.bytes, full.len).unwrap();
    assert_eq!(
        delta_codec.state().snapshots[&101],
        full_codec.state().snapshots[&101]
    );
}

#[test]
fn missing_delta_base_fails_without_partial_state() {
    let mut codec = codec_with_schema();
    let before = codec.snapshot();
    let delta = state_packet(101, Some(100), Some(9));
    assert!(codec.decode_packet(&delta.bytes, delta.len).is_err());
    assert_eq!(codec.state(), &before);
}

#[test]
fn event_schema_event_and_user_message_preserve_order_and_bits() {
    let mut schema_payload = Bits::default();
    schema_payload.unsigned(5, 9);
    schema_payload.string(b"player_hurt");
    schema_payload.unsigned(3, 3);
    schema_payload.string(b"userid");
    schema_payload.unsigned(0, 3);

    let mut event_payload = Bits::default();
    event_payload.unsigned(5, 9);
    event_payload.i32(42);

    let mut packet = Bits::default();
    packet.unsigned(30, 6);
    packet.unsigned(1, 9);
    packet.unsigned(schema_payload.len as u32, 20);
    packet.payload(&schema_payload);
    packet.unsigned(25, 6);
    packet.unsigned(event_payload.len as u32, 11);
    packet.payload(&event_payload);
    packet.unsigned(23, 6);
    packet.unsigned(7, 8);
    packet.unsigned(3, 11);
    packet.unsigned(0b101, 3);

    let mut codec = RecordedStateCodec::new(limits());
    let decoded = codec.decode_packet(&packet.bytes, packet.len).unwrap();
    assert_eq!(decoded.messages.len(), 3);
    assert!(matches!(
        decoded.messages[0].body,
        MessageBody::GameEventList { .. }
    ));
    assert!(matches!(
        decoded.messages[1].body,
        MessageBody::GameEvent(_)
    ));
    assert!(matches!(
        decoded.messages[2].body,
        MessageBody::UserMessage(_)
    ));
    assert!(matches!(
        &decoded.operations[1],
        NetworkOperation::Event(event)
            if event.fields == vec![(b"userid".to_vec(), EventValue::Integer32(42))]
    ));
    assert!(matches!(
        &decoded.operations[2],
        NetworkOperation::UserMessage(message)
            if message.message_type == 7
                && message.payload.bit_length == 3
                && message.payload.bytes == vec![0b101]
    ));
}

#[test]
fn schema_flattening_moves_changes_often_without_losing_identity() {
    let mut changed = integer_property(b"changed");
    changed.flags |= playsrc_networking::FLAG_CHANGES_OFTEN;
    let normal = integer_property(b"normal");
    let mut registry = SchemaRegistry::default();
    registry
        .replace(
            vec![SendTable {
                name: b"DT_Order".to_vec(),
                needs_decoder: true,
                properties: vec![normal.clone(), changed.clone()],
            }],
            vec![Class {
                id: 0,
                name: b"COrder".to_vec(),
                table_name: b"DT_Order".to_vec(),
            }],
            limits(),
        )
        .unwrap();
    let names: Vec<_> = registry
        .flat_properties(0)
        .unwrap()
        .iter()
        .map(|property: &FlatProperty| property.property.name.clone())
        .collect();
    assert_eq!(names, vec![b"changed".to_vec(), b"normal".to_vec()]);
}

#[test]
fn snapshots_are_canonical_ordered_maps() {
    let snapshot = EntitySnapshot {
        server_tick: 1,
        entities: BTreeMap::new(),
    };
    let keys: BTreeSet<_> = snapshot.entities.keys().copied().collect();
    assert!(keys.is_empty());
}
