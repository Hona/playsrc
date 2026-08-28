//! NextBot drop-down crossings. The landing hull must clear the ledge before
//! the follower may advance to the lower NAV area (Path::ComputePathDetails).
use super::*;

#[derive(Clone, Debug)]
pub(super) struct Crossing {
    pub position: [f32; 3],
    pub drop_position: Option<[f32; 3]>,
    landing_height: f32,
}

impl Crossing {
    pub fn reached(&self, feet: [f32; 3]) -> bool {
        if self.drop_position.is_some() {
            self.landed(feet)
        } else {
            (self.position[0] - feet[0]).hypot(self.position[1] - feet[1]) < 25.0
        }
    }
    pub fn landed(&self, feet: [f32; 3]) -> bool {
        self.drop_position.is_some() && feet[2] - self.landing_height < STEP_HEIGHT
    }

    pub fn compute<W: GameplayWorld>(
        world: &W,
        from: &Area,
        to: &Area,
        direction: Direction,
        mut portal: [f32; 3],
        feet: [f32; 3],
        class: PlayerClass,
    ) -> Result<Self, playsrc_movement::Error> {
        portal[2] = from.height(portal[0], portal[1]);
        let mut result = Self {
            position: portal,
            drop_position: None,
            landing_height: 0.0,
        };
        let from_position = [feet[0], feet[1], from.height(feet[0], feet[1])];
        let lower = to.height(portal[0], portal[1]);
        let u = [
            from.southeast[0] - from.northwest[0],
            0.0,
            from.northeast_z - from.northwest[2],
        ];
        let v = [
            0.0,
            from.southeast[1] - from.northwest[1],
            from.southwest_z - from.northwest[2],
        ];
        let normal = crate::normalized([-u[2] * v[1], -u[0] * v[2], u[0] * v[1]]);
        let along = crate::sub([portal[0], portal[1], lower], from_position);
        if -along
            .into_iter()
            .zip(normal)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            <= STEP_HEIGHT
        {
            return Ok(result);
        }
        let policy = MovementPolicy {
            class,
            modifiers: MovementModifiers::default(),
        }
        .resolve();
        // Path::ComputePathDetails uses a five-unit safety inflation for both
        // the clearance hull and the maximum ledge push distance.
        let width = policy.standing_hull.maxs[0] - policy.standing_hull.mins[0] + 5.0;
        let hull = Hull {
            mins: [-width / 2.0, -width / 2.0, STEP_HEIGHT],
            maxs: [width / 2.0, width / 2.0, policy.crouched_hull.maxs[2]],
        };
        let direction = match direction {
            Direction::North => [0.0, -1.0],
            Direction::East => [1.0, 0.0],
            Direction::South => [0.0, 1.0],
            Direction::West => [-1.0, 0.0],
        };
        let mut push = 0.0;
        while push <= 2.0 * width {
            let position = [
                portal[0] + push * direction[0],
                portal[1] + push * direction[1],
                portal[2],
            ];
            let trace = world.trace(
                position,
                [position[0], position[1], lower],
                hull,
                MovementConfiguration::default().solid_mask,
            )?;
            if trace.fraction >= 1.0 {
                break;
            }
            push += 10.0;
        }
        let position = [
            portal[0] + push * direction[0],
            portal[1] + push * direction[1],
            portal[2],
        ];
        if let Some(ground) = ground_height(world, [position[0], position[1], lower])? {
            if position[2] > ground + STEP_HEIGHT {
                result.drop_position = Some(position);
                result.landing_height = ground;
            }
        }
        Ok(result)
    }
}

