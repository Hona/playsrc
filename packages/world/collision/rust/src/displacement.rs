// Source 1 SDK License applies; see ../../../../../LICENSE.source-sdk-2013.
// Behavior is derived from Valve Source SDK 2013 dispcoll_common and collisionutils contracts.
use crate::{
    DisplacementFeature, Error, ErrorCode, Hull, Plane, SurfaceIdentity, Trace, World, add, dot,
    error, interpolate, scale, sub,
};
use std::collections::HashMap;

const DIST_EPSILON: f32 = 1.0 / 32.0;
const INVALID_FRACTION: f32 = -99_999.9;
const SURFACE_FLAG: u16 = 1;
const SURFACE_PROP_1: u16 = 1 << 3;
const SURFACE_PROP_2: u16 = 1 << 4;
const NO_HULL_COLLISION: u32 = 1 << 2;
const NO_RAY_COLLISION: u32 = 1 << 3;
const SECONDARY_SURFACE: u16 = 1 << 15;

#[derive(Clone, Debug, Default)]
pub(crate) struct Acceleration {
    trees: Vec<Tree>,
    nodes: Vec<BoundsNode>,
}

#[cfg(feature = "replay-reference")]
pub(crate) fn storage(world: &World) -> [usize; 4] {
    let acceleration = &world.displacement_trees;
    let mut result = [
        0,
        acceleration.nodes.len(),
        0,
        acceleration.trees.capacity() * size_of::<Tree>()
            + acceleration.nodes.capacity() * size_of::<BoundsNode>(),
    ];
    for tree in &acceleration.trees {
        result[0] += tree.triangles.len();
        result[1] += tree.nodes.len();
        result[2] += tree.edge_planes.len();
        result[3] += tree.triangles.capacity() * size_of::<Triangle>()
            + tree.nodes.capacity() * size_of::<BoundsNode>()
            + tree.edge_planes.capacity() * size_of::<PackedEdgePlane>();
    }
    result
}

#[derive(Clone, Debug)]
pub(crate) struct Tree {
    source: usize,
    parent_face: usize,
    contents: u32,
    surface_flags: u32,
    bounds: Hull,
    triangles: Vec<Triangle>,
    nodes: Vec<BoundsNode>,
    edge_planes: Vec<PackedEdgePlane>,
    primary_surface: SurfaceIdentity,
    secondary_surface: Option<SurfaceIdentity>,
}

#[derive(Clone, Debug)]
struct Triangle {
    // Source vertices remain owned by World. Two-bit extrema selectors avoid
    // another six coordinates per triangle while retaining their exact bits.
    vertices: [u16; 3],
    normal: [f32; 3],
    distance: f32,
    bounds_vertices: u16,
    flags: u16,
    edges: [u16; 9],
}

const NO_PLANE: u16 = u16::MAX;

#[derive(Clone, Copy, Debug)]
struct EdgePlane {
    normal: [f32; 3],
    distance: f32,
}

#[derive(Clone, Copy, Debug)]
struct PackedEdgePlane([f32; 3]);

impl PackedEdgePlane {
    fn unpack(self, axis: usize, index: u16) -> EdgePlane {
        // The omitted axial component is zero, but its sign still belongs to
        // the original arithmetic. It is retained in the plane index's high bit.
        let mut normal = [0.0; 3];
        normal[axis] = if index & 0x8000 == 0 { 0.0 } else { -0.0 };
        normal[(axis + 1) % 3] = self.0[0];
        normal[(axis + 2) % 3] = self.0[1];
        EdgePlane {
            normal,
            distance: self.0[2],
        }
    }
}

impl Triangle {
    fn bounds(&self, positions: &[[f32; 3]]) -> Hull {
        Hull {
            mins: std::array::from_fn(|axis| {
                positions[usize::from(
                    self.vertices[usize::from((self.bounds_vertices >> (axis * 2)) & 3)],
                )][axis]
            }),
            maxs: std::array::from_fn(|axis| {
                positions[usize::from(
                    self.vertices[usize::from((self.bounds_vertices >> (axis * 2 + 6)) & 3)],
                )][axis]
            }),
        }
    }
}

