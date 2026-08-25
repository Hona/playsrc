use crate::{Error, FailureKind, Operation};
use playsrc_collision::{
    Candidate, Hull, ObjectOverlapRequest, Snapshot, SnapshotTraceRequest, TraceScope, Transform,
    World,
};
use std::collections::BTreeSet;

pub const PUSHER_SNAPSHOT_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PusherLimits {
    pub max_pushers: usize,
    pub max_subjects: usize,
    pub max_contacts: usize,
    pub max_subject_moves: usize,
    pub max_snapshot_bytes: usize,
}
impl Default for PusherLimits {
    fn default() -> Self {
        Self {
            max_pushers: 64,
            max_subjects: 1_024,
            max_contacts: 192,
            max_subject_moves: 65_536,
            max_snapshot_bytes: 1024 * 1024,
        }
    }
}
impl PusherLimits {
    fn validate(self) -> Result<Self, Error> {
        if [
            self.max_pushers,
            self.max_subjects,
            self.max_contacts,
            self.max_subject_moves,
            self.max_snapshot_bytes,
        ]
        .contains(&0)
        {
            Err(mover_error(FailureKind::Malformed, "pusher-limits"))
        } else {
            Ok(self)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearPusherRequest {
    pub request_id: u64,
    pub identity: u64,
    pub start: [f32; 3],
    pub angles: [f32; 3],
    pub destination: [f32; 3],
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformPusherRequest {
    pub request_id: u64,
    pub identity: u64,
    pub start: Transform,
    pub destination: Transform,
    pub linear_speed: f32,
    pub angular_speed: f32,
    pub hierarchy: Vec<PusherHierarchyMemberRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PusherHierarchyMemberRequest {
    pub identity: u64,
    pub start: Transform,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pushability {
    Pushable,
    Pusher,
    Stationary,
    Physics,
    Noclip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PushSubject {
    pub identity: u64,
    pub root_identity: u64,
    pub position: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
    pub collision_group: i32,
    pub support: Option<u64>,
    pub pushability: Pushability,
    pub solid: bool,
    pub point_sized: bool,
    pub volume_contents: bool,
    pub unblockable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockContactKind {
    End,
    Start,
    Stay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockContact {
    pub pusher: u64,
    pub subject: u64,
    pub kind: BlockContactKind,
    pub order: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubjectMove {
    pub pusher: u64,
    pub subject: u64,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub displacement: [f32; 3],
    pub supported: bool,
    pub support_velocity: [f32; 3],
    pub base_velocity: [f32; 3],
    pub angular_displacement: [f32; 3],
    pub order: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PusherStatus {
    Progress,
    Completed,
    Blocked,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PusherResult {
    pub request_id: u64,
    pub identity: u64,
    pub status: PusherStatus,
    pub transform: Transform,
    pub displacement: [f32; 3],
    pub trajectory_velocity: [f32; 3],
    pub angular_displacement: [f32; 3],
    pub angular_velocity: [f32; 3],
    pub blocker: Option<u64>,
    pub subject_moves: Vec<SubjectMove>,
    pub hierarchy: Vec<PusherHierarchyTransform>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PusherHierarchyTransform {
    pub identity: u64,
    pub transform: Transform,
}

#[derive(Clone, Debug, PartialEq)]
struct PusherState {
    request_id: u64,
    identity: u64,
    origin: [f32; 3],
    angles: [f32; 3],
    destination: [f32; 3],
    velocity: [f32; 3],
    destination_angles: [f32; 3],
    angular_velocity: [f32; 3],
    blocker: Option<u64>,
    hierarchy: Vec<PusherHierarchyState>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PusherHierarchyState {
    identity: u64,
    local: Transform,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PusherSnapshot {
    revision: u64,
    states: Vec<PusherState>,
    limits: PusherLimits,
}
impl PusherSnapshot {
    pub fn start(
        revision: u64,
        requests: &[LinearPusherRequest],
        limits: PusherLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if requests.len() > limits.max_pushers {
            return Err(mover_error(FailureKind::Malformed, "pusher-count"));
        }
        let mut identities = BTreeSet::new();
        let mut request_ids = BTreeSet::new();
        let mut states = Vec::with_capacity(requests.len());
        for request in requests {
            if !identities.insert(request.identity) || !request_ids.insert(request.request_id) {
                return Err(mover_error(FailureKind::Malformed, "pusher-identity"));
            }
            if request
                .start
                .into_iter()
                .chain(request.angles)
                .chain(request.destination)
                .chain([request.speed])
                .any(|value| !value.is_finite())
                || request.speed <= 0.0
            {
                return Err(mover_error(FailureKind::Malformed, "pusher-request"));
            }
            let delta = subtract(request.destination, request.start);
            let distance = length(delta);
            let velocity = if distance == 0.0 {
                [0.0; 3]
            } else {
                let travel_time = distance / request.speed;
                if !travel_time.is_finite() || travel_time <= 0.0 {
                    return Err(mover_error(FailureKind::Malformed, "pusher-travel-time"));
                }
                scale(delta, 1.0 / travel_time)
            };
            states.push(PusherState {
                request_id: request.request_id,
                identity: request.identity,
                origin: request.start,
                angles: request.angles,
                destination: request.destination,
                velocity,
                destination_angles: request.angles,
                angular_velocity: [0.0; 3],
                blocker: None,
                hierarchy: Vec::new(),
            });
        }
        Ok(Self {
            revision,
            states,
            limits,
        })
    }

    pub fn start_transforms(
        revision: u64,
        requests: &[TransformPusherRequest],
        limits: PusherLimits,
    ) -> Result<Self, Error> {
        let limits = limits.validate()?;
        if requests.len() > limits.max_pushers {
            return Err(mover_error(FailureKind::Malformed, "pusher-count"));
        }
        let mut identities = BTreeSet::new();
        let mut request_ids = BTreeSet::new();
        let mut states = Vec::with_capacity(requests.len());
        for request in requests {
            if !identities.insert(request.identity) || !request_ids.insert(request.request_id) {
                return Err(mover_error(FailureKind::Malformed, "pusher-identity"));
            }
            if request
                .start
                .origin
                .into_iter()
                .chain(request.start.angles)
                .chain(request.destination.origin)
                .chain(request.destination.angles)
                .chain([request.linear_speed, request.angular_speed])
                .any(|value| !value.is_finite())
                || request.linear_speed < 0.0
                || request.angular_speed < 0.0
            {
                return Err(mover_error(FailureKind::Malformed, "pusher-request"));
            }
            let linear_delta = subtract(request.destination.origin, request.start.origin);
            let angular_delta = subtract(request.destination.angles, request.start.angles);
            let linear_distance = length(linear_delta);
            let angular_distance = length(angular_delta);
            if linear_distance > 0.0 && request.linear_speed == 0.0
                || angular_distance > 0.0 && request.angular_speed == 0.0
            {
                return Err(mover_error(FailureKind::Malformed, "pusher-speed"));
            }
            let velocity = if linear_distance == 0.0 {
                [0.0; 3]
            } else {
                scale(linear_delta, request.linear_speed / linear_distance)
            };
            let angular_velocity = if angular_distance == 0.0 {
                [0.0; 3]
            } else {
                scale(angular_delta, request.angular_speed / angular_distance)
            };
            let mut hierarchy = Vec::with_capacity(request.hierarchy.len());
            for member in &request.hierarchy {
                if !identities.insert(member.identity)
                    || member
                        .start
                        .origin
                        .into_iter()
                        .chain(member.start.angles)
                        .any(|value| !value.is_finite())
                {
                    return Err(mover_error(FailureKind::Malformed, "pusher-hierarchy"));
                }
                hierarchy.push(PusherHierarchyState {
                    identity: member.identity,
                    local: member
                        .start
                        .relative_to(request.start)
                        .map_err(collision_error)?,
                });
            }
            if identities.len() > limits.max_pushers {
                return Err(mover_error(FailureKind::Malformed, "pusher-count"));
            }
            states.push(PusherState {
                request_id: request.request_id,
                identity: request.identity,
                origin: request.start.origin,
                angles: request.start.angles,
                destination: request.destination.origin,
                velocity,
                destination_angles: request.destination.angles,
                angular_velocity,
                blocker: None,
                hierarchy,
            });
        }
        Ok(Self {
            revision,
            states,
            limits,
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn active_count(&self) -> usize {
        self.states.len()
    }

    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut output = BoundedBytes::new(self.limits.max_snapshot_bytes);
        output.bytes(b"PUSH")?;
        output.u32(PUSHER_SNAPSHOT_VERSION)?;
        output.u64(self.revision)?;
        output.u32(as_u32(self.states.len(), "pusher-count")?)?;
        for state in &self.states {
            output.u64(state.request_id)?;
            output.u64(state.identity)?;
            for value in state
                .origin
                .into_iter()
                .chain(state.angles)
                .chain(state.destination)
                .chain(state.velocity)
                .chain(state.destination_angles)
                .chain(state.angular_velocity)
            {
                output.f32(value)?;
            }
            output.u64(state.blocker.unwrap_or(u64::MAX))?;
            output.u32(as_u32(state.hierarchy.len(), "pusher-hierarchy-count")?)?;
            for member in &state.hierarchy {
                output.u64(member.identity)?;
                for value in member.local.origin.into_iter().chain(member.local.angles) {
                    output.f32(value)?;
                }
            }
        }
        Ok(output.finish())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PusherFrame {
    pub collision_snapshot: u64,
    pub next: PusherSnapshot,
    pub results: Vec<PusherResult>,
    pub contacts: Vec<BlockContact>,
}
impl PusherFrame {
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut output = BoundedBytes::new(self.next.limits.max_snapshot_bytes);
        output.bytes(b"PRES")?;
        output.u32(PUSHER_SNAPSHOT_VERSION)?;
        output.u64(self.collision_snapshot)?;
        output.u64(self.next.revision)?;
        output.u32(as_u32(self.results.len(), "pusher-result-count")?)?;
        output.u32(as_u32(self.contacts.len(), "pusher-contact-count")?)?;
        for result in &self.results {
            output.u64(result.request_id)?;
            output.u64(result.identity)?;
            output.u8(match result.status {
                PusherStatus::Progress => 0,
                PusherStatus::Completed => 1,
                PusherStatus::Blocked => 2,
            })?;
            output.u64(result.blocker.unwrap_or(u64::MAX))?;
            output.u32(as_u32(
                result.subject_moves.len(),
                "pusher-subject-move-count",
            )?)?;
            for value in result
                .transform
                .origin
                .into_iter()
                .chain(result.transform.angles)
                .chain(result.displacement)
                .chain(result.trajectory_velocity)
                .chain(result.angular_displacement)
                .chain(result.angular_velocity)
            {
                output.f32(value)?;
            }
            for moved in &result.subject_moves {
                output.u64(moved.subject)?;
                output.u8(u8::from(moved.supported))?;
                output.u32(moved.order)?;
                for value in moved
                    .from
                    .into_iter()
                    .chain(moved.to)
                    .chain(moved.displacement)
                    .chain(moved.support_velocity)
                    .chain(moved.base_velocity)
                    .chain(moved.angular_displacement)
                {
                    output.f32(value)?;
                }
            }
            output.u32(as_u32(result.hierarchy.len(), "pusher-hierarchy-count")?)?;
            for member in &result.hierarchy {
                output.u64(member.identity)?;
                for value in member
                    .transform
                    .origin
                    .into_iter()
                    .chain(member.transform.angles)
                {
                    output.f32(value)?;
                }
            }
        }
        for contact in &self.contacts {
            output.u64(contact.pusher)?;
            output.u64(contact.subject)?;
            output.u8(match contact.kind {
                BlockContactKind::End => 0,
                BlockContactKind::Start => 1,
                BlockContactKind::Stay => 2,
            })?;
            output.u32(contact.order)?;
        }
        Ok(output.finish())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn advance_linear_pushers(
    world: &World,
    collision: &Snapshot,
    prior: &PusherSnapshot,
    next_revision: u64,
    subjects: &[PushSubject],
    tick_interval: f32,
    pusher_collides_with_subject: impl Fn(u64, &PushSubject) -> bool,
    subject_collides_with_candidate: impl Fn(&PushSubject, Candidate) -> bool,
) -> Result<PusherFrame, Error> {
    if !tick_interval.is_finite() || tick_interval <= 0.0 {
        return Err(mover_error(FailureKind::Malformed, "pusher-tick-interval"));
    }
    if subjects.len() > prior.limits.max_subjects {
        return Err(mover_error(FailureKind::Malformed, "pusher-subject-count"));
    }
    let mut subject_identities = BTreeSet::new();
    for subject in subjects {
        if !subject_identities.insert(subject.identity)
            || subject
                .position
                .into_iter()
                .chain(subject.hull.mins)
                .chain(subject.hull.maxs)
                .any(|value| !value.is_finite())
            || subject
                .hull
                .mins
                .into_iter()
                .zip(subject.hull.maxs)
                .any(|(minimum, maximum)| minimum > maximum)
        {
            return Err(mover_error(FailureKind::Malformed, "pusher-subject"));
        }
    }

    let mut next_states = Vec::with_capacity(prior.states.len());
    let mut results = Vec::with_capacity(prior.states.len());
    let mut contacts = Vec::new();
    let mut total_subject_moves = 0_usize;
    for state in &prior.states {
        let remaining = subtract(state.destination, state.origin);
        let distance = length(remaining);
        let remaining_angles = subtract(state.destination_angles, state.angles);
        let angular_distance = length(remaining_angles);
        if distance == 0.0 && angular_distance == 0.0 {
            append_contact_transition(
                &mut contacts,
                state.identity,
                state.blocker,
                None,
                prior.limits.max_contacts,
            )?;
            results.push(PusherResult {
                request_id: state.request_id,
                identity: state.identity,
                status: PusherStatus::Completed,
                transform: Transform {
                    origin: state.destination,
                    angles: state.destination_angles,
                },
                displacement: [0.0; 3],
                trajectory_velocity: state.velocity,
                angular_displacement: [0.0; 3],
                angular_velocity: state.angular_velocity,
                blocker: None,
                subject_moves: Vec::new(),
                hierarchy: state
                    .hierarchy
                    .iter()
                    .map(|member| PusherHierarchyTransform {
                        identity: member.identity,
                        transform: Transform {
                            origin: state.destination,
                            angles: state.destination_angles,
                        }
                        .compose(member.local)
                        .expect("validated pusher hierarchy"),
                    })
                    .collect(),
            });
            continue;
        }
        let transform = collision
            .object_transform(state.identity)
            .ok_or_else(|| mover_error(FailureKind::Missing, "pusher-collision-object"))?;
        if transform.origin != state.origin || transform.angles != state.angles {
            return Err(mover_error(
                FailureKind::Malformed,
                "pusher-collision-revision",
            ));
        }
        let current_root = Transform {
            origin: state.origin,
            angles: state.angles,
        };
        for member in &state.hierarchy {
            let expected = current_root
                .compose(member.local)
                .map_err(collision_error)?;
            if collision.object_transform(member.identity) != Some(expected) {
                return Err(mover_error(
                    FailureKind::Malformed,
                    "pusher-collision-revision",
                ));
            }
        }
        let speed = length(state.velocity);
        let angular_speed = length(state.angular_velocity);
        let linear_remaining_time = if distance == 0.0 {
            0.0
        } else {
            distance / speed
        };
        let angular_remaining_time = if angular_distance == 0.0 {
            0.0
        } else {
            angular_distance / angular_speed
        };
        let remaining_time = linear_remaining_time.max(angular_remaining_time);
        let move_time = remaining_time.min(tick_interval);
        if !move_time.is_finite() {
            return Err(mover_error(FailureKind::Malformed, "pusher-move-time"));
        }
        let linear_completed = linear_remaining_time <= tick_interval;
        let angular_completed = angular_remaining_time <= tick_interval;
        let completed = linear_completed && angular_completed;
        let destination = if linear_completed {
            state.destination
        } else {
            add(state.origin, scale(state.velocity, move_time))
        };
        let destination_angles = if angular_completed {
            state.destination_angles
        } else {
            add(state.angles, scale(state.angular_velocity, move_time))
        };
        let displacement = subtract(destination, state.origin);
        let angular_displacement = subtract(destination_angles, state.angles);
        let candidate_transform = Transform {
            origin: destination,
            angles: destination_angles,
        };
        let hierarchy = state
            .hierarchy
            .iter()
            .map(|member| {
                candidate_transform
                    .compose(member.local)
                    .map(|transform| PusherHierarchyTransform {
                        identity: member.identity,
                        transform,
                    })
                    .map_err(collision_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidates = Vec::new();
        for (order, subject) in subjects.iter().enumerate() {
            if subject.pushability != Pushability::Pushable
                || !subject.solid
                || subject.root_identity == state.identity
                || state
                    .hierarchy
                    .iter()
                    .any(|member| member.identity == subject.root_identity)
            {
                continue;
            }
            let colliding_pushers = std::iter::once(state.identity)
                .chain(state.hierarchy.iter().map(|member| member.identity))
                .filter(|identity| pusher_collides_with_subject(*identity, subject))
                .collect::<Vec<_>>();
            if colliding_pushers.is_empty() {
                continue;
            }
            let supported = subject
                .support
                .is_some_and(|support| colliding_pushers.contains(&support));
            let intersects = if supported {
                true
            } else {
                std::iter::once(PusherHierarchyTransform {
                    identity: state.identity,
                    transform: candidate_transform,
                })
                .chain(hierarchy.iter().copied())
                .filter(|member| colliding_pushers.contains(&member.identity))
                .try_fold(false, |overlap, member| {
                    if overlap {
                        Ok(true)
                    } else {
                        world
                            .overlaps_object_hull_at(
                                collision,
                                ObjectOverlapRequest {
                                    identity: member.identity,
                                    transform: member.transform,
                                    position: subject.position,
                                    hull: subject.hull,
                                    mask: subject.mask,
                                },
                            )
                            .map_err(collision_error)
                    }
                })?
            };
            if intersects {
                candidates.push((order, *subject, supported));
            }
        }

        let mut moved = Vec::with_capacity(candidates.len());
        let mut blocker = None;
        for (source_order, subject, supported) in candidates.into_iter().rev() {
            let requested = if angular_distance > 0.0 {
                let local = transform
                    .inverse_transform_point(subject.position)
                    .map_err(collision_error)?;
                candidate_transform
                    .transform_point(local)
                    .map_err(collision_error)?
            } else {
                add(subject.position, displacement)
            };
            let ignored = std::iter::once(subject.identity)
                .chain(std::iter::once(state.identity))
                .chain(state.hierarchy.iter().map(|member| member.identity))
                .collect::<Vec<_>>();
            let trace = world
                .trace_snapshot_hull(
                    collision,
                    SnapshotTraceRequest {
                        start: subject.position,
                        end: requested,
                        hull: subject.hull,
                        mask: subject.mask,
                        scope: TraceScope::Everything,
                        ignored: &ignored,
                    },
                    |candidate| subject_collides_with_candidate(&subject, candidate),
                )
                .map_err(collision_error)?;
            let position = if subject.unblockable {
                requested
            } else if trace.fraction != 0.0 {
                trace.end
            } else {
                subject.position
            };
            let accepted = if subject.unblockable
                || subject.point_sized
                || subject.volume_contents
                || trace.fraction == 1.0
            {
                true
            } else {
                let embedded_in_environment = world
                    .trace_snapshot_hull(
                        collision,
                        SnapshotTraceRequest {
                            start: position,
                            end: position,
                            hull: subject.hull,
                            mask: subject.mask,
                            scope: TraceScope::Everything,
                            ignored: &ignored,
                        },
                        |candidate| subject_collides_with_candidate(&subject, candidate),
                    )
                    .map_err(collision_error)?
                    .start_solid;
                let embedded_in_pusher = std::iter::once(PusherHierarchyTransform {
                    identity: state.identity,
                    transform: candidate_transform,
                })
                .chain(hierarchy.iter().copied())
                .try_fold(false, |overlap, member| {
                    if overlap {
                        Ok(true)
                    } else {
                        world
                            .overlaps_object_hull_at(
                                collision,
                                ObjectOverlapRequest {
                                    identity: member.identity,
                                    transform: member.transform,
                                    position,
                                    hull: subject.hull,
                                    mask: subject.mask,
                                },
                            )
                            .map_err(collision_error)
                    }
                })?;
                !embedded_in_environment && !embedded_in_pusher
            };
            if !accepted {
                blocker = Some(subject.identity);
                break;
            }
            let support_velocity = if supported {
                add(
                    state.velocity,
                    angular_point_velocity(
                        state.angular_velocity,
                        subtract(subject.position, state.origin),
                    ),
                )
            } else {
                [0.0; 3]
            };
            moved.push(SubjectMove {
                pusher: state.identity,
                subject: subject.identity,
                from: subject.position,
                to: position,
                displacement: subtract(position, subject.position),
                supported,
                support_velocity,
                base_velocity: support_velocity,
                angular_displacement,
                order: u32::try_from(source_order)
                    .map_err(|_| mover_error(FailureKind::Malformed, "pusher-subject-order"))?,
            });
        }

        append_contact_transition(
            &mut contacts,
            state.identity,
            state.blocker,
            blocker,
            prior.limits.max_contacts,
        )?;
        if let Some(blocker) = blocker {
            next_states.push(PusherState {
                blocker: Some(blocker),
                ..state.clone()
            });
            results.push(PusherResult {
                request_id: state.request_id,
                identity: state.identity,
                status: PusherStatus::Blocked,
                transform,
                displacement: [0.0; 3],
                trajectory_velocity: state.velocity,
                angular_displacement: [0.0; 3],
                angular_velocity: state.angular_velocity,
                blocker: Some(blocker),
                subject_moves: Vec::new(),
                hierarchy: state
                    .hierarchy
                    .iter()
                    .map(|member| PusherHierarchyTransform {
                        identity: member.identity,
                        transform: current_root
                            .compose(member.local)
                            .expect("validated pusher hierarchy"),
                    })
                    .collect(),
            });
            continue;
        }

        total_subject_moves = total_subject_moves
            .checked_add(moved.len())
            .ok_or_else(|| mover_error(FailureKind::Malformed, "pusher-subject-move-count"))?;
        if total_subject_moves > prior.limits.max_subject_moves {
            return Err(mover_error(
                FailureKind::Malformed,
                "pusher-subject-move-count",
            ));
        }
        let status = if completed {
            PusherStatus::Completed
        } else {
            PusherStatus::Progress
        };
        if !completed {
            next_states.push(PusherState {
                origin: destination,
                angles: destination_angles,
                blocker: None,
                ..state.clone()
            });
        }
        results.push(PusherResult {
            request_id: state.request_id,
            identity: state.identity,
            status,
            transform: candidate_transform,
            displacement,
            trajectory_velocity: state.velocity,
            angular_displacement,
            angular_velocity: state.angular_velocity,
            blocker: None,
            subject_moves: moved,
            hierarchy,
        });
    }
    let next = PusherSnapshot {
        revision: next_revision,
        states: next_states,
        limits: prior.limits,
    };
    Ok(PusherFrame {
        collision_snapshot: collision.identity(),
        next,
        results,
        contacts,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn advance_transform_pushers(
    world: &World,
    collision: &Snapshot,
    prior: &PusherSnapshot,
    next_revision: u64,
    subjects: &[PushSubject],
    tick_interval: f32,
    pusher_collides_with_subject: impl Fn(u64, &PushSubject) -> bool,
    subject_collides_with_candidate: impl Fn(&PushSubject, Candidate) -> bool,
) -> Result<PusherFrame, Error> {
    advance_linear_pushers(
        world,
        collision,
        prior,
        next_revision,
        subjects,
        tick_interval,
        pusher_collides_with_subject,
        subject_collides_with_candidate,
    )
}

fn append_contact_transition(
    contacts: &mut Vec<BlockContact>,
    pusher: u64,
    previous: Option<u64>,
    current: Option<u64>,
    maximum: usize,
) -> Result<(), Error> {
    if previous != current {
        if let Some(subject) = previous {
            push_contact(contacts, pusher, subject, BlockContactKind::End, maximum)?;
        }
        if let Some(subject) = current {
            push_contact(contacts, pusher, subject, BlockContactKind::Start, maximum)?;
        }
    }
    if let Some(subject) = current {
        push_contact(contacts, pusher, subject, BlockContactKind::Stay, maximum)?;
    }
    Ok(())
}

fn push_contact(
    contacts: &mut Vec<BlockContact>,
    pusher: u64,
    subject: u64,
    kind: BlockContactKind,
    maximum: usize,
) -> Result<(), Error> {
    if contacts.len() >= maximum {
        return Err(mover_error(FailureKind::Malformed, "pusher-contact-count"));
    }
    contacts.push(BlockContact {
        pusher,
        subject,
        kind,
        order: u32::try_from(contacts.len())
            .map_err(|_| mover_error(FailureKind::Malformed, "pusher-contact-order"))?,
    });
    Ok(())
}

fn collision_error(_: playsrc_collision::Error) -> Error {
    mover_error(FailureKind::Malformed, "pusher-collision")
}

fn mover_error(kind: FailureKind, field: &'static str) -> Error {
    Error::new(Operation::Mover, kind, field)
}

fn as_u32(value: usize, field: &'static str) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| mover_error(FailureKind::Malformed, field))
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn angular_point_velocity(degrees_per_second: [f32; 3], radius: [f32; 3]) -> [f32; 3] {
    let angular = degrees_per_second.map(f32::to_radians);
    [
        angular[1] * radius[2] - angular[2] * radius[1],
        angular[2] * radius[0] - angular[0] * radius[2],
        angular[0] * radius[1] - angular[1] * radius[0],
    ]
}

struct BoundedBytes {
    bytes: Vec<u8>,
    maximum: usize,
}
impl BoundedBytes {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), Error> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(mover_error(FailureKind::Malformed, "pusher-snapshot-bytes"));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), Error> {
        self.bytes(&[value])
    }

    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn f32(&mut self, value: f32) -> Result<(), Error> {
        self.u32(value.to_bits())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_collision::{ObjectInput, ObjectRole, SnapshotLimits, SnapshotShape};

    fn object(identity: u64, origin: [f32; 3], bounds: Hull) -> ObjectInput {
        ObjectInput {
            identity,
            role: ObjectRole::Entity,
            enabled: true,
            volume_contents: false,
            transform: Transform {
                origin,
                angles: [0.0; 3],
            },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 1,
            surface_flags: 0,
            shape: SnapshotShape::BoundingBox { bounds },
        }
    }

    fn mover_bounds() -> Hull {
        Hull {
            mins: [-1.0, -2.0, -1.0],
            maxs: [1.0, 2.0, 0.0],
        }
    }

    fn subject(identity: u64) -> PushSubject {
        PushSubject {
            identity,
            root_identity: identity,
            position: [0.0; 3],
            hull: Hull {
                mins: [-0.5, -0.5, 0.0],
                maxs: [0.5, 0.5, 2.0],
            },
            mask: 1,
            collision_group: 0,
            support: Some(1),
            pushability: Pushability::Pushable,
            solid: true,
            point_sized: false,
            volume_contents: false,
            unblockable: false,
        }
    }

    fn request(destination: [f32; 3]) -> LinearPusherRequest {
        LinearPusherRequest {
            request_id: 4,
            identity: 1,
            start: [0.0; 3],
            angles: [0.0; 3],
            destination,
            speed: 100.0,
        }
    }

    #[test]
    fn supported_subject_is_carried_and_fractional_final_tick_completes_exactly() {
        let world = World::empty();
        let collision = Snapshot::compile(
            &world,
            20,
            vec![object(1, [0.0; 3], mover_bounds())],
            SnapshotLimits::default(),
        )
        .unwrap();
        let prior =
            PusherSnapshot::start(5, &[request([1.0, 0.0, 0.0])], PusherLimits::default()).unwrap();
        let frame = advance_linear_pushers(
            &world,
            &collision,
            &prior,
            6,
            &[subject(9)],
            0.015,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(frame.results[0].status, PusherStatus::Completed);
        assert_eq!(frame.results[0].transform.origin, [1.0, 0.0, 0.0]);
        assert_eq!(frame.results[0].trajectory_velocity, [100.0, 0.0, 0.0]);
        assert_eq!(frame.results[0].subject_moves[0].to, [1.0, 0.0, 0.0]);
        assert_eq!(
            frame.results[0].subject_moves[0].base_velocity,
            [100.0, 0.0, 0.0]
        );
        assert_eq!(frame.next.active_count(), 0);
        assert!(frame.contacts.is_empty());
        assert_eq!(&prior.snapshot_bytes().unwrap()[..8], b"PUSH\x02\0\0\0");
        assert_eq!(&frame.snapshot_bytes().unwrap()[..8], b"PRES\x02\0\0\0");
    }

    #[test]
    fn blocker_rolls_back_then_emits_stay_and_end_in_order() {
        let world = World::empty();
        let blocked_collision = Snapshot::compile(
            &world,
            30,
            vec![
                object(1, [0.0; 3], mover_bounds()),
                object(
                    20,
                    [0.0; 3],
                    Hull {
                        mins: [1.0, -2.0, -1.0],
                        maxs: [3.0, 2.0, 3.0],
                    },
                ),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let prior = PusherSnapshot::start(1, &[request([10.0, 0.0, 0.0])], PusherLimits::default())
            .unwrap();
        let first = advance_linear_pushers(
            &world,
            &blocked_collision,
            &prior,
            2,
            &[subject(9)],
            0.015,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(first.results[0].status, PusherStatus::Blocked);
        assert_eq!(first.results[0].transform.origin, [0.0; 3]);
        assert_eq!(first.results[0].blocker, Some(9));
        assert_eq!(
            first
                .contacts
                .iter()
                .map(|contact| contact.kind)
                .collect::<Vec<_>>(),
            [BlockContactKind::Start, BlockContactKind::Stay]
        );

        let stay = advance_linear_pushers(
            &world,
            &blocked_collision,
            &first.next,
            3,
            &[subject(9)],
            0.015,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(
            stay.contacts
                .iter()
                .map(|contact| contact.kind)
                .collect::<Vec<_>>(),
            [BlockContactKind::Stay]
        );

        let clear_collision = Snapshot::compile(
            &world,
            31,
            vec![object(1, [0.0; 3], mover_bounds())],
            SnapshotLimits::default(),
        )
        .unwrap();
        let clear = advance_linear_pushers(
            &world,
            &clear_collision,
            &stay.next,
            4,
            &[subject(9)],
            0.015,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(clear.results[0].status, PusherStatus::Progress);
        assert_eq!(clear.results[0].transform.origin, [1.5, 0.0, 0.0]);
        assert_eq!(
            clear
                .contacts
                .iter()
                .map(|contact| contact.kind)
                .collect::<Vec<_>>(),
            [BlockContactKind::End]
        );
    }

    #[test]
    fn reverse_enumeration_order_selects_first_actual_blocker_and_limits_fail_whole_batch() {
        let world = World::empty();
        let collision = Snapshot::compile(
            &world,
            40,
            vec![
                object(1, [0.0; 3], mover_bounds()),
                object(
                    20,
                    [0.0; 3],
                    Hull {
                        mins: [1.0, -2.0, -1.0],
                        maxs: [3.0, 2.0, 3.0],
                    },
                ),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let prior = PusherSnapshot::start(1, &[request([10.0, 0.0, 0.0])], PusherLimits::default())
            .unwrap();
        let frame = advance_linear_pushers(
            &world,
            &collision,
            &prior,
            2,
            &[subject(8), subject(9)],
            0.015,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(frame.results[0].blocker, Some(9));

        let limited = PusherSnapshot::start(
            1,
            &[request([10.0, 0.0, 0.0])],
            PusherLimits {
                max_subjects: 1,
                ..PusherLimits::default()
            },
        )
        .unwrap();
        assert!(matches!(
            advance_linear_pushers(
                &world,
                &collision,
                &limited,
                2,
                &[subject(8), subject(9)],
                0.015,
                |_, _| true,
                |_, _| true,
            ),
            Err(Error {
                operation: Operation::Mover,
                field: "pusher-subject-count",
                ..
            })
        ));
    }

    #[test]
    fn angular_hierarchy_carries_supported_subject_and_publishes_current_transforms() {
        let world = World::empty();
        let collision = Snapshot::compile(
            &world,
            50,
            vec![
                object(1, [0.0; 3], mover_bounds()),
                object(2, [2.0, 0.0, 0.0], mover_bounds()),
            ],
            SnapshotLimits::default(),
        )
        .unwrap();
        let prior = PusherSnapshot::start_transforms(
            1,
            &[TransformPusherRequest {
                request_id: 9,
                identity: 1,
                start: Transform::IDENTITY,
                destination: Transform {
                    origin: [0.0; 3],
                    angles: [0.0, 90.0, 0.0],
                },
                linear_speed: 0.0,
                angular_speed: 90.0,
                hierarchy: vec![PusherHierarchyMemberRequest {
                    identity: 2,
                    start: Transform {
                        origin: [2.0, 0.0, 0.0],
                        angles: [0.0; 3],
                    },
                }],
            }],
            PusherLimits::default(),
        )
        .unwrap();
        let mut carried = subject(7);
        carried.position = [3.0, 0.0, 0.0];
        carried.support = Some(2);
        let frame = advance_transform_pushers(
            &world,
            &collision,
            &prior,
            2,
            &[carried],
            1.0,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(frame.results[0].status, PusherStatus::Completed);
        assert!((frame.results[0].hierarchy[0].transform.origin[0]).abs() < 0.000_01);
        assert!((frame.results[0].hierarchy[0].transform.origin[1] - 2.0).abs() < 0.000_01);
        let moved = frame.results[0].subject_moves[0];
        assert!(moved.to[0].abs() < 0.000_01);
        assert!((moved.to[1] - 3.0).abs() < 0.000_01);
        assert_eq!(moved.angular_displacement, [0.0, 90.0, 0.0]);
        assert_eq!(&frame.snapshot_bytes().unwrap()[..8], b"PRES\x02\0\0\0");
        for _ in 0..1_024 {
            assert_eq!(
                advance_transform_pushers(
                    &world,
                    &collision,
                    &prior,
                    2,
                    &[carried],
                    1.0,
                    |_, _| true,
                    |_, _| true,
                )
                .unwrap(),
                frame
            );
        }
    }

    #[test]
    fn pitch_roll_transform_request_clamps_exact_final_orientation() {
        let world = World::empty();
        let collision = Snapshot::compile(
            &world,
            60,
            vec![object(1, [0.0; 3], mover_bounds())],
            SnapshotLimits::default(),
        )
        .unwrap();
        let angles = [30.0, 45.0, 20.0];
        let speed = length(angles);
        let prior = PusherSnapshot::start_transforms(
            1,
            &[TransformPusherRequest {
                request_id: 11,
                identity: 1,
                start: Transform::IDENTITY,
                destination: Transform {
                    origin: [0.0; 3],
                    angles,
                },
                linear_speed: 0.0,
                angular_speed: speed,
                hierarchy: Vec::new(),
            }],
            PusherLimits::default(),
        )
        .unwrap();
        let frame = advance_transform_pushers(
            &world,
            &collision,
            &prior,
            2,
            &[],
            1.0,
            |_, _| true,
            |_, _| true,
        )
        .unwrap();
        assert_eq!(frame.results[0].status, PusherStatus::Completed);
        assert_eq!(frame.results[0].transform.angles, angles);
        assert_eq!(frame.results[0].angular_displacement, angles);
        assert_eq!(frame.next.active_count(), 0);
    }
}
