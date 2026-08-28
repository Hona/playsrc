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

/// Quantize the bus multiplier, retaining fractional channel gains until the
/// sample conversion. Dry voices bypass room/player processing entirely.
pub fn split_volume(
    volume: [f32; 2],
    send: f32,
    dsp_volume: f32,
    room_off: bool,
) -> ([f32; 2], [f32; 2]) {
    let send = if room_off { 0.0 } else { send };
    let send = send * 256.0;
    let wet = ((send * dsp_volume) as i32).min(256);
    let dry = if dsp_volume < 1.0 {
        send * dsp_volume
    } else {
        send
    };
    (
        volume.map(|v| (v * wet as f32 * (1.0 / 256.0)).clamp(0.0, 255.0)),
        volume.map(|v| (v * (256.0 - dry) as i32 as f32 * (1.0 / 256.0)).clamp(0.0, 255.0)),
    )
}

pub fn sample16(sample: i16, volume: f32) -> i32 {
    (f32::from(sample) * volume * (1.0 / 256.0)) as i32
}
pub fn sample8(sample: i8, volume: f32) -> i32 {
    i32::from(sample) * ((volume as i32) & !1)
}

/// Paint compression precedes the device's floating master gain. Keep that
/// fractional gain through the browser boundary instead of requantizing PCM.
pub fn device_sample(sample: i32, master: f32) -> f32 {
    sample.clamp(-32767, 32767) as f32 * master * (1.0 / 32768.0)
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
    fn bus_scale_quantization_and_fractional_channel_gain_are_preserved() {
        let (wet, facing) = split_volume([254.0, 254.0], 0.33, 1.0, false);
        assert_eq!(wet.map(f32::to_bits), [0x42a6b000; 2]);
        assert_eq!(facing.map(f32::to_bits), [0x4329aa00; 2]);
        assert_eq!(
            split_volume([255.0, 127.0], 0.3, 1.0, false),
            ([75.703125, 37.703125], [178.30078, 88.80078])
        );
        assert_eq!(
            split_volume([255.0, 127.0], 0.3, 1.0, true),
            ([0.0, 0.0], [255.0, 127.0])
        );
        assert_eq!(sample16(-1, 1.0), 0);
        assert_eq!(sample16(-1000, 128.5), -501);
        assert_eq!(sample8(-128, 255.0), -32512);
        assert_eq!(device_sample(-32768, 1.0), -32767.0 / 32768.0);
        assert_eq!(device_sample(32768, 1.0), 32767.0 / 32768.0);
        assert_eq!(device_sample(1, 0.5), 0.5 / 32768.0);
    }
}