fn bounds_vertices(points: [[f32; 3]; 3]) -> u16 {
    let bounds = points.into_iter().fold(empty_bounds(), include);
    let mut result = 0;
    for axis in 0..3 {
        let minimum = points
            .iter()
            .position(|point| point[axis].to_bits() == bounds.mins[axis].to_bits())
            .unwrap();
        let maximum = points
            .iter()
            .position(|point| point[axis].to_bits() == bounds.maxs[axis].to_bits())
            .unwrap();
        result |= (minimum as u16) << (axis * 2);
        result |= (maximum as u16) << (axis * 2 + 6);
    }
    result
}

// Preorder ranges, not a spatial permutation: equal-fraction features must keep
// their original triangle order. A rejected parent only skips contained bounds.
#[derive(Clone, Debug)]
struct BoundsNode {
    bounds: Hull,
    first: usize,
    end: usize,
    escape: usize,
}

fn build_nodes(bounds: &[Hull], first: usize, nodes: &mut Vec<BoundsNode>, leaf_size: usize) {
    let index = nodes.len();
    nodes.push(BoundsNode {
        bounds: bounds.iter().copied().fold(empty_bounds(), union),
        first,
        end: first + bounds.len(),
        escape: 0,
    });
    if bounds.len() > leaf_size {
        let middle = bounds.len() / 2;
        build_nodes(&bounds[..middle], first, nodes, leaf_size);
        build_nodes(&bounds[middle..], first + middle, nodes, leaf_size);
    }
    nodes[index].escape = nodes.len();
}

struct Candidates<'a> {
    nodes: &'a [BoundsNode],
    query: Hull,
    expansion: [f32; 3],
    node: usize,
    next: usize,
    end: usize,
}

impl<'a> Candidates<'a> {
    fn new(
        nodes: &'a [BoundsNode],
        length: usize,
        query: Hull,
        expansion: [f32; 3],
        reference: bool,
    ) -> Self {
        Self {
            nodes,
            query,
            expansion,
            node: if reference { nodes.len() } else { 0 },
            next: 0,
            end: if reference { length } else { 0 },
        }
    }
}

impl<'a> Iterator for Candidates<'a> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.next < self.end {
                let index = self.next;
                self.next += 1;
                return Some(index);
            }
            let node = self.nodes.get(self.node)?;
            #[cfg(feature = "replay-reference")]
            crate::replay_diagnostics::count(6, 1);
            if !intersects(self.query, expand(node.bounds, self.expansion)) {
                self.node = node.escape;
            } else {
                self.node += 1;
                if self.node == node.escape {
                    self.next = node.first;
                    self.end = node.end;
                }
            }
        }
    }
}

fn edge_plane(points: [[f32; 3]; 3], axis: usize, edge_index: usize) -> Option<EdgePlane> {
    let edge = sub(points[(edge_index + 1) % 3], points[edge_index]);
    let mut axis_vector = [0.0; 3];
    axis_vector[axis] = 1.0;
    let mut normal = normalize(cross(edge, axis_vector))?;
    if normal[(axis + 1) % 3] == 0.0 || normal[(axis + 2) % 3] == 0.0 {
        return None;
    }
    let mut distance = dot(normal, points[edge_index]);
    let off = dot(normal, points[(edge_index + 2) % 3]);
    if off > distance && (off - distance).abs() >= DIST_EPSILON {
        normal = scale(normal, -1.0);
        distance = -distance;
    }
    Some(EdgePlane { normal, distance })
}

fn cache_edges(
    points: [[f32; 3]; 3],
    edge_planes: &mut Vec<PackedEdgePlane>,
    plane_indices: &mut HashMap<[u32; 4], usize>,
    item: usize,
) -> Result<[u16; 9], Error> {
    let mut edges = [NO_PLANE; 9];
    for (slot, target) in edges.iter_mut().enumerate() {
        if let Some(plane) = edge_plane(points, slot / 3, slot % 3) {
            // Bit keys deliberately distinguish signed zero. Neither
            // approximate normals nor opposite planes are merged.
            let key = [
                plane.normal[0].to_bits(),
                plane.normal[1].to_bits(),
                plane.normal[2].to_bits(),
                plane.distance.to_bits(),
            ];
            let index = *plane_indices.entry(key).or_insert_with(|| {
                let index = edge_planes.len();
                let axis = slot / 3;
                edge_planes.push(PackedEdgePlane([
                    plane.normal[(axis + 1) % 3],
                    plane.normal[(axis + 2) % 3],
                    plane.distance,
                ]));
                index
            });
            *target = u16::try_from(index)
                .ok()
                .filter(|v| *v < 0x7fff)
                .ok_or_else(|| error(ErrorCode::InvalidRange, Some(item)))?;
            if plane.normal[slot / 3].is_sign_negative() {
                *target |= 0x8000;
            }
        }
    }
    Ok(edges)
}

