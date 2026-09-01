use playsrc_bsp::{
    Brush as BspBrush, BrushSide as BspSide, Bsp, Leaf, LumpData, Model, Node, Plane as BspPlane,
};
use sha2::{Digest, Sha256};
use std::{fmt, ops::Range, sync::Arc};

mod brush_visits;
mod bounds;
mod contact;
mod displacement;
mod entity_queries;
mod hitbox;
mod lighting;
#[cfg(feature = "replay-reference")]
pub mod replay_diagnostics;
mod snapshot;

use brush_visits::BrushVisits;
pub use bounds::{BoundsTrace,trace_bounds};
pub use entity_queries::EntityQuerySpace;

pub use contact::{
    CONTACT_SNAPSHOT_VERSION, ContactEdge, ContactEdgeKind, ContactFrame, ContactLimits,
    ContactSnapshot, ContactSubject, TriggerVolume, produce_trigger_contacts,
};

pub use hitbox::{
    StudioHitbox, StudioHitboxError, StudioHitboxRequest, StudioHitboxTrace, trace_studio_hitboxes,
};
pub use lighting::{
    LIGHTING_RAY_BATCH_VERSION, LightingOccluders, LightingRay, LightingRayBatch,
    LightingRayLimits, LightingRayResult,
};

pub use snapshot::{
    PhysicsQuery,
    AuthoredConvex, AuthoredEnclosure, AuthoredHierarchy, AuthoredHierarchyNode, AuthoredHullRef,
    AuthoredShapeProperties, AuthoredTriangle, Candidate, ConvexInput, ObjectInput,
    ObjectOverlapRequest, ObjectRole, ObjectTraceRequest, PhysicsShape, PointContentsContributor,
    PointContentsResult, QueryScratch, SNAPSHOT_VERSION, Snapshot, SnapshotLimits,
    SnapshotRayRequest, SnapshotRecord, SnapshotShape, SnapshotTraceRequest, TraceScope, Transform,
};

pub const CONTENTS_SOLID: u32 = 0x0000_0001;
pub const CONTENTS_WINDOW: u32 = 0x0000_0002;
pub const CONTENTS_GRATE: u32 = 0x0000_0008;
pub const CONTENTS_SLIME: u32 = 0x0000_0010;
pub const CONTENTS_WATER: u32 = 0x0000_0020;
pub const CONTENTS_OPAQUE: u32 = 0x0000_0080;
pub const MASK_CURRENT: u32 = 0x00fc_0000;
pub const CONTENTS_MOVEABLE: u32 = 0x0000_4000;
pub const CONTENTS_PLAYERCLIP: u32 = 0x0001_0000;
pub const CONTENTS_MONSTER: u32 = 0x0200_0000;
pub const CONTENTS_DEBRIS: u32 = 0x0400_0000;
pub const CONTENTS_TRANSLUCENT: u32 = 0x1000_0000;
pub const CONTENTS_HITBOX: u32 = 0x4000_0000;
pub const MASK_SOLID: u32 =
    CONTENTS_SOLID | CONTENTS_MOVEABLE | CONTENTS_WINDOW | CONTENTS_MONSTER | CONTENTS_GRATE;
