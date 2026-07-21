use playsrc_keyvalues::ConditionEnvironment;
use playsrc_material::{
    AuthoredTexturePlane, BlendFactor, CompareFunction, FragmentDiscardRequirement, HdrMode,
    Material, ParticleMaterialInput, SelectionEnvironment, TextureAlphaFacts, TextureColorRead,
    TextureDisposition, TextureFace, TextureMagFilter, TextureMetadataManifest, TextureMinFilter,
    TextureSamplingState, TextureSubresourceIdentity, TextureWrapMode, WaterInputRequirement,
    WaterShaderVariant, bind_authored_texture_use, evaluate_water_material,
    resolve_for_environment, static_state, validate_authored_planes, water_material_output,
};
use playsrc_vmt::{Composition, DependencyResponse, Limits, compose};
use std::{collections::BTreeMap, fs, path::Path};

const ENTRY_COUNT: usize = 317;
const VMT_COUNT: usize = 108;
const VTF_COUNT: usize = 126;

#[test]
#[ignore = "requires configured build-24207079 jump_beef source bundle"]
fn configured_alpha_spritecard_and_water_semantics_are_exact() {
    let files = configured_bundle();
    assert_eq!(
        files.keys().filter(|path| path.ends_with(".vmt")).count(),
        VMT_COUNT
    );
    assert_eq!(
        files.keys().filter(|path| path.ends_with(".vtf")).count(),
        VTF_COUNT
    );

    let fence = resolved(
        &files,
        "materials/metal/metalfence007a.vmt",
        SelectionEnvironment::default(),
    );
    let fence_texture = source_base(&fence);
    assert_eq!(fence_texture.color_read, TextureColorRead::Srgb);
    let fence_bytes = exact(&files, fence_texture.logical_path.as_deref().unwrap());
    let fence_metadata = inspect(fence_bytes);
    assert!(has_alpha(fence_metadata));
    let fence_state = static_state(&fence, TextureAlphaFacts { base: true }).unwrap();
    assert_eq!(
        fence_state.alpha_test_function,
        CompareFunction::GreaterOrEqual
    );
    assert_eq!(fence_state.alpha_test_reference, 0.35);
    assert!(!fence_state.blend.enabled);
    assert!(fence_state.depth_write);
    assert!(fence_state.fragment_discard.discards(0.0));
    let fence_usage = fence
        .texture_uses
        .iter()
        .find(|usage| usage.role == playsrc_material::TextureRole::Base)
        .unwrap();
    let fence_manifest = manifest(&fence_metadata);
    let fence_binding =
        bind_authored_texture_use(fence_texture, fence_usage, &fence_manifest).unwrap();
    assert_eq!(fence_binding.mip_count, 10);
    assert_eq!(fence_binding.frame_count, 1);
    assert_eq!(fence_binding.initial_frame, Some(0));
    assert_eq!(fence_binding.sampling.wrap_s, TextureWrapMode::Repeat);
    assert_eq!(
        fence_binding.sampling.min_filter,
        TextureMinFilter::LinearMipmapNearest
    );

    let glass_paths = [
        "materials/maps/jump_beef/glass/glasswindow002a_-4787_3137_-2159.vmt",
        "materials/maps/jump_beef/glass/glasswindow002a_12672_539_-2562.vmt",
        "materials/maps/jump_beef/glass/glasswindow002a_12672_683_-4448.vmt",
    ];
    for path in glass_paths {
        let glass = resolved(&files, path, SelectionEnvironment::default());
        let state = static_state(&glass, TextureAlphaFacts { base: true }).unwrap();
        assert!(state.alpha_ownership.opacity);
        assert_eq!(state.blend.source, BlendFactor::SourceAlpha);
        assert_eq!(state.blend.destination, BlendFactor::OneMinusSourceAlpha);
        assert!(!state.depth_write);
        assert_eq!(state.fragment_discard, FragmentDiscardRequirement::None);
    }

    let mark_paths = (0..10)
        .map(|index| format!("materials/signs/number_{index:02}.vmt"))
        .chain(
            ["lt", "rt", "up"]
                .into_iter()
                .map(|direction| format!("materials/signs/arrow_{direction}_blue.vmt")),
        )
        .collect::<Vec<_>>();
    for path in &mark_paths {
        let mark = resolved(&files, path, SelectionEnvironment::default());
        let state = static_state(&mark, TextureAlphaFacts { base: true }).unwrap();
        assert!(state.alpha_ownership.opacity);
        assert!(state.alpha_ownership.vertex_alpha);
        assert!(state.blend.enabled);
        assert!(!state.depth_write);
        assert_eq!(state.fragment_discard, FragmentDiscardRequirement::None);
        let texture = source_base(&mark);
        assert_eq!(texture.color_read, TextureColorRead::Srgb);
        let bytes = exact(&files, texture.logical_path.as_deref().unwrap());
        let metadata = inspect(bytes);
        assert!(has_alpha(metadata));
    }

    let model_paths = files
        .keys()
        .filter(|path| path.starts_with("materials/models/") && path.ends_with(".vmt"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(model_paths.len(), 55);
    let mut alpha_models = 0;
    let mut alpha_test = 0;
    let mut phong_mask = 0;
    let mut tint_mask = 0;
    let mut self_illumination = 0;
    for path in model_paths {
        let material = resolved(
            &files,
            &path,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        );
        let base_alpha = material
            .textures
            .iter()
            .find(|texture| texture.role == playsrc_material::TextureRole::Base)
            .and_then(|texture| texture.logical_path.as_ref())
            .map(|path| inspect(exact(&files, path)))
            .is_some_and(has_alpha);
        alpha_models += usize::from(base_alpha);
        let state = static_state(&material, TextureAlphaFacts { base: base_alpha }).unwrap();
        if let Some(base) = source_base_optional(&material) {
            assert_eq!(base.color_read, TextureColorRead::Srgb);
        }
        alpha_test += usize::from(state.alpha_ownership.alpha_test);
        phong_mask += usize::from(state.alpha_ownership.phong_mask);
        tint_mask += usize::from(state.alpha_ownership.tint_mask);
        self_illumination += usize::from(state.alpha_ownership.self_illumination_mask);
    }
    assert_eq!(
        (
            alpha_models,
            alpha_test,
            phong_mask,
            tint_mask,
            self_illumination
        ),
        (19, 8, 8, 2, 1)
    );

    let particle_paths = [
        "materials/effects/rocketrailsmoke.vmt",
        "materials/effects/brightglow_y_nomodel.vmt",
        "materials/effects/sc_brightglow_y_nomodel.vmt",
        "materials/effects/smokelit2/smoke2lit.vmt",
        "materials/effects/sc_softglow.vmt",
        "materials/effects/circle2.vmt",
        "materials/effects/softglow_translucent.vmt",
        "materials/effects/debris/debris_chunk.vmt",
        "materials/effects/softglow.vmt",
        "materials/particle/smoke1/smoke1.vmt",
        "materials/effects/circle4.vmt",
        "materials/effects/circle3.vmt",
    ];
    let mut sprite_cards = 0;
    let mut additive = 0;
    let mut source_alpha = 0;
    let mut missing_depth_default = 0;
    for path in particle_paths {
        let material = resolved(&files, path, SelectionEnvironment::default());
        let base_alpha = source_base_optional(&material)
            .and_then(|texture| texture.logical_path.as_ref())
            .map(|path| inspect(exact(&files, path)))
            .is_some_and(has_alpha);
        let state = static_state(&material, TextureAlphaFacts { base: base_alpha }).unwrap();
        assert_eq!(source_base(&material).color_read, TextureColorRead::Srgb);
        if let Some(particle) = &material.particle {
            sprite_cards += 1;
            assert!(!particle.dual_sequence.value);
            assert_eq!(state.alpha_test_function, CompareFunction::Greater);
            assert_eq!(state.alpha_test_reference, 0.01);
            assert!(state.fragment_discard.discards(0.0));
            assert!(!state.depth_write);
            assert!(!particle.writes_destination_alpha);
            if particle.depth_blend.is_none() {
                missing_depth_default += 1;
                assert!(
                    particle
                        .required_inputs
                        .contains(&ParticleMaterialInput::SpriteCardDepthBlendDefault)
                );
            }
        }
        if state.blend.destination == BlendFactor::One {
            additive += 1;
        } else {
            source_alpha += 1;
        }
    }
    assert_eq!(
        (sprite_cards, additive, source_alpha, missing_depth_default),
        (8, 5, 7, 3)
    );

    let surface = resolved(
        &files,
        "materials/maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159.vmt",
        SelectionEnvironment {
            hdr_mode: HdrMode::Integer,
            ..SelectionEnvironment::default()
        },
    );
    let beneath = resolved(
        &files,
        "materials/water/water_2fort_beneath.vmt",
        SelectionEnvironment {
            hdr_mode: HdrMode::Integer,
            ..SelectionEnvironment::default()
        },
    );
    let surface_output = water_material_output(&surface).unwrap().unwrap();
    let beneath_output = water_material_output(&beneath).unwrap().unwrap();
    assert_eq!(surface_output.shader, WaterShaderVariant::Dx9Hdr);
    assert!(surface_output.textures.base.is_none());
    assert!(surface_output.textures.flow.is_none());
    assert!(surface_output.textures.environment.is_some());
    assert_eq!(
        surface_output.textures.normal.as_ref().unwrap().color_read,
        TextureColorRead::Linear
    );
    assert_eq!(
        surface_output
            .textures
            .environment
            .as_ref()
            .unwrap()
            .color_read,
        TextureColorRead::Srgb
    );
    assert_eq!(
        surface_output
            .textures
            .reflection
            .as_ref()
            .unwrap()
            .disposition,
        TextureDisposition::BuiltInRenderTarget
    );
    assert_eq!(
        surface_output
            .textures
            .refraction
            .as_ref()
            .unwrap()
            .disposition,
        TextureDisposition::BuiltInRenderTarget
    );
    assert_eq!(surface_output.reflect_amount.value, 0.25);
    assert_eq!(surface_output.refract_amount.value, 0.32);
    assert!(surface_output.above_water.value);
    assert!(surface_output.fog.enabled.as_ref().unwrap().value);
    assert_eq!(surface.proxy_program.entries.len(), 7);
    assert!(beneath_output.textures.environment.is_none());
    assert!(beneath_output.textures.reflection.is_none());
    assert!(beneath_output.textures.refraction.is_some());
    assert!(!beneath_output.above_water.value);
    assert_eq!(beneath_output.refract_amount.value, 0.5);
    assert!(beneath_output.blur_refraction.value);
    assert_eq!(beneath.proxy_program.entries.len(), 3);
    assert!(
        surface_output
            .required_inputs
            .contains(&WaterInputRequirement::WaterLodController)
    );

    let normal = surface_output.textures.normal.as_ref().unwrap();
    let normal_bytes = exact(&files, normal.logical_path.as_deref().unwrap());
    let normal_metadata = inspect(normal_bytes);
    assert_eq!(normal_metadata.frame_count, 60);
    assert_eq!(normal_metadata.mip_count, 9);
    let normal_manifest = manifest(&normal_metadata);
    let normal_usage = surface
        .texture_uses
        .iter()
        .find(|usage| usage.role == playsrc_material::TextureRole::Normal)
        .unwrap();
    let normal_binding = bind_authored_texture_use(normal, normal_usage, &normal_manifest).unwrap();
    assert_eq!(normal_binding.initial_frame, Some(0));
    assert_eq!(normal_binding.subresources.len(), 540);
    assert_eq!(normal_binding.sampling.wrap_s, TextureWrapMode::Repeat);
    assert_eq!(normal_binding.sampling.wrap_t, TextureWrapMode::Repeat);
    assert_eq!(
        normal_binding.sampling.min_filter,
        TextureMinFilter::LinearMipmapNearest
    );
    let normal_planes = authored_planes(&normal_binding, 3);
    validate_authored_planes(&normal_binding, &normal_planes).unwrap();

    let context = playsrc_material::ProxyEvaluationContext {
        time: 1.0,
        frame_time: 0.05,
        water_lod: Some([1000.0, 2000.0]),
        texture_frames: BTreeMap::from([(b"$normalmap".to_vec(), 60)]),
        ..playsrc_material::ProxyEvaluationContext::default()
    };
    let evaluated = evaluate_water_material(&surface, &context).unwrap();
    assert_eq!(evaluated.normal_frame, 30);
    assert_eq!(evaluated.cheap_start, 1000.0);
    assert_eq!(evaluated.cheap_end, 2000.0);

    let semantic = [
        fence_state.alpha_test_reference.to_bits(),
        204_175,
        alpha_models as u32,
        alpha_test as u32,
        phong_mask as u32,
        tint_mask as u32,
        self_illumination as u32,
        sprite_cards as u32,
        additive as u32,
        source_alpha as u32,
        missing_depth_default as u32,
        surface.proxy_program.entries.len() as u32,
        beneath.proxy_program.entries.len() as u32,
        normal_binding.subresources.len() as u32,
        evaluated.normal_frame as u32,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    assert_eq!(
        hex(&sha256(&semantic)),
        "ccac4488cf11a82cd43511bdf83300bf799b3788ad0448481736d5118cc64f2a"
    );
}

fn configured_bundle() -> BTreeMap<String, Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    let config = fs::read_to_string(root.join("playsrc.local.json")).unwrap();
    let source_cache = json_string(&config, "sourceCacheDir");
    let bytes = playsrc_asset_graph::read_resource_set(
        &Path::new(&source_cache).join("browser-bundles/jump_beef.graph.json"),
        None,
    )
    .unwrap();
    parse_bundle(&bytes)
}

fn parse_bundle(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    assert_eq!(&bytes[..4], b"PSRE");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 1);
    let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(count, ENTRY_COUNT);
    let mut offset = 12;
    let mut files = BTreeMap::new();
    for _ in 0..count {
        let path = field(bytes, &mut offset);
        let path = std::str::from_utf8(path).unwrap().to_ascii_lowercase();
        let data = field(bytes, &mut offset).to_vec();
        assert!(files.insert(path, data).is_none());
    }
    assert_eq!(offset, bytes.len());
    files
}

fn field<'a>(bytes: &'a [u8], offset: &mut usize) -> &'a [u8] {
    let length = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap()) as usize;
    *offset += 4;
    let value = &bytes[*offset..*offset + length];
    *offset += length;
    value
}

