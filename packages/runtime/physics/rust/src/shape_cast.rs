use crate::{EdgeId, FeatureTopology, ShapeError, SourceAngleBasis, normalize_source_vector};
use playsrc_collision::{AuthoredHullRef, Hull, PhysicsShape};
use std::sync::Arc;

type V = [f32; 3];
fn add(a: V, b: V) -> V { std::array::from_fn(|i| a[i] + b[i]) }
fn sub(a: V, b: V) -> V { std::array::from_fn(|i| a[i] - b[i]) }
fn scale(a: V, b: f32) -> V { a.map(|a| a * b) }
fn neg(a: V) -> V { a.map(|a| -a) }
fn dot(a: V, b: V) -> f32 { (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2] }
fn cross(a: V, b: V) -> V { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
fn unit(a: V) -> Result<V, ShapeError> { normalize_source_vector(a).map_err(|_| ShapeError::NonFinite) }
fn plane_distance(direction: V, point: V, normal: V) -> f32 {
    let projection = dot(direction, normal);
    if projection > 0.0 { dot(point, normal) / projection } else { 1e24 }
}

#[derive(Clone, Copy, Debug, Default)]
struct Witness {
    point: V,
    corner: u8,
    vertex: usize,
}

#[derive(Default)]
struct Polytope {
    points: [Witness; 4],
    count: usize,
}

impl Polytope {
    fn point(&mut self, incoming: Witness) -> V {
        self.points[0] = incoming;
        self.count = 1;
        incoming.point
    }

    fn edge(&mut self, incoming: Witness, endpoint: usize, delta: V) -> V {
        if endpoint == 1 { self.points[0] = incoming; }
        else { self.points[0] = self.points[endpoint]; self.points[1] = incoming; }
        self.count = 2;
        cross(cross(delta, incoming.point), delta)
    }

    fn face(&mut self, incoming: Witness, face: usize, normal: V) -> V {
        self.points[(face + 2) % 3] = incoming;
        self.count = 3;
        neg(normal)
    }

    fn triangle(&mut self, incoming: Witness) -> V {
        let a = incoming.point;
        let edges = [sub(self.points[0].point, a), sub(self.points[1].point, a)];
        let normal = cross(edges[0], edges[1]);
        if dot(cross(normal, edges[1]), a) < 0.0 {
            if dot(edges[1], a) < 0.0 { return self.edge(incoming, 1, edges[1]); }
        } else if dot(cross(edges[0], normal), a) > 0.0 {
            self.count = 3;
            if dot(normal, a) < 0.0 { self.points[2] = incoming; return neg(normal); }
            self.points[2] = self.points[1];
            self.points[1] = incoming;
            return normal;
        }
        if dot(edges[0], a) < 0.0 { self.edge(incoming, 0, edges[0]) } else { self.point(incoming) }
    }

    fn include(&mut self, incoming: Witness) -> Option<V> {
        let a = incoming.point;
        match self.count {
            0 => Some(self.point(incoming)),
            1 => {
                let edge = sub(self.points[0].point, a);
                Some(if dot(edge, a) < 0.0 { self.edge(incoming, 0, edge) } else { self.point(incoming) })
            }
            2 => Some(self.triangle(incoming)),
            3 => {
                let edges: [V; 3] = std::array::from_fn(|i| sub(self.points[i].point, a));
                let normals: [V; 3] = std::array::from_fn(|i| cross(edges[i], edges[(i + 1) % 3]));
                let outside: [bool; 3] = normals.map(|normal| dot(normal, a) < 0.0);
                match outside.into_iter().filter(|value| *value).count() {
                    0 => { self.points[3] = incoming; self.count = 4; None }
                    1 => {
                        if outside[1] { self.points[0] = self.points[2]; }
                        if outside[2] { self.points[1] = self.points[2]; }
                        Some(self.triangle(incoming))
                    }
                    3 => {
                        Some(match (0..3).find(|&i| dot(edges[i], a) < 0.0) {
                            Some(i) => self.edge(incoming, i, edges[i]),
                            None => self.point(incoming),
                        })
                    }
                    _ => {
                        let left = (outside.iter().position(|outside| !outside).unwrap() + 1) % 3;
                        let shared = (left + 1) % 3;
                        let right = (left + 2) % 3;
                        let next = if dot(cross(normals[left], edges[shared]), a) < 0.0 {
                            if dot(cross(edges[shared], normals[shared]), a) < 0.0 {
                                if dot(edges[shared], a) < 0.0 { self.edge(incoming, shared, edges[shared]) } else { self.point(incoming) }
                            } else if dot(cross(normals[shared], edges[right]), a) < 0.0 {
                                self.edge(incoming, right, edges[right])
                            } else { self.face(incoming, shared, normals[shared]) }
                        } else if dot(cross(edges[left], normals[left]), a) < 0.0 {
                            if dot(edges[left], a) < 0.0 { self.edge(incoming, left, edges[left]) } else { self.point(incoming) }
                        } else { self.face(incoming, left, normals[left]) };
                        Some(next)
                    }
                }
            }
            _ => None,
        }
    }

    fn exit_face(&mut self, direction: V, first: bool) -> V {
        let a = self.points[3].point;
        let edges: [V; 3] = std::array::from_fn(|i| sub(self.points[i].point, a));
        let mut normals = [[0.0; 3]; 4];
        for i in 0..3 { normals[i] = cross(edges[i], edges[(i + 1) % 3]); }
        normals[3] = cross(sub(self.points[2].point, self.points[0].point), sub(self.points[1].point, self.points[0].point));
        let mut selected = if first { 3 } else { 2 };
        let distance = |i| plane_distance(direction, if i == 3 { self.points[0].point } else { a }, normals[i]);
        let mut nearest = distance(selected);
        for i in 0..selected {
            let candidate = distance(i);
            if candidate < nearest { nearest = candidate; selected = i; }
        }
        self.count = 3;
        if selected == 3 { self.points.swap(1, 2); }
        else { self.points[(selected + 2) % 3] = self.points[3]; }
        normals[selected]
    }

    fn planar_exit(&self, direction: V, epsilon: f32) -> Result<f32, ShapeError> {
        if self.count != 3 { return Ok(0.0); }
        let a = sub(self.points[0].point, self.points[2].point);
        let b = sub(self.points[1].point, self.points[2].point);
        let c = sub(self.points[1].point, self.points[0].point);
        let normal = unit(cross(a, b))?;
        let edges = [unit(cross(a, normal))?, unit(cross(normal, b))?, unit(cross(c, normal))?];
        let points = [self.points[0].point, self.points[1].point, self.points[1].point];
        let mut best = 0;
        let mut distance = plane_distance(direction, points[0], edges[0]);
        for i in 1..3 {
            let next = plane_distance(direction, points[i], edges[i]);
            if next < distance { best = i; distance = next; }
        }
        let projection = dot(direction, edges[best]);
        Ok(if projection <= 0.0 { 1e24 } else { distance + epsilon / projection })
    }
}

#[derive(Clone, Copy)]
struct Segment {
    start: V,
    end: V,
    delta: V,
    direction: V,
    length: f32,
    base_length: f32,
    inverse_length: f32,
}

impl Segment {
    fn new(start: V, delta: V) -> Self {
        let length = dot(delta, delta).sqrt();
        let inverse = if length > 0.0 { 1.0 / length } else { 0.0 };
        Self { start, end: add(start, delta), delta, direction: if length > 0.0 { scale(delta, inverse) } else { delta }, length, base_length: length, inverse_length: inverse }
    }
    fn shorten(&mut self, fraction: f32) {
        self.length = self.base_length * fraction;
        self.end = add(self.start, scale(self.delta, fraction));
    }
}

#[derive(Debug)]
struct Part {
    topology: FeatureTopology,
    compact: bool,
    seeds: [EdgeId; 8],
    contents: u32,
}

#[derive(Debug)]
pub struct ShapeCastModel {
    shape: Arc<PhysicsShape>,
    parts: Vec<Part>,
}

struct Support<'a> {
    part: &'a Part,
    basis: [f32; 9],
    points: [V; 128],
}

