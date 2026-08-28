use playsrc_entity::{EntityWorld, EntityWorldConfig, Limits, Variant, WorldCommand, parse, smokestack::{Systems, binding}};

fn world(source: &[u8]) -> (EntityWorld, Systems) {
    let graph = parse(source, Limits::default()).unwrap();
    let (world, _) = EntityWorld::compile(&graph, EntityWorldConfig { external_classes: vec![binding()], ..Default::default() }).unwrap();
    let mut systems = Systems::default();
    systems.synchronize(&world).unwrap();
    (world, systems)
}

#[test]
fn smokestack_authored_keys_inputs_and_generation_lifecycle() {
    let (mut world, mut systems) = world(br#"{ "classname" "env_smokestack" "targetname" "flame" "InitialState" "1"
        "StartSize" "10" "EndSize" "5" "Speed" "45" "Rate" "15" "JetLength" "32" "SpreadSpeed" "10"
        "Twist" "10" "Roll" "6" "BaseSpread" "0" "SmokeMaterial" "particle/smokesprites_0001"
        "rendercolor" "241 158 103" "renderamt" "255" "origin" "1 2 3" "angles" "10 20 30" }"#);
    let first = systems.presentation(&world).remove(0);
    assert!(first.emit);
    assert_eq!(first.color, [241, 158, 103, 255]);
    assert_eq!(first.transform.origin, [1.0, 2.0, 3.0]);
    assert_eq!(first.transform.angles, [10.0, 20.0, 30.0]);
    assert_eq!(first.parameters.material, "particle/smokesprites_0001");
    assert_eq!((first.parameters.speed, first.parameters.jet_length, first.parameters.rate), (45.0, 32.0, 15.0));
    for (input, value, emitting) in [(b"TurnOff".as_slice(), Variant::Void, false), (b"Start", Variant::Void, false),
        (b"Toggle", Variant::Void, true), (b"TurnOn", Variant::Void, true)] {
        systems.input(first.entity, input, &value);
        assert_eq!(systems.presentation(&world)[0].emit, emitting);
    }
    systems.input(first.entity, b"Rate", &Variant::String(b"7.5".to_vec()));
    systems.input(first.entity, b"Speed", &Variant::Integer(90));
    assert_eq!(systems.presentation(&world)[0].parameters.rate, 7.5);
    assert_eq!(systems.presentation(&world)[0].parameters.speed, 90.0);
    let definition = world.entity(first.entity).unwrap().definition.as_ref().clone();
    world.phase(1, &[WorldCommand::Remove(first.entity)]).unwrap();
    systems.synchronize(&world).unwrap();
    assert!(systems.presentation(&world).is_empty());
    world.phase(2, &[WorldCommand::Spawn(definition)]).unwrap();
    systems.synchronize(&world).unwrap();
    let next = systems.presentation(&world).remove(0);
    assert_ne!(next.entity, first.entity);
    assert!(next.emit);
    assert_eq!(next.parameters.rate, 15.0);
}

#[test]
fn smokestack_wind_key_order_and_constructor_defaults() {
    let (world, systems) = world(br#"{ "classname" "env_smokestack" "Wind" "1 2 3" "WindSpeed" "20" "WindAngle" "90" }
        { "classname" "env_smokestack" "WindSpeed" "20" "WindAngle" "90" "Wind" "1 2 3" }"#);
    let states = systems.presentation(&world);
    assert!(states[0].parameters.wind[0].abs() < 0.00001);
    assert_eq!(states[0].parameters.wind[1..], [20.0, 0.0]);
    assert_eq!(states[1].parameters.wind, [1.0, 2.0, 3.0]);
    assert_eq!(states[0].color, [0, 0, 0, 255]);
    assert!(!states[0].emit);
    assert_eq!(states[0].parameters.material, "particle/smokestack");
    assert_eq!(states[0].parameters.rate, 0.0);
}

#[test]
fn smokestack_lights_last_matching_entity_wins_and_normalizes_color() {
    let (world, systems) = world(br#"{ "classname" "env_smokestack" "targetname" "a" }
      { "classname" "env_particlelight" "PSName" "a" "Color" "255 128 64" "Intensity" "10" "origin" "1 2 3" }
      { "classname" "env_particlelight" "PSName" "A" "Color" "3 4 5" }
      { "classname" "env_particlelight" "PSName" "a" "Directional" "1" "Color" "10 20 30" "Intensity" "20" }
      { "classname" "env_particlelight" "PSName" "a" "Color" "0 255 0" "Intensity" "30" "origin" "4 5 6" }"#);
    let parameters = &systems.presentation(&world)[0].parameters;
    assert_eq!(parameters.ambient.color, [0.0, 1.0, 0.0]);
    assert_eq!(parameters.ambient.position, [4.0, 5.0, 6.0]);
    assert_eq!(parameters.ambient.intensity, 30.0);
    assert_eq!(parameters.directional.color, [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0]);
}
