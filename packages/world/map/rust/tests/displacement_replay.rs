#![cfg(feature = "collision-replay")]

use playsrc_collision::{DisplacementInput, Hull, SurfaceIdentity, Trace, replay_diagnostics};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::Instant};

fn assert_bits(actual: Trace, expected: Trace) {
    assert_eq!(actual, expected);
    assert_eq!(actual.fraction.to_bits(), expected.fraction.to_bits());
    assert_eq!(
        actual.fraction_left_solid.to_bits(),
        expected.fraction_left_solid.to_bits()
    );
    assert_eq!(actual.end.map(f32::to_bits), expected.end.map(f32::to_bits));
    assert_eq!(
        actual
            .plane
            .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits())),
        expected
            .plane
            .map(|p| (p.normal.map(f32::to_bits), p.distance.to_bits()))
    );
}

#[test]
#[ignore = "requires the exact configured Upward BSP"]
fn configured_upward_displacement_traces_match_direct_reference() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    let bytes =
        fs::read(PathBuf::from(config["tf2Dir"].as_str().unwrap()).join("maps/pl_upward.bsp"))
            .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709"
    );
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )
    .unwrap();
    let map = playsrc_map::compile(&bsp, playsrc_map::LightingProfile::Hdr).unwrap();
    assert_eq!(map.collision_displacements.len(), 558);
    let world = playsrc_collision::compile(&bsp)
        .unwrap()
        .with_displacement_inputs(
            map.collision_displacements
                .iter()
                .map(|patch| DisplacementInput {
                    source: patch.source,
                    parent_face: patch.parent_face,
                    contents: patch.contents,
                    positions: patch.positions.clone(),
                    triangles: patch.triangles.clone(),
                    triangle_tags: patch.triangle_tags.clone(),
                    primary_surface: SurfaceIdentity {
                        registry: [23; 32],
                        index: patch.material as u32,
                    },
                    secondary_surface: Some(SurfaceIdentity {
                        registry: [23; 32],
                        index: patch.material as u32 + 10000,
                    }),
                    use_secondary_surface: patch.secondary_surface.clone(),
                })
                .collect(),
        )
        .unwrap();
    let identity = world.identity;
    println!(
        "displacementStorage={:?}",
        replay_diagnostics::displacement_storage(&world)
    );
    let mut random = 0x185189_u32;
    let mut sample = |range: f32| {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        (random as f32 / u32::MAX as f32 * 2.0 - 1.0) * range
    };
    let mut queries = Vec::new();
    for patch in &map.collision_displacements {
        for (index, triangle) in patch.triangles.iter().enumerate() {
            let points = triangle.map(|i| patch.positions[i as usize]);
            let center: [f32; 3] = std::array::from_fn(|axis| {
                (points[0][axis] + points[1][axis] + points[2][axis]) / 3.0
            });
            for case in 0..8 {
                let start = match case {
                    0 => points[index % 3],
                    1 => center,
                    _ => std::array::from_fn(|axis| center[axis] + sample(40.0)),
                };
                let delta = match case {
                    0 | 1 => [0.0; 3],
                    2 => [0.0, 0.0, -18.0],
                    3 => [0.0, 0.0, 18.0],
                    4 => [sample(64.0), sample(64.0), 1.0 / 32.0],
                    _ => std::array::from_fn(|_| sample(160.0)),
                };
                let end = std::array::from_fn(|axis| start[axis] + delta[axis]);
                let hull = match (index + case) % 4 {
                    0 => Hull {
                        mins: [0.0; 3],
                        maxs: [0.0; 3],
                    },
                    1 => Hull {
                        mins: [-24.0, -24.0, 0.0],
                        maxs: [24.0, 24.0, 82.0],
                    },
                    2 => Hull {
                        mins: [-24.0, -24.0, 0.0],
                        maxs: [24.0, 24.0, 62.0],
                    },
                    _ => Hull {
                        mins: std::array::from_fn(|_| -sample(32.0).abs()),
                        maxs: std::array::from_fn(|_| sample(32.0).abs()),
                    },
                };
                queries.push((start, end, hull));
            }
        }
    }
    let mut reference = Vec::with_capacity(queries.len());
    let mut totals = Vec::new();
    for direct in [true, false] {
        replay_diagnostics::select_displacement_reference(direct);
        let started = Instant::now();
        let mut hits = 0;
        for (index, &(start, end, hull)) in queries.iter().enumerate() {
            let result = world.trace_hull(start, end, hull, u32::MAX).unwrap();
            hits += usize::from(result.displacement.is_some());
            if direct {
                reference.push(result);
            } else {
                assert_bits(result, reference[index]);
            }
        }
        totals.push(replay_diagnostics::counters());
        println!(
            "direct={direct} traces={} displacementHits={hits} milliseconds={} counters={:?}",
            queries.len(),
            started.elapsed().as_secs_f64() * 1000.0,
            totals.last().unwrap()
        );
        assert!(hits > 1000);
    }
    assert_eq!(
        world.identity, identity,
        "acceleration is not source identity"
    );
    assert!(totals[1][7] < totals[0][7]);
    assert!(totals[1][9] < totals[0][9]);
    assert!(totals[1][10] > 0);
    replay_diagnostics::select_displacement_reference(false);
}
