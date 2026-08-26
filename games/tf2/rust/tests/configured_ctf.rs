use std::{fs, path::PathBuf};

use playsrc_collision::Hull;
use playsrc_entity::ModelBounds;
use playsrc_movement::{Error as MoveError, Trace, Tracer};
use playsrc_tf2::{ActorContact, GameplayWorld, MapRuntime, PlayerContactFacts, PlayerTeam, ctf};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct CaptureBrushes;

impl Tracer for CaptureBrushes {
    fn trace(
        &self,
        _start: [f32; 3],
        end: [f32; 3],
        _hull: Hull,
        _mask: u32,
    ) -> Result<Trace, MoveError> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }
}

impl GameplayWorld for CaptureBrushes {
    fn overlaps_model_hull(
        &self,
        model: usize,
        _origin: [f32; 3],
        position: [f32; 3],
        _hull: Hull,
    ) -> Result<bool, MoveError> {
        Ok(match model {
            25 => {
                (-600.0..=-400.0).contains(&position[0]) && (3276.0..=3456.0).contains(&position[1])
            }
            111 => {
                (400.345..=600.345).contains(&position[0])
                    && (-3456.0..=-3276.0).contains(&position[1])
            }
            _ => false,
        })
    }
}

#[derive(Clone)]
struct AuthoredBrushes(playsrc_collision::World);

impl Tracer for AuthoredBrushes {
    fn trace(
        &self,
        _start: [f32; 3],
        end: [f32; 3],
        _hull: Hull,
        _mask: u32,
    ) -> Result<Trace, MoveError> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }
}

impl GameplayWorld for AuthoredBrushes {
    fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, MoveError> {
        self.0
            .overlaps_model_hull(model, origin, position, hull)
            .map_err(|_| {
                MoveError::new(
                    playsrc_movement::Operation::Trace,
                    playsrc_movement::FailureKind::Malformed,
                    "authored trigger brush",
                )
            })
    }
}

fn actor(identity: u32, team: PlayerTeam, position: [f32; 3]) -> ctf::Actor {
    ctf::Actor::active(
        identity,
        team,
        position,
        Hull {
            mins: [-24.0, -24.0, 0.0],
            maxs: [24.0, 24.0, 82.0],
        },
    )
}

#[test]
#[ignore = "requires playsrc.local.json and the exact configured ctf_2fort BSP"]
fn configured_2fort_intelligence_capture_and_round_win_match_authored_entities() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let marker = "\"tf2Dir\"";
    let value = &config[config.find(marker).unwrap() + marker.len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    let bytes = fs::read(tf2.join("maps/ctf_2fort.bsp")).unwrap();
    assert_eq!(bytes.len(), 22_751_863);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "cbd191411c0be57099da73458167001ec80d58bf37c71cb3c36b2911b6e80fd7"
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let graph = playsrc_entity::parse(
        bsp.lump(0).unwrap().bytes(&bsp),
        playsrc_entity::Limits::default(),
    )
    .unwrap();
    let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else {
        panic!("configured ctf_2fort model lump is absent");
    };
    let bounds = models
        .iter()
        .enumerate()
        .map(|(model, value)| ModelBounds {
            model,
            mins: [
                value.mins.x.value(),
                value.mins.y.value(),
                value.mins.z.value(),
            ],
            maxs: [
                value.maxs.x.value(),
                value.maxs.y.value(),
                value.maxs.z.value(),
            ],
        })
        .collect();
    let mut map = MapRuntime::compile(&graph, 0.015, 0xcbd1_9141_1c0b_e570, bounds).unwrap();
    assert_eq!(map.counts().capture_flags, 2);
    assert_eq!(map.counts().capture_zones, 2);
    let world = map.objectives_mut().unwrap();
    let red = world
        .flags()
        .find(|flag| flag.team == PlayerTeam::Red)
        .unwrap();
    assert_eq!(red.home, [-488.66, 3348.51, -131.026]);
    assert_eq!(red.home_angles, [0.0, 120.0, 0.0]);
    assert_eq!(red.configured_return_seconds, 60);
    let blue = world
        .flags()
        .find(|flag| flag.team == PlayerTeam::Blue)
        .unwrap();
    let blue_identity = blue.identity;
    assert_eq!(blue.home, [489.005, -3348.51, -131.106]);
    assert_eq!(blue.home_angles, [0.0, 300.0, 0.0]);
    let red_zone = world
        .zones()
        .find(|zone| zone.team == Some(PlayerTeam::Red))
        .unwrap();
    assert_eq!(red_zone.model, 25);
    assert_eq!(red_zone.center, [-500.0, 3366.0, -98.0]);
    let blue_zone = world
        .zones()
        .find(|zone| zone.team == Some(PlayerTeam::Blue))
        .unwrap();
    assert_eq!(blue_zone.model, 111);
    assert!((blue_zone.center[0] - 500.3447).abs() < 0.001);

    for capture in 1..=3 {
        let steal = actor(1, PlayerTeam::Red, [489.005, -3348.51, -131.106]);
        let taken = world
            .advance(&CaptureBrushes, capture as f32, &[steal])
            .unwrap();
        assert!(taken.iter().any(|event| matches!(
            event,
            ctf::Event::Flag {
                kind: ctf::FlagEventKind::Pickup,
                home: Some(true),
                ..
            }
        )));
        assert_eq!(world.flag(blue_identity).unwrap().carrier, Some(1));
        let delivered = actor(1, PlayerTeam::Red, [-500.0, 3366.0, -98.0]);
        let events = world
            .advance(&CaptureBrushes, capture as f32 + 0.1, &[delivered])
            .unwrap();
        assert_eq!(world.scores().red_captures, capture);
        assert_eq!(
            world.flag(blue_identity).unwrap().status,
            ctf::FlagStatus::Home
        );
        assert_eq!(
            events
                .iter()
                .any(|event| matches!(event, ctf::Event::CaptureBonus { .. })),
            capture != 3
        );
    }
    assert_eq!(world.scores().winner, Some(PlayerTeam::Red));
}

