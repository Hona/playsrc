use super::*;
#[test]
fn static_objects_keep_zero_aerodynamics_without_changing_pinned_dynamic_bases() {
    let (mut world, _) = super::super::tests::automatic_pair_world(false);
    let static_body = world
        .bodies()
        .iter()
        .find(|body| body.kind() == BodyKind::Static)
        .unwrap();
    assert_eq!(static_body.physical.linear_drag_basis, [0.0; 3]);
    assert_eq!(static_body.physical.angular_drag_basis, [0.0; 3]);
    assert_eq!(static_body.drag, 0.0);
    let dynamic = world.body(1).unwrap().physical;
    world.set_motion_enabled(1, false).unwrap();
    assert_eq!(world.body(1).unwrap().physical, dynamic);
    let snapshot = world.snapshot();
    world.restore(snapshot.clone()).unwrap();
    assert_eq!(world.snapshot(), snapshot);
}
#[test]
fn shadow_attachment_retains_limits_and_rejects_bad_commands_atomically() {
    let (mut world, _) = super::super::tests::automatic_pair_world(false);
    let before = world.snapshot();
    assert_eq!(
        world.set_shadow(1, f32::NAN, 10000.0, false, false),
        Err(EnvironmentError::NonFinite)
    );
    assert_eq!(world.snapshot(), before);
    world.set_shadow(1, 10000.0, 10000.0, false, false).unwrap();
    assert_eq!(world.body(1).unwrap().physical.mass, 50000.0);
    assert_eq!(world.body(1).unwrap().physical.inertia, [1.0e18; 3]);
    assert_eq!(world.body(1).unwrap().material_index(), 0xf000);
    assert!(!world.gravity_enabled(1).unwrap());
    assert!(!world.drag_enabled(1).unwrap());
    let saved = world.snapshot();
    world.restore(saved.clone()).unwrap();
    assert_eq!(
        world.update_shadow(1, [f32::NAN, 0.0, 0.0], [0.0; 3], false, 0.015),
        Err(EnvironmentError::NonFinite)
    );
    assert_eq!(world.snapshot(), saved);
    world.set_shadow(1, 7.0, 11.0, true, true).unwrap();
    assert!(!world.shadows.bodies[&1].translation);
    assert!(!world.shadows.bodies[&1].rotation);
    assert_eq!(world.shadows.bodies[&1].state.maximum_linear_speed, 7.0);
    assert_eq!(world.shadows.bodies[&1].state.maximum_angular_speed, 11.0);
}
#[test]
fn removing_shadow_restores_core_properties_but_retains_drag_cache() {
    let (mut world, _) = super::super::tests::automatic_pair_world(false);
    let original = world.body(1).unwrap().physical;
    world.set_shadow(1, 10000.0, 10000.0, false, false).unwrap();
    let attached = world.body(1).unwrap().physical;
    world.remove_shadow(1).unwrap();
    let restored = world.body(1).unwrap().physical;
    assert_eq!(restored.mass, original.mass);
    assert_eq!(restored.inertia, original.inertia);
    assert_eq!(restored.linear_drag_basis, attached.linear_drag_basis);
    assert_eq!(restored.angular_drag_basis, attached.angular_drag_basis);
    assert!(world.gravity_enabled(1).unwrap());
    assert!(world.drag_enabled(1).unwrap());
    assert!(world.shadows.bodies.is_empty());
    let saved = world.snapshot();
    world.restore(saved.clone()).unwrap();
    world.remove_shadow(1).unwrap();
    assert_eq!(world.snapshot(), saved);
}
