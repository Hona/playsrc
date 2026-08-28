//! CTeamTrainWatcher policy. Observes the entity world's actual track train;
//! never integrates a cart pose or substitutes a parent for a physics constraint.
use crate::control_point;
use playsrc_entity::{BehaviorState, Entity, EntityHandle, EntityWorld, MoverClass, Variant};

pub const INPUTS: &[&[u8]] = &[
    b"RoundActivate",
    b"SetNumTrainCappers",
    b"OnStartOvertime",
    b"Enable",
    b"Disable",
    b"SetSpeedForwardModifier",
    b"SetTrainRecedeTime",
    b"SetTrainCanRecede",
    b"SetTrainRecedeTimeAndUpdate",
];
pub const VOICE_SOUNDS: &[&str] = &["Announcer.Cart.Warning", "Announcer.Cart.FinalWarning"];
pub const GENERAL_SOUNDS: &[&str] = &["Cart.Warning", "Cart.WarningSingle"];

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    TrainInput {
        train: EntityHandle,
        input: &'static [u8],
        value: Variant,
    },
    StartRecede {
        watcher: u32,
    },
    Sparks {
        name: Vec<u8>,
        active: bool,
    },
    CaptureAlert {
        point: usize,
        final_point: bool,
    },
    AlarmStart {
        point: usize,
    },
    AlarmStop {
        point: usize,
    },
    AlarmSingle {
        point: usize,
    },
    Speak {
        concept: &'static str,
        touching_area: Option<u32>,
    },
    Pushed {
        player: u32,
        distance: u32,
    },
}

#[derive(Clone, Debug)]
struct Link {
    node: EntityHandle,
    point: usize,
    distance: f32,
    alerted: bool,
}

#[derive(Clone, Debug)]
pub struct Watcher {
    pub identity: u32,
    pub team: i32,
    pub disabled: bool,
    pub cappers: i32,
    pub blocked: bool,
    pub recede_at: f32,
    pub waiting_to_recede: bool,
    pub progress: f32,
    pub speed_level: i32,
    pub distance_from_start: f32,
    pub capture_area: Option<u32>,
    pub track_alarm: bool,
    train_name: Vec<u8>,
    start_name: Vec<u8>,
    goal_name: Vec<u8>,
    linked_names: Vec<(Vec<u8>, Vec<u8>)>,
    spark_name: Vec<u8>,
    train: Option<EntityHandle>,
    start: Option<EntityHandle>,
    goal: Option<EntityHandle>,
    links: Vec<Link>,
    total_distance: f32,
    can_recede: bool,
    handle_movement: bool,
    forward_modifier: f32,
    recede_seconds: i32,
    speed_levels: [f32; 3],
    current_speed: f32,
    receding: bool,
    hill: i32,
    next_think: f32,
    next_speak: f32,
    distance_accumulator: f32,
    alarm: Option<usize>,
    alarm_played: bool,
    alarm_end: f32,
    next_alarm: f32,
}

