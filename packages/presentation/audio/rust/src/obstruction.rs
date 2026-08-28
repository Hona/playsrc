//! World-only obstruction queries. Scheduling and gain smoothing belong to voices.
use crate::playback::ObstructionRequest;

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub hit: bool,
    pub fraction: f32,
    pub start_solid: bool,
}
fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    // Portable SDK mathlib normalization (including the zero-length epsilon).
    let inverse = 1.0 / (length + f32::EPSILON);
    value.map(|value| value * inverse)
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn gain(
    listener: [f32; 3],
    request: ObstructionRequest,
    mut trace: impl FnMut([f32; 3], [f32; 3]) -> Hit,
) -> f32 {
    let center = trace(listener, request.origin);
    if !center.hit || center.fraction >= 0.99 {
        return 1.0;
    }
    let exponent = 60.0_f32 * 0.05 - request.level as f32 * 0.05;
    let multiplier = (10.0_f64.powf(f64::from(exponent)) / 36.0) as f32;
    let level = (20.0 * (1000.0 / f64::from(multiplier * 36.0)).log10()) as i32;
    let radius = if request.radius > 0.0 {
        request.radius
    } else {
        (24.0 + 216.0 * (f64::from(level) - 60.0) / 80.0) as f32
    };
    let forward = normalize(std::array::from_fn(|axis| {
        listener[axis] - request.origin[axis]
    }));
    let (right, mut up) = if forward[0] == 0.0 && forward[1] == 0.0 {
        ([0.0, -1.0, 0.0], [-forward[2], 0.0, 0.0])
    } else {
        let right = normalize(cross(forward, [0.0, 0.0, 1.0]));
        (right, normalize(cross(right, forward)))
    };
    let left_diagonal = normalize(std::array::from_fn(|axis| up[axis] + right[axis]));
    if request.origin[2] > listener[2] + 120.0 {
        up[2] = -up[2];
    }
    let right_diagonal = normalize(std::array::from_fn(|axis| up[axis] - right[axis]));
    let mut obscured = 0;
    let mut gain = 1.0;
    for (direction, distance) in [
        (left_diagonal, radius * 0.5),
        (right_diagonal, radius * 0.5),
        (left_diagonal, radius),
        (right_diagonal, radius),
    ] {
        let end = std::array::from_fn(|axis| request.origin[axis] + distance * direction[axis]);
        let hit = trace(listener, end);
        if hit.hit && hit.fraction < 0.99 && !hit.start_solid {
            obscured += 1;
            if obscured > 1 {
                gain *= 10.0_f32.powf(-2.70_f32 / 20.0);
            }
        }
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_visibility_and_extent_startsolid_preserve_trace_and_loss_boundaries() {
        let request = ObstructionRequest {
            voice: 1,
            origin: [100.0, 0.0, 0.0],
            level: 75,
            radius: 32.0,
        };
        let mut count = 0;
        assert_eq!(
            gain([0.0; 3], request, |_, _| {
                count += 1;
                Hit {
                    hit: true,
                    fraction: 0.99,
                    start_solid: false,
                }
            }),
            1.0
        );
        assert_eq!(count, 1);
        count = 0;
        let result = gain([0.0; 3], request, |_, _| {
            count += 1;
            Hit {
                hit: true,
                fraction: 0.5,
                start_solid: count == 2,
            }
        });
        assert_eq!(count, 5);
        let attenuation = 10.0_f32.powf(-2.70_f32 / 20.0);
        assert_eq!(result, attenuation * attenuation);
    }
}
