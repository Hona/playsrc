//! Control-point scenario decisions from CTFBotCapturePoint, CTFBotDefendPoint,
//! CTFBotDefendPointBlockCapture and CTFBot::GetMyControlPoint.
use std::collections::{BTreeMap, VecDeque};
use super::*;

#[derive(Clone, Debug, Default)]
pub(super) struct Action {
    point: Option<usize>,
    evaluate_at: f32,
    hunt_until: f32,
    defense_area: Option<u32>,
    idle_until: f32,
    idle_duration: f32,
    allowed_to_roam: bool,
    defending_started: bool,
    pub on_point: bool,
    pub route: Option<Route>,
    seek_area: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Navigation {
    data: Arc<NavigationData>,
    combat: Arc<BTreeMap<u32, (f32, f32)>>,
    combat_throttle: Arc<BTreeMap<u32, f32>>,
}

#[derive(Debug, Default)]
pub(super) struct NavigationData {
    areas: Vec<Vec<u32>>,
    centers: Vec<Option<u32>>,
    incursion: BTreeMap<u32, [f32;2]>,
    defenses: Vec<[Vec<u32>;2]>,
    thresholds: [Vec<u32>; 2],
}

impl std::ops::Deref for Navigation {
    type Target = NavigationData;
    fn deref(&self) -> &Self::Target { &self.data }
}

impl Navigation {
    pub fn compile(mesh: &Mesh, points: &crate::control_point::World, spawns: &[Vec<Spawn>;2]) -> Result<Self, Error> {
        let mut result = NavigationData { areas: vec![Vec::new();points.points().len()], centers: vec![None;points.points().len()], ..NavigationData::default() };
        let exit_ids: std::collections::BTreeSet<_> = mesh.areas.iter().filter(|area| area.game_attributes & (TF_NAV_SPAWN_ROOM_RED | TF_NAV_SPAWN_ROOM_BLUE) != 0 && area.connections.iter().flatten().any(|id| mesh.area(*id).is_some_and(|next| next.game_attributes & (TF_NAV_SPAWN_ROOM_RED | TF_NAV_SPAWN_ROOM_BLUE) == 0))).map(|area| area.identity).collect();
        for (t, flags) in [TF_NAV_SPAWN_ROOM_RED, TF_NAV_SPAWN_ROOM_BLUE].into_iter().enumerate() {
            let count = mesh.areas.iter().filter(|area| area.game_attributes & flags != 0 && exit_ids.contains(&area.identity)).count();
            // The SDK's threshold collector indexes TheNavAreas by exit ordinal.
            for area in mesh.areas.iter().take(count) {
                let mut largest = None;
                let mut size = 0.0;
                for id in area.connections.iter().flatten() {
                    let next = mesh.area(*id).unwrap();
                    if next.game_attributes & (TF_NAV_SPAWN_ROOM_RED | TF_NAV_SPAWN_ROOM_BLUE) == 0 && !exit_ids.contains(id) && area_size(next) > size { largest = Some(*id); size = area_size(next); }
                }
                if let Some(area) = largest { result.thresholds[t].push(area); }
            }
        }
        for capture in points.areas() {
            let (mut lo, mut hi) = capture.bounds.ok_or(Error::InvalidEntity)?;
            for axis in 0..3 { lo[axis] += capture.origin[axis]; hi[axis] += capture.origin[axis]; }
            let center = [(lo[0]+hi[0])*0.5,(lo[1]+hi[1])*0.5,(lo[2]+hi[2])*0.5];
            lo[2] -= 35.5; hi[2] += 35.5;
            for area in &mesh.areas {
                let low_z = area.northwest[2].min(area.southeast[2]).min(area.northeast_z).min(area.southwest_z);
                let high_z = area.northwest[2].max(area.southeast[2]).max(area.northeast_z).max(area.southwest_z);
                if area.southeast[0] >= lo[0] && area.northwest[0] <= hi[0] && area.southeast[1] >= lo[1] && area.northwest[1] <= hi[1] && high_z >= lo[2] && low_z <= hi[2] {
                    result.areas[capture.point].push(area.identity);
                }
            }
            result.centers[capture.point] = result.areas[capture.point].iter().copied().min_by(|a,b| {
                let a = mesh.area(*a).unwrap().center(); let b = mesh.area(*b).unwrap().center();
                ((a[0]-center[0]).powi(2)+(a[1]-center[1]).powi(2)).total_cmp(&((b[0]-center[0]).powi(2)+(b[1]-center[1]).powi(2)))
            });
        }
        result.incursion = mesh.areas.iter().map(|a| (a.identity, [-1.0;2])).collect();
        for t in 0..2 {
            let Some(start) = spawns[t].first().and_then(|s| mesh.nearest_area(s.position)) else { continue; };
            let mut queue = VecDeque::from([start.identity]);
            result.incursion.get_mut(&start.identity).unwrap()[t] = 0.0;
            while let Some(id) = queue.pop_front() {
                let area = mesh.area(id).unwrap();
                if area.game_attributes & (8 | TF_NAV_BLUE_SETUP_GATE | TF_NAV_RED_SETUP_GATE) == 0 && area.game_attributes & TF_NAV_BLOCKED != 0 { continue; }
                for direction in Direction::ALL {
                    for adjacent in &area.connections[direction as usize] {
                        let next = mesh.area(*adjacent).unwrap();
                        if area.connection_height_change(next, direction) > MAX_JUMP_HEIGHT { continue; }
                        let new = result.incursion[&id][t] + distance(area.center(), next.center());
                        let old = result.incursion.get_mut(adjacent).unwrap();
                        if old[t] < 0.0 || old[t] > new {
                            old[t] = new;
                            if !queue.contains(adjacent) { queue.push_back(*adjacent); }
                        }
                    }
                }
            }
        }
        // ComputeIncursionDistances' non-MvM pass derives RED from BLU flow.
        let maximum = result.incursion.values().map(|v| v[1]).fold(0.0_f32, f32::max);
        for flow in result.incursion.values_mut() { if flow[1] >= 0.0 { flow[0] = maximum-flow[1]; } }
        for center in &result.centers {
            let mut teams: [Vec<u32>;2] = std::array::from_fn(|_| Vec::new());
            if let Some(center) = center.and_then(|id| mesh.area(id)) {
                for t in 0..2 {
                    let mut queue = VecDeque::from([(center.identity,0.0)]);
                    let mut seen = std::collections::BTreeSet::from([center.identity]);
                    while let Some((id, travelled)) = queue.pop_front() {
                        let area = mesh.area(id).unwrap();
                        if (points.points().len() == 1 || result.incursion[&id][t] <= result.incursion[&center.identity][t] + 250.0)
                            && potentially_visible(mesh, area, center.identity) && center.center()[2] - area.center()[2] < 220.0 {
                            teams[t].push(id);
                        }
                        for direction in Direction::ALL {
                            for adjacent in &area.connections[direction as usize] {
                                let next = mesh.area(*adjacent).unwrap();
                                let travel = travelled + distance(area.center(), next.center());
                                if travel <= 1250.0 && area.connection_height_change(next, direction).abs() < 65.0 && next.game_attributes & TF_NAV_BLOCKED == 0 && seen.insert(*adjacent) {
                                    queue.push_back((*adjacent,travel));
                                }
                            }
                        }
                    }
                }
            }
            result.defenses.push(teams);
        }
        Ok(Self { data: Arc::new(result), ..Self::default() })
    }

