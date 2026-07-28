use crate::{Error, ErrorCode, error, lightmap_uv, uv};
use playsrc_bsp::{DispInfo, DispVert, Face, TextureData, TextureInfo};

const DISP_TRI_TAG_REMOVE: u16 = 0x20;

#[derive(Clone, Debug, PartialEq)]
pub struct DisplacementSurface {
    pub source: usize,
    pub power: u8,
    pub minimum_tessellation: i32,
    pub smoothing_angle: f32,
    pub contents: u32,
    pub map_face: u16,
    pub start_position: [f32; 3],
    pub vertex_start: u32,
    pub triangle_start: u32,
    pub allowed_vertices: [u32; 10],
    pub edge_neighbors: [playsrc_bsp::DispNeighbor; 4],
    pub corner_neighbors: [playsrc_bsp::DispCornerNeighbors; 4],
    pub triangle_tags: Vec<u16>,
}

pub(crate) struct Geometry {
    pub descriptor: DisplacementSurface,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub alpha: Vec<f32>,
    pub uv: Vec<[f32; 2]>,
    pub lightmap_uv: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
}

pub(crate) struct Inputs<'a> {
    pub face_index: usize,
    pub face: &'a Face,
    pub corners: &'a [[f32; 3]],
    pub info: &'a TextureInfo,
    pub material: &'a TextureData,
    pub displacements: &'a [DispInfo],
    pub vertices: &'a [DispVert],
    pub triangle_tags: &'a [u16],
}

