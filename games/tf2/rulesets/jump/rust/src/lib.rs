use std::collections::{BTreeMap, BTreeSet};

pub const COURSE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_zones: usize,
    pub max_checkpoints: usize,
    pub max_contacts_per_tick: usize,
    pub max_events_per_tick: usize,
    pub max_snapshot_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_zones: 4_096,
            max_checkpoints: 1_024,
            max_contacts_per_tick: 16_384,
            max_events_per_tick: 16_384,
            max_snapshot_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ZoneKind {
    MapStart = 1,
    Checkpoint = 2,
    MapEnd = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Zone {
    pub identity: u32,
    pub trigger_entity: u32,
    pub kind: ZoneKind,
    pub index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseDefinition {
    pub identity: u64,
    pub map_identity: [u8; 32],
    pub zones: Vec<Zone>,
    by_trigger: BTreeMap<u32, u32>,
    checkpoint_count: u32,
}

impl CourseDefinition {
    pub fn linear(
        identity: u64,
        map_identity: [u8; 32],
        zones: Vec<Zone>,
        limits: Limits,
    ) -> Result<Self, Error> {
        if identity == 0 || zones.len() > limits.max_zones {
            return Err(Error::new(Failure::Malformed, Field::Course));
        }
        let mut identities = BTreeSet::new();
        let mut by_trigger = BTreeMap::new();
        let mut start_count = 0;
        let mut end_count = 0;
        let mut checkpoints = BTreeSet::new();
        for zone in &zones {
            if zone.identity == 0
                || !identities.insert(zone.identity)
                || by_trigger
                    .insert(zone.trigger_entity, zone.identity)
                    .is_some()
            {
                return Err(Error::new(Failure::Malformed, Field::Zone));
            }
            match zone.kind {
                ZoneKind::MapStart if zone.index == 1 => start_count += 1,
                ZoneKind::MapEnd if zone.index == 1 => end_count += 1,
                ZoneKind::Checkpoint if zone.index > 0 => {
                    if !checkpoints.insert(zone.index) {
                        return Err(Error::new(Failure::Malformed, Field::Checkpoint));
                    }
                }
                _ => return Err(Error::new(Failure::Malformed, Field::Zone)),
            }
        }
        if start_count != 1
            || end_count != 1
            || checkpoints.len() > limits.max_checkpoints
            || checkpoints
                .iter()
                .copied()
                .ne(1..=u32::try_from(checkpoints.len()).unwrap_or(u32::MAX))
        {
            return Err(Error::new(Failure::Malformed, Field::Checkpoint));
        }
        Ok(Self {
            identity,
            map_identity,
            zones,
            by_trigger,
            checkpoint_count: checkpoints.len() as u32,
        })
    }

    pub fn checkpoint_count(&self) -> u32 {
        self.checkpoint_count
    }

    pub fn zone_for_trigger(&self, trigger_entity: u32) -> Option<Zone> {
        let identity = self.by_trigger.get(&trigger_entity)?;
        self.zones
            .iter()
            .find(|zone| zone.identity == *identity)
            .copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Class {
    Soldier = 3,
    Demoman = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Team {
    Unassigned = 0,
    Spectator = 1,
    Red = 2,
    Blue = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerFacts {
    pub identity: u32,
    pub class: Class,
    pub team: Team,
    pub alive: bool,
    pub active: bool,
    pub noclip: bool,
    pub respawned: bool,
    pub teleported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ContactKind {
    Enter = 1,
    Stay = 2,
    Exit = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Contact {
    pub sequence: u64,
    pub trigger_entity: u32,
    pub kind: ContactKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Command {
    pub reset: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RunDisposition {
    Idle = 0,
    Running = 1,
    Completed = 2,
    Invalidated = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InvalidationReason {
    Reset = 1,
    Respawn = 2,
    Ineligible = 3,
    Noclip = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointVisit {
    pub zone_identity: u32,
    pub index: u32,
    pub tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
    pub instance: u32,
    pub disposition: RunDisposition,
    pub player_identity: u32,
    pub class: Class,
    pub team: Team,
    pub start_tick: u64,
    pub end_tick: Option<u64>,
    pub invalidation: Option<InvalidationReason>,
    pub checkpoints: Vec<CheckpointVisit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResult {
    pub course_identity: u64,
    pub map_identity: [u8; 32],
    pub run_instance: u32,
    pub player_identity: u32,
    pub class: Class,
    pub team: Team,
    pub run_kind: RunKind,
    pub zone_index: u32,
    pub disposition: RunDisposition,
    pub start_tick: u64,
    pub end_tick: u64,
    pub elapsed_ticks: u64,
    pub tick_interval_bits: u32,
    pub checkpoints: Vec<CheckpointVisit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RunKind {
    Map = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RejectionReason {
    Ineligible = 1,
    ActiveRun = 2,
    NoRun = 3,
    CheckpointOrder = 4,
    CheckpointsMissing = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
    Started,
    CheckpointAccepted,
    Rejected(RejectionReason),
    Completed,
    Invalidated(InvalidationReason),
    ResetRequested,
    RespawnObserved,
    TeleportObserved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub sequence: u64,
    pub tick: u64,
    pub run_instance: u32,
    pub zone_identity: Option<u32>,
    pub zone_index: Option<u32>,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    FizzleOwnedProjectiles { player_identity: u32 },
    Respawn { player_identity: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TickOutput {
    pub run: Option<Run>,
    pub events: Vec<Event>,
    pub requests: Vec<Request>,
    pub result: Option<RunResult>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickInput<'a> {
    pub tick: u64,
    pub tick_interval: f32,
    pub player: PlayerFacts,
    pub contacts: &'a [Contact],
    pub command: Command,
}

#[derive(Clone, Debug)]
pub struct Session {
    definition: CourseDefinition,
    limits: Limits,
    current_tick: u64,
    next_run_instance: u32,
    next_event_sequence: u64,
    touching: BTreeSet<u32>,
    run: Option<Run>,
}

impl Session {
    pub fn new(definition: CourseDefinition, limits: Limits) -> Self {
        Self {
            definition,
            limits,
            current_tick: 0,
            next_run_instance: 1,
            next_event_sequence: 1,
            touching: BTreeSet::new(),
            run: None,
        }
    }

    pub fn definition(&self) -> &CourseDefinition {
        &self.definition
    }

    pub fn run(&self) -> Option<&Run> {
        self.run.as_ref()
    }

    pub fn advance(&mut self, input: TickInput<'_>) -> Result<TickOutput, Error> {
        let mut candidate = self.clone();
        let output = candidate.advance_inner(input)?;
        *self = candidate;
        Ok(output)
    }

    fn advance_inner(&mut self, input: TickInput<'_>) -> Result<TickOutput, Error> {
        if input.tick < self.current_tick
            || !input.tick_interval.is_finite()
            || input.tick_interval <= 0.0
            || input.player.identity == 0
            || input.contacts.len() > self.limits.max_contacts_per_tick
        {
            return Err(Error::new(Failure::Malformed, Field::Tick));
        }
        let mut prior_sequence = 0;
        for contact in input.contacts {
            if contact.sequence <= prior_sequence
                || self
                    .definition
                    .zone_for_trigger(contact.trigger_entity)
                    .is_none()
            {
                return Err(Error::new(Failure::Malformed, Field::Contact));
            }
            prior_sequence = contact.sequence;
        }
        self.current_tick = input.tick;
        let mut output = TickOutput {
            run: None,
            events: Vec::new(),
            requests: Vec::new(),
            result: None,
        };

        if input.command.reset {
            self.invalidate(
                InvalidationReason::Reset,
                input.tick,
                None,
                &mut output.events,
            )?;
            output.requests.extend([
                Request::FizzleOwnedProjectiles {
                    player_identity: input.player.identity,
                },
                Request::Respawn {
                    player_identity: input.player.identity,
                },
            ]);
            self.event(
                input.tick,
                None,
                None,
                EventKind::ResetRequested,
                &mut output.events,
            )?;
            output.run = self.run.clone();
            return Ok(output);
        } else if input.player.respawned {
            self.invalidate(
                InvalidationReason::Respawn,
                input.tick,
                None,
                &mut output.events,
            )?;
            self.event(
                input.tick,
                None,
                None,
                EventKind::RespawnObserved,
                &mut output.events,
            )?;
        } else if self
            .run
            .as_ref()
            .is_some_and(|run| run.disposition == RunDisposition::Running)
        {
            let reason = if input.player.noclip {
                Some(InvalidationReason::Noclip)
            } else if !eligible(input.player)
                || self.run.as_ref().is_some_and(|run| {
                    run.player_identity != input.player.identity
                        || run.class != input.player.class
                        || run.team != input.player.team
                })
            {
                Some(InvalidationReason::Ineligible)
            } else {
                None
            };
            if let Some(reason) = reason {
                self.invalidate(reason, input.tick, None, &mut output.events)?;
            }
        }

        if input.player.teleported {
            self.event(
                input.tick,
                None,
                None,
                EventKind::TeleportObserved,
                &mut output.events,
            )?;
        }

        for contact in input.contacts {
            let zone = self
                .definition
                .zone_for_trigger(contact.trigger_entity)
                .expect("validated contact trigger");
            match contact.kind {
                ContactKind::Enter => {
                    if !self.touching.insert(zone.identity) {
                        return Err(Error::new(Failure::Malformed, Field::Contact));
                    }
                    self.enter(zone, input, &mut output)?;
                }
                ContactKind::Stay => {
                    if !self.touching.contains(&zone.identity) {
                        return Err(Error::new(Failure::Malformed, Field::Contact));
                    }
                }
                ContactKind::Exit => {
                    if !self.touching.remove(&zone.identity) {
                        return Err(Error::new(Failure::Malformed, Field::Contact));
                    }
                }
            }
        }
        output.run = self.run.clone();
        Ok(output)
    }

    fn enter(
        &mut self,
        zone: Zone,
        input: TickInput<'_>,
        output: &mut TickOutput,
    ) -> Result<(), Error> {
        match zone.kind {
            ZoneKind::MapStart => {
                if !eligible(input.player) {
                    self.event(
                        input.tick,
                        Some(zone),
                        None,
                        EventKind::Rejected(RejectionReason::Ineligible),
                        &mut output.events,
                    )?;
                    return Ok(());
                }
                if self
                    .run
                    .as_ref()
                    .is_some_and(|run| run.disposition == RunDisposition::Running)
                {
                    self.event(
                        input.tick,
                        Some(zone),
                        None,
                        EventKind::Rejected(RejectionReason::ActiveRun),
                        &mut output.events,
                    )?;
                    return Ok(());
                }
                let instance = self.next_run_instance;
                self.next_run_instance = self.next_run_instance.wrapping_add(1).max(1);
                self.run = Some(Run {
                    instance,
                    disposition: RunDisposition::Running,
                    player_identity: input.player.identity,
                    class: input.player.class,
                    team: input.player.team,
                    start_tick: input.tick,
                    end_tick: None,
                    invalidation: None,
                    checkpoints: Vec::new(),
                });
                self.event(
                    input.tick,
                    Some(zone),
                    Some(instance),
                    EventKind::Started,
                    &mut output.events,
                )?;
            }
            ZoneKind::Checkpoint => {
                let Some(run) = self
                    .run
                    .as_ref()
                    .filter(|run| run.disposition == RunDisposition::Running)
                else {
                    self.event(
                        input.tick,
                        Some(zone),
                        None,
                        EventKind::Rejected(RejectionReason::NoRun),
                        &mut output.events,
                    )?;
                    return Ok(());
                };
                let instance = run.instance;
                if zone.index != run.checkpoints.len() as u32 + 1 {
                    self.event(
                        input.tick,
                        Some(zone),
                        Some(instance),
                        EventKind::Rejected(RejectionReason::CheckpointOrder),
                        &mut output.events,
                    )?;
                    return Ok(());
                }
                self.run
                    .as_mut()
                    .expect("running run")
                    .checkpoints
                    .push(CheckpointVisit {
                        zone_identity: zone.identity,
                        index: zone.index,
                        tick: input.tick,
                    });
                self.event(
                    input.tick,
                    Some(zone),
                    Some(instance),
                    EventKind::CheckpointAccepted,
                    &mut output.events,
                )?;
            }
            ZoneKind::MapEnd => {
                let Some(run) = self
                    .run
                    .as_ref()
                    .filter(|run| run.disposition == RunDisposition::Running)
                else {
                    self.event(
                        input.tick,
                        Some(zone),
                        None,
                        EventKind::Rejected(RejectionReason::NoRun),
                        &mut output.events,
                    )?;
                    return Ok(());
                };
                let instance = run.instance;
                if run.checkpoints.len() as u32 != self.definition.checkpoint_count {
                    self.event(
                        input.tick,
                        Some(zone),
                        Some(instance),
                        EventKind::Rejected(RejectionReason::CheckpointsMissing),
                        &mut output.events,
                    )?;
                    return Ok(());
                }
                let run = self.run.as_mut().expect("running run");
                run.disposition = RunDisposition::Completed;
                run.end_tick = Some(input.tick);
                let result = RunResult {
                    course_identity: self.definition.identity,
                    map_identity: self.definition.map_identity,
                    run_instance: run.instance,
                    player_identity: run.player_identity,
                    class: run.class,
                    team: run.team,
                    run_kind: RunKind::Map,
                    zone_index: 1,
                    disposition: RunDisposition::Completed,
                    start_tick: run.start_tick,
                    end_tick: input.tick,
                    elapsed_ticks: input.tick - run.start_tick,
                    tick_interval_bits: input.tick_interval.to_bits(),
                    checkpoints: run.checkpoints.clone(),
                };
                self.event(
                    input.tick,
                    Some(zone),
                    Some(instance),
                    EventKind::Completed,
                    &mut output.events,
                )?;
                output.result = Some(result);
            }
        }
        Ok(())
    }

    fn invalidate(
        &mut self,
        reason: InvalidationReason,
        tick: u64,
        zone: Option<Zone>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let Some(run) = self
            .run
            .as_mut()
            .filter(|run| run.disposition == RunDisposition::Running)
        else {
            return Ok(());
        };
        run.disposition = RunDisposition::Invalidated;
        run.end_tick = Some(tick);
        run.invalidation = Some(reason);
        let instance = run.instance;
        self.event(
            tick,
            zone,
            Some(instance),
            EventKind::Invalidated(reason),
            events,
        )
    }

    fn event(
        &mut self,
        tick: u64,
        zone: Option<Zone>,
        instance: Option<u32>,
        kind: EventKind,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if events.len() >= self.limits.max_events_per_tick {
            return Err(Error::new(Failure::Malformed, Field::Limit));
        }
        events.push(Event {
            sequence: self.next_event_sequence,
            tick,
            run_instance: instance
                .or_else(|| self.run.as_ref().map(|run| run.instance))
                .unwrap_or(0),
            zone_identity: zone.map(|value| value.identity),
            zone_index: zone.map(|value| value.index),
            kind,
        });
        self.next_event_sequence += 1;
        Ok(())
    }

    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PJST");
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&self.definition.identity.to_le_bytes());
        bytes.extend_from_slice(&self.current_tick.to_le_bytes());
        bytes.extend_from_slice(&self.next_run_instance.to_le_bytes());
        bytes.extend_from_slice(&self.next_event_sequence.to_le_bytes());
        bytes.extend_from_slice(&(self.touching.len() as u32).to_le_bytes());
        for identity in &self.touching {
            bytes.extend_from_slice(&identity.to_le_bytes());
        }
        match &self.run {
            None => bytes.push(0),
            Some(run) => {
                bytes.push(1);
                bytes.extend_from_slice(&run.instance.to_le_bytes());
                bytes.push(run.disposition as u8);
                bytes.push(run.class as u8);
                bytes.push(run.team as u8);
                bytes.push(run.invalidation.map_or(0, |reason| reason as u8));
                bytes.extend_from_slice(&run.player_identity.to_le_bytes());
                bytes.extend_from_slice(&run.start_tick.to_le_bytes());
                bytes.extend_from_slice(&run.end_tick.unwrap_or(u64::MAX).to_le_bytes());
                bytes.extend_from_slice(&(run.checkpoints.len() as u32).to_le_bytes());
                for visit in &run.checkpoints {
                    bytes.extend_from_slice(&visit.zone_identity.to_le_bytes());
                    bytes.extend_from_slice(&visit.index.to_le_bytes());
                    bytes.extend_from_slice(&visit.tick.to_le_bytes());
                }
            }
        }
        if bytes.len() > self.limits.max_snapshot_bytes {
            return Err(Error::new(Failure::Malformed, Field::Limit));
        }
        Ok(bytes)
    }
}

fn eligible(player: PlayerFacts) -> bool {
    player.alive && player.active && !player.noclip && matches!(player.team, Team::Red | Team::Blue)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Malformed,
    Unknown,
    Missing,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Course,
    Zone,
    Checkpoint,
    Tick,
    Contact,
    Limit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub failure: Failure,
    pub field: Field,
}

impl Error {
    const fn new(failure: Failure, field: Field) -> Self {
        Self { failure, field }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?} {:?}", self.failure, self.field)
    }
}

impl std::error::Error for Error {}

pub fn decode_course(bytes: &[u8], limits: Limits) -> Result<CourseDefinition, Error> {
    const HEADER: usize = 52;
    const RECORD: usize = 16;
    if bytes.len() < HEADER
        || &bytes[..4] != b"PJMP"
        || u32::from_le_bytes(bytes[4..8].try_into().expect("course version")) != COURSE_VERSION
    {
        return Err(Error::new(Failure::Malformed, Field::Course));
    }
    let count = u32::from_le_bytes(bytes[48..52].try_into().expect("zone count")) as usize;
    if count > limits.max_zones
        || HEADER.checked_add(
            count
                .checked_mul(RECORD)
                .ok_or_else(|| Error::new(Failure::Malformed, Field::Limit))?,
        ) != Some(bytes.len())
    {
        return Err(Error::new(Failure::Malformed, Field::Limit));
    }
    let identity = u64::from_le_bytes(bytes[8..16].try_into().expect("course identity"));
    let mut map_identity = [0; 32];
    map_identity.copy_from_slice(&bytes[16..48]);
    let mut zones = Vec::with_capacity(count);
    for index in 0..count {
        let at = HEADER + index * RECORD;
        if bytes[at + 9..at + 12] != [0, 0, 0] {
            return Err(Error::new(Failure::Malformed, Field::Zone));
        }
        zones.push(Zone {
            identity: u32::from_le_bytes(bytes[at..at + 4].try_into().expect("zone identity")),
            trigger_entity: u32::from_le_bytes(
                bytes[at + 4..at + 8].try_into().expect("trigger identity"),
            ),
            kind: match bytes[at + 8] {
                1 => ZoneKind::MapStart,
                2 => ZoneKind::Checkpoint,
                3 => ZoneKind::MapEnd,
                _ => return Err(Error::new(Failure::Unknown, Field::Zone)),
            },
            index: u32::from_le_bytes(bytes[at + 12..at + 16].try_into().expect("zone index")),
        });
    }
    CourseDefinition::linear(identity, map_identity, zones, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course() -> CourseDefinition {
        CourseDefinition::linear(
            7,
            [9; 32],
            vec![
                Zone {
                    identity: 10,
                    trigger_entity: 100,
                    kind: ZoneKind::MapStart,
                    index: 1,
                },
                Zone {
                    identity: 11,
                    trigger_entity: 101,
                    kind: ZoneKind::Checkpoint,
                    index: 1,
                },
                Zone {
                    identity: 12,
                    trigger_entity: 102,
                    kind: ZoneKind::MapEnd,
                    index: 1,
                },
            ],
            Limits::default(),
        )
        .unwrap()
    }

    fn player() -> PlayerFacts {
        PlayerFacts {
            identity: 1,
            class: Class::Soldier,
            team: Team::Red,
            alive: true,
            active: true,
            noclip: false,
            respawned: false,
            teleported: false,
        }
    }

    #[test]
    fn only_authentic_red_and_blu_players_are_course_eligible() {
        for team in [Team::Red, Team::Blue] {
            assert!(eligible(PlayerFacts { team, ..player() }));
        }
        for team in [Team::Unassigned, Team::Spectator] {
            assert!(!eligible(PlayerFacts { team, ..player() }));
        }
    }

    #[test]
    fn linear_course_requires_order_and_freezes_completion() {
        let mut session = Session::new(course(), Limits::default());
        let enter = |sequence, trigger| Contact {
            sequence,
            trigger_entity: trigger,
            kind: ContactKind::Enter,
        };
        session
            .advance(TickInput {
                tick: 4,
                tick_interval: 0.015,
                player: player(),
                contacts: &[enter(1, 100)],
                command: Command::default(),
            })
            .unwrap();
        let rejected = session
            .advance(TickInput {
                tick: 5,
                tick_interval: 0.015,
                player: player(),
                contacts: &[enter(2, 102)],
                command: Command::default(),
            })
            .unwrap();
        assert!(matches!(
            rejected.events[0].kind,
            EventKind::Rejected(RejectionReason::CheckpointsMissing)
        ));
        session
            .advance(TickInput {
                tick: 6,
                tick_interval: 0.015,
                player: player(),
                contacts: &[enter(3, 101)],
                command: Command::default(),
            })
            .unwrap();
        let complete = session
            .advance(TickInput {
                tick: 8,
                tick_interval: 0.015,
                player: player(),
                contacts: &[
                    Contact {
                        sequence: 4,
                        trigger_entity: 102,
                        kind: ContactKind::Exit,
                    },
                    enter(5, 102),
                ],
                command: Command::default(),
            })
            .unwrap();
        assert_eq!(complete.result.unwrap().elapsed_ticks, 4);
        assert_eq!(
            session.run().unwrap().disposition,
            RunDisposition::Completed
        );
    }

    #[test]
    fn reset_is_atomic_and_requests_tf2_effects() {
        let mut session = Session::new(course(), Limits::default());
        session
            .advance(TickInput {
                tick: 1,
                tick_interval: 0.015,
                player: player(),
                contacts: &[Contact {
                    sequence: 1,
                    trigger_entity: 100,
                    kind: ContactKind::Enter,
                }],
                command: Command::default(),
            })
            .unwrap();
        let before = session.snapshot_bytes().unwrap();
        assert!(
            session
                .advance(TickInput {
                    tick: 2,
                    tick_interval: f32::NAN,
                    player: player(),
                    contacts: &[],
                    command: Command { reset: true },
                })
                .is_err()
        );
        assert_eq!(session.snapshot_bytes().unwrap(), before);
        let reset = session
            .advance(TickInput {
                tick: 2,
                tick_interval: 0.015,
                player: player(),
                contacts: &[],
                command: Command { reset: true },
            })
            .unwrap();
        assert_eq!(reset.requests.len(), 2);
        assert_eq!(
            session.run().unwrap().invalidation,
            Some(InvalidationReason::Reset)
        );
    }
}
