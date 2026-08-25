use crate::{EdgeId, FeatureTopology, TopologyError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactDescriptor {
    pub moving_point: usize,
    pub fixed_edge: EdgeId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionKnot {
    pub position: [f64; 3],
    pub orientation: [f64; 4],
    pub linear_velocity: [f32; 3],
    pub duration: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactProjection {
    pub descriptor: ContactDescriptor,
    pub point: [f64; 3],
}

impl ContactDescriptor {
    pub fn project(
        self,
        moving: &FeatureTopology,
        fixed: &FeatureTopology,
        knot: ProjectionKnot,
    ) -> Result<ContactProjection, TopologyError> {
        fixed.edge(self.fixed_edge)?;
        let point = moving
            .points()
            .get(self.moving_point)
            .copied()
            .ok_or(TopologyError::InvalidEdge)?
            .map(f64::from);
        let [x, y, z, w] = knot.orientation;
        let twice_cross = [
            2.0 * (y * point[2] - z * point[1]),
            2.0 * (z * point[0] - x * point[2]),
            2.0 * (x * point[1] - y * point[0]),
        ];
        let cross = [
            y * twice_cross[2] - z * twice_cross[1],
            z * twice_cross[0] - x * twice_cross[2],
            x * twice_cross[1] - y * twice_cross[0],
        ];
        Ok(ContactProjection {
            descriptor: self,
            point: std::array::from_fn(|axis| {
                knot.position[axis]
                    + point[axis]
                    + w * twice_cross[axis]
                    + cross[axis]
                    + f64::from(knot.linear_velocity[axis]) * knot.duration
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ContactDescriptor, ProjectionKnot};
    use crate::{AuthoredFace, FeatureTopology};

    fn topology(points: Vec<[f32; 3]>) -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            points,
            &[
                AuthoredFace {
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn every_contact_in_one_phase_uses_the_same_translation_knot() {
        let moving = topology(vec![[1.0, 2.0, 3.0], [-2.0, 4.0, 1.0], [0.0, 0.0, 0.0]]);
        let fixed = topology(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let knot = ProjectionKnot {
            position: [10.0, 20.0, 30.0],
            orientation: [0.0, 0.0, 0.0, 1.0],
            linear_velocity: [2.0, -4.0, 1.0],
            duration: 0.25,
        };
        let descriptor = |moving_point| ContactDescriptor {
            moving_point,
            fixed_edge: fixed.edge_id(0).unwrap(),
        };
        assert_eq!(
            descriptor(0).project(&moving, &fixed, knot).unwrap().point,
            [11.5, 21.0, 33.25]
        );
        assert_eq!(
            descriptor(1).project(&moving, &fixed, knot).unwrap().point,
            [8.5, 23.0, 31.25]
        );
    }
}