pub(crate) fn build(world: &World) -> Result<Acceleration, Error> {
    let mut trees = Vec::with_capacity(world.displacement_inputs.len());
    for (item, (input, patch)) in world
        .displacement_inputs
        .iter()
        .zip(&world.displacements)
        .enumerate()
    {
        let mut triangles = Vec::with_capacity(input.triangles.len());
        let mut bounds = empty_bounds();
        let mut edge_planes = Vec::new();
        let mut plane_indices = HashMap::new();
        for (index, vertices) in input.triangles.iter().copied().enumerate() {
            let points = vertices.map(|vertex| input.positions[vertex as usize]);
            let mut compact_vertices = [0; 3];
            for (target, source) in compact_vertices.iter_mut().zip(vertices) {
                *target = u16::try_from(source)
                    .map_err(|_| error(ErrorCode::InvalidRange, Some(item)))?;
            }
            let normal = normalize(cross(sub(points[2], points[0]), sub(points[1], points[0])))
                .ok_or_else(|| error(ErrorCode::InvalidRange, Some(item)))?;
            let triangle_bounds = points.into_iter().fold(empty_bounds(), include);
            bounds = union(bounds, triangle_bounds);
            let secondary = input.use_secondary_surface[index];
            triangles.push(Triangle {
                vertices: compact_vertices,
                normal,
                distance: dot(normal, points[0]),
                bounds_vertices: bounds_vertices(points),
                flags: (input.triangle_tags[index] & 0x1f)
                    | if secondary { SECONDARY_SURFACE } else { 0 }
                    | SURFACE_FLAG
                    | if secondary {
                        SURFACE_PROP_2
                    } else {
                        SURFACE_PROP_1
                    },
                edges: cache_edges(points, &mut edge_planes, &mut plane_indices, item)?,
            });
        }
        for axis in 0..3 {
            bounds.mins[axis] -= 1.0;
            bounds.maxs[axis] += 1.0;
        }
        let minimum = patch.minimum_tessellation as u32;
        let mut nodes = Vec::new();
        build_nodes(
            &triangles
                .iter()
                .map(|t| t.bounds(&input.positions))
                .collect::<Vec<_>>(),
            0,
            &mut nodes,
            8,
        );
        nodes.shrink_to_fit();
        edge_planes.shrink_to_fit();
        trees.push(Tree {
            source: input.source,
            parent_face: input.parent_face,
            contents: input.contents,
            surface_flags: if minimum & 0x8000_0000 != 0 {
                minimum & 0x7fff_ffff
            } else {
                0
            },
            bounds,
            triangles,
            nodes,
            edge_planes,
            primary_surface: input.primary_surface,
            secondary_surface: input.secondary_surface,
        });
    }
    let mut nodes = Vec::new();
    build_nodes(
        &trees.iter().map(|tree| tree.bounds).collect::<Vec<_>>(),
        0,
        &mut nodes,
        4,
    );
    nodes.shrink_to_fit();
    Ok(Acceleration { trees, nodes })
}

pub(crate) fn trace(
    world: &World,
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    mask: u32,
    output: &mut Trace,
) -> Result<(), Error> {
    #[cfg(feature = "replay-reference")]
    if crate::replay_diagnostics::displacement_reference() {
        return trace_inner::<true>(world, start, end, hull, mask, output);
    }
    trace_inner::<false>(world, start, end, hull, mask, output)
}