impl<'a> Support<'a> {
    fn new(part: &'a Part, basis: [f32; 9]) -> Self {
        let mut support = Self { part, basis, points: [[0.0; 3]; 128] };
        if part.compact {
            for index in 0..part.topology.points().len().div_ceil(4) * 4 {
                support.points[index] = support.position(part.topology.points()[index.min(part.topology.points().len() - 1)]);
            }
        }
        support
    }
    fn position(&self, point: V) -> V {
        std::array::from_fn(|axis| {
            let row = &self.basis[axis * 3..axis * 3 + 3];
            let factor = crate::units::INCHES_PER_METER;
            (point[0] * (row[0] * factor) + point[1] * (-row[2] * factor)) + point[2] * (row[1] * factor)
        })
    }
    fn local(&self, direction: V, factor: f32) -> V {
        let value = direction.map(|value| value * factor);
        let local: V = std::array::from_fn(|axis| ((f64::from(value[0]) * f64::from(self.basis[axis])
            + f64::from(value[2]) * f64::from(self.basis[6 + axis])) + f64::from(value[1]) * f64::from(self.basis[3 + axis])) as f32);
        [local[0], -local[2], local[1]]
    }
    fn point(&self, index: usize) -> V {
        if self.part.compact { self.points[index] } else { self.position(self.part.topology.points()[index]) }
    }
    fn extreme(&self, direction: V) -> Result<usize, ShapeError> {
        if self.part.compact {
            let count = self.part.topology.points().len().div_ceil(4) * 4;
            let mut index = 0;
            let mut score = dot(direction, self.points[0]);
            for lane in [0, 2, 1, 3] {
                for next in (lane..count).step_by(4) {
                    let value = dot(direction, self.points[next]);
                    if value > score { index = next; score = value; }
                }
            }
            Ok(index)
        } else {
            let direction = self.local(direction, 1.0);
            let octant = usize::from(direction[0] < 0.0) | usize::from(direction[1] < 0.0) << 1 | usize::from(direction[2] < 0.0) << 2;
            let edge = crate::shape::extreme_edge(&self.part.topology, direction, self.part.seeds[octant], true)?;
            Ok(self.part.topology.edge(edge).map_err(ShapeError::Topology)?.start as usize)
        }
    }
    fn sample(&self, direction: V, extents: V, point: bool, segment: Segment) -> Result<Witness, ShapeError> {
        let corner = if point { 0 } else { direction.iter().enumerate().fold(0, |bits, (axis, value)| bits | ((value.to_bits() >> 31) as u8) << axis) };
        let hull = if point { [0.0; 3] } else { std::array::from_fn(|axis| if corner & (1 << axis) != 0 { -extents[axis] } else { extents[axis] }) };
        let ray = if dot(direction, segment.delta) > 0.0 { segment.end } else { segment.start };
        let vertex = self.extreme(neg(direction))?;
        Ok(Witness { point: sub(add(hull, ray), self.point(vertex)), corner, vertex })
    }

