//! Integer stereo buses and persistent mono effect transitions.
use crate::{
    dsp::{Error, Preset, PresetProcessor},
    ramp::Ramp,
    soundscape::Random,
};

pub type Frame = [i32; 2];

#[derive(Clone, Debug)]
struct Fade {
    ramp: Ramp,
    squared: bool,
}

impl Fade {
    fn new(seconds: f32, squared: bool) -> Self {
        Self {
            ramp: Ramp::new(seconds, 0, 4096),
            squared,
        }
    }

    fn advance(&mut self) -> i32 {
        self.ramp.next()
    }
}

#[derive(Clone, Debug)]
struct State {
    identity: i32,
    definition: Preset,
    processor: PresetProcessor,
}

/// Preset zero preserves stereo rather than averaging it. Each call is one
/// paint block: a transition finishing within it retires on the next call.
#[derive(Clone, Debug)]
pub struct MonoEffect {
    current: State,
    previous: Option<State>,
    fade: Fade,
    default_fade: f32,
    scratch: Vec<i32>,
}

impl MonoEffect {
    pub fn new(identity: i32, definition: Preset, default_fade: f32) -> Result<Self, Error> {
        definition.validate_processing()?;
        let processor = PresetProcessor::new(&definition)?;
        let mut fade = Fade::new(default_fade, false);
        fade.ramp.finish();
        Ok(Self {
            current: State {
                identity,
                definition,
                processor,
            },
            previous: None,
            fade,
            default_fade,
            scratch: Vec::with_capacity(1020),
        })
    }

    pub fn identity(&self) -> i32 {
        self.current.identity
    }
    pub fn definition(&self) -> &Preset {
        &self.current.definition
    }
    pub fn is_off(&self) -> bool {
        self.current.identity == 0
            && self
                .previous
                .as_ref()
                .is_none_or(|state| state.identity == 0)
    }

    pub fn select(&mut self, identity: i32, definition: Preset) -> Result<(), Error> {
        if identity == self.current.identity {
            return Ok(());
        }
        definition.validate_processing()?;
        let processor = PresetProcessor::new(&definition)?;
        let seconds = if self.current.definition.fade != 0.0 {
            self.current.definition.fade.abs()
        } else {
            self.default_fade
        };
        self.fade = Fade::new(seconds, self.current.definition.fade < 0.0);
        self.previous = Some(std::mem::replace(
            &mut self.current,
            State {
                identity,
                definition,
                processor,
            },
        ));
        Ok(())
    }

    pub fn process(&mut self, frames: &mut [Frame], random: &mut impl Random) {
        if self.fade.ramp.finished() {
            self.previous = None;
        }
        if self.is_off() {
            return;
        }
        if let Some(previous) = &mut self.previous {
            for frame in frames {
                let mono = frame[0].wrapping_add(frame[1]) >> 1;
                let current = if self.current.identity == 0 {
                    *frame
                } else {
                    [self.current.processor.sample(mono, random); 2]
                };
                let old = if previous.identity == 0 {
                    *frame
                } else {
                    [previous.processor.sample(mono, random); 2]
                };
                let blend = self.fade.advance();
                for channel in 0..2 {
                    let mut delta = current[channel]
                        .wrapping_sub(old[channel])
                        .wrapping_mul(blend)
                        >> 12;
                    if self.fade.squared {
                        delta = delta.wrapping_mul(blend) >> 12;
                    }
                    frame[channel] = old[channel].wrapping_add(delta);
                }
            }
        } else {
            self.scratch.clear();
            self.scratch.extend(
                frames
                    .iter()
                    .map(|frame| frame[0].wrapping_add(frame[1]) >> 1),
            );
            self.current.processor.process(&mut self.scratch, random);
            for (frame, value) in frames.iter_mut().zip(&self.scratch) {
                *frame = [*value; 2];
            }
        }
    }
}

/// Quantize each bus before sampling. A dry-decorated voice bypasses room and
/// player processing; ordinary zero-room voices still belong to the facing bus.
pub fn split_volume(
    volume: [u8; 2],
    send: f32,
    dsp_volume: f32,
    room_off: bool,
) -> ([u8; 2], [u8; 2]) {
    let send = if room_off { 0.0 } else { send };
    let wet = (send * dsp_volume).min(1.0);
    let dry = if dsp_volume < 1.0 {
        send * dsp_volume
    } else {
        send
    };
    (
        volume.map(|v| (v as f32 * wet) as u8),
        volume.map(|v| (f64::from(v) * (1.0 - f64::from(dry))) as u8),
    )
}

pub fn sample16(sample: i16, volume: u8) -> i32 {
    (i32::from(sample) * i32::from(volume)) >> 8
}
pub fn sample8(sample: i8, volume: u8) -> i32 {
    i32::from(sample) * i32::from(volume & !1)
}

pub fn finish(wet: &[Frame], facing: &[Frame], dry: &[Frame], output: &mut [i16]) {
    assert_eq!(wet.len(), facing.len());
    assert_eq!(wet.len(), dry.len());
    assert_eq!(wet.len() * 2, output.len());
    for (((wet, facing), dry), out) in wet
        .iter()
        .zip(facing)
        .zip(dry)
        .zip(output.chunks_exact_mut(2))
    {
        for channel in 0..2 {
            out[channel] = wet[channel]
                .wrapping_add(facing[channel])
                .wrapping_add(dry[channel])
                .clamp(-32768, 32767) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoDraw;
    impl Random for NoDraw {
        fn float(&mut self, _: f32, _: f32) -> f32 {
            panic!("unexpected draw")
        }
        fn integer(&mut self, _: i32, _: i32) -> i32 {
            panic!("unexpected draw")
        }
    }
    fn identity() -> Preset {
        Preset {
            configuration: 1,
            mix: [0.2, 0.7],
            duration: 0.0,
            fade: 0.0,
            minimum_level: 80.0,
            quiet_mix_drop: 0.5,
            processors: vec![],
        }
    }
    #[test]
    fn zero_is_stereo_and_nonzero_is_mono_even_when_both_have_identity_kernels() {
        let mut effect = MonoEffect::new(0, identity(), 0.2).unwrap();
        let mut frames = [[4096, 0]; 1020];
        effect.process(&mut frames, &mut NoDraw);
        assert_eq!(frames[0], [4096, 0]);
        effect.select(1, identity()).unwrap();
        effect.process(&mut frames, &mut NoDraw);
        assert_eq!(frames[0], [4096, 0]);
        assert!(frames[1019][0] < 4096 && frames[1019][1] > 0);
        for _ in 0..9 {
            frames.fill([4096, 0]);
            effect.process(&mut frames, &mut NoDraw);
        }
        frames.fill([4096, 0]);
        effect.process(&mut frames, &mut NoDraw);
        assert_eq!(frames[0], [2048, 2048]);
        effect.select(0, identity()).unwrap();
        effect.process(&mut frames, &mut NoDraw);
        assert!(!effect.is_off());
    }
    #[test]
    fn bus_integer_boundaries_and_eight_bit_gain_steps_are_preserved() {
        assert_eq!(
            split_volume([255, 127], 0.3, 1.0, false),
            ([76, 38], [178, 88])
        );
        assert_eq!(
            split_volume([255, 127], 0.3, 1.0, true),
            ([0, 0], [255, 127])
        );
        assert_eq!(sample16(-1, 1), -1);
        assert_eq!(sample8(-128, 255), -32512);
        let mut output = [0; 2];
        finish(&[[32767, -32768]], &[[1, -1]], &[[0; 2]], &mut output);
        assert_eq!(output, [32767, -32768]);
    }
}
