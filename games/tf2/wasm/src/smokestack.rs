//! TF2 (non-episodic) legacy smokestack client, separate from PCF simulation.
//! Source SDK c_smokestack.cpp, timedevent.h, particle_iterators.h and
//! particlemgr.cpp define the emission, integration and delayed sorting order.
use std::collections::BTreeMap;
use playsrc_entity::{EntityHandle, smokestack::Presentation};
use playsrc_particle::{Bounds, RenderItem, Primitive};
use playsrc_tf2::UniformRandomStream;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Particle {
    identity: u32,
    position: [f32; 3],
    velocity: [f32; 3],
    acceleration: [f32; 3],
    age: f32,
    angle: f32,
    roll_delta: f32,
    sort_z: f32,
}

#[derive(Clone)]
struct Stack {
    state: Presentation,
    particles: Vec<Particle>,
    created: f32,
    interval: f32,
    next_event: f32,
    next_identity: u32,
    first_frame: bool,
    drawn: bool,
    bounds: Bounds,
    registered_bounds: Bounds,
    last_bounds: Bounds,
    registration: u64,
    bbox_counter: u8,
    sky: bool,
}

#[derive(Clone)]
pub struct Smokestacks {
    stacks: BTreeMap<EntityHandle, Stack>,
    order: Vec<EntityHandle>,
    registration: u64,
    random: UniformRandomStream,
}

pub struct Frames {
    world: Smokestacks,
    candidate: Option<(u32, Smokestacks)>,
    pub views: [Option<RenderView>; 2],
}

impl Frames {
    pub fn new(seed: i32) -> Self { Self { world: Smokestacks::new(seed), candidate: None, views: [None, None] } }
    pub fn candidate(&mut self, accepted: u32) -> Smokestacks {
        if let Some((identity, world)) = self.candidate.take() && identity == accepted { self.world = world; }
        self.world.clone()
    }
    pub fn prepare(&mut self, identity: u32, candidate: Smokestacks) { self.candidate = Some((identity, candidate)); }
}

#[derive(Clone)]
pub struct RenderView {
    pub camera: View,
    planes: [super::WorldFrustumPlane; 4],
    leaves: Arc<[bool]>,
    leaf_order: Arc<[usize]>,
    query: [f32; 14],
}

impl RenderView {
    pub fn from_query(input: &[f32], leaves: &[usize], world: &playsrc_visibility::World, node_cull_modes: &[i8]) -> Result<Self, ()> {
        let position = [input[3], input[4], input[5]];
        let forward = angle_vectors([input[7], input[6], 0.0])[0];
        let mut allowed = vec![false; world.leaves.len()];
        for &leaf in leaves { allowed[leaf] = true; }
        let mut leaf_order = vec![usize::MAX; world.leaves.len()];
        for (order, leaf) in super::frustum_world_leaves(world, node_cull_modes, leaves, position, [input[6], input[7]], input[8], input[9])?.into_iter().enumerate() { leaf_order[leaf] = order; }
        Ok(Self { camera: View { position, forward }, planes: super::world_frustum(position, input[6], input[7], input[8], input[9]), leaves: allowed.into(), leaf_order: leaf_order.into(), query: input[..14].try_into().expect("view query prefix") })
    }

    pub fn matches(&self, input: &[f32; 14]) -> bool {
        self.query[..10] == input[..10] && self.query[13] == input[13]
    }

    pub fn render_leaf(&self, world: &playsrc_visibility::World, bounds: Bounds, registered: Bounds) -> Option<usize> {
        for plane in &self.planes {
            let corner = std::array::from_fn(|axis| if plane.normal[axis] < 0.0 { bounds.minimum[axis] } else { bounds.maximum[axis] });
            if dot(corner, plane.normal) < plane.distance { return None; }
        }
        world.leaves_in_box(playsrc_visibility::Aabb { minimum: registered.minimum, maximum: registered.maximum }).ok()?
            .into_iter().map(|leaf| self.leaf_order[leaf]).filter(|order| *order != usize::MAX).min()
    }

