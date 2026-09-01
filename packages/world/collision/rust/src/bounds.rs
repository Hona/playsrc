use crate::{Error, ErrorCode, Hull, Plane, error};

const BOUNDS_PADDING: f32 = 1.0 / 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundsTrace {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub fraction: f32,
    pub fraction_left_solid: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub contents: u32,
    pub plane: Option<Plane>,
}

#[derive(Clone, Copy)]
struct Axis {
    lower: f32,
    upper: f32,
    reciprocal: f32,
    crossing: bool,
}
impl Axis {
    fn interval(self, padding: f32) -> (f32, f32, bool) {
        if !self.crossing {
            return (-f32::MAX, f32::MAX, false);
        }
        let first = (self.lower - padding) * self.reciprocal;
        let second = (self.upper + padding) * self.reciprocal;
        if first <= second {
            (first, second, false)
        } else {
            (second, first, true)
        }
    }
}

/// Clip one point or asymmetric hull against an entity's world-aligned bounds.
/// This is distinct from brush half-space and studio hitbox clipping.
pub fn trace_bounds(
    start: [f32; 3],
    end: [f32; 3],
    hull: Hull,
    bounds: Hull,
) -> Result<BoundsTrace, Error> {
    if start
        .into_iter()
        .chain(end)
        .chain(hull.mins)
        .chain(hull.maxs)
        .chain(bounds.mins)
        .chain(bounds.maxs)
        .any(|v| !v.is_finite())
        || hull.mins.into_iter().zip(hull.maxs).any(|(a, b)| a > b)
        || bounds.mins.into_iter().zip(bounds.maxs).any(|(a, b)| a > b)
    {
        return Err(error(ErrorCode::InvalidSnapshot, None));
    }
    let delta = std::array::from_fn::<_, 3, _>(|i| end[i] - start[i]);
    let center = std::array::from_fn::<_, 3, _>(|i| (hull.mins[i] + hull.maxs[i]) * 0.5);
    let ray_start = std::array::from_fn::<_, 3, _>(|i| start[i] + center[i]);
    let reference = std::array::from_fn::<_, 3, _>(|i| ray_start[i] + -center[i]);
    if delta
        .into_iter()
        .chain(center)
        .chain(ray_start)
        .chain(reference)
        .any(|v| !v.is_finite())
    {
        return Err(error(ErrorCode::NonFinite, None));
    }
    let mut result = BoundsTrace {
        start: reference,
        end: std::array::from_fn(|i| reference[i] + delta[i]),
        fraction: 1.0,
        fraction_left_solid: 0.0,
        start_solid: false,
        all_solid: false,
        contents: 0,
        plane: None,
    };
    let mut outside = false;
    let mut separated = false;
    let axes = std::array::from_fn::<_, 3, _>(|i| {
        let extent = (hull.maxs[i] - hull.mins[i]) * 0.5;
        let lower = (bounds.mins[i] - ray_start[i]) - extent;
        let upper = (bounds.maxs[i] - ray_start[i]) + extent;
        let before = lower > 0.0;
        let after = upper < 0.0;
        let end_before = delta[i] < lower;
        let end_after = delta[i] > upper;
        outside |= before || after;
        separated |= (before && end_before) || (after && end_after);
        Axis {
            lower,
            upper,
            reciprocal: if delta[i] == 0.0 {
                f32::MAX
            } else {
                1.0 / delta[i]
            },
            crossing: (before != end_before) || (after != end_after),
        }
    });
    if separated {
        return Ok(result);
    }
    let overlap = |padding| {
        let mut entry = -f32::MAX;
        let mut exit = f32::MAX;
        let mut face = (0, false);
        for (index, axis) in axes.into_iter().enumerate() {
            let (begin, end, positive) = axis.interval(padding);
            if begin >= entry {
                entry = begin;
                face = (index, positive);
            }
            exit = exit.min(end);
        }
        (entry.max(0.0), exit.min(1.0), face)
    };
    let (entry, exit, _) = overlap(0.0);
    if entry > exit {
        return Ok(result);
    }
    let (entry, exit, (axis, positive)) = overlap(BOUNDS_PADDING);
    if entry > exit {
        return Err(error(ErrorCode::InvalidSnapshot, None));
    }
    result.contents = crate::CONTENTS_SOLID;
    if !outside {
        result.start_solid = true;
        result.fraction = 0.0;
        result.end = reference;
        result.all_solid = exit >= 1.0;
        if !result.all_solid && exit > 0.0 {
            result.fraction_left_solid = exit;
            result.start = std::array::from_fn(|i| reference[i] + delta[i] * exit);
        }
    } else {
        result.fraction = entry;
        result.end = std::array::from_fn(|i| reference[i] + delta[i] * entry);
        let mut normal = [0.0; 3];
        normal[axis] = if positive { 1.0 } else { -1.0 };
        result.plane = Some(Plane {
            normal,
            distance: if positive {
                bounds.maxs[axis]
            } else {
                -bounds.mins[axis]
            },
            kind: axis as i32,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const POINT: Hull = Hull {
        mins: [0.0; 3],
        maxs: [0.0; 3],
    };
    const PLAYER: Hull = Hull {
        mins: [-24.0, -24.0, 0.0],
        maxs: [24.0, 24.0, 82.0],
    };
    #[test]
    fn bounds_exit_starts_at_zero_fraction_and_keeps_the_expanded_leave_interval() {
        let trace = trace_bounds([0.0, 0.0, 40.0], [0.0, 0.0, -40.0], POINT, PLAYER).unwrap();
        assert!(trace.start_solid && !trace.all_solid);
        assert_eq!(trace.fraction.to_bits(), 0);
        assert_eq!(trace.fraction_left_solid.to_bits(), 1056971162);
        assert_eq!(trace.end, [0.0, 0.0, 40.0]);
        assert_eq!(trace.start.map(f32::to_bits), [0, 0, 3170893824]);
        assert_eq!(trace.plane, None);
    }
    #[test]
    fn bounds_entry_uses_reciprocal_grouping_and_last_axis_on_ties() {
        let trace =
            trace_bounds([-100.0, 100.0, 0.0], [100.0, -100.0, 82.0], POINT, PLAYER).unwrap();
        assert_eq!(trace.fraction.to_bits(), 0x3ec2_7ae1);
        assert_eq!(
            trace.plane,
            Some(Plane {
                normal: [0.0, 1.0, 0.0],
                distance: 24.0,
                kind: 1
            })
        );
    }
    #[test]
    fn padding_does_not_turn_a_parallel_near_miss_into_a_hit() {
        let trace = trace_bounds(
            [24.015625, 0.0, 40.0],
            [24.015625, 0.0, 50.0],
            POINT,
            PLAYER,
        )
        .unwrap();
        assert_eq!(trace.fraction, 1.0);
        assert!(!trace.start_solid);
        assert_eq!(trace.contents, 0);
        let boundary = trace_bounds([24.0, 24.0, 82.0], [24.0, 24.0, 82.0], POINT, PLAYER).unwrap();
        assert!(boundary.start_solid && boundary.all_solid);
        assert_eq!(boundary.fraction_left_solid, 0.0);
    }
    #[test]
    fn asymmetric_sweep_preserves_reference_point_and_target_plane() {
        let trace = trace_bounds(
            [48.0, 0.0, 40.0],
            [0.0, 0.0, 40.0],
            Hull {
                mins: [-1.0, -2.0, -3.0],
                maxs: [4.0, 5.0, 6.0],
            },
            PLAYER,
        )
        .unwrap();
        assert_eq!(trace.fraction, 0.478515625);
        assert_eq!(trace.end, [25.03125, 0.0, 40.0]);
        assert_eq!(
            trace.plane,
            Some(Plane {
                normal: [1.0, 0.0, 0.0],
                distance: 24.0,
                kind: 0
            })
        );
        assert!(trace_bounds([f32::NAN, 0.0, 0.0], [0.0; 3], POINT, PLAYER).is_err());
    }
}
