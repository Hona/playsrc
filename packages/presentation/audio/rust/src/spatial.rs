use crate::{
    dsp::Preset,
    soundscape::{Listener, Position},
};

/// Ordinary Source sound-level distance gain with the configured PC defaults.
/// Occlusion is a separate gain, not part of this distance-only function.
pub fn distance_gain(sound_level: i32, distance: f32) -> f32 {
    if sound_level == 0 {
        return 1.0;
    }
    let exponent = 60.0_f32 * 0.05 - sound_level as f32 * 0.05;
    let multiplier = (10.0_f64.powf(f64::from(exponent)) / 36.0) as f32;
    let foliage_exponent = distance * ((1.0_f32 / 1200.0) * 0.05) * 4.0;
    let relative = distance * multiplier * (10.0_f64.powf(f64::from(foliage_exponent)) as f32);
    let mut gain = if f64::from(relative) > 0.1 {
        1.0 / relative
    } else {
        10.0
    };
    if gain > 0.5 {
        let recovered = (20.0 * (1000.0 / f64::from(multiplier * 36.0)).log10()) as i32;
        let power = if recovered > 90 {
            2.5 - (recovered as f32 - 90.0) * ((2.5_f32 - 0.8) / 50.0)
        } else {
            2.5
        };
        let crossover = (2.0 * 0.5_f64.powf(-f64::from(power))) as f32;
        gain = (1.0 - 1.0 / (f64::from(crossover) * f64::from(gain).powf(f64::from(power)))) as f32;
    }
    if gain < 0.01 {
        gain = (2.0 - relative * 0.01) * 0.01;
        if gain <= 0.0 {
            gain = 0.001;
        }
    }
    gain
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomSend {
    range: [f32; 2],
}
impl RoomSend {
    /// A voice captures these limits once. Moving/replacing rooms must not
    /// change the send interval of voices that were already playing.
    pub fn new(sound_level: i32, preset: &Preset) -> Self {
        let scale = if sound_level <= preset.minimum_level as i32 {
            preset.quiet_mix_drop
        } else {
            1.0
        };
        Self {
            range: preset.mix.map(|value| value * scale),
        }
    }
    pub fn gain(self, distance: f32) -> f32 {
        let distance = (distance as i32 as f32).clamp(0.0, 1440.0) / 1440.0;
        self.range[0] + (self.range[1] - self.range[0]) * distance
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stereo {
    pub volume: [u8; 2],
    pub source_distance: f32,
    pub distance_gain: f32,
}

pub fn stereo(
    volume: f32,
    level: i32,
    origin: Option<Position>,
    listener: Listener,
    radius: f32,
    acoustic_gain: f32,
    omni: bool,
) -> Stereo {
    let master = (volume * 255.0) as i32;
    let (distance, pan, mono) = if let Some(origin) = origin {
        let delta: Position = std::array::from_fn(|axis| origin[axis] - listener.origin[axis]);
        let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        let pan = if distance > 0.0 {
            (0..3)
                .map(|axis| listener.right[axis] * (delta[axis] / distance))
                .sum()
        } else {
            0.0
        };
        let mono = if level == 0 || omni {
            1.0
        } else if radius > 0.0 && distance < radius {
            1.0 - (distance - radius * 0.5).max(0.0) / (radius * 0.5)
        } else {
            0.0
        };
        (distance, pan, mono)
    } else {
        (12.0, 0.0, 1.0)
    };
    let gain = if origin.is_none() {
        acoustic_gain
    } else {
        distance_gain(level, distance) * acoustic_gain
    };
    let pan = if mono > 0.0 {
        (f64::from(pan) * (1.0 - f64::from(mono))) as f32
    } else {
        pan
    };
    let mut channels = [
        ((master as f32 * (gain * ((1.0 - f64::from(pan)) as f32) / 2.0)) as i32).clamp(0, 255),
        ((master as f32 * (gain * ((1.0 + f64::from(pan)) as f32) / 2.0)) as i32).clamp(0, 255),
    ];
    if origin.is_none() {
        let total = (channels[0] + channels[1]).clamp(0, 255);
        channels = [total, total];
    }
    Stereo {
        volume: channels.map(|value| value as u8),
        source_distance: distance,
        distance_gain: gain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_distance_only_response() {
        assert_eq!(distance_gain(70, 100.0).to_bits(), 1064172826);
        assert_eq!(distance_gain(0, 100000.0), 1.0);
    }
    #[test]
    fn ambient_and_centered_world_remain_distinct_and_wet_send_retains_start_limits() {
        let listener = Listener {
            origin: [0.0; 3],
            forward: [1.0, 0.0, 0.0],
            right: [0.0, -1.0, 0.0],
        };
        let ambient = stereo(1.0, 0, None, listener, 0.0, 1.0, false);
        let world = stereo(1.0, 0, Some([1.0, 0.0, 0.0]), listener, 0.0, 1.0, false);
        assert_eq!(ambient.volume, [254, 254]);
        assert_eq!(world.volume, [127, 127]);
        let mut preset = Preset {
            configuration: 1,
            mix: [0.2, 0.7],
            minimum_level: 80.0,
            quiet_mix_drop: 0.5,
            duration: 0.0,
            fade: 0.0,
            processors: vec![],
        };
        let send = RoomSend::new(80, &preset);
        preset.mix = [0.8, 1.0];
        assert_eq!(send.gain(0.0), 0.1);
        assert_eq!(send.gain(2000.0), 0.35);
        assert_ne!(send.gain(0.0), RoomSend::new(80, &preset).gain(0.0));
    }
}