    fn normal(&self, polytope: &Polytope, segment: Segment, extents: V, point: bool) -> Result<V, ShapeError> {
        let mut corners = Vec::with_capacity(4);
        let mut vertices = Vec::with_capacity(4);
        for witness in &polytope.points[..polytope.count] {
            if !corners.contains(&witness.corner) { corners.push(witness.corner); }
            if !vertices.contains(&witness.vertex) { vertices.push(witness.vertex); }
        }
        let corner = |index: usize| -> V {
            if point { [0.0; 3] } else { std::array::from_fn(|axis| if corners[index] & (1 << axis) != 0 { -extents[axis] } else { extents[axis] }) }
        };
        if polytope.count == 2 && corners.len() == 2 {
            let edge = sub(corner(1), corner(0));
            return unit(cross(edge, cross(edge, segment.delta)));
        }
        let normal = if corners.len() == 3 {
            cross(sub(corner(1), corner(0)), sub(corner(2), corner(0)))
        } else if corners.len() == 2 && vertices.len() == 2 {
            cross(sub(corner(1), corner(0)), sub(self.point(vertices[1]), self.point(vertices[0])))
        } else if vertices.len() == 3 {
            cross(sub(self.point(vertices[1]), self.point(vertices[0])), sub(self.point(vertices[2]), self.point(vertices[0])))
        } else { return Ok(neg(segment.direction)); };
        let normal = unit(normal)?;
        Ok(if dot(normal, segment.delta) > 0.0 { neg(normal) } else { normal })
    }