fn trace_inner<const REFERENCE: bool>(
    world: &World,
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    mask: u32,
    output: &mut Trace,
) -> Result<(), Error> {
    let acceleration = &world.displacement_trees;
    if output.all_solid || acceleration.trees.is_empty() {
        return Ok(());
    }
    let center_offset = scale(add(hull.mins, hull.maxs), 0.5);
    let extents = scale(sub(hull.maxs, hull.mins), 0.5);
    let ray_start = add(start, center_offset);
    let ray_end = add(end, center_offset);
    let delta = sub(ray_end, ray_start);
    let point = dot(extents, extents) < 1.0e-6;
    let swept = delta != [0.0; 3];
    let query_bounds = expand(swept_bounds(ray_start, ray_end, extents), [DIST_EPSILON; 3]);

    // The stationary-point contract's one-sided entry and positive post-hit tests are
    // mutually exclusive. Points remain non-solid; stationary boxes use triangle overlap.
    if !swept && point {
        return Ok(());
    }
    // Swept displacement hits only replace a strictly larger fraction; unlike
    // stationary overlaps they cannot change start/all-solid state.
    if !REFERENCE && swept && output.fraction == 0.0 {
        return Ok(());
    }

    for index in Candidates::new(
        &acceleration.nodes,
        acceleration.trees.len(),
        query_bounds,
        [0.0; 3],
        REFERENCE,
    ) {
        let tree = &acceleration.trees[index];
        let positions = &world.displacement_inputs[index].positions;
        if tree.contents & mask == 0 || !intersects(query_bounds, tree.bounds) {
            continue;
        }
        if !swept {
            if tree.surface_flags & NO_HULL_COLLISION != 0 {
                continue;
            }
            for index in Candidates::new(
                &tree.nodes,
                tree.triangles.len(),
                query_bounds,
                [0.0; 3],
                REFERENCE,
            ) {
                let triangle = &tree.triangles[index];
                #[cfg(feature = "replay-reference")]
                crate::replay_diagnostics::count(7, 1);
                if intersects(query_bounds, triangle.bounds(positions))
                    && box_triangle_overlap(ray_start, extents, triangle, positions)
                {
                    output.fraction = 0.0;
                    output.fraction_left_solid = 0.0;
                    output.start_solid = true;
                    output.all_solid = true;
                    set_hit(output, tree, triangle, index, None, start);
                    return Ok(());
                }
            }
            continue;
        }
        if point {
            if tree.surface_flags & NO_RAY_COLLISION != 0 {
                continue;
            }
            for index in Candidates::new(
                &tree.nodes,
                tree.triangles.len(),
                query_bounds,
                [0.0; 3],
                REFERENCE,
            ) {
                let triangle = &tree.triangles[index];
                #[cfg(feature = "replay-reference")]
                crate::replay_diagnostics::count(7, 1);
                if !intersects(query_bounds, triangle.bounds(positions)) {
                    continue;
                }
                if let Some(fraction) = ray_triangle(ray_start, delta, triangle, positions)
                    && fraction < output.fraction
                {
                    let end_position = interpolate(start, end, fraction);
                    set_hit(output, tree, triangle, index, Some(fraction), end_position);
                }
            }
        } else {
            if tree.surface_flags & NO_HULL_COLLISION != 0 {
                continue;
            }
            let mut scratch = PlaneScratch::new();
            for index in Candidates::new(
                &tree.nodes,
                tree.triangles.len(),
                query_bounds,
                extents,
                REFERENCE,
            ) {
                let triangle = &tree.triangles[index];
                #[cfg(feature = "replay-reference")]
                crate::replay_diagnostics::count(7, 1);
                let bounds = triangle.bounds(positions);
                if !intersects(query_bounds, expand(bounds, extents)) {
                    continue;
                }
                if let Some((fraction, normal, distance)) = sweep_box_triangle::<REFERENCE>(
                    ray_start,
                    delta,
                    extents,
                    triangle,
                    bounds,
                    positions,
                    tree,
                    &mut scratch,
                    output.fraction,
                ) && fraction < output.fraction
                {
                    let end_position = interpolate(start, end, fraction);
                    set_hit_with_plane(
                        output,
                        tree,
                        triangle,
                        index,
                        fraction,
                        normal,
                        distance,
                        end_position,
                    );
                }
            }
        }
    }
    Ok(())
}