pub(super) fn compute<W: GameplayWorld>(
    world: &W,
    mesh: &Mesh,
    route: &[u32],
    mut position: [f32; 3],
    class: PlayerClass,
) -> Result<Vec<Crossing>, playsrc_movement::Error> {
    let mut crossings = Vec::with_capacity(route.len().saturating_sub(1));
    if let Some(first)=route.first().and_then(|identity|mesh.area(*identity)){
        let height=first.height(position[0],position[1]);
        let contains=first.contains_xy(position)&&height-STEP_HEIGHT<=position[2]
            &&!mesh.areas_at(position).any(|area|area.identity!=first.identity&&area.contains_xy(position)&&{
                let other=area.height(position[0],position[1]);other<=position[2]&&other>height
            });
        if !contains{position=first.center();}
    }
    for edge in route.windows(2) {
        let from = mesh.area(edge[0]).expect("path area");
        let to = mesh.area(edge[1]).expect("path area");
        let direction = Direction::ALL
            .into_iter()
            .find(|direction| from.connections[*direction as usize].contains(&to.identity))
            .expect("path connection");
        let portal = mesh.closest_point_in_portal(from, to, direction, position);
        let crossing = Crossing::compute(world, from, to, direction, portal, position, class)?;
        position = if let Some(drop) = crossing.drop_position {
            [drop[0], drop[1], crossing.landing_height]
        } else {
            crossing.position
        };
        crossings.push(crossing);
    }
    Ok(crossings)
}

const NAV_MASK: u32 = crate::MASK_SOLID_BRUSH_ONLY | 0x20000;

pub(super) fn avoid<W: GameplayWorld>(
    world: &W,
    mesh: &Mesh,
    bot: &mut Bot,
    goal: [f32; 3],
    now: f32,
) -> Result<[f32; 3], playsrc_movement::Error> {
    if now < bot.avoid_at {
        return Ok(goal);
    }
    bot.avoid_at = now + 0.5;
    if bot.movement.ground.is_none()
        || bot
            .current_area
            .and_then(|id| mesh.area(id))
            .is_some_and(|area| area.attributes & 4 != 0)
    {
        return Ok(goal);
    }
    let feet = bot.movement.position;
    let forward = crate::normalized([goal[0] - feet[0], goal[1] - feet[1], 0.0]);
    let left = [-forward[1], forward[0], 0.0];
    let policy = MovementPolicy {
        class: bot.class,
        modifiers: MovementModifiers::default(),
    }
    .resolve();
    let size = (policy.standing_hull.maxs[0] - policy.standing_hull.mins[0]) / 4.0;
    let offset = size + 2.0;
    let hull = Hull {
        mins: [-size, -size, STEP_HEIGHT + 0.1],
        maxs: [size, size, policy.crouched_hull.maxs[2]],
    };
    let mut obstruction = [0.0_f32; 2];
    for (index, sign) in [1.0, -1.0].into_iter().enumerate() {
        let start = [
            feet[0] + sign * offset * left[0],
            feet[1] + sign * offset * left[1],
            feet[2],
        ];
        let end = [
            start[0] + 50.0 * forward[0],
            start[1] + 50.0 * forward[1],
            start[2],
        ];
        let trace = world.trace(
            start,
            end,
            hull,
            MovementConfiguration::default().solid_mask,
        )?;
        obstruction[index] = if trace.start_solid {
            1.0
        } else {
            (1.0 - trace.fraction).clamp(0.0, 1.0)
        };
    }
    let [l, r] = obstruction;
    if (l == 0.0 && r == 0.0) || (l > 0.0 && r > 0.0 && (r - l).abs() < 0.01) {
        return Ok(goal);
    }
    let result = if l == 0.0 || (r != 0.0 && r > l) {
        -r
    } else {
        l
    };
    let direction = crate::normalized([
        0.5 * forward[0] - left[0] * result,
        0.5 * forward[1] - left[1] * result,
        0.0,
    ]);
    bot.avoid_at = 0.0;
    Ok([
        feet[0] + 100.0 * direction[0],
        feet[1] + 100.0 * direction[1],
        feet[2],
    ])
}

