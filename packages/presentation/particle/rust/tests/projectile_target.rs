use std::collections::{BTreeMap, BTreeSet};

use playsrc_particle::{
    AdvanceRequest, CollisionQuery, CollisionResult, ControlPoint, DefinitionLookup, Error,
    ErrorCode, Event, EventCommand, ParticleBlendFactor, ParticleBlendState, ParticleColorSpace,
    ParticleMaterial, ParticleMaterialShader, ParticleSheet, PcfSource, Registry, RegistryLimits,
    SheetFrame, SheetSampleRequest, SheetSequence, StopMode, TraceRequest, WorldLimits,
    encode_render_output, resolve_render_output, sample_sheet,
};

#[derive(Clone)]
enum TestValue {
    Ref(i32),
    Int(i32),
    Float(f32),
    Bool(bool),
    Text(&'static str),
    Color([u8; 4]),
    Vector([f32; 3]),
    Refs(Vec<i32>),
}

#[derive(Clone)]
struct TestElement {
    kind: &'static str,
    name: &'static str,
    uuid: [u8; 16],
    attributes: Vec<(&'static str, TestValue)>,
}

fn element(
    name: &'static str,
    ordinal: u8,
    attributes: Vec<(&'static str, TestValue)>,
) -> TestElement {
    TestElement {
        kind: "DmeParticleFunction",
        name,
        uuid: [ordinal; 16],
        attributes,
    }
}

fn fixture(with_constraint: bool) -> Vec<u8> {
    fixture_with_limit(with_constraint, 32)
}

fn fixture_with_limit(with_constraint: bool, maximum_particles: i32) -> Vec<u8> {
    fixture_with_renderer(
        with_constraint,
        maximum_particles,
        "render_animated_sprites",
    )
}

fn fixture_with_renderer(
    with_constraint: bool,
    maximum_particles: i32,
    renderer_identity: &'static str,
) -> Vec<u8> {
    fixture_with_renderer_and_local_velocity(
        with_constraint,
        maximum_particles,
        renderer_identity,
        [4.0, 0.0, 0.0],
    )
}

fn fixture_with_renderer_and_local_velocity(
    with_constraint: bool,
    maximum_particles: i32,
    renderer_identity: &'static str,
    local_velocity: [f32; 3],
) -> Vec<u8> {
    let mut elements = vec![TestElement {
        kind: "DmeElement",
        name: "root",
        uuid: [0; 16],
        attributes: vec![("particleSystemDefinitions", TestValue::Refs(vec![1, 2]))],
    }];
    let mut root_attributes = vec![
        ("material", TestValue::Text("effects\\rocketrailsmoke.vmt")),
        ("renderers", TestValue::Refs(vec![4, 28])),
        (
            "operators",
            TestValue::Refs(vec![5, 6, 7, 8, 9, 22, 23, 24]),
        ),
        (
            "initializers",
            TestValue::Refs(vec![10, 11, 12, 13, 14, 15, 16, 17, 25, 26, 27]),
        ),
        ("emitters", TestValue::Refs(vec![18])),
        ("forces", TestValue::Refs(vec![19])),
        ("children", TestValue::Refs(vec![3])),
        ("sort particles", TestValue::Bool(true)),
        ("max_particles", TestValue::Int(maximum_particles)),
        ("initial_particles", TestValue::Int(1)),
    ];
    if with_constraint {
        root_attributes.push(("constraints", TestValue::Refs(vec![20])));
    }
    elements.push(TestElement {
        kind: "DmeParticleSystemDefinition",
        name: "rockettrail",
        uuid: [1; 16],
        attributes: root_attributes,
    });
    elements.push(TestElement {
        kind: "DmeParticleSystemDefinition",
        name: "rockettrail_fire",
        uuid: [2; 16],
        attributes: vec![
            (
                "material",
                TestValue::Text("effects/sc_brightglow_y_nomodel.vmt"),
            ),
            ("renderers", TestValue::Refs(vec![4])),
            ("initializers", TestValue::Refs(vec![10, 11, 12, 13])),
            ("emitters", TestValue::Refs(vec![21])),
        ],
    });
    elements.push(TestElement {
        kind: "DmeParticleChild",
        name: "rockettrail_fire",
        uuid: [3; 16],
        attributes: vec![
            ("child", TestValue::Ref(2)),
            ("delay", TestValue::Float(0.25)),
        ],
    });
    elements.push(element(
        renderer_identity,
        4,
        vec![
            ("functionName", TestValue::Text(renderer_identity)),
            ("animation rate", TestValue::Float(0.25)),
            ("animation_fit_lifetime", TestValue::Bool(true)),
        ],
    ));
    elements.push(element(
        "Movement Basic",
        5,
        vec![
            ("functionName", TestValue::Text("Movement Basic")),
            ("gravity", TestValue::Vector([0.0, 0.0, -10.0])),
            ("drag", TestValue::Float(0.1)),
        ],
    ));
    elements.push(element(
        "Alpha Fade and Decay",
        6,
        vec![
            ("functionName", TestValue::Text("Alpha Fade and Decay")),
            ("start_fade_out_time", TestValue::Float(0.5)),
            ("end_fade_out_time", TestValue::Float(1.0)),
        ],
    ));
    elements.push(element(
        "Radius Scale",
        7,
        vec![
            ("functionName", TestValue::Text("Radius Scale")),
            ("radius_start_scale", TestValue::Float(1.0)),
            ("radius_end_scale", TestValue::Float(2.0)),
        ],
    ));
    elements.push(element(
        "Color Fade",
        8,
        vec![
            ("functionName", TestValue::Text("Color Fade")),
            ("color_fade", TestValue::Color([10, 20, 30, 255])),
        ],
    ));
    elements.push(element(
        "Movement Lock to Control Point",
        9,
        vec![(
            "functionName",
            TestValue::Text("Movement Lock to Control Point"),
        )],
    ));
    elements.push(element(
        "Lifetime Random",
        10,
        vec![
            ("functionName", TestValue::Text("Lifetime Random")),
            ("lifetime_min", TestValue::Float(2.0)),
            ("lifetime_max", TestValue::Float(2.0)),
        ],
    ));
    elements.push(element(
        "Position Within Sphere Random",
        11,
        vec![
            (
                "functionName",
                TestValue::Text("Position Within Sphere Random"),
            ),
            ("distance_min", TestValue::Float(1.0)),
            ("distance_max", TestValue::Float(1.0)),
            (
                "speed_in_local_coordinate_system_min",
                TestValue::Vector(local_velocity),
            ),
            (
                "speed_in_local_coordinate_system_max",
                TestValue::Vector(local_velocity),
            ),
        ],
    ));
    elements.push(element(
        "Radius Random",
        12,
        vec![
            ("functionName", TestValue::Text("Radius Random")),
            ("radius_min", TestValue::Float(2.0)),
            ("radius_max", TestValue::Float(2.0)),
        ],
    ));
    elements.push(element(
        "Alpha Random",
        13,
        vec![
            ("functionName", TestValue::Text("Alpha Random")),
            ("alpha_min", TestValue::Int(255)),
            ("alpha_max", TestValue::Int(255)),
        ],
    ));
    elements.push(element(
        "Color Random",
        14,
        vec![
            ("functionName", TestValue::Text("Color Random")),
            ("color1", TestValue::Color([255, 100, 0, 255])),
            ("color2", TestValue::Color([255, 100, 0, 255])),
        ],
    ));
    elements.push(element(
        "Rotation Random",
        15,
        vec![
            ("functionName", TestValue::Text("Rotation Random")),
            ("rotation_offset_min", TestValue::Float(90.0)),
            ("rotation_offset_max", TestValue::Float(90.0)),
        ],
    ));
    elements.push(element(
        "Sequence Random",
        16,
        vec![
            ("functionName", TestValue::Text("Sequence Random")),
            ("sequence_min", TestValue::Int(2)),
            ("sequence_max", TestValue::Int(2)),
        ],
    ));
    elements.push(element(
        "Trail Length Random",
        17,
        vec![
            ("functionName", TestValue::Text("Trail Length Random")),
            ("length_min", TestValue::Float(0.25)),
            ("length_max", TestValue::Float(0.25)),
        ],
    ));
    elements.push(element(
        "emit_continuously",
        18,
        vec![
            ("functionName", TestValue::Text("emit_continuously")),
            ("emission_rate", TestValue::Float(4.0)),
            ("emission_duration", TestValue::Float(1.0)),
        ],
    ));
    elements.push(element(
        "random force",
        19,
        vec![
            ("functionName", TestValue::Text("random force")),
            ("min force", TestValue::Vector([0.0, 0.0, 0.0])),
            ("max force", TestValue::Vector([0.0, 0.0, 0.0])),
        ],
    ));
    elements.push(element(
        "Collision via traces",
        20,
        vec![
            ("functionName", TestValue::Text("Collision via traces")),
            ("amount of bounce", TestValue::Float(1.0)),
            ("collision mode", TestValue::Int(1)),
            ("brush only", TestValue::Bool(true)),
            ("collision group", TestValue::Text("NONE")),
        ],
    ));
    elements.push(element(
        "emit_instantaneously",
        21,
        vec![
            ("functionName", TestValue::Text("emit_instantaneously")),
            ("num_to_emit", TestValue::Int(1)),
        ],
    ));
    elements.push(element(
        "Rotation Basic",
        22,
        vec![("functionName", TestValue::Text("Rotation Basic"))],
    ));
    elements.push(element(
        "Rotation Spin Roll",
        23,
        vec![
            ("functionName", TestValue::Text("Rotation Spin Roll")),
            ("spin_rate_min", TestValue::Int(30)),
            ("spin_rate_degrees", TestValue::Int(30)),
        ],
    ));
    elements.push(element(
        "Oscillate Scalar",
        24,
        vec![
            ("functionName", TestValue::Text("Oscillate Scalar")),
            ("oscillation field", TestValue::Int(4)),
            ("oscillation rate min", TestValue::Float(0.1)),
            ("oscillation rate max", TestValue::Float(0.1)),
        ],
    ));
    elements.push(element(
        "Position Within Box Random",
        25,
        vec![
            (
                "functionName",
                TestValue::Text("Position Within Box Random"),
            ),
            ("min", TestValue::Vector([-1.0, -1.0, -1.0])),
            ("max", TestValue::Vector([1.0, 1.0, 1.0])),
        ],
    ));
    elements.push(element(
        "Position Modify Offset Random",
        26,
        vec![
            (
                "functionName",
                TestValue::Text("Position Modify Offset Random"),
            ),
            ("offset min", TestValue::Vector([1.0, 0.0, 0.0])),
            ("offset max", TestValue::Vector([1.0, 0.0, 0.0])),
            ("offset in local space 0/1", TestValue::Bool(true)),
        ],
    ));
    elements.push(element(
        "Rotation Speed Random",
        27,
        vec![
            ("functionName", TestValue::Text("Rotation Speed Random")),
            ("rotation_speed_random_min", TestValue::Float(5.0)),
            ("rotation_speed_random_max", TestValue::Float(5.0)),
        ],
    ));
    elements.push(element(
        "render_sprite_trail",
        28,
        vec![("functionName", TestValue::Text("render_sprite_trail"))],
    ));
    encode(&elements)
}

fn encode(elements: &[TestElement]) -> Vec<u8> {
    let mut strings = BTreeSet::new();
    for element in elements {
        strings.insert(element.kind);
        for (name, _) in &element.attributes {
            strings.insert(*name);
        }
    }
    let strings: Vec<&str> = strings.into_iter().collect();
    let indexes: BTreeMap<&str, u16> = strings
        .iter()
        .enumerate()
        .map(|(index, value)| (*value, index as u16))
        .collect();
    let mut output = b"<!-- dmx encoding binary 2 format pcf 1 -->\n\0".to_vec();
    output.extend_from_slice(&(strings.len() as u16).to_le_bytes());
    for value in &strings {
        output.extend_from_slice(value.as_bytes());
        output.push(0);
    }
    output.extend_from_slice(&(elements.len() as i32).to_le_bytes());
    for element in elements {
        output.extend_from_slice(&indexes[element.kind].to_le_bytes());
        output.extend_from_slice(element.name.as_bytes());
        output.push(0);
        output.extend_from_slice(&element.uuid);
    }
    for element in elements {
        output.extend_from_slice(&(element.attributes.len() as i32).to_le_bytes());
        for (name, value) in &element.attributes {
            output.extend_from_slice(&indexes[name].to_le_bytes());
            match value {
                TestValue::Ref(value) => {
                    output.push(1);
                    output.extend_from_slice(&value.to_le_bytes());
                }
                TestValue::Int(value) => {
                    output.push(2);
                    output.extend_from_slice(&value.to_le_bytes());
                }
                TestValue::Float(value) => {
                    output.push(3);
                    output.extend_from_slice(&value.to_le_bytes());
                }
                TestValue::Bool(value) => {
                    output.push(4);
                    output.push(u8::from(*value));
                }
                TestValue::Text(value) => {
                    output.push(5);
                    output.extend_from_slice(value.as_bytes());
                    output.push(0);
                }
                TestValue::Color(value) => {
                    output.push(8);
                    output.extend_from_slice(value);
                }
                TestValue::Vector(value) => {
                    output.push(10);
                    for component in value {
                        output.extend_from_slice(&component.to_le_bytes());
                    }
                }
                TestValue::Refs(values) => {
                    output.push(15);
                    output.extend_from_slice(&(values.len() as i32).to_le_bytes());
                    for value in values {
                        output.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
    }
    output
}

fn registry(bytes: &[u8]) -> Registry {
    Registry::from_pcf(
        &[PcfSource {
            logical_path: "particles/projectile_test.pcf",
            bytes,
        }],
        RegistryLimits::default(),
    )
    .unwrap()
}

fn control(position: [f32; 3], previous_position: [f32; 3]) -> ControlPoint {
    ControlPoint {
        index: 0,
        position,
        previous_position,
        orientation: [0.0, 0.0, 0.0, 1.0],
        velocity: [0.0; 3],
        parent: None,
        object_identity: Some(10),
    }
}

fn create_event(control_points: Vec<ControlPoint>) -> Event {
    Event {
        identity: 1,
        timestamp_seconds: 0.0,
        source_order: 0,
        command: EventCommand::Create {
            effect_identity: 7,
            definition: "ROCKETTRAIL".to_owned(),
            seed: 99,
            owner_identity: Some(10),
            control_points,
        },
    }
}

fn material(sheet: ParticleSheet) -> ParticleMaterial {
    ParticleMaterial {
        shader: ParticleMaterialShader::MeshSprite,
        blend: ParticleBlendState {
            source: ParticleBlendFactor::SourceAlpha,
            destination: ParticleBlendFactor::OneMinusSourceAlpha,
        },
        color_space: ParticleColorSpace::SrgbTextureLinearTint,
        dual_sequence: false,
        sheet,
    }
}

fn full_texture_sheet() -> ParticleSheet {
    ParticleSheet {
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
    }
}

fn animated_sheet() -> ParticleSheet {
    let images = |left: f32| [[left, 0.0, left + 0.25, 1.0]; 4];
    ParticleSheet {
        sequences: BTreeMap::from([(
            0,
            SheetSequence {
                clamp: false,
                duration_seconds: 1.0,
                frames: vec![
                    SheetFrame {
                        duration_seconds: 0.5,
                        images: images(0.0),
                    },
                    SheetFrame {
                        duration_seconds: 0.5,
                        images: images(0.5),
                    },
                ],
            },
        )]),
    }
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
}

#[test]
fn parses_registry_and_rejects_malformed_documents_atomically() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let definition = registry
        .definition(DefinitionLookup::Name("RocketTrail"))
        .unwrap();
    assert_eq!(definition.material, "effects/rocketrailsmoke.vmt");
    let closure = registry
        .target_closure(&[DefinitionLookup::Name("rockettrail")])
        .unwrap();
    assert_eq!(closure.definitions, vec![[1; 16], [2; 16]]);
    assert_eq!(closure.materials.len(), 2);

    let mut truncated = bytes.clone();
    truncated.truncate(10);
    assert_eq!(
        Registry::from_pcf(
            &[PcfSource {
                logical_path: "particles/truncated.pcf",
                bytes: &truncated,
            }],
            RegistryLimits::default(),
        )
        .unwrap_err()
        .code,
        ErrorCode::Truncated,
    );

    let over_limit = RegistryLimits {
        max_elements_per_source: 1,
        ..RegistryLimits::default()
    };
    assert_eq!(
        Registry::from_pcf(
            &[PcfSource {
                logical_path: "particles/limited.pcf",
                bytes: &bytes,
            }],
            over_limit,
        )
        .unwrap_err()
        .code,
        ErrorCode::BoundExceeded,
    );
}

#[test]
fn retains_unknown_modules_and_rejects_them_before_target_creation() {
    let bytes = fixture_with_renderer(false, 32, "unknown projectile renderer");
    let registry = registry(&bytes);
    let definition = registry
        .definition(DefinitionLookup::Name("rockettrail"))
        .unwrap();
    assert_eq!(
        definition.functions[0].identity,
        "unknown projectile renderer"
    );
    assert_eq!(
        registry
            .target_closure(&[DefinitionLookup::Name("rockettrail")])
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedFunction
    );
}

#[test]
fn advances_children_controls_and_equivalent_partitions_deterministically() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut whole =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    let mut partitioned = whole.clone();
    let event = create_event(vec![control([1.0, 2.0, 3.0], [1.0, 2.0, 3.0])]);
    let mut no_hit = NoHit;
    let whole_output = whole
        .advance(
            std::slice::from_ref(&event),
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.25,
                camera_position: [100.0, 0.0, 0.0],
            },
            &mut no_hit,
        )
        .unwrap();
    partitioned
        .advance(
            &[event],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.5,
                maximum_step_seconds: 0.25,
                camera_position: [100.0, 0.0, 0.0],
            },
            &mut no_hit,
        )
        .unwrap();
    let partitioned_output = partitioned
        .advance(
            &[],
            AdvanceRequest {
                from_seconds: 0.5,
                to_seconds: 1.0,
                maximum_step_seconds: 0.25,
                camera_position: [100.0, 0.0, 0.0],
            },
            &mut no_hit,
        )
        .unwrap();
    assert_eq!(whole, partitioned);
    assert_eq!(whole_output, partitioned_output);
    assert!(
        whole_output
            .0
            .iter()
            .any(|item| item.system_uuid == [2; 16])
    );
    assert!(whole_output.1.is_some());

    let material_names = vec![
        "effects/rocketrailsmoke.vmt".to_owned(),
        "effects/sc_brightglow_y_nomodel.vmt".to_owned(),
    ];
    let materials = BTreeMap::from([
        (material_names[0].clone(), material(full_texture_sheet())),
        (material_names[1].clone(), material(full_texture_sheet())),
    ]);
    let resolved = resolve_render_output(whole_output.0, &materials).unwrap();
    let encoded =
        encode_render_output(&resolved, whole_output.1, &material_names, 1024 * 1024).unwrap();
    assert_eq!(&encoded[0..4], &0x5250_5350_u32.to_le_bytes());
    assert_eq!(encoded.len(), 40 + resolved.len() * 436);
}

#[test]
fn sphere_local_velocity_uses_source_forward_right_up_basis() {
    let bytes = fixture_with_renderer_and_local_velocity(
        false,
        32,
        "render_animated_sprites",
        [4.0, 6.0, 8.0],
    );
    let registry = registry(&bytes);
    let half_turn = std::f32::consts::FRAC_1_SQRT_2;
    for (orientation, expected_velocity) in [
        ([0.0, 0.0, 0.0, 1.0], [4.0, -6.0, 8.0]),
        ([0.0, 0.0, half_turn, half_turn], [6.0, 4.0, 8.0]),
    ] {
        let mut world =
            playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
        let mut point = control([10.0, 20.0, 30.0], [10.0, 20.0, 30.0]);
        point.orientation = orientation;
        let (items, _) = world
            .advance(
                &[create_event(vec![point])],
                AdvanceRequest {
                    from_seconds: 0.0,
                    to_seconds: 0.0,
                    maximum_step_seconds: 0.1,
                    camera_position: [0.0; 3],
                },
                &mut NoHit,
            )
            .unwrap();
        let particle = items
            .iter()
            .find(|item| item.system_uuid == [1; 16])
            .unwrap();
        for (axis, expected) in expected_velocity.into_iter().enumerate() {
            let actual = (particle.position[axis] - particle.previous_position[axis]) / 0.05;
            assert!(
                (actual - expected).abs() < 0.0001,
                "orientation {orientation:?}, axis {axis}: expected {expected}, got {actual}"
            );
        }
    }
}

#[test]
fn first_frame_creates_only_authored_initial_particles_before_emitters() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    let (items, bounds) = world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(
        items.len(),
        2,
        "one initial particle is emitted by two root renderers"
    );
    assert!(items.iter().all(|item| item.system_uuid == [1; 16]));
    let bounds = bounds.unwrap();
    for component in 0..3 {
        assert!((bounds.minimum[component] - (items[0].position[component] - 10.0)).abs() < 1.0e-6);
        assert!((bounds.maximum[component] - (items[0].position[component] + 10.0)).abs() < 1.0e-6);
    }
}

#[test]
fn samples_variable_duration_particle_sheets_at_source_table_resolution() {
    let rect = |left: f32| [[left, 0.0, left + 0.1, 1.0]; 4];
    let sheet = ParticleSheet {
        sequences: BTreeMap::from([(
            3,
            SheetSequence {
                clamp: false,
                duration_seconds: 1.0,
                frames: vec![
                    SheetFrame {
                        duration_seconds: 0.25,
                        images: rect(0.0),
                    },
                    SheetFrame {
                        duration_seconds: 0.75,
                        images: rect(0.5),
                    },
                ],
            },
        )]),
    };
    let sample = sample_sheet(
        &sheet,
        SheetSampleRequest {
            sequence: 3,
            age_seconds: 0.5,
            lifetime_seconds: 2.0,
            animation_rate: 1.0,
            fit_lifetime: false,
            animation_rate_as_fps: false,
        },
    )
    .unwrap();
    assert_eq!(sample.current, rect(0.5));
    assert_eq!(sample.next, rect(0.0));
    assert!((sample.blend - 1.0 / 3.0).abs() < 1.0e-6);
}

#[test]
fn consumes_supplied_collisions_and_preserves_state_after_atomic_failure() {
    let bytes = fixture(true);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    let mut collision_calls = 0;
    let mut collision_requests = 0;
    let mut collision = |requests: &[TraceRequest]| {
        collision_calls += 1;
        collision_requests += requests.len();
        Ok(requests
            .iter()
            .map(|request| CollisionResult {
                identity: request.identity,
                fraction: 0.5,
                start_solid: false,
                normal: [1.0, 0.0, 0.0],
            })
            .collect())
    };
    struct Query<'a, F>(&'a mut F);
    impl<F> CollisionQuery for Query<'_, F>
    where
        F: FnMut(&[TraceRequest]) -> Result<Vec<CollisionResult>, Error>,
    {
        fn trace_batch(
            &mut self,
            requests: &[TraceRequest],
        ) -> Result<Vec<CollisionResult>, Error> {
            (self.0)(requests)
        }
    }
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.5,
                maximum_step_seconds: 0.25,
                camera_position: [0.0; 3],
            },
            &mut Query(&mut collision),
        )
        .unwrap();
    assert_eq!(collision_calls, 1);
    assert_eq!(collision_requests, 52);
    let before = world.clone();
    let error = world
        .advance(
            &[Event {
                identity: 2,
                timestamp_seconds: 0.5,
                source_order: 0,
                command: EventCommand::StopEmission {
                    effect_identity: 999,
                    mode: StopMode::Immediate,
                },
            }],
            AdvanceRequest {
                from_seconds: 0.5,
                to_seconds: 0.5,
                maximum_step_seconds: 0.25,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidState);
    assert_eq!(world, before);
}

