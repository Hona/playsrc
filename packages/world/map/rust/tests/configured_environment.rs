use playsrc_bsp::{Limits as BspLimits, LumpData, Profile as BspProfile, parse as parse_bsp};
use playsrc_collision::{
    ObjectInput, ObjectRole, Snapshot, SnapshotLimits, SnapshotShape, Transform,
};
use playsrc_entity::{Graph, Limits as EntityLimits, parse as parse_entities};
use playsrc_map::{
    CubeFace, CubemapSelection, DependencyMetadata, DependencyRequest, DependencyResponse,
    DependencyRole, EnvironmentInputs, EnvironmentLimits, LightingProfile, MarkKind, MarkMaterial,
    MarkPlacement, MarkPlacementSnapshot, MarkStatus, MaterialBinding, ResolvedTexture,
    WaterViewInput, WaterViewPolicy, compile, compile_environment,
};
use playsrc_material::{
    HdrMode, Material, SelectionEnvironment, TextureDisposition, resolve_for_environment,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

const BSP_SHA256: &str = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959";
const BUNDLE_SHA256: &str = "6abb49c8f6ee46f58a80587a5506a41be97da7f8e6e30da52daec53d3102f8f0";
const MARK_STREAM_SHA256: &str = "dc240ad45952f19150071cf235b433dcd1d035fd3c2f3afad55e9bd1f84d26c7";
const RECEIVER_REVISION: u64 = 0x4d41_524b_5f52_3031;
const PLACEMENT_REVISION: u64 = 0x4d41_524b_5f50_3031;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    tf2_dir: String,
    source_cache_dir: String,
    asset_dir: String,
}

