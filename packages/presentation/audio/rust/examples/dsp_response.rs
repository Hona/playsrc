use playsrc_audio::dsp::{PresetProcessor, Presets, Processor, ProcessorSpec, SAMPLE_RATE};
use playsrc_audio::soundscape::Random;
use std::io::Write;

struct Journal(std::vec::IntoIter<[i32; 3]>);
impl Random for Journal {
    fn float(&mut self, _: f32, _: f32) -> f32 {
        panic!("unexpected float draw");
    }
    fn integer(&mut self, low: i32, high: i32) -> i32 {
        let [a, b, value] = self.0.next().expect("missing random draw");
        assert_eq!((a, b), (low, high));
        value
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let kind = arguments.next().expect("processor kind");
    let mut parameters = [0.0; 16];
    for (index, value) in arguments.enumerate() {
        parameters[index] = value.parse().unwrap();
    }
    let chain = kind == "preset" || kind == "auto";
    let mut processor = if !chain {
        Some(
            Processor::new(&ProcessorSpec {
                kind: kind.parse().unwrap(),
                parameters,
            })
            .unwrap(),
        )
    } else {
        None
    };
    let mut preset = if chain {
        let bytes = std::fs::read(std::env::var("DSP_PRESETS").expect("DSP_PRESETS path")).unwrap();
        let definitions = Presets::parse(&bytes).unwrap();
        let definition = if kind == "auto" {
            use playsrc_audio::room::{DEFAULT_TEMPLATES, Room};
            definitions
                .automatic(
                    Room {
                        outside: parameters[0] != 0.0,
                        width: parameters[1] as i32,
                        length: parameters[2] as i32,
                        height: parameters[3] as i32,
                        diffusion: parameters[4],
                        reflectivity: parameters[5],
                        surfaces: parameters[6..12].try_into().unwrap(),
                    },
                    &DEFAULT_TEMPLATES,
                )
                .unwrap()
                .1
        } else {
            definitions.0[parameters[0] as usize].clone()
        };
        if let Ok(path) = std::env::var("DSP_PRESET_OUTPUT") {
            let mut text = format!("{}\n", definition.configuration);
            for processor in &definition.processors {
                text.push_str(&format!("{}", processor.kind));
                for parameter in processor.parameters {
                    text.push_str(&format!(" {:08x}", parameter.to_bits()));
                }
                text.push('\n');
            }
            std::fs::write(path, text).unwrap();
        }
        Some(PresetProcessor::new(&definition).unwrap())
    } else {
        None
    };
    let journal = std::env::var("DSP_RANDOM_JOURNAL")
        .ok()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .unwrap_or_default();
    let mut random = Journal(
        journal
            .lines()
            .filter_map(|line| line.strip_prefix("random "))
            .map(|line| {
                let values = line
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect::<Vec<_>>();
                values.try_into().unwrap()
            })
            .collect::<Vec<_>>()
            .into_iter(),
    );
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for frame in 0..SAMPLE_RATE {
        let input = if frame == 0 { 16384 } else { 0 };
        let value = match &mut preset {
            Some(preset) => preset.sample(input, &mut random),
            None => processor.as_mut().unwrap().sample(input, &mut random),
        };
        writeln!(output, "{value}").unwrap();
    }
    assert!(random.0.next().is_none(), "unconsumed random draw");
}
