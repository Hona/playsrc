use std::collections::{BTreeMap, BTreeSet};

use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph};

use crate::{GameplayWorld, PlayerTeam};

pub const DEFAULT_RETURN_SECONDS: u16 = 60;
pub const DEFAULT_CAPTURES_PER_ROUND: u16 = 3;
pub const DEFAULT_CAPTURE_BONUS_SECONDS: f32 = 10.0;
pub const FLAG_THINK_SECONDS: f32 = 0.25;
pub const OWNER_PICKUP_DELAY_SECONDS: f32 = 3.0;
pub const FLAG_TRIGGER_EXPANSION: f32 = 24.0;
pub const FLAG_MODEL: &str = "models/flag/briefcase.mdl";
pub const FLAG_ICON: &str = "../hud/objectives_flagpanel_carried";
pub const FLAG_PAPER_EFFECT: &str = "player_intel_papertrail";
pub const FLAG_TRAIL_EFFECT: &str = "flagtrail";
pub const WIN_REASON_FLAG_CAPTURE_LIMIT: u8 = 3;
pub const CRIT_BOOSTED_CTF_CAPTURE: u8 = 39;
pub const FLAG_COLLISION_HULL: Hull = Hull {
    mins: [-19.5, -22.5, -6.5],
    maxs: [19.5, 22.5, 6.5],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FlagStatus {
    Home = 0,
    Stolen = 1,
    Dropped = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FlagEventKind {
    Pickup = 1,
    Capture = 2,
    Defend = 3,
    Dropped = 4,
    Returned = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Notification {
    YourFlagTaken = 0,
    YourFlagDropped = 1,
    YourFlagReturned = 2,
    YourFlagCaptured = 3,
    EnemyFlagTaken = 4,
    EnemyFlagDropped = 5,
    EnemyFlagReturned = 6,
    EnemyFlagCaptured = 7,
    TouchingEnemyCapture = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AnnouncerSound {
    EnemyStolen = 1,
    EnemyDropped = 2,
    EnemyCaptured = 3,
    EnemyReturned = 4,
    TeamStolen = 5,
    TeamDropped = 6,
    TeamCaptured = 7,
    TeamReturned = 8,
    FlagSpawn = 9,
}

impl AnnouncerSound {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::EnemyStolen => "CaptureFlag.EnemyStolen",
            Self::EnemyDropped => "CaptureFlag.EnemyDropped",
            Self::EnemyCaptured => "CaptureFlag.EnemyCaptured",
            Self::EnemyReturned => "CaptureFlag.EnemyReturned",
            Self::TeamStolen => "CaptureFlag.TeamStolen",
            Self::TeamDropped => "CaptureFlag.TeamDropped",
            Self::TeamCaptured => "CaptureFlag.TeamCaptured",
            Self::TeamReturned => "CaptureFlag.TeamReturned",
            Self::FlagSpawn => "CaptureFlag.FlagSpawn",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub captures_per_round: u16,
    pub return_on_touch: bool,
    pub return_time_credit_factor: f32,
    pub capture_bonus_seconds: f32,
    pub waiting_for_players: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuleConfiguration {
    pub captures_per_round: u16,
    pub return_on_touch: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            captures_per_round: DEFAULT_CAPTURES_PER_ROUND,
            return_on_touch: false,
            return_time_credit_factor: 1.0,
            capture_bonus_seconds: DEFAULT_CAPTURE_BONUS_SECONDS,
            waiting_for_players: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flag {
    pub identity: u32,
    pub team: PlayerTeam,
    pub status: FlagStatus,
    pub disabled: bool,
    pub visible_when_disabled: bool,
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub home: [f32; 3],
    pub home_angles: [f32; 3],
    pub carrier: Option<u32>,
    pub previous_carrier: Option<u32>,
    pub initial_carrier: Option<u32>,
    pub return_deadline: Option<f32>,
    pub maximum_return_seconds: f32,
    pub configured_return_seconds: u16,
    pub shot_clock: bool,
    pub allow_owner_pickup: bool,
    pub owner_pickup_deadline: Option<f32>,
    pub model: String,
    pub icon: String,
    pub paper_effect: String,
    pub trail_effect: String,
    pub trail_enabled: bool,
    pub skin: u8,
    pub captured: bool,
    last_pickup_time: f32,
    last_reset_duration: f32,
    next_think: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureZone {
    pub identity: u32,
    pub team: Option<PlayerTeam>,
    pub model: usize,
    pub origin: [f32; 3],
    pub center: [f32; 3],
    pub capture_point: i32,
    pub disabled: bool,
    next_enemy_warning: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actor {
    pub identity: u32,
    pub team: PlayerTeam,
    pub position: [f32; 3],
    pub hull: Hull,
    pub alive: bool,
    pub allowed_to_pick_up: bool,
    pub invulnerable: bool,
    pub stealthed: bool,
    pub invisibility: f32,
    pub selected_to_teleport: bool,
    pub phased: bool,
    pub in_respawn_room: bool,
}

impl Actor {
    pub fn active(identity: u32, team: PlayerTeam, position: [f32; 3], hull: Hull) -> Self {
        Self {
            identity,
            team,
            position,
            hull,
            alive: true,
            allowed_to_pick_up: true,
            invulnerable: false,
            stealthed: false,
            invisibility: 0.0,
            selected_to_teleport: false,
            phased: false,
            in_respawn_room: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scores {
    pub red_captures: u16,
    pub blue_captures: u16,
    pub red_score: u16,
    pub blue_score: u16,
    pub limit: u16,
    pub winner: Option<PlayerTeam>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    StatusChanged {
        flag: u32,
        status: FlagStatus,
        owner: Option<u32>,
    },
    Flag {
        flag: u32,
        kind: FlagEventKind,
        player: Option<u32>,
        team: PlayerTeam,
        priority: u8,
        home: Option<bool>,
    },
    Announcer {
        flag: u32,
        recipient: PlayerTeam,
        sound: AnnouncerSound,
        exclude_player: Option<u32>,
    },
    Notification {
        flag: u32,
        recipient: PlayerTeam,
        notification: Notification,
        exclude_player: Option<u32>,
    },
    MapOutput {
        entity: u32,
        output: &'static str,
        activator: Option<u32>,
    },
    Captured {
        zone: u32,
        player: u32,
        team: PlayerTeam,
        team_score: u16,
    },
    CaptureBonus {
        team: PlayerTeam,
        condition: u8,
        duration: f32,
    },
    RoundWon {
        team: PlayerTeam,
        reason: u8,
        capture_limit: u16,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub flags: Vec<Flag>,
    pub zones: Vec<CaptureZone>,
    pub scores: Scores,
    pub events: Vec<Event>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotObjective {
    pub flag: u32,
    pub position: [f32; 3],
    pub carrier: Option<u32>,
    pub capture_zone: Option<u32>,
    pub capture_position: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidEntity(u32),
    DuplicateFlag(PlayerTeam),
    DuplicateZone(u32),
    InvalidConfiguration,
    MissingFlag(u32),
    Collision,
}

#[derive(Clone, Debug)]
pub struct World {
    flags: BTreeMap<u32, Flag>,
    zones: BTreeMap<u32, CaptureZone>,
    configuration: Configuration,
    scores: Scores,
    now: f32,
}

impl World {
    pub fn compile(graph: &Graph, configuration: Configuration) -> Result<Option<Self>, Error> {
        validate_configuration(configuration)?;
        let mut flags = BTreeMap::new();
        let mut zones = BTreeMap::new();
        let mut teams = BTreeSet::new();
        for entity in &graph.entities {
            let identity =
                u32::try_from(entity.index).map_err(|_| Error::InvalidEntity(u32::MAX))?;
            if class(entity, b"item_teamflag") {
                if integer(entity, b"GameType", 0) != 0 {
                    continue;
                }
                let team = team(entity).ok_or(Error::InvalidEntity(identity))?;
                if !teams.insert(team) {
                    return Err(Error::DuplicateFlag(team));
                }
                let home = vector(entity, b"origin", None).ok_or(Error::InvalidEntity(identity))?;
                let angles = vector(entity, b"angles", Some([0.0; 3]))
                    .ok_or(Error::InvalidEntity(identity))?;
                let seconds = integer(entity, b"ReturnTime", i32::from(DEFAULT_RETURN_SECONDS));
                let configured_return_seconds =
                    u16::try_from(seconds).map_err(|_| Error::InvalidEntity(identity))?;
                let flag = Flag {
                    identity,
                    team,
                    status: FlagStatus::Home,
                    disabled: boolean(entity, b"StartDisabled", false),
                    visible_when_disabled: boolean(entity, b"VisibleWhenDisabled", false),
                    position: home,
                    angles,
                    home,
                    home_angles: angles,
                    carrier: None,
                    previous_carrier: None,
                    initial_carrier: None,
                    return_deadline: None,
                    maximum_return_seconds: 0.0,
                    configured_return_seconds,
                    shot_clock: boolean(entity, b"ShotClockMode", false),
                    allow_owner_pickup: true,
                    owner_pickup_deadline: None,
                    model: string(entity, b"flag_model", FLAG_MODEL),
                    icon: string(entity, b"flag_icon", FLAG_ICON),
                    paper_effect: string(entity, b"flag_paper", FLAG_PAPER_EFFECT),
                    trail_effect: string(entity, b"flag_trail", FLAG_TRAIL_EFFECT),
                    trail_enabled: integer(entity, b"trail_effect", 1) != 0,
                    skin: team.source_number() - 2,
                    captured: false,
                    last_pickup_time: 0.0,
                    last_reset_duration: 0.0,
                    next_think: 0.0,
                };
                flags.insert(identity, flag);
            } else if class(entity, b"func_capturezone") {
                let zone = CaptureZone {
                    identity,
                    team: optional_team(entity).ok_or(Error::InvalidEntity(identity))?,
                    model: entity
                        .bsp_model_index
                        .ok_or(Error::InvalidEntity(identity))?,
                    origin: vector(entity, b"origin", Some([0.0; 3]))
                        .ok_or(Error::InvalidEntity(identity))?,
                    center: vector(entity, b"origin", Some([0.0; 3]))
                        .ok_or(Error::InvalidEntity(identity))?,
                    capture_point: integer(entity, b"CapturePoint", 0),
                    disabled: boolean(entity, b"StartDisabled", false),
                    next_enemy_warning: -1.0,
                };
                if zones.insert(identity, zone).is_some() {
                    return Err(Error::DuplicateZone(identity));
                }
            }
        }
        if flags.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            flags,
            zones,
            configuration,
            scores: Scores {
                red_captures: 0,
                blue_captures: 0,
                red_score: 0,
                blue_score: 0,
                limit: configuration.captures_per_round,
                winner: None,
            },
            now: 0.0,
        }))
    }

    pub fn set_model_bounds(&mut self, bounds: &[playsrc_entity::ModelBounds]) {
        for zone in self.zones.values_mut() {
            if let Some(model) = bounds
                .iter()
                .find(|candidate| candidate.model == zone.model)
            {
                zone.center = std::array::from_fn(|axis| {
                    zone.origin[axis] + (model.mins[axis] + model.maxs[axis]) * 0.5
                });
            }
        }
    }

    pub fn configuration(&self) -> Configuration {
        self.configuration
    }

    pub fn configure(&mut self, value: Configuration) -> Result<(), Error> {
        validate_configuration(value)?;
        self.configuration = value;
        self.scores.limit = value.captures_per_round;
        Ok(())
    }

    pub fn flags(&self) -> impl Iterator<Item = &Flag> {
        self.flags.values()
    }

    pub fn flag(&self, identity: u32) -> Option<&Flag> {
        self.flags.get(&identity)
    }

    pub fn zones(&self) -> impl Iterator<Item = &CaptureZone> {
        self.zones.values()
    }

    pub fn scores(&self) -> Scores {
        self.scores
    }

    pub fn set_round_scores(&mut self, red: u16, blue: u16) {
        if self.configuration.captures_per_round > 0 {
            self.scores.red_score = red;
            self.scores.blue_score = blue;
        }
    }

    pub fn reset_round(&mut self, red_score: u16, blue_score: u16) {
        self.scores.red_captures = 0;
        self.scores.blue_captures = 0;
        self.scores.red_score = red_score;
        self.scores.blue_score = blue_score;
        self.scores.winner = None;
        for flag in self.flags.values_mut() {
            let mut events = Vec::new();
            reset(flag, &mut events);
            flag.captured = false;
            flag.allow_owner_pickup = true;
            flag.owner_pickup_deadline = None;
        }
    }

    pub fn carrier_flag(&self, actor: u32) -> Option<&Flag> {
        self.flags.values().find(|flag| flag.carrier == Some(actor))
    }

    pub fn bot_objective(&self, actor: u32, team: PlayerTeam) -> Option<BotObjective> {
        let flag = self.carrier_flag(actor).or_else(|| {
            self.flags
                .values()
                .find(|flag| flag.team.is_enemy(team) && !flag.disabled)
        })?;
        let zone = self
            .zones
            .values()
            .find(|zone| !zone.disabled && (zone.team.is_none() || zone.team == Some(team)));
        Some(BotObjective {
            flag: flag.identity,
            position: flag.position,
            carrier: flag.carrier,
            capture_zone: zone.map(|value| value.identity),
            capture_position: zone.map(|value| value.center),
        })
    }

    pub fn snapshot(&self, events: Vec<Event>) -> Snapshot {
        Snapshot {
            flags: self.flags.values().cloned().collect(),
            zones: self.zones.values().copied().collect(),
            scores: self.scores,
            events,
        }
    }

    pub fn advance<W: GameplayWorld>(
        &mut self,
        collision: &W,
        now: f32,
        actors: &[Actor],
    ) -> Result<Vec<Event>, Error> {
        if !now.is_finite() || now < self.now || actors.iter().any(invalid_actor) {
            return Err(Error::InvalidConfiguration);
        }
        self.now = now;
        let mut events = Vec::new();
        let missing = self
            .flags
            .values()
            .filter_map(|flag| {
                let carrier = flag.carrier?;
                let actor = actors.iter().find(|actor| actor.identity == carrier);
                if actor.is_some_and(|actor| actor.alive) {
                    None
                } else {
                    Some((
                        carrier,
                        if flag.team == PlayerTeam::Red {
                            PlayerTeam::Blue
                        } else {
                            PlayerTeam::Red
                        },
                        actor.map_or(flag.position, |actor| actor.position),
                        actor.map_or(
                            Hull {
                                mins: [-24.0, -24.0, 0.0],
                                maxs: [24.0, 24.0, 82.0],
                            },
                            |actor| actor.hull,
                        ),
                    ))
                }
            })
            .collect::<Vec<_>>();
        for (identity, team, position, hull) in missing {
            let center = [
                position[0],
                position[1],
                position[2] + (hull.mins[2] + hull.maxs[2]) * 0.5,
            ];
            let trace = collision
                .trace(
                    center,
                    [center[0], center[1], center[2] - 8_000.0],
                    FLAG_COLLISION_HULL,
                    0x0200_400b,
                )
                .map_err(|_| Error::Collision)?;
            events.extend(self.drop(
                Actor::active(identity, team, position, hull),
                if trace.start_solid { center } else { trace.end },
                false,
                false,
            )?);
        }
        for flag in self.flags.values_mut() {
            if let Some(carrier) = flag.carrier.and_then(|identity| {
                actors
                    .iter()
                    .find(|actor| actor.identity == identity && actor.alive)
            }) {
                flag.position = carrier.position;
            }
        }
        let identities: Vec<_> = self.flags.keys().copied().collect();
        for identity in identities {
            self.think(identity, &mut events);
        }
        if self.scores.winner.is_some() || self.configuration.waiting_for_players {
            return Ok(events);
        }
        for actor in actors
            .iter()
            .filter(|actor| actor.alive && actor.team.is_gameplay())
        {
            for identity in self.flags.keys().copied().collect::<Vec<_>>() {
                let Some(flag) = self.flags.get(&identity) else {
                    continue;
                };
                if flag.status == FlagStatus::Stolen || flag.disabled || flag.captured {
                    continue;
                }
                if touches_flag(actor, flag.position) {
                    self.touch_flag(identity, *actor, &mut events)?;
                }
            }
            for identity in self.zones.keys().copied().collect::<Vec<_>>() {
                let zone = self.zones[&identity];
                if zone.disabled || self.carrier_flag(actor.identity).is_none() {
                    continue;
                }
                let overlap = collision
                    .overlaps_model_hull(zone.model, zone.origin, actor.position, actor.hull)
                    .map_err(|_| Error::Collision)?;
                if overlap {
                    self.touch_zone(identity, *actor, &mut events)?;
                }
            }
        }
        self.check_round_win(&mut events);
        Ok(events)
    }

    pub fn drop(
        &mut self,
        actor: Actor,
        position: [f32; 3],
        thrown: bool,
        silent: bool,
    ) -> Result<Vec<Event>, Error> {
        if invalid_actor(&actor) || position.into_iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidConfiguration);
        }
        let Some(identity) = self.carrier_flag(actor.identity).map(|flag| flag.identity) else {
            return Ok(Vec::new());
        };
        let mut events = Vec::new();
        let flag = self
            .flags
            .get_mut(&identity)
            .expect("existing carried flag");
        let team = flag.team;
        if thrown {
            flag.allow_owner_pickup = false;
            flag.owner_pickup_deadline = Some(self.now + OWNER_PICKUP_DELAY_SECONDS);
        }
        flag.position = position;
        flag.angles = flag.home_angles;
        flag.carrier = None;
        flag.skin = flag.team.source_number() - 2;
        if !silent {
            announce_drop(flag.identity, team, actor.team, &mut events);
        }
        let maximum = f32::from(flag.configured_return_seconds);
        let duration = if flag.shot_clock {
            let credit = (self.now - flag.last_pickup_time)
                * self.configuration.return_time_credit_factor
                + flag.last_reset_duration;
            maximum.min(credit.round_ties_even().max(0.0))
        } else {
            maximum
        };
        flag.return_deadline = Some(self.now + duration);
        flag.maximum_return_seconds = maximum;
        set_status(flag, FlagStatus::Dropped, None, &mut events);
        output(&mut events, identity, "OnDrop", None);
        output(&mut events, identity, "OnDrop1", Some(actor.identity));
        events.push(Event::Flag {
            flag: identity,
            kind: FlagEventKind::Dropped,
            player: Some(actor.identity),
            team,
            priority: 8,
            home: None,
        });
        Ok(events)
    }

    pub fn set_flag_disabled(&mut self, identity: u32, disabled: bool) -> Result<(), Error> {
        let flag = self
            .flags
            .get_mut(&identity)
            .ok_or(Error::MissingFlag(identity))?;
        flag.disabled = disabled;
        if !disabled {
            flag.next_think = self.now;
        }
        Ok(())
    }

    pub fn set_zone_disabled(&mut self, identity: u32, disabled: bool) -> Result<(), Error> {
        self.zones
            .get_mut(&identity)
            .ok_or(Error::MissingFlag(identity))?
            .disabled = disabled;
        Ok(())
    }

    fn think(&mut self, identity: u32, events: &mut Vec<Event>) {
        let flag = self.flags.get_mut(&identity).expect("known flag");
        if flag.disabled || self.now < flag.next_think {
            return;
        }
        flag.next_think = self.now + FLAG_THINK_SECONDS;
        if self.scores.winner.is_some() {
            return;
        }
        if flag.captured {
            flag.captured = false;
        }
        if flag.status != FlagStatus::Dropped {
            return;
        }
        if !flag.allow_owner_pickup
            && flag
                .owner_pickup_deadline
                .is_some_and(|deadline| self.now > deadline)
        {
            flag.allow_owner_pickup = true;
        }
        if flag
            .return_deadline
            .is_some_and(|deadline| self.now > deadline)
        {
            reset(flag, events);
            announce_return(flag, events);
        }
    }

    fn touch_flag(
        &mut self,
        identity: u32,
        actor: Actor,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let carrying = self.carrier_flag(actor.identity).is_some();
        let flag = self
            .flags
            .get_mut(&identity)
            .ok_or(Error::MissingFlag(identity))?;
        if !flag.allow_owner_pickup && flag.previous_carrier == Some(actor.identity) {
            return Ok(());
        }
        if flag.team == actor.team {
            output(events, identity, "OnTouchSameTeam", None);
            if !self.configuration.return_on_touch || flag.status != FlagStatus::Dropped {
                return Ok(());
            }
        }
        if !actor.allowed_to_pick_up
            || actor.selected_to_teleport
            || actor.invulnerable
            || actor.stealthed
            || actor.invisibility > 0.25
            || actor.phased
            || actor.in_respawn_room
            || carrying && !self.configuration.return_on_touch
        {
            return Ok(());
        }
        if flag.team == actor.team {
            reset(flag, events);
            announce_return(flag, events);
            return Ok(());
        }
        let was_home = flag.status == FlagStatus::Home;
        flag.last_reset_duration = if was_home {
            f32::from(flag.configured_return_seconds)
        } else {
            flag.return_deadline
                .map_or(0.0, |deadline| deadline - self.now)
        };
        flag.last_pickup_time = self.now;
        flag.carrier = Some(actor.identity);
        flag.previous_carrier = Some(actor.identity);
        flag.initial_carrier.get_or_insert(actor.identity);
        flag.allow_owner_pickup = true;
        flag.owner_pickup_deadline = None;
        flag.position = actor.position;
        flag.skin = flag.team.source_number() - 2 + 3;
        announce_pickup(flag.identity, flag.team, actor, events);
        set_status(flag, FlagStatus::Stolen, Some(actor.identity), events);
        flag.return_deadline = None;
        flag.maximum_return_seconds = 0.0;
        events.push(Event::Flag {
            flag: identity,
            kind: FlagEventKind::Pickup,
            player: Some(actor.identity),
            team: flag.team,
            priority: 8,
            home: Some(was_home),
        });
        output(events, identity, "OnPickUp", None);
        output(events, identity, "OnPickup1", Some(actor.identity));
        output(
            events,
            identity,
            if actor.team == PlayerTeam::Red {
                "OnPickupTeam1"
            } else {
                "OnPickupTeam2"
            },
            None,
        );
        Ok(())
    }

    fn touch_zone(
        &mut self,
        identity: u32,
        actor: Actor,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let zone = self
            .zones
            .get_mut(&identity)
            .ok_or(Error::MissingFlag(identity))?;
        if zone.team.is_some_and(|team| team != actor.team) {
            if zone.next_enemy_warning < self.now {
                events.push(Event::Notification {
                    flag: identity,
                    recipient: actor.team,
                    notification: Notification::TouchingEnemyCapture,
                    exclude_player: None,
                });
                zone.next_enemy_warning = self.now + 5.0;
            }
            return Ok(());
        }
        if self.configuration.return_on_touch
            && self.flags.values().any(|flag| {
                flag.team == actor.team && !flag.disabled && flag.status != FlagStatus::Home
            })
        {
            return Ok(());
        }
        let flag_identity = self
            .carrier_flag(actor.identity)
            .map(|flag| flag.identity)
            .ok_or(Error::MissingFlag(identity))?;
        let captures = if actor.team == PlayerTeam::Red {
            self.scores.red_captures
        } else {
            self.scores.blue_captures
        };
        let winning_capture = self.configuration.captures_per_round > 0
            && self
                .configuration
                .captures_per_round
                .saturating_sub(captures)
                <= 1;
        if !winning_capture {
            announce_capture(flag_identity, actor.team, events);
            if self.configuration.capture_bonus_seconds > 0.0 {
                events.push(Event::CaptureBonus {
                    team: actor.team,
                    condition: CRIT_BOOSTED_CTF_CAPTURE,
                    duration: self.configuration.capture_bonus_seconds,
                });
            }
        }
        if self.configuration.captures_per_round > 0 {
            let value = if actor.team == PlayerTeam::Red {
                &mut self.scores.red_captures
            } else {
                &mut self.scores.blue_captures
            };
            *value = value.saturating_add(1);
        } else {
            let value = if actor.team == PlayerTeam::Red {
                &mut self.scores.red_score
            } else {
                &mut self.scores.blue_score
            };
            *value = value.saturating_add(1);
        }
        let team_score = if self.configuration.captures_per_round > 0 {
            if actor.team == PlayerTeam::Red {
                self.scores.red_captures
            } else {
                self.scores.blue_captures
            }
        } else if actor.team == PlayerTeam::Red {
            self.scores.red_score
        } else {
            self.scores.blue_score
        };
        let flag = self
            .flags
            .get_mut(&flag_identity)
            .expect("known carried flag");
        events.push(Event::Flag {
            flag: flag_identity,
            kind: FlagEventKind::Capture,
            player: Some(actor.identity),
            team: flag.team,
            priority: 9,
            home: None,
        });
        reset(flag, events);
        output(events, flag_identity, "OnCapture", None);
        output(events, flag_identity, "OnCapture1", Some(actor.identity));
        output(
            events,
            flag_identity,
            if actor.team == PlayerTeam::Red {
                "OnCapTeam1"
            } else {
                "OnCapTeam2"
            },
            None,
        );
        flag.captured = true;
        flag.next_think = self.now + FLAG_THINK_SECONDS;
        output(events, identity, "OnCapture", None);
        output(
            events,
            identity,
            if actor.team == PlayerTeam::Red {
                "OnCapTeam1"
            } else {
                "OnCapTeam2"
            },
            None,
        );
        events.push(Event::Captured {
            zone: identity,
            player: actor.identity,
            team: actor.team,
            team_score,
        });
        Ok(())
    }

    fn check_round_win(&mut self, events: &mut Vec<Event>) {
        if self.configuration.captures_per_round == 0 || self.scores.winner.is_some() {
            return;
        }
        let red = self.scores.red_captures;
        let blue = self.scores.blue_captures;
        if red < self.configuration.captures_per_round
            && blue < self.configuration.captures_per_round
        {
            return;
        }
        let team = if blue > red {
            PlayerTeam::Blue
        } else {
            PlayerTeam::Red
        };
        self.scores.winner = Some(team);
        events.push(Event::RoundWon {
            team,
            reason: WIN_REASON_FLAG_CAPTURE_LIMIT,
            capture_limit: self.configuration.captures_per_round,
        });
    }
}

fn validate_configuration(value: Configuration) -> Result<(), Error> {
    if !value.return_time_credit_factor.is_finite()
        || value.return_time_credit_factor < 0.0
        || !value.capture_bonus_seconds.is_finite()
        || value.capture_bonus_seconds < 0.0
    {
        return Err(Error::InvalidConfiguration);
    }
    Ok(())
}

fn invalid_actor(actor: &Actor) -> bool {
    actor.identity == 0
        || actor
            .position
            .into_iter()
            .chain(actor.hull.mins)
            .chain(actor.hull.maxs)
            .chain([actor.invisibility])
            .any(|value| !value.is_finite())
        || actor.invisibility < 0.0
        || actor.invisibility > 1.0
        || actor
            .hull
            .mins
            .into_iter()
            .zip(actor.hull.maxs)
            .any(|(minimum, maximum)| minimum > maximum)
}

fn touches_flag(actor: &Actor, position: [f32; 3]) -> bool {
    (0..3).all(|axis| {
        let player_minimum = actor.position[axis] + actor.hull.mins[axis];
        let player_maximum = actor.position[axis] + actor.hull.maxs[axis];
        let flag_minimum = position[axis] + FLAG_COLLISION_HULL.mins[axis]
            - if axis < 2 {
                FLAG_TRIGGER_EXPANSION
            } else {
                0.0
            };
        let flag_maximum = position[axis]
            + FLAG_COLLISION_HULL.maxs[axis]
            + if axis < 2 {
                FLAG_TRIGGER_EXPANSION
            } else {
                FLAG_TRIGGER_EXPANSION * 0.5
            };
        player_minimum <= flag_maximum && player_maximum >= flag_minimum
    })
}

fn set_status(flag: &mut Flag, status: FlagStatus, owner: Option<u32>, events: &mut Vec<Event>) {
    if flag.status != status {
        flag.status = status;
        events.push(Event::StatusChanged {
            flag: flag.identity,
            status,
            owner,
        });
    }
}

fn reset(flag: &mut Flag, events: &mut Vec<Event>) {
    flag.position = flag.home;
    flag.angles = flag.home_angles;
    flag.carrier = None;
    flag.previous_carrier = None;
    flag.initial_carrier = None;
    flag.return_deadline = None;
    flag.maximum_return_seconds = 0.0;
    flag.allow_owner_pickup = true;
    flag.owner_pickup_deadline = None;
    flag.skin = flag.team.source_number() - 2;
    set_status(flag, FlagStatus::Home, None, events);
}

fn output(events: &mut Vec<Event>, entity: u32, name: &'static str, activator: Option<u32>) {
    events.push(Event::MapOutput {
        entity,
        output: name,
        activator,
    });
}

fn announce_pickup(flag: u32, flag_team: PlayerTeam, actor: Actor, events: &mut Vec<Event>) {
    for recipient in [PlayerTeam::Red, PlayerTeam::Blue] {
        let defender = recipient != actor.team;
        events.push(Event::Announcer {
            flag,
            recipient,
            sound: if defender {
                AnnouncerSound::EnemyStolen
            } else {
                AnnouncerSound::TeamStolen
            },
            exclude_player: None,
        });
        events.push(Event::Notification {
            flag,
            recipient,
            notification: if defender {
                Notification::YourFlagTaken
            } else {
                Notification::EnemyFlagTaken
            },
            exclude_player: (!defender).then_some(actor.identity),
        });
    }
    debug_assert!(flag_team.is_enemy(actor.team));
}

fn announce_drop(
    flag: u32,
    flag_team: PlayerTeam,
    carrier_team: PlayerTeam,
    events: &mut Vec<Event>,
) {
    for recipient in [PlayerTeam::Red, PlayerTeam::Blue] {
        let defender = recipient != carrier_team;
        events.push(Event::Announcer {
            flag,
            recipient,
            sound: if defender {
                AnnouncerSound::EnemyDropped
            } else {
                AnnouncerSound::TeamDropped
            },
            exclude_player: None,
        });
        events.push(Event::Notification {
            flag,
            recipient,
            notification: if defender {
                Notification::YourFlagDropped
            } else {
                Notification::EnemyFlagDropped
            },
            exclude_player: None,
        });
    }
    debug_assert!(flag_team.is_enemy(carrier_team));
}

fn announce_return(flag: &Flag, events: &mut Vec<Event>) {
    for recipient in [PlayerTeam::Red, PlayerTeam::Blue] {
        let owner = recipient == flag.team;
        events.push(Event::Announcer {
            flag: flag.identity,
            recipient,
            sound: if owner {
                AnnouncerSound::EnemyReturned
            } else {
                AnnouncerSound::TeamReturned
            },
            exclude_player: None,
        });
        events.push(Event::Notification {
            flag: flag.identity,
            recipient,
            notification: if owner {
                Notification::YourFlagReturned
            } else {
                Notification::EnemyFlagReturned
            },
            exclude_player: None,
        });
    }
    events.push(Event::Flag {
        flag: flag.identity,
        kind: FlagEventKind::Returned,
        player: None,
        team: flag.team,
        priority: 8,
        home: None,
    });
    events.push(Event::Announcer {
        flag: flag.identity,
        recipient: PlayerTeam::Unassigned,
        sound: AnnouncerSound::FlagSpawn,
        exclude_player: None,
    });
    output(events, flag.identity, "OnReturn", None);
}

fn announce_capture(flag: u32, scoring_team: PlayerTeam, events: &mut Vec<Event>) {
    for recipient in [PlayerTeam::Red, PlayerTeam::Blue] {
        let defender = recipient != scoring_team;
        events.push(Event::Announcer {
            flag,
            recipient,
            sound: if defender {
                AnnouncerSound::EnemyCaptured
            } else {
                AnnouncerSound::TeamCaptured
            },
            exclude_player: None,
        });
        events.push(Event::Notification {
            flag,
            recipient,
            notification: if defender {
                Notification::YourFlagCaptured
            } else {
                Notification::EnemyFlagCaptured
            },
            exclude_player: None,
        });
    }
}

fn class(entity: &Entity, expected: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn field<'a>(entity: &'a Entity, name: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_slice())
}

fn integer(entity: &Entity, name: &[u8], default: i32) -> i32 {
    field(entity, name).map_or(default, |bytes| {
        playsrc_keyvalues::NumericValue::Bytes(bytes).get_int()
    })
}

fn boolean(entity: &Entity, name: &[u8], default: bool) -> bool {
    field(entity, name).map_or(default, |bytes| {
        playsrc_keyvalues::NumericValue::Bytes(bytes).get_bool()
    })
}

fn string(entity: &Entity, name: &[u8], default: &str) -> String {
    field(entity, name)
        .filter(|value| !value.is_empty())
        .and_then(|value| std::str::from_utf8(value).ok())
        .unwrap_or(default)
        .to_owned()
}

fn team(entity: &Entity) -> Option<PlayerTeam> {
    match integer(entity, b"TeamNum", 0) {
        2 => Some(PlayerTeam::Red),
        3 => Some(PlayerTeam::Blue),
        _ => None,
    }
}

fn optional_team(entity: &Entity) -> Option<Option<PlayerTeam>> {
    match integer(entity, b"TeamNum", 0) {
        0 => Some(None),
        2 => Some(Some(PlayerTeam::Red)),
        3 => Some(Some(PlayerTeam::Blue)),
        _ => None,
    }
}

fn vector(entity: &Entity, name: &[u8], default: Option<[f32; 3]>) -> Option<[f32; 3]> {
    let Some(bytes) = field(entity, name) else {
        return default;
    };
    let values: Vec<_> = std::str::from_utf8(bytes)
        .ok()?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<_, _>>()
        .ok()?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    Some([values[0], values[1], values[2]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::{Error as MoveError, Trace, Tracer};

    #[derive(Clone)]
    struct Collision;

    impl Tracer for Collision {
        fn trace(
            &self,
            _start: [f32; 3],
            end: [f32; 3],
            _hull: Hull,
            _mask: u32,
        ) -> Result<Trace, MoveError> {
            Ok(Trace {
                fraction: 1.0,
                end,
                normal: None,
                start_solid: false,
                all_solid: false,
                hit: None,
                contents: 0,
            })
        }
    }

    impl GameplayWorld for Collision {
        fn overlaps_model_hull(
            &self,
            model: usize,
            _origin: [f32; 3],
            position: [f32; 3],
            _hull: Hull,
        ) -> Result<bool, MoveError> {
            Ok((position[0] - if model == 25 { -500.0 } else { 500.0 }).abs() <= 32.0)
        }
    }

    fn graph() -> Graph {
        playsrc_entity::parse(
            br#"{"classname""func_capturezone""TeamNum""2""model""*25""CapturePoint""1"}{"classname""item_teamflag""TeamNum""2""origin""-500 200 8""angles""0 120 0""ReturnTime""60""GameType""0""flag_model""models/flag/briefcase.mdl"}{"classname""func_capturezone""TeamNum""3""model""*111""CapturePoint""1"}{"classname""item_teamflag""TeamNum""3""origin""500 -200 8""angles""0 300 0""ReturnTime""60""GameType""0"}"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap()
    }

    fn actor(identity: u32, team: PlayerTeam, position: [f32; 3]) -> Actor {
        Actor::active(
            identity,
            team,
            position,
            Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
        )
    }

    #[test]
    fn compiles_authored_two_team_briefcases_and_capture_brushes() {
        let world = World::compile(&graph(), Configuration::default())
            .unwrap()
            .unwrap();
        assert_eq!(world.flags().count(), 2);
        assert_eq!(world.zones().count(), 2);
        let red = world.flag(1).unwrap();
        assert_eq!(red.status, FlagStatus::Home);
        assert_eq!(red.home, [-500.0, 200.0, 8.0]);
        assert_eq!(red.angles, [0.0, 120.0, 0.0]);
        assert_eq!(red.configured_return_seconds, 60);
        assert_eq!(red.model, FLAG_MODEL);
        assert_eq!(red.icon, FLAG_ICON);
        assert_eq!(red.skin, 0);
        assert_eq!(world.flag(3).unwrap().skin, 1);
        assert_eq!(world.scores().limit, 3);
    }

    #[test]
    fn pickup_drop_owner_delay_and_strict_think_return_match_source_order() {
        let mut world = World::compile(&graph(), Configuration::default())
            .unwrap()
            .unwrap();
        let blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
        let pickup = world.advance(&Collision, 1.0, &[blue]).unwrap();
        let red = world.flag(1).unwrap();
        assert_eq!(red.status, FlagStatus::Stolen);
        assert_eq!(red.carrier, Some(7));
        assert_eq!(red.skin, 3);
        assert!(matches!(
            pickup[0],
            Event::Announcer {
                recipient: PlayerTeam::Red,
                sound: AnnouncerSound::EnemyStolen,
                ..
            }
        ));
        assert!(matches!(
            pickup[1],
            Event::Notification {
                notification: Notification::YourFlagTaken,
                ..
            }
        ));
        assert!(pickup.iter().any(|event| matches!(
            event,
            Event::Flag {
                kind: FlagEventKind::Pickup,
                home: Some(true),
                ..
            }
        )));
        let drop = world.drop(blue, [-300.0, 200.0, 8.0], true, false).unwrap();
        let red = world.flag(1).unwrap();
        assert_eq!(red.status, FlagStatus::Dropped);
        assert_eq!(red.return_deadline, Some(61.0));
        assert!(!red.allow_owner_pickup);
        assert_eq!(red.owner_pickup_deadline, Some(4.0));
        assert!(matches!(
            drop.last(),
            Some(Event::Flag {
                kind: FlagEventKind::Dropped,
                ..
            })
        ));
        let former = actor(7, PlayerTeam::Blue, [-300.0, 200.0, 8.0]);
        world.advance(&Collision, 4.0, &[former]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Dropped);
        world.advance(&Collision, 4.255, &[former]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Stolen);
        world
            .drop(former, [-300.0, 200.0, 8.0], false, false)
            .unwrap();
        world.advance(&Collision, 64.255, &[]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Dropped);
        let returned = world.advance(&Collision, 64.51, &[]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Home);
        assert_eq!(world.flag(1).unwrap().position, [-500.0, 200.0, 8.0]);
        assert!(matches!(
            returned[0],
            Event::StatusChanged {
                status: FlagStatus::Home,
                ..
            }
        ));
        assert!(returned.iter().any(|event| matches!(
            event,
            Event::Flag {
                kind: FlagEventKind::Returned,
                priority: 8,
                ..
            }
        )));
        assert!(returned.iter().any(|event| matches!(
            event,
            Event::Announcer {
                sound: AnnouncerSound::FlagSpawn,
                ..
            }
        )));
    }

    #[test]
    fn own_flag_touch_does_not_return_by_default_and_opt_in_blocks_capture() {
        let mut world = World::compile(&graph(), Configuration::default())
            .unwrap()
            .unwrap();
        let blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
        world.advance(&Collision, 1.0, &[blue]).unwrap();
        world
            .drop(blue, [-300.0, 200.0, 8.0], false, false)
            .unwrap();
        let red = actor(8, PlayerTeam::Red, [-300.0, 200.0, 8.0]);
        let touched = world.advance(&Collision, 1.1, &[red]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Dropped);
        assert!(touched.iter().any(|event| matches!(
            event,
            Event::MapOutput {
                output: "OnTouchSameTeam",
                ..
            }
        )));
        world
            .configure(Configuration {
                return_on_touch: true,
                ..Configuration::default()
            })
            .unwrap();
        let returned = world.advance(&Collision, 1.2, &[red]).unwrap();
        assert_eq!(world.flag(1).unwrap().status, FlagStatus::Home);
        assert!(returned.iter().any(|event| matches!(
            event,
            Event::Flag {
                kind: FlagEventKind::Returned,
                ..
            }
        )));
    }

    #[test]
    fn three_captures_win_and_final_capture_suppresses_announcer_and_bonus() {
        let mut world = World::compile(&graph(), Configuration::default())
            .unwrap()
            .unwrap();
        for capture in 1..=3 {
            let now = capture as f32;
            let blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
            world.advance(&Collision, now, &[blue]).unwrap();
            let base = actor(7, PlayerTeam::Blue, [500.0, 0.0, 8.0]);
            let events = world.advance(&Collision, now + 0.1, &[base]).unwrap();
            assert_eq!(world.scores().blue_captures, capture);
            assert_eq!(world.flag(1).unwrap().status, FlagStatus::Home);
            assert_eq!(events.iter().any(|event| matches!(event, Event::CaptureBonus { team: PlayerTeam::Blue, duration, .. } if *duration == 10.0)), capture != 3);
            assert_eq!(
                events.iter().any(|event| matches!(
                    event,
                    Event::Announcer {
                        sound: AnnouncerSound::TeamCaptured,
                        ..
                    }
                )),
                capture != 3
            );
            assert!(events.iter().any(|event| matches!(event, Event::Captured { team: PlayerTeam::Blue, team_score, .. } if *team_score == capture)));
            if capture == 3 {
                assert!(matches!(
                    events.last(),
                    Some(Event::RoundWon {
                        team: PlayerTeam::Blue,
                        reason: 3,
                        capture_limit: 3
                    })
                ));
            }
        }
        assert_eq!(world.scores().winner, Some(PlayerTeam::Blue));
    }

    #[test]
    fn zero_limit_scores_without_round_win() {
        let mut world = World::compile(
            &graph(),
            Configuration {
                captures_per_round: 0,
                ..Configuration::default()
            },
        )
        .unwrap()
        .unwrap();
        let blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
        world.advance(&Collision, 1.0, &[blue]).unwrap();
        let events = world
            .advance(
                &Collision,
                1.1,
                &[actor(7, PlayerTeam::Blue, [500.0, 0.0, 8.0])],
            )
            .unwrap();
        assert_eq!(world.scores().blue_captures, 0);
        assert_eq!(world.scores().blue_score, 1);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::RoundWon { .. }))
        );
    }

    #[test]
    fn shot_clock_carries_remaining_time_and_credits_carried_seconds() {
        let graph = playsrc_entity::parse(
            br#"{"classname""item_teamflag""TeamNum""2""origin""0 0 0""ReturnTime""60""ShotClockMode""1"}"#,
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let mut world = World::compile(&graph, Configuration::default())
            .unwrap()
            .unwrap();
        let blue = actor(7, PlayerTeam::Blue, [0.0, 0.0, 0.0]);
        world.advance(&Collision, 1.0, &[blue]).unwrap();
        world.drop(blue, [100.0, 0.0, 0.0], false, false).unwrap();
        world.advance(&Collision, 21.0, &[]).unwrap();
        let other = actor(8, PlayerTeam::Blue, [100.0, 0.0, 0.0]);
        world.advance(&Collision, 21.1, &[other]).unwrap();
        world.advance(&Collision, 31.1, &[]).unwrap();
        world.drop(other, [200.0, 0.0, 0.0], false, false).unwrap();
        let flag = world.flag(0).unwrap();
        assert_eq!(flag.return_deadline, Some(81.1));
        assert_eq!(flag.maximum_return_seconds, 60.0);
    }

    #[test]
    fn trigger_bounds_expand_xy_but_never_below_the_authored_collision_hull() {
        let position = [0.0, 0.0, 0.0];
        let mut player = actor(7, PlayerTeam::Blue, [67.5, 0.0, 0.0]);
        assert!(touches_flag(&player, position));
        player.position[0] = 67.501;
        assert!(!touches_flag(&player, position));
        player.position = [0.0, 70.5, 0.0];
        assert!(touches_flag(&player, position));
        player.position[1] = 70.501;
        assert!(!touches_flag(&player, position));
        player.position = [0.0, 0.0, -88.5];
        assert!(touches_flag(&player, position));
        player.position[2] = -88.501;
        assert!(!touches_flag(&player, position));
        player.position[2] = 18.5;
        assert!(touches_flag(&player, position));
        player.position[2] = 18.501;
        assert!(!touches_flag(&player, position));
    }

    #[test]
    fn pickup_rejects_invulnerability_stealth_phase_respawn_and_friendly_home() {
        let cases: [fn(&mut Actor); 7] = [
            |actor| actor.allowed_to_pick_up = false,
            |actor| actor.invulnerable = true,
            |actor| actor.stealthed = true,
            |actor| actor.invisibility = 0.251,
            |actor| actor.selected_to_teleport = true,
            |actor| actor.phased = true,
            |actor| actor.in_respawn_room = true,
        ];
        for modify in cases {
            let mut world = World::compile(&graph(), Configuration::default())
                .unwrap()
                .unwrap();
            let mut blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
            modify(&mut blue);
            world.advance(&Collision, 1.0, &[blue]).unwrap();
            assert_eq!(world.flag(1).unwrap().status, FlagStatus::Home);
        }
    }

    #[test]
    fn bot_objective_switches_from_enemy_briefcase_to_capture_zone_when_carrying() {
        let mut world = World::compile(&graph(), Configuration::default())
            .unwrap()
            .unwrap();
        let initial = world.bot_objective(7, PlayerTeam::Blue).unwrap();
        assert_eq!(initial.flag, 1);
        assert_eq!(initial.position, [-500.0, 200.0, 8.0]);
        assert_eq!(initial.capture_zone, Some(2));
        let blue = actor(7, PlayerTeam::Blue, [-500.0, 200.0, 8.0]);
        world.advance(&Collision, 1.0, &[blue]).unwrap();
        assert_eq!(
            world.bot_objective(7, PlayerTeam::Blue).unwrap().carrier,
            Some(7)
        );
    }
}