    fn exit_distance(&self, polytope: &mut Polytope, extents: V, point: bool, segment: Segment) -> Result<f32, ShapeError> {
        if polytope.count < 4 { return Ok(0.0); }
        let mut direction = polytope.exit_face(segment.direction, true);
        for _ in 0..100 {
            direction = unit(direction)?;
            let incoming = self.sample(direction, extents, point, segment)?;
            if dot(direction, incoming.point) <= dot(direction, polytope.points[0].point) + 1.0 / 256.0 {
                let normal = cross(sub(polytope.points[1].point, polytope.points[0].point), sub(polytope.points[2].point, polytope.points[0].point));
                return Ok(plane_distance(segment.direction, polytope.points[0].point, normal));
            }
            polytope.points[polytope.count] = incoming;
            polytope.count += 1;
            direction = polytope.exit_face(segment.direction, false);
        }
        Ok(0.0)
    }

    fn sweep(&self, extents: V, point: bool, segment: Segment, closest: &mut V) -> Result<Option<LocalHit>, ShapeError> {
        const EPSILON: f32 = 1.0 / 32.0;
        let mut polytope = Polytope::default();
        if *closest == [0.0; 3] { *closest = [1.0, 0.0, 0.0]; }
        let mut direction = unit(neg(*closest))?;
        for _ in 0..100 {
            let mut incoming = self.sample(direction, extents, point, segment)?;
            let projection = dot(direction, incoming.point);
            if projection < 0.0 {
                let mut separation = projection.abs();
                if separation >= EPSILON || segment.length <= 0.0 { return Ok(None); }
                let mut normal = direction;
                if separation > 0.0 {
                    for _ in 0..20 {
                        let previous = incoming.point;
                        if let Some(next) = polytope.include(incoming) { *closest = next; }
                        direction = unit(neg(*closest))?;
                        incoming = self.sample(direction, extents, point, segment)?;
                        let next = -dot(direction, incoming.point);
                        if next > EPSILON { return Ok(None); }
                        if next > separation { separation = next; normal = direction; }
                        if next - -dot(direction, previous) > -1e-4_f32 { break; }
                    }
                }
                let speed = -dot(segment.delta, normal);
                if !(f64::from(speed) < -(f64::from(EPSILON) * 0.1)
                    || speed < -1e-4_f32 && f64::from(separation) < f64::from(EPSILON) * 0.9) { return Ok(None); }
                let backup = -((EPSILON - separation) * segment.base_length) / speed;
                let mut distance = segment.length - backup;
                if distance < 0.0 {
                    distance = 0.0;
                    let backup = polytope.planar_exit(segment.direction, EPSILON)?;
                    if segment.length > backup { distance = segment.length - backup; }
                }
                return Ok(Some(LocalHit { distance, normal: neg(normal), inside: false }));
            }
            match polytope.include(incoming) {
                Some(next) => { *closest = next; direction = unit(neg(next))?; }
                None => {
                    if segment.length != 0.0 {
                        let exit = self.exit_distance(&mut polytope, extents, point, segment)?;
                        if exit < segment.length && exit > 0.0 {
                            let normal = self.normal(&polytope, segment, extents, point)?;
                            let mut distance = segment.length - exit;
                            let speed = dot(segment.direction, normal);
                            if speed < 0.0 { distance += EPSILON / speed; }
                            return Ok(Some(LocalHit { distance: distance.max(0.0), normal, inside: false }));
                        }
                    }
                    return Ok(Some(LocalHit { distance: 0.0, normal: [0.0; 3], inside: true }));
                }
            }
        }
        Ok(None)
    }
}

