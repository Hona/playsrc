use crate::{MotionError, arithmetic::refined_inverse_root};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidPressureFrame {
    pub object_basis: [f64; 9],
    pub object_position: [f64; 3],
    pub core_basis: [f64; 9],
    pub core_position: [f64; 3],
    pub angular: [f32; 3],
    pub linear: [f32; 3],
    pub current: [f32; 3],
    pub pressure: f32,
    pub friction: f32,
    pub aerodynamic: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FluidPressure {
    pub area: f32,
    pub impulse: [f32; 3],
    pub point_torque: [f32; 3],
    pub current_torque: [f32; 3],
}
impl FluidPressure {
    pub fn add_triangle(
        &mut self,
        points: [[f32; 3]; 3],
        plane: [f32; 4],
        epsilon: f32,
        frame: FluidPressureFrame,
    ) -> Result<(), MotionError> {
        if points
            .iter()
            .flatten()
            .chain(&plane)
            .chain([&epsilon, &frame.pressure, &frame.friction])
            .chain(&frame.angular)
            .chain(&frame.linear)
            .chain(&frame.current)
            .any(|v| !v.is_finite())
            || epsilon < 0.0
            || frame
                .object_basis
                .iter()
                .chain(&frame.core_basis)
                .chain(&frame.object_position)
                .chain(&frame.core_position)
                .any(|v| !v.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        let Some((centroid, fraction)) = wet_centroid(points, plane, epsilon) else {
            return Ok(());
        };
        let a =
            std::array::from_fn::<_, 3, _>(|i| f64::from(points[1][i]) - f64::from(points[0][i]));
        let b =
            std::array::from_fn::<_, 3, _>(|i| f64::from(points[2][i]) - f64::from(points[0][i]));
        let normal = [
            (a[1] * b[2] - a[2] * b[1]) as f32,
            (a[2] * b[0] - a[0] * b[2]) as f32,
            (a[0] * b[1] - a[1] * b[0]) as f32,
        ];
        let normal_squared = dot(normal, normal);
        if normal_squared < epsilon {
            return Ok(());
        }
        let world = std::array::from_fn::<_, 3, _>(|row| {
            ((frame.object_basis[row * 3] * f64::from(centroid[0])
                + frame.object_basis[row * 3 + 1] * f64::from(centroid[1]))
                + frame.object_basis[row * 3 + 2] * f64::from(centroid[2]))
                + frame.object_position[row]
        });
        let delta = std::array::from_fn::<_, 3, _>(|i| world[i] - frame.core_position[i]);
        let lever = transpose(frame.core_basis, delta);
        let spin = cross(frame.angular, lever);
        let velocity = std::array::from_fn::<_, 3, _>(|row| {
            let rotated = ((frame.core_basis[row * 3] * f64::from(spin[0])
                + frame.core_basis[row * 3 + 1] * f64::from(spin[1]))
                + frame.core_basis[row * 3 + 2] * f64::from(spin[2]))
                as f32;
            rotated + frame.linear[row]
        });
        let local_velocity = transpose(frame.object_basis, velocity.map(f64::from));
        let local_current = transpose(frame.object_basis, frame.current.map(f64::from));
        let mut relative = std::array::from_fn(|axis| local_current[axis] - local_velocity[axis]);
        let mut speed_squared = dot(relative, relative);
        if speed_squared < epsilon {
            relative = [0.0, 1.0, 0.0];
            speed_squared = 0.0;
        }
        if dot(relative, normal) > epsilon {
            return Ok(());
        }
        let direction = normalize::<4>(relative).0;
        let (unit, length) = normalize::<5>(normal);
        let area = (length * 0.5) as f32;
        let projected = -((dot(unit, direction) * fraction) * area);
        let pressure = (projected * speed_squared) * frame.pressure;
        let friction = (area * speed_squared) * frame.friction;
        let factor = f64::from(pressure) + f64::from(friction);
        let impulse = if frame.aerodynamic {
            unit.map(|v| (f64::from(v) * (-factor)) as f32)
        } else {
            direction.map(|v| (f64::from(v) * factor) as f32)
        };
        let torque = cross(centroid, impulse);
        let projected_direction = -dot(direction, unit);
        let tangent = std::array::from_fn(|i| {
            (f64::from(unit[i]) * f64::from(projected_direction) + f64::from(direction[i])) as f32
        });
        let current_torque = cross(tangent, impulse);
        let next = Self {
            area: self.area + projected.abs(),
            impulse: std::array::from_fn(|i| self.impulse[i] + impulse[i]),
            point_torque: std::array::from_fn(|i| self.point_torque[i] + torque[i]),
            current_torque: std::array::from_fn(|i| self.current_torque[i] + current_torque[i]),
        };
        if [&next.area]
            .into_iter()
            .chain(&next.impulse)
            .chain(&next.point_torque)
            .chain(&next.current_torque)
            .any(|v| !v.is_finite())
        {
            return Err(MotionError::NonFinite);
        }
        *self = next;
        Ok(())
    }
}
fn transpose(basis: [f64; 9], value: [f64; 3]) -> [f32; 3] {
    std::array::from_fn(|column| {
        ((basis[column + 3] * value[1] + basis[column] * value[0]) + basis[column + 6] * value[2])
            as f32
    })
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize<const STEPS: usize>(value: [f32; 3]) -> ([f32; 3], f64) {
    let squared = f64::from(dot(value, value));
    if squared < 1.0e-19 {
        return (value, 0.0);
    }
    let inverse = refined_inverse_root::<STEPS>(squared);
    (
        value.map(|v| (f64::from(v) * inverse) as f32),
        inverse * squared,
    )
}
fn wet_centroid(points: [[f32; 3]; 3], plane: [f32; 4], epsilon: f32) -> Option<([f32; 3], f32)> {
    let mut distances = [0.0; 3];
    let mut negative = 0;
    let mut positive_index = 0;
    let mut negative_index = 0;
    for i in (0..3).rev() {
        let p = points[i];
        let d = (p[0] * plane[0] + p[1] * plane[1]) + (p[2] * plane[2] + plane[3]);
        if d < 0.0 {
            negative += 1;
            negative_index = i;
            distances[i] = d - epsilon;
        } else {
            positive_index = i;
            distances[i] = d + epsilon;
        }
    }
    let full = std::array::from_fn::<_, 3, _>(|axis| {
        (f64::from((points[0][axis] + points[1][axis]) + points[2][axis]) * (1.0 / 3.0)) as f32
    });
    if negative == 3 {
        return None;
    }
    if negative == 0 {
        return Some((full, 1.0));
    }
    let index = if negative == 1 {
        negative_index
    } else {
        positive_index
    };
    let next = (index + 1) % 3;
    let last = (index + 2) % 3;
    let a = f64::from(distances[index] / (distances[index] - distances[next]));
    let b = f64::from(distances[index] / (distances[index] - distances[last]));
    let product = a * b;
    let weights = [
        ((3.0 - a) - b) * (1.0 / 3.0),
        a * (1.0 / 3.0),
        b * (1.0 / 3.0),
    ];
    let clipped = std::array::from_fn::<_, 3, _>(|axis| {
        let p = (f64::from(points[index][axis]) * weights[0]) as f32;
        let q = (f64::from(p) + f64::from(points[next][axis]) * weights[1]) as f32;
        (f64::from(q) + f64::from(points[last][axis]) * weights[2]) as f32
    });
    if negative == 2 {
        return Some((clipped, product as f32));
    }
    let remaining = 1.0 - product;
    if remaining < f64::from(epsilon) {
        return None;
    }
    let inverse = 1.0 / remaining;
    Some((
        std::array::from_fn(|axis| {
            (f64::from(full[axis] - (f64::from(clipped[axis]) * product) as f32) * inverse) as f32
        }),
        remaining as f32,
    ))
}
