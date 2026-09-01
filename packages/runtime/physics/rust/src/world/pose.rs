use super::*;

impl PhysicsEnvironment {
    /// Sets the Source object-space origin and Euler orientation without waking it.
    pub fn set_position(
        &mut self,
        identity: u64,
        position: [f32; 3],
        angles: [f32; 3],
        teleport: bool,
    ) -> Result<(), EnvironmentError> {
        let orientation = SourceAngleBasis::from_degrees(angles)?.object_orientation()?;
        self.change_object_pose(identity, internal_position(position), orientation, teleport)
    }

    /// Source row-major 3-by-4 object transform, with translation in inches.
    pub fn set_position_matrix(
        &mut self,
        identity: u64,
        matrix: [f32; 12],
        teleport: bool,
    ) -> Result<(), EnvironmentError> {
        let basis = SourceAngleBasis {
            matrix: std::array::from_fn(|axis| matrix[axis / 3 * 4 + axis % 3]),
        };
        self.change_object_pose(
            identity,
            internal_position([matrix[3], matrix[7], matrix[11]]),
            basis.object_orientation()?,
            teleport,
        )
    }

    pub(super) fn change_object_pose(
        &mut self,
        identity: u64,
        origin: [f64; 3],
        orientation: CoreOrientation,
        teleport: bool,
    ) -> Result<(), EnvironmentError> {
        if origin.iter().any(|value| !value.is_finite()) {
            return Err(EnvironmentError::NonFinite);
        }
        let mut candidate = self.clone();
        let body = candidate
            .body(identity)
            .ok_or(EnvironmentError::MissingBody)?;
        let reconnect = teleport && body.collisions_enabled;
        if reconnect {
            candidate.set_collisions_enabled(identity, false)?;
        }
        candidate.move_core_pose(identity, origin, orientation)?;
        if reconnect {
            candidate.set_collisions_enabled(identity, true)?;
        }
        *self = candidate;
        Ok(())
    }

    fn move_core_pose(
        &mut self,
        identity: u64,
        origin: [f64; 3],
        orientation: CoreOrientation,
    ) -> Result<(), EnvironmentError> {
        self.clock.visit(self.time())?;
        let now = self.time();
        let last = self.clock.last_boundary();
        let end = self.clock.next_boundary();
        let timestep = self.config.timestep;
        let body = self.body_mut(identity)?;
        let core = body.core_identity;
        let matrix = orientation.matrix();
        let position = if let Some(shift) = body.frame.shift() {
            let center = shift.map(|value| f64::from(-value));
            std::array::from_fn(|axis| {
                ((matrix[axis * 3] * center[0] + matrix[axis * 3 + 1] * center[1])
                    + matrix[axis * 3 + 2] * center[2])
                    + origin[axis]
            })
        } else {
            origin
        };
        let elapsed = (now - body.core_time) as f32;
        let inverse = if elapsed >= timestep {
            1.0_f32 / elapsed
        } else {
            (1.0 / f64::from(timestep)) as f32
        };
        let previous = body
            .motion_phase()
            .map_or(body.core_position, |phase| phase.position);
        let delta = std::array::from_fn::<_, 3, _>(|axis| position[axis] - previous[axis]);
        let distance =
            ((delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2]).sqrt() as f32;
        body.core_position = position;
        body.previous_core_position = position;
        body.orientation = orientation;
        body.previous_orientation = orientation;
        body.collision_orientation = Some(matrix);
        body.core_time = last;
        body.core_inverse_step = inverse;
        body.velocity.linear = [0.0; 3];
        body.motion_phase = (end > last && body.kind != BodyKind::Static && !body.asleep)
            .then_some(RetainedMotionPhase {
                phase: BodyMotionPhase {
                    position,
                    prior_orientation: orientation,
                    next_orientation: orientation,
                    projection_velocity: [0.0; 3],
                    start: last,
                    end,
                    inverse_step: inverse,
                },
                motion: CollisionMotion::stationary(),
            });
        body.publication_phase = None;
        body.movement_range.shift_position(distance)?;
        if self.islands.movement(core) == Some(crate::CoreMovement::Dormant) {
            self.transforms.invalidate(identity)?;
            self.move_dormant_pairs(core)?;
            self.recheck_invalid_body(core)?;
        }
        self.recheck_spatial(core)?;
        self.dispatch_ranges(&[core])?;
        self.body_mut(identity)?.collision_orientation = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_position_commands_never_publish_partial_spatial_or_contact_changes() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.simulate(world.clock().timestep()).unwrap();
        let before = world.snapshot();
        assert!(
            world
                .set_position(1, [f32::INFINITY, 0.0, 0.0], [0.0; 3], true)
                .is_err()
        );
        assert_eq!(world.snapshot(), before);
        assert!(
            world
                .set_position(1, [0.0; 3], [f32::NAN, 0.0, 0.0], false)
                .is_err()
        );
        assert_eq!(world.snapshot(), before);
        let mut matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        matrix[7] = f32::NAN;
        assert!(world.set_position_matrix(1, matrix, true).is_err());
        assert_eq!(world.snapshot(), before);
        assert_eq!(
            world.set_position(999, [0.0; 3], [0.0; 3], true),
            Err(EnvironmentError::MissingBody)
        );
        assert_eq!(world.snapshot(), before);
    }
    #[test]
    fn dormant_pose_mutations_preserve_sleep_and_replay_without_finite_motion_phases() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.sleep(1).unwrap();
        world.simulate(world.clock().timestep()).unwrap();
        let before = world.snapshot();
        world
            .set_position(1, [500.0, -100.0, 20.0], [30.0, 45.0, 0.0], true)
            .unwrap();
        assert!(world.body(1).unwrap().is_asleep());
        assert!(world.body(1).unwrap().motion_phase().is_none());
        let expected = world.snapshot();
        world.restore(before).unwrap();
        world
            .set_position(1, [500.0, -100.0, 20.0], [30.0, 45.0, 0.0], true)
            .unwrap();
        assert_eq!(world.snapshot(), expected);
        world.restore(expected).unwrap();
        world.simulate(world.clock().timestep()).unwrap();
        assert!(world.body(1).unwrap().is_asleep());
    }
}
