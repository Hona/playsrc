use crate::{
    BlendEquation, BlendFactor, BlendState, CompareFunction, EffectiveParameter, Error, Material,
    ModelShaderState, ParameterOrigin, PhongMaskSource, SelectionEnvironment, SelfIllumMaskSource,
    TextureAlphaFacts, TextureDisposition, TextureRequest, TextureRole, float_or, get, integer_or,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FragmentAlphaSource {
    BaseTextureOrOne,
    ShaderOutput,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FragmentDiscardRequirement {
    None,
    Alpha {
        source: FragmentAlphaSource,
        pass: CompareFunction,
        reference: f32,
    },
}

impl FragmentDiscardRequirement {
    pub fn discards(self, alpha: f32) -> bool {
        match self {
            Self::None => false,
            Self::Alpha {
                pass: CompareFunction::Greater,
                reference,
                ..
            } => alpha.partial_cmp(&reference) != Some(core::cmp::Ordering::Greater),
            Self::Alpha {
                pass: CompareFunction::GreaterOrEqual,
                reference,
                ..
            } => !matches!(
                alpha.partial_cmp(&reference),
                Some(core::cmp::Ordering::Greater | core::cmp::Ordering::Equal)
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AlphaOwnership {
    pub base_texture_available: bool,
    pub opacity: bool,
    pub alpha_test: bool,
    pub self_illumination_mask: bool,
    pub environment_mask: bool,
    pub phong_mask: bool,
    pub tint_mask: bool,
    pub vertex_alpha: bool,
    pub material_alpha_modulation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticleMaterialInput {
    AuthoredTexturePlanes,
    SpriteCardDepthBlendDefault,
    Viewport,
    CameraMatrices,
    SceneDepth,
    VertexColor,
    VertexAlpha,
    SheetCoordinates,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticleMaterialShader {
    SpriteCard,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleMaterialState {
    pub shader: ParticleMaterialShader,
    pub base: Option<TextureRequest>,
    pub base_frame: EffectiveParameter<i32>,
    pub orientation: EffectiveParameter<i32>,
    pub dual_sequence: EffectiveParameter<bool>,
    pub sequence_blend_mode: EffectiveParameter<i32>,
    pub blend_frames: EffectiveParameter<bool>,
    pub maximum_luminance_frame_blend: [EffectiveParameter<bool>; 2],
    pub depth_blend: Option<EffectiveParameter<bool>>,
    pub depth_blend_scale: EffectiveParameter<f32>,
    pub add_base_texture_2: EffectiveParameter<f32>,
    pub add_self: EffectiveParameter<f32>,
    pub add_over_blend: EffectiveParameter<bool>,
    pub extract_green_alpha: EffectiveParameter<bool>,
    pub overbright_factor: EffectiveParameter<f32>,
    pub zoom_sequence_2: EffectiveParameter<f32>,
    pub minimum_size: EffectiveParameter<f32>,
    pub start_fade_size: EffectiveParameter<f32>,
    pub end_fade_size: EffectiveParameter<f32>,
    pub maximum_size: EffectiveParameter<f32>,
    pub maximum_distance: EffectiveParameter<f32>,
    pub far_fade_interval: EffectiveParameter<f32>,
    pub use_instancing: EffectiveParameter<bool>,
    pub spline_type: EffectiveParameter<i32>,
    pub alpha_test: bool,
    pub writes_destination_alpha: bool,
    pub required_inputs: Vec<ParticleMaterialInput>,
}

pub fn missing_particle_material_inputs(
    state: &ParticleMaterialState,
    available: &[ParticleMaterialInput],
) -> Vec<ParticleMaterialInput> {
    state
        .required_inputs
        .iter()
        .copied()
        .filter(|required| !available.contains(required))
        .collect()
}

pub(crate) fn resolve_particle_state(
    shader_token: &[u8],
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    textures: &[TextureRequest],
    environment: SelectionEnvironment,
) -> Result<Option<ParticleMaterialState>, Error> {
    if !shader_token.eq_ignore_ascii_case(b"SpriteCard") {
        return Ok(None);
    }

    let add_base_texture_2 = effective_float(
        parameters,
        b"$addbasetexture2",
        0.0,
        ParameterOrigin::ShaderInitializer,
    )?;
    let add_self = effective_float(
        parameters,
        b"$addself",
        0.0,
        ParameterOrigin::ShaderInitializer,
    )?;
    let add_over_blend = effective_bool(
        parameters,
        b"$addoverblend",
        false,
        ParameterOrigin::ShaderInitializer,
    );
    let authored_depth_blend = get(parameters, b"$depthblend").is_some();
    let depth_blend = if authored_depth_blend {
        Some(effective_bool(
            parameters,
            b"$depthblend",
            false,
            ParameterOrigin::Authored,
        ))
    } else if !environment.pixel_shader_2_b {
        Some(EffectiveParameter {
            value: false,
            origin: ParameterOrigin::ShaderInitializer,
        })
    } else {
        environment
            .sprite_card_default_depth_blend
            .map(|value| EffectiveParameter {
                value,
                origin: ParameterOrigin::ShaderInitializer,
            })
    };
    let orientation = effective_integer(
        parameters,
        b"$orientation",
        0,
        ParameterOrigin::TypeInitializer,
    )?;
    let alpha_test = add_base_texture_2.value == 0.0 && add_self.value == 0.0;

    let base = textures
        .iter()
        .find(|texture| texture.role == TextureRole::Base)
        .cloned();
    let mut required_inputs = Vec::new();
    if base
        .as_ref()
        .is_some_and(|texture| texture.disposition == TextureDisposition::Source)
    {
        required_inputs.push(ParticleMaterialInput::AuthoredTexturePlanes);
    }
    if depth_blend.is_none() {
        required_inputs.push(ParticleMaterialInput::SpriteCardDepthBlendDefault);
    }
    required_inputs.push(ParticleMaterialInput::Viewport);
    if orientation.value.clamp(0, 2) == 0 {
        required_inputs.push(ParticleMaterialInput::CameraMatrices);
    }
    if depth_blend.as_ref().is_some_and(|value| value.value) {
        required_inputs.push(ParticleMaterialInput::SceneDepth);
    }
    required_inputs.extend([
        ParticleMaterialInput::VertexColor,
        ParticleMaterialInput::VertexAlpha,
        ParticleMaterialInput::SheetCoordinates,
    ]);

    Ok(Some(ParticleMaterialState {
        shader: ParticleMaterialShader::SpriteCard,
        base,
        base_frame: effective_integer(parameters, b"$frame", 0, ParameterOrigin::TypeInitializer)?,
        orientation,
        dual_sequence: effective_bool(
            parameters,
            b"$dualsequence",
            false,
            ParameterOrigin::ShaderInitializer,
        ),
        sequence_blend_mode: effective_integer(
            parameters,
            b"$sequence_blend_mode",
            0,
            ParameterOrigin::TypeInitializer,
        )?,
        blend_frames: effective_bool(
            parameters,
            b"$blendframes",
            true,
            ParameterOrigin::ShaderInitializer,
        ),
        maximum_luminance_frame_blend: [
            effective_bool(
                parameters,
                b"$maxlumframeblend1",
                false,
                ParameterOrigin::ShaderInitializer,
            ),
            effective_bool(
                parameters,
                b"$maxlumframeblend2",
                false,
                ParameterOrigin::ShaderInitializer,
            ),
        ],
        depth_blend,
        depth_blend_scale: effective_float(
            parameters,
            b"$depthblendscale",
            50.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        add_base_texture_2,
        add_self,
        add_over_blend,
        extract_green_alpha: effective_bool(
            parameters,
            b"$extractgreenalpha",
            false,
            ParameterOrigin::ShaderInitializer,
        ),
        overbright_factor: effective_float(
            parameters,
            b"$overbrightfactor",
            1.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        zoom_sequence_2: effective_float(
            parameters,
            b"$zoomanimateseq2",
            0.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        minimum_size: effective_float(
            parameters,
            b"$minsize",
            0.0,
            ParameterOrigin::TypeInitializer,
        )?,
        start_fade_size: effective_float(
            parameters,
            b"$startfadesize",
            10.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        end_fade_size: effective_float(
            parameters,
            b"$endfadesize",
            20.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        maximum_size: effective_float(
            parameters,
            b"$maxsize",
            20.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        maximum_distance: effective_float(
            parameters,
            b"$maxdistance",
            100_000.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        far_fade_interval: effective_float(
            parameters,
            b"$farfadeinterval",
            400.0,
            ParameterOrigin::ShaderInitializer,
        )?,
        use_instancing: effective_bool(
            parameters,
            b"$useinstancing",
            false,
            ParameterOrigin::ShaderInitializer,
        ),
        spline_type: effective_integer(
            parameters,
            b"$splinetype",
            0,
            ParameterOrigin::TypeInitializer,
        )?,
        alpha_test,
        writes_destination_alpha: false,
        required_inputs,
    }))
}

pub(crate) fn sprite_card_blend(material: &Material) -> Result<BlendState, Error> {
    let particle = material
        .particle
        .as_ref()
        .ok_or_else(|| crate::error(crate::ErrorCode::InvalidParameter, None))?;
    let premultiplied = particle.add_base_texture_2.value != 0.0
        || particle.add_over_blend.value
        || particle.add_self.value != 0.0;
    let (source, destination) = if premultiplied {
        (BlendFactor::One, BlendFactor::OneMinusSourceAlpha)
    } else if material.features.additive {
        (BlendFactor::SourceAlpha, BlendFactor::One)
    } else {
        (BlendFactor::SourceAlpha, BlendFactor::OneMinusSourceAlpha)
    };
    Ok(BlendState {
        enabled: true,
        equation: BlendEquation::Add,
        source,
        destination,
    })
}

pub(crate) fn alpha_ownership(
    material: &Material,
    texture_alpha: TextureAlphaFacts,
    alpha_test: bool,
    effective_alpha: f32,
) -> AlphaOwnership {
    let base_texture_available = texture_alpha.base;
    let self_illumination_mask = base_texture_available
        && material.features.self_illum
        && material.model.as_ref().map_or_else(
            || {
                !material
                    .textures
                    .iter()
                    .any(|texture| texture.role == TextureRole::SelfIllumMask)
            },
            |model| {
                if matches!(model.state, ModelShaderState::UnlitGeneric(_)) {
                    return !material
                        .textures
                        .iter()
                        .any(|texture| texture.role == TextureRole::SelfIllumMask);
                }
                let ModelShaderState::VertexLitGeneric(state) = &model.state else {
                    return false;
                };
                state
                    .self_illumination
                    .as_ref()
                    .is_some_and(|state| state.source == SelfIllumMaskSource::BaseAlpha)
            },
        );
    let environment_mask = base_texture_available && material.features.base_alpha_environment_mask;
    let phong_mask = base_texture_available
        && material.model.as_ref().is_some_and(|model| {
            let ModelShaderState::VertexLitGeneric(state) = &model.state else {
                return false;
            };
            state
                .phong
                .as_ref()
                .is_some_and(|state| state.mask_source == PhongMaskSource::BaseAlpha)
        });
    let tint_mask = base_texture_available
        && crate::boolean(&material.first_parameters, b"$blendtintbybasealpha");
    let alpha_test_owner = base_texture_available && alpha_test;
    let opacity = if material.particle.is_some() {
        base_texture_available
    } else {
        base_texture_available
            && !alpha_test_owner
            && !self_illumination_mask
            && !environment_mask
            && !tint_mask
            && !material.features.opaque_texture
            && material.features.translucent
    };
    AlphaOwnership {
        base_texture_available,
        opacity,
        alpha_test: alpha_test_owner,
        self_illumination_mask,
        environment_mask,
        phong_mask,
        tint_mask,
        vertex_alpha: material.features.vertex_alpha || material.particle.is_some(),
        material_alpha_modulation: effective_alpha < 1.0,
    }
}

fn effective_bool(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: bool,
    default_origin: ParameterOrigin,
) -> EffectiveParameter<bool> {
    match get(parameters, parameter) {
        Some(value) => EffectiveParameter {
            value: crate::source_integer(value) != 0,
            origin: ParameterOrigin::Authored,
        },
        None => EffectiveParameter {
            value: default,
            origin: default_origin,
        },
    }
}

fn effective_integer(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: i32,
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<i32>, Error> {
    Ok(EffectiveParameter {
        value: integer_or(parameters, parameter, default)?,
        origin: if get(parameters, parameter).is_some() {
            ParameterOrigin::Authored
        } else {
            default_origin
        },
    })
}

fn effective_float(
    parameters: &BTreeMap<Vec<u8>, Vec<u8>>,
    parameter: &[u8],
    default: f32,
    default_origin: ParameterOrigin,
) -> Result<EffectiveParameter<f32>, Error> {
    Ok(EffectiveParameter {
        value: float_or(parameters, parameter, default)?,
        origin: if get(parameters, parameter).is_some() {
            ParameterOrigin::Authored
        } else {
            default_origin
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CullState, DepthFunction, ErrorCode, FragmentDiscardRequirement, PolygonOffset,
        TextureFace, TextureFrameSelection, TextureMagFilter, TextureMetadataManifest,
        TextureMinFilter, TextureSamplingState, TextureSubresourceIdentity, TextureWrapMode,
        bind_authored_texture_use, resolve_for_environment, static_state,
    };
    use playsrc_keyvalues::ConditionEnvironment;
    use playsrc_vmt::{Composition, Limits, compose};

    fn material(bytes: &[u8], environment: SelectionEnvironment) -> Material {
        let Composition::Complete(document) = compose(
            bytes,
            "materials/alpha.vmt",
            &[],
            &ConditionEnvironment::default(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("test material requested a dependency")
        };
        resolve_for_environment(&document, environment).unwrap()
    }

    #[test]
    fn alpha_ownership_and_fragment_discard_are_typed_and_shader_qualified() {
        let fence = material(
            br#"LightmappedGeneric {
                "$basetexture" "metal/fence"
                "$alphatest" "1"
                "$alphatestreference" "0.35"
            }"#,
            SelectionEnvironment::default(),
        );
        let state = static_state(&fence, TextureAlphaFacts { base: true }).unwrap();
        assert_eq!(
            state.fragment_discard,
            FragmentDiscardRequirement::Alpha {
                source: FragmentAlphaSource::BaseTextureOrOne,
                pass: CompareFunction::GreaterOrEqual,
                reference: 0.35,
            }
        );
        assert!(state.fragment_discard.discards(0.0));
        assert!(state.fragment_discard.discards(0.349));
        assert!(!state.fragment_discard.discards(0.35));
        assert!(state.alpha_ownership.alpha_test);
        assert!(!state.alpha_ownership.opacity);
        assert!(!state.blend.enabled);
        assert!(state.depth_write);

        let decal = material(
            br#"LightmappedGeneric {
                "$basetexture" "signs/number_01"
                "$decal" "1" "$translucent" "1"
                "$vertexcolor" "1" "$vertexalpha" "1"
            }"#,
            SelectionEnvironment::default(),
        );
        let state = static_state(&decal, TextureAlphaFacts { base: true }).unwrap();
        assert_eq!(state.fragment_discard, FragmentDiscardRequirement::None);
        assert!(state.alpha_ownership.opacity);
        assert!(state.alpha_ownership.vertex_alpha);
        assert_eq!(state.polygon_offset, PolygonOffset::Decal);
        assert!(!state.depth_write);
        assert_eq!(state.depth_function, DepthFunction::NearerOrEqual);
        assert_eq!(state.cull, CullState::Back);

        let model = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/item"
                "$phong" "1" "$basemapalphaphongmask" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        );
        let state = static_state(&model, TextureAlphaFacts { base: true }).unwrap();
        assert!(state.alpha_ownership.phong_mask);
        assert!(!state.alpha_ownership.opacity);

        let tint = material(
            br#"VertexLitGeneric {
                "$basetexture" "models/item"
                "$blendtintbybasealpha" "1"
            }"#,
            SelectionEnvironment {
                model: true,
                ..SelectionEnvironment::default()
            },
        );
        assert!(
            static_state(&tint, TextureAlphaFacts { base: true })
                .unwrap()
                .alpha_ownership
                .tint_mask
        );

        let self_illuminated = material(
            br#"UnlitGeneric { "$basetexture" "effects/light" "$selfillum" "1" }"#,
            SelectionEnvironment::default(),
        );
        assert!(
            static_state(&self_illuminated, TextureAlphaFacts { base: true })
                .unwrap()
                .alpha_ownership
                .self_illumination_mask
        );

        let forced_opaque = material(
            br#"UnlitGeneric {
                "$basetexture" "effects/opaque"
                "$translucent" "1" "$opaquetexture" "1"
            }"#,
            SelectionEnvironment::default(),
        );
        assert!(
            !static_state(&forced_opaque, TextureAlphaFacts { base: true })
                .unwrap()
                .alpha_ownership
                .opacity
        );
    }

    #[test]
    fn sprite_card_emits_forced_blend_discard_and_explicit_missing_default() {
        let sprite = material(
            br#"SpriteCard {
                "$basetexture" "effects/smoke"
                "$vertexcolor" "1" "$vertexalpha" "1"
            }"#,
            SelectionEnvironment::default(),
        );
        let particle = sprite.particle.as_ref().unwrap();
        assert_eq!(sprite.shader, crate::Shader::Sprite);
        assert_eq!(particle.shader, ParticleMaterialShader::SpriteCard);
        assert!(particle.depth_blend.is_none());
        assert!(
            particle
                .required_inputs
                .contains(&ParticleMaterialInput::SpriteCardDepthBlendDefault)
        );
        assert_eq!(particle.zoom_sequence_2.value, 0.0);
        assert_eq!(
            particle.zoom_sequence_2.origin,
            ParameterOrigin::ShaderInitializer
        );
        assert!(!particle.dual_sequence.value);
        assert!(particle.alpha_test);
        assert!(!particle.writes_destination_alpha);
        let state = static_state(&sprite, TextureAlphaFacts { base: true }).unwrap();
        assert_eq!(
            state.blend,
            BlendState {
                enabled: true,
                equation: BlendEquation::Add,
                source: BlendFactor::SourceAlpha,
                destination: BlendFactor::OneMinusSourceAlpha,
            }
        );
        assert_eq!(state.cull, CullState::None);
        assert!(!state.depth_write);
        assert_eq!(state.alpha_test_function, CompareFunction::Greater);
        assert_eq!(state.alpha_test_reference, 0.01);
        assert!(state.fragment_discard.discards(0.01));
        assert!(!state.fragment_discard.discards(0.010_001));

        let additive = material(
            br#"SpriteCard { "$basetexture" "effects/glow" "$additive" "1" }"#,
            SelectionEnvironment {
                sprite_card_default_depth_blend: Some(true),
                ..SelectionEnvironment::default()
            },
        );
        let particle = additive.particle.as_ref().unwrap();
        assert!(particle.depth_blend.as_ref().unwrap().value);
        assert!(
            particle
                .required_inputs
                .contains(&ParticleMaterialInput::SceneDepth)
        );
        assert_eq!(
            static_state(&additive, TextureAlphaFacts { base: false })
                .unwrap()
                .blend
                .destination,
            BlendFactor::One
        );

        let premultiplied = material(
            br#"SpriteCard { "$basetexture" "effects/glow" "$addoverblend" "1" }"#,
            SelectionEnvironment::default(),
        );
        let state = static_state(&premultiplied, TextureAlphaFacts { base: false }).unwrap();
        assert_eq!(state.blend.source, BlendFactor::One);
        assert_eq!(state.blend.destination, BlendFactor::OneMinusSourceAlpha);
        assert!(state.alpha_test);
    }

    #[test]
    fn texture_use_retains_frame_transform_proxy_mutation_and_authored_chain() {
        let selected = material(
            br#"LightmappedGeneric {
                "$basetexture" "test/alpha"
                "$frame" "1"
                "$basetexturetransform" "center .5 .5 scale 2 3 rotate 0 translate .25 -.25"
                Proxies {
                    AnimatedTexture {
                        "animatedtexturevar" "$basetexture"
                        "animatedtextureframenumvar" "$frame"
                    }
                }
            }"#,
            SelectionEnvironment::default(),
        );
        let usage = &selected.texture_uses[0];
        assert_eq!(usage.role, TextureRole::Base);
        assert_eq!(
            usage.frame,
            TextureFrameSelection::Static {
                parameter: b"$frame".to_vec(),
                initial: 1,
                proxy_mutated: true,
            }
        );
        assert_eq!(
            usage.transform.as_ref().unwrap().matrix,
            [
                2.0, 0.0, 0.0, -0.25, 0.0, 3.0, 0.0, -1.25, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]
        );
        let metadata = TextureMetadataManifest {
            width: 2,
            height: 2,
            depth: 1,
            mip_count: 1,
            frame_count: 2,
            faces: vec![TextureFace::Right],
            sampling: TextureSamplingState {
                wrap_s: TextureWrapMode::Clamp,
                wrap_t: TextureWrapMode::Repeat,
                wrap_u: TextureWrapMode::Repeat,
                min_filter: TextureMinFilter::LinearMipmapLinear,
                mag_filter: TextureMagFilter::Linear,
                anisotropy_level: 1,
                mipmapped: true,
                no_lod: false,
                all_mips: false,
            },
            subresources: vec![
                TextureSubresourceIdentity {
                    mip: 0,
                    frame: 0,
                    face: TextureFace::Right,
                    slice: 0,
                },
                TextureSubresourceIdentity {
                    mip: 0,
                    frame: 1,
                    face: TextureFace::Right,
                    slice: 0,
                },
            ],
        };
        let binding = bind_authored_texture_use(&selected.textures[0], usage, &metadata).unwrap();
        assert_eq!(binding.initial_frame, Some(1));
        assert_eq!(binding.sampling, metadata.sampling);
        let mut invalid = selected.clone();
        invalid.texture_uses[0].frame = TextureFrameSelection::Static {
            parameter: b"$frame".to_vec(),
            initial: 2,
            proxy_mutated: false,
        };
        assert_eq!(
            bind_authored_texture_use(&invalid.textures[0], &invalid.texture_uses[0], &metadata,)
                .unwrap_err()
                .code,
            ErrorCode::InvalidTextureMetadata
        );
    }
}
