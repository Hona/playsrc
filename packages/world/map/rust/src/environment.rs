use super::{CanonicalMap, LightingMember, LightingProfile, Surface, face_positions};
use playsrc_bsp::{Bsp, Face, Leaf, LumpData, TextureInfo};
use playsrc_entity::{Entity, Graph};
use playsrc_material::{DecalState, Material, TextureDisposition, TextureRole, WaterState};
use playsrc_visibility::World as VisibilityWorld;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const SURF_SKY_2D: i32 = 0x0002;
const SURF_SKY: i32 = 0x0004;
const SURF_WARP: i32 = 0x0008;
const SURF_NODECALS: i32 = 0x2000;
const INFODECAL_TRACE_EXTENT: f32 = 5.0;
const INFODECAL_PLANE_DISTANCE: f32 = 4.0;
const MARK_NORMAL_OFFSET: f32 = 0.1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentLimits {
    pub max_cubemaps: usize,
    pub max_water_surfaces: usize,
    pub max_water_volumes: usize,
    pub max_marks: usize,
    pub max_fragments: usize,
    pub max_fragment_vertices: usize,
}

impl Default for EnvironmentLimits {
    fn default() -> Self {
        Self {
            max_cubemaps: 65_536,
            max_water_surfaces: 2_000_000,
            max_water_volumes: 32_768,
            max_marks: 16_384,
            max_fragments: 65_536,
            max_fragment_vertices: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentErrorCode {
    InvalidMapPath,
    MissingLump,
    InvalidRecord,
    InvalidReference,
    InvalidField,
    NonFinite,
    BoundExceeded,
    MissingDependency,
    DependencyMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentError {
    pub code: EnvironmentErrorCode,
    pub item: Option<usize>,
    pub field: Option<Vec<u8>>,
    pub logical_path: Option<String>,
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?}", self.code)
    }
}

impl std::error::Error for EnvironmentError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CubeFace {
    Right,
    Left,
    Back,
    Front,
    Up,
    Down,
}

impl CubeFace {
    const ALL: [Self; 6] = [
        Self::Right,
        Self::Left,
        Self::Back,
        Self::Front,
        Self::Up,
        Self::Down,
    ];

    const fn suffix(self) -> &'static str {
        match self {
            Self::Right => "rt",
            Self::Left => "lf",
            Self::Back => "bk",
            Self::Front => "ft",
            Self::Up => "up",
            Self::Down => "dn",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRole {
    SkyMaterial(CubeFace),
    CubemapTexture { sample: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRequest {
    pub role: DependencyRole,
    pub profile: LightingProfile,
    pub logical_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTexture {
    pub logical_path: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SkyEncoding {
    Srgb = 0,
    Linear = 1,
    HdrRgbs = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyMetadata {
    SkyMaterial {
        source_sha256: [u8; 32],
        encoding: SkyEncoding,
        selected_textures: Vec<ResolvedTexture>,
    },
    CubemapTexture {
        source_sha256: [u8; 32],
        width: u32,
        height: u32,
        mip_count: u8,
        source_face_count: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyResponse {
    pub request: DependencyRequest,
    pub metadata: DependencyMetadata,
}

#[derive(Clone, Copy)]
pub struct MaterialBinding<'a> {
    pub material_index: usize,
    pub material: &'a Material,
}

#[derive(Clone, Copy)]
pub struct NamedMaterialBinding<'a> {
    pub logical_path: &'a str,
    pub material: &'a Material,
}

#[derive(Clone, Debug)]
pub struct MarkMaterial {
    pub reference: Vec<u8>,
    pub logical_path: String,
    pub source_sha256: [u8; 32],
    pub width: u32,
    pub height: u32,
    pub state: DecalState,
}

pub struct EnvironmentInputs<'a> {
    pub logical_map_path: &'a str,
    pub entities: &'a Graph,
    pub visibility: &'a VisibilityWorld,
    pub collision: &'a playsrc_collision::World,
    pub receiver_snapshot: &'a playsrc_collision::Snapshot,
    pub mark_placements: &'a MarkPlacementSnapshot,
    pub materials: &'a [MaterialBinding<'a>],
    pub dependent_materials: &'a [NamedMaterialBinding<'a>],
    pub mark_materials: &'a [MarkMaterial],
    pub dependencies: &'a [DependencyResponse],
    pub limits: EnvironmentLimits,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkPlacement {
    pub entity: usize,
    pub world_origin: [f32; 3],
    pub parent_entity: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkPlacementSnapshot {
    pub revision: u64,
    pub placements: Vec<MarkPlacement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LightingProvenance {
    pub profile: LightingProfile,
    pub closure_sha256: [u8; 32],
    pub members: Vec<LightingMember>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyTextureDependency {
    pub logical_path: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyFace {
    pub face: CubeFace,
    pub material_path: String,
    pub material_sha256: [u8; 32],
    pub encoding: SkyEncoding,
    pub selected_textures: Vec<SkyTextureDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkyEnvironment {
    pub name: Vec<u8>,
    pub profile: LightingProfile,
    pub surface_faces: Vec<usize>,
    pub faces: Vec<SkyFace>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CubemapFaceDependency {
    pub face: CubeFace,
    pub mip_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CubemapSample {
    pub index: usize,
    pub origin: [i32; 3],
    pub encoded_size: u8,
    pub requested_dimension: Option<u32>,
    pub profile: LightingProfile,
    pub logical_path: String,
    pub source_sha256: [u8; 32],
    pub width: u32,
    pub height: u32,
    pub source_face_count: u8,
    pub faces: Vec<CubemapFaceDependency>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterSurface {
    pub profile: LightingProfile,
    pub selected: bool,
    pub face: usize,
    pub model: usize,
    pub material: usize,
    pub texture_info: usize,
    pub fog_volume: Option<usize>,
    pub plane: [f32; 4],
    pub bounds: [[f32; 3]; 2],
    pub bindings: WaterBindings,
    pub state: WaterState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterVolume {
    pub index: usize,
    pub surface_z: f32,
    pub minimum_z: f32,
    pub texture_info: usize,
    pub surface_material: usize,
    pub bottom_material: Option<WaterMaterialIdentity>,
    pub leaves: Vec<usize>,
    pub clusters: Vec<i16>,
    pub areas: Vec<usize>,
    pub contents: u32,
    pub bounds: [[f32; 3]; 2],
    pub plane: [f32; 4],
    pub surface_bindings: WaterBindings,
    pub bottom_bindings: Option<WaterBindings>,
    pub surface_state: WaterState,
    pub bottom_state: Option<WaterState>,
    pub surface_translucent: bool,
    pub bottom_translucent: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaterMaterialIdentity {
    Map(usize),
    Dependency(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CubemapSelection {
    Nearest { sample: usize },
    Declared { sample: usize },
    External { logical_path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterBindings {
    pub environment: Option<CubemapSelection>,
    pub reflection: bool,
    pub refraction: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterPointFact {
    Outside,
    Above { volume: usize },
    Below { volume: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterEnvironment {
    pub surfaces: Vec<WaterSurface>,
    pub volumes: Vec<WaterVolume>,
    pub leaf_minimum_distance_to_water: Option<Vec<u16>>,
}

impl WaterEnvironment {
    pub fn classify_leaf_height(
        &self,
        leaf: usize,
        height: f32,
    ) -> Result<WaterPointFact, EnvironmentError> {
        if !height.is_finite() {
            return Err(failure(EnvironmentErrorCode::NonFinite, None));
        }
        let Some(volume) = self
            .volumes
            .iter()
            .find(|volume| volume.leaves.contains(&leaf))
        else {
            return Ok(WaterPointFact::Outside);
        };
        Ok(if height < volume.surface_z {
            WaterPointFact::Below {
                volume: volume.index,
            }
        } else {
            WaterPointFact::Above {
                volume: volume.index,
            }
        })
    }

    pub fn plan_view(
        &self,
        visibility: &VisibilityWorld,
        input: WaterViewInput<'_>,
    ) -> Result<WaterViewPlan, EnvironmentError> {
        let visible = self.visible_water(visibility, &input)?;
        let render = water_render_selection(visible.as_ref(), input.policy);
        let mut passes = Vec::new();
        let Some(water) = visible.as_ref() else {
            passes.push(simple_view_pass(&input, None, &render));
            return Ok(WaterViewPlan {
                visible_water: None,
                render,
                passes,
            });
        };
        if render.cheap {
            passes.push(simple_view_pass(&input, Some(water), &render));
            return Ok(WaterViewPlan {
                visible_water: visible,
                render,
                passes,
            });
        }
        let (adjusted_height, water_z_adjust, software_clip) =
            water_eye_adjustment(input.origin[2], water.surface_z, input.policy)?;
        if !water.eye_in_volume {
            if render.reflect {
                let mut reflected_origin = input.origin;
                reflected_origin[2] -= 2.0 * (reflected_origin[2] - water.surface_z);
                passes.push(EnvironmentViewPass {
                    kind: EnvironmentViewKind::Reflection,
                    origin: reflected_origin,
                    angles: [-input.angles[0], input.angles[1], -input.angles[2]],
                    render_above_water: true,
                    render_under_water: false,
                    render_water_surface: false,
                    draw_entities: render.reflect_entities,
                    draw_sky_2d: true,
                    clip: input.policy.height_clipping.then_some(WaterClipPlane {
                        height: water.surface_z - WATER_CLIP_SPREAD,
                        keep: WaterClipKeep::Above,
                    }),
                    forced_visibility_leaf: Some(water.visible_leaf),
                    fog: ViewFog::World,
                });
            }
            if render.refract {
                passes.push(EnvironmentViewPass {
                    kind: EnvironmentViewKind::Refraction,
                    origin: input.origin,
                    angles: input.angles,
                    render_above_water: false,
                    render_under_water: true,
                    render_water_surface: false,
                    draw_entities: true,
                    draw_sky_2d: false,
                    clip: input.policy.height_clipping.then_some(WaterClipPlane {
                        height: adjusted_height + WATER_CLIP_SPREAD,
                        keep: WaterClipKeep::Below,
                    }),
                    forced_visibility_leaf: None,
                    fog: ViewFog::Water {
                        volume: water.volume,
                        height_fog: true,
                    },
                });
            }
            passes.push(EnvironmentViewPass {
                kind: EnvironmentViewKind::Main,
                origin: input.origin,
                angles: input.angles,
                render_above_water: true,
                render_under_water: !render.refract && !render.opaque,
                render_water_surface: render.draw_surface,
                draw_entities: true,
                draw_sky_2d: input.draw_sky_2d,
                clip: (input.near_plane_intersects_selected_volume
                    && !input.policy.fast_clipping
                    && input.policy.height_clipping)
                    .then_some(WaterClipPlane {
                        height: adjusted_height - WATER_CLIP_SPREAD,
                        keep: WaterClipKeep::Above,
                    }),
                forced_visibility_leaf: None,
                fog: ViewFog::World,
            });
            if render.refract && (software_clip || input.near_plane_intersects_selected_volume) {
                passes.push(EnvironmentViewPass {
                    kind: EnvironmentViewKind::Intersection,
                    origin: input.origin,
                    angles: input.angles,
                    render_above_water: false,
                    render_under_water: true,
                    render_water_surface: false,
                    draw_entities: !software_clip,
                    draw_sky_2d: false,
                    clip: (!software_clip && input.policy.height_clipping).then_some(
                        WaterClipPlane {
                            height: water.surface_z - WATER_CLIP_SPREAD,
                            keep: WaterClipKeep::Below,
                        },
                    ),
                    forced_visibility_leaf: None,
                    fog: ViewFog::Water {
                        volume: water.volume,
                        height_fog: true,
                    },
                });
            }
        } else {
            if render.refract {
                passes.push(EnvironmentViewPass {
                    kind: EnvironmentViewKind::Refraction,
                    origin: input.origin,
                    angles: input.angles,
                    render_above_water: true,
                    render_under_water: false,
                    render_water_surface: false,
                    draw_entities: true,
                    draw_sky_2d: input.draw_sky_2d,
                    clip: input.policy.height_clipping.then_some(WaterClipPlane {
                        height: adjusted_height - WATER_CLIP_SPREAD,
                        keep: WaterClipKeep::Above,
                    }),
                    forced_visibility_leaf: None,
                    fog: ViewFog::World,
                });
            }
            passes.push(EnvironmentViewPass {
                kind: EnvironmentViewKind::Main,
                origin: input.origin,
                angles: input.angles,
                render_above_water: !render.refract && !render.opaque,
                render_under_water: true,
                render_water_surface: render.draw_surface,
                draw_entities: true,
                draw_sky_2d: false,
                clip: (!input.policy.fast_clipping && input.policy.height_clipping).then_some(
                    WaterClipPlane {
                        height: adjusted_height + WATER_CLIP_SPREAD,
                        keep: WaterClipKeep::Below,
                    },
                ),
                forced_visibility_leaf: None,
                fog: ViewFog::Water {
                    volume: water.volume,
                    height_fog: false,
                },
            });
            if render.refract && software_clip && water_z_adjust != 0.0 {
                passes.push(EnvironmentViewPass {
                    kind: EnvironmentViewKind::Intersection,
                    origin: input.origin,
                    angles: input.angles,
                    render_above_water: true,
                    render_under_water: false,
                    render_water_surface: false,
                    draw_entities: false,
                    draw_sky_2d: false,
                    clip: None,
                    forced_visibility_leaf: None,
                    fog: ViewFog::World,
                });
            }
        }
        Ok(WaterViewPlan {
            visible_water: visible,
            render,
            passes,
        })
    }

    fn visible_water(
        &self,
        visibility: &VisibilityWorld,
        input: &WaterViewInput<'_>,
    ) -> Result<Option<VisibleWater>, EnvironmentError> {
        if input
            .origin
            .iter()
            .chain(input.angles.iter())
            .any(|v| !v.is_finite())
        {
            return Err(failure(EnvironmentErrorCode::NonFinite, None));
        }
        let eye = visibility
            .leaves
            .get(input.eye_leaf)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(input.eye_leaf)))?;
        let eye_volume = usize::try_from(eye.leaf_water_data_id).ok();
        let (volume, visible_leaf, eye_in_volume) = if let Some(volume) = eye_volume {
            (volume, input.eye_leaf, true)
        } else if eye.contents as u32 & CONTENTS_TEST_FOG_VOLUME != 0 {
            let mut selected = None;
            for leaf in input.qualified_visible_leaves {
                let record = visibility
                    .leaves
                    .get(*leaf)
                    .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(*leaf)))?;
                if record.contents as u32 & CONTENTS_SLIME == 0
                    && let Ok(volume) = usize::try_from(record.leaf_water_data_id)
                {
                    selected = Some((volume, *leaf, false));
                    break;
                }
            }
            let Some(selected) = selected else {
                return Ok(None);
            };
            selected
        } else {
            return Ok(None);
        };
        let source = self
            .volumes
            .get(volume)
            .filter(|source| source.index == volume && source.leaves.contains(&visible_leaf))
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(volume)))?;
        let (material, state, bindings, translucent) = if eye_in_volume {
            (
                source
                    .bottom_material
                    .clone()
                    .unwrap_or(WaterMaterialIdentity::Map(source.surface_material)),
                source
                    .bottom_state
                    .as_ref()
                    .unwrap_or(&source.surface_state),
                source
                    .bottom_bindings
                    .as_ref()
                    .unwrap_or(&source.surface_bindings),
                source
                    .bottom_translucent
                    .unwrap_or(source.surface_translucent),
            )
        } else {
            (
                WaterMaterialIdentity::Map(source.surface_material),
                &source.surface_state,
                &source.surface_bindings,
                source.surface_translucent,
            )
        };
        let distance_to_water = self
            .leaf_minimum_distance_to_water
            .as_ref()
            .and_then(|values| values.get(input.eye_leaf))
            .copied();
        Ok(Some(VisibleWater {
            volume,
            visible_leaf,
            eye_leaf: input.eye_leaf,
            eye_in_volume,
            surface_z: source.surface_z,
            distance_to_water,
            material,
            state: state.clone(),
            bindings: bindings.clone(),
            translucent,
        }))
    }
}

const CONTENTS_SLIME: u32 = 0x10;
const CONTENTS_TEST_FOG_VOLUME: u32 = 0x100;
const WATER_CLIP_SPREAD: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterViewPolicy {
    pub draw_water: bool,
    pub expensive_supported: bool,
    pub draw_reflection: bool,
    pub draw_refraction: bool,
    pub force_expensive: bool,
    pub force_reflect_entities: bool,
    pub fast_clipping: bool,
    pub height_clipping: bool,
    pub eye_water_epsilon: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterViewInput<'a> {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub eye_leaf: usize,
    pub qualified_visible_leaves: &'a [usize],
    pub near_plane_intersects_selected_volume: bool,
    pub draw_sky_2d: bool,
    pub policy: WaterViewPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleWater {
    pub volume: usize,
    pub visible_leaf: usize,
    pub eye_leaf: usize,
    pub eye_in_volume: bool,
    pub surface_z: f32,
    pub distance_to_water: Option<u16>,
    pub material: WaterMaterialIdentity,
    pub state: WaterState,
    pub bindings: WaterBindings,
    pub translucent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaterRenderSelection {
    pub cheap: bool,
    pub reflect: bool,
    pub refract: bool,
    pub reflect_entities: bool,
    pub draw_surface: bool,
    pub opaque: bool,
    pub environment: Option<CubemapSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentViewKind {
    Reflection,
    Refraction,
    Main,
    Intersection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaterClipKeep {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterClipPlane {
    pub height: f32,
    pub keep: WaterClipKeep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewFog {
    World,
    Water { volume: usize, height_fog: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentViewPass {
    pub kind: EnvironmentViewKind,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub render_above_water: bool,
    pub render_under_water: bool,
    pub render_water_surface: bool,
    pub draw_entities: bool,
    pub draw_sky_2d: bool,
    pub clip: Option<WaterClipPlane>,
    pub forced_visibility_leaf: Option<usize>,
    pub fog: ViewFog,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterViewPlan {
    pub visible_water: Option<VisibleWater>,
    pub render: WaterRenderSelection,
    pub passes: Vec<EnvironmentViewPass>,
}

fn water_render_selection(
    water: Option<&VisibleWater>,
    policy: WaterViewPolicy,
) -> WaterRenderSelection {
    let Some(water) = water else {
        return WaterRenderSelection {
            cheap: true,
            reflect: false,
            refract: false,
            reflect_entities: false,
            draw_surface: false,
            opaque: true,
            environment: None,
        };
    };
    if !policy.draw_water {
        return WaterRenderSelection {
            cheap: true,
            reflect: false,
            refract: false,
            reflect_entities: false,
            draw_surface: false,
            opaque: false,
            environment: water.bindings.environment.clone(),
        };
    }
    let force_cheap = water.state.force_cheap;
    let force_expensive = !force_cheap && (policy.force_expensive || water.state.force_expensive);
    let local_reflection = policy.expensive_supported
        && force_expensive
        && policy.draw_reflection
        && water.bindings.reflection;
    let beyond_lod = water
        .distance_to_water
        .is_some_and(|distance| f32::from(distance) >= water.state.cheap_end);
    if !policy.expensive_supported || force_cheap || beyond_lod && !local_reflection {
        return WaterRenderSelection {
            cheap: true,
            reflect: false,
            refract: false,
            reflect_entities: false,
            draw_surface: true,
            opaque: !water.translucent,
            environment: water.bindings.environment.clone(),
        };
    }
    let refract = policy.draw_refraction && water.bindings.refraction;
    WaterRenderSelection {
        cheap: !local_reflection && !refract,
        reflect: local_reflection,
        refract,
        reflect_entities: local_reflection
            && (policy.force_reflect_entities || water.state.reflect_entities),
        draw_surface: true,
        opaque: !water.translucent && !refract,
        environment: water.bindings.environment.clone(),
    }
}

fn water_eye_adjustment(
    eye_z: f32,
    water_z: f32,
    policy: WaterViewPolicy,
) -> Result<(f32, f32, bool), EnvironmentError> {
    if !policy.eye_water_epsilon.is_finite() || policy.eye_water_epsilon < 0.0 {
        return Err(failure(EnvironmentErrorCode::InvalidField, None));
    }
    if !policy.fast_clipping {
        return Ok((water_z, 0.0, false));
    }
    let delta = eye_z - water_z;
    if delta.abs() >= policy.eye_water_epsilon || delta == 0.0 && policy.eye_water_epsilon == 0.0 {
        return Ok((water_z, 0.0, false));
    }
    let adjusted = if delta > 0.0 {
        eye_z - policy.eye_water_epsilon
    } else {
        eye_z + policy.eye_water_epsilon
    };
    Ok((adjusted, adjusted - water_z, true))
}

fn simple_view_pass(
    input: &WaterViewInput<'_>,
    water: Option<&VisibleWater>,
    render: &WaterRenderSelection,
) -> EnvironmentViewPass {
    let eye_in_water = water.is_some_and(|water| water.eye_in_volume);
    let both = !render.opaque || input.near_plane_intersects_selected_volume;
    EnvironmentViewPass {
        kind: EnvironmentViewKind::Main,
        origin: input.origin,
        angles: input.angles,
        render_above_water: both || !eye_in_water,
        render_under_water: both || eye_in_water,
        render_water_surface: render.draw_surface,
        draw_entities: true,
        draw_sky_2d: !eye_in_water && input.draw_sky_2d,
        clip: None,
        forced_visibility_leaf: None,
        fog: water.map_or(ViewFog::World, |water| {
            if water.eye_in_volume {
                ViewFog::Water {
                    volume: water.volume,
                    height_fog: false,
                }
            } else {
                ViewFog::World
            }
        }),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FogState {
    pub enabled: bool,
    pub blend: bool,
    pub direction: [f32; 3],
    pub primary: [u8; 4],
    pub secondary: [u8; 4],
    pub start: f32,
    pub end: f32,
    pub maximum_density: f32,
    pub far_z: Option<f32>,
    pub radial: bool,
    pub transition_duration: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FogPlayerTransition {
    pub from: FogState,
    pub to: FogState,
    pub start_time: f32,
    pub snap: bool,
}

impl FogPlayerTransition {
    pub fn sample(&self, time: f32) -> Result<FogState, EnvironmentError> {
        if !time.is_finite()
            || !self.start_time.is_finite()
            || time < self.start_time
            || !self.to.transition_duration.is_finite()
            || self.to.transition_duration < 0.0
        {
            return Err(failure(EnvironmentErrorCode::InvalidField, None));
        }
        let mut output = self.to.clone();
        if self.snap || self.to.transition_duration == 0.0 {
            return Ok(output);
        }
        let elapsed = time - self.start_time;
        if elapsed < self.to.transition_duration {
            let amount = elapsed / self.to.transition_duration;
            for channel in 0..3 {
                output.primary[channel] = (f32::from(self.to.primary[channel]) * amount
                    + f32::from(self.from.primary[channel]) * (1.0 - amount))
                    .trunc() as u8;
            }
            output.start = self.to.start * amount + self.from.start * (1.0 - amount);
            output.end = self.to.end * amount + self.from.end * (1.0 - amount);
        }
        Ok(output)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControllerState {
    Fog(FogState),
    SkyCamera {
        origin: [f32; 3],
        scale: i32,
        area: usize,
        fog: FogState,
    },
    WaterLod {
        start: f32,
        end: f32,
    },
    EnvironmentLight {
        origin: [f32; 3],
        angles: [f32; 3],
        pitch: f32,
        sunlight: [f32; 4],
        sunlight_hdr: [f32; 4],
        sunlight_hdr_scale: f32,
        ambient: [f32; 4],
        ambient_hdr: [f32; 4],
        ambient_hdr_scale: f32,
        sun_spread_angle: f32,
    },
    Shadow {
        angles: [f32; 3],
        color: [u8; 4],
        maximum_distance: f32,
        disabled: bool,
    },
    ToneMap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentController {
    pub entity: usize,
    pub classname: Vec<u8>,
    pub state: ControllerState,
    pub raw_fields: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkKind {
    InfoDecal,
    Overlay,
    WaterOverlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkStatus {
    Projected,
    Ineligible,
    Missing,
    Inert,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkFragment {
    pub model: usize,
    pub face: usize,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub lightmap_uv: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub visibility: MarkVisibility,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformedMarkFragment {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

impl MarkFragment {
    pub fn at_transform(
        &self,
        transform: playsrc_collision::Transform,
    ) -> Result<TransformedMarkFragment, EnvironmentError> {
        let transform = ModelTransform::from_collision(transform)?;
        Ok(TransformedMarkFragment {
            positions: self
                .positions
                .iter()
                .map(|position| transform.point_to_world(*position))
                .collect(),
            normals: self
                .normals
                .iter()
                .map(|normal| transform.vector_to_world(*normal))
                .collect(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkVisibility {
    World {
        leaves: Vec<usize>,
        clusters: Vec<i16>,
        areas: Vec<usize>,
    },
    BrushModel {
        entity: u64,
        model: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkActivation {
    MapActivation,
    Input,
    Compiled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkLifetime {
    Permanent,
    PoolManaged,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkRenderRequest {
    pub normal_offset: f32,
    pub polygon_offset: playsrc_material::PolygonOffset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkReceiver {
    pub entity: Option<u64>,
    pub model: usize,
    pub local_origin: [f32; 3],
    pub transform: playsrc_collision::Transform,
    pub parent_entity: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkRecord {
    pub kind: MarkKind,
    pub source_index: usize,
    pub entity: Option<usize>,
    pub overlay_id: Option<i32>,
    pub status: MarkStatus,
    pub material_path: String,
    pub material_sha256: Option<[u8; 32]>,
    pub material_state: Option<DecalState>,
    pub origin: [f32; 3],
    pub receiver: Option<MarkReceiver>,
    pub target_faces: Vec<usize>,
    pub render_order: u8,
    pub fade_distances_squared: Option<[f32; 2]>,
    pub initially_enabled: bool,
    pub dynamic: bool,
    pub low_priority: bool,
    pub parent_entity: Option<usize>,
    pub activation: MarkActivation,
    pub lifetime: MarkLifetime,
    pub render: MarkRenderRequest,
    pub fragments: Vec<MarkFragment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkEnvironment {
    pub collision_world_identity: [u8; 32],
    pub receiver_snapshot_revision: u64,
    pub placement_revision: u64,
    pub records: Vec<MarkRecord>,
    pub fragment_count: usize,
    pub vertex_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleMarkFragment {
    pub record: usize,
    pub fragment: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleBrushMarkReceiver {
    pub entity: u64,
    pub model: usize,
}

impl MarkEnvironment {
    pub fn visible_fragments(
        &self,
        view: &playsrc_visibility::ViewResult,
        visible_brush_receivers: &[VisibleBrushMarkReceiver],
        collision_world_identity: [u8; 32],
        receiver_snapshot_revision: u64,
        placement_revision: u64,
    ) -> Result<Vec<VisibleMarkFragment>, EnvironmentError> {
        if collision_world_identity != self.collision_world_identity
            || receiver_snapshot_revision != self.receiver_snapshot_revision
            || placement_revision != self.placement_revision
        {
            return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
        }
        let mut output = Vec::new();
        for (record_index, record) in self.records.iter().enumerate() {
            for (fragment_index, fragment) in record.fragments.iter().enumerate() {
                let visible = match &fragment.visibility {
                    MarkVisibility::World { .. } => u16::try_from(fragment.face)
                        .ok()
                        .is_some_and(|face| view.world_surfaces.contains(&face)),
                    MarkVisibility::BrushModel { entity, model } => visible_brush_receivers
                        .contains(&VisibleBrushMarkReceiver {
                            entity: *entity,
                            model: *model,
                        }),
                };
                if visible {
                    output.push(VisibleMarkFragment {
                        record: record_index,
                        fragment: fragment_index,
                    });
                }
            }
        }
        Ok(output)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldEnvironment {
    pub lighting: LightingProvenance,
    pub sky: Option<SkyEnvironment>,
    pub cubemaps: Vec<CubemapSample>,
    pub water: WaterEnvironment,
    pub marks: MarkEnvironment,
    pub controllers: Vec<EnvironmentController>,
    pub master_fog_controller: Option<usize>,
    pub dependencies: Vec<DependencyRequest>,
}

pub fn compile_environment(
    map: &CanonicalMap,
    bsp: &Bsp,
    input: EnvironmentInputs<'_>,
) -> Result<WorldEnvironment, EnvironmentError> {
    if map.bsp_version != bsp.container_version
        || map.map_revision != bsp.map_revision
        || input.entities.source != bsp.lumps[0].bytes(bsp)
    {
        return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
    }
    let visibility_identity = playsrc_visibility::compile(bsp)
        .map_err(|_| failure(EnvironmentErrorCode::DependencyMismatch, None))?
        .identity;
    if input.visibility.identity != visibility_identity {
        return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
    }
    if input.collision.identity != map.collision_world_identity
        || input.receiver_snapshot.world_identity() != input.collision.identity
        || input.receiver_snapshot.records().iter().any(|record| {
            record.role != playsrc_collision::ObjectRole::Entity
                || !matches!(
                    record.shape,
                    playsrc_collision::SnapshotShape::BrushModel { .. }
                )
        })
    {
        return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
    }
    let map_name = map_name(input.logical_map_path)?;
    let max_dependencies = input
        .limits
        .max_cubemaps
        .checked_add(CubeFace::ALL.len())
        .ok_or_else(|| failure(EnvironmentErrorCode::BoundExceeded, None))?;
    if input.dependencies.len() > max_dependencies {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    let dependency_catalog = DependencyCatalog::new(input.dependencies)?;
    let material_bindings = material_bindings(input.materials, map.materials.len())?;
    let dependent_materials = named_material_bindings(input.dependent_materials)?;
    let (sky, mut requests) = compile_sky(map, input.entities, &dependency_catalog)?;
    let (cubemaps, cubemap_requests) = compile_cubemaps(
        bsp,
        map.lighting_profile,
        map_name,
        &dependency_catalog,
        input.limits,
    )?;
    requests.extend(cubemap_requests);
    if dependency_catalog.len() != requests.len() {
        return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
    }
    let water = compile_water(
        map,
        bsp,
        &material_bindings,
        &dependent_materials,
        &cubemaps,
        input.limits,
    )?;
    let marks = compile_marks(MarkCompileInput {
        map,
        bsp,
        graph: input.entities,
        visibility: input.visibility,
        collision: input.collision,
        receiver_snapshot: input.receiver_snapshot,
        mark_placements: input.mark_placements,
        materials: &material_bindings,
        mark_materials: input.mark_materials,
        limits: input.limits,
    })?;
    let controllers = compile_controllers(input.entities, input.visibility)?;
    let master_fog_controller = master_fog_controller(input.entities, &controllers)?;
    Ok(WorldEnvironment {
        lighting: LightingProvenance {
            profile: map.lighting_profile,
            closure_sha256: map.lighting.closure_sha256,
            members: map.lighting.members.clone(),
        },
        sky,
        cubemaps,
        water,
        marks,
        controllers,
        master_fog_controller,
        dependencies: requests,
    })
}

pub fn select_cubemap(
    cubemaps: &[CubemapSample],
    position: [f32; 3],
    declared: Option<usize>,
) -> Result<&CubemapSample, EnvironmentError> {
    if position.iter().any(|value| !value.is_finite()) {
        return Err(failure(EnvironmentErrorCode::NonFinite, None));
    }
    if let Some(index) = declared {
        return cubemaps
            .iter()
            .find(|sample| sample.index == index)
            .ok_or_else(|| failure(EnvironmentErrorCode::MissingDependency, Some(index)));
    }
    cubemaps
        .iter()
        .min_by(|left, right| {
            squared_distance(position, left.origin)
                .total_cmp(&squared_distance(position, right.origin))
                .then(left.index.cmp(&right.index))
        })
        .ok_or_else(|| failure(EnvironmentErrorCode::MissingDependency, None))
}

fn map_name(logical_path: &str) -> Result<&str, EnvironmentError> {
    let name = logical_path
        .strip_prefix("maps/")
        .and_then(|path| path.strip_suffix(".bsp"))
        .filter(|path| !path.is_empty())
        .ok_or_else(|| failure(EnvironmentErrorCode::InvalidMapPath, None))?;
    if name
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(failure(EnvironmentErrorCode::InvalidMapPath, None));
    }
    Ok(name)
}

fn material_bindings<'a>(
    bindings: &'a [MaterialBinding<'a>],
    material_count: usize,
) -> Result<BTreeMap<usize, &'a Material>, EnvironmentError> {
    if bindings.len() > material_count {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    let mut output = BTreeMap::new();
    for binding in bindings {
        if binding.material_index >= material_count
            || output
                .insert(binding.material_index, binding.material)
                .is_some()
        {
            return Err(failure(
                EnvironmentErrorCode::InvalidReference,
                Some(binding.material_index),
            ));
        }
    }
    Ok(output)
}

fn named_material_bindings<'a>(
    bindings: &'a [NamedMaterialBinding<'a>],
) -> Result<BTreeMap<String, &'a Material>, EnvironmentError> {
    if bindings.len() > 65_536 {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    let mut output = BTreeMap::new();
    for (index, binding) in bindings.iter().enumerate() {
        if binding.logical_path.is_empty()
            || output
                .insert(binding.logical_path.to_ascii_lowercase(), binding.material)
                .is_some()
        {
            return Err(failure(EnvironmentErrorCode::InvalidReference, Some(index)));
        }
    }
    Ok(output)
}

fn compile_sky(
    map: &CanonicalMap,
    graph: &Graph,
    responses: &DependencyCatalog<'_>,
) -> Result<(Option<SkyEnvironment>, Vec<DependencyRequest>), EnvironmentError> {
    let surface_faces: Vec<_> = map
        .surfaces
        .iter()
        .filter(|surface| surface.flags & (SURF_SKY_2D | SURF_SKY) != 0)
        .map(|surface| surface.face)
        .collect();
    if surface_faces.is_empty() {
        return Ok((None, Vec::new()));
    }
    let worldspawn = graph
        .entities
        .iter()
        .find(|entity| class_is(entity, b"worldspawn"))
        .ok_or_else(|| failure(EnvironmentErrorCode::InvalidField, None))?;
    let sky_name = pair(worldspawn, b"skyname")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            entity_failure(EnvironmentErrorCode::InvalidField, worldspawn, b"skyname")
        })?;
    let sky = std::str::from_utf8(sky_name)
        .map_err(|_| entity_failure(EnvironmentErrorCode::InvalidField, worldspawn, b"skyname"))?;
    if sky.contains(['/', '\\']) || matches!(sky, "." | "..") {
        return Err(entity_failure(
            EnvironmentErrorCode::InvalidField,
            worldspawn,
            b"skyname",
        ));
    }
    let mut requests = Vec::with_capacity(6);
    let mut faces = Vec::with_capacity(6);
    for face in CubeFace::ALL {
        let profile_suffix = if map.lighting_profile == LightingProfile::Hdr {
            "_hdr"
        } else {
            ""
        };
        let path = format!(
            "materials/skybox/{sky}{profile_suffix}{}.vmt",
            face.suffix()
        );
        let request = DependencyRequest {
            role: DependencyRole::SkyMaterial(face),
            profile: map.lighting_profile,
            logical_path: path.clone(),
        };
        let response = responses.get(&request)?;
        let DependencyMetadata::SkyMaterial {
            source_sha256,
            encoding,
            selected_textures,
        } = &response.metadata
        else {
            return Err(dependency_failure(
                EnvironmentErrorCode::DependencyMismatch,
                &request,
            ));
        };
        if selected_textures.len() != 1
            || selected_textures
                .iter()
                .any(|texture| texture.logical_path.is_empty())
        {
            return Err(dependency_failure(
                EnvironmentErrorCode::DependencyMismatch,
                &request,
            ));
        }
        faces.push(SkyFace {
            face,
            material_path: path,
            material_sha256: *source_sha256,
            encoding: *encoding,
            selected_textures: selected_textures
                .iter()
                .map(|texture| SkyTextureDependency {
                    logical_path: texture.logical_path.clone(),
                    sha256: texture.sha256,
                })
                .collect(),
        });
        requests.push(request);
    }
    Ok((
        Some(SkyEnvironment {
            name: sky_name.to_vec(),
            profile: map.lighting_profile,
            surface_faces,
            faces,
        }),
        requests,
    ))
}

fn compile_cubemaps(
    bsp: &Bsp,
    profile: LightingProfile,
    map_name: &str,
    responses: &DependencyCatalog<'_>,
    limits: EnvironmentLimits,
) -> Result<(Vec<CubemapSample>, Vec<DependencyRequest>), EnvironmentError> {
    let records = match &bsp.lumps[42].records {
        LumpData::Cubemaps(records) => records.as_slice(),
        LumpData::Opaque if bsp.lumps[42].bytes(bsp).is_empty() => &[],
        _ => return Err(failure(EnvironmentErrorCode::MissingLump, Some(42))),
    };
    if records.len() > limits.max_cubemaps {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, Some(42)));
    }
    let mut output = Vec::with_capacity(records.len());
    let mut requests = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let requested_dimension = if record.size == 0 {
            None
        } else {
            1_u32.checked_shl(u32::from(record.size - 1))
        };
        if record.size != 0 && requested_dimension.is_none() {
            return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(index)));
        }
        let profile_suffix = if profile == LightingProfile::Hdr {
            ".hdr"
        } else {
            ""
        };
        let path = format!(
            "materials/maps/{map_name}/c{}_{}_{}{profile_suffix}.vtf",
            record.origin[0], record.origin[1], record.origin[2]
        );
        let request = DependencyRequest {
            role: DependencyRole::CubemapTexture { sample: index },
            profile,
            logical_path: path.clone(),
        };
        let response = responses.get(&request)?;
        let DependencyMetadata::CubemapTexture {
            source_sha256,
            width,
            height,
            mip_count,
            source_face_count,
        } = response.metadata
        else {
            return Err(dependency_failure(
                EnvironmentErrorCode::DependencyMismatch,
                &request,
            ));
        };
        if width == 0 || width != height || mip_count == 0 || source_face_count < 6 {
            return Err(dependency_failure(
                EnvironmentErrorCode::DependencyMismatch,
                &request,
            ));
        }
        output.push(CubemapSample {
            index,
            origin: record.origin,
            encoded_size: record.size,
            requested_dimension,
            profile,
            logical_path: path,
            source_sha256,
            width,
            height,
            source_face_count,
            faces: CubeFace::ALL
                .into_iter()
                .map(|face| CubemapFaceDependency { face, mip_count })
                .collect(),
        });
        requests.push(request);
    }
    Ok((output, requests))
}

type DependencyKey = (u8, usize, u8, String);

#[derive(Debug)]
struct DependencyCatalog<'a> {
    responses: BTreeMap<DependencyKey, &'a DependencyResponse>,
}

impl<'a> DependencyCatalog<'a> {
    fn new(responses: &'a [DependencyResponse]) -> Result<Self, EnvironmentError> {
        let mut output = BTreeMap::new();
        for response in responses {
            if output
                .insert(dependency_key(&response.request), response)
                .is_some()
            {
                return Err(dependency_failure(
                    EnvironmentErrorCode::DependencyMismatch,
                    &response.request,
                ));
            }
        }
        Ok(Self { responses: output })
    }

    fn get(&self, request: &DependencyRequest) -> Result<&'a DependencyResponse, EnvironmentError> {
        self.responses
            .get(&dependency_key(request))
            .copied()
            .ok_or_else(|| dependency_failure(EnvironmentErrorCode::MissingDependency, request))
    }

    fn len(&self) -> usize {
        self.responses.len()
    }
}

fn dependency_key(request: &DependencyRequest) -> DependencyKey {
    let (kind, item) = match request.role {
        DependencyRole::SkyMaterial(face) => (0, face as usize),
        DependencyRole::CubemapTexture { sample } => (1, sample),
    };
    let profile = match request.profile {
        LightingProfile::Ldr => 0,
        LightingProfile::Hdr => 1,
    };
    (kind, item, profile, request.logical_path.clone())
}

fn compile_water(
    map: &CanonicalMap,
    bsp: &Bsp,
    materials: &BTreeMap<usize, &Material>,
    dependent_materials: &BTreeMap<String, &Material>,
    cubemaps: &[CubemapSample],
    limits: EnvironmentLimits,
) -> Result<WaterEnvironment, EnvironmentError> {
    let surface_input = WaterSurfaceInput {
        bsp,
        materials,
        cubemaps,
        limits,
    };
    let mut surfaces = Vec::new();
    for profile in [LightingProfile::Ldr, LightingProfile::Hdr] {
        let slot = if profile == LightingProfile::Hdr {
            58
        } else {
            7
        };
        let faces = match &bsp.lumps[slot].records {
            LumpData::Faces(faces) => faces,
            LumpData::Opaque if bsp.lumps[slot].bytes(bsp).is_empty() => continue,
            _ => return Err(failure(EnvironmentErrorCode::MissingLump, Some(slot))),
        };
        append_water_surfaces(
            &mut surfaces,
            profile,
            profile == map.lighting_profile,
            faces,
            &surface_input,
        )?;
    }
    let leaf_minimum_distance_to_water = leaf_minimum_distances(bsp)?;
    let volumes =
        compile_water_volumes(map, bsp, materials, dependent_materials, cubemaps, limits)?;
    if surfaces
        .iter()
        .filter_map(|surface| surface.fog_volume)
        .any(|volume| volume >= volumes.len())
    {
        return Err(failure(EnvironmentErrorCode::InvalidReference, Some(36)));
    }
    Ok(WaterEnvironment {
        surfaces,
        volumes,
        leaf_minimum_distance_to_water,
    })
}

struct WaterSurfaceInput<'a> {
    bsp: &'a Bsp,
    materials: &'a BTreeMap<usize, &'a Material>,
    cubemaps: &'a [CubemapSample],
    limits: EnvironmentLimits,
}

fn append_water_surfaces(
    output: &mut Vec<WaterSurface>,
    profile: LightingProfile,
    selected: bool,
    faces: &[Face],
    input: &WaterSurfaceInput<'_>,
) -> Result<(), EnvironmentError> {
    let bsp = input.bsp;
    let vertices = lump_records(bsp, 3, |data| match data {
        LumpData::Vertices(value) => Some(value.as_slice()),
        _ => None,
    })?;
    let edges = lump_records(bsp, 12, |data| match data {
        LumpData::Edges(value) => Some(value.as_slice()),
        _ => None,
    })?;
    let surfedges = lump_records(bsp, 13, |data| match data {
        LumpData::SurfaceEdges(value) => Some(value.as_slice()),
        _ => None,
    })?;
    let texture_info = texture_info(bsp)?;
    let planes = lump_records(bsp, 1, |data| match data {
        LumpData::Planes(value) => Some(value.as_slice()),
        _ => None,
    })?;
    let owners = face_owners(bsp, faces.len())?;
    for (face_index, face) in faces.iter().enumerate() {
        let info_index = usize::try_from(face.texture_info_index)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(face_index)))?;
        let info = texture_info
            .get(info_index)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(face_index)))?;
        if info.flags & SURF_WARP == 0 {
            continue;
        }
        if output.len() >= input.limits.max_water_surfaces {
            return Err(failure(
                EnvironmentErrorCode::BoundExceeded,
                Some(face_index),
            ));
        }
        let material = usize::try_from(info.texture_data_index)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(face_index)))?;
        let material_output = input
            .materials
            .get(&material)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material)))?;
        let state = material_output
            .water
            .clone()
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material)))?;
        let positions = face_positions(face, face_index, vertices, edges, surfedges)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidRecord, Some(face_index)))?;
        let plane = planes
            .get(face.plane_index as usize)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(face_index)))?;
        let sign = if face.side == 0 { 1.0 } else { -1.0 };
        let oriented = [
            plane.normal.x.value() * sign,
            plane.normal.y.value() * sign,
            plane.normal.z.value() * sign,
            plane.distance.value() * sign,
        ];
        if oriented.iter().any(|value| !value.is_finite()) {
            return Err(failure(EnvironmentErrorCode::NonFinite, Some(face_index)));
        }
        let bounds = finite_bounds(&positions, face_index)?;
        output.push(WaterSurface {
            profile,
            selected,
            face: face_index,
            model: owners[face_index],
            material,
            texture_info: info_index,
            fog_volume: usize::try_from(face.surface_fog_volume_id).ok(),
            plane: oriented,
            bounds,
            bindings: water_bindings(
                material_output,
                &state,
                bounds_center(bounds),
                input.cubemaps,
            )?,
            state,
        });
    }
    Ok(())
}

fn compile_water_volumes(
    map: &CanonicalMap,
    bsp: &Bsp,
    materials: &BTreeMap<usize, &Material>,
    dependent_materials: &BTreeMap<String, &Material>,
    cubemaps: &[CubemapSample],
    limits: EnvironmentLimits,
) -> Result<Vec<WaterVolume>, EnvironmentError> {
    let bytes = bsp.lumps[36].bytes(bsp);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bsp.lumps[36].version != 0 || !bytes.len().is_multiple_of(12) {
        return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(36)));
    }
    let count = bytes.len() / 12;
    if count > limits.max_water_volumes {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, Some(36)));
    }
    let leaves = leaves(bsp)?;
    let texture_info = texture_info(bsp)?;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        let record = &bytes[index * 12..index * 12 + 12];
        let surface_z = f32_at(record, 0);
        let minimum_z = f32_at(record, 4);
        let info_index = usize::try_from(i16::from_le_bytes([record[8], record[9]]))
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(index)))?;
        if !surface_z.is_finite() || !minimum_z.is_finite() || minimum_z > surface_z {
            return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(index)));
        }
        let info = texture_info
            .get(info_index)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(index)))?;
        let material = usize::try_from(info.texture_data_index)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(index)))?;
        let material_output = materials
            .get(&material)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material)))?;
        let state = material_output
            .water
            .clone()
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material)))?;
        let member_leaves: Vec<_> = leaves
            .iter()
            .enumerate()
            .filter_map(|(leaf, record)| {
                (record.leaf_water_data_id == index as i16).then_some(leaf)
            })
            .collect();
        if member_leaves.is_empty() {
            return Err(failure(EnvironmentErrorCode::InvalidReference, Some(index)));
        }
        let bounds = leaf_union_bounds(leaves, &member_leaves, index)?;
        let bottom = state
            .bottom_material
            .as_ref()
            .map(|request| {
                if let Some(candidate) = map.materials.iter().find(|candidate| {
                    candidate
                        .logical_path
                        .eq_ignore_ascii_case(&request.logical_path)
                }) {
                    return materials
                        .get(&candidate.index)
                        .copied()
                        .map(|material| (WaterMaterialIdentity::Map(candidate.index), material))
                        .ok_or_else(|| {
                            failure(
                                EnvironmentErrorCode::InvalidReference,
                                Some(candidate.index),
                            )
                        });
                }
                dependent_materials
                    .get(&request.logical_path.to_ascii_lowercase())
                    .copied()
                    .map(|material| {
                        (
                            WaterMaterialIdentity::Dependency(request.logical_path.clone()),
                            material,
                        )
                    })
                    .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material)))
            })
            .transpose()?;
        let bottom_state = bottom
            .as_ref()
            .map(|(_, bottom_value)| {
                bottom_value
                    .water
                    .clone()
                    .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(index)))
            })
            .transpose()?;
        let bottom_material = bottom.as_ref().map(|(identity, _)| identity.clone());
        let center = bounds_center(bounds);
        let surface_bindings = water_bindings(material_output, &state, center, cubemaps)?;
        let bottom_bindings = bottom
            .as_ref()
            .zip(bottom_state.as_ref())
            .map(|((_, material), state)| water_bindings(material, state, center, cubemaps))
            .transpose()?;
        let clusters = member_leaves
            .iter()
            .map(|leaf| leaves[*leaf].cluster)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let areas = member_leaves
            .iter()
            .map(|leaf| usize::from(leaves[*leaf].area_and_flags & 0x01ff))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let contents = member_leaves.iter().fold(0_u32, |contents, leaf| {
            contents | leaves[*leaf].contents as u32
        });
        output.push(WaterVolume {
            index,
            surface_z,
            minimum_z,
            texture_info: info_index,
            surface_material: material,
            bottom_material,
            leaves: member_leaves,
            clusters,
            areas,
            contents,
            bounds,
            plane: [0.0, 0.0, 1.0, surface_z],
            surface_bindings,
            bottom_bindings,
            surface_translucent: material_output.features.translucent,
            bottom_translucent: bottom.map(|(_, material)| material.features.translucent),
            surface_state: state,
            bottom_state,
        });
    }
    for (leaf_index, leaf) in leaves.iter().enumerate() {
        if leaf.leaf_water_data_id >= 0 && leaf.leaf_water_data_id as usize >= count {
            return Err(failure(
                EnvironmentErrorCode::InvalidReference,
                Some(leaf_index),
            ));
        }
    }
    Ok(output)
}