fn field<'a>(entity: &'a Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .rev()
        .find(|p| p.key.eq_ignore_ascii_case(key))
        .map(|p| p.value.as_slice())
}
fn text(entity: &Entity, key: &[u8]) -> Vec<u8> {
    field(entity, key).unwrap_or_default().to_vec()
}
fn integer(entity: &Entity, key: &[u8], default: i32) -> i32 {
    field(entity, key).map_or(default, |v| {
        playsrc_keyvalues::NumericValue::Bytes(v).get_int()
    })
}
fn number(entity: &Entity, key: &[u8], default: f32) -> f32 {
    field(entity, key).map_or(default, |v| {
        playsrc_keyvalues::NumericValue::Bytes(v).get_float()
    })
}
fn class(entity: &Entity, name: &[u8]) -> bool {
    entity
        .classname
        .as_deref()
        .is_some_and(|c| c.eq_ignore_ascii_case(name))
}
fn resolve(world: &EntityWorld, name: &[u8], kind: &[u8]) -> Option<EntityHandle> {
    let handle = *world.resolve(name, None, None, None).first()?;
    world
        .entity(handle)
        .filter(|e| class(&e.definition, kind))
        .map(|_| handle)
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn hill_type(world: &EntityWorld, node: EntityHandle) -> i32 {
    match world.entity(node).map(|e| &e.behavior) {
        Some(BehaviorState::PathNode(path)) if path.flags & 0x20 != 0 => 1,
        Some(BehaviorState::PathNode(path)) if path.flags & 0x40 != 0 => 2,
        _ => 0,
    }
}
fn input_integer(value: &Variant) -> i32 {
    match value {
        Variant::Integer(n) => *n,
        Variant::Bool(n) => i32::from(*n),
        Variant::String(s) => playsrc_entity::source_integer(s),
        _ => value.as_float().unwrap_or(0.0) as i32,
    }
}

impl Watcher {
    pub fn from_entities(entities: &[Entity]) -> Vec<Self> {
        entities
            .iter()
            .filter(|e| class(e, b"team_train_watcher"))
            .map(|e| Self {
                identity: e.index as u32,
                team: integer(e, b"TeamNum", 3),
                disabled: integer(e, b"StartDisabled", 0) != 0,
                cappers: 0,
                blocked: false,
                recede_at: 0.0,
                waiting_to_recede: false,
                progress: 0.0,
                speed_level: 0,
                distance_from_start: 0.0,
                capture_area: None,
                track_alarm: false,
                train_name: text(e, b"train"),
                start_name: text(e, b"start_node"),
                goal_name: text(e, b"goal_node"),
                linked_names: (1..=8)
                    .map(|i| {
                        (
                            text(e, format!("linked_pathtrack_{i}").as_bytes()),
                            text(e, format!("linked_cp_{i}").as_bytes()),
                        )
                    })
                    .collect(),
                spark_name: text(e, b"env_spark_name"),
                train: None,
                start: None,
                goal: None,
                links: vec![],
                total_distance: 0.0,
                can_recede: integer(e, b"train_can_recede", 1) != 0,
                handle_movement: integer(e, b"handle_train_movement", 0) != 0,
                forward_modifier: number(e, b"speed_forward_modifier", 1.0),
                recede_seconds: integer(e, b"train_recede_time", 0),
                speed_levels: std::array::from_fn(|i| {
                    number(e, format!("hud_min_speed_level_{}", i + 1).as_bytes(), 0.0)
                }),
                current_speed: 0.0,
                receding: false,
                hill: 0,
                next_think: f32::INFINITY,
                next_speak: 0.0,
                distance_accumulator: 0.0,
                alarm: None,
                alarm_played: false,
                alarm_end: -1.0,
                next_alarm: f32::INFINITY,
            })
            .collect()
    }

    pub fn stop_alarm(&mut self, events: &mut Vec<Event>) {
        if let Some(point) = self.alarm.take() {
            events.push(Event::AlarmStop { point });
            self.alarm_end = -1.0;
        }
        self.next_alarm = f32::INFINITY;
    }

    pub fn activate(
        &mut self,
        world: &EntityWorld,
        points: &control_point::World,
        now: f32,
        events: &mut Vec<Event>,
    ) {
        self.recede_at = 0.0;
        self.waiting_to_recede = false;
        self.blocked = false;
        self.next_speak = 0.0;
        self.capture_area = None;
        self.distance_from_start = 0.0;
        self.alarm_played = false;
        self.stop_alarm(events);
        self.train = resolve(world, &self.train_name, b"func_tracktrain");
        self.start = resolve(world, &self.start_name, b"path_track");
        self.goal = resolve(world, &self.goal_name, b"path_track");
        if self.handle_movement
            && let Some(train) = self.train
        {
            self.capture_area = world.live_handles().into_iter().find_map(|h| {
                let e = world.entity(h)?;
                (e.parent == Some(train) && class(&e.definition, b"trigger_capture_area"))
                    .then_some(e.source_index as u32)
            });
        }
        self.links.clear();
        for (node, point) in &self.linked_names {
            if let (Some(node), Some(point_handle)) = (
                resolve(world, node, b"path_track"),
                resolve(world, point, b"team_control_point"),
            ) {
                let source = world.entity(point_handle).unwrap().source_index as u32;
                if let Some(point) = points.points().iter().position(|p| p.identity == source) {
                    self.links.push(Link {
                        node,
                        point,
                        distance: 0.0,
                        alerted: false,
                    });
                }
            }
        }
        self.total_distance = 0.0;
        if let (Some(start), Some(goal)) = (self.start, self.goal) {
            let mut previous = start;
            let mut visited = std::collections::BTreeSet::from([start]);
            while let Some(node) = world.path_next(previous, true, false) {
                if !visited.insert(node) {
                    break;
                }
                self.total_distance += distance(
                    world.entity(previous).unwrap().local_transform.origin,
                    world.entity(node).unwrap().local_transform.origin,
                );
                if let Some(link) = self.links.iter_mut().find(|link| link.node == node) {
                    link.distance = self.total_distance;
                }
                if node == goal {
                    break;
                }
                previous = node;
            }
        }
        self.set_forward_modifier(self.forward_modifier, events);
        self.next_think = now + 0.1;
    }

    fn set_forward_modifier(&mut self, modifier: f32, events: &mut Vec<Event>) {
        if self.disabled || !self.handle_movement {
            return;
        }
        self.forward_modifier = modifier.abs().clamp(0.0, 1.0);
        if let Some(train) = self.train {
            events.push(Event::TrainInput {
                train,
                input: b"SetSpeedForwardModifier",
                value: Variant::float(self.forward_modifier),
            });
        }
    }

    fn movement(&mut self, start_receding: bool, events: &mut Vec<Event>) {
        if self.disabled || !self.handle_movement {
            return;
        }
        let Some(train) = self.train else {
            return;
        };
        let speed = if start_receding {
            self.receding = true;
            -0.1
        } else if self.cappers > 0 {
            self.receding = false;
            if self.hill == 2 {
                1.0
            } else {
                match self.cappers {
                    1 => 0.55,
                    2 => 0.77,
                    _ => 1.0,
                }
            }
        } else if self.cappers == -1 {
            if self.hill == 2 { 1.0 } else { 0.0 }
        } else if self.current_speed > 0.0 {
            if self.hill == 2 { 1.0 } else { 0.0 }
        } else if self.hill == 1 {
            -1.0
        } else if self.receding {
            -0.1
        } else {
            0.0
        };
        if self.current_speed != speed {
            if speed >= 0.0 {
                self.receding = false;
            }
            self.current_speed = speed;
            events.push(Event::TrainInput {
                train,
                input: b"SetSpeedDirAccel",
                value: Variant::float(speed),
            });
            events.push(Event::Sparks {
                name: self.spark_name.clone(),
                active: speed < 0.0,
            });
        }
    }

    pub fn set_cappers(
        &mut self,
        count: i32,
        area: Option<(u32, bool)>,
        now: f32,
        overtime: bool,
        events: &mut Vec<Event>,
    ) {
        if self.disabled {
            return;
        }
        self.cappers = count;
        if let Some((area, blocked)) = area {
            self.capture_area = Some(area);
            self.blocked = blocked;
        }
        if count <= 0 && !self.blocked && self.can_recede {
            if !self.waiting_to_recede {
                self.waiting_to_recede = true;
                self.recede_at = now
                    + if overtime {
                        5.0
                    } else if self.recede_seconds > 0 {
                        self.recede_seconds as f32
                    } else {
                        30.0
                    };
            }
        } else {
            self.waiting_to_recede = false;
            self.recede_at = 0.0;
        }
        self.movement(false, events);
    }

    pub fn input(
        &mut self,
        name: &[u8],
        value: &Variant,
        caller_area: Option<(u32, bool)>,
        world: &EntityWorld,
        points: &control_point::World,
        now: f32,
        overtime: bool,
        events: &mut Vec<Event>,
    ) -> bool {
        if name.eq_ignore_ascii_case(b"RoundActivate") {
            self.stop_alarm(events);
            if !self.disabled {
                self.activate(world, points, now, events);
            }
        } else if name.eq_ignore_ascii_case(b"Enable") {
            self.stop_alarm(events);
            self.disabled = false;
            self.activate(world, points, now, events);
        } else if name.eq_ignore_ascii_case(b"Disable") {
            self.stop_alarm(events);
            self.disabled = true;
            self.next_think = f32::INFINITY;
            self.waiting_to_recede = false;
            if self.handle_movement {
                self.current_speed = 0.0;
                if let Some(train) = self.train {
                    events.push(Event::TrainInput {
                        train,
                        input: b"SetSpeedDirAccel",
                        value: Variant::float(0.0),
                    });
                }
            }
        } else if name.eq_ignore_ascii_case(b"SetNumTrainCappers") {
            self.set_cappers(input_integer(value), caller_area, now, overtime, events);
        } else if name.eq_ignore_ascii_case(b"SetSpeedForwardModifier") {
            self.set_forward_modifier(value.as_float().unwrap_or(0.0), events);
        } else if name.eq_ignore_ascii_case(b"SetTrainRecedeTime")
            || name.eq_ignore_ascii_case(b"SetTrainRecedeTimeAndUpdate")
        {
            self.recede_seconds = input_integer(value).max(0);
            if name.eq_ignore_ascii_case(b"SetTrainRecedeTimeAndUpdate") && self.recede_at > 0.0 {
                self.recede_at = now
                    + if self.recede_seconds > 0 {
                        self.recede_seconds as f32
                    } else {
                        30.0
                    };
            }
        } else if name.eq_ignore_ascii_case(b"SetTrainCanRecede") {
            self.can_recede = input_integer(value) != 0;
        } else if name.eq_ignore_ascii_case(b"OnStartOvertime") {
            if self.waiting_to_recede && self.recede_at - now > 5.0 {
                self.recede_at = now + 5.0;
            }
        } else {
            return false;
        }
        true
    }

    pub fn path_passed(
        &mut self,
        node: EntityHandle,
        world: &EntityWorld,
        events: &mut Vec<Event>,
    ) {
        if self.disabled || !self.handle_movement {
            return;
        }
        let mut next = self.start;
        let mut visited = std::collections::BTreeSet::new();
        let mut belongs = false;
        while let Some(current) = next {
            if !visited.insert(current) {
                break;
            }
            if current == node {
                belongs = true;
                break;
            }
            next = world.path_next(current, true, false);
        }
        if !belongs {
            return;
        }
        if self.receding&&world.path_next(node,false,false).is_some_and(|previous|matches!(&world.entity(previous).unwrap().behavior,BehaviorState::PathNode(path) if path.flags&1!=0)){return;}
        let mut hill = hill_type(world, node);
        let mut update = self.hill != hill;
        if !update
            && self.hill != 0
            && world
                .path_next(node, self.current_speed >= 0.0, false)
                .is_some_and(|next| hill_type(world, next) != self.hill)
        {
            hill = 0;
            update = true;
        }
        if update {
            self.hill = hill;
            self.movement(false, events);
        }
    }

    pub fn timer_may_expire(&self) -> bool {
        self.disabled || (!self.waiting_to_recede && !self.blocked)
    }

    pub fn entity_removed(&mut self, entity: EntityHandle, source: u32, events: &mut Vec<Event>) {
        if self.train == Some(entity) {
            self.train = None;
        }
        if self.start == Some(entity) {
            self.start = None;
        }
        if self.goal == Some(entity) {
            self.goal = None;
        }
        if self.capture_area == Some(source) {
            self.capture_area = None;
        }
        if self.identity == source {
            self.disabled = true;
            self.next_think = f32::INFINITY;
            self.stop_alarm(events);
        }
    }

    pub fn checkpoint_progress(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        self.links
            .iter()
            .map(|link| (link.point, link.distance / self.total_distance))
    }

    pub fn project_point(&self, world: &EntityWorld, position: [f32; 3]) -> ([f32; 3], f32) {
        let mut node = self.start;
        let mut visited = std::collections::BTreeSet::new();
        let mut along = 0.0;
        let mut best = [0.0; 3];
        let mut best_distance = f32::MAX;
        let mut best_along = f32::MAX;
        while let Some(current) = node {
            if !visited.insert(current) {
                break;
            }
            let Some(next) = world
                .path_next(current, true, false)
                .filter(|next| !visited.contains(next))
            else {
                break;
            };
            let a = world.entity(current).unwrap().world_transform.origin;
            let b = world.entity(next).unwrap().world_transform.origin;
            let length = distance(a, b);
            let direction = if length > 0.0 {
                std::array::from_fn(|i| (b[i] - a[i]) / length)
            } else {
                [0.0; 3]
            };
            let overlap = (position[0] - a[0]) * direction[0]
                + (position[1] - a[1]) * direction[1]
                + (position[2] - a[2]) * direction[2];
            if overlap >= 0.0 && overlap < length {
                let projected = std::array::from_fn(|i| a[i] + direction[i] * overlap);
                let squared = (projected[0] - position[0]).powi(2)
                    + (projected[1] - position[1]).powi(2)
                    + (projected[2] - position[2]).powi(2);
                if squared < best_distance {
                    best = projected;
                    best_distance = squared;
                    best_along = along + overlap;
                }
            }
            along += length;
            node = Some(next);
        }
        (best, best_along)
    }

    pub fn is_ahead(&self, world: &EntityWorld, position: [f32; 3]) -> bool {
        self.project_point(world, position).1 > self.distance_from_start
    }
    pub fn near_checkpoint(&self) -> bool {
        self.links.iter().any(|link| {
            self.distance_from_start > link.distance - 750.0
                && self.distance_from_start < link.distance
        })
    }
    pub fn at_start(&self) -> bool {
        self.distance_from_start < 200.0
    }
    pub fn next_checkpoint_position(&self, world: &EntityWorld) -> [f32; 3] {
        self.links
            .iter()
            .find(|link| self.distance_from_start < link.distance)
            .and_then(|link| world.entity(link.node))
            .map_or([0.0; 3], |entity| entity.world_transform.origin)
    }

    pub fn think(
        &mut self,
        world: &EntityWorld,
        points: &control_point::World,
        now: f32,
        running: bool,
        overtime: bool,
        events: &mut Vec<Event>,
    ) {
        self.watcher_think(world, points, now, running, overtime, events);
        if now >= self.next_alarm {
            if let Some(link) = self.links.last() {
                events.push(Event::AlarmSingle { point: link.point });
            }
            self.next_alarm = now + 8.0;
        }
    }

    fn watcher_think(
        &mut self,
        world: &EntityWorld,
        points: &control_point::World,
        now: f32,
        running: bool,
        overtime: bool,
        events: &mut Vec<Event>,
    ) {
        if self.disabled || now < self.next_think {
            return;
        }
        self.next_think = now + 0.1;
        if self.waiting_to_recede && self.recede_at < now {
            self.waiting_to_recede = false;
            if !overtime {
                events.push(Event::StartRecede {
                    watcher: self.identity,
                });
                self.movement(true, events);
            }
        }
        if !running {
            self.stop_alarm(events);
        }
        let Some(train) = self.train.and_then(|h| world.entity(h)) else {
            return;
        };
        let BehaviorState::Mover(mover) = &train.behavior else {
            return;
        };
        if mover.class != MoverClass::TrackTrain {
            return;
        }
        let Some(path) = &mover.path else {
            return;
        };
        let speed = f32::from_bits(path.desired_speed_bits);
        let old = self.speed_level;
        self.speed_level = if speed < 0.0 {
            if f32::from_bits(path.current_speed_bits) == 0.0 {
                0
            } else {
                -1
            }
        } else if speed > self.speed_levels[2] {
            3
        } else if speed > self.speed_levels[1] {
            2
        } else if speed > self.speed_levels[0] {
            1
        } else {
            0
        };
        if old != self.speed_level {
            if self.speed_level == 0 && old != 0 {
                if self.handle_movement {
                    events.push(Event::Sparks {
                        name: self.spark_name.clone(),
                        active: false,
                    });
                }
                events.push(Event::Speak {
                    concept: "TLK_CART_STOP",
                    touching_area: None,
                });
                self.next_speak = 0.0;
            } else if self.speed_level < 0 && old == 0 {
                events.push(Event::Speak {
                    concept: "TLK_CART_MOVING_BACKWARD",
                    touching_area: None,
                });
                self.next_speak = 0.0;
            }
        }
        if self.speed_level > 0 && self.next_speak < now {
            if self.capture_area.is_some() {
                events.push(Event::Speak {
                    concept: "TLK_CART_MOVING_FORWARD",
                    touching_area: self.capture_area,
                });
            }
            self.next_speak = now + 3.0;
        }
        let node = if speed < 0.0 {
            path.current
        } else {
            path.current
                .and_then(|node| world.path_next(node, true, false))
        };
        let Some(mut node) = node else {
            return;
        };
        let mut remaining = distance(
            world.entity(node).unwrap().local_transform.origin,
            train.local_transform.origin,
        );
        let mut visited = std::collections::BTreeSet::from([node]);
        while Some(node) != self.goal {
            let Some(next) = world.path_next(node, true, false) else {
                break;
            };
            if !visited.insert(next) {
                break;
            }
            remaining += distance(
                world.entity(node).unwrap().local_transform.origin,
                world.entity(next).unwrap().local_transform.origin,
            );
            node = next;
        }
        if self.total_distance <= 0.0 {
            self.total_distance = 1.0;
        }
        self.progress = (1.0 - remaining / self.total_distance).clamp(0.0, 1.0);
        let previous = self.distance_from_start;
        self.distance_from_start = self.total_distance - remaining;
        if self.distance_from_start > previous {
            self.distance_accumulator += self.distance_from_start - previous;
            if self.speed_level > 0 && self.distance_accumulator >= 16.0 {
                self.distance_accumulator -= 16.0;
                if let Some(area) = self.capture_area.and_then(|identity| {
                    points.areas().iter().find(|area| area.identity == identity)
                }) {
                    let mut players = area.touching.clone();
                    players.sort_unstable();
                    for player in players {
                        events.push(Event::Pushed {
                            player,
                            distance: 1,
                        });
                    }
                }
            }
        }
        for link in &mut self.links {
            if self.distance_from_start < link.distance - 750.0 {
                if self.distance_from_start < link.distance - 1500.0 || !self.can_recede {
                    link.alerted = false;
                }
            } else if self.distance_from_start < link.distance && !link.alerted {
                link.alerted = true;
                if running {
                    events.push(Event::CaptureAlert {
                        point: link.point,
                        final_point: points.would_capture_win(link.point),
                    });
                }
            }
        }
        self.track_alarm = remaining <= 200.0;
        if self.track_alarm {
            if running {
                if self.alarm.is_none() {
                    if let Some(link) = self.links.last().filter(|_| !self.alarm_played) {
                        self.alarm = Some(link.point);
                        self.alarm_played = true;
                        self.alarm_end = now + 18.0;
                        events.push(Event::AlarmStart { point: link.point });
                    }
                } else if !self.can_recede && self.alarm_end > 0.0 && self.alarm_end < now {
                    self.stop_alarm(events);
                    self.next_alarm = now + 8.0;
                }
            }
        } else {
            self.stop_alarm(events);
            self.alarm_played = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_entity::{EventTarget, InputRecord, WorldCommand};

    fn fixture() -> (EntityWorld, control_point::World, Watcher) {
        let graph=playsrc_entity::parse(br#"
          {"classname" "team_control_point_master" "cpm_restrict_team_cap_win" "2"}
          {"classname" "team_control_point" "targetname" "first" "point_index" "1" "point_default_owner" "2"}
          {"classname" "team_control_point" "targetname" "last" "point_index" "2" "point_default_owner" "2" "team_previouspoint_3_0" "first"}
          {"classname" "path_track" "targetname" "start" "target" "middle" "origin" "0 0 0"}
          {"classname" "path_track" "targetname" "middle" "target" "goal" "origin" "1000 0 0" "spawnflags" "32"}
          {"classname" "path_track" "targetname" "goal" "origin" "2000 0 0"}
          {"classname" "func_tracktrain" "targetname" "train" "target" "start" "model" "*1" "speed" "100" "startspeed" "0" "orientationtype" "0"}
          {"classname" "trigger_capture_area" "targetname" "area" "parentname" "train" "model" "*2" "area_cap_point" "first" "area_time_to_cap" "99999" "team_cancap_3" "1"}
          {"classname" "team_train_watcher" "train" "train" "start_node" "start" "goal_node" "goal" "handle_train_movement" "1" "linked_pathtrack_1" "middle" "linked_cp_1" "first" "linked_pathtrack_2" "goal" "linked_cp_2" "last" "hud_min_speed_level_1" "1" "hud_min_speed_level_2" "60" "hud_min_speed_level_3" "80"}
        "#,Default::default()).unwrap();
        let (mut world, _) = EntityWorld::compile(
            &graph,
            playsrc_entity::EntityWorldConfig {
                model_bounds: (1..=2)
                    .map(|model| playsrc_entity::ModelBounds {
                        model,
                        mins: [-1.0; 3],
                        maxs: [1.0; 3],
                    })
                    .collect(),
                ..Default::default()
            },
        )
        .unwrap();
        world.phase(20, &[]).unwrap();
        let points = control_point::World::from_graph(&graph).unwrap().unwrap();
        let mut watcher = Watcher::from_entities(&graph.entities).remove(0);
        watcher.activate(&world, &points, 0.3, &mut vec![]);
        (world, points, watcher)
    }
    fn speed(events: &[Event]) -> Option<f32> {
        events.iter().rev().find_map(|event| match event {
            Event::TrainInput {
                input: b"SetSpeedDirAccel",
                value,
                ..
            } => value.as_float(),
            _ => None,
        })
    }

    #[test]
    fn capper_recede_overtime_and_disable_contracts_do_not_integrate_motion() {
        let (world, points, mut watcher) = fixture();
        let original = world
            .entity(watcher.train.unwrap())
            .unwrap()
            .world_transform;
        for (count, expected) in [(1, 0.55), (2, 0.77), (3, 1.0), (0, 0.0)] {
            let mut events = vec![];
            watcher.set_cappers(count, Some((7, false)), 1.0, false, &mut events);
            assert_eq!(speed(&events), Some(expected));
        }
        assert_eq!(watcher.recede_at, 31.0);
        assert!(!watcher.timer_may_expire());
        watcher.set_cappers(0, None, 5.0, false, &mut vec![]);
        assert_eq!(watcher.recede_at, 31.0);
        watcher.input(
            b"OnStartOvertime",
            &Variant::Void,
            None,
            &world,
            &points,
            10.0,
            true,
            &mut vec![],
        );
        assert_eq!(watcher.recede_at, 15.0);
        let mut events = vec![];
        watcher.think(&world, &points, 15.0, true, true, &mut events);
        assert!(watcher.waiting_to_recede);
        watcher.think(&world, &points, 15.105, true, true, &mut events);
        assert!(!watcher.waiting_to_recede);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::StartRecede { .. }))
        );
        watcher.set_cappers(-1, Some((7, true)), 16.0, false, &mut vec![]);
        assert!(!watcher.timer_may_expire());
        assert_eq!(watcher.recede_at, 0.0);
        watcher.set_cappers(0, Some((7, false)), 17.0, false, &mut vec![]);
        watcher.input(
            b"SetTrainRecedeTimeAndUpdate",
            &Variant::Integer(2),
            None,
            &world,
            &points,
            18.0,
            true,
            &mut vec![],
        );
        assert_eq!(watcher.recede_at, 20.0);
        watcher.input(
            b"SetTrainCanRecede",
            &Variant::Integer(0),
            None,
            &world,
            &points,
            18.0,
            false,
            &mut vec![],
        );
        events.clear();
        watcher.think(&world, &points, 20.2, true, false, &mut events);
        assert!(matches!(events.first(), Some(Event::StartRecede { .. })));
        assert_eq!(speed(&events), Some(-0.1));
        watcher.input(
            b"Disable",
            &Variant::Void,
            None,
            &world,
            &points,
            21.0,
            false,
            &mut events,
        );
        assert!(watcher.timer_may_expire());
        let cappers = watcher.cappers;
        watcher.set_cappers(4, None, 22.0, false, &mut vec![]);
        assert_eq!(watcher.cappers, cappers);
        assert_eq!(
            world
                .entity(watcher.train.unwrap())
                .unwrap()
                .world_transform,
            original,
            "policy requests must not counterfeit the missing physics motion"
        );
    }

    #[test]
    fn path_observation_alert_alarm_and_projection_contracts() {
        let (mut world, mut points, mut watcher) = fixture();
        assert_eq!(
            watcher.checkpoint_progress().collect::<Vec<_>>(),
            [(0, 0.5), (1, 1.0)]
        );
        assert_eq!(
            watcher.project_point(&world, [500.0, 30.0, 0.0]),
            ([500.0, 0.0, 0.0], 500.0)
        );
        assert_eq!(
            watcher.project_point(&world, [2000.0, 0.0, 0.0]),
            ([0.0; 3], f32::MAX)
        );
        assert!(watcher.at_start());
        let train = watcher.train.unwrap();
        // Controlled entity observations, not a cart/constraint simulation.
        world
            .phase(
                21,
                &[
                    WorldCommand::Input(InputRecord {
                        target: EventTarget::Direct(train),
                        input: b"SetSpeedDirAccel".to_vec(),
                        value: Variant::float(1.0),
                        activator: None,
                        caller: None,
                        output_action: None,
                        producer_sequence: 1,
                    }),
                    WorldCommand::SetWorldTransform {
                        entity: train,
                        transform: playsrc_entity::Transform {
                            origin: [900.0, 0.0, 0.0],
                            angles: [0.0; 3],
                        },
                    },
                ],
            )
            .unwrap();
        let mut events = vec![];
        watcher.think(&world, &points, 1.0, true, false, &mut events);
        assert!((watcher.progress - 0.45).abs() < 0.00001);
        assert_eq!(watcher.speed_level, 3);
        assert!(watcher.near_checkpoint());
        assert!(events.iter().any(|e| matches!(
            e,
            Event::CaptureAlert {
                point: 0,
                final_point: false
            }
        )));
        assert!(
            !events.iter().any(|e| matches!(e, Event::Pushed { .. })),
            "no touching players means no pushed game events"
        );
        events.clear();
        watcher.think(&world, &points, 1.11, true, false, &mut events);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::CaptureAlert { .. }))
        );
        let facts = control_point::Facts {
            points_may_be_captured: true,
            round_running: true,
            ..Default::default()
        };
        points.apply_input(
            1,
            b"SetOwner",
            &Variant::Integer(3),
            2.0,
            facts,
            &mut vec![],
        );
        points.retarget_area(7, Some(2), 2.0, facts, &mut vec![]);
        world
            .phase(
                22,
                &[
                    WorldCommand::Input(InputRecord {
                        target: EventTarget::Direct(train),
                        input: b"TeleportToPathTrack".to_vec(),
                        value: Variant::String(b"middle".to_vec()),
                        activator: None,
                        caller: None,
                        output_action: None,
                        producer_sequence: 2,
                    }),
                    WorldCommand::SetWorldTransform {
                        entity: train,
                        transform: playsrc_entity::Transform {
                            origin: [1805.0, 0.0, 0.0],
                            angles: [0.0; 3],
                        },
                    },
                ],
            )
            .unwrap();
        events.clear();
        watcher.think(&world, &points, 3.0, true, false, &mut events);
        assert!(events.iter().any(|e| matches!(
            e,
            Event::CaptureAlert {
                point: 1,
                final_point: true
            }
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AlarmStart { point: 1 }))
        );
        assert!(watcher.track_alarm);
        watcher.input(
            b"SetTrainCanRecede",
            &Variant::Bool(false),
            None,
            &world,
            &points,
            3.0,
            false,
            &mut vec![],
        );
        events.clear();
        watcher.think(&world, &points, 21.1, true, false, &mut events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AlarmStop { point: 1 }))
        );
        events.clear();
        watcher.think(&world, &points, 29.2, true, false, &mut events);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AlarmSingle { point: 1 }))
        );
        events.clear();
        watcher.think(&world, &points, 37.3, false, false, &mut events);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::AlarmSingle { .. })),
            "round-end cleanup precedes the alarm context"
        );
    }
}
