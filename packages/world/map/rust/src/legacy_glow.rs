//! Source glow-overlay geometry. These quads are placed on the overlay plane,
//! while their visibility proxies remain at the authored world positions.
use crate::pixel_visibility::{Parameters, Proxy, View};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightGlow {
    pub horizontal_size: i32,
    pub vertical_size: i32,
    pub minimum_distance: i32,
    pub maximum_distance: i32,
    pub outer_maximum_distance: i32,
    pub one_sided: bool,
    pub proxy_radius: f32,
    pub distance_origin: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub positions: [[f32; 3]; 4],
    pub color: [f32; 3],
}

impl LightGlow {
    pub fn proxy(&self, view: &View, position: [f32; 3]) -> Option<Proxy> {
        let aspect = if self.horizontal_size != 0 && self.vertical_size != 0 {
            self.horizontal_size as f32 / self.vertical_size as f32
        } else { 1.0 };
        view.proxy(Parameters { position, size: self.proxy_radius, aspect, screen_space: false })
    }

    pub fn quad(&self, view: &View, position: [f32; 3], direction: [f32; 3], color: [u8; 3], visibility: f32) -> Option<Quad> {
        let (to_view, distance) = normalize(sub(view.origin, self.distance_origin));
        if self.one_sided && dot(to_view, direction) < 0.0 { return None; }
        let fade = if self.outer_maximum_distance > self.maximum_distance && distance > self.maximum_distance as f32 {
            remap(distance, self.maximum_distance as f32, self.outer_maximum_distance as f32, 1.0, 0.0)
        } else { remap(distance, self.minimum_distance as f32, self.maximum_distance as f32, 0.0, 1.0) };
        let color = color.map(|value| value as f32 / 255.0 * fade * visibility);
        overlay_quad(view,sub(position,view.origin),[self.horizontal_size as f32,self.vertical_size as f32],color)
    }
}

pub fn overlay_quad(view:&View,direction:[f32;3],size:[f32;2],color:[f32;3])->Option<Quad>{
    if dot(color,color)<0.00001{return None;}
    let (direction,_)=normalize(direction);let center=add(view.origin,scale(direction,100.0));
    let (right,_)=normalize(cross(direction,[0.0,0.0,1.0]));let (up,_)=normalize(cross(right,direction));
    let right=scale(right,size[0]);let up=scale(up,size[1]);
    let color=color.map(|value|((value*255.0+8_388_608.0).to_bits()&255) as f32/255.0);
    Some(Quad{positions:[add(sub(center,right),up),add(add(center,right),up),sub(add(center,right),up),sub(sub(center,right),up)],color})
}

fn remap(value: f32, low: f32, high: f32, from: f32, to: f32) -> f32 {
    if low == high { return if value >= high { to } else { from }; }
    from + (to - from) * ((value - low) / (high - low)).clamp(0.0, 1.0)
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { std::array::from_fn(|i| a[i] + b[i]) }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { std::array::from_fn(|i| a[i] - b[i]) }
fn scale(a: [f32; 3], b: f32) -> [f32; 3] { a.map(|value| value * b) }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
fn normalize(a: [f32; 3]) -> ([f32; 3], f32) { let length=dot(a,a).sqrt(); (if length==0.0 { a } else { scale(a, 1.0/length) },length) }

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View { View { origin: [0.0;3], forward: [1.0,0.0,0.0], right: [0.0,-1.0,0.0], up: [0.0,0.0,1.0], world_to_clip: [[0.0;4];4], height: 720,far:30000.0 } }
    fn glow() -> LightGlow { LightGlow { horizontal_size: 8, vertical_size: 4, minimum_distance: 0, maximum_distance: 200, outer_maximum_distance: 400, one_sided: false, proxy_radius: 2.0, distance_origin: [100.0,0.0,0.0] } }
    #[test]
    fn overlay_plane_and_distance_fade_are_not_a_world_sprite() {
        let quad=glow().quad(&view(), [1000.0,0.0,0.0], [1.0,0.0,0.0], [255;3], 0.5).unwrap();
        assert_eq!(quad.positions, [[100.0,8.0,4.0],[100.0,-8.0,4.0],[100.0,-8.0,-4.0],[100.0,8.0,-4.0]]);
        assert_eq!(quad.color, [64.0/255.0;3]);
    }
    #[test]
    fn one_sided_outer_fade_and_zero_width_ranges_follow_authored_values() {
        let mut glow=glow(); glow.one_sided=true;
        assert!(glow.quad(&view(),[100.0,0.0,0.0],[1.0,0.0,0.0],[255;3],1.0).is_none());
        glow.one_sided=false; glow.distance_origin=[300.0,0.0,0.0];
        assert_eq!(glow.quad(&view(),[100.0,0.0,0.0],[1.0,0.0,0.0],[255;3],1.0).unwrap().color,[128.0/255.0;3]);
        glow.maximum_distance=0; glow.outer_maximum_distance=0;
        assert_eq!(glow.quad(&view(),[100.0,0.0,0.0],[1.0,0.0,0.0],[255;3],1.0).unwrap().color,[1.0;3]);
    }
}