fn water_bindings(
    material: &Material,
    state: &WaterState,
    position: [f32; 3],
    cubemaps: &[CubemapSample],
) -> Result<WaterBindings, EnvironmentError> {
    Ok(WaterBindings {
        environment: if material.active_textures.contains(&TextureRole::Environment) {
            water_cubemap(state, position, cubemaps)?
        } else {
            None
        },
        reflection: material.active_textures.contains(&TextureRole::Reflection),
        refraction: material.active_textures.contains(&TextureRole::Refraction),
    })
}

fn water_cubemap(
    state: &WaterState,
    position: [f32; 3],
    cubemaps: &[CubemapSample],
) -> Result<Option<CubemapSelection>, EnvironmentError> {
    match state.environment_map.disposition {
        TextureDisposition::BuiltInEnvironment => Ok(Some(CubemapSelection::Nearest {
            sample: select_cubemap(cubemaps, position, None)?.index,
        })),
        TextureDisposition::Source => {
            let Some(path) = state.environment_map.logical_path.as_ref() else {
                return Ok(None);
            };
            let declared = cubemap_stem(path).and_then(|requested| {
                cubemaps
                    .iter()
                    .find(|sample| {
                        cubemap_stem(&sample.logical_path)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(requested))
                    })
                    .map(|sample| sample.index)
            });
            Ok(Some(declared.map_or_else(
                || CubemapSelection::External {
                    logical_path: path.clone(),
                },
                |sample| CubemapSelection::Declared { sample },
            )))
        }
        TextureDisposition::BuiltInRenderTarget => {
            Err(failure(EnvironmentErrorCode::InvalidReference, None))
        }
    }
}

