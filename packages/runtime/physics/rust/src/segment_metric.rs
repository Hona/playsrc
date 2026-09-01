use crate::{EdgeId, FeaturePlacement, FeatureTopology, FeatureWalkError};

const BIAS: f64 = f32::MIN_POSITIVE as f64;
const LINE_REGULARIZER: f64 = 1.0e-18_f32 as f64;

impl FeatureTopology {
    pub(crate) fn point_line_distance_squared(
        &self,
        edge: EdgeId,
        point: [f64; 3],
    ) -> Result<f64, FeatureWalkError> {
        finite(line_distance(segment_points(self, edge)?, point))
    }
    pub fn segment_region_checks(
        &self,
        edge: EdgeId,
        point: [f64; 3],
    ) -> Result<[f32; 2], FeatureWalkError> {
        let endpoints = segment_points(self, edge)?;
        let checks = region_checks(endpoints, point);
        if point.iter().any(|v| !v.is_finite()) || checks.iter().any(|v| !v.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(checks)
    }
    pub fn point_segment_distance_squared(
        &self,
        edge: EdgeId,
        point: [f64; 3],
    ) -> Result<f64, FeatureWalkError> {
        let endpoints = segment_points(self, edge)?;
        let checks = self.segment_region_checks(edge, point)?;
        let result = if checks.iter().all(|v| v.to_bits() & 0x8000_0000 == 0) {
            line_distance(endpoints, point)
        } else {
            let endpoint = usize::from(checks[0] >= 0.0);
            let delta = sub(point, endpoints[endpoint].map(f64::from));
            dot(delta, delta)
        };
        finite(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentPairMetric {
    pub regions: [f32; 4],
    pub squared_distance: f64,
}

impl SegmentPairMetric {
    pub fn evaluate(
        placements: [FeaturePlacement<'_>; 2],
        edges: [EdgeId; 2],
    ) -> Result<Self, FeatureWalkError> {
        let geometry = SegmentPairGeometry::new(placements, edges)?;
        let regions = geometry.regions()?;
        Ok(Self {
            regions,
            squared_distance: geometry.squared_distance(regions)?,
        })
    }
}

pub(crate) struct SegmentPairGeometry<'a> {
    placements: [FeaturePlacement<'a>; 2],
    edges: [EdgeId; 2],
    pub first_points: [[f64; 3]; 2],
    pub second_points: [[f32; 3]; 2],
    first_direction: [f64; 3],
    second_direction: [f64; 3],
    normal: [f64; 3],
    normal_squared: f64,
}

impl<'a> SegmentPairGeometry<'a> {
    pub(crate) fn new(
        placements: [FeaturePlacement<'a>; 2],
        edges: [EdgeId; 2],
    ) -> Result<Self, FeatureWalkError> {
        for p in placements {
            if p.position
                .iter()
                .chain(p.orientation.iter())
                .any(|v| !v.is_finite())
            {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
        }
        let [first, second] = placements;
        let a = [
            second.local_point(first.world_point(edges[0])?),
            second.local_point(first.world_point(first.topology.next(edges[0])?)?),
        ];
        let b = segment_points(second.topology, edges[1])?;
        let a_direction = second.local_vector(first.world_vector(first.edge_direction(edges[0])?));
        let b_direction = second.edge_direction(edges[1])?;
        let normal = cross(a_direction, b_direction);
        let normal_squared = dot(normal, normal);
        Ok(Self {
            placements,
            edges,
            first_points: a,
            second_points: b,
            first_direction: a_direction,
            second_direction: b_direction,
            normal,
            normal_squared,
        })
    }
    fn second_in_first(&self) -> Result<[[f64; 3]; 2], FeatureWalkError> {
        let [first, second] = self.placements;
        let edge = self.edges[1];
        Ok([
            first.local_point(second.world_point(edge)?),
            first.local_point(second.world_point(second.topology.next(edge)?)?),
        ])
    }
    pub(crate) fn regions(&self) -> Result<[f32; 4], FeatureWalkError> {
        let [first, _] = self.placements;
        let edges = self.edges;
        let a = self.first_points;
        let b = self.second_points;
        let a_direction = self.first_direction;
        let b_direction = self.second_direction;
        let normal = self.normal;
        let normal_squared = self.normal_squared;
        let regions = if self.direct_regions() {
            let plane = cross(a_direction, normal);
            let start = dot(plane, b[0].map(f64::from));
            let end = dot(plane, b[1].map(f64::from));
            let from_first = dot(plane, a[0]);
            let second_checks = [
                ((start - from_first) * (start - end)) as f32,
                ((from_first - end) * (start - end)) as f32,
            ];
            let plane = cross(b_direction, normal);
            let start = dot(plane, a[0]);
            let end = dot(plane, a[1]);
            let from_second = dot(plane, b[0].map(f64::from));
            [
                ((start - from_second) * (start - end) + BIAS) as f32,
                ((from_second - end) * (start - end) + BIAS) as f32,
                second_checks[0],
                second_checks[1],
            ]
        } else {
            const SAMPLES: [f32; 11] = [
                -1.0, 0.5, 2.0, 0.0, 1.0, -0.001, 0.001, 0.999, 1.001, -1.0e-6, 1.0e-6,
            ];
            let mut best = 1.0e101;
            let mut result = None;
            for factor in SAMPLES {
                let point = interpolate(a, f64::from(factor));
                let squared = line_distance(b, point);
                if squared < best {
                    best = squared;
                    let other = region_checks(b, point);
                    result = Some([factor, 1.0 - factor, other[0], other[1]]);
                }
            }
            let reverse = self.second_in_first()?;
            let original = segment_points(first.topology, edges[0])?;
            for factor in SAMPLES.into_iter().take(9) {
                let point = interpolate(reverse, f64::from(factor));
                let squared = line_distance(original, point);
                if squared < best {
                    best = squared;
                    let direction: [f64; 3] = std::array::from_fn(|axis| {
                        f64::from(original[1][axis] - original[0][axis])
                    });
                    let start = sub(original[0].map(f64::from), point);
                    let end = sub(original[1].map(f64::from), point);
                    let first_check = (BIAS - dot(start, direction)) as f32;
                    let second_check = ((end[1] * direction[1] + end[0] * direction[0])
                        + (end[2] * direction[2] + BIAS))
                        as f32;
                    result = Some([first_check, second_check, factor, 1.0 - factor]);
                }
            }
            result.ok_or(FeatureWalkError::NonFiniteTransform)?
        };
        if regions.iter().any(|v| !v.is_finite()) || !normal_squared.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(regions)
    }
    pub(crate) fn direct_regions(&self) -> bool {
        self.normal_squared > 9.999999999999999e-19
    }
    pub(crate) fn line_squared_distance(&self) -> Result<f64, FeatureWalkError> {
        if self.normal_squared > 1.0e-24 {
            let projected = dot(self.normal, self.first_points[0])
                - dot(self.normal, self.second_points[0].map(f64::from));
            finite(projected * projected / self.normal_squared)
        } else {
            self.placements[1]
                .topology
                .point_segment_distance_squared(self.edges[1], self.first_points[0])
        }
    }
    fn squared_distance(&self, regions: [f32; 4]) -> Result<f64, FeatureWalkError> {
        let [first, second] = self.placements;
        let edges = self.edges;
        let a = self.first_points;
        let inside = |checks: &[f32]| checks.iter().all(|v| v.to_bits() & 0x8000_0000 == 0);
        let squared_distance = if inside(&regions[2..]) {
            if regions[0] <= 0.0 {
                second
                    .topology
                    .point_segment_distance_squared(edges[1], a[0])?
            } else if regions[1] <= 0.0 {
                second
                    .topology
                    .point_segment_distance_squared(edges[1], a[1])?
            } else {
                self.line_squared_distance()?
            }
        } else if inside(&regions[..2]) {
            let endpoint = if regions[2] <= 0.0 {
                0
            } else if regions[3] <= 0.0 {
                1
            } else {
                return Err(FeatureWalkError::UnsupportedFeaturePair);
            };
            first
                .topology
                .point_segment_distance_squared(edges[0], self.second_in_first()?[endpoint])?
        } else {
            let reverse = self.second_in_first()?;
            let mut best = 1.0e101;
            for index in 0..2 {
                let from_second = first
                    .topology
                    .point_segment_distance_squared(edges[0], reverse[index])?;
                let from_first = second
                    .topology
                    .point_segment_distance_squared(edges[1], a[index])?;
                if from_second < best {
                    best = from_second;
                }
                if from_first < best {
                    best = from_first;
                }
            }
            best
        };
        finite(squared_distance)
    }
}

fn segment_points(
    topology: &FeatureTopology,
    edge: EdgeId,
) -> Result<[[f32; 3]; 2], FeatureWalkError> {
    let edge = topology.edge(edge)?;
    Ok([
        topology.points()[edge.start as usize],
        topology.points()[edge.end as usize],
    ])
}
fn region_checks(endpoints: [[f32; 3]; 2], point: [f64; 3]) -> [f32; 2] {
    let direction = std::array::from_fn(|axis| f64::from(endpoints[1][axis] - endpoints[0][axis]));
    [
        (dot(direction, sub(point, endpoints[0].map(f64::from))) + BIAS) as f32,
        (dot(direction, sub(endpoints[1].map(f64::from), point)) + BIAS) as f32,
    ]
}
fn line_distance(endpoints: [[f32; 3]; 2], point: [f64; 3]) -> f64 {
    let delta = sub(point, endpoints[0].map(f64::from));
    let precise = sub(endpoints[1].map(f64::from), endpoints[0].map(f64::from));
    let normal = cross(delta, precise);
    let denominator: [f64; 3] =
        std::array::from_fn(|axis| f64::from(endpoints[0][axis] - endpoints[1][axis]));
    dot(normal, normal) / (dot(denominator, denominator) + LINE_REGULARIZER)
}
fn finite(value: f64) -> Result<f64, FeatureWalkError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FeatureWalkError::NonFiniteTransform)
    }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn interpolate(points: [[f64; 3]; 2], factor: f64) -> [f64; 3] {
    std::array::from_fn(|i| (1.0 - factor) * points[0][i] + factor * points[1][i])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthoredFace;
    fn triangle(length: f32) -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            vec![[0.0; 3], [length, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap()
    }
    #[test]
    fn point_distance_keeps_endpoint_branches_and_the_native_line_regularizer() {
        let shape = triangle(1.0);
        let edge = shape.edge_id(0).unwrap();
        assert_eq!(
            shape
                .point_segment_distance_squared(edge, [0.5, 0.5, 0.0])
                .unwrap(),
            0.25
        );
        assert_eq!(
            shape
                .point_segment_distance_squared(edge, [2.0, 0.5, 0.0])
                .unwrap(),
            1.25
        );
        let short = triangle(1.0e-10);
        let length = f64::from(1.0e-10_f32);
        let squared = length * length;
        assert_eq!(
            short
                .point_segment_distance_squared(edge, [length * 0.5, 1.0, 0.0])
                .unwrap(),
            squared / (squared + f64::from(1.0e-18_f32))
        );
        assert!(
            shape
                .point_segment_distance_squared(edge, [f64::NAN; 3])
                .is_err()
        );
    }
    #[test]
    fn parallel_region_samples_keep_the_first_strict_minimum() {
        let shape = triangle(1.0);
        let edge = shape.edge_id(0).unwrap();
        let first = FeaturePlacement {
            topology: &shape,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let second = FeaturePlacement {
            position: [0.0, 1.0, 0.0],
            ..first
        };
        let result = SegmentPairMetric::evaluate([first, second], [edge; 2]).unwrap();
        assert_eq!(result.regions, [-1.0, 2.0, -1.0, 2.0]);
        assert_eq!(result.squared_distance, 1.0);
    }
}