struct LocalHit {
    distance: f32,
    normal: V,
    inside: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeCastResult {
    pub convex: Option<usize>,
    pub fraction: f32,
    pub start_solid: bool,
    pub all_solid: bool,
    pub normal: V,
    pub contents: u32,
    pub start: V,
    pub end: V,
    pub plane_distance: f32,
}

impl ShapeCastModel {
    pub fn new(shape: Arc<PhysicsShape>) -> Result<Self, ShapeError> {
        if shape.authored_hierarchy().is_none() { return Err(ShapeError::MissingAuthoredProperties); }
        let mut parts = Vec::with_capacity(shape.convex_count());
        for convex in 0..shape.convex_count() {
            let geometry = shape.authored_convex(convex).ok_or(ShapeError::MissingAuthoredConvex { convex })?;
            let topology = FeatureTopology::from_collision(&shape, convex).map_err(ShapeError::Topology)?;
            let mut indices = geometry.triangles.iter().flat_map(|triangle| triangle.edge_words()).map(|word| word & 0xffff);
            let first = indices.next().ok_or(ShapeError::MissingAuthoredConvex { convex })?;
            let (low, high) = indices.fold((first, first), |(low, high), index| (low.min(index), high.max(index)));
            let compact = high - low + 1 < 128 && geometry.triangles.len() <= 512;
            let mut seeds = [topology.edge_id(0).ok_or(ShapeError::MissingAuthoredConvex { convex })?; 8];
            if !compact {
                for (octant, seed) in seeds.iter_mut().enumerate() {
                    let direction = std::array::from_fn(|axis| if octant & (1 << axis) == 0 { 1.0 } else { -1.0 });
                    let selected = crate::shape::extreme_edge(&topology, direction, *seed, false)?;
                    let edge = topology.edge(selected).map_err(ShapeError::Topology)?;
                    let packed = ((edge.face << 2) | edge.position) as u16;
                    *seed = topology.edge_id(usize::from(packed >> 2) * 3 + usize::from(packed & 3)).ok_or(ShapeError::MissingAuthoredConvex { convex })?;
                }
            }
            let contents = shape.convex_contents(convex).ok_or(ShapeError::MissingAuthoredConvex { convex })?;
            parts.push(Part { topology, compact, seeds, contents });
        }
        Ok(Self { shape, parts })
    }

