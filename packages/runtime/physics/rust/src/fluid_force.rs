use crate::{
    FeatureTopology, FluidPressure, FluidPressureFrame, MotionError, QueuedVelocity,
    SubmergedVolume,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FluidBodyState {
    pub entrained_velocity: [f32; 3],
    pub visible_area: f32,
    pub previous_area: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidSettings {
    pub density: f32,
    pub pressure_damping: f32,
    pub friction_damping: f32,
    pub torque_factor: f32,
    pub epsilon: f32,
    pub viscosity: f32,
    pub entrainment: f32,
    pub aerodynamic: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidBodyInput {
    pub frame: FluidPressureFrame,
    pub plane: [f32; 4],
    pub current: [f32; 3],
    pub gravity: [f32; 3],
    pub inverse_mass: f32,
    pub inverse_inertia: [f32; 3],
    pub queued: QueuedVelocity,
    pub timestep: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidBodyOutput {
    pub volume: f32,
    pub center: [f32; 3],
    pub pressure: FluidPressure,
    pub queued: QueuedVelocity,
}
impl FluidBodyState {
    pub fn advance(
        &mut self,
        topologies: &[FeatureTopology],
        settings: FluidSettings,
        input: FluidBodyInput,
    ) -> Result<FluidBodyOutput, MotionError> {
        let numbers = [
            settings.density,
            settings.pressure_damping,
            settings.friction_damping,
            settings.torque_factor,
            settings.epsilon,
            settings.viscosity,
            settings.entrainment,
            input.inverse_mass,
            input.timestep,
            self.visible_area,
            self.previous_area,
        ];
        if numbers
            .iter()
            .chain(&input.current)
            .chain(&input.gravity)
            .chain(&input.inverse_inertia)
            .chain(&input.queued.angular)
            .chain(&input.queued.linear)
            .chain(&self.entrained_velocity)
            .any(|v| !v.is_finite())
            || input.timestep <= 0.0
            || settings.epsilon < 0.0
        {
            return Err(MotionError::NonFinite);
        }
        let mut output = FluidBodyOutput {
            volume: 0.0,
            center: [0.0; 3],
            pressure: FluidPressure::default(),
            queued: input.queued,
        };
        if settings.density == 0.0 {
            return Ok(output);
        }
        let mut next = *self;
        if next.visible_area > next.previous_area {
            let fraction = next.previous_area / next.visible_area;
            next.entrained_velocity = next
                .entrained_velocity
                .map(|v| (f64::from(v) * f64::from(fraction)) as f32);
        }
        next.previous_area = next.visible_area;
        next.visible_area = 0.0;
        let current = std::array::from_fn(|i| input.current[i] + next.entrained_velocity[i]);
        let frame = FluidPressureFrame {
            current,
            pressure: (settings.pressure_damping * 0.5) * settings.density,
            friction: (settings.friction_damping * 0.5) * settings.density,
            aerodynamic: settings.aerodynamic,
            ..input.frame
        };
        let mut submerged = SubmergedVolume::default();
        for topology in topologies.iter().rev() {
            for face in topology.edges().chunks_exact(3) {
                let triangle = std::array::from_fn(|i| topology.points()[face[i].start as usize]);
                submerged.add_triangle(triangle, input.plane, settings.epsilon)?;
                output
                    .pressure
                    .add_triangle(triangle, input.plane, settings.epsilon, frame)?;
            }
        }
        (output.volume, output.center) = submerged.finish(settings.epsilon)?;
        if output.volume > settings.epsilon {
            let point = point_world(frame, output.center);
            let scale = f64::from(-(output.volume * settings.density)) * f64::from(input.timestep);
            let impulse = input.gravity.map(|v| (f64::from(v) * scale) as f32);
            queue_impulse(&mut output.queued, point, impulse, input);
        }
        let delta = f64::from(input.timestep);
        let move_factor = ((f64::from(output.pressure.area.abs()).sqrt()
            * f64::from(-settings.torque_factor))
            * f64::from(0.1_f32)) as f32;
        let move_torque = output
            .pressure
            .current_torque
            .map(|v| (f64::from(v) * (f64::from(move_factor) * delta)) as f32);
        let point_torque = output
            .pressure
            .point_torque
            .map(|v| (f64::from(v) * delta) as f32);
        let local_impulse = output
            .pressure
            .impulse
            .map(|v| (f64::from(v) * delta) as f32);
        let impulse = rotate(frame.object_basis, local_impulse);
        queue_impulse(&mut output.queued, frame.object_position, impulse, input);
        // The retained object-to-core rotation is identity for authored polygon bodies.
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let a = rotate(identity, point_torque);
        let b = rotate(identity, move_torque);
        for axis in 0..3 {
            output.queued.angular[axis] +=
                (b[axis] * input.inverse_inertia[axis]) + (a[axis] * input.inverse_inertia[axis]);
        }
        next.visible_area =
            (f64::from(next.visible_area) + f64::from(output.pressure.area).abs()) as f32;
        let retention = 1.0_f32 - settings.viscosity * input.timestep;
        next.entrained_velocity = next
            .entrained_velocity
            .map(|v| (f64::from(v) * f64::from(retention)) as f32);
        let weight = f64::from(settings.entrainment * input.timestep);
        next.entrained_velocity = std::array::from_fn(|axis| {
            (f64::from(input.frame.linear[axis] - current[axis]) * weight
                + f64::from(next.entrained_velocity[axis])) as f32
        });
        if output
            .queued
            .angular
            .iter()
            .chain(&output.queued.linear)
            .chain(&next.entrained_velocity)
            .any(|v| !v.is_finite())
            || !next.visible_area.is_finite()
        {
            return Err(MotionError::NonFinite);
        }
        *self = next;
        Ok(output)
    }
}
fn point_world(frame: FluidPressureFrame, point: [f32; 3]) -> [f64; 3] {
    std::array::from_fn(|row| {
        ((frame.object_basis[row * 3] * f64::from(point[0])
            + frame.object_basis[row * 3 + 1] * f64::from(point[1]))
            + frame.object_basis[row * 3 + 2] * f64::from(point[2]))
            + frame.object_position[row]
    })
}
fn rotate(basis: [f64; 9], vector: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|row| {
        ((basis[row * 3] * f64::from(vector[0]) + basis[row * 3 + 1] * f64::from(vector[1]))
            + basis[row * 3 + 2] * f64::from(vector[2])) as f32
    })
}
fn queue_impulse(
    queue: &mut QueuedVelocity,
    point: [f64; 3],
    impulse: [f32; 3],
    input: FluidBodyInput,
) {
    let lever =
        std::array::from_fn::<_, 3, _>(|i| (point[i] - input.frame.core_position[i]) as f32);
    let torque = [
        lever[1] * impulse[2] - lever[2] * impulse[1],
        lever[2] * impulse[0] - lever[0] * impulse[2],
        lever[0] * impulse[1] - lever[1] * impulse[0],
    ];
    let basis = input.frame.core_basis;
    let local = std::array::from_fn::<_, 3, _>(|column| {
        ((basis[column + 3] * f64::from(torque[1]) + basis[column] * f64::from(torque[0]))
            + basis[column + 6] * f64::from(torque[2])) as f32
    });
    for axis in 0..3 {
        queue.angular[axis] += local[axis] * input.inverse_inertia[axis];
        queue.linear[axis] = (f64::from(queue.linear[axis])
            + f64::from(impulse[axis]) * f64::from(input.inverse_mass))
            as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> FluidBodyInput {
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        FluidBodyInput {
            frame: FluidPressureFrame {
                object_basis: identity,
                object_position: [0.0; 3],
                core_basis: identity,
                core_position: [0.0; 3],
                angular: [0.0; 3],
                linear: [0.0; 3],
                current: [0.0; 3],
                pressure: 0.0,
                friction: 0.0,
                aerodynamic: false,
            },
            plane: [0.0, 1.0, 0.0, 0.0],
            current: [0.0; 3],
            gravity: [0.0, 20.32, 0.0],
            inverse_mass: 0.2,
            inverse_inertia: [1.0; 3],
            queued: QueuedVelocity {
                linear: [1.0, 2.0, 3.0],
                angular: [4.0, 5.0, 6.0],
            },
            timestep: 0.015,
        }
    }
    #[test]
    fn disabled_density_preserves_memory_and_queues_and_bad_step_is_atomic() {
        let settings = FluidSettings {
            density: 0.0,
            pressure_damping: 0.01,
            friction_damping: 0.05,
            torque_factor: 0.01,
            epsilon: 1.0e-10,
            viscosity: 0.0,
            entrainment: 0.1,
            aerodynamic: false,
        };
        let mut state = FluidBodyState {
            entrained_velocity: [1.0, 2.0, 3.0],
            visible_area: 0.5,
            previous_area: 0.2,
        };
        let before = state;
        let data = input();
        let result = state.advance(&[], settings, data).unwrap();
        assert_eq!(state, before);
        assert_eq!(result.queued, data.queued);
        assert_eq!(
            state.advance(
                &[],
                settings,
                FluidBodyInput {
                    timestep: f32::NAN,
                    ..data
                }
            ),
            Err(MotionError::NonFinite)
        );
        assert_eq!(state, before);
    }
}