#[test]
fn emission_overrun_fails_without_publishing_an_effect() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let limits = WorldLimits {
        max_particles_total: 1,
        ..WorldLimits::default()
    };
    let mut world = playsrc_particle::ParticleWorld::new(&registry, limits).unwrap();
    let error = world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.25,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BoundExceeded);
    assert_eq!(world.effect_count(), 0);
    assert_eq!(world.time(), 0.0);
}

#[test]
fn substep_limit_fails_without_consuming_partial_time() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let limits = WorldLimits {
        max_substeps: 2,
        ..WorldLimits::default()
    };
    let mut world = playsrc_particle::ParticleWorld::new(&registry, limits).unwrap();
    let error = world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.3,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::BoundExceeded);
    assert_eq!(world.time(), 0.0);
    assert_eq!(world.effect_count(), 0);
}

#[test]
fn authored_capacity_drops_excess_emission_without_failing_the_world() {
    let bytes = fixture_with_limit(false, 1);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(world.effect_count(), 1);
}

#[test]
fn replacement_and_immediate_stop_are_explicit_and_ordered() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.5,
                maximum_step_seconds: 0.25,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    world
        .advance(
            &[
                Event {
                    identity: 2,
                    timestamp_seconds: 0.5,
                    source_order: 0,
                    command: EventCommand::Replace {
                        effect_identity: 7,
                        definition: "rockettrail".to_owned(),
                        seed: 100,
                        owner_identity: Some(10),
                        control_points: vec![control([5.0, 0.0, 0.0], [5.0, 0.0, 0.0])],
                    },
                },
                Event {
                    identity: 3,
                    timestamp_seconds: 0.5,
                    source_order: 1,
                    command: EventCommand::StopEmission {
                        effect_identity: 7,
                        mode: StopMode::Immediate,
                    },
                },
            ],
            AdvanceRequest {
                from_seconds: 0.5,
                to_seconds: 0.5,
                maximum_step_seconds: 0.25,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(world.effect_count(), 0);
}

#[test]
fn resolves_shader_blend_sheet_and_derived_trail_output() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    let (raw, _) = world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [100.0, 0.0, 0.0],
            },
            &mut NoHit,
        )
        .unwrap();
    let root_material = "effects/rocketrailsmoke.vmt".to_owned();
    let child_material = "effects/sc_brightglow_y_nomodel.vmt".to_owned();
    let mesh_materials = BTreeMap::from([
        (root_material.clone(), material(animated_sheet())),
        (child_material.clone(), material(animated_sheet())),
    ]);
    assert_eq!(
        resolve_render_output(raw.clone(), &BTreeMap::new())
            .unwrap_err()
            .code,
        ErrorCode::MissingDependency
    );
    let mesh = resolve_render_output(raw.clone(), &mesh_materials).unwrap();
    let trail = mesh
        .iter()
        .find(|item| {
            item.material == root_material && item.primitive == playsrc_particle::Primitive::Trail
        })
        .unwrap();
    assert!(trail.trail_length > 0.0);
    assert!(trail.trail_width <= trail.radius && trail.trail_width <= trail.trail_length);
    assert_ne!(trail.trail_end_position, trail.position);
    assert_eq!(
        trail.material_state.unwrap().blend.destination,
        ParticleBlendFactor::OneMinusSourceAlpha
    );

    let sprite_card_materials = BTreeMap::from([
        (
            root_material.clone(),
            ParticleMaterial {
                shader: ParticleMaterialShader::SpriteCard,
                blend: ParticleBlendState {
                    source: ParticleBlendFactor::SourceAlpha,
                    destination: ParticleBlendFactor::One,
                },
                color_space: ParticleColorSpace::SrgbTextureLinearTint,
                dual_sequence: false,
                sheet: animated_sheet(),
            },
        ),
        (child_material, material(animated_sheet())),
    ]);
    let sprite_card = resolve_render_output(raw, &sprite_card_materials).unwrap();
    assert!(sprite_card.iter().all(|item| {
        item.material != root_material || item.primitive == playsrc_particle::Primitive::Sprite
    }));
    let mesh_sprite = mesh
        .iter()
        .find(|item| {
            item.material == root_material && item.primitive == playsrc_particle::Primitive::Sprite
        })
        .unwrap();
    let card_sprite = sprite_card
        .iter()
        .find(|item| item.material == root_material)
        .unwrap();
    assert_ne!(mesh_sprite.primary_sheet, card_sprite.primary_sheet);
    assert!(card_sprite.secondary_sheet.is_none());

    let malformed = BTreeMap::from([
        (root_material, material(ParticleSheet::default())),
        (
            "effects/sc_brightglow_y_nomodel.vmt".to_owned(),
            material(animated_sheet()),
        ),
    ]);
    assert_eq!(
        resolve_render_output(mesh, &malformed).unwrap_err().code,
        ErrorCode::MissingDependency
    );
}

