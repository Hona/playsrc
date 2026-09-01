use crate::{
    CollisionBody, ContactFeatureBinding, ContactSurface, ContactTolerances, FeatureTopology,
    FrictionImpulseLimit, ImpactContactPoint, ProjectionKnot, ResponseError,
    RetainedFrictionTransport, SingleContactNormal, SingleContactNormalResult, TangentAssembly,
    TangentBody, TangentEnergyTracker, TangentFrame, TangentImpulseSystem,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManifoldContact {
    pub id: u64,
    pub binding: ContactFeatureBinding,
    pub frame: TangentFrame,
    pub local_offset: [f32; 3],
    pub synchronized_offsets: [[f32; 3]; 2],
    pub response_coefficient: f32,
    pub previous_point: [f64; 3],
    pub retained: [f32; 2],
    pub energy: TangentEnergyTracker,
    pub last_update_time: f64,
    pub normal_force: f32,
    pub absorbed_energy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManifoldTangentResult {
    pub bodies: [Option<TangentBody>; 2],
    pub point: [f64; 3],
    pub world_offsets: [Option<[f64; 3]>; 2],
    pub retained: [f32; 2],
    pub current_velocity: [f32; 2],
    pub impulse: [f32; 2],
    pub magnitude_squared: f64,
    pub energy_change: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactResponseMass {
    pub local_offset: [f32; 3],
    pub inverse_inertia: [f32; 3],
    pub inverse_mass: f32,
}

impl ManifoldContact {
    pub fn clamp_retained(
        &mut self,
        limit: f32,
        friction: f32,
    ) -> Result<crate::RetainedFrictionClampResult, ResponseError> {
        if !friction.is_finite()
            || !self.normal_force.is_finite()
            || !self.absorbed_energy.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if friction < 0.0 {
            return Err(ResponseError::NegativeFriction);
        }
        if self.normal_force < 0.0 {
            return Err(ResponseError::NegativeNormalForce);
        }
        let result = crate::response::clamp_retained_friction(self.retained, limit)?;
        if let Some(inverse) = result.inverse_magnitude {
            let lost = (f64::from(inverse) * result.magnitude_squared) - f64::from(limit);
            let energy = (((lost * f64::from(friction)) * f64::from(self.normal_force))
                + f64::from(self.absorbed_energy)) as f32;
            if !energy.is_finite() {
                return Err(ResponseError::NonFinite);
            }
            self.absorbed_energy = energy;
            self.binding.request_feature_check();
        }
        self.retained = result.coordinates;
        Ok(result)
    }
    pub fn source_normal_force(self) -> f32 {
        self.normal_force * crate::units::INCHES_PER_METER
    }
    pub fn response_coefficient(
        endpoints: [Option<ContactResponseMass>; 2],
    ) -> Result<f32, ResponseError> {
        let mut masses = [None; 2];
        for (side, endpoint) in endpoints.into_iter().enumerate() {
            let Some(ContactResponseMass {
                local_offset,
                inverse_inertia,
                inverse_mass,
            }) = endpoint
            else {
                continue;
            };
            if local_offset
                .iter()
                .chain(inverse_inertia.iter())
                .any(|value| !value.is_finite())
                || !inverse_mass.is_finite()
            {
                return Err(ResponseError::NonFinite);
            }
            if inverse_mass <= 0.0 || inverse_inertia.iter().any(|value| *value <= 0.0) {
                return Err(ResponseError::NonPositiveMass);
            }
            let squared = local_offset.map(|value| value * value);
            let largest = ((squared[0] + squared[2]) * inverse_inertia[1])
                .max((squared[1] + squared[2]) * inverse_inertia[0])
                .max((squared[0] + squared[1]) * inverse_inertia[2]);
            let mass = 1.0 / (f64::from(largest) + f64::from(inverse_mass));
            if !mass.is_finite() {
                return Err(ResponseError::NonFinite);
            }
            masses[side] = Some(mass);
        }
        let mass = match masses {
            [Some(first), Some(second)] => (first * second) / (first + second),
            [Some(mass), None] | [None, Some(mass)] => mass,
            [None, None] => return Err(ResponseError::NonPositiveMass),
        };
        let coefficient = (1.0 / mass) as f32;
        if !coefficient.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        Ok(coefficient)
    }

    pub fn refresh(
        &mut self,
        bodies: [Option<TangentBody>; 2],
        current_time: f64,
        topologies: [&FeatureTopology; 2],
        poses: [ProjectionKnot; 2],
        tolerances: ContactTolerances,
    ) -> Result<ContactSurface, ResponseError> {
        if bodies.iter().all(Option::is_none) {
            return Err(ResponseError::NonPositiveMass);
        }
        for body in bodies.into_iter().flatten() {
            self.validate(body)?;
        }
        if !current_time.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if current_time < self.last_update_time {
            return Err(ResponseError::NegativeElapsed);
        }
        let mut binding = self.binding;
        let mut surface = binding.project(topologies, poses, tolerances)?;
        surface.clamp_penetration();
        let point = surface.point;
        let frame = surface.frame()?;
        let mut synchronized = [[0.0; 3]; 2];
        let mut velocities = [[0.0; 3]; 2];
        for (side, body) in bodies.into_iter().enumerate() {
            let Some(body) = body else {
                continue;
            };
            synchronized[side] =
                ImpactContactPoint::from_world(body.position, body.orientation, point)?;
            velocities[side] = CollisionBody {
                orientation: body.orientation,
                local_offset: synchronized[side],
                inverse_mass: body.inverse_mass,
                inverse_inertia: body.inverse_inertia,
                linear_velocity: body.linear_velocity,
                angular_velocity: body.angular_velocity,
            }
            .point_velocity();
        }
        let velocity = std::array::from_fn(|axis| velocities[0][axis] - velocities[1][axis]);
        let elapsed = (current_time - self.last_update_time) as f32;
        let retained = RetainedFrictionTransport {
            coordinates: self.retained,
            frame,
            point_velocity: velocity,
            elapsed: f64::from(elapsed),
        }
        .advance()?;
        self.synchronized_offsets = synchronized;
        self.retained = retained;
        self.previous_point = point;
        self.frame = frame;
        self.last_update_time = current_time;
        self.binding = binding;
        Ok(surface)
    }

    pub fn solve_normal(
        &mut self,
        endpoints: [Option<TangentBody>; 2],
        normal: [f32; 3],
        distance: f32,
        target_distance: f32,
        inverse_step: f32,
    ) -> Result<SingleContactNormalResult, ResponseError> {
        for endpoint in endpoints.into_iter().flatten() {
            self.validate(endpoint)?;
        }
        let result = SingleContactNormal {
            point: self.previous_point,
            normal,
            distance,
            target_distance,
            inverse_step,
            endpoints,
        }
        .solve()?;
        self.normal_force = result.normal_force;
        Ok(result)
    }

    pub fn solve_tangent(
        &mut self,
        bodies: [Option<TangentBody>; 2],
        timestep: f32,
        friction: f32,
    ) -> Result<Option<ManifoldTangentResult>, ResponseError> {
        for body in bodies.into_iter().flatten() {
            self.validate(body)?;
        }
        if !timestep.is_finite() || !friction.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        if friction < 0.0 {
            return Err(ResponseError::NegativeFriction);
        }
        let impulse_limit = FrictionImpulseLimit {
            normal_force: self.normal_force,
            friction,
            timestep,
        }
        .impulse()?;
        if impulse_limit < f64::from(1.0e-6_f32) {
            return Ok(None);
        }
        let point = self.previous_point;
        let world_offsets = bodies.map(|body| {
            body.map(|body| std::array::from_fn(|axis| point[axis] - body.position[axis]))
        });
        let assembly = TangentAssembly {
            bodies,
            point,
            frame: self.frame,
            retained: self.retained,
            timestep,
        }
        .assemble()?;
        let solved = TangentImpulseSystem {
            inverse_response: assembly.inverse_response,
            right_hand_side: assembly.right_hand_side,
            impulse_limit,
        }
        .solve()?;
        let bodies = assembly.apply(solved.impulse)?;
        let mut energy = self.energy;
        let transition = energy.advance(crate::TangentEnergySample {
            magnitude_squared: solved.magnitude_squared,
            retained: self.retained,
            timestep,
        })?;
        self.energy = energy;
        Ok(Some(ManifoldTangentResult {
            bodies,
            point,
            world_offsets,
            retained: self.retained,
            current_velocity: assembly.current_velocity,
            impulse: solved.impulse,
            magnitude_squared: solved.magnitude_squared,
            energy_change: transition.change,
        }))
    }

    fn validate(self, body: TangentBody) -> Result<(), ResponseError> {
        if self
            .local_offset
            .iter()
            .chain(self.synchronized_offsets.iter().flatten())
            .chain(self.retained.iter())
            .chain(self.frame.first.iter())
            .chain(self.frame.second.iter())
            .chain(body.linear_velocity.iter())
            .chain(body.angular_velocity.iter())
            .chain(body.inverse_inertia.iter())
            .any(|value| !value.is_finite())
            || self
                .previous_point
                .iter()
                .chain(body.position.iter())
                .chain(body.orientation.iter())
                .any(|value| !value.is_finite())
            || !self.last_update_time.is_finite()
            || !self.normal_force.is_finite()
            || !self.response_coefficient.is_finite()
            || !body.inverse_mass.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.response_coefficient < 0.0 {
            return Err(ResponseError::NegativeResponseCoefficient);
        }
        if self.normal_force < 0.0 {
            return Err(ResponseError::NegativeNormalForce);
        }
        if body.inverse_mass < 0.0 || body.inverse_inertia.iter().any(|value| *value < 0.0) {
            return Err(ResponseError::NonPositiveMass);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthoredFace, FeatureTopology};

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

    fn contact() -> (ManifoldContact, FeatureTopology) {
        let topology = topology(vec![[0.0, 0.1, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        let features = crate::SurfaceFeaturePair {
            first: crate::SurfaceFeature {
                edge: topology.edge_id(0).unwrap(),
                kind: crate::SurfaceFeatureKind::Vertex,
            },
            second: crate::SurfaceFeature {
                edge: topology.edge_id(0).unwrap(),
                kind: crate::SurfaceFeatureKind::Face,
            },
        };
        let contact = ManifoldContact {
            id: 0,
            binding: ContactFeatureBinding::new([&topology, &topology], features, true).unwrap(),
            frame: TangentFrame {
                first: [1.0, 0.0, 0.0],
                second: [0.0, 0.0, 1.0],
            },
            local_offset: [0.0, 0.1, 0.0],
            synchronized_offsets: [[0.0, 0.1, 0.0], [0.0; 3]],
            response_coefficient: 1.0,
            previous_point: [0.0, 0.1, 0.0],
            retained: [0.0; 2],
            energy: TangentEnergyTracker::default(),
            last_update_time: 0.0,
            normal_force: 1.0,
            absorbed_energy: 0.0,
        };
        (contact, topology)
    }

    #[test]
    fn response_coefficient_uses_the_largest_inertia_weighted_axis_pair() {
        let inverse = [58.337_39, 59.713_345, 58.337_39];
        assert_eq!(
            ManifoldContact::response_coefficient([
                Some(ContactResponseMass {
                    local_offset: [-0.055_984_9, -0.096_968_59, -2.447_177_6e-9],
                    inverse_inertia: inverse,
                    inverse_mass: 0.2
                }),
                None
            ])
            .unwrap()
            .to_bits(),
            0.931_388_5_f32.to_bits()
        );
        assert_eq!(
            ManifoldContact::response_coefficient([
                Some(ContactResponseMass {
                    local_offset: [-0.111_969_724, -5.903_435e-16, -4.894_352e-9],
                    inverse_inertia: inverse,
                    inverse_mass: 0.2
                }),
                None
            ])
            .unwrap()
            .to_bits(),
            0.948_639_3_f32.to_bits()
        );
    }

    #[test]
    fn admission_weights_combine_both_virtual_masses_without_an_extra_float_round() {
        let first = ContactResponseMass {
            local_offset: [0.0; 3],
            inverse_inertia: [1.0; 3],
            inverse_mass: 0.2,
        };
        let second = ContactResponseMass {
            inverse_mass: 0.1,
            ..first
        };
        assert_eq!(
            ManifoldContact::response_coefficient([Some(first), Some(second)])
                .unwrap()
                .to_bits(),
            0.3_f32.to_bits()
        );
        assert_eq!(
            ManifoldContact::response_coefficient([None, Some(first)])
                .unwrap()
                .to_bits(),
            0.2_f32.to_bits()
        );
        assert_eq!(
            ManifoldContact::response_coefficient([None, None]),
            Err(ResponseError::NonPositiveMass)
        );
        assert_eq!(
            ManifoldContact::response_coefficient([
                Some(ContactResponseMass {
                    inverse_mass: f32::NAN,
                    ..first
                }),
                Some(second)
            ]),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn paired_refresh_transports_relative_motion_and_keeps_both_synchronized_offsets() {
        let (mut paired, topology) = contact();
        let mut single = paired;
        let body = TangentBody {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            linear_velocity: [2.0, 0.0, 0.0],
            inverse_inertia: [1.0; 3],
            inverse_mass: 1.0,
        };
        let other = TangentBody {
            linear_velocity: [1.0, 0.0, 0.0],
            ..body
        };
        let pose = ProjectionKnot {
            position: body.position,
            orientation: body.orientation,
        };
        let poses = [
            ProjectionKnot {
                position: [0.5, 0.0, 0.0],
                ..pose
            },
            pose,
        ];
        let tolerances = ContactTolerances::from_gravity([0.0, 0.0, -800.0]).unwrap();
        paired
            .refresh(
                [Some(body), Some(other)],
                0.015,
                [&topology; 2],
                poses,
                tolerances,
            )
            .unwrap();
        single
            .refresh(
                [Some(other), None],
                0.015,
                [&topology; 2],
                poses,
                tolerances,
            )
            .unwrap();
        assert_eq!(
            paired.retained.map(f32::to_bits),
            single.retained.map(f32::to_bits)
        );
        assert_eq!(paired.synchronized_offsets, [[0.5, 0.1, 0.0]; 2]);
        assert_eq!(single.synchronized_offsets[1], [0.0; 3]);
        let before = paired;
        assert_eq!(
            paired.refresh([None, None], 0.03, [&topology; 2], poses, tolerances),
            Err(ResponseError::NonPositiveMass)
        );
        assert_eq!(paired, before);
    }

    #[test]
    fn stored_pressure_gates_tangent_work_without_clearing_retained_energy() {
        let (mut contact, _) = contact();
        let body = TangentBody {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            linear_velocity: [0.0; 3],
            inverse_inertia: [1.0; 3],
            inverse_mass: 1.0,
        };
        contact.normal_force = 0.0;
        contact.energy = TangentEnergyTracker::new(2.0).unwrap();
        let before = contact;
        assert_eq!(
            contact
                .solve_tangent([Some(body), None], 0.015, 1.0)
                .unwrap(),
            None
        );
        assert_eq!(contact, before);
        contact.normal_force = 1.0;
        let before = contact;
        assert_eq!(
            contact
                .solve_tangent(
                    [Some(body), None],
                    f32::from_bits(1.0e-6_f32.to_bits() - 1),
                    1.0
                )
                .unwrap(),
            None
        );
        assert_eq!(contact, before);
        let solved = contact
            .solve_tangent([Some(body), None], 1.0e-6_f32, 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(solved.energy_change, -2.0);
        assert_eq!(contact.energy.previous(), 0.0);
        assert_eq!(contact.normal_force, 1.0);
    }

    #[test]
    fn retained_contact_refresh_uses_cached_projection_and_rejects_time_reversal_atomically() {
        let (mut contact, topology) = contact();
        let body = TangentBody {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            linear_velocity: [2.0, 0.0, 0.0],
            inverse_inertia: [1.0; 3],
            inverse_mass: 0.2,
        };
        let cached = ProjectionKnot {
            position: [0.5, 0.0, 0.0],
            orientation: body.orientation,
        };
        let poses = [
            cached,
            ProjectionKnot {
                position: [0.0; 3],
                orientation: body.orientation,
            },
        ];
        let tolerances = ContactTolerances::from_gravity([0.0, 0.0, -800.0]).unwrap();
        contact
            .refresh(
                [Some(body), None],
                0.015,
                [&topology, &topology],
                poses,
                tolerances,
            )
            .unwrap();
        assert_eq!(contact.previous_point, [0.5, f64::from(0.1_f32), 0.0]);
        assert_eq!(contact.synchronized_offsets, [[0.5, 0.1, 0.0], [0.0; 3]]);
        let retained = contact;
        assert_eq!(
            contact.refresh(
                [Some(body), None],
                0.0,
                [&topology, &topology],
                poses,
                tolerances
            ),
            Err(ResponseError::NegativeElapsed)
        );
        assert_eq!(contact, retained);
        assert!(
            contact
                .solve_tangent([Some(body), None], 0.015, 0.8)
                .is_ok()
        );
    }

    #[test]
    fn edge_refresh_keeps_its_derived_world_point_through_tangent_solve() {
        let (mut contact, _) = contact();
        contact.local_offset = [99.0; 3];
        let body = TangentBody {
            position: [1.0, 1.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            linear_velocity: [0.0; 3],
            inverse_inertia: [1.0; 3],
            inverse_mass: 0.2,
        };
        let shapes = [
            topology(vec![[0.0; 3], [4.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            topology(vec![[3.0, 1.0, 2.0], [3.0, 3.0, 2.0], [4.0, 1.0, 2.0]]),
        ];
        contact.binding = ContactFeatureBinding::new(
            shapes.each_ref(),
            crate::SurfaceFeaturePair {
                first: crate::SurfaceFeature {
                    edge: shapes[0].edge_id(0).unwrap(),
                    kind: crate::SurfaceFeatureKind::Edge,
                },
                second: crate::SurfaceFeature {
                    edge: shapes[1].edge_id(0).unwrap(),
                    kind: crate::SurfaceFeatureKind::Edge,
                },
            },
            true,
        )
        .unwrap();
        let mut poses = [
            ProjectionKnot {
                position: [1.0, 2.0, 1.0],
                orientation: body.orientation,
            },
            ProjectionKnot {
                position: [0.0; 3],
                orientation: body.orientation,
            },
        ];
        let tolerances = ContactTolerances::from_gravity([0.0, 0.0, -800.0]).unwrap();
        let geometry = contact
            .refresh(
                [Some(body), None],
                0.015,
                shapes.each_ref(),
                poses,
                tolerances,
            )
            .unwrap();
        assert_eq!(geometry.point, [3.0, 2.0, 1.0]);
        assert_eq!(contact.previous_point, geometry.point);
        assert_eq!(contact.synchronized_offsets, [[2.0, 1.0, 1.0], [0.0; 3]]);
        assert_eq!(
            contact.frame,
            TangentFrame {
                first: [1.0, 0.0, 0.0],
                second: [0.0, 1.0, 0.0]
            }
        );
        let solved = contact
            .solve_tangent([Some(body), None], 0.015, 0.8)
            .unwrap()
            .unwrap();
        assert_eq!(solved.point, geometry.point);
        assert_eq!(solved.world_offsets, [Some([2.0, 1.0, 1.0]), None]);
        let before = contact;
        poses[0].position = [f64::NAN; 3];
        assert_eq!(
            contact.refresh(
                [Some(body), None],
                0.03,
                shapes.each_ref(),
                poses,
                tolerances,
            ),
            Err(ResponseError::Geometry(
                crate::FeatureWalkError::NonFiniteTransform
            ))
        );
        assert_eq!(contact, before);
    }
}