pub(crate) fn compile(inputs: Inputs<'_>) -> Result<Geometry, Error> {
    let source = usize::try_from(inputs.face.displacement_info_index)
        .map_err(|_| error(ErrorCode::InvalidReference, Some(inputs.face_index)))?;
    let displacement = inputs
        .displacements
        .get(source)
        .ok_or_else(|| error(ErrorCode::InvalidReference, Some(inputs.face_index)))?;
    if displacement.map_face as usize != inputs.face_index
        || inputs.corners.len() != 4
        || !(2..=4).contains(&displacement.power)
        || displacement.corner_neighbors.iter().any(|corner| {
            corner.neighbor_count > 4
                || corner.neighbors[..usize::from(corner.neighbor_count)]
                    .iter()
                    .any(|neighbor| {
                        *neighbor != u16::MAX
                            && usize::from(*neighbor) >= inputs.displacements.len()
                    })
        })
        || displacement.edge_neighbors.iter().any(|edge| {
            edge.sub_neighbors.iter().any(|neighbor| {
                neighbor.neighbor != u16::MAX
                    && (usize::from(neighbor.neighbor) >= inputs.displacements.len()
                        || neighbor.orientation > 3
                        || neighbor.span > 2
                        || neighbor.neighbor_span > 2)
            })
        })
    {
        return Err(error(ErrorCode::InvalidRange, Some(inputs.face_index)));
    }
    let start_position = vector(displacement.start_position);
    let smoothing_angle = displacement.smoothing_angle.value();
    if start_position
        .iter()
        .chain([smoothing_angle].iter())
        .any(|value| !value.is_finite())
    {
        return Err(error(ErrorCode::NonFinite, Some(inputs.face_index)));
    }
    let side = 1_usize
        .checked_shl(displacement.power as u32)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(inputs.face_index)))?;
    let vertex_count = side
        .checked_mul(side)
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(inputs.face_index)))?;
    let triangle_count = (side - 1)
        .checked_mul(side - 1)
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(inputs.face_index)))?;
    let vertex_start = usize::try_from(displacement.vertex_start)
        .map_err(|_| error(ErrorCode::InvalidRange, Some(inputs.face_index)))?;
    let triangle_start = usize::try_from(displacement.triangle_start)
        .map_err(|_| error(ErrorCode::InvalidRange, Some(inputs.face_index)))?;
    let vertex_end = vertex_start
        .checked_add(vertex_count)
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(inputs.face_index)))?;
    let triangle_end = triangle_start
        .checked_add(triangle_count)
        .ok_or_else(|| error(ErrorCode::BoundExceeded, Some(inputs.face_index)))?;
    let samples = inputs
        .vertices
        .get(vertex_start..vertex_end)
        .ok_or_else(|| error(ErrorCode::InvalidRange, Some(inputs.face_index)))?;
    let tags = inputs
        .triangle_tags
        .get(triangle_start..triangle_end)
        .ok_or_else(|| error(ErrorCode::InvalidRange, Some(inputs.face_index)))?;

    let start_corner = inputs
        .corners
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            distance_squared(left, &start_position)
                .total_cmp(&distance_squared(right, &start_position))
        })
        .map(|(index, _)| index)
        .ok_or_else(|| error(ErrorCode::InvalidRange, Some(inputs.face_index)))?;
    let corners: [[f32; 3]; 4] =
        std::array::from_fn(|index| inputs.corners[(start_corner + index) % 4]);
    let corner_uv = corners.map(|point| {
        uv(
            &point,
            &inputs.info.texture_vectors,
            inputs.material.width,
            inputs.material.height,
        )
    });
    let corner_lightmap = corners.map(|point| lightmap_uv(&point, inputs.info, inputs.face));
    let mut positions = Vec::with_capacity(vertex_count);
    let mut texture_coordinates = Vec::with_capacity(vertex_count);
    let mut lightmap_coordinates = Vec::with_capacity(vertex_count);
    let mut alpha_values = Vec::with_capacity(vertex_count);
    let interval = 1.0 / (side - 1) as f32;
    for column in 0..side {
        let along = column as f32 * interval;
        let left = lerp3(corners[0], corners[1], along);
        let right = lerp3(corners[3], corners[2], along);
        let left_uv = lerp2(corner_uv[0], corner_uv[1], along);
        let right_uv = lerp2(corner_uv[3], corner_uv[2], along);
        let left_lightmap = lerp2(corner_lightmap[0], corner_lightmap[1], along);
        let right_lightmap = lerp2(corner_lightmap[3], corner_lightmap[2], along);
        for row in 0..side {
            let across = row as f32 * interval;
            let sample = samples[column * side + row];
            let vector = vector(sample.vector);
            let distance = sample.distance.value();
            let alpha = sample.alpha.value();
            if vector
                .iter()
                .chain([distance, alpha].iter())
                .any(|value| !value.is_finite())
            {
                return Err(error(ErrorCode::NonFinite, Some(inputs.face_index)));
            }
            let flat = lerp3(left, right, across);
            positions.push([
                flat[0] + vector[0] * distance,
                flat[1] + vector[1] * distance,
                flat[2] + vector[2] * distance,
            ]);
            texture_coordinates.push(lerp2(left_uv, right_uv, across));
            lightmap_coordinates.push(lerp2(left_lightmap, right_lightmap, across));
            alpha_values.push(alpha);
        }
    }
    let all_triangles = source_triangles(side);
    let normals = source_normals(side, &positions);
    let mut triangles = all_triangles
        .into_iter()
        .zip(tags)
        .filter_map(|(triangle, tag)| (tag & DISP_TRI_TAG_REMOVE == 0).then_some(triangle))
        .collect::<Vec<_>>();
    crate::normalize_triangle_winding(&positions, &normals, &mut triangles);
    Ok(Geometry {
        descriptor: DisplacementSurface {
            source,
            power: displacement.power as u8,
            minimum_tessellation: displacement.minimum_tessellation,
            smoothing_angle,
            contents: displacement.contents as u32,
            map_face: displacement.map_face,
            start_position,
            vertex_start: vertex_start as u32,
            triangle_start: triangle_start as u32,
            allowed_vertices: displacement.allowed_vertices,
            edge_neighbors: displacement.edge_neighbors,
            corner_neighbors: displacement.corner_neighbors,
            triangle_tags: tags.to_vec(),
        },
        positions,
        normals,
        alpha: alpha_values,
        uv: texture_coordinates,
        lightmap_uv: lightmap_coordinates,
        triangles,
    })
}

