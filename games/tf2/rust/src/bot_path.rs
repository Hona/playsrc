//! NextBot drop-down crossings. The landing hull must clear the ledge before
//! the follower may advance to the lower NAV area (Path::ComputePathDetails).
use super::*;

#[derive(Clone, Debug)]
pub(super) struct Crossing {
    pub from: u32,
    pub to: u32,
    pub drop_position: Option<[f32; 3]>,
    landing_height: f32,
}

impl Crossing {
    pub fn landed(&self, feet: [f32; 3]) -> bool {
        self.drop_position.is_some() && feet[2] - self.landing_height < STEP_HEIGHT
    }

    pub fn compute<W: GameplayWorld>(world: &W, from: &Area, to: &Area, direction: Direction, mut portal: [f32; 3], feet: [f32; 3], class: PlayerClass) -> Result<Self, playsrc_movement::Error> {
        let mut result = Self { from: from.identity, to: to.identity, drop_position: None, landing_height: 0.0 };
        portal[2] = from.height(portal[0], portal[1]);
        let from_position = [feet[0], feet[1], from.height(feet[0], feet[1])];
        let lower = to.height(portal[0], portal[1]);
        let u = [from.southeast[0] - from.northwest[0], 0.0, from.northeast_z - from.northwest[2]];
        let v = [0.0, from.southeast[1] - from.northwest[1], from.southwest_z - from.northwest[2]];
        let normal = crate::normalized([-u[2] * v[1], -u[0] * v[2], u[0] * v[1]]);
        let along = crate::sub([portal[0], portal[1], lower], from_position);
        if -along.into_iter().zip(normal).map(|(a,b)|a*b).sum::<f32>() <= STEP_HEIGHT { return Ok(result); }
        let policy = MovementPolicy { class, modifiers: MovementModifiers::default() }.resolve();
        let width = policy.standing_hull.maxs[0] - policy.standing_hull.mins[0];
        let hull = Hull { mins: [-width / 2.0, -width / 2.0, STEP_HEIGHT], maxs: [width / 2.0, width / 2.0, policy.crouched_hull.maxs[2]] };
        let direction = match direction { Direction::North => [0.0,-1.0], Direction::East => [1.0,0.0], Direction::South => [0.0,1.0], Direction::West => [-1.0,0.0] };
        let mut push = 0.0;
        while push <= 2.0 * width {
            let position = [portal[0] + push * direction[0], portal[1] + push * direction[1], portal[2]];
            let trace = world.trace(position, [position[0],position[1],lower], hull, MovementConfiguration::default().solid_mask)?;
            if trace.fraction >= 1.0 { break; }
            push += 10.0;
        }
        let position = [portal[0] + push * direction[0], portal[1] + push * direction[1], portal[2]];
        // CNavMesh::GetGroundHeight raises the line through low overhangs and
        // requires half a human hull of headroom. It does not trace actors.
        let mut end = [position[0], position[1], lower - 10000.0];
        let mut start = [position[0], position[1], lower + 35.5 + 0.001];
        while end[2] - lower < 100.0 {
            let trace = world.trace(start, end, POINT_HULL, crate::MASK_SOLID_BRUSH_ONLY | 0x20000)?;
            if !trace.start_solid && (trace.fraction == 1.0 || start[2] - trace.end[2] >= 35.5) {
                if position[2] > trace.end[2] + STEP_HEIGHT {
                    result.drop_position = Some(position);
                    result.landing_height = trace.end[2];
                }
                break;
            }
            end[2] = if trace.start_solid { start[2] } else { trace.end[2] };
            start[2] = end[2] + 35.5 + 0.001;
        }
        Ok(result)
    }
}
