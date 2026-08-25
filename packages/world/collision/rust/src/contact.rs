use crate::{Error, ErrorCode, Hull, ObjectTraceRequest, Snapshot, Transform, World, error};
use std::collections::{BTreeMap, BTreeSet};

pub const CONTACT_SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactLimits {
    pub max_triggers: usize,
    pub max_subjects: usize,
    pub max_pairs: usize,
    pub max_edges: usize,
    pub max_snapshot_bytes: usize,
}

impl Default for ContactLimits {
    fn default() -> Self {
        Self {
            max_triggers: 4_096,
            max_subjects: 4_096,
            max_pairs: 65_536,
            max_edges: 131_072,
            max_snapshot_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerVolume {
    pub identity: u64,
    pub enabled: bool,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactSubject {
    pub identity: u64,
    pub enabled: bool,
    pub position: [f32; 3],
    pub hull: Hull,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContactEdgeKind {
    Enter,
    Stay,
    Exit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContactEdge {
    pub trigger: u64,
    pub subject: u64,
    pub kind: ContactEdgeKind,
    pub order: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContactSnapshot {
    collision_snapshot: u64,
    pairs: Vec<(u64, u64)>,
    subject_positions: BTreeMap<u64, [f32; 3]>,
    trigger_transforms: BTreeMap<u64, Transform>,
    limits: ContactLimits,
}

impl ContactSnapshot {
    pub fn empty(collision_snapshot: u64, limits: ContactLimits) -> Result<Self, Error> {
        validate_limits(limits)?;
        Ok(Self {
            collision_snapshot,
            pairs: Vec::new(),
            subject_positions: BTreeMap::new(),
            trigger_transforms: BTreeMap::new(),
            limits,
        })
    }

    pub fn collision_snapshot(&self) -> u64 {
        self.collision_snapshot
    }

    pub fn pairs(&self) -> &[(u64, u64)] {
        &self.pairs
    }

    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut output = ContactBytes::new(self.limits.max_snapshot_bytes);
        output.bytes(b"CCON")?;
        output.u32(CONTACT_SNAPSHOT_VERSION)?;
        output.u64(self.collision_snapshot)?;
        output.u32(to_u32(self.pairs.len())?)?;
        output.u32(to_u32(self.subject_positions.len())?)?;
        output.u32(to_u32(self.trigger_transforms.len())?)?;
        for (trigger, subject) in &self.pairs {
            output.u64(*trigger)?;
            output.u64(*subject)?;
        }
        for (identity, position) in &self.subject_positions {
            output.u64(*identity)?;
            for value in position {
                output.u32(value.to_bits())?;
            }
        }
        for (identity, transform) in &self.trigger_transforms {
            output.u64(*identity)?;
            for value in transform.origin.into_iter().chain(transform.angles) {
                output.u32(value.to_bits())?;
            }
        }
        Ok(output.finish())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContactFrame {
    pub next: ContactSnapshot,
    pub edges: Vec<ContactEdge>,
}

pub fn produce_trigger_contacts(
    world: &World,
    collision: &Snapshot,
    prior: &ContactSnapshot,
    triggers: &[TriggerVolume],
    subjects: &[ContactSubject],
    should_touch: impl Fn(TriggerVolume, ContactSubject) -> bool,
) -> Result<ContactFrame, Error> {
    let limits = prior.limits;
    validate_limits(limits)?;
    if triggers.len() > limits.max_triggers || subjects.len() > limits.max_subjects {
        return Err(error(ErrorCode::Limit, None));
    }
    unique(triggers.iter().map(|trigger| trigger.identity))?;
    unique(subjects.iter().map(|subject| subject.identity))?;
    for (item, subject) in subjects.iter().enumerate() {
        if subject
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
            return Err(error(ErrorCode::InvalidHull, Some(item)));
        }
    }

    let prior_pairs = prior.pairs.iter().copied().collect::<BTreeSet<_>>();
    let mut next_pairs = Vec::new();
    let mut edges = Vec::new();
    for trigger in triggers {
        let object = collision
            .records()
            .iter()
            .find(|record| record.identity == trigger.identity)
            .ok_or_else(|| error(ErrorCode::InvalidReference, None))?;
        for subject in subjects {
            let pair = (trigger.identity, subject.identity);
            let was_touching = prior_pairs.contains(&pair);
            let admitted = trigger.enabled
                && object.enabled
                && subject.enabled
                && trigger.mask & subject.mask != 0
                && should_touch(*trigger, *subject);
            let mut touching = false;
            let mut swept = false;
            if admitted {
                touching = world.overlaps_object_hull_at(
                    collision,
                    crate::ObjectOverlapRequest {
                        identity: trigger.identity,
                        transform: object.transform,
                        position: subject.position,
                        hull: subject.hull,
                        mask: trigger.mask & subject.mask,
                    },
                )?;
                if !was_touching
                    && !touching
                    && let Some(start) = prior.subject_positions.get(&subject.identity)
                {
                    let start = if let Some(previous_transform) =
                        prior.trigger_transforms.get(&trigger.identity)
                    {
                        object
                            .transform
                            .transform_point(previous_transform.inverse_transform_point(*start)?)?
                    } else {
                        *start
                    };
                    swept = world
                        .trace_object_hull_at(
                            collision,
                            ObjectTraceRequest {
                                identity: trigger.identity,
                                transform: object.transform,
                                start,
                                end: subject.position,
                                hull: subject.hull,
                                mask: trigger.mask & subject.mask,
                            },
                        )?
                        .did_hit();
                }
            }
            match (was_touching, touching, swept) {
                (false, true, _) => push_edge(&mut edges, pair, ContactEdgeKind::Enter, limits)?,
                (true, true, _) => push_edge(&mut edges, pair, ContactEdgeKind::Stay, limits)?,
                (true, false, _) => push_edge(&mut edges, pair, ContactEdgeKind::Exit, limits)?,
                (false, false, true) => {
                    push_edge(&mut edges, pair, ContactEdgeKind::Enter, limits)?;
                    push_edge(&mut edges, pair, ContactEdgeKind::Exit, limits)?;
                }
                (false, false, false) => {}
            }
            if touching {
                if next_pairs.len() >= limits.max_pairs {
                    return Err(error(ErrorCode::Limit, None));
                }
                next_pairs.push(pair);
            }
        }
    }
    let subject_positions = subjects
        .iter()
        .filter(|subject| subject.enabled)
        .map(|subject| (subject.identity, subject.position))
        .collect();
    let trigger_transforms = triggers
        .iter()
        .filter_map(|trigger| {
            collision
                .records()
                .iter()
                .find(|record| record.identity == trigger.identity)
                .map(|record| (trigger.identity, record.transform))
        })
        .collect();
    Ok(ContactFrame {
        next: ContactSnapshot {
            collision_snapshot: collision.identity(),
            pairs: next_pairs,
            subject_positions,
            trigger_transforms,
            limits,
        },
        edges,
    })
}

fn validate_limits(limits: ContactLimits) -> Result<(), Error> {
    if [
        limits.max_triggers,
        limits.max_subjects,
        limits.max_pairs,
        limits.max_edges,
        limits.max_snapshot_bytes,
    ]
    .contains(&0)
    {
        Err(error(ErrorCode::Limit, None))
    } else {
        Ok(())
    }
}

fn unique(values: impl Iterator<Item = u64>) -> Result<(), Error> {
    let mut identities = BTreeSet::new();
    for value in values {
        if !identities.insert(value) {
            return Err(error(ErrorCode::DuplicateIdentity, None));
        }
    }
    Ok(())
}

fn push_edge(
    edges: &mut Vec<ContactEdge>,
    pair: (u64, u64),
    kind: ContactEdgeKind,
    limits: ContactLimits,
) -> Result<(), Error> {
    if edges.len() >= limits.max_edges {
        return Err(error(ErrorCode::Limit, None));
    }
    edges.push(ContactEdge {
        trigger: pair.0,
        subject: pair.1,
        kind,
        order: to_u32(edges.len())?,
    });
    Ok(())
}

fn to_u32(value: usize) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| error(ErrorCode::Limit, None))
}

struct ContactBytes {
    bytes: Vec<u8>,
    maximum: usize,
}

impl ContactBytes {
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
            return Err(error(ErrorCode::Limit, None));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u32(&mut self, value: u32) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), Error> {
        self.bytes(&value.to_le_bytes())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectInput, ObjectRole, SnapshotLimits, SnapshotShape};

    fn trigger(position: [f32; 3]) -> ObjectInput {
        ObjectInput {
            identity: 10,
            role: ObjectRole::Entity,
            enabled: true,
            volume_contents: false,
            transform: Transform {
                origin: position,
                angles: [0.0; 3],
            },
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            collision_group: 0,
            contents: 1,
            surface_flags: 0,
            shape: SnapshotShape::BoundingBox {
                bounds: Hull {
                    mins: [-1.0; 3],
                    maxs: [1.0; 3],
                },
            },
        }
    }

    fn subject(position: [f32; 3]) -> ContactSubject {
        ContactSubject {
            identity: 20,
            enabled: true,
            position,
            hull: Hull {
                mins: [-0.5; 3],
                maxs: [0.5; 3],
            },
            mask: 1,
        }
    }

    #[test]
    fn contacts_emit_enter_stay_exit_and_swept_crossing_in_source_order() {
        let world = World::empty();
        let collision = Snapshot::compile(
            &world,
            7,
            vec![trigger([0.0; 3])],
            SnapshotLimits::default(),
        )
        .unwrap();
        let prior = ContactSnapshot::empty(7, ContactLimits::default()).unwrap();
        let volumes = [TriggerVolume {
            identity: 10,
            enabled: true,
            mask: 1,
        }];
        let entered = produce_trigger_contacts(
            &world,
            &collision,
            &prior,
            &volumes,
            &[subject([0.0; 3])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(entered.edges[0].kind, ContactEdgeKind::Enter);
        let stayed = produce_trigger_contacts(
            &world,
            &collision,
            &entered.next,
            &volumes,
            &[subject([0.0; 3])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(stayed.edges[0].kind, ContactEdgeKind::Stay);
        let exited = produce_trigger_contacts(
            &world,
            &collision,
            &stayed.next,
            &volumes,
            &[subject([4.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(exited.edges[0].kind, ContactEdgeKind::Exit);

        let start = produce_trigger_contacts(
            &world,
            &collision,
            &prior,
            &volumes,
            &[subject([-4.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        let crossed = produce_trigger_contacts(
            &world,
            &collision,
            &start.next,
            &volumes,
            &[subject([4.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(
            crossed
                .edges
                .iter()
                .map(|edge| edge.kind)
                .collect::<Vec<_>>(),
            [ContactEdgeKind::Enter, ContactEdgeKind::Exit]
        );
        assert_eq!(
            &crossed.next.snapshot_bytes().unwrap()[..8],
            b"CCON\x01\0\0\0"
        );
        for _ in 0..1_024 {
            assert_eq!(
                produce_trigger_contacts(
                    &world,
                    &collision,
                    &start.next,
                    &volumes,
                    &[subject([4.0, 0.0, 0.0])],
                    |_, _| true,
                )
                .unwrap(),
                crossed
            );
        }
    }

    #[test]
    fn moving_trigger_disable_duplicates_limits_and_bytes_are_atomic() {
        let world = World::empty();
        let first_collision = Snapshot::compile(
            &world,
            1,
            vec![trigger([0.0; 3])],
            SnapshotLimits::default(),
        )
        .unwrap();
        let volumes = [TriggerVolume {
            identity: 10,
            enabled: true,
            mask: 1,
        }];
        let empty = ContactSnapshot::empty(1, ContactLimits::default()).unwrap();
        let entered = produce_trigger_contacts(
            &world,
            &first_collision,
            &empty,
            &volumes,
            &[subject([0.0; 3])],
            |_, _| true,
        )
        .unwrap();
        let moved_collision = Snapshot::compile(
            &world,
            2,
            vec![trigger([10.0, 0.0, 0.0])],
            SnapshotLimits::default(),
        )
        .unwrap();
        let moved = produce_trigger_contacts(
            &world,
            &moved_collision,
            &entered.next,
            &volumes,
            &[subject([10.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(moved.edges[0].kind, ContactEdgeKind::Stay);
        assert_eq!(moved.next.collision_snapshot(), 2);
        assert_eq!(
            moved.next.snapshot_bytes().unwrap(),
            moved.next.snapshot_bytes().unwrap()
        );
        let disabled = produce_trigger_contacts(
            &world,
            &moved_collision,
            &moved.next,
            &[TriggerVolume {
                enabled: false,
                ..volumes[0]
            }],
            &[subject([10.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(disabled.edges[0].kind, ContactEdgeKind::Exit);
        assert_eq!(
            produce_trigger_contacts(
                &world,
                &moved_collision,
                &moved.next,
                &[volumes[0], volumes[0]],
                &[],
                |_, _| true,
            )
            .unwrap_err()
            .code,
            ErrorCode::DuplicateIdentity
        );
        let limited = ContactSnapshot::empty(
            2,
            ContactLimits {
                max_edges: 1,
                ..ContactLimits::default()
            },
        )
        .unwrap();
        let start = produce_trigger_contacts(
            &world,
            &moved_collision,
            &limited,
            &volumes,
            &[subject([6.0, 0.0, 0.0])],
            |_, _| true,
        )
        .unwrap();
        assert_eq!(
            produce_trigger_contacts(
                &world,
                &moved_collision,
                &start.next,
                &volumes,
                &[subject([14.0, 0.0, 0.0])],
                |_, _| true,
            )
            .unwrap_err()
            .code,
            ErrorCode::Limit
        );
    }
}