fn resolved(
    files: &BTreeMap<String, Vec<u8>>,
    identity: &str,
    environment: SelectionEnvironment,
) -> Material {
    let identity = identity.to_ascii_lowercase();
    let root = exact(files, &identity).to_vec();
    let mut responses = Vec::new();
    loop {
        match compose(
            &root,
            identity.clone(),
            &responses,
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap()
        {
            Composition::Complete(document) => {
                return resolve_for_environment(&document, environment).unwrap();
            }
            Composition::Needs(requests) => {
                for request in requests {
                    let path = material_path(&request.target_token);
                    responses.push(DependencyResponse {
                        parent_identity: request.parent_identity,
                        target_token: request.target_token,
                        canonical_identity: path.clone(),
                        bytes: Some(exact(files, &path).to_vec()),
                    });
                }
            }
        }
    }
}

fn material_path(token: &[u8]) -> String {
    let mut path = std::str::from_utf8(token).unwrap().replace('\\', "/");
    if !path.to_ascii_lowercase().starts_with("materials/") {
        path = format!("materials/{path}");
    }
    if !path.to_ascii_lowercase().ends_with(".vmt") {
        path.push_str(".vmt");
    }
    path.to_ascii_lowercase()
}

fn exact<'a>(files: &'a BTreeMap<String, Vec<u8>>, path: &str) -> &'a [u8] {
    files.get(&path.to_ascii_lowercase()).unwrap()
}

