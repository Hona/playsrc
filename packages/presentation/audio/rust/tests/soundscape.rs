use playsrc_audio::soundscape::*;
use playsrc_keyvalues::{EscapeMode, Limits, parse_text};

#[derive(Default)]
struct Draws(Vec<(f32, f32)>);
impl Random for Draws {
    fn float(&mut self, low: f32, high: f32) -> f32 {
        self.0.push((low, high));
        low + (high - low) * 0.25
    }
    fn integer(&mut self, low: i32, high: i32) -> i32 {
        self.0.push((low as f32, high as f32));
        high
    }
}
fn registry(script: &str) -> Registry {
    let mut registry = Registry::default();
    registry.append(
        &parse_text(
            script.as_bytes(),
            EscapeMode::LiteralBackslash,
            Limits::default(),
        )
        .unwrap()
        .roots,
    );
    registry
}
fn selection(index: i32, entity: i32) -> Selection {
    Selection {
        entity,
        soundscape: index,
        position_bits: 5,
        positions: [[1.0, 2.0, 3.0]; 8],
    }
}
fn listener() -> Listener {
    Listener {
        origin: [0.0; 3],
        forward: [1.0, 0.0, 0.0],
        right: [0.0, -1.0, 0.0],
    }
}
fn activation(time: f32, restoring: bool) -> Activation {
    Activation {
        time,
        restoring,
        can_set_mixer: true,
    }
}

#[test]
fn interval_tokenization_reversed_ranges_and_scalar_draw_suppression() {
    assert_eq!(
        Interval::read(b",,5,,2,8"),
        Interval {
            start: 5.0,
            range: -3.0
        }
    );
    assert_eq!(
        Interval::read(b"0.1,0.3").range.to_bits(),
        0.2_f32.to_bits()
    );
    let content = registry(
        r#"zone { playlooping { volume 1 pitch 100 wave a.wav }
        playlooping { volume "1,0" pitch "80,120" wave b.wav } }"#,
    );
    let mut state = Soundscape::default();
    let mut rng = Draws::default();
    let mut actions = vec![];
    state.select(
        &content,
        selection(0, 1),
        activation(0.0, false),
        &mut rng,
        &mut actions,
    );
    assert_eq!(rng.0, [(0.0, -1.0), (0.0, 40.0)]);
    assert_eq!(state.loops()[1].target, 0.75);
    assert_eq!(state.loops()[1].voice.pitch, 90);
}

#[test]
fn manifest_order_case_sensitive_map_append_and_last_definition_wins() {
    let manifest = parse_text(br#"soundscapes_manifest { file "a.txt" bogus "ignore.txt" file "scripts/soundscapes_Z.txt" }"#,
        EscapeMode::LiteralBackslash, Limits::default()).unwrap();
    assert_eq!(
        document_paths(&manifest.roots, "z"),
        [
            b"a.txt".to_vec(),
            b"scripts/soundscapes_Z.txt".to_vec(),
            b"scripts/soundscapes_z.txt".to_vec()
        ]
    );
    assert_eq!(document_paths(&manifest.roots, "Z").len(), 2);
    let content = registry("zone { dsp 1 } ZONE { dsp 2 } empty { }");
    assert_eq!(content.find(b"Zone"), Some(1));
    assert_eq!(content.len(), 2);
}

#[test]
fn loops_fade_at_absolute_rate_reuse_and_ignore_same_selection_position_update() {
    let content = registry(
        r#"a { dsp 1 playlooping { volume .45 pitch 100 position 0 wave factory.wav }
        playlooping { volume .75 pitch 100 wave outside.wav } }
        b { playlooping { volume 1 pitch 100 wave outside.wav } }"#,
    );
    let mut state = Soundscape::default();
    let mut rng = Draws::default();
    let mut actions = vec![];
    state.select(
        &content,
        selection(0, 1),
        activation(0.0, false),
        &mut rng,
        &mut actions,
    );
    assert_eq!(state.loops()[0].voice.volume, 0.05);
    assert_eq!(state.loops()[1].voice.volume, 0.0);
    state.update(0.3, 0.3, listener(), &mut rng, &mut actions);
    assert_eq!(state.loops()[0].voice.volume, 0.15);
    assert_eq!(state.loops()[1].voice.volume, 0.1);
    actions.clear();
    let mut changed = selection(0, 1);
    changed.positions[0] = [9.0; 3];
    state.select(
        &content,
        changed,
        activation(0.3, false),
        &mut rng,
        &mut actions,
    );
    assert!(actions.is_empty());
    assert_eq!(state.selection().positions[0], [1.0, 2.0, 3.0]);
    state.select(
        &content,
        selection(1, 2),
        activation(0.3, false),
        &mut rng,
        &mut actions,
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Start(_)))
    );
    state.update(0.6, 0.9, listener(), &mut rng, &mut actions);
    assert_eq!(state.loops().len(), 1);
    assert_eq!(state.loops()[0].voice.wave, b"outside.wav");
    assert!(actions.contains(&Action::Stop {
        wave: b"factory.wav".to_vec(),
        ambient: false
    }));
}

