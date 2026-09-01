use crate::{CollisionMotion, ContactTolerances, ContinuousError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PairResidence {
    Exact,
    Movement { distances: [f32; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairResidenceInput {
    pub gap: f32,
    pub selected_first: usize,
    pub moving: [bool; 2],
    pub timestep: f32,
    pub allow_movement: bool,
}

impl PairResidence {
    pub fn select(
        input: PairResidenceInput,
        motion: [CollisionMotion; 2],
        tolerances: ContactTolerances,
    ) -> Result<Self, ContinuousError> {
        let PairResidenceInput {
            gap,
            selected_first,
            moving,
            timestep,
            allow_movement,
        } = input;
        if selected_first > 1 {
            return Err(ContinuousError::InvalidBracket);
        }
        for endpoint in motion {
            endpoint.validate()?;
        }
        if !gap.is_finite() || !timestep.is_finite() || !tolerances.collision_distance.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        if timestep <= 0.0 {
            return Err(ContinuousError::InvalidInterval);
        }
        let ordered = [motion[selected_first], motion[1 - selected_first]];
        let boundary = ((f64::from(timestep) * CollisionMotion::combined_speed(ordered))
            * f64::from(2.1_f32))
            + f64::from(tolerances.collision_distance);
        if !boundary.is_finite() {
            return Err(ContinuousError::NonFinite);
        }
        if f64::from(gap) <= boundary || !allow_movement {
            return Ok(Self::Exact);
        }
        let distance = (f64::from(gap) - f64::from(tolerances.collision_distance)) as f32;
        let distances = if !moving[0] {
            [0.0, distance]
        } else if !moving[1] {
            [distance, 0.0]
        } else {
            split_moving_distance(distance, motion)?
        };
        Ok(Self::Movement { distances })
    }
}

pub(crate) fn split_moving_distance(
    distance: f32,
    motion: [CollisionMotion; 2],
) -> Result<[f32; 2], ContinuousError> {
    if !distance.is_finite() {
        return Err(ContinuousError::NonFinite);
    }
    if distance < 0.0 {
        return Err(ContinuousError::InvalidInterval);
    }
    for endpoint in motion {
        endpoint.validate()?;
    }
    let minimum = 1.0e-10_f32;
    let distances = {
        let speeds = motion
            .map(|endpoint| (endpoint.rotation.surface_speed + endpoint.linear_speed) + minimum);
        let first = speeds[1] * 0.1_f32 + speeds[0];
        let second = speeds[0] * 0.1_f32 + speeds[1];
        let factor = distance / (second + first);
        [factor * first, factor * second]
    };
    if distances.iter().any(|value| !value.is_finite()) {
        return Err(ContinuousError::NonFinite);
    }
    Ok(distances)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(gap: f32, moving: [bool; 2], allow_movement: bool) -> PairResidenceInput {
        PairResidenceInput {
            gap,
            selected_first: 0,
            moving,
            timestep: 0.015,
            allow_movement,
        }
    }
    #[test]
    fn exact_threshold_and_disabled_conversion_do_not_create_distance_listeners() {
        let tolerances = ContactTolerances::from_gravity([0.0; 3]).unwrap();
        let motion = [CollisionMotion::stationary(); 2];
        let gap = tolerances.collision_distance;
        assert_eq!(
            PairResidence::select(input(gap, [true; 2], true), motion, tolerances).unwrap(),
            PairResidence::Exact
        );
        let above = f32::from_bits(gap.to_bits() + 1);
        assert!(matches!(
            PairResidence::select(input(above, [true; 2], true), motion, tolerances).unwrap(),
            PairResidence::Movement { .. }
        ));
        assert_eq!(
            PairResidence::select(input(1.0, [true; 2], false), motion, tolerances).unwrap(),
            PairResidence::Exact
        );
    }
    #[test]
    fn inactive_endpoint_gets_no_allowance_and_both_moving_share_in_float() {
        let tolerances = ContactTolerances::from_gravity([0.0; 3]).unwrap();
        let motion = [CollisionMotion::stationary(); 2];
        let distance = 1.0 - tolerances.collision_distance;
        assert_eq!(
            PairResidence::select(input(1.0, [false, true], true), motion, tolerances).unwrap(),
            PairResidence::Movement {
                distances: [0.0, distance]
            }
        );
        assert_eq!(
            PairResidence::select(input(1.0, [true, false], true), motion, tolerances).unwrap(),
            PairResidence::Movement {
                distances: [distance, 0.0]
            }
        );
        let PairResidence::Movement { distances } =
            PairResidence::select(input(1.0, [true; 2], true), motion, tolerances).unwrap()
        else {
            panic!("expected deferred pair")
        };
        assert_eq!(distances[0], distances[1]);
        assert!((distances[0] * 2.0 - distance).abs() < 1.0e-6);
    }
}