fn cubemap_stem(path: &str) -> Option<&str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".hdr.vtf") {
        Some(&path[..path.len() - ".hdr.vtf".len()])
    } else if lower.ends_with(".vtf") {
        Some(&path[..path.len() - ".vtf".len()])
    } else {
        None
    }
}

fn bounds_center(bounds: [[f32; 3]; 2]) -> [f32; 3] {
    [
        (bounds[0][0] + bounds[1][0]) * 0.5,
        (bounds[0][1] + bounds[1][1]) * 0.5,
        (bounds[0][2] + bounds[1][2]) * 0.5,
    ]
}

fn compile_controllers(
    graph: &Graph,
    visibility: &VisibilityWorld,
) -> Result<Vec<EnvironmentController>, EnvironmentError> {
    let mut output = Vec::new();
    for entity in &graph.entities {
        let Some(classname) = entity.classname.as_deref() else {
            continue;
        };
        let state = if classname.eq_ignore_ascii_case(b"env_fog_controller") {
            ControllerState::Fog(fog_state(entity, true)?)
        } else if classname.eq_ignore_ascii_case(b"sky_camera") {
            let origin = vector3_required(entity, b"origin")?;
            let scale = integer_or(entity, b"scale", 0)?;
            let leaf = visibility.locate_leaf(origin).map_err(|_| {
                entity_failure(EnvironmentErrorCode::InvalidReference, entity, b"origin")
            })?;
            let area = visibility
                .leaves
                .get(leaf)
                .map(|leaf| usize::from(leaf.area_and_flags & 0x01ff))
                .ok_or_else(|| {
                    entity_failure(EnvironmentErrorCode::InvalidReference, entity, b"origin")
                })?;
            ControllerState::SkyCamera {
                origin,
                scale,
                area,
                fog: fog_state(entity, false)?,
            }
        } else if classname.eq_ignore_ascii_case(b"water_lod_control") {
            let start = float_or_entity(entity, b"cheapwaterstartdistance", 1000.0)?;
            let end = float_or_entity(entity, b"cheapwaterenddistance", 2000.0)?;
            ControllerState::WaterLod { start, end }
        } else if classname.eq_ignore_ascii_case(b"light_environment") {
            ControllerState::EnvironmentLight {
                origin: vector3_or(entity, b"origin", [0.0; 3])?,
                angles: vector3_or(entity, b"angles", [0.0; 3])?,
                pitch: float_or_entity(entity, b"pitch", 0.0)?,
                sunlight: vector4_or(entity, b"_light", [255.0, 255.0, 255.0, 200.0])?,
                sunlight_hdr: vector4_or(entity, b"_lightHDR", [-1.0; 4])?,
                sunlight_hdr_scale: float_or_entity(entity, b"_lightscaleHDR", 1.0)?,
                ambient: vector4_or(entity, b"_ambient", [255.0, 255.0, 255.0, 20.0])?,
                ambient_hdr: vector4_or(entity, b"_ambientHDR", [-1.0; 4])?,
                ambient_hdr_scale: float_or_entity(entity, b"_AmbientScaleHDR", 1.0)?,
                sun_spread_angle: float_or_entity(entity, b"SunSpreadAngle", 0.0)?,
            }
        } else if classname.eq_ignore_ascii_case(b"shadow_control") {
            ControllerState::Shadow {
                angles: vector3_or(entity, b"angles", [80.0, 30.0, 0.0])?,
                color: color_or_entity(entity, b"color", [64, 64, 64, 0])?,
                maximum_distance: float_or_entity(entity, b"distance", 50.0)?,
                disabled: bool_or_entity(entity, b"disableallshadows", false)?,
            }
        } else if classname.eq_ignore_ascii_case(b"env_tonemap_controller") {
            ControllerState::ToneMap
        } else {
            continue;
        };
        output.push(EnvironmentController {
            entity: entity.index,
            classname: classname.to_vec(),
            state,
            raw_fields: entity
                .pairs
                .iter()
                .map(|pair| (pair.key.clone(), pair.value.clone()))
                .collect(),
        });
    }
    Ok(output)
}