    pub fn in_pvs(&self, world: &playsrc_visibility::World, bounds: Bounds) -> bool {
        world.leaves_in_box(playsrc_visibility::Aabb { minimum: bounds.minimum, maximum: bounds.maximum })
            .is_ok_and(|leaves| leaves.iter().any(|&leaf| self.leaves[leaf]))
    }
}

#[derive(Clone, Copy)]
pub struct View {
    pub position: [f32; 3],
    pub forward: [f32; 3],
}

impl Smokestacks {
    pub fn new(seed: i32) -> Self {
        Self { stacks: BTreeMap::new(), order: Vec::new(), registration: 0, random: UniformRandomStream::from_seed(seed).expect("presentation seed") }
    }

    /// `visible` is the owning view's renderable bounds test, not a depth test:
    /// fully depth-occluded particles still count as drawn in Source.
    pub fn advance(&mut self, states: &[Presentation], now: f32, frame_time: f32,
        view: impl Fn(bool) -> View, is_sky: impl Fn([f32; 3]) -> bool,
        render_leaf: impl Fn(bool, Bounds, Bounds) -> Option<usize>,
        active: impl Fn(&Presentation) -> bool,
    ) -> Vec<RenderItem> {
        self.stacks.retain(|entity, _| states.iter().any(|state| state.entity == *entity));
        self.order.retain(|entity| self.stacks.contains_key(entity));
        let dt = frame_time.min(0.1);
        let mut output = Vec::new();
        for state in states {
            let active = is_sky(state.transform.origin) || active(state);
            if !active && !self.stacks.contains_key(&state.entity) { continue; }
            let stack = self.stacks.entry(state.entity).or_insert_with(|| {
                self.order.push(state.entity);
                self.registration += 1;
                Stack {
                state: state.clone(), particles: Vec::new(), created: now, interval: 1.0 / state.parameters.rate,
                next_event: 0.0, next_identity: 0, first_frame: true, drawn: false,
                bounds: Bounds { minimum: state.transform.origin, maximum: state.transform.origin }, bbox_counter: 0,
                registered_bounds: Bounds { minimum: state.transform.origin, maximum: state.transform.origin },
                last_bounds: Bounds { minimum: state.transform.origin.map(|v| v - 2.0), maximum: state.transform.origin.map(|v| v + 2.0) },
                registration: self.registration,
                sky: is_sky(state.transform.origin),
            }});
            // Rate changes update the server field but do not reinitialize TimedEvent.
            if active { stack.state = state.clone(); }
        }
        for entity in &self.order {
            let stack = self.stacks.get_mut(entity).expect("effect order");
            let state = &stack.state;
            let always = now < stack.created + 5.0;
            let parameters = &state.parameters;
            let inv_lifetime = parameters.speed / parameters.jet_length;
            let camera = view(stack.sky);
            if state.emit && (stack.drawn || always) {
                let [forward, right, up] = angle_vectors(state.transform.angles);
                let mut remaining = dt;
                while remaining >= stack.next_event {
                    remaining -= stack.next_event;
                    stack.next_event = stack.interval;
                    let angle = frand(&mut self.random, 0.0, 2.0 * std::f32::consts::PI);
                    let (sin, cos) = angle.sin_cos();
                    let position = std::array::from_fn(|axis| state.transform.origin[axis]
                        + right[axis] * (cos * parameters.base_spread) + forward[axis] * (sin * parameters.base_spread));
                    let spread_right = frand(&mut self.random, -parameters.spread_speed, parameters.spread_speed);
                    let spread_forward = frand(&mut self.random, -parameters.spread_speed, parameters.spread_speed);
                    stack.next_identity += 1;
                    // AddParticle inserts at the front of the material's linked list.
                    stack.particles.insert(0, Particle {
                        identity: stack.next_identity, position,
                        velocity: std::array::from_fn(|axis| spread_right * right[axis] + spread_forward * forward[axis] + parameters.speed * up[axis]),
                        acceleration: parameters.wind, age: 0.0, angle: 0.0,
                        roll_delta: self.random.random_float(-parameters.roll, parameters.roll), sort_z: position[2],
                    });
                }
                stack.next_event -= remaining;
            }
            if !stack.first_frame {
                stack.bbox_counter += 1;
                let recalculate = (stack.bbox_counter >= 8 && self.random.random_int(0, 8).unwrap() == 0) || stack.bbox_counter >= 16;
                if !state.emit || stack.drawn || always {
                    let twist = (parameters.twist * (std::f32::consts::PI * 2.0) / 360.0) * frame_time;
                    let (sin, cos) = twist.sin_cos();
                    stack.particles.retain_mut(|particle| {
                        particle.age += dt;
                        if particle.age * inv_lifetime >= 1.0 { return false; }
                        if twist != 0.0 {
                            let x = particle.position[0] - state.transform.origin[0];
                            let y = particle.position[1] - state.transform.origin[1];
                            particle.position[0] = x * cos + y * sin + state.transform.origin[0];
                            particle.position[1] = x * -sin + y * cos + state.transform.origin[1];
                        }
                        for axis in 0..3 {
                            particle.position[axis] = particle.position[axis] + particle.velocity[axis] * dt + particle.acceleration[axis] * (0.5 * dt * dt);
                            particle.velocity[axis] += particle.acceleration[axis] * dt;
                        }
                        particle.angle += particle.roll_delta * dt;
                        particle.sort_z = -dot(sub(particle.position, camera.position), camera.forward);
                        true
                    });
                }
                if recalculate {
                    stack.bbox_counter = 0;
                    stack.bounds = bounds(&stack.particles, state.transform.origin);
                }
            }
            stack.first_frame = false;
            // ParticleMgr's buffered relink test affects nearest-leaf ownership,
            // independently of the current exact frustum bounds.
            if (0..3).any(|axis| stack.bounds.minimum[axis] < stack.last_bounds.minimum[axis]
                || stack.bounds.minimum[axis] > stack.last_bounds.minimum[axis] + 2.6
                || stack.bounds.maximum[axis] > stack.last_bounds.maximum[axis]
                || stack.bounds.maximum[axis] < stack.last_bounds.maximum[axis] - 2.6) {
                self.registration += 1;
                stack.registration = self.registration;
                stack.registered_bounds = stack.bounds;
                stack.last_bounds = Bounds { minimum: stack.bounds.minimum.map(|v| v - 2.0), maximum: stack.bounds.maximum.map(|v| v + 2.0) };
            }
        }
        // ParticleMgr updates every effect before rendering any material. Keep
        // render-time random draws out of the update loop.
        let mut render_order = Vec::new();
        for (&entity, stack) in &mut self.stacks {
            let leaf = render_leaf(stack.sky, stack.bounds, stack.registered_bounds);
            stack.drawn = leaf.is_some();
            if let Some(leaf) = leaf {
                let camera = view(stack.sky);
                let center = std::array::from_fn(|axis| (stack.bounds.minimum[axis] + stack.bounds.maximum[axis]) * 0.5);
                render_order.push(RenderEntry { entity, sky: stack.sky, leaf, registration: stack.registration,
                    depth: dot(sub(center, camera.position), camera.forward) });
            }
        }
        sort_render_entries(&mut render_order);
        for entry in render_order.into_iter().rev() {
            let stack = self.stacks.get_mut(&entry.entity).expect("rendered effect");
            let state = &stack.state;
            let parameters = &state.parameters;
            let inv_lifetime = parameters.speed / parameters.jet_length;
            if stack.particles.is_empty() { continue; }
            let bucket_sort = self.random.random_int(0, 8).unwrap() == 0;
            for particle in &stack.particles {
                let t = particle.age * inv_lifetime;
                let mut alpha = table_cos(-std::f32::consts::PI + t * std::f32::consts::PI * 2.0) * 0.5 + 0.5;
                if t > 0.5 { alpha *= alpha; }
                alpha *= f32::from(state.color[3]);
                if alpha < 0.5 { continue; }
                let mut color = std::array::from_fn(|axis| f32::from(state.color[axis]) / 255.0);
                for light in [parameters.ambient, parameters.directional] {
                    if light.intensity != 0.0 {
                        let delta = sub(particle.position, light.position);
                        let distance = dot(delta, delta);
                        let amount = if distance > 0.0001 { light.intensity / distance } else { 1000.0 };
                        for axis in 0..3 { color[axis] += light.color[axis] * amount; }
                    }
                }
                let maximum = color[0].max(color[1].max(color[2]));
                let scale = if maximum > 1.0 { 255.0 / maximum } else { 255.0 };
                output.push(RenderItem {
                    sky: stack.sky, effect_identity: 0x5000_0000 + state.source as u32,
                    system_uuid: [0; 16], particle_identity: particle.identity, renderer_index: 0, primitive: Primitive::Sprite,
                    material: parameters.material.clone(), position: particle.position, previous_position: particle.position,
                    radius: parameters.start_size + (parameters.end_size - parameters.start_size) * t,
                    roll_radians: particle.angle.to_radians(), yaw_radians: 0.0,
                    color: color.map(|v| (v * scale).round_ties_even() as u8), opacity: alpha.round_ties_even() / 255.0,
                    sequence: 0, secondary_sequence: 0, trail_length_scale: 0.0, sort_key: particle.sort_z,
                    age_seconds: particle.age, lifetime_seconds: 1.0 / inv_lifetime,
                    animation_rate: 0.0, secondary_animation_rate: 0.0, step_seconds: dt,
                    trail_min_length: 0.0, trail_max_length: 0.0, trail_fade_in_seconds: 0.0,
                    orientation_type: 0, animation_fit_lifetime: false, animation_rate_as_fps: false,
                    primary_sheet: None, secondary_sheet: None, trail_end_position: [0.0; 3], trail_width: 0.0, trail_length: 0.0,
                    material_state: None, stable_tie_identity: (state.source as u64) << 32 | u64::from(particle.identity),
                });
            }
            // The current frame renders the *old* list. Sorting affects next frame.
            sort_after_render(&mut stack.particles, bucket_sort);
        }
        output
    }
}

