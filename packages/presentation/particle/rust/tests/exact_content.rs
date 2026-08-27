use std::{collections::BTreeMap, fs, path::PathBuf};

use playsrc_particle::{
    AdvanceRequest, Bounds, CollisionQuery, CollisionResult, ControlPoint, DefinitionLookup, Error,
    Event, EventCommand, ParticleBlendFactor, ParticleBlendState, ParticleColorSpace,
    ParticleMaterial, ParticleMaterialShader, ParticleSheet, ParticleWorld, PcfSource, Registry,
    RegistryLimits, SheetFrame, SheetSequence, StopMode, TraceRequest, WorldLimits,
    encode_render_output, resolve_render_output,
};
fn configured_bundle() -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("particle package is inside the workspace")
        .to_owned();
    let configuration = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let source_cache = json_string_field(&configuration, "sourceCacheDir");
    playsrc_asset_graph::read_resource_set(
        &PathBuf::from(source_cache).join("browser-bundles/jump_beef.graph.json"),
        None,
    )
    .unwrap()
}

fn json_string_field(document: &str, field: &str) -> String {
    let key = format!("\"{field}\"");
    let after_key = document.split_once(&key).unwrap().1;
    let after_colon = after_key.split_once(':').unwrap().1.trim_start();
    assert!(after_colon.starts_with('"'));
    let mut output = String::new();
    let mut escaped = false;
    for value in after_colon[1..].chars() {
        if escaped {
            output.push(match value {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => panic!("unsupported local-configuration escape"),
            });
            escaped = false;
        } else if value == '\\' {
            escaped = true;
        } else if value == '"' {
            return output;
        } else {
            output.push(value);
        }
    }
    panic!("unterminated local-configuration string")
}

fn u32_at(bytes: &[u8], at: &mut usize) -> u32 {
    let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
    *at += 4;
    value
}

fn bytes_at<'a>(bytes: &'a [u8], at: &mut usize) -> &'a [u8] {
    let length = u32_at(bytes, at) as usize;
    let value = &bytes[*at..*at + length];
    *at += length;
    value
}

fn bundle(bytes: &[u8]) -> BTreeMap<String, &[u8]> {
    assert_eq!(&bytes[..4], b"PSRE");
    let mut at = 4;
    assert_eq!(u32_at(bytes, &mut at), 1);
    let count = u32_at(bytes, &mut at);
    let mut output = BTreeMap::new();
    for _ in 0..count {
        let path = std::str::from_utf8(bytes_at(bytes, &mut at)).unwrap();
        output.insert(path.to_owned(), bytes_at(bytes, &mut at));
    }
    assert_eq!(at, bytes.len());
    output
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> &'a [u8] {
        let value = &self.bytes[self.at..self.at + count];
        self.at += count;
        value
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take(4).try_into().unwrap())
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take(4).try_into().unwrap())
    }
}

