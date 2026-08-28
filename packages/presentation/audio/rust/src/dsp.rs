//! Sample-domain DSP. Processing uses the Source 44.1 kHz integer mix domain;
//! browser sample-rate conversion belongs outside this processor.
use crate::{ramp::Ramp, soundscape::Random};
use std::fmt;

pub const SAMPLE_RATE: u32 = 44_100;
const ONE: i32 = 4096;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Malformed(&'static str),
    UnsupportedProcessor(i32),
    UnsupportedConfiguration(i32),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DSP {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessorSpec {
    pub kind: i32,
    pub parameters: [f32; 16],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub configuration: i32,
    pub mix: [f32; 2],
    pub duration: f32,
    pub fade: f32,
    pub minimum_level: f32,
    pub quiet_mix_drop: f32,
    pub processors: Vec<ProcessorSpec>,
}

impl Preset {
    pub fn validate_processing(&self) -> Result<(), Error> {
        let required = match self.configuration {
            0 => 1,
            1 => 0,
            5 => 2,
            6 | 10 => 4,
            7 | 8 => 5,
            9 => 3,
            kind => return Err(Error::UnsupportedConfiguration(kind)),
        };
        if self.processors.len() < required || self.processors.len() > 5 {
            return Err(Error::Malformed("processor count for configuration"));
        }
        for processor in &self.processors {
            if ![0, 1, 2, 3, 10, 11].contains(&processor.kind) {
                return Err(Error::UnsupportedProcessor(processor.kind));
            }
            if processor.parameters.iter().any(|value| !value.is_finite()) {
                return Err(Error::Malformed("nonfinite parameter"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Presets(pub Vec<Preset>);

fn symbol(token: &str) -> Result<f32, Error> {
    let value = match token {
        "NULL" | "LP" | "LO" | "PLAIN" | "SIN" | "LIN" | "SIMPLE" => 0.0,
        "DLY" | "HP" | "MED" | "ALLPASS" | "TRI" | "EXP" | "LINEAR" => 1.0,
        "RVA" | "BP" | "HI" | "LOWPASS" | "SQR" => 2.0,
        "FLT" | "VHI" | "DLINEAR" | "SAW" => 3.0,
        "CRS" | "FLINEAR" | "RND" => 4.0,
        "PTC" | "LOWPASS_4TAP" | "LOG_IN" | "PARALLEL2" => 5.0,
        "ENV" | "PLAIN_4TAP" | "LOG_OUT" | "PARALLEL4" => 6.0,
        "LFO" | "LIN_IN" | "PARALLEL5" => 7.0,
        "EFO" | "LIN_OUT" | "FEEDBACK" => 8.0,
        "MDY" | "FEEDBACK3" => 9.0,
        "DFR" | "FEEDBACK4" => 10.0,
        "AMP" | "MOD" => 11.0,
        "MOD2" => 12.0,
        "MOD3" => 13.0,
        _ => token
            .parse()
            .map_err(|_| Error::Malformed("unknown preset token"))?,
    };
    if f32::is_finite(value) {
        Ok(value)
    } else {
        Err(Error::Malformed("nonfinite parameter"))
    }
}

impl Presets {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let input = std::str::from_utf8(bytes).map_err(|_| Error::Malformed("preset encoding"))?;
        let mut cleaned = String::with_capacity(input.len());
        for line in input.lines() {
            for c in line.split("//").next().unwrap_or_default().chars() {
                if c == '{' || c == '}' {
                    cleaned.push(' ');
                    cleaned.push(c);
                    cleaned.push(' ');
                } else {
                    cleaned.push(c);
                }
            }
            cleaned.push(' ');
        }
        let mut tokens = cleaned.split_ascii_whitespace();
        let mut presets = std::collections::BTreeMap::new();
        while let Some(token) = tokens.next() {
            if token != "{" {
                continue;
            }
            let mut next = || symbol(tokens.next().ok_or(Error::Malformed("truncated preset"))?);
            let index = next()? as i32;
            if !(0..256).contains(&index) {
                return Err(Error::Malformed("preset index"));
            }
            let mut preset = Preset {
                configuration: next()? as i32,
                mix: [next()?, next()?],
                duration: next()?,
                fade: next()?,
                minimum_level: next()?,
                quiet_mix_drop: next()?,
                processors: Vec::new(),
            };
            loop {
                match tokens.next() {
                    Some("}") => break,
                    Some("{") => {
                        let kind = symbol(tokens.next().ok_or(Error::Malformed("processor type"))?)?
                            as i32;
                        let mut parameters = [0.0; 16];
                        let mut count = 0;
                        loop {
                            let token = tokens
                                .next()
                                .ok_or(Error::Malformed("truncated processor"))?;
                            if token == "}" {
                                break;
                            }
                            if count == 16 {
                                return Err(Error::Malformed("too many parameters"));
                            }
                            parameters[count] = symbol(token)?;
                            count += 1;
                        }
                        if preset.processors.len() == 5 {
                            return Err(Error::Malformed("too many processors"));
                        }
                        preset.processors.push(ProcessorSpec { kind, parameters });
                    }
                    _ => return Err(Error::Malformed("preset processor block")),
                }
            }
            presets.insert(index, preset);
        }
        if presets.is_empty() || presets.keys().copied().ne(0..presets.len() as i32) {
            return Err(Error::Malformed("preset sequence has holes"));
        }
        Ok(Self(presets.into_values().collect()))
    }
}

fn mul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b) >> 12
}
fn samples(milliseconds: f32) -> usize {
    (milliseconds * (SAMPLE_RATE as f32 / 1000.0)) as usize
}
fn parameter(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value == 0.0 {
        0.0
    } else {
        value.clamp(minimum, maximum)
    }
}

#[derive(Clone, Debug)]
struct Section {
    a: i32,
    b: [i32; 2],
    previous: i32,
}
impl Section {
    fn new(cutoff: f32, highpass: bool, gain: f32) -> Self {
        let omega = (std::f64::consts::PI * f64::from(cutoff) / f64::from(SAMPLE_RATE)).tan();
        let pole = (1.0 - omega) / (1.0 + omega);
        let numerator =
            (((if highpass { 1.0 + pole } else { 1.0 - pole }) / 2.0) * f64::from(ONE)) as i32;
        let b = (numerator as f32 * gain) as i32;
        Self {
            a: (-pole * f64::from(ONE)) as i32,
            b: [b, if highpass { -b } else { b }],
            previous: 0,
        }
    }
    fn next(&mut self, input: i32) -> i32 {
        let value = input.wrapping_sub(mul(self.a, self.previous));
        let output = self.b[1]
            .wrapping_mul(self.previous)
            .wrapping_add(self.b[0].wrapping_mul(value))
            >> 12;
        self.previous = value;
        output
    }
}

#[derive(Clone, Debug)]
struct Filter(Vec<Section>);
impl Filter {
    fn new(p: &[f32; 16]) -> Self {
        let cutoff = parameter(p[1], 10.0, 22050.0);
        let width = p[2].clamp(0.0, 11025.0);
        let bandpass = width > 0.0;
        let count = (p[3] as usize).min(3).max(usize::from(bandpass)) + 1;
        let gain = p[4].clamp(0.0, 10.0);
        Self(
            (0..count)
                .map(|index| {
                    let highpass = if bandpass {
                        index == 0 || index == 3
                    } else {
                        p[0] as i32 == 1
                    };
                    Section::new(
                        if bandpass && !highpass {
                            cutoff + width
                        } else {
                            cutoff
                        },
                        highpass,
                        if index == 0 { gain } else { 1.0 },
                    )
                })
                .collect(),
        )
    }
    fn next(&mut self, mut value: i32) -> i32 {
        for section in &mut self.0 {
            value = section.next(value);
        }
        value
    }
}

#[derive(Clone, Debug)]
struct Delay {
    data: Vec<i32>,
    cursor: usize,
    taps: [usize; 4],
    kind: i32,
    feedback: i32,
    gain: i32,
    filter: Option<Section>,
    modulation: Option<Modulation>,
}
impl Delay {
    fn new(
        length: usize,
        kind: i32,
        feedback: i32,
        gain: i32,
        mut filter: Option<Section>,
    ) -> Self {
        let mut gain = if kind == 5 || kind == 6 {
            (gain as f32 * 0.25) as i32
        } else {
            gain
        };
        if let Some(section) = &mut filter {
            for b in &mut section.b {
                *b = (*b as f32 * (feedback as f32 / ONE as f32)) as i32;
            }
        }
        let mut feedback_clamped = feedback.min(ONE - 1);
        if kind == 3 || kind == 4 {
            feedback_clamped = 0;
            gain = ONE;
        } else {
            let fb = (feedback as f32 / ONE as f32).min(0.999);
            let amplification = (1.0 / (1.0 - f64::from(fb))) as f32;
            let correction = ((1.0 / f64::from(amplification) * f64::from(ONE)) as i32 as f32
                * 4.0)
                .clamp(ONE as f32 * 0.25, ONE as f32);
            gain = ((if gain == 0 { ONE } else { gain }) as f32 / ONE as f32 * correction) as i32;
        }
        Self {
            data: vec![0; length + 1],
            cursor: 0,
            taps: [length; 4],
            kind,
            feedback: feedback_clamped,
            gain,
            filter,
            modulation: None,
        }
    }
    fn from_parameters(p: &[f32; 16]) -> Self {
        let kind = (p[0] as i32).clamp(0, 6);
        let length = samples(p[1].clamp(-1.0, 1000.0).abs());
        let feedback = (p[2].clamp(0.0, 0.99) * ONE as f32) as i32;
        let gain = (p[3].clamp(0.0, 10.0) * ONE as f32) as i32;
        let filter = if [2, 4, 5].contains(&kind) {
            // Delay uses only the first section of its designed filter.
            let width = parameter(p[6], 100.0, 11025.0);
            Some(Section::new(
                parameter(p[5], 10.0, 22050.0),
                width > 0.0 || p[4] as i32 == 1,
                1.0,
            ))
        } else {
            None
        };
        let mut result = Self::new(length, kind, feedback, gain, filter);
        if kind == 5 || kind == 6 {
            result.taps = [
                length,
                samples(p[8].clamp(-1.0, 1000.0).abs()),
                samples(p[9].clamp(-1.0, 1000.0).abs()),
                samples(p[10].clamp(-1.0, 1000.0).abs()),
            ];
            result.taps.sort_unstable();
            for tap in &mut result.taps {
                *tap = (*tap).min(length);
            }
        }
        result
    }
    fn read(&self, tap: usize) -> i32 {
        self.data[(self.cursor + tap) % self.data.len()]
    }
    fn next(&mut self, input: i32, random: &mut impl Random) -> i32 {
        let mut delayed = self.read(if self.kind >= 5 {
            self.taps[3]
        } else {
            self.taps[0]
        });
        if let Some(modulation) = &self.modulation
            && modulation.changing
        {
            delayed = delayed.wrapping_add(mul(
                self.read(modulation.target).wrapping_sub(delayed),
                modulation.blend,
            ));
        }
        let returned;
        let stored;
        match self.kind {
            3 => {
                returned = delayed;
                stored = input;
            }
            4 => {
                returned = self.filter.as_mut().unwrap().next(delayed);
                stored = input;
            }
            1 => {
                stored = input.wrapping_add(mul(self.feedback, delayed));
                returned = mul(delayed.wrapping_add(mul(-self.feedback, stored)), self.gain);
            }
            2 | 5 => {
                let filtered = self.filter.as_mut().unwrap().next(delayed);
                stored = input.wrapping_add(filtered);
                let value = if self.kind == 5 {
                    input
                        .wrapping_add(self.read(self.taps[0]))
                        .wrapping_add(self.read(self.taps[1]))
                        .wrapping_add(self.read(self.taps[2]))
                        .wrapping_add(delayed)
                } else {
                    stored
                };
                returned = mul(value, self.gain);
            }
            _ => {
                stored = input.wrapping_add(mul(delayed, self.feedback));
                let value = if self.kind == 6 {
                    input
                        .wrapping_add(self.read(self.taps[0]))
                        .wrapping_add(self.read(self.taps[1]))
                        .wrapping_add(self.read(self.taps[2]))
                        .wrapping_add(delayed)
                } else {
                    stored
                };
                returned = mul(value, self.gain);
            }
        }
        self.data[self.cursor] = stored;
        self.cursor = if self.cursor == 0 {
            self.data.len() - 1
        } else {
            self.cursor - 1
        };
        if let Some(modulation) = &mut self.modulation {
            if modulation.changing {
                let previous = modulation.position;
                modulation.accumulator = modulation.accumulator.wrapping_add(modulation.step);
                modulation.position += modulation.accumulator >> 20;
                modulation.accumulator &= (1 << 20) - 1;
                if modulation.step == 0 || modulation.position >= ONE {
                    modulation.blend = previous;
                    self.taps[0] = modulation.target;
                    modulation.changing = false;
                } else {
                    modulation.blend = modulation.position;
                }
            }
            if modulation.period != 0 {
                let due = modulation.remaining == 0;
                modulation.remaining -= 1;
                if due {
                    modulation.remaining = modulation.period;
                    let maximum = self.data.len() as i32 - 1;
                    let minimum = (f64::from(maximum as f32) * (1.0 - f64::from(modulation.depth)))
                        as f32 as i32;
                    modulation.target = random.integer(minimum, maximum).min(maximum) as usize;
                    modulation.changing = true;
                    modulation.blend = 0;
                    modulation.position = 0;
                    modulation.accumulator = 0;
                }
            }
            if modulation.mix == ONE {
                returned
            } else if modulation.mix == ONE / 2 {
                returned.wrapping_add(input) >> 1
            } else {
                input.wrapping_add(mul(returned.wrapping_sub(input), modulation.mix))
            }
        } else {
            returned
        }
    }
}

#[derive(Clone, Debug)]
struct Modulation {
    period: i32,
    remaining: i32,
    depth: f32,
    mix: i32,
    changing: bool,
    target: usize,
    blend: i32,
    step: i32,
    accumulator: i32,
    position: i32,
}
impl Modulation {
    fn new(ramp_seconds: f32, period_seconds: f32, depth: f32, mix: f32) -> Self {
        let period = (period_seconds * SAMPLE_RATE as f32) as i32;
        let run = (ramp_seconds * SAMPLE_RATE as f32) as i32;
        let step = ONE as f32 / run as f32;
        let step = if step as i32 > 4095 {
            (step - step as i32 as f32) + 4095.0
        } else {
            step
        };
        Self {
            period,
            remaining: period,
            depth,
            mix: (mix * ONE as f32) as i32,
            changing: false,
            target: 0,
            blend: 0,
            step: (step * (1 << 20) as f32) as i32,
            accumulator: 0,
            position: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct Amplifier {
    gain: i32,
    maximum: i32,
    threshold: i32,
    distortion: i32,
    period: i32,
    remaining: i32,
    depth: i32,
    random: bool,
    glide: f32,
    ramp: Option<Ramp>,
}
impl Amplifier {
    fn new(parameters: &[f32; 16]) -> Self {
        let rate = parameters[4].clamp(0.0, 200.0);
        let seconds = if rate > 0.0 {
            (1.0 / f64::from(rate).max(0.01)) as f32
        } else {
            0.0
        };
        let period = (seconds * SAMPLE_RATE as f32) as i32;
        let gain = (parameters[0].clamp(0.0, 1000.0) * ONE as f32) as i32;
        Self {
            gain,
            maximum: gain,
            threshold: (f64::from(parameters[1].clamp(0.0, 1.0)) * 32767.0) as i32,
            distortion: (parameters[2].clamp(0.0, 1.0) * ONE as f32) as i32,
            period,
            remaining: period,
            depth: if rate > 0.0 {
                (parameters[5].clamp(0.0, 1.0) * ONE as f32) as i32
            } else {
                0
            },
            random: parameters[7].clamp(0.0, 1.0) > 0.0,
            glide: if rate > 0.0 {
                (f64::from(parameter(parameters[6], 0.01, 100.0)) / 1000.0) as f32
            } else {
                0.0
            },
            ramp: None,
        }
    }
    fn sample(&mut self, input: i32, random: &mut impl Random) -> i32 {
        let value = if self.threshold < ONE && self.distortion != 0 {
            let clipped = input.clamp(-self.threshold, self.threshold);
            if self.distortion < ONE {
                input.wrapping_add(mul(clipped.wrapping_sub(input), self.distortion))
            } else {
                clipped
            }
        } else {
            input
        };
        let output = mul(value, self.gain);
        if let Some(ramp) = &mut self.ramp {
            self.gain = ramp.next();
            if ramp.finished() {
                self.ramp = None;
            }
        }
        if self.period != 0 {
            let due = self.remaining == 0;
            self.remaining -= 1;
            if due {
                self.remaining = self.period;
                let minimum = self.maximum.wrapping_sub(mul(self.maximum, self.depth));
                let target = if self.random {
                    random.integer(minimum.min(self.maximum), minimum.max(self.maximum))
                } else if self.gain == minimum {
                    self.maximum
                } else {
                    minimum
                };
                self.ramp = Some(Ramp::new(self.glide, self.gain, target));
            }
        }
        output
    }
}

#[derive(Clone, Debug)]
enum Kernel {
    Identity,
    Filter(Filter),
    Delay(Delay),
    Cascade(Vec<Delay>),
    Amplifier(Amplifier),
    Reverb {
        delays: Vec<Delay>,
        output_filter: Option<Filter>,
    },
}

/// A processor can only be constructed when its complete sample path exists.
/// Unsupported processing never becomes an identity/dry substitute.
#[derive(Clone, Debug)]
pub struct Processor(Kernel);

#[derive(Clone, Copy, Debug)]
enum Operation {
    Sum(usize, usize, usize),
    Run(usize, usize, usize),
}

#[derive(Clone, Debug)]
pub struct PresetProcessor {
    processors: Vec<Processor>,
    operations: Vec<Operation>,
    registers: [i32; 8],
    output: usize,
    linear: bool,
}

impl PresetProcessor {
    pub fn new(preset: &Preset) -> Result<Self, Error> {
        use Operation::{Run, Sum};
        let (operations, output, required) = match preset.configuration {
            0 => (vec![Run(1, 0, 0)], 1, 1),
            1 => (vec![], 0, 0),
            5 => (vec![Sum(3, 1, 2), Run(1, 0, 0), Run(2, 1, 0)], 3, 2),
            6 => (
                vec![
                    Sum(5, 2, 4),
                    Run(2, 1, 1),
                    Run(4, 3, 3),
                    Run(1, 0, 0),
                    Run(3, 2, 0),
                ],
                5,
                4,
            ),
            7 => (
                vec![
                    Sum(5, 2, 4),
                    Run(2, 1, 1),
                    Run(4, 3, 3),
                    Run(1, 0, 0),
                    Run(3, 2, 0),
                    Run(6, 4, 5),
                ],
                6,
                5,
            ),
            8 => (
                vec![
                    Sum(2, 1, 6),
                    Run(6, 4, 5),
                    Run(5, 3, 4),
                    Run(4, 2, 3),
                    Run(3, 1, 2),
                    Run(1, 0, 0),
                ],
                4,
                5,
            ),
            9 => (
                vec![Sum(1, 0, 4), Run(4, 2, 3), Run(3, 1, 2), Run(2, 0, 1)],
                2,
                3,
            ),
            10 => (
                vec![
                    Sum(1, 0, 4),
                    Run(5, 3, 2),
                    Run(4, 2, 3),
                    Run(3, 1, 2),
                    Run(2, 0, 1),
                ],
                2,
                4,
            ),
            kind => return Err(Error::UnsupportedConfiguration(kind)),
        };
        if preset.processors.len() < required || preset.processors.len() > 5 {
            return Err(Error::Malformed("processor count for configuration"));
        }
        let mut processors = preset
            .processors
            .iter()
            .map(Processor::new)
            .collect::<Result<Vec<_>, _>>()?;
        if processors.is_empty() {
            processors.push(Processor(Kernel::Identity));
        }
        Ok(Self {
            processors,
            operations,
            registers: [0; 8],
            output,
            linear: preset.configuration == 1,
        })
    }

    pub fn sample(&mut self, value: i32, random: &mut impl Random) -> i32 {
        if self.linear {
            return self
                .processors
                .iter_mut()
                .fold(value, |value, processor| processor.sample(value, random));
        }
        self.registers[0] = value;
        for operation in &self.operations {
            match *operation {
                Operation::Sum(out, a, b) => {
                    self.registers[out] = self.registers[a].wrapping_add(self.registers[b])
                }
                Operation::Run(out, processor, input) => {
                    self.registers[out] =
                        self.processors[processor].sample(self.registers[input], random)
                }
            }
        }
        self.registers[self.output]
    }

    pub fn process(&mut self, samples: &mut [i32], random: &mut impl Random) {
        if self.linear {
            for processor in &mut self.processors {
                processor.process(samples, random);
            }
        } else {
            for value in samples {
                *value = self.sample(*value, random);
            }
        }
    }
}

impl Processor {
    pub fn new(spec: &ProcessorSpec) -> Result<Self, Error> {
        let p = &spec.parameters;
        if p.iter().any(|value| !value.is_finite()) {
            return Err(Error::Malformed("nonfinite parameter"));
        }
        Ok(Self(match spec.kind {
            0 => Kernel::Identity,
            1 => Kernel::Delay(Delay::from_parameters(p)),
            2 => reverb(p)?,
            3 => Kernel::Filter(Filter::new(p)),
            11 => Kernel::Amplifier(Amplifier::new(p)),
            10 => {
                let delays = [13, 19, 26, 21, 32, 36, 38, 16];
                let count = (p[1].clamp(0.0, 4.0) as usize).clamp(1, 8);
                let feedback = (f64::from(p[2].clamp(0.0, 1.0) * ONE as f32))
                    .min(0.999 * f64::from(ONE)) as i32;
                let gain = if p[3] == 0.0 {
                    1.0
                } else {
                    p[3].clamp(0.0, 10.0)
                };
                Kernel::Cascade(
                    delays[..count]
                        .iter()
                        .map(|delay| {
                            let milliseconds = (*delay as f32 * p[0].clamp(0.0, 1.0)) as usize;
                            Delay::new(
                                milliseconds * SAMPLE_RATE as usize / 1000,
                                1,
                                feedback,
                                (gain * ONE as f32) as i32,
                                None,
                            )
                        })
                        .collect(),
                )
            }
            kind => return Err(Error::UnsupportedProcessor(kind)),
        }))
    }
    pub fn sample(&mut self, value: i32, random: &mut impl Random) -> i32 {
        match &mut self.0 {
            Kernel::Identity => value,
            Kernel::Filter(filter) => filter.next(value),
            Kernel::Delay(delay) => delay.next(value, random),
            Kernel::Cascade(delays) => delays
                .iter_mut()
                .fold(value, |value, delay| delay.next(value, random)),
            Kernel::Amplifier(amplifier) => amplifier.sample(value, random),
            Kernel::Reverb {
                delays,
                output_filter,
            } => {
                let sum = delays.iter_mut().fold(0_i32, |sum, delay| {
                    sum.wrapping_add(delay.next(value, random))
                });
                output_filter
                    .as_mut()
                    .map_or(sum, |filter| filter.next(sum))
            }
        }
    }
    pub fn process(&mut self, samples: &mut [i32], random: &mut impl Random) {
        for value in samples {
            *value = self.sample(*value, random);
        }
    }
}

fn reverb(p: &[f32; 16]) -> Result<Kernel, Error> {
    let maximum = p[0].clamp(0.0, 1000.0);
    let minimum = p[1].clamp(0.0, 1000.0);
    let feedback = p[3].clamp(0.0, 1.0);
    let gain = p[4].clamp(0.0, 10.0);
    let mut count = p[2].clamp(1.0, 12.0) as usize;
    let mut dimensions: [(f32, f32); 3] = std::array::from_fn(|index| {
        (
            p[9 + index].abs().min(1000.0),
            p[12 + index].clamp(-1.0, 1.0),
        )
    });
    let explicit = dimensions.iter().any(|(size, _)| *size as i32 != 0);
    let mut parameters = Vec::with_capacity(12);
    if explicit {
        count = count.div_ceil(3) * 3;
        dimensions.sort_by(|a, b| b.0.total_cmp(&a.0));
        if dimensions
            .iter()
            .all(|(_, reflectivity)| *reflectivity == 0.0)
        {
            let smallest = dimensions[2].0 as i32;
            for (size, reflectivity) in &mut dimensions {
                *reflectivity = feedback.powf(*size as i32 as f32 / smallest as f32);
            }
            dimensions[2].1 = feedback;
        }
        let separation = (count / 3 * 7) as f32;
        let first_gap = dimensions[1].0 - dimensions[2].0;
        if first_gap <= separation {
            dimensions[1].0 += separation - first_gap;
        }
        if dimensions[0].0 - dimensions[1].0 <= separation {
            dimensions[0].0 += separation - first_gap;
        }
        for index in 0..count {
            let (dimension, reflectivity) = dimensions[index % 3];
            let predelay = reflectivity < 0.0 && index < 3;
            let mut milliseconds = if reflectivity < 0.0 && !predelay {
                ((f64::from(dimension) / 4.0) as i32).max(7)
            } else {
                dimension as i32
            };
            if index >= 3 {
                milliseconds += ((index / 3 * 7) as f64)
                    .max((index / 3) as f64 * (f64::from(milliseconds) * 0.18))
                    as f32 as i32;
            }
            let coefficient =
                (4096.0_f64 * f64::from(reflectivity.abs())).min(0.999 * 4096.0) as i32;
            parameters.push((
                milliseconds as usize * SAMPLE_RATE as usize / 1000,
                coefficient,
                predelay,
            ));
        }
    } else {
        for index in 0..count {
            let length = samples(
                minimum
                    + (((maximum - minimum) * (1.0 / count as f32)) * index as f32) as i32 as f32,
            );
            let coefficient = if index == 0 {
                feedback
            } else {
                feedback.powf(length as f32 / parameters[0].0 as f32)
            };
            parameters.push((
                length,
                (4096.0_f64 * f64::from(coefficient)).min(0.999 * 4096.0) as i32,
                false,
            ));
        }
    }
    let filter = Section::new(parameter(p[5], 10.0, 22050.0), false, 1.0);
    let parallel = p[6].clamp(0.0, 1.0) as i32 != 0;
    let tap_seed = p[15].clamp(0.0, 0.333);
    #[allow(clippy::approx_constant)] // A tap-spacing coefficient, not mathematical pi.
    let tap_offsets = [3.141592, 1.697043, 0.96325];
    let tap_minimums = [
        5 * SAMPLE_RATE as usize / 1000,
        7 * SAMPLE_RATE as usize / 1000,
        10 * SAMPLE_RATE as usize / 1000,
    ];
    let delays = parameters
        .into_iter()
        .map(|(length, feedback, predelay)| {
            let kind = if parallel && p[5] != 0.0 {
                if predelay {
                    4
                } else if tap_seed > 0.0 {
                    5
                } else {
                    2
                }
            } else if tap_seed > 0.0 {
                6
            } else {
                0
            };
            let mut delay = Delay::new(
                length,
                kind,
                feedback,
                (gain * ONE as f32 * (1.0 / count as f32)) as i32,
                parallel.then(|| filter.clone()),
            );
            if kind == 5 || kind == 6 {
                delay.taps = [0, 0, 0, length];
                for index in 0..3 {
                    delay.taps[index] = ((length as f64
                        * (1.0 - f64::from(tap_seed) * tap_offsets[index]))
                        .max(tap_minimums[index] as f64)
                        as f32 as usize)
                        .min(length);
                }
                delay.taps.sort_unstable();
            }
            if p[7] > 0.0 {
                let seconds = length as f32 * (1.0 / SAMPLE_RATE as f32);
                let depth = ((p[7].min(50.0) * 0.001) / seconds).clamp(0.01, 0.99);
                let period = seconds * p[8].clamp(0.0, 10.0);
                delay.modulation = Some(Modulation::new(
                    (20.0_f32 / 1000.0).min(period / 2.0),
                    period,
                    depth,
                    1.0,
                ));
            }
            delay
        })
        .collect();
    Ok(Kernel::Reverb {
        delays,
        output_filter: (!parallel && p[5] != 0.0).then(|| Filter(vec![filter.clone(), filter])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_room_quantized_delay_and_filter_coefficients() {
        let parameters = [
            0x42d33333, 0x4204cccd, 0x40d33334, 0x3f5b22d1, 0x3fe66667, 0x45192000, 0x3f800000,
            0x4099999a, 0x40000000, 0, 0, 0, 0, 0, 0, 0,
        ]
        .map(f32::from_bits);
        let Kernel::Reverb { delays, .. } = reverb(&parameters).unwrap() else {
            panic!()
        };
        let actual = delays
            .iter()
            .map(|delay| {
                (
                    delay.data.len() - 1,
                    delay.feedback,
                    delay.gain,
                    delay.filter.as_ref().unwrap().a,
                    delay.filter.as_ref().unwrap().b,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                (1464, 3506, 707, -2868, [524, 524]),
                (1993, 3314, 937, -2868, [495, 495]),
                (2522, 3133, 1153, -2868, [468, 468]),
                (3051, 2962, 1228, -2868, [443, 443]),
                (3580, 2800, 1228, -2868, [419, 419]),
                (4110, 2647, 1228, -2868, [396, 396])
            ]
        );
        assert_eq!(
            delays
                .iter()
                .map(|delay| delay.modulation.as_ref().unwrap().period)
                .collect::<Vec<_>>(),
            [2928, 3986, 5044, 6101, 7159, 8220]
        );
        assert_eq!(
            delays
                .iter()
                .map(|delay| delay.modulation.as_ref().unwrap().depth.to_bits())
                .collect::<Vec<_>>(),
            [
                0x3e140f72, 0x3dd9858b, 0x3dabe545, 0x3d8e1762, 0x3d7230b9, 0x3d52f57e
            ]
        );
    }
}
