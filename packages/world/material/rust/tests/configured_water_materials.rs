use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{
    HdrMode, Material, MaterialRole, ParameterOrigin, ProxyEvaluationContext, ProxyValue,
    SelectionEnvironment, Shader, TextureColorRead, TextureFrameSelection, TextureRole,
    WaterShaderVariant, WaterSurfaceOpacity, evaluate_proxy_program, evaluate_water_material,
    resolve_for_environment, water_material_output,
};
use playsrc_vmt::{Composition, DependencyResponse, Limits, compose};
use std::{collections::BTreeMap, fs, path::Path};

const JUMP_SURFACE: &str =
    "materials/maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159.vmt";
const JUMP_SURFACE_HASH: &str = "2824deec4ec65df3ee5d5ef8e4c3419145ff5ff33479fe005518b859376f6335";
const JUMP_INCLUDED: &str = "materials/water/water_2fort_expensive.vmt";
const JUMP_INCLUDED_HASH: &str = "5f61b7786628a7e267419a7b709548102c115f2ef1f468bd3e3dc73aa6349806";
const JUMP_BENEATH: &str = "materials/water/water_2fort_beneath.vmt";
const JUMP_BENEATH_HASH: &str = "118cae4c43eda381491f99c0753fbc8963b35c2de787ee58194d3d7feaa028c8";
const UPWARD_SURFACE: &str =
    "materials/maps/pl_upward/water/water_hydro_cheap_dx80_7168_-2048_128.vmt";
const UPWARD_SURFACE_HASH: &str =
    "83a4ce4f5abe24b9446a634423b2f0b06e0f05dc9e3faf3e6b022d7c5ab34f57";
const UPWARD_INCLUDED: &str = "materials/water/water_hydro_cheap_dx80.vmt";
const UPWARD_INCLUDED_HASH: &str =
    "019a37e6ad1c042baa5bacc4641d88166d034cdbbe7ed0e1e36c87367cd28e65";
const UPWARD_BENEATH: &str = "materials/water/water_well_beneath.vmt";
const UPWARD_BENEATH_HASH: &str =
    "8b3a2f179dd8544d02dc2364a7cd692d5150e38b2fa2df6ee10935badb6a1aab";

