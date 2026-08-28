//! Source pixel-visibility proxy geometry and asynchronous result fading.

pub const DEFAULT_PROXY_SIZE: f32 = 2.0;
pub const DEFAULT_FADE_TIME: f32 = 0.0625;
const MIN_PROXY_PIXELS: f32 = 5.0;

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub origin: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub world_to_clip: [[f32; 4]; 4],
    pub height: u32,
    pub far:f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Parameters {
    pub position: [f32; 3],
    pub size: f32,
    pub aspect: f32,
    pub screen_space: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proxy {
    /// Center and four corners, matching the four-triangle Source proxy fan.
    pub vertices: [[f32; 3]; 5],
    pub clip_vertices: [[f32; 4]; 5],
    pub ndc: [[f32; 2]; 4],
    pub clip_fraction: f32,
}

impl View {
    pub fn perspective(origin: [f32; 3], yaw: f32, pitch: f32, fov: f32, aspect: f32, near: f32, far: f32, height: u32) -> Self {
        let (sy, cy) = yaw.to_radians().sin_cos();
        let (sp, cp) = pitch.to_radians().sin_cos();
        let forward = [cp*cy, cp*sy, -sp];
        let right = [sy, -cy, 0.0];
        let up = [sp*cy, sp*sy, cp];
        let row = |axis: [f32; 3], amount: f32, offset: f32| {
            let a = scale(axis, amount);
            [a[0], a[1], a[2], offset - (a[0]*origin[0]+a[1]*origin[1]+a[2]*origin[2])]
        };
        let sy = 1.0 / (fov.to_radians() * 0.5).tan();
        Self { origin, forward, right, up, height,far, world_to_clip: [row(right,sy/aspect,0.0),row(up,sy,0.0),row(forward,far/(far-near),-near*far/(far-near)),row(forward,1.0,0.0)] }
    }
    fn clip(&self, point: [f32; 3]) -> [f32; 4] {
        self.world_to_clip.map(|row| row[0] * point[0] + row[1] * point[1] + row[2] * point[2] + row[3])
    }

    fn diameter(&self, origin: [f32; 3], radius: f32) -> f32 {
        let top = self.clip(add(origin, scale(self.up, radius)));
        let bottom = self.clip(add(origin, scale(self.up, -radius)));
        let project = |point: [f32; 4]| if point[3] >= 0.001 { point[1] / point[3] } else { point[1] * 1000.0 };
        self.height as f32 * (project(bottom) - project(top)).abs() * 0.5
    }

    pub fn proxy(&self, parameters: Parameters) -> Option<Proxy> {
        let pixels_per_unit = self.diameter(parameters.position, 1.0).max(1.0e-4);
        let mut size = parameters.size;
        if parameters.screen_space {
            size = self.diameter(add(self.origin, self.forward), size * 0.5) / pixels_per_unit;
        } else if size * pixels_per_unit < MIN_PROXY_PIXELS {
            size = MIN_PROXY_PIXELS / pixels_per_unit;
        }
        let direction = sub(parameters.position, self.origin);
        let length = direction.iter().map(|value| value * value).sum::<f32>().sqrt();
        let direction = if length == 0.0 { [0.0; 3] } else { scale(direction, 1.0 / length) };
        let center = add(parameters.position, scale(direction, -parameters.size));
        let horizontal = size * 0.707106781_f32;
        let vertical = horizontal / parameters.aspect;
        let right = scale(self.right, horizontal);
        let up = scale(self.up, vertical);
        let vertices = [center, sub(add(center, up), right), add(add(center, up), right), add(sub(center, up), right), sub(sub(center, up), right)];
        let mut ndc = [[0.0; 2]; 4];
        for (point, output) in vertices[1..].iter().zip(&mut ndc) {
            let clip = self.clip(*point);
            if clip[3] < 0.001 { return None; }
            *output = [clip[0] / clip[3], clip[1] / clip[3]];
        }
        let area = (ndc[1][0] - ndc[0][0]) * (ndc[0][1] - ndc[3][1]);
        let clipped = (ndc[1][0].min(1.0) - ndc[0][0].max(-1.0)) * (ndc[0][1].min(1.0) - ndc[3][1].max(-1.0));
        let clip_fraction = if area == 0.0 { 0.0 } else { (clipped / area).clamp(0.0, 1.0) };
        Some(Proxy { clip_vertices: vertices.map(|point| self.clip(point)), vertices, ndc, clip_fraction })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Query {
    brightness: f32,
    clip_fraction: f32,
    issued: Option<u64>,
    sampled: Option<u64>,
    failed: bool,
}

impl Query {
    pub fn sample(&mut self, frame: u64, elapsed: f32, fade_time: f32, counts: Option<(u32, u32)>) -> f32 {
        if self.sampled != Some(frame) {
            self.sampled = Some(frame);
            if let Some((visible, possible)) = counts {
                if possible == 0 { self.brightness = 0.0; }
                else {
                    let target = visible as f32 / possible as f32;
                    let target = if target >= 0.95 { 1.0 } else { target.max(0.0) };
                    let fade_time_inverse = if fade_time > 0.0 { 1.0 / fade_time } else { 1.0 / 0.125 };
                    let rate = elapsed * fade_time_inverse;
                    self.brightness += (target - self.brightness).clamp(-rate, rate);
                }
            } else {
                self.failed = self.issued.is_some();
            }
        }
        self.brightness * self.clip_fraction
    }

    /// A pending read skips reissuing the GPU work, but still marks this view active.
    pub fn issue(&mut self, frame: u64, proxy: Option<&Proxy>) -> bool {
        let issue = !self.failed;
        if issue {
            let Some(proxy) = proxy else {
                self.clip_fraction = 0.0; self.sampled = None; self.failed = false;
                return false;
            };
            self.clip_fraction = proxy.clip_fraction;
        }
        self.issued = Some(frame); self.sampled = None; self.failed = false;
        issue
    }

    pub fn active(&self, frame: u64) -> bool { self.issued.is_some_and(|issued| frame.saturating_sub(issued) <= 1) }
    /// EndScene already retired queries unused for two frames before this draw.
    pub fn expired_before_frame(&self, frame:u64)->bool {self.issued.is_some_and(|issued|frame.saturating_sub(issued)>2)}
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { std::array::from_fn(|axis| a[axis] + b[axis]) }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { std::array::from_fn(|axis| a[axis] - b[axis]) }
fn scale(value: [f32; 3], amount: f32) -> [f32; 3] { value.map(|value| value * amount) }

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View { origin: [0.0; 3], forward: [1.0, 0.0, 0.0], right: [0.0, -1.0, 0.0], up: [0.0, 0.0, 1.0],
            world_to_clip: [[0.0, -1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [1.0, 0.0, 0.0, -1.0], [1.0, 0.0, 0.0, 0.0]], height: 1000,far:30000.0 }
    }

    #[test]
    fn proxy_expands_only_perpendicular_to_view_and_preserves_authored_depth_bias() {
        let proxy = view().proxy(Parameters { position: [1000.0, 0.0, 0.0], size: 2.0, aspect: 2.0, screen_space: false }).unwrap();
        assert_eq!(proxy.vertices[0], [998.0, 0.0, 0.0]);
        assert!((proxy.vertices[1][1] - 5.0 * 0.707106781).abs() < 1e-5);
        assert!((proxy.vertices[1][2] * 2.0 - proxy.vertices[1][1]).abs() < 1e-5);
        assert_eq!(proxy.clip_fraction, 1.0);
        assert!(view().proxy(Parameters { position: [-10.0, 0.0, 0.0], size: 2.0, aspect: 1.0, screen_space: false }).is_none());
    }

    #[test]
    fn partial_queries_fade_once_per_frame_and_do_not_overwrite_pending_queries() {
        let proxy = view().proxy(Parameters { position: [10.0, 0.0, 0.0], size: 2.0, aspect: 1.0, screen_space: false }).unwrap();
        let mut state = Query::default();
        assert_eq!(state.sample(0, 0.015625, DEFAULT_FADE_TIME, None), 0.0);
        assert!(state.issue(0, Some(&proxy)));
        assert_eq!(state.sample(1, 0.015625, DEFAULT_FADE_TIME, Some((95, 100))), 0.25);
        assert_eq!(state.sample(1, 0.015625, DEFAULT_FADE_TIME, Some((95, 100))), 0.25);
        assert!(state.issue(1, Some(&proxy)));
        assert_eq!(state.sample(2, 0.015625, DEFAULT_FADE_TIME, None), 0.25);
        assert!(!state.issue(2, Some(&proxy)));
        assert!(state.active(3)); assert!(!state.active(4));
        assert_eq!(state.sample(3, 0.015625, DEFAULT_FADE_TIME, Some((0, 100))), 0.0);
    }

    #[test]
    fn screen_space_proxy_scales_with_distance_without_scaling_depth_bias() {
        let near = view().proxy(Parameters { position: [10.0, 0.0, 0.0], size: 0.1, aspect: 1.0, screen_space: true }).unwrap();
        let far = view().proxy(Parameters { position: [100.0, 0.0, 0.0], size: 0.1, aspect: 1.0, screen_space: true }).unwrap();
        assert!((far.vertices[1][1] / near.vertices[1][1] - 10.0).abs() < 1e-5);
        assert_eq!(near.vertices[0][0], 9.9);
        assert_eq!(far.vertices[0][0], 99.9);
        assert_eq!(near.clip_vertices[0], view().clip(near.vertices[0]));
    }

    #[test]
    fn offscreen_and_zero_possible_results_do_not_leak_the_previous_glow() {
        let proxy = view().proxy(Parameters { position: [10.0, 0.0, 0.0], size: 2.0, aspect: 1.0, screen_space: false }).unwrap();
        let mut state = Query::default();
        state.issue(0, Some(&proxy));
        // Nonpositive fade time uses the SDK's 1/8 second fallback, not an instant change.
        assert_eq!(state.sample(1, 0.03125, 0.0, Some((100, 100))), 0.25);
        assert!(!state.issue(1, None));
        assert_eq!(state.sample(2, 0.03125, 0.0, Some((100, 100))), 0.0);
        state.issue(2, Some(&proxy));
        assert_eq!(state.sample(3, 0.03125, 0.0, Some((0, 0))), 0.0);
    }

    #[test]
    fn inactivity_is_retired_at_the_previous_end_scene_not_before_the_current_draw() {
        let proxy=view().proxy(Parameters{position:[10.0,0.0,0.0],size:2.0,aspect:1.0,screen_space:false}).unwrap();
        let mut query=Query::default();query.issue(2,Some(&proxy));
        assert!(!query.expired_before_frame(4));assert!(query.expired_before_frame(5));
    }
}