fn source_triangles(side: usize) -> Vec<[u32; 3]> {
    let mut triangles = Vec::with_capacity((side - 1) * (side - 1) * 2);
    for column in 0..side - 1 {
        for row in 0..side - 1 {
            let index = column * side + row;
            if index % 2 == 1 {
                triangles.push([index as u32, (index + side) as u32, (index + 1) as u32]);
                triangles.push([
                    (index + 1) as u32,
                    (index + side) as u32,
                    (index + side + 1) as u32,
                ]);
            } else {
                triangles.push([
                    index as u32,
                    (index + side) as u32,
                    (index + side + 1) as u32,
                ]);
                triangles.push([index as u32, (index + side + 1) as u32, (index + 1) as u32]);
            }
        }
    }
    triangles
}

fn source_normals(side: usize, positions: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let point = |column: usize, row: usize| positions[column * side + row];
    let mut normals = Vec::with_capacity(positions.len());
    for column in 0..side {
        for row in 0..side {
            let mut sum = [0.0; 3];
            let mut count = 0;
            let mut add = |base: [f32; 3], first: [f32; 3], second: [f32; 3]| {
                sum = add3(sum, normalize(cross(sub3(first, base), sub3(second, base))));
                count += 1;
            };
            if column + 1 < side && row + 1 < side {
                add(
                    point(column, row),
                    point(column, row + 1),
                    point(column + 1, row),
                );
                add(
                    point(column, row + 1),
                    point(column + 1, row + 1),
                    point(column + 1, row),
                );
            }
            if column + 1 < side && row > 0 {
                add(
                    point(column, row - 1),
                    point(column, row),
                    point(column + 1, row - 1),
                );
                add(
                    point(column, row),
                    point(column + 1, row),
                    point(column + 1, row - 1),
                );
            }
            if column > 0 && row > 0 {
                add(
                    point(column - 1, row - 1),
                    point(column - 1, row),
                    point(column, row - 1),
                );
                add(
                    point(column - 1, row),
                    point(column, row),
                    point(column, row - 1),
                );
            }
            if column > 0 && row + 1 < side {
                add(
                    point(column - 1, row),
                    point(column - 1, row + 1),
                    point(column, row),
                );
                add(
                    point(column - 1, row + 1),
                    point(column, row + 1),
                    point(column, row),
                );
            }
            normals.push(if count == 0 {
                [0.0; 3]
            } else {
                scale3(sum, 1.0 / count as f32)
            });
        }
    }
    normals
}

fn vector(value: playsrc_bsp::Vector3) -> [f32; 3] {
    [value.x.value(), value.y.value(), value.z.value()]
}

fn distance_squared(left: &[f32; 3], right: &[f32; 3]) -> f32 {
    let value = sub3(*left, *right);
    value[0] * value[0] + value[1] * value[1] + value[2] * value[2]
}

fn lerp3(left: [f32; 3], right: [f32; 3], amount: f32) -> [f32; 3] {
    add3(left, scale3(sub3(right, left), amount))
}

fn lerp2(left: [f32; 2], right: [f32; 2], amount: f32) -> [f32; 2] {
    [
        left[0] + (right[0] - left[0]) * amount,
        left[1] + (right[1] - left[1]) * amount,
    ]
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(value: [f32; 3], amount: f32) -> [f32; 3] {
    [value[0] * amount, value[1] * amount, value[2] * amount]
}

fn cross(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length > 0.0 {
        scale3(value, length.recip())
    } else {
        [0.0; 3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_cell_parity_order_and_normal_accumulation_are_deterministic() {
        assert_eq!(
            source_triangles(3),
            vec![
                [0, 3, 4],
                [0, 4, 1],
                [1, 4, 2],
                [2, 4, 5],
                [3, 6, 4],
                [4, 6, 7],
                [4, 7, 8],
                [4, 8, 5]
            ]
        );
        let positions = (0..3)
            .flat_map(|column| (0..3).map(move |row| [row as f32, column as f32, 0.0]))
            .collect::<Vec<_>>();
        let normals = source_normals(3, &positions);
        assert_eq!(normals, vec![[0.0, 0.0, 1.0]; 9]);
    }
}
