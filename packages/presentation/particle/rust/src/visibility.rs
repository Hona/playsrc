#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilitySample {
    pub identity: u64,
    pub visible_pixels: i32,
    pub possible_pixels: i32,
    pub clip_fraction: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityProxy {
    pub identity: u64,
    pub vertices: [[f32; 3]; 5],
    pub clip_fraction: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibilityView {
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub vertical_fov_degrees: f32,
    pub width: f32,
    pub height: f32,
}

impl VisibilityView {
    pub fn valid(&self) -> bool {
        [
            self.yaw_degrees,
            self.pitch_degrees,
            self.vertical_fov_degrees,
            self.width,
            self.height,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.vertical_fov_degrees > 0.0
            && self.vertical_fov_degrees < 180.0
            && self.width > 0.0
            && self.height > 0.0
    }

    pub fn proxy(
        &self,
        identity: u64,
        mut origin: [f32; 3],
        radius: f32,
        camera: [f32; 3],
    ) -> VisibilityProxy {
        use crate::world::{add, mul, normalize, sub};
        let (sy, cy) = self.yaw_degrees.to_radians().sin_cos();
        let (sp, cp) = self.pitch_degrees.to_radians().sin_cos();
        let forward = [cp * cy, cp * sy, -sp];
        let right = [sy, -cy, 0.0];
        let up = [sp * cy, sp * sy, cp];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let y_scale = 1.0 / (self.vertical_fov_degrees.to_radians() * 0.5).tan();
        let project = |point| {
            let delta = sub(point, camera);
            [
                dot(delta, right) * y_scale * self.height / self.width,
                dot(delta, up) * y_scale,
                dot(delta, forward),
            ]
        };
        let a = project(add(origin, up));
        let b = project(sub(origin, up));
        let divide = |p: [f32; 3]| {
            if p[2] >= 0.001 {
                p[1] / p[2]
            } else {
                p[1] * 1000.0
            }
        };
        let pixels_per_unit = (self.height * (divide(b) - divide(a)).abs() * 0.5).max(0.0001);
        let scale = radius.max(5.0 / pixels_per_unit) * 0.707106781;
        origin = sub(
            origin,
            mul(normalize(sub(origin, camera)).unwrap_or([0.0; 3]), radius),
        );
        let vertices = [
            origin,
            sub(add(origin, mul(up, scale)), mul(right, scale)),
            add(add(origin, mul(up, scale)), mul(right, scale)),
            add(sub(origin, mul(up, scale)), mul(right, scale)),
            sub(sub(origin, mul(up, scale)), mul(right, scale)),
        ];
        let mut screen = [[0.0; 2]; 4];
        for (index, vertex) in vertices[1..].iter().enumerate() {
            let p = project(*vertex);
            if p[2] < 0.001 {
                return VisibilityProxy {
                    identity,
                    vertices,
                    clip_fraction: 0.0,
                };
            }
            screen[index] = [p[0] / p[2], p[1] / p[2]];
        }
        let width = screen[1][0] - screen[0][0];
        let height = screen[0][1] - screen[3][1];
        let clipped_width = screen[1][0].min(1.0) - screen[0][0].max(-1.0);
        let clipped_height = screen[0][1].min(1.0) - screen[3][1].max(-1.0);
        let clip_fraction = if width * height == 0.0 {
            0.0
        } else {
            (clipped_width * clipped_height / (width * height)).clamp(0.0, 1.0)
        };
        VisibilityProxy {
            identity,
            vertices,
            clip_fraction,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct VisibilityState {
    brightness: f32,
    clip_fraction: f32,
}

impl VisibilityState {
    pub fn sample(&mut self, sample: Option<&VisibilitySample>, seconds: f32) -> f32 {
        if let Some(sample) = sample {
            self.clip_fraction = sample.clip_fraction;
            if sample.visible_pixels >= 0 && sample.possible_pixels >= 0 {
                if sample.possible_pixels == 0 {
                    self.brightness = 0.0;
                } else {
                    let target = sample.visible_pixels as f32 / sample.possible_pixels as f32;
                    let target = if target >= 0.95 { 1.0 } else { target.max(0.0) };
                    let rate = seconds / 0.0625;
                    self.brightness += (target - self.brightness).clamp(-rate, rate);
                }
            }
        }
        self.brightness * self.clip_fraction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proxies_use_camera_facing_four_triangle_geometry_and_exact_frustum_clipping() {
        let view = VisibilityView {
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            vertical_fov_degrees: 90.0,
            width: 100.0,
            height: 100.0,
        };
        let proxy = view.proxy(7, [100.0, 0.0, 0.0], 8.0, [0.0; 3]);
        assert_eq!(proxy.vertices[0], [92.0, 0.0, 0.0]);
        assert_eq!(proxy.clip_fraction, 1.0);
        assert!((proxy.vertices[1][1] - 8.0 * 0.707106781).abs() < 0.00001);
        let clipped = view.proxy(7, [100.0, 100.0, 0.0], 8.0, [0.0; 3]);
        assert!((clipped.clip_fraction - 0.5).abs() < 0.00001);
        assert_eq!(
            view.proxy(7, [-100.0, 0.0, 0.0], 8.0, [0.0; 3])
                .clip_fraction,
            0.0
        );
        let tiny = view.proxy(7, [100.0, 0.0, 0.0], 1.0, [0.0; 3]);
        assert!((tiny.vertices[1][1] - 5.0 * 0.707106781).abs() < 0.00001);
    }
    #[test]
    fn partial_queries_fade_snap_and_hold_pending_without_reusing_another_lifetime() {
        let mut state = VisibilityState::default();
        assert_eq!(state.sample(None, 0.015), 0.0);
        let mut sample = VisibilitySample {
            identity: 1,
            visible_pixels: 95,
            possible_pixels: 100,
            clip_fraction: 0.5,
        };
        assert_eq!(state.sample(Some(&sample), 0.015625), 0.125);
        assert_eq!(state.sample(Some(&sample), 0.0625), 0.5);
        sample.visible_pixels = -1;
        assert_eq!(state.sample(Some(&sample), 0.0625), 0.5);
        assert_eq!(state.sample(None, 0.0625), 0.5);
        sample.visible_pixels = 25;
        assert_eq!(state.sample(Some(&sample), 0.0625), 0.125);
        sample.possible_pixels = 0;
        assert_eq!(state.sample(Some(&sample), 0.015), 0.0);
        assert_eq!(VisibilityState::default().sample(None, 1.0), 0.0);
    }
}
