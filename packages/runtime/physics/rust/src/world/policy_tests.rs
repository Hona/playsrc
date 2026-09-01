use super::*;

#[derive(Clone, Debug, PartialEq)]
struct StatefulPolicy {
    calls: usize,
}
impl CollisionSolver for StatefulPolicy {
    fn should_collide(&self, _: u64, _: u64) -> bool {
        true
    }
    fn should_solve_penetration(&mut self, _: u64, _: u64, _: f32) -> bool {
        self.calls += 1;
        self.calls % 2 == 1
    }
    fn should_freeze_object(&mut self, _: u64) -> bool {
        true
    }
    fn additional_collision_checks_this_tick(&mut self, _: i32) -> i32 {
        0
    }
    fn should_freeze_contacts(&mut self, _: &[u64]) -> bool {
        false
    }
}

#[test]
fn collision_policy_state_is_owned_by_clone_and_restored_with_the_world() {
    let (mut world, _) = super::tests::automatic_pair_world(false);
    world.set_collision_solver(Some(Box::new(StatefulPolicy { calls: 0 })));
    let snapshot = world.snapshot();
    let mut clone = world.clone();
    assert!(
        clone
            .collision_solver_mut()
            .unwrap()
            .should_solve_penetration(1, 2, 0.015)
    );
    assert_eq!(
        world
            .collision_solver()
            .unwrap()
            .as_any()
            .downcast_ref::<StatefulPolicy>()
            .unwrap()
            .calls,
        0
    );
    assert_ne!(clone.snapshot(), snapshot);
    clone.restore(snapshot.clone()).unwrap();
    assert_eq!(clone.snapshot(), snapshot);
    assert_eq!(
        clone
            .collision_solver()
            .unwrap()
            .as_any()
            .downcast_ref::<StatefulPolicy>()
            .unwrap()
            .calls,
        0
    );
    clone
        .collision_solver_mut()
        .unwrap()
        .as_any_mut()
        .downcast_mut::<StatefulPolicy>()
        .unwrap()
        .calls = 9;
    assert_eq!(world.snapshot(), snapshot);
}
