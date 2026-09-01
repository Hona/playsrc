use crate::{CoreOrientation, EdgeId, FeatureTopology, TopologyError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactDescriptor {
    pub moving_point: usize,
    pub fixed_edge: EdgeId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionKnot {
    pub position: [f64; 3],
    pub orientation: [f64; 9],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoreTransformState {
    pub object_frame: crate::ObjectFrame,
    pub position: [f64; 3],
    pub prior_orientation: CoreOrientation,
    pub next_orientation: CoreOrientation,
    pub projection_velocity: [f32; 3],
    pub core_time: f64,
    pub environment_time: f64,
    pub inverse_step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactProjection {
    pub descriptor: ContactDescriptor,
    pub point: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredContactPlane {
    pub edge: EdgeId,
    pub normal: [f64; 3],
    pub origin: [f32; 3],
    pub scale: f32,
}

impl ProjectionKnot {
    pub fn from_core(
        position: [f64; 3],
        prior_orientation: CoreOrientation,
        next_orientation: CoreOrientation,
        projection_velocity: [f32; 3],
        core_time: f64,
        environment_time: f64,
        inverse_step: f32,
    ) -> Result<Self, TopologyError> {
        Ok(Self::sample_core(
            position,
            prior_orientation,
            next_orientation,
            projection_velocity,
            core_time,
            environment_time,
            inverse_step,
        )?
        .1)
    }

    pub(crate) fn sample_core(
        position: [f64; 3],
        prior_orientation: CoreOrientation,
        next_orientation: CoreOrientation,
        projection_velocity: [f32; 3],
        core_time: f64,
        environment_time: f64,
        inverse_step: f32,
    ) -> Result<(CoreOrientation, Self), TopologyError> {
        if !inverse_step.is_finite() {
            return Err(TopologyError::NonFiniteTransform { component: 12 });
        }
        if inverse_step <= 0.0 {
            return Err(TopologyError::InvalidTransformInterval);
        }
        if !core_time.is_finite() || !environment_time.is_finite() {
            return Err(TopologyError::NonFiniteTransform {
                component: if core_time.is_finite() { 8 } else { 7 },
            });
        }
        let elapsed = (environment_time - core_time) as f32;
        if !elapsed.is_finite() {
            return Err(TopologyError::NonFiniteTransform { component: 7 });
        }
        let orientation = if elapsed == 0.0 {
            prior_orientation
        } else {
            prior_orientation
                .interpolate(next_orientation, f64::from(elapsed * inverse_step))
                .map_err(|_| TopologyError::InvalidTransformOrientation)?
        };
        let transform = Self::from_cache(
            position,
            orientation,
            projection_velocity,
            core_time,
            environment_time,
        )?;
        Ok((orientation, transform))
    }

    pub fn from_cache(
        position: [f64; 3],
        orientation: CoreOrientation,
        projection_velocity: [f32; 3],
        core_time: f64,
        environment_time: f64,
    ) -> Result<Self, TopologyError> {
        if let Some(component) = position
            .iter()
            .chain(orientation.quaternion.iter())
            .chain([core_time, environment_time].iter())
            .position(|component| !component.is_finite())
        {
            return Err(TopologyError::NonFiniteTransform { component });
        }
        if let Some(component) = projection_velocity
            .iter()
            .position(|component| !component.is_finite())
        {
            return Err(TopologyError::NonFiniteTransform {
                component: component + 9,
            });
        }
        let elapsed = (environment_time - core_time) as f32;
        if !elapsed.is_finite() {
            return Err(TopologyError::NonFiniteTransform { component: 7 });
        }
        Ok(Self {
            position: if elapsed == 0.0 {
                position
            } else {
                std::array::from_fn(|axis| {
                    position[axis] + f64::from(projection_velocity[axis]) * f64::from(elapsed)
                })
            },
            orientation: orientation.matrix(),
        })
    }
}

impl AuthoredContactPlane {
    pub fn from_edge(topology: &FeatureTopology, identity: EdgeId) -> Result<Self, TopologyError> {
        let edge = topology.edge(identity)?;
        let next = topology.edge(topology.next(identity)?)?;
        let previous = topology.edge(topology.previous(identity)?)?;
        let origin = topology.points()[edge.start as usize];
        let along: [f64; 3] = std::array::from_fn(|axis| {
            f64::from(topology.points()[next.start as usize][axis]) - f64::from(origin[axis])
        });
        let across: [f64; 3] = std::array::from_fn(|axis| {
            f64::from(topology.points()[previous.start as usize][axis]) - f64::from(origin[axis])
        });
        let unscaled = [
            along[1] * across[2] - along[2] * across[1],
            along[2] * across[0] - along[0] * across[2],
            along[0] * across[1] - along[1] * across[0],
        ];
        let length = ((unscaled[1] * unscaled[1] + unscaled[0] * unscaled[0])
            + unscaled[2] * unscaled[2])
            .sqrt();
        if length == 0.0 {
            return Err(TopologyError::DegenerateFace { face: edge.face });
        }
        let scale = (1.0 / (length + f64::from(1.0e-18_f32))) as f32;
        Ok(Self {
            edge: identity,
            normal: unscaled.map(|component| component * f64::from(scale)),
            origin,
            scale,
        })
    }

    pub fn distance(self, point: [f64; 3]) -> Result<f32, TopologyError> {
        if let Some((axis, _)) = point
            .iter()
            .enumerate()
            .find(|(_, component)| !component.is_finite())
        {
            return Err(TopologyError::NonFiniteSeparation { axis });
        }
        let projected_point =
            (self.normal[1] * point[1] + self.normal[0] * point[0]) + self.normal[2] * point[2];
        let projected_origin = (self.normal[1] * f64::from(self.origin[1])
            + self.normal[0] * f64::from(self.origin[0]))
            + self.normal[2] * f64::from(self.origin[2]);
        Ok((projected_point - projected_origin) as f32)
    }

    pub fn contains(
        self,
        topology: &FeatureTopology,
        point: [f64; 3],
    ) -> Result<bool, TopologyError> {
        if let Some((axis, _)) = point
            .iter()
            .enumerate()
            .find(|(_, component)| !component.is_finite())
        {
            return Err(TopologyError::NonFiniteSeparation { axis });
        }
        let selected = topology.edge(self.edge)?;
        let first = topology
            .edge_id(selected.face * 3)
            .ok_or(TopologyError::InvalidEdge)?;
        let second = topology.next(first)?;
        let third = topology.previous(first)?;
        let first = topology.points()[topology.edge(first)?.start as usize].map(f64::from);
        let second = topology.points()[topology.edge(second)?.start as usize].map(f64::from);
        let third = topology.points()[topology.edge(third)?.start as usize].map(f64::from);
        let primary: [f64; 3] = std::array::from_fn(|axis| first[axis] - second[axis]);
        let secondary: [f64; 3] = std::array::from_fn(|axis| third[axis] - second[axis]);
        let offset: [f64; 3] = std::array::from_fn(|axis| point[axis] - second[axis]);
        let dot = |left: [f64; 3], right: [f64; 3]| {
            (left[1] * right[1] + left[0] * right[0]) + left[2] * right[2]
        };
        let primary_squared = dot(primary, primary);
        let secondary_squared = dot(secondary, secondary);
        let mutual = dot(primary, secondary);
        let determinant = primary_squared * secondary_squared - mutual * mutual;
        let first_coordinate =
            secondary_squared * dot(offset, primary) - dot(offset, secondary) * mutual;
        let second_coordinate =
            primary_squared * dot(offset, secondary) - dot(offset, primary) * mutual;
        let checks = [
            first_coordinate as f32,
            second_coordinate as f32,
            (determinant - first_coordinate - second_coordinate) as f32,
        ];
        Ok(checks
            .iter()
            .all(|value| value.to_bits() & 0x8000_0000 == 0))
    }
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
        if let Some(component) = knot
            .position
            .iter()
            .chain(knot.orientation.iter())
            .position(|component| !component.is_finite())
        {
            return Err(TopologyError::NonFiniteTransform { component });
        }
        Ok(ContactProjection {
            descriptor: self,
            point: std::array::from_fn(|axis| {
                knot.position[axis]
                    + ((knot.orientation[axis * 3] * point[0]
                        + knot.orientation[axis * 3 + 1] * point[1])
                        + knot.orientation[axis * 3 + 2] * point[2])
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthoredContactPlane, ContactDescriptor, CoreTransformState, ProjectionKnot};
    use crate::{AuthoredFace, CoreOrientation, FeatureTopology, TopologyError};

    fn topology(points: Vec<[f32; 3]>) -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            points,
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
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
            position: [10.5, 19.0, 30.25],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
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
        assert_eq!(
            descriptor(0).project(
                &moving,
                &fixed,
                ProjectionKnot {
                    position: [f64::NAN, 0.0, 0.0],
                    ..knot
                }
            ),
            Err(TopologyError::NonFiniteTransform { component: 0 })
        );
    }

    #[test]
    fn cached_transform_uses_retained_projection_velocity_and_binary32_elapsed() {
        let knot = ProjectionKnot::from_cache(
            [10.0, 20.0, 30.0],
            CoreOrientation {
                quaternion: [0.0, 0.0, 0.0, 1.0],
            },
            [2.0, -4.0, 1.0],
            1.0,
            1.25,
        )
        .unwrap();
        assert_eq!(knot.position, [10.5, 19.0, 30.25]);
        assert_eq!(
            knot.orientation,
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            ProjectionKnot::from_cache(
                [0.0; 3],
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                [2.0, -4.0, 1.0],
                2.0,
                1.0,
            )
            .unwrap()
            .position,
            [-2.0, 4.0, -1.0]
        );
        assert_eq!(
            ProjectionKnot::from_core(
                [10.0, 20.0, 30.0],
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                [2.0, -4.0, 1.0],
                1.0,
                1.25,
                4.0,
            )
            .unwrap(),
            knot
        );
        assert_eq!(
            ProjectionKnot::from_core(
                [0.0; 3],
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                CoreOrientation {
                    quaternion: [0.0, 0.0, 0.0, 1.0],
                },
                [0.0; 3],
                0.0,
                0.0,
                0.0,
            ),
            Err(TopologyError::InvalidTransformInterval)
        );
    }

    #[test]
    fn object_cache_retains_same_time_code_across_mutation_and_rejects_atomic_replacement() {
        use crate::{CacheActivity, TransformCache, TransformCacheError};
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        let state = CoreTransformState {
            object_frame: crate::ObjectFrame::identity(),
            position: [10.0, 20.0, 30.0],
            prior_orientation: orientation,
            next_orientation: orientation,
            projection_velocity: [2.0, -4.0, 1.0],
            core_time: 1.0,
            environment_time: 1.25,
            inverse_step: 4.0,
        };
        let mut cache = TransformCache::default();
        let original = cache
            .resolve(1, CacheActivity::Simulated, 41, state)
            .unwrap();
        let mutated = CoreTransformState {
            position: [100.0, 200.0, 300.0],
            projection_velocity: [9.0; 3],
            ..state
        };
        assert_eq!(
            cache
                .resolve(1, CacheActivity::Simulated, 41, mutated)
                .unwrap(),
            original
        );
        assert_eq!(cache.current(1), Some((41, original)));
        assert_eq!(
            cache.resolve(
                1,
                CacheActivity::Simulated,
                42,
                CoreTransformState {
                    inverse_step: 0.0,
                    ..mutated
                }
            ),
            Err(TransformCacheError::Projection(
                TopologyError::InvalidTransformInterval
            ))
        );
        assert_eq!(cache.current(1), Some((41, original)));
        let replacement = cache
            .resolve(1, CacheActivity::Simulated, 42, mutated)
            .unwrap();
        assert_ne!(replacement, original);
        let restored = cache.clone();
        cache.invalidate(1).unwrap();
        assert_eq!(cache.current(1), None);
        assert_eq!(restored.current(1), Some((42, replacement)));
    }

    #[test]
    fn authored_contact_plane_preserves_binary32_scale_before_binary64_projection() {
        let shape = topology(vec![
            [2.54, -0.0, 2.54],
            [-2.54, -0.0, 2.54],
            [2.54, -0.0, -2.54],
        ]);
        let plane = AuthoredContactPlane::from_edge(&shape, shape.edge_id(0).unwrap()).unwrap();
        assert_eq!(plane.scale.to_bits(), 0x3d1e_b867);
        assert_eq!(
            plane.normal.map(f64::to_bits),
            [
                0x8000_0000_0000_0000,
                0xbfef_ffff_fe21_48b1,
                0x8000_0000_0000_0000
            ]
        );
        assert_eq!(
            plane
                .distance([
                    1.123_463_611_113_104_4,
                    -0.006_347_459_057_654_256,
                    -6.804_853_629_663_891e-7
                ])
                .unwrap()
                .to_bits(),
            0x3bcf_fe58
        );
        assert_eq!(
            plane.distance([0.0, f64::NAN, 0.0]),
            Err(TopologyError::NonFiniteSeparation { axis: 1 })
        );
        assert!(plane.contains(&shape, [0.0, 0.0, 0.0]).unwrap());
        assert!(!plane.contains(&shape, [-1.0, 0.0, -2.0]).unwrap());
    }
}