#[test]
fn positioned_move_stops_old_wave_and_immediately_changes_volume() {
    let content = registry("a { playlooping { volume 1 position 0 wave a.wav } }");
    let mut state = Soundscape::default();
    let mut rng = Draws::default();
    let mut actions = vec![];
    state.select(
        &content,
        selection(0, 1),
        activation(0.0, false),
        &mut rng,
        &mut actions,
    );
    let mut moved = selection(0, 2);
    moved.positions[0] = [2.0; 3];
    actions.clear();
    state.select(
        &content,
        moved,
        activation(0.0, false),
        &mut rng,
        &mut actions,
    );
    assert!(matches!(
        &actions[..2],
        [Action::Stop { ambient: false, .. }, Action::Volume(_)]
    ));
    assert_eq!(state.loops()[0].voice.volume, 0.05);
}

#[test]
fn random_first_half_delay_reverse_order_due_equality_no_catchup() {
    let content = registry(
        r#"a {
        playrandom { time "5,15" volume 1 pitch 100 position 0 attenuation 1 rndwave { wave a.mp3 wave b.mp3 } }
        playrandom { time "5,15" volume 1 pitch 100 position 2 attenuation 1 rndwave { wave c.mp3 wave d.mp3 } }
    }"#,
    );
    let mut state = Soundscape::default();
    let mut rng = Draws::default();
    let mut actions = vec![];
    state.select(
        &content,
        selection(0, 1),
        activation(0.0, false),
        &mut rng,
        &mut actions,
    );
    assert_eq!(rng.0, [(0.0, 10.0), (0.0, 10.0)]);
    assert_eq!(state.random_layers()[0].next_time, 3.75);
    actions.clear();
    state.update(0.0, 3.74, listener(), &mut rng, &mut actions);
    assert!(actions.is_empty());
    state.update(0.0, 3.75, listener(), &mut rng, &mut actions);
    let waves: Vec<_> = actions
        .iter()
        .filter_map(|action| match action {
            Action::Start(voice) => Some(voice.wave.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(waves, [b"d.mp3", b"b.mp3"]);
    assert_eq!(state.random_layers()[0].next_time, 11.25);
    actions.clear();
    state.update(0.0, 100.0, listener(), &mut rng, &mut actions);
    assert_eq!(actions.len(), 2);
    assert_eq!(state.random_layers()[0].next_time, 107.5);
}

#[test]
fn nested_overrides_restore_suppression_resource_closure_and_depth() {
    let content = registry(
        r#"parent { dsp 1 playsoundscape { name child volume .5 positionoverride 2 } }
        child { dsp 9 soundmixer bad dsp_volume 0
          playlooping { volume 1 wave "*#AMBIENT/A.wav" }
          playlooping { volume 1 position 0 wave b.wav suppress_on_restore 1 }
          playrandom { time 1 position 7 rndwave { wave c.mp3 } }
          playsoundscape { name child }
        }"#,
    );
    let mut state = Soundscape::default();
    let mut rng = Draws::default();
    let mut actions = vec![];
    state.select(
        &content,
        selection(0, 1),
        activation(0.0, true),
        &mut rng,
        &mut actions,
    );
    assert_eq!(state.loops().len(), 8);
    assert!(
        state
            .loops()
            .iter()
            .all(|layer| layer.target == 0.5 && layer.voice.position == Some([1.0, 2.0, 3.0]))
    );
    assert_eq!(state.random_layers().len(), 8);
    assert!(actions.contains(&Action::RoomDsp(1)));
    assert!(!actions.contains(&Action::RoomDsp(9)));
    assert!(actions.contains(&Action::Mixer(None)));
    assert!(actions.contains(&Action::DspVolume(None)));
    assert_eq!(
        content.resources(&[0]).into_iter().collect::<Vec<_>>(),
        [
            b"sound/ambient/a.wav".to_vec(),
            b"sound/b.wav".to_vec(),
            b"sound/c.mp3".to_vec()
        ]
    );
    actions.clear();
    state.reset(&mut actions);
    assert_eq!(actions.len(), 8);
    assert!(state.loops().is_empty());
    assert!(state.random_layers().is_empty());
}
