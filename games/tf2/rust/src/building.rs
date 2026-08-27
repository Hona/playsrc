use playsrc_collision::Hull;
use playsrc_movement::Tracer;

use crate::{PlayerClass, PlayerLifecycle, PlayerTeam, bot};

pub const MAX_BUILDINGS: usize = 4;
pub const MAX_METAL: u16 = 200;
pub const UPGRADE_METAL: u16 = 200;
pub const UPGRADE_PER_HIT: u16 = 25;
pub const CONSTRUCTION_INTERVAL: f32 = 0.1;
pub const STARTING_HEALTH: f32 = 0.1;
pub const CONSTRUCTION_HIT_DURATION: f32 = 1.0;
pub const CONSTRUCTION_HIT_VALUE: f32 = 1.5;
pub const WRENCH_REPAIR: u16 = 100;
pub const REPAIR_HEALTH_PER_METAL: u16 = 3;
pub const SENTRY_RANGE: f32 = 1100.0;
pub const SENTRY_DAMAGE: i32 = 16;
const MASK_SOLID: u32 = playsrc_collision::MASK_SOLID;
const MASK_BRUSH: u32 = 0x0001_400b;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Kind {
    Dispenser = 0,
    Teleporter = 1,
    Sentry = 2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Mode {
    Entrance = 0,
    Exit = 1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Object {
    pub kind: Kind,
    pub mode: Mode,
}

impl Object {
    pub const SENTRY: Self = Self {
        kind: Kind::Sentry,
        mode: Mode::Entrance,
    };
    pub const DISPENSER: Self = Self {
        kind: Kind::Dispenser,
        mode: Mode::Entrance,
    };
    pub const ENTRANCE: Self = Self {
        kind: Kind::Teleporter,
        mode: Mode::Entrance,
    };
    pub const EXIT: Self = Self {
        kind: Kind::Teleporter,
        mode: Mode::Exit,
    };
    pub const MENU: [Self; 4] = [Self::SENTRY, Self::DISPENSER, Self::ENTRANCE, Self::EXIT];

    pub const fn cost(self) -> u16 {
        match self.kind {
            Kind::Sentry => 130,
            Kind::Dispenser => 100,
            Kind::Teleporter => 50,
        }
    }

    pub const fn construction_seconds(self) -> f32 {
        match self.kind {
            Kind::Sentry => 10.0,
            Kind::Dispenser | Kind::Teleporter => 20.0,
        }
    }

    pub const fn hull(self) -> Hull {
        match self.kind {
            Kind::Sentry => Hull {
                mins: [-20.0, -20.0, 0.0],
                maxs: [20.0, 20.0, 66.0],
            },
            Kind::Dispenser => Hull {
                mins: [-20.0, -20.0, 0.0],
                maxs: [20.0, 20.0, 55.0],
            },
            Kind::Teleporter => Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 12.0],
            },
        }
    }

    pub const fn blueprint(self) -> &'static str {
        match (self.kind, self.mode) {
            (Kind::Sentry, _) => "models/buildables/sentry1_blueprint.mdl",
            (Kind::Dispenser, _) => "models/buildables/dispenser_blueprint.mdl",
            (Kind::Teleporter, Mode::Entrance) => {
                "models/buildables/teleporter_blueprint_enter.mdl"
            }
            (Kind::Teleporter, Mode::Exit) => "models/buildables/teleporter_blueprint_exit.mdl",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Phase {
    Constructing = 0,
    Active = 1,
    Upgrading = 2,
    Recharging = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Build(Object),
    Destroy(Object),
    Rotate,
    Cancel,
    Hurt(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub object: Object,
    pub position: [f32; 3],
    pub yaw_degrees: f32,
    pub valid: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub identity: u32,
    pub object: Object,
    pub owner: u32,
    pub team: PlayerTeam,
    pub phase: Phase,
    pub level: u8,
    pub health: f32,
    pub maximum_health: u16,
    pub position: [f32; 3],
    pub yaw_degrees: f32,
    pub construction: f32,
    pub upgrade_metal: u16,
    pub shells: u16,
    pub maximum_shells: u16,
    pub rockets: u16,
    pub maximum_rockets: u16,
    pub dispenser_metal: u16,
    pub target: Option<u32>,
    pub recharge_end_tick: Option<u64>,
    pub times_used: u32,
    pub started_tick: u64,
}

impl Snapshot {
    pub fn model(&self) -> &'static str {
        match (self.object.kind, self.phase, self.level) {
            (Kind::Sentry, Phase::Constructing, 1) => "models/buildables/sentry1_heavy.mdl",
            (Kind::Sentry, _, 1) => "models/buildables/sentry1.mdl",
            (Kind::Sentry, Phase::Upgrading, 2) => "models/buildables/sentry2_heavy.mdl",
            (Kind::Sentry, _, 2) => "models/buildables/sentry2.mdl",
            (Kind::Sentry, Phase::Upgrading, _) => "models/buildables/sentry3_heavy.mdl",
            (Kind::Sentry, _, _) => "models/buildables/sentry3.mdl",
            (Kind::Dispenser, Phase::Constructing, 1) => "models/buildables/dispenser.mdl",
            (Kind::Dispenser, Phase::Upgrading, 2) => "models/buildables/dispenser_lvl2.mdl",
            (Kind::Dispenser, Phase::Upgrading, _) => "models/buildables/dispenser_lvl3.mdl",
            (Kind::Dispenser, _, 1) => "models/buildables/dispenser_light.mdl",
            (Kind::Dispenser, _, 2) => "models/buildables/dispenser_lvl2_light.mdl",
            (Kind::Dispenser, _, _) => "models/buildables/dispenser_lvl3_light.mdl",
            (Kind::Teleporter, Phase::Constructing | Phase::Upgrading, _) => {
                "models/buildables/teleporter.mdl"
            }
            (Kind::Teleporter, _, _) => "models/buildables/teleporter_light.mdl",
        }
    }
}

#[derive(Clone, Debug)]
struct Building {
    snapshot: Snapshot,
    construction_left: f32,
    next_construction_tick: u64,
    construction_boost_until: Option<u64>,
    upgrade_complete_tick: Option<u64>,
    next_attack_tick: u64,
    next_think_tick: u64,
    next_dispenser_tick: u64,
    next_generate_tick: u64,
    accumulated_heal: f32,
}

#[derive(Clone, Debug)]
pub struct World {
    buildings: Vec<Building>,
    placement: Option<Placement>,
    rotation: f32,
    next_identity: u32,
    interval: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    pub healing: i32,
    pub teleport: Option<([f32; 3], f32)>,
    pub sentry_target: Option<(u32, i32)>,
}

impl World {
    pub fn new(interval: f32) -> Self {
        Self {
            buildings: Vec::new(),
            placement: None,
            rotation: 0.0,
            next_identity: 0x5000_0000,
            interval,
        }
    }

    pub fn placement(&self) -> Option<Placement> {
        self.placement
    }

    pub fn invalidate_placement(&mut self) {
        if let Some(placement) = &mut self.placement {
            placement.valid = false;
        }
    }

    pub fn snapshots(&self) -> Vec<Snapshot> {
        self.buildings
            .iter()
            .map(|building| building.snapshot.clone())
            .collect()
    }

    pub fn reset(&mut self) {
        self.buildings.clear();
        self.placement = None;
        self.rotation = 0.0;
    }

    pub fn request(&mut self, request: Request, class: PlayerClass, metal: u16) {
        if class != PlayerClass::Engineer {
            return;
        }
        match request {
            Request::Build(object)
                if metal >= object.cost()
                    && !self
                        .buildings
                        .iter()
                        .any(|value| value.snapshot.object == object) =>
            {
                self.rotation = 0.0;
                self.placement = Some(Placement {
                    object,
                    position: [0.0; 3],
                    yaw_degrees: 0.0,
                    valid: false,
                });
            }
            Request::Destroy(object) => {
                self.buildings
                    .retain(|building| building.snapshot.object != object);
                if self
                    .placement
                    .is_some_and(|placement| placement.object == object)
                {
                    self.placement = None;
                }
            }
            Request::Rotate if self.placement.is_some() => {
                self.rotation = (self.rotation + 90.0) % 360.0;
            }
            Request::Cancel => self.placement = None,
            Request::Hurt(amount) => {
                for building in &mut self.buildings {
                    building.snapshot.health = (building.snapshot.health - amount as f32).max(0.0);
                }
                self.buildings
                    .retain(|building| building.snapshot.health > 0.0);
            }
            _ => {}
        }
    }

    pub fn update_placement<T: Tracer>(
        &mut self,
        world: &T,
        player_position: [f32; 3],
        eye_height: f32,
        yaw: f32,
    ) -> Result<(), playsrc_movement::Error> {
        let Some(mut placement) = self.placement else {
            return Ok(());
        };
        let hull = placement.object.hull();
        let radius = (hull.maxs[0] * hull.maxs[0] + hull.maxs[1] * hull.maxs[1]).sqrt();
        let player_radius = (24.0_f32 * 24.0 * 2.0).sqrt();
        let distance = radius + player_radius + 4.0;
        let radians = yaw.to_radians();
        let center = [
            player_position[0] + radians.cos() * distance,
            player_position[1] + radians.sin() * distance,
            player_position[2] + 41.0,
        ];
        let height = hull.maxs[2] - hull.mins[2];
        let top = player_position[2] + 82.0 + height;
        let bottom = player_position[2] - height;
        let half = Hull {
            mins: [-hull.maxs[0], -hull.maxs[1], 0.0],
            maxs: [hull.maxs[0], hull.maxs[1], 0.0],
        };
        let mut found = None;
        for index in 0..8 {
            let z = top + (bottom - top) * (index as f32 / 7.0);
            let trace = world.trace(
                [center[0], center[1], z],
                [center[0], center[1], bottom],
                half,
                MASK_BRUSH,
            )?;
            if trace.fraction == 1.0 {
                break;
            }
            if z - trace.end[2] > height && !trace.start_solid {
                found = Some(trace.end[2]);
                break;
            }
        }
        placement.position = [center[0], center[1], found.unwrap_or(player_position[2])];
        placement.yaw_degrees = yaw + self.rotation;
        placement.valid = found.is_some();
        if let Some(ground) = found {
            for x in [-hull.maxs[0], hull.maxs[0]] {
                for y in [-hull.maxs[1], hull.maxs[1]] {
                    let start = [center[0] + x, center[1] + y, ground + 2.0];
                    let corner = world.trace(
                        start,
                        [start[0], start[1], start[2] - 32.0],
                        Hull {
                            mins: [0.0; 3],
                            maxs: [0.0; 3],
                        },
                        MASK_BRUSH,
                    )?;
                    if corner.start_solid
                        || corner.fraction >= 1.0
                        || corner.normal.is_some_and(|normal| normal[2] < 0.65)
                    {
                        placement.valid = false;
                    }
                }
            }
            let overlap = world.trace(placement.position, placement.position, hull, MASK_SOLID)?;
            if overlap.start_solid || overlap.fraction < 1.0 {
                placement.valid = false;
            }
            let sight = world.trace(
                [
                    player_position[0],
                    player_position[1],
                    player_position[2] + eye_height,
                ],
                [center[0], center[1], ground + height * 0.5],
                Hull {
                    mins: [-2.0; 3],
                    maxs: [2.0; 3],
                },
                MASK_BRUSH,
            )?;
            if sight.fraction < 1.0 {
                placement.valid = false;
            }
            if self.buildings.iter().any(|existing| {
                let p = existing.snapshot.position;
                (p[0] - center[0]).abs() <= 48.0
                    && (p[1] - center[1]).abs() <= 48.0
                    && (p[2] - ground).abs() <= 32.0
            }) {
                placement.valid = false;
            }
        }
        self.placement = Some(placement);
        Ok(())
    }

    pub fn confirm(&mut self, tick: u64, owner: u32, team: PlayerTeam, metal: &mut u16) -> bool {
        let Some(placement) = self.placement else {
            return false;
        };
        if !placement.valid
            || *metal < placement.object.cost()
            || self.buildings.len() >= MAX_BUILDINGS
        {
            return false;
        }
        *metal -= placement.object.cost();
        self.placement = None;
        let identity = self.next_identity;
        self.next_identity += 1;
        self.buildings.push(Building {
            snapshot: Snapshot {
                identity,
                object: placement.object,
                owner,
                team,
                phase: Phase::Constructing,
                level: 1,
                health: STARTING_HEALTH,
                maximum_health: 150,
                position: placement.position,
                yaw_degrees: placement.yaw_degrees,
                construction: 0.0,
                upgrade_metal: 0,
                shells: 0,
                maximum_shells: if placement.object.kind == Kind::Sentry {
                    150
                } else {
                    0
                },
                rockets: 0,
                maximum_rockets: if placement.object.kind == Kind::Sentry {
                    20
                } else {
                    0
                },
                dispenser_metal: 0,
                target: None,
                recharge_end_tick: None,
                times_used: 0,
                started_tick: tick,
            },
            construction_left: placement.object.construction_seconds(),
            next_construction_tick: tick + ticks(CONSTRUCTION_INTERVAL, self.interval),
            construction_boost_until: None,
            upgrade_complete_tick: None,
            next_attack_tick: 0,
            next_think_tick: tick,
            next_dispenser_tick: 0,
            next_generate_tick: 0,
            accumulated_heal: 0.0,
        });
        true
    }

    pub fn wrench(&mut self, identity: u32, team: PlayerTeam, tick: u64, metal: &mut u16) -> bool {
        let Some(index) = self
            .buildings
            .iter()
            .position(|value| value.snapshot.identity == identity && value.snapshot.team == team)
        else {
            return false;
        };
        let building = &mut self.buildings[index];
        if building.snapshot.phase == Phase::Constructing {
            building.construction_boost_until =
                Some(tick + ticks(CONSTRUCTION_HIT_DURATION, self.interval));
            return true;
        }
        let missing = building
            .snapshot
            .maximum_health
            .saturating_sub(building.snapshot.health.round() as u16);
        let cost = WRENCH_REPAIR
            .min(missing)
            .div_ceil(REPAIR_HEALTH_PER_METAL)
            .min(*metal);
        let repaired = cost > 0;
        if repaired {
            *metal -= cost;
            building.snapshot.health = (building.snapshot.health
                + f32::from(cost * REPAIR_HEALTH_PER_METAL))
            .min(f32::from(building.snapshot.maximum_health));
        } else if building.snapshot.level < 3 && building.snapshot.phase != Phase::Upgrading {
            let added = UPGRADE_PER_HIT
                .min(*metal)
                .min(UPGRADE_METAL - building.snapshot.upgrade_metal);
            *metal -= added;
            building.snapshot.upgrade_metal += added;
            if building.snapshot.upgrade_metal >= UPGRADE_METAL {
                building.snapshot.upgrade_metal = 0;
                building.snapshot.level += 1;
                building.snapshot.phase = Phase::Upgrading;
                building.snapshot.maximum_health = match building.snapshot.level {
                    2 => 180,
                    _ => 216,
                };
                building.snapshot.health = f32::from(building.snapshot.maximum_health);
                building.upgrade_complete_tick = Some(tick + ticks(1.5, self.interval));
                if building.snapshot.object.kind == Kind::Sentry {
                    building.snapshot.maximum_shells = 200;
                    building.snapshot.shells = 200;
                    if building.snapshot.level == 3 {
                        building.snapshot.rockets = 20;
                    }
                }
            }
        }
        let building = &mut self.buildings[index];
        if building.snapshot.object.kind == Kind::Sentry
            && building.snapshot.phase != Phase::Upgrading
        {
            let shells = 40_u16
                .min(*metal)
                .min(building.snapshot.maximum_shells - building.snapshot.shells);
            *metal -= shells;
            building.snapshot.shells += shells;
            if building.snapshot.level == 3 {
                let rockets = 8_u16
                    .min(*metal / 2)
                    .min(building.snapshot.maximum_rockets - building.snapshot.rockets);
                *metal -= rockets * 2;
                building.snapshot.rockets += rockets;
            }
        }
        if self.buildings[index].snapshot.object.kind == Kind::Teleporter
            && self.buildings[index].snapshot.phase == Phase::Upgrading
        {
            let level = self.buildings[index].snapshot.level;
            let mode = self.buildings[index].snapshot.object.mode;
            if let Some(partner) = self.buildings.iter_mut().find(|value| {
                value.snapshot.object.kind == Kind::Teleporter && value.snapshot.object.mode != mode
            }) {
                if partner.snapshot.level < level {
                    partner.snapshot.level = level;
                    partner.snapshot.phase = Phase::Upgrading;
                    partner.snapshot.maximum_health = if level == 2 { 180 } else { 216 };
                    partner.snapshot.health = f32::from(partner.snapshot.maximum_health);
                    partner.upgrade_complete_tick = Some(tick + ticks(1.5, self.interval));
                }
            }
        }
        true
    }

    pub fn nearest_wrench_target(
        &self,
        origin: [f32; 3],
        forward: [f32; 3],
        team: PlayerTeam,
    ) -> Option<u32> {
        self.buildings
            .iter()
            .filter(|building| building.snapshot.team == team)
            .find_map(|building| {
                let hull = building.snapshot.object.hull();
                let center = [
                    building.snapshot.position[0],
                    building.snapshot.position[1],
                    building.snapshot.position[2] + hull.maxs[2] * 0.5,
                ];
                let delta = [
                    center[0] - origin[0],
                    center[1] - origin[1],
                    center[2] - origin[2],
                ];
                let along = delta[0] * forward[0] + delta[1] * forward[1] + delta[2] * forward[2];
                let radius = hull.maxs[0].max(hull.maxs[1]) + 18.0;
                (along >= -radius
                    && along <= crate::ballistics::WRENCH_BUILDING_RANGE + radius
                    && delta.iter().map(|value| value * value).sum::<f32>()
                        <= (crate::ballistics::WRENCH_BUILDING_RANGE + radius).powi(2))
                .then_some(building.snapshot.identity)
            })
    }

    pub fn advance<T: Tracer>(
        &mut self,
        world: &T,
        tick: u64,
        team: PlayerTeam,
        position: [f32; 3],
        health: i32,
        maximum_health: i32,
        bots: &[bot::Snapshot],
        metal: &mut u16,
    ) -> Result<Effects, playsrc_movement::Error> {
        let mut effects = Effects::default();
        for building in &mut self.buildings {
            if building.snapshot.phase == Phase::Constructing
                && tick >= building.next_construction_tick
            {
                building.next_construction_tick += ticks(CONSTRUCTION_INTERVAL, self.interval);
                let multiplier = if building
                    .construction_boost_until
                    .is_some_and(|until| tick <= until)
                {
                    1.0 + CONSTRUCTION_HIT_VALUE
                } else {
                    1.0
                };
                building.construction_left =
                    (building.construction_left - CONSTRUCTION_INTERVAL * multiplier).max(0.0);
                let duration = building.snapshot.object.construction_seconds();
                building.snapshot.construction = 1.0 - building.construction_left / duration;
                building.snapshot.health = (STARTING_HEALTH
                    + (f32::from(building.snapshot.maximum_health) - STARTING_HEALTH)
                        * building.snapshot.construction)
                    .min(f32::from(building.snapshot.maximum_health));
                if building.construction_left <= 0.0 {
                    building.snapshot.phase = Phase::Active;
                    building.snapshot.health = f32::from(building.snapshot.maximum_health);
                    if building.snapshot.object.kind == Kind::Sentry {
                        building.snapshot.shells = building.snapshot.maximum_shells;
                        building.snapshot.rockets = building.snapshot.maximum_rockets;
                    }
                    if building.snapshot.object.kind == Kind::Dispenser {
                        building.snapshot.dispenser_metal = 25;
                        building.next_dispenser_tick = tick + ticks(0.5, self.interval);
                        building.next_generate_tick = tick + ticks(3.0, self.interval);
                    }
                }
            }
            if building.snapshot.phase == Phase::Upgrading
                && building
                    .upgrade_complete_tick
                    .is_some_and(|due| tick >= due)
            {
                building.snapshot.phase = Phase::Active;
                building.upgrade_complete_tick = None;
            }
            if building.snapshot.phase == Phase::Recharging
                && building
                    .snapshot
                    .recharge_end_tick
                    .is_some_and(|due| tick > due)
            {
                building.snapshot.phase = Phase::Active;
                building.snapshot.recharge_end_tick = None;
            }
            if building.snapshot.phase != Phase::Active {
                continue;
            }
            match building.snapshot.object.kind {
                Kind::Sentry if tick >= building.next_think_tick => {
                    building.next_think_tick = tick + ticks(0.05, self.interval);
                    let eye = [
                        building.snapshot.position[0],
                        building.snapshot.position[1],
                        building.snapshot.position[2]
                            + match building.snapshot.level {
                                1 => 32.0,
                                2 => 40.0,
                                _ => 46.0,
                            },
                    ];
                    let mut nearest = None;
                    let mut nearest_distance = SENTRY_RANGE * SENTRY_RANGE;
                    for bot in bots {
                        if bot.team == building.snapshot.team
                            || bot.lifecycle != PlayerLifecycle::Active
                        {
                            continue;
                        }
                        let target = [
                            bot.position[0],
                            bot.position[1],
                            bot.position[2] + bot.class.standing_eye_height(),
                        ];
                        let distance = (0..3)
                            .map(|axis| (target[axis] - eye[axis]).powi(2))
                            .sum::<f32>();
                        if distance > nearest_distance {
                            continue;
                        }
                        let trace = world.trace_without(
                            u64::from(building.snapshot.identity),
                            eye,
                            target,
                            Hull {
                                mins: [0.0; 3],
                                maxs: [0.0; 3],
                            },
                            MASK_SOLID,
                        )?;
                        if trace.fraction >= 1.0 {
                            nearest = Some(bot.identity);
                            nearest_distance = distance;
                        }
                    }
                    building.snapshot.target = nearest;
                    if let Some(target) = nearest
                        && building.snapshot.shells > 0
                        && tick >= building.next_attack_tick
                    {
                        building.snapshot.shells -= 1;
                        building.next_attack_tick = tick
                            + ticks(
                                if building.snapshot.level == 1 {
                                    0.2
                                } else {
                                    0.1
                                },
                                self.interval,
                            );
                        effects.sentry_target = Some((target, SENTRY_DAMAGE));
                    }
                }
                Kind::Dispenser => {
                    if tick >= building.next_generate_tick {
                        building.next_generate_tick += ticks(6.0, self.interval);
                        building.snapshot.dispenser_metal = (building.snapshot.dispenser_metal
                            + 40
                            + u16::from(building.snapshot.level - 1) * 10)
                            .min(400);
                    }
                    if building.snapshot.team == team {
                        let delta = [
                            position[0] - building.snapshot.position[0],
                            position[1] - building.snapshot.position[1],
                            position[2] - building.snapshot.position[2],
                        ];
                        if delta[0].abs() <= 70.0
                            && delta[1].abs() <= 70.0
                            && (0.0..=50.0).contains(&delta[2])
                        {
                            building.accumulated_heal += [0.0, 10.0, 15.0, 20.0]
                                [building.snapshot.level as usize]
                                * self.interval;
                            let amount = (building.accumulated_heal.floor() as i32)
                                .min((maximum_health - health).max(0));
                            if amount > 0 {
                                effects.healing += amount;
                                building.accumulated_heal -= amount as f32;
                            }
                            if tick >= building.next_dispenser_tick {
                                let given = (40 + u16::from(building.snapshot.level - 1) * 10)
                                    .min(building.snapshot.dispenser_metal)
                                    .min(MAX_METAL - *metal);
                                building.snapshot.dispenser_metal -= given;
                                *metal += given;
                                building.next_dispenser_tick =
                                    tick + ticks(if given == 0 { 0.1 } else { 1.0 }, self.interval);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let entrance = self.buildings.iter().position(|building| {
            building.snapshot.object == Object::ENTRANCE
                && building.snapshot.phase == Phase::Active
                && building.snapshot.team == team
        });
        let exit = self.buildings.iter().position(|building| {
            building.snapshot.object == Object::EXIT
                && building.snapshot.phase == Phase::Active
                && building.snapshot.team == team
        });
        if let (Some(entrance), Some(exit)) = (entrance, exit) {
            let origin = self.buildings[entrance].snapshot.position;
            if (position[0] - origin[0]).abs() <= 24.0
                && (position[1] - origin[1]).abs() <= 24.0
                && (position[2] - origin[2]).abs() <= 24.0
            {
                let target = self.buildings[exit].snapshot.position;
                effects.teleport = Some((
                    [target[0], target[1], target[2] + 13.0],
                    self.buildings[exit].snapshot.yaw_degrees,
                ));
                for index in [entrance, exit] {
                    let building = &mut self.buildings[index];
                    building.snapshot.phase = Phase::Recharging;
                    let seconds = [0.0, 10.0, 5.0, 3.0][building.snapshot.level as usize];
                    building.snapshot.recharge_end_tick =
                        Some(tick + ticks(seconds, self.interval));
                    building.snapshot.times_used += 1;
                }
            }
        }
        Ok(effects)
    }
}

fn ticks(seconds: f32, interval: f32) -> u64 {
    (seconds / interval).ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;
    impl Tracer for Flat {
        fn trace(
            &self,
            start: [f32; 3],
            end: [f32; 3],
            _hull: Hull,
            _mask: u32,
        ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
            let hits_ground = start[2] > 0.0 && end[2] <= 0.0;
            let fraction = if hits_ground {
                start[2] / (start[2] - end[2])
            } else {
                1.0
            };
            Ok(playsrc_movement::Trace {
                fraction,
                start_solid: false,
                all_solid: false,
                end: if hits_ground {
                    [end[0], end[1], 0.0]
                } else {
                    end
                },
                normal: if hits_ground {
                    Some([0.0, 0.0, 1.0])
                } else {
                    None
                },
                hit: None,
                contents: 0,
            })
        }
    }

    fn build(world: &mut World, object: Object, position: [f32; 3], metal: &mut u16) -> u32 {
        world.placement = Some(Placement {
            object,
            position,
            yaw_degrees: 0.0,
            valid: true,
        });
        assert!(world.confirm(0, 1, PlayerTeam::Red, metal));
        world.buildings.last().unwrap().snapshot.identity
    }

    #[test]
    fn configured_objects_preserve_exact_stock_content() {
        assert_eq!(Object::MENU.map(Object::cost), [130, 100, 50, 50]);
        assert_eq!(
            Object::MENU.map(Object::construction_seconds),
            [10.0, 20.0, 20.0, 20.0]
        );
        assert_eq!(Object::SENTRY.hull().maxs, [20.0, 20.0, 66.0]);
        assert_eq!(Object::DISPENSER.hull().maxs, [20.0, 20.0, 55.0]);
        assert_eq!(Object::EXIT.hull().maxs, [24.0, 24.0, 12.0]);
        assert_ne!(Object::ENTRANCE.blueprint(), Object::EXIT.blueprint());
    }

    #[test]
    fn every_stock_blueprint_finds_ground_with_the_exact_player_clip_brush_mask() {
        struct PlayerClipGround;

        impl Tracer for PlayerClipGround {
            fn trace(
                &self,
                start: [f32; 3],
                end: [f32; 3],
                hull: Hull,
                mask: u32,
            ) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
                if start != end && hull.maxs[2] <= 2.0 {
                    assert_eq!(mask, 0x0001_400b);
                }
                let hit = start[2] > 0.0 && end[2] <= 0.0 && mask & 0x0001_0000 != 0;
                let fraction = if hit {
                    start[2] / (start[2] - end[2])
                } else {
                    1.0
                };
                Ok(playsrc_movement::Trace {
                    fraction,
                    start_solid: false,
                    all_solid: false,
                    end: if hit { [end[0], end[1], 0.0] } else { end },
                    normal: hit.then_some([0.0, 0.0, 1.0]),
                    hit: None,
                    contents: if hit { 0x0001_0000 } else { 0 },
                })
            }
        }

        for object in Object::MENU {
            let mut world = World::new(0.015);
            world.request(Request::Build(object), PlayerClass::Engineer, MAX_METAL);
            world
                .update_placement(&PlayerClipGround, [0.0, 0.0, 0.0], 68.0, 90.0)
                .unwrap();
            let placement = world.placement().unwrap();
            assert!(placement.valid, "{object:?}");
            assert_eq!(placement.position[2], 0.0);
            assert_eq!(placement.yaw_degrees, 90.0);
        }
    }

    #[test]
    fn wrench_repairs_before_upgrade_with_exact_metal_ratio() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        world.placement = Some(Placement {
            object: Object::ENTRANCE,
            position: [0.0; 3],
            yaw_degrees: 0.0,
            valid: true,
        });
        assert!(world.confirm(0, 1, PlayerTeam::Red, &mut metal));
        assert_eq!(metal, 150);
        world.buildings[0].snapshot.phase = Phase::Active;
        world.buildings[0].snapshot.health = 100.0;
        assert!(world.wrench(0x5000_0000, PlayerTeam::Red, 1, &mut metal));
        assert_eq!(metal, 133);
        assert_eq!(world.buildings[0].snapshot.health, 150.0);
        assert_eq!(world.buildings[0].snapshot.upgrade_metal, 0);
        assert!(world.wrench(0x5000_0000, PlayerTeam::Red, 2, &mut metal));
        assert_eq!(metal, 108);
        assert_eq!(world.buildings[0].snapshot.upgrade_metal, 25);
    }

    #[test]
    fn affordability_ownership_and_destruction_follow_stock_object_modes() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        world.request(Request::Build(Object::SENTRY), PlayerClass::Soldier, metal);
        assert!(world.placement().is_none());
        world.request(Request::Build(Object::SENTRY), PlayerClass::Engineer, metal);
        world.placement.as_mut().unwrap().valid = true;
        assert!(world.confirm(3, 1, PlayerTeam::Red, &mut metal));
        assert_eq!(metal, 70);
        world.request(
            Request::Build(Object::DISPENSER),
            PlayerClass::Engineer,
            metal,
        );
        assert!(world.placement().is_none());
        world.request(
            Request::Build(Object::ENTRANCE),
            PlayerClass::Engineer,
            metal,
        );
        assert!(world.placement().is_some());
        world.request(
            Request::Destroy(Object::SENTRY),
            PlayerClass::Engineer,
            metal,
        );
        assert!(world.snapshots().is_empty());
        assert_eq!(metal, 70);
    }

    #[test]
    fn construction_uses_source_starting_health_interval_and_wrench_multiplier() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        let sentry = build(&mut world, Object::SENTRY, [64.0, 0.0, 0.0], &mut metal);
        assert_eq!(world.buildings[0].snapshot.health, STARTING_HEALTH);
        assert_eq!(
            world.buildings[0].snapshot.model(),
            "models/buildables/sentry1_heavy.mdl"
        );
        let due = ticks(CONSTRUCTION_INTERVAL, 0.015);
        for tick in 0..=due {
            world
                .advance(
                    &Flat,
                    tick,
                    PlayerTeam::Red,
                    [0.0; 3],
                    125,
                    125,
                    &[],
                    &mut metal,
                )
                .unwrap();
        }
        let ordinary = world.buildings[0].snapshot.construction;
        assert!(world.wrench(sentry, PlayerTeam::Red, due, &mut metal));
        for tick in due + 1..=due * 2 {
            world
                .advance(
                    &Flat,
                    tick,
                    PlayerTeam::Red,
                    [0.0; 3],
                    125,
                    125,
                    &[],
                    &mut metal,
                )
                .unwrap();
        }
        assert!((world.buildings[0].snapshot.construction - ordinary * 3.5).abs() < 0.0001);
        assert_eq!(metal, 70);
    }

    #[test]
    fn upgrade_models_health_ammo_and_teleporter_partners_follow_source_levels() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        let entrance = build(&mut world, Object::ENTRANCE, [0.0; 3], &mut metal);
        let _exit = build(&mut world, Object::EXIT, [200.0, 0.0, 0.0], &mut metal);
        for value in &mut world.buildings {
            value.snapshot.phase = Phase::Active;
            value.snapshot.health = 150.0;
        }
        metal = MAX_METAL;
        for tick in 1..=8 {
            assert!(world.wrench(entrance, PlayerTeam::Red, tick, &mut metal));
        }
        assert_eq!(metal, 0);
        assert!(world.buildings.iter().all(|value| value.snapshot.level == 2
            && value.snapshot.maximum_health == 180
            && value.snapshot.phase == Phase::Upgrading));
        metal = MAX_METAL;
        for value in &mut world.buildings {
            value.snapshot.phase = Phase::Active;
        }
        for tick in 20..28 {
            assert!(world.wrench(entrance, PlayerTeam::Red, tick, &mut metal));
        }
        assert!(
            world
                .buildings
                .iter()
                .all(|value| value.snapshot.level == 3 && value.snapshot.maximum_health == 216)
        );
    }

    #[test]
    fn sentry_acquires_only_visible_enemy_players_and_uses_exact_damage_and_cadence() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        build(&mut world, Object::SENTRY, [0.0; 3], &mut metal);
        world.buildings[0].snapshot.phase = Phase::Active;
        world.buildings[0].snapshot.health = 150.0;
        world.buildings[0].snapshot.shells = 150;
        let enemy = bot::Snapshot {
            spy: None,
            identity: 2,
            name: "Enemy Soldier".to_owned(),
            class: PlayerClass::Soldier,
            team: PlayerTeam::Blue,
            lifecycle: PlayerLifecycle::Active,
            difficulty: bot::Difficulty::Normal,
            objective: bot::ObjectiveKind::Attack,
            position: [200.0, 0.0, 0.0],
            velocity: [0.0; 3],
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            health: 200,
            maximum_health: 200,
            target: None,
            area: None,
            remaining_path_areas: 0,
            weapon: None,
            shots: 0,
            hits: 0,
            kills: 0,
            killstreak: 0,
            deaths: 0,
            damage: 0,
            captures: 0,
            carrying_flag: false,
            animation_role: bot::AnimationRole::Primary,
            last_fire_tick: None,
            respawn_tick: None,
        };
        let friendly = bot::Snapshot {
            identity: 3,
            team: PlayerTeam::Red,
            ..enemy.clone()
        };
        let first = world
            .advance(
                &Flat,
                0,
                PlayerTeam::Red,
                [3000.0, 0.0, 0.0],
                125,
                125,
                &[friendly, enemy],
                &mut metal,
            )
            .unwrap();
        assert_eq!(first.sentry_target, Some((2, 16)));
        assert_eq!(world.buildings[0].snapshot.target, Some(2));
        assert_eq!(world.buildings[0].snapshot.shells, 149);
        assert_eq!(world.buildings[0].next_attack_tick, ticks(0.2, 0.015));
    }

    #[test]
    fn dispenser_uses_exact_healing_refill_and_stock_metal_cadence() {
        let mut world = World::new(0.015);
        let mut metal = MAX_METAL;
        build(&mut world, Object::DISPENSER, [0.0; 3], &mut metal);
        world.buildings[0].snapshot.phase = Phase::Active;
        world.buildings[0].snapshot.health = 150.0;
        world.buildings[0].snapshot.dispenser_metal = 25;
        world.buildings[0].next_dispenser_tick = 0;
        world.buildings[0].next_generate_tick = ticks(3.0, 0.015);
        assert_eq!(metal, 100);
        let initial = world
            .advance(
                &Flat,
                0,
                PlayerTeam::Red,
                [0.0; 3],
                100,
                125,
                &[],
                &mut metal,
            )
            .unwrap();
        assert_eq!(initial.healing, 0);
        assert_eq!(metal, 125);
        let mut healed = 0;
        for tick in 1..=ticks(1.0, 0.015) {
            healed += world
                .advance(
                    &Flat,
                    tick,
                    PlayerTeam::Red,
                    [0.0; 3],
                    100 + healed,
                    125,
                    &[],
                    &mut metal,
                )
                .unwrap()
                .healing;
        }
        assert_eq!(healed, 10);
        let generation = ticks(3.0, 0.015);
        world
            .advance(
                &Flat,
                generation,
                PlayerTeam::Red,
                [500.0, 0.0, 0.0],
                110,
                125,
                &[],
                &mut metal,
            )
            .unwrap();
        assert_eq!(world.buildings[0].snapshot.dispenser_metal, 40);
        assert_eq!(
            world.buildings[0].next_generate_tick,
            generation + ticks(6.0, 0.015)
        );
    }
}
