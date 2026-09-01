use crate::units::{
    DEGREES_PER_RADIAN, INCHES_PER_METER, METERS_PER_INCH, RADIANS_PER_DEGREE, internal_direction,
    internal_position, source_direction,
};
use crate::{MotionError, QueuedVelocity};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImpulseFrame {
    pub position: [f64; 3],
    pub orientation: [f64; 9],
    pub inverse_mass: f32,
    pub inverse_inertia: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ImpulseCommand {
    Center([f32; 3]),
    Offset { force: [f32; 3], point: [f32; 3] },
    Torque([f32; 3]),
}

impl ImpulseFrame {
    fn force(force: [f32; 3]) -> Result<[f32; 3], MotionError> {
        if force.iter().any(|value| !value.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        Ok(internal_direction(force, METERS_PER_INCH))
    }

    fn local_vector(self, world: [f32; 3]) -> Result<[f32; 3], MotionError> {
        let vector = std::array::from_fn(|axis| {
            ((f64::from(world[0]) * self.orientation[axis]
                + f64::from(world[1]) * self.orientation[3 + axis])
                + f64::from(world[2]) * self.orientation[6 + axis]) as f32
        });
        if vector.iter().any(|value| !value.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        Ok(vector)
    }

    fn offset(self, force: [f32; 3], point: [f32; 3]) -> Result<([f32; 3], [f32; 3]), MotionError> {
        let force = Self::force(force)?;
        if point.iter().any(|value| !value.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        let point = internal_position(point);
        self.internal_offset(force, point)
    }

    fn internal_offset(
        self,
        force: [f32; 3],
        point: [f64; 3],
    ) -> Result<([f32; 3], [f32; 3]), MotionError> {
        if force.iter().any(|value| !value.is_finite())
            || point.iter().any(|value| !value.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let lever: [f32; 3] =
            std::array::from_fn(|axis| (point[axis] - self.position[axis]) as f32);
        let torque = self.local_vector([
            lever[1] * force[2] - lever[2] * force[1],
            lever[2] * force[0] - lever[0] * force[2],
            lever[0] * force[1] - lever[1] * force[0],
        ])?;
        Ok((force, torque))
    }

    pub fn calculate_force(
        self,
        force: [f32; 3],
        point: [f32; 3],
    ) -> Result<([f32; 3], [f32; 3]), MotionError> {
        let (force, torque) = self.offset(force, point)?;
        Ok((
            source_direction(force, INCHES_PER_METER),
            source_direction(torque, DEGREES_PER_RADIAN),
        ))
    }

    pub fn calculate_velocity(
        self,
        force: [f32; 3],
        point: [f32; 3],
    ) -> Result<([f32; 3], [f32; 3]), MotionError> {
        let (force, torque) = self.offset(force, point)?;
        let linear = force.map(|value| (f64::from(value) * f64::from(self.inverse_mass)) as f32);
        let angular = std::array::from_fn(|axis| torque[axis] * self.inverse_inertia[axis]);
        Ok((
            source_direction(linear, INCHES_PER_METER),
            source_direction(angular, DEGREES_PER_RADIAN),
        ))
    }

    pub fn queue(
        self,
        command: ImpulseCommand,
        queued: &mut QueuedVelocity,
    ) -> Result<(), MotionError> {
        match command {
            ImpulseCommand::Center(force) => {
                let force = Self::force(force)?;
                for (pending, value) in queued.linear.iter_mut().zip(force) {
                    *pending += (f64::from(value) * f64::from(self.inverse_mass)) as f32;
                }
            }
            ImpulseCommand::Offset { force, point } => {
                let (force, torque) = self.offset(force, point)?;
                self.accumulate_offset(force, torque, queued);
            }
            ImpulseCommand::Torque(torque) => {
                if torque.iter().any(|value| !value.is_finite()) {
                    return Err(MotionError::NonFinite);
                }
                let torque = self.local_vector(internal_direction(torque, RADIANS_PER_DEGREE))?;
                for (axis, torque) in torque.into_iter().enumerate() {
                    queued.angular[axis] += torque * self.inverse_inertia[axis];
                }
            }
        }
        Ok(())
    }

    pub(crate) fn queue_internal_offset(
        self,
        force: [f32; 3],
        point: [f64; 3],
        queued: &mut QueuedVelocity,
    ) -> Result<(), MotionError> {
        if point.iter().any(|v| !v.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        let delta = std::array::from_fn::<_, 3, _>(|axis| point[axis] - self.position[axis]);
        let local: [f32; 3] = std::array::from_fn(|axis| {
            ((delta[1] * self.orientation[3 + axis] + delta[0] * self.orientation[axis])
                + delta[2] * self.orientation[6 + axis]) as f32
        });
        let direction = self.local_vector(force)?;
        let torque = [
            local[1] * direction[2] - local[2] * direction[1],
            local[2] * direction[0] - local[0] * direction[2],
            local[0] * direction[1] - local[1] * direction[0],
        ];
        for axis in 0..3 {
            queued.angular[axis] += torque[axis] * self.inverse_inertia[axis];
            queued.linear[axis] += (f64::from(force[axis]) * f64::from(self.inverse_mass)) as f32;
        }
        Ok(())
    }
    fn accumulate_offset(self, force: [f32; 3], torque: [f32; 3], queued: &mut QueuedVelocity) {
        for axis in 0..3 {
            queued.angular[axis] += torque[axis] * self.inverse_inertia[axis];
            queued.linear[axis] = (f64::from(force[axis]) * f64::from(self.inverse_mass)
                + f64::from(queued.linear[axis])) as f32;
        }
    }
}
