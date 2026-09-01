use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsContactData {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub velocity: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsCollisionData {
    pub materials: [u32; 2],
    pub collision: bool,
    pub shadow: bool,
    pub elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsCallbackKind {
    Fluid {
        controller: u64,
        entered: bool,
    },
    PreCollision(PhysicsCollisionData),
    PostCollision {
        collision: PhysicsCollisionData,
        speed: f32,
    },
    TouchStart,
    TouchEnd,
    Wake,
    Sleep,
    Friction {
        energy: f32,
        materials: [u32; 2],
    },
    PostSimulation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsCallback {
    pub time: f64,
    pub kind: PhysicsCallbackKind,
    pub bodies: [Option<PublishedBody>; 2],
    pub contact: Option<PhysicsContactData>,
    pub point_velocities: [Option<[f32; 3]>; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CallbackState {
    pub(super) detached_body: Option<u64>,
    pair_clocks: std::collections::BTreeMap<[u64; 2], f64>,
    objects: bool,
    collecting: bool,
    pending: Vec<PhysicsCallback>,
    completed: Vec<PhysicsCallback>,
}

impl CallbackState {
    pub(super) fn reset_publication(&mut self) {
        self.completed.clear();
    }
    pub(super) fn validate(&self, maximum: usize, next_storage: u64) -> bool {
        !self.collecting
            && self.detached_body.is_none()
            && self.pair_clocks.len() <= 16
            && self.pair_clocks.iter().all(|(cores, last)| {
                cores[0] < cores[1] && cores[1] < next_storage && last.is_finite()
            })
            && self.pending.len() <= maximum
            && self.completed.len() <= maximum
            && self
                .pending
                .iter()
                .chain(&self.completed)
                .all(PhysicsCallback::valid)
    }
    pub(super) fn begin(&mut self) {
        std::mem::swap(&mut self.completed, &mut self.pending);
        self.pending.clear();
        self.collecting = true;
    }
    pub(super) fn finish(&mut self) {
        self.collecting = false;
    }
    pub(super) fn expire_pair_clocks(&mut self, time: f64) {
        self.pair_clocks
            .retain(|_, last| (time - *last) as f32 <= 1.0);
    }
    pub(super) fn admit_pair_clock(&mut self, mut cores: [u64; 2]) -> f64 {
        cores.sort_unstable();
        if let Some(last) = self.pair_clocks.get(&cores) {
            *last
        } else {
            if self.pair_clocks.len() < 16 {
                self.pair_clocks.insert(cores, -1000.0);
            }
            -1000.0
        }
    }
    pub(super) fn retire_pair_clock(&mut self, mut cores: [u64; 2], time: f64) {
        cores.sort_unstable();
        if let Some(last) = self.pair_clocks.get_mut(&cores) {
            *last = last.max(time);
        } else if self.pair_clocks.len() < 16 {
            self.pair_clocks.insert(cores, time);
        }
    }
}

impl PhysicsCallback {
    fn valid(&self) -> bool {
        if self
            .point_velocities
            .iter()
            .flatten()
            .flatten()
            .any(|v| !v.is_finite())
            || (0..2).any(|side| {
                self.point_velocities[side].is_some()
                    != (self.contact.is_some() && self.bodies[side].is_some())
            })
        {
            return false;
        }
        if !self.time.is_finite()
            || self.bodies.iter().flatten().any(|body| {
                body.position
                    .iter()
                    .chain(&body.linear_velocity)
                    .chain(&body.angular_velocity)
                    .chain(&body.angles)
                    .chain(&body.orientation)
                    .any(|v| !v.is_finite())
            })
        {
            return false;
        }
        if self.contact.is_some_and(|data| {
            data.point
                .iter()
                .chain(&data.normal)
                .chain(data.velocity.as_ref().into_iter().flatten())
                .any(|v| !v.is_finite())
        }) {
            return false;
        }
        let bodies = self.bodies.map(|body| body.is_some());
        let contact = self.contact.map(|data| data.velocity.is_some());
        match self.kind {
            PhysicsCallbackKind::Fluid { .. } => bodies == [true, false] && contact.is_none(),
            PhysicsCallbackKind::PreCollision(data) => {
                data.valid() && bodies == [true, true] && contact == Some(false)
            }
            PhysicsCallbackKind::PostCollision { collision, speed } => {
                collision.valid()
                    && speed.is_finite()
                    && speed >= 0.0
                    && bodies == [true, true]
                    && contact == Some(true)
            }
            PhysicsCallbackKind::TouchStart | PhysicsCallbackKind::TouchEnd => {
                bodies == [true, true] && contact == Some(false)
            }
            PhysicsCallbackKind::Friction { energy, .. } => {
                energy.is_finite()
                    && energy >= 0.0
                    && bodies == [true, false]
                    && contact == Some(false)
            }
            PhysicsCallbackKind::Wake | PhysicsCallbackKind::Sleep => {
                bodies == [true, false] && contact.is_none()
            }
            PhysicsCallbackKind::PostSimulation => bodies == [false, false] && contact.is_none(),
        }
    }
}
impl PhysicsCollisionData {
    fn valid(self) -> bool {
        self.elapsed.is_finite() && (self.collision || self.shadow)
    }
}

fn collision_modes(flags: [u16; 2], fixed: [bool; 2]) -> Option<(bool, bool)> {
    let collision = flags[0] & flags[1] & 1 != 0
        && (!fixed[0] || flags[1] & 0x20 != 0)
        && (!fixed[1] || flags[0] & 0x20 != 0);
    let shadow = (flags[0] ^ flags[1]) & 0x10 != 0;
    (collision || shadow).then_some((collision, shadow))
}

impl PhysicsEnvironment {
    pub(super) fn collision_callback_velocity(
        &self,
        geometry: ContactGeometry,
        source: [u64; 2],
    ) -> Result<[f32; 3], EnvironmentError> {
        let mut velocities = [[0.0; 3]; 2];
        for (side, endpoint) in geometry.endpoints.iter().enumerate() {
            let body = self
                .body(endpoint.body)
                .ok_or(EnvironmentError::MissingBody)?;
            let motion = body.reported_velocity();
            velocities[side] = crate::CollisionBody {
                orientation: body
                    .collision_orientation
                    .unwrap_or_else(|| body.orientation.matrix()),
                local_offset: geometry.synchronized_offsets[side],
                inverse_mass: 0.0,
                inverse_inertia: [0.0; 3],
                linear_velocity: motion.linear,
                angular_velocity: motion.angular,
            }
            .point_velocity();
        }
        if self.body(geometry.endpoints[1].body).unwrap().kind == BodyKind::Static {
            velocities.swap(0, 1);
        }
        let mut result = std::array::from_fn(|axis| velocities[1][axis] - velocities[0][axis]);
        if self.bodies[self.core_body_index(source[1])?].kind == BodyKind::Static {
            result = result.map(|v| -v);
        }
        Ok(result)
    }
    pub fn set_object_event_reporting(&mut self, enabled: bool) {
        self.callbacks.objects = enabled;
    }
    pub fn callbacks(&self) -> &[PhysicsCallback] {
        &self.callbacks.completed
    }

    fn callback_body(&self, identity: u64) -> Result<PublishedBody, EnvironmentError> {
        let body = self.body(identity).ok_or(EnvironmentError::MissingBody)?;
        let (orientation, position) = if let Some(phase) = body.motion_phase() {
            let elapsed = (self.time() - phase.start) as f32;
            let next = if body.collision_orientation.is_some() {
                body.orientation
            } else {
                phase.next_orientation
            };
            (
                phase
                    .prior_orientation
                    .interpolate(next, f64::from(elapsed * phase.inverse_step))?,
                std::array::from_fn(|axis| {
                    phase.position[axis]
                        + f64::from(phase.projection_velocity[axis]) * f64::from(elapsed)
                }),
            )
        } else {
            (
                body.previous_orientation
                    .interpolate(body.orientation, 0.0)?,
                body.core_position,
            )
        };
        let pose = body.frame.object_pose(ProjectionKnot {
            position,
            orientation: orientation.matrix(),
        })?;
        let velocity = body.reported_velocity();
        Ok(PublishedBody {
            angles: orientation.source_angles()?,
            identity,
            position: source_position(pose.position),
            orientation: orientation.source_matrix(),
            linear_velocity: source_direction(velocity.linear, INCHES_PER_METER),
            angular_velocity: source_direction(velocity.angular, DEGREES_PER_RADIAN),
            asleep: body.asleep,
            motion_enabled: body.motion_enabled,
            is_static: body.kind == BodyKind::Static,
        })
    }

    pub(super) fn emit_callback(
        &mut self,
        kind: PhysicsCallbackKind,
        identities: [Option<u64>; 2],
        contact: Option<PhysicsContactData>,
    ) -> Result<(), EnvironmentError> {
        if self
            .callbacks
            .detached_body
            .is_some_and(|id| identities.contains(&Some(id)))
        {
            return Ok(());
        }
        let count = if self.callbacks.collecting {
            self.callbacks.completed.len()
        } else {
            self.callbacks.pending.len()
        };
        if count >= self.config.max_events {
            return Err(EnvironmentError::CallbackLimit);
        }
        let event = PhysicsCallback {
            time: self.time(),
            kind,
            bodies: [
                identities[0].map(|id| self.callback_body(id)).transpose()?,
                identities[1].map(|id| self.callback_body(id)).transpose()?,
            ],
            contact,
            point_velocities: if let Some(contact) = contact {
                [
                    identities[0]
                        .map(|id| self.velocity_at_point(id, contact.point))
                        .transpose()?,
                    identities[1]
                        .map(|id| self.velocity_at_point(id, contact.point))
                        .transpose()?,
                ]
            } else {
                [None, None]
            },
        };
        if !event.valid() {
            return Err(EnvironmentError::NonFinite);
        }
        if self.callbacks.collecting {
            self.callbacks.completed.push(event);
        } else {
            self.callbacks.pending.push(event);
        }
        Ok(())
    }

    pub(super) fn emit_object_callback(
        &mut self,
        identity: u64,
        awake: bool,
    ) -> Result<(), EnvironmentError> {
        if self.callbacks.objects {
            self.emit_callback(
                if awake {
                    PhysicsCallbackKind::Wake
                } else {
                    PhysicsCallbackKind::Sleep
                },
                [Some(identity), None],
                None,
            )?;
        }
        Ok(())
    }

    pub(super) fn emit_touch_callback(
        &mut self,
        endpoints: [BodyConvex; 2],
        surface: ContactSurface,
        begin: bool,
    ) -> Result<(), EnvironmentError> {
        if !self.event_reporting {
            return Ok(());
        }
        let first = self
            .body(endpoints[0].body)
            .ok_or(EnvironmentError::MissingBody)?;
        let second = self
            .body(endpoints[1].body)
            .ok_or(EnvironmentError::MissingBody)?;
        let flags = first.callback_flags | second.callback_flags;
        if flags & 4 == 0
            || ((first.kind == BodyKind::Static || second.kind == BodyKind::Static)
                && flags & 8 == 0)
        {
            return Ok(());
        }
        let point = if begin {
            surface.point
        } else {
            surface.point.map(|v| f64::from(v as f32))
        };
        self.emit_callback(
            if begin {
                PhysicsCallbackKind::TouchStart
            } else {
                PhysicsCallbackKind::TouchEnd
            },
            endpoints.map(|endpoint| Some(endpoint.body)),
            Some(PhysicsContactData {
                point: source_position(point),
                normal: source_direction(surface.normal, 1.0),
                velocity: None,
            }),
        )
    }

    pub(super) fn collision_callback_data(
        &self,
        geometry: ContactGeometry,
        elapsed: f32,
    ) -> Result<Option<PhysicsCollisionData>, EnvironmentError> {
        if !self.event_reporting {
            return Ok(None);
        }
        let first = self
            .body(geometry.endpoints[0].body)
            .ok_or(EnvironmentError::MissingBody)?;
        let second = self
            .body(geometry.endpoints[1].body)
            .ok_or(EnvironmentError::MissingBody)?;
        Ok(collision_modes(
            [first.callback_flags, second.callback_flags],
            [
                first.kind == BodyKind::Static,
                second.kind == BodyKind::Static,
            ],
        )
        .map(|(collision, shadow)| PhysicsCollisionData {
            materials: geometry.materials,
            collision,
            shadow,
            elapsed: if elapsed > 999.0 { 1.0 } else { elapsed },
        }))
    }

    pub(super) fn emit_collision_callback(
        &mut self,
        geometry: ContactGeometry,
        collision: PhysicsCollisionData,
        velocity: Option<[f32; 3]>,
    ) -> Result<(), EnvironmentError> {
        let kind = if let Some(velocity) = velocity {
            PhysicsCallbackKind::PostCollision {
                collision,
                speed: crate::response::ordered_dot(geometry.surface.normal, velocity).abs()
                    * INCHES_PER_METER,
            }
        } else {
            PhysicsCallbackKind::PreCollision(collision)
        };
        self.emit_callback(
            kind,
            geometry.endpoints.map(|endpoint| Some(endpoint.body)),
            Some(PhysicsContactData {
                point: source_position(geometry.surface.point),
                normal: source_direction(geometry.surface.normal, 1.0),
                velocity: velocity.map(|v| source_direction(v, INCHES_PER_METER)),
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collision_reporting_keeps_static_opt_in_separate_from_shadow_xor() {
        for (flags, fixed, expected) in [
            ([1, 1], [false, false], Some((true, false))),
            ([1, 0], [false, false], None),
            ([1, 1], [true, false], None),
            ([1, 0x21], [true, false], Some((true, false))),
            ([0x21, 1], [true, false], None),
            ([0x21, 1], [false, true], Some((true, false))),
            ([0x10, 0], [true, false], Some((false, true))),
            ([0x10, 0x10], [false, false], None),
            ([0x11, 1], [false, false], Some((true, true))),
        ] {
            assert_eq!(collision_modes(flags, fixed), expected);
        }
    }
    #[test]
    fn pair_clocks_survive_retirement_with_exact_capacity_and_expiry() {
        let mut state = CallbackState::default();
        for core in 1..=16 {
            assert_eq!(state.admit_pair_clock([core, 100]), -1000.0);
            state.retire_pair_clock([100, core], 2.0);
        }
        assert_eq!(state.admit_pair_clock([17, 100]), -1000.0);
        assert_eq!(state.pair_clocks.len(), 16);
        state.retire_pair_clock([100, 1], 1.0);
        assert_eq!(state.admit_pair_clock([1, 100]), 2.0);
        state.expire_pair_clocks(3.0 + f64::from(f32::EPSILON) * 0.25);
        assert_eq!(state.pair_clocks.len(), 16);
        state.expire_pair_clocks(3.0 + f64::from(f32::EPSILON));
        assert!(state.pair_clocks.is_empty());
    }
    #[test]
    fn callback_buffers_preserve_commands_and_reject_malformed_snapshots() {
        let event = PhysicsCallback {
            time: 0.5,
            kind: PhysicsCallbackKind::PostSimulation,
            bodies: [None, None],
            contact: None,
            point_velocities: [None, None],
        };
        let mut state = CallbackState::default();
        state.pending.push(event);
        state.begin();
        assert_eq!(state.completed, [event]);
        assert!(state.pending.is_empty());
        assert!(!state.validate(1, 2));
        state.finish();
        assert!(state.validate(1, 2));
        state.completed[0].time = f64::NAN;
        assert!(!state.validate(1, 2));
        state.completed[0] = PhysicsCallback {
            kind: PhysicsCallbackKind::Wake,
            ..event
        };
        assert!(!state.validate(1, 2));
    }
}
