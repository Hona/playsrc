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
#[ignore = "requires exact configured DSP presets; no game assets are distributed"]
fn configured_water_response_matches_the_complete_ordered_draw_trace() {
    let presets = Presets::parse(&file(
        "dsp_presets.txt",
        "5a59d25d656fdabc27ec2da2f0598aa970b962a8b4edd9ff9fe8f3cf407a3c23",
    ))
    .unwrap();
    struct Draws(std::vec::IntoIter<[i32; 3]>);
    impl Random for Draws {
        fn float(&mut self, _: f32, _: f32) -> f32 {
            panic!("unexpected float draw")
        }
        fn integer(&mut self, low: i32, high: i32) -> i32 {
            let [a, b, value] = self.0.next().expect("missing draw");
            assert_eq!((a, b), (low, high));
            value
        }
    }
    let mut draws = Draws(
        vec![
            [2160, 2601, 2582],
            [2646, 3087, 3073],
            [2160, 2601, 2579],
            [2646, 3087, 2789],
            [2160, 2601, 2515],
            [2646, 3087, 3072],
            [2160, 2601, 2470],
            [2646, 3087, 2664],
            [2160, 2601, 2215],
        ]
        .into_iter(),
    );
    let mut processor = playsrc_audio::dsp::PresetProcessor::new(&presets.0[14]).unwrap();
    let mut signature = Sha256::new();
    for frame in 0..44100 {
        signature.update(
            processor
                .sample(if frame == 0 { 16384 } else { 0 }, &mut draws)
                .to_le_bytes(),
        );
    }
    assert!(draws.0.next().is_none());
    assert_eq!(
        format!("{:x}", signature.finalize()),
        "c640c03a7b51b7bbafa068f7c96f5fe06171f376af309f71485d19bf33a9f0f1"
    );
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

#[test]
#[ignore = "requires configured soundmixers.txt; no game assets are distributed"]
fn configured_mixer_selects_specific_rules_before_all() {
    let mut mixers = playsrc_audio::mixers::Mixers::parse(&file(
        "soundmixers.txt",
        "3e95dd0bc9182f99cb3fad543cd7df77371e4ce3ad7bfaa833eaadec8ea99e47",
    ))
    .unwrap();
    let ambient = mixers.membership(b"ambient/outdoors.wav", b"", 6, 0);
    let explosion = mixers.membership(b"weapons/explode.wav", b"", 0, 140);
    assert_eq!(mixers.gain(ambient), 0.72);
    assert_eq!(mixers.gain(explosion), 0.90);
    mixers.select(Some(b"Display_Mix"));
    assert_eq!(mixers.gain(ambient), 0.7);
    mixers.select(None);
    assert_eq!(mixers.gain(ambient), 0.72);
}
