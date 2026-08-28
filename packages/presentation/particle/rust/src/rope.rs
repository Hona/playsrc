use crate::world::sub;
use crate::{Error, ErrorCode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RopePoint {
    pub position: [f32; 3],
    pub width: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct RopeRender {
    pub points: Vec<RopePoint>,
    pub subdivisions: usize,
    pub texel_size: f32,
    pub scroll_offset: f32,
    pub camera: [f32; 3],
    pub mesh: Option<playsrc_beam::Mesh>,
}

impl RopeRender {
    pub fn vertex_count(points: usize, subdivisions: usize) -> Option<usize> {
        points
            .checked_sub(1)?
            .checked_mul(subdivisions)?
            .checked_mul(2)?
            .checked_add(2)
    }

    pub fn resolve(&mut self, mapping_height: u32) -> Result<(), Error> {
        if self.mesh.is_some() {
            return Ok(());
        }
        if self.points.len() < 2 || self.subdivisions == 0 || mapping_height == 0 {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "particle-rope",
                0,
                "rope points, subdivisions or mapping height are invalid",
            ));
        }
        let vertices =
            Self::vertex_count(self.points.len(), self.subdivisions).ok_or_else(|| {
                Error::new(
                    ErrorCode::BoundExceeded,
                    "particle-rope",
                    0,
                    "rope vertex count overflow",
                )
            })?;
        let mut beam=playsrc_beam::Builder::new(vertices/2,self.camera);
        let scale = 1.0 / (mapping_height as f32 * self.texel_size);
        let step = 1.0 / self.subdivisions as f32;
        let mut length = 0.0;
        let mut previous = self.points[0];
        let mut previous_v = self.scroll_offset * scale;
        for index in 1..self.points.len() {
            let start = self.points[index - 1];
            let end = self.points[index];
            let delta = sub(end.position, start.position);
            length += (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
            let end_v = (length + self.scroll_offset) * scale;
            let mut subdivided = start;
            let mut v = previous_v;
            let color_step =
                std::array::from_fn::<_, 4, _>(|axis| (end.color[axis] - start.color[axis]) * step);
            let width_step = (end.width - start.width) * step;
            let v_step = (end_v - previous_v) * step;
            for division in 1..=self.subdivisions {
                let (next, next_v) = if division == self.subdivisions {
                    (end, end_v)
                } else {
                    for axis in 0..4 {
                        subdivided.color[axis] =
                            (subdivided.color[axis] + color_step[axis]).clamp(0.0, 1.0);
                    }
                    subdivided.width += width_step;
                    v += v_step;
                    subdivided.position = catmull_rom(
                        self.points[index.saturating_sub(2)].position,
                        start.position,
                        end.position,
                        self.points[(index + 1).min(self.points.len() - 1)].position,
                        division as f32 / self.subdivisions as f32,
                    );
                    (subdivided, v)
                };
                beam.push(playsrc_beam::Segment{position:previous.position,width:previous.width,color:previous.color,texture_coordinate:previous_v});
                previous = next;
                previous_v = next_v;
            }
        }
        beam.push(playsrc_beam::Segment{position:previous.position,width:previous.width,color:previous.color,texture_coordinate:previous_v});
        let mesh=beam.finish();
        debug_assert_eq!(mesh.vertices.len(), vertices);
        self.points = Vec::new();
        self.mesh = Some(mesh);
        Ok(())
    }
}

fn catmull_rom(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], t: f32) -> [f32; 3] {
    let t2 = t * t;
    let t3 = t * t2;
    std::array::from_fn(|axis| {
        0.5 * ((2.0 * b[axis])
            + (-a[axis] + c[axis]) * t
            + (2.0 * a[axis] - 5.0 * b[axis] + 4.0 * c[axis] - d[axis]) * t2
            + (-a[axis] + 3.0 * b[axis] - 3.0 * c[axis] + d[axis]) * t3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ribbon_subdivisions_interpolate_width_color_and_mapping_length() {
        let mut rope = RopeRender {
            points: vec![
                RopePoint {
                    position: [0.0; 3],
                    width: 2.0,
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                RopePoint {
                    position: [12.0, 0.0, 0.0],
                    width: 4.0,
                    color: [0.0, 1.0, 0.0, 0.0],
                },
            ],
            subdivisions: 3,
            texel_size: 4.0,
            scroll_offset: 8.0,
            camera: [0.0, 0.0, 10.0],
            mesh: None,
        };
        rope.resolve(2).unwrap();
        let mesh = rope.mesh.as_ref().unwrap();
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(
            mesh.indices,
            [0, 1, 2, 1, 3, 2, 2, 3, 4, 3, 5, 4, 4, 5, 6, 5, 7, 6]
        );
        assert_eq!(mesh.vertices[0].position, [0.0, -1.0, 0.0]);
        assert_eq!(mesh.vertices[7].position, [12.0, 2.0, 0.0]);
        assert_eq!(mesh.vertices[0].uv, [0.0, 1.0]);
        assert_eq!(mesh.vertices[7].uv, [1.0, 2.5]);
        assert_eq!(mesh.vertices[2].color, [170, 85, 0, 170]);
        assert!((mesh.vertices[2].position[0] - 3.5555556).abs() < 0.00001);
        assert!(rope.points.is_empty());
        let before = rope.mesh.clone();
        rope.resolve(2).unwrap();
        assert_eq!(rope.mesh, before);
    }

    #[test]
    fn degenerate_camera_alignment_is_finite_and_counts_are_checked() {
        let mut rope = RopeRender {
            points: vec![
                RopePoint {
                    position: [0.0; 3],
                    width: 1.0,
                    color: [1.0; 4]
                };
                2
            ],
            subdivisions: 1,
            texel_size: 4.0,
            scroll_offset: 0.0,
            camera: [0.0; 3],
            mesh: None,
        };
        rope.resolve(1).unwrap();
        assert!(
            rope.mesh
                .unwrap()
                .vertices
                .iter()
                .all(|vertex| vertex.position == [0.0; 3])
        );
        assert_eq!(RopeRender::vertex_count(64, 3), Some(380));
        assert_eq!(RopeRender::vertex_count(usize::MAX, usize::MAX), None);
    }
}
