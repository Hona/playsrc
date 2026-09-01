use crate::{AssembledNormalSystem, NormalAssembly, ResponseError, response::ordered_dot};

impl NormalAssembly<'_> {
    pub fn assemble(self) -> Result<AssembledNormalSystem, ResponseError> {
        let count = self.rows.len();
        if count > self.maximum_dimension {
            return Err(ResponseError::InvalidSystemShape);
        }
        let entries = count
            .checked_mul(count)
            .ok_or(ResponseError::InvalidSystemShape)?;
        if !self.target_distance.is_finite() || !self.timestep.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        for body in self.bodies {
            if !body.inverse_mass.is_finite()
                || body
                    .angular_velocity
                    .iter()
                    .chain(&body.linear_velocity)
                    .chain(&body.inverse_inertia)
                    .any(|v| !v.is_finite())
            {
                return Err(ResponseError::NonFinite);
            }
            if body.inverse_mass < 0.0 || body.inverse_inertia.iter().any(|v| *v < 0.0) {
                return Err(ResponseError::NonPositiveMass);
            }
        }
        for row in self.rows {
            if !row.distance.is_finite()
                || row.normal.iter().any(|v| !v.is_finite())
                || row
                    .endpoints
                    .iter()
                    .flatten()
                    .any(|endpoint| endpoint.angular_jacobian.iter().any(|v| !v.is_finite()))
            {
                return Err(ResponseError::NonFinite);
            }
            if row.endpoints.iter().all(Option::is_none)
                || row
                    .endpoints
                    .iter()
                    .flatten()
                    .any(|e| e.body >= self.bodies.len())
                || matches!(row.endpoints,[Some(a),Some(b)] if a.body==b.body)
            {
                return Err(ResponseError::InvalidSystemShape);
            }
        }
        let mut matrix = vec![0.0; entries];
        let mut right_hand_side = Vec::with_capacity(count);
        let mut inverse_responses = Vec::with_capacity(count);
        for (column, impulse) in self.rows.iter().enumerate() {
            let mut closing = 0.0;
            let mut inverse = 0.0_f32;
            for (side, endpoint) in impulse.endpoints.iter().enumerate() {
                let Some(endpoint) = endpoint else {
                    continue;
                };
                let body = self.bodies[endpoint.body];
                let linear = ordered_dot(impulse.normal, body.linear_velocity);
                let angular = ordered_dot(endpoint.angular_jacobian, body.angular_velocity);
                closing += if side == 0 {
                    f64::from(linear) + f64::from(angular)
                } else {
                    f64::from(-linear) - f64::from(angular)
                };
                let angular_response = std::array::from_fn(|axis| {
                    endpoint.angular_jacobian[axis] * body.inverse_inertia[axis]
                });
                inverse +=
                    ordered_dot(endpoint.angular_jacobian, angular_response) + body.inverse_mass;
                let signed_inverse = if side == 0 {
                    -body.inverse_mass
                } else {
                    body.inverse_mass
                };
                let translation = impulse
                    .normal
                    .map(|value| (f64::from(value) * f64::from(signed_inverse)) as f32);
                for (row_index, row) in self.rows.iter().enumerate() {
                    let Some((row_side, other)) =
                        row.endpoints.iter().enumerate().find_map(|(i, other)| {
                            other.filter(|v| v.body == endpoint.body).map(|v| (i, v))
                        })
                    else {
                        continue;
                    };
                    let linear = f64::from(ordered_dot(translation, row.normal));
                    let angular = f64::from(ordered_dot(angular_response, other.angular_jacobian));
                    let contribution = (if side == 0 {
                        linear - angular
                    } else {
                        linear + angular
                    }) * if row_side == 0 { -1.0 } else { 1.0 };
                    let updated = contribution + matrix[row_index * count + column];
                    matrix[row_index * count + column] = updated;
                }
            }
            let delta = self.target_distance - impulse.distance;
            let stiffness = if delta < 0.0 { 20.0 } else { 1.0 };
            right_hand_side.push(closing + f64::from(delta) * stiffness);
            inverse_responses.push(inverse);
        }
        if matrix
            .iter()
            .chain(&right_hand_side)
            .any(|v| !v.is_finite())
            || inverse_responses.iter().any(|v| !v.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        Ok(AssembledNormalSystem {
            matrix,
            right_hand_side,
            inverse_responses,
            timestep: self.timestep,
        })
    }
}
