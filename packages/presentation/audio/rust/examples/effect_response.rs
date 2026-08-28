//! Offline stereo effect/transition fixture; input PCM and draws are supplied.
use playsrc_audio::{
    dsp::Presets,
    output::{self, MonoEffect},
    soundscape::Random,
};
use std::io::Write;

struct Journal(std::vec::IntoIter<[i32; 3]>);
impl Random for Journal {
    fn float(&mut self, _: f32, _: f32) -> f32 {
        panic!("unexpected float draw")
    }
    fn integer(&mut self, low: i32, high: i32) -> i32 {
        let [a, b, value] = self.0.next().expect("missing draw");
        assert_eq!((a, b), (low, high));
        value
    }
}
fn input(name: &str, maximum: u64) -> Vec<u8> {
    let path = std::env::var(name).expect(name);
    assert!(std::fs::metadata(&path).unwrap().len() <= maximum);
    std::fs::read(path).unwrap()
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert!((3..=4).contains(&args.len()));
    let mixed = args
        .get(3)
        .is_some_and(|value| matches!(value.as_str(), "mix" | "mix-auto" | "mix-exp"));
    assert!(args.len() == 3 || mixed);
    let initial: i32 = args[0].parse().unwrap();
    let fade: f32 = args[1].parse().unwrap();
    let quantum: usize = args[2].parse().unwrap();
    assert!((1..=1020).contains(&quantum));
    let mut definitions = Presets::parse(&input("DSP_PRESETS", 1024 * 1024)).unwrap();
    if args.get(3).is_some_and(|value| value == "mix-exp") {
        definitions.0[14].fade = -0.2;
    }
    if args.get(3).is_some_and(|value| value == "mix-auto") {
        let rooms = input("DSP_ROOMS", 96);
        assert_eq!(rooms.len(), 96);
        for (index, bytes) in rooms.chunks_exact(48).enumerate() {
            let integer =
                |offset| i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let float = |offset| f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let room = playsrc_audio::room::Room {
                outside: integer(0) != 0,
                width: integer(4),
                length: integer(8),
                height: integer(12),
                diffusion: float(16),
                reflectivity: float(20),
                surfaces: std::array::from_fn(|i| float(24 + i * 4)),
            };
            definitions.0[60 + index] = definitions
                .automatic(room, &playsrc_audio::room::DEFAULT_TEMPLATES)
                .unwrap()
                .1;
        }
    }
    let bytes = input("DSP_INPUT_PCM", 441000 * 8);
    assert!(bytes.len().is_multiple_of(8));
    let mut samples = bytes
        .chunks_exact(8)
        .map(|bytes| {
            [
                i32::from_le_bytes(bytes[..4].try_into().unwrap()),
                i32::from_le_bytes(bytes[4..].try_into().unwrap()),
            ]
        })
        .collect::<Vec<_>>();
    let events = input("DSP_TRANSITIONS", 128 * 8);
    assert!(events.len().is_multiple_of(8));
    let events = events
        .chunks_exact(8)
        .map(|bytes| {
            (
                u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
                i32::from_le_bytes(bytes[4..].try_into().unwrap()),
            )
        })
        .collect::<Vec<_>>();
    assert!(events.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(events.iter().all(|(frame, preset)| *frame < samples.len()
        && *preset >= 0
        && (*preset as usize) < definitions.0.len()));
    let journal = input("DSP_RANDOM_JOURNAL", 8 * 1024 * 1024);
    let mut random = Journal(
        String::from_utf8(journal)
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("random "))
            .map(|line| {
                let values = line
                    .split_whitespace()
                    .map(|value| value.parse().unwrap())
                    .collect::<Vec<i32>>();
                values.try_into().unwrap()
            })
            .collect::<Vec<_>>()
            .into_iter(),
    );
    let mut effect = MonoEffect::new(
        initial,
        definitions.0[initial as usize].clone(),
        fade / 1000.0,
    )
    .unwrap();
    let mut water = MonoEffect::new(14, definitions.0[14].clone(), 0.1).unwrap();
    let mut frame = 0;
    let mut event = 0;
    while frame < samples.len() {
        if event < events.len() && events[event].0 == frame {
            effect
                .select(
                    events[event].1,
                    definitions.0[events[event].1 as usize].clone(),
                )
                .unwrap();
            event += 1;
        }
        let end = (frame + quantum).min(events.get(event).map_or(samples.len(), |event| event.0));
        assert!(end > frame);
        if mixed {
            let count = end - frame;
            let mut buses = [
                vec![[0_i32; 2]; count],
                vec![[0_i32; 2]; count],
                vec![[0_i32; 2]; count],
            ];
            for (voice, (volume, send)) in [
                ([254.0_f32, 254.0], 0.33),
                ([178.125, 72.75], 0.71),
                ([94.25, 230.875], 0.0),
            ]
            .into_iter()
            .enumerate()
            {
                let (wet, facing) = output::split_volume(volume, send, 1.0, effect.is_off());
                for (bus, volumes) in if voice == 2 {
                    vec![(2, volume)]
                } else {
                    vec![(0, wet), (1, facing)]
                } {
                    for (offset, sample) in samples[frame..end].iter().enumerate() {
                        for channel in 0..2 {
                            buses[bus][offset][channel] = buses[bus][offset][channel].wrapping_add(
                                output::sample16(sample[voice % 2] as i16, volumes[channel]),
                            );
                        }
                    }
                }
            }
            effect.process(&mut buses[0], &mut random);
            let [wet, facing, _] = &mut buses;
            for (target, source) in facing.iter_mut().zip(wet.iter()) {
                for (target, source) in target.iter_mut().zip(source) {
                    *target = target.wrapping_add(*source);
                }
            }
            if (8820..22048).contains(&frame) {
                water.process(&mut buses[1], &mut random);
            }
            for offset in 0..count {
                for channel in 0..2 {
                    samples[frame + offset][channel] =
                        buses[1][offset][channel].wrapping_add(buses[2][offset][channel]);
                }
            }
        } else {
            effect.process(&mut samples[frame..end], &mut random);
        }
        frame = end;
    }
    assert!(random.0.next().is_none(), "unconsumed draw");
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for sample in samples.into_iter().flatten() {
        output.write_all(&sample.to_le_bytes()).unwrap();
    }
}