#[test]
#[ignore = "requires playsrc.local.json and the exact configured ctf_2fort BSP"]
fn configured_2fort_bots_activate_only_their_real_team_spawn_door_triggers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let marker = "\"tf2Dir\"";
    let value = &config[config.find(marker).unwrap() + marker.len()..];
    let value = value[value.find(':').unwrap() + 1..].trim_start();
    let tf2 = PathBuf::from(&value[1..value[1..].find('"').unwrap() + 1]);
    let bytes = fs::read(tf2.join("maps/ctf_2fort.bsp")).unwrap();
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let graph = playsrc_entity::parse(
        bsp.lump(0).unwrap().bytes(&bsp),
        playsrc_entity::Limits::default(),
    )
    .unwrap();
    let playsrc_bsp::LumpData::Models(models) = &bsp.lump(14).unwrap().records else {
        panic!("configured ctf_2fort model lump is absent");
    };
    let bounds = models
        .iter()
        .enumerate()
        .map(|(model, value)| ModelBounds {
            model,
            mins: [
                value.mins.x.value(),
                value.mins.y.value(),
                value.mins.z.value(),
            ],
            maxs: [
                value.maxs.x.value(),
                value.maxs.y.value(),
                value.maxs.z.value(),
            ],
        })
        .collect();
    let collision = AuthoredBrushes(playsrc_collision::compile(&bsp).unwrap());
    let mut map = MapRuntime::compile(&graph, 0.015, 0xcbd1_9141_1c0b_e570, bounds).unwrap();
    let hull = Hull {
        mins: [-24.0, -24.0, 0.0],
        maxs: [24.0, 24.0, 82.0],
    };
    let actor = |identity, team, position| ActorContact {
        identity,
        position,
        hull,
        facts: PlayerContactFacts {
            team,
            class: 3,
            ..PlayerContactFacts::default()
        },
        alive: true,
    };
    let red = actor(2, 2, [-1239.0, 1862.0, 258.0]);
    let blue = actor(3, 3, [1239.0, -1840.0, 258.0]);
    let opened = map
        .contact_phase(
            &collision,
            0,
            [0.0, 0.0, 1000.0],
            hull,
            PlayerContactFacts::default(),
            &[red, blue],
        )
        .unwrap();
    let contacts = opened
        .events
        .iter()
        .filter(|event| event.kind == playsrc_tf2::EntityEventKind::Contact && event.accepted)
        .map(|event| event.subject.unwrap())
        .collect::<Vec<_>>();
    assert!(contacts.contains(&red.identity));
    assert!(contacts.contains(&blue.identity));
    assert!(opened.mover_requests.len() >= 4);
    assert!(opened.mover_requests.iter().all(|request| request.opening));
}
