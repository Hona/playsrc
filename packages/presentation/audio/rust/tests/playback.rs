use playsrc_audio::{
    dsp::{Preset, Presets},
    mixers::Mixers,
    playback::{Clip, Playback, Start},
    soundscape::{Listener, Random, Registry, Selection},
};

struct NoRandom;
impl Random for NoRandom {
    fn float(&mut self, _: f32, _: f32) -> f32 {
        panic!("unexpected random float")
    }
    fn integer(&mut self, _: i32, _: i32) -> i32 {
        panic!("unexpected random integer")
    }
}
fn setup(rate: u32, loop_frame: Option<u32>) -> Playback {
    let preset = Preset {
        configuration: 1,
        mix: [0.2, 0.7],
        duration: 0.0,
        fade: 0.0,
        minimum_level: 80.0,
        quiet_mix_drop: 0.5,
        processors: vec![],
    };
    let mixers =
        Mixers::parse(br#"GROUPRULES { All "" "" "" "" "" 50 0 0 100 40 } Default_Mix { All 1 }"#)
            .unwrap();
    let mut playback =
        Playback::new(Registry::default(), Presets(vec![preset; 15]), mixers).unwrap();
    playback
        .register(
            b"sound/test.wav".to_vec(),
            Clip {
                samples: vec![1000, 2000, 3000],
                rate,
                channels: 1,
                bits: 16,
                loop_frame,
            },
        )
        .unwrap();
    playback
}
fn start() -> Start {
    Start {
        identity: 17,
        wave: b"test.wav".to_vec(),
        volume: 1.0,
        pitch: 100,
        level: 0,
        origin: None,
        local: true,
        radius: 0.0,
        channel: 0,
        source_class: b"player".to_vec(),
        offset_seconds: 0.0,
        delay_seconds: 0.0,
        envelope: None,
        entity: None,
    }
}
fn frame(playback: &mut Playback) {
    playback
        .frame(
            playsrc_audio::playback::Frame {
                selection: Selection::default(),
                listener: Listener {
                    origin: [0.0; 3],
                    forward: [1.0, 0.0, 0.0],
                    right: [0.0, -1.0, 0.0],
                },
                elapsed: 0.015,
                game_time: 1.0,
                host_time: 1.0,
                master_volume: 1.0,
                can_set_mixer: true,
                entities: vec![],
            },
            &mut NoRandom,
        )
        .unwrap();
    playback.spatialize(&[], false).unwrap();
}

fn controls(master_volume: f32) -> playsrc_audio::playback::Frame {
    playsrc_audio::playback::Frame {
        selection: Selection::default(),
        listener: Listener {
            origin: [0.0; 3],
            forward: [1.0, 0.0, 0.0],
            right: [0.0, -1.0, 0.0],
        },
        elapsed: 0.015,
        game_time: 1.0,
        host_time: 1.0,
        master_volume,
        can_set_mixer: true,
        entities: vec![],
    }
}

fn paint_units(playback: &mut Playback) -> Vec<i32> {
    playback
        .paint(4, &mut NoRandom)
        .unwrap()
        .iter()
        .map(|value| (value * 32768.0) as i32)
        .collect()
}

#[test]
fn extra_paints_use_committed_controls_and_do_not_consume_unspatialized_voices() {
    let mut playback = setup(44100, Some(1));
    playback.start(start()).unwrap();
    frame(&mut playback);
    playback.paint(4, &mut NoRandom).unwrap();
    playback.frame(controls(0.0), &mut NoRandom).unwrap();
    assert_eq!(
        paint_units(&mut playback),
        [2976, 2976, 1984, 1984, 2976, 2976, 1984, 1984]
    );
    playback.spatialize(&[], false).unwrap();
    assert_eq!(paint_units(&mut playback), [0; 8]);

    let mut playback = setup(44100, Some(1));
    playback.start(start()).unwrap();
    frame(&mut playback);
    playback.paint(4, &mut NoRandom).unwrap();
    let mut additional = start();
    additional.identity = 18;
    playback.start(additional).unwrap();
    playback.frame(controls(1.0), &mut NoRandom).unwrap();
    playback.paint(4, &mut NoRandom).unwrap();
    playback.spatialize(&[], false).unwrap();
    assert_eq!(
        paint_units(&mut playback),
        [3968, 3968, 3968, 3968, 5952, 5952, 3968, 3968]
    );
}

#[test]
fn entity_sources_follow_current_origins_and_missing_entities_become_inaudible() {
    let mut playback = setup(44100, Some(0));
    let mut voice = start();
    voice.local = false;
    voice.level = 70;
    voice.origin = Some([-100.0, 0.0, 0.0]);
    voice.entity = Some((1, 2));
    playback.start(voice).unwrap();
    let mut input = controls(1.0);
    input.entities.push(playsrc_audio::playback::EntityOrigin {
        domain: 1,
        identity: 2,
        origin: [100.0, 0.0, 0.0],
    });
    playback.frame(input, &mut NoRandom).unwrap();
    assert_eq!(playback.obstruction_requests()[0].origin, [100.0, 0.0, 0.0]);
    playback.spatialize(&[(1, 1.0)], false).unwrap();
    assert!(
        playback
            .paint(4, &mut NoRandom)
            .unwrap()
            .iter()
            .any(|value| *value != 0.0)
    );
    playback.frame(controls(1.0), &mut NoRandom).unwrap();
    assert!(playback.obstruction_requests().is_empty());
    playback.spatialize(&[], false).unwrap();
    assert_eq!(paint_units(&mut playback), [0; 8]);
    assert_eq!(playback.active_count(), 1);
}

#[test]
fn cue_loops_preserve_pcm_position_and_stops_are_source_scoped() {
    let mut playback = setup(44100, Some(1));
    playback.start(start()).unwrap();
    frame(&mut playback);
    assert_eq!(
        paint_units(&mut playback),
        [992, 992, 1984, 1984, 2976, 2976, 1984, 1984]
    );
    assert_eq!(playback.active_external().collect::<Vec<_>>(), [17]);
    playback.stop(16);
    assert_eq!(playback.active_count(), 1);
    let mut invalid = start();
    invalid.wave = b"missing.wav".to_vec();
    assert!(playback.start(invalid).is_err());
    assert_eq!(playback.active_external().collect::<Vec<_>>(), [17]);
    playback.stop(17);
    assert_eq!(paint_units(&mut playback), [0; 8]);
}

#[test]
fn integer_upsampling_keeps_previous_bus_sample_across_paint_calls() {
    let mut playback = setup(22050, Some(0));
    playback.start(start()).unwrap();
    frame(&mut playback);
    assert_eq!(
        paint_units(&mut playback),
        [496, 496, 992, 992, 1488, 1488, 1984, 1984]
    );
    assert_eq!(
        paint_units(&mut playback),
        [2480, 2480, 2976, 2976, 1984, 1984, 992, 992]
    );
}

#[test]
fn end_of_wave_zero_padding_retires_at_the_next_paint_block() {
    let mut playback = setup(44100, None);
    playback.start(start()).unwrap();
    frame(&mut playback);
    assert_eq!(
        paint_units(&mut playback),
        [992, 992, 1984, 1984, 2976, 2976, 0, 0]
    );
    assert_eq!(playback.active_count(), 1);
    assert_eq!(paint_units(&mut playback), [0; 8]);
    assert_eq!(playback.active_count(), 0);
}

#[test]
fn device_gain_keeps_fractional_paint_units() {
    let mut playback = setup(44100, None);
    playback.start(start()).unwrap();
    playback.frame(controls(0.37), &mut NoRandom).unwrap();
    playback.spatialize(&[], false).unwrap();
    let pcm = playback.paint(4, &mut NoRandom).unwrap();
    assert_eq!(pcm[0].to_bits(), (992.0_f32 * 0.37 / 32768.0).to_bits());
    assert_ne!(pcm[0] * 32768.0, (pcm[0] * 32768.0).trunc());
}
