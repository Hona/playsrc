use crate::{
    CoreOrientation, MotionError, ShadowVelocityInput, VelocityState, normalize_source_vector,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowControlState {
    pub target_position: [f64; 3],
    pub target_orientation: CoreOrientation,
    pub previous_position: [f64; 3],
    pub last_impulse: [f32; 3],
    pub maximum_angular_speed: f32,
    pub maximum_angular_damping: f32,
    pub maximum_linear_speed: f32,
    pub maximum_linear_damping: f32,
    pub damping: f32,
    pub teleport_distance: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowControlBody {
    pub position: [f64; 3],
    pub basis: [f64; 9],
    pub orientation: CoreOrientation,
    pub shift: Option<[f32; 3]>,
    pub velocity: VelocityState,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowControlPlan {
    pub teleport: bool,
    pub remaining: f32,
    object_position: [f64; 3],
    displacement: [f32; 3],
    scale: f32,
    timestep: f32,
}
impl ShadowControlState {
    pub fn prepare(
        &self,
        body: ShadowControlBody,
        arrival: f32,
        timestep: f32,
    ) -> Result<ShadowControlPlan, MotionError> {
        if !arrival.is_finite()
            || !timestep.is_finite()
            || timestep <= 0.0
            || self
                .target_position
                .iter()
                .chain(&self.previous_position)
                .chain(&self.target_orientation.quaternion)
                .any(|v| !v.is_finite())
            || [
                self.maximum_angular_speed,
                self.maximum_angular_damping,
                self.maximum_linear_speed,
                self.maximum_linear_damping,
                self.damping,
                self.teleport_distance,
            ]
            .iter()
            .any(|v| !v.is_finite())
            || self.last_impulse.iter().any(|v| !v.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        if body
            .position
            .iter()
            .chain(&body.basis)
            .chain(&body.orientation.quaternion)
            .any(|v| !v.is_finite())
            || body
                .shift
                .into_iter()
                .flatten()
                .chain(body.velocity.linear)
                .chain(body.velocity.angular)
                .any(|v| !v.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let fraction = if arrival > 0.0 {
            (timestep / arrival).min(1.0)
        } else {
            1.0
        };
        let mut object_position = body.position;
        if let Some(shift) = body.shift {
            for (row, component) in object_position.iter_mut().enumerate() {
                let delta = ((body.basis[row * 3] * f64::from(shift[0])
                    + body.basis[row * 3 + 1] * f64::from(shift[1]))
                    + body.basis[row * 3 + 2] * f64::from(shift[2]))
                    as f32;
                *component += f64::from(delta);
            }
        }
        let mut displacement =
            std::array::from_fn(|i| (self.target_position[i] - object_position[i]) as f32);
        let mut teleport = false;
        if self.teleport_distance > 0.0 {
            let distance = if self.previous_position == [0.0; 3] {
                displacement
            } else {
                std::array::from_fn(|i| (object_position[i] - self.previous_position[i]) as f32)
            };
            let squared =
                (distance[1] * distance[1] + distance[0] * distance[0]) + distance[2] * distance[2];
            teleport = squared > self.teleport_distance * self.teleport_distance;
            if teleport {
                displacement = [0.0; 3];
            }
        }
        Ok(ShadowControlPlan {
            teleport,
            remaining: (arrival - timestep).max(0.0),
            object_position,
            displacement,
            scale: (1.0_f32 / timestep) * fraction,
            timestep,
        })
    }
    /// If the plan requests teleportation, its owner applies that mutation before
    /// supplying the resulting core state here. The prior object point stays in the plan.
    pub fn finish(
        &mut self,
        plan: ShadowControlPlan,
        body: ShadowControlBody,
    ) -> Result<VelocityState, MotionError> {
        let linear = ShadowVelocityInput {
            velocity: body.velocity.linear,
            displacement: plan.displacement,
            maximum_speed: self.maximum_linear_speed,
            maximum_damping_speed: self.maximum_linear_damping,
            scale: plan.scale,
            damping: self.damping,
        }
        .solve()?;
        let inverse = CoreOrientation {
            quaternion: [
                -body.orientation.quaternion[0],
                -body.orientation.quaternion[1],
                -body.orientation.quaternion[2],
                body.orientation.quaternion[3],
            ],
        };
        let difference = inverse
            .product(self.target_orientation)
            .and_then(CoreOrientation::normalized)
            .map_err(|_| MotionError::NonFinite)?;
        let mut angle = (difference.quaternion[3].acos() * 2.0) as f32;
        if f64::from(angle) > std::f64::consts::PI {
            angle = (f64::from(angle) - std::f64::consts::TAU) as f32;
        }
        let axis =
            normalize_source_vector(std::array::from_fn(|i| difference.quaternion[i] as f32))
                .and_then(normalize_source_vector)
                .map_err(|_| MotionError::NonFinite)?;
        let angular = ShadowVelocityInput {
            velocity: body.velocity.angular,
            displacement: axis.map(|v| v * angle),
            maximum_speed: self.maximum_angular_speed,
            maximum_damping_speed: self.maximum_angular_damping,
            scale: plan.scale,
            damping: self.damping,
        }
        .solve()?;
        let previous_position = std::array::from_fn(|i| {
            f64::from(linear.velocity[i]) * f64::from(plan.timestep) + plan.object_position[i]
        });
        self.previous_position = previous_position;
        self.last_impulse = linear.impulse;
        Ok(VelocityState {
            linear: linear.velocity,
            angular: angular.velocity,
        })
    }
}