fn source_base(material: &Material) -> &playsrc_material::TextureRequest {
    source_base_optional(material).unwrap()
}

fn source_base_optional(material: &Material) -> Option<&playsrc_material::TextureRequest> {
    material
        .textures
        .iter()
        .find(|texture| texture.role == playsrc_material::TextureRole::Base)
}

#[derive(Clone, Copy)]
struct VtfMetadata {
    width: u32,
    height: u32,
    frame_count: u16,
    mip_count: u8,
    flags: u32,
}

fn inspect(bytes: &[u8]) -> VtfMetadata {
    assert_eq!(&bytes[..4], b"VTF\0");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 7);
    VtfMetadata {
        width: u16::from_le_bytes(bytes[16..18].try_into().unwrap()).into(),
        height: u16::from_le_bytes(bytes[18..20].try_into().unwrap()).into(),
        flags: u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
        frame_count: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
        mip_count: bytes[56],
    }
}

fn has_alpha(metadata: VtfMetadata) -> bool {
    metadata.flags & 0x3000 != 0
}

fn manifest(metadata: &VtfMetadata) -> TextureMetadataManifest {
    let point = metadata.flags & 0x1 != 0;
    let mipmapped = !point && metadata.flags & 0x100 == 0;
    let anisotropic = mipmapped && metadata.flags & 0x10 != 0;
    let min_filter = if point {
        TextureMinFilter::Nearest
    } else if !mipmapped {
        TextureMinFilter::Linear
    } else if anisotropic {
        TextureMinFilter::Anisotropic
    } else if metadata.flags & 0x2 != 0 {
        TextureMinFilter::LinearMipmapLinear
    } else {
        TextureMinFilter::LinearMipmapNearest
    };
    let mag_filter = if point {
        TextureMagFilter::Nearest
    } else if anisotropic {
        TextureMagFilter::Anisotropic
    } else {
        TextureMagFilter::Linear
    };
    TextureMetadataManifest {
        width: metadata.width,
        height: metadata.height,
        depth: 1,
        mip_count: metadata.mip_count,
        frame_count: metadata.frame_count,
        faces: vec![TextureFace::Right],
        sampling: TextureSamplingState {
            wrap_s: wrap(metadata.flags, 0x4),
            wrap_t: wrap(metadata.flags, 0x8),
            wrap_u: wrap(metadata.flags, 0x0200_0000),
            min_filter,
            mag_filter,
            anisotropy_level: if anisotropic { 4 } else { 1 },
            mipmapped,
            no_lod: metadata.flags & 0x200 != 0,
            all_mips: metadata.flags & 0x400 != 0,
        },
        subresources: (0..metadata.mip_count)
            .rev()
            .flat_map(|mip| {
                (0..metadata.frame_count).map(move |frame| TextureSubresourceIdentity {
                    mip,
                    frame,
                    face: TextureFace::Right,
                    slice: 0,
                })
            })
            .collect(),
    }
}

