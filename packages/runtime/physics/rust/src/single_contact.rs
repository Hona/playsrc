use crate::{
    ContactNormalRow, DynamicEndpoint, ImpactContactPoint, NormalBody, NormalContact,
    ResponseError, TangentBody,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleContactNormal {
    pub point: [f64; 3],
    pub normal: [f32; 3],
    pub distance: f32,
    pub target_distance: f32,
    pub inverse_step: f32,
    pub endpoints: [Option<TangentBody>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleContactNormalResult {
    pub local_offsets: [Option<[f32; 3]>; 2],
    pub rows: [Option<ContactNormalRow>; 2],
    pub effective_mass: f32,
    pub relative_velocity: f64,
    pub impulse: f64,
    pub normal_force: f32,
    pub endpoints: [Option<NormalBody>; 2],
}

impl SingleContactNormal {
    pub fn solve(self) -> Result<SingleContactNormalResult, ResponseError> {
        if self.point.iter().any(|value| !value.is_finite())
            || self.normal.iter().any(|value| !value.is_finite())
            || !self.distance.is_finite()
            || !self.target_distance.is_finite()
            || !self.inverse_step.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.inverse_step <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        if self.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection);
        }
        if self.endpoints.iter().all(Option::is_none) {
            return Err(ResponseError::InvalidSystemShape);
        }
        let mut offsets = [None; 2];
        let mut rows = [None; 2];
        let mut bodies = [None; 2];
        let mut inverse_response = 0.0_f32;
        let mut relative = 0.0_f64;
        for (index, endpoint) in self.endpoints.into_iter().enumerate() {
            let Some(endpoint) = endpoint else {
                continue;
            };
            if endpoint
                .linear_velocity
                .iter()
                .chain(endpoint.angular_velocity.iter())
                .chain(endpoint.inverse_inertia.iter())
                .any(|value| !value.is_finite())
                || !endpoint.inverse_mass.is_finite()
            {
                return Err(ResponseError::NonFinite);
            }
            if endpoint.inverse_mass < 0.0
                || endpoint.inverse_inertia.iter().any(|value| *value < 0.0)
            {
                return Err(ResponseError::NonPositiveMass);
            }
            let offset = ImpactContactPoint::from_world(
                endpoint.position,
                endpoint.orientation,
                self.point,
            )?;
            let row = ContactNormalRow::from_local(
                self.normal,
                offset,
                endpoint.orientation,
                self.distance,
                if index == 0 {
                    DynamicEndpoint::First
                } else {
                    DynamicEndpoint::Second
                },
            )?;
            let weighted = std::array::from_fn::<_, 3, _>(|axis| {
                row.angular_jacobian[axis] * endpoint.inverse_inertia[axis]
            });
            inverse_response += dot(row.angular_jacobian, weighted) + endpoint.inverse_mass;
            let linear = dot(self.normal, endpoint.linear_velocity);
            let angular = dot(row.angular_jacobian, endpoint.angular_velocity);
            relative += if index == 0 {
                f64::from(linear) + f64::from(angular)
            } else {
                f64::from(-linear) - f64::from(angular)
            };
            bodies[index] = Some(NormalBody {
                linear_velocity: endpoint.linear_velocity,
                angular_velocity: endpoint.angular_velocity,
                inverse_mass: endpoint.inverse_mass,
                inverse_inertia: endpoint.inverse_inertia,
            });
            offsets[index] = Some(offset);
            rows[index] = Some(row);
        }
        if !inverse_response.is_finite() || !relative.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        let effective_mass = 1.0_f32 / inverse_response;
        let impulse = NormalContact {
            distance: self.distance,
            target_distance: self.target_distance,
            relative_speed: relative,
            effective_mass,
        }
        .impulse()?;
        if impulse > 0.0 {
            for (row, body) in rows.iter().zip(bodies.iter_mut()) {
                if let (Some(row), Some(value)) = (row, *body) {
                    *body = Some(row.apply_impulse(value, impulse)?);
                }
            }
        }
        let normal_force = (impulse * f64::from(self.inverse_step)) as f32;
        if !normal_force.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        Ok(SingleContactNormalResult {
            local_offsets: offsets,
            rows,
            effective_mass,
            relative_velocity: relative,
            impulse,
            normal_force,
            endpoints: bodies,
        })
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_retained_world_point_produces_exact_normal_offsets_and_velocity() {
        let input = SingleContactNormal {
            point: [
                13799038378304667861,
                13804538105322059980,
                4602750876922383349,
            ]
            .map(f64::from_bits),
            normal: [0.0, 1.0, 0.0],
            distance: f32::from_bits(1016893987),
            target_distance: f32::from_bits(1011875418),
            inverse_step: f32::from_bits(1116034390),
            endpoints: [
                Some(TangentBody {
                    position: [
                        13783748225472481322,
                        13817254807444739744,
                        4602750876922383354,
                    ]
                    .map(f64::from_bits),
                    orientation: [
                        4607164579040287763,
                        13812569389701371861,
                        4356787494549360152,
                        4589197352846596053,
                        4607164579040287763,
                        4390729384596335211,
                        13596649178654469738,
                        13614081035514712853,
                        4607182418800017408,
                    ]
                    .map(f64::from_bits),
                    inverse_mass: f32::from_bits(1045220557),
                    inverse_inertia: [1114200445, 1114561143, 1114200445].map(f32::from_bits),
                    angular_velocity: [2846769914, 659495879, 1068842629].map(f32::from_bits),
                    linear_velocity: [3163764038, 1047193066, 2783633189].map(f32::from_bits),
                }),
                None,
            ],
        };
        let result = input.solve().unwrap();
        assert_eq!(
            result.local_offsets[0].unwrap().map(f32::to_bits),
            [204080102, 1038438498, 2772741644]
        );
        assert_eq!(
            result.rows[0].unwrap().angular_jacobian.map(f32::to_bits),
            [639641984, 2739271164, 3152465595]
        );
        assert_eq!(result.effective_mass.to_bits(), 1084078002);
        assert_eq!(result.relative_velocity.to_bits(), 4597073423800205312);
        assert_eq!(result.impulse.to_bits(), 4601756960705402496);
        let body = result.endpoints[0].unwrap();
        assert_eq!(
            body.angular_velocity.map(f32::to_bits),
            [2848917029, 660854096, 1070389719]
        );
        assert_eq!(
            body.linear_velocity.map(f32::to_bits),
            [3163764038, 1041169018, 2783633189]
        );
    }

    #[test]
    fn normal_response_updates_both_endpoints_and_leaves_separating_bodies_unchanged() {
        let body = TangentBody {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            inverse_mass: 1.0,
            inverse_inertia: [1.0; 3],
            angular_velocity: [0.0; 3],
            linear_velocity: [0.0, 1.0, 0.0],
        };
        let input = SingleContactNormal {
            point: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: 0.0,
            target_distance: 0.0,
            inverse_step: 64.0,
            endpoints: [
                Some(body),
                Some(TangentBody {
                    linear_velocity: [0.0, -1.0, 0.0],
                    ..body
                }),
            ],
        };
        let result = input.solve().unwrap();
        assert_eq!(result.impulse, 1.0);
        assert_eq!(result.normal_force, 64.0);
        for endpoint in result.endpoints.into_iter().flatten() {
            assert_eq!(endpoint.linear_velocity, [0.0; 3]);
        }
        let separating = SingleContactNormal {
            normal: [0.0, -1.0, 0.0],
            ..input
        }
        .solve()
        .unwrap();
        assert_eq!(separating.impulse, 0.0);
        assert_eq!(
            separating.endpoints[0].unwrap().linear_velocity,
            body.linear_velocity
        );
        assert_eq!(
            SingleContactNormal {
                endpoints: [None, None],
                ..input
            }
            .solve(),
            Err(ResponseError::InvalidSystemShape)
        );
        assert_eq!(
            SingleContactNormal {
                inverse_step: 0.0,
                ..input
            }
            .solve(),
            Err(ResponseError::NonPositiveTimestep)
        );
        assert_eq!(
            SingleContactNormal {
                normal: [f32::NAN, 0.0, 0.0],
                ..input
            }
            .solve(),
            Err(ResponseError::NonFinite)
        );
    }
}
