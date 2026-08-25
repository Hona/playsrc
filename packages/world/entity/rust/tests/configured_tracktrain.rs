use playsrc_bsp::{Limits as BspLimits, LumpData, Profile, parse as parse_bsp};
use playsrc_entity::{
    BehaviorState, EntityWorld, EntityWorldConfig, EventTarget, InputRecord, Limits, ModelBounds,
    RuntimeRequest, Transition, Variant, WorldCommand, parse,
};
use std::{fs, path::PathBuf};

const UPWARD_SHA256: &str = "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709";

#[test]
#[ignore = "requires playsrc.local.json and the exact configured pl_upward BSP"]
fn configured_upward_track_graph_and_accelerating_cart_match_authored_source_entities() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let configuration = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let cache = configuration
        .split_once("\"sourceCacheDir\"")
        .unwrap()
        .1
        .split_once(':')
        .unwrap()
        .1
        .trim_start()
        .strip_prefix('"')
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let bytes = fs::read(
        PathBuf::from(cache)
            .join("objects/sha256")
            .join(&UPWARD_SHA256[..2])
            .join(UPWARD_SHA256),
    )
    .unwrap();
    assert_eq!(bytes.len(), 25_446_018);
    let bsp = parse_bsp(&bytes, Profile::Source2013V20, BspLimits::default()).unwrap();
    let graph = parse(bsp.lumps[0].bytes(&bsp), Limits::default()).unwrap();
    assert_eq!(
        graph.inventory.class_counts.get(b"path_track".as_slice()),
        Some(&175)
    );
    assert_eq!(
        graph
            .inventory
            .class_counts
            .get(b"func_tracktrain".as_slice()),
        Some(&1)
    );
    assert_eq!(
        graph
            .inventory
            .class_counts
            .get(b"phys_constraint".as_slice()),
        Some(&1)
    );

    let LumpData::Models(models) = &bsp.lumps[14].records else {
        panic!("configured BSP model records are unavailable")
    };
    let bounds = models
        .iter()
        .enumerate()
        .map(|(model, bounds)| ModelBounds {
            model,
            mins: [
                bounds.mins.x.value(),
                bounds.mins.y.value(),
                bounds.mins.z.value(),
            ],
            maxs: [
                bounds.maxs.x.value(),
                bounds.maxs.y.value(),
                bounds.maxs.z.value(),
            ],
        })
        .collect();
    let (mut world, _) = EntityWorld::compile(
        &graph,
        EntityWorldConfig {
            tick_interval: 0.015,
            model_bounds: bounds,
            ..EntityWorldConfig::default()
        },
    )
    .unwrap();
    let cart = world.resolve(b"minecart_tracktrain", None, None, None)[0];
    world.phase(0, &[]).unwrap();
    assert_eq!(
        world.entity(cart).unwrap().world_transform.origin,
        [-1664.0, -1536.0, 57.0]
    );
    assert_eq!(world.entity(cart).unwrap().world_transform.angles, [0.0; 3]);

    let mut current = world.resolve(b"minecart_path_1", None, None, None)[0];
    let mut distance = 0.0_f32;
    let mut links = 1;
    let mut checkpoints = Vec::new();
    loop {
        let entity = world.entity(current).unwrap();
        let BehaviorState::PathNode(node) = &entity.behavior else {
            panic!("configured path node behavior is unavailable")
        };
        let Some(next) = node.next else {
            break;
        };
        let following = world.entity(next).unwrap();
        distance += entity
            .world_transform
            .origin
            .into_iter()
            .zip(following.world_transform.origin)
            .map(|(first, second)| (second - first).powi(2))
            .sum::<f32>()
            .sqrt();
        links += 1;
        if matches!(
            following.targetname.as_deref(),
            Some(
                b"minecart_path_44"
                    | b"minecart_path_79"
                    | b"minecart_path_135"
                    | b"minecart_path_174"
            )
        ) {
            checkpoints.push(distance);
        }
        current = next;
    }
    assert_eq!(links, 175);
    for (actual, expected) in
        checkpoints
            .into_iter()
            .zip([3_165.862_3, 6_631.875, 11_237.307, 14_447.085])
    {
        assert!(
            (actual - expected).abs() < 0.01,
            "checkpoint {actual} != {expected}"
        );
    }

    let started = world
        .phase(
            0,
            &[WorldCommand::Input(InputRecord {
                target: EventTarget::Direct(cart),
                input: b"SetSpeedDirAccel".to_vec(),
                value: Variant::float(0.55),
                activator: None,
                caller: None,
                output_action: None,
                producer_sequence: 1,
            })],
        )
        .unwrap();
    assert!(started.records.iter().any(|record| matches!(
        &record.transition,
        Transition::Request(RuntimeRequest::Mover { entity, speed, world_destination, .. })
            if *entity == cart && (*speed - 1.15).abs() < 0.000_01
                && world_destination[0] > -1664.0
    )));
    assert!(matches!(
        &world.entity(cart).unwrap().behavior,
        BehaviorState::Mover(mover) if mover.path.as_ref().is_some_and(|path|
            f32::from_bits(path.maximum_speed_bits) == 90.0
                && f32::from_bits(path.desired_speed_bits) == 49.5
                && f32::from_bits(path.acceleration_bits) == 70.0
                && f32::from_bits(path.deceleration_bits) == 150.0)
    ));
}