fn fog_state(entity: &Entity, main_view: bool) -> Result<FogState, EnvironmentError> {
    let maximum_density = float_or_entity(entity, b"fogmaxdensity", 1.0)?;
    let direction = if bool_or_entity(entity, b"use_angles", false)? {
        let [pitch, yaw, _] = vector3_or(entity, b"angles", [0.0; 3])?.map(f32::to_radians);
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        [-cos_pitch * cos_yaw, -cos_pitch * sin_yaw, sin_pitch]
    } else {
        vector3_or(entity, b"fogdir", [0.0; 3])?
    };
    Ok(FogState {
        enabled: bool_or_entity(entity, b"fogenable", false)?,
        blend: bool_or_entity(entity, b"fogblend", false)?,
        direction,
        primary: color_or_entity(entity, b"fogcolor", [0, 0, 0, 255])?,
        secondary: color_or_entity(entity, b"fogcolor2", [0, 0, 0, 255])?,
        start: float_or_entity(entity, b"fogstart", 0.0)?,
        end: float_or_entity(entity, b"fogend", 0.0)?,
        maximum_density,
        far_z: main_view
            .then(|| float_or_entity(entity, b"farz", 0.0))
            .transpose()?,
        radial: bool_or_entity(entity, b"fogradial", false)?,
        transition_duration: float_or_entity(entity, b"foglerptime", 0.0)?,
    })
}

fn master_fog_controller(
    graph: &Graph,
    controllers: &[EnvironmentController],
) -> Result<Option<usize>, EnvironmentError> {
    let fog_entities = controllers
        .iter()
        .filter_map(|controller| {
            matches!(controller.state, ControllerState::Fog(_)).then_some(controller.entity)
        })
        .collect::<Vec<_>>();
    let mut selected = fog_entities.first().copied();
    for entity in fog_entities {
        let source = graph
            .entities
            .get(entity)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(entity)))?;
        let flags = integer_or(source, b"spawnflags", 0)?;
        if flags & 1 != 0 {
            selected = Some(entity);
        }
    }
    Ok(selected)
}

fn class_is(entity: &Entity, classname: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(classname))
}

fn pair<'a>(entity: &'a Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}

fn vector_values(entity: &Entity, key: &[u8]) -> Result<Option<Vec<f32>>, EnvironmentError> {
    let Some(bytes) = pair(entity, key) else {
        return Ok(None);
    };
    let values = std::str::from_utf8(bytes)
        .ok()
        .map(|value| {
            value
                .split_ascii_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .ok()
        .flatten()
        .filter(|values| values.iter().all(|value| value.is_finite()))
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))?;
    Ok(Some(values))
}

fn vector3_required(entity: &Entity, key: &[u8]) -> Result<[f32; 3], EnvironmentError> {
    vector3_or_optional(entity, key)?
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))
}

fn vector3_or(
    entity: &Entity,
    key: &[u8],
    default: [f32; 3],
) -> Result<[f32; 3], EnvironmentError> {
    Ok(vector3_or_optional(entity, key)?.unwrap_or(default))
}

fn vector3_or_optional(entity: &Entity, key: &[u8]) -> Result<Option<[f32; 3]>, EnvironmentError> {
    let Some(values) = vector_values(entity, key)? else {
        return Ok(None);
    };
    if values.len() != 3 {
        return Err(entity_failure(
            EnvironmentErrorCode::InvalidField,
            entity,
            key,
        ));
    }
    Ok(Some([values[0], values[1], values[2]]))
}

fn vector4_or(
    entity: &Entity,
    key: &[u8],
    default: [f32; 4],
) -> Result<[f32; 4], EnvironmentError> {
    let Some(values) = vector_values(entity, key)? else {
        return Ok(default);
    };
    if values.len() != 4 {
        return Err(entity_failure(
            EnvironmentErrorCode::InvalidField,
            entity,
            key,
        ));
    }
    Ok([values[0], values[1], values[2], values[3]])
}

fn float_or_entity(entity: &Entity, key: &[u8], default: f32) -> Result<f32, EnvironmentError> {
    Ok(optional_float_entity(entity, key)?.unwrap_or(default))
}

fn optional_float_entity(entity: &Entity, key: &[u8]) -> Result<Option<f32>, EnvironmentError> {
    let Some(bytes) = pair(entity, key) else {
        return Ok(None);
    };
    let value = std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))?;
    Ok(Some(value))
}

fn integer_or(entity: &Entity, key: &[u8], default: i32) -> Result<i32, EnvironmentError> {
    let Some(bytes) = pair(entity, key) else {
        return Ok(default);
    };
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))
}

fn bool_or_entity(entity: &Entity, key: &[u8], default: bool) -> Result<bool, EnvironmentError> {
    let Some(bytes) = pair(entity, key) else {
        return Ok(default);
    };
    if bytes.eq_ignore_ascii_case(b"true") {
        return Ok(true);
    }
    if bytes.eq_ignore_ascii_case(b"false") {
        return Ok(false);
    }
    let value = std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))?;
    Ok(value != 0.0)
}

