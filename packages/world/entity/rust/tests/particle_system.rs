use playsrc_entity::{EntityWorld, EntityWorldConfig, EventTarget, InputRecord, Variant, WorldCommand, parse, Limits, particle_system::{Systems, binding}};

#[test]
fn authored_particles_start_follow_stop_and_resolve_controls_at_restart() {
    let graph = parse(br#"{ "classname" "worldspawn" }
      { "classname" "info_particle_system" "targetname" "smoke" "effect_name" "smoke_test" "start_active" "1" "origin" "1 2 3" "angles" "0 90 0" "cpoint1" "target" "cpoint1_parent" "2" }
      { "classname" "info_target" "targetname" "target" "origin" "4 5 6" }"#, Limits::default()).unwrap();
    let (mut world, _) = EntityWorld::compile(&graph, EntityWorldConfig { external_classes: vec![binding()], ..Default::default() }).unwrap();
    let mut systems = Systems::from_world(&world, 0.0);
    let first = systems.presentation(&world);
    assert!(first[0].active);
    assert_eq!(first[0].definition, "smoke_test");
    assert_eq!(first[0].controls[0].transform.angles, [0.0, 90.0, 0.0]);
    assert_eq!(first[0].controls[1].transform.origin, [4.0, 5.0, 6.0]);
    assert_eq!(first[0].controls[1].parent, 2);
    let entity = first[0].entity;
    systems.input(&world, entity, b"Start", 2.0);
    assert_eq!(systems.presentation(&world)[0].started_seconds, 0.0);
    systems.input(&world, entity, b"Stop", 3.0);
    assert!(!systems.presentation(&world)[0].active);
    systems.input(&world, entity, b"Start", 4.0);
    assert_eq!(systems.presentation(&world)[0].started_seconds, 4.0);
    world.phase(1, &[WorldCommand::Input(InputRecord { target: EventTarget::Direct(entity), input: b"Kill".to_vec(), value: Variant::Void,
        activator: None, caller: None, output_action: None, producer_sequence: 0 })]).unwrap();
    assert!(systems.presentation(&world).is_empty());
}
