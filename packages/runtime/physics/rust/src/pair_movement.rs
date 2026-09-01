use crate::{
    CollisionMotion, ContinuousError, HierarchyError, MovementRangeClock, MovementRangeError,
    PairCoreProjection,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementPairHint {
    pub gap: f32,
    pub normal: [f32; 3],
    pub projection: f32,
    pub rotation_travel: f64,
    pub selected_first: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairRangeEndpoint {
    pub core: PairCoreProjection,
    pub motion: CollisionMotion,
    pub range: MovementRangeClock,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairRangeQuery {
    pub endpoints: [PairRangeEndpoint; 2],
    pub time: f64,
    pub timestep: f32,
    pub intrusion: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PairRangeAction {
    Recalculate,
    Renew { distances: [f64; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PairRangeError {
    Motion(ContinuousError),
    Projection(HierarchyError),
    Clock(MovementRangeError),
}
impl fmt::Display for PairRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Motion(e) => e.fmt(f),
            Self::Projection(e) => e.fmt(f),
            Self::Clock(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for PairRangeError {}
impl From<ContinuousError> for PairRangeError {
    fn from(e: ContinuousError) -> Self {
        Self::Motion(e)
    }
}
impl From<HierarchyError> for PairRangeError {
    fn from(e: HierarchyError) -> Self {
        Self::Projection(e)
    }
}
impl From<MovementRangeError> for PairRangeError {
    fn from(e: MovementRangeError) -> Self {
        Self::Clock(e)
    }
}

impl MovementPairHint {
    /// A pair without an admitted separation hint must run full minimization.
    pub fn on_range_exceeded(
        hint: Option<&mut Self>,
        query: PairRangeQuery,
    ) -> Result<PairRangeAction, PairRangeError> {
        let Some(hint) = hint else {
            return Ok(PairRangeAction::Recalculate);
        };
        if hint.selected_first > 1 {
            return Err(ContinuousError::InvalidBracket.into());
        }
        if !hint.gap.is_finite()
            || !hint.projection.is_finite()
            || !hint.rotation_travel.is_finite()
            || hint.normal.iter().any(|value| !value.is_finite())
            || !query.intrusion.is_finite()
            || !query.timestep.is_finite()
        {
            return Err(ContinuousError::NonFinite.into());
        }
        if query.timestep <= 0.0 {
            return Err(ContinuousError::InvalidInterval.into());
        }
        let first = hint.selected_first;
        let endpoints = [query.endpoints[first], query.endpoints[1 - first]];
        for endpoint in endpoints {
            endpoint.motion.validate()?;
        }
        let positions = [
            endpoints[0].core.at(query.time)?,
            endpoints[1].core.at(query.time)?,
        ];
        let delta: [f64; 3] = std::array::from_fn(|axis| positions[0][axis] - positions[1][axis]);
        let projection = (delta[1] * f64::from(hint.normal[1])
            + delta[0] * f64::from(hint.normal[0]))
            + delta[2] * f64::from(hint.normal[2]);
        let rotation = endpoints[0].range.projected_rotation_travel(query.time)?
            + endpoints[1].range.projected_rotation_travel(query.time)?;
        let gap = (f64::from(hint.gap) - (rotation - hint.rotation_travel))
            - (f64::from(hint.projection) - projection);
        let remaining = f64::from(query.intrusion) + gap;
        let speeds = endpoints.map(|endpoint| {
            f64::from(endpoint.motion.rotation.surface_speed + endpoint.motion.linear_speed)
                + 1.0e-19
        });
        let sum = speeds[1] + speeds[0];
        let threshold = (f64::from(query.timestep) * sum) * 6.0;
        if [projection, rotation, gap, remaining, threshold]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite.into());
        }
        if remaining <= threshold {
            return Ok(PairRangeAction::Recalculate);
        }
        let interval = remaining / sum;
        let mut distances = [0.0; 2];
        distances[first] = interval * speeds[0];
        distances[1 - first] = interval * speeds[1];
        let projection = projection as f32;
        let gap = gap as f32;
        if !projection.is_finite()
            || !gap.is_finite()
            || distances.iter().any(|value| !value.is_finite())
        {
            return Err(ContinuousError::NonFinite.into());
        }
        hint.gap = gap;
        hint.projection = projection;
        hint.rotation_travel = rotation;
        Ok(PairRangeAction::Renew { distances })
    }

    pub fn shift_range(&mut self, total: f32, center: f32) -> Result<(), MovementRangeError> {
        self.rotation_travel = Self::shift_rotation_travel(self.rotation_travel, total, center)?;
        Ok(())
    }
    pub(crate) fn shift_rotation_travel(
        rotation: f64,
        total: f32,
        center: f32,
    ) -> Result<f64, MovementRangeError> {
        let value = rotation + f64::from(total - center);
        if !value.is_finite() {
            return Err(MovementRangeError::NonFinite);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_hints_recalculate_and_failed_updates_leave_retained_words_unchanged() {
        let endpoint = PairRangeEndpoint {
            core: PairCoreProjection {
                position: [0.0; 3],
                velocity: [0.0; 3],
                time: 0.0,
                radius: 1.0,
            },
            motion: CollisionMotion::stationary(),
            range: MovementRangeClock::default(),
        };
        let query = PairRangeQuery {
            endpoints: [endpoint; 2],
            time: 0.0,
            timestep: 0.015,
            intrusion: 0.0,
        };
        assert_eq!(
            MovementPairHint::on_range_exceeded(None, query).unwrap(),
            PairRangeAction::Recalculate
        );
        let mut hint = MovementPairHint {
            gap: 1.0,
            normal: [1.0, 0.0, 0.0],
            projection: 0.0,
            rotation_travel: 0.0,
            selected_first: 0,
        };
        let before = hint;
        assert!(
            MovementPairHint::on_range_exceeded(
                Some(&mut hint),
                PairRangeQuery {
                    time: f64::NAN,
                    ..query
                }
            )
            .is_err()
        );
        assert_eq!(hint, before);
        assert!(matches!(
            MovementPairHint::on_range_exceeded(Some(&mut hint), query).unwrap(),
            PairRangeAction::Renew { .. }
        ));
        hint.shift_range(-0.5, -0.25).unwrap();
        assert_eq!(hint.rotation_travel, -0.25);
        let before = hint;
        assert!(hint.shift_range(f32::NAN, 0.0).is_err());
        assert_eq!(hint, before);
    }

    #[test]
    fn renewal_requires_strictly_more_than_six_steps_of_remaining_travel() {
        let mut endpoint = PairRangeEndpoint {
            core: PairCoreProjection {
                position: [0.0; 3],
                velocity: [0.0; 3],
                time: 0.0,
                radius: 1.0,
            },
            motion: CollisionMotion::stationary(),
            range: MovementRangeClock::default(),
        };
        let other = endpoint;
        endpoint.motion.linear_speed = 1.0;
        let query = PairRangeQuery {
            endpoints: [endpoint, other],
            time: 0.0,
            timestep: 1.0,
            intrusion: 0.0,
        };
        let mut hint = MovementPairHint {
            gap: 6.0,
            normal: [1.0, 0.0, 0.0],
            projection: 0.0,
            rotation_travel: 0.0,
            selected_first: 0,
        };
        let before = hint;
        assert_eq!(
            MovementPairHint::on_range_exceeded(Some(&mut hint), query).unwrap(),
            PairRangeAction::Recalculate
        );
        assert_eq!(hint, before);
        hint.gap = f32::from_bits(6.0_f32.to_bits() + 1);
        assert!(matches!(
            MovementPairHint::on_range_exceeded(Some(&mut hint), query).unwrap(),
            PairRangeAction::Renew { .. }
        ));
    }
}