fn color_or_entity(
    entity: &Entity,
    key: &[u8],
    default: [u8; 4],
) -> Result<[u8; 4], EnvironmentError> {
    let Some(bytes) = pair(entity, key) else {
        return Ok(default);
    };
    let values = std::str::from_utf8(bytes)
        .ok()
        .map(|value| {
            value
                .split_ascii_whitespace()
                .map(str::parse::<u8>)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .ok()
        .flatten()
        .ok_or_else(|| entity_failure(EnvironmentErrorCode::InvalidField, entity, key))?;
    if !(3..=4).contains(&values.len()) {
        return Err(entity_failure(
            EnvironmentErrorCode::InvalidField,
            entity,
            key,
        ));
    }
    Ok([
        values[0],
        values[1],
        values[2],
        values.get(3).copied().unwrap_or(255),
    ])
}

#[derive(Clone, Copy)]
struct ModelTransform {
    origin: [f32; 3],
    rotation: [[f32; 3]; 3],
}

impl ModelTransform {
    fn from_collision(transform: playsrc_collision::Transform) -> Result<Self, EnvironmentError> {
        let [pitch, yaw, roll] = transform.angles.map(f32::to_radians);
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_roll, cos_roll) = roll.sin_cos();
        Ok(Self {
            origin: transform.origin,
            rotation: [
                [
                    cos_pitch * cos_yaw,
                    sin_roll * sin_pitch * cos_yaw - cos_roll * sin_yaw,
                    cos_roll * sin_pitch * cos_yaw + sin_roll * sin_yaw,
                ],
                [
                    cos_pitch * sin_yaw,
                    sin_roll * sin_pitch * sin_yaw + cos_roll * cos_yaw,
                    cos_roll * sin_pitch * sin_yaw - sin_roll * cos_yaw,
                ],
                [-sin_pitch, sin_roll * cos_pitch, cos_roll * cos_pitch],
            ],
        })
    }

    fn point_to_local(self, value: [f32; 3]) -> [f32; 3] {
        let value = sub(value, self.origin);
        [
            value[0] * self.rotation[0][0]
                + value[1] * self.rotation[1][0]
                + value[2] * self.rotation[2][0],
            value[0] * self.rotation[0][1]
                + value[1] * self.rotation[1][1]
                + value[2] * self.rotation[2][1],
            value[0] * self.rotation[0][2]
                + value[1] * self.rotation[1][2]
                + value[2] * self.rotation[2][2],
        ]
    }

    fn vector_to_world(self, value: [f32; 3]) -> [f32; 3] {
        [
            dot(self.rotation[0], value),
            dot(self.rotation[1], value),
            dot(self.rotation[2], value),
        ]
    }

    fn point_to_world(self, value: [f32; 3]) -> [f32; 3] {
        add(self.origin, self.vector_to_world(value))
    }
}

#[derive(Clone, Copy)]
struct ClipVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

struct MarkProjection {
    receiver: Option<MarkReceiver>,
    fragments: Vec<MarkFragment>,
}

#[derive(Clone)]
struct OverlaySource {
    kind: MarkKind,
    index: usize,
    id: i32,
    texture_info: usize,
    faces: Vec<usize>,
    render_order: u8,
    uv: [[f32; 2]; 2],
    points: [[f32; 2]; 4],
    origin: [f32; 3],
    basis_u: [f32; 3],
    basis_v: [f32; 3],
    normal: [f32; 3],
}

struct MarkCompileInput<'a> {
    map: &'a CanonicalMap,
    bsp: &'a Bsp,
    graph: &'a Graph,
    visibility: &'a VisibilityWorld,
    collision: &'a playsrc_collision::World,
    receiver_snapshot: &'a playsrc_collision::Snapshot,
    mark_placements: &'a MarkPlacementSnapshot,
    materials: &'a BTreeMap<usize, &'a Material>,
    mark_materials: &'a [MarkMaterial],
    limits: EnvironmentLimits,
}

fn compile_marks(input: MarkCompileInput<'_>) -> Result<MarkEnvironment, EnvironmentError> {
    let MarkCompileInput {
        map,
        bsp,
        graph,
        visibility,
        collision,
        receiver_snapshot,
        mark_placements,
        materials,
        mark_materials,
        limits,
    } = input;
    if mark_materials.len() > limits.max_marks {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    let mut mark_lookup = BTreeMap::new();
    for (index, material) in mark_materials.iter().enumerate() {
        if material.reference.is_empty()
            || material.logical_path.is_empty()
            || material.width == 0
            || material.height == 0
            || material.width > 16_384
            || material.height > 16_384
        {
            return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(index)));
        }
        let key = lower(&material.reference);
        if mark_lookup.insert(key, material).is_some() {
            return Err(failure(EnvironmentErrorCode::InvalidReference, Some(index)));
        }
    }
    let standard = parse_overlays(bsp, 45, MarkKind::Overlay, 352, 64)?;
    let water = parse_overlays(bsp, 50, MarkKind::WaterOverlay, 1_120, 256)?;
    let infodecal_count = graph
        .entities
        .iter()
        .filter(|entity| class_is(entity, b"infodecal"))
        .count();
    if infodecal_count + standard.len() + water.len() > limits.max_marks {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    let mut records = Vec::with_capacity(infodecal_count + standard.len() + water.len());
    let placement_lookup = mark_placement_lookup(graph, mark_placements, limits)?;
    for entity in graph
        .entities
        .iter()
        .filter(|entity| class_is(entity, b"infodecal"))
    {
        let placement = placement_lookup
            .get(&entity.index)
            .copied()
            .ok_or_else(|| {
                entity_failure(EnvironmentErrorCode::InvalidReference, entity, b"origin")
            })?;
        let origin = placement.world_origin;
        let reference = pair(entity, b"texture")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                entity_failure(EnvironmentErrorCode::InvalidField, entity, b"texture")
            })?;
        let material_path = material_path(reference).ok_or_else(|| {
            entity_failure(EnvironmentErrorCode::InvalidField, entity, b"texture")
        })?;
        let dynamic = entity
            .targetname
            .as_deref()
            .is_some_and(|value| !value.is_empty());
        let low_priority = bool_or_entity(entity, b"LowPriority", false)?;
        let parent_entity = placement.parent_entity;
        let Some(material) = mark_lookup.get(&lower(reference)).copied() else {
            records.push(MarkRecord {
                kind: MarkKind::InfoDecal,
                source_index: entity.index,
                entity: Some(entity.index),
                overlay_id: None,
                status: MarkStatus::Missing,
                material_path,
                material_sha256: None,
                material_state: None,
                origin,
                receiver: None,
                target_faces: Vec::new(),
                render_order: 0,
                fade_distances_squared: None,
                initially_enabled: !dynamic,
                dynamic,
                low_priority,
                parent_entity,
                activation: mark_activation(dynamic),
                lifetime: mark_lifetime(dynamic, low_priority),
                render: mark_render_request(),
                fragments: Vec::new(),
            });
            continue;
        };
        let width = decal_dimension(material.width, material.state.scale)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidRecord, Some(entity.index)))?;
        let height = decal_dimension(material.height, material.state.scale)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidRecord, Some(entity.index)))?;
        let projection = project_infodecal(
            MarkProjectionInput {
                map,
                graph,
                visibility,
                collision,
                receiver_snapshot,
                materials,
            },
            origin,
            [width, height],
        )?;
        let target_faces = unique_fragment_faces(&projection.fragments);
        records.push(MarkRecord {
            kind: MarkKind::InfoDecal,
            source_index: entity.index,
            entity: Some(entity.index),
            overlay_id: None,
            status: if !projection.fragments.is_empty() {
                MarkStatus::Projected
            } else if projection.receiver.is_some() {
                MarkStatus::Ineligible
            } else {
                MarkStatus::Inert
            },
            material_path: material.logical_path.clone(),
            material_sha256: Some(material.source_sha256),
            material_state: Some(material.state),
            origin,
            receiver: projection.receiver,
            target_faces,
            render_order: 0,
            fade_distances_squared: None,
            initially_enabled: !dynamic,
            dynamic,
            low_priority,
            parent_entity,
            activation: mark_activation(dynamic),
            lifetime: mark_lifetime(dynamic, low_priority),
            render: mark_render_request(),
            fragments: projection.fragments,
        });
    }
    let fade_bytes = bsp.lumps[60].bytes(bsp);
    if !fade_bytes.is_empty()
        && (!fade_bytes.len().is_multiple_of(8) || fade_bytes.len() / 8 != standard.len())
    {
        return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(60)));
    }
    for overlay in standard.iter().chain(&water) {
        let info = texture_info(bsp)?
            .get(overlay.texture_info)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(overlay.index)))?;
        let material_index = usize::try_from(info.texture_data_index)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(overlay.index)))?;
        let material_reference = map
            .materials
            .get(material_index)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(overlay.index)))?;
        let material_state = materials
            .get(&material_index)
            .map(|material| material.decal)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(material_index)))?;
        let fragments = project_overlay(map, visibility, overlay)?;
        let target_faces = unique_fragment_faces(&fragments);
        let fade = (overlay.kind == MarkKind::Overlay && !fade_bytes.is_empty()).then(|| {
            let offset = overlay.index * 8;
            [f32_at(fade_bytes, offset), f32_at(fade_bytes, offset + 4)]
        });
        if fade.is_some_and(|values| {
            values.iter().any(|value| !value.is_finite())
                || values[0] < 0.0
                || values[0] > values[1]
        }) {
            return Err(failure(
                EnvironmentErrorCode::InvalidRecord,
                Some(overlay.index),
            ));
        }
        records.push(MarkRecord {
            kind: overlay.kind,
            source_index: overlay.index,
            entity: None,
            overlay_id: Some(overlay.id),
            status: if fragments.is_empty() {
                MarkStatus::Inert
            } else {
                MarkStatus::Projected
            },
            material_path: material_reference.logical_path.clone(),
            material_sha256: None,
            material_state: Some(material_state),
            origin: overlay.origin,
            receiver: None,
            target_faces,
            render_order: overlay.render_order,
            fade_distances_squared: fade,
            initially_enabled: true,
            dynamic: false,
            low_priority: false,
            parent_entity: None,
            activation: MarkActivation::Compiled,
            lifetime: MarkLifetime::Permanent,
            render: mark_render_request(),
            fragments,
        });
    }
    let fragment_count = records.iter().map(|record| record.fragments.len()).sum();
    let vertex_count = records
        .iter()
        .flat_map(|record| &record.fragments)
        .map(|fragment| fragment.positions.len())
        .sum();
    if fragment_count > limits.max_fragments || vertex_count > limits.max_fragment_vertices {
        return Err(failure(EnvironmentErrorCode::BoundExceeded, None));
    }
    Ok(MarkEnvironment {
        collision_world_identity: collision.identity,
        receiver_snapshot_revision: receiver_snapshot.identity(),
        placement_revision: mark_placements.revision,
        records,
        fragment_count,
        vertex_count,
    })
}

fn mark_placement_lookup(
    graph: &Graph,
    snapshot: &MarkPlacementSnapshot,
    limits: EnvironmentLimits,
) -> Result<BTreeMap<usize, MarkPlacement>, EnvironmentError> {
    let expected = graph
        .entities
        .iter()
        .filter(|entity| class_is(entity, b"infodecal"))
        .count();
    if snapshot.placements.len() != expected || snapshot.placements.len() > limits.max_marks {
        return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
    }
    let mut output = BTreeMap::new();
    for placement in &snapshot.placements {
        let entity = graph
            .entities
            .get(placement.entity)
            .filter(|entity| class_is(entity, b"infodecal"))
            .ok_or_else(|| {
                failure(
                    EnvironmentErrorCode::InvalidReference,
                    Some(placement.entity),
                )
            })?;
        if placement
            .world_origin
            .iter()
            .any(|value| !value.is_finite())
            || placement
                .parent_entity
                .is_some_and(|parent| parent >= graph.entities.len())
            || output.insert(entity.index, *placement).is_some()
        {
            return Err(failure(
                EnvironmentErrorCode::InvalidReference,
                Some(placement.entity),
            ));
        }
    }
    Ok(output)
}

fn mark_activation(dynamic: bool) -> MarkActivation {
    if dynamic {
        MarkActivation::Input
    } else {
        MarkActivation::MapActivation
    }
}

fn mark_lifetime(dynamic: bool, low_priority: bool) -> MarkLifetime {
    if dynamic || !low_priority {
        MarkLifetime::Permanent
    } else {
        MarkLifetime::PoolManaged
    }
}

fn mark_render_request() -> MarkRenderRequest {
    MarkRenderRequest {
        normal_offset: MARK_NORMAL_OFFSET,
        polygon_offset: playsrc_material::PolygonOffset::Decal,
    }
}

fn parse_overlays(
    bsp: &Bsp,
    slot: usize,
    kind: MarkKind,
    record_size: usize,
    max_faces: usize,
) -> Result<Vec<OverlaySource>, EnvironmentError> {
    let bytes = bsp.lumps[slot].bytes(bsp);
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bsp.lumps[slot].version != 0 || !bytes.len().is_multiple_of(record_size) {
        return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(slot)));
    }
    let mut output = Vec::with_capacity(bytes.len() / record_size);
    for (index, record) in bytes.chunks_exact(record_size).enumerate() {
        let packed = u16_at(record, 6);
        let face_count = usize::from(packed & 0x3fff);
        if face_count > max_faces {
            return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(index)));
        }
        let faces = (0..face_count)
            .map(|face| {
                usize::try_from(i32_at(record, 8 + face * 4))
                    .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(index)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vectors = 8 + max_faces * 4;
        let uv = [
            [f32_at(record, vectors), f32_at(record, vectors + 4)],
            [f32_at(record, vectors + 8), f32_at(record, vectors + 12)],
        ];
        let mut encoded = [[0.0; 3]; 4];
        for (point, value) in encoded.iter_mut().enumerate() {
            *value = f32x3(record, vectors + 16 + point * 12);
        }
        let points = encoded.map(|value| [value[0], value[1]]);
        let origin = f32x3(record, vectors + 64);
        let normal = f32x3(record, vectors + 76);
        let (basis_u, basis_v) = overlay_basis(&encoded, normal)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidRecord, Some(index)))?;
        if uv
            .iter()
            .flatten()
            .chain(origin.iter())
            .chain(normal.iter())
            .chain(basis_u.iter())
            .any(|value| !value.is_finite())
        {
            return Err(failure(EnvironmentErrorCode::NonFinite, Some(index)));
        }
        output.push(OverlaySource {
            kind,
            index,
            id: i32_at(record, 0),
            texture_info: usize::try_from(i16_at(record, 4))
                .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(index)))?,
            faces,
            render_order: (packed >> 14) as u8,
            uv,
            points,
            origin,
            basis_u,
            basis_v,
            normal,
        });
    }
    Ok(output)
}

fn mark_receiver(
    trace: playsrc_collision::Trace,
    origin: [f32; 3],
    graph: &Graph,
    snapshot: &playsrc_collision::Snapshot,
) -> Result<Option<MarkReceiver>, EnvironmentError> {
    let (entity, model, transform) = match trace.hit {
        Some(playsrc_collision::Hit::WorldBrush { .. }) => {
            (None, 0, playsrc_collision::Transform::IDENTITY)
        }
        Some(playsrc_collision::Hit::Object {
            identity,
            feature: playsrc_collision::Feature::Brush { model, .. },
            ..
        }) => {
            let record = snapshot
                .records()
                .iter()
                .find(|record| record.identity == identity)
                .ok_or_else(|| failure(EnvironmentErrorCode::DependencyMismatch, None))?;
            if !record.enabled
                || !matches!(
                    record.shape,
                    playsrc_collision::SnapshotShape::BrushModel { model: value } if value == model
                )
            {
                return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
            }
            (Some(identity), model, record.transform)
        }
        Some(playsrc_collision::Hit::Object { .. }) => {
            return Err(failure(EnvironmentErrorCode::DependencyMismatch, None));
        }
        None => return Ok(None),
    };
    let local_origin = ModelTransform::from_collision(transform)?.point_to_local(origin);
    let parent_entity = entity
        .and_then(|identity| usize::try_from(identity).ok())
        .and_then(|index| graph.entities.get(index))
        .and_then(|receiver| receiver.parentname.as_deref())
        .and_then(|name| {
            graph.entities.iter().find(|candidate| {
                candidate
                    .targetname
                    .as_deref()
                    .is_some_and(|target| target.eq_ignore_ascii_case(name))
            })
        })
        .map(|entity| entity.index);
    Ok(Some(MarkReceiver {
        entity,
        model,
        local_origin,
        transform,
        parent_entity,
    }))
}

