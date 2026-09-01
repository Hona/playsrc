use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnergyError {
    NonFinite,
    NonPositiveTimestep,
    NonPositiveMass,
    NonPositiveInertia,
    NegativeMagnitude,
    NegativeEnergy,
}

impl fmt::Display for EnergyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => formatter.write_str("contact energy contains a non-finite value"),
            Self::NonPositiveTimestep => formatter.write_str("contact timestep must be positive"),
            Self::NonPositiveMass => formatter.write_str("contact endpoint mass must be positive"),
            Self::NonPositiveInertia => {
                formatter.write_str("contact endpoint inertia must be positive")
            }
            Self::NegativeMagnitude => {
                formatter.write_str("contact squared impulse cannot be negative")
            }
            Self::NegativeEnergy => {
                formatter.write_str("contact retained energy cannot be negative")
            }
        }
    }
}

impl std::error::Error for EnergyError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentEnergySample {
    pub magnitude_squared: f64,
    pub retained: [f32; 2],
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TangentEnergyTracker {
    previous: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentEnergyTransition {
    pub previous: f32,
    pub current: f32,
    pub change: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QueuedVelocity {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KineticEnergyInput {
    pub mass: f32,
    pub inertia: [f32; 3],
    pub linear: [f32; 3],
    pub angular: [f32; 3],
    pub queued: QueuedVelocity,
}
impl KineticEnergyInput {
    pub fn energy(self) -> Result<f64, EnergyError> {
        if !self.mass.is_finite()
            || self
                .inertia
                .iter()
                .chain(&self.linear)
                .chain(&self.angular)
                .chain(&self.queued.linear)
                .chain(&self.queued.angular)
                .any(|v| !v.is_finite())
        {
            return Err(EnergyError::NonFinite);
        }
        if self.mass <= 0.0 {
            return Err(EnergyError::NonPositiveMass);
        }
        if self.inertia.iter().any(|v| *v <= 0.0) {
            return Err(EnergyError::NonPositiveInertia);
        }
        let linear = std::array::from_fn::<_, 3, _>(|i| self.linear[i] + self.queued.linear[i]);
        let angular = std::array::from_fn::<_, 3, _>(|i| self.angular[i] + self.queued.angular[i]);
        let angular = ((angular[1] * angular[1]) * self.inertia[1]
            + (angular[0] * angular[0]) * self.inertia[0])
            + (angular[2] * angular[2]) * self.inertia[2];
        let linear = (linear[0] * linear[0] + linear[1] * linear[1]) + linear[2] * linear[2];
        let energy = (f64::from(angular) + f64::from(linear) * f64::from(self.mass)) * 0.5;
        if !energy.is_finite() {
            return Err(EnergyError::NonFinite);
        }
        Ok(energy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MutualEnergyEndpoint {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
    pub orientation: [f64; 9],
    pub inertia: [f32; 3],
    pub mass: f32,
    pub inverse_mass: f32,
    pub immovable: bool,
    pub pending: QueuedVelocity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MutualEnergyInput {
    pub endpoints: [MutualEnergyEndpoint; 2],
    pub accumulated: f32,
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MutualEnergyResult {
    pub damped: f32,
    pub removed: f64,
    pub remaining: f32,
    pub translation_direction: [f32; 3],
    pub angular_directions: [[f32; 3]; 2],
    pub translation_speed: f64,
    pub angular_speed: f64,
    pub translation_mass: [f64; 2],
    pub translation_inverse: [f64; 2],
    pub angular_mass: [f64; 2],
    pub angular_inverse: [f64; 2],
    pub energies: [f64; 2],
    pub pending: [QueuedVelocity; 2],
}

impl TangentEnergyTracker {
    pub fn new(previous: f32) -> Result<Self, EnergyError> {
        if !previous.is_finite() {
            return Err(EnergyError::NonFinite);
        }
        if previous < 0.0 {
            return Err(EnergyError::NegativeEnergy);
        }
        Ok(Self { previous })
    }

    pub fn previous(self) -> f32 {
        self.previous
    }

    pub fn advance(
        &mut self,
        sample: TangentEnergySample,
    ) -> Result<TangentEnergyTransition, EnergyError> {
        if !sample.magnitude_squared.is_finite()
            || !sample.timestep.is_finite()
            || sample.retained.iter().any(|value| !value.is_finite())
        {
            return Err(EnergyError::NonFinite);
        }
        if sample.timestep <= 0.0 {
            return Err(EnergyError::NonPositiveTimestep);
        }
        if sample.magnitude_squared < 0.0 {
            return Err(EnergyError::NegativeMagnitude);
        }
        let squared = (f64::from(sample.timestep)
            * sample.magnitude_squared
            * f64::from(sample.timestep)) as f32;
        let retained_squared =
            sample.retained[0] * sample.retained[0] + sample.retained[1] * sample.retained[1];
        let current = (f64::from(squared * retained_squared).sqrt() * 0.5) as f32;
        let previous = self.previous;
        self.previous = current;
        Ok(TangentEnergyTransition {
            previous,
            current,
            change: current - previous,
        })
    }
}

impl MutualEnergyInput {
    pub fn reduce(self) -> Result<MutualEnergyResult, EnergyError> {
        if !self.accumulated.is_finite()
            || !self.timestep.is_finite()
            || self.endpoints.iter().any(|endpoint| {
                !endpoint.mass.is_finite()
                    || !endpoint.inverse_mass.is_finite()
                    || endpoint
                        .linear
                        .iter()
                        .chain(endpoint.angular.iter())
                        .chain(endpoint.inertia.iter())
                        .chain(endpoint.pending.linear.iter())
                        .chain(endpoint.pending.angular.iter())
                        .any(|component| !component.is_finite())
                    || endpoint
                        .orientation
                        .iter()
                        .any(|component| !component.is_finite())
            })
        {
            return Err(EnergyError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(EnergyError::NonPositiveTimestep);
        }
        if self.accumulated < 0.0 {
            return Err(EnergyError::NegativeEnergy);
        }
        if self
            .endpoints
            .iter()
            .any(|endpoint| endpoint.mass <= 0.0 || endpoint.inverse_mass <= 0.0)
        {
            return Err(EnergyError::NonPositiveMass);
        }
        if self
            .endpoints
            .iter()
            .any(|endpoint| endpoint.inertia.iter().any(|component| *component <= 0.0))
        {
            return Err(EnergyError::NonPositiveInertia);
        }

        let mut endpoints = self.endpoints;
        let swapped = endpoints[1].immovable && !endpoints[0].immovable;
        if swapped {
            endpoints.swap(0, 1);
        }

        let relative_linear =
            std::array::from_fn(|axis| endpoints[1].linear[axis] - endpoints[0].linear[axis]);
        let (mut translation_direction, translation_speed) = normalize(relative_linear);
        let first_world = rotate(endpoints[0].orientation, endpoints[0].angular);
        let second_world = rotate(endpoints[1].orientation, endpoints[1].angular);
        let relative_angular = std::array::from_fn(|axis| second_world[axis] - first_world[axis]);
        let (world_direction, angular_speed) = normalize(relative_angular);
        let first_direction = inverse_rotate(endpoints[0].orientation, world_direction);
        let second_direction = inverse_rotate(
            endpoints[1].orientation,
            world_direction.map(|component| -component),
        );
        let mut angular_directions = [first_direction, second_direction];
        let mut angular_mass = std::array::from_fn(|index| {
            let weighted: [f32; 3] = std::array::from_fn(|axis| {
                angular_directions[index][axis] * endpoints[index].inertia[axis]
            });
            let squared =
                (weighted[0] * weighted[0] + weighted[1] * weighted[1]) + weighted[2] * weighted[2];
            let length = f64::from(squared).sqrt();
            if length < 1.0e-19 { 1.0 } else { length }
        });
        let mut angular_inverse = angular_mass.map(|mass| 1.0 / mass);
        let mut translation_mass = endpoints.map(|endpoint| f64::from(endpoint.mass));
        let mut translation_inverse = endpoints.map(|endpoint| f64::from(endpoint.inverse_mass));
        if endpoints[0].immovable {
            translation_mass[0] = translation_mass[1] * 10_000.0;
            translation_inverse[0] = translation_inverse[1] * 0.0001;
            angular_mass[0] = angular_mass[1] * 10_000.0;
            angular_inverse[0] = angular_inverse[1] * 0.0001;
        }

        let rotational_energy = potential(angular_speed, angular_mass, angular_inverse);
        let translational_energy =
            potential(translation_speed, translation_mass, translation_inverse);
        let energies = [rotational_energy, translational_energy];
        let total = rotational_energy + translational_energy;
        let damping = (f64::from(0.9_f32).ln() * f64::from(self.timestep)).exp();
        let damped = (f64::from(self.accumulated) * damping) as f32;
        let removed = if total < 1.0e-19 {
            0.0
        } else {
            f64::from(damped).min(total * f64::from(0.1_f32))
        };
        let mut pending = endpoints.map(|endpoint| endpoint.pending);
        if total >= 1.0e-19 {
            let fraction = removed / total;
            let angular_impulse =
                impulse(angular_speed, angular_inverse, fraction * rotational_energy);
            let linear_impulse = impulse(
                translation_speed,
                translation_inverse,
                fraction * translational_energy,
            );
            if !endpoints[0].immovable {
                for axis in 0..3 {
                    pending[0].linear[axis] = (f64::from(pending[0].linear[axis])
                        + f64::from(translation_direction[axis])
                            * (translation_inverse[0] * linear_impulse))
                        as f32;
                    pending[0].angular[axis] = (f64::from(pending[0].angular[axis])
                        + f64::from(angular_directions[0][axis])
                            * (angular_inverse[0] * angular_impulse))
                        as f32;
                }
            }
            translation_direction = translation_direction.map(|component| -component);
            for axis in 0..3 {
                pending[1].linear[axis] = (f64::from(pending[1].linear[axis])
                    + f64::from(translation_direction[axis])
                        * (translation_inverse[1] * linear_impulse))
                    as f32;
                pending[1].angular[axis] = (f64::from(pending[1].angular[axis])
                    + f64::from(angular_directions[1][axis])
                        * (angular_inverse[1] * angular_impulse))
                    as f32;
            }
        }
        if swapped {
            pending.swap(0, 1);
            angular_directions.swap(0, 1);
            angular_mass.swap(0, 1);
            angular_inverse.swap(0, 1);
            translation_mass.swap(0, 1);
            translation_inverse.swap(0, 1);
        }
        Ok(MutualEnergyResult {
            damped,
            removed,
            remaining: (f64::from(damped) - removed) as f32,
            translation_direction,
            angular_directions,
            translation_speed,
            angular_speed,
            translation_mass,
            translation_inverse,
            angular_mass,
            angular_inverse,
            energies,
            pending,
        })
    }
}

fn normalize(vector: [f32; 3]) -> ([f32; 3], f64) {
    let squared = (vector[0] * vector[0] + vector[1] * vector[1]) + vector[2] * vector[2];
    if squared < 1.0e-19 {
        return (vector, 0.0);
    }
    let value = f64::from(squared);
    let high = (value.to_bits() >> 32) as u32;
    let exponent = (0x7ff0_0000_u32.wrapping_sub(high) as i32) >> 1;
    let mut inverse = f64::from_bits(u64::from((exponent as u32).wrapping_add(0x1ff0_0000)) << 32);
    let half = value * 0.5;
    for _ in 0..5 {
        inverse *= 1.5 - inverse * inverse * half;
    }
    (
        vector.map(|component| (f64::from(component) * inverse) as f32),
        inverse * value,
    )
}

fn rotate(matrix: [f64; 9], vector: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|axis| {
        ((matrix[axis * 3] * f64::from(vector[0]) + matrix[axis * 3 + 1] * f64::from(vector[1]))
            + matrix[axis * 3 + 2] * f64::from(vector[2])) as f32
    })
}

fn inverse_rotate(matrix: [f64; 9], vector: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|axis| {
        ((matrix[axis] * f64::from(vector[0]) + matrix[axis + 3] * f64::from(vector[1]))
            + matrix[axis + 6] * f64::from(vector[2])) as f32
    })
}

fn potential(speed: f64, mass: [f64; 2], inverse: [f64; 2]) -> f64 {
    let present = mass[1] * speed * speed;
    let impulse = speed / (inverse[0] + inverse[1]);
    let second_speed = speed - impulse * inverse[1];
    let second_energy = mass[1] * second_speed * second_speed;
    let first_speed = impulse * inverse[0];
    let first_energy = mass[0] * first_speed * first_speed;
    ((present + 1.0e-19) - (first_energy + second_energy)).max(0.0) * 0.5
}

fn impulse(speed: f64, inverse: [f64; 2], energy: f64) -> f64 {
    let doubled = energy * 2.0;
    let root = (speed * speed - (inverse[0] + inverse[1]) * doubled)
        .abs()
        .sqrt();
    (speed - root) / (inverse[0] + inverse[1])
}

#[cfg(test)]
mod tests {
    use super::{
        EnergyError, MutualEnergyEndpoint, MutualEnergyInput, QueuedVelocity, TangentEnergySample,
        TangentEnergyTracker,
    };

    fn dynamic() -> MutualEnergyEndpoint {
        MutualEnergyEndpoint {
            linear: [
                f32::from_bits(0x3ef7_7c5c),
                f32::from_bits(0xbd1f_7a78),
                f32::from_bits(0xb267_ee49),
            ],
            angular: [
                f32::from_bits(0x345a_830e),
                f32::from_bits(0xb42d_b5f1),
                f32::from_bits(0x40ab_6110),
            ],
            orientation: [
                f64::from_bits(0x3feb_4c2d_4755_c0ca),
                f64::from_bits(0xbfe0_b2d6_c557_806e),
                f64::from_bits(0xbe06_c060_0f15_48f0),
                f64::from_bits(0x3fe0_b2d6_c557_806e),
                f64::from_bits(0x3feb_4c2d_4755_c0ca),
                f64::from_bits(0xbe3d_7dde_5da5_6562),
                f64::from_bits(0x3e31_d0d0_879d_c7c9),
                f64::from_bits(0x3e37_ac7d_6fcb_08c2),
                1.0,
            ],
            inertia: [
                f32::from_bits(0x3c8c_6cad),
                f32::from_bits(0x3c89_3053),
                f32::from_bits(0x3c8c_6cad),
            ],
            mass: 5.0,
            inverse_mass: 0.2,
            immovable: false,
            pending: QueuedVelocity::default(),
        }
    }

    fn fixed() -> MutualEnergyEndpoint {
        MutualEnergyEndpoint {
            linear: [0.0; 3],
            angular: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            inertia: [1.0; 3],
            mass: 1.0,
            inverse_mass: 1.0,
            immovable: true,
            pending: QueuedVelocity::default(),
        }
    }

    #[test]
    fn tangent_energy_retains_target_widths_and_previous_owner_energy() {
        let mut owner = TangentEnergyTracker::default();
        let update = owner
            .advance(TangentEnergySample {
                magnitude_squared: f64::from_bits(0x3fc7_1088_6000_0000),
                retained: [f32::from_bits(0x3b2d_36f4), f32::from_bits(0xaf48_fe86)],
                timestep: f32::from_bits(0x3c75_c28f),
            })
            .unwrap();
        assert_eq!(update.previous.to_bits(), 0);
        assert_eq!(update.current.to_bits(), 0x370d_2c75);
        assert_eq!(update.change.to_bits(), 0x370d_2c75);
        assert_eq!(owner.previous().to_bits(), 0x370d_2c75);
    }

    #[test]
    fn static_pair_energy_reduction_matches_directions_potentials_and_queued_impulses() {
        let result = MutualEnergyInput {
            endpoints: [fixed(), dynamic()],
            accumulated: f32::from_bits(0x370d_2c75),
            timestep: f32::from_bits(0x3c75_c28f),
        }
        .reduce()
        .unwrap();
        assert_eq!(result.damped.to_bits(), 0x370c_f363);
        assert_eq!(result.removed.to_bits(), 0x3ee1_9e6c_6000_0000);
        assert_eq!(
            result.energies.map(f64::to_bits),
            [0x3fcf_76a0_e0db_ee6d, 0x3fe2_cfa8_d418_0fc5]
        );
        assert_eq!(
            result.pending[1].linear.map(f32::to_bits),
            [0xb623_6fa4, 0x3452_a271, 0x2999_29ef]
        );
        assert_eq!(
            result.pending[1].angular.map(f32::to_bits),
            [0xab90_4d5b, 0x2b65_6ea2, 0xb7e2_5a4b]
        );
    }

    #[test]
    fn nonfinite_negative_and_invalid_energy_inputs_are_rejected() {
        assert_eq!(
            TangentEnergyTracker::new(-1.0),
            Err(EnergyError::NegativeEnergy)
        );
        let mut owner = TangentEnergyTracker::default();
        assert_eq!(
            owner.advance(TangentEnergySample {
                magnitude_squared: -1.0,
                retained: [0.0; 2],
                timestep: 0.015,
            }),
            Err(EnergyError::NegativeMagnitude)
        );
        assert_eq!(
            MutualEnergyInput {
                endpoints: [fixed(), dynamic()],
                accumulated: 0.0,
                timestep: 0.0,
            }
            .reduce(),
            Err(EnergyError::NonPositiveTimestep)
        );
    }
}
