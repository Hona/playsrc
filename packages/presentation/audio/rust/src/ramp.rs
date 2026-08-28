//! Shared quantized parameter ramps for effect transitions and modulation.
use crate::dsp::SAMPLE_RATE;

#[derive(Clone, Debug)]
pub(crate) struct Ramp {
    initial: i32,
    direction: i32,
    extent: i32,
    position: i32,
    fraction: i32,
    step: i32,
    value: i32,
    done: bool,
}
fn truncate(value: f32) -> i32 {
    if !value.is_finite() || !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}
impl Ramp {
    pub fn new(seconds: f32, initial: i32, target: i32) -> Self {
        let delta = target.wrapping_sub(initial);
        let count = truncate(seconds * SAMPLE_RATE as f32);
        let rate = delta.wrapping_abs() as f32 / count as f32;
        let integer = truncate(rate);
        let rate = if integer > 4095 {
            rate - integer as f32 + 4095.0
        } else {
            rate
        };
        Self {
            initial,
            direction: if delta < 0 { -1 } else { 1 },
            extent: delta.wrapping_abs(),
            position: 0,
            fraction: 0,
            step: truncate(rate * (1 << 20) as f32),
            value: initial,
            done: false,
        }
    }
    pub fn finished(&self) -> bool {
        self.done
    }
    pub fn finish(&mut self) {
        self.done = true;
    }
    pub fn next(&mut self) -> i32 {
        if !self.done {
            let prior = self.position;
            self.fraction = self.fraction.wrapping_add(self.step);
            self.position = self.position.wrapping_add(self.fraction >> 20);
            self.fraction &= (1 << 20) - 1;
            self.done = self.step == 0 || self.position < 0 || self.position >= self.extent;
            let position = if self.done { prior } else { self.position };
            self.value = self
                .initial
                .wrapping_add(self.direction.wrapping_mul(position));
        }
        self.value
    }
}