    pub fn trace(&self, start: V, end: V, hull: Hull, origin: V, angles: V, mask: u32) -> Result<ShapeCastResult, ShapeError> {
        if start.into_iter().chain(end).chain(hull.mins).chain(hull.maxs).chain(origin).chain(angles).any(|value| !value.is_finite()) { return Err(ShapeError::NonFinite); }
        if (0..3).any(|axis| hull.mins[axis] > hull.maxs[axis]) { return Err(ShapeError::InvalidHull); }
        let basis = SourceAngleBasis::from_degrees(angles).map_err(ShapeError::Orientation)?.matrix;
        let center = scale(add(hull.mins, hull.maxs), 0.5);
        let extents = scale(sub(hull.maxs, hull.mins), 0.5);
        let point = dot(extents, extents) < 1e-6_f32;
        let delta = sub(end, start);
        let reference = add(add(start, center), neg(center));
        let mut ray = Segment::new(sub(add(start, center), origin), delta);
        if !ray.length.is_finite() { return Err(ShapeError::NonFinite); }
        let mut result = ShapeCastResult { convex: None, fraction: 1.0, start_solid: false, all_solid: false, normal: [0.0; 3], contents: 0,
            start: reference, end: add(reference, delta), plane_distance: 0.0 };
        let hierarchy = self.shape.authored_hierarchy().ok_or(ShapeError::MissingAuthoredProperties)?;
        let transform = Support::new(self.parts.first().ok_or(ShapeError::MissingAuthoredProperties)?, basis);
        let local_start = transform.local(ray.start, crate::units::METERS_PER_INCH);
        let local_delta = transform.local(ray.delta, crate::units::METERS_PER_INCH);
        let mut local_direction = local_delta;
        let square = f64::from(dot(local_direction, local_direction));
        if square >= 1e-20 {
            let inverse = crate::arithmetic::refined_inverse_root::<4>(square);
            local_direction = local_direction.map(|value| (f64::from(value) * inverse) as f32);
        }
        let radius = if point { 0.0 } else { dot(extents, extents).sqrt() };
        let mut fraction = 1.0;
        let mut distance = ray.base_length.max(1e-8_f32);
        let mut closest = ray.start;
        let mut pending = vec![0];
        while let Some(index) = pending.pop() {
            let node = hierarchy.nodes.get(index).ok_or(ShapeError::MissingAuthoredProperties)?;
            let center = add(local_start, scale(local_delta, 0.5 * fraction));
            let intersects = |index: usize| -> bool {
                let node = &hierarchy.nodes[index];
                let gap = sub(node.center(), center);
                let limit = node.radius() + radius;
                if ray.length * crate::units::METERS_PER_INCH > 0.0 { dot(cross(local_direction, gap), cross(local_direction, gap)) < limit * limit }
                else { dot(gap, gap) < limit * limit }
            };
            if index == 0 && !intersects(index) { continue; }
            if let Some(children) = node.children {
                let live = children.map(intersects);
                match live {
                    [true, true] => {
                        let distances = children.map(|index| { let gap = sub(local_start, hierarchy.nodes[index].center()); dot(gap, gap) });
                        let near = usize::from(distances[0] >= distances[1]);
                        pending.push(children[1 - near]); pending.push(children[near]);
                    }
                    [true, false] => pending.push(children[0]),
                    [false, true] => pending.push(children[1]),
                    _ => {}
                }
                continue;
            }
            let Some(AuthoredHullRef::Piece(piece)) = node.hull else { return Err(ShapeError::MissingAuthoredProperties); };
            let part = self.parts.get(piece).ok_or(ShapeError::MissingAuthoredConvex { convex: piece })?;
            if part.contents & mask == 0 { continue; }
            if let Some(hit) = Support::new(part, basis).sweep(extents, point, ray, &mut closest)? {
                if hit.distance < distance {
                    distance = hit.distance;
                    result.fraction = distance * ray.inverse_length;
                    result.normal = hit.normal;
                    result.start_solid = hit.inside;
                    result.all_solid = hit.inside;
                    result.contents = part.contents;
                    result.convex = Some(piece);
                    let next = (distance + 2.0 / 32.0) * ray.inverse_length;
                    if next < 1.0 { fraction = next; ray.shorten(next); }
                }
            }
        }
        result.end = add(reference, scale(delta, result.fraction));
        if result.fraction < 1.0 || result.start_solid || result.all_solid { result.plane_distance = dot(result.end, result.normal); }
        Ok(result)
    }
}

impl playsrc_collision::PhysicsQuery for ShapeCastModel {
    fn geometry(&self) -> &PhysicsShape { &self.shape }

