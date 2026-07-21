use playsrc_bsp::{Limits as BspLimits, Profile as BspProfile, parse as parse_bsp};
use playsrc_collision::{
    CONTENTS_GRATE, CONTENTS_TRANSLUCENT, Feature, Hit, Hull, MASK_PLAYERSOLID, ObjectInput,
    ObjectRole, PhysicsShape, SNAPSHOT_VERSION, Snapshot, SnapshotLimits, SnapshotRayRequest,
    SnapshotShape, SnapshotTraceRequest, TraceScope, Transform, World, compile,
};
use playsrc_phy::{Limits as PhyLimits, Profile as PhyProfile, parse_standalone};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const COLLISION_WORLD_SHA256: &str =
    "66d42c750648487669e1b9d7a1b36fc81e213624030f812667fb728ee61aa6ed";
const LOCKER_PHY_SHA256: &str = "c3ff7d83b9bf5cbab075ae814e3348194a5cb8e08f2092b89419bbad11b48a03";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and the configured TF2 target cache"]
fn configured_jump_beef_rocket_and_mover_inputs_are_queryable() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config: Config =
        serde_json::from_slice(&fs::read(root.join("playsrc.local.json")).unwrap()).unwrap();
    assert!(PathBuf::from(&config.tf2_dir).is_absolute());
    assert!(PathBuf::from(&config.asset_dir).is_absolute());
    let cache = PathBuf::from(config.source_cache_dir);
    assert!(cache.is_absolute());

    let bsp_bytes = fs::read(
        cache
            .join("objects/sha256")
            .join(&BSP_SHA256[..2])
            .join(BSP_SHA256),
    )
    .unwrap();
    assert_eq!(hex(&Sha256::digest(&bsp_bytes)), BSP_SHA256);
    let bsp = parse_bsp(&bsp_bytes, BspProfile::Source2013V20, BspLimits::default()).unwrap();
    let world = compile(&bsp).unwrap();
    assert_eq!(hex(&world.identity), COLLISION_WORLD_SHA256);
    assert_eq!(world.brushes.len(), 476);
    assert_eq!(world.models.len(), 123);
    assert_eq!(world.model_brushes.len(), 123);
    assert_eq!(world.model_contents.len(), 123);
    assert_eq!(
        world
            .sides
            .iter()
            .filter(|side| {
                usize::try_from(side.texture_info)
                    .ok()
                    .and_then(|index| world.texture_flags.get(index))
                    .is_some_and(|flags| flags & playsrc_collision::SURF_SKY != 0)
            })
            .count(),
        40
    );
    assert!(bsp.lumps[26].bytes(&bsp).is_empty());
    assert!(bsp.lumps[33].bytes(&bsp).is_empty());

    assert_eq!(world.model_brushes[109], [454]);
    assert_eq!(world.model_contents[109], 1);
    for model in [113, 117, 118] {
        assert_eq!(
            world.model_contents[model],
            CONTENTS_TRANSLUCENT | CONTENTS_GRATE
        );
    }
    let divider_and_non_solid_fences = Snapshot::compile(
        &world,
        0x4d41_505f_4252_5553,
        [
            (294, 109, true, [5300.0, 1244.0, -2662.3]),
            (307, 113, false, [13144.0, 628.0, -5160.0]),
            (322, 117, false, [-5400.0, 2248.0, -1337.98]),
            (323, 118, false, [-5400.0, 1464.0, -1337.98]),
        ]
        .into_iter()
        .map(|(identity, model, enabled, origin)| ObjectInput {
            identity,
            role: ObjectRole::Entity,
            enabled,
            transform: Transform {
                origin,
                angles: [0.0; 3],
            },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model },
        })
        .collect(),
        SnapshotLimits::default(),
    )
    .unwrap();
    assert_eq!(
        divider_and_non_solid_fences.world_identity(),
        world.identity
    );
    assert_eq!(divider_and_non_solid_fences.records().len(), 4);
    assert_eq!(
        divider_and_non_solid_fences
            .records()
            .iter()
            .map(|record| (record.identity, record.enabled, record.contents))
            .collect::<Vec<_>>(),
        [
            (294, true, 1),
            (307, false, CONTENTS_TRANSLUCENT | CONTENTS_GRATE),
            (322, false, CONTENTS_TRANSLUCENT | CONTENTS_GRATE),
            (323, false, CONTENTS_TRANSLUCENT | CONTENTS_GRATE),
        ]
    );
    let bytes = divider_and_non_solid_fences.snapshot_bytes().unwrap();
    assert_eq!(&bytes[..4], b"CSNP");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        SNAPSHOT_VERSION
    );
    assert_eq!(&bytes[8..40], &world.identity);
    for hull in [
        Hull {
            mins: [-24.0, -24.0, 0.0],
            maxs: [24.0, 24.0, 82.0],
        },
        Hull {
            mins: [-24.0, -24.0, 0.0],
            maxs: [24.0, 24.0, 62.0],
        },
    ] {
        let trace = world
            .trace_snapshot_hull(
                &divider_and_non_solid_fences,
                SnapshotTraceRequest {
                    start: [5300.0, 1100.0, -2662.3],
                    end: [5300.0, 1400.0, -2662.3],
                    hull,
                    mask: MASK_PLAYERSOLID,
                    scope: TraceScope::EntitiesOnly,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(trace.world, world.identity);
        assert_eq!(
            trace.snapshot,
            Some(divider_and_non_solid_fences.identity())
        );
        assert!(trace.fraction < 1.0);
        assert!(matches!(
            trace.hit,
            Some(Hit::Object {
                identity: 294,
                feature: Feature::Brush { model: 109, .. },
                ..
            })
        ));
    }

    let model = &world.models[26];
    let mins = vector(model.mins);
    let maxs = vector(model.maxs);
    let center = scale(add(mins, maxs), 0.5);
    let mover_origin = [11_545.3, 328.0, -3_308.0];
    let mover = Snapshot::compile(
        &world,
        1,
        vec![ObjectInput {
            identity: 67,
            role: ObjectRole::Entity,
            enabled: true,
            transform: Transform {
                origin: mover_origin,
                angles: [0.0; 3],
            },
            linear_velocity: [0.0, 75.0, 0.0],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model: 26 },
        }],
        SnapshotLimits::default(),
    )
    .unwrap();
    let start = add(mover_origin, [mins[0] - 16.0, center[1], center[2]]);
    let end = add(mover_origin, [maxs[0] + 16.0, center[1], center[2]]);
    let trace = world
        .trace_snapshot_ray(
            &mover,
            SnapshotRayRequest {
                start,
                end,
                mask: u32::MAX,
                scope: TraceScope::EntitiesOnly,
                ignored: &[],
            },
            |_| true,
        )
        .unwrap();
    assert!(matches!(
        trace.hit,
        Some(Hit::Object {
            identity: 67,
            feature: Feature::Brush { model: 26, .. },
            ..
        })
    ));

    let bundle_bytes = playsrc_asset_graph::read_resource_set(
        &cache.join("browser-bundles/jump_beef.graph.json"),
        None,
    )
    .unwrap();
    let bundle = parse_bundle(&bundle_bytes).unwrap();
    assert!(!bundle.contains_key("models/props_2fort/cow001_reference.phy"));
    assert!(!bundle.contains_key("models/props_2fort/frog.phy"));
    for required in [
        "models/player/soldier.phy",
        "models/player/items/soldier/soldier_viking.phy",
        "models/props_gameplay/resupply_locker.phy",
    ] {
        assert!(bundle.contains_key(required), "missing {required}");
    }
    let locker_bytes = &bundle["models/props_gameplay/resupply_locker.phy"];
    assert_eq!(hex(&Sha256::digest(locker_bytes)), LOCKER_PHY_SHA256);
    let locker = parse_standalone(
        locker_bytes,
        PhyProfile::SourcePcPolygon,
        PhyLimits::default(),
    )
    .unwrap();
    let shape = PhysicsShape::from_phy(2, &locker, 0, SnapshotLimits::default(), |_| 1).unwrap();
    assert!(shape.convex_count() > 0);
    let bounds = shape.local_bounds();
    let center = scale(add(bounds.mins, bounds.maxs), 0.5);
    let prop = Snapshot::compile(
        &World::empty(),
        2,
        vec![ObjectInput {
            identity: 315,
            role: ObjectRole::Entity,
            enabled: true,
            transform: Transform::IDENTITY,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::Physics(Arc::new(shape)),
        }],
        SnapshotLimits::default(),
    )
    .unwrap();
    let trace = World::empty()
        .trace_snapshot_ray(
            &prop,
            SnapshotRayRequest {
                start: [bounds.mins[0] - 16.0, center[1], center[2]],
                end: [bounds.maxs[0] + 16.0, center[1], center[2]],
                mask: 1,
                scope: TraceScope::Everything,
                ignored: &[],
            },
            |_| true,
        )
        .unwrap();
    assert!(matches!(
        trace.hit,
        Some(Hit::Object {
            identity: 315,
            feature: Feature::Convex { .. },
            ..
        })
    ));
}

