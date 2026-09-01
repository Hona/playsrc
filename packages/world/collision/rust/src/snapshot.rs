use crate::{
    Error, ErrorCode, Feature, Hit, Hull, Plane, Trace, World, add, dot, error, interpolate, scale,
    sub,
};
use playsrc_phy::{Asset as PhyAsset, Classification as PhyClassification};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;

pub const SNAPSHOT_VERSION: u32 = 4;
const DIST_EPSILON: f32 = 1.0 / 32.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectRole {
    Entity,
    StaticProp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
}
impl Transform {
    pub const IDENTITY: Self = Self {
        origin: [0.0; 3],
        angles: [0.0; 3],
    };

    pub(crate) fn basis(self) -> Result<Basis, Error> {
        if self
            .origin
            .into_iter()
            .chain(self.angles)
            .any(|value| !value.is_finite())
        {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let (sp, cp) = self.angles[0].to_radians().sin_cos();
        let (sy, cy) = self.angles[1].to_radians().sin_cos();
        let (sr, cr) = self.angles[2].to_radians().sin_cos();
        let crcy = cr * cy;
        let crsy = cr * sy;
        let srcy = sr * cy;
        let srsy = sr * sy;
        Ok(Basis {
            columns: [
                [cp * cy, cp * sy, -sp],
                [sp * srcy - crsy, sp * srsy + crcy, sr * cp],
                [sp * crcy + srsy, sp * crsy - srcy, cr * cp],
            ],
            origin: self.origin,
        })
    }

    pub fn transform_point(self, point: [f32; 3]) -> Result<[f32; 3], Error> {
        self.basis().map(|basis| basis.point(point))
    }

    pub fn inverse_transform_point(self, point: [f32; 3]) -> Result<[f32; 3], Error> {
        self.basis().map(|basis| basis.inverse_point(point))
    }

    pub fn compose(self, local: Self) -> Result<Self, Error> {
        self.basis()?;
        local.basis()?;
        let rotation = quaternion_from_angles(self.angles);
        Ok(Self {
            origin: add(self.origin, quaternion_rotate(rotation, local.origin)),
            angles: angles_from_quaternion(quaternion_multiply(
                rotation,
                quaternion_from_angles(local.angles),
            )),
        })
    }

    pub fn relative_to(self, parent: Self) -> Result<Self, Error> {
        self.basis()?;
        parent.basis()?;
        let inverse = quaternion_inverse(quaternion_from_angles(parent.angles));
        Ok(Self {
            origin: quaternion_rotate(inverse, sub(self.origin, parent.origin)),
            angles: angles_from_quaternion(quaternion_multiply(
                inverse,
                quaternion_from_angles(self.angles),
            )),
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Basis {
    columns: [[f32; 3]; 3],
    origin: [f32; 3],
}
impl Basis {
    pub(crate) fn point(self, point: [f32; 3]) -> [f32; 3] {
        add(self.origin, self.vector(point))
    }

    pub(crate) fn vector(self, vector: [f32; 3]) -> [f32; 3] {
        add(
            add(
                scale(self.columns[0], vector[0]),
                scale(self.columns[1], vector[1]),
            ),
            scale(self.columns[2], vector[2]),
        )
    }

    pub(crate) fn inverse_point(self, point: [f32; 3]) -> [f32; 3] {
        self.inverse_vector(sub(point, self.origin))
    }

    pub(crate) fn inverse_vector(self, vector: [f32; 3]) -> [f32; 3] {
        [
            dot(vector, self.columns[0]),
            dot(vector, self.columns[1]),
            dot(vector, self.columns[2]),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pub max_objects: usize,
    pub max_convexes: usize,
    pub max_hierarchy_nodes: usize,
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_axes_per_convex: usize,
    pub max_candidate_visits: usize,
    pub max_ignored_identities: usize,
    pub max_snapshot_bytes: usize,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_objects: 4_096,
            max_convexes: 65_536,
            max_hierarchy_nodes: 65_536,
            max_vertices: 3_000_000,
            max_triangles: 1_000_000,
            max_axes_per_convex: 1_000_000,
            max_candidate_visits: 4_096,
            max_ignored_identities: 4_096,
            max_snapshot_bytes: 16 * 1024 * 1024,
        }
    }
}
impl SnapshotLimits {
    fn validate(self) -> Result<Self, Error> {
        if [
            self.max_objects,
            self.max_convexes,
            self.max_hierarchy_nodes,
            self.max_vertices,
            self.max_triangles,
            self.max_axes_per_convex,
            self.max_candidate_visits,
            self.max_ignored_identities,
            self.max_snapshot_bytes,
        ]
        .contains(&0)
        {
            Err(error(ErrorCode::Limit, None))
        } else {
            Ok(self)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredTriangle {
    pub vertices: [u32; 3],
    pub raw: [u8; 16],
}

impl AuthoredTriangle {
    pub fn metadata(&self) -> u32 {
        u32::from_le_bytes(
            self.raw[..4]
                .try_into()
                .expect("authored triangle metadata"),
        )
    }

    pub fn edge_words(&self) -> [u32; 3] {
        std::array::from_fn(|edge| {
            u32::from_le_bytes(
                self.raw[4 + edge * 4..8 + edge * 4]
                    .try_into()
                    .expect("authored directed edge"),
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredConvex {
    pub raw_header: [u8; 16],
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<AuthoredTriangle>,
}
impl AuthoredConvex {
    fn header_matches_geometry(&self) -> bool {
        let triangles =
            i16::from_le_bytes(self.raw_header[12..14].try_into().expect("triangle count"));
        let size = ((u32::from_le_bytes(self.raw_header[8..12].try_into().expect("geometry size"))
            >> 8) as usize)
            * 16;
        triangles >= 0
            && triangles as usize == self.triangles.len()
            && 16 + self.triangles.len() * 16 <= size
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AuthoredHullRef {
    Piece(usize),
    Enclosure(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredHierarchyNode {
    pub raw: [u8; 28],
    pub children: Option<[usize; 2]>,
    pub hull: Option<AuthoredHullRef>,
}
impl AuthoredHierarchyNode {
    pub fn center(&self) -> [f32; 3] {
        std::array::from_fn(|axis| {
            f32::from_le_bytes(
                self.raw[8 + axis * 4..12 + axis * 4]
                    .try_into()
                    .expect("node center"),
            )
        })
    }
    pub fn radius(&self) -> f32 {
        f32::from_le_bytes(self.raw[20..24].try_into().expect("node radius"))
    }
    pub fn box_sizes(&self) -> [u8; 3] {
        self.raw[24..27].try_into().expect("node bound sizes")
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredEnclosure {
    pub geometry: AuthoredConvex,
    pub subtree: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredHierarchy {
    /// Node zero is the root; child order is authored left, right.
    pub nodes: Vec<AuthoredHierarchyNode>,
    pub enclosures: Vec<AuthoredEnclosure>,
}
impl AuthoredHierarchy {
    fn validate(&self, shape: &PhysicsShape, limits: SnapshotLimits) -> Result<(), Error> {
        if self.nodes.is_empty()
            || self.nodes.len() > limits.max_hierarchy_nodes
            || shape
                .convexes
                .len()
                .checked_add(self.enclosures.len())
                .is_none_or(|count| count > limits.max_convexes)
        {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut vertices = shape.vertex_count;
        let mut triangles = shape.triangle_count;
        for (index, enclosure) in self.enclosures.iter().enumerate() {
            vertices = vertices
                .checked_add(enclosure.geometry.points.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(index)))?;
            triangles = triangles
                .checked_add(enclosure.geometry.triangles.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(index)))?;
            if vertices > limits.max_vertices || triangles > limits.max_triangles {
                return Err(error(ErrorCode::Limit, Some(index)));
            }
            if !enclosure.geometry.header_matches_geometry()
                || enclosure.geometry.points.len() < 4
                || enclosure.geometry.triangles.len() < 4
                || enclosure
                    .geometry
                    .points
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || enclosure.geometry.triangles.iter().any(|triangle| {
                    triangle
                        .vertices
                        .iter()
                        .any(|vertex| *vertex as usize >= enclosure.geometry.points.len())
                })
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(index)));
            }
            let flags = u32::from_le_bytes(
                enclosure.geometry.raw_header[8..12]
                    .try_into()
                    .expect("hull flags"),
            );
            if (flags & 3 != 0) != enclosure.subtree.is_some()
                || enclosure
                    .subtree
                    .is_some_and(|node| node >= self.nodes.len())
            {
                return Err(error(ErrorCode::InvalidReference, Some(index)));
            }
        }
        let mut pieces = BTreeSet::new();
        let mut enclosures = BTreeSet::new();
        let mut states = vec![0_u8; self.nodes.len()];
        let mut pending = vec![(0_usize, false)];
        while let Some((index, exit)) = pending.pop() {
            let node = self
                .nodes
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            if exit {
                states[index] = 2;
                continue;
            }
            if states[index] == 1 {
                return Err(error(ErrorCode::InvalidSnapshot, Some(index)));
            }
            if states[index] == 2 {
                continue;
            }
            states[index] = 1;
            pending.push((index, true));
            if node.center().iter().any(|value| !value.is_finite())
                || !node.radius().is_finite()
                || node.radius() < 0.0
                || (i32::from_le_bytes(node.raw[..4].try_into().expect("right child")) != 0)
                    != node.children.is_some()
                || (i32::from_le_bytes(node.raw[4..8].try_into().expect("hull offset")) != 0)
                    != node.hull.is_some()
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(index)));
            }
            if let Some(hull) = node.hull {
                match hull {
                    AuthoredHullRef::Piece(piece) => {
                        if piece >= shape.convexes.len() {
                            return Err(error(ErrorCode::InvalidReference, Some(index)));
                        }
                        if node.children.is_none() {
                            pieces.insert(piece);
                        }
                    }
                    AuthoredHullRef::Enclosure(enclosure) => {
                        if enclosure >= self.enclosures.len() {
                            return Err(error(ErrorCode::InvalidReference, Some(index)));
                        }
                        enclosures.insert(enclosure);
                    }
                }
            }
            if let Some([left, right]) = node.children {
                pending.push((left, false));
                pending.push((right, false));
            } else if !matches!(node.hull, Some(AuthoredHullRef::Piece(_))) {
                return Err(error(ErrorCode::InvalidSnapshot, Some(index)));
            }
        }
        if states.contains(&0)
            || pieces.len() != shape.convexes.len()
            || enclosures.len() != self.enclosures.len()
        {
            return Err(error(ErrorCode::InvalidReference, None));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredShapeProperties {
    pub center: [f32; 3],
    pub inertia: [f32; 3],
    pub radius: f32,
    pub max_surface_deviation: u8,
    pub drag_axes: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConvexInput {
    pub solid: usize,
    pub convex: usize,
    pub contents: u32,
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub authored: Option<AuthoredConvex>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsShape {
    pub identity: u64,
    convexes: Vec<Convex>,
    bounds: Hull,
    contents: u32,
    vertex_count: usize,
    triangle_count: usize,
    authored_properties: Option<AuthoredShapeProperties>,
    authored_hierarchy: Option<AuthoredHierarchy>,
}
impl PhysicsShape {
    pub fn compile(
        identity: u64,
        inputs: Vec<ConvexInput>,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if inputs.is_empty() || inputs.len() > limits.max_convexes {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut vertex_count = 0_usize;
        let mut triangle_count = 0_usize;
        let mut convexes = Vec::with_capacity(inputs.len());
        for (item, input) in inputs.into_iter().enumerate() {
            vertex_count = vertex_count
                .checked_add(input.vertices.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            triangle_count = triangle_count
                .checked_add(input.triangles.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            if vertex_count > limits.max_vertices
                || triangle_count > limits.max_triangles
                || input.vertices.len() < 4
                || input.triangles.len() < 4
                || input
                    .vertices
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || input.authored.as_ref().is_some_and(|authored| {
                    !authored.header_matches_geometry()
                        || u32::from_le_bytes(
                            authored.raw_header[8..12].try_into().expect("convex flags"),
                        ) & 3
                            != 0
                        || authored.points.len() != input.vertices.len()
                        || authored.triangles.len() != input.triangles.len()
                        || authored
                            .points
                            .iter()
                            .flatten()
                            .any(|value| !value.is_finite())
                        || authored
                            .triangles
                            .iter()
                            .zip(&input.triangles)
                            .any(|(authored, indexes)| authored.vertices != *indexes)
                })
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
            }
            let center = scale(
                input.vertices.iter().copied().fold([0.0; 3], add),
                1.0 / input.vertices.len() as f32,
            );
            let mut faces = Vec::with_capacity(input.triangles.len());
            let edge_capacity = input
                .triangles
                .len()
                .checked_mul(3)
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            let mut edges = Vec::with_capacity(edge_capacity);
            for (triangle, indexes) in input.triangles.iter().copied().enumerate() {
                let [a, b, c] = indexes.map(|index| {
                    input
                        .vertices
                        .get(index as usize)
                        .copied()
                        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(item)))
                });
                let (a, b, c) = (a?, b?, c?);
                let mut normal = cross(sub(b, a), sub(c, a));
                let length = length(normal);
                if !length.is_finite() || length <= f32::EPSILON {
                    return Err(error(ErrorCode::Unsupported, Some(item)));
                }
                normal = scale(normal, 1.0 / length);
                if dot(normal, sub(center, a)) > 0.0 {
                    normal = scale(normal, -1.0);
                }
                faces.push(Face { normal, triangle });
                edges.extend([sub(b, a), sub(c, b), sub(a, c)]);
            }
            let axis_count = faces
                .len()
                .checked_add(3)
                .and_then(|value| {
                    edges
                        .len()
                        .checked_mul(3)
                        .and_then(|edges| value.checked_add(edges))
                })
                .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
            if axis_count > limits.max_axes_per_convex {
                return Err(error(ErrorCode::Limit, Some(item)));
            }
            convexes.push(Convex {
                solid: input.solid,
                convex: input.convex,
                contents: input.contents,
                vertices: input.vertices,
                authored: input.authored,
                faces,
                edges,
            });
        }
        let mut bounds = Hull {
            mins: [f32::INFINITY; 3],
            maxs: [f32::NEG_INFINITY; 3],
        };
        for vertex in convexes.iter().flat_map(|convex| &convex.vertices) {
            for (axis, coordinate) in vertex.iter().enumerate() {
                bounds.mins[axis] = bounds.mins[axis].min(*coordinate);
                bounds.maxs[axis] = bounds.maxs[axis].max(*coordinate);
            }
        }
        let contents = convexes
            .iter()
            .fold(0, |contents, convex| contents | convex.contents);
        Ok(Self {
            identity,
            convexes,
            bounds,
            contents,
            vertex_count,
            triangle_count,
            authored_properties: None,
            authored_hierarchy: None,
        })
    }

    pub fn from_phy(
        identity: u64,
        asset: &PhyAsset,
        solid: usize,
        limits: SnapshotLimits,
        mut contents_for_game_data: impl FnMut(i32) -> u32,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        let source = asset
            .solids
            .get(solid)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(solid)))?;
        if source.classification != PhyClassification::Handled {
            return Err(error(ErrorCode::Unsupported, Some(solid)));
        }
        let source_tree = source
            .hierarchy
            .as_ref()
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(solid)))?;
        if source.geometries.len() > limits.max_convexes
            || source_tree.nodes.len() > limits.max_hierarchy_nodes
        {
            return Err(error(ErrorCode::Limit, Some(solid)));
        }
        let mut vertices = 0_usize;
        let mut triangles = 0_usize;
        for geometry in &source.geometries {
            vertices = vertices
                .checked_add(geometry.points.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(solid)))?;
            triangles = triangles
                .checked_add(geometry.triangles.len())
                .ok_or_else(|| error(ErrorCode::Limit, Some(solid)))?;
            if vertices > limits.max_vertices || triangles > limits.max_triangles {
                return Err(error(ErrorCode::Limit, Some(solid)));
            }
        }
        let authored = |value: &playsrc_phy::ConvexGeometry| AuthoredConvex {
            raw_header: value.raw_header,
            points: value
                .points
                .iter()
                .map(|point| point.source_bits.map(|axis| f32::from_bits(axis.0)))
                .collect(),
            triangles: value
                .triangles
                .iter()
                .map(|triangle| AuthoredTriangle {
                    vertices: triangle.point_indices,
                    raw: triangle.raw,
                })
                .collect(),
        };
        let mut references = BTreeMap::new();
        let inputs = source
            .convexes
            .iter()
            .enumerate()
            .map(|(convex, value)| {
                let geometry = source
                    .geometries
                    .get(value.geometry)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(convex)))?;
                if references
                    .insert(value.geometry, AuthoredHullRef::Piece(convex))
                    .is_some()
                {
                    return Err(error(ErrorCode::InvalidReference, Some(convex)));
                }
                Ok(ConvexInput {
                    solid,
                    convex,
                    contents: contents_for_game_data(value.client_data),
                    vertices: geometry
                        .points
                        .iter()
                        .map(|point| point.source_inches.map(|axis| f32::from_bits(axis.0)))
                        .collect(),
                    triangles: geometry
                        .triangles
                        .iter()
                        .map(|triangle| triangle.point_indices)
                        .collect(),
                    authored: Some(authored(geometry)),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut hierarchy = AuthoredHierarchy {
            nodes: Vec::new(),
            enclosures: Vec::new(),
        };
        for (index, geometry) in source.geometries.iter().enumerate() {
            if references.contains_key(&index) {
                continue;
            }
            references.insert(
                index,
                AuthoredHullRef::Enclosure(hierarchy.enclosures.len()),
            );
            hierarchy.enclosures.push(AuthoredEnclosure {
                geometry: authored(geometry),
                subtree: geometry.subtree,
            });
        }
        hierarchy.nodes = source_tree
            .nodes
            .iter()
            .map(|node| {
                Ok(AuthoredHierarchyNode {
                    raw: node.raw,
                    children: node.children,
                    hull: node
                        .hull
                        .map(|index| {
                            references
                                .get(&index)
                                .copied()
                                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(solid)))
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<_, Error>>()?;
        Self::compile_authored(
            identity,
            inputs,
            AuthoredShapeProperties {
                center: source
                    .center_bits
                    .map(|component| f32::from_bits(component.0)),
                inertia: source
                    .inertia_bits
                    .map(|component| f32::from_bits(component.0)),
                radius: f32::from_bits(source.radius_bits.0),
                max_surface_deviation: source.max_surface_deviation,
                drag_axes: source
                    .drag_axis_bits
                    .map(|axes| axes.map(|component| f32::from_bits(component.0))),
            },
            hierarchy,
            limits,
        )
    }

    pub fn compile_authored(
        identity: u64,
        inputs: Vec<ConvexInput>,
        properties: AuthoredShapeProperties,
        hierarchy: AuthoredHierarchy,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        if properties
            .center
            .iter()
            .chain(properties.inertia.iter())
            .chain(properties.drag_axes.iter().flatten())
            .any(|component| !component.is_finite())
            || !properties.radius.is_finite()
            || inputs.iter().any(|input| input.authored.is_none())
        {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let mut shape = Self::compile(identity, inputs, limits)?;
        hierarchy.validate(&shape, limits)?;
        shape.authored_properties = Some(properties);
        shape.authored_hierarchy = Some(hierarchy);
        Ok(shape)
    }

    fn contents(&self) -> u32 {
        self.contents
    }

    fn counts(&self) -> (usize, usize, usize) {
        (self.convexes.len(), self.vertex_count, self.triangle_count)
    }

    pub fn convex_count(&self) -> usize {
        self.convexes.len()
    }

    pub fn convex_contents(&self, convex: usize) -> Option<u32> {
        self.convexes.get(convex).map(|convex| convex.contents)
    }

    pub fn authored_convex(&self, convex: usize) -> Option<&AuthoredConvex> {
        self.convexes.get(convex)?.authored.as_ref()
    }

    pub fn authored_properties(&self) -> Option<AuthoredShapeProperties> {
        self.authored_properties
    }

    pub fn authored_hierarchy(&self) -> Option<&AuthoredHierarchy> {
        self.authored_hierarchy.as_ref()
    }

    pub fn authored_hull(&self, reference: AuthoredHullRef) -> Option<&AuthoredConvex> {
        match reference {
            AuthoredHullRef::Piece(index) => self.authored_convex(index),
            AuthoredHullRef::Enclosure(index) => Some(
                &self
                    .authored_hierarchy
                    .as_ref()?
                    .enclosures
                    .get(index)?
                    .geometry,
            ),
        }
    }

    pub fn local_bounds(&self) -> Hull {
        self.bounds
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Face {
    normal: [f32; 3],
    triangle: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct Convex {
    solid: usize,
    convex: usize,
    contents: u32,
    vertices: Vec<[f32; 3]>,
    authored: Option<AuthoredConvex>,
    faces: Vec<Face>,
    edges: Vec<[f32; 3]>,
}

pub trait PhysicsQuery: std::fmt::Debug + Send + Sync {
    fn geometry(&self) -> &PhysicsShape;
    fn bounds(&self, transform: Transform) -> Result<Hull, Error>;
    fn trace(&self, request: ObjectTraceRequest) -> Result<(crate::BoundsTrace, Option<usize>), Error>;
    fn storage_bytes(&self) -> usize;
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct FixturePhysicsQuery {
    pub geometry: Arc<PhysicsShape>,
    pub calls: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl PhysicsQuery for FixturePhysicsQuery {
    fn geometry(&self) -> &PhysicsShape { &self.geometry }
    fn bounds(&self, transform: Transform) -> Result<Hull, Error> {
        Ok(transformed_bounds(self.geometry.local_bounds(), transform.basis()?))
    }
    fn trace(&self, request: ObjectTraceRequest) -> Result<(crate::BoundsTrace, Option<usize>), Error> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut trace = crate::trace_bounds(request.start, request.end, request.hull, self.bounds(request.transform)?)?;
        let hit = trace.fraction < 1.0 || trace.start_solid || trace.all_solid;
        trace.contents = if hit { self.geometry.contents() } else { 0 };
        Ok((trace, hit.then_some(0)))
    }
    fn storage_bytes(&self) -> usize { 0 }
}

#[derive(Clone, Debug)]
pub enum SnapshotShape {
    BrushModel { model: usize },
    BoundingBox { bounds: Hull },
    OrientedBox { bounds: Hull },
    Physics(Arc<dyn PhysicsQuery>),
    Follower { parent: u64, query: Arc<dyn PhysicsQuery> },
}

impl PartialEq for SnapshotShape {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BrushModel { model: a }, Self::BrushModel { model: b }) => a == b,
            (Self::BoundingBox { bounds: a }, Self::BoundingBox { bounds: b })
            | (Self::OrientedBox { bounds: a }, Self::OrientedBox { bounds: b }) => a == b,
            (Self::Physics(a), Self::Physics(b)) => Arc::ptr_eq(a, b) || a.geometry() == b.geometry(),
            (Self::Follower { parent: a, query: left }, Self::Follower { parent: b, query: right }) => a == b && (Arc::ptr_eq(left, right) || left.geometry() == right.geometry()),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectInput {
    pub identity: u64,
    pub role: ObjectRole,
    pub enabled: bool,
    pub volume_contents: bool,
    pub transform: Transform,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub collision_group: i32,
    pub contents: u32,
    pub surface_flags: u16,
    pub shape: SnapshotShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotRecord {
    pub identity: u64,
    pub role: ObjectRole,
    pub enabled: bool,
    pub volume_contents: bool,
    pub transform: Transform,
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub collision_group: i32,
    pub contents: u32,
    pub surface_flags: u16,
    pub shape: SnapshotShape,
    pub bounds: Hull,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    world: [u8; 32],
    identity: u64,
    objects: Arc<[SnapshotRecord]>,
    broadphase: Arc<[BroadphaseNode]>,
    order: Arc<[usize]>,
    limits: SnapshotLimits,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BroadphaseNode {
    bounds: Hull,
    start: usize,
    end: usize,
    right: usize,
}

impl BroadphaseNode {
    fn build(
        objects: &[SnapshotRecord],
        order: &mut [usize],
        nodes: &mut Vec<Self>,
        start: usize,
        end: usize,
    ) -> usize {
        let identity = nodes.len();
        let mut bounds = objects[order[start]].bounds;
        for &index in &order[start + 1..end] {
            let object = &objects[index];
            for axis in 0..3 {
                bounds.mins[axis] = bounds.mins[axis].min(object.bounds.mins[axis]);
                bounds.maxs[axis] = bounds.maxs[axis].max(object.bounds.maxs[axis]);
            }
        }
        nodes.push(Self {
            bounds,
            start,
            end,
            right: 0,
        });
        if end - start > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.maxs[a] - bounds.mins[a]).total_cmp(&(bounds.maxs[b] - bounds.mins[b]))
                })
                .unwrap();
            order[start..end].sort_unstable_by(|&a, &b| {
                let center = |index: usize| {
                    objects[index].bounds.mins[axis] * 0.5 + objects[index].bounds.maxs[axis] * 0.5
                };
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let middle = start + (end - start) / 2;
            Self::build(objects, order, nodes, start, middle);
            nodes[identity].right = Self::build(objects, order, nodes, middle, end);
        }
        identity
    }
}

#[derive(Default)]
pub struct QueryScratch {
    candidates: Vec<usize>,
    brushes: super::BrushTraceScratch,
}

impl QueryScratch {
    /// Retained vector capacity, not allocator overhead or process resident memory.
    pub fn storage_bytes(&self) -> usize {
        self.candidates.capacity() * std::mem::size_of::<usize>()
            + self.brushes.ordered.capacity() * std::mem::size_of::<usize>()
            + self.brushes.pending.capacity() * std::mem::size_of::<(i32, [f32; 3], [f32; 3], usize)>()
    }
}

fn bounds_intersect(left: Hull, right: Hull) -> bool {
    (0..3).all(|axis| left.mins[axis] <= right.maxs[axis] && left.maxs[axis] >= right.mins[axis])
}

impl Snapshot {
    pub fn compile(
        world: &World,
        identity: u64,
        inputs: Vec<ObjectInput>,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        Self::compile_inner(world, identity, inputs, limits)
    }

    pub fn recompile(
        &self,
        world: &World,
        identity: u64,
        inputs: Vec<ObjectInput>,
    ) -> Result<Self, Error> {
        if self.world != world.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        Self::compile_inner(world, identity, inputs, self.limits)
    }

    fn compile_inner(
        world: &World,
        identity: u64,
        inputs: Vec<ObjectInput>,
        limits: SnapshotLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if inputs.len() > limits.max_objects {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut identities = BTreeSet::new();
        let mut objects = Vec::with_capacity(inputs.len());
        let mut convex_count = 0_usize;
        let mut vertex_count = 0_usize;
        let mut triangle_count = 0_usize;
        for (item, input) in inputs.into_iter().enumerate() {
            if !identities.insert(input.identity) {
                return Err(error(ErrorCode::DuplicateIdentity, Some(item)));
            }
            let basis = input.transform.basis()?;
            if input
                .linear_velocity
                .into_iter()
                .chain(input.angular_velocity)
                .any(|value| !value.is_finite())
            {
                return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
            }
            let (contents, local_bounds) = match &input.shape {
                SnapshotShape::BrushModel { model } => {
                    let model_record = world
                        .models
                        .get(*model)
                        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(item)))?;
                    let brushes = world
                        .model_brushes
                        .get(*model)
                        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(item)))?;
                    let contents = brushes
                        .iter()
                        .fold(0, |value, brush| value | world.brushes[*brush].contents);
                    (
                        contents,
                        Hull {
                            mins: [
                                model_record.mins.x.value(),
                                model_record.mins.y.value(),
                                model_record.mins.z.value(),
                            ],
                            maxs: [
                                model_record.maxs.x.value(),
                                model_record.maxs.y.value(),
                                model_record.maxs.z.value(),
                            ],
                        },
                    )
                }
                SnapshotShape::BoundingBox { bounds } => {
                    validate_hull(*bounds, item)?;
                    if input.transform.angles != [0.0; 3] {
                        return Err(error(ErrorCode::InvalidSnapshot, Some(item)));
                    }
                    (input.contents, *bounds)
                }
                SnapshotShape::OrientedBox { bounds } => {
                    validate_hull(*bounds, item)?;
                    (input.contents, *bounds)
                }
                SnapshotShape::Physics(shape) | SnapshotShape::Follower { query: shape, .. } => {
                    let counts = shape.geometry().counts();
                    convex_count = convex_count
                        .checked_add(counts.0)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    vertex_count = vertex_count
                        .checked_add(counts.1)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    triangle_count = triangle_count
                        .checked_add(counts.2)
                        .ok_or_else(|| error(ErrorCode::Limit, Some(item)))?;
                    if convex_count > limits.max_convexes
                        || vertex_count > limits.max_vertices
                        || triangle_count > limits.max_triangles
                    {
                        return Err(error(ErrorCode::Limit, Some(item)));
                    }
                    (shape.geometry().contents(), shape.geometry().local_bounds())
                }
            };
            let bounds = match &input.shape {
                SnapshotShape::Physics(shape) | SnapshotShape::Follower { query: shape, .. } => {
                    let bounds = shape.bounds(input.transform)?;
                    Hull { mins: bounds.mins.map(|value| value - DIST_EPSILON), maxs: bounds.maxs.map(|value| value + DIST_EPSILON) }
                }
                _ => transformed_bounds(local_bounds, basis),
            };
            objects.push(SnapshotRecord {
                identity: input.identity,
                role: input.role,
                enabled: input.enabled,
                volume_contents: input.volume_contents,
                transform: input.transform,
                linear_velocity: input.linear_velocity,
                angular_velocity: input.angular_velocity,
                collision_group: input.collision_group,
                contents,
                surface_flags: input.surface_flags,
                shape: input.shape,
                bounds,
            });
        }
        let mut broadphase = Vec::new();
        let mut order = (0..objects.len()).collect::<Vec<_>>();
        if !objects.is_empty() {
            BroadphaseNode::build(&objects, &mut order, &mut broadphase, 0, objects.len());
        }
        Ok(Self {
            world: world.identity,
            identity,
            objects: objects.into(),
            broadphase: broadphase.into(),
            order: order.into(),
            limits,
        })
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn with_identity(&self, identity: u64) -> Self {
        Self {
            world: self.world,
            identity,
            objects: Arc::clone(&self.objects),
            broadphase: Arc::clone(&self.broadphase),
            order: Arc::clone(&self.order),
            limits: self.limits,
        }
    }

    pub fn world_identity(&self) -> [u8; 32] {
        self.world
    }

    pub fn records(&self) -> &[SnapshotRecord] {
        &self.objects
    }

    /// Shared immutable physical-query storage, excluding geometry and allocator overhead.
    pub fn physics_query_storage_bytes(&self) -> usize {
        let mut seen = BTreeSet::new();
        self.objects.iter().filter_map(|object| {
            let query = match &object.shape { SnapshotShape::Physics(query) | SnapshotShape::Follower { query, .. } => query, _ => return None };
            seen.insert(Arc::as_ptr(query) as *const () as usize).then(|| query.storage_bytes())
        }).sum()
    }

    pub fn object_transform(&self, identity: u64) -> Option<Transform> {
        self.object(identity).map(|object| object.transform)
    }

    pub fn object_velocity(&self, identity: u64) -> Option<([f32; 3], [f32; 3])> {
        self.object(identity)
            .map(|object| (object.linear_velocity, object.angular_velocity))
    }

    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut output = BoundedBytes::new(self.limits.max_snapshot_bytes);
        output.bytes(b"CSNP")?;
        output.u32(SNAPSHOT_VERSION)?;
        output.bytes(&self.world)?;
        output.u64(self.identity)?;
        output
            .u32(u32::try_from(self.objects.len()).map_err(|_| error(ErrorCode::Limit, None))?)?;
        for object in self.objects.iter() {
            output.u64(object.identity)?;
            output.u8(match object.role {
                ObjectRole::Entity => 0,
                ObjectRole::StaticProp => 1,
            })?;
            output.u8(u8::from(object.enabled) | (u8::from(object.volume_contents) << 1))?;
            output.u16(object.surface_flags)?;
            output.i32(object.collision_group)?;
            output.u32(object.contents)?;
            for value in object
                .transform
                .origin
                .into_iter()
                .chain(object.transform.angles)
                .chain(object.linear_velocity)
                .chain(object.angular_velocity)
            {
                output.f32(value)?;
            }
            match &object.shape {
                SnapshotShape::BrushModel { model } => {
                    output.u8(0)?;
                    output.u64(*model as u64)?;
                }
                SnapshotShape::BoundingBox { bounds } => {
                    output.u8(1)?;
                    output.hull(*bounds)?;
                }
                SnapshotShape::OrientedBox { bounds } => {
                    output.u8(2)?;
                    output.hull(*bounds)?;
                }
                SnapshotShape::Physics(shape) => {
                    output.u8(3)?;
                    output.u64(shape.geometry().identity)?;
                    output.u32(
                        u32::try_from(shape.geometry().convexes.len())
                            .map_err(|_| error(ErrorCode::Limit, None))?,
                    )?;
                }
                SnapshotShape::Follower { parent, query } => {
                    output.u8(4)?;
                    output.u64(*parent)?;
                    output.u64(query.geometry().identity)?;
                    output.u32(u32::try_from(query.geometry().convex_count()).map_err(|_| error(ErrorCode::Limit, None))?)?;
                }
            }
        }
        Ok(output.finish())
    }

    fn object(&self, identity: u64) -> Option<&SnapshotRecord> {
        self.objects
            .iter()
            .find(|object| object.identity == identity)
    }

    fn candidate_indices(&self, bounds: Hull, scratch: &mut QueryScratch) {
        #[cfg(feature = "replay-reference")]
        crate::replay_diagnostics::count(0, 1);
        fn visit(snapshot: &Snapshot, identity: usize, bounds: Hull, output: &mut Vec<usize>) {
            let node = snapshot.broadphase[identity];
            #[cfg(feature = "replay-reference")]
            crate::replay_diagnostics::count(1, 1);
            if !bounds_intersect(bounds, node.bounds) {
                return;
            }
            if node.right == 0 {
                for &index in &snapshot.order[node.start..node.end] {
                    if bounds_intersect(bounds, snapshot.objects[index].bounds) {
                        output.push(index);
                    }
                }
            } else {
                visit(snapshot, identity + 1, bounds, output);
                visit(snapshot, node.right, bounds, output);
            }
        }
        scratch.candidates.clear();
        if !self.broadphase.is_empty() {
            visit(self, 0, bounds, &mut scratch.candidates);
        }
        // The hierarchy is spatial; callbacks, limits and equal-hit selection
        // still consume the exact authored order, never traversal order.
        scratch.candidates.sort_unstable();
        #[cfg(feature = "replay-reference")]
        crate::replay_diagnostics::count(2, scratch.candidates.len());
    }

    fn candidates(&self, bounds: Hull) -> impl Iterator<Item = &SnapshotRecord> {
        let mut scratch = QueryScratch::default();
        self.candidate_indices(bounds, &mut scratch);
        scratch
            .candidates
            .into_iter()
            .map(|index| &self.objects[index])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceScope {
    Everything,
    WorldOnly,
    EntitiesOnly,
    EverythingFilterProps,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotTraceRequest<'a> {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
    pub scope: TraceScope,
    pub ignored: &'a [u64],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapshotRayRequest<'a> {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub mask: u32,
    pub scope: TraceScope,
    pub ignored: &'a [u64],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectTraceRequest {
    pub identity: u64,
    pub transform: Transform,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectOverlapRequest {
    pub identity: u64,
    pub transform: Transform,
    pub position: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointContentsContributor {
    WorldLeaf { leaf: usize },
    WorldBrush { brush: usize },
    Object { identity: u64, role: ObjectRole },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointContentsResult {
    pub world: [u8; 32],
    pub snapshot: Option<u64>,
    pub contents: u32,
    pub entity: u64,
    pub contributors: Vec<PointContentsContributor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub identity: u64,
    pub role: ObjectRole,
    pub collision_group: i32,
    pub contents: u32,
}

impl World {
    pub fn point_contents(&self, point: [f32; 3]) -> Result<PointContentsResult, Error> {
        self.point_contents_headnode(
            point,
            self.models.first().map(|model| model.head_node),
            true,
        )
    }

    pub fn point_contents_snapshot(
        &self,
        snapshot: &Snapshot,
        point: [f32; 3],
    ) -> Result<PointContentsResult, Error> {
        self.point_contents_snapshot_inner(snapshot, point, true)
    }

    pub fn point_contents_snapshot_value(
        &self,
        snapshot: &Snapshot,
        point: [f32; 3],
    ) -> Result<u32, Error> {
        self.point_contents_snapshot_inner(snapshot, point, false)
            .map(|result| result.contents)
    }

    fn point_contents_snapshot_inner(
        &self,
        snapshot: &Snapshot,
        point: [f32; 3],
        collect_contributors: bool,
    ) -> Result<PointContentsResult, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let mut result = self.point_contents_headnode(
            point,
            self.models.first().map(|model| model.head_node),
            collect_contributors,
        )?;
        result.snapshot = Some(snapshot.identity);
        if result.contents & crate::MASK_CURRENT != 0 {
            result.contents = crate::CONTENTS_WATER;
        }
        if result.contents == crate::CONTENTS_SOLID {
            return Ok(result);
        }

        let mut visits = 0_usize;
        for object in snapshot.candidates(Hull {
            mins: point,
            maxs: point,
        }) {
            if !object.enabled
                || object.role == ObjectRole::Entity && !object.volume_contents
                || point
                    .into_iter()
                    .zip(object.bounds.mins.into_iter().zip(object.bounds.maxs))
                    .any(|(value, (minimum, maximum))| value < minimum || value > maximum)
            {
                continue;
            }
            visits += 1;
            if visits > snapshot.limits.max_candidate_visits {
                return Err(error(ErrorCode::Limit, Some(visits)));
            }
            let Some(contents) =
                self.point_contents_object_record(object, point, snapshot.limits)?
            else {
                continue;
            };
            result.contents = contents;
            result.entity = if object.role == ObjectRole::StaticProp {
                0
            } else {
                object.identity
            };
            if collect_contributors {
                result.contributors.push(PointContentsContributor::Object {
                    identity: object.identity,
                    role: object.role,
                });
            }
            return Ok(result);
        }
        Ok(result)
    }

    pub fn point_contents_object(
        &self,
        snapshot: &Snapshot,
        identity: u64,
        point: [f32; 3],
    ) -> Result<PointContentsResult, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        if point.into_iter().any(|value| !value.is_finite()) {
            return Err(error(ErrorCode::NonFinite, None));
        }
        let object = snapshot
            .object(identity)
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        let contents = if object.enabled
            && (object.role == ObjectRole::StaticProp || object.volume_contents)
        {
            self.point_contents_object_record(object, point, snapshot.limits)?
                .unwrap_or(0)
        } else {
            0
        };
        Ok(PointContentsResult {
            world: self.identity,
            snapshot: Some(snapshot.identity),
            contents,
            entity: if contents == 0 || object.role == ObjectRole::StaticProp {
                0
            } else {
                object.identity
            },
            contributors: if contents == 0 {
                Vec::new()
            } else {
                vec![PointContentsContributor::Object {
                    identity: object.identity,
                    role: object.role,
                }]
            },
        })
    }

    fn point_contents_headnode(
        &self,
        point: [f32; 3],
        head_node: Option<i32>,
        collect_contributors: bool,
    ) -> Result<PointContentsResult, Error> {
        if point.into_iter().any(|value| !value.is_finite()) {
            return Err(error(ErrorCode::NonFinite, None));
        }
        let Some(mut child) = head_node else {
            return Ok(PointContentsResult {
                world: self.identity,
                snapshot: None,
                contents: 0,
                entity: 0,
                contributors: Vec::new(),
            });
        };
        let mut depth = 0;
        while child >= 0 {
            depth += 1;
            if depth > self.nodes.len() {
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
            child = node.children[usize::from(dot(point, plane.normal) < plane.distance)];
        }
        let leaf_index = (-1_i64 - i64::from(child)) as usize;
        let leaf = self
            .leaves
            .get(leaf_index)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?;
        let mut result = PointContentsResult {
            world: self.identity,
            snapshot: None,
            contents: leaf.contents as u32,
            entity: 0,
            contributors: if collect_contributors {
                vec![PointContentsContributor::WorldLeaf { leaf: leaf_index }]
            } else {
                Vec::new()
            },
        };
        let begin = leaf.first_leaf_brush as usize;
        let finish = begin + leaf.leaf_brush_count as usize;
        for &index in self
            .leaf_brushes
            .get(begin..finish)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(leaf_index)))?
        {
            let index = usize::from(index);
            let brush = self
                .brushes
                .get(index)
                .ok_or_else(|| error(ErrorCode::InvalidReference, Some(index)))?;
            if brush.side_count == 0 {
                continue;
            }
            let inside = self.sides[brush.first_side..brush.first_side + brush.side_count]
                .iter()
                .all(|side| {
                    let plane = self.planes[side.plane];
                    dot(point, plane.normal) - plane.distance <= 0.0
                });
            if inside {
                result.contents |= brush.contents;
                if collect_contributors {
                    result
                        .contributors
                        .push(PointContentsContributor::WorldBrush { brush: index });
                }
            }
        }
        Ok(result)
    }

    fn point_contents_object_record(
        &self,
        object: &SnapshotRecord,
        point: [f32; 3],
        limits: SnapshotLimits,
    ) -> Result<Option<u32>, Error> {
        match (&object.role, &object.shape) {
            (ObjectRole::StaticProp, _) => {
                let trace = self.trace_object(
                    object,
                    ObjectTraceRequest {
                        identity: object.identity,
                        transform: object.transform,
                        start: point,
                        end: point,
                        hull: Hull {
                            mins: [0.0; 3],
                            maxs: [0.0; 3],
                        },
                        mask: u32::MAX,
                    },
                    limits,
                )?;
                Ok(trace.start_solid.then_some(crate::CONTENTS_SOLID))
            }
            (ObjectRole::Entity, SnapshotShape::BrushModel { model }) => {
                let model = self
                    .models
                    .get(*model)
                    .ok_or_else(|| error(ErrorCode::InvalidReference, Some(*model)))?;
                let local = object.transform.inverse_transform_point(point)?;
                let contents = self
                    .point_contents_headnode(local, Some(model.head_node), false)?
                    .contents;
                Ok((contents != 0).then_some(contents))
            }
            (ObjectRole::Entity, _) => Ok(None),
        }
    }

    pub fn trace_snapshot_hull(
        &self,
        snapshot: &Snapshot,
        request: SnapshotTraceRequest<'_>,
        should_hit: impl Fn(Candidate) -> bool,
    ) -> Result<Trace, Error> {
        self.trace_snapshot_hull_with_scratch(
            snapshot,
            request,
            &mut QueryScratch::default(),
            should_hit,
        )
    }

    pub fn trace_snapshot_hull_with_scratch(
        &self,
        snapshot: &Snapshot,
        request: SnapshotTraceRequest<'_>,
        scratch: &mut QueryScratch,
        should_hit: impl Fn(Candidate) -> bool,
    ) -> Result<Trace, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        if request.ignored.len() > snapshot.limits.max_ignored_identities {
            return Err(error(ErrorCode::Limit, None));
        }
        let mut world_trace = if request.scope == TraceScope::EntitiesOnly {
            miss(request.start, request.end)
        } else {
            self.trace_hull_with_scratch(request.start, request.end, request.hull, request.mask, &mut scratch.brushes)?
        };
        world_trace.world = self.identity;
        world_trace.snapshot = Some(snapshot.identity);
        if world_trace.start_solid || request.scope == TraceScope::WorldOnly {
            return Ok(world_trace);
        }
        let world_fraction = world_trace.fraction;
        let dynamic_end = world_trace.end;
        let mut object_trace = miss(request.start, dynamic_end);
        object_trace.world = self.identity;
        let mut visits = 0_usize;
        let swept_bounds = Hull {
            mins: std::array::from_fn(|axis| {
                request.start[axis].min(dynamic_end[axis]) + request.hull.mins[axis]
            }),
            maxs: std::array::from_fn(|axis| {
                request.start[axis].max(dynamic_end[axis]) + request.hull.maxs[axis]
            }),
        };
        snapshot.candidate_indices(swept_bounds, scratch);
        for &index in &scratch.candidates {
            let object = &snapshot.objects[index];
            if !object.enabled
                || request.scope == TraceScope::EntitiesOnly
                    && object.role == ObjectRole::StaticProp
                || request.ignored.contains(&object.identity)
                || matches!(&object.shape, SnapshotShape::Follower { parent, .. } if request.ignored.contains(parent))
                || object.contents & request.mask == 0
            {
                continue;
            }
            if !swept_hull_intersects(request.start, dynamic_end, request.hull, object.bounds) {
                continue;
            }
            let candidate = Candidate {
                identity: object.identity,
                role: object.role,
                collision_group: object.collision_group,
                contents: object.contents,
            };
            if (object.role != ObjectRole::StaticProp
                || request.scope == TraceScope::EverythingFilterProps)
                && !should_hit(candidate)
            {
                continue;
            }
            visits += 1;
            if visits > snapshot.limits.max_candidate_visits {
                return Err(error(ErrorCode::Limit, Some(visits)));
            }
            let candidate = self.trace_object(
                object,
                ObjectTraceRequest {
                    identity: object.identity,
                    transform: object.transform,
                    start: request.start,
                    end: dynamic_end,
                    hull: request.hull,
                    mask: request.mask,
                },
                snapshot.limits,
            )?;
            merge(&mut object_trace, candidate);
            if object_trace.all_solid {
                break;
            }
        }
        if object_trace.hit.is_none() && !object_trace.start_solid && !object_trace.all_solid {
            return Ok(world_trace);
        }
        object_trace.fraction *= world_fraction;
        object_trace.fraction_left_solid *= world_fraction;
        object_trace.end = interpolate(request.start, request.end, object_trace.fraction);
        object_trace.snapshot = Some(snapshot.identity);
        Ok(object_trace)
    }

    pub fn trace_snapshot_ray(
        &self,
        snapshot: &Snapshot,
        request: SnapshotRayRequest<'_>,
        should_hit: impl Fn(Candidate) -> bool,
    ) -> Result<Trace, Error> {
        self.trace_snapshot_hull(
            snapshot,
            SnapshotTraceRequest {
                start: request.start,
                end: request.end,
                hull: Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                mask: request.mask,
                scope: request.scope,
                ignored: request.ignored,
            },
            should_hit,
        )
    }

    pub fn trace_object_hull_at(
        &self,
        snapshot: &Snapshot,
        request: ObjectTraceRequest,
    ) -> Result<Trace, Error> {
        if snapshot.world != self.identity {
            return Err(error(ErrorCode::InvalidSnapshot, None));
        }
        let object = snapshot
            .object(request.identity)
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        let mut trace = self.trace_object(object, request, snapshot.limits)?;
        trace.world = self.identity;
        trace.snapshot = Some(snapshot.identity);
        Ok(trace)
    }

    pub fn overlaps_object_hull_at(
        &self,
        snapshot: &Snapshot,
        request: ObjectOverlapRequest,
    ) -> Result<bool, Error> {
        self.trace_object_hull_at(
            snapshot,
            ObjectTraceRequest {
                identity: request.identity,
                transform: request.transform,
                start: request.position,
                end: request.position,
                hull: request.hull,
                mask: request.mask,
            },
        )
        .map(|trace| trace.start_solid)
    }

    fn trace_object(
        &self,
        object: &SnapshotRecord,
        request: ObjectTraceRequest,
        limits: SnapshotLimits,
    ) -> Result<Trace, Error> {
        let mut trace = match &object.shape {
            SnapshotShape::BrushModel { model } => {
                self.trace_model_hull(*model, request, object.identity, object.role)?
            }
            SnapshotShape::BoundingBox { bounds } => {
                let result = crate::trace_bounds(
                    request.start,
                    request.end,
                    request.hull,
                    Hull {mins:add(request.transform.origin, bounds.mins),maxs:add(request.transform.origin, bounds.maxs)},
                )?;
                let mut trace=miss(result.start,result.end);
                trace.fraction=result.fraction;
                trace.fraction_left_solid=result.fraction_left_solid;
                trace.start_solid=result.start_solid;
                trace.all_solid=result.all_solid;
                trace.contents=if result.contents==0 {0}else{object.contents};
                trace.plane=result.plane;
                if trace.did_hit() {trace.hit=Some(Hit::Object {identity:object.identity,role:object.role,feature:Feature::Box});}
                set_box_feature(&mut trace);
                trace
            }
            SnapshotShape::OrientedBox { bounds } => {
                let basis = request.transform.basis()?;
                let point = point_hull(request.hull);
                if point {
                    let local_start = basis.inverse_point(request.start);
                    let local_end = add(
                        local_start,
                        basis.inverse_vector(sub(request.end, request.start)),
                    );
                    let mut trace = trace_box(
                        local_start,
                        local_end,
                        request.hull,
                        bounds.mins,
                        bounds.maxs,
                        object.contents,
                    )?;
                    if let Some(plane) = trace.plane.as_mut() {
                        plane.normal = basis.vector(plane.normal);
                    }
                    trace.end = interpolate(request.start, request.end, trace.fraction);
                    set_box_feature(&mut trace);
                    trace
                } else {
                    let vertices = box_vertices(*bounds);
                    let faces = box_faces();
                    let edges = box_edges();
                    let mut trace = trace_convex(
                        request.start,
                        request.end,
                        request.hull,
                        &vertices,
                        &faces,
                        &edges,
                        basis,
                        object.contents,
                        limits,
                    )?;
                    set_box_feature(&mut trace);
                    trace
                }
            }
            SnapshotShape::Physics(shape) | SnapshotShape::Follower { query: shape, .. } => {
                let request = if matches!(object.shape, SnapshotShape::Follower { .. }) { ObjectTraceRequest { mask: u32::MAX, ..request } } else { request };
                let (result, selected) = shape.trace(request)?;
                let mut output = miss(result.start, result.end);
                output.fraction = result.fraction;
                output.fraction_left_solid = result.fraction_left_solid;
                output.start_solid = result.start_solid;
                output.all_solid = result.all_solid;
                output.contents = result.contents;
                output.plane = result.plane;
                if output.did_hit() {
                    let convex = selected.and_then(|index| shape.geometry().convexes.get(index)).ok_or_else(|| error(ErrorCode::InvalidReference, selected))?;
                    output.hit = Some(Hit::Object { identity: object.identity, role: object.role,
                        feature: Feature::Convex { solid: convex.solid, convex: convex.convex, triangle: None } });
                }
                output
            }
        };
        if (trace.hit.is_some() || trace.start_solid || trace.all_solid)
            && !matches!(&object.shape, SnapshotShape::BrushModel { .. })
        {
            trace.surface_flags = object.surface_flags;
            let feature = match trace.hit {
                Some(Hit::Object { feature, .. }) => feature,
                _ => Feature::Box,
            };
            trace.hit = Some(Hit::Object {
                identity: match &object.shape { SnapshotShape::Follower { parent, .. } => *parent, _ => object.identity },
                role: object.role,
                feature,
            });
        }
        trace.world = self.identity;
        Ok(trace)
    }
}

fn trace_box(
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    mins: [f32; 3],
    maxs: [f32; 3],
    contents: u32,
) -> Result<Trace, Error> {
    validate_hull(hull, 0)?;
    if start
        .into_iter()
        .chain(end)
        .chain(mins)
        .chain(maxs)
        .any(|value| !value.is_finite())
        || mins.into_iter().zip(maxs).any(|(min, max)| min > max)
    {
        return Err(error(ErrorCode::InvalidSnapshot, None));
    }
    let center = scale(add(hull.mins, hull.maxs), 0.5);
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    let ray_start = add(start, center);
    let ray_end = add(end, center);
    let expanded_mins = sub(mins, extents);
    let expanded_maxs = add(maxs, extents);
    let axes = [
        IntervalAxis::new([-1.0, 0.0, 0.0], -expanded_mins[0], None),
        IntervalAxis::new([0.0, -1.0, 0.0], -expanded_mins[1], None),
        IntervalAxis::new([0.0, 0.0, -1.0], -expanded_mins[2], None),
        IntervalAxis::new([1.0, 0.0, 0.0], expanded_maxs[0], None),
        IntervalAxis::new([0.0, 1.0, 0.0], expanded_maxs[1], None),
        IntervalAxis::new([0.0, 0.0, 1.0], expanded_maxs[2], None),
    ];
    clip_axes(
        start,
        end,
        ray_start,
        ray_end,
        axes,
        contents,
        point_hull(hull),
    )
}

#[allow(clippy::too_many_arguments)]
fn trace_convex(
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    local_vertices: &[[f32; 3]],
    faces: &[Face],
    edges: &[[f32; 3]],
    basis: Basis,
    contents: u32,
    limits: SnapshotLimits,
) -> Result<Trace, Error> {
    validate_hull(hull, 0)?;
    let center = scale(add(hull.mins, hull.maxs), 0.5);
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    let ray_start = add(start, center);
    let ray_end = add(end, center);
    let vertices = local_vertices
        .iter()
        .copied()
        .map(|vertex| basis.point(vertex))
        .collect::<Vec<_>>();
    let mut directions = faces
        .iter()
        .map(|face| (basis.vector(face.normal), Some(face.triangle)))
        .collect::<Vec<_>>();
    if !point_hull(hull) {
        directions.extend([
            ([1.0, 0.0, 0.0], None),
            ([0.0, 1.0, 0.0], None),
            ([0.0, 0.0, 1.0], None),
        ]);
        for edge in edges.iter().copied().map(|edge| basis.vector(edge)) {
            directions.extend([
                (cross(edge, [1.0, 0.0, 0.0]), None),
                (cross(edge, [0.0, 1.0, 0.0]), None),
                (cross(edge, [0.0, 0.0, 1.0]), None),
            ]);
        }
    }
    if directions.len() > limits.max_axes_per_convex {
        return Err(error(ErrorCode::Limit, None));
    }
    // Project support intervals on demand. A separating face already proves a
    // miss; projecting every remaining edge axis first made compound prop hull
    // queries dominate bot movement even when the convex could not be hit.
    let axes = directions
        .into_iter()
        .filter_map(|(direction, triangle)| {
            let length = length(direction);
            if length <= f32::EPSILON {
                return None;
            }
            let normal = scale(direction, 1.0 / length);
            let radius = dot_abs(normal, extents);
            #[cfg(feature = "replay-reference")]
            crate::replay_diagnostics::count(5, vertices.len());
            let (minimum, maximum) = vertices.iter().copied().fold(
                (f32::INFINITY, f32::NEG_INFINITY),
                |(minimum, maximum), vertex| {
                    let projection = dot(vertex, normal);
                    (minimum.min(projection), maximum.max(projection))
                },
            );
            Some([
                IntervalAxis::new(scale(normal, -1.0), -(minimum - radius), triangle),
                IntervalAxis::new(normal, maximum + radius, triangle),
            ])
        })
        .flatten();
    #[cfg(test)]
    let eager = clip_axes(
        start,
        end,
        ray_start,
        ray_end,
        axes.clone().collect::<Vec<_>>(),
        contents,
        point_hull(hull),
    );
    let mut trace = clip_axes(
        start,
        end,
        ray_start,
        ray_end,
        axes,
        contents,
        point_hull(hull),
    )?;
    #[cfg(test)]
    assert_eq!(
        eager,
        Ok(trace),
        "lazy support projection must preserve the full eager trace"
    );
    set_convex_feature(&mut trace);
    Ok(trace)
}

fn set_convex_feature(trace: &mut Trace) {
    if trace.hit.is_some() || trace.start_solid || trace.all_solid {
        let triangle = trace.hit.and_then(|hit| match hit {
            Hit::Object {
                feature: Feature::Convex { triangle, .. },
                ..
            } => triangle,
            _ => None,
        });
        trace.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Convex {
                solid: 0,
                convex: 0,
                triangle,
            },
        });
    }
}

#[derive(Clone, Copy)]
struct IntervalAxis {
    normal: [f32; 3],
    distance: f32,
    triangle: Option<usize>,
}

impl IntervalAxis {
    const fn new(normal: [f32; 3], distance: f32, triangle: Option<usize>) -> Self {
        Self {
            normal,
            distance,
            triangle,
        }
    }
}

fn clip_axes(
    requested_start: [f32; 3],
    requested_end: [f32; 3],
    start: [f32; 3],
    end: [f32; 3],
    axes: impl IntoIterator<Item = IntervalAxis>,
    contents: u32,
    point: bool,
) -> Result<Trace, Error> {
    #[cfg(feature = "replay-reference")]
    let axes = crate::replay_diagnostics::Planes::new(axes.into_iter());
    let mut output = miss(requested_start, requested_end);
    let mut enter = -1.0_f32;
    let mut leave = 1.0_f32;
    let mut starts_out = false;
    let mut gets_out = false;
    let mut selected = None;
    for axis in axes {
        let d1 = dot(start, axis.normal) - axis.distance;
        let d2 = dot(end, axis.normal) - axis.distance;
        if d1 > 0.0 {
            starts_out = true;
            if d2 > 0.0 {
                return Ok(output);
            }
        } else {
            if d2 <= 0.0 {
                continue;
            }
            gets_out = true;
        }
        if d1 > d2 {
            let fraction = (d1 - DIST_EPSILON).max(0.0) / (d1 - d2);
            if fraction > enter {
                enter = fraction;
                selected = Some(axis);
            }
        } else {
            leave = leave.min((d1 + DIST_EPSILON) / (d1 - d2));
        }
    }
    if !starts_out {
        output.start_solid = true;
        output.contents = contents;
        output.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Box,
        });
        if !gets_out {
            output.all_solid = true;
            output.fraction = 0.0;
            output.fraction_left_solid = 1.0;
            output.end = requested_start;
        } else if point && leave != 1.0 {
            output.fraction_left_solid = leave;
        }
        return Ok(output);
    }
    if enter < leave && enter > -1.0 && enter < 1.0 {
        let axis = selected.ok_or_else(|| error(ErrorCode::InvalidSnapshot, None))?;
        output.fraction = enter.max(0.0);
        output.end = interpolate(requested_start, requested_end, output.fraction);
        output.contents = contents;
        output.plane = Some(Plane {
            normal: axis.normal,
            distance: dot(output.end, axis.normal),
            kind: 3,
        });
        output.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Convex {
                solid: 0,
                convex: 0,
                triangle: axis.triangle,
            },
        });
    }
    Ok(output)
}

fn merge(output: &mut Trace, candidate: Trace) {
    if candidate.all_solid || candidate.start_solid || candidate.fraction < output.fraction {
        if output.start_solid {
            let left = output.fraction_left_solid;
            let snapshot = output.snapshot;
            *output = candidate;
            output.start_solid = true;
            output.fraction_left_solid = left.max(output.fraction_left_solid);
            output.snapshot = snapshot.or(output.snapshot);
        } else {
            let snapshot = output.snapshot;
            *output = candidate;
            output.snapshot = snapshot.or(output.snapshot);
        }
    }
}

fn set_box_feature(trace: &mut Trace) {
    if trace.hit.is_some() || trace.start_solid || trace.all_solid {
        trace.hit = Some(Hit::Object {
            identity: 0,
            role: ObjectRole::Entity,
            feature: Feature::Box,
        });
    }
}

fn miss(_start: [f32; 3], end: [f32; 3]) -> Trace {
    Trace {
        world: [0; 32],
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
    }
}

fn validate_hull(hull: Hull, item: usize) -> Result<(), Error> {
    if hull
        .mins
        .into_iter()
        .chain(hull.maxs)
        .any(|value| !value.is_finite())
        || hull
            .mins
            .into_iter()
            .zip(hull.maxs)
            .any(|(min, max)| min > max)
    {
        Err(error(ErrorCode::InvalidHull, Some(item)))
    } else {
        Ok(())
    }
}

fn transformed_bounds(bounds: Hull, basis: Basis) -> Hull {
    let mut mins = [f32::INFINITY; 3];
    let mut maxs = [f32::NEG_INFINITY; 3];
    for vertex in box_vertices(bounds)
        .into_iter()
        .map(|vertex| basis.point(vertex))
    {
        for axis in 0..3 {
            mins[axis] = mins[axis].min(vertex[axis]);
            maxs[axis] = maxs[axis].max(vertex[axis]);
        }
    }
    Hull { mins, maxs }
}

fn swept_hull_intersects(start: [f32; 3], end: [f32; 3], hull: Hull, bounds: Hull) -> bool {
    (0..3).all(|axis| {
        let minimum = start[axis].min(end[axis]) + hull.mins[axis];
        let maximum = start[axis].max(end[axis]) + hull.maxs[axis];
        maximum >= bounds.mins[axis] && minimum <= bounds.maxs[axis]
    })
}

fn point_hull(hull: Hull) -> bool {
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    dot(extents, extents) < 1.0e-6
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn quaternion_from_angles(angles: [f32; 3]) -> [f64; 4] {
    let (sp, cp) = (f64::from(angles[0]).to_radians() * 0.5).sin_cos();
    let (sy, cy) = (f64::from(angles[1]).to_radians() * 0.5).sin_cos();
    let (sr, cr) = (f64::from(angles[2]).to_radians() * 0.5).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

fn quaternion_multiply(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quaternion_inverse(value: [f64; 4]) -> [f64; 4] {
    [-value[0], -value[1], -value[2], value[3]]
}

fn quaternion_rotate(rotation: [f64; 4], value: [f32; 3]) -> [f32; 3] {
    let vector = [
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
        0.0,
    ];
    let result = quaternion_multiply(
        quaternion_multiply(rotation, vector),
        quaternion_inverse(rotation),
    );
    [result[0] as f32, result[1] as f32, result[2] as f32]
}

fn angles_from_quaternion(value: [f64; 4]) -> [f32; 3] {
    let roll = (2.0 * (value[3] * value[0] + value[1] * value[2]))
        .atan2(1.0 - 2.0 * (value[0] * value[0] + value[1] * value[1]));
    let sine_pitch = 2.0 * (value[3] * value[1] - value[2] * value[0]);
    let pitch = if sine_pitch.abs() >= 1.0 {
        std::f64::consts::FRAC_PI_2.copysign(sine_pitch)
    } else {
        sine_pitch.asin()
    };
    let yaw = (2.0 * (value[3] * value[2] + value[0] * value[1]))
        .atan2(1.0 - 2.0 * (value[1] * value[1] + value[2] * value[2]));
    [
        pitch.to_degrees() as f32,
        yaw.to_degrees() as f32,
        roll.to_degrees() as f32,
    ]
}

fn length(value: [f32; 3]) -> f32 {
    dot(value, value).sqrt()
}

fn dot_abs(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0].abs() * b[0] + a[1].abs() * b[1] + a[2].abs() * b[2]
}

fn box_vertices(bounds: Hull) -> Vec<[f32; 3]> {
    let mut vertices = Vec::with_capacity(8);
    for z in [bounds.mins[2], bounds.maxs[2]] {
        for y in [bounds.mins[1], bounds.maxs[1]] {
            for x in [bounds.mins[0], bounds.maxs[0]] {
                vertices.push([x, y, z]);
            }
        }
    }
    vertices
}

fn box_faces() -> Vec<Face> {
    [
        ([-1.0, 0.0, 0.0], 0),
        ([1.0, 0.0, 0.0], 1),
        ([0.0, -1.0, 0.0], 2),
        ([0.0, 1.0, 0.0], 3),
        ([0.0, 0.0, -1.0], 4),
        ([0.0, 0.0, 1.0], 5),
    ]
    .into_iter()
    .map(|(normal, triangle)| Face { normal, triangle })
    .collect()
}

fn box_edges() -> Vec<[f32; 3]> {
    vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

struct BoundedBytes {
    bytes: Vec<u8>,
    maximum: usize,
}
impl BoundedBytes {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), Error> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(error(ErrorCode::Limit, None));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), Error> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn i32(&mut self, value: i32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn f32(&mut self, value: f32) -> Result<(), Error> {
        self.u32(value.to_bits())
    }

    fn hull(&mut self, hull: Hull) -> Result<(), Error> {
        for value in hull.mins.into_iter().chain(hull.maxs) {
            self.f32(value)?;
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod lazy_support_tests {
    use super::*;

    #[test]
    fn separated_queries_do_not_project_unused_support_axes() {
        let visited = std::cell::Cell::new(0);
        let axes = [
            IntervalAxis::new([1.0, 0.0, 0.0], 1.0, None),
            IntervalAxis::new([0.0, 1.0, 0.0], 1.0, None),
        ]
        .into_iter()
        .inspect(|_| visited.set(visited.get() + 1));
        let trace = clip_axes([3.0; 3], [4.0; 3], [3.0; 3], [4.0; 3], axes, 1, false).unwrap();
        assert_eq!(trace.fraction, 1.0);
        assert_eq!(visited.get(), 1);
    }

    #[test]
    fn lazy_and_eager_queries_match_for_rotations_extents_inside_and_sweeps() {
        let vertices = box_vertices(Hull {
            mins: [-8.0, -16.0, -4.0],
            maxs: [8.0, 16.0, 4.0],
        });
        for yaw in [0.0, 15.0, 90.0, 180.0, 273.0] {
            let basis = Transform {
                origin: [11.0, -7.0, 3.0],
                angles: [21.0, yaw, -13.0],
            }
            .basis()
            .unwrap();
            for hull in [
                Hull {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                },
                Hull {
                    mins: [-24.0, -24.0, 0.0],
                    maxs: [24.0, 24.0, 82.0],
                },
            ] {
                for x in -8..=8 {
                    for y in -8..=8 {
                        let start = [x as f32 * 8.0, y as f32 * 8.0, 3.0];
                        for end in [start, [0.0; 3], [-start[0], -start[1], -8.0]] {
                            // Each test-build trace compares every returned field
                            // against eager support projection before returning.
                            trace_convex(
                                start,
                                end,
                                hull,
                                &vertices,
                                &box_faces(),
                                &box_edges(),
                                basis,
                                1,
                                SnapshotLimits::default(),
                            )
                            .unwrap();
                        }
                    }
                }
            }
        }
    }
}
