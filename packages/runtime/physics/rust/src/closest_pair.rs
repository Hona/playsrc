use crate::{
    EdgeId, FeaturePlacement, FeatureSelection, FeatureTransition, FeatureWalkError,
    SurfaceFeature, SurfaceFeatureKind, SurfaceFeaturePair,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosestFeatureStatus {
    Uninitialized,
    Separated,
    Intruded,
    RecoveryLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosestFeatureUpdate {
    Cached,
    Calculated(ClosestFeatureStatus),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosestFeatureMode {
    Ordinary,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestFeatureGeometry {
    pub separation: f32,
    pub normal: [f32; 3],
    pub core_projection: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ClosestFeatureInputs<'a> {
    pub placements: [FeaturePlacement<'a>; 2],
    pub core_positions: [[f64; 3]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestFeaturePair {
    selection: FeatureSelection,
    geometry: Option<ClosestFeatureGeometry>,
    status: ClosestFeatureStatus,
    time_code: u32,
    extra_radius: f32,
}
impl ClosestFeaturePair {
    pub fn new(edges: [EdgeId; 2], extra_radius: f32) -> Result<Self, FeatureWalkError> {
        if !extra_radius.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        Ok(Self {
            selection: FeatureSelection::initial(SurfaceFeaturePair {
                first: SurfaceFeature {
                    edge: edges[0],
                    kind: SurfaceFeatureKind::Vertex,
                },
                second: SurfaceFeature {
                    edge: edges[1],
                    kind: SurfaceFeatureKind::Vertex,
                },
            }),
            geometry: None,
            status: ClosestFeatureStatus::Uninitialized,
            time_code: 0,
            extra_radius,
        })
    }
    pub fn selection(&self) -> FeatureSelection {
        self.selection
    }
    pub fn geometry(&self) -> Option<ClosestFeatureGeometry> {
        self.geometry
    }
    pub fn status(&self) -> ClosestFeatureStatus {
        self.status
    }
    pub fn time_code(&self) -> u32 {
        self.time_code
    }
    pub(crate) fn renew_separation(
        &mut self,
        separation: f32,
        projection: f32,
    ) -> Result<(), FeatureWalkError> {
        if !separation.is_finite() || !projection.is_finite() {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        if self.status != ClosestFeatureStatus::Separated {
            return Err(FeatureWalkError::UnsupportedFeaturePair);
        }
        let geometry = self
            .geometry
            .as_mut()
            .ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
        geometry.separation = separation;
        geometry.core_projection = projection;
        Ok(())
    }

    pub fn recalculate<'a>(
        &mut self,
        time_code: u32,
        mode: ClosestFeatureMode,
        maximum_transitions: usize,
        resolve: impl FnOnce() -> Result<ClosestFeatureInputs<'a>, FeatureWalkError>,
        observe: impl FnMut(FeatureTransition),
    ) -> Result<ClosestFeatureUpdate, FeatureWalkError> {
        if time_code == self.time_code {
            return Ok(ClosestFeatureUpdate::Cached);
        }
        let inputs = resolve()?;
        if inputs
            .core_positions
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(FeatureWalkError::NonFiniteTransform);
        }
        let result = crate::feature_walk::minimize_features(
            inputs.placements,
            self.selection,
            self.extra_radius,
            mode,
            maximum_transitions,
            observe,
        )?;
        let geometry = if let Some(geometry) = result.geometry {
            let first = geometry.first_side;
            let delta: [f32; 3] = std::array::from_fn(|axis| {
                (inputs.core_positions[first][axis] - inputs.core_positions[1 - first][axis]) as f32
            });
            let normal = geometry.normal;
            let projection = (delta[1] * normal[1] + delta[0] * normal[0]) + delta[2] * normal[2];
            if !projection.is_finite() {
                return Err(FeatureWalkError::NonFiniteTransform);
            }
            Some(ClosestFeatureGeometry {
                separation: geometry.separation,
                normal,
                core_projection: projection,
            })
        } else {
            self.geometry
        };
        self.selection = result.selection;
        self.geometry = geometry;
        self.status = result.status;
        self.time_code = time_code;
        Ok(ClosestFeatureUpdate::Calculated(result.status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthoredFace, FeatureTopology};
    fn topology() -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[
                AuthoredFace {
                    metadata: 1 << 12,
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
    fn distance_renewal_updates_the_retained_geometry_without_changing_feature_codes() {
        let shape = topology();
        let edge = shape.edge_id(0).unwrap();
        let mut pair = ClosestFeaturePair::new([edge; 2], 0.0).unwrap();
        let first = FeaturePlacement {
            topology: &shape,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let second = FeaturePlacement {
            position: [0.0, 0.0, 1.0],
            ..first
        };
        pair.recalculate(
            7,
            ClosestFeatureMode::Ordinary,
            1024,
            || {
                Ok(ClosestFeatureInputs {
                    placements: [first, second],
                    core_positions: [first.position, second.position],
                })
            },
            |_| {},
        )
        .unwrap();
        let selection = pair.selection();
        let normal = pair.geometry().unwrap().normal;
        pair.renew_separation(0.75, 2.0).unwrap();
        let expected = pair;
        assert_eq!(
            pair.geometry(),
            Some(ClosestFeatureGeometry {
                separation: 0.75,
                normal,
                core_projection: 2.0
            })
        );
        assert_eq!(pair.selection(), selection);
        assert_eq!(pair.time_code(), 7);
        assert_eq!(
            pair.recalculate(
                7,
                ClosestFeatureMode::Ordinary,
                0,
                || panic!("cached geometry must not resolve poses"),
                |_| panic!("no walk")
            ),
            Ok(ClosestFeatureUpdate::Cached)
        );
        assert_eq!(pair, expected);
        assert!(pair.renew_separation(f32::NAN, 2.0).is_err());
        assert_eq!(pair, expected);
    }
    #[test]
    fn cached_codes_skip_pose_resolution_and_intrusion_preserves_unwritten_geometry() {
        let shape = topology();
        let edge = shape.edge_id(0).unwrap();
        let mut pair = ClosestFeaturePair::new([edge; 2], 0.0).unwrap();
        assert_eq!(
            pair.recalculate(
                0,
                ClosestFeatureMode::Ordinary,
                0,
                || panic!("code-zero cache must not resolve"),
                |_| panic!("no walk")
            ),
            Ok(ClosestFeatureUpdate::Cached)
        );
        let first = FeaturePlacement {
            topology: &shape,
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let second = FeaturePlacement {
            position: [0.0, 0.0, 1.0],
            ..first
        };
        let input = ClosestFeatureInputs {
            placements: [first, second],
            core_positions: [first.position, second.position],
        };
        assert_eq!(
            pair.recalculate(1, ClosestFeatureMode::Ordinary, 1024, || Ok(input), |_| {})
                .unwrap(),
            ClosestFeatureUpdate::Calculated(ClosestFeatureStatus::Separated)
        );
        let geometry = pair.geometry();
        let before = pair;
        assert!(
            pair.recalculate(
                2,
                ClosestFeatureMode::Ordinary,
                1024,
                || Err(FeatureWalkError::NonFiniteTransform),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(pair, before);
        assert_eq!(
            pair.recalculate(
                1,
                ClosestFeatureMode::Ordinary,
                0,
                || panic!("cached pose"),
                |_| panic!("cached walk")
            )
            .unwrap(),
            ClosestFeatureUpdate::Cached
        );
        assert_eq!(
            pair.recalculate(
                2,
                ClosestFeatureMode::Ordinary,
                1024,
                || Ok(ClosestFeatureInputs {
                    placements: [first; 2],
                    core_positions: [first.position; 2]
                }),
                |_| {}
            )
            .unwrap(),
            ClosestFeatureUpdate::Calculated(ClosestFeatureStatus::Intruded)
        );
        assert_eq!(pair.geometry(), geometry);
        assert_eq!(pair.time_code(), 2);
    }
}