pub const MASK_PLAYERSOLID: u32 = MASK_SOLID | CONTENTS_PLAYERCLIP;
pub const MASK_SHOT_HULL:u32 = MASK_SOLID | CONTENTS_DEBRIS;
pub const MASK_WATER: u32 = CONTENTS_WATER | CONTENTS_MOVEABLE | CONTENTS_SLIME;
pub const MASK_OPAQUE: u32 = CONTENTS_SOLID | CONTENTS_MOVEABLE | CONTENTS_OPAQUE;
pub const SURF_SKY: u16 = 0x0004;
pub const SURF_HITBOX: u16 = 0x8000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: [f32; 3],
    pub distance: f32,
    pub kind: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brush {
    pub first_side: usize,
    pub side_count: usize,
    pub contents: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Side {
    pub plane: usize,
    pub texture_info: i16,
    pub displacement: i16,
    pub bevel: i16,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DisplacementPatch {
    pub source: usize,
    pub map_face: u16,
    pub power: u8,
    pub contents: u32,
    pub vertex_start: u32,
    pub triangle_start: u32,
    pub start_position: [f32; 3],
    pub minimum_tessellation: i32,
    pub smoothing_angle: f32,
    pub allowed_vertices: [u32; 10],
    pub edge_neighbors: [playsrc_bsp::DispNeighbor; 4],
    pub corner_neighbors: [playsrc_bsp::DispCornerNeighbors; 4],
    pub vertices: Vec<([f32; 3], f32, f32)>,
    pub triangle_tags: Vec<u16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceIdentity {
    pub registry: [u8; 32],
    pub index: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DisplacementInput {
    pub source: usize,
    pub parent_face: usize,
    pub contents: u32,
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub triangle_tags: Vec<u16>,
    pub primary_surface: SurfaceIdentity,
    pub secondary_surface: Option<SurfaceIdentity>,
    pub use_secondary_surface: Vec<bool>,
}
#[derive(Clone, Debug)]
pub struct World {
    pub identity: [u8; 32],
    pub planes: Vec<Plane>,
    pub sides: Vec<Side>,
    pub brushes: Vec<Brush>,
    pub leaves: Vec<Leaf>,
    pub leaf_brushes: Vec<u16>,
    pub nodes: Vec<Node>,
    pub models: Vec<Model>,
    pub world_brushes: Vec<usize>,
    pub model_brushes: Vec<Vec<usize>>,
    pub model_contents: Vec<u32>,
    pub texture_flags: Vec<u16>,
    pub displacements: Vec<DisplacementPatch>,
    displacement_inputs: Vec<DisplacementInput>,
    displacement_trees: displacement::Acceleration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalModelSolid {
    pub solid: usize,
    pub contents: u32,
    pub surface_property: Option<Vec<u8>>,
    pub shape: Arc<PhysicsShape>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalModel {
    pub model: usize,
    pub authored_bounds: Hull,
    pub solids: Vec<PhysicalModelSolid>,
    pub key_data: playsrc_phy::KeyData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalModelInventory {
    world: [u8; 32],
    models: Vec<PhysicalModel>,
}

impl PhysicalModelInventory {
    pub fn compile(world: &World, bsp: &Bsp, limits: SnapshotLimits) -> Result<Self, Error> {
        let blocks = bsp
            .physics_models(world.models.len())
            .map_err(|_| error(ErrorCode::InvalidRange, Some(29)))?;
        let mut models = Vec::with_capacity(blocks.len());
        for block in blocks {
            let contents = *world
                .model_contents
                .get(block.model_index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(block.model_index)))?;
            let asset = playsrc_phy::parse_payload(
                block.collision,
                block.keydata,
                block.solid_count,
                playsrc_phy::Profile::SourcePcPolygon,
                playsrc_phy::Limits::default(),
            )
            .map_err(|_| error(ErrorCode::InvalidRange, Some(block.model_index)))?;
            let mut solids = Vec::with_capacity(asset.solids.len());
            for solid in 0..asset.solids.len() {
                let properties = asset.key_data.blocks.iter().find(|candidate| {
                    candidate.entries.iter().any(|entry| {
                        matches!(entry, playsrc_phy::KeyValue::Scalar { key, value }
                            if key.eq_ignore_ascii_case(b"index")
                                && std::str::from_utf8(value).ok().and_then(|value| value.parse::<usize>().ok()) == Some(solid))
                    })
                });
                let scalar = |name: &[u8]| {
                    properties.and_then(|block| {
                        block.entries.iter().find_map(|entry| match entry {
                            playsrc_phy::KeyValue::Scalar { key, value }
                                if key.eq_ignore_ascii_case(name) =>
                            {
                                Some(value.as_slice())
                            }
                            _ => None,
                        })
                    })
                };
                let solid_contents = scalar(b"contents")
                    .map(|value| {
                        std::str::from_utf8(value)
                            .ok()
                            .and_then(|value| value.parse::<u32>().ok())
                            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(block.model_index)))
                    })
                    .transpose()?
                    .unwrap_or(contents);
                let mut identity = Sha256::new();
                identity.update(world.identity);
                identity.update((block.model_index as u64).to_le_bytes());
                identity.update((solid as u64).to_le_bytes());
                identity.update(block.collision);
                let digest = identity.finalize();
                let identity = u64::from_le_bytes(digest[..8].try_into().expect("shape identity"));
                let shape =
                    PhysicsShape::from_phy(identity, &asset, solid, limits, |_| solid_contents)?;
                solids.push(PhysicalModelSolid {
                    solid,
                    contents: solid_contents,
                    surface_property: scalar(b"surfaceprop").map(<[u8]>::to_vec),
                    shape: Arc::new(shape),
                });
            }
            let model = world.models.get(block.model_index).ok_or_else(|| error(ErrorCode::InvalidReference, Some(block.model_index)))?;
            models.push(PhysicalModel {
                model: block.model_index,
                authored_bounds: Hull {
                    mins: [model.mins.x.value(), model.mins.y.value(), model.mins.z.value()],
                    maxs: [model.maxs.x.value(), model.maxs.y.value(), model.maxs.z.value()],
                },
                solids,
                key_data:asset.key_data,
            });
        }
        Ok(Self {
            world: world.identity,
            models,
        })
    }

    pub fn world_identity(&self) -> [u8; 32] {
        self.world
    }

    pub fn models(&self) -> &[PhysicalModel] {
        &self.models
    }

    pub fn model(&self, model: usize) -> Option<&PhysicalModel> {
        self.models.iter().find(|entry| entry.model == model)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub world: [u8; 32],
    pub fraction: f32,
    pub fraction_left_solid: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub brush: Option<usize>,
    pub contents: u32,
    pub plane: Option<Plane>,
    pub surface_flags: u16,
    pub displacement_flags: u16,
    pub surface: Option<SurfaceIdentity>,
    pub displacement: Option<DisplacementFeature>,
    pub hit: Option<Hit>,
    pub snapshot: Option<u64>,
    pub end: [f32; 3],
}
impl Trace {
    pub fn did_hit(self) -> bool {
        self.fraction < 1.0 || self.start_solid || self.all_solid
    }

    pub fn is_sky(self) -> bool {
        self.surface_flags & SURF_SKY != 0
    }

    pub fn entity_identity(self) -> Option<u64> {
        match self.hit {
            Some(Hit::Object {
                identity,
                role: ObjectRole::Entity,
                ..
            }) => Some(identity),
            _ => None,
        }
    }

    pub fn hit_world(self) -> bool {
        self.displacement.is_some()
            || matches!(
                self.hit,
                Some(Hit::WorldBrush { .. })
                    | Some(Hit::Object {
                        role: ObjectRole::StaticProp,
                        ..
                    })
            )
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplacementFeature {
    pub source: usize,
    pub parent_face: usize,
    pub triangle: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hit {
    WorldBrush {
        brush: usize,
    },
    Object {
        identity: u64,
        role: ObjectRole,
        feature: Feature,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Feature {
    Brush {
        model: usize,
        brush: usize,
    },
    Box,
    Convex {
        solid: usize,
        convex: usize,
        triangle: Option<usize>,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    MissingLump,
    InvalidRange,
    InvalidReference,
    NonFinite,
    InvalidHull,
    InvalidSnapshot,
    DuplicateIdentity,
    Limit,
    Cancelled,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub item: Option<usize>,
    pub range: Option<Range<usize>>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for Error {}

pub fn compile(bsp: &Bsp) -> Result<World, Error> {
    let planes = match &bsp.lumps[1].records {
        LumpData::Planes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let sides = match &bsp.lumps[19].records {
        LumpData::BrushSides(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let texture_info = match &bsp.lumps[6].records {
        LumpData::TextureInfo(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let brushes = match &bsp.lumps[18].records {
        LumpData::Brushes(v) => v,
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaves = match &bsp.lumps[10].records {
        LumpData::Leaves(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let leaf_brushes = match &bsp.lumps[17].records {
        LumpData::LeafBrushes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let nodes = match &bsp.lumps[5].records {
        LumpData::Nodes(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let models = match &bsp.lumps[14].records {
        LumpData::Models(v) => v.clone(),
        _ => return Err(error(ErrorCode::MissingLump, None)),
    };
    let displacement_info = match &bsp.lumps[26].records {
        LumpData::DispInfo(value) => value.as_slice(),
        LumpData::Opaque if bsp.lumps[26].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(26))),
    };
    let displacement_vertices = match &bsp.lumps[33].records {
        LumpData::DispVertices(value) => value.as_slice(),
        LumpData::Opaque if bsp.lumps[33].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(33))),
    };
    let displacement_triangles = match &bsp.lumps[48].records {
        LumpData::DispTriangles(value) => value.as_slice(),
        LumpData::Opaque if bsp.lumps[48].bytes(bsp).is_empty() => &[],
        _ => return Err(error(ErrorCode::MissingLump, Some(48))),
    };
    let planes = planes
        .iter()
        .enumerate()
        .map(|(i, p)| plane(p, i))
        .collect::<Result<Vec<_>, _>>()?;
    let texture_flags = texture_info
        .iter()
        .map(|value| value.flags as u16)
        .collect::<Vec<_>>();
    let sides = sides.iter().map(side).collect::<Vec<_>>();
    let mut output = Vec::with_capacity(brushes.len());
    for (i, b) in brushes.iter().enumerate() {
        let brush = brush(b, i, sides.len())?;
        for side in &sides[brush.first_side..brush.first_side + brush.side_count] {
            if side.plane >= planes.len() {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
            if side.texture_info < -1
                || side.texture_info >= 0 && side.texture_info as usize >= texture_flags.len()
            {
                return Err(error(ErrorCode::InvalidReference, Some(i)));
            }
        }
        output.push(brush);
    }
    for (i, leaf) in leaves.iter().enumerate() {
        let start = leaf.first_leaf_brush as usize;
        let end = start
            .checked_add(leaf.leaf_brush_count as usize)
            .ok_or_else(|| error(ErrorCode::InvalidRange, Some(i)))?;
        if end > leaf_brushes.len()
            || leaf_brushes[start..end]
                .iter()
                .any(|v| *v as usize >= output.len())
        {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        if node.plane_index < 0 || node.plane_index as usize >= planes.len() {
            return Err(error(ErrorCode::InvalidReference, Some(i)));
        }
    }
    if models.is_empty() {
        return Err(error(ErrorCode::InvalidReference, None));
    }
    let model_brushes = models
        .iter()
        .map(|model| {
            brushes_for_model(
                model.head_node,
                &nodes,
                &leaves,
                &leaf_brushes,
                output.len(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let world_brushes = model_brushes[0].clone();
    let model_contents = model_brushes
        .iter()
        .map(|model| {
            model
                .iter()
                .fold(0_u32, |contents, brush| contents | output[*brush].contents)
        })
        .collect();
    let displacements = displacement_info
        .iter()
        .enumerate()
        .map(|(source, info)| {
            let start_position = [
                info.start_position.x.value(),
                info.start_position.y.value(),
                info.start_position.z.value(),
            ];
            let smoothing_angle = info.smoothing_angle.value();
            if !(2..=4).contains(&info.power)
                || start_position
                    .iter()
                    .chain([smoothing_angle].iter())
                    .any(|value| !value.is_finite())
                || info.corner_neighbors.iter().any(|corner| {
                    corner.neighbor_count > 4
                        || corner.neighbors[..usize::from(corner.neighbor_count)]
                            .iter()
                            .any(|neighbor| {
                                *neighbor != u16::MAX
                                    && usize::from(*neighbor) >= displacement_info.len()
                            })
                })
                || info.edge_neighbors.iter().any(|edge| {
                    edge.sub_neighbors.iter().any(|neighbor| {
                        neighbor.neighbor != u16::MAX
                            && (usize::from(neighbor.neighbor) >= displacement_info.len()
                                || neighbor.orientation > 3
                                || neighbor.span > 2
                                || neighbor.neighbor_span > 2)
                    })
                })
            {
                return Err(error(ErrorCode::InvalidRange, Some(source)));
            }
            let side = (1_usize << info.power) + 1;
            let vertex_count = side * side;
            let triangle_count = (side - 1) * (side - 1) * 2;
            let vertex_start = usize::try_from(info.vertex_start)
                .map_err(|_| error(ErrorCode::InvalidRange, Some(source)))?;
            let triangle_start = usize::try_from(info.triangle_start)
                .map_err(|_| error(ErrorCode::InvalidRange, Some(source)))?;
            let vertex_end = vertex_start
                .checked_add(vertex_count)
                .ok_or_else(|| error(ErrorCode::Limit, Some(source)))?;
            let triangle_end = triangle_start
                .checked_add(triangle_count)
                .ok_or_else(|| error(ErrorCode::Limit, Some(source)))?;
            let vertices = displacement_vertices
                .get(vertex_start..vertex_end)
                .ok_or_else(|| error(ErrorCode::InvalidRange, Some(source)))?
                .iter()
                .map(|vertex| {
                    let direction = [
                        vertex.vector.x.value(),
                        vertex.vector.y.value(),
                        vertex.vector.z.value(),
                    ];
                    let distance = vertex.distance.value();
                    let alpha = vertex.alpha.value();
                    if direction
                        .iter()
                        .chain([distance, alpha].iter())
                        .any(|value| !value.is_finite())
                    {
                        return Err(error(ErrorCode::NonFinite, Some(source)));
                    }
                    Ok((direction, distance, alpha))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let triangle_tags = displacement_triangles
                .get(triangle_start..triangle_end)
                .ok_or_else(|| error(ErrorCode::InvalidRange, Some(source)))?
                .to_vec();
            Ok(DisplacementPatch {
                source,
                map_face: info.map_face,
                power: info.power as u8,
                contents: info.contents as u32,
                vertex_start: vertex_start as u32,
                triangle_start: triangle_start as u32,
                start_position,
                minimum_tessellation: info.minimum_tessellation,
                smoothing_angle,
                allowed_vertices: info.allowed_vertices,
                edge_neighbors: info.edge_neighbors,
                corner_neighbors: info.corner_neighbors,
                vertices,
                triangle_tags,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut world = World {
        identity: [0; 32],
        planes,
        sides,
        brushes: output,
        leaves,
        leaf_brushes,
        nodes,
        models,
        world_brushes,
        model_brushes,
        model_contents,
        texture_flags,
        displacements,
        displacement_inputs: Vec::new(),
        displacement_trees: displacement::Acceleration::default(),
    };
    world.identity = world_identity(&world);
    Ok(world)
}
#[derive(Default)]
struct BrushTraceScratch {
    pending: Vec<(i32, [f32; 3], [f32; 3], usize)>,
    ordered: Vec<usize>,
}

impl World {
    pub fn with_displacement_inputs(
        mut self,
        inputs: Vec<DisplacementInput>,
    ) -> Result<Self, Error> {
        if !self.displacement_inputs.is_empty() || inputs.len() != self.displacements.len() {
            return Err(error(ErrorCode::InvalidReference, None));
        }
        for (index, (input, source)) in inputs.iter().zip(&self.displacements).enumerate() {
            if input.source != source.source
                || input.parent_face != usize::from(source.map_face)
                || input.contents != source.contents
                || input.positions.len() != (1usize << source.power).saturating_add(1).pow(2)
                || input.triangles.len() != source.triangle_tags.len()
                || input.triangle_tags != source.triangle_tags
                || input.use_secondary_surface.len() != input.triangles.len()
                || input
                    .positions
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || input
                    .triangles
                    .iter()
                    .flatten()
                    .any(|vertex| *vertex as usize >= input.positions.len())
                || input.secondary_surface.is_none()
                    && input.use_secondary_surface.iter().any(|selected| *selected)
            {
                return Err(error(ErrorCode::InvalidReference, Some(index)));
            }
        }
        self.displacement_inputs = inputs;
        self.displacement_trees = displacement::build(&self)?;
        self.identity = world_identity(&self);
        Ok(self)
    }

    pub fn displacement_input_count(&self) -> usize {
        self.displacement_inputs.len()
    }
}
fn plane(p: &BspPlane, i: usize) -> Result<Plane, Error> {
    let normal = [p.normal.x.value(), p.normal.y.value(), p.normal.z.value()];
    let distance = p.distance.value();
    if normal.iter().any(|v| !v.is_finite()) || !distance.is_finite() {
        return Err(error(ErrorCode::NonFinite, Some(i)));
    }
    Ok(Plane {
        normal,
        distance,
        kind: p.kind,
    })
}
fn side(v: &BspSide) -> Side {
    Side {
        plane: v.plane_index as usize,
        texture_info: v.texture_info_index,
        displacement: v.displacement_info_index,
        bevel: v.bevel,
    }
}
fn brush(v: &BspBrush, i: usize, total: usize) -> Result<Brush, Error> {
    let first =
        usize::try_from(v.first_side).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    let count =
        usize::try_from(v.side_count).map_err(|_| error(ErrorCode::InvalidRange, Some(i)))?;
    if first.checked_add(count).is_none_or(|x| x > total) {
        return Err(error(ErrorCode::InvalidRange, Some(i)));
    }
    Ok(Brush {
        first_side: first,
        side_count: count,
        contents: v.contents as u32,
    })
}

impl World {
    pub fn empty() -> Self {
        let mut world = Self {
            identity: [0; 32],
            planes: Vec::new(),
            sides: Vec::new(),
            brushes: Vec::new(),
            leaves: Vec::new(),
            leaf_brushes: Vec::new(),
            nodes: Vec::new(),
            models: Vec::new(),
            world_brushes: Vec::new(),
            model_brushes: Vec::new(),
            model_contents: Vec::new(),
            texture_flags: Vec::new(),
            displacements: Vec::new(),
            displacement_inputs: Vec::new(),
            displacement_trees: displacement::Acceleration::default(),
        };
        world.identity = world_identity(&world);
        world
    }

    pub fn trace_hull(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
    ) -> Result<Trace, Error> {
        self.trace_hull_with_scratch(start, end, hull, mask, &mut BrushTraceScratch::default())
    }

    fn trace_hull_with_scratch(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
        scratch: &mut BrushTraceScratch,
    ) -> Result<Trace, Error> {
        let mut trace = if let Some(model) = self.models.first() {
            self.order_brushes(model.head_node, start, end, hull, scratch)?;
            self.trace_brushes(start, end, hull, mask, &scratch.ordered, BrushOwner::World)?
        } else {
            self.trace_brushes(start, end, hull, mask, &[], BrushOwner::World)?
        };
        displacement::trace(self, start, end, hull, mask, &mut trace)?;
        Ok(trace)
    }

    pub fn overlaps_model_hull(
        &self,
        model: usize,
        origin: [f32; 3],
        position: [f32; 3],
        hull: Hull,
    ) -> Result<bool, Error> {
        let model_record = self
            .models
            .get(model)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(model)))?;
        if origin.iter().any(|value| !value.is_finite()) {
            return Err(error(ErrorCode::InvalidHull, Some(model)));
        }
        let local = [
            position[0] - origin[0],
            position[1] - origin[1],
            position[2] - origin[2],
        ];
        self.trace_headnode(
            model_record.head_node,
            local,
            local,
            hull,
            u32::MAX,
            BrushOwner::Model {
                identity: 0,
                role: ObjectRole::Entity,
                model,
            },
        )
        .map(|trace| trace.start_solid)
    }

    pub fn overlaps_transformed_model_hull(&self, model:usize, transform:Transform, position:[f32;3], hull:Hull) -> Result<bool,Error> {
        self.trace_model_hull(model,ObjectTraceRequest {identity:0,transform,start:position,end:position,hull,mask:u32::MAX},0,ObjectRole::Entity).map(|trace|trace.start_solid)
    }

    pub fn trace_brush_model(&self, model: usize, request: ObjectTraceRequest) -> Result<Trace, Error> {
        self.trace_model_hull(model, request, request.identity, ObjectRole::Entity)
    }

    pub(crate) fn trace_model_hull(
        &self,
        model: usize,
        request: ObjectTraceRequest,
        identity: u64,
        role: ObjectRole,
    ) -> Result<Trace, Error> {
        let model_record = self
            .models
            .get(model)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(model)))?;
        let basis = request.transform.basis()?;
        let local_start = basis.inverse_point(request.start);
        let local_end = add(
            local_start,
            basis.inverse_vector(sub(request.end, request.start)),
        );
        let mut trace = self.trace_headnode(
            model_record.head_node,
            local_start,
            local_end,
            request.hull,
            request.mask,
            BrushOwner::Model {
                identity,
                role,
                model,
            },
        )?;
        if let Some(plane) = trace.plane.as_mut() {
            plane.normal = basis.vector(plane.normal);
        }
        trace.end = interpolate(request.start, request.end, trace.fraction);
        Ok(trace)
    }

    fn trace_headnode(
        &self,
        head_node: i32,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
        owner: BrushOwner,
    ) -> Result<Trace, Error> {
        let mut scratch = BrushTraceScratch::default();
        self.order_brushes(head_node, start, end, hull, &mut scratch)?;
        self.trace_brushes(start, end, hull, mask, &scratch.ordered, owner)
    }

    #[cfg(test)]
    fn ordered_brushes(
        &self,
        head_node: i32,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
    ) -> Result<Vec<usize>, Error> {
        let mut scratch = BrushTraceScratch::default();
        self.order_brushes(head_node, start, end, hull, &mut scratch)?;
        Ok(scratch.ordered)
    }

    fn order_brushes(
        &self,
        head_node: i32,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        scratch: &mut BrushTraceScratch,
    ) -> Result<(), Error> {
        // Storage only: every query still traverses the current BSP and builds
        // fresh first-visit membership in the same near/far and leaf order.
        scratch.pending.clear();
        scratch.ordered.clear();
        if start
            .into_iter()
            .chain(end)
            .chain(hull.mins)
            .chain(hull.maxs)
            .any(|value| !value.is_finite())
            || hull
                .mins
                .into_iter()
                .zip(hull.maxs)
                .any(|(minimum, maximum)| minimum > maximum)
        {
            return Err(error(ErrorCode::InvalidHull, None));
        }
        let center = scale(add(hull.mins, hull.maxs), 0.5);
        let extents = scale(sub(hull.maxs, hull.mins), 0.5);
        let point = dot(extents, extents) < 1.0e-6;
        // The root is already the current traversal, not a deferred branch.
        // Allocate pending storage only when a split actually needs it.
        let mut current = Some((head_node, add(start, center), add(end, center), 0_usize));
        let pending = &mut scratch.pending;
        let mut seen = BrushVisits::new();
        let ordered = &mut scratch.ordered;
        while let Some((mut child, first, mut second, depth)) = current {
            let mut traversal_depth = depth;
            while child >= 0 {
                traversal_depth += 1;
                if traversal_depth > self.nodes.len() {
                    return Err(error(ErrorCode::InvalidReference, None));
                }
                let node = self
                    .nodes
                    .get(child as usize)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
                let plane = self
                    .planes
                    .get(node.plane_index as usize)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(child as usize)))?;
                let first_distance = dot(first, plane.normal) - plane.distance;
                let second_distance = dot(second, plane.normal) - plane.distance;
                let offset = if point {
                    0.0
                } else if (0..3).contains(&plane.kind) {
                    extents[plane.kind as usize]
                } else {
                    dot_abs(extents, plane.normal)
                };
                if first_distance > offset && second_distance > offset {
                    child = node.children[0];
                    continue;
                }
                if first_distance < -offset && second_distance < -offset {
                    child = node.children[1];
                    continue;
                }
                let (side, near_fraction, far_fraction) = if first_distance < second_distance {
                    let reciprocal = 1.0 / (first_distance - second_distance);
                    (
                        1,
                        ((first_distance - offset - 0.03125) * reciprocal).clamp(0.0, 1.0),
                        ((first_distance + offset + 0.03125) * reciprocal).clamp(0.0, 1.0),
                    )
                } else if first_distance > second_distance {
                    let reciprocal = 1.0 / (first_distance - second_distance);
                    (
                        0,
                        ((first_distance + offset + 0.03125) * reciprocal).clamp(0.0, 1.0),
                        ((first_distance - offset - 0.03125) * reciprocal).clamp(0.0, 1.0),
                    )
                } else {
                    (0, 1.0, 0.0)
                };
                let near_end = interpolate(first, second, near_fraction);
                let far_start = interpolate(first, second, far_fraction);
                if pending.capacity() == 0 {
                    // Preserve the one-entry initial capacity and subsequent
                    // growth of the old root-initialized stack.
                    pending.reserve_exact(1);
                }
                pending.push((node.children[side ^ 1], far_start, second, traversal_depth));
                child = node.children[side];
                second = near_end;
            }
            let leaf_index = (-1_i64 - i64::from(child)) as usize;
            let leaf = self
                .leaves
                .get(leaf_index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?;
            let begin = leaf.first_leaf_brush as usize;
            let finish = begin + leaf.leaf_brush_count as usize;
            for brush in self
                .leaf_brushes
                .get(begin..finish)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?
            {
                if seen.insert(*brush) {
                    ordered.push(*brush as usize);
                }
            }
            current = pending.pop();
        }
        Ok(())
    }

    fn trace_brushes(
        &self,
        start: [f32; 3],
        end: [f32; 3],
        hull: Hull,
        mask: u32,
        brushes: &[usize],
        owner: BrushOwner,
    ) -> Result<Trace, Error> {
        if start
            .iter()
            .chain(end.iter())
            .chain(hull.mins.iter())
            .chain(hull.maxs.iter())
            .any(|v| !v.is_finite())
            || hull.mins.iter().zip(hull.maxs).any(|(a, b)| *a > b)
        {
            return Err(error(ErrorCode::InvalidHull, None));
        }
        let center = [
            (hull.mins[0] + hull.maxs[0]) * 0.5,
            (hull.mins[1] + hull.maxs[1]) * 0.5,
            (hull.mins[2] + hull.maxs[2]) * 0.5,
        ];
        let extents = [
            (hull.maxs[0] - hull.mins[0]) * 0.5,
            (hull.maxs[1] - hull.mins[1]) * 0.5,
            (hull.maxs[2] - hull.mins[2]) * 0.5,
        ];
        let trace_start = [
            start[0] + center[0],
            start[1] + center[1],
            start[2] + center[2],
        ];
        let trace_end = [end[0] + center[0], end[1] + center[1], end[2] + center[2]];
        let point = dot(extents, extents) < 1.0e-6;
        let mut result = Trace {
            world: self.identity,
            fraction: 1.0,
            fraction_left_solid: 0.0,
            start_solid: false,
            all_solid: false,
            brush: None,
            contents: 0,
            plane: None,
            surface_flags: 0,
            displacement_flags: 0,
            surface: None,
            displacement: None,
            hit: None,
            snapshot: None,
            end,
        };
        for &index in brushes {
            let brush = &self.brushes[index];
            if brush.contents & mask == 0 {
                continue;
            }
            let mut enter = -1.0_f32;
            let mut leave = 1.0_f32;
            let mut starts_out = false;
            let mut gets_out = false;
            let mut clip = None;
            let mut rejected = false;
            for side in &self.sides[brush.first_side..brush.first_side + brush.side_count] {
                if point && side.bevel != 0 {
                    continue;
                }
                let p = self.planes[side.plane];
                let offset = if point {
                    0.0
                } else {
                    extents[0] * p.normal[0].abs()
                        + extents[1] * p.normal[1].abs()
                        + extents[2] * p.normal[2].abs()
                };
                let d1 = dot(trace_start, p.normal) - p.distance - offset;
                let d2 = dot(trace_end, p.normal) - p.distance - offset;
                if d1 > 0.0 {
                    starts_out = true;
                    if d2 > 0.0 {
                        rejected = true;
                        break;
                    }
                } else {
                    if d2 <= 0.0 {
                        continue;
                    }
                    gets_out = true;
                }
                if d1 > d2 {
                    let f = (d1 - 0.03125).max(0.0) / (d1 - d2);
                    if f > enter {
                        enter = f;
                        clip = Some((p, *side));
                    }
                } else {
                    leave = leave.min((d1 + 0.03125) / (d1 - d2));
                }
            }
            if rejected {
                continue;
            }
            if point && starts_out && result.fraction_left_solid - enter > 0.0 {
                starts_out = false;
            }
            if !starts_out {
                result.start_solid = true;
                result.contents = brush.contents;
                result.brush = Some(index);
                result.hit = Some(owner.hit(index));
                if !gets_out {
                    result.all_solid = true;
                    result.fraction = 0.0;
                    result.fraction_left_solid = 1.0;
                    result.plane = None;
                    result.surface_flags = 0;
                    break;
                } else if point && leave != 1.0 && leave > result.fraction_left_solid {
                    result.fraction_left_solid = leave;
                    if result.fraction <= leave {
                        result.fraction = 1.0;
                        result.plane = None;
                        result.surface_flags = 0;
                    }
                }
                continue;
            }
            if enter < leave && enter > -1.0 && enter < result.fraction {
                result.fraction = enter.max(0.0);
                result.brush = Some(index);
                result.contents = brush.contents;
                let (plane, side) = clip.expect("an entering brush plane is selected");
                result.plane = Some(plane);
                result.surface_flags = self.surface_flags(side);
                result.hit = Some(owner.hit(index));
            }
        }
        result.end = interpolate(start, end, result.fraction);
        Ok(result)
    }

    fn surface_flags(&self, side: Side) -> u16 {
        usize::try_from(side.texture_info)
            .ok()
            .and_then(|index| self.texture_flags.get(index))
            .copied()
            .unwrap_or(0)
    }
}
pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn scale(a: [f32; 3], value: f32) -> [f32; 3] {
    [a[0] * value, a[1] * value, a[2] * value]
}
pub(crate) fn interpolate(start: [f32; 3], end: [f32; 3], fraction: f32) -> [f32; 3] {
    add(start, scale(sub(end, start), fraction))
}
fn dot_abs(extents: [f32; 3], normal: [f32; 3]) -> f32 {
    extents[0] * normal[0].abs() + extents[1] * normal[1].abs() + extents[2] * normal[2].abs()
}
fn error(code: ErrorCode, item: Option<usize>) -> Error {
    Error {
        code,
        item,
        range: None,
    }
}
fn brushes_for_model(
    head: i32,
    nodes: &[Node],
    leaves: &[Leaf],
    leaf_brushes: &[u16],
    brush_count: usize,
) -> Result<Vec<usize>, Error> {
    let mut pending = vec![head];
    let mut seen_nodes = std::collections::BTreeSet::new();
    let mut seen_leaves = std::collections::BTreeSet::new();
    let mut brushes = std::collections::BTreeSet::new();
    while let Some(child) = pending.pop() {
        if child >= 0 {
            let index = child as usize;
            let node = nodes
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            if seen_nodes.insert(index) {
                pending.extend(node.children);
            }
        } else {
            let index = (-1_i64 - child as i64) as usize;
            if !seen_leaves.insert(index) {
                continue;
            }
            let leaf = leaves
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            let start = leaf.first_leaf_brush as usize;
            let end = start + leaf.leaf_brush_count as usize;
            for &brush in leaf_brushes
                .get(start..end)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?
            {
                if brush as usize >= brush_count {
                    return Err(error(ErrorCode::InvalidReference, Some(index)));
                }
                brushes.insert(brush as usize);
            }
        }
    }
    Ok(brushes.into_iter().collect())
}

fn world_identity(world: &World) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(if world.displacements.is_empty() {
        b"playsrc-collision-world-v2".as_slice()
    } else {
        b"playsrc-collision-world-v3".as_slice()
    });
    for plane in &world.planes {
        for value in plane.normal.into_iter().chain([plane.distance]) {
            digest.update(value.to_bits().to_le_bytes());
        }
        digest.update(plane.kind.to_le_bytes());
    }
    for side in &world.sides {
        digest.update((side.plane as u64).to_le_bytes());
        digest.update(side.texture_info.to_le_bytes());
        digest.update(side.displacement.to_le_bytes());
        digest.update(side.bevel.to_le_bytes());
    }
    for brush in &world.brushes {
        digest.update((brush.first_side as u64).to_le_bytes());
        digest.update((brush.side_count as u64).to_le_bytes());
        digest.update(brush.contents.to_le_bytes());
    }
    for leaf in &world.leaves {
        digest.update(leaf.contents.to_le_bytes());
        digest.update(leaf.cluster.to_le_bytes());
        digest.update(leaf.area_and_flags.to_le_bytes());
        for value in leaf.mins.into_iter().chain(leaf.maxs) {
            digest.update(value.to_le_bytes());
        }
        digest.update(leaf.first_leaf_face.to_le_bytes());
        digest.update(leaf.leaf_face_count.to_le_bytes());
        digest.update(leaf.first_leaf_brush.to_le_bytes());
        digest.update(leaf.leaf_brush_count.to_le_bytes());
        digest.update(leaf.leaf_water_data_id.to_le_bytes());
    }
    for brush in &world.leaf_brushes {
        digest.update(brush.to_le_bytes());
    }
    for node in &world.nodes {
        digest.update(node.plane_index.to_le_bytes());
        for child in node.children {
            digest.update(child.to_le_bytes());
        }
        for value in node.mins.into_iter().chain(node.maxs) {
            digest.update(value.to_le_bytes());
        }
        digest.update(node.first_face.to_le_bytes());
        digest.update(node.face_count.to_le_bytes());
        digest.update(node.area.to_le_bytes());
    }
    for model in &world.models {
        for bits in [model.mins, model.maxs, model.origin]
            .into_iter()
            .flat_map(|vector| [vector.x.0, vector.y.0, vector.z.0])
        {
            digest.update(bits.to_le_bytes());
        }
        digest.update(model.head_node.to_le_bytes());
        digest.update(model.first_face.to_le_bytes());
        digest.update(model.face_count.to_le_bytes());
    }
    for (brushes, contents) in world.model_brushes.iter().zip(&world.model_contents) {
        digest.update((brushes.len() as u64).to_le_bytes());
        for brush in brushes {
            digest.update((*brush as u64).to_le_bytes());
        }
        digest.update(contents.to_le_bytes());
    }
    for displacement in &world.displacements {
        digest.update((displacement.source as u64).to_le_bytes());
        digest.update(displacement.map_face.to_le_bytes());
        digest.update([displacement.power]);
        digest.update(displacement.contents.to_le_bytes());
        digest.update(displacement.vertex_start.to_le_bytes());
        digest.update(displacement.triangle_start.to_le_bytes());
        for value in displacement.start_position {
            digest.update(value.to_bits().to_le_bytes());
        }
        digest.update(displacement.minimum_tessellation.to_le_bytes());
        digest.update(displacement.smoothing_angle.to_bits().to_le_bytes());
        for value in displacement.allowed_vertices {
            digest.update(value.to_le_bytes());
        }
        for edge in displacement.edge_neighbors {
            for neighbor in edge.sub_neighbors {
                digest.update(neighbor.neighbor.to_le_bytes());
                digest.update([
                    neighbor.orientation,
                    neighbor.span,
                    neighbor.neighbor_span,
                    neighbor.padding,
                ]);
            }
        }
        for corner in displacement.corner_neighbors {
            for neighbor in corner.neighbors {
                digest.update(neighbor.to_le_bytes());
            }
            digest.update([corner.neighbor_count, corner.padding]);
        }
        for (direction, distance, alpha) in &displacement.vertices {
            for value in direction.iter().chain([distance, alpha]) {
                digest.update(value.to_bits().to_le_bytes());
            }
        }
        for tag in &displacement.triangle_tags {
            digest.update(tag.to_le_bytes());
        }
    }
    for input in &world.displacement_inputs {
        digest.update((input.source as u64).to_le_bytes());
        digest.update((input.parent_face as u64).to_le_bytes());
        digest.update(input.contents.to_le_bytes());
        for position in &input.positions {
            for value in position {
                digest.update(value.to_bits().to_le_bytes());
            }
        }
        for triangle in &input.triangles {
            for index in triangle {
                digest.update(index.to_le_bytes());
            }
        }
        for tag in &input.triangle_tags {
            digest.update(tag.to_le_bytes());
        }
        digest.update(input.primary_surface.registry);
        digest.update(input.primary_surface.index.to_le_bytes());
        match input.secondary_surface {
            Some(surface) => {
                digest.update([1]);
                digest.update(surface.registry);
                digest.update(surface.index.to_le_bytes());
            }
            None => digest.update([0]),
        }
        for selected in &input.use_secondary_surface {
            digest.update([u8::from(*selected)]);
        }
    }
    for flags in &world.texture_flags {
        digest.update(flags.to_le_bytes());
    }
    digest.finalize().into()
}

#[derive(Clone, Copy)]
enum BrushOwner {
    World,
    Model {
        identity: u64,
        role: ObjectRole,
        model: usize,
    },
}
impl BrushOwner {
    fn hit(self, brush: usize) -> Hit {
        match self {
            Self::World => Hit::WorldBrush { brush },
            Self::Model {
                identity,
                role,
                model,
            } => Hit::Object {
                identity,
                role,
                feature: Feature::Brush { model, brush },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_bsp::{Limits, Profile, parse};
    fn set(b: &mut [u8], i: usize, o: usize, l: usize, v: i32) {
        let h = 8 + i * 16;
        b[h..h + 4].copy_from_slice(&(o as i32).to_le_bytes());
        b[h + 4..h + 8].copy_from_slice(&(l as i32).to_le_bytes());
        b[h + 8..h + 12].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Bsp {
        let mut b = vec![0; 1036];
        b[..4].copy_from_slice(b"VBSP");
        b[4..8].copy_from_slice(&20_i32.to_le_bytes());
        let mut add = |i: usize, v: i32, x: &[u8]| {
            let o = b.len();
            b.extend_from_slice(x);
            set(&mut b, i, o, x.len(), v);
        };
        let mut p = Vec::new();
        for (n, d) in [
            ([1_f32, 0., 0.], 16_f32),
            ([-1., 0., 0.], 16.),
            ([0., 1., 0.], 16.),
            ([0., -1., 0.], 16.),
            ([0., 0., 1.], 16.),
            ([0., 0., -1.], 16.),
        ] {
            for x in n {
                p.extend_from_slice(&x.to_le_bytes())
            }
            p.extend_from_slice(&d.to_le_bytes());
            p.extend_from_slice(&0_i32.to_le_bytes());
        }
        add(1, 0, &p);
        let mut texture_info = [0_u8; 72];
        texture_info[64..68].copy_from_slice(&(SURF_SKY as i32).to_le_bytes());
        add(6, 0, &texture_info);
        let mut sides = Vec::new();
        for i in 0..6_u16 {
            sides.extend_from_slice(&i.to_le_bytes());
            sides.extend_from_slice(&0_i16.to_le_bytes());
            sides.extend_from_slice(&(-1_i16).to_le_bytes());
            sides.extend_from_slice(&0_i16.to_le_bytes());
        }
        add(19, 0, &sides);
        let mut brush = 0_i32.to_le_bytes().to_vec();
        brush.extend_from_slice(&6_i32.to_le_bytes());
        brush.extend_from_slice(&1_i32.to_le_bytes());
        brush.extend_from_slice(&0_i32.to_le_bytes());
        brush.extend_from_slice(&6_i32.to_le_bytes());
        brush.extend_from_slice(&1_i32.to_le_bytes());
        add(18, 0, &brush);
        let mut leaf = vec![0; 32];
        leaf[24..26].copy_from_slice(&0_u16.to_le_bytes());
        leaf[26..28].copy_from_slice(&1_u16.to_le_bytes());
        add(10, 1, &leaf);
        add(17, 0, &0_u16.to_le_bytes());
        add(5, 0, &[0; 32]);
        let mut model = [0; 48];
        model[36..40].copy_from_slice(&(-1_i32).to_le_bytes());
        add(14, 0, &model);
        parse(&b, Profile::Source2013V20, Limits::default()).unwrap()
    }
    #[test]
    fn traces_point_and_hull_against_brush() {
        let w = compile(&fixture()).unwrap();
        assert_eq!(w.brushes.len(), 2);
        assert_eq!(w.world_brushes, [0]);
        assert_eq!(w.model_brushes, [vec![0]]);
        let point = Hull {
            mins: [0.; 3],
            maxs: [0.; 3],
        };
        let t = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], point, 1)
            .unwrap();
        assert_eq!(t.brush, Some(0));
        assert!(t.fraction > 0.24 && t.fraction < 0.26);
        assert_eq!(t.plane.unwrap().normal, [-1., 0., 0.]);
        assert!(t.is_sky());
        assert_eq!(t.hit, Some(Hit::WorldBrush { brush: 0 }));
        let hull = Hull {
            mins: [-2.; 3],
            maxs: [2.; 3],
        };
        let h = w
            .trace_hull([-32., 0., 0.], [32., 0., 0.], hull, 1)
            .unwrap();
        assert!(h.fraction < t.fraction);
        let inside = w.trace_hull([0.; 3], [1., 0., 0.], point, 1).unwrap();
        assert!(inside.start_solid && inside.all_solid);
        assert!(
            w.overlaps_model_hull(0, [100., 0., 0.], [100., 0., 0.], point)
                .unwrap()
        );
        assert!(
            !w.overlaps_model_hull(0, [100., 0., 0.], [0., 0., 0.], point)
                .unwrap()
        );
    }

    #[test]
    fn leaf_brush_membership_preserves_encounter_order_and_equal_hit_winner() {
        let mut world = compile(&fixture()).unwrap();
        let point = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        world.leaf_brushes = vec![65535, 64, 63, 0, 511, 512, 65535, 63];
        world.leaves[0].leaf_brush_count = world.leaf_brushes.len() as u16;
        assert_eq!(world.ordered_brushes(-1, [32.0, 0.0, 0.0], [-32.0, 0.0, 0.0], point).unwrap(),
            [65535, 64, 63, 0, 511, 512]);
        for order in [vec![1, 0, 1], vec![0, 1, 0]] {
            world.leaf_brushes = order;
            world.leaves[0].leaf_brush_count = 3;
            let trace = world.trace_hull([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], point, 1).unwrap();
            assert_eq!(trace.brush, Some(usize::from(world.leaf_brushes[0])));
        }
    }

    #[test]
    fn brush_visitation_keeps_traversal_before_clip_error_precedence() {
        let mut world = compile(&fixture()).unwrap();
        let point = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        world.models[0].head_node = 0;
        world.nodes[0].children = [-1, -2];
        world.leaf_brushes[0] = u16::MAX;
        let query = |world: &World, hull| world.trace_hull([32.0, 0.0, 0.0], [-32.0, 0.0, 0.0], hull, 1);
        assert_eq!(query(&world, point), Err(error(ErrorCode::InvalidReference, Some(1))));
        assert_eq!(query(&world, Hull { mins: [f32::NAN; 3], maxs: [0.0; 3] }), Err(error(ErrorCode::InvalidHull, None)));
        world.leaves.push(world.leaves[0].clone());
        world.leaf_brushes[0] = 0;
        assert!(query(&world, point).is_ok());
    }

    #[test]
    fn traversal_pending_branches_keep_deep_order_and_depth_errors() {
        let mut world = compile(&fixture()).unwrap();
        let count = 1024;
        world.models[0].head_node = 0;
        let node = world.nodes[0].clone();
        world.nodes = vec![node; count];
        world.planes[world.nodes[0].plane_index as usize].distance = 0.0;
        for (index, node) in world.nodes.iter_mut().enumerate() {
            node.children = [if index + 1 == count { -(count as i32) - 1 } else { index as i32 + 1 }, -(index as i32) - 1];
        }
        world.leaves = vec![world.leaves[0].clone(); count + 1];
        for (index, leaf) in world.leaves.iter_mut().enumerate() {
            leaf.first_leaf_brush = index as u16;
            leaf.leaf_brush_count = 1;
        }
        world.leaf_brushes = (0..=count as u16).collect();
        let hull = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let expected: Vec<_> = (0..=count).rev().collect();
        for _ in 0..4 {
            assert_eq!(world.ordered_brushes(0, [0.0; 3], [0.0; 3], hull).unwrap(), expected);
        }
        // A cycle must fail at the existing map-derived depth, not a scratch cap.
        world.nodes[count - 1].children[0] = 0;
        assert_eq!(world.ordered_brushes(0, [0.0; 3], [0.0; 3], hull), Err(error(ErrorCode::InvalidReference, None)));
        world.nodes[count - 1].children[0] = -(count as i32) - 1;
        assert_eq!(world.ordered_brushes(0, [0.0; 3], [0.0; 3], hull).unwrap(), expected);
        // Far branches remain LIFO even when one contains an invalid leaf.
        world.nodes[0].children[1] = i32::MIN;
        assert_eq!(world.ordered_brushes(0, [0.0; 3], [0.0; 3], hull), Err(error(ErrorCode::InvalidReference, Some(i32::MAX as usize))));
    }

    #[test]
    fn reused_brush_storage_retraverses_after_changes_and_errors() {
        let mut world = compile(&fixture()).unwrap();
        let mut scratch = BrushTraceScratch::default();
        let point = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let start = [32.0, 0.0, 0.0];
        let end = [-32.0, 0.0, 0.0];
        world.models[0].head_node = 0;
        world.nodes[0].children = [-1, -2];
        world.leaves.push(world.leaves[0].clone());
        world.leaves[1].first_leaf_brush = 1;
        world.leaf_brushes = vec![1, 0];
        let reference = world.trace_hull(start, end, point, 1).unwrap();
        assert_eq!(world.trace_hull_with_scratch(start, end, point, 1, &mut scratch).unwrap(), reference);
        assert_eq!(scratch.ordered, [1, 0]);
        let storage = (scratch.ordered.as_ptr(), scratch.ordered.capacity(), scratch.pending.as_ptr(), scratch.pending.capacity());
        for _ in 0..32 {
            assert_eq!(world.trace_hull_with_scratch(start, end, point, 1, &mut scratch).unwrap(), reference);
            assert_eq!((scratch.ordered.as_ptr(), scratch.ordered.capacity(), scratch.pending.as_ptr(), scratch.pending.capacity()), storage);
        }
        world.leaf_brushes.swap(0, 1);
        assert_eq!(world.trace_hull_with_scratch(start, end, point, 1, &mut scratch).unwrap().brush, Some(0));
        assert_eq!(scratch.ordered, [0, 1]);
        world.nodes[0].children[1] = i32::MIN;
        assert_eq!(world.trace_hull_with_scratch(start, end, point, 1, &mut scratch), Err(error(ErrorCode::InvalidReference, Some(i32::MAX as usize))));
        world.nodes[0].children[1] = -2;
        assert_eq!(world.trace_hull_with_scratch(start, end, point, 1, &mut scratch).unwrap(), world.trace_hull(start, end, point, 1).unwrap());
        world.leaves.iter_mut().for_each(|leaf| leaf.leaf_brush_count = 0);
        let empty = world.trace_hull_with_scratch(start, end, point, 1, &mut scratch).unwrap();
        assert_eq!(empty.fraction, 1.0);
        assert!(scratch.ordered.is_empty());
    }

    #[test]
    fn snapshot_world_queries_reuse_bsp_storage_without_reusing_answers() {
        let world = compile(&fixture()).unwrap();
        let snapshot = Snapshot::compile(&world, 1, vec![], SnapshotLimits::default()).unwrap();
        let mut scratch = QueryScratch::default();
        let point = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        assert_eq!(scratch.storage_bytes(), 0);
        for (start, end, mask) in [([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], 1), ([32.0, 0.0, 0.0], [-32.0, 0.0, 0.0], 1), ([0.0; 3], [1.0; 3], 1), ([0.0; 3], [1.0; 3], 0)] {
            let request = SnapshotTraceRequest { start, end, mask, hull: point, scope: TraceScope::WorldOnly, ignored: &[] };
            let expected = world.trace_snapshot_hull(&snapshot, request, |_| true).unwrap();
            assert_eq!(world.trace_snapshot_hull_with_scratch(&snapshot, request, &mut scratch, |_| true).unwrap(), expected);
            let retained = scratch.storage_bytes();
            assert!(retained > 0);
            for _ in 0..32 {
                assert_eq!(world.trace_snapshot_hull_with_scratch(&snapshot, request, &mut scratch, |_| true).unwrap(), expected);
                assert_eq!(scratch.storage_bytes(), retained);
            }
        }
    }

    #[test]
    fn traversal_queries_are_independent_across_nested_and_concurrent_readers() {
        let world = compile(&fixture()).unwrap();
        let hull = Hull { mins: [-2.0; 3], maxs: [2.0; 3] };
        let start = [-32.0, 0.0, 0.0];
        let end = [32.0, 0.0, 0.0];
        let expected = world.trace_hull(start, end, hull, 1).unwrap();
        let snapshot = Snapshot::compile(&world, 1, vec![box_object(1, [-1.0; 3], [1.0; 3])], SnapshotLimits::default()).unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let world = &world;
                let snapshot = &snapshot;
                scope.spawn(move || {
                    for _ in 0..256 {
                        assert_eq!(world.trace_hull(start, end, hull, 1).unwrap(), expected);
                        let called = std::cell::Cell::new(false);
                        world.trace_snapshot_hull(snapshot, SnapshotTraceRequest {
                            start, end, hull, mask: 1, scope: TraceScope::EntitiesOnly, ignored: &[],
                        }, |_| {
                            called.set(true);
                            assert_eq!(world.trace_hull(start, end, hull, 1).unwrap(), expected);
                            true
                        }).unwrap();
                        assert!(called.get());
                    }
                });
            }
        });
    }

    fn box_object(identity: u64, minimum: [f32; 3], maximum: [f32; 3]) -> ObjectInput {
        ObjectInput {
            identity,
            role: ObjectRole::Entity,
            enabled: true,
            volume_contents: false,
            transform: Transform::IDENTITY,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 1,
            surface_flags: 0,
            shape: SnapshotShape::BoundingBox {
                bounds: Hull {
                    mins: minimum,
                    maxs: maximum,
                },
            },
        }
    }

    #[test]
    fn point_contents_unions_leaf_brushes_and_includes_exact_boundaries() {
        let mut world = compile(&fixture()).unwrap();
        world.leaves[0].contents = CONTENTS_OPAQUE as i32;
        world.leaves[0].leaf_brush_count = 2;
        world.leaf_brushes = vec![0, 1];
        world.brushes[0].contents = CONTENTS_WATER | CONTENTS_TRANSLUCENT;
        world.brushes[1].contents = CONTENTS_SLIME;

        let inside = world.point_contents([0.0; 3]).unwrap();
        assert_eq!(
            inside.contents,
            CONTENTS_OPAQUE | CONTENTS_WATER | CONTENTS_TRANSLUCENT | CONTENTS_SLIME
        );
        assert_eq!(
            inside.contributors,
            [
                PointContentsContributor::WorldLeaf { leaf: 0 },
                PointContentsContributor::WorldBrush { brush: 0 },
                PointContentsContributor::WorldBrush { brush: 1 },
            ]
        );
        assert_eq!(
            world.point_contents([16.0, 0.0, 0.0]).unwrap().contents,
            inside.contents
        );
        assert_eq!(
            world
                .point_contents([16.000_002, 0.0, 0.0])
                .unwrap()
                .contents,
            CONTENTS_OPAQUE
        );
        world.brushes[1].side_count = 0;
        assert_eq!(
            world.point_contents([0.0; 3]).unwrap().contents,
            CONTENTS_OPAQUE | CONTENTS_WATER | CONTENTS_TRANSLUCENT
        );
        assert_eq!(
            world.point_contents([f32::NAN, 0.0, 0.0]).unwrap_err().code,
            ErrorCode::NonFinite
        );
    }

    #[test]
    fn point_contents_uses_first_eligible_fluid_volume_and_static_prop_world_identity() {
        let mut world = compile(&fixture()).unwrap();
        world.brushes[0].contents = CONTENTS_WATER | CONTENTS_TRANSLUCENT;
        world.brushes[1].contents = CONTENTS_SLIME;
        world.leaf_brushes = vec![0, 0, 1];
        let mut water_leaf = world.leaves[0].clone();
        water_leaf.first_leaf_brush = 1;
        let mut slime_leaf = world.leaves[0].clone();
        slime_leaf.first_leaf_brush = 2;
        world.leaves.extend([water_leaf, slime_leaf]);
        let mut water_model = world.models[0];
        water_model.head_node = -2;
        let mut slime_model = world.models[0];
        slime_model.head_node = -3;
        world.models.extend([water_model, slime_model]);
        world.model_brushes.extend([vec![0], vec![1]]);
        world
            .model_contents
            .extend([CONTENTS_WATER | CONTENTS_TRANSLUCENT, CONTENTS_SLIME]);
        let volume = |identity, model, eligible| ObjectInput {
            identity,
            role: ObjectRole::Entity,
            enabled: true,
            volume_contents: eligible,
            transform: Transform::IDENTITY,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model },
        };
        let snapshot = Snapshot::compile(
            &world,
            42,
            vec![volume(1, 1, false), volume(2, 2, true), volume(3, 1, true)],
            SnapshotLimits::default(),
        )
        .unwrap();
        let result = world.point_contents_snapshot(&snapshot, [0.0; 3]).unwrap();
        assert_eq!(result.contents, CONTENTS_SLIME);
        assert_eq!(result.entity, 2);
        assert_eq!(result.snapshot, Some(42));
        assert_eq!(
            result.contributors,
            [
                PointContentsContributor::WorldLeaf { leaf: 0 },
                PointContentsContributor::WorldBrush { brush: 0 },
                PointContentsContributor::Object {
                    identity: 2,
                    role: ObjectRole::Entity,
                },
            ]
        );
        assert_eq!(
            world
                .point_contents_snapshot_value(&snapshot, [0.0; 3])
                .unwrap(),
            CONTENTS_SLIME
        );
        assert_eq!(
            world
                .point_contents_object(&snapshot, 1, [0.0; 3])
                .unwrap()
                .contents,
            0
        );
        assert_eq!(
            world
                .point_contents_object(&snapshot, 3, [0.0; 3])
                .unwrap()
                .contents,
            CONTENTS_WATER | CONTENTS_TRANSLUCENT
        );

        let static_prop = Snapshot::compile(
            &world,
            43,
            vec![ObjectInput {
                role: ObjectRole::StaticProp,
                ..box_object(7, [-1.0; 3], [1.0; 3])
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let result = world
            .point_contents_snapshot(&static_prop, [0.0; 3])
            .unwrap();
        assert_eq!(result.contents, CONTENTS_SOLID);
        assert_eq!(result.entity, 0);

        world.brushes[0].contents = CONTENTS_WATER | CONTENTS_TRANSLUCENT | 0x0004_0000;
        let current = Snapshot::compile(&world, 44, Vec::new(), SnapshotLimits::default()).unwrap();
        assert_eq!(
            world
                .point_contents_snapshot(&current, [0.0; 3])
                .unwrap()
                .contents,
            CONTENTS_WATER
        );
        world.brushes[0].contents = CONTENTS_SOLID;
        let sealed = Snapshot::compile(
            &world,
            45,
            vec![volume(9, 2, true)],
            SnapshotLimits::default(),
        )
        .unwrap();
        assert_eq!(
            world
                .point_contents_snapshot(&sealed, [0.0; 3])
                .unwrap()
                .contents,
            CONTENTS_SOLID
        );
    }

    #[test]
    fn bounded_snapshot_traces_world_models_and_entities_in_source_order() {
        let world = compile(&fixture()).unwrap();
        let translated_model = ObjectInput {
            identity: 40,
            role: ObjectRole::Entity,
            enabled: true,
            volume_contents: false,
            transform: Transform {
                origin: [100.0, 0.0, 0.0],
                angles: [0.0, 90.0, 0.0],
            },
            linear_velocity: [10.0, 0.0, 0.0],
            angular_velocity: [0.0, 90.0, 0.0],
            collision_group: 3,
            contents: 0,
            surface_flags: 0,
            shape: SnapshotShape::BrushModel { model: 0 },
        };
        let snapshot =
            Snapshot::compile(&world, 9, vec![translated_model], SnapshotLimits::default())
                .unwrap();
        let trace = world
            .trace_snapshot_ray(
                &snapshot,
                SnapshotRayRequest {
                    start: [100.0, -32.0, 0.0],
                    end: [100.0, 32.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(trace.snapshot, Some(9));
        assert_eq!(trace.end[0], 100.0);
        assert!(trace.end[1] < -15.9);
        let normal = trace.plane.unwrap().normal;
        assert!(normal[0].abs() < 0.000001 && (normal[1] + 1.0).abs() < 0.000001);
        assert_eq!(
            trace.hit,
            Some(Hit::Object {
                identity: 40,
                role: ObjectRole::Entity,
                feature: Feature::Brush { model: 0, brush: 0 },
            })
        );

        let empty = World::empty();
        let ordered = Snapshot::compile(
            &empty,
            10,
            vec![
                box_object(8, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
                box_object(7, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let direct = empty
            .trace_snapshot_ray(
                &ordered,
                SnapshotRayRequest {
                    start: [-10.0, 0.0, 0.0],
                    end: [10.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(
            direct.hit,
            Some(Hit::Object {
                identity: 8,
                role: ObjectRole::Entity,
                feature: Feature::Box,
            })
        );
        assert_eq!(direct.plane.unwrap().normal, [-1.0, 0.0, 0.0]);
        assert_eq!(&ordered.snapshot_bytes().unwrap()[..8], b"CSNP\x04\0\0\0");
    }

    #[test]
    fn retained_collision_revision_preserves_exact_order_and_changes_only_identity_bytes() {
        let world = World::empty();
        let original = Snapshot::compile(
            &world,
            9,
            vec![
                box_object(8, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
                box_object(7, [4.0, -1.0, -1.0], [6.0, 1.0, 1.0]),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let retained = original.with_identity(10);
        assert_eq!(original.identity(), 9);
        assert_eq!(retained.identity(), 10);
        assert_eq!(retained.records(), original.records());
        let original_bytes = original.snapshot_bytes().unwrap();
        let retained_bytes = retained.snapshot_bytes().unwrap();
        assert_eq!(&retained_bytes[..40], &original_bytes[..40]);
        assert_eq!(&retained_bytes[40..48], &10_u64.to_le_bytes());
        assert_eq!(&retained_bytes[48..], &original_bytes[48..]);
    }

    #[test]
    fn snapshot_broad_phase_rejects_only_disjoint_records_and_preserves_order() {
        let world = World::empty();
        let snapshot = Snapshot::compile(
            &world,
            11,
            vec![
                box_object(1, [1_000.0, -1.0, -1.0], [1_002.0, 1.0, 1.0]),
                box_object(2, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
                box_object(3, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0]),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let candidates = std::cell::RefCell::new(Vec::new());
        let trace = world
            .trace_snapshot_ray(
                &snapshot,
                SnapshotRayRequest {
                    start: [-10.0, 0.0, 0.0],
                    end: [10.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |candidate| {
                    candidates.borrow_mut().push(candidate.identity);
                    true
                },
            )
            .unwrap();
        assert_eq!(*candidates.borrow(), [2, 3]);
        assert!(matches!(trace.hit, Some(Hit::Object { identity: 2, .. })));
    }

    #[test]
    fn hierarchical_snapshot_broad_phase_keeps_source_order_across_nested_partitions() {
        let world = World::empty();
        let objects = (0_u64..257)
            .map(|identity| {
                if matches!(identity, 17 | 128 | 255) {
                    box_object(identity + 1, [0.0, -1.0, -1.0], [2.0, 1.0, 1.0])
                } else {
                    let position = 1_000.0 + identity as f32 * 8.0;
                    box_object(
                        identity + 1,
                        [position, -1.0, -1.0],
                        [position + 2.0, 1.0, 1.0],
                    )
                }
            })
            .collect();
        let snapshot = Snapshot::compile(&world, 12, objects, SnapshotLimits::default()).unwrap();
        let candidates = std::cell::RefCell::new(Vec::new());
        let trace = world
            .trace_snapshot_hull(
                &snapshot,
                SnapshotTraceRequest {
                    start: [-10.0, 2.0, 0.0],
                    end: [10.0, 2.0, 0.0],
                    hull: Hull {
                        mins: [0.0, -1.0, 0.0],
                        maxs: [0.0; 3],
                    },
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |candidate| {
                    candidates.borrow_mut().push(candidate.identity);
                    true
                },
            )
            .unwrap();
        assert_eq!(*candidates.borrow(), [18, 129, 256]);
        assert!(matches!(trace.hit, Some(Hit::Object { identity: 18, .. })));
    }

    #[test]
    fn physics_query_dispatch_preserves_feature_contents_and_transform() {
        let vertices = vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let shape = PhysicsShape::compile(
            55,
            vec![ConvexInput {
                solid: 2,
                convex: 3,
                contents: 0x4000_0001,
                vertices,
                triangles,
                authored: None,
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        assert!(shape.authored_convex(0).is_none());
        let world = World::empty();
        let snapshot = Snapshot::compile(
            &world,
            12,
            vec![ObjectInput {
                identity: 99,
                role: ObjectRole::StaticProp,
                enabled: true,
                volume_contents: false,
                transform: Transform {
                    origin: [5.0, 0.0, 0.0],
                    angles: [0.0, 45.0, 0.0],
                },
                linear_velocity: [0.0; 3],
                angular_velocity: [0.0; 3],
                collision_group: 0,
                contents: 0,
                surface_flags: 0,
                shape: SnapshotShape::Physics(std::sync::Arc::new(crate::snapshot::FixturePhysicsQuery {
                    geometry: std::sync::Arc::new(shape), calls: Default::default(),
                })),
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let filter_calls = std::cell::Cell::new(0);
        let trace = world
            .trace_snapshot_ray(
                &snapshot,
                SnapshotRayRequest {
                    start: [0.0, 0.0, 0.0],
                    end: [10.0, 0.0, 0.0],
                    mask: u32::MAX,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| {
                    filter_calls.set(filter_calls.get() + 1);
                    false
                },
            )
            .unwrap();
        assert_eq!(
            filter_calls.get(),
            0,
            "ordinary static props bypass entity filters"
        );
        assert!(trace.fraction < 0.5);
        assert_eq!(trace.contents, 0x4000_0001);
        assert!(matches!(
            trace.hit,
            Some(Hit::Object {
                identity: 99,
                role: ObjectRole::StaticProp,
                feature: Feature::Convex {
                    solid: 2,
                    convex: 3,
                    triangle: None,
                },
            })
        ));
    }

    #[test]
    fn snapshot_limits_and_world_equal_hit_precedence_are_categorical() {
        let world = compile(&fixture()).unwrap();
        let coincident = Snapshot::compile(
            &world,
            13,
            vec![box_object(1, [-16.0; 3], [16.0; 3])],
            SnapshotLimits::default(),
        )
        .unwrap();
        let trace = world
            .trace_snapshot_ray(
                &coincident,
                SnapshotRayRequest {
                    start: [-32.0, 0.0, 0.0],
                    end: [32.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(trace.hit, Some(Hit::WorldBrush { brush: 0 }));

        let behind_world = Snapshot::compile(
            &world,
            14,
            vec![ObjectInput {
                transform: Transform {
                    origin: [64.0, 0.0, 0.0],
                    ..Transform::IDENTITY
                },
                shape: SnapshotShape::BrushModel { model: 0 },
                ..box_object(2, [0.0; 3], [1.0; 3])
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let depth = world
            .trace_snapshot_ray(
                &behind_world,
                SnapshotRayRequest {
                    start: [-32.0, 0.0, 0.0],
                    end: [96.0, 0.0, 0.0],
                    mask: 1,
                    scope: TraceScope::Everything,
                    ignored: &[],
                },
                |_| true,
            )
            .unwrap();
        assert_eq!(depth.hit, Some(Hit::WorldBrush { brush: 0 }));

        assert_eq!(
            World::empty()
                .trace_snapshot_ray(
                    &behind_world,
                    SnapshotRayRequest {
                        start: [0.0; 3],
                        end: [1.0, 0.0, 0.0],
                        mask: 1,
                        scope: TraceScope::Everything,
                        ignored: &[],
                    },
                    |_| true,
                )
                .unwrap_err()
                .code,
            ErrorCode::InvalidSnapshot
        );

        assert_eq!(
            Snapshot::compile(
                &World::empty(),
                1,
                vec![box_object(1, [0.0; 3], [1.0; 3])],
                SnapshotLimits {
                    max_objects: 0,
                    ..SnapshotLimits::default()
                },
            )
            .unwrap_err()
            .code,
            ErrorCode::Limit
        );
        let tiny = Snapshot::compile(
            &World::empty(),
            1,
            vec![box_object(1, [0.0; 3], [1.0; 3])],
            SnapshotLimits {
                max_snapshot_bytes: 8,
                ..SnapshotLimits::default()
            },
        )
        .unwrap();
        assert_eq!(tiny.snapshot_bytes().unwrap_err().code, ErrorCode::Limit);
    }

    #[test]
    fn equal_world_contacts_retain_near_leaf_brush_order() {
        let mut world = compile(&fixture()).unwrap();
        world.leaf_brushes = vec![1, 0];
        world.leaves[0].first_leaf_brush = 0;
        world.leaves[0].leaf_brush_count = 2;
        let point = Hull {
            mins: [0.0; 3],
            maxs: [0.0; 3],
        };
        let first = world
            .trace_hull([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], point, 1)
            .unwrap();
        assert_eq!(first.hit, Some(Hit::WorldBrush { brush: 1 }));
        world.leaf_brushes = vec![0, 1];
        let reversed = world
            .trace_hull([-32.0, 0.0, 0.0], [32.0, 0.0, 0.0], point, 1)
            .unwrap();
        assert_eq!(reversed.hit, Some(Hit::WorldBrush { brush: 0 }));
    }

    #[test]
    fn lighting_batch_uses_the_existing_world_ray_authority() {
        let world = compile(&fixture()).unwrap();
        let snapshot = Snapshot::compile(&world, 77, vec![], SnapshotLimits::default()).unwrap();
        let batch = world
            .trace_lighting_rays(
                &snapshot,
                0x4c49_4748_545f_574f,
                LightingOccluders::World,
                &[LightingRay {
                    identity: 100,
                    start: [-32.0, 0.0, 0.0],
                    end: [32.0, 0.0, 0.0],
                    ignored_static_prop: None,
                }],
                LightingRayLimits::default(),
                |_| false,
            )
            .unwrap();
        assert_eq!(batch.rays[0].trace.hit, Some(Hit::WorldBrush { brush: 0 }));
        assert!(batch.rays[0].trace.is_sky());
        assert_eq!(batch.snapshot, 77);
    }

    #[test]
    fn displacement_inputs_attach_once_without_render_tag_filtering() {
        let mut world = World::empty();
        world.displacements.push(DisplacementPatch {
            source: 0,
            map_face: 3,
            power: 2,
            contents: CONTENTS_SOLID,
            vertex_start: 0,
            triangle_start: 0,
            start_position: [0.0; 3],
            minimum_tessellation: 0,
            smoothing_angle: 0.0,
            allowed_vertices: [u32::MAX; 10],
            edge_neighbors: [playsrc_bsp::DispNeighbor {
                sub_neighbors: [playsrc_bsp::DispSubNeighbor {
                    neighbor: u16::MAX,
                    orientation: 0,
                    span: 0,
                    neighbor_span: 0,
                    padding: 0,
                }; 2],
            }; 4],
            corner_neighbors: [playsrc_bsp::DispCornerNeighbors {
                neighbors: [u16::MAX; 4],
                neighbor_count: 0,
                padding: 0,
            }; 4],
            vertices: vec![([0.0; 3], 0.0, 0.0); 25],
            triangle_tags: vec![0x20; 32],
        });
        let registry = [7; 32];
        let positions = (0..25)
            .map(|index| [(index / 5) as f32, (index % 5) as f32, 0.0])
            .collect::<Vec<_>>();
        let mut triangles = Vec::new();
        for column in 0..4 {
            for row in 0..4 {
                let index = column * 5 + row;
                if index % 2 == 1 {
                    triangles.push([index, index + 5, index + 1]);
                    triangles.push([index + 1, index + 5, index + 6]);
                } else {
                    triangles.push([index, index + 5, index + 6]);
                    triangles.push([index, index + 6, index + 1]);
                }
            }
        }
        let input = DisplacementInput {
            source: 0,
            parent_face: 3,
            contents: CONTENTS_SOLID,
            positions,
            triangles,
            triangle_tags: vec![0x20; 32],
            primary_surface: SurfaceIdentity { registry, index: 1 },
            secondary_surface: Some(SurfaceIdentity { registry, index: 2 }),
            use_secondary_surface: vec![false; 32],
        };
        let assembled = world.with_displacement_inputs(vec![input]).unwrap();
        assert_eq!(assembled.displacement_inputs[0].triangles.len(), 32);
        assert!(
            assembled.displacement_inputs[0]
                .triangle_tags
                .iter()
                .all(|tag| *tag == 0x20)
        );
        let point = Hull {
            mins: [0.0; 3],
            maxs: [0.0; 3],
        };
        let ray = assembled
            .trace_hull([0.25, 0.25, -1.0], [0.25, 0.25, 1.0], point, CONTENTS_SOLID)
            .unwrap();
        assert!(matches!(
            ray.displacement,
            Some(DisplacementFeature {
                source: 0,
                parent_face: 3,
                triangle: 0,
            })
        ));
        assert_eq!(ray.displacement_flags, 0x09);
        assert_eq!(ray.surface, Some(SurfaceIdentity { registry, index: 1 }));
        assert_eq!(ray.plane.unwrap().normal, [0.0, 0.0, -1.0]);
        assert!(
            !assembled
                .trace_hull([0.25, 0.25, 1.0], [0.25, 0.25, -1.0], point, CONTENTS_SOLID)
                .unwrap()
                .did_hit()
        );
        assert!(
            !assembled
                .trace_hull([0.25, 0.25, 0.0], [0.25, 0.25, 0.0], point, CONTENTS_SOLID)
                .unwrap()
                .start_solid
        );
        let overlap = assembled
            .trace_hull(
                [0.25, 0.25, 0.0],
                [0.25, 0.25, 0.0],
                Hull {
                    mins: [-0.1; 3],
                    maxs: [0.1; 3],
                },
                CONTENTS_SOLID,
            )
            .unwrap();
        assert!(overlap.start_solid && overlap.all_solid && overlap.fraction == 0.0);
        let swept_hull = assembled
            .trace_hull(
                [0.25, 0.25, -1.0],
                [0.25, 0.25, 1.0],
                Hull {
                    mins: [-0.1; 3],
                    maxs: [0.1; 3],
                },
                CONTENTS_SOLID,
            )
            .unwrap();
        assert!(swept_hull.displacement.is_some());
        assert!(swept_hull.fraction < ray.fraction);
        let snapshot =
            Snapshot::compile(&assembled, 88, vec![], SnapshotLimits::default()).unwrap();
        let lighting = assembled
            .trace_lighting_rays(
                &snapshot,
                0x4449_5350_5f43_4c52,
                LightingOccluders::World,
                &[LightingRay {
                    identity: 1,
                    start: [0.25, 0.25, -1.0],
                    end: [0.25, 0.25, 1.0],
                    ignored_static_prop: None,
                }],
                LightingRayLimits::default(),
                |_| false,
            )
            .unwrap();
        assert_eq!(lighting.rays[0].trace.displacement, ray.displacement);
        assert!(lighting.comparison_bytes().len() > 100);
        assert_eq!(
            assembled
                .clone()
                .with_displacement_inputs(Vec::new())
                .unwrap_err()
                .code,
            ErrorCode::InvalidReference
        );
    }
}