#[test]
#[ignore = "requires bun packages/world/material/scripts/water-content.ts"]
fn configured_water_roots_preserve_shader_opacity_proxy_and_overlay_contracts() {
    let files = BTreeMap::from([
        (JUMP_SURFACE, source(JUMP_SURFACE_HASH)),
        (JUMP_INCLUDED, source(JUMP_INCLUDED_HASH)),
        (JUMP_BENEATH, source(JUMP_BENEATH_HASH)),
        (UPWARD_SURFACE, source(UPWARD_SURFACE_HASH)),
        (UPWARD_INCLUDED, source(UPWARD_INCLUDED_HASH)),
        (UPWARD_BENEATH, source(UPWARD_BENEATH_HASH)),
    ]);

    let surface = resolve(&files, JUMP_SURFACE);
    let beneath = resolve(&files, JUMP_BENEATH);
    let surface_state = water_material_output(&surface).unwrap().unwrap();
    let beneath_state = water_material_output(&beneath).unwrap().unwrap();
    assert_eq!(surface.shader, Shader::Water);
    assert_eq!(surface_state.shader, WaterShaderVariant::Dx9Hdr);
    assert_eq!(surface_state.opacity, WaterSurfaceOpacity::Opaque);
    assert_eq!(beneath_state.opacity, WaterSurfaceOpacity::Opaque);
    assert!(surface_state.textures.reflection.is_some());
    assert!(surface_state.textures.refraction.is_some());
    assert!(!surface_state.blur_refraction.value);
    assert!(
        surface
            .first_parameters
            .contains_key(b"$refractblur".as_slice())
    );
    assert_eq!(surface_state.reflect_amount.value, 0.25);
    assert_eq!(surface_state.refract_amount.value, 0.32);
    assert_eq!(surface.proxy_program.entries.len(), 7);
    assert_eq!(
        beneath_state
            .underwater_overlay
            .as_ref()
            .unwrap()
            .logical_path,
        "materials/effects/water_warp_2fort.vmt",
    );
    assert_eq!(beneath_state.refract_amount.value, 0.5);
    assert!(beneath_state.blur_refraction.value);
    assert_eq!(beneath.proxy_program.entries.len(), 3);
    let evaluated = evaluate_water_material(
        &surface,
        &ProxyEvaluationContext {
            time: 1.0,
            frame_time: 0.015,
            water_lod: Some([1000.0, 2000.0]),
            texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 60)]),
            ..ProxyEvaluationContext::default()
        },
    )
    .unwrap();
    assert_eq!(evaluated.normal_frame, 30);
    assert_eq!(
        [evaluated.cheap_start, evaluated.cheap_end],
        [1000.0, 2000.0]
    );

    let cheap_surface = resolve(&files, UPWARD_SURFACE);
    assert_eq!(cheap_surface.shader, Shader::LightmappedGeneric);
    assert!(cheap_surface.water.is_none());
    assert!(water_material_output(&cheap_surface).unwrap().is_none());
    assert_eq!(
        cheap_surface.first_parameters[b"%compilewater".as_slice()],
        b"1",
    );
    assert_eq!(
        cheap_surface
            .material_requests
            .iter()
            .find(|request| request.role == MaterialRole::Bottom)
            .unwrap()
            .logical_path,
        UPWARD_BENEATH,
    );
    for role in [TextureRole::Bump, TextureRole::Normal] {
        let request = cheap_surface
            .textures
            .iter()
            .find(|request| request.role == role)
            .unwrap();
        assert_eq!(
            request.logical_path.as_deref(),
            Some("materials/water/dx80_tfwater001_normal.vtf"),
        );
        assert_eq!(request.color_read, TextureColorRead::Linear);
        assert!(matches!(
            cheap_surface
                .texture_uses
                .iter()
                .find(|usage| usage.role == role)
                .unwrap()
                .frame,
            TextureFrameSelection::Static {
                initial: 0,
                proxy_mutated: true,
                ..
            }
        ));
    }
    assert_eq!(cheap_surface.proxy_program.entries.len(), 1);
    let evaluated = evaluate_proxy_program(
        &cheap_surface.proxy_program,
        &BTreeMap::from([(b"$bumpframe".to_vec(), ProxyValue::Int(0))]),
        &ProxyEvaluationContext {
            time: 0.5,
            frame_time: 0.015,
            texture_frames: BTreeMap::from([(b"$bumpmap".to_vec(), 30)]),
            ..ProxyEvaluationContext::default()
        },
    )
    .unwrap();
    assert_eq!(
        evaluated.variables[b"$bumpframe".as_slice()],
        ProxyValue::Int(15),
    );

    let beneath = resolve(&files, UPWARD_BENEATH);
    let beneath_state = water_material_output(&beneath).unwrap().unwrap();
    assert_eq!(beneath.shader, Shader::Water);
    assert_eq!(beneath_state.opacity, WaterSurfaceOpacity::Opaque);
    assert!(!beneath_state.above_water.value);
    assert!(beneath_state.textures.reflection.is_none());
    assert!(beneath_state.textures.environment.is_none());
    assert!(beneath_state.textures.refraction.is_some());
    assert_eq!(beneath_state.refract_amount.value, 0.2);
    assert_eq!(
        beneath_state.force_expensive.origin,
        ParameterOrigin::Authored
    );
    assert!(beneath_state.force_expensive.value);
    assert_eq!(
        [beneath_state.fog.start.value, beneath_state.fog.end.value],
        [-350.0, 1550.0]
    );
    assert_eq!(
        beneath_state
            .underwater_overlay
            .as_ref()
            .unwrap()
            .logical_path,
        "materials/effects/water_warp_well.vmt",
    );
}

fn source(hash: &str) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let cache = config
        .split_once("\"sourceCacheDir\"")
        .unwrap()
        .1
        .split_once(':')
        .unwrap()
        .1
        .trim_start()
        .strip_prefix('"')
        .unwrap();
    let cache = &cache[..cache.find('"').unwrap()];
    fs::read(
        Path::new(cache)
            .join("evidence/tf2-water-rendering/content")
            .join(hash),
    )
    .unwrap()
}

fn resolve(files: &BTreeMap<&str, Vec<u8>>, identity: &str) -> Material {
    let root = files[identity].as_slice();
    let mut dependencies = Vec::new();
    loop {
        match compose(
            root,
            identity.to_owned(),
            &dependencies,
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap()
        {
            Composition::Complete(document) => {
                return resolve_for_environment(
                    &document,
                    SelectionEnvironment {
                        hdr_mode: HdrMode::Integer,
                        ..SelectionEnvironment::default()
                    },
                )
                .unwrap();
            }
            Composition::Needs(requests) => {
                for request in requests {
                    let mut identity = std::str::from_utf8(&request.target_token)
                        .unwrap()
                        .replace('\\', "/")
                        .to_ascii_lowercase();
                    if !identity.starts_with("materials/") {
                        identity.insert_str(0, "materials/");
                    }
                    if !identity.ends_with(".vmt") {
                        identity.push_str(".vmt");
                    }
                    dependencies.push(DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: identity.clone(),
                        bytes: Some(files[identity.as_str()].clone()),
                    });
                }
            }
        }
    }
}