#[test]
fn complete_render_transaction_rolls_back_missing_materials_and_output_limits() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    let event = create_event(vec![control([0.0; 3], [0.0; 3])]);
    let request = AdvanceRequest {
        from_seconds: 0.0,
        to_seconds: 0.5,
        maximum_step_seconds: 0.1,
        camera_position: [0.0; 3],
    };
    let material_names = vec![
        "effects/rocketrailsmoke.vmt".to_owned(),
        "effects/sc_brightglow_y_nomodel.vmt".to_owned(),
    ];
    let materials = BTreeMap::from([
        (material_names[0].clone(), material(animated_sheet())),
        (material_names[1].clone(), material(animated_sheet())),
    ]);
    let before = world.clone();

    assert_eq!(
        world
            .transact_render_output(
                std::slice::from_ref(&event),
                request,
                &mut NoHit,
                &BTreeMap::new(),
                &material_names,
                1024 * 1024,
            )
            .unwrap_err()
            .code,
        ErrorCode::MissingDependency
    );
    assert_eq!(world, before);
    assert_eq!(
        world
            .transact_render_output(
                std::slice::from_ref(&event),
                request,
                &mut NoHit,
                &materials,
                &material_names,
                1,
            )
            .unwrap_err()
            .code,
        ErrorCode::BoundExceeded
    );
    assert_eq!(world, before);

    let output = world
        .transact_render_output(
            &[event],
            request,
            &mut NoHit,
            &materials,
            &material_names,
            1024 * 1024,
        )
        .unwrap();
    assert_eq!(&output[..4], b"PSPR");
    assert_eq!(world.time(), 0.5);
    assert_eq!(world.effect_count(), 1);
}