fn set_hit(
    output: &mut Trace,
    tree: &Tree,
    triangle: &Triangle,
    index: usize,
    fraction: Option<f32>,
    end: [f32; 3],
) {
    if let Some(fraction) = fraction {
        output.fraction = fraction;
        output.plane = Some(Plane {
            normal: triangle.normal,
            distance: triangle.distance,
            kind: 3,
        });
    }
    output.end = end;
    output.brush = None;
    output.contents = tree.contents;
    output.surface_flags = 0;
    output.displacement_flags = triangle.flags & 0x1f;
    output.surface = Some(if triangle.flags & SECONDARY_SURFACE != 0 {
        tree.secondary_surface.expect("validated secondary surface")
    } else {
        tree.primary_surface
    });
    output.hit = None;
    output.displacement = Some(DisplacementFeature {
        source: tree.source,
        parent_face: tree.parent_face,
        triangle: index,
    });
}

fn set_hit_with_plane(
    output: &mut Trace,
    tree: &Tree,
    triangle: &Triangle,
    index: usize,
    fraction: f32,
    normal: [f32; 3],
    distance: f32,
    end: [f32; 3],
) {
    set_hit(output, tree, triangle, index, Some(fraction), end);
    output.plane = Some(Plane {
        normal,
        distance,
        kind: 3,
    });
}

fn ray_triangle(
    start: [f32; 3],
    delta: [f32; 3],
    triangle: &Triangle,
    positions: &[[f32; 3]],
) -> Option<f32> {
    if dot(triangle.normal, delta) >= 0.0 {
        return None;
    }
    let points = triangle.vertices.map(|index| positions[usize::from(index)]);
    let edge1 = sub(points[2], points[0]);
    let edge2 = sub(points[1], points[0]);
    let direction_cross_edge2 = cross(delta, edge2);
    let denominator = dot(direction_cross_edge2, edge1);
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    let inverse = 1.0 / denominator;
    let origin = sub(start, points[0]);
    let u = dot(direction_cross_edge2, origin) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let origin_cross_edge1 = cross(origin, edge1);
    let v = dot(origin_cross_edge1, delta) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let fraction = dot(origin_cross_edge1, edge2) * inverse;
    (-0.001..=1.001)
        .contains(&fraction)
        .then(|| fraction.clamp(0.0, 1.0))
}

#[derive(Clone, Copy)]
struct PlaneInterval {
    index: u16,
    first: f32,
    second: f32,
}

struct PlaneScratch([PlaneInterval; 64]);

impl PlaneScratch {
    fn new() -> Self {
        Self(
            [PlaneInterval {
                index: NO_PLANE,
                first: 0.0,
                second: 0.0,
            }; 64],
        )
    }

    fn interval(
        &mut self,
        index: u16,
        plane: EdgePlane,
        start: [f32; 3],
        delta: [f32; 3],
        extents: [f32; 3],
    ) -> PlaneInterval {
        let slot = &mut self.0[usize::from(index) % 64];
        if slot.index != index {
            #[cfg(feature = "replay-reference")]
            crate::replay_diagnostics::count(9, 1);
            let expanded = plane.distance + dot_abs(extents, plane.normal);
            *slot = PlaneInterval {
                index,
                first: dot(plane.normal, start) - expanded,
                second: dot(plane.normal, add(start, delta)) - expanded,
            };
        } else {
            #[cfg(feature = "replay-reference")]
            crate::replay_diagnostics::count(10, 1);
        }
        *slot
    }
}

