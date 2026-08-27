use playsrc_entity::{
    EntityWorld, EntityWorldConfig, ModelBounds, RuntimeFailureCode, Transform, WorldCommand,
};

#[test]
fn invisible_pusher_world_transform_is_published_for_each_authored_visible_child() {
    let graph = playsrc_entity::parse(br#"
{"classname" "func_door" "targetname" "door" "model" "*1" "origin" "128 262 784" "rendermode" "10" "movedir" "-90 0 0"}
{"classname" "prop_dynamic" "parentname" "door" "origin" "128 260 784" "model" "models/props_gameplay/door_slide_door.mdl"}
{"classname" "prop_dynamic" "parentname" "door" "origin" "128 264 784" "model" "models/props_gameplay/door_slide_door.mdl"}
"#, playsrc_entity::Limits::default()).unwrap();
    let (mut world, _) = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            model_bounds: vec![ModelBounds {
                model: 1,
                mins: [-64.0, -4.0, -64.0],
                maxs: [64.0, 4.0, 64.0],
            }],
            ..EntityWorldConfig::default()
        },
    )
    .unwrap();
    let door = world.resolve(b"door", None, None, None)[0];
    let closed = world.snapshot().unwrap();
    let old_revision = world.revision();
    for (tick, z) in [784.0, 820.0, 908.0, 856.0, 856.0, 884.0, 784.0]
        .into_iter()
        .enumerate()
    {
        world
            .phase(
                tick as u64 + 1,
                &[WorldCommand::SetWorldTransform {
                    entity: door,
                    transform: Transform {
                        origin: [128.0, 262.0, z],
                        angles: [0.0; 3],
                    },
                }],
            )
            .unwrap();
        assert!(
            !world
                .brush_model_presentation(world.revision())
                .unwrap()
                .models[0]
                .draw
        );
        let studio = world.studio_model_presentation(world.revision()).unwrap();
        assert_eq!(studio.len(), 2);
        assert_eq!(studio[0].world_transform.origin, [128.0, 260.0, z]);
        assert_eq!(studio[1].world_transform.origin, [128.0, 264.0, z]);
        assert!(studio.iter().all(|model| model.draw));
    }
    assert_eq!(
        world
            .studio_model_presentation(old_revision)
            .unwrap_err()
            .code,
        RuntimeFailureCode::RevisionMismatch
    );
    world.restore(&closed).unwrap();
    assert_eq!(
        world.studio_model_presentation(old_revision).unwrap()[0]
            .world_transform
            .origin,
        [128.0, 260.0, 784.0]
    );
}
