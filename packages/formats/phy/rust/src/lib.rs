use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::Range,
};

const VPHY: u32 = u32::from_le_bytes(*b"VPHY");
const YHPV: u32 = u32::from_le_bytes(*b"YHPV");
const IVPS: u32 = u32::from_le_bytes(*b"IVPS");
const SVPI: u32 = u32::from_le_bytes(*b"SVPI");
const MOPP: u32 = u32::from_le_bytes(*b"MOPP");
const METERS_PER_UNIT: f32 = 0.0254;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    SourcePcPolygon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    /// Capacity bytes of buffers retained by the returned asset, excluding parser scratch.
    pub max_retained_bytes: usize,
    pub max_solids: usize,
    pub max_solid_bytes: usize,
    pub max_keydata_bytes: usize,
    pub max_tree_nodes: usize,
    pub max_tree_depth: usize,
    pub max_convex_pieces: usize,
    pub max_triangles: usize,
    pub max_points: usize,
    pub max_keydata_tokens: usize,
    pub max_keydata_token_bytes: usize,
    pub max_keydata_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 128 * 1024 * 1024,
            max_retained_bytes: 256 * 1024 * 1024,
            max_solids: 4096,
            max_solid_bytes: 64 * 1024 * 1024,
            max_keydata_bytes: 8 * 1024 * 1024,
            max_tree_nodes: 65_536,
            max_tree_depth: 1024,
            max_convex_pieces: 65_536,
            max_triangles: 1_000_000,
            max_points: 3_000_000,
            max_keydata_tokens: 1_000_000,
            max_keydata_token_bytes: 65_536,
            max_keydata_depth: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float32(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Handled,
    Unsupported,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    ModernPolygon,
    LegacyPolygon,
    ModernOther,
    LegacyOther,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeKind {
    Polygon,
    Mopp,
    Ball,
    Virtual,
    SwappedEndian,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Point {
    pub source_bits: [Float32; 3],
    pub source_inches: [Float32; 3],
    pub raw_range: Range<usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Triangle {
    pub raw: [u8; 16],
    pub point_indices: [u32; 3],
    pub material_index: u8,
    pub is_virtual: bool,
    pub unclassified_bits: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Convex {
    pub client_data: i32,
    pub geometry: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvexGeometry {
    pub raw_header: [u8; 16],
    pub subtree: Option<usize>,
    pub raw_range: Range<usize>,
    pub points: Vec<Point>,
    pub triangles: Vec<Triangle>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchyNode {
    pub raw: [u8; 28],
    pub children: Option<[usize; 2]>,
    /// Index into the solid's geometry array, including nonterminal hulls.
    pub hull: Option<usize>,
}
impl HierarchyNode {
    pub fn center_bits(&self) -> [Float32; 3] {
        std::array::from_fn(|axis| {
            Float32(u32::from_le_bytes(
                self.raw[8 + axis * 4..12 + axis * 4]
                    .try_into()
                    .expect("node center"),
            ))
        })
    }
    pub fn radius_bits(&self) -> Float32 {
        Float32(u32::from_le_bytes(
            self.raw[20..24].try_into().expect("node radius"),
        ))
    }
    pub fn box_sizes(&self) -> [u8; 3] {
        self.raw[24..27].try_into().expect("node bounds")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConvexHierarchy {
    /// The root is always entry zero. Child order is authored left, right.
    pub nodes: Vec<HierarchyNode>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Solid {
    pub index: usize,
    pub length_prefix: i32,
    pub body_range: Range<usize>,
    pub body: Vec<u8>,
    pub encoding: Encoding,
    pub shape: ShapeKind,
    pub classification: Classification,
    pub center_bits: [Float32; 3],
    pub inertia_bits: [Float32; 3],
    pub radius_bits: Float32,
    pub center_source_inches: [Float32; 3],
    pub inertia_source: [Float32; 3],
    pub radius_source_inches: Float32,
    pub max_surface_deviation: u8,
    pub drag_axis_bits: Option<[Float32; 3]>,
    pub axis_map: Vec<u8>,
    pub game_data: Vec<u32>,
    pub convexes: Vec<Convex>,
    pub geometries: Vec<ConvexGeometry>,
    pub hierarchy: Option<ConvexHierarchy>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    pub raw: [u8; 16],
    pub size: i32,
    pub id: i32,
    pub solid_count: i32,
    pub checksum: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyValue {
    Scalar {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Block {
        key: Vec<u8>,
        entries: Vec<KeyValue>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyBlock {
    pub name: Vec<u8>,
    pub entries: Vec<KeyValue>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyData {
    pub raw: Vec<u8>,
    pub document_range: Range<usize>,
    pub terminator_range: Range<usize>,
    pub suffix: Vec<u8>,
    pub blocks: Vec<KeyBlock>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asset {
    pub profile: Profile,
    pub source: Vec<u8>,
    pub header: Option<Header>,
    pub solids: Vec<Solid>,
    pub key_data: KeyData,
}

impl Asset {
    pub fn retained_bytes(&self) -> usize {
        fn capacity<T>(values: &Vec<T>) -> usize {
            values.capacity() * std::mem::size_of::<T>()
        }
        fn entries(values: &Vec<KeyValue>) -> usize {
            capacity(values)
                + values
                    .iter()
                    .map(|value| match value {
                        KeyValue::Scalar { key, value } => capacity(key) + capacity(value),
                        KeyValue::Block {
                            key,
                            entries: children,
                        } => capacity(key) + entries(children),
                    })
                    .sum::<usize>()
        }
        capacity(&self.source)
            + capacity(&self.solids)
            + self
                .solids
                .iter()
                .map(|solid| {
                    capacity(&solid.body)
                        + capacity(&solid.axis_map)
                        + capacity(&solid.game_data)
                        + capacity(&solid.convexes)
                        + capacity(&solid.geometries)
                        + solid
                            .hierarchy
                            .as_ref()
                            .map_or(0, |tree| capacity(&tree.nodes))
                        + solid
                            .geometries
                            .iter()
                            .map(|geometry| {
                                capacity(&geometry.points) + capacity(&geometry.triangles)
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
            + capacity(&self.key_data.raw)
            + capacity(&self.key_data.suffix)
            + capacity(&self.key_data.blocks)
            + self
                .key_data
                .blocks
                .iter()
                .map(|block| capacity(&block.name) + entries(&block.entries))
                .sum::<usize>()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidLimits,
    InputLimit,
    RetainedLimit,
    TruncatedHeader,
    InvalidHeader,
    InvalidSolidCount,
    InvalidSolidRange,
    InvalidVersion,
    InvalidSurface,
    TreeCycle,
    TreeLimit,
    InvalidConvex,
    InvalidPoint,
    NonFinite,
    MissingKeydataTerminator,
    InvalidKeydata,
    KeydataLimit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub range: Range<usize>,
    pub solid: Option<usize>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at {}..{}",
            self.code, self.range.start, self.range.end
        )
    }
}
impl std::error::Error for Error {}

struct RetainedBudget {
    remaining: usize,
    input_size: usize,
}
impl RetainedBudget {
    fn new(limits: Limits, input_size: usize) -> Self {
        Self {
            remaining: limits.max_retained_bytes,
            input_size,
        }
    }
    fn reserve<T>(&mut self, values: &mut Vec<T>, capacity: usize) -> Result<(), Error> {
        let extra = capacity.saturating_sub(values.capacity());
        let bytes = extra
            .checked_mul(std::mem::size_of::<T>())
            .filter(|bytes| *bytes <= self.remaining)
            .ok_or_else(|| err(ErrorCode::RetainedLimit, 0..self.input_size, None))?;
        if extra > 0 {
            values
                .try_reserve_exact(capacity - values.len())
                .map_err(|_| err(ErrorCode::RetainedLimit, 0..self.input_size, None))?;
            self.remaining -= bytes;
        }
        Ok(())
    }
    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        if values.len() == values.capacity() {
            let capacity = values.capacity().saturating_mul(2).max(4);
            self.reserve(values, capacity)?;
        }
        values.push(value);
        Ok(())
    }
    fn copy(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        let mut values = Vec::new();
        self.reserve(&mut values, bytes.len())?;
        values.extend_from_slice(bytes);
        Ok(values)
    }
}

pub fn parse_standalone(bytes: &[u8], profile: Profile, limits: Limits) -> Result<Asset, Error> {
    validate_limits(limits)?;
    if bytes.len() > limits.max_input_bytes {
        return Err(err(ErrorCode::InputLimit, 0..bytes.len(), None));
    }
    if bytes.len() < 16 {
        return Err(err(ErrorCode::TruncatedHeader, bytes.len()..16, None));
    }
    let size = i32_at(bytes, 0, None)?;
    let solid_count = i32_at(bytes, 8, None)?;
    if size != 16 {
        return Err(err(ErrorCode::InvalidHeader, 0..4, None));
    }
    if solid_count <= 0 {
        return Err(err(ErrorCode::InvalidSolidCount, 8..12, None));
    }
    let header = Header {
        raw: bytes[..16].try_into().expect("header"),
        size,
        id: i32_at(bytes, 4, None)?,
        solid_count,
        checksum: i32_at(bytes, 12, None)?,
    };
    let mut budget = RetainedBudget::new(limits, bytes.len());
    let source = budget.copy(bytes)?;
    let (solids, end) = parse_solids(
        bytes,
        16,
        solid_count as usize,
        profile,
        limits,
        &mut budget,
    )?;
    let key_data = parse_keydata(&bytes[end..], end, limits, &mut budget)?;
    Ok(Asset {
        profile,
        source,
        header: Some(header),
        solids,
        key_data,
    })
}

pub fn parse_payload(
    collision: &[u8],
    keydata: &[u8],
    solid_count: usize,
    profile: Profile,
    limits: Limits,
) -> Result<Asset, Error> {
    validate_limits(limits)?;
    if collision.len() + keydata.len() > limits.max_input_bytes {
        return Err(err(
            ErrorCode::InputLimit,
            0..collision.len() + keydata.len(),
            None,
        ));
    }
    let mut budget = RetainedBudget::new(limits, collision.len() + keydata.len());
    let mut source = Vec::new();
    budget.reserve(&mut source, collision.len() + keydata.len())?;
    source.extend_from_slice(collision);
    source.extend_from_slice(keydata);
    let (solids, end) = parse_solids(collision, 0, solid_count, profile, limits, &mut budget)?;
    if end != collision.len() {
        return Err(err(
            ErrorCode::InvalidSolidRange,
            end..collision.len(),
            None,
        ));
    }
    let key_data = parse_keydata(keydata, collision.len(), limits, &mut budget)?;
    Ok(Asset {
        profile,
        source,
        header: None,
        solids,
        key_data,
    })
}

fn parse_solids(
    bytes: &[u8],
    mut cursor: usize,
    count: usize,
    _profile: Profile,
    limits: Limits,
    budget: &mut RetainedBudget,
) -> Result<(Vec<Solid>, usize), Error> {
    if count == 0 || count > limits.max_solids {
        return Err(err(ErrorCode::InvalidSolidCount, cursor..cursor, None));
    }
    let mut output = Vec::new();
    budget.reserve(&mut output, count)?;
    for index in 0..count {
        let prefix = cursor;
        let length = i32_at(bytes, cursor, Some(index))?;
        if length < 0 || length as usize > limits.max_solid_bytes {
            return Err(err(
                ErrorCode::InvalidSolidRange,
                prefix..prefix + 4,
                Some(index),
            ));
        }
        cursor = cursor
            .checked_add(4)
            .ok_or_else(|| err(ErrorCode::InvalidSolidRange, prefix..prefix, Some(index)))?;
        let body = take(bytes, cursor, length as usize, Some(index))?;
        output.push(parse_solid(body, index, cursor, length, limits, budget)?);
        cursor += length as usize;
    }
    Ok((output, cursor))
}

fn parse_solid(
    body: &[u8],
    index: usize,
    base: usize,
    length: i32,
    limits: Limits,
    budget: &mut RetainedBudget,
) -> Result<Solid, Error> {
    let mut encoding = Encoding::LegacyOther;
    let mut shape = ShapeKind::Unknown;
    let mut classification = Classification::Unknown;
    let mut surface = None;
    let mut surface_base = base;
    let mut drag = None;
    let mut axis_map = Vec::new();
    if body.len() >= 4 && u32_at(body, 0, Some(index))? == YHPV {
        encoding = Encoding::ModernOther;
        shape = ShapeKind::SwappedEndian;
        classification = Classification::Unsupported;
    } else if body.len() >= 8 && u32_at(body, 0, Some(index))? == VPHY {
        encoding = Encoding::ModernOther;
        let version = i16_at(body, 4, Some(index))?;
        if version != 0x0100 {
            return Err(err(
                ErrorCode::InvalidVersion,
                base + 4..base + 6,
                Some(index),
            ));
        }
        match i16_at(body, 6, Some(index))? {
            0 => {
                encoding = Encoding::ModernPolygon;
                shape = ShapeKind::Polygon;
                classification = Classification::Handled;
                take(body, 0, 28, Some(index))?;
                let surface_size = positive_i32(body, 8, index)?;
                let axis_size = positive_i32(body, 24, index)?;
                let start = 28_usize;
                let end = start.checked_add(surface_size).ok_or_else(|| {
                    err(
                        ErrorCode::InvalidSolidRange,
                        base..base + body.len(),
                        Some(index),
                    )
                })?;
                let axis_end = end.checked_add(axis_size).ok_or_else(|| {
                    err(
                        ErrorCode::InvalidSolidRange,
                        base..base + body.len(),
                        Some(index),
                    )
                })?;
                if axis_end != body.len() {
                    return Err(err(
                        ErrorCode::InvalidSolidRange,
                        base..base + body.len(),
                        Some(index),
                    ));
                }
                surface = Some(&body[start..end]);
                surface_base = base + start;
                drag = Some(float3(body, 12, index)?);
                axis_map = budget.copy(&body[end..axis_end])?;
            }
            1 => {
                shape = ShapeKind::Mopp;
                classification = Classification::Unsupported;
            }
            2 => {
                shape = ShapeKind::Ball;
                classification = Classification::Unsupported;
            }
            3 => {
                shape = ShapeKind::Virtual;
                classification = Classification::Unsupported;
            }
            _ => {
                shape = ShapeKind::Unknown;
                classification = Classification::Unknown;
            }
        }
    } else if body.len() >= 48 {
        let id = u32_at(body, 44, Some(index))?;
        match id {
            IVPS | 0 => {
                encoding = Encoding::LegacyPolygon;
                shape = ShapeKind::Polygon;
                classification = Classification::Handled;
                surface = Some(body);
            }
            MOPP => {
                shape = ShapeKind::Mopp;
                classification = Classification::Unsupported;
            }
            SVPI => {
                shape = ShapeKind::SwappedEndian;
                classification = Classification::Unsupported;
            }
            _ => {}
        }
    }
    let empty = [Float32(0); 3];
    let mut solid = Solid {
        index,
        length_prefix: length,
        body_range: base..base + body.len(),
        body: budget.copy(body)?,
        encoding,
        shape,
        classification,
        center_bits: empty,
        inertia_bits: empty,
        radius_bits: Float32(0),
        center_source_inches: empty,
        inertia_source: empty,
        radius_source_inches: Float32(0),
        max_surface_deviation: 0,
        drag_axis_bits: drag,
        axis_map,
        game_data: Vec::new(),
        convexes: Vec::new(),
        geometries: Vec::new(),
        hierarchy: None,
    };
    if let Some(surface) = surface {
        decode_surface(surface, surface_base, &mut solid, limits, budget)?;
    }
    Ok(solid)
}

fn decode_surface(
    surface: &[u8],
    base: usize,
    solid: &mut Solid,
    limits: Limits,
    budget: &mut RetainedBudget,
) -> Result<(), Error> {
    take(surface, 0, 48, Some(solid.index))?;
    let center = float3(surface, 0, solid.index)?;
    let inertia = float3(surface, 12, solid.index)?;
    let radius = float(surface, 24, solid.index)?;
    let packed = u32_at(surface, 28, Some(solid.index))?;
    let size = (packed >> 8) as usize;
    if size < 48 || size > surface.len() {
        return Err(err(
            ErrorCode::InvalidSurface,
            base + 28..base + 32,
            Some(solid.index),
        ));
    }
    let surface = &surface[..size];
    let root = positive_i32(surface, 32, solid.index)?;
    take(surface, root, 28, Some(solid.index))?;
    solid.center_bits = center;
    solid.inertia_bits = inertia;
    solid.radius_bits = radius;
    solid.max_surface_deviation = (packed & 0xff) as u8;
    solid.center_source_inches = position_to_source(center)?;
    solid.inertia_source = [inertia[0], inertia[2], inertia[1]];
    solid.radius_source_inches = Float32((finite(radius)? / METERS_PER_UNIT).to_bits());
    let mut pending = vec![(root, 0usize, false)];
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut ledges = BTreeSet::new();
    let mut node_indices = BTreeMap::new();
    let mut geometry_indices = BTreeMap::new();
    let mut hierarchy = ConvexHierarchy { nodes: Vec::new() };
    let mut children = Vec::<Option<[usize; 2]>>::new();
    let mut geometry_links = Vec::<Option<usize>>::new();
    while let Some((node, depth, exit)) = pending.pop() {
        if exit {
            active.remove(&node);
            done.insert(node);
            continue;
        }
        if done.contains(&node) {
            continue;
        }
        if !active.insert(node) {
            return Err(err(
                ErrorCode::TreeCycle,
                base + node..base + node + 28,
                Some(solid.index),
            ));
        }
        if hierarchy.nodes.len() >= limits.max_tree_nodes || depth > limits.max_tree_depth {
            return Err(err(
                ErrorCode::TreeLimit,
                base + node..base + node + 28,
                Some(solid.index),
            ));
        }
        let raw: [u8; 28] = take(surface, node, 28, Some(solid.index))?
            .try_into()
            .expect("hierarchy node");
        for value in float3(surface, node + 8, solid.index)? {
            finite(value)?;
        }
        if finite(float(surface, node + 20, solid.index)?)? < 0.0 {
            return Err(err(
                ErrorCode::InvalidSurface,
                base + node + 20..base + node + 24,
                Some(solid.index),
            ));
        }
        let node_index = hierarchy.nodes.len();
        node_indices.insert(node, node_index);
        pending.push((node, depth, true));
        let right = i32_at(surface, node, Some(solid.index))?;
        let ledge = i32_at(surface, node + 4, Some(solid.index))?;
        let mut hull = None;
        if ledge != 0 {
            let at = add_signed(node, ledge, solid.index)?;
            let geometry = if let Some(index) = geometry_indices.get(&at) {
                *index
            } else {
                if solid.geometries.len() >= limits.max_convex_pieces {
                    return Err(err(
                        ErrorCode::TreeLimit,
                        base + at..base + at,
                        Some(solid.index),
                    ));
                }
                let geometry = parse_convex(surface, base, at, solid.index, limits, budget)?;
                let recursive = u32_at(surface, at + 8, Some(solid.index))? & 3 != 0;
                let link = if recursive {
                    Some(add_signed(
                        at,
                        i32_at(surface, at + 4, Some(solid.index))?,
                        solid.index,
                    )?)
                } else {
                    None
                };
                let index = solid.geometries.len();
                budget.push(&mut solid.geometries, geometry)?;
                geometry_links.push(link);
                geometry_indices.insert(at, index);
                index
            };
            hull = Some(geometry);
            if right == 0 && ledges.insert(at) {
                if geometry_links[geometry].is_some() {
                    return Err(err(
                        ErrorCode::InvalidConvex,
                        base + at..base + at + 16,
                        Some(solid.index),
                    ));
                }
                let client_data = i32_at(surface, at + 4, Some(solid.index))?;
                budget.push(&mut solid.game_data, client_data as u32)?;
                budget.push(
                    &mut solid.convexes,
                    Convex {
                        client_data,
                        geometry,
                    },
                )?;
            }
        }
        budget.push(
            &mut hierarchy.nodes,
            HierarchyNode {
                raw,
                children: None,
                hull,
            },
        )?;
        if right == 0 {
            if ledge == 0 {
                return Err(err(
                    ErrorCode::InvalidSurface,
                    base + node..base + node + 28,
                    Some(solid.index),
                ));
            }
            children.push(None);
        } else {
            let left = node.checked_add(28).ok_or_else(|| {
                err(
                    ErrorCode::InvalidSurface,
                    base + node..base + node,
                    Some(solid.index),
                )
            })?;
            let right = add_signed(node, right, solid.index)?;
            take(surface, left, 28, Some(solid.index))?;
            take(surface, right, 28, Some(solid.index))?;
            children.push(Some([left, right]));
            pending.push((left, depth + 1, false));
            pending.push((right, depth + 1, false));
        }
    }
    for (index, children) in children.into_iter().enumerate() {
        if let Some([left, right]) = children {
            hierarchy.nodes[index].children = Some([
                *node_indices.get(&left).ok_or_else(|| {
                    err(
                        ErrorCode::InvalidSurface,
                        base + left..base + left,
                        Some(solid.index),
                    )
                })?,
                *node_indices.get(&right).ok_or_else(|| {
                    err(
                        ErrorCode::InvalidSurface,
                        base + right..base + right,
                        Some(solid.index),
                    )
                })?,
            ]);
        }
    }
    for (index, link) in geometry_links.into_iter().enumerate() {
        if let Some(link) = link {
            solid.geometries[index].subtree = Some(*node_indices.get(&link).ok_or_else(|| {
                err(
                    ErrorCode::InvalidSurface,
                    base + link..base + link,
                    Some(solid.index),
                )
            })?);
        }
    }
    if solid.convexes.is_empty() {
        return Err(err(
            ErrorCode::InvalidSurface,
            base..base + size,
            Some(solid.index),
        ));
    }
    solid.hierarchy = Some(hierarchy);
    Ok(())
}

fn parse_convex(
    surface: &[u8],
    base: usize,
    offset: usize,
    solid: usize,
    limits: Limits,
    budget: &mut RetainedBudget,
) -> Result<ConvexGeometry, Error> {
    let raw_header = take(surface, offset, 16, Some(solid))?
        .try_into()
        .expect("convex header");
    let point_relative = i32_at(surface, offset, Some(solid))?;
    let packed = u32_at(surface, offset + 8, Some(solid))?;
    let size = ((packed >> 8) as usize).checked_mul(16).ok_or_else(|| {
        err(
            ErrorCode::InvalidConvex,
            base + offset..base + offset,
            Some(solid),
        )
    })?;
    if size < 16 {
        return Err(err(
            ErrorCode::InvalidConvex,
            base + offset..base + offset + 16,
            Some(solid),
        ));
    }
    take(surface, offset, size, Some(solid))?;
    let triangle_count = i16_at(surface, offset + 12, Some(solid))?;
    if triangle_count < 0
        || triangle_count as usize > limits.max_triangles
        || 16 + (triangle_count as usize) * 16 > size
    {
        return Err(err(
            ErrorCode::InvalidConvex,
            base + offset + 12..base + offset + 14,
            Some(solid),
        ));
    }
    let mut raw_triangles = Vec::with_capacity(triangle_count as usize);
    let mut source_points = BTreeSet::new();
    for i in 0..triangle_count as usize {
        let at = offset + 16 + i * 16;
        let raw: [u8; 16] = take(surface, at, 16, Some(solid))?
            .try_into()
            .expect("triangle");
        let metadata = u32_at(surface, at, Some(solid))?;
        let mut points = [0; 3];
        for (edge, point) in points.iter_mut().enumerate() {
            *point = u32_at(surface, at + 4 + edge * 4, Some(solid))? & 0xffff;
            source_points.insert(*point);
        }
        raw_triangles.push((raw, metadata, points));
    }
    if source_points.len() > limits.max_points {
        return Err(err(
            ErrorCode::InvalidPoint,
            base + offset..base + offset + size,
            Some(solid),
        ));
    }
    let points_start = add_signed(offset, point_relative, solid)?;
    if let Some(&last) = source_points.last() {
        let last = points_start
            .checked_add(last as usize * 16)
            .ok_or_else(|| {
                err(
                    ErrorCode::InvalidPoint,
                    base + offset..base + offset,
                    Some(solid),
                )
            })?;
        take(surface, last, 16, Some(solid))?;
    }
    let mut remap = BTreeMap::new();
    let mut points = Vec::new();
    budget.reserve(&mut points, source_points.len())?;
    for source in source_points {
        let at = points_start + source as usize * 16;
        let bits = float3(surface, at, solid)?;
        let converted = position_to_source(bits)?;
        remap.insert(source, points.len() as u32);
        points.push(Point {
            source_bits: bits,
            source_inches: converted,
            raw_range: base + at..base + at + 16,
        });
    }
    let mut triangles = Vec::new();
    budget.reserve(&mut triangles, raw_triangles.len())?;
    for (raw, metadata, p) in raw_triangles {
        triangles.push(Triangle {
            raw,
            point_indices: [remap[&p[2]], remap[&p[1]], remap[&p[0]]],
            material_index: ((metadata >> 24) & 0x7f) as u8,
            is_virtual: metadata & 0x8000_0000 != 0,
            unclassified_bits: metadata & 0x00ff_ffff,
        });
    }
    Ok(ConvexGeometry {
        raw_header,
        subtree: None,
        raw_range: base + offset..base + offset + size,
        points,
        triangles,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Word(Vec<u8>),
    Open,
    Close,
}
fn parse_keydata(
    bytes: &[u8],
    base: usize,
    limits: Limits,
    budget: &mut RetainedBudget,
) -> Result<KeyData, Error> {
    if bytes.len() > limits.max_keydata_bytes {
        return Err(err(ErrorCode::KeydataLimit, base..base + bytes.len(), None));
    }
    let Some(nul) = bytes.iter().position(|b| *b == 0) else {
        return Err(err(
            ErrorCode::MissingKeydataTerminator,
            base..base + bytes.len(),
            None,
        ));
    };
    let document = &bytes[..nul];
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    while cursor < document.len() {
        match document[cursor] {
            b if b.is_ascii_whitespace() => cursor += 1,
            b'/' if document.get(cursor + 1) == Some(&b'/') => {
                cursor += 2;
                while cursor < document.len() && !matches!(document[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'/' if document.get(cursor + 1) == Some(&b'*') => {
                let start = cursor;
                cursor += 2;
                while cursor + 1 < document.len() && &document[cursor..cursor + 2] != b"*/" {
                    cursor += 1;
                }
                if cursor + 1 >= document.len() {
                    return Err(err(
                        ErrorCode::InvalidKeydata,
                        base + start..base + document.len(),
                        None,
                    ));
                }
                cursor += 2;
            }
            b'{' => {
                tokens.push(TokenKind::Open);
                cursor += 1;
            }
            b'}' => {
                tokens.push(TokenKind::Close);
                cursor += 1;
            }
            b'"' => {
                let start = cursor;
                cursor += 1;
                while cursor < document.len() && document[cursor] != b'"' {
                    cursor += 1;
                }
                if cursor >= document.len() {
                    return Err(err(
                        ErrorCode::InvalidKeydata,
                        base + start..base + document.len(),
                        None,
                    ));
                }
                let value = document[start + 1..cursor].to_vec();
                cursor += 1;
                tokens.push(TokenKind::Word(value));
            }
            _ => {
                let start = cursor;
                while cursor < document.len()
                    && !document[cursor].is_ascii_whitespace()
                    && !matches!(document[cursor], b'{' | b'}')
                {
                    cursor += 1;
                }
                tokens.push(TokenKind::Word(document[start..cursor].to_vec()));
            }
        }
        if tokens.len() > limits.max_keydata_tokens {
            return Err(err(ErrorCode::KeydataLimit, base..base + cursor, None));
        }
        if matches!(tokens.last(),Some(TokenKind::Word(v)) if v.len()>limits.max_keydata_token_bytes)
        {
            return Err(err(ErrorCode::KeydataLimit, base..base + cursor, None));
        }
    }
    let mut at = 0;
    let mut blocks = Vec::new();
    while at < tokens.len() {
        let name = word(&tokens, &mut at, base, budget)?;
        if !matches!(tokens.get(at), Some(TokenKind::Open)) {
            return Err(err(ErrorCode::InvalidKeydata, base..base + nul, None));
        }
        at += 1;
        let entries = key_entries(&tokens, &mut at, 1, limits, base, budget)?;
        budget.push(&mut blocks, KeyBlock { name, entries })?;
    }
    let mut term_end = nul + 1;
    while term_end < bytes.len() && bytes[term_end] == 0 {
        term_end += 1;
    }
    Ok(KeyData {
        raw: budget.copy(bytes)?,
        document_range: base..base + nul,
        terminator_range: base + nul..base + term_end,
        suffix: budget.copy(&bytes[term_end..])?,
        blocks,
    })
}
fn word(
    tokens: &[TokenKind],
    at: &mut usize,
    base: usize,
    budget: &mut RetainedBudget,
) -> Result<Vec<u8>, Error> {
    match tokens.get(*at) {
        Some(TokenKind::Word(v)) => {
            *at += 1;
            budget.copy(v)
        }
        _ => Err(err(ErrorCode::InvalidKeydata, base..base, None)),
    }
}
fn key_entries(
    tokens: &[TokenKind],
    at: &mut usize,
    depth: usize,
    limits: Limits,
    base: usize,
    budget: &mut RetainedBudget,
) -> Result<Vec<KeyValue>, Error> {
    if depth > limits.max_keydata_depth {
        return Err(err(ErrorCode::KeydataLimit, base..base, None));
    }
    let mut out = Vec::new();
    loop {
        if matches!(tokens.get(*at), Some(TokenKind::Close)) {
            *at += 1;
            return Ok(out);
        }
        let key = word(tokens, at, base, budget)?;
        match tokens.get(*at) {
            Some(TokenKind::Open) => {
                *at += 1;
                let entries = key_entries(tokens, at, depth + 1, limits, base, budget)?;
                budget.push(&mut out, KeyValue::Block { key, entries })?;
            }
            Some(TokenKind::Word(value)) => {
                let value = budget.copy(value)?;
                budget.push(&mut out, KeyValue::Scalar { key, value })?;
                *at += 1;
            }
            _ => return Err(err(ErrorCode::InvalidKeydata, base..base, None)),
        }
    }
}

fn position_to_source(bits: [Float32; 3]) -> Result<[Float32; 3], Error> {
    let [x, y, z] = bits.map(finite);
    let (x, y, z) = (x?, y?, z?);
    Ok([
        Float32((x / METERS_PER_UNIT).to_bits()),
        Float32((z / METERS_PER_UNIT).to_bits()),
        Float32((-y / METERS_PER_UNIT).to_bits()),
    ])
}
fn finite(value: Float32) -> Result<f32, Error> {
    let value = f32::from_bits(value.0);
    if !value.is_finite() {
        Err(err(ErrorCode::NonFinite, 0..0, None))
    } else {
        Ok(value)
    }
}
fn float3(bytes: &[u8], offset: usize, solid: usize) -> Result<[Float32; 3], Error> {
    Ok([
        float(bytes, offset, solid)?,
        float(bytes, offset + 4, solid)?,
        float(bytes, offset + 8, solid)?,
    ])
}
fn float(bytes: &[u8], offset: usize, solid: usize) -> Result<Float32, Error> {
    Ok(Float32(u32_at(bytes, offset, Some(solid))?))
}
fn add_signed(base: usize, relative: i32, solid: usize) -> Result<usize, Error> {
    base.checked_add_signed(relative as isize)
        .ok_or_else(|| err(ErrorCode::InvalidSurface, base..base, Some(solid)))
}
fn positive_i32(bytes: &[u8], offset: usize, solid: usize) -> Result<usize, Error> {
    usize::try_from(i32_at(bytes, offset, Some(solid))?).map_err(|_| {
        err(
            ErrorCode::InvalidSolidRange,
            offset..offset + 4,
            Some(solid),
        )
    })
}
fn take(bytes: &[u8], offset: usize, length: usize, solid: Option<usize>) -> Result<&[u8], Error> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| err(ErrorCode::InvalidSolidRange, offset..offset, solid))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| err(ErrorCode::InvalidSolidRange, offset..end, solid))
}
fn i16_at(bytes: &[u8], offset: usize, solid: Option<usize>) -> Result<i16, Error> {
    Ok(i16::from_le_bytes(
        take(bytes, offset, 2, solid)?.try_into().expect("field"),
    ))
}
fn i32_at(bytes: &[u8], offset: usize, solid: Option<usize>) -> Result<i32, Error> {
    Ok(i32::from_le_bytes(
        take(bytes, offset, 4, solid)?.try_into().expect("field"),
    ))
}
fn u32_at(bytes: &[u8], offset: usize, solid: Option<usize>) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        take(bytes, offset, 4, solid)?.try_into().expect("field"),
    ))
}
fn validate_limits(l: Limits) -> Result<(), Error> {
    if [
        l.max_input_bytes,
        l.max_retained_bytes,
        l.max_solids,
        l.max_solid_bytes,
        l.max_keydata_bytes,
        l.max_tree_nodes,
        l.max_tree_depth,
        l.max_convex_pieces,
        l.max_triangles,
        l.max_points,
        l.max_keydata_tokens,
        l.max_keydata_token_bytes,
        l.max_keydata_depth,
    ]
    .contains(&0)
    {
        Err(err(ErrorCode::InvalidLimits, 0..0, None))
    } else {
        Ok(())
    }
}
fn err(code: ErrorCode, range: Range<usize>, solid: Option<usize>) -> Error {
    Error { code, range, solid }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact_surface() -> Vec<u8> {
        let mut bytes = vec![0; 156];
        for (offset, value) in [(0, 0.0254_f32), (4, 0.0508), (8, -0.0254)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (offset, value) in [(12, 1.0_f32), (16, 2.0), (20, 3.0), (24, 0.254)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let surface_bytes = bytes.len() as u32;
        bytes[28..32].copy_from_slice(&((surface_bytes << 8) | 7).to_le_bytes());
        bytes[32..36].copy_from_slice(&48_i32.to_le_bytes());
        bytes[44..48].copy_from_slice(&IVPS.to_le_bytes());
        bytes[52..56].copy_from_slice(&28_i32.to_le_bytes());
        let ledge = 76;
        bytes[ledge..ledge + 4].copy_from_slice(&32_i32.to_le_bytes());
        bytes[ledge + 4..ledge + 8].copy_from_slice(&5_i32.to_le_bytes());
        bytes[ledge + 8..ledge + 12].copy_from_slice(&(2_u32 << 8).to_le_bytes());
        bytes[ledge + 12..ledge + 14].copy_from_slice(&1_i16.to_le_bytes());
        let triangle = ledge + 16;
        bytes[triangle..triangle + 4].copy_from_slice(&(3_u32 << 24).to_le_bytes());
        for (edge, point) in [0_u32, 1, 2].into_iter().enumerate() {
            bytes[triangle + 4 + edge * 4..triangle + 8 + edge * 4]
                .copy_from_slice(&point.to_le_bytes());
        }
        let points = ledge + 32;
        for (index, point) in [[0.0_f32, 0.0, 0.0], [0.0254, 0.0, 0.0], [0.0, 0.0, -0.0254]]
            .into_iter()
            .enumerate()
        {
            for (axis, value) in point.into_iter().enumerate() {
                let offset = points + index * 16 + axis * 4;
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    fn modern_body(model_type: i16) -> Vec<u8> {
        if model_type != 0 {
            let mut bytes = b"VPHY".to_vec();
            bytes.extend_from_slice(&0x0100_i16.to_le_bytes());
            bytes.extend_from_slice(&model_type.to_le_bytes());
            return bytes;
        }
        let surface = compact_surface();
        let mut bytes = b"VPHY".to_vec();
        bytes.extend_from_slice(&0x0100_i16.to_le_bytes());
        bytes.extend_from_slice(&0_i16.to_le_bytes());
        bytes.extend_from_slice(&(surface.len() as i32).to_le_bytes());
        for value in [1.0_f32, 2.0, 3.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&surface);
        bytes
    }

    fn stream(body: &[u8]) -> Vec<u8> {
        let mut bytes = (body.len() as i32).to_le_bytes().to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    fn hierarchy_surface() -> Vec<u8> {
        let source = compact_surface();
        let mut bytes = vec![0_u8; 384];
        bytes[..48].copy_from_slice(&source[..48]);
        bytes[28..32].copy_from_slice(&((384_u32 << 8) | 7).to_le_bytes());
        for (node, right, hull) in [(48_usize, 56_i32, 96_i32), (76, 0, 148), (104, 0, 200)] {
            bytes[node..node + 4].copy_from_slice(&right.to_le_bytes());
            bytes[node + 4..node + 8].copy_from_slice(&hull.to_le_bytes());
            bytes[node + 20..node + 24].copy_from_slice(&1.0_f32.to_le_bytes());
            bytes[node + 24..node + 27].copy_from_slice(&[250, 249, 248]);
        }
        bytes[75] = 0x95;
        for (offset, link, flags) in [(144_usize, -96_i32, 5_u32), (224, 10, 4), (304, 20, 4)] {
            bytes[offset..offset + 80].copy_from_slice(&source[76..156]);
            bytes[offset + 4..offset + 8].copy_from_slice(&link.to_le_bytes());
            bytes[offset + 8..offset + 12].copy_from_slice(&((5 << 8) | flags).to_le_bytes());
        }
        bytes[158..160].copy_from_slice(&[0x7e, 0x65]);
        bytes
    }
    #[test]
    fn retained_budget_counts_every_output_buffer_before_growth() {
        let mut collision = stream(&modern_body(0));
        collision.extend(stream(&hierarchy_surface()));
        let keys = b"solid { mass 5 child { name x } } second { key value }\0\0suffix";
        let expected = parse_payload(
            &collision,
            keys,
            2,
            Profile::SourcePcPolygon,
            Limits::default(),
        )
        .unwrap();
        let bytes = expected.retained_bytes();
        let limits = Limits {
            max_retained_bytes: bytes,
            ..Limits::default()
        };
        assert_eq!(
            parse_payload(&collision, keys, 2, Profile::SourcePcPolygon, limits).unwrap(),
            expected
        );
        for limit in [1, collision.len(), bytes - 1] {
            assert_eq!(
                parse_payload(
                    &collision,
                    keys,
                    2,
                    Profile::SourcePcPolygon,
                    Limits {
                        max_retained_bytes: limit,
                        ..limits
                    }
                )
                .unwrap_err()
                .code,
                ErrorCode::RetainedLimit
            );
        }
        let mut standalone = Vec::new();
        for word in [16_i32, 0, 2, 0] {
            standalone.extend_from_slice(&word.to_le_bytes());
        }
        standalone.extend_from_slice(&collision);
        standalone.extend_from_slice(keys);
        let full =
            parse_standalone(&standalone, Profile::SourcePcPolygon, Limits::default()).unwrap();
        assert_eq!(full.retained_bytes(), bytes + 16);
        assert_eq!(
            parse_standalone(
                &standalone,
                Profile::SourcePcPolygon,
                Limits {
                    max_retained_bytes: bytes + 15,
                    ..limits
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::RetainedLimit
        );
        assert_eq!(
            parse_standalone(
                &standalone,
                Profile::SourcePcPolygon,
                Limits {
                    max_retained_bytes: bytes + 16,
                    ..limits
                }
            )
            .unwrap(),
            full
        );
    }

    #[test]
    fn retains_ordered_hierarchy_enclosing_geometry_and_raw_headers() {
        let surface = hierarchy_surface();
        let asset = parse_payload(
            &stream(&surface),
            &[0],
            1,
            Profile::SourcePcPolygon,
            Limits::default(),
        )
        .unwrap();
        let solid = &asset.solids[0];
        let tree = solid.hierarchy.as_ref().unwrap();
        assert_eq!(
            solid
                .convexes
                .iter()
                .map(|piece| piece.client_data)
                .collect::<Vec<_>>(),
            [20, 10]
        );
        assert_eq!(solid.geometries.len(), 3);
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].children, Some([2, 1]));
        assert_eq!(tree.nodes[0].hull, Some(0));
        assert_eq!(solid.geometries[0].subtree, Some(0));
        assert_eq!(tree.nodes[0].raw, &surface[48..76]);
        assert_eq!(tree.nodes[0].box_sizes(), [250, 249, 248]);
        assert_eq!(solid.geometries[0].raw_header, &surface[144..160]);
        assert_eq!(solid.convexes[0].geometry, 1);
        assert_eq!(solid.convexes[1].geometry, 2);
        assert!(
            solid
                .convexes
                .iter()
                .all(|piece| solid.geometries[piece.geometry].subtree.is_none())
        );
    }

    #[test]
    fn signed_geometry_point_offsets_can_share_an_earlier_point_array() {
        let mut surface = hierarchy_surface();
        surface[304..308].copy_from_slice(&(-48_i32).to_le_bytes());
        surface[312..316].copy_from_slice(&(5_u32 << 8).to_le_bytes());
        let asset = parse_payload(
            &stream(&surface),
            &[0],
            1,
            Profile::SourcePcPolygon,
            Limits::default(),
        )
        .unwrap();
        let solid = &asset.solids[0];
        assert_eq!(solid.geometries[1].points, solid.geometries[2].points);
        assert_eq!(solid.geometries[1].points[0].raw_range.start, 4 + 256);
    }

    #[test]
    fn hierarchy_cycles_links_geometry_ranges_and_total_hull_limits_are_bounded() {
        let parse = |surface: &[u8], limits| {
            parse_payload(&stream(surface), &[0], 1, Profile::SourcePcPolygon, limits)
        };
        let surface = hierarchy_surface();
        assert_eq!(
            parse(
                &surface,
                Limits {
                    max_tree_nodes: 2,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::TreeLimit
        );
        assert_eq!(
            parse(
                &surface,
                Limits {
                    max_convex_pieces: 2,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::TreeLimit
        );
        let mut cycle = surface.clone();
        cycle[104..108].copy_from_slice(&(-56_i32).to_le_bytes());
        assert_eq!(
            parse(&cycle, Limits::default()).unwrap_err().code,
            ErrorCode::TreeCycle
        );
        let mut link = surface.clone();
        link[148..152].copy_from_slice(&1_i32.to_le_bytes());
        assert_eq!(
            parse(&link, Limits::default()).unwrap_err().code,
            ErrorCode::InvalidSurface
        );
        let mut truncated = surface;
        truncated[152..156].copy_from_slice(&((1_u32 << 8) | 5).to_le_bytes());
        assert_eq!(
            parse(&truncated, Limits::default()).unwrap_err().code,
            ErrorCode::InvalidConvex
        );
    }

    #[test]
    fn parses_modern_polygon_geometry_metadata_and_exact_keydata() {
        let collision = stream(&modern_body(0));
        let keydata = b"// retained\nsolid { \"index\" \"0\" \"surfaceprop\" \"metal\" }\0\0";
        let asset = parse_payload(
            &collision,
            keydata,
            1,
            Profile::SourcePcPolygon,
            Limits::default(),
        )
        .unwrap();
        let solid = &asset.solids[0];
        assert_eq!(solid.encoding, Encoding::ModernPolygon);
        assert_eq!(solid.classification, Classification::Handled);
        assert_eq!(
            solid
                .center_source_inches
                .map(|value| f32::from_bits(value.0)),
            [1.0, -1.0, -2.0]
        );
        assert_eq!(
            solid.inertia_source.map(|value| f32::from_bits(value.0)),
            [1.0, 3.0, 2.0]
        );
        assert_eq!(solid.convexes.len(), 1);
        assert_eq!(solid.convexes[0].client_data, 5);
        let geometry = &solid.geometries[solid.convexes[0].geometry];
        assert_eq!(geometry.triangles[0].point_indices, [2, 1, 0]);
        assert_eq!(geometry.triangles[0].material_index, 3);
        assert_eq!(asset.key_data.raw, keydata);
        assert_eq!(asset.key_data.terminator_range.len(), 2);
        assert_eq!(asset.key_data.blocks[0].name, b"solid");
    }

    #[test]
    fn standalone_header_and_unsupported_shape_remain_explicit() {
        let body = modern_body(1);
        let collision = stream(&body);
        let mut bytes = 16_i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&9_i32.to_le_bytes());
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&123_i32.to_le_bytes());
        bytes.extend_from_slice(&collision);
        bytes.push(0);
        let asset = parse_standalone(&bytes, Profile::SourcePcPolygon, Limits::default()).unwrap();
        assert_eq!(asset.header.as_ref().unwrap().checksum, 123);
        assert_eq!(asset.solids[0].shape, ShapeKind::Mopp);
        assert_eq!(asset.solids[0].classification, Classification::Unsupported);
        assert_eq!(asset.solids[0].body, body);
    }

    #[test]
    fn rejects_header_solid_keydata_and_limit_failures() {
        assert_eq!(
            parse_standalone(&[], Profile::SourcePcPolygon, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::TruncatedHeader
        );
        let collision = stream(&modern_body(0));
        assert_eq!(
            parse_payload(
                &collision,
                b"solid {}",
                1,
                Profile::SourcePcPolygon,
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::MissingKeydataTerminator
        );
        assert_eq!(
            parse_payload(
                &collision,
                b"\0",
                1,
                Profile::SourcePcPolygon,
                Limits {
                    max_solid_bytes: 1,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidSolidRange
        );
    }
}
mod fluid;
mod solid;
pub use fluid::FluidProperties;
pub use solid::SolidProperties;