    pub fn recompute(&mut self, mesh: &Mesh, points: &crate::control_point::World, spawns: &[Vec<Spawn>;2]) -> Result<(), Error> {
        let next = Self::compile(mesh, points, spawns)?;
        self.data = next.data;
        Ok(())
    }

    pub fn reset_combat(&mut self) { self.combat = Arc::default(); self.combat_throttle = Arc::default(); }

    pub fn combat_intensity(&self, area: u32, now: f32) -> f32 {
        self.combat.get(&area).map_or(0.0, |(value, at)| (value - (now - at) * 0.022).max(0.0))
    }

    pub fn record_combat(&mut self, mesh: &Mesh, actor: u32, weapon: Weapon, position: [f32;3], now: f32) {
        if matches!(weapon, Weapon::MediGun | Weapon::Wrench | Weapon::BuildPda | Weapon::DestroyPda | Weapon::Toolbox | Weapon::DisguiseKit | Weapon::InvisibilityWatch | Weapon::Sapper) || self.combat_throttle.get(&actor).is_some_and(|last| now < *last + 1.0) { return; }
        Arc::make_mut(&mut self.combat_throttle).insert(actor, now);
        let Some(start) = mesh.nearest_area(position) else { return; };
        let mut queue = VecDeque::from([(start.identity, 0.0)]);
        let mut distances = BTreeMap::from([(start.identity, 0.0)]);
        while let Some((id, travelled)) = queue.pop_front() {
            let area = mesh.area(id).unwrap();
            for direction in Direction::ALL {
                for adjacent in &area.connections[direction as usize] {
                    let next = mesh.area(*adjacent).unwrap();
                    let travel = travelled + distance(area.center(), next.center());
                    if travel <= 1000.0 && area.connection_height_change(next, direction).abs() <= STEP_HEIGHT && distances.get(adjacent).is_none_or(|old| travel < *old) {
                        distances.insert(*adjacent, travel); queue.push_back((*adjacent, travel));
                    }
                }
            }
        }
        for id in distances.keys() {
            let state = Arc::make_mut(&mut self.combat).entry(*id).or_insert((0.0, now));
            state.0 = (state.0 + 0.05).min(1.0); state.1 = now;
        }
    }
}

fn potentially_visible(mesh: &Mesh, area: &Area, to: u32) -> bool {
    if area.identity == to { return true; }
    if let Some(visible) = area.visible_areas.iter().find(|a| a.identity == to) { return visible.attributes != 0; }
    area.inherited_visibility.and_then(|id| mesh.area(id)).is_some_and(|parent| parent.visible_areas.iter().any(|a| a.identity == to && a.attributes != 0))
}

pub(super) fn goal(bot: &mut Bot, frame: Objectives<'_>, navigation: &Navigation, mesh: &Mesh, interval: f32, now: f32, tick: u64, threat: Option<Actor>, random: &mut UniformRandomStream) -> Result<(ObjectiveKind,[f32;3]),Error> {
    let points = frame.points.unwrap();
    bot.point_action.route = Some(Route::Default);
    if bot.point_action.point.is_none() || now >= bot.point_action.evaluate_at {
        bot.point_action.evaluate_at = now + random.random_float(1.0,2.0);
        let capture: Vec<_> = points.bot_capture_points(bot.team).map(|p| p.index).collect();
        let defend: Vec<_> = points.bot_defend_points(bot.team).map(|p| p.index).collect();
        let selected = if !defend.is_empty() && matches!(bot.class, PlayerClass::Engineer | PlayerClass::Sniper) {
            Some(defend[random.random_int(0,defend.len() as i32-1).map_err(|_| Error::Limit)? as usize])
        } else if capture.len() == 1 { Some(capture[0]) }
        else if !capture.is_empty() {
            points.areas().iter().find(|a| capture.contains(&a.point) && a.touching.contains(&bot.identity)).map(|a| a.point)
                .or_else(|| capture.iter().copied().find(|p| points.points()[*p].last_contested_at > 0.0 && now-points.points()[*p].last_contested_at < 5.0))
                .or_else(|| Some(capture[((capture.len() as f32 * consistent(bot,now,60.0)) as usize).min(capture.len()-1)]))
        } else if !defend.is_empty() { Some(defend[random.random_int(0,defend.len() as i32-1).map_err(|_| Error::Limit)? as usize]) }
        else { None };
        if selected != bot.point_action.point { bot.next_repath_tick = 0; bot.point_action.defense_area = None; }
        bot.point_action.point = selected;
    }
    let Some(index) = bot.point_action.point else {
        return seek(bot, points, navigation, mesh, threat, random);
    };
    let point = &points.points()[index];
    let touching = point.owner != bot.team && points.actor_can_capture(control_point_actor(bot), index) && points.areas().iter().any(|a| a.point == index && a.touching.contains(&bot.identity));
    bot.point_action.on_point = touching;
    let threatened = point.last_contested_at > 0.0 && now-point.last_contested_at < 5.0;
    let time_left = frame.time_left[team_index(bot.team)];
    if point.owner != bot.team {
        bot.point_action.defending_started = false;
        let near = bot.current_area.zip(navigation.centers[index]).is_some_and(|(a,b)| (navigation.incursion[&a][team_index(bot.team)]-navigation.incursion[&b][team_index(bot.team)]).abs() < 750.0);
        let in_combat = bot.last_fire_tick.is_some_and(|last| now - last as f32 * interval < 2.0);
        let pushing = (threatened && !in_combat) || touching || frame.in_overtime || time_left < 120.0 || near;
        if pushing { bot.point_action.hunt_until = 0.0; bot.point_action.seek_area = None; }
        if !pushing && threat.is_some() && now >= bot.point_action.hunt_until {
            bot.point_action.hunt_until = now + random.random_float(15.0,30.0);
        }
        if now < bot.point_action.hunt_until && !pushing {
            return seek(bot, points, navigation, mesh, threat, random);
        }
        if touching && tick >= bot.next_repath_tick && !navigation.areas[index].is_empty() {
            let areas = &navigation.areas[index];
            let which = random.random_int(0,areas.len() as i32-1).map_err(|_| Error::Limit)? as usize;
            return Ok((ObjectiveKind::CapturePoint, random_point(mesh.area(areas[which]).unwrap(),random)));
        }
        bot.point_action.route = Some(if touching { Route::Default } else { Route::Safest });
        return Ok((ObjectiveKind::CapturePoint, if touching { bot.goal } else { point.position }));
    }
    if !bot.point_action.defending_started {
        bot.point_action.defending_started = true;
        bot.point_action.allowed_to_roam = random.random_float(0.0,100.0) < [10.0,50.0,75.0,90.0][bot.difficulty as usize];
    }
    let blocks = match bot.difficulty { Difficulty::Easy => false, Difficulty::Normal => consistent(bot,now,10.0) > 0.5, _ => true };
    let progress = points.areas().iter().find(|a| a.point == index).map_or(0.0, |a| a.progress(points.configuration()));
    let keep_blocking = bot.objective == ObjectiveKind::BlockCapture && (progress > 0.5 || threat.is_some_and(|a| distance(a.position,point.position) < 500.0));
    if (threatened && blocks) || keep_blocking {
        if tick >= bot.next_repath_tick && !navigation.areas[index].is_empty() {
            let total = navigation.areas[index].iter().map(|id| area_size(mesh.area(*id).unwrap())).sum::<f32>();
            let mut which = random.random_float(0.0,total-1.0);
            for id in &navigation.areas[index] {
                let area = mesh.area(*id).unwrap(); which -= area_size(area);
                if which <= 0.0 { return Ok((ObjectiveKind::BlockCapture,random_point(area,random))); }
            }
        }
        return Ok((ObjectiveKind::BlockCapture,bot.goal));
    }
    if point.locked || (bot.point_action.allowed_to_roam && time_left > 300.0) || (bot.class == PlayerClass::Pyro && threat.is_some()) {
        return seek(bot, points, navigation, mesh, threat, random);
    }
    if bot.point_action.defense_area.is_none() || now >= bot.point_action.idle_until {
        let areas = &navigation.defenses[index][team_index(bot.team)];
        if !areas.is_empty() {
            bot.point_action.idle_duration = random.random_float(10.0,20.0);
            bot.point_action.idle_until = now + bot.point_action.idle_duration;
            bot.point_action.defense_area = Some(areas[random.random_int(0,areas.len() as i32-1).map_err(|_| Error::Limit)? as usize]);
        }
    }
    if bot.current_area != bot.point_action.defense_area || threat.is_some() || frame.in_setup { bot.point_action.idle_until = now + bot.point_action.idle_duration; }
    Ok((ObjectiveKind::DefendPoint,bot.point_action.defense_area.and_then(|a| mesh.area(a)).map_or(point.position, Area::center)))
}

fn seek(bot: &mut Bot, points: &crate::control_point::World, navigation: &Navigation, mesh: &Mesh, threat: Option<Actor>, random: &mut UniformRandomStream) -> Result<(ObjectiveKind,[f32;3]),Error> {
    if let Some(threat) = threat.filter(|threat| distance(threat.position, bot.movement.position) < 1000.0) {
        return Ok((ObjectiveKind::Attack, threat.position));
    }
    bot.point_action.route = Some(Route::Safest);
    let reached = bot.point_action.seek_area.and_then(|id| mesh.area(id)).is_some_and(|area| area.contains_xy(bot.movement.position) && (bot.movement.position[2] - area.height(bot.movement.position[0],bot.movement.position[1])).abs() <= STEP_HEIGHT);
    if bot.point_action.seek_area.is_none() || reached || bot.path.is_empty() {
        let mut goals = navigation.thresholds[1 - team_index(bot.team)].clone();
        if let Some(point) = bot.point_action.point.filter(|index| !points.points()[*index].locked) {
            let areas = &navigation.areas[point];
            if !areas.is_empty() { goals.push(areas[random.random_int(0, areas.len() as i32 - 1).map_err(|_| Error::Limit)? as usize]); }
        }
        bot.point_action.seek_area = if goals.is_empty() { None } else { Some(goals[random.random_int(0, goals.len() as i32 - 1).map_err(|_| Error::Limit)? as usize]) };
        bot.next_repath_tick = 0;
    }
    Ok((ObjectiveKind::Attack, bot.point_action.seek_area.and_then(|id| mesh.area(id)).map_or(bot.movement.position, Area::center)))
}

fn consistent(bot: &Bot, now: f32, period: f32) -> f32 {
    bot.current_area.map_or(0.0, |area| ((bot.identity as i64 * area as i64 * ((now / period) as i64 + 1)) as f32).cos().abs())
}
fn area_size(area: &Area) -> f32 { (area.southeast[0]-area.northwest[0])*(area.southeast[1]-area.northwest[1]) }
fn random_point(area: &Area, random: &mut UniformRandomStream) -> [f32;3] {
    let x = random.random_float(area.northwest[0],area.southeast[0]); let y = random.random_float(area.northwest[1],area.southeast[1]);
    [x,y,area.height(x,y)]
}
