use playsrc_audio::{
    dsp::Presets,
    room::{DEFAULT_TEMPLATES, Room},
    soundscape::*,
};
use sha2::{Digest, Sha256};

fn file(name: &str, expected: &str) -> Vec<u8> {
    let root = std::path::PathBuf::from(
        std::env::var_os("PLAYSRC_AUDIO_EVIDENCE").expect("explicit configured evidence directory"),
    );
    let bytes = std::fs::read(root.join(name)).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected);
    bytes
}
struct NoRandom;
impl Random for NoRandom {
    fn float(&mut self, low: f32, high: f32) -> f32 {
        assert_eq!((low, high), (0.0, 10.0));
        2.5
    }
    fn integer(&mut self, low: i32, high: i32) -> i32 {
        assert_eq!((low, high), (0, 5));
        5
    }
}

#[test]
#[ignore = "requires exact configured content bytes; no assets are distributed with tests"]
fn granary_actual_loops_random_mp3_positions_and_transition() {
    let bytes = file(
        "soundscapes_granary.txt",
        "13244e5e922df7afaff7993580c791ef4d815e74304cf717d3b3eaeaa18a73b8",
    );
    let mut registry = Registry::default();
    registry.append(
        &playsrc_keyvalues::parse_text(
            &bytes,
            playsrc_keyvalues::EscapeMode::LiteralBackslash,
            Default::default(),
        )
        .unwrap()
        .roots,
    );
    let outside = registry.find(b"Granary.Outside").unwrap();
    let inside = registry.find(b"Granary.Inside").unwrap();
    assert_eq!(registry.resources(&[outside, inside]).len(), 10);
    let mut state = Soundscape::default();
    let mut random = NoRandom;
    let mut actions = vec![];
    let mut selection = Selection {
        entity: 1,
        soundscape: outside as i32,
        positions: [[10.0; 3]; 8],
        position_bits: 15,
    };
    state.select(
        &registry,
        selection,
        Activation {
            time: 0.0,
            restoring: false,
            can_set_mixer: true,
        },
        &mut random,
        &mut actions,
    );
    assert_eq!(state.loops().len(), 4);
    assert_eq!(state.random_layers().len(), 2);
    assert!(actions.contains(&Action::RoomDsp(1)));
    actions.clear();
    state.update(
        0.015,
        3.75,
        Listener {
            origin: [0.0; 3],
            forward: [1.0, 0.0, 0.0],
            right: [0.0, -1.0, 0.0],
        },
        &mut random,
        &mut actions,
    );
    assert_eq!(actions.iter().filter(|action| matches!(action,Action::Start(voice) if voice.wave == b"ambient_mp3/bird3.mp3" && voice.position.is_some())).count(),2);
    selection.entity = 2;
    selection.soundscape = inside as i32;
    state.select(
        &registry,
        selection,
        Activation {
            time: 3.75,
            restoring: false,
            can_set_mixer: true,
        },
        &mut random,
        &mut actions,
    );
    assert!(state.random_layers().is_empty());
    assert_eq!(state.loops().len(), 5);
    assert_eq!(state.loops()[4].voice.wave, b"ambient/indoors.wav");
}

#[test]
#[ignore = "requires exact configured content bytes; no assets are distributed with tests"]
fn configured_automatic_preset_template_candidate() {
    let presets = Presets::parse(&file(
        "dsp_presets.txt",
        "5a59d25d656fdabc27ec2da2f0598aa970b962a8b4edd9ff9fe8f3cf407a3c23",
    ))
    .unwrap();
    assert_eq!(presets.0.len(), 136);
    let (_, preset) = presets
        .automatic(
            Room {
                outside: false,
                width: 256,
                length: 512,
                height: 128,
                diffusion: 0.2,
                reflectivity: 0.5,
                surfaces: [0.5; 6],
            },
            &DEFAULT_TEMPLATES,
        )
        .unwrap();
    assert_eq!(
        preset
            .processors
            .iter()
            .map(|processor| processor.kind)
            .collect::<Vec<_>>(),
        [10, 2]
    );
    assert_eq!(
        preset.processors[1].parameters.map(f32::to_bits),
        [
            0x42d33333, 0x4204cccd, 0x40d33334, 0x3f5b22d1, 0x3fe66667, 0x45192000, 0x3f800000,
            0x4099999a, 0x40000000, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}