fn mark_visibility(
    visibility: &VisibilityWorld,
    surface: &Surface,
    receiver: Option<u64>,
) -> Result<MarkVisibility, EnvironmentError> {
    if surface.model != 0 {
        return Ok(MarkVisibility::BrushModel {
            entity: receiver.ok_or_else(|| {
                failure(EnvironmentErrorCode::InvalidReference, Some(surface.face))
            })?,
            model: surface.model,
        });
    }
    let face = u16::try_from(surface.face)
        .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(surface.face)))?;
    let mut leaves = Vec::new();
    let mut clusters = BTreeSet::new();
    let mut areas = BTreeSet::new();
    for (leaf_index, leaf) in visibility.leaves.iter().enumerate() {
        let start = usize::from(leaf.first_leaf_face);
        let end = start
            .checked_add(usize::from(leaf.leaf_face_count))
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(leaf_index)))?;
        if visibility
            .leaf_faces
            .get(start..end)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(leaf_index)))?
            .contains(&face)
        {
            leaves.push(leaf_index);
            clusters.insert(leaf.cluster);
            areas.insert(usize::from(leaf.area_and_flags & 0x01ff));
        }
    }
    Ok(MarkVisibility::World {
        leaves,
        clusters: clusters.into_iter().collect(),
        areas: areas.into_iter().collect(),
    })
}

struct MarkProjectionInput<'a> {
    map: &'a CanonicalMap,
    graph: &'a Graph,
    visibility: &'a VisibilityWorld,
    collision: &'a playsrc_collision::World,
    receiver_snapshot: &'a playsrc_collision::Snapshot,
    materials: &'a BTreeMap<usize, &'a Material>,
}

fn project_infodecal(
    input: MarkProjectionInput<'_>,
    origin: [f32; 3],
    dimensions: [f32; 2],
) -> Result<MarkProjection, EnvironmentError> {
    let MarkProjectionInput {
        map,
        graph,
        visibility,
        collision,
        receiver_snapshot,
        materials,
    } = input;
    let start = sub(origin, [INFODECAL_TRACE_EXTENT; 3]);
    let end = add(origin, [INFODECAL_TRACE_EXTENT; 3]);
    let trace = collision
        .trace_snapshot_ray(
            receiver_snapshot,
            playsrc_collision::SnapshotRayRequest {
                start,
                end,
                mask: playsrc_collision::MASK_SOLID,
                scope: playsrc_collision::TraceScope::Everything,
                ignored: &[],
            },
            |_| true,
        )
        .map_err(|_| failure(EnvironmentErrorCode::DependencyMismatch, None))?;
    let Some(receiver) = mark_receiver(trace, origin, graph, receiver_snapshot)? else {
        return Ok(MarkProjection {
            receiver: None,
            fragments: Vec::new(),
        });
    };
    let local_origin = receiver.local_origin;
    let mut fragments = Vec::new();
    for surface in map
        .surfaces
        .iter()
        .filter(|surface| surface.model == receiver.model)
    {
        let normal = stored_surface_normal(surface)?;
        let plane_distance = surface.plane[3];
        if (dot(local_origin, normal) - plane_distance).abs() >= INFODECAL_PLANE_DISTANCE {
            continue;
        }
        let receiving = materials.get(&surface.material).ok_or_else(|| {
            failure(
                EnvironmentErrorCode::InvalidReference,
                Some(surface.material),
            )
        })?;
        if surface.flags & SURF_NODECALS != 0
            || receiving.decal.suppress_decals
            || receiving.decal.alpha_tested
        {
            continue;
        }
        let Some((basis_u, basis_v)) = decal_basis(normal) else {
            continue;
        };
        let basis_u = scale(basis_u, dimensions[0].recip());
        let basis_v = scale(basis_v, dimensions[1].recip());
        let offset_u = 0.5 - dot(local_origin, basis_u);
        let offset_v = 0.5 - dot(local_origin, basis_v);
        let face = surface
            .positions
            .iter()
            .map(|position| ClipVertex {
                position: *position,
                uv: [
                    dot(*position, basis_u) + offset_u,
                    dot(*position, basis_v) + offset_v,
                ],
            })
            .collect();
        if let Some(mut fragment) = clipped_fragment(surface, clip_decal_unit(face), normal) {
            fragment.visibility = mark_visibility(visibility, surface, receiver.entity)?;
            fragments.push(fragment);
        }
    }
    Ok(MarkProjection {
        receiver: Some(receiver),
        fragments,
    })
}

fn project_overlay(
    map: &CanonicalMap,
    visibility: &VisibilityWorld,
    overlay: &OverlaySource,
) -> Result<Vec<MarkFragment>, EnvironmentError> {
    let source = overlay
        .points
        .iter()
        .enumerate()
        .map(|(index, point)| ClipVertex {
            position: add(
                add(overlay.origin, scale(overlay.basis_u, point[0])),
                scale(overlay.basis_v, point[1]),
            ),
            uv: match index {
                0 => [overlay.uv[0][0], overlay.uv[1][0]],
                1 => [overlay.uv[0][0], overlay.uv[1][1]],
                2 => [overlay.uv[0][1], overlay.uv[1][1]],
                _ => [overlay.uv[0][1], overlay.uv[1][0]],
            },
        })
        .collect::<Vec<_>>();
    let mut fragments = Vec::new();
    for face in &overlay.faces {
        let surface = map
            .surfaces
            .get(*face)
            .filter(|surface| surface.face == *face)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(overlay.index)))?;
        let normal = stored_surface_normal(surface)?;
        let denominator = dot(normal, overlay.normal);
        if denominator.abs() < 1.0e-5 {
            continue;
        }
        let plane_distance = surface.plane[3];
        let projected: Vec<ClipVertex> = source
            .iter()
            .map(|vertex| ClipVertex {
                position: add(
                    vertex.position,
                    scale(
                        overlay.normal,
                        (plane_distance - dot(normal, vertex.position)) / denominator,
                    ),
                ),
                uv: vertex.uv,
            })
            .collect();
        let mut clipped = clip_overlay_face(surface, &projected, normal);
        let membership = mark_visibility(visibility, surface, None)?;
        for fragment in &mut clipped {
            fragment.visibility = membership.clone();
        }
        fragments.extend(clipped);
    }
    Ok(fragments)
}

fn overlay_basis(encoded: &[[f32; 3]; 4], normal: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    if normal
        .iter()
        .chain(encoded.iter().flatten())
        .any(|value| !value.is_finite())
        || normalize(normal).is_none()
    {
        return None;
    }
    let basis_u = [encoded[0][2], encoded[1][2], encoded[2][2]];
    normalize(basis_u)?;
    Some((basis_u, normalize(cross(normal, basis_u))?))
}

fn clip_overlay_face(
    surface: &Surface,
    projected: &[ClipVertex],
    normal: [f32; 3],
) -> Vec<MarkFragment> {
    let mut fragments = Vec::new();
    for triangle in (1..surface.positions.len().saturating_sub(1)).map(|index| {
        [
            surface.positions[0],
            surface.positions[index],
            surface.positions[index + 1],
        ]
    }) {
        let triangle_normal = cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0]));
        let area = dot(triangle_normal, triangle_normal).sqrt() * 0.5;
        if area <= 1.0 {
            continue;
        }
        if let Some(fragment) = clipped_fragment(
            surface,
            clip_to_convex(projected.to_vec(), &triangle, normal),
            normal,
        ) {
            fragments.push(fragment);
        }
    }
    fragments
}

fn stored_surface_normal(surface: &Surface) -> Result<[f32; 3], EnvironmentError> {
    let normal = [surface.plane[0], surface.plane[1], surface.plane[2]];
    if normal.iter().any(|value| !value.is_finite()) || dot(normal, normal) <= 1.0e-12 {
        Err(failure(
            EnvironmentErrorCode::InvalidRecord,
            Some(surface.face),
        ))
    } else {
        Ok(normal)
    }
}

fn decal_basis(normal: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    if normal[2].abs() > std::f32::consts::FRAC_1_SQRT_2 {
        let vertical = normalize(cross([1.0, 0.0, 0.0], normal))?;
        Some((normalize(cross(normal, vertical))?, vertical))
    } else {
        let horizontal = normalize(cross(normal, [0.0, 0.0, -1.0]))?;
        Some((horizontal, normalize(cross(horizontal, normal))?))
    }
}

fn clip_to_convex(
    mut subject: Vec<ClipVertex>,
    clip: &[[f32; 3]],
    normal: [f32; 3],
) -> Vec<ClipVertex> {
    if clip.len() < 3 {
        return Vec::new();
    }
    let orientation = dot(cross(sub(clip[1], clip[0]), sub(clip[2], clip[1])), normal).signum();
    for index in 0..clip.len() {
        let edge_start = clip[index];
        let edge = sub(clip[(index + 1) % clip.len()], edge_start);
        let signed_distance =
            |point| dot(cross(edge, sub(point, edge_start)), normal) * orientation;
        let input = std::mem::take(&mut subject);
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let mut previous_distance = signed_distance(previous.position);
        for current in input {
            let current_distance = signed_distance(current.position);
            let previous_inside = previous_distance >= -0.001;
            let current_inside = current_distance >= -0.001;
            if previous_inside != current_inside {
                let denominator = previous_distance - current_distance;
                if denominator.abs() > 1.0e-8 {
                    let amount = previous_distance / denominator;
                    subject.push(ClipVertex {
                        position: add(
                            previous.position,
                            scale(sub(current.position, previous.position), amount),
                        ),
                        uv: [
                            previous.uv[0] + (current.uv[0] - previous.uv[0]) * amount,
                            previous.uv[1] + (current.uv[1] - previous.uv[1]) * amount,
                        ],
                    });
                }
            }
            if current_inside {
                subject.push(current);
            }
            previous = current;
            previous_distance = current_distance;
        }
    }
    subject
}

fn clip_decal_unit(mut polygon: Vec<ClipVertex>) -> Vec<ClipVertex> {
    polygon = clip_decal_boundary(
        polygon,
        |uv| uv[1] < 1.0,
        |one, two| (1.0 - one[1]) / (two[1] - one[1]),
    );
    polygon = clip_decal_boundary(
        polygon,
        |uv| uv[0] > 0.0,
        |one, two| one[0] / (one[0] - two[0]),
    );
    polygon = clip_decal_boundary(
        polygon,
        |uv| uv[0] < 1.0,
        |one, two| (1.0 - one[0]) / (two[0] - one[0]),
    );
    clip_decal_boundary(
        polygon,
        |uv| uv[1] > 0.0,
        |one, two| one[1] / (one[1] - two[1]),
    )
}

fn clip_decal_boundary(
    input: Vec<ClipVertex>,
    inside: impl Fn([f32; 2]) -> bool,
    amount: impl Fn([f32; 2], [f32; 2]) -> f32,
) -> Vec<ClipVertex> {
    let Some(mut previous) = input.last().copied() else {
        return Vec::new();
    };
    let mut output = Vec::new();
    for current in input {
        let previous_inside = inside(previous.uv);
        let current_inside = inside(current.uv);
        if previous_inside != current_inside {
            let amount = amount(previous.uv, current.uv);
            output.push(ClipVertex {
                position: add(
                    previous.position,
                    scale(sub(current.position, previous.position), amount),
                ),
                uv: [
                    previous.uv[0] + (current.uv[0] - previous.uv[0]) * amount,
                    previous.uv[1] + (current.uv[1] - previous.uv[1]) * amount,
                ],
            });
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
    }
    output
}

fn clipped_fragment(
    surface: &Surface,
    polygon: Vec<ClipVertex>,
    normal: [f32; 3],
) -> Option<MarkFragment> {
    if polygon.len() < 3 {
        return None;
    }
    let mut triangles: Vec<_> = (1..polygon.len() - 1)
        .map(|index| [0, index as u32, index as u32 + 1])
        .collect();
    if triangles
        .iter()
        .find_map(|triangle| {
            let a = polygon[triangle[0] as usize].position;
            let b = polygon[triangle[1] as usize].position;
            let c = polygon[triangle[2] as usize].position;
            let facing = dot(cross(sub(b, a), sub(c, a)), normal);
            (facing.abs() > 1.0e-8).then_some(facing)
        })
        .is_some_and(|facing| facing < 0.0)
    {
        for triangle in &mut triangles {
            triangle.swap(1, 2);
        }
    }
    Some(MarkFragment {
        model: surface.model,
        face: surface.face,
        positions: polygon
            .iter()
            .map(|vertex| add(vertex.position, scale(normal, MARK_NORMAL_OFFSET)))
            .collect(),
        normals: vec![normal; polygon.len()],
        uv: polygon.iter().map(|vertex| vertex.uv).collect(),
        lightmap_uv: polygon
            .iter()
            .map(|vertex| {
                std::array::from_fn(|axis| {
                    dot4(surface.lightmap_vectors[axis], vertex.position)
                        - surface.lightmap_mins[axis] as f32
                })
            })
            .collect(),
        triangles,
        visibility: MarkVisibility::World {
            leaves: Vec::new(),
            clusters: Vec::new(),
            areas: Vec::new(),
        },
    })
}

fn decal_dimension(pixels: u32, scale: f32) -> Option<f32> {
    let dimension = pixels as f32 * scale;
    (scale.is_finite() && scale > 0.0 && dimension > 0.0 && dimension <= i32::MAX as f32)
        .then_some(dimension)
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
    let path = format!("{prefix}{reference}{suffix}");
    (!path
        .split('/')
        .any(|component| component.is_empty() || matches!(component, "." | "..")))
    .then_some(path)
}

fn unique_fragment_faces(fragments: &[MarkFragment]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    fragments
        .iter()
        .filter_map(|fragment| seen.insert(fragment.face).then_some(fragment.face))
        .collect()
}

fn texture_info(bsp: &Bsp) -> Result<&[TextureInfo], EnvironmentError> {
    lump_records(bsp, 6, |data| match data {
        LumpData::TextureInfo(value) => Some(value.as_slice()),
        _ => None,
    })
}

fn leaves(bsp: &Bsp) -> Result<&[Leaf], EnvironmentError> {
    lump_records(bsp, 10, |data| match data {
        LumpData::Leaves(value) => Some(value.as_slice()),
        _ => None,
    })
}

fn leaf_minimum_distances(bsp: &Bsp) -> Result<Option<Vec<u16>>, EnvironmentError> {
    let bytes = bsp.lumps[46].bytes(bsp);
    if bytes.is_empty() {
        return Ok(None);
    }
    let leaf_count = leaves(bsp)?.len();
    if bsp.lumps[46].version != 0 || bytes.len() != leaf_count.saturating_mul(2) {
        return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(46)));
    }
    Ok(Some(
        bytes
            .chunks_exact(2)
            .map(|value| u16::from_le_bytes([value[0], value[1]]))
            .collect(),
    ))
}

fn lump_records<T>(
    bsp: &Bsp,
    slot: usize,
    select: impl FnOnce(&LumpData) -> Option<&[T]>,
) -> Result<&[T], EnvironmentError> {
    select(&bsp.lumps[slot].records)
        .ok_or_else(|| failure(EnvironmentErrorCode::MissingLump, Some(slot)))
}

