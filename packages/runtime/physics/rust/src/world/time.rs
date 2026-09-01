use super::*;

impl PhysicsEnvironment {
    pub fn set_gravity(&mut self, gravity: [f32; 3]) -> Result<(), EnvironmentError> {
        let tolerances = ContactTolerances::from_gravity(gravity)?;
        self.config.gravity = gravity;
        self.tolerances = tolerances;
        Ok(())
    }
    pub fn set_air_density(&mut self, density: f32) -> Result<(), EnvironmentError> {
        let mut config = self.config;
        config.air_density = density;
        config.validate()?;
        self.config = config;
        Ok(())
    }
    pub fn set_performance_settings(
        &mut self,
        performance: PerformanceSettings,
    ) -> Result<(), EnvironmentError> {
        let mut config = self.config;
        config.performance = performance;
        config.validate()?;
        let ranges = crate::CollisionSearchRanges::new(
            config.timestep,
            performance.lookahead_world,
            performance.lookahead_bodies,
        )?;
        self.config = config;
        self.search_ranges = ranges;
        Ok(())
    }
    pub fn set_simulation_timestep(&mut self, timestep: f32) -> Result<(), EnvironmentError> {
        let mut clock = self.clock;
        clock.set_timestep(timestep)?;
        let search_ranges = crate::CollisionSearchRanges::new(
            timestep,
            self.config.performance.lookahead_world,
            self.config.performance.lookahead_bodies,
        )?;
        self.clock = clock;
        self.config.timestep = timestep;
        self.search_ranges = search_ranges;
        Ok(())
    }
    /// Resets Source simulation time without relocating bodies or resetting contact pressure.
    pub fn reset_simulation_clock(&mut self) -> Result<(), EnvironmentError> {
        let mut candidate = self.clone();
        let shift = candidate.clock.last_boundary();
        candidate.queue.shift_clock_origin(shift)?;
        for island in candidate.islands.active().to_vec() {
            let unit = candidate
                .islands
                .island(island)
                .ok_or(crate::IslandError::MissingIsland)?
                .clone();
            for controller in unit.controllers.iter().rev() {
                candidate.shift_contact_controller_time(controller.controller, shift)?;
            }
            for core in unit.cores.iter().rev() {
                let index = candidate.core_body_index(*core)?;
                let body = &mut candidate.bodies[index];
                body.core_time -= shift;
                body.movement_range.shift_clock_origin(shift);
                if let Some(retained) = &mut body.motion_phase {
                    retained.phase.start -= shift;
                    retained.phase.end -= shift;
                }
                body.publication_phase = body.motion_phase().map(|phase| (phase, 0.0));
            }
        }
        candidate.shift_pair_event_times(shift);
        for queued in &mut candidate.queued_collisions {
            if let QueuedCollisionInput::Pair { pair, predicted } = &mut queued.input {
                pair.start -= shift;
                pair.end -= shift;
                predicted.time -= shift;
            }
        }
        candidate.sleep_scheduler =
            crate::SleepScheduler::new(candidate.sleep_scheduler.countdown(), 1);
        candidate.clock.reset_simulation();
        candidate.callbacks.reset_publication();
        candidate.collisions.clear();
        candidate.friction_events.clear();
        candidate.clear_convex_observations();
        if let Some(values) = &mut candidate.collision_observations {
            values.clear();
        }
        if let Some(values) = &mut candidate.normal_observations {
            values.clear();
        }
        if let Some(values) = &mut candidate.tangent_observations {
            values.clear();
        }
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestep_changes_keep_the_scheduled_boundary_and_reject_invalid_inputs_atomically() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.simulate(world.config.timestep).unwrap();
        let next = world.clock.next_boundary();
        let current = world.time();
        world.set_simulation_timestep(0.001).unwrap();
        assert_eq!(world.clock.next_boundary(), next);
        assert_eq!(world.time(), current);
        let before = world.snapshot();
        for step in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(world.set_simulation_timestep(step).is_err());
            assert_eq!(world.snapshot(), before);
        }
        world.simulate(0.001).unwrap();
        assert!(world.time() < current);
        assert_eq!(world.clock.next_boundary(), next);
        let rewound = world.snapshot();
        world.restore(before).unwrap();
        world.simulate(0.001).unwrap();
        assert_eq!(world.snapshot(), rewound);
    }
    #[test]
    fn reset_rebases_active_phases_preserves_dormant_clocks_and_restores_fixed_scheduling() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.simulate(0.0075).unwrap();
        world.simulate(0.0075).unwrap();
        world.simulate(0.0075).unwrap();
        let dormant = world.body(2).unwrap().core_time;
        let before = world.snapshot();
        let code = world.clock().time_code();
        world.reset_simulation_clock().unwrap();
        let expected = world.snapshot();
        assert_eq!(world.time(), 0.0);
        assert_eq!(world.clock().time_code(), code.wrapping_add(3));
        assert_eq!(world.body(2).unwrap().core_time, dormant);
        assert_eq!(world.random_state(), 1);
        assert_eq!(world.queue.base(), 0.0);
        assert_eq!(
            world.clock.next_boundary(),
            f64::from(world.config.timestep)
        );
        world.restore(expected.clone()).unwrap();
        world.restore(before).unwrap();
        world.reset_simulation_clock().unwrap();
        assert_eq!(world.snapshot(), expected);
        world.simulate(world.config.timestep).unwrap();
        assert_eq!(
            world.time(),
            f64::from(world.config.timestep * crate::clock::FRAME_LOOKAHEAD)
        );
    }
    #[test]
    fn clock_reset_preserves_earlier_undelivered_command_notifications() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        world.set_object_event_reporting(true);
        world.simulate(world.config.timestep).unwrap();
        world.sleep(1).unwrap();
        world.reset_simulation_clock().unwrap();
        let before = world.snapshot();
        world.restore(before).unwrap();
        world.simulate(world.config.timestep).unwrap();
        assert!(
            world
                .callbacks()
                .iter()
                .any(|callback| callback.kind == PhysicsCallbackKind::Sleep)
        );
    }
}
