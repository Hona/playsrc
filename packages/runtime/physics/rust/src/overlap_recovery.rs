use crate::impulses::ImpulseFrame;
use crate::{FeaturePlacement, FeatureWalkError, MotionError, QueuedVelocity, SurfaceFeatureKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryError {
    Motion(MotionError),
    Geometry(FeatureWalkError),
}
impl From<MotionError> for RecoveryError {
    fn from(value: MotionError) -> Self {
        Self::Motion(value)
    }
}
impl From<FeatureWalkError> for RecoveryError {
    fn from(value: FeatureWalkError) -> Self {
        Self::Geometry(value)
    }
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Motion(error) => error.fmt(f),
            Self::Geometry(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RecoveryError {}

#[derive(Clone, Copy, Debug)]
pub struct RecoveryEndpoint {
    pub position: [f64; 3],
    pub impulse_position: [f64; 3],
    pub orientation: [f64; 9],
    pub phase_linear: [f32; 3],
    pub queued: QueuedVelocity,
    pub mass: f32,
    pub inverse_mass: f32,
    pub inverse_inertia: [f32; 3],
    pub simulated: bool,
    pub immovable: bool,
    pub pinned: bool,
    pub feature: SurfaceFeatureKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoveryPlane {
    pub face: usize,
    pub local_normal: [f64; 3],
    pub world_normal: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecoveryResult {
    pub queued: [QueuedVelocity; 2],
    pub refreshed: [bool; 2],
    pub plane: Option<RecoveryPlane>,
}

pub fn penetration_recovery_speed(gravity: [f64; 3], timestep: f32) -> Result<f32, MotionError> {
    if gravity.iter().any(|value| !value.is_finite()) || !timestep.is_finite() {
        return Err(MotionError::NonFinite);
    }
    if timestep <= 0.0 {
        return Err(MotionError::NonPositiveTimestep);
    }
    let length =
        ((gravity[0] * gravity[0] + gravity[1] * gravity[1]) + gravity[2] * gravity[2]).sqrt();
    let speed = ((length * 4.0) * f64::from(timestep)) as f32;
    if !speed.is_finite() {
        return Err(MotionError::NonFinite);
    }
    Ok(speed)
}

pub fn recover_overlap<'a>(
    endpoints: [RecoveryEndpoint; 2],
    speed: f64,
    timestep: f32,
    resolve_fixed: impl FnOnce(usize) -> Result<FeaturePlacement<'a>, FeatureWalkError>,
) -> Result<RecoveryResult, RecoveryError> {
    if !speed.is_finite() || !timestep.is_finite() {
        return Err(MotionError::NonFinite.into());
    }
    if timestep <= 0.0 {
        return Err(MotionError::NonPositiveTimestep.into());
    }
    let mut result = RecoveryResult {
        queued: endpoints.map(|endpoint| endpoint.queued),
        refreshed: [false; 2],
        plane: None,
    };
    let fixed = usize::from(endpoints[1].immovable);
    let moving = 1 - fixed;
    if endpoints[fixed].immovable && endpoints[fixed].feature != SurfaceFeatureKind::InteriorFace {
        if !endpoints[moving].simulated {
            return Ok(result);
        }
        let placement = resolve_fixed(fixed)?;
        let point = placement.local_point(endpoints[moving].position);
        let mut choice = None;
        let mut best = -1.0e101;
        for face in 0..placement.topology.edges().len() / 3 {
            let edge = placement
                .topology
                .edge_id(face * 3)
                .expect("enumerated face");
            let (normal, offset) = placement.topology.unit_face_plane(edge)?;
            let distance =
                ((point[0] * normal[0] + point[1] * normal[1]) + point[2] * normal[2]) + offset;
            if !distance.is_finite() {
                return Err(FeatureWalkError::NonFiniteTransform.into());
            }
            if distance > best {
                best = distance;
                choice = Some((face, normal));
            }
        }
        let (face, normal) = choice.ok_or(FeatureWalkError::UnsupportedFeaturePair)?;
        let world = placement.world_vector(normal);
        let direction = world.map(|v| v as f32);
        if direction.iter().any(|v| !v.is_finite()) {
            return Err(MotionError::NonFinite.into());
        }
        let velocity: [f32; 3] = std::array::from_fn(|axis| {
            endpoints[moving].phase_linear[axis] + result.queued[moving].linear[axis]
        });
        let away =
            (velocity[1] * direction[1] + velocity[0] * direction[0]) + velocity[2] * direction[2];
        if f64::from(away) < speed * f64::from(0.1_f32) {
            for (pending, axis) in result.queued[moving].linear.iter_mut().zip(direction) {
                *pending += (f64::from(axis) * speed) as f32;
            }
            result.refreshed[moving] = true;
        }
        result.plane = Some(RecoveryPlane {
            face,
            local_normal: normal,
            world_normal: world,
        });
    } else {
        if endpoints
            .iter()
            .any(|endpoint| endpoint.position.iter().any(|v| !v.is_finite()))
        {
            return Err(MotionError::NonFinite.into());
        }
        let mut direction: [f32; 3] = std::array::from_fn(|axis| {
            (endpoints[1].position[axis] - endpoints[0].position[axis]) as f32
        });
        let squared = f64::from(
            (direction[0] * direction[0] + direction[1] * direction[1])
                + direction[2] * direction[2],
        );
        let length = if squared >= 1.0e-19 {
            let inverse = crate::arithmetic::refined_inverse_root::<5>(squared);
            direction = direction.map(|value| (f64::from(value) * inverse) as f32);
            (squared * inverse) as f32
        } else {
            0.0
        };
        if f64::from(length) <= 0.01 {
            direction = [1.0, 0.0, 0.0];
        }
        for side in 0..2 {
            let endpoint = endpoints[side];
            if !endpoint.simulated || endpoint.pinned {
                continue;
            }
            let mass = if endpoints[1 - side].pinned {
                endpoint.mass
            } else {
                endpoint.mass.min(endpoints[1 - side].mass)
            };
            let scale = f64::from(mass) * speed * if side == 0 { -1.0 } else { 1.0 };
            let force = direction.map(|value| (f64::from(value) * scale) as f32);
            let frame = ImpulseFrame {
                position: endpoint.impulse_position,
                orientation: endpoint.orientation,
                inverse_mass: endpoint.inverse_mass,
                inverse_inertia: endpoint.inverse_inertia,
            };
            frame
                .queue_internal_offset(force, endpoints[0].position, &mut result.queued[side])
                .map_err(|_| FeatureWalkError::NonFiniteTransform)?;
            result.queued[side].angular[0] += if side == 0 { -timestep } else { timestep };
            result.refreshed[side] = true;
        }
    }
    if result.queued.iter().any(|q| {
        q.linear
            .iter()
            .chain(q.angular.iter())
            .any(|v| !v.is_finite())
    }) {
        return Err(MotionError::NonFinite.into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn endpoint() -> RecoveryEndpoint {
        RecoveryEndpoint {
            position: [0.0; 3],
            impulse_position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            phase_linear: [0.0; 3],
            queued: QueuedVelocity::default(),
            mass: 1.0,
            inverse_mass: 1.0,
            inverse_inertia: [1.0; 3],
            simulated: true,
            immovable: false,
            pinned: false,
            feature: SurfaceFeatureKind::Vertex,
        }
    }
    #[test]
    fn coincident_centers_use_the_declared_axis_and_queue_opposite_angular_steps() {
        let result = recover_overlap([endpoint(); 2], 2.0, 0.015, |_| {
            panic!("paired response does not acquire a fixed transform")
        })
        .unwrap();
        assert_eq!(result.queued[0].linear, [-2.0, 0.0, 0.0]);
        assert_eq!(result.queued[1].linear, [2.0, 0.0, 0.0]);
        assert_eq!(result.queued[0].angular, [-0.015, 0.0, 0.0]);
        assert_eq!(result.queued[1].angular, [0.015, 0.0, 0.0]);
        assert_eq!(result.refreshed, [true, true]);
        assert!(result.plane.is_none());
    }
    #[test]
    fn inactive_planar_recovery_does_not_resolve_a_transform() {
        let fixed = RecoveryEndpoint {
            immovable: true,
            ..endpoint()
        };
        let moving = RecoveryEndpoint {
            simulated: false,
            ..endpoint()
        };
        let result = recover_overlap([fixed, moving], 1.0, 0.015, |_| {
            panic!("inactive recovery has no transform work")
        })
        .unwrap();
        assert_eq!(result.refreshed, [false; 2]);
        assert_eq!(result.queued, [QueuedVelocity::default(); 2]);
        assert_eq!(
            penetration_recovery_speed([0.0, 20.32, 0.0], 0.015).unwrap(),
            ((20.32_f64 * 4.0) * f64::from(0.015_f32)) as f32
        );
        assert_eq!(
            penetration_recovery_speed([0.0; 3], 0.0),
            Err(MotionError::NonPositiveTimestep)
        );
        assert!(
            recover_overlap(
                [
                    RecoveryEndpoint {
                        position: [f64::NAN; 3],
                        ..endpoint()
                    },
                    endpoint()
                ],
                1.0,
                0.015,
                |_| unreachable!()
            )
            .is_err()
        );
    }
}