struct RenderEntry { entity: EntityHandle, sky: bool, leaf: usize, registration: u64, depth: f32 }

fn sort_render_entries(entries: &mut [RenderEntry]) {
    // Leaf buckets insert at the head; Source then H-sorts each nearest leaf
    // by bounds-center view depth. The draw traverses this list backwards.
    entries.sort_by_key(|entry| (entry.sky, entry.leaf, std::cmp::Reverse(entry.registration)));
    let mut start = 0;
    while start < entries.len() {
        let mut end = start + 1;
        while end < entries.len() && (entries[end].sky, entries[end].leaf) == (entries[start].sky, entries[start].leaf) { end += 1; }
        for step in [4, 2, 1] {
            let mut index = start;
            while index + step < end {
                if entries[index].depth > entries[index + step].depth {
                    entries.swap(index, index + step);
                    if index == start { continue; }
                    index -= step;
                } else { index += step; }
            }
        }
        start = end;
    }
}

fn bounds(particles: &[Particle], origin: [f32; 3]) -> Bounds {
    let Some(first) = particles.first() else { return Bounds { minimum: origin, maximum: origin }; };
    let mut bounds = Bounds { minimum: first.position, maximum: first.position };
    for particle in &particles[1..] {
        for axis in 0..3 {
            bounds.minimum[axis] = bounds.minimum[axis].min(particle.position[axis]);
            bounds.maximum[axis] = bounds.maximum[axis].max(particle.position[axis]);
        }
    }
    bounds
}