    fn bounds(&self, transform: playsrc_collision::Transform) -> Result<Hull, playsrc_collision::Error> {
        let basis = SourceAngleBasis::from_degrees(transform.angles).map_err(|error| query_error(ShapeError::Orientation(error)))?;
        crate::source_shape_bounds(&self.shape, transform.origin, basis).map_err(query_error)
    }

    fn trace(&self, request: playsrc_collision::ObjectTraceRequest) -> Result<(playsrc_collision::BoundsTrace, Option<usize>), playsrc_collision::Error> {
        let result = self.trace(request.start, request.end, request.hull, request.transform.origin, request.transform.angles, request.mask)
            .map_err(query_error)?;
        Ok((playsrc_collision::BoundsTrace { start: result.start, end: result.end, fraction: result.fraction, fraction_left_solid: 0.0,
            start_solid: result.start_solid, all_solid: result.all_solid, contents: result.contents,
            plane: (result.fraction < 1.0 || result.start_solid || result.all_solid).then_some(playsrc_collision::Plane { normal: result.normal, distance: result.plane_distance, kind: 0 }),
        }, result.convex))
    }

    fn storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.parts.capacity() * std::mem::size_of::<Part>()
            + self.parts.iter().map(|part| part.topology.storage_bytes()).sum::<usize>()
    }
}

fn query_error(error: ShapeError) -> playsrc_collision::Error {
    use playsrc_collision::ErrorCode;
    let (code, item) = match error {
        ShapeError::NonFinite | ShapeError::Orientation(crate::OrientationError::NonFinite) => (ErrorCode::NonFinite, None),
        ShapeError::InvalidHull => (ErrorCode::InvalidHull, None),
        ShapeError::Orientation(_) => (ErrorCode::Unsupported, None),
        ShapeError::MissingAuthoredProperties => (ErrorCode::InvalidReference, None),
        ShapeError::MissingAuthoredConvex { convex } => (ErrorCode::InvalidReference, Some(convex)),
        _ => (ErrorCode::InvalidSnapshot, None),
    };
    playsrc_collision::Error { code, item, range: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_casts_keep_physical_solid_flags_and_masks_do_not_mutate_the_model() {
        let model = ShapeCastModel::new(crate::world::tests::tetrahedron(1.0)).unwrap();
        let before = playsrc_collision::PhysicsQuery::storage_bytes(&model);
        let hull = Hull { mins: [0.0; 3], maxs: [0.0; 3] };
        let start = [0.5, 0.5, -0.5];
        let end = [32.0, 0.5, -0.5];
        let inside = model.trace(start, end, hull, [0.0; 3], [0.0; 3], 1).unwrap();
        assert_eq!(inside.fraction, 0.0);
        assert!(inside.start_solid && inside.all_solid);
        assert_eq!(inside.start, start);
        assert_eq!(inside.end, start);
        assert_eq!(inside.contents, 1);
        let excluded = model.trace(start, end, hull, [0.0; 3], [0.0; 3], 0).unwrap();
        assert_eq!(excluded.fraction, 1.0);
        assert!(!excluded.start_solid && !excluded.all_solid);
        assert_eq!(excluded.end, end);
        assert_eq!(excluded.convex, None);
        let reverse = model.trace(end, start, hull, [0.0; 3], [0.0; 3], 1).unwrap();
        assert!(reverse.fraction > 0.0 && reverse.fraction < 1.0);
        assert!(reverse.normal[0] > 0.0 && reverse.normal[2] < 0.0);
        assert_eq!(playsrc_collision::PhysicsQuery::storage_bytes(&model), before);
        assert_eq!(model.trace(start, end, Hull { mins: [1.0; 3], maxs: [0.0; 3] }, [0.0; 3], [0.0; 3], 1), Err(ShapeError::InvalidHull));
        assert_eq!(model.trace(start, end, hull, [0.0; 3], [f32::NAN; 3], 1), Err(ShapeError::NonFinite));
    }
}
