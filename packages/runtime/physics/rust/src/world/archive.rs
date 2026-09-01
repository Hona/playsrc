use super::*;

/// Getter-space rigid-object state and its immutable collision resource.
/// Unlike an environment snapshot, contacts and clocks are reconstructed.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyArchive {
    volume: f32,
    shape: Arc<PhysicsShape>,
    material: u32,
    kind: BodyKind,
    mass: f32,
    inertia: [f32; 3],
    position: [f32; 3],
    angles: [f32; 3],
    linear_velocity: [f32; 3],
    angular_velocity: [f32; 3],
    damping: [f32; 2],
    drag: f32,
    collisions: bool,
    gravity: bool,
    drag_enabled: bool,
    motion: bool,
    asleep: bool,
    callback_flags: u16,
}

impl PhysicsEnvironment {
    pub fn serialize_body(&self, identity: u64) -> Result<BodyArchive, EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let state = body.published()?;
        let inertia = body.physical.inertia;
        Ok(BodyArchive {
            volume: body.volume,
            shape: Arc::clone(&body.shape),
            material: body.material_index(),
            kind: body.kind,
            mass: body.physical.mass,
            inertia: [inertia[0].abs(), inertia[2].abs(), inertia[1].abs()],
            position: state.position,
            angles: state.angles,
            linear_velocity: state.linear_velocity,
            angular_velocity: state.angular_velocity,
            damping: [body.linear_damping, body.angular_damping],
            drag: body.drag,
            collisions: body.collisions_enabled,
            gravity: self.gravity_enabled(identity)?,
            drag_enabled: self.drag_enabled(identity)?,
            motion: body.motion_enabled,
            asleep: body.asleep,
            callback_flags: body.callback_flags,
        })
    }
    pub fn unserialize_body(
        &mut self,
        identity: u64,
        archive: &BodyArchive,
        enable_collisions: bool,
    ) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        let mut physical = PhysicalShape::from_archive(
            &archive.shape,
            archive.mass,
            [archive.inertia[0], archive.inertia[2], archive.inertia[1]],
        )?;
        if archive.kind == BodyKind::Static {
            physical = physical.static_drag_bases();
        }
        candidate.create_body_inner(BodyInput {
            volume: archive.volume,
            identity,
            shape: Arc::clone(&archive.shape),
            material: archive.material,
            kind: archive.kind,
            mass: archive.mass,
            inertia_factor: 1.0,
            rotational_inertia_limit: 0.03,
            position: archive.position,
            angles: archive.angles,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            linear_damping: archive.damping[0],
            angular_damping: archive.damping[1],
            drag: archive.drag,
            collisions_enabled: false,
            gravity_enabled: true,
            drag_enabled: false,
        })?;
        candidate.body_mut(identity)?.physical = physical;
        candidate.set_callback_flags(identity, archive.callback_flags)?;
        if archive.drag_enabled {
            candidate.set_drag_enabled(identity, true)?;
        }
        if !archive.motion {
            candidate.set_motion_enabled(identity, false)?;
        }
        if !archive.gravity {
            candidate.set_gravity_enabled(identity, false)?;
        }
        if archive.collisions {
            candidate.set_collisions_enabled(identity, true)?;
        }
        let squared =
            |value: [f32; 3]| (value[0] * value[0] + value[1] * value[1]) + value[2] * value[2];
        if squared(archive.linear_velocity) != 0.0 || squared(archive.angular_velocity) != 0.0 {
            candidate.set_velocity_instantaneous(
                identity,
                Some(archive.linear_velocity),
                Some(archive.angular_velocity),
            )?;
            if archive.asleep {
                candidate.sleep(identity)?;
            }
        }
        if !archive.asleep {
            candidate.wake(identity)?;
        }
        if archive.collisions && enable_collisions {
            candidate.set_collisions_enabled(identity, true)?;
        }
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_deletion_retains_hidden_core_until_fifo_cleanup_and_replays() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.enable_delete_queue(true);
        world.simulate(world.config.timestep).unwrap();
        let archive = world.serialize_body(1).unwrap();
        world.destroy_body(1).unwrap();
        assert_eq!(
            world
                .bodies()
                .iter()
                .map(|body| body.identity)
                .collect::<Vec<_>>(),
            [2]
        );
        assert!(world.body(1).unwrap().callback_flags & 0x0400 != 0);
        world.unserialize_body(3, &archive, true).unwrap();
        let before = world.snapshot();
        world.restore(before.clone()).unwrap();
        assert_eq!(
            world
                .bodies()
                .iter()
                .map(|body| body.identity)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        world.simulate(world.config.timestep).unwrap();
        assert!(world.body(1).is_none());
        let after = world.snapshot();
        world.restore(before.clone()).unwrap();
        world.simulate(world.config.timestep).unwrap();
        assert_eq!(world.snapshot(), after);
        world.restore(before).unwrap();
        world.enable_delete_queue(false);
        world.destroy_body(2).unwrap();
        assert_eq!(
            world
                .bodies()
                .iter()
                .map(|body| body.identity)
                .collect::<Vec<_>>(),
            [3]
        );
        world.cleanup_delete_list().unwrap();
        assert!(world.body(1).is_none());
        assert_eq!(world.bodies.len(), 1);
    }
    #[test]
    fn object_archive_reconstructs_ownership_and_rejects_corrupt_parameters_atomically() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.simulate(world.config.timestep).unwrap();
        let saved = world.serialize_body(1).unwrap();
        let before = world.snapshot();
        assert_eq!(
            world.unserialize_body(1, &saved, true),
            Err(EnvironmentError::DuplicateBody)
        );
        assert_eq!(world.snapshot(), before);
        let mut invalid = saved.clone();
        invalid.inertia[1] = f32::NAN;
        assert!(world.unserialize_body(3, &invalid, true).is_err());
        assert_eq!(world.snapshot(), before);
        let mut invalid = saved.clone();
        invalid.position[2] = f32::INFINITY;
        assert!(world.unserialize_body(3, &invalid, true).is_err());
        assert_eq!(world.snapshot(), before);
        world.destroy_body(1).unwrap();
        let removed = world.snapshot();
        world.unserialize_body(3, &saved, true).unwrap();
        let restored = world.snapshot();
        assert_ne!(
            world.body(3).unwrap().core_identity,
            before
                .bodies
                .iter()
                .find(|body| body.identity == 1)
                .unwrap()
                .core_identity
        );
        world.restore(restored.clone()).unwrap();
        world.restore(removed).unwrap();
        world.unserialize_body(3, &saved, true).unwrap();
        assert_eq!(world.snapshot(), restored);
    }
}
