use crate::{EdgeId, FeatureTopology, FeatureWalkError, TopologyError};

impl FeatureTopology {
    pub(crate) fn unit_face_plane(
        &self,
        edge: EdgeId,
    ) -> Result<([f64; 3], f64), FeatureWalkError> {
        let point = |edge| -> Result<[f64; 3], FeatureWalkError> {
            Ok(self.points()[self.edge(edge)?.start as usize].map(f64::from))
        };
        let origin = point(edge)?;
        let next = point(self.next(edge)?)?;
        let previous = point(self.previous(edge)?)?;
        let a: [f64; 3] = std::array::from_fn(|axis| next[axis] - origin[axis]);
        let b: [f64; 3] = std::array::from_fn(|axis| previous[axis] - origin[axis]);
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let offset = -((normal[1] * origin[1] + normal[0] * origin[0]) + normal[2] * origin[2]);
        let inverse =
            1.0 / ((normal[0] * normal[0] + normal[1] * normal[1]) + normal[2] * normal[2]).sqrt();
        let normal = normal.map(|value| value * inverse);
        let offset = offset * inverse;
        if normal.iter().any(|value| !value.is_finite()) || !offset.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok((normal, offset))
    }
    /// Unnormalized region tests in directed-edge order, followed by the Gram determinant.
    pub fn triangle_region_checks(
        &self,
        edge: EdgeId,
        point: [f64; 3],
    ) -> Result<[f32; 4], FeatureWalkError> {
        if point.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let selected = self.edge(edge)?;
        let points = std::array::from_fn::<_, 3, _>(|index| {
            let edge = self.edges()[selected.face * 3 + index];
            self.points()[edge.start as usize]
        });
        let primary: [f64; 3] =
            std::array::from_fn(|axis| f64::from(points[0][axis] - points[1][axis]));
        let secondary: [f64; 3] =
            std::array::from_fn(|axis| f64::from(points[2][axis] - points[1][axis]));
        let offset: [f64; 3] = std::array::from_fn(|axis| point[axis] - f64::from(points[1][axis]));
        let dot = |a: [f64; 3], b: [f64; 3]| (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2];
        let aa = dot(primary, primary);
        let bb = dot(secondary, secondary);
        let ab = dot(secondary, primary);
        let ap = dot(offset, primary);
        let bp = dot(offset, secondary);
        let determinant = bb * aa - ab * ab;
        let first = ap * bb - bp * ab;
        let second = bp * aa - ap * ab;
        let bias = f64::from(f32::MIN_POSITIVE);
        let mut result = [0.0; 4];
        let rotation = [0, 2, 1, 0, 2];
        result[rotation[selected.position]] = (second + bias) as f32;
        result[rotation[selected.position + 1]] = ((determinant - first) - second + bias) as f32;
        result[rotation[selected.position + 2]] = (first + bias) as f32;
        result[3] = determinant as f32;
        if result.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(result)
    }

    pub fn recover_interior_face(
        &self,
        edge: EdgeId,
        point: [f64; 3],
        mut observe: impl FnMut(EdgeId),
    ) -> Result<EdgeId, FeatureWalkError> {
        if point.iter().any(|value| !value.is_finite()) {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let initial = self.edge(edge)?;
        let metadata = self.face_metadata(edge)?;
        if (metadata & 0xfff) as usize != initial.face {
            return Err(TopologyError::InvalidRecoveryFace.into());
        }
        let face = ((metadata >> 12) & 0xfff) as usize;
        let mut current = self
            .edge_id(face * 3)
            .ok_or(TopologyError::InvalidRecoveryFace)?;
        let mut visited = vec![false; self.edges().len() / 3];
        loop {
            observe(current);
            visited[self.edge(current)?.face] = true;
            let checks = self.triangle_region_checks(current, point)?;
            let mut edge = current;
            let mut next = None;
            for check in checks.into_iter().take(3) {
                if check <= 0.0 {
                    let opposite = self.edge(edge)?.opposite;
                    if !visited[self.edge(opposite)?.face] {
                        next = Some(opposite);
                        break;
                    }
                }
                edge = self.next(edge)?;
            }
            let Some(next) = next else {
                return Ok(current);
            };
            current = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthoredFace;
    fn fixture(pierce: u32) -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[
                AuthoredFace {
                    metadata: pierce << 12,
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
    fn authored_recovery_seed_and_positive_zero_boundary_bias_are_preserved() {
        let topology = fixture(1);
        let edge = topology.edge_id(0).unwrap();
        let mut visits = Vec::new();
        let result = topology
            .recover_interior_face(edge, [0.25, 0.25, 0.0], |edge| visits.push(edge.index()))
            .unwrap();
        assert_eq!(result.index(), 3);
        assert_eq!(visits, [3]);
        let checks = topology.triangle_region_checks(edge, [0.0; 3]).unwrap();
        assert_eq!(
            checks
                .iter()
                .filter(|value| value.to_bits() == f32::MIN_POSITIVE.to_bits())
                .count(),
            2
        );
    }
    #[test]
    fn invalid_points_and_authored_links_fail_before_observing_a_walk() {
        let topology = fixture(5);
        let edge = topology.edge_id(0).unwrap();
        let mut visits = 0;
        assert_eq!(
            topology.recover_interior_face(edge, [0.0; 3], |_| visits += 1),
            Err(FeatureWalkError::Topology(
                TopologyError::InvalidRecoveryFace
            ))
        );
        let topology = fixture(1);
        assert_eq!(
            topology.recover_interior_face(edge, [f64::NAN; 3], |_| visits += 1),
            Err(FeatureWalkError::NonFiniteTransform)
        );
        assert_eq!(visits, 0);
    }
}