fn sweep_box_triangle<const REFERENCE: bool>(
    start: [f32; 3],
    delta: [f32; 3],
    extents: [f32; 3],
    triangle: &Triangle,
    bounds: Hull,
    positions: &[[f32; 3]],
    tree: &Tree,
    scratch: &mut PlaneScratch,
    best_fraction: f32,
) -> Option<(f32, [f32; 3], f32)> {
    #[cfg(feature = "replay-reference")]
    crate::replay_diagnostics::count(8, 1);
    if dot(triangle.normal, delta) > DIST_EPSILON {
        return None;
    }
    let mut helper = SweepHelper {
        enter: INVALID_FRACTION,
        leave: 1.0,
        normal: [0.0; 3],
        distance: 0.0,
    };
    for axis in (0..3).rev() {
        let minimum = bounds.mins[axis];
        let maximum = bounds.maxs[axis];
        let mut negative = [0.0; 3];
        negative[axis] = -1.0;
        if !resolve_plane(
            start,
            delta,
            negative,
            -minimum + extents[axis],
            minimum,
            &mut helper,
        ) {
            return None;
        }
        let mut positive = [0.0; 3];
        positive[axis] = 1.0;
        if !resolve_plane(
            start,
            delta,
            positive,
            maximum + extents[axis],
            maximum,
            &mut helper,
        ) {
            return None;
        }
    }
    // Enter is monotone through the ordered clip planes. Once it cannot beat
    // the current hit, no later edge/face can change any published field.
    if !REFERENCE && helper.enter >= best_fraction {
        return None;
    }
    if REFERENCE {
        let points = triangle.vertices.map(|index| positions[usize::from(index)]);
        let edges = [
            sub(points[1], points[0]),
            sub(points[2], points[1]),
            sub(points[0], points[2]),
        ];
        for axis in 0..3 {
            for (edge_index, edge) in edges.iter().copied().enumerate() {
                let on = points[edge_index];
                let off = points[(edge_index + 2) % 3];
                let axis_vector = match axis {
                    0 => [1.0, 0.0, 0.0],
                    1 => [0.0, 1.0, 0.0],
                    _ => [0.0, 0.0, 1.0],
                };
                let Some(mut normal) = normalize(cross(edge, axis_vector)) else {
                    continue;
                };
                let defined = match axis {
                    0 => normal[1] != 0.0 && normal[2] != 0.0,
                    1 => normal[0] != 0.0 && normal[2] != 0.0,
                    _ => normal[0] != 0.0 && normal[1] != 0.0,
                };
                if !defined {
                    continue;
                }
                let mut distance = dot(normal, on);
                if dot(normal, off) > distance
                    && (dot(normal, off) - distance).abs() >= DIST_EPSILON
                {
                    normal = scale(normal, -1.0);
                    distance = -distance;
                }
                let expanded = distance + dot_abs(extents, normal);
                #[cfg(feature = "replay-reference")]
                crate::replay_diagnostics::count(9, 1);
                if !resolve_plane(start, delta, normal, expanded, distance, &mut helper) {
                    return None;
                }
            }
        }
    } else {
        for (slot, index) in triangle.edges.into_iter().enumerate() {
            if index == NO_PLANE {
                continue;
            }
            let plane = tree.edge_planes[usize::from(index & 0x7fff)].unpack(slot / 3, index);
            let interval = scratch.interval(index, plane, start, delta, extents);
            if !resolve_interval(
                interval.first,
                interval.second,
                plane.normal,
                plane.distance,
                &mut helper,
            ) || helper.enter >= best_fraction
            {
                return None;
            }
        }
    }
    let face_expanded = triangle.distance + dot_abs(extents, triangle.normal);
    if !resolve_plane(
        start,
        delta,
        triangle.normal,
        face_expanded,
        triangle.distance,
        &mut helper,
    ) {
        return None;
    }
    if (helper.enter < helper.leave || (helper.enter - helper.leave).abs() < 0.001)
        && helper.enter != INVALID_FRACTION
    {
        Some((helper.enter.max(0.0), helper.normal, helper.distance))
    } else {
        None
    }
}

struct SweepHelper {
    enter: f32,
    leave: f32,
    normal: [f32; 3],
    distance: f32,
}

fn resolve_plane(
    start: [f32; 3],
    delta: [f32; 3],
    normal: [f32; 3],
    expanded_distance: f32,
    impact_distance: f32,
    helper: &mut SweepHelper,
) -> bool {
    let first = dot(normal, start) - expanded_distance;
    let second = dot(normal, add(start, delta)) - expanded_distance;
    resolve_interval(first, second, normal, impact_distance, helper)
}