fn sort_after_render(particles: &mut [Particle], bucket: bool) {
    if bucket {
        let minimum = particles.iter().map(|p| p.sort_z).fold(1e24_f32, f32::min);
        let maximum = particles.iter().map(|p| p.sort_z).fold(-1e24_f32, f32::max);
        // The two head insertions in DoBucketSort retain order within each bucket.
        particles.sort_by_key(|p| if maximum == minimum { 0 } else { ((p.sort_z - minimum) / (maximum - minimum) * (32.0 - 0.0001)) as i32 });
    } else {
        let mut previous = 0.0;
        for index in 0..particles.len() {
            let z = particles[index].sort_z;
            if index > 0 && previous > z { particles.swap(index - 1, index); }
            else { previous = z; }
        }
    }
}

fn table_cos(theta: f32) -> f32 {
    static TABLE: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| std::array::from_fn(|index| ((index as f64 * 2.0 * std::f64::consts::PI) / 256.0).sin() as f32));
    let index = (theta * (256.0 / (2.0 * std::f64::consts::PI)) as f32 + (12_582_912.0 + 64.0)).to_bits() & 255;
    TABLE[index as usize]
}
fn frand(random: &mut UniformRandomStream, minimum: f32, maximum: f32) -> f32 {
    minimum + (random.random_int(0, 0x7fff).unwrap() as f32 / 32767.0) * (maximum - minimum)
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { std::array::from_fn(|i| a[i] - b[i]) }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn angle_vectors(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (sp, cp) = angles[0].to_radians().sin_cos();
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    [[cp * cy, cp * sy, -sp], [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_entity::{Transform, smokestack::{Parameters, Light}};
    fn state() -> Presentation {
        Presentation { entity: EntityHandle { slot: 1, generation: 1 }, source: 1, transform: Transform::IDENTITY,
            color: [241, 158, 103, 255], emit: true, parameters: Parameters {
                start_size: 10.0, end_size: 5.0, base_spread: 0.0, spread_speed: 10.0, speed: 45.0,
                rate: 15.0, jet_length: 32.0, twist: 10.0, roll: 6.0, wind: [0.0; 3], material: "particle/smokesprites_0001".into(),
                ambient: Light::default(), directional: Light::default(),
            } }
    }
    fn advance(world: &mut Smokestacks, state: &Presentation, now: f32, dt: f32, visible: bool) -> Vec<RenderItem> {
        world.advance(std::slice::from_ref(state), now, dt, |_| View { position: [-100.0, 0.0, 0.0], forward: [1.0, 0.0, 0.0] }, |_| false, |_, _, _| visible.then_some(0), |_| true)
    }
    #[test]
    fn immediate_clock_first_frame_rate_input_and_stop_restart() {
        let mut world = Smokestacks::new(1337);
        let mut state = state();
        assert!(advance(&mut world, &state, 0.0, 0.015, true).is_empty());
        assert_eq!(world.stacks[&state.entity].particles.len(), 1);
        assert_eq!(world.stacks[&state.entity].particles[0].age, 0.0);
        state.parameters.rate = 500.0;
        for frame in 1..=20 { advance(&mut world, &state, frame as f32 * 0.015, 0.015, true); }
        assert_eq!(world.stacks[&state.entity].next_identity, 5);
        let clock = world.stacks[&state.entity].next_event;
        state.emit = false;
        for frame in 21..=90 { advance(&mut world, &state, frame as f32 * 0.015, 0.015, false); }
        assert!(world.stacks[&state.entity].particles.is_empty());
        assert_eq!(world.stacks[&state.entity].next_event, clock);
        state.emit = true;
        advance(&mut world, &state, 1.5, clock, true);
        assert_eq!(world.stacks[&state.entity].next_identity, 6);
    }
    #[test]
    fn exact_wind_twist_roll_size_color_alpha_and_material() {
        let mut world = Smokestacks::new(1337);
        let mut state = state();
        state.parameters.wind = [2.0, -4.0, 6.0];
        state.parameters.base_spread = 8.0;
        advance(&mut world, &state, 0.0, 0.0, true);
        let born = world.stacks[&state.entity].particles[0].clone();
        assert!((born.position[0].hypot(born.position[1]) - 8.0).abs() < 1e-6);
        assert_eq!(born.angle, 0.0);
        let output = advance(&mut world, &state, 0.03, 0.03, true);
        let particle = &world.stacks[&state.entity].particles[0];
        let (sin, cos) = (10.0_f32.to_radians() * 0.03).sin_cos();
        assert!((particle.position[0] - (born.position[0] * cos + born.position[1] * sin + born.velocity[0] * 0.03 + 2.0 * 0.5 * 0.03 * 0.03)).abs() < 1e-6);
        assert!((particle.position[2] - (45.0 * 0.03 + 6.0 * 0.5 * 0.03 * 0.03)).abs() < 1e-6);
        assert_eq!(particle.velocity[2], 45.0 + 6.0 * 0.03);
        assert_eq!(particle.angle, born.roll_delta * 0.03);
        assert_eq!(output[0].color, [241, 158, 103]);
        assert_eq!(output[0].radius, 10.0 + (5.0 - 10.0) * (0.03 * (45.0 / 32.0)));
        assert_eq!(output[0].material, "particle/smokesprites_0001");
        assert_eq!(output[0].animation_rate, 0.0);
        assert_eq!(output[0].opacity * 255.0, (output[0].opacity * 255.0).round());
        assert_eq!(table_cos(0.0), 1.0);
        assert_eq!(table_cos(std::f32::consts::PI), -1.0);
    }
    #[test]
    fn hidden_emission_freezes_after_five_seconds_but_turnoff_drains_and_removal_frees() {
        let mut world = Smokestacks::new(1);
        let mut state = state();
        for frame in 0..=350 { advance(&mut world, &state, frame as f32 * 0.015, 0.015, false); }
        let stack = &world.stacks[&state.entity];
        let ages: Vec<_> = stack.particles.iter().map(|p| p.age).collect();
        let count = stack.next_identity;
        for frame in 351..=500 { advance(&mut world, &state, frame as f32 * 0.015, 0.015, false); }
        assert_eq!(world.stacks[&state.entity].next_identity, count);
        assert_eq!(world.stacks[&state.entity].particles.iter().map(|p| p.age).collect::<Vec<_>>(), ages);
        advance(&mut world, &state, 8.0, 0.015, true);
        advance(&mut world, &state, 8.015, 0.015, true);
        assert_ne!(world.stacks[&state.entity].particles.iter().map(|p| p.age).collect::<Vec<_>>(), ages);
        state.emit = false;
        for frame in 0..60 { advance(&mut world, &state, 9.0 + frame as f32 * 0.015, 0.015, false); }
        assert!(world.stacks[&state.entity].particles.is_empty());
        world.advance(&[], 10.0, 0.015, |_| unreachable!(), |_| unreachable!(), |_, _, _| unreachable!(), |_| unreachable!());
        assert!(world.stacks.is_empty());
    }
    #[test]
    fn forty_six_emitters_stabilize_without_emission_or_quality_cuts() {
        let mut world = Smokestacks::new(1);
        let states: Vec<_> = (0..46).map(|index| { let mut state = state(); state.entity.slot = index; state.source = usize::from(index); state }).collect();
        for frame in 0..=1000 {
            world.advance(&states, frame as f32 * 0.015, 0.015, |_| View { position: [0.0; 3], forward: [1.0, 0.0, 0.0] }, |_| false, |_, _, _| Some(0), |_| true);
        }
        assert_eq!(world.stacks.len(), 46);
        for stack in world.stacks.values() {
            assert_eq!(stack.next_identity, 226);
            assert!((10..=11).contains(&stack.particles.len()));
            assert!(stack.particles.capacity() <= 16);
        }
    }
    #[test]
    fn incremental_and_bucket_sort_change_next_frame_list_not_the_current_draw() {
        let mut world = Smokestacks::new(1);
        let state = state(); advance(&mut world, &state, 0.0, 0.0, true);
        let particle = world.stacks[&state.entity].particles[0].clone();
        let mut particles: Vec<_> = [3.0, 2.0, 1.0, 0.0].into_iter().enumerate().map(|(index, z)| Particle { identity: index as u32, sort_z: z, ..particle.clone() }).collect();
        sort_after_render(&mut particles, false);
        assert_eq!(particles.iter().map(|p| p.sort_z).collect::<Vec<_>>(), [2.0, 1.0, 0.0, 3.0]);
        sort_after_render(&mut particles, true);
        assert_eq!(particles.iter().map(|p| p.sort_z).collect::<Vec<_>>(), [0.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn only_acknowledged_frames_commit_particle_clock_and_random_stream() {
        let mut frames = Frames::new(1337);
        let state = state();
        let mut first = frames.candidate(0);
        advance(&mut first, &state, 0.0, 0.0, true);
        frames.prepare(1, first);
        let mut second = frames.candidate(1);
        advance(&mut second, &state, 0.05, 0.05, true);
        frames.prepare(2, second);
        let third = frames.candidate(1);
        assert_eq!(third.stacks[&state.entity].particles[0].age, 0.0);
        assert_eq!(third.stacks[&state.entity].next_identity, 1);
        assert_eq!(third.random.state(), frames.world.random.state());
    }

    #[test]
    fn particle_dt_caps_at_point_one_but_twist_uses_uncapped_client_frame() {
        let mut world = Smokestacks::new(1337);
        let mut state = state(); state.parameters.base_spread = 8.0;
        advance(&mut world, &state, 0.0, 0.0, true);
        let before = world.stacks[&state.entity].particles[0].clone();
        state.emit = false;
        advance(&mut world, &state, 0.4, 0.4, true);
        let particle = &world.stacks[&state.entity].particles[0];
        let (sin, cos) = (state.parameters.twist * (std::f32::consts::PI * 2.0) / 360.0 * 0.4).sin_cos();
        assert_eq!(particle.age, 0.1);
        assert!((particle.position[0] - (before.position[0] * cos + before.position[1] * sin + before.velocity[0] * 0.1)).abs() < 1e-6);
        assert_eq!(particle.angle, before.roll_delta * 0.1);
    }

    #[test]
    fn client_creation_and_dormant_updates_follow_pvs_not_frustum() {
        let mut world = Smokestacks::new(1);
        let mut state = state();
        let view = |_| View { position: [0.0; 3], forward: [1.0, 0.0, 0.0] };
        world.advance(&[state.clone()], 0.0, 0.015, view, |_| false, |_, _, _| None, |_| false);
        assert!(world.stacks.is_empty());
        world.advance(&[state.clone()], 1.0, 0.015, view, |_| false, |_, _, _| None, |_| true);
        assert_eq!(world.stacks[&state.entity].created, 1.0);
        state.emit = false;
        world.advance(&[state.clone()], 7.0, 0.015, view, |_| false, |_, _, _| None, |_| false);
        assert!(world.stacks[&state.entity].state.emit);
        assert_eq!(world.stacks[&state.entity].particles[0].age, 0.0);
        world.advance(&[state.clone()], 7.015, 0.015, view, |_| false, |_, _, _| Some(0), |_| true);
        assert!(!world.stacks[&state.entity].state.emit);
        assert_eq!(world.stacks[&state.entity].particles[0].age, 0.015);
    }

    #[test]
    fn render_order_uses_nearest_leaf_then_bounds_depth_and_keeps_sky_separate() {
        let entry = |slot, sky, leaf, depth, registration| RenderEntry { entity: EntityHandle { slot, generation: 1 }, sky, leaf, depth, registration };
        let mut entries = vec![entry(1, false, 0, 200.0, 1), entry(2, false, 1, 10.0, 2),
            entry(3, false, 0, 100.0, 3), entry(4, true, 0, 5.0, 4), entry(5, false, 0, 100.0, 5)];
        sort_render_entries(&mut entries);
        assert_eq!(entries.iter().rev().map(|entry| entry.entity.slot).collect::<Vec<_>>(), [4, 2, 1, 3, 5]);
    }
}
