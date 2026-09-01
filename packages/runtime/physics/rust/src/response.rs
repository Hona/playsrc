use crate::topology::reciprocal_square_root_f64;
use crate::{EdgeId, FeatureTopology, TopologyError};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalContact {
    pub target_distance: f32,
    pub distance: f32,
    pub relative_speed: f64,
    pub effective_mass: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngularVelocityLimit {
    pub maximum_per_step: f32,
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionVelocityLimits {
    pub maximum_linear: f32,
    pub angular: AngularVelocityLimit,
}

impl CollisionVelocityLimits {
    pub fn apply(
        self,
        mut velocity: crate::VelocityState,
    ) -> Result<crate::VelocityState, ResponseError> {
        if !self.maximum_linear.is_finite()
            || velocity.linear.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if self.maximum_linear <= 0.0 {
            return Err(ResponseError::NonPositiveLinearLimit);
        }
        velocity.angular = self.angular.apply(velocity.angular)?;
        let squared = f64::from(
            (velocity.linear[0] * velocity.linear[0] + velocity.linear[1] * velocity.linear[1])
                + velocity.linear[2] * velocity.linear[2],
        );
        let maximum = f64::from(self.maximum_linear);
        if squared > maximum * maximum {
            let scale = (maximum * f64::from(0.99_f32)) / squared.sqrt();
            velocity.linear = velocity
                .linear
                .map(|value| (f64::from(value) * scale) as f32);
        }
        Ok(velocity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionRequest {
    pub contact_distance: f32,
    pub contact_threshold: f32,
    pub rotations: [Option<CollisionRotation>; 2],
    pub inverse_timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionRotation {
    pub angular_velocity: [f32; 3],
    pub contact_radius: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrectionVelocity {
    Actual,
    ClosingOnly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionCorrection {
    pub endpoints: [Option<CollisionBody>; 2],
    pub normal: [f32; 3],
    pub required_speed: f32,
    pub velocity: CorrectionVelocity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorrectedCollision {
    pub endpoints: [Option<CollisionBody>; 2],
    pub impulse: f64,
    pub required_speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionMotionCommit {
    pub endpoints: [Option<CollisionBody>; 2],
    pub current: [crate::VelocityState; 2],
    pub effective_masses: [f64; 2],
    pub normal: [f32; 3],
    pub required_speed: f32,
    pub allow_delay: bool,
    pub collision_counts: [u16; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommittedCollisionMotion {
    pub active: [crate::VelocityState; 2],
    pub queued: [crate::QueuedVelocity; 2],
    pub applied: [bool; 2],
    pub collision_counts: [u16; 2],
}

impl CollisionMotionCommit {
    pub fn apply(self) -> Result<CommittedCollisionMotion, ResponseError> {
        if self.normal.iter().any(|value| !value.is_finite())
            || !self.required_speed.is_finite()
            || self.effective_masses.iter().any(|value| !value.is_finite())
            || self.current.iter().any(|value| {
                value
                    .linear
                    .iter()
                    .chain(value.angular.iter())
                    .any(|value| !value.is_finite())
            })
        {
            return Err(ResponseError::NonFinite);
        }
        if self.effective_masses.iter().any(|value| *value <= 0.0) {
            return Err(ResponseError::NonPositiveMass);
        }
        if self.required_speed < 0.0 {
            return Err(ResponseError::NegativeRequestSpeed);
        }
        if self.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection);
        }
        if self.endpoints.iter().all(Option::is_none) {
            return Err(ResponseError::NonPositiveMass);
        }
        for body in self.endpoints.into_iter().flatten() {
            body.validate()?;
        }
        let mut result = CommittedCollisionMotion {
            active: std::array::from_fn(|side| {
                self.endpoints[side].map_or(self.current[side], |body| crate::VelocityState {
                    linear: body.linear_velocity,
                    angular: body.angular_velocity,
                })
            }),
            queued: [crate::QueuedVelocity::default(); 2],
            applied: [true; 2],
            collision_counts: std::array::from_fn(|side| {
                self.collision_counts[side].wrapping_add(u16::from(self.endpoints[side].is_some()))
            }),
        };
        let delayed = usize::from(self.effective_masses[0] <= self.effective_masses[1]);
        if self.allow_delay
            && let Some(mut original) = self.endpoints[delayed]
        {
            original.linear_velocity = self.current[delayed].linear;
            original.angular_velocity = self.current[delayed].angular;
            let original_speed = original.point_velocity();
            let other_speed =
                self.endpoints[1 - delayed].map_or([0.0; 3], CollisionBody::point_velocity);
            let difference = std::array::from_fn(|axis| original_speed[axis] - other_speed[axis]);
            let closing = collision_dot(difference, self.normal);
            let closing = if delayed == 1 { -closing } else { closing };
            if closing < f64::from(self.required_speed * f32::from_bits(0xbf55_5555)) {
                let solved = result.active[delayed];
                result.active[delayed] = self.current[delayed];
                result.queued[delayed] = crate::QueuedVelocity {
                    linear: std::array::from_fn(|axis| {
                        solved.linear[axis] - self.current[delayed].linear[axis]
                    }),
                    angular: std::array::from_fn(|axis| {
                        solved.angular[axis] - self.current[delayed].angular[axis]
                    }),
                };
                result.applied[delayed] = false;
                result.collision_counts[delayed] = result.collision_counts[delayed].wrapping_sub(1);
            }
        }
        Ok(result)
    }
}

impl CollisionCorrection {
    pub fn from_request(
        endpoints: [Option<CollisionBody>; 2],
        normal: [f32; 3],
        request_speed: f32,
    ) -> Result<Self, ResponseError> {
        if !request_speed.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if request_speed < 0.0 {
            return Err(ResponseError::NegativeRequestSpeed);
        }
        let required_speed =
            (request_speed + f32::from_bits(0x3c4f_fe5a)) * f32::from_bits(0x3f99_999a);
        if !required_speed.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        Ok(Self {
            endpoints,
            normal,
            required_speed,
            velocity: CorrectionVelocity::Actual,
        })
    }
    pub fn apply(self) -> Result<CorrectedCollision, ResponseError> {
        if !self.required_speed.is_finite() || self.normal.iter().any(|value| !value.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        if self.required_speed < 0.0 {
            return Err(ResponseError::NegativeRequestSpeed);
        }
        if self.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection);
        }
        if self.endpoints.iter().all(Option::is_none) {
            return Err(ResponseError::NonPositiveMass);
        }
        for body in self.endpoints.into_iter().flatten() {
            body.validate()?;
        }
        let speeds = self
            .endpoints
            .map(|body| body.map_or([0.0; 3], CollisionBody::point_velocity));
        let relative = std::array::from_fn(|axis| speeds[1][axis] - speeds[0][axis]);
        let mut separating = -collision_dot(relative, self.normal);
        if self.velocity == CorrectionVelocity::ClosingOnly {
            separating = separating.min(0.0);
        }
        let missing = f64::from(self.required_speed) - separating;
        let mut result = CorrectedCollision {
            endpoints: self.endpoints,
            impulse: 0.0,
            required_speed: self.required_speed,
        };
        if missing < 0.0 {
            return Ok(result);
        }
        let opposite = self.normal.map(|value| -value);
        let second =
            self.endpoints[1].map_or(0.0, |body| collision_dot(body.response(opposite), opposite));
        let first = self.endpoints[0].map_or(0.0, |body| {
            collision_dot(body.response(self.normal), self.normal)
        });
        let response = (second + first) + f64::from(1.0e-15_f32);
        let impulse = missing / response;
        if !impulse.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if impulse < 0.0 {
            return Ok(result);
        }
        if let Some(body) = &mut result.endpoints[0] {
            body.apply(self.normal, impulse);
            body.validate()?;
        }
        if let Some(body) = &mut result.endpoints[1] {
            body.apply(opposite, impulse);
            body.validate()?;
        }
        result.impulse = impulse;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrictionImpulseLimit {
    pub normal_force: f32,
    pub friction: f32,
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionCone {
    pub normal: [f32; 3],
    pub cosine: f32,
    pub sine: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionImpulseStep {
    pub dynamic_response_mass: f64,
    pub opposing_response_mass: f64,
    pub approaching_speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionBody {
    pub orientation: [f64; 9],
    pub local_offset: [f32; 3],
    pub inverse_mass: f32,
    pub inverse_inertia: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionResponse {
    pub body: CollisionBody,
    pub opposing: Option<CollisionBody>,
    pub normal: [f32; 3],
    pub friction: f32,
    pub elasticity: f32,
    pub repetitions: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionPush {
    pub impulse: f64,
    pub direction: [f32; 3],
    pub relative_velocity: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub linear_delta: [f32; 3],
    pub angular_delta: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionResponseResult {
    pub body: CollisionBody,
    pub opposing: Option<CollisionBody>,
    pub pushes: Vec<CollisionPush>,
    pub effective_masses: [f64; 2],
    pub correction_velocity: CorrectionVelocity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImpactContactPoint {
    pub center: [f64; 3],
    pub orientation: [f64; 9],
    pub authored_offset: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoupledNormalSystem<'a> {
    pub matrix: &'a [f64],
    pub right_hand_side: &'a [f64],
    pub scale: f64,
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicEndpoint {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactNormalRow {
    pub normal: [f32; 3],
    pub angular_jacobian: [f32; 3],
    pub distance: f32,
    pub dynamic_endpoint: DynamicEndpoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalBody {
    pub angular_velocity: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub inverse_inertia: [f32; 3],
    pub inverse_mass: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalAssembly<'a> {
    pub rows: &'a [NormalContactRow],
    pub bodies: &'a [NormalBody],
    pub target_distance: f32,
    pub timestep: f32,
    pub maximum_dimension: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalEndpointRow {
    pub body: usize,
    pub angular_jacobian: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalContactRow {
    pub normal: [f32; 3],
    pub distance: f32,
    pub endpoints: [Option<NormalEndpointRow>; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssembledNormalSystem {
    pub matrix: Vec<f64>,
    pub right_hand_side: Vec<f64>,
    pub timestep: f32,
    pub inverse_responses: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawNormalSystem<'a> {
    pub matrix: &'a [f64],
    pub right_hand_side: &'a [f64],
    pub timestep: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedNormalSystem {
    pub matrix: Vec<f64>,
    pub right_hand_side: Vec<f64>,
    pub scale: f64,
    pub timestep: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoupledNormalSolution {
    pub active_rows: Vec<usize>,
    pub impulses: Vec<f64>,
    pub forces: Vec<f32>,
    pub history: Vec<i16>,
    pub method: crate::NormalSolveMethod,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrictionRedistributionOwner {
    pub first_core: u64,
    pub normal: [f32; 3],
    pub point: [f64; 3],
    pub frame: TangentFrame,
    pub coordinates: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedFrictionOwner {
    pub normal_force: f32,
    pub friction: f32,
    pub response_coefficient: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedFrictionClamp<'a> {
    pub owners: &'a [RetainedFrictionOwner],
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedFrictionClampResult {
    pub inverse_magnitude: Option<f32>,
    pub coordinates: [f32; 2],
    pub limit: f32,
    pub threshold_squared: f64,
    pub magnitude_squared: f64,
    pub scale: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetainedFrictionTransport {
    pub coordinates: [f32; 2],
    pub frame: TangentFrame,
    pub point_velocity: [f32; 3],
    pub elapsed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentImpulseSystem {
    pub inverse_response: [f64; 4],
    pub right_hand_side: [f64; 2],
    pub impulse_limit: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentImpulseSolution {
    pub unconstrained: [f32; 2],
    pub impulse: [f32; 2],
    pub magnitude_squared: f64,
    pub clamped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentBody {
    pub position: [f64; 3],
    pub orientation: [f64; 9],
    pub angular_velocity: [f32; 3],
    pub linear_velocity: [f32; 3],
    pub inverse_inertia: [f32; 3],
    pub inverse_mass: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentAssembly {
    pub bodies: [Option<TangentBody>; 2],
    pub point: [f64; 3],
    pub frame: TangentFrame,
    pub retained: [f32; 2],
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssembledTangentSystem {
    pub bodies: [Option<TangentBody>; 2],
    pub frame: TangentFrame,
    pub response: [f64; 4],
    pub inverse_response: [f64; 4],
    pub angular_response: [[[f32; 3]; 2]; 2],
    pub current_velocity: [f32; 2],
    pub right_hand_side: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentFrame {
    pub first: [f32; 3],
    pub second: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseError {
    SingularFactorization,
    NonFinite,
    NonPositiveMass,
    NonPositiveTimestep,
    NonPositiveAngularLimit,
    NonPositiveLinearLimit,
    NegativeContactRadius,
    NegativeNormalForce,
    NegativeFriction,
    NegativeResponseCoefficient,
    NegativeImpulseLimit,
    NegativeRequestSpeed,
    NegativeElapsed,
    InvalidSystemShape,
    ZeroDirection,
    Topology(TopologyError),
    Geometry(crate::FeatureWalkError),
    Energy(crate::EnergyError),
}

impl fmt::Display for ResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingularFactorization => formatter.write_str("contact factorization is singular"),
            Self::NonFinite => formatter.write_str("contact response contains a non-finite value"),
            Self::NonPositiveMass => formatter.write_str("contact effective mass must be positive"),
            Self::NonPositiveTimestep => formatter.write_str("contact timestep must be positive"),
            Self::NonPositiveAngularLimit => {
                formatter.write_str("angular velocity limit must be positive")
            }
            Self::NonPositiveLinearLimit => {
                formatter.write_str("collision linear speed limit must be positive")
            }
            Self::NegativeContactRadius => {
                formatter.write_str("collision contact radius cannot be negative")
            }
            Self::NegativeNormalForce => {
                formatter.write_str("contact normal force cannot be negative")
            }
            Self::NegativeFriction => formatter.write_str("contact friction cannot be negative"),
            Self::NegativeResponseCoefficient => {
                formatter.write_str("contact response coefficient cannot be negative")
            }
            Self::NegativeImpulseLimit => {
                formatter.write_str("contact impulse limit cannot be negative")
            }
            Self::NegativeRequestSpeed => {
                formatter.write_str("collision request speed cannot be negative")
            }
            Self::NegativeElapsed => {
                formatter.write_str("retained friction elapsed time cannot be negative")
            }
            Self::InvalidSystemShape => {
                formatter.write_str("contact response matrix is not square")
            }
            Self::ZeroDirection => formatter.write_str("contact tangent direction must be nonzero"),
            Self::Topology(error) => error.fmt(formatter),
            Self::Geometry(error) => error.fmt(formatter),
            Self::Energy(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResponseError {}

impl From<TopologyError> for ResponseError {
    fn from(error: TopologyError) -> Self {
        Self::Topology(error)
    }
}

impl From<crate::FeatureWalkError> for ResponseError {
    fn from(error: crate::FeatureWalkError) -> Self {
        Self::Geometry(error)
    }
}

impl From<crate::EnergyError> for ResponseError {
    fn from(error: crate::EnergyError) -> Self {
        Self::Energy(error)
    }
}

impl NormalContact {
    pub fn impulse(self) -> Result<f64, ResponseError> {
        if !self.target_distance.is_finite()
            || !self.distance.is_finite()
            || !self.relative_speed.is_finite()
            || !self.effective_mass.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.effective_mass <= 0.0 {
            return Err(ResponseError::NonPositiveMass);
        }
        let distance = self.target_distance - self.distance;
        let stiffness = if distance < 0.0 { 20.0 } else { 1.0 };
        Ok(((self.relative_speed + f64::from(distance) * stiffness)
            * f64::from(self.effective_mass))
        .max(0.0))
    }
}

impl AngularVelocityLimit {
    pub fn apply(self, velocity: [f32; 3]) -> Result<[f32; 3], ResponseError> {
        if !self.maximum_per_step.is_finite()
            || !self.timestep.is_finite()
            || velocity.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        if self.maximum_per_step <= 0.0 {
            return Err(ResponseError::NonPositiveAngularLimit);
        }
        let length = f64::from(
            velocity
                .iter()
                .map(|component| component * component)
                .sum::<f32>(),
        )
        .sqrt();
        let inverse_timestep = (1.0 / f64::from(self.timestep)) as f32;
        let maximum = f64::from(self.maximum_per_step * inverse_timestep);
        if length <= maximum {
            return Ok(velocity);
        }
        let scale = (maximum * f64::from(0.9_f32)) / length;
        Ok(velocity.map(|component| (f64::from(component) * scale) as f32))
    }
}

impl CollisionRequest {
    pub fn speed(self) -> Result<f32, ResponseError> {
        if !self.contact_distance.is_finite()
            || !self.contact_threshold.is_finite()
            || !self.inverse_timestep.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.inverse_timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        let inverse_timestep = self.inverse_timestep;
        let mut uncertainty = 0.0_f32;
        for rotation in self.rotations.into_iter().flatten() {
            if rotation
                .angular_velocity
                .iter()
                .any(|value| !value.is_finite())
                || !rotation.contact_radius.is_finite()
            {
                return Err(ResponseError::NonFinite);
            }
            if rotation.contact_radius < 0.0 {
                return Err(ResponseError::NegativeContactRadius);
            }
            let squared = (rotation.angular_velocity[0] * rotation.angular_velocity[0]
                + rotation.angular_velocity[1] * rotation.angular_velocity[1])
                + rotation.angular_velocity[2] * rotation.angular_velocity[2];
            let angle = (f64::from(squared) * 0.000_025).min(0.25).sqrt();
            let contribution = (1.0 - angle.cos())
                * f64::from(rotation.contact_radius)
                * f64::from(inverse_timestep);
            uncertainty = (f64::from(uncertainty) + contribution) as f32;
        }
        let angular_speed = uncertainty + uncertainty;
        let distance_speed = if self.contact_distance < self.contact_threshold {
            (f64::from(self.contact_threshold) - f64::from(self.contact_distance))
                * (f64::from(inverse_timestep) + f64::from(inverse_timestep))
        } else {
            0.0
        };
        Ok((f64::from(angular_speed) + distance_speed) as f32)
    }
}

impl FrictionImpulseLimit {
    pub fn impulse(self) -> Result<f64, ResponseError> {
        if !self.normal_force.is_finite()
            || !self.friction.is_finite()
            || !self.timestep.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.normal_force < 0.0 {
            return Err(ResponseError::NegativeNormalForce);
        }
        if self.friction < 0.0 {
            return Err(ResponseError::NegativeFriction);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        Ok(f64::from(
            (self.normal_force * self.friction) * self.timestep,
        ))
    }
}

impl CollisionCone {
    pub fn direction(self, relative_velocity: [f32; 3]) -> Result<[f32; 3], ResponseError> {
        if self.normal.iter().any(|value| !value.is_finite())
            || relative_velocity.iter().any(|value| !value.is_finite())
            || !self.cosine.is_finite()
            || !self.sine.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection);
        }

        let normalize = |value: [f32; 3]| {
            let squared = (value[0] * value[0] + value[1] * value[1]) + value[2] * value[2];
            if squared < 1.0e-19_f32 {
                return None;
            }
            let inverse = reciprocal_square_root_f64(f64::from(squared));
            Some(value.map(|component| (f64::from(component) * inverse) as f32))
        };
        let Some(relative_unit) = normalize(relative_velocity) else {
            return Ok(self.normal);
        };
        let incoming = relative_unit.map(|value| -value);
        let alignment = incoming
            .iter()
            .zip(self.normal)
            .map(|(left, right)| left * right)
            .sum::<f32>();
        if alignment >= self.cosine {
            return Ok(incoming);
        }

        let projection = relative_unit
            .iter()
            .zip(self.normal)
            .map(|(left, right)| left * right)
            .sum::<f32>();
        let tangent = std::array::from_fn(|axis| {
            (f64::from(relative_unit[axis]) - f64::from(self.normal[axis]) * f64::from(projection))
                as f32
        });
        let Some(tangent) = normalize(tangent) else {
            return Ok(self.normal);
        };
        Ok(std::array::from_fn(|axis| {
            let tangent_component = (f64::from(tangent[axis]) * f64::from(self.sine)) as f32;
            let component = (f64::from(self.normal[axis]) * f64::from(self.cosine)
                - f64::from(tangent_component)) as f32;
            if component == 0.0 { -0.0 } else { component }
        }))
    }
}

impl CollisionImpulseStep {
    pub fn impulse(self) -> Result<f64, ResponseError> {
        if !self.dynamic_response_mass.is_finite()
            || !self.opposing_response_mass.is_finite()
            || !self.approaching_speed.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.dynamic_response_mass <= 0.0 || self.opposing_response_mass <= 0.0 {
            return Err(ResponseError::NonPositiveMass);
        }
        if self.approaching_speed <= 0.0 {
            return Ok(0.0);
        }
        let combined_mass = self.dynamic_response_mass + self.opposing_response_mass;
        if !combined_mass.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        Ok(
            (f64::from(0.1_f32) / combined_mass * self.opposing_response_mass)
                * ((self.dynamic_response_mass + self.dynamic_response_mass)
                    * self.approaching_speed),
        )
    }
}

impl CollisionBody {
    fn validate(self) -> Result<(), ResponseError> {
        if self.orientation.iter().any(|value| !value.is_finite())
            || self
                .local_offset
                .iter()
                .chain(self.inverse_inertia.iter())
                .chain(self.linear_velocity.iter())
                .chain(self.angular_velocity.iter())
                .any(|value| !value.is_finite())
            || !self.inverse_mass.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.inverse_mass < 0.0
            || self.inverse_inertia.iter().any(|value| *value < 0.0)
            || (self.inverse_mass == 0.0 && self.inverse_inertia.iter().any(|v| *v != 0.0))
        {
            return Err(ResponseError::NonPositiveMass);
        }
        Ok(())
    }

    pub fn point_velocity(self) -> [f32; 3] {
        let angular_local = collision_cross(self.angular_velocity, self.local_offset);
        let angular_world = collision_rotate(self.orientation, angular_local.map(f64::from))
            .map(|component| component as f32);
        std::array::from_fn(|axis| self.linear_velocity[axis] + angular_world[axis])
    }

    fn response(self, direction: [f32; 3]) -> [f32; 3] {
        let local: [f32; 3] = std::array::from_fn(|axis| {
            (self.orientation[axis] * f64::from(direction[0])
                + self.orientation[3 + axis] * f64::from(direction[1])
                + self.orientation[6 + axis] * f64::from(direction[2])) as f32
        });
        let torque = collision_cross(self.local_offset, local);
        let angular = std::array::from_fn(|axis| torque[axis] * self.inverse_inertia[axis]);
        let angular_point = collision_cross(angular, self.local_offset);
        let angular_world = collision_rotate(self.orientation, angular_point.map(f64::from))
            .map(|component| component as f32);
        std::array::from_fn(|axis| {
            let linear = (f64::from(direction[axis]) * f64::from(self.inverse_mass)) as f32;
            (f64::from(linear) + f64::from(angular_world[axis])) as f32
        })
    }

    fn apply(&mut self, direction: [f32; 3], impulse: f64) -> ([f32; 3], [f32; 3]) {
        let world = direction.map(|component| (f64::from(component) * impulse) as f32);
        let local: [f32; 3] = std::array::from_fn(|axis| {
            (self.orientation[3 + axis] * f64::from(world[1])
                + self.orientation[axis] * f64::from(world[0])
                + self.orientation[6 + axis] * f64::from(world[2])) as f32
        });
        let torque = collision_cross(self.local_offset, local);
        let angular = std::array::from_fn(|axis| torque[axis] * self.inverse_inertia[axis]);
        let linear =
            world.map(|component| (f64::from(component) * f64::from(self.inverse_mass)) as f32);
        self.linear_velocity =
            std::array::from_fn(|axis| self.linear_velocity[axis] + linear[axis]);
        self.angular_velocity =
            std::array::from_fn(|axis| self.angular_velocity[axis] + angular[axis]);
        (linear, angular)
    }
}

impl CollisionResponse {
    pub fn solve(self) -> Result<CollisionResponseResult, ResponseError> {
        self.body.validate()?;
        if let Some(opposing) = self.opposing {
            opposing.validate()?;
        }
        if self.normal.iter().any(|value| !value.is_finite())
            || !self.friction.is_finite()
            || !self.elasticity.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.friction < 0.0 || self.elasticity < 0.0 {
            return Err(ResponseError::NegativeFriction);
        }
        if self.repetitions > i16::MAX as u16 {
            return Err(ResponseError::InvalidSystemShape);
        }
        if self.normal == [0.0; 3] {
            return Err(ResponseError::ZeroDirection);
        }
        let mut body = self.body;
        let mut opposing = self.opposing;
        let mut pushes = Vec::new();
        let relative = collision_relative(body, opposing);
        let approaching = -collision_dot(relative, self.normal);
        let body_response = body.response(self.normal);
        let body_mass = 1.0 / collision_length(body_response);
        let opposing_mass = opposing
            .map(|other| {
                if other.inverse_mass == 0.0 {
                    1.0
                } else {
                    1.0 / collision_length(other.response(self.normal.map(|value| -value)))
                }
            })
            .unwrap_or(body_mass * 100_000.0);
        let effective_masses = [body_mass, opposing_mass];
        if approaching < f64::from(1.0e-4_f32) {
            return Ok(CollisionResponseResult {
                body,
                opposing,
                pushes,
                effective_masses,
                correction_velocity: CorrectionVelocity::ClosingOnly,
            });
        }

        let restitution = f64::from(self.elasticity).sqrt();
        let tangent = f64::from(self.friction) * (1.0 + restitution);
        let angle = tangent.atan() as f32;
        let square = angle * angle;
        let cosine = f64::from(1.0 - 0.5 * square + (1.0 / 24.0) * square * square);
        let sine = f64::from((cosine * tangent) as f32);
        let cone = CollisionCone {
            normal: self.normal,
            cosine: cosine as f32,
            sine: sine as f32,
        };
        let increment = CollisionImpulseStep {
            dynamic_response_mass: body_mass,
            opposing_response_mass: opposing_mass,
            approaching_speed: approaching,
        }
        .impulse()?;
        let denominator = 1.0_f32 + f32::from(self.repetitions) * 0.5_f32;
        let effective = 1.0_f32 - (1.0_f32 - self.elasticity) / denominator;
        let desired = f64::from(self.repetitions).sqrt() * f64::from(0.01_f32)
            + approaching * f64::from(effective).sqrt();

        for iteration in 0..100 {
            let relative = collision_relative(body, opposing);
            let direction = cone.direction(relative)?;
            let (linear_delta, angular_delta) = body.apply(direction, increment);
            if let Some(other) = &mut opposing {
                other.apply(direction.map(|value| -value), increment);
            }
            pushes.push(CollisionPush {
                impulse: increment,
                direction,
                relative_velocity: relative,
                linear_velocity: body.linear_velocity,
                angular_velocity: body.angular_velocity,
                linear_delta,
                angular_delta,
            });

            let separating = collision_dot(collision_relative(body, opposing), self.normal);
            if separating <= 0.0 {
                continue;
            }
            if iteration == 99 {
                break;
            }
            if separating < desired {
                let initial_relative = collision_relative(body, opposing);
                let (unit_linear, unit_angular) = body.apply(self.normal, 1.0);
                pushes.push(CollisionPush {
                    impulse: 1.0,
                    direction: self.normal,
                    relative_velocity: initial_relative,
                    linear_velocity: body.linear_velocity,
                    angular_velocity: body.angular_velocity,
                    linear_delta: unit_linear,
                    angular_delta: unit_angular,
                });
                let mut restored = body;
                restored.linear_velocity =
                    std::array::from_fn(|axis| body.linear_velocity[axis] - unit_linear[axis]);
                restored.angular_velocity =
                    std::array::from_fn(|axis| body.angular_velocity[axis] - unit_angular[axis]);
                let mut restored_opposing = opposing;
                if let Some(other) = &mut opposing {
                    let (linear, angular) = other.apply(self.normal.map(|value| -value), 1.0);
                    let mut reset = *other;
                    reset.linear_velocity =
                        std::array::from_fn(|axis| other.linear_velocity[axis] - linear[axis]);
                    reset.angular_velocity =
                        std::array::from_fn(|axis| other.angular_velocity[axis] - angular[axis]);
                    restored_opposing = Some(reset);
                }
                let unit_relative = collision_relative(body, opposing);
                let response = collision_dot(unit_relative, self.normal) - separating;
                if !response.is_finite() {
                    return Err(ResponseError::NonFinite);
                }
                let magnitude = if response.abs() > f64::from(1.0e-4_f32) {
                    (desired - separating) / response
                } else {
                    0.0
                };
                body = restored;
                opposing = restored_opposing;
                let (linear_delta, angular_delta) = body.apply(self.normal, magnitude);
                if let Some(other) = &mut opposing {
                    other.apply(self.normal.map(|value| -value), magnitude);
                }
                pushes.push(CollisionPush {
                    impulse: magnitude,
                    direction: self.normal,
                    relative_velocity: unit_relative,
                    linear_velocity: body.linear_velocity,
                    angular_velocity: body.angular_velocity,
                    linear_delta,
                    angular_delta,
                });
            }
            break;
        }

        Ok(CollisionResponseResult {
            body,
            opposing,
            pushes,
            effective_masses,
            correction_velocity: CorrectionVelocity::Actual,
        })
    }
}

fn collision_cross(first: [f32; 3], second: [f32; 3]) -> [f32; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

fn collision_rotate(matrix: [f64; 9], value: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| {
        matrix[axis * 3 + 1] * value[1]
            + matrix[axis * 3] * value[0]
            + matrix[axis * 3 + 2] * value[2]
    })
}

fn collision_relative(body: CollisionBody, opposing: Option<CollisionBody>) -> [f32; 3] {
    let current = body.point_velocity();
    let other = opposing
        .map(CollisionBody::point_velocity)
        .unwrap_or([0.0; 3]);
    std::array::from_fn(|axis| (f64::from(current[axis]) - f64::from(other[axis])) as f32)
}

fn collision_dot(first: [f32; 3], second: [f32; 3]) -> f64 {
    f64::from((first[0] * second[0] + first[1] * second[1]) + first[2] * second[2])
}

fn collision_length(value: [f32; 3]) -> f64 {
    f64::from((value[0] * value[0] + value[1] * value[1]) + value[2] * value[2]).sqrt()
}

impl ImpactContactPoint {
    pub fn local_offset(self) -> Result<[f32; 3], ResponseError> {
        if self
            .center
            .iter()
            .chain(self.orientation.iter())
            .any(|value| !value.is_finite())
            || self.authored_offset.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let local = self.authored_offset.map(f64::from);
        let world: [f64; 3] = std::array::from_fn(|axis| {
            (self.orientation[axis * 3] * local[0] + self.orientation[axis * 3 + 1] * local[1])
                + self.orientation[axis * 3 + 2] * local[2]
                + self.center[axis]
        });
        Self::from_world(self.center, self.orientation, world)
    }

    pub fn from_world(
        center: [f64; 3],
        orientation: [f64; 9],
        point: [f64; 3],
    ) -> Result<[f32; 3], ResponseError> {
        if center
            .iter()
            .chain(orientation.iter())
            .chain(point.iter())
            .any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let offset: [f64; 3] = std::array::from_fn(|axis| point[axis] - center[axis]);
        let local = std::array::from_fn(|axis| {
            ((orientation[axis] * offset[0] + orientation[3 + axis] * offset[1])
                + orientation[6 + axis] * offset[2]) as f32
        });
        if local.iter().any(|value| !value.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        Ok(local)
    }
}

impl ContactNormalRow {
    pub fn from_local(
        normal: [f32; 3],
        local_offset: [f32; 3],
        orientation: [f64; 9],
        distance: f32,
        dynamic_endpoint: DynamicEndpoint,
    ) -> Result<Self, ResponseError> {
        if !distance.is_finite()
            || normal.iter().any(|value| !value.is_finite())
            || local_offset.iter().any(|value| !value.is_finite())
            || orientation.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let local_normal: [f32; 3] = std::array::from_fn(|axis| {
            (orientation[3 + axis] * f64::from(normal[1])
                + orientation[axis] * f64::from(normal[0])
                + orientation[6 + axis] * f64::from(normal[2])) as f32
        });
        let angular_jacobian = [
            local_offset[1] * local_normal[2] - local_offset[2] * local_normal[1],
            local_offset[2] * local_normal[0] - local_offset[0] * local_normal[2],
            local_offset[0] * local_normal[1] - local_offset[1] * local_normal[0],
        ];
        Ok(Self {
            normal,
            angular_jacobian,
            distance,
            dynamic_endpoint,
        })
    }

    pub fn apply_impulse(
        self,
        mut body: NormalBody,
        impulse: f64,
    ) -> Result<NormalBody, ResponseError> {
        if !impulse.is_finite()
            || !body.inverse_mass.is_finite()
            || self
                .normal
                .iter()
                .chain(self.angular_jacobian.iter())
                .chain(body.angular_velocity.iter())
                .chain(body.linear_velocity.iter())
                .chain(body.inverse_inertia.iter())
                .any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if body.inverse_mass < 0.0 || body.inverse_inertia.iter().any(|value| *value < 0.0) {
            return Err(ResponseError::NonPositiveMass);
        }
        let signed_impulse = if self.dynamic_endpoint == DynamicEndpoint::First {
            -impulse
        } else {
            impulse
        };
        let linear_scale = signed_impulse * f64::from(body.inverse_mass);
        for (axis, (angular, linear)) in body
            .angular_velocity
            .iter_mut()
            .zip(body.linear_velocity.iter_mut())
            .enumerate()
        {
            let response = self.angular_jacobian[axis] * body.inverse_inertia[axis];
            *angular = (f64::from(*angular) + f64::from(response) * signed_impulse) as f32;
            *linear = (f64::from(*linear) + f64::from(self.normal[axis]) * linear_scale) as f32;
        }
        Ok(body)
    }
}

pub(crate) fn ordered_dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    (left[1] * right[1] + left[0] * right[0]) + left[2] * right[2]
}

impl AssembledNormalSystem {
    pub fn prepare(&self) -> Result<PreparedNormalSystem, ResponseError> {
        RawNormalSystem {
            matrix: &self.matrix,
            right_hand_side: &self.right_hand_side,
            timestep: self.timestep,
        }
        .prepare()
    }
}

impl RawNormalSystem<'_> {
    pub fn prepare(self) -> Result<PreparedNormalSystem, ResponseError> {
        let rows = self.right_hand_side.len();
        if rows.checked_mul(rows) != Some(self.matrix.len()) {
            return Err(ResponseError::InvalidSystemShape);
        }
        if !self.timestep.is_finite()
            || self.matrix.iter().any(|value| !value.is_finite())
            || self.right_hand_side.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        let maximum_diagonal = (0..rows)
            .map(|row| self.matrix[row * rows + row])
            .fold(0.0_f64, f64::max);
        let maximum_request = self
            .right_hand_side
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max);
        let inverse_diagonal = if maximum_diagonal > 1.0e-19 {
            1.0 / maximum_diagonal
        } else {
            1.0
        };
        let (inverse_request, maximum_request) = if maximum_request > 1.0e-19 {
            (1.0 / maximum_request, maximum_request)
        } else {
            (1.0, 1.0)
        };
        Ok(PreparedNormalSystem {
            matrix: self
                .matrix
                .iter()
                .map(|value| value * inverse_diagonal)
                .collect(),
            right_hand_side: self
                .right_hand_side
                .iter()
                .map(|value| value * inverse_request)
                .collect(),
            scale: inverse_diagonal * maximum_request,
            timestep: self.timestep,
        })
    }
}

impl PreparedNormalSystem {
    pub fn solve(
        &self,
        policy: crate::NormalSolvePolicy<'_>,
    ) -> Result<Option<CoupledNormalSolution>, ResponseError> {
        CoupledNormalSystem {
            matrix: &self.matrix,
            right_hand_side: &self.right_hand_side,
            scale: self.scale,
            timestep: self.timestep,
        }
        .solve(policy)
    }
}

impl CoupledNormalSystem<'_> {
    pub fn solve(
        self,
        policy: crate::NormalSolvePolicy<'_>,
    ) -> Result<Option<CoupledNormalSolution>, ResponseError> {
        crate::normal_solver::solve(self, policy)
    }
}

pub fn redistribute_retained_friction(
    owners: &mut [FrictionRedistributionOwner],
) -> Result<(), ResponseError> {
    for owner in owners.iter() {
        if owner.point.iter().any(|value| !value.is_finite())
            || owner.normal.iter().any(|value| !value.is_finite())
            || owner.frame.first.iter().any(|value| !value.is_finite())
            || owner.frame.second.iter().any(|value| !value.is_finite())
            || owner.coordinates.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
    }
    if owners.len() < 2 {
        return Ok(());
    }
    let count = owners.len();
    let inverse_count = 1.0 / count as f64;
    let mut corrections = vec![[0.0_f32; 3]; count];
    for first in 0..count - 1 {
        for second in first + 1..count {
            let first_normal = owners[first].normal;
            let second_normal = owners[second].normal;
            let alignment = (first_normal[1] * second_normal[1]
                + first_normal[0] * second_normal[0])
                + first_normal[2] * second_normal[2];
            if (f64::from(alignment.abs()) - 1.0).abs() >= 0.001_000_000_047_497_451_3 {
                continue;
            }
            let mut direction: [f32; 3] = std::array::from_fn(|axis| {
                (owners[first].point[axis] - owners[second].point[axis]) as f32
            });
            let squared = (direction[0] * direction[0] + direction[1] * direction[1])
                + direction[2] * direction[2];
            if squared < 1.0e-19_f32 {
                continue;
            }
            let inverse_length = reciprocal_square_root_f64(f64::from(squared));
            direction = direction.map(|value| (f64::from(value) * inverse_length) as f32);
            let world = |owner: &FrictionRedistributionOwner| {
                std::array::from_fn(|axis| {
                    let first = (f64::from(owner.frame.first[axis])
                        * f64::from(owner.coordinates[0])) as f32;
                    (f64::from(first)
                        + f64::from(owner.frame.second[axis]) * f64::from(owner.coordinates[1]))
                        as f32
                })
            };
            let project = |value: [f32; 3]| {
                let scalar =
                    (direction[1] * value[1] + direction[0] * value[0]) + direction[2] * value[2];
                direction.map(|component| component * scalar)
            };
            let reverse = if owners[first].first_core == owners[second].first_core {
                1.0_f32
            } else {
                -1.0_f32
            };
            let first_projected = project(world(&owners[first])).map(|value| value * reverse);
            let second_projected = project(world(&owners[second]));
            for axis in 0..3 {
                let average = (second_projected[axis] + first_projected[axis]) * 0.5;
                corrections[first][axis] += (f64::from(average - first_projected[axis])
                    * (inverse_count * f64::from(reverse)))
                    as f32;
                corrections[second][axis] +=
                    (f64::from(average - second_projected[axis]) * inverse_count) as f32;
            }
        }
    }
    for (owner, correction) in owners.iter().zip(&mut corrections) {
        let update = |direction: [f32; 3], prior: f32| {
            (direction[1] * correction[1] + direction[0] * correction[0])
                + (direction[2] * correction[2] + prior)
        };
        let next = [
            update(owner.frame.first, owner.coordinates[0]),
            update(owner.frame.second, owner.coordinates[1]),
        ];
        if next.iter().any(|value| !value.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        correction[0] = next[0];
        correction[1] = next[1];
    }
    for (owner, correction) in owners.iter_mut().zip(corrections) {
        owner.coordinates = [correction[0], correction[1]];
    }
    Ok(())
}

impl RetainedFrictionClamp<'_> {
    pub fn apply(
        self,
        coordinates: [f32; 2],
    ) -> Result<RetainedFrictionClampResult, ResponseError> {
        clamp_retained_friction(coordinates, self.limit()?)
    }
    pub fn limit(self) -> Result<f32, ResponseError> {
        if !self.timestep.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        let mut combined = 0.0_f32;
        for owner in self.owners.iter().rev() {
            if !owner.normal_force.is_finite()
                || !owner.friction.is_finite()
                || !owner.response_coefficient.is_finite()
            {
                return Err(ResponseError::NonFinite);
            }
            if owner.normal_force < 0.0 {
                return Err(ResponseError::NegativeNormalForce);
            }
            if owner.friction < 0.0 {
                return Err(ResponseError::NegativeFriction);
            }
            if owner.response_coefficient < 0.0 {
                return Err(ResponseError::NegativeResponseCoefficient);
            }
            combined += (owner.normal_force * owner.friction) * owner.response_coefficient;
        }
        let limit = (combined * self.timestep) * self.timestep;
        if !limit.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        Ok(limit)
    }
}

pub(crate) fn clamp_retained_friction(
    coordinates: [f32; 2],
    limit: f32,
) -> Result<RetainedFrictionClampResult, ResponseError> {
    if !limit.is_finite() || coordinates.iter().any(|v| !v.is_finite()) {
        return Err(ResponseError::NonFinite);
    }
    if limit < 0.0 {
        return Err(ResponseError::NegativeFriction);
    }
    let threshold_squared = f64::from(limit * limit + 1.0e-6_f32);
    let magnitude_squared =
        f64::from(coordinates[0] * coordinates[0] + coordinates[1] * coordinates[1]);
    let inverse_magnitude = (magnitude_squared > threshold_squared)
        .then(|| reciprocal_square_root_f64(f64::from(magnitude_squared as f32)) as f32);
    let scale = inverse_magnitude.map(|inverse| f64::from(limit) * f64::from(inverse));
    Ok(RetainedFrictionClampResult {
        inverse_magnitude,
        coordinates: scale.map_or(coordinates, |scale| {
            coordinates.map(|value| (f64::from(value) * scale) as f32)
        }),
        limit,
        threshold_squared,
        magnitude_squared,
        scale,
    })
}

impl RetainedFrictionTransport {
    pub fn advance(self) -> Result<[f32; 2], ResponseError> {
        if self.coordinates.iter().any(|value| !value.is_finite())
            || self.frame.first.iter().any(|value| !value.is_finite())
            || self.frame.second.iter().any(|value| !value.is_finite())
            || self.point_velocity.iter().any(|value| !value.is_finite())
            || !self.elapsed.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.elapsed < 0.0 {
            return Err(ResponseError::NegativeElapsed);
        }
        let projected = |direction: [f32; 3]| {
            (direction[1] * self.point_velocity[1] + direction[0] * self.point_velocity[0])
                + direction[2] * self.point_velocity[2]
        };
        Ok([
            (f64::from(self.coordinates[0]) - f64::from(projected(self.frame.first)) * self.elapsed)
                as f32,
            (f64::from(self.coordinates[1])
                - f64::from(projected(self.frame.second)) * self.elapsed) as f32,
        ])
    }
}

impl TangentAssembly {
    pub fn assemble(self) -> Result<AssembledTangentSystem, ResponseError> {
        if self
            .frame
            .first
            .iter()
            .chain(self.frame.second.iter())
            .chain(self.retained.iter())
            .any(|value| !value.is_finite())
            || self.point.iter().any(|value| !value.is_finite())
            || !self.timestep.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.timestep <= 0.0 {
            return Err(ResponseError::NonPositiveTimestep);
        }
        let directions = [self.frame.first, self.frame.second];
        let mut angular_responses = [[[0.0_f32; 3]; 2]; 2];
        let mut current_velocity = [0.0_f32; 2];
        let mut response = [0.0_f64; 4];
        for (side, body) in self.bodies.into_iter().enumerate() {
            let Some(body) = body else {
                continue;
            };
            if body
                .position
                .iter()
                .chain(body.orientation.iter())
                .any(|value| !value.is_finite())
                || body
                    .angular_velocity
                    .iter()
                    .chain(body.linear_velocity.iter())
                    .chain(body.inverse_inertia.iter())
                    .any(|value| !value.is_finite())
                || !body.inverse_mass.is_finite()
            {
                return Err(ResponseError::NonFinite);
            }
            if body.inverse_mass < 0.0 || body.inverse_inertia.iter().any(|value| *value < 0.0) {
                return Err(ResponseError::NonPositiveMass);
            }
            let lever = std::array::from_fn::<_, 3, _>(|axis| {
                (self.point[axis] - body.position[axis]) as f32
            });
            let mut torque = [[0.0_f32; 3]; 2];
            let angular_response = &mut angular_responses[side];
            for (index, direction) in directions.into_iter().enumerate() {
                let world_torque = [
                    direction[2] * lever[1] - direction[1] * lever[2],
                    direction[0] * lever[2] - direction[2] * lever[0],
                    direction[1] * lever[0] - direction[0] * lever[1],
                ];
                for axis in 0..3 {
                    torque[index][axis] = (f64::from(world_torque[0]) * body.orientation[axis]
                        + f64::from(world_torque[1]) * body.orientation[3 + axis]
                        + f64::from(world_torque[2]) * body.orientation[6 + axis])
                        as f32;
                    angular_response[index][axis] =
                        torque[index][axis] * body.inverse_inertia[axis];
                }
                let angular = (body.angular_velocity[0] * torque[index][0]
                    + body.angular_velocity[1] * torque[index][1])
                    + body.angular_velocity[2] * torque[index][2];
                let linear = (body.linear_velocity[0] * direction[0]
                    + body.linear_velocity[1] * direction[1])
                    + body.linear_velocity[2] * direction[2];
                current_velocity[index] += (angular + linear) * if side == 0 { 1.0 } else { -1.0 };
            }
            let diagonal = |index: usize| {
                (angular_response[index][0] * torque[index][0]
                    + angular_response[index][1] * torque[index][1])
                    + (body.inverse_mass + angular_response[index][2] * torque[index][2])
            };
            let cross = (angular_response[1][1] * torque[0][1]
                + angular_response[1][0] * torque[0][0])
                + angular_response[1][2] * torque[0][2];
            let contribution = [
                f64::from(diagonal(0)),
                f64::from(cross),
                f64::from(cross),
                f64::from(diagonal(1)),
            ];
            for (value, term) in response.iter_mut().zip(contribution) {
                *value += term;
            }
        }
        let determinant = response[0] * response[3] - response[1] * response[2];
        if response.iter().any(|value| !value.is_finite())
            || !determinant.is_finite()
            || current_velocity.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if determinant == 0.0 {
            return Err(ResponseError::NonPositiveMass);
        }
        let inverse_determinant = 1.0 / determinant;
        let inverse_response = [
            response[3] * inverse_determinant,
            -(response[1] * inverse_determinant),
            -(response[2] * inverse_determinant),
            response[0] * inverse_determinant,
        ];
        let inverse_timestep = 1.0_f32 / self.timestep;
        let right_hand_side = [
            f64::from(inverse_timestep * self.retained[0] - current_velocity[0]),
            f64::from(inverse_timestep * self.retained[1] - current_velocity[1]),
        ];
        Ok(AssembledTangentSystem {
            bodies: self.bodies,
            frame: self.frame,
            response,
            inverse_response,
            angular_response: angular_responses,
            current_velocity,
            right_hand_side,
        })
    }
}

impl AssembledTangentSystem {
    pub fn apply(self, impulse: [f32; 2]) -> Result<[Option<TangentBody>; 2], ResponseError> {
        if impulse.iter().any(|value| !value.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        let directions = [self.frame.first, self.frame.second];
        let mut bodies = self.bodies;
        for (side, body) in bodies.iter_mut().enumerate() {
            let Some(body) = body else {
                continue;
            };
            let mass_scales = impulse.map(|coefficient| {
                let scale = coefficient * body.inverse_mass;
                if side == 0 { scale } else { -scale }
            });
            let impulse = impulse.map(|value| if side == 0 { value } else { -value });
            for (axis, (angular_velocity, linear_velocity)) in body
                .angular_velocity
                .iter_mut()
                .zip(body.linear_velocity.iter_mut())
                .enumerate()
            {
                let first_angular = (f64::from(self.angular_response[side][0][axis])
                    * f64::from(impulse[0])) as f32;
                let angular_delta = (f64::from(first_angular)
                    + f64::from(self.angular_response[side][1][axis]) * f64::from(impulse[1]))
                    as f32;
                let first_linear =
                    (f64::from(directions[0][axis]) * f64::from(mass_scales[0])) as f32;
                let linear_delta = (f64::from(first_linear)
                    + f64::from(directions[1][axis]) * f64::from(mass_scales[1]))
                    as f32;
                *angular_velocity += angular_delta;
                *linear_velocity += linear_delta;
            }
            if body
                .angular_velocity
                .iter()
                .chain(body.linear_velocity.iter())
                .any(|value| !value.is_finite())
            {
                return Err(ResponseError::NonFinite);
            }
        }
        Ok(bodies)
    }
}

impl TangentImpulseSystem {
    pub fn solve(self) -> Result<TangentImpulseSolution, ResponseError> {
        if self.inverse_response.iter().any(|value| !value.is_finite())
            || self.right_hand_side.iter().any(|value| !value.is_finite())
            || !self.impulse_limit.is_finite()
        {
            return Err(ResponseError::NonFinite);
        }
        if self.impulse_limit < 0.0 {
            return Err(ResponseError::NegativeImpulseLimit);
        }
        let unconstrained = [
            (self.inverse_response[1] * self.right_hand_side[1]
                + self.inverse_response[0] * self.right_hand_side[0]) as f32,
            (self.inverse_response[3] * self.right_hand_side[1]
                + self.inverse_response[2] * self.right_hand_side[0]) as f32,
        ];
        let magnitude_squared =
            f64::from(unconstrained[0] * unconstrained[0] + unconstrained[1] * unconstrained[1]);
        let clamped = magnitude_squared > self.impulse_limit * self.impulse_limit;
        let impulse = if clamped {
            let inverse_length =
                reciprocal_square_root_f64(f64::from(magnitude_squared as f32)) as f32;
            let scale = f64::from(inverse_length) * self.impulse_limit;
            unconstrained.map(|value| (f64::from(value) * scale) as f32)
        } else {
            unconstrained
        };
        Ok(TangentImpulseSolution {
            unconstrained,
            impulse,
            magnitude_squared,
            clamped,
        })
    }
}

impl TangentFrame {
    pub fn from_edge(
        topology: &FeatureTopology,
        identity: EdgeId,
        endpoint_basis: [f64; 12],
        normal: [f32; 3],
    ) -> Result<Self, ResponseError> {
        if endpoint_basis.iter().any(|value| !value.is_finite())
            || normal.iter().any(|value| !value.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let edge = topology.edge(identity)?;
        let start = topology.points()[edge.start as usize];
        let end = topology.points()[edge.end as usize];
        let direction: [f32; 3] = std::array::from_fn(|axis| end[axis] - start[axis]);
        let transformed = [
            (f64::from(direction[1]) * endpoint_basis[1]
                + f64::from(direction[0]) * endpoint_basis[0]
                + f64::from(direction[2]) * endpoint_basis[2]) as f32,
            (f64::from(direction[1]) * endpoint_basis[5]
                + f64::from(direction[0]) * endpoint_basis[4]
                + f64::from(direction[2]) * endpoint_basis[6]) as f32,
            (f64::from(direction[1]) * endpoint_basis[9]
                + f64::from(direction[0]) * endpoint_basis[8]
                + f64::from(direction[2]) * endpoint_basis[10]) as f32,
        ];
        let length_squared = transformed[0] * transformed[0]
            + transformed[1] * transformed[1]
            + transformed[2] * transformed[2];
        if length_squared == 0.0 {
            return Err(ResponseError::ZeroDirection);
        }
        let inverse = reciprocal_square_root_f64(f64::from(length_squared));
        let first = transformed.map(|value| (f64::from(value) * inverse) as f32);
        let second = [
            normal[1] * first[2] - normal[2] * first[1],
            normal[2] * first[0] - normal[0] * first[2],
            normal[0] * first[1] - normal[1] * first[0],
        ];
        Ok(Self { first, second })
    }

    pub fn from_world_edge(
        topology: &FeatureTopology,
        identity: EdgeId,
        endpoint_basis: [f64; 12],
        endpoint_position: [f64; 3],
        normal: [f32; 3],
    ) -> Result<Self, ResponseError> {
        if endpoint_basis
            .iter()
            .any(|component| !component.is_finite())
            || endpoint_position
                .iter()
                .any(|component| !component.is_finite())
            || normal.iter().any(|component| !component.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let edge = topology.edge(identity)?;
        let endpoints = [
            topology.points()[edge.start as usize].map(f64::from),
            topology.points()[edge.end as usize].map(f64::from),
        ];
        let transformed = endpoints.map(|point| {
            std::array::from_fn::<_, 3, _>(|axis| {
                ((endpoint_basis[axis * 4] * point[0] + endpoint_basis[axis * 4 + 1] * point[1])
                    + endpoint_basis[axis * 4 + 2] * point[2])
                    + endpoint_position[axis]
            })
        });
        let direction: [f64; 3] =
            std::array::from_fn(|axis| transformed[1][axis] - transformed[0][axis]);
        let squared = (direction[1] * direction[1] + direction[0] * direction[0])
            + direction[2] * direction[2];
        if squared == 0.0 {
            return Err(ResponseError::ZeroDirection);
        }
        let inverse = 1.0 / squared.sqrt();
        let first = direction.map(|component| (component * inverse) as f32);
        let second = [
            normal[1] * first[2] - normal[2] * first[1],
            normal[2] * first[0] - normal[0] * first[2],
            normal[0] * first[1] - normal[1] * first[0],
        ];
        Ok(Self { first, second })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AngularVelocityLimit, CollisionBody, CollisionCone, CollisionCorrection,
        CollisionImpulseStep, CollisionRequest, CollisionResponse, ContactNormalRow,
        CoupledNormalSystem, DynamicEndpoint, FrictionImpulseLimit, FrictionRedistributionOwner,
        ImpactContactPoint, NormalAssembly, NormalBody, NormalContact, NormalContactRow,
        NormalEndpointRow, RawNormalSystem, ResponseError, RetainedFrictionClamp,
        RetainedFrictionOwner, RetainedFrictionTransport, TangentAssembly, TangentBody,
        TangentFrame, TangentImpulseSystem, redistribute_retained_friction,
    };
    use crate::{AuthoredFace, FeatureTopology};

    fn topology() -> FeatureTopology {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        FeatureTopology::new(
            vec![[0.0, 0.0, 0.0], [-5.08, 0.0, 5.08], [0.0, 1.0, 0.0]],
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn configured_persistent_normal_response_preserves_binary_width() {
        let response = NormalContact {
            target_distance: 0.012_694_919_5,
            distance: 0.018_893_182,
            relative_speed: 0.229_297_846_555_709_84,
            effective_mass: 5.0,
        };
        assert_eq!(
            response.impulse().unwrap().to_bits(),
            0.526_662_953_197_956_1_f64.to_bits()
        );
        assert_eq!(
            NormalContact {
                relative_speed: -1.0,
                ..response
            }
            .impulse()
            .unwrap(),
            0.0
        );
    }

    #[test]
    fn complete_collision_response_preserves_ordered_pushes_and_restitution() {
        let body = CollisionBody {
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            local_offset: [0.0; 3],
            inverse_mass: 0.2,
            inverse_inertia: [1.0; 3],
            linear_velocity: [0.0, 1.0, 0.0],
            angular_velocity: [0.0; 3],
        };
        let response = CollisionResponse {
            repetitions: 0,
            body,
            opposing: None,
            normal: [0.0, -1.0, 0.0],
            friction: 0.64,
            elasticity: 0.25,
        }
        .solve()
        .unwrap();
        assert!(response.pushes.len() <= 102);
        assert_eq!(response.pushes[0].relative_velocity, [0.0, 1.0, 0.0]);
        assert!((response.body.linear_velocity[1] + 0.5).abs() < 0.000_001);
        assert!(response.pushes.iter().any(|push| push.impulse == 1.0));
        assert!(response.opposing.is_none());
    }

    #[test]
    fn complete_collision_response_rejects_malformed_endpoints_and_nonapproaching_contacts() {
        let body = CollisionBody {
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            local_offset: [0.0; 3],
            inverse_mass: 0.2,
            inverse_inertia: [1.0; 3],
            linear_velocity: [0.0, -1.0, 0.0],
            angular_velocity: [0.0; 3],
        };
        let input = CollisionResponse {
            repetitions: 0,
            body,
            opposing: None,
            normal: [0.0, -1.0, 0.0],
            friction: 0.5,
            elasticity: 0.25,
        };
        let idle = input.solve().unwrap();
        assert_eq!(idle.body, body);
        assert!(idle.pushes.is_empty());
        assert_eq!(
            CollisionResponse {
                body: CollisionBody {
                    inverse_mass: 0.0,
                    ..body
                },
                ..input
            }
            .solve(),
            Err(ResponseError::NonPositiveMass)
        );
        assert_eq!(
            CollisionResponse {
                normal: [f32::NAN, 0.0, 0.0],
                ..input
            }
            .solve(),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn post_impact_correction_preserves_target_request_and_minimum_separation() {
        let body = CollisionBody {
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            local_offset: [0.0; 3],
            inverse_mass: 0.2,
            inverse_inertia: [1.0; 3],
            linear_velocity: [0.0, f32::from_bits(0xbd9c_0d60), 0.0],
            angular_velocity: [0.0; 3],
        };
        let input = CollisionCorrection::from_request(
            [Some(body), None],
            [0.0, -1.0, 0.0],
            f32::from_bits(0x3f0e_e856),
        )
        .unwrap();
        let corrected = input.apply().unwrap();
        assert_eq!(corrected.required_speed.to_bits(), 0x3f2f_6392);
        assert_eq!(
            corrected.endpoints[0].unwrap().linear_velocity[1].to_bits(),
            0xbf2f_6392
        );
        assert!(corrected.impulse > 0.0);
        let minimum = CollisionCorrection::from_request([Some(body), None], input.normal, 0.0)
            .unwrap()
            .apply()
            .unwrap();
        assert_eq!(minimum.required_speed.to_bits(), 0x3c79_97a0);
        assert_eq!(minimum.impulse, 0.0);
        assert_eq!(minimum.endpoints[0], Some(body));
        assert_eq!(
            CollisionCorrection::from_request([Some(body), None], input.normal, -1.0),
            Err(ResponseError::NegativeRequestSpeed)
        );
    }

    #[test]
    fn paired_correction_uses_both_responses_and_distinguishes_closing_only_mode() {
        let body = CollisionBody {
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            local_offset: [0.0; 3],
            inverse_mass: 1.0,
            inverse_inertia: [1.0; 3],
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
        };
        let input = CollisionCorrection {
            endpoints: [Some(body); 2],
            normal: [1.0, 0.0, 0.0],
            required_speed: 1.0,
            velocity: super::CorrectionVelocity::Actual,
        };
        let corrected = input.apply().unwrap();
        assert_eq!(
            corrected.endpoints[0].unwrap().linear_velocity,
            [0.5, 0.0, 0.0]
        );
        assert_eq!(
            corrected.endpoints[1].unwrap().linear_velocity,
            [-0.5, 0.0, 0.0]
        );
        let separating = CollisionBody {
            linear_velocity: [2.0, 0.0, 0.0],
            ..body
        };
        let actual = CollisionCorrection {
            endpoints: [Some(separating), Some(body)],
            ..input
        };
        assert_eq!(actual.apply().unwrap().impulse, 0.0);
        assert!(
            CollisionCorrection {
                velocity: super::CorrectionVelocity::ClosingOnly,
                ..actual
            }
            .apply()
            .unwrap()
            .impulse
                > 0.0
        );
        assert_eq!(
            CollisionCorrection {
                required_speed: f32::NAN,
                ..input
            }
            .apply(),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn heavier_endpoint_delay_retains_active_state_and_stores_only_the_change() {
        let current = crate::VelocityState {
            linear: [0.0; 3],
            angular: [0.0; 3],
        };
        let body = CollisionBody {
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            local_offset: [0.0; 3],
            inverse_mass: 1.0,
            inverse_inertia: [1.0; 3],
            linear_velocity: [1.0, 0.0, 0.0],
            angular_velocity: [0.0; 3],
        };
        let opposing = CollisionBody {
            inverse_mass: 0.01,
            linear_velocity: [-0.01, 0.0, 0.0],
            ..body
        };
        let input = super::CollisionMotionCommit {
            endpoints: [Some(body), Some(opposing)],
            current: [current; 2],
            effective_masses: [1.0, 100.0],
            normal: [-1.0, 0.0, 0.0],
            required_speed: 0.1,
            allow_delay: true,
            collision_counts: [0, 2],
        };
        let delayed = input.apply().unwrap();
        assert_eq!(delayed.active[0].linear, body.linear_velocity);
        assert_eq!(delayed.active[1], current);
        assert_eq!(delayed.queued[1].linear, opposing.linear_velocity);
        assert_eq!(delayed.applied, [true, false]);
        assert_eq!(delayed.collision_counts, [1, 2]);
        let immediate = super::CollisionMotionCommit {
            allow_delay: false,
            ..input
        }
        .apply()
        .unwrap();
        assert_eq!(immediate.queued, [crate::QueuedVelocity::default(); 2]);
        assert_eq!(immediate.active[1].linear, opposing.linear_velocity);
        assert_eq!(immediate.collision_counts, [1, 3]);
    }

    #[test]
    fn collision_speed_limits_run_after_response_and_use_distinct_linear_damping() {
        let limits = super::CollisionVelocityLimits {
            maximum_linear: 10.0,
            angular: AngularVelocityLimit {
                maximum_per_step: 1.0,
                timestep: 0.1,
            },
        };
        let result = limits
            .apply(crate::VelocityState {
                linear: [20.0, 0.0, 0.0],
                angular: [0.0, 20.0, 0.0],
            })
            .unwrap();
        assert_eq!(
            result.linear[0].to_bits(),
            (10.0 * f64::from(0.99_f32) as f32).to_bits()
        );
        assert_eq!(result.angular, [0.0, 9.0, 0.0]);
        let bounded = crate::VelocityState {
            linear: [10.0, 0.0, 0.0],
            angular: [0.0, 10.0, 0.0],
        };
        assert_eq!(limits.apply(bounded).unwrap(), bounded);
    }

    #[test]
    fn impact_contact_round_trips_the_synchronized_world_point() {
        let offset = ImpactContactPoint {
            center: [
                f64::from_bits(0x3ff1_ff29_7ef5_1f8a),
                f64::from_bits(0xbfbe_4984_0cdc_3054),
                f64::from_bits(0xbe95_bfbb_d7bc_9763),
            ],
            orientation: [
                f64::from_bits(0xbfef_ff6b_a4c1_d66c),
                f64::from_bits(0x3f88_5c22_572a_a014),
                f64::from_bits(0xbe87_1918_8a28_471d),
                f64::from_bits(0xbf88_5c22_572e_e9a4),
                f64::from_bits(0xbfef_ff6b_a4c1_24b2),
                f64::from_bits(0x3eca_af8b_e8de_99d1),
                f64::from_bits(0xbe82_048d_e333_9081),
                f64::from_bits(0x3eca_b375_85b6_68cf),
                f64::from_bits(0x3fef_ffff_ffff_4d72),
            ],
            authored_offset: [
                f32::from_bits(0x0000_0000),
                f32::from_bits(0xbde5_5062),
                f32::from_bits(0xa680_0000),
            ],
        };
        assert_eq!(
            offset.local_offset().unwrap().map(f32::to_bits),
            [0x22e6_915a, 0xbde5_5062, 0xa67f_ffcd]
        );
        assert_eq!(
            ImpactContactPoint {
                center: [f64::NAN, 0.0, 0.0],
                ..offset
            }
            .local_offset(),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn coupled_normal_application_preserves_target_async_velocity_rounding() {
        let row = ContactNormalRow {
            normal: [0.0, 1.0, 0.0],
            angular_jacobian: [
                f32::from_bits(0x320f_a356),
                f32::from_bits(0xb201_3888),
                f32::from_bits(0xbc9d_4606),
            ],
            distance: 0.0,
            dynamic_endpoint: DynamicEndpoint::First,
        };
        let body = NormalBody {
            angular_velocity: [
                f32::from_bits(0xb5a3_1f39),
                f32::from_bits(0xb5a9_c715),
                f32::from_bits(0x4172_93ba),
            ],
            linear_velocity: [
                f32::from_bits(0x3fd5_5bf4),
                f32::from_bits(0x3eaa_26a5),
                f32::from_bits(0xb494_02e7),
            ],
            inverse_inertia: [
                f32::from_bits(0x4269_597d),
                f32::from_bits(0x426e_da77),
                f32::from_bits(0x4269_597d),
            ],
            inverse_mass: f32::from_bits(0x3e4c_cccd),
        };
        let queued = row
            .apply_impulse(
                NormalBody {
                    angular_velocity: [0.0; 3],
                    linear_velocity: [0.0; 3],
                    ..body
                },
                f64::from_bits(0x3fca_c506_bb2a_160b),
            )
            .unwrap();
        let applied = NormalBody {
            angular_velocity: std::array::from_fn(|i| {
                body.angular_velocity[i] + queued.angular_velocity[i]
            }),
            linear_velocity: std::array::from_fn(|i| {
                body.linear_velocity[i] + queued.linear_velocity[i]
            }),
            ..body
        };
        assert_eq!(
            applied.angular_velocity.map(f32::to_bits),
            [0xb5b0_d026, 0xb59d_2b95, 0x4176_5323]
        );
        assert_eq!(
            applied.linear_velocity.map(f32::to_bits),
            [0x3fd5_5bf4, 0x3e94_bc39, 0xb494_02e7]
        );
    }

    #[test]
    fn isolated_normal_application_preserves_one_final_binary32_rounding() {
        let row = ContactNormalRow::from_local(
            [0.0, 1.0, 0.0],
            [
                f32::from_bits(0x0000_0000),
                f32::from_bits(0x3de5_5062),
                f32::from_bits(0xa580_0000),
            ],
            [
                f64::from_bits(0x3fef_e10c_9a61_4453),
                f64::from_bits(0xbfb6_3b7a_7124_0d09),
                f64::from_bits(0xbc85_2dc3_9f47_e7f4),
                f64::from_bits(0x3fb6_3b7a_7124_0d09),
                f64::from_bits(0x3fef_e10c_9a61_4453),
                f64::from_bits(0x3cf5_cd6e_13d5_dc4a),
                f64::from_bits(0xbcbb_a85a_c59f_11bc),
                f64::from_bits(0xbcf5_bc05_5c64_b628),
                1.0,
            ],
            0.0,
            DynamicEndpoint::First,
        )
        .unwrap();
        let body = NormalBody {
            angular_velocity: [
                f32::from_bits(0x282c_39b8),
                f32::from_bits(0xa61d_4782),
                f32::from_bits(0xbe89_a343),
            ],
            linear_velocity: [
                f32::from_bits(0x3d21_b372),
                f32::from_bits(0x3ee3_8898),
                f32::from_bits(0x2746_64ec),
            ],
            inverse_inertia: [
                f32::from_bits(0x4269_597d),
                f32::from_bits(0x426e_da77),
                f32::from_bits(0x4269_597d),
            ],
            inverse_mass: f32::from_bits(0x3e4c_cccd),
        };
        let applied = row
            .apply_impulse(body, f64::from_bits(0x3ffb_c19f_e543_07b0))
            .unwrap();
        assert_eq!(
            applied.angular_velocity.map(f32::to_bits),
            [0xa998_68e0, 0x26d1_3d14, 0x3f37_1bbb]
        );
        assert_eq!(
            applied.linear_velocity.map(f32::to_bits),
            [0x3d21_b372, 0x3dc7_9262, 0x2746_64ec]
        );
        let queued = row
            .apply_impulse(
                NormalBody {
                    angular_velocity: [0.0; 3],
                    linear_velocity: [0.0; 3],
                    ..body
                },
                f64::from_bits(0x3ffb_c19f_e543_07b0),
            )
            .unwrap();
        assert_ne!(
            std::array::from_fn::<_, 3, _>(|i| body.linear_velocity[i] + queued.linear_velocity[i]),
            applied.linear_velocity
        );
    }

    #[test]
    fn tangent_assembly_and_atomic_application_preserve_target_bits() {
        let body = TangentBody {
            position: [
                f64::from_bits(0x3fe0_80a2_1e33_d0e8),
                f64::from_bits(0xbfbe_c51c_4d35_4eea),
                f64::from_bits(0xbe57_a89d_cea4_05fd),
            ],
            orientation: [
                f64::from_bits(0xbfe7_c6db_18d6_8e06),
                f64::from_bits(0x3fe5_6a9d_08f2_6f5e),
                f64::from_bits(0x3e85_c453_71ef_9444),
                f64::from_bits(0xbfe5_6a9d_08f2_7008),
                f64::from_bits(0xbfe7_c6db_18d6_8da0),
                f64::from_bits(0xbe81_cf4e_dd30_18ca),
                f64::from_bits(0x3e61_03f7_a8ee_08e4),
                f64::from_bits(0xbe8b_cd09_4007_d148),
                f64::from_bits(0x3fef_ffff_ffff_ff3a),
            ],
            angular_velocity: [
                f32::from_bits(0xb5aa_03ff),
                f32::from_bits(0xb5a3_16ea),
                f32::from_bits(0x416a_40c9),
            ],
            linear_velocity: [
                f32::from_bits(0x3fd5_e840),
                f32::from_bits(0x3e91_38d6),
                f32::from_bits(0xb485_370e),
            ],
            inverse_inertia: [
                f32::from_bits(0x4269_597d),
                f32::from_bits(0x426e_da77),
                f32::from_bits(0x4269_597d),
            ],
            inverse_mass: f32::from_bits(0x3e4c_cccd),
        };
        let assembled = TangentAssembly {
            bodies: [Some(body), None],
            point: [
                f64::from_bits(0x3fdf_8387_2938_cc48),
                f64::from_bits(0xbf85_dd87_f030_7088),
                f64::from_bits(0xbe3a_7ffc_74ca_9754),
            ],
            frame: TangentFrame {
                first: [-1.0, 0.0, 0.0],
                second: [0.0, -0.0, 1.0],
            },
            retained: [f32::from_bits(0x397e_c728), f32::from_bits(0xaef7_a97d)],
            timestep: 0.015,
        }
        .assemble()
        .unwrap();
        assert_eq!(
            assembled.current_velocity.map(f32::to_bits),
            [0xbd8a_a9d0, 0x32b8_3990]
        );
        assert_eq!(
            assembled.response.map(f64::to_bits),
            [
                0x3fec_ca7f_c000_0000,
                0xbe58_0dd8_0000_0000,
                0xbe58_0dd8_0000_0000,
                0x3fed_f143_a000_0000,
            ]
        );
        assert_eq!(
            assembled.inverse_response.map(f64::to_bits),
            [
                0x3ff1_c888_2b48_31b6,
                0x3e5c_927f_8540_c382,
                0x3e5c_927f_8540_c382,
                0x3ff1_1977_4128_067d,
            ]
        );
        assert_eq!(
            assembled.right_hand_side.map(f64::to_bits),
            [0x3fb5_7acd_2000_0000, 0xbe5f_170b_c000_0000]
        );
        let solved = TangentImpulseSystem {
            inverse_response: assembled.inverse_response,
            right_hand_side: assembled.right_hand_side,
            impulse_limit: f64::from_bits(0x3f91_cf1b_4000_0000),
        }
        .solve()
        .unwrap();
        assert_eq!(solved.impulse.map(f32::to_bits), [0x3c8e_78da, 0xb1b7_fabd]);
        let applied = assembled.apply(solved.impulse).unwrap()[0].unwrap();
        assert_eq!(
            applied.angular_velocity.map(f32::to_bits),
            [0xb5a4_081f, 0xb5a7_02d4, 0x416c_07ea]
        );
        assert_eq!(
            applied.linear_velocity.map(f32::to_bits),
            [0x3fd5_7646, 0x3e91_38d6, 0xb485_ca3d]
        );
    }

    #[test]
    fn collision_request_combines_binary32_distance_and_angular_response() {
        let threshold = 0.006_347_459_7;
        let radius = 0.111_969_75;
        let initial = CollisionRequest {
            contact_distance: 0.002_310_065_8,
            contact_threshold: threshold,
            rotations: [
                Some(super::CollisionRotation {
                    angular_velocity: [0.0; 3],
                    contact_radius: radius,
                }),
                None,
            ],
            inverse_timestep: (1.0 / f64::from(0.015_f32)) as f32,
        };
        assert_eq!(
            initial.speed().unwrap().to_bits(),
            0.538_319_23_f32.to_bits()
        );

        let rotating = CollisionRequest {
            contact_distance: 0.006_347_457,
            rotations: [
                Some(super::CollisionRotation {
                    angular_velocity: [-1.220_610_8e-6, -1.242_780_9e-6, 14.733_575],
                    contact_radius: radius,
                }),
                None,
            ],
            ..initial
        };
        assert_eq!(
            rotating.speed().unwrap().to_bits(),
            0.040_492_382_f32.to_bits()
        );
        assert_eq!(
            CollisionRequest {
                rotations: [
                    Some(super::CollisionRotation {
                        angular_velocity: [0.0; 3],
                        contact_radius: -1.0
                    }),
                    None
                ],
                ..initial
            }
            .speed(),
            Err(ResponseError::NegativeContactRadius)
        );
    }

    #[test]
    fn paired_tangent_response_accumulates_relative_velocity_and_updates_both_bodies() {
        let first = TangentBody {
            position: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            angular_velocity: [0.0; 3],
            linear_velocity: [1.0, 2.0, 0.0],
            inverse_inertia: [1.0; 3],
            inverse_mass: 0.5,
        };
        let second = TangentBody {
            linear_velocity: [-1.0, 0.0, 0.0],
            inverse_mass: 1.0,
            ..first
        };
        let input = TangentAssembly {
            bodies: [Some(first), Some(second)],
            point: [0.0; 3],
            frame: TangentFrame {
                first: [1.0, 0.0, 0.0],
                second: [0.0, 1.0, 0.0],
            },
            retained: [0.0; 2],
            timestep: 0.015,
        };
        let assembled = input.assemble().unwrap();
        assert_eq!(assembled.response, [1.5, 0.0, 0.0, 1.5]);
        assert_eq!(assembled.current_velocity, [2.0, 2.0]);
        let solved = TangentImpulseSystem {
            inverse_response: assembled.inverse_response,
            right_hand_side: assembled.right_hand_side,
            impulse_limit: 10.0,
        }
        .solve()
        .unwrap();
        let bodies = assembled.apply(solved.impulse).unwrap();
        assert_eq!(
            bodies[0].unwrap().linear_velocity,
            [
                1.0 + solved.impulse[0] * 0.5,
                2.0 + solved.impulse[1] * 0.5,
                0.0
            ]
        );
        assert_eq!(
            bodies[1].unwrap().linear_velocity,
            [-1.0 - solved.impulse[0], -solved.impulse[1], 0.0]
        );
        assert_eq!(assembled.apply(solved.impulse).unwrap(), bodies);
        assert_eq!(
            TangentAssembly {
                bodies: [None, None],
                ..input
            }
            .assemble(),
            Err(ResponseError::NonPositiveMass)
        );
    }

    #[test]
    fn friction_impulse_limit_uses_the_configured_simulation_timestep() {
        let limit = FrictionImpulseLimit {
            normal_force: 1.811_626_1,
            friction: 0.640_000_05,
            timestep: 0.015,
        };
        assert_eq!(
            limit.impulse().unwrap().to_bits(),
            f64::from(0.017_391_61_f32).to_bits()
        );
        assert_eq!(
            FrictionImpulseLimit {
                normal_force: -1.0,
                ..limit
            }
            .impulse(),
            Err(ResponseError::NegativeNormalForce)
        );
        assert_eq!(
            FrictionImpulseLimit {
                friction: -1.0,
                ..limit
            }
            .impulse(),
            Err(ResponseError::NegativeFriction)
        );
    }

    #[test]
    fn angular_velocity_clamps_in_body_coordinates_without_unit_round_trips() {
        let limit = AngularVelocityLimit {
            maximum_per_step: 3_600.0 * (std::f32::consts::PI / 180.0) * (1.0 / 66.0),
            timestep: 0.015,
        };
        assert_eq!(
            limit
                .apply([3.467_232_8e-6, -3.700_566e-6, 77.933_85])
                .unwrap()
                .map(f32::to_bits),
            [
                2.541_230_5e-6_f32.to_bits(),
                (-2.712_246_7e-6_f32).to_bits(),
                57.119_87_f32.to_bits(),
            ]
        );
        assert_eq!(limit.apply([1.0, 2.0, 3.0]).unwrap(), [1.0, 2.0, 3.0]);
        assert_eq!(
            AngularVelocityLimit {
                maximum_per_step: 0.0,
                ..limit
            }
            .apply([1.0, 2.0, 3.0]),
            Err(ResponseError::NonPositiveAngularLimit)
        );
    }

    #[test]
    fn collision_cone_normalizes_incoming_velocity_before_tangent_projection() {
        let cone = CollisionCone {
            normal: [-0.0, -1.0, -0.0],
            cosine: 0.780_998_77,
            sine: 0.624_799_1,
        };
        assert_eq!(
            cone.direction([2.539_188, 0.304_702_55, 0.0])
                .unwrap()
                .map(f32::to_bits),
            [
                (-0.624_799_1_f32).to_bits(),
                (-0.780_998_77_f32).to_bits(),
                (-0.0_f32).to_bits(),
            ]
        );
        assert_eq!(
            cone.direction([0.0, 0.304_702_55, 2.539_188])
                .unwrap()
                .map(f32::to_bits),
            [
                (-0.0_f32).to_bits(),
                (-0.780_998_77_f32).to_bits(),
                (-0.624_799_1_f32).to_bits(),
            ]
        );
    }

    #[test]
    fn collision_cone_rejects_invalid_vectors() {
        let cone = CollisionCone {
            normal: [0.0, -1.0, 0.0],
            cosine: 1.0,
            sine: 0.0,
        };
        assert_eq!(
            CollisionCone {
                normal: [0.0; 3],
                ..cone
            }
            .direction([1.0, 0.0, 0.0]),
            Err(ResponseError::ZeroDirection)
        );
        assert_eq!(
            cone.direction([f32::NAN, 0.0, 0.0]),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn collision_impulse_step_preserves_target_binary64_operation_order() {
        let first_mass = 4.999_999_823_048_721;
        let first = CollisionImpulseStep {
            dynamic_response_mass: first_mass,
            opposing_response_mass: first_mass * 100_000.0,
            approaching_speed: 0.304_702_550_172_805_8,
        };
        assert_eq!(
            first.impulse().unwrap().to_bits(),
            0.304_699_496_934_756_9_f64.to_bits()
        );

        let second_mass = 4.856_728_519_690_989;
        let second = CollisionImpulseStep {
            dynamic_response_mass: second_mass,
            opposing_response_mass: second_mass * 100_000.0,
            approaching_speed: 0.194_813_892_245_292_66,
        };
        assert_eq!(
            second.impulse().unwrap().to_bits(),
            0.189_229_747_822_236_98_f64.to_bits()
        );
        assert_eq!(
            CollisionImpulseStep {
                approaching_speed: -1.0,
                ..second
            }
            .impulse()
            .unwrap(),
            0.0
        );
    }

    #[test]
    fn collision_impulse_step_rejects_invalid_mass_and_nonfinite_inputs() {
        let valid = CollisionImpulseStep {
            dynamic_response_mass: 1.0,
            opposing_response_mass: 2.0,
            approaching_speed: 3.0,
        };
        assert_eq!(
            CollisionImpulseStep {
                dynamic_response_mass: 0.0,
                ..valid
            }
            .impulse(),
            Err(ResponseError::NonPositiveMass)
        );
        assert_eq!(
            CollisionImpulseStep {
                approaching_speed: f64::INFINITY,
                ..valid
            }
            .impulse(),
            Err(ResponseError::NonFinite)
        );
        assert_eq!(
            CollisionImpulseStep {
                dynamic_response_mass: f64::MAX,
                opposing_response_mass: f64::MAX,
                ..valid
            }
            .impulse(),
            Err(ResponseError::NonFinite)
        );
    }

    #[test]
    fn coupled_normal_rows_keep_zero_force_and_publish_exact_binary32_force() {
        let quiet = CoupledNormalSystem {
            matrix: &[
                0.661_758_499_366_860_5,
                0.766_965_040_127_837_3,
                0.766_965_040_127_837_3,
                1.0,
            ],
            right_hand_side: &[-0.035_640_335_193_518_92, -1.0],
            scale: 1.466_082_090_148_670_6,
            timestep: 0.015,
        }
        .solve(crate::NormalSolvePolicy {
            history: &[0, 0],
            inverse_responses: &[1.0, 1.0],
            gravity_magnitude: 0.0,
            maximum_dimension: 2,
        })
        .unwrap()
        .unwrap();
        assert!(quiet.active_rows.is_empty());
        assert_eq!(quiet.forces, [0.0, 0.0]);

        let active = CoupledNormalSystem {
            matrix: &[
                0.604_573_662_376_492,
                0.562_285_263_951_652_7,
                0.522_050_694_171_242_6,
                0.562_285_263_951_652_7,
                0.667_176_234_426_615_7,
                0.766_972_931_472_805_4,
                0.522_050_694_171_242_6,
                0.766_972_942_507_545_9,
                1.0,
            ],
            right_hand_side: &[0.307_768_866_288_212_04, -0.288_909_776_152_941_17, -1.0],
            scale: 1.966_854_817_818_422,
            timestep: 0.015,
        }
        .solve(crate::NormalSolvePolicy {
            history: &[0, 0, 0],
            inverse_responses: &[1.0, 1.0, 1.0],
            gravity_magnitude: 0.0,
            maximum_dimension: 3,
        })
        .unwrap()
        .unwrap();
        assert_eq!(active.active_rows, [0]);
        assert_eq!(active.forces[0].to_bits(), 66.750_81_f32.to_bits());
        assert_eq!(active.forces[1..], [0.0, 0.0]);
    }

    #[test]
    fn redistribution_uses_endpoint_orientation_and_rejects_overflow_atomically() {
        let owner = FrictionRedistributionOwner {
            first_core: 1,
            normal: [0.0, 1.0, 0.0],
            point: [-1.0, 0.0, 0.0],
            frame: TangentFrame {
                first: [1.0, 0.0, 0.0],
                second: [0.0, 0.0, 1.0],
            },
            coordinates: [1.0, 0.0],
        };
        let mut same = [
            owner,
            FrictionRedistributionOwner {
                point: [1.0, 0.0, 0.0],
                coordinates: [-1.0, 0.0],
                ..owner
            },
        ];
        let mut reversed = same;
        reversed[1].first_core = 2;
        redistribute_retained_friction(&mut same).unwrap();
        assert_eq!(
            [same[0].coordinates[0], same[1].coordinates[0]],
            [0.5, -0.5]
        );
        redistribute_retained_friction(&mut reversed).unwrap();
        assert_eq!(
            [reversed[0].coordinates[0], reversed[1].coordinates[0]],
            [1.0, -1.0]
        );
        let mut invalid = [owner, owner];
        invalid[1].point = [1.0, 0.0, 0.0];
        invalid[0].frame.first = [2.0, 0.0, 0.0];
        invalid[0].coordinates = [f32::MAX, 0.0];
        let before = invalid;
        assert_eq!(
            redistribute_retained_friction(&mut invalid),
            Err(ResponseError::NonFinite)
        );
        assert_eq!(invalid, before);
    }

    #[test]
    fn local_contact_offset_produces_target_width_normal_jacobian() {
        let row = ContactNormalRow::from_local(
            [0.0, 1.0, 0.0],
            [-0.055_984_877, 0.096_968_61, -2.447_176_5e-9],
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            0.006_935_851_6,
            DynamicEndpoint::First,
        )
        .unwrap();
        assert_eq!(
            row.angular_jacobian.map(f32::to_bits),
            [
                2.447_176_5e-9_f32.to_bits(),
                0,
                (-0.055_984_877_f32).to_bits(),
            ]
        );
    }

    #[test]
    fn authored_row_jacobians_assemble_exact_coupled_normal_inputs() {
        let rows = [
            ContactNormalRow {
                normal: [0.0, 1.0, 0.0],
                angular_jacobian: [-5.195_486e-8, -3.025_869e-8, -0.010_402_542],
                distance: 0.006_935_851_6,
                dynamic_endpoint: DynamicEndpoint::First,
            },
            ContactNormalRow {
                normal: [0.0, 1.0, 0.0],
                angular_jacobian: [-4.109_175e-8, -4.279_225e-8, -0.038_902_644],
                distance: 0.013_426_987,
                dynamic_endpoint: DynamicEndpoint::First,
            },
        ];
        let system = NormalAssembly {
            rows: &rows.map(|row| NormalContactRow {
                normal: row.normal,
                distance: row.distance,
                endpoints: [
                    Some(NormalEndpointRow {
                        body: 0,
                        angular_jacobian: row.angular_jacobian,
                    }),
                    None,
                ],
            }),
            bodies: &[NormalBody {
                angular_velocity: [-5.007_661e-6, 8.524_655e-6, 13.607_761],
                linear_velocity: [1.573_743_1, 0.278_306_2, -6.894_792e-7],
                inverse_inertia: [58.337_39, 59.713_345, 58.337_39],
                inverse_mass: 0.2,
            }],
            target_distance: 0.012_694_919_5,
            timestep: 0.015,
            maximum_dimension: 2,
        }
        .assemble()
        .unwrap();
        assert_eq!(
            system
                .right_hand_side
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            [
                0.142_509_944_736_957_55_f64.to_bits(),
                (-0.265_713_050_961_494_45_f64).to_bits(),
            ]
        );
        assert_eq!(
            system
                .matrix
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            [
                0.206_312_859_430_909_16_f64.to_bits(),
                0.223_608_350_381_255_15_f64.to_bits(),
                0.223_608_350_381_255_15_f64.to_bits(),
                0.288_288_727_402_687_1_f64.to_bits(),
            ]
        );
    }

    #[test]
    fn raw_normal_system_normalizes_matrix_requests_and_scale_once() {
        let system = RawNormalSystem {
            matrix: &[
                0.206_312_859_430_909_16,
                0.223_607_007_414_102_55,
                0.223_607_007_414_102_55,
                0.288_288_727_402_687_1,
            ],
            right_hand_side: &[0.142_509_944_736_957_55, -0.265_713_050_961_494_45],
            timestep: 0.015,
        }
        .prepare()
        .unwrap();
        assert_eq!(
            system.matrix[0].to_bits(),
            0.715_646_641_093_696_f64.to_bits()
        );
        assert_eq!(
            system.right_hand_side[0].to_bits(),
            0.536_330_241_293_301_2_f64.to_bits()
        );
        assert_eq!(system.right_hand_side[1].to_bits(), (-1.0_f64).to_bits());
        assert_eq!(
            system.scale.to_bits(),
            0.921_690_741_623_558_2_f64.to_bits()
        );
    }

    #[test]
    fn retained_friction_redistributes_pair_coordinates_in_world_tangent_space() {
        let frame = TangentFrame {
            first: [-1.0, 0.0, 0.0],
            second: [0.0, -0.0, 1.0],
        };
        let mut owners = [
            FrictionRedistributionOwner {
                first_core: 1,
                normal: [0.0, 1.0, 0.0],
                point: [
                    0.435_621_953_722_732_2,
                    -0.022_354_500_803_097_27,
                    1.659_582_307_435_818_2e-9,
                ],
                frame,
                coordinates: [0.0, 0.0],
            },
            FrictionRedistributionOwner {
                first_core: 1,
                normal: [0.0, 1.0, 0.0],
                point: [
                    0.491_753_970_168_399_95,
                    -0.007_913_597_625_994_806,
                    -6.590_452_772_277_116e-9,
                ],
                frame,
                coordinates: [0.000_216_610_22, -2.184_464_3e-10],
            },
        ];
        redistribute_retained_friction(&mut owners).unwrap();
        assert_eq!(
            owners[0].coordinates.map(f32::to_bits),
            [5.079_090_3e-5_f32.to_bits(), 7.465_022e-12_f32.to_bits()]
        );
        assert_eq!(
            owners[1].coordinates.map(f32::to_bits),
            [
                0.000_165_819_32_f32.to_bits(),
                (-2.259_114_6e-10_f32).to_bits()
            ]
        );
    }

    #[test]
    fn retained_friction_clamp_combines_reverse_order_owner_forces() {
        let owners = [
            RetainedFrictionOwner {
                normal_force: 0.0,
                friction: 0.640_000_05,
                response_coefficient: 0.931_388_7,
            },
            RetainedFrictionOwner {
                normal_force: 1.811_626_1,
                friction: 0.640_000_05,
                response_coefficient: 0.931_388_5,
            },
        ];
        let clamped = RetainedFrictionClamp {
            owners: &owners,
            timestep: 0.015,
        }
        .apply([0.001_181_419_8, -5.476_108e-10])
        .unwrap();
        assert_eq!(clamped.limit.to_bits(), 0.000_242_975_19_f32.to_bits());
        assert_eq!(
            clamped.threshold_squared.to_bits(),
            1.059_036_890_183_051_6e-6_f64.to_bits()
        );
        assert_eq!(
            clamped.scale.unwrap().to_bits(),
            0.205_663_713_823_291_52_f64.to_bits()
        );
        assert_eq!(
            clamped.coordinates.map(f32::to_bits),
            [
                0.000_242_975_19_f32.to_bits(),
                (-1.126_236_7e-10_f32).to_bits(),
            ]
        );
    }

    #[test]
    fn retained_friction_transport_projects_both_authored_tangent_axes() {
        let transport = RetainedFrictionTransport {
            coordinates: [0.000_983_986_5, -2.774_21e-10],
            frame: TangentFrame {
                first: [-1.0, 0.0, 0.0],
                second: [0.0, 0.0, 1.0],
            },
            point_velocity: [0.171_300_77, 0.0, 7.665e-9],
            elapsed: f64::from(0.015_f32),
        };
        let actual = transport.advance().unwrap();
        assert_eq!(
            actual[0].to_bits(),
            ((f64::from(transport.coordinates[0])
                + f64::from(transport.point_velocity[0]) * transport.elapsed) as f32)
                .to_bits()
        );
        assert_eq!(
            actual[1].to_bits(),
            ((f64::from(transport.coordinates[1])
                - f64::from(transport.point_velocity[2]) * transport.elapsed) as f32)
                .to_bits()
        );
        assert_eq!(
            RetainedFrictionTransport {
                elapsed: -1.0,
                ..transport
            }
            .advance(),
            Err(ResponseError::NegativeElapsed)
        );
    }

    #[test]
    fn tangent_impulse_preserves_unbounded_magnitude_and_clamped_target_bits() {
        let system = TangentImpulseSystem {
            inverse_response: [
                1.111_457_985_945_077_7,
                2.660_998_917_064_412_2e-8,
                2.660_998_917_064_412_2e-8,
                1.068_717_245_594_967_2,
            ],
            right_hand_side: [0.083_905_048_668_384_55, -2.895_484_030_318_584_7e-8],
            impulse_limit: 0.017_391_610_890_626_907,
        };
        let result = system.solve().unwrap();
        assert!(result.clamped);
        assert_eq!(
            result.magnitude_squared.to_bits(),
            0.008_696_855_977_177_62_f64.to_bits()
        );
        assert_eq!(
            result.impulse.map(f32::to_bits),
            [0.017_391_61_f32.to_bits(), (-5.354_506_6e-9_f32).to_bits(),]
        );
        assert_eq!(
            TangentImpulseSystem {
                impulse_limit: -1.0,
                ..system
            }
            .solve(),
            Err(ResponseError::NegativeImpulseLimit)
        );
    }

    #[test]
    fn tangent_frame_uses_authored_edge_direction_and_source_precision() {
        let topology = topology();
        let basis = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let frame = TangentFrame::from_edge(
            &topology,
            topology.edge_id(0).unwrap(),
            basis,
            [0.0, -1.0, 0.0],
        )
        .unwrap();
        assert_eq!(
            frame.first.map(f32::to_bits),
            [(-0.707_106_77_f32).to_bits(), 0, 0.707_106_77_f32.to_bits()]
        );
        assert_eq!(
            frame.second.map(f32::to_bits),
            [
                (-0.707_106_77_f32).to_bits(),
                (-0.0_f32).to_bits(),
                (-0.707_106_77_f32).to_bits()
            ]
        );
    }

    #[test]
    fn moving_edge_tangent_frame_subtracts_world_endpoints_before_normalization() {
        let word = |point: u32, opposite: i32| point | (((opposite as u32) & 0x7fff) << 16);
        let shape = FeatureTopology::new(
            vec![
                [
                    f32::from_bits(0xbddd_8017),
                    f32::from_bits(0xbced_6720),
                    f32::from_bits(0x31f4_b87c),
                ],
                [
                    f32::from_bits(0xbdc6_977f),
                    f32::from_bits(0xbd65_505c),
                    f32::from_bits(0xb191_a36c),
                ],
                [0.0, 0.0, 1.0],
            ],
            &[
                AuthoredFace {
                    metadata: 0,
                    vertices: [2, 1, 0],
                    edge_words: [word(0, 6), word(1, 4), word(2, 2)],
                },
                AuthoredFace {
                    metadata: 1,
                    vertices: [1, 2, 0],
                    edge_words: [word(0, -2), word(2, -4), word(1, -6)],
                },
            ],
        )
        .unwrap();
        let frame = TangentFrame::from_world_edge(
            &shape,
            shape.edge_id(0).unwrap(),
            [
                f64::from_bits(0x3fdb_8b25_bc46_6d22),
                f64::from_bits(0x3fec_e289_9e10_67d6),
                f64::from_bits(0xbf36_f18e_cee9_f319),
                0.0,
                f64::from_bits(0xbfec_e289_7638_2b9a),
                f64::from_bits(0x3fdb_8b26_3a73_0c34),
                f64::from_bits(0x3f39_d5c8_94f9_7766),
                0.0,
                f64::from_bits(0x3f40_98df_85d5_7f4a),
                f64::from_bits(0x3f23_2eda_b4f6_8bf0),
                f64::from_bits(0x3fef_ffff_b562_3cc5),
                0.0,
            ],
            [
                f64::from_bits(0x4005_1528_7eef_8a42),
                f64::from_bits(0xbfb6_9be8_9a47_e059),
                f64::from_bits(0x3f08_6e34_51d3_d2c4),
            ],
            [
                f32::from_bits(0xbf3e_37c4),
                f32::from_bits(0x3f2b_53e3),
                f32::from_bits(0x28e4_3977),
            ],
        )
        .unwrap();
        assert_eq!(
            frame.first.map(f32::to_bits),
            [0xbf2b_53e3, 0xbf3e_37c4, 0x3874_39cf]
        );
        assert_eq!(
            frame.second.map(f32::to_bits),
            [0x3823_72a5, 0x3835_781b, 0x3f80_0000]
        );
    }

    #[test]
    fn rejects_nonfinite_values_and_nonpositive_effective_mass() {
        let valid = NormalContact {
            target_distance: 1.0,
            distance: 0.0,
            relative_speed: 1.0,
            effective_mass: 1.0,
        };
        assert_eq!(
            NormalContact {
                effective_mass: 0.0,
                ..valid
            }
            .impulse(),
            Err(ResponseError::NonPositiveMass)
        );
        assert_eq!(
            NormalContact {
                relative_speed: f64::NAN,
                ..valid
            }
            .impulse(),
            Err(ResponseError::NonFinite)
        );
    }
}