fn resolve_interval(
    first: f32,
    second: f32,
    normal: [f32; 3],
    impact_distance: f32,
    helper: &mut SweepHelper,
) -> bool {
    if first > 0.0 && second > 0.0 {
        return false;
    }
    if first < 0.0 && second < 0.0 {
        return true;
    }
    let denominator = first - second;
    if first >= 0.0 && second <= 0.0 {
        let fraction = if denominator != 0.0 {
            (first - DIST_EPSILON) / denominator
        } else {
            0.0
        };
        if fraction > helper.enter {
            helper.enter = fraction;
            helper.normal = normal;
            helper.distance = impact_distance;
        }
    } else {
        let fraction = if denominator != 0.0 {
            (first + DIST_EPSILON) / denominator
        } else {
            0.0
        };
        if fraction < helper.leave {
            helper.leave = fraction;
        }
    }
    true
}

fn box_triangle_overlap(
    center: [f32; 3],
    extents: [f32; 3],
    triangle: &Triangle,
    positions: &[[f32; 3]],
) -> bool {
    // Keep translation before edge subtraction here. Reusing world-space edges
    // for a stationary overlap would change binary32 rounding at boundaries.
    let points = triangle
        .vertices
        .map(|index| sub(positions[usize::from(index)], center));
    for axis in 0..3 {
        let (minimum, maximum) = points.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(minimum, maximum), point| (minimum.min(point[axis]), maximum.max(point[axis])),
        );
        if minimum > extents[axis] || maximum < -extents[axis] {
            return false;
        }
    }
    if (dot(triangle.normal, center) - triangle.distance).abs() > dot_abs(extents, triangle.normal)
    {
        return false;
    }
    let edges = [
        sub(points[1], points[0]),
        sub(points[2], points[1]),
        sub(points[0], points[2]),
    ];
    for edge in edges {
        for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let candidate = cross(edge, axis);
            let length_squared = dot(candidate, candidate);
            if length_squared <= f32::EPSILON {
                continue;
            }
            let projections = points.map(|point| dot(point, candidate));
            let minimum = projections.into_iter().fold(f32::INFINITY, f32::min);
            let maximum = projections.into_iter().fold(f32::NEG_INFINITY, f32::max);
            let radius = dot_abs(extents, candidate);
            if minimum > radius || maximum < -radius {
                return false;
            }
        }
    }
    true
}

fn empty_bounds() -> Hull {
    Hull {
        mins: [f32::INFINITY; 3],
        maxs: [f32::NEG_INFINITY; 3],
    }
}

fn include(mut bounds: Hull, point: [f32; 3]) -> Hull {
    for (axis, value) in point.into_iter().enumerate() {
        bounds.mins[axis] = bounds.mins[axis].min(value);
        bounds.maxs[axis] = bounds.maxs[axis].max(value);
    }
    bounds
}

fn union(mut left: Hull, right: Hull) -> Hull {
    for axis in 0..3 {
        left.mins[axis] = left.mins[axis].min(right.mins[axis]);
        left.maxs[axis] = left.maxs[axis].max(right.maxs[axis]);
    }
    left
}

fn expand(mut bounds: Hull, amount: [f32; 3]) -> Hull {
    for (axis, value) in amount.into_iter().enumerate() {
        bounds.mins[axis] -= value;
        bounds.maxs[axis] += value;
    }
    bounds
}

fn swept_bounds(start: [f32; 3], end: [f32; 3], extents: [f32; 3]) -> Hull {
    Hull {
        mins: std::array::from_fn(|axis| start[axis].min(end[axis]) - extents[axis]),
        maxs: std::array::from_fn(|axis| start[axis].max(end[axis]) + extents[axis]),
    }
}

fn intersects(left: Hull, right: Hull) -> bool {
    (0..3).all(|axis| left.maxs[axis] >= right.mins[axis] && left.mins[axis] <= right.maxs[axis])
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(value, value).sqrt();
    (length.is_finite() && length > f32::EPSILON).then(|| scale(value, 1.0 / length))
}

fn dot_abs(extents: [f32; 3], normal: [f32; 3]) -> f32 {
    extents[0] * normal[0].abs() + extents[1] * normal[1].abs() + extents[2] * normal[2].abs()
}

#[cfg(test)]
#[path = "displacement_tests.rs"]
mod tests;