fn authored_planes(
    binding: &playsrc_material::AuthoredTextureBinding,
    bytes_per_pixel: usize,
) -> Vec<AuthoredTexturePlane> {
    binding
        .subresources
        .iter()
        .map(|identity| {
            let width = (binding.width >> identity.mip).max(1);
            let height = (binding.height >> identity.mip).max(1);
            let row_stride = width as usize * bytes_per_pixel;
            AuthoredTexturePlane {
                identity: *identity,
                width,
                height,
                row_stride,
                sample_bytes: row_stride * height as usize,
            }
        })
        .collect()
}

fn wrap(flags: u32, clamp_flag: u32) -> TextureWrapMode {
    if flags & 0x2000_0000 != 0 {
        TextureWrapMode::Border
    } else if flags & clamp_flag != 0 {
        TextureWrapMode::Clamp
    } else {
        TextureWrapMode::Repeat
    }
}

fn json_string(document: &str, key: &str) -> String {
    let key = format!("\"{key}\"");
    let after_key = document.split_once(&key).unwrap().1;
    let after_colon = after_key.split_once(':').unwrap().1.trim_start();
    let value = after_colon.strip_prefix('"').unwrap();
    let end = value.find('"').unwrap();
    assert!(!value[..end].contains('\\'));
    value[..end].to_owned()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = [
        0x6a09e667_u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_length = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((bytes.len() + 72) & !63);
    padded.extend_from_slice(bytes);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(chunk[index * 4..index * 4 + 4].try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let big1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let first = h
                .wrapping_add(big1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let big0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = big0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut output = [0_u8; 32];
    for (index, value) in state.into_iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