#[test]
#[ignore = "requires playsrc.local.json and the configured jump_beef source bundle"]
fn configured_environment_retains_collision_selected_marks_water_and_view_inputs() {
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
    let bundle_bytes = fs::read(cache.join("browser-bundles/jump_beef.psdb")).unwrap();
    assert_eq!(hex(&Sha256::digest(&bundle_bytes)), BUNDLE_SHA256);
    let bundle = parse_bundle(&bundle_bytes).unwrap();
    assert_eq!(bundle.len(), 321);

    let bsp = parse_bsp(&bsp_bytes, BspProfile::Source2013V20, BspLimits::default()).unwrap();
    let graph = parse_entities(bsp.lumps[0].bytes(&bsp), EntityLimits::default()).unwrap();
    let map = compile(&bsp, LightingProfile::Hdr).unwrap();
    let visibility = playsrc_visibility::compile(&bsp).unwrap();
    let collision = playsrc_collision::compile(&bsp).unwrap();
    let receiver_snapshot = receiver_snapshot(&map, &collision);
    let mark_placements = mark_placements(&graph);

    let materials = map
        .materials
        .iter()
        .map(|reference| {
            resolve_material(
                &reference.logical_path.to_ascii_lowercase(),
                &bundle,
                material_environment(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let bindings = map
        .materials
        .iter()
        .zip(&materials)
        .map(|(reference, material)| MaterialBinding {
            material_index: reference.index,
            material,
        })
        .collect::<Vec<_>>();
    let dependencies = environment_dependencies(&bsp, &graph, &bundle);
    let mark_materials = mark_materials(&graph, &bundle);
    let environment = compile_environment(
        &map,
        &bsp,
        EnvironmentInputs {
            logical_map_path: "maps/jump_beef.bsp",
            entities: &graph,
            visibility: &visibility,
            collision: &collision,
            receiver_snapshot: &receiver_snapshot,
            mark_placements: &mark_placements,
            materials: &bindings,
            dependent_materials: &[],
            mark_materials: &mark_materials,
            dependencies: &dependencies,
            limits: EnvironmentLimits::default(),
        },
    )
    .unwrap();

    let sky = environment.sky.as_ref().unwrap();
    assert_eq!(sky.faces.len(), 6);
    assert!(
        sky.faces
            .iter()
            .all(|face| face.encoding == playsrc_map::SkyEncoding::HdrRgbs)
    );

    assert_eq!(
        environment.marks.collision_world_identity,
        collision.identity
    );
    assert_eq!(
        environment.marks.receiver_snapshot_revision,
        RECEIVER_REVISION
    );
    assert_eq!(environment.marks.placement_revision, PLACEMENT_REVISION);
    assert_eq!(environment.marks.records.len(), 39);
    assert!(
        environment
            .marks
            .records
            .iter()
            .all(|record| record.kind == MarkKind::InfoDecal)
    );
    for (mark, entity, model) in [(220, 216_u64, 93), (221, 217, 94), (222, 218, 95)] {
        let record = environment
            .marks
            .records
            .iter()
            .find(|record| record.source_index == mark)
            .unwrap();
        let receiver = record.receiver.unwrap();
        assert_eq!(receiver.entity, Some(entity));
        assert_eq!(receiver.model, model);
        assert_eq!(receiver.parent_entity, None);
        assert!(!record.fragments.is_empty());
        assert!(
            record
                .fragments
                .iter()
                .all(|fragment| fragment.model == model)
        );
    }
    assert!(
        environment
            .marks
            .records
            .iter()
            .all(|record| record.fragments.iter().all(|fragment| record
                .receiver
                .is_none_or(|receiver| fragment.model == receiver.model)))
    );
    let dispositions = environment
        .marks
        .records
        .iter()
        .fold([0_usize; 4], |mut counts, record| {
            counts[match record.status {
                MarkStatus::Projected => 0,
                MarkStatus::Ineligible => 1,
                MarkStatus::Missing => 2,
                MarkStatus::Inert => 3,
            }] += 1;
            counts
        });
    assert_eq!(dispositions, [38, 0, 1, 0]);
    assert_eq!(environment.marks.fragment_count, 73);
    assert_eq!(environment.marks.vertex_count, 292);
    assert_eq!(
        mark_stream_identity(&environment.marks.records),
        MARK_STREAM_SHA256
    );

    assert_eq!(environment.water.surfaces.len(), 16);
    let above_surfaces = environment
        .water
        .surfaces
        .iter()
        .filter(|surface| surface.state.above_water)
        .collect::<Vec<_>>();
    let beneath_surfaces = environment
        .water
        .surfaces
        .iter()
        .filter(|surface| !surface.state.above_water)
        .collect::<Vec<_>>();
    assert_eq!(above_surfaces.len(), 8);
    assert_eq!(beneath_surfaces.len(), 8);
    assert!(above_surfaces.iter().all(|surface| {
        surface.bindings.environment == Some(CubemapSelection::Declared { sample: 0 })
            && surface.bindings.reflection
            && surface.bindings.refraction
    }));
    assert!(beneath_surfaces.iter().all(|surface| {
        surface.bindings.environment.is_none()
            && !surface.bindings.reflection
            && surface.bindings.refraction
    }));
    assert_eq!(environment.water.volumes.len(), 1);
    assert_eq!(
        environment
            .water
            .leaf_minimum_distance_to_water
            .as_ref()
            .unwrap()
            .len(),
        1_899
    );
    let water = &environment.water.volumes[0];
    assert_eq!(water.leaves, [663, 675, 886, 911]);
    assert_eq!(water.clusters, [188, 191, 252, 262]);
    assert_eq!(water.areas, [5]);
    assert_eq!(water.contents, 0x1000_0020);
    assert_eq!(water.surface_material, 9);
    assert_eq!(
        water.bottom_material,
        Some(playsrc_map::WaterMaterialIdentity::Map(13))
    );
    assert!(water.surface_state.above_water);
    assert!(!water.bottom_state.as_ref().unwrap().above_water);
    assert_eq!(
        water.surface_bindings.environment,
        Some(CubemapSelection::Declared { sample: 0 })
    );
    assert!(water.surface_bindings.reflection && water.surface_bindings.refraction);
    let bottom_bindings = water.bottom_bindings.as_ref().unwrap();
    assert_eq!(bottom_bindings.environment, None);
    assert!(!bottom_bindings.reflection && bottom_bindings.refraction);
    let policy = WaterViewPolicy {
        draw_water: true,
        expensive_supported: true,
        draw_reflection: true,
        draw_refraction: true,
        force_expensive: true,
        force_reflect_entities: false,
        fast_clipping: false,
        height_clipping: true,
        eye_water_epsilon: 1.0,
    };
    let underwater = environment
        .water
        .plan_view(
            &visibility,
            WaterViewInput {
                origin: [-4784.0, 3432.0, -2300.0],
                angles: [0.0; 3],
                eye_leaf: 663,
                qualified_visible_leaves: &[],
                near_plane_intersects_selected_volume: false,
                draw_sky_2d: true,
                policy,
            },
        )
        .unwrap();
    assert_eq!(underwater.render.environment, None);
    assert!(!underwater.render.reflect && underwater.render.refract);
    let above_leaf = visibility
        .leaves
        .iter()
        .enumerate()
        .find(|(_, leaf)| leaf.leaf_water_data_id < 0 && leaf.contents as u32 & 0x100 != 0)
        .map(|(index, _)| index)
        .unwrap();
    let above = environment
        .water
        .plan_view(
            &visibility,
            WaterViewInput {
                origin: [-4784.0, 3432.0, -2100.0],
                angles: [0.0; 3],
                eye_leaf: above_leaf,
                qualified_visible_leaves: &[663],
                near_plane_intersects_selected_volume: false,
                draw_sky_2d: true,
                policy,
            },
        )
        .unwrap();
    assert_eq!(
        above.render.environment,
        Some(CubemapSelection::Declared { sample: 0 })
    );
    assert!(above.render.reflect && above.render.refract);
    assert_eq!(environment.master_fog_controller, None);
    assert_eq!(environment.controllers.len(), 2);
}

fn receiver_snapshot(
    map: &playsrc_map::CanonicalMap,
    collision: &playsrc_collision::World,
) -> Snapshot {
    let records = map
        .brush_model_occurrences
        .iter()
        .filter(|occurrence| {
            matches!(
                occurrence.classname.as_slice(),
                b"func_door" | b"func_button" | b"func_movelinear"
            ) || occurrence.classname.eq_ignore_ascii_case(b"func_brush")
                && occurrence.start_disabled.as_deref() == Some(b"0")
                && occurrence.solidity.as_deref() != Some(b"1")
        })
        .map(|occurrence| ObjectInput {
            identity: occurrence.entity as u64,
            role: ObjectRole::Entity,
            enabled: true,
            transform: Transform {
                origin: occurrence.origin,
                angles: occurrence.angles,
            },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel {
                model: occurrence.model,
            },
        })
        .collect();
    let snapshot = Snapshot::compile(
        collision,
        RECEIVER_REVISION,
        records,
        SnapshotLimits::default(),
    )
    .unwrap();
    assert_eq!(snapshot.records().len(), 14);
    snapshot
}

fn mark_placements(graph: &Graph) -> MarkPlacementSnapshot {
    MarkPlacementSnapshot {
        revision: PLACEMENT_REVISION,
        placements: graph
            .entities
            .iter()
            .filter(|entity| class(entity, b"infodecal"))
            .map(|entity| MarkPlacement {
                entity: entity.index,
                world_origin: vector(field(entity, b"origin").unwrap()),
                parent_entity: entity.parentname.as_deref().and_then(|name| {
                    graph
                        .entities
                        .iter()
                        .find(|candidate| candidate.targetname.as_deref() == Some(name))
                        .map(|candidate| candidate.index)
                }),
            })
            .collect(),
    }
}

fn environment_dependencies(
    bsp: &playsrc_bsp::Bsp,
    graph: &Graph,
    bundle: &BTreeMap<String, Vec<u8>>,
) -> Vec<DependencyResponse> {
    let world = graph
        .entities
        .iter()
        .find(|entity| class(entity, b"worldspawn"))
        .unwrap();
    let sky = std::str::from_utf8(field(world, b"skyname").unwrap()).unwrap();
    let mut output = Vec::new();
    for (face, suffix) in [
        (CubeFace::Right, "rt"),
        (CubeFace::Left, "lf"),
        (CubeFace::Back, "bk"),
        (CubeFace::Front, "ft"),
        (CubeFace::Up, "up"),
        (CubeFace::Down, "dn"),
    ] {
        let path = format!("materials/skybox/{sky}_hdr{suffix}.vmt");
        let source = &bundle[&path];
        let material = resolve_material(&path, bundle, material_environment()).unwrap();
        let selected_textures = material
            .textures
            .iter()
            .filter(|texture| {
                material.selected_textures.contains(&texture.role)
                    && texture.disposition == TextureDisposition::Source
            })
            .map(|texture| {
                let logical_path = texture.logical_path.as_ref().unwrap().to_ascii_lowercase();
                ResolvedTexture {
                    sha256: Sha256::digest(&bundle[&logical_path]).into(),
                    logical_path,
                }
            })
            .collect();
        output.push(DependencyResponse {
            request: DependencyRequest {
                role: DependencyRole::SkyMaterial(face),
                profile: LightingProfile::Hdr,
                logical_path: path,
            },
            metadata: DependencyMetadata::SkyMaterial {
                source_sha256: Sha256::digest(source).into(),
                encoding: playsrc_map::SkyEncoding::HdrRgbs,
                selected_textures,
            },
        });
    }
    let LumpData::Cubemaps(cubemaps) = &bsp.lumps[42].records else {
        panic!("missing cubemaps")
    };
    for (index, cubemap) in cubemaps.iter().enumerate() {
        let path = format!(
            "materials/maps/jump_beef/c{}_{}_{}.hdr.vtf",
            cubemap.origin[0], cubemap.origin[1], cubemap.origin[2]
        );
        let source = &bundle[&path];
        let metadata = playsrc_vtf::inspect(
            source,
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .unwrap();
        output.push(DependencyResponse {
            request: DependencyRequest {
                role: DependencyRole::CubemapTexture { sample: index },
                profile: LightingProfile::Hdr,
                logical_path: path,
            },
            metadata: DependencyMetadata::CubemapTexture {
                source_sha256: Sha256::digest(source).into(),
                width: metadata.width,
                height: metadata.height,
                mip_count: metadata.mip_count,
                source_face_count: metadata.faces.len() as u8,
            },
        });
    }
    output
}

fn mark_materials(graph: &Graph, bundle: &BTreeMap<String, Vec<u8>>) -> Vec<MarkMaterial> {
    let mut output = BTreeMap::new();
    for entity in graph
        .entities
        .iter()
        .filter(|entity| class(entity, b"infodecal"))
    {
        let reference = field(entity, b"texture").unwrap().to_vec();
        let path = material_path(&reference).unwrap();
        let Some(source) = bundle.get(&path) else {
            continue;
        };
        let material = resolve_material(&path, bundle, material_environment()).unwrap();
        let texture = material
            .textures
            .iter()
            .find(|texture| {
                material.selected_textures.contains(&texture.role)
                    && texture.disposition == TextureDisposition::Source
            })
            .unwrap();
        let texture_path = texture.logical_path.as_ref().unwrap().to_ascii_lowercase();
        let metadata = playsrc_vtf::inspect(
            &bundle[&texture_path],
            playsrc_vtf::Dialect::Source2013Pc,
            playsrc_vtf::Limits::default(),
        )
        .unwrap();
        output
            .entry(reference.to_ascii_lowercase())
            .or_insert(MarkMaterial {
                reference,
                logical_path: path,
                source_sha256: Sha256::digest(source).into(),
                width: metadata.width,
                height: metadata.height,
                state: material.decal,
            });
    }
    output.into_values().collect()
}

fn resolve_material(
    identity: &str,
    bundle: &BTreeMap<String, Vec<u8>>,
    environment: SelectionEnvironment,
) -> Result<Material, ()> {
    let root = bundle.get(identity).ok_or(())?;
    let mut responses = Vec::new();
    loop {
        match playsrc_vmt::compose(
            root,
            identity.to_owned(),
            &responses,
            &playsrc_keyvalues::ConditionEnvironment::default(),
            playsrc_vmt::Limits::default(),
        )
        .map_err(|_| ())?
        {
            playsrc_vmt::Composition::Complete(document) => {
                return resolve_for_environment(&document, environment).map_err(|_| ());
            }
            playsrc_vmt::Composition::Needs(requests) => {
                for request in requests {
                    let canonical_identity = material_path(&request.target_token).ok_or(())?;
                    responses.push(playsrc_vmt::DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        bytes: Some(bundle.get(&canonical_identity).ok_or(())?.clone()),
                        canonical_identity,
                    });
                }
            }
        }
    }
}

fn material_environment() -> SelectionEnvironment {
    SelectionEnvironment {
        hdr_mode: HdrMode::Integer,
        ..SelectionEnvironment::default()
    }
}

fn material_path(reference: &[u8]) -> Option<String> {
    let reference = std::str::from_utf8(reference).ok()?.replace('\\', "/");
    let lower = reference.to_ascii_lowercase();
    let prefix = if lower.starts_with("materials/") {
        ""
    } else {
        "materials/"
    };
    let suffix = if lower.ends_with(".vmt") { "" } else { ".vmt" };
    Some(format!("{prefix}{reference}{suffix}").to_ascii_lowercase())
}

fn class(entity: &playsrc_entity::Entity, classname: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(classname))
}

fn field<'a>(entity: &'a playsrc_entity::Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn vector(value: &[u8]) -> [f32; 3] {
    let values = std::str::from_utf8(value)
        .unwrap()
        .split_ascii_whitespace()
        .map(|value| value.parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    [values[0], values[1], values[2]]
}

fn mark_stream_identity(records: &[playsrc_map::MarkRecord]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"playsrc-map-mark-stream-v2");
    digest.update((records.len() as u32).to_le_bytes());
    for record in records {
        digest.update((record.source_index as u32).to_le_bytes());
        digest.update([
            record.status as u8,
            record.kind as u8,
            record.activation as u8,
            record.lifetime as u8,
            u8::from(record.initially_enabled),
            u8::from(record.dynamic),
            u8::from(record.low_priority),
            0,
        ]);
        digest.update((record.material_path.len() as u32).to_le_bytes());
        digest.update(record.material_path.as_bytes());
        if let Some(receiver) = record.receiver {
            digest.update([1]);
            digest.update(receiver.entity.unwrap_or(0).to_le_bytes());
            digest.update((receiver.model as u32).to_le_bytes());
            for value in receiver
                .local_origin
                .into_iter()
                .chain(receiver.transform.origin)
                .chain(receiver.transform.angles)
            {
                digest.update(value.to_bits().to_le_bytes());
            }
        } else {
            digest.update([0]);
        }
        digest.update((record.target_faces.len() as u32).to_le_bytes());
        for face in &record.target_faces {
            digest.update((*face as u32).to_le_bytes());
        }
        digest.update((record.fragments.len() as u32).to_le_bytes());
        for fragment in &record.fragments {
            digest.update((fragment.model as u32).to_le_bytes());
            digest.update((fragment.face as u32).to_le_bytes());
            digest.update((fragment.positions.len() as u32).to_le_bytes());
            for value in fragment.positions.iter().flatten() {
                digest.update(value.to_bits().to_le_bytes());
            }
            digest.update((fragment.uv.len() as u32).to_le_bytes());
            for value in fragment.uv.iter().flatten() {
                digest.update(value.to_bits().to_le_bytes());
            }
            digest.update((fragment.triangles.len() as u32).to_le_bytes());
            for index in fragment.triangles.iter().flatten() {
                digest.update(index.to_le_bytes());
            }
        }
    }
    hex(&digest.finalize())
}

fn parse_bundle(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, &'static str> {
    if bytes.get(..4) != Some(b"PSDB") || u32_at(bytes, 4)? != 1 {
        return Err("invalid bundle header");
    }
    let count = u32_at(bytes, 8)? as usize;
    let mut offset = 12_usize;
    let mut files = BTreeMap::new();
    for _ in 0..count {
        let path = field_bytes(bytes, &mut offset, 65_536)?;
        let path = std::str::from_utf8(path).map_err(|_| "non-UTF-8 path")?;
        let data = field_bytes(bytes, &mut offset, 128 * 1024 * 1024)?.to_vec();
        if files.insert(path.to_owned(), data).is_some() {
            return Err("duplicate bundle path");
        }
    }
    (offset == bytes.len())
        .then_some(files)
        .ok_or("trailing bundle bytes")
}

fn field_bytes<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    maximum: usize,
) -> Result<&'a [u8], &'static str> {
    let length = u32_at(bytes, *offset)? as usize;
    *offset += 4;
    if length > maximum {
        return Err("field limit");
    }
    let end = offset.checked_add(length).ok_or("field overflow")?;
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