fn face_owners(bsp: &Bsp, face_count: usize) -> Result<Vec<usize>, EnvironmentError> {
    let models = lump_records(bsp, 14, |data| match data {
        LumpData::Models(value) => Some(value.as_slice()),
        _ => None,
    })?;
    let mut output = vec![usize::MAX; face_count];
    for (model_index, model) in models.iter().enumerate() {
        let start = usize::try_from(model.first_face)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(model_index)))?;
        let count = usize::try_from(model.face_count)
            .map_err(|_| failure(EnvironmentErrorCode::InvalidReference, Some(model_index)))?;
        let end = start
            .checked_add(count)
            .filter(|end| *end <= face_count)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(model_index)))?;
        if output[start..end].iter().any(|owner| *owner != usize::MAX) {
            return Err(failure(
                EnvironmentErrorCode::InvalidReference,
                Some(model_index),
            ));
        }
        output[start..end].fill(model_index);
    }
    if output.contains(&usize::MAX) {
        return Err(failure(EnvironmentErrorCode::InvalidReference, None));
    }
    Ok(output)
}

fn finite_bounds(points: &[[f32; 3]], item: usize) -> Result<[[f32; 3]; 2], EnvironmentError> {
    let Some(first) = points.first().copied() else {
        return Err(failure(EnvironmentErrorCode::InvalidRecord, Some(item)));
    };
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(failure(EnvironmentErrorCode::NonFinite, Some(item)));
    }
    let mut minimum = first;
    let mut maximum = first;
    for point in &points[1..] {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    Ok([minimum, maximum])
}

fn leaf_union_bounds(
    leaves: &[Leaf],
    membership: &[usize],
    item: usize,
) -> Result<[[f32; 3]; 2], EnvironmentError> {
    let first = leaves
        .get(membership[0])
        .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(item)))?;
    let mut minimum = first.mins.map(f32::from);
    let mut maximum = first.maxs.map(f32::from);
    for leaf in membership.iter().skip(1) {
        let leaf = leaves
            .get(*leaf)
            .ok_or_else(|| failure(EnvironmentErrorCode::InvalidReference, Some(item)))?;
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(f32::from(leaf.mins[axis]));
            maximum[axis] = maximum[axis].max(f32::from(leaf.maxs[axis]));
        }
    }
    Ok([minimum, maximum])
}

fn squared_distance(position: [f32; 3], origin: [i32; 3]) -> f64 {
    (0..3)
        .map(|axis| {
            let delta = f64::from(position[axis]) - f64::from(origin[axis]);
            delta * delta
        })
        .sum()
}

fn lower(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(u8::to_ascii_lowercase).collect()
}

fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length_squared = dot(value, value);
    if !length_squared.is_finite() || length_squared <= 1.0e-12 {
        return None;
    }
    let inverse = length_squared.sqrt().recip();
    Some(scale(value, inverse))
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn dot4(vector: [f32; 4], point: [f32; 3]) -> f32 {
    point[0] * vector[0] + point[1] * vector[1] + point[2] * vector[2] + vector[3]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn sub(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn scale(value: [f32; 3], scale: f32) -> [f32; 3] {
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated record range"),
    )
}

fn f32x3(bytes: &[u8], offset: usize) -> [f32; 3] {
    [
        f32_at(bytes, offset),
        f32_at(bytes, offset + 4),
        f32_at(bytes, offset + 8),
    ]
}

fn i16_at(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated record range"),
    )
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated record range"),
    )
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated record range"),
    )
}

fn failure(code: EnvironmentErrorCode, item: Option<usize>) -> EnvironmentError {
    EnvironmentError {
        code,
        item,
        field: None,
        logical_path: None,
    }
}

fn entity_failure(code: EnvironmentErrorCode, entity: &Entity, field: &[u8]) -> EnvironmentError {
    EnvironmentError {
        code,
        item: Some(entity.index),
        field: Some(field.to_vec()),
        logical_path: None,
    }
}