#[test]
fn natural_death_graceful_stop_reset_and_repeated_failure_clean_up_atomically() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.5,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    let (live, _) = world
        .advance(
            &[Event {
                identity: 2,
                timestamp_seconds: 0.5,
                source_order: 0,
                command: EventCommand::StopEmission {
                    effect_identity: 7,
                    mode: StopMode::Graceful,
                },
            }],
            AdvanceRequest {
                from_seconds: 0.5,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert!(!live.is_empty());
    for (from, to) in [(1.0, 2.0), (2.0, 3.0), (3.0, 4.0)] {
        world
            .advance(
                &[],
                AdvanceRequest {
                    from_seconds: from,
                    to_seconds: to,
                    maximum_step_seconds: 0.1,
                    camera_position: [0.0; 3],
                },
                &mut NoHit,
            )
            .unwrap();
    }
    assert_eq!(world.effect_count(), 0);
    let before = world.clone();
    assert_eq!(
        world
            .advance(
                &[Event {
                    identity: 3,
                    timestamp_seconds: 4.0,
                    source_order: 0,
                    command: EventCommand::StopEmission {
                        effect_identity: 7,
                        mode: StopMode::Graceful,
                    },
                }],
                AdvanceRequest {
                    from_seconds: 4.0,
                    to_seconds: 4.0,
                    maximum_step_seconds: 0.1,
                    camera_position: [0.0; 3],
                },
                &mut NoHit,
            )
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert_eq!(world, before);

    world
        .advance(
            &[
                Event {
                    identity: 4,
                    timestamp_seconds: 4.0,
                    source_order: 0,
                    command: EventCommand::Create {
                        effect_identity: 8,
                        definition: "rockettrail".to_owned(),
                        seed: 7,
                        owner_identity: None,
                        control_points: vec![control([0.0; 3], [0.0; 3])],
                    },
                },
                Event {
                    identity: 5,
                    timestamp_seconds: 4.0,
                    source_order: 1,
                    command: EventCommand::Reset,
                },
            ],
            AdvanceRequest {
                from_seconds: 4.0,
                to_seconds: 4.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(world.effect_count(), 0);
}

#[test]
fn restart_preserves_seed_and_live_particles_while_rescheduling_finite_emitters() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    let before = world.clone();
    let (items, _) = world
        .advance(
            &[Event {
                identity: 2,
                timestamp_seconds: 1.0,
                source_order: 0,
                command: EventCommand::Restart { effect_identity: 7 },
            }],
            AdvanceRequest {
                from_seconds: 1.0,
                to_seconds: 1.5,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert!(!items.is_empty());
    assert_ne!(world, before);
}

#[test]
fn dormancy_stops_emission_but_keeps_the_collection_and_live_simulation() {
    let bytes = fixture(false);
    let registry = registry(&bytes);
    let mut world =
        playsrc_particle::ParticleWorld::new(&registry, WorldLimits::default()).unwrap();
    world
        .advance(
            &[create_event(vec![control([0.0; 3], [0.0; 3])])],
            AdvanceRequest {
                from_seconds: 0.0,
                to_seconds: 0.5,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    let before = world.clone();
    world
        .advance(
            &[Event {
                identity: 2,
                timestamp_seconds: 0.5,
                source_order: 0,
                command: EventCommand::SetDormant {
                    effect_identity: 7,
                    dormant: true,
                },
            }],
            AdvanceRequest {
                from_seconds: 0.5,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(world.effect_count(), 1);
    assert_ne!(world, before);
    world
        .advance(
            &[Event {
                identity: 3,
                timestamp_seconds: 1.0,
                source_order: 0,
                command: EventCommand::SetDormant {
                    effect_identity: 7,
                    dormant: false,
                },
            }],
            AdvanceRequest {
                from_seconds: 1.0,
                to_seconds: 1.0,
                maximum_step_seconds: 0.1,
                camera_position: [0.0; 3],
            },
            &mut NoHit,
        )
        .unwrap();
    assert_eq!(world.effect_count(), 1);
}