fn ground_height<W: GameplayWorld>(
    world: &W,
    position: [f32; 3],
) -> Result<Option<f32>, playsrc_movement::Error> {
    let mut end = [position[0], position[1], position[2] - 10000.0];
    let mut start = [position[0], position[1], position[2] + 35.5 + 0.001];
    while end[2] - position[2] < 100.0 {
        let trace = world.trace(start, end, POINT_HULL, NAV_MASK)?;
        if !trace.start_solid && (trace.fraction == 1.0 || start[2] - trace.end[2] >= 35.5) {
            return Ok(Some(trace.end[2]));
        }
        end[2] = if trace.start_solid {
            start[2]
        } else {
            trace.end[2]
        };
        start[2] = end[2] + 35.5 + 0.001;
    }
    Ok(None)
}

/// CBaseCombatCharacter::UpdateLastKnownArea retains its previous area when a
/// ledge leaves the character off-mesh. A Euclidean nearest-area replacement
/// would route it back onto that ledge halfway through the drop.
pub(super) fn update_last_known_area<W: GameplayWorld>(
    mesh: &Mesh,
    world: &W,
    bot: &mut Bot,
) -> Result<(), playsrc_movement::Error> {
    let feet = bot.movement.position;
    if bot.current_area.is_some()
        && bot
            .nav_area_mark
            .is_some_and(|mark| distance(mark, feet) <= 4.0)
        && bot
            .movement
            .ground
            .is_none_or(|ground| ground.support.is_none_or(|support| support == 0))
    {
        return Ok(());
    }
    bot.nav_area_mark = Some(feet);
    if bot
        .current_area
        .and_then(|id| mesh.area(id))
        .is_some_and(|area| {
            area.contains_xy(feet) && (area.height(feet[0], feet[1]) - feet[2]).abs() <= STEP_HEIGHT
        })
    {
        return Ok(());
    }
    let traversable = |area: &Area| {
        area.game_attributes & TF_NAV_UNBLOCKABLE != 0 || area.game_attributes & TF_NAV_BLOCKED == 0
    };
    let mut selected = None;
    let mut z = -99999999.9_f32;
    for area in mesh.areas_at(feet) {
        let height = area.height(feet[0], feet[1]);
        if area.contains_xy(feet)
            && traversable(area)
            && height <= feet[2] + STEP_HEIGHT
            && height >= feet[2] - 120.0
            && height > z
        {
            selected = Some(area.identity);
            z = height;
        }
    }
    if selected.is_some() && z < feet[2] - STEP_HEIGHT {
        let trace = world.trace(feet, [feet[0], feet[1], z], POINT_HULL, NAV_MASK)?;
        if trace.fraction != 1.0 && (trace.end[2] - z).abs() > STEP_HEIGHT {
            selected = None;
        }
    }
    if selected.is_none() && ground_height(world, feet)?.is_some() {
        let mut failure = None;
        selected = mesh
            .nearest_area_matching(feet, 50.0, |area, point| {
                let visible = || -> Result<bool, playsrc_movement::Error> {
                    if !traversable(area) {
                        return Ok(false);
                    }
                    let up = [feet[0], feet[1], feet[2] + STEP_HEIGHT];
                    let trace = world.trace(feet, up, POINT_HULL, NAV_MASK)?;
                    let safe = if trace.start_solid {
                        [trace.end[0], trace.end[1], trace.end[2] + 1.0]
                    } else {
                        feet
                    };
                    if (point[2] - safe[2]).abs() > STEP_HEIGHT
                        && world
                            .trace(
                                [point[0], point[1], point[2] + STEP_HEIGHT],
                                [point[0], point[1], safe[2]],
                                POINT_HULL,
                                NAV_MASK,
                            )?
                            .fraction
                            != 1.0
                    {
                        return Ok(false);
                    }
                    Ok(world
                        .trace(
                            safe,
                            [point[0], point[1], safe[2] + STEP_HEIGHT],
                            POINT_HULL,
                            NAV_MASK,
                        )?
                        .fraction
                        == 1.0)
                };
                match visible() {
                    Ok(value) => value,
                    Err(error) => {
                        failure = Some(error);
                        false
                    }
                }
            })
            .map(|area| area.identity);
        if let Some(error) = failure {
            return Err(error);
        }
    }
    if let Some(area) = selected {
        bot.current_area = Some(area);
    }
    Ok(())
}