fn dependency_failure(code: EnvironmentErrorCode, request: &DependencyRequest) -> EnvironmentError {
    EnvironmentError {
        code,
        item: match request.role {
            DependencyRole::CubemapTexture { sample } => Some(sample),
            DependencyRole::SkyMaterial(_) => None,
        },
        field: None,
        logical_path: Some(request.logical_path.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cubemap(index: usize, origin: [i32; 3]) -> CubemapSample {
        CubemapSample {
            index,
            origin,
            encoded_size: 0,
            requested_dimension: None,
            profile: LightingProfile::Hdr,
            logical_path: format!(
                "materials/maps/test/c{}_{}_{}.hdr.vtf",
                origin[0], origin[1], origin[2]
            ),
            source_sha256: [index as u8; 32],
            width: 32,
            height: 32,
            source_face_count: 7,
            faces: CubeFace::ALL
                .into_iter()
                .map(|face| CubemapFaceDependency { face, mip_count: 6 })
                .collect(),
        }
    }

    #[test]
    fn cubemap_selection_is_nearest_stable_and_never_substitutes() {
        let samples = [cubemap(0, [-10, 0, 0]), cubemap(1, [10, 0, 0])];
        assert_eq!(select_cubemap(&samples, [0.0; 3], None).unwrap().index, 0);
        assert_eq!(
            select_cubemap(&samples, [9.0, 0.0, 0.0], None)
                .unwrap()
                .index,
            1
        );
        assert_eq!(
            select_cubemap(&samples, [9.0, 0.0, 0.0], Some(0))
                .unwrap()
                .index,
            0
        );
        assert_eq!(
            select_cubemap(&samples, [0.0; 3], Some(2))
                .unwrap_err()
                .code,
            EnvironmentErrorCode::MissingDependency
        );
        assert_eq!(
            select_cubemap(&[], [0.0; 3], None).unwrap_err().code,
            EnvironmentErrorCode::MissingDependency
        );
    }

    #[test]
    fn exact_dependency_response_is_required_once() {
        let request = DependencyRequest {
            role: DependencyRole::CubemapTexture { sample: 2 },
            profile: LightingProfile::Hdr,
            logical_path: "materials/maps/test/c1_2_3.hdr.vtf".to_owned(),
        };
        let missing = DependencyCatalog::new(&[])
            .unwrap()
            .get(&request)
            .unwrap_err();
        assert_eq!(missing.code, EnvironmentErrorCode::MissingDependency);
        assert_eq!(
            missing.logical_path.as_deref(),
            Some(request.logical_path.as_str())
        );

        let response = DependencyResponse {
            request: request.clone(),
            metadata: DependencyMetadata::CubemapTexture {
                source_sha256: [7; 32],
                width: 32,
                height: 32,
                mip_count: 6,
                source_face_count: 7,
            },
        };
        assert_eq!(
            DependencyCatalog::new(std::slice::from_ref(&response))
                .unwrap()
                .get(&request)
                .unwrap(),
            &response
        );
        assert_eq!(
            DependencyCatalog::new(&[response.clone(), response])
                .unwrap_err()
                .code,
            EnvironmentErrorCode::DependencyMismatch
        );
    }

    #[test]
    fn water_facts_distinguish_above_below_and_outside() {
        let environment = WaterEnvironment {
            surfaces: Vec::new(),
            leaf_minimum_distance_to_water: None,
            volumes: vec![WaterVolume {
                index: 3,
                surface_z: 10.0,
                minimum_z: -10.0,
                texture_info: 0,
                surface_material: 0,
                bottom_material: None,
                leaves: vec![4, 5],
                clusters: vec![0],
                areas: vec![0],
                contents: 0x20,
                bounds: [[-1.0; 3], [1.0; 3]],
                plane: [0.0, 0.0, 1.0, 10.0],
                surface_bindings: WaterBindings {
                    environment: Some(CubemapSelection::External {
                        logical_path: "materials/test.vtf".to_owned(),
                    }),
                    reflection: true,
                    refraction: true,
                },
                bottom_bindings: None,
                surface_state: test_water_state(),
                bottom_state: None,
                surface_translucent: true,
                bottom_translucent: None,
            }],
        };
        assert_eq!(
            environment.classify_leaf_height(4, 9.0).unwrap(),
            WaterPointFact::Below { volume: 3 }
        );
        assert_eq!(
            environment.classify_leaf_height(5, 10.0).unwrap(),
            WaterPointFact::Above { volume: 3 }
        );
        assert_eq!(
            environment.classify_leaf_height(6, 0.0).unwrap(),
            WaterPointFact::Outside
        );
    }

    #[test]
    fn water_view_plans_select_qualified_volume_and_preserve_pass_order() {
        let mut state = test_water_state();
        state.force_expensive = true;
        state.reflection.disposition = TextureDisposition::BuiltInRenderTarget;
        state.reflection.logical_path = None;
        state.refraction.disposition = TextureDisposition::BuiltInRenderTarget;
        state.refraction.logical_path = None;
        state.reflect_entities = true;
        let mut bottom = state.clone();
        bottom.above_water = false;
        let environment = WaterEnvironment {
            surfaces: Vec::new(),
            volumes: vec![WaterVolume {
                index: 0,
                surface_z: 0.0,
                minimum_z: -16.0,
                texture_info: 0,
                surface_material: 4,
                bottom_material: Some(WaterMaterialIdentity::Map(5)),
                leaves: vec![1],
                clusters: vec![1],
                areas: vec![2],
                contents: 0x1000_0020,
                bounds: [[-16.0; 3], [16.0; 3]],
                plane: [0.0, 0.0, 1.0, 0.0],
                surface_bindings: WaterBindings {
                    environment: Some(CubemapSelection::Nearest { sample: 0 }),
                    reflection: true,
                    refraction: true,
                },
                bottom_bindings: Some(WaterBindings {
                    environment: None,
                    reflection: false,
                    refraction: true,
                }),
                surface_state: state,
                bottom_state: Some(bottom),
                surface_translucent: true,
                bottom_translucent: Some(true),
            }],
            leaf_minimum_distance_to_water: Some(vec![100, 0]),
        };
        let visibility = water_test_visibility();
        let policy = WaterViewPolicy {
            draw_water: true,
            expensive_supported: true,
            draw_reflection: true,
            draw_refraction: true,
            force_expensive: false,
            force_reflect_entities: false,
            fast_clipping: false,
            height_clipping: true,
            eye_water_epsilon: 1.0,
        };
        let above = environment
            .plan_view(
                &visibility,
                WaterViewInput {
                    origin: [0.0, 0.0, 10.0],
                    angles: [10.0, 20.0, 30.0],
                    eye_leaf: 0,
                    qualified_visible_leaves: &[1],
                    near_plane_intersects_selected_volume: true,
                    draw_sky_2d: true,
                    policy,
                },
            )
            .unwrap();
        assert_eq!(above.visible_water.as_ref().unwrap().volume, 0);
        assert_eq!(
            above.render.environment,
            Some(CubemapSelection::Nearest { sample: 0 })
        );
        assert!(above.render.reflect && above.render.refract);
        assert_eq!(
            above.visible_water.as_ref().unwrap().distance_to_water,
            Some(100)
        );
        assert_eq!(
            above
                .passes
                .iter()
                .map(|pass| pass.kind)
                .collect::<Vec<_>>(),
            [
                EnvironmentViewKind::Reflection,
                EnvironmentViewKind::Refraction,
                EnvironmentViewKind::Main,
                EnvironmentViewKind::Intersection,
            ]
        );
        assert_eq!(above.passes[0].origin, [0.0, 0.0, -10.0]);
        assert_eq!(above.passes[0].angles, [-10.0, 20.0, -30.0]);
        assert_eq!(above.passes[0].forced_visibility_leaf, Some(1));
        assert!(above.passes[0].draw_sky_2d && above.passes[0].draw_entities);
        assert_eq!(above.passes[0].clip.unwrap().height, -2.0);
        assert_eq!(above.passes[1].clip.unwrap().height, 2.0);
        assert_eq!(above.passes[3].clip.unwrap().keep, WaterClipKeep::Below);

        let underwater = environment
            .plan_view(
                &visibility,
                WaterViewInput {
                    origin: [0.0, 0.0, -4.0],
                    angles: [0.0; 3],
                    eye_leaf: 1,
                    qualified_visible_leaves: &[],
                    near_plane_intersects_selected_volume: false,
                    draw_sky_2d: true,
                    policy,
                },
            )
            .unwrap();
        assert!(underwater.visible_water.as_ref().unwrap().eye_in_volume);
        assert_eq!(
            underwater.visible_water.as_ref().unwrap().material,
            WaterMaterialIdentity::Map(5)
        );
        assert_eq!(
            underwater
                .visible_water
                .as_ref()
                .unwrap()
                .bindings
                .environment,
            None
        );
        assert!(
            !underwater
                .visible_water
                .as_ref()
                .unwrap()
                .bindings
                .reflection
        );
        assert!(
            underwater
                .visible_water
                .as_ref()
                .unwrap()
                .bindings
                .refraction
        );
        assert_eq!(underwater.render.environment, None);
        assert!(!underwater.render.reflect && underwater.render.refract);
        assert_eq!(
            underwater
                .passes
                .iter()
                .map(|pass| pass.kind)
                .collect::<Vec<_>>(),
            [EnvironmentViewKind::Refraction, EnvironmentViewKind::Main]
        );
        assert!(underwater.passes[0].draw_sky_2d);
        assert!(!underwater.passes[1].draw_sky_2d);

        assert!(
            environment
                .plan_view(
                    &visibility,
                    WaterViewInput {
                        origin: [0.0, 0.0, 10.0],
                        angles: [0.0; 3],
                        eye_leaf: 0,
                        qualified_visible_leaves: &[],
                        near_plane_intersects_selected_volume: false,
                        draw_sky_2d: true,
                        policy,
                    },
                )
                .unwrap()
                .visible_water
                .is_none()
        );
    }

    #[test]
    fn fog_player_transition_only_blends_primary_color_start_and_end() {
        let from = fog_test_state([10, 20, 30, 40], [1, 2, 3, 4], 100.0, 200.0, 0.0);
        let to = fog_test_state([110, 120, 130, 140], [9, 8, 7, 6], 300.0, 500.0, 4.0);
        let transition = FogPlayerTransition {
            from: from.clone(),
            to: to.clone(),
            start_time: 10.0,
            snap: false,
        };
        let halfway = transition.sample(12.0).unwrap();
        assert_eq!(halfway.primary, [60, 70, 80, 140]);
        assert_eq!(halfway.secondary, to.secondary);
        assert_eq!(halfway.start, 200.0);
        assert_eq!(halfway.end, 350.0);
        assert_eq!(halfway.enabled, to.enabled);
        assert_eq!(transition.sample(14.0).unwrap(), to);
        assert_eq!(
            FogPlayerTransition {
                from,
                to: to.clone(),
                start_time: 10.0,
                snap: true,
            }
            .sample(10.0)
            .unwrap(),
            to
        );
    }

    #[test]
    fn last_explicit_fog_master_replaces_the_first_controller() {
        let graph = playsrc_entity::parse(
            br#"{"classname" "env_fog_controller" "fogcolor" "1 2 3"}
{"classname" "env_fog_controller" "spawnflags" "1" "fogcolor" "4 5 6"}
{"classname" "env_fog_controller" "spawnflags" "1" "fogcolor" "7 8 9"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let visibility = water_test_visibility();
        let controllers = compile_controllers(&graph, &visibility).unwrap();
        assert_eq!(
            master_fog_controller(&graph, &controllers).unwrap(),
            Some(2)
        );
    }

    fn fog_test_state(
        primary: [u8; 4],
        secondary: [u8; 4],
        start: f32,
        end: f32,
        transition_duration: f32,
    ) -> FogState {
        FogState {
            enabled: true,
            blend: false,
            direction: [1.0, 0.0, 0.0],
            primary,
            secondary,
            start,
            end,
            maximum_density: 1.0,
            far_z: Some(0.0),
            radial: false,
            transition_duration,
        }
    }

    fn water_test_visibility() -> VisibilityWorld {
        let leaf = |contents: i32, cluster: i16, area: u16, water: i16| Leaf {
            contents,
            cluster,
            area_and_flags: area,
            mins: [-16; 3],
            maxs: [16; 3],
            first_leaf_face: 0,
            leaf_face_count: 0,
            first_leaf_brush: 0,
            leaf_brush_count: 0,
            leaf_water_data_id: water,
            padding: 0,
            ambient_cube: None,
        };
        VisibilityWorld {
            identity: [0; 32],
            visibility_mode: playsrc_visibility::VisibilityMode::NoVis,
            cluster_count: 2,
            words_per_row: 1,
            pvs: Vec::new(),
            pas: Vec::new(),
            planes: Vec::new(),
            nodes: Vec::new(),
            leaves: vec![
                leaf(CONTENTS_TEST_FOG_VOLUME as i32, 0, 1, -1),
                leaf(0x1000_0020, 1, 2, 0),
            ],
            leaf_faces: Vec::new(),
            models: Vec::new(),
            areas: Vec::new(),
            portals: Vec::new(),
            portal_vertices: Vec::new(),
        }
    }

    #[test]
    fn water_cubemap_preserves_declared_profile_sample_or_nearest_request() {
        let samples = [cubemap(0, [-10, 0, 0]), cubemap(1, [10, 0, 0])];
        let mut state = test_water_state();
        state.environment_map.logical_path =
            Some(samples[0].logical_path.replace(".hdr.vtf", ".vtf"));
        assert_eq!(
            water_cubemap(&state, [9.0, 0.0, 0.0], &samples).unwrap(),
            Some(CubemapSelection::Declared { sample: 0 })
        );
        state.environment_map.logical_path = None;
        assert_eq!(
            water_cubemap(&state, [9.0, 0.0, 0.0], &samples).unwrap(),
            None
        );
        state.environment_map.disposition = TextureDisposition::BuiltInEnvironment;
        state.environment_map.logical_path = None;
        assert_eq!(
            water_cubemap(&state, [9.0, 0.0, 0.0], &samples).unwrap(),
            Some(CubemapSelection::Nearest { sample: 1 })
        );
    }

    fn test_water_state() -> WaterState {
        let texture = playsrc_material::TextureRequest {
            role: playsrc_material::TextureRole::Normal,
            parameter: Vec::new(),
            reference: b"test".to_vec(),
            logical_path: Some("materials/test.vtf".to_owned()),
            disposition: playsrc_material::TextureDisposition::Source,
            color_read: playsrc_material::TextureColorRead::Linear,
        };
        WaterState {
            above_water: true,
            normal_map: texture.clone(),
            environment_map: texture.clone(),
            reflection: texture.clone(),
            refraction: texture,
            bottom_material: None,
            underwater_overlay: None,
            reflect_amount: 0.8,
            refract_amount: 0.0,
            reflect_tint: [1.0; 3],
            refract_tint: [1.0; 3],
            fog: playsrc_material::WaterFog {
                enabled: true,
                color: [0.0; 3],
                start: 0.0,
                end: 1.0,
            },
            cheap_start: 500.0,
            cheap_end: 1000.0,
            force_cheap: false,
            force_expensive: true,
            no_fresnel: false,
            reflect_entities: false,
            blur_refraction: false,
            scroll: [[0.0; 3]; 2],
            has_proxy_program: false,
        }
    }

    #[test]
    fn convex_clipping_emits_stable_counter_clockwise_geometry() {
        let subject = vec![
            ClipVertex {
                position: [-2.0, -2.0, 0.0],
                uv: [0.0, 0.0],
            },
            ClipVertex {
                position: [2.0, -2.0, 0.0],
                uv: [1.0, 0.0],
            },
            ClipVertex {
                position: [2.0, 2.0, 0.0],
                uv: [1.0, 1.0],
            },
            ClipVertex {
                position: [-2.0, 2.0, 0.0],
                uv: [0.0, 1.0],
            },
        ];
        let clip = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let result = clip_to_convex(subject, &clip, [0.0, 0.0, 1.0]);
        assert_eq!(result.len(), 4);
        assert!(
            result
                .iter()
                .all(|vertex| vertex.position[0].abs() <= 1.0 && vertex.position[1].abs() <= 1.0)
        );
    }

    fn mark_surface(positions: Vec<[f32; 3]>, plane: [f32; 4]) -> Surface {
        Surface {
            face: 7,
            model: 0,
            material: 0,
            texture_info: 0,
            flags: 0,
            draw: true,
            plane,
            plane_back: false,
            texture_vectors: [[0.0; 4]; 2],
            lightmap_vectors: [[0.0; 4]; 2],
            lightmap_mins: [0; 2],
            texture_size: [64, 64],
            uv_origin: super::super::TextureCoordinateOrigin::TopLeft,
            normals: vec![[plane[0], plane[1], plane[2]]; positions.len()],
            uv: vec![[0.0; 2]; positions.len()],
            lightmap_uv: vec![[0.0; 2]; positions.len()],
            triangles: Vec::new(),
            positions,
            light_offset: -1,
            light_styles: [255; 4],
            lightmap_size: [0; 2],
            compiled_primitives: false,
        }
    }

    #[test]
    fn decal_basis_fractional_dimensions_and_normal_offset_are_exact() {
        assert_eq!(
            decal_basis([0.0, 0.0, 1.0]).unwrap(),
            ([1.0, 0.0, 0.0], [0.0, -1.0, 0.0])
        );
        assert_eq!(
            decal_basis([1.0, 0.0, 0.0]).unwrap(),
            ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
        );
        assert_eq!(decal_dimension(3, 0.5), Some(1.5));
        assert_eq!(decal_dimension(3, 0.25), Some(0.75));

        let mut surface = mark_surface(
            vec![
                [-1.0, -1.0, 0.0],
                [1.0, -1.0, 0.0],
                [1.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0],
            ],
            [0.0, 0.0, 1.0, 0.0],
        );
        surface.lightmap_vectors = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]];
        surface.lightmap_mins = [-1, -1];
        let polygon = vec![
            ClipVertex {
                position: [-1.0, -1.0, 0.0],
                uv: [0.0, 0.0],
            },
            ClipVertex {
                position: [1.0, -1.0, 0.0],
                uv: [1.0, 0.0],
            },
            ClipVertex {
                position: [1.0, 1.0, 0.0],
                uv: [1.0, 1.0],
            },
            ClipVertex {
                position: [-1.0, 1.0, 0.0],
                uv: [0.0, 1.0],
            },
        ];
        let fragment = clipped_fragment(&surface, polygon, [0.0, 0.0, 1.0]).unwrap();
        assert!(fragment.positions.iter().all(|position| position[2] == 0.1));
        assert_eq!(
            fragment.uv,
            [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        );
        assert_eq!(
            fragment.lightmap_uv,
            [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        );

        let clipped = clip_decal_unit(vec![
            ClipVertex {
                position: [-1.0, -1.0, 0.0],
                uv: [-1.0, -1.0],
            },
            ClipVertex {
                position: [2.0, -1.0, 0.0],
                uv: [2.0, -1.0],
            },
            ClipVertex {
                position: [2.0, 2.0, 0.0],
                uv: [2.0, 2.0],
            },
            ClipVertex {
                position: [-1.0, 2.0, 0.0],
                uv: [-1.0, 2.0],
            },
        ]);
        assert_eq!(
            clipped.iter().map(|vertex| vertex.uv).collect::<Vec<_>>(),
            [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]
        );
    }

    #[test]
    fn model_local_mark_geometry_follows_current_receiver_transform() {
        let fragment = MarkFragment {
            model: 4,
            face: 7,
            positions: vec![[1.0, 0.0, 0.0]],
            normals: vec![[1.0, 0.0, 0.0]],
            uv: vec![[0.5, 0.5]],
            lightmap_uv: vec![[0.0, 0.0]],
            triangles: Vec::new(),
            visibility: MarkVisibility::BrushModel {
                entity: 9,
                model: 4,
            },
        };
        let moved = fragment
            .at_transform(playsrc_collision::Transform {
                origin: [10.0, 20.0, 30.0],
                angles: [0.0, 90.0, 0.0],
            })
            .unwrap();
        assert!((moved.positions[0][0] - 10.0).abs() < 0.000_001);
        assert!((moved.positions[0][1] - 21.0).abs() < 0.000_001);
        assert_eq!(moved.positions[0][2], 30.0);
        assert!(moved.normals[0][0].abs() < 0.000_001);
        assert!((moved.normals[0][1] - 1.0).abs() < 0.000_001);
    }

    #[test]
    fn mark_visibility_requires_view_admission_and_all_producer_revisions() {
        let fragment = |model, face, visibility| MarkFragment {
            model,
            face,
            positions: vec![[0.0; 3]],
            normals: vec![[0.0, 0.0, 1.0]],
            uv: vec![[0.0; 2]],
            lightmap_uv: vec![[0.0; 2]],
            triangles: Vec::new(),
            visibility,
        };
        let environment = MarkEnvironment {
            collision_world_identity: [3; 32],
            receiver_snapshot_revision: 4,
            placement_revision: 5,
            fragment_count: 2,
            vertex_count: 2,
            records: vec![MarkRecord {
                kind: MarkKind::InfoDecal,
                source_index: 1,
                entity: Some(1),
                overlay_id: None,
                status: MarkStatus::Projected,
                material_path: "materials/test.vmt".to_owned(),
                material_sha256: Some([0; 32]),
                material_state: None,
                origin: [0.0; 3],
                receiver: None,
                target_faces: vec![7],
                render_order: 0,
                fade_distances_squared: None,
                initially_enabled: true,
                dynamic: false,
                low_priority: false,
                parent_entity: None,
                activation: MarkActivation::MapActivation,
                lifetime: MarkLifetime::Permanent,
                render: mark_render_request(),
                fragments: vec![
                    fragment(
                        0,
                        7,
                        MarkVisibility::World {
                            leaves: vec![2],
                            clusters: vec![1],
                            areas: vec![3],
                        },
                    ),
                    fragment(
                        4,
                        8,
                        MarkVisibility::BrushModel {
                            entity: 9,
                            model: 4,
                        },
                    ),
                ],
            }],
        };
        let view = playsrc_visibility::ViewResult {
            cache_identity: [0; 32],
            origin_leaves: vec![0],
            origin_clusters: vec![0],
            outside_world: false,
            merged_pvs: vec![1],
            visible_areas: vec![3],
            sky: playsrc_visibility::SkyVisibility::NotVisible,
            leaves: vec![2],
            world_surfaces: vec![7],
            candidates: Vec::new(),
        };
        assert_eq!(
            environment
                .visible_fragments(
                    &view,
                    &[VisibleBrushMarkReceiver {
                        entity: 9,
                        model: 4,
                    }],
                    [3; 32],
                    4,
                    5,
                )
                .unwrap(),
            [
                VisibleMarkFragment {
                    record: 0,
                    fragment: 0,
                },
                VisibleMarkFragment {
                    record: 0,
                    fragment: 1,
                },
            ]
        );
        assert_eq!(
            environment
                .visible_fragments(&view, &[], [3; 32], 4, 5)
                .unwrap(),
            [VisibleMarkFragment {
                record: 0,
                fragment: 0,
            }]
        );
        assert_eq!(
            environment
                .visible_fragments(&view, &[], [3; 32], 4, 6)
                .unwrap_err()
                .code,
            EnvironmentErrorCode::DependencyMismatch
        );
    }

    #[test]
    fn explicit_mark_placement_preserves_parent_resolved_world_origin() {
        let graph = playsrc_entity::parse(
            br#"{"classname" "func_movelinear" "targetname" "mover" "model" "*1"}
{"classname" "infodecal" "parentname" "mover" "texture" "test" "origin" "1 2 3"}
"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let snapshot = MarkPlacementSnapshot {
            revision: 7,
            placements: vec![MarkPlacement {
                entity: 1,
                world_origin: [11.0, 22.0, 33.0],
                parent_entity: Some(0),
            }],
        };
        assert_eq!(
            mark_placement_lookup(&graph, &snapshot, EnvironmentLimits::default()).unwrap()[&1],
            snapshot.placements[0]
        );
        let mut stale = snapshot;
        stale.placements[0].entity = 0;
        assert_eq!(
            mark_placement_lookup(&graph, &stale, EnvironmentLimits::default())
                .unwrap_err()
                .code,
            EnvironmentErrorCode::InvalidReference
        );
    }

    #[test]
    fn overlay_basis_ignores_fourth_point_z_and_clips_each_source_triangle() {
        let encoded = [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0],
        ];
        let (basis_u, basis_v) = overlay_basis(&encoded, [0.0, 0.0, 1.0]).unwrap();
        assert_eq!(basis_u, [1.0, 0.0, 0.0]);
        assert_eq!(basis_v, [0.0, 1.0, 0.0]);

        let surface = mark_surface(
            vec![
                [-2.0, -2.0, 0.0],
                [2.0, -2.0, 0.0],
                [2.0, 2.0, 0.0],
                [-2.0, 2.0, 0.0],
            ],
            [0.0, 0.0, 1.0, 0.0],
        );
        let projected = vec![
            ClipVertex {
                position: [-2.0, -2.0, 0.0],
                uv: [0.0, 0.0],
            },
            ClipVertex {
                position: [-2.0, 2.0, 0.0],
                uv: [0.0, 1.0],
            },
            ClipVertex {
                position: [2.0, 2.0, 0.0],
                uv: [1.0, 1.0],
            },
            ClipVertex {
                position: [2.0, -2.0, 0.0],
                uv: [1.0, 0.0],
            },
        ];
        let fragments = clip_overlay_face(&surface, &projected, [0.0, 0.0, 1.0]);
        assert_eq!(fragments.len(), 2);
        assert!(
            fragments
                .iter()
                .flat_map(|fragment| &fragment.positions)
                .all(|position| position[2] == 0.1)
        );

        let small = mark_surface(
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [0.0, 0.0, 1.0, 0.0],
        );
        assert!(clip_overlay_face(&small, &projected, [0.0, 0.0, 1.0]).is_empty());
    }

    #[test]
    fn fog_angles_far_z_and_density_preserve_entity_state() {
        let graph = playsrc_entity::parse(
            br#"{"classname" "env_fog_controller" "use_angles" "1" "angles" "0 90 0" "fogmaxdensity" "2.5"}"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let fog = fog_state(&graph.entities[0], true).unwrap();
        assert!(fog.direction[0].abs() < 1.0e-6);
        assert_eq!(fog.direction[1], -1.0);
        assert_eq!(fog.direction[2], 0.0);
        assert_eq!(fog.maximum_density, 2.5);
        assert_eq!(fog.far_z, Some(0.0));
    }
}