fn parse_bundle(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, &'static str> {
    if bytes.get(..4) != Some(b"PSRE") || u32_at(bytes, 4)? != 1 {
        return Err("invalid bundle header");
    }
    let count = u32_at(bytes, 8)? as usize;
    if count > 4_096 {
        return Err("bundle count limit");
    }
    let mut offset = 12_usize;
    let mut files = BTreeMap::new();
    for _ in 0..count {
        let path = field(bytes, &mut offset, 65_536)?;
        let path = std::str::from_utf8(path).map_err(|_| "non-UTF-8 path")?;
        let data = field(bytes, &mut offset, 128 * 1024 * 1024)?.to_vec();
        if files.insert(path.to_owned(), data).is_some() {
            return Err("duplicate bundle path");
        }
    }
    if offset != bytes.len() {
        return Err("trailing bundle bytes");
    }
    Ok(files)
}

fn field<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    maximum: usize,
) -> Result<&'a [u8], &'static str> {
    let length = u32_at(bytes, *offset)? as usize;
    *offset += 4;
    if length > maximum {
        return Err("field limit");
    }
    let end = (*offset).checked_add(length).ok_or("field overflow")?;
    let value = bytes.get(*offset..end).ok_or("truncated field")?;
    *offset = end;
    Ok(value)
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or("truncated integer")?
            .try_into()
            .map_err(|_| "integer width")?,
    ))
}

fn vector(value: playsrc_bsp::Vector3) -> [f32; 3] {
    [value.x.value(), value.y.value(), value.z.value()]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
