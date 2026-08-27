// Source 1 SDK License applies; see ../../../../../LICENSE.source-sdk-2013.
// Behavior is derived from Valve Source SDK 2013 dispcoll_common and collisionutils contracts.
use crate::{
    DisplacementFeature, Error, ErrorCode, Hull, Plane, SurfaceIdentity, Trace, World, add, dot,
    error, interpolate, scale, sub,
};

const DIST_EPSILON: f32 = 1.0 / 32.0;
const INVALID_FRACTION: f32 = -99_999.9;
const SURFACE_FLAG: u16 = 1;
const SURFACE_PROP_1: u16 = 1 << 3;
const SURFACE_PROP_2: u16 = 1 << 4;
const NO_HULL_COLLISION: u32 = 1 << 2;
const NO_RAY_COLLISION: u32 = 1 << 3;

#[derive(Clone, Debug)]
pub(crate) struct Tree {
    source: usize,
    parent_face: usize,
    contents: u32,
    surface_flags: u32,
    bounds: Hull,
    triangles: Vec<Triangle>,
}

#[derive(Clone, Debug)]
struct Triangle {
    index: usize,
    points: [[f32; 3]; 3],
    normal: [f32; 3],
    distance: f32,
    bounds: Hull,
    flags: u16,
    surface: SurfaceIdentity,
}

pub(crate) fn build(world: &World) -> Result<Vec<Tree>, Error> {
    let mut trees = Vec::with_capacity(world.displacement_inputs.len());
    for (item, (input, patch)) in world
        .displacement_inputs
        .iter()
        .zip(&world.displacements)
        .enumerate()
    {
        let mut triangles = Vec::with_capacity(input.triangles.len());
        let mut bounds = empty_bounds();
        for (index, vertices) in input.triangles.iter().copied().enumerate() {
            let points = vertices.map(|vertex| input.positions[vertex as usize]);
            let normal = normalize(cross(sub(points[2], points[0]), sub(points[1], points[0])))
                .ok_or_else(|| error(ErrorCode::InvalidRange, Some(item)))?;
            let triangle_bounds = points.into_iter().fold(empty_bounds(), include);
            bounds = union(bounds, triangle_bounds);
            let secondary = input.use_secondary_surface[index];
            let surface = if secondary {
                input
                    .secondary_surface
                    .expect("validated secondary surface")
            } else {
                input.primary_surface
            };
            triangles.push(Triangle {
                index,
                points,
                normal,
                distance: dot(normal, points[0]),
                bounds: triangle_bounds,
                flags: (input.triangle_tags[index] & 0x1f)
                    | SURFACE_FLAG
                    | if secondary {
                        SURFACE_PROP_2
                    } else {
                        SURFACE_PROP_1
                    },
                surface,
            });
        }
        for axis in 0..3 {
            bounds.mins[axis] -= 1.0;
            bounds.maxs[axis] += 1.0;
        }
        let minimum = patch.minimum_tessellation as u32;
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
        });
    }
    Ok(trees)
}

pub(crate) fn trace(
    world: &World,
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    mask: u32,
    output: &mut Trace,
) -> Result<(), Error> {
    if output.all_solid || world.displacement_trees.is_empty() {
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

    for tree in &world.displacement_trees {
        if tree.contents & mask == 0 || !intersects(query_bounds, tree.bounds) {
            continue;
        }
        if !swept {
            if tree.surface_flags & NO_HULL_COLLISION != 0 {
                continue;
            }
            for triangle in &tree.triangles {
                if intersects(query_bounds, triangle.bounds)
                    && box_triangle_overlap(ray_start, extents, triangle)
                {
                    output.fraction = 0.0;
                    output.fraction_left_solid = 0.0;
                    output.start_solid = true;
                    output.all_solid = true;
                    set_hit(output, tree, triangle, None, start);
                    return Ok(());
                }
            }
            continue;
        }
        if point {
            if tree.surface_flags & NO_RAY_COLLISION != 0 {
                continue;
            }
            for triangle in &tree.triangles {
                if !intersects(query_bounds, triangle.bounds) {
                    continue;
                }
                if let Some(fraction) = ray_triangle(ray_start, delta, triangle)
                    && fraction < output.fraction
                {
                    let end_position = interpolate(start, end, fraction);
                    set_hit(output, tree, triangle, Some(fraction), end_position);
                }
            }
        } else {
            if tree.surface_flags & NO_HULL_COLLISION != 0 {
                continue;
            }
            for triangle in &tree.triangles {
                if !intersects(query_bounds, expand(triangle.bounds, extents)) {
                    continue;
                }
                if let Some((fraction, normal, distance)) =
                    sweep_box_triangle(ray_start, delta, extents, triangle)
                    && fraction < output.fraction
                {
                    let end_position = interpolate(start, end, fraction);
                    set_hit_with_plane(
                        output,
                        tree,
                        triangle,
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
    output.displacement_flags = triangle.flags;
    output.surface = Some(triangle.surface);
    output.hit = None;
    output.displacement = Some(DisplacementFeature {
        source: tree.source,
        parent_face: tree.parent_face,
        triangle: triangle.index,
    });
}

fn set_hit_with_plane(
    output: &mut Trace,
    tree: &Tree,
    triangle: &Triangle,
    fraction: f32,
    normal: [f32; 3],
    distance: f32,
    end: [f32; 3],
) {
    set_hit(output, tree, triangle, Some(fraction), end);
    output.plane = Some(Plane {
        normal,
        distance,
        kind: 3,
    });
}

fn ray_triangle(start: [f32; 3], delta: [f32; 3], triangle: &Triangle) -> Option<f32> {
    if dot(triangle.normal, delta) >= 0.0 {
        return None;
    }
    let edge1 = sub(triangle.points[2], triangle.points[0]);
    let edge2 = sub(triangle.points[1], triangle.points[0]);
    let direction_cross_edge2 = cross(delta, edge2);
    let denominator = dot(direction_cross_edge2, edge1);
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    let inverse = 1.0 / denominator;
    let origin = sub(start, triangle.points[0]);
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

fn sweep_box_triangle(
    start: [f32; 3],
    delta: [f32; 3],
    extents: [f32; 3],
    triangle: &Triangle,
) -> Option<(f32, [f32; 3], f32)> {
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
        let minimum = triangle.bounds.mins[axis];
        let maximum = triangle.bounds.maxs[axis];
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
    let edges = [
        sub(triangle.points[1], triangle.points[0]),
        sub(triangle.points[2], triangle.points[1]),
        sub(triangle.points[0], triangle.points[2]),
    ];
    for axis in 0..3 {
        for (edge_index, edge) in edges.iter().copied().enumerate() {
            let on = triangle.points[edge_index];
            let off = triangle.points[(edge_index + 2) % 3];
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
            if dot(normal, off) > distance && (dot(normal, off) - distance).abs() >= DIST_EPSILON {
                normal = scale(normal, -1.0);
                distance = -distance;
            }
            let expanded = distance + dot_abs(extents, normal);
            if !resolve_plane(start, delta, normal, expanded, distance, &mut helper) {
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

fn box_triangle_overlap(center: [f32; 3], extents: [f32; 3], triangle: &Triangle) -> bool {
    let points = triangle.points.map(|point| sub(point, center));
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
