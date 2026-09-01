use crate::{
    CollisionMotion, CollisionMotionBounds, ContactTolerances, ContinuousRoot, EdgePairEventKind,
    EdgePairEventQuery, EdgePairSeparation, EventTimingKind, FeatureEventError, FeatureEventKind,
    FeatureMotion, FeaturePlacement, FeatureSelection, FeatureTopology, PhysicalShape,
    SectorThreshold, SurfaceFeatureKind, SurfaceFeaturePair, VertexEdgeEventKind,
    VertexEdgeEventQuery, VertexEdgeSeparation, VertexFaceEventQuery, VertexFaceSeparation,
    VertexPairEventKind, VertexPairEventQuery, VertexPairSeparation, walk_compact_features,
};

#[derive(Clone, Copy, Debug)]
pub struct ConvexEndpoint<'a> {
    pub topology: &'a FeatureTopology,
    pub physical: &'a PhysicalShape,
    pub motion: FeatureMotion,
    pub bounds: CollisionMotion,
}

#[derive(Clone, Copy, Debug)]
pub struct ConvexPairQuery<'a> {
    pub endpoints: [ConvexEndpoint<'a>; 2],
    pub seed: SurfaceFeaturePair,
    pub start: f64,
    pub end: f64,
    pub extra_radius: f32,
    pub tolerances: ContactTolerances,
    pub maximum_feature_transitions: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexPairEvent {
    pub time: f64,
    pub kind: EventTimingKind,
    pub root: ContinuousRoot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvexPairQueryResult {
    pub selection: FeatureSelection,
    pub separation: f32,
    /// Directed away from the second selected endpoint toward the first.
    pub normal: [f32; 3],
    pub bounds: CollisionMotionBounds,
    pub event: Option<ConvexPairEvent>,
}

impl ConvexPairQuery<'_> {
    fn validate(self) -> Result<(), FeatureEventError> {
        if !self.start.is_finite() || !self.end.is_finite() || !self.extra_radius.is_finite() {
            return Err(FeatureEventError::NonFinite);
        }
        if self.start >= self.end {
            return Err(FeatureEventError::InvalidInterval);
        }
        for endpoint in self.endpoints {
            endpoint.motion.validate_interval(self.start, self.end)?;
            endpoint.bounds.validate()?;
            if !endpoint.physical.radius.is_finite()
                || !endpoint.physical.inverse_diameter.is_finite()
            {
                return Err(FeatureEventError::NonFinite);
            }
            if endpoint.physical.radius <= 0.0 || endpoint.physical.inverse_diameter <= 0.0 {
                return Err(FeatureEventError::InvalidDeviation);
            }
        }
        Ok(())
    }
    pub fn next(self) -> Result<ConvexPairQueryResult, FeatureEventError> {
        self.validate()?;
        let poses = [
            self.endpoints[0].motion.sample(self.start)?,
            self.endpoints[1].motion.sample(self.start)?,
        ];
        let placements = std::array::from_fn::<_, 2, _>(|side| FeaturePlacement {
            topology: self.endpoints[side].topology,
            position: poses[side].position,
            orientation: poses[side].orientation,
        });
        let selection = walk_compact_features(
            placements[0],
            placements[1],
            self.seed,
            self.maximum_feature_transitions,
            |_| {},
        )?;
        let order = selection.order();
        let placements = order.map(|side| placements[side]);
        let features = selection.ordered_pair();
        let ids = [features.first.edge, features.second.edge];
        let (separation, normal) = match (features.first.kind, features.second.kind) {
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Vertex) => {
                let value = VertexPairSeparation::evaluate(placements, ids, self.extra_radius)?;
                (value.distance, value.normal)
            }
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Edge) => {
                let value = VertexEdgeSeparation::evaluate(placements, ids, self.extra_radius)?;
                (value.distance, value.normal)
            }
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face) => {
                let value = VertexFaceSeparation::evaluate(placements, ids, self.extra_radius)?;
                (value.distance, value.normal)
            }
            (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Edge) => {
                let value = EdgePairSeparation::evaluate(placements, ids, self.extra_radius)?;
                (value.distance, value.normal)
            }
            _ => return Err(crate::FeatureWalkError::UnsupportedFeaturePair.into()),
        };
        self.predict_geometry(selection, separation, normal)
    }

    pub fn predict_selected(
        self,
        selection: FeatureSelection,
        geometry: crate::ClosestFeatureGeometry,
    ) -> Result<ConvexPairQueryResult, FeatureEventError> {
        self.validate()?;
        self.predict_geometry(selection, geometry.separation, geometry.normal)
    }

    fn predict_geometry(
        self,
        selection: FeatureSelection,
        separation: f32,
        normal: [f32; 3],
    ) -> Result<ConvexPairQueryResult, FeatureEventError> {
        if !separation.is_finite() || normal.iter().any(|v| !v.is_finite()) {
            return Err(FeatureEventError::NonFinite);
        }
        let endpoints = selection.order().map(|side| self.endpoints[side]);
        let features = selection.ordered_pair();
        let ids = [features.first.edge, features.second.edge];
        for (endpoint, id) in endpoints.iter().zip(ids) {
            endpoint.topology.edge(id)?;
        }
        let bounds = CollisionMotionBounds::from_endpoints(
            endpoints.map(|endpoint| endpoint.bounds),
            normal,
        )?;
        let mut result = ConvexPairQueryResult {
            selection,
            separation,
            normal,
            bounds,
            event: None,
        };
        if !bounds.admits(
            f64::from(separation),
            f64::from(self.tolerances.collision_distance),
            self.start,
            self.end,
        )? {
            return Ok(result);
        }
        let sector = |index: usize| {
            SectorThreshold {
                separation,
                collision_distance: self.tolerances.collision_distance,
                extra_radius: self.extra_radius,
                change_distance: self.tolerances.feature_change_distance,
                inverse_diameter: endpoints[index].physical.inverse_diameter,
            }
            .value()
        };
        let motions = endpoints.map(|endpoint| endpoint.motion);
        let topologies = endpoints.map(|endpoint| endpoint.topology);
        let event = |root: ContinuousRoot, collision| ConvexPairEvent {
            time: root.time,
            root,
            kind: if collision {
                EventTimingKind::Collision
            } else {
                EventTimingKind::FeatureTransition
            },
        };
        result.event = match (features.first.kind, features.second.kind) {
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Vertex) => VertexPairEventQuery {
                topologies,
                vertices: ids,
                motions,
                motion_bounds: endpoints.map(|endpoint| endpoint.bounds),
                radii: endpoints.map(|endpoint| endpoint.physical.radius),
                separating_normal: normal,
                start: self.start,
                end: self.end,
                initial_distance: separation,
                extra_radius: self.extra_radius,
                tolerances: self.tolerances,
                maximum_collision_deviation: bounds.maximum_deviation,
                worst_case_speed: bounds.worst_case_speed,
            }
            .next()?
            .map(|value| {
                event(
                    value.root,
                    matches!(value.kind, VertexPairEventKind::Collision),
                )
            }),
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Edge) => VertexEdgeEventQuery {
                topologies,
                vertex: ids[0],
                edge: ids[1],
                motions,
                motion_bounds: endpoints.map(|endpoint| endpoint.bounds),
                opposing_inverse_diameter: endpoints[1].physical.inverse_diameter,
                start: self.start,
                end: self.end,
                initial_distance: separation,
                extra_radius: self.extra_radius,
                tolerances: self.tolerances,
                maximum_collision_deviation: bounds.maximum_deviation,
                worst_case_speed: bounds.worst_case_speed,
            }
            .next()?
            .map(|value| {
                event(
                    value.root,
                    matches!(value.kind, VertexEdgeEventKind::Collision),
                )
            }),
            (SurfaceFeatureKind::Vertex, SurfaceFeatureKind::Face) => VertexFaceEventQuery {
                moving: topologies[0],
                fixed: topologies[1],
                vertex: ids[0],
                face: ids[1],
                moving_motion: motions[0],
                fixed_motion: motions[1],
                start: self.start,
                end: self.end,
                initial_distance: f64::from(separation + self.extra_radius),
                collision_distance: f64::from(self.tolerances.collision_distance)
                    + f64::from(self.extra_radius),
                real_distance: f64::from(
                    self.tolerances.real_surface + self.extra_radius * 0.5_f32,
                ),
                sector_threshold: sector(1)?,
                maximum_collision_deviation: bounds.maximum_deviation,
                maximum_angular_deviation: bounds.maximum_angular_deviation,
            }
            .next()?
            .map(|value| {
                event(
                    value.root,
                    matches!(value.kind, FeatureEventKind::Collision),
                )
            }),
            (SurfaceFeatureKind::Edge, SurfaceFeatureKind::Edge) => EdgePairEventQuery {
                topologies,
                edges: ids,
                motions,
                start: self.start,
                end: self.end,
                contact_normal: normal,
                collision_distance: f64::from(self.tolerances.collision_distance),
                real_distance: f64::from(self.tolerances.real_surface),
                sector_thresholds: endpoints.map(|endpoint| {
                    -f64::from(
                        self.tolerances.feature_change_distance
                            * endpoint.physical.inverse_diameter,
                    )
                }),
                maximum_collision_deviation: bounds.maximum_deviation,
                angular_deviation: endpoints[0].bounds.rotation.angular_speed
                    + endpoints[1].bounds.rotation.angular_speed,
            }
            .next()?
            .map(|value| {
                event(
                    value.root,
                    matches!(value.kind, EdgePairEventKind::Collision),
                )
            }),
            _ => return Err(crate::FeatureWalkError::UnsupportedFeaturePair.into()),
        };
        Ok(result)
    }
}
