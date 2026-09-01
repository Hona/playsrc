use crate::{CoreOrientation, OrientationError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngularStep {
    pub delta: CoreOrientation,
    pub angular_velocity: [f32; 3],
    pub substeps: u32,
}

impl AngularStep {
    pub fn integrate(
        angular_velocity: [f32; 3],
        inertia: [f32; 3],
        timestep: f32,
    ) -> Result<Self, OrientationError> {
        if !timestep.is_finite()
            || angular_velocity
                .iter()
                .chain(&inertia)
                .any(|v| !v.is_finite())
        {
            return Err(OrientationError::NonFinite);
        }
        if timestep <= 0.0 || inertia.iter().any(|v| *v <= 0.0) {
            return Err(OrientationError::Degenerate);
        }
        let inverse = inertia.map(|v| 1.0 / v);
        let factors = [
            (inertia[1] - inertia[2]) * inverse[0],
            (inertia[2] - inertia[0]) * inverse[1],
            (inertia[0] - inertia[1]) * inverse[2],
        ]
        .map(f64::from);
        let squared = (angular_velocity[1] * angular_velocity[1]
            + angular_velocity[0] * angular_velocity[0])
            + angular_velocity[2] * angular_velocity[2];
        let excursion = (f64::from(squared) * f64::from(timestep)) * f64::from(timestep);
        if !excursion.is_finite() {
            return Err(OrientationError::NonFinite);
        }
        let count = if excursion > 1.0 / 36.0 {
            (excursion * 144.0).sqrt().trunc() + 1.0
        } else {
            1.0
        };
        if count > 65532.0 {
            return Err(OrientationError::UnsupportedAngle);
        }
        let count = count as u32;
        let step = f64::from(timestep) / f64::from(count as f32);
        let mut angular = angular_velocity;
        let mut delta = substep(angular, step)?;
        angular = advance_velocity(angular, factors, step);
        for _ in 1..count {
            let b = substep(angular, step)?.quaternion;
            let a = delta.quaternion;
            delta.quaternion = [
                ((a[0] * b[3] + a[3] * b[0]) + a[2] * b[1]) - a[1] * b[2],
                ((a[1] * b[3] + a[3] * b[1]) + a[0] * b[2]) - a[2] * b[0],
                ((a[2] * b[3] + a[3] * b[2]) + a[1] * b[0]) - a[0] * b[1],
                ((a[3] * b[3] - a[0] * b[0]) - a[1] * b[1]) - a[2] * b[2],
            ];
            angular = advance_velocity(angular, factors, step);
        }
        if angular.iter().any(|v| !v.is_finite()) || delta.quaternion.iter().any(|v| !v.is_finite())
        {
            return Err(OrientationError::NonFinite);
        }
        Ok(Self {
            delta,
            angular_velocity: angular,
            substeps: count,
        })
    }
}
fn advance_velocity(angular: [f32; 3], factors: [f64; 3], step: f64) -> [f32; 3] {
    let products = [
        angular[2] * angular[1],
        angular[2] * angular[0],
        angular[1] * angular[0],
    ];
    std::array::from_fn(|axis| {
        let acceleration = (f64::from(products[axis]) * factors[axis]) as f32;
        (f64::from(acceleration) * step + f64::from(angular[axis])) as f32
    })
}
fn substep(angular: [f32; 3], step: f64) -> Result<CoreOrientation, OrientationError> {
    let half = step * 0.5;
    let vector = angular.map(|v| {
        let angle = (f64::from(v) * half) as f32;
        f64::from(angle - (angle * angle) * (angle * (1.0_f32 / 6.0)))
    });
    let squared = (vector[1] * vector[1] + vector[0] * vector[0]) + vector[2] * vector[2];
    if !squared.is_finite() || squared > 1.0 {
        return Err(OrientationError::UnsupportedAngle);
    }
    Ok(CoreOrientation {
        quaternion: [vector[0], vector[1], vector[2], (1.0 - squared).sqrt()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inertia_step_is_bounded_and_retains_first_order_velocity_updates() {
        let result = AngularStep::integrate([1.0, 2.0, 3.0], [2.0, 3.0, 4.0], 0.015).unwrap();
        assert_eq!(result.substeps, 1);
        let factors = [-0.5, 2.0_f32 / 3.0, -0.25];
        assert_eq!(
            result.angular_velocity,
            advance_velocity(
                [1.0, 2.0, 3.0],
                factors.map(f64::from),
                f64::from(0.015_f32)
            )
        );
        assert_eq!(
            AngularStep::integrate([0.0; 3], [2.0, 3.0, 4.0], 0.015)
                .unwrap()
                .delta
                .quaternion,
            [0.0, 0.0, 0.0, 1.0]
        );
        assert!(
            AngularStep::integrate([4.0, 2.0, 3.0], [2.0, 3.0, 4.0], 0.1)
                .unwrap()
                .substeps
                > 1
        );
        assert!(AngularStep::integrate([f32::MAX; 3], [1.0; 3], 1.0).is_err());
        assert!(AngularStep::integrate([1.0; 3], [1.0; 3], 0.0).is_err());
        assert!(AngularStep::integrate([1.0; 3], [f32::NAN; 3], 0.015).is_err());
    }
}