fn particle_sheet(bytes: &[u8]) -> ParticleSheet {
    assert_eq!(&bytes[..4], b"VTF\0");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 7);
    let minor = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let resource = if minor < 3 {
        None
    } else {
        let resource_count = u32::from_le_bytes(bytes[68..72].try_into().unwrap()) as usize;
        let header_size = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(header_size, 80 + resource_count * 8);
        (0..resource_count).find_map(|index| {
            let offset = 80 + index * 8;
            if bytes[offset..offset + 3] == [0x10, 0, 0] && bytes[offset + 3] == 0 {
                let start =
                    u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
                let length =
                    u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap()) as usize;
                Some(&bytes[start + 4..start + 4 + length])
            } else {
                None
            }
        })
    };
    let Some(resource) = resource else {
        return ParticleSheet {
            sequences: BTreeMap::from([(
                0,
                SheetSequence {
                    clamp: true,
                    duration_seconds: 1.0,
                    frames: vec![SheetFrame {
                        duration_seconds: 1.0,
                        images: [[0.0, 0.0, 1.0, 1.0]; 4],
                    }],
                },
            )]),
        };
    };
    let mut reader = Reader {
        bytes: resource,
        at: 0,
    };
    let version = reader.u32();
    assert!(version <= 1);
    let count = reader.u32();
    let mut sequences = BTreeMap::new();
    for _ in 0..count {
        let identity = reader.i32();
        let clamp = match reader.u32() {
            0 => false,
            1 => true,
            _ => panic!("invalid sheet clamp"),
        };
        let frame_count = reader.u32();
        let duration_seconds = reader.f32();
        let mut frames = Vec::new();
        for _ in 0..frame_count {
            let frame_duration = reader.f32();
            let image_count = if version == 0 { 1 } else { 4 };
            let mut images = [[0.0; 4]; 4];
            for image in images.iter_mut().take(image_count) {
                *image = [reader.f32(), reader.f32(), reader.f32(), reader.f32()];
            }
            if image_count == 1 {
                let first = images[0];
                images.fill(first);
            }
            frames.push(SheetFrame {
                duration_seconds: frame_duration,
                images,
            });
        }
        assert!(
            sequences
                .insert(
                    identity,
                    SheetSequence {
                        clamp,
                        duration_seconds,
                        frames,
                    },
                )
                .is_none()
        );
    }
    assert_eq!(reader.at, resource.len());
    ParticleSheet { sequences }
}

fn material(
    shader: ParticleMaterialShader,
    destination: ParticleBlendFactor,
    sheet: ParticleSheet,
) -> ParticleMaterial {
    ParticleMaterial {
        shader,
        blend: ParticleBlendState {
            source: ParticleBlendFactor::SourceAlpha,
            destination,
        },
        color_space: ParticleColorSpace::SrgbTextureLinearTint,
        dual_sequence: false,
        sheet,
    }
}

