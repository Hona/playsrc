use playsrc_collision::{Hull, World as CollisionWorld};
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub tick: f32,
    pub max_speed: f32,
    pub acceleration: f32,
    pub air_acceleration: f32,
    pub friction: f32,
    pub stop_speed: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub step_height: f32,
    pub ground_probe: f32,
    pub standing: Hull,
    pub crouched: Hull,
    pub solid_mask: u32,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            tick: 0.015,
            max_speed: 240.,
            acceleration: 10.,
            air_acceleration: 10.,
            friction: 4.,
            stop_speed: 100.,
            gravity: 800.,
            jump_speed: 289.,
            step_height: 18.,
            ground_probe: 2.,
            standing: Hull {
                mins: [-24., -24., 0.],
                maxs: [24., 24., 82.],
            },
            crouched: Hull {
                mins: [-24., -24., 0.],
                maxs: [24., 24., 62.],
            },
            solid_mask: 0x0204_000b,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Command {
    pub forward: f32,
    pub side: f32,
    pub yaw_degrees: f32,
    pub jump: bool,
    pub crouch: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub grounded: bool,
    pub crouched: bool,
    pub jump_latched: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub end: [f32; 3],
    pub normal: Option<[f32; 3]>,
}
pub trait Tracer {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error>;
}
impl Tracer for CollisionWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<Trace, Error> {
        let t = self
            .trace_hull(start, end, hull, mask)
            .map_err(|_| Error::Trace)?;
        Ok(Trace {
            fraction: t.fraction,
            start_solid: t.start_solid,
            all_solid: t.all_solid,
            end: t.end,
            normal: t.plane.map(|p| p.normal),
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInput,
    Trace,
    Stuck,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for Error {}
pub fn advance(
    tracer: &impl Tracer,
    mut player: Player,
    command: Command,
    p: Parameters,
) -> Result<Player, Error> {
    if !finite_player(player)
        || ![
            command.forward,
            command.side,
            command.yaw_degrees,
            p.tick,
            p.max_speed,
        ]
        .iter()
        .all(|v| v.is_finite())
        || p.tick <= 0.
    {
        return Err(Error::InvalidInput);
    }
    let hull = if command.crouch {
        p.crouched
    } else {
        p.standing
    };
    player.crouched = command.crouch;
    player.grounded = grounded(tracer, player.position, hull, p)?;
    if player.grounded {
        player.velocity[2] = player.velocity[2].max(0.);
        friction(&mut player.velocity, p);
        if command.jump && !player.jump_latched {
            player.velocity[2] = p.jump_speed;
            player.grounded = false;
            player.jump_latched = true;
        }
    } else {
        player.velocity[2] -= p.gravity * p.tick * 0.5;
    }
    if !command.jump {
        player.jump_latched = false
    }
    let yaw = command.yaw_degrees.to_radians();
    let forward = [yaw.cos(), yaw.sin(), 0.];
    let right = [-yaw.sin(), yaw.cos(), 0.];
    let mut wish = [
        forward[0] * command.forward + right[0] * command.side,
        forward[1] * command.forward + right[1] * command.side,
        0.,
    ];
    let speed = length(wish);
    if speed > 0. {
        let target = speed.min(p.max_speed);
        wish[0] /= speed;
        wish[1] /= speed;
        accelerate(
            &mut player.velocity,
            wish,
            target,
            if player.grounded {
                p.acceleration
            } else {
                p.air_acceleration
            },
            p.tick,
        );
    }
    let moved = step_move(tracer, player.position, player.velocity, hull, p)?;
    player.position = moved.0;
    player.velocity = moved.1;
    if !player.grounded {
        player.velocity[2] -= p.gravity * p.tick * 0.5;
    }
    player.grounded = grounded(tracer, player.position, hull, p)?;
    if player.grounded && player.velocity[2] < 0. {
        player.velocity[2] = 0.;
    }
    Ok(player)
}
fn grounded(t: &impl Tracer, pos: [f32; 3], h: Hull, p: Parameters) -> Result<bool, Error> {
    let end = [pos[0], pos[1], pos[2] - p.ground_probe];
    let trace = t.trace(pos, end, h, p.solid_mask)?;
    Ok(trace.fraction < 1. && trace.normal.is_some_and(|n| n[2] >= 0.7))
}
fn friction(v: &mut [f32; 3], p: Parameters) {
    let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if speed < 0.1 {
        return;
    }
    let drop = p.stop_speed.max(speed) * p.friction * p.tick;
    let next = (speed - drop).max(0.) / speed;
    v[0] *= next;
    v[1] *= next;
}
fn accelerate(v: &mut [f32; 3], wish: [f32; 3], speed: f32, accel: f32, tick: f32) {
    let add = speed - dot(*v, wish);
    if add <= 0. {
        return;
    }
    let amount = (accel * tick * speed).min(add);
    v[0] += amount * wish[0];
    v[1] += amount * wish[1];
}
fn step_move(
    t: &impl Tracer,
    start: [f32; 3],
    velocity: [f32; 3],
    h: Hull,
    p: Parameters,
) -> Result<([f32; 3], [f32; 3]), Error> {
    let ordinary = slide(t, start, velocity, h, p)?;
    let up_end = [start[0], start[1], start[2] + p.step_height];
    let up = t.trace(start, up_end, h, p.solid_mask)?;
    if up.start_solid || up.all_solid {
        return Ok(ordinary);
    }
    let mut stepped = slide(t, up.end, velocity, h, p)?;
    let down_end = [stepped.0[0], stepped.0[1], stepped.0[2] - p.step_height];
    let down = t.trace(stepped.0, down_end, h, p.solid_mask)?;
    if down.fraction < 1. && down.normal.is_some_and(|n| n[2] >= 0.7) {
        stepped.0 = down.end;
        stepped.1 = clip(stepped.1, down.normal.expect("checked normal"), 1.);
    }
    let ordinary_distance = (ordinary.0[0] - start[0]).powi(2) + (ordinary.0[1] - start[1]).powi(2);
    let step_distance = (stepped.0[0] - start[0]).powi(2) + (stepped.0[1] - start[1]).powi(2);
    Ok(if step_distance > ordinary_distance {
        stepped
    } else {
        ordinary
    })
}
fn slide(
    t: &impl Tracer,
    start: [f32; 3],
    mut velocity: [f32; 3],
    h: Hull,
    p: Parameters,
) -> Result<([f32; 3], [f32; 3]), Error> {
    let mut position = start;
    let mut remaining = p.tick;
    let mut planes = Vec::new();
    for _ in 0..4 {
        if length(velocity) < 0.0001 {
            break;
        }
        let end = [
            position[0] + velocity[0] * remaining,
            position[1] + velocity[1] * remaining,
            position[2] + velocity[2] * remaining,
        ];
        let trace = t.trace(position, end, h, p.solid_mask)?;
        if trace.all_solid {
            return Err(Error::Stuck);
        }
        if trace.fraction > 0. {
            position = trace.end
        }
        if trace.fraction == 1. {
            break;
        }
        remaining *= 1. - trace.fraction;
        let Some(normal) = trace.normal else { break };
        planes.push(normal);
        let original = velocity;
        for plane in &planes {
            velocity = clip(original, *plane, 1.);
            if planes.iter().all(|other| dot(velocity, *other) >= 0.) {
                break;
            }
        }
        if planes.len() >= 2 && planes.iter().any(|plane| dot(velocity, *plane) < 0.) {
            let crease = cross(planes[0], planes[1]);
            let along = dot(crease, velocity);
            velocity = [crease[0] * along, crease[1] * along, crease[2] * along];
        }
        if dot(velocity, original) <= 0. {
            velocity = [0.; 3];
            break;
        }
    }
    Ok((position, velocity))
}
fn clip(v: [f32; 3], n: [f32; 3], over: f32) -> [f32; 3] {
    let back = dot(v, n) * over;
    let mut out = [v[0] - n[0] * back, v[1] - n[1] * back, v[2] - n[2] * back];
    for x in &mut out {
        if x.abs() < 0.1 {
            *x = 0.
        }
    }
    out
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}
fn finite_player(p: Player) -> bool {
    p.position
        .iter()
        .chain(p.velocity.iter())
        .all(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Floor;
    impl Tracer for Floor {
        fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<Trace, Error> {
            if end[2] < 0. {
                let f = start[2] / (start[2] - end[2]);
                Ok(Trace {
                    fraction: f.clamp(0., 1.),
                    start_solid: start[2] < 0.,
                    all_solid: start[2] < 0. && end[2] < 0.,
                    end: [
                        start[0] + (end[0] - start[0]) * f,
                        start[1] + (end[1] - start[1]) * f,
                        0.,
                    ],
                    normal: Some([0., 0., 1.]),
                })
            } else {
                Ok(Trace {
                    fraction: 1.,
                    start_solid: false,
                    all_solid: false,
                    end,
                    normal: None,
                })
            }
        }
    }
    #[test]
    fn accelerates_friction_and_jumps() {
        let p = Player {
            position: [0., 0., 0.],
            velocity: [0.; 3],
            grounded: true,
            crouched: false,
            jump_latched: false,
        };
        let moved = advance(
            &Floor,
            p,
            Command {
                forward: 240.,
                ..Command::default()
            },
            Parameters::default(),
        )
        .unwrap();
        assert!(moved.position[0] > 0.);
        let jumped = advance(
            &Floor,
            p,
            Command {
                jump: true,
                ..Command::default()
            },
            Parameters::default(),
        )
        .unwrap();
        assert!(jumped.velocity[2] > 250. && !jumped.grounded);
        let held = advance(
            &Floor,
            jumped,
            Command {
                jump: true,
                ..Command::default()
            },
            Parameters::default(),
        )
        .unwrap();
        assert!(held.velocity[2] < jumped.velocity[2]);
    }
}
