//! Shared TF2 control point master, point and capture-area simulation.
//! Source SDK 2013: team_control_point{,_master}.cpp, trigger_area_capture.cpp,
//! and CTFGameRules' capture eligibility and linear prerequisite rules.
use playsrc_collision::Hull;
use playsrc_entity::{Entity, Graph, Variant};

use crate::{GameplayWorld, PlayerClass, PlayerTeam};

pub const AREA_THINK_SECONDS: f32 = 0.1;
pub const MASTER_THINK_SECONDS: f32 = 0.2;
pub const WIN_REASON_ALL_POINTS_CAPTURED: u8 = 1;
const TEAMS: [PlayerTeam; 2] = [PlayerTeam::Red, PlayerTeam::Blue];

fn slot(team: PlayerTeam) -> usize {
    team.source_number() as usize
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub linear: bool,
    pub scales_with_players: bool,
    pub block_style: u8,
    pub deteriorate_seconds: f32,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            linear: true,
            scales_with_players: true,
            block_style: 1,
            deteriorate_seconds: 90.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Facts {
    pub points_may_be_captured: bool,
    pub in_overtime: bool,
    pub waiting_for_players: bool,
    /// UnlockThink only unlocks in GR_STATE_RND_RUNNING.
    pub round_running: bool,
    /// KOTH master victory is gated by the owning team's clock and TimerMayExpire.
    pub koth_timer_remaining: Option<[f32; 2]>,
    pub timer_may_expire: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actor {
    pub identity: u32,
    pub team: PlayerTeam,
    pub class: PlayerClass,
    pub position: [f32; 3],
    pub hull: Hull,
    pub alive: bool,
    pub stealthed: bool,
    pub invulnerable: bool,
    pub megaheal: bool,
    pub phased: bool,
    pub control_stunned: bool,
    pub enemy_disguise: bool,
    /// Resolved add_player_capturevalue attribute, added after the class value.
    pub capture_value_bonus: i32,
}

impl Actor {
    pub fn active(
        identity: u32,
        team: PlayerTeam,
        class: PlayerClass,
        position: [f32; 3],
        hull: Hull,
    ) -> Self {
        Self {
            identity,
            team,
            class,
            position,
            hull,
            alive: true,
            stealthed: false,
            invulnerable: false,
            megaheal: false,
            phased: false,
            control_stunned: false,
            enemy_disguise: false,
            capture_value_bonus: 0,
        }
    }

    pub fn may_capture(self) -> bool {
        !self.stealthed
            && !self.invulnerable
            && !self.megaheal
            && !self.phased
            && !self.control_stunned
            && !self.enemy_disguise
    }

    pub fn capture_value(self, scales: bool) -> i32 {
        (if self.class == PlayerClass::Scout {
            if scales { 2 } else { 10 }
        } else {
            1
        }) + self.capture_value_bonus
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub identity: u32,
    pub index: usize,
    pub name: String,
    pub print_name: String,
    pub position: [f32; 3],
    pub angles: [f32; 3],
    pub owner: PlayerTeam,
    pub default_owner: PlayerTeam,
    pub locked: bool,
    pub unlock_at: Option<f32>,
    pub visible: bool,
    pub model_visible: bool,
    pub bots_ignore: bool,
    pub models: [String; 4],
    pub icons: [String; 4],
    pub overlays: [String; 4],
    pub previous: [[Option<usize>; 3]; 4],
    pub last_contested_at: f32,
    pub warn_on_cap: i32,
    pub warn_sound: String,
    pub capture_sounds: [String; 4],
    initial_locked: bool,
    next_unlock_think: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamCapture {
    pub can_cap: bool,
    pub required: i32,
    pub required_to_start: i32,
    pub spawn_adjust: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub identity: u32,
    pub point: usize,
    pub model: usize,
    pub origin: [f32; 3],
    pub bounds: Option<([f32; 3], [f32; 3])>,
    pub disabled: bool,
    pub cap_seconds: f32,
    pub teams: [TeamCapture; 4],
    pub capturing_team: PlayerTeam,
    pub team_in_zone: PlayerTeam,
    pub remaining: f32,
    pub blocked: bool,
    pub num_players: [i32; 4],
    pub touching: Vec<u32>,
    initial_disabled: bool,
    initial_teams: [TeamCapture; 4],
    initial_point: usize,
    next_think: f32,
    last_reduction: f32,
    attempt: u32,
    blockers: Vec<(u32, u32)>,
    blocked_touching: [i32; 4],
}

impl Area {
    pub fn total_time(&self, team: PlayerTeam, configuration: Configuration) -> f32 {
        if configuration.scales_with_players {
            (self.cap_seconds * 2.0) * self.teams[slot(team)].required as f32
        } else {
            self.cap_seconds
        }
    }

    pub fn progress(&self, configuration: Configuration) -> f32 {
        if !self.capturing_team.is_gameplay() {
            return 0.0;
        }
        1.0 - self.remaining / self.total_time(self.capturing_team, configuration)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Master {
    pub identity: u32,
    pub disabled: bool,
    pub restricted_winner: u8,
    pub switch_teams: bool,
    pub score_per_capture: bool,
    pub partial_capture_points_rate: f32,
    pub cap_layout: String,
    pub custom_position: [f32; 2],
    pub base_points: [Option<usize>; 4],
    initial_disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    MapOutput {
        entity: u32,
        output: &'static str,
        value: Variant,
    },
    Touch {
        point: usize,
        player: u32,
        start: bool,
    },
    OwnerChanged {
        point: usize,
        previous: PlayerTeam,
        owner: PlayerTeam,
    },
    CaptureStarted {
        point: usize,
        team: PlayerTeam,
        cappers: Vec<u32>,
    },
    CaptureBroken {
        point: usize,
    },
    Captured {
        point: usize,
        team: PlayerTeam,
        cappers: Vec<u32>,
    },
    Blocked {
        point: usize,
        player: u32,
        victim: Option<u32>,
    },
    CapperKilled {
        player: u32,
        victim: u32,
    },
    LockChanged {
        point: usize,
        locked: bool,
    },
    RespawnWaveAdjustment {
        team: PlayerTeam,
        seconds: i32,
    },
    RoundWon {
        team: PlayerTeam,
        reason: u8,
        switch_teams: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub points: Vec<Point>,
    pub areas: Vec<Area>,
    pub master: Master,
    pub configuration: Configuration,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    InvalidEntity(u32),
    MissingPoint(u32),
    UnsupportedRounds,
    TooManyPoints,
    Collision(playsrc_movement::Error),
}

#[derive(Clone, Debug)]
pub struct World {
    points: Vec<Point>,
    areas: Vec<Area>,
    master: Master,
    configuration: Configuration,
    koth: bool,
    won: bool,
    next_master_think: f32,
    facts: Facts,
}

impl World {
    pub fn from_graph(graph: &Graph) -> Result<Option<Self>, Error> {
        // Payload has its own train watcher, and is not a control-point match.
        if graph
            .entities
            .iter()
            .any(|e| class(e, b"team_train_watcher"))
        {
            return Ok(None);
        }
        let Some(master) = graph
            .entities
            .iter()
            .find(|e| class(e, b"team_control_point_master"))
        else {
            return Ok(None);
        };
        if graph
            .entities
            .iter()
            .any(|e| class(e, b"team_control_point_round"))
        {
            return Err(Error::UnsupportedRounds);
        }
        let mut definitions: Vec<_> = graph
            .entities
            .iter()
            .filter(|e| class(e, b"team_control_point") && integer(e, b"StartDisabled", 0) == 0)
            .collect();
        definitions.sort_by_key(|e| integer(e, b"point_index", 0));
        definitions.dedup_by_key(|e| integer(e, b"point_index", 0));
        if definitions.len() > 8 {
            return Err(Error::TooManyPoints);
        }
        let mut points = Vec::new();
        for (index, entity) in definitions.iter().enumerate() {
            let owner = team(integer(entity, b"point_default_owner", 0))
                .ok_or(Error::InvalidEntity(entity.index as u32))?;
            let locked = integer(entity, b"point_start_locked", 0) != 0;
            let mut previous = [[None; 3]; 4];
            for t in TEAMS {
                for p in 0..3 {
                    let name = text(
                        entity,
                        format!("team_previouspoint_{}_{p}", slot(t)).as_bytes(),
                    );
                    if !name.is_empty() {
                        previous[slot(t)][p] = definitions
                            .iter()
                            .position(|e| text(e, b"targetname") == name);
                    }
                }
            }
            points.push(Point {
                identity: entity.index as u32,
                index,
                name: text(entity, b"targetname"),
                print_name: text(entity, b"point_printname"),
                position: vector(entity, b"origin")?,
                angles: vector(entity, b"angles")?,
                owner,
                default_owner: owner,
                locked,
                unlock_at: None,
                visible: integer(entity, b"spawnflags", 0) & 1 == 0,
                model_visible: integer(entity, b"spawnflags", 0) & 2 == 0,
                bots_ignore: integer(entity, b"spawnflags", 0) & 16 != 0,
                models: std::array::from_fn(|t| text(entity, format!("team_model_{t}").as_bytes())),
                icons: std::array::from_fn(|t| text(entity, format!("team_icon_{t}").as_bytes())),
                overlays: std::array::from_fn(|t| {
                    text(entity, format!("team_overlay_{t}").as_bytes())
                }),
                capture_sounds: std::array::from_fn(|t| {
                    text(entity, format!("team_capsound_{t}").as_bytes())
                }),
                previous,
                last_contested_at: -1.0,
                warn_on_cap: integer(entity, b"point_warn_on_cap", 0),
                warn_sound: text(entity, b"point_warn_sound"),
                initial_locked: locked,
                next_unlock_think: f32::INFINITY,
            });
        }
        if points.is_empty() {
            return Err(Error::MissingPoint(master.index as u32));
        }
        let mut areas = Vec::new();
        for e in graph
            .entities
            .iter()
            .filter(|e| class(e, b"trigger_capture_area"))
        {
            let point = points
                .iter()
                .position(|p| p.name == text(e, b"area_cap_point"))
                .ok_or(Error::MissingPoint(e.index as u32))?;
            let model = text(e, b"model")
                .strip_prefix('*')
                .and_then(|v| v.parse().ok())
                .ok_or(Error::InvalidEntity(e.index as u32))?;
            let teams = std::array::from_fn(|t| TeamCapture {
                can_cap: integer(e, format!("team_cancap_{t}").as_bytes(), 0) != 0,
                required: integer(e, format!("team_numcap_{t}").as_bytes(), 1).max(1),
                required_to_start: integer(e, format!("team_startcap_{t}").as_bytes(), 1).max(1),
                spawn_adjust: integer(e, format!("team_spawn_{t}").as_bytes(), 0),
            });
            let disabled = integer(e, b"StartDisabled", 0) != 0;
            areas.push(Area {
                identity: e.index as u32,
                point,
                model,
                origin: vector(e, b"origin")?,
                bounds: None,
                disabled,
                cap_seconds: number(e, b"area_time_to_cap", 0.0),
                teams,
                capturing_team: PlayerTeam::Unassigned,
                team_in_zone: PlayerTeam::Unassigned,
                remaining: 0.0,
                blocked: false,
                num_players: [0; 4],
                touching: Vec::new(),
                initial_disabled: disabled,
                initial_teams: teams,
                initial_point: point,
                next_think: AREA_THINK_SECONDS,
                last_reduction: 0.0,
                attempt: 0,
                blockers: Vec::new(),
                blocked_touching: [0; 4],
            });
        }
        let base_points = std::array::from_fn(|t| {
            if slot(points.last().unwrap().default_owner) == t {
                Some(points.len() - 1)
            } else if slot(points[0].default_owner) == t {
                Some(0)
            } else {
                None
            }
        });
        let disabled = integer(master, b"StartDisabled", 0) != 0;
        Ok(Some(Self {
            points,
            areas,
            configuration: Configuration::default(),
            koth: graph.entities.iter().any(|e| class(e, b"tf_logic_koth")),
            won: false,
            next_master_think: 0.1,
            facts: Facts::default(),
            master: Master {
                identity: master.index as u32,
                disabled,
                initial_disabled: disabled,
                restricted_winner: integer(master, b"cpm_restrict_team_cap_win", 0) as u8,
                switch_teams: integer(master, b"switch_teams", 0) != 0,
                score_per_capture: integer(master, b"score_style", 0) != 0,
                partial_capture_points_rate: number(master, b"partial_cap_points_rate", 0.0),
                cap_layout: text(master, b"caplayout"),
                custom_position: [
                    number(master, b"custom_position_x", -1.0),
                    number(master, b"custom_position_y", -1.0),
                ],
                base_points,
            },
        }))
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn set_model_bounds(&mut self, bounds: &[playsrc_entity::ModelBounds]) {
        for area in &mut self.areas {
            area.bounds = bounds.iter().find(|b| b.model == area.model).map(|b| (b.mins, b.maxs));
        }
    }

    pub fn bot_capture_points(&self, team: PlayerTeam) -> impl Iterator<Item = &Point> {
        self.points.iter().filter(move |point| {
            (self.koth && self.points.len() == 1) || (point.owner != team && !point.bots_ignore
                && self.areas.iter().any(|a| a.point == point.index && a.teams[slot(team)].can_cap)
                && self.team_may_capture(team, point.index, self.facts.waiting_for_players))
        })
    }

    pub fn bot_defend_points(&self, team: PlayerTeam) -> impl Iterator<Item = &Point> {
        let enemy = if team == PlayerTeam::Red { PlayerTeam::Blue } else { PlayerTeam::Red };
        self.points.iter().filter(move |point| point.owner == team && !point.bots_ignore
            && self.areas.iter().any(|a| a.point == point.index && a.teams[slot(enemy)].can_cap)
            && self.team_may_capture(enemy, point.index, self.facts.waiting_for_players))
    }
    pub fn areas(&self) -> &[Area] {
        &self.areas
    }
    pub fn master(&self) -> &Master {
        &self.master
    }
    pub fn configuration(&self) -> Configuration {
        self.configuration
    }
    pub fn snapshot(&self, events: Vec<Event>) -> Snapshot {
        Snapshot {
            points: self.points.clone(),
            areas: self.areas.clone(),
            master: self.master.clone(),
            configuration: self.configuration,
            events,
        }
    }

    pub fn farthest_owned(&self, team: PlayerTeam) -> Option<usize> {
        let base = self.master.base_points[slot(team)]?;
        let end = if base == 0 { self.points.len() - 1 } else { 0 };
        let mut current = base;
        let mut farthest = base;
        while current != end {
            if self.points[current].owner != team {
                break;
            }
            farthest = current;
            if base == 0 {
                current += 1;
            } else {
                current -= 1;
            }
        }
        Some(farthest)
    }

    pub fn team_may_capture(&self, team: PlayerTeam, index: usize, waiting: bool) -> bool {
        let Some(point) = self.points.get(index) else {
            return false;
        };
        if !team.is_gameplay() {
            return false;
        }
        if !self.configuration.linear {
            return true;
        }
        let previous = point.previous[slot(team)];
        // The self-prerequisite intentionally precedes the lock/waiting checks.
        if previous[0] == Some(index) {
            return true;
        }
        if (self.koth && waiting) || point.locked {
            return false;
        }
        if previous[0].is_none() {
            let farthest = self.farthest_owned(team).map_or(-1, |i| i as i32);
            return (farthest - index as i32).abs() <= 1;
        }
        previous
            .into_iter()
            .flatten()
            .all(|p| self.points[p].owner == team)
    }

    pub fn contested(&self) -> bool {
        self.areas.iter().any(|a| a.capturing_team.is_gameplay())
    }

    pub fn reset(&mut self, now: f32, events: &mut Vec<Event>) {
        self.won = false;
        self.master.disabled = self.master.initial_disabled;
        self.next_master_think = now + 0.1;
        for p in &mut self.points {
            p.owner = p.default_owner;
            p.locked = p.initial_locked;
            p.unlock_at = None;
            p.next_unlock_think = f32::INFINITY;
            p.last_contested_at = -1.0;
            output(events, p.identity, owner_output(p.owner), Variant::Void);
        }
        for a in &mut self.areas {
            a.disabled = a.initial_disabled;
            a.teams = a.initial_teams;
            a.point = a.initial_point;
            a.capturing_team = PlayerTeam::Unassigned;
            a.team_in_zone = PlayerTeam::Unassigned;
            a.remaining = 0.0;
            a.blocked = false;
            a.num_players = [0; 4];
            a.blocked_touching = [0; 4];
            a.touching.clear();
            a.blockers.clear();
            a.attempt = 0;
            a.next_think = now + AREA_THINK_SECONDS;
        }
    }

    pub fn apply_input(
        &mut self,
        entity: u32,
        input: &[u8],
        value: &Variant,
        now: f32,
        facts: Facts,
        events: &mut Vec<Event>,
    ) -> bool {
        self.facts = facts;
        if let Some(i) = self.points.iter().position(|p| p.identity == entity) {
            if input.eq_ignore_ascii_case(b"SetOwner") {
                if let Some(owner) = team(variant_integer(value)) {
                    if facts.points_may_be_captured && self.points[i].owner != owner {
                        self.change_owner(i, owner, false, events);
                    }
                }
            } else if input.eq_ignore_ascii_case(b"SetLocked") {
                if !facts.waiting_for_players {
                    set_locked(&mut self.points[i], variant_integer(value) > 0, events);
                }
            } else if input.eq_ignore_ascii_case(b"SetUnlockTime") {
                if !facts.waiting_for_players {
                    let seconds = variant_integer(value);
                    if seconds <= 0 {
                        set_locked(&mut self.points[i], false, events);
                    } else {
                        self.points[i].unlock_at = Some(now + seconds as f32);
                        self.points[i].next_unlock_think = now + 0.1;
                    }
                }
            } else if input.eq_ignore_ascii_case(b"RoundActivate") {
                let p = &mut self.points[i];
                if p.owner.is_gameplay() {
                    output(
                        events,
                        entity,
                        if p.owner == PlayerTeam::Red {
                            "OnRoundStartOwnedByTeam1"
                        } else {
                            "OnRoundStartOwnedByTeam2"
                        },
                        Variant::Void,
                    );
                }
                set_locked(p, p.locked, events);
            } else if input.eq_ignore_ascii_case(b"ShowModel") {
                self.points[i].model_visible = true;
            } else if input.eq_ignore_ascii_case(b"HideModel") {
                self.points[i].model_visible = false;
            } else {
                return false;
            }
            return true;
        }
        if let Some(i) = self.areas.iter().position(|a| a.identity == entity) {
            let a = &mut self.areas[i];
            if input.eq_ignore_ascii_case(b"Enable") {
                a.disabled = false;
            } else if input.eq_ignore_ascii_case(b"Disable") {
                a.disabled = true;
            } else if input.eq_ignore_ascii_case(b"Toggle") {
                a.disabled = !a.disabled;
            } else if input.eq_ignore_ascii_case(b"SetTeamCanCap") {
                if let Variant::String(s) = value {
                    let values: Vec<_> = s
                        .split(|b| b.is_ascii_whitespace())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if values.len() >= 2 {
                        if let (Some(t), Ok(v)) = (
                            std::str::from_utf8(values[0])
                                .ok()
                                .and_then(|s| s.parse().ok())
                                .and_then(team),
                            std::str::from_utf8(values[1]).unwrap_or("").parse::<i32>(),
                        ) {
                            a.teams[slot(t)].can_cap = v != 0;
                        }
                    }
                }
            } else if input.eq_ignore_ascii_case(b"SetControlPoint") {
                break_capture(a, events);
                if let Variant::String(name) = value {
                    if let Some(p) = self.points.iter().position(|p| p.name.as_bytes() == name) {
                        a.point = p;
                    }
                }
            } else if input.eq_ignore_ascii_case(b"CaptureCurrentCP") {
                if a.capturing_team.is_gameplay() {
                    self.end_capture(i, &[], events);
                }
            } else if !input.eq_ignore_ascii_case(b"RoundSpawn") {
                return false;
            }
            return true;
        }
        if entity != self.master.identity {
            return false;
        }
        if input.eq_ignore_ascii_case(b"Enable") {
            self.master.disabled = false;
        } else if input.eq_ignore_ascii_case(b"Disable") {
            self.master.disabled = true;
        } else if input.eq_ignore_ascii_case(b"SetWinner")
            || input.eq_ignore_ascii_case(b"SetWinnerAndForceCaps")
        {
            if let Some(t) = team(variant_integer(value)) {
                if input.eq_ignore_ascii_case(b"SetWinnerAndForceCaps") {
                    for i in 0..self.points.len() {
                        self.change_owner(i, t, false, events);
                    }
                }
                self.win(t, events);
            }
        } else if input.eq_ignore_ascii_case(b"SetCapLayout") {
            if let Variant::String(s) = value {
                self.master.cap_layout = String::from_utf8_lossy(s).into_owned();
            }
        } else if input.eq_ignore_ascii_case(b"RoundSpawn") {
            self.next_master_think = now + 0.1;
        } else if !input.eq_ignore_ascii_case(b"RoundActivate") {
            return false;
        }
        true
    }

    pub fn step<W: GameplayWorld>(
        &mut self,
        now: f32,
        facts: Facts,
        actors: &[Actor],
        collision: &W,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        self.facts = facts;
        for point in &mut self.points {
            if now >= point.next_unlock_think {
                if point
                    .unlock_at
                    .is_some_and(|deadline| deadline > 0.0 && deadline < now)
                    && facts.round_running
                {
                    set_locked(point, false, events);
                } else {
                    point.next_unlock_think = now + 0.1;
                }
            }
        }
        for i in 0..self.areas.len() {
            // StartTouch runs CaptureThink immediately. EndTouch removes the contact but
            // does not run CaptureThink. Keep that ordering separate from the 10Hz think.
            let mut contacts = Vec::new();
            if !self.areas[i].disabled {
                for actor in actors {
                    if collision
                        .overlaps_model_hull(
                            self.areas[i].model,
                            self.areas[i].origin,
                            actor.position,
                            actor.hull,
                        )
                        .map_err(Error::Collision)?
                    {
                        contacts.push(actor.identity);
                    }
                }
            }
            let old = self.areas[i].touching.clone();
            for id in old {
                if !contacts.contains(&id) {
                    self.areas[i].touching.retain(|v| *v != id);
                    events.push(Event::Touch {
                        point: self.areas[i].point,
                        player: id,
                        start: false,
                    });
                }
            }
            for id in contacts {
                if !self.areas[i].touching.contains(&id) {
                    self.areas[i].touching.push(id);
                    events.push(Event::Touch {
                        point: self.areas[i].point,
                        player: id,
                        start: true,
                    });
                    self.think_area(i, now, facts, actors, true, events);
                }
            }
            if now >= self.areas[i].next_think {
                self.think_area(i, now, facts, actors, false, events);
            }
        }
        if now >= self.next_master_think {
            self.next_master_think = now + MASTER_THINK_SECONDS;
            if facts.points_may_be_captured {
                self.check_win(events);
            }
        }
        Ok(())
    }

    fn think_area(
        &mut self,
        index: usize,
        now: f32,
        facts: Facts,
        actors: &[Actor],
        start_touch: bool,
        events: &mut Vec<Event>,
    ) {
        self.areas[index].next_think = now + AREA_THINK_SECONDS;
        if !facts.points_may_be_captured {
            break_capture(&mut self.areas[index], events);
            return;
        }
        let point = self.areas[index].point;
        let may = std::array::from_fn::<_, 4, _>(|t| {
            team(t as i32)
                .is_some_and(|t| self.team_may_capture(t, point, facts.waiting_for_players))
        });
        let owner = self.points[point].owner;
        let area = &mut self.areas[index];
        let mut counts = [0; 4];
        let mut blockers = [0; 4];
        let mut first = [None; 4];
        for id in &area.touching {
            let Some(actor) = actors.iter().find(|a| a.identity == *id) else {
                continue;
            };
            let t = slot(actor.team);
            if !actor.alive || !actor.team.is_gameplay() || !may[t] {
                continue;
            }
            if actor.may_capture() {
                first[t].get_or_insert(actor.identity);
                counts[t] += actor.capture_value(self.configuration.scales_with_players);
            } else if actor.invulnerable {
                first[t].get_or_insert(actor.identity);
                blockers[t] += actor.capture_value(self.configuration.scales_with_players);
            }
        }
        let mut teams_in_zone = 0;
        let mut zone = PlayerTeam::Unassigned;
        for t in TEAMS {
            if counts[slot(t)] != 0 {
                teams_in_zone += 1;
                zone = t;
            }
        }
        if teams_in_zone > 1 {
            zone = PlayerTeam::Unassigned;
        } else {
            for t in TEAMS {
                if blockers[slot(t)] != 0 && zone != t {
                    teams_in_zone += 1;
                }
            }
        }
        area.team_in_zone = zone;
        area.blocked_touching = counts;
        let blocked = self.configuration.block_style == 1
            && area.capturing_team.is_gameplay()
            && teams_in_zone > 1;
        let changed = area.num_players != counts || (blocked && counts != [0; 4]);
        if blocked {
            counts = [0; 4];
        }
        area.num_players = counts;
        if changed && area.capturing_team.is_gameplay() {
            num_cappers(area, counts[slot(area.capturing_team)], blocked, events);
        }
        if !area.capturing_team.is_gameplay() {
            if teams_in_zone > 0 {
                for t in TEAMS {
                    let config = area.teams[slot(t)];
                    if !config.can_cap
                        || owner == t
                        || counts[slot(t)] == 0
                        || counts[slot(t)] < config.required_to_start
                        || (!self.configuration.scales_with_players
                            && counts[slot(t)] < config.required)
                    {
                        continue;
                    }
                    output(
                        events,
                        area.identity,
                        if t == PlayerTeam::Red {
                            "OnStartTeam1"
                        } else {
                            "OnStartTeam2"
                        },
                        Variant::Void,
                    );
                    output(events, area.identity, "OnStartCap", Variant::Void);
                    area.capturing_team = t;
                    area.remaining = area.total_time(t, self.configuration);
                    area.blocked = false;
                    area.last_reduction = now;
                    num_cappers(area, counts[slot(t)], false, events);
                    events.push(Event::CaptureStarted {
                        point,
                        team: t,
                        cappers: cappers(area, t, actors),
                    });
                    break;
                }
            }
            return;
        }
        self.points[point].last_contested_at = now;
        let delta = now - area.last_reduction;
        let mut reduction = delta;
        if self.configuration.scales_with_players {
            for n in 1..counts[slot(zone)] {
                reduction += delta / (n + 1) as f32;
            }
        }
        area.last_reduction = now;
        if teams_in_zone > 1 {
            area.blocked = true;
            if area.remaining / area.total_time(area.capturing_team, self.configuration) <= 0.5 {
                if let Some(player) = TEAMS
                    .into_iter()
                    .filter(|t| *t != area.capturing_team)
                    .find_map(|t| first[slot(t)])
                {
                    area.blockers.retain(|(id, attempt)| {
                        *attempt == area.attempt && area.touching.contains(id)
                    });
                    if !area.blockers.contains(&(player, area.attempt)) {
                        area.blockers.push((player, area.attempt));
                        events.push(Event::Blocked {
                            point,
                            player,
                            victim: None,
                        });
                    }
                }
            }
            if self.configuration.block_style == 0 {
                break_capture(area, events);
            }
            return;
        }
        area.blocked = false;
        let total = area.total_time(area.capturing_team, self.configuration);
        if area.capturing_team == zone {
            area.remaining -= reduction;
        } else if owner == PlayerTeam::Unassigned && zone != PlayerTeam::Unassigned {
            area.remaining += reduction;
        } else if may[slot(area.capturing_team)] {
            let scale = if self.configuration.scales_with_players {
                self.configuration.deteriorate_seconds
            } else {
                total
            };
            let mut decrease = total / scale * delta;
            if facts.in_overtime {
                decrease *= 6.0;
            }
            area.remaining += decrease;
        } else {
            area.remaining = total;
        }
        if area.remaining <= 0.0 {
            self.end_capture(index, actors, events);
        } else if !start_touch && area.remaining >= total {
            break_capture(area, events);
        }
    }

    fn end_capture(&mut self, index: usize, actors: &[Actor], events: &mut Vec<Event>) {
        let area = &mut self.areas[index];
        let team = area.capturing_team;
        let point = area.point;
        let cappers = cappers(area, team, actors);
        area.attempt += 1;
        area.blockers.clear();
        output(
            events,
            area.identity,
            if team == PlayerTeam::Red {
                "OnCapTeam1"
            } else {
                "OnCapTeam2"
            },
            Variant::Void,
        );
        output(events, area.identity, "OnEndCap", Variant::Void);
        let previous = self.points[point].owner;
        if previous.is_gameplay() && area.teams[slot(previous)].spawn_adjust != 0 {
            events.push(Event::RespawnWaveAdjustment {
                team: previous,
                seconds: -area.teams[slot(previous)].spawn_adjust,
            });
        }
        if area.teams[slot(team)].spawn_adjust != 0 {
            events.push(Event::RespawnWaveAdjustment {
                team,
                seconds: area.teams[slot(team)].spawn_adjust,
            });
        }
        area.capturing_team = PlayerTeam::Unassigned;
        area.remaining = 0.0;
        self.change_owner(point, team, true, events);
        events.push(Event::Captured {
            point,
            team,
            cappers,
        });
        num_cappers(&self.areas[index], 0, false, events);
    }

    fn change_owner(
        &mut self,
        index: usize,
        owner: PlayerTeam,
        real_capture: bool,
        events: &mut Vec<Event>,
    ) {
        let point = &mut self.points[index];
        let previous = point.owner;
        point.owner = owner;
        output(events, point.identity, owner_output(owner), Variant::Void);
        if real_capture && owner.is_gameplay() {
            output(
                events,
                point.identity,
                if owner == PlayerTeam::Red {
                    "OnOwnerChangedToTeam1"
                } else {
                    "OnOwnerChangedToTeam2"
                },
                Variant::Void,
            );
        }
        events.push(Event::OwnerChanged {
            point: index,
            previous,
            owner,
        });
        self.check_win(events);
    }

    fn check_win(&mut self, events: &mut Vec<Event>) {
        if self.won || self.master.disabled || self.master.restricted_winner == 1 {
            return;
        }
        let team = self.points[0].owner;
        if self.koth && team.is_gameplay() {
            if let Some(timers) = self.facts.koth_timer_remaining {
                if timers[slot(team) - 2] > 0.0 || !self.facts.timer_may_expire {
                    return;
                }
            }
        }
        if team.is_gameplay()
            && slot(team) != self.master.restricted_winner as usize
            && self.points.iter().all(|p| p.owner == team)
        {
            self.win(team, events);
        }
    }

    fn win(&mut self, team: PlayerTeam, events: &mut Vec<Event>) {
        if self.won {
            return;
        }
        self.won = true;
        events.push(Event::RoundWon {
            team,
            reason: WIN_REASON_ALL_POINTS_CAPTURED,
            switch_teams: self.master.switch_teams,
        });
        if team.is_gameplay() {
            output(
                events,
                self.master.identity,
                if team == PlayerTeam::Red {
                    "OnWonByTeam1"
                } else {
                    "OnWonByTeam2"
                },
                Variant::Void,
            );
        }
    }
}

fn cappers(area: &Area, team: PlayerTeam, actors: &[Actor]) -> Vec<u32> {
    // GetNumCappingPlayers uses client index order and touching/team, not eligibility.
    let mut ids: Vec<_> = actors
        .iter()
        .filter(|a| a.team == team && area.touching.contains(&a.identity))
        .map(|a| a.identity)
        .collect();
    ids.sort_unstable();
    ids.truncate(7);
    ids
}

fn break_capture(area: &mut Area, events: &mut Vec<Event>) {
    if !area.capturing_team.is_gameplay() {
        return;
    }
    output(
        events,
        area.identity,
        if area.capturing_team == PlayerTeam::Red {
            "OnBreakTeam1"
        } else {
            "OnBreakTeam2"
        },
        Variant::Void,
    );
    output(events, area.identity, "OnBreakCap", Variant::Void);
    area.capturing_team = PlayerTeam::Unassigned;
    area.remaining = 0.0;
    events.push(Event::CaptureBroken { point: area.point });
    num_cappers(area, 0, false, events);
}

fn num_cappers(area: &Area, count: i32, blocked: bool, events: &mut Vec<Event>) {
    output(
        events,
        area.identity,
        "OnNumCappersChanged",
        Variant::Integer(count),
    );
    output(
        events,
        area.identity,
        "OnNumCappersChanged2",
        Variant::Integer(if blocked { -1 } else { count }),
    );
}

fn set_locked(point: &mut Point, locked: bool, events: &mut Vec<Event>) {
    if point.locked != locked {
        events.push(Event::LockChanged {
            point: point.index,
            locked,
        });
    }
    point.locked = locked;
    point.unlock_at = None;
    if !locked {
        point.next_unlock_think = f32::INFINITY;
        output(events, point.identity, "OnUnlocked", Variant::Void);
    }
}

fn output(events: &mut Vec<Event>, entity: u32, output: &'static str, value: Variant) {
    events.push(Event::MapOutput {
        entity,
        output,
        value,
    });
}
fn owner_output(team: PlayerTeam) -> &'static str {
    match team {
        PlayerTeam::Red => "OnCapTeam1",
        PlayerTeam::Blue => "OnCapTeam2",
        _ => "OnCapReset",
    }
}
fn class(e: &Entity, name: &[u8]) -> bool {
    e.classname
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case(name))
}
fn text(e: &Entity, key: &[u8]) -> String {
    e.pairs
        .iter()
        .find(|p| p.key.eq_ignore_ascii_case(key))
        .map_or_else(String::new, |p| {
            String::from_utf8_lossy(&p.value).into_owned()
        })
}
fn integer(e: &Entity, key: &[u8], default: i32) -> i32 {
    text(e, key).parse().unwrap_or(default)
}
fn number(e: &Entity, key: &[u8], default: f32) -> f32 {
    text(e, key).parse().unwrap_or(default)
}
fn vector(e: &Entity, key: &[u8]) -> Result<[f32; 3], Error> {
    let s = text(e, key);
    if s.is_empty() {
        return Ok([0.0; 3]);
    }
    let numbers: Vec<_> = s.split_whitespace().map(str::parse::<f32>).collect();
    if numbers.len() != 3
        || numbers
            .iter()
            .any(|v| !v.as_ref().is_ok_and(|v| v.is_finite()))
    {
        return Err(Error::InvalidEntity(e.index as u32));
    }
    Ok([
        *numbers[0].as_ref().unwrap(),
        *numbers[1].as_ref().unwrap(),
        *numbers[2].as_ref().unwrap(),
    ])
}
fn team(value: i32) -> Option<PlayerTeam> {
    match value {
        0 => Some(PlayerTeam::Unassigned),
        2 => Some(PlayerTeam::Red),
        3 => Some(PlayerTeam::Blue),
        _ => None,
    }
}
fn variant_integer(value: &Variant) -> i32 {
    match value {
        Variant::Integer(v) => *v,
        Variant::String(v) => String::from_utf8_lossy(v).parse().unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(koth: bool) -> World {
        let mut entities = String::from("{\"classname\" \"team_control_point_master\"}");
        if koth {
            entities.push_str("{\"classname\" \"tf_logic_koth\"}");
        }
        for i in 0..if koth { 1 } else { 5 } {
            let owner = if koth || i == 2 {
                0
            } else if i < 2 {
                3
            } else {
                2
            };
            entities.push_str(&format!(r#"{{"classname" "team_control_point" "targetname" "cp{i}" "point_index" "{}" "point_default_owner" "{owner}"}}
                {{"classname" "trigger_capture_area" "area_cap_point" "cp{i}" "model" "*{}" "area_time_to_cap" "10" "team_cancap_2" "1" "team_cancap_3" "1"}}"#, i+1, i+1));
        }
        let graph =
            playsrc_entity::parse(entities.as_bytes(), playsrc_entity::Limits::default()).unwrap();
        World::from_graph(&graph).unwrap().unwrap()
    }
    fn facts() -> Facts {
        Facts {
            points_may_be_captured: true,
            round_running: true,
            timer_may_expire: true,
            ..Facts::default()
        }
    }
    fn actor(id: u32, team: PlayerTeam, class: PlayerClass) -> Actor {
        Actor::active(
            id,
            team,
            class,
            [0.0; 3],
            Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
        )
    }
    fn think(w: &mut World, point: usize, now: f32, actors: &[Actor], start: bool) -> Vec<Event> {
        w.facts = facts();
        w.areas[point].touching = actors.iter().map(|a| a.identity).collect();
        let mut events = Vec::new();
        w.think_area(point, now, facts(), actors, start, &mut events);
        events
    }

    #[test]
    fn master_remaps_authored_indices_and_requires_contiguous_front() {
        let mut w = world(false);
        assert_eq!(
            w.points.iter().map(|p| p.index).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        assert_eq!(w.master.base_points[2], Some(4));
        assert_eq!(w.master.base_points[3], Some(0));
        assert!(w.team_may_capture(PlayerTeam::Red, 2, false));
        assert!(!w.team_may_capture(PlayerTeam::Red, 1, false));
        w.points[2].owner = PlayerTeam::Red;
        assert!(w.team_may_capture(PlayerTeam::Red, 1, false));
        assert!(!w.team_may_capture(PlayerTeam::Red, 0, false));
        w.points[1].owner = PlayerTeam::Red;
        assert!(w.team_may_capture(PlayerTeam::Red, 0, false));
    }

    #[test]
    fn explicit_prerequisites_all_required_self_overrides_lock_and_waiting() {
        let mut w = world(true);
        w.points[0].locked = true;
        assert!(!w.team_may_capture(PlayerTeam::Red, 0, false));
        w.points[0].previous[2][0] = Some(0);
        assert!(w.team_may_capture(PlayerTeam::Red, 0, true));
        let mut w = world(false);
        w.points[0].previous[2] = [Some(3), Some(2), None];
        assert!(!w.team_may_capture(PlayerTeam::Red, 0, false));
        w.points[2].owner = PlayerTeam::Red;
        assert!(w.team_may_capture(PlayerTeam::Red, 0, false));
    }

    #[test]
    fn scout_harmonic_rate_not_linear_multiplier_and_immediate_starttouch() {
        let mut w = world(false);
        let scout = actor(1, PlayerTeam::Red, PlayerClass::Scout);
        let events = think(&mut w, 2, 1.0, &[scout], true);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::CaptureStarted { .. }))
        );
        assert_eq!(w.areas[2].remaining, 20.0);
        think(&mut w, 2, 2.0, &[scout], false);
        assert_eq!(w.areas[2].remaining, 18.5);
        let soldier = actor(2, PlayerTeam::Red, PlayerClass::Soldier);
        think(&mut w, 2, 2.0, &[scout, soldier], true);
        assert_eq!(w.areas[2].remaining, 18.5);
        think(&mut w, 2, 3.0, &[scout, soldier], false);
        assert!((w.areas[2].remaining - (18.5 - 1.0 - 0.5 - 1.0 / 3.0)).abs() < 0.00001);
        let same_time = think(&mut world(false), 2, 1.0, &[scout, soldier], true);
        assert!(
            !same_time
                .iter()
                .any(|e| matches!(e, Event::CaptureBroken { .. }))
        );
    }

    #[test]
    fn eligibility_invulnerability_blocks_but_other_exclusions_do_not() {
        let base = actor(1, PlayerTeam::Blue, PlayerClass::Spy);
        for excluded in [
            Actor {
                stealthed: true,
                ..base
            },
            Actor {
                megaheal: true,
                ..base
            },
            Actor {
                phased: true,
                ..base
            },
            Actor {
                control_stunned: true,
                ..base
            },
            Actor {
                enemy_disguise: true,
                ..base
            },
        ] {
            assert!(!excluded.may_capture());
            let mut w = world(false);
            let red = actor(2, PlayerTeam::Red, PlayerClass::Soldier);
            think(&mut w, 2, 0.0, &[red], true);
            think(&mut w, 2, 1.0, &[red, excluded], false);
            assert!(!w.areas[2].blocked);
            assert_eq!(w.areas[2].remaining, 19.0);
        }
        let mut w = world(false);
        let red = actor(2, PlayerTeam::Red, PlayerClass::Soldier);
        think(&mut w, 2, 0.0, &[red], true);
        think(
            &mut w,
            2,
            1.0,
            &[
                red,
                Actor {
                    invulnerable: true,
                    ..base
                },
            ],
            false,
        );
        assert!(w.areas[2].blocked);
        assert_eq!(w.areas[2].remaining, 20.0);
        assert_eq!(w.areas[2].num_players, [0; 4]);
    }

    #[test]
    fn blocking_freezes_decay_and_credits_each_blocker_once_per_attempt() {
        let mut w = world(false);
        let red = actor(1, PlayerTeam::Red, PlayerClass::Soldier);
        let blue = actor(2, PlayerTeam::Blue, PlayerClass::Soldier);
        think(&mut w, 2, 0.0, &[red], true);
        think(&mut w, 2, 11.0, &[red], false);
        let events = think(&mut w, 2, 12.0, &[red, blue], true);
        assert_eq!(w.areas[2].remaining, 9.0);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Blocked { .. }))
                .count(),
            1
        );
        let events = think(&mut w, 2, 13.0, &[red, blue], false);
        assert!(!events.iter().any(|e| matches!(e, Event::Blocked { .. })));
        think(&mut w, 2, 14.0, &[red], false);
        assert_eq!(w.areas[2].remaining, 8.0);
    }

    #[test]
    fn neutral_enemy_reversal_uses_harmonic_rate_empty_decay_uses_90_seconds() {
        let mut w = world(false);
        let red = actor(1, PlayerTeam::Red, PlayerClass::Soldier);
        let blue = actor(2, PlayerTeam::Blue, PlayerClass::Scout);
        think(&mut w, 2, 0.0, &[red], true);
        think(&mut w, 2, 10.0, &[red], false);
        think(&mut w, 2, 11.0, &[blue], false);
        assert_eq!(w.areas[2].remaining, 11.5);
        think(&mut w, 2, 20.0, &[], false);
        assert_eq!(w.areas[2].remaining, 13.5);
        w.think_area(
            2,
            29.0,
            Facts {
                in_overtime: true,
                ..facts()
            },
            &[],
            false,
            &mut Vec::new(),
        );
        assert_eq!(w.areas[2].capturing_team, PlayerTeam::Unassigned);
    }

    #[test]
    fn cap_emits_authored_outputs_before_ownership_and_cappers_use_client_order() {
        let mut w = world(false);
        let red = actor(3, PlayerTeam::Red, PlayerClass::Soldier);
        let spy = Actor {
            enemy_disguise: true,
            ..actor(1, PlayerTeam::Red, PlayerClass::Spy)
        };
        think(&mut w, 2, 0.0, &[red, spy], true);
        let events = think(&mut w, 2, 20.0, &[red, spy], false);
        assert_eq!(w.points[2].owner, PlayerTeam::Red);
        assert_eq!(w.areas[2].capturing_team, PlayerTeam::Unassigned);
        assert!(matches!(
            &events[0],
            Event::MapOutput {
                output: "OnCapTeam1",
                ..
            }
        ));
        assert!(matches!(
            &events[1],
            Event::MapOutput {
                output: "OnEndCap",
                ..
            }
        ));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Captured { cappers, .. } if cappers == &[1,3]))
        );
    }

    #[test]
    fn input_restart_locks_and_master_koth_gate_have_one_owner() {
        let mut w = world(true);
        let id = w.points[0].identity;
        let mut events = Vec::new();
        w.apply_input(
            id,
            b"SetLocked",
            &Variant::Integer(1),
            0.0,
            Facts {
                waiting_for_players: true,
                ..facts()
            },
            &mut events,
        );
        assert!(!w.points[0].locked);
        w.apply_input(
            id,
            b"SetLocked",
            &Variant::Integer(1),
            0.0,
            facts(),
            &mut events,
        );
        w.apply_input(
            id,
            b"SetUnlockTime",
            &Variant::Integer(5),
            1.0,
            facts(),
            &mut events,
        );
        assert_eq!(w.points[0].unlock_at, Some(6.0));
        w.apply_input(
            id,
            b"SetLocked",
            &Variant::Integer(0),
            2.0,
            facts(),
            &mut events,
        );
        assert_eq!(w.points[0].unlock_at, None);
        let koth = Facts {
            koth_timer_remaining: Some([100.0, 100.0]),
            ..facts()
        };
        w.apply_input(
            id,
            b"SetOwner",
            &Variant::Integer(2),
            2.0,
            koth,
            &mut events,
        );
        assert!(!events.iter().any(|e| matches!(e, Event::RoundWon { .. })));
        w.facts = Facts {
            koth_timer_remaining: Some([0.0, 100.0]),
            timer_may_expire: false,
            ..facts()
        };
        w.check_win(&mut events);
        assert!(!w.won);
        w.facts.timer_may_expire = true;
        w.check_win(&mut events);
        assert!(w.won);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::MapOutput {
                output: "OnWonByTeam1",
                ..
            }
        )));
        w.reset(10.0, &mut events);
        assert!(!w.won);
        assert_eq!(w.points[0].owner, PlayerTeam::Unassigned);
        assert!(!w.contested());
    }
}