fn target_materials(bundle: &BTreeMap<String, &[u8]>) -> BTreeMap<String, ParticleMaterial> {
    [
        (
            "effects/rocketrailsmoke.vmt",
            "materials/effects/smoke/smokelit.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/brightglow_y_nomodel.vmt",
            "materials/effects/brightglow_y.vtf",
            ParticleMaterialShader::MeshSprite,
            ParticleBlendFactor::One,
        ),
        (
            "effects/sc_brightglow_y_nomodel.vmt",
            "materials/effects/brightglow_y.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::One,
        ),
        (
            "effects/smokelit2/smoke2lit.vmt",
            "materials/effects/smokelit2/smoke2lit.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/sc_softglow.vmt",
            "materials/effects/softglow.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::One,
        ),
        (
            "effects/circle2.vmt",
            "materials/effects/circle2.vtf",
            ParticleMaterialShader::MeshSprite,
            ParticleBlendFactor::One,
        ),
        (
            "effects/softglow_translucent.vmt",
            "materials/effects/softglow_translucent.vtf",
            ParticleMaterialShader::MeshSprite,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/debris/debris_chunk.vmt",
            "materials/effects/debris/debris_chunk.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/softglow.vmt",
            "materials/effects/softglow.vtf",
            ParticleMaterialShader::MeshSprite,
            ParticleBlendFactor::One,
        ),
        (
            "particle/smoke1/smoke1.vmt",
            "materials/particle/smoke1/smoke1.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/circle4.vmt",
            "materials/effects/circle4.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
        (
            "effects/circle3.vmt",
            "materials/effects/circle3.vtf",
            ParticleMaterialShader::SpriteCard,
            ParticleBlendFactor::OneMinusSourceAlpha,
        ),
    ]
    .into_iter()
    .map(|(identity, texture, shader, destination)| {
        (
            identity.to_owned(),
            material(shader, destination, particle_sheet(bundle[texture])),
        )
    })
    .collect()
}

#[derive(Default)]
struct NoHit;

impl CollisionQuery for NoHit {
    fn trace_batch(&mut self, requests: &[TraceRequest]) -> Result<Vec<CollisionResult>, Error> {
        Ok(requests
            .iter()
            .map(|request| CollisionResult {
                identity: request.identity,
                fraction: 1.0,
                start_solid: false,
                normal: [0.0; 3],
            })
            .collect())
    }

    fn lighting_at(&mut self, _: [f32; 3]) -> Result<[u8; 3], Error> {
        Ok([192, 160, 128])
    }
}

fn update_digest(mut state: u64, bytes: &[u8]) -> u64 {
    for value in bytes {
        state ^= u64::from(*value);
        state = state.wrapping_mul(0x100_0000_01b3);
    }
    state
}

fn update_visual_region_digest(state: u64, bounds: Option<Bounds>) -> u64 {
    let mut bytes = [0_u8; 28];
    bytes[0..4].copy_from_slice(&u32::from(bounds.is_some()).to_le_bytes());
    if let Some(bounds) = bounds {
        for (index, value) in bounds.minimum.into_iter().chain(bounds.maximum).enumerate() {
            let offset = 4 + index * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    update_digest(state, &bytes)
}

fn expected_effect_count(root: &str, time: f32) -> usize {
    let death = match root {
        "rockettrail" | "ExplosionCore_Wall" | "ExplosionCore_MidAir" => 2.5,
        "rocketbackblast" => 2.0,
        "stickybombtrail_red" | "stickybombtrail_blue" => 3.0,
        "stickybomb_pulse_red" | "stickybomb_pulse_blue" => 1.0,
        "muzzle_pipelauncher" => 1.5,
        _ => unreachable!(),
    };
    usize::from(time < death)
}

#[test]
#[ignore = "requires the exact configured TF2 source bundle"]
fn exact_projectile_timelines_cover_every_output_field_through_cleanup() {
    let bytes = configured_bundle();
    let bundle = bundle(&bytes);
    let pcf_rows = [
        ("particles/rockettrail.pcf", 118_126, 0xf9eb_b5af_904c_c01a),
        (
            "particles/rocketbackblast.pcf",
            5_922,
            0x933a_2cc3_73c1_5a48,
        ),
        ("particles/stickybomb.pcf", 43_918, 0x42ab_97b6_df79_8cc5),
        ("particles/muzzle_flash.pcf", 83_923, 0x6c8b_cef3_dd98_b249),
        ("particles/explosion.pcf", 132_368, 0xbfb3_9363_2812_71ff),
    ];
    for &(path, expected_length, expected_digest) in &pcf_rows {
        assert_eq!(bundle[path].len(), expected_length, "{path}");
        assert_eq!(
            update_digest(0xcbf2_9ce4_8422_2325, bundle[path]),
            expected_digest,
            "{path}"
        );
    }
    for (path, expected_length, expected_digest) in [
        (
            "materials/effects/smoke/smokelit.vtf",
            176_420,
            0x07d7_e5cb_8cd8_832a,
        ),
        (
            "materials/effects/brightglow_y.vtf",
            11_144,
            0x459c_b266_cfb6_e168,
        ),
        (
            "materials/effects/softglow.vtf",
            11_168,
            0xb3ef_ff3d_b13d_21a3,
        ),
        (
            "materials/effects/softglow_translucent.vtf",
            22_112,
            0x666a_b1c8_f1dd_08d9,
        ),
        (
            "materials/effects/smokelit2/smoke2lit.vtf",
            175_440,
            0x16f2_14a1_c6b2_97c8,
        ),
        (
            "materials/effects/circle2.vtf",
            43_936,
            0x86f8_396d_f94f_4c2a,
        ),
        (
            "materials/effects/circle3.vtf",
            87_640,
            0x6eaf_66b7_ff29_b969,
        ),
        (
            "materials/effects/circle4.vtf",
            87_640,
            0x5993_e22b_4fcb_f2e7,
        ),
        (
            "materials/effects/debris/debris_chunk.vtf",
            44_460,
            0xebef_d021_3825_e12f,
        ),
        (
            "materials/particle/smoke1/smoke1.vtf",
            351_084,
            0xbf24_7f0c_cbb3_70ea,
        ),
    ] {
        assert_eq!(bundle[path].len(), expected_length, "{path}");
        assert_eq!(
            update_digest(0xcbf2_9ce4_8422_2325, bundle[path]),
            expected_digest,
            "{path}"
        );
    }
    let sources = pcf_rows.map(|(logical_path, _, _)| PcfSource {
        logical_path,
        bytes: bundle[logical_path],
    });
    let registry = Registry::from_pcf(&sources, RegistryLimits::default()).unwrap();
    let roots = [
        "rockettrail",
        "rocketbackblast",
        "stickybombtrail_red",
        "stickybombtrail_blue",
        "stickybomb_pulse_red",
        "stickybomb_pulse_blue",
        "muzzle_pipelauncher",
        "ExplosionCore_Wall",
        "ExplosionCore_MidAir",
    ];
    assert_eq!(
        registry
            .target_closure(&roots.map(DefinitionLookup::Name))
            .unwrap()
            .definitions
            .len(),
        33
    );
    let materials = target_materials(&bundle);
    let material_names = materials.keys().cloned().collect::<Vec<_>>();
    let times = [
        0.0_f32, 0.05, 0.1, 0.15, 0.2, 0.25, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0,
    ];
    let expected_counts = [
        [1, 20, 40, 61, 81, 90, 128, 195, 81, 3, 0, 0, 0, 0],
        [0, 15, 18, 16, 14, 13, 9, 6, 6, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 12, 76, 102, 102, 102, 26, 0, 0, 0],
        [0, 0, 0, 0, 0, 12, 76, 102, 102, 102, 26, 0, 0, 0],
        [0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 25, 30, 17, 17, 16, 14, 0, 0, 0, 0, 0, 0, 0],
        [0, 153, 149, 127, 111, 101, 79, 56, 27, 11, 0, 0, 0, 0],
        [0, 159, 155, 138, 119, 104, 77, 58, 26, 11, 0, 0, 0, 0],
    ];
    let mut complete_output = 0xcbf2_9ce4_8422_2325_u64;
    let mut visual_region_digests = Vec::new();
    for (root_index, root) in roots.into_iter().enumerate() {
        let mut world = ParticleWorld::new(&registry, &BTreeMap::new(), WorldLimits::default()).unwrap();
        let create = Event {
            identity: 1,
            timestamp_seconds: 0.0,
            source_order: 0,
            command: EventCommand::Create {
                effect_identity: 1,
                definition: root.to_owned(),
                seed: 1_337 + root_index as u64,
                owner_identity: None,
                control_points: vec![ControlPoint {
                    index: 0,
                    position: [10.0, 20.0, 30.0],
                    previous_position: [10.0, 20.0, 30.0],
                    orientation: [0.0, 0.0, 0.0, 1.0],
                    velocity: [0.0; 3],
                    radius: 0.0,
                    density: 1.0,
                    duration: 0.0,
                    parent: None,
                    object_identity: None,
                }],
            },
        };
        let mut from = 0.0;
        let mut visual_region_digest = 0xcbf2_9ce4_8422_2325_u64;
        for (time_index, to) in times.into_iter().enumerate() {
            let mut events = Vec::new();
            if time_index == 0 {
                events.push(create.clone());
            }
            if root == "rockettrail" && to == 1.0 {
                events.push(Event {
                    identity: 2,
                    timestamp_seconds: 1.0,
                    source_order: 0,
                    command: EventCommand::StopEmission {
                        effect_identity: 1,
                        mode: StopMode::Graceful,
                    },
                });
            }
            let (raw, bounds): (_, Option<Bounds>) = world
                .advance(
                    &events,
                    AdvanceRequest {
                        from_seconds: from,
                        to_seconds: to,
                        maximum_step_seconds: 0.05,
                        camera_position: [100.0, 50.0, 25.0],
                    },
                    &mut NoHit,
                )
                .unwrap();
            let items = resolve_render_output(raw, &materials).unwrap();
            assert_eq!(items.len(), expected_counts[root_index][time_index]);
            assert_eq!(world.effect_count(), expected_effect_count(root, to));
            visual_region_digest = update_visual_region_digest(visual_region_digest, bounds);
            complete_output = update_digest(
                complete_output,
                &encode_render_output(&items, bounds, &material_names, 64 * 1024 * 1024).unwrap(),
            );
            from = to;
        }
        visual_region_digests.push(visual_region_digest);
    }
    assert_eq!(
        visual_region_digests,
        [
            0xed87_0cbc_5ca3_f210,
            0x3926_0253_9c03_5e83,
            0x974a_dd1f_bfe1_6b10,
            0x974a_dd1f_bfe1_6b10,
            0x7a3c_e299_52dc_5472,
            0x7a3c_e299_52dc_5472,
            0x9fa3_0016_5611_aaca,
            0x5879_35bd_8124_e0f2,
            0x83e5_b31e_3de0_6424,
        ]
    );
    assert_eq!(complete_output, 0x79e5_a974_65e7_e0eb);
}

#[test]
#[ignore = "requires the exact configured TF2 explosion source bundle"]
fn configured_wall_explosion_stays_in_front_of_every_oriented_impact_plane() {
    let bytes = configured_bundle();
    let resources = bundle(&bytes);
    let registry = Registry::from_pcf(
        &[PcfSource {
            logical_path: "particles/explosion.pcf",
            bytes: resources["particles/explosion.pcf"],
        }],
        RegistryLimits::default(),
    )
    .unwrap();
    let closure = registry
        .target_closure(&[DefinitionLookup::Name("ExplosionCore_Wall")])
        .unwrap();
    let names = closure
        .definitions
        .iter()
        .map(|index| registry.definition_at(*index).unwrap().name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "ExplosionCore_Wall",
            "Explosion_Debris001",
            "Explosion_Dustup",
            "Explosion_Dustup_2",
            "Explosion_CoreFlash",
            "Explosion_FloatieEmbers",
            "Explosion_Smoke_1",
            "Explosion_Flash_1",
            "Explosion_FlyingEmbers",
            "Explosion_Flashup",
        ]
    );
    let flash = registry
        .definition(DefinitionLookup::Name("Explosion_Flash_1"))
        .unwrap();
    let flash_offset = flash
        .functions
        .iter()
        .find(|function| function.identity == "Position Modify Offset Random")
        .unwrap();
    assert_eq!(
        flash_offset.parameter("offset min"),
        Some(&playsrc_particle::Value::Vector3([50.0, 0.0, 0.0]))
    );
    assert_eq!(
        flash_offset.parameter("offset max"),
        Some(&playsrc_particle::Value::Vector3([50.0, 0.0, 0.0]))
    );
    assert_eq!(
        flash_offset.parameter("offset in local space 0/1"),
        Some(&playsrc_particle::Value::Bool(true))
    );

    let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
    for (orientation, normal) in [
        ([0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, quarter_turn, quarter_turn], [0.0, 1.0, 0.0]),
        ([0.0, -quarter_turn, 0.0, quarter_turn], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]),
    ] {
        let origin = [10.0, 20.0, 30.0];
        let wall: [f32; 3] = std::array::from_fn(|axis| origin[axis] - normal[axis]);
        let mut world = ParticleWorld::new(&registry, &BTreeMap::new(), WorldLimits::default()).unwrap();
        let (items, _) = world
            .advance(
                &[Event {
                    identity: 1,
                    timestamp_seconds: 0.0,
                    source_order: 0,
                    command: EventCommand::Create {
                        effect_identity: 1,
                        definition: "ExplosionCore_Wall".to_owned(),
                        seed: 1_337,
                        owner_identity: None,
                        control_points: vec![ControlPoint {
                            index: 0,
                            position: origin,
                            previous_position: origin,
                            orientation,
                            velocity: [0.0; 3],
                            radius: 0.0,
                            density: 1.0,
                            duration: 0.0,
                            parent: None,
                            object_identity: None,
                        }],
                    },
                }],
                AdvanceRequest {
                    from_seconds: 0.0,
                    to_seconds: 0.05,
                    maximum_step_seconds: 0.05,
                    camera_position: [100.0, 50.0, 25.0],
                },
                &mut NoHit,
            )
            .unwrap();
        assert!(!items.is_empty());
        let mut flashes = 0;
        for item in items {
            let signed_distance = (0..3)
                .map(|axis| (item.position[axis] - wall[axis]) * normal[axis])
                .sum::<f32>();
            let name = registry
                .definitions().iter().find(|definition| definition.uuid == item.system_uuid)
                .unwrap()
                .name
                .as_str();
            if name != "Explosion_FloatieEmbers" {
                assert!(
                    signed_distance >= -0.001,
                    "{name} crossed its impact plane: normal={normal:?}, position={:?}, signed={signed_distance}",
                    item.position
                );
            }
            if item.system_uuid == flash.uuid {
                flashes += 1;
                assert!((49.0..=53.0).contains(&signed_distance));
            }
        }
        assert_eq!(flashes, 1);
    }
}

#[test]
#[ignore = "requires the exact configured TF2 source bundle"]
fn exact_pyro_flame_airblast_and_shotgun_closures_emit_authored_particles() {
    struct ConfiguredSegments(PathBuf);
    impl playsrc_vpk::SegmentReader for ConfiguredSegments {
        fn len(&self, index: u32) -> Result<u64, playsrc_vpk::SourceError> {
            fs::metadata(self.0.join(format!("tf2_misc_{index:03}.vpk")))
                .map(|metadata| metadata.len())
                .map_err(|_| playsrc_vpk::SourceError {
                    code: playsrc_vpk::SourceErrorCode::Missing,
                    range: 0..0,
                })
        }
        fn read(
            &self,
            index: u32,
            range: std::ops::Range<u64>,
        ) -> Result<Vec<u8>, playsrc_vpk::SourceError> {
            use std::io::{Read, Seek};
            let mut file = fs::File::open(self.0.join(format!("tf2_misc_{index:03}.vpk")))
                .map_err(|_| playsrc_vpk::SourceError {
                    code: playsrc_vpk::SourceErrorCode::Missing,
                    range: range.clone(),
                })?;
            file.seek(std::io::SeekFrom::Start(range.start))
                .map_err(|_| playsrc_vpk::SourceError {
                    code: playsrc_vpk::SourceErrorCode::Io,
                    range: range.clone(),
                })?;
            let mut bytes = vec![0; (range.end - range.start) as usize];
            file.read_exact(&mut bytes)
                .map_err(|_| playsrc_vpk::SourceError {
                    code: playsrc_vpk::SourceErrorCode::ShortRead,
                    range,
                })?;
            Ok(bytes)
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_owned();
    let local = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let tf2 = PathBuf::from(json_string_field(&local, "tf2Dir"));
    let index = fs::read(tf2.join("tf2_misc_dir.vpk")).unwrap();
    let archive = playsrc_vpk::parse(
        &index,
        "tf2_misc_dir.vpk",
        playsrc_vpk::Layout::Split,
        playsrc_vpk::Limits::default(),
    )
    .unwrap();
    let segments = ConfiguredSegments(tf2);
    let paths = [
        "particles/flamethrower.pcf",
        "particles/muzzle_flash.pcf",
        "particles/blood_impact.pcf",
        "particles/bullet_tracers.pcf",
        "particles/impact_fx.pcf",
        "particles/crit.pcf",
    ];
    let bytes = paths.map(|path| archive.read_entry(path, &segments).unwrap().bytes);
    let sources = paths
        .iter()
        .zip(&bytes)
        .map(|(logical_path, bytes)| PcfSource {
            logical_path,
            bytes,
        })
        .collect::<Vec<_>>();
    let registry = Registry::from_pcf(&sources, RegistryLimits::default()).unwrap();
    for root in [
        "blood_impact_red_01",
        "blood_spray_red_01",
        "blood_spray_red_01_far",
        "bullet_scattergun_tracer01_red",
        "bullet_scattergun_tracer01_blue",
        "bullet_pistol_tracer01_red",
        "bullet_pistol_tracer01_blue",
        "bullet_shotgun_tracer01_red",
        "bullet_shotgun_tracer01_blue",
        "bullet_tracer01_red",
        "bullet_tracer01_blue",
        "impact_concrete",
        "impact_wood",
        "impact_metal",
        "impact_dirt",
        "impact_glass",
        "crit_text",
    ] {
        registry
            .target_closure(&[DefinitionLookup::Name(root)])
            .unwrap_or_else(|error| panic!("{root}: {error:?}"));
        let mut world = ParticleWorld::new(&registry, &BTreeMap::new(), WorldLimits::default()).unwrap();
        let control = |index, position| ControlPoint {
            index,
            position,
            previous_position: position,
            orientation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            radius: 0.0,
            density: 0.0,
            duration: 0.0,
            parent: None,
            object_identity: Some(1),
        };
        let event = Event {
            identity: 1,
            timestamp_seconds: 0.0,
            source_order: 0,
            command: EventCommand::Create {
                effect_identity: 1,
                definition: root.into(),
                seed: 42,
                owner_identity: Some(1),
                control_points: vec![control(0, [0.0; 3]), control(1, [400.0, 0.0, 0.0])],
            },
        };
        let (items, _) = world
            .advance(
                &[event],
                AdvanceRequest {
                    from_seconds: 0.0,
                    to_seconds: 0.015,
                    maximum_step_seconds: 0.015,
                    camera_position: [-10.0, 0.0, 0.0],
                },
                &mut NoHit,
            )
            .unwrap_or_else(|error| panic!("{root} simulation: {error:?}"));
        assert!(!items.is_empty(), "{root} emitted no authored particles");
    }
    for (root, definitions, materials) in [
        ("new_flame", 5, 4),
        ("new_flame_crit_red", 6, 5),
        ("new_flame_crit_blue", 6, 5),
        ("pyro_blast", 5, 4),
        ("muzzle_shotgun", 4, 3),
    ] {
        let closure = registry
            .target_closure(&[DefinitionLookup::Name(root)])
            .unwrap();
        assert_eq!(
            (closure.definitions.len(), closure.materials.len()),
            (definitions, materials)
        );
        let mut world = ParticleWorld::new(&registry, &BTreeMap::new(), WorldLimits::default()).unwrap();
        let control = |index, position, duration| ControlPoint {
            index,
            position,
            previous_position: position,
            orientation: [0.0, 0.0, 0.0, 1.0],
            velocity: [200.0, 0.0, 0.0],
            radius: 12.0,
            density: 1.0,
            duration,
            parent: None,
            object_identity: Some(1),
        };
        let event = Event {
            identity: 1,
            timestamp_seconds: 0.0,
            source_order: 0,
            command: EventCommand::Create {
                effect_identity: 1,
                definition: root.into(),
                seed: 42,
                owner_identity: Some(1),
                control_points: vec![control(0, [0.0; 3], 0.0), control(1, [20.0, 0.0, 0.0], 0.6)],
            },
        };
        let (items, _) = world
            .advance(
                &[event],
                AdvanceRequest {
                    from_seconds: 0.0,
                    to_seconds: 0.06,
                    maximum_step_seconds: 0.015,
                    camera_position: [-10.0, 0.0, 0.0],
                },
                &mut NoHit,
            )
            .unwrap();
        assert!(!items.is_empty(), "{root} emitted no authored particles");
        assert!(
            items
                .iter()
                .all(|item| item.position.iter().all(|value| value.is_finite()))
        );
    }
}
