use playsrc_audio::{dsp::*, room::*, soundscape::Random};
use sha2::{Digest, Sha256};

struct NoRandom;
impl Random for NoRandom {
    fn float(&mut self, _: f32, _: f32) -> f32 {
        panic!("unexpected random float")
    }
    fn integer(&mut self, _: i32, _: i32) -> i32 {
        panic!("unexpected random integer")
    }
}
fn spec(kind: i32, values: &[f32]) -> ProcessorSpec {
    let mut parameters = [0.0; 16];
    parameters[..values.len()].copy_from_slice(values);
    ProcessorSpec { kind, parameters }
}

#[test]
fn one_second_integer_response_vectors_and_quantum_partition_equivalence() {
    for (spec, signature) in [
        (
            spec(1, &[0.0, 100.0, 0.5, 1.0]),
            "bcdae63555b1ed41d5065f7fa6d3fb4a6764eab6c8be30a2be4942945fb1c78f",
        ),
        (
            spec(3, &[0.0, 4000.0, 0.0, 0.0, 1.0]),
            "8f406d264d6934d7d1d2e23ceb8fac99cc1f002448e8578ea0fbbb92191eab25",
        ),
        (
            spec(10, &[1.0, 3.0, 0.15]),
            "b0eb204d0d122fc1bc7d6fb58bb8ffd22a34e91dd61a87e3c19f84492d06c70b",
        ),
        (
            spec(2, &[80.0, 30.0, 4.0, 0.85, 1.1, 4000.0, 1.0]),
            "2b4fe2b4bb6f88a9909173bbe3b1ef6f7ea9138b09732d8642d8a74792d41952",
        ),
    ] {
        let mut processor = Processor::new(&spec).unwrap();
        let mut samples = vec![0; 44100];
        samples[0] = 16384;
        processor.process(&mut samples, &mut NoRandom);
        let mut hash = Sha256::new();
        for sample in &samples {
            hash.update(sample.to_le_bytes());
        }
        assert_eq!(format!("{:x}", hash.finalize()), signature);
        let mut partitioned = vec![0; 44100];
        partitioned[0] = 16384;
        let mut processor = Processor::new(&spec).unwrap();
        for block in partitioned.chunks_mut(128) {
            processor.process(block, &mut NoRandom);
        }
        assert_eq!(partitioned, samples);
    }
}

#[test]
fn classification_boundaries_shaft_and_outside_open_sides_are_not_a_generic_preset() {
    let mut room = Room {
        outside: false,
        width: 48,
        length: 192,
        height: 100,
        diffusion: 0.0,
        reflectivity: 0.5,
        surfaces: [0.5; 6],
    };
    assert_eq!(room.classify().shape, Shape::Room);
    room.length = 193;
    assert_eq!(room.classify().shape, Shape::Duct);
    room.width = 49;
    room.length = 123;
    assert_eq!(room.classify().shape, Shape::Hall);
    room.width = 96;
    room.length = 500;
    assert_eq!(room.classify().shape, Shape::Hall);
    room.width = 97;
    assert_eq!(room.classify().shape, Shape::Tunnel);
    room.width = 48;
    room.length = 100;
    room.height = 301;
    assert_eq!(room.classify().shape, Shape::Duct);
    room.outside = true;
    room.height = 100;
    room.width = 200;
    room.length = 500;
    assert_eq!(room.classify().shape, Shape::Courtyard);
    for (index, shape) in [
        Shape::OpenCourtyard,
        Shape::OpenStreet,
        Shape::Wall,
        Shape::OpenSpace,
    ]
    .into_iter()
    .enumerate()
    {
        room.surfaces[index] = 0.0;
        assert_eq!(room.classify().shape, shape);
    }
}

#[test]
fn unsupported_processors_and_malformed_graphs_fail_instead_of_dry_playback() {
    assert!(matches!(
        Processor::new(&spec(99, &[])),
        Err(Error::UnsupportedProcessor(99))
    ));
    assert!(Presets::parse(b"{ 2 LINEAR 0 1 0 0 80 .5 { RVA 80 } }").is_err());
    assert!(Processor::new(&spec(2, &[f32::NAN])).is_err());
    let preset = Preset {
        configuration: 6,
        mix: [0.2, 0.7],
        duration: 0.0,
        fade: 0.0,
        minimum_level: 80.0,
        quiet_mix_drop: 0.5,
        processors: vec![spec(0, &[])],
    };
    assert!(preset.validate_processing().is_err());
    assert!(PresetProcessor::new(&preset).is_err());
}
