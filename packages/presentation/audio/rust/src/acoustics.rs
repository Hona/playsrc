//! Incremental room observation and automatic-preset node lifetime.
//! Trace ownership remains with Collision. No render visibility or PVS result
//! can substitute for an acoustic ray.
use crate::room::Room;

pub type Position = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    WorldSolid,
    WorldAndStaticProps,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub start: Position,
    pub end: Position,
    pub hit: bool,
    pub sky: bool,
    pub reflectivity: Option<f32>,
}
impl Hit {
    fn solid(self) -> bool {
        self.hit && !self.sky
    }
    fn sky(self) -> bool {
        self.hit && self.sky || !self.hit && self.end[2] - self.start[2] > 2400.0
    }
}

pub trait Geometry {
    /// WorldSolid: CONTENTS_SOLID and world only. WorldAndStaticProps:
    /// SOLID|MOVEABLE|WINDOW, excluding client entities but including collidable
    /// static props. Return actual start/end positions, including startsolid.
    fn trace(&mut self, start: Position, end: Position, kind: TraceKind) -> Hit;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomChange {
    pub node: usize,
    /// Some means this cyclic node has just been constructed/replaced. None
    /// selects an existing node without rebuilding its processor parameters.
    pub created: Option<Room>,
}

#[derive(Clone, Debug)]
struct Scan {
    origin: Position,
    eyes: Position,
    distances: [i32; 20],
    reflectivity: [f32; 20],
    sky: [bool; 20],
    next: usize,
    low_ceiling: i32,
    ready: Option<Room>,
}
impl Default for Scan {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            eyes: [0.0; 3],
            distances: [0; 20],
            reflectivity: [0.0; 20],
            sky: [false; 20],
            next: 0,
            low_ceiling: 0,
            ready: None,
        }
    }
}

const DIRECTIONS: [Position; 20] = [
    [4800.0, 0.0, 0.0],
    [-4800.0, 0.0, 0.0],
    [0.0, 4800.0, 0.0],
    [0.0, -4800.0, 0.0],
    [-2496.0, 2304.0, 0.0],
    [2496.0, -2304.0, 0.0],
    [2496.0, 2304.0, 0.0],
    [-2496.0, -2304.0, 0.0],
    [2496.0, 2304.0, 1248.0],
    [2496.0, 2304.0, 2496.0],
    [1248.0, 1152.0, 2496.0],
    [-2496.0, -2304.0, 1248.0],
    [-2496.0, -2304.0, 2496.0],
    [-1248.0, -1152.0, 2496.0],
    [-2496.0, 0.0, 2496.0],
    [0.0, 1152.0, 2496.0],
    [0.0, -2496.0, 2496.0],
    [1248.0, 0.0, 2496.0],
    [0.0, 0.0, 4800.0],
    [0.0, 0.0, -4800.0],
];

fn add(a: Position, b: Position) -> Position {
    std::array::from_fn(|axis| a[axis] + b[axis])
}
fn distance(a: Position, b: Position) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl Scan {
    fn step(&mut self, eyes: Position, geometry: &mut impl Geometry) {
        if self.ready.is_some() {
            return;
        }
        if self.next == 0 {
            self.eyes = eyes;
            self.origin = eyes;
            self.low_ceiling = 0;
            let down = geometry.trace(
                eyes,
                add(eyes, DIRECTIONS[19]),
                TraceKind::WorldAndStaticProps,
            );
            if down.hit && (down.end[2] - down.start[2]).abs() > 72.0 {
                return;
            }
            let up = geometry.trace(
                eyes,
                add(eyes, DIRECTIONS[18]),
                TraceKind::WorldAndStaticProps,
            );
            if down.solid() {
                let height = (up.end[2] - up.start[2]).abs() as i32
                    + (down.end[2] - down.start[2]).abs() as i32;
                let offset = if height > 108 && height <= 128 {
                    if height > 112 { 113 } else { height - 1 }
                } else if height > 128 {
                    129
                } else {
                    self.low_ceiling = height;
                    height - 1
                };
                self.origin[2] = down.end[2].min(down.start[2]) + offset as f32;
            }
        }
        let index = self.next;
        let start = if index >= 8 { self.eyes } else { self.origin };
        let end = add(start, DIRECTIONS[index]);
        let mut hit = geometry.trace(start, end, TraceKind::WorldSolid);
        if index < 8 && !hit.solid() {
            hit = geometry.trace(start, end, TraceKind::WorldAndStaticProps);
        }
        self.sky[index] = hit.sky();
        let mut length = if self.sky[index] && index < 8 {
            1.0
        } else {
            4800.0
        };
        let mut reflectivity = 0.0;
        if hit.solid() {
            reflectivity = hit.reflectivity.unwrap_or(0.5);
            length = distance(hit.start, hit.end);
            if (8..19).contains(&index) {
                let ceiling = if hit.end[2] >= hit.start[2] {
                    hit.end
                } else {
                    hit.start
                };
                let floor = geometry.trace(
                    ceiling,
                    add(ceiling, DIRECTIONS[19]),
                    TraceKind::WorldAndStaticProps,
                );
                if floor.hit {
                    length = distance(floor.start, floor.end);
                }
            }
        }
        self.distances[index] = length as i32;
        self.reflectivity[index] = reflectivity.clamp(0.0, 1.0);
        self.next += 1;
        if self.next == 20 {
            self.next = 0;
            self.ready = self.finish();
        }
    }

    fn finish(&self) -> Option<Room> {
        if self.sky[8..18].iter().any(|value| *value) && !self.sky[18] {
            return None;
        }
        let spans: [i32; 4] =
            std::array::from_fn(|index| self.distances[index * 2] + self.distances[index * 2 + 1]);
        let area = [
            spans[0].wrapping_mul(spans[1]).max(1) as f32,
            spans[2].wrapping_mul(spans[3]).max(1) as f32,
        ];
        let difference = 1.0_f64 - f64::from(area[0].min(area[1]) / area[0].max(area[1]));
        let pair = if difference as f32 > 0.25 {
            if area[0] > area[1] { 0 } else { 2 }
        } else {
            let mut index = 0;
            for candidate in 1..4 {
                if spans[candidate] > spans[index] {
                    index = candidate;
                }
            }
            if index > 1 { 2 } else { 0 }
        };
        let mut surfaces = [0.0; 6];
        surfaces[..4].copy_from_slice(&self.reflectivity[pair * 2..pair * 2 + 4]);
        for &reflectivity in &self.reflectivity[8..19] {
            surfaces[4] = reflectivity;
            if reflectivity == 0.0 {
                break;
            }
        }
        surfaces[5] = self.reflectivity[19];
        let reflectivity = (surfaces.into_iter().sum::<f32>() as f64 / 6.0) as f32;
        Some(Room {
            outside: self.sky[8..19].iter().any(|value| *value),
            width: spans[pair].min(spans[pair + 1]),
            length: spans[pair].max(spans[pair + 1]),
            height: self.distances[8..19].iter().copied().max().unwrap_or(0),
            // Source disables object-volume diffusion enumeration.
            diffusion: 0.0,
            reflectivity,
            surfaces,
        })
    }
}

#[derive(Clone, Debug)]
struct Node {
    room: Room,
    origin: Position,
    visible: bool,
}

#[derive(Clone, Debug)]
pub struct Detector {
    scan: Scan,
    nodes: [Option<Node>; 40],
    store_next: usize,
    check_next: usize,
    checked: usize,
    last_node: Option<usize>,
    selected: Option<usize>,
    last_change: f64,
    enabled: bool,
}
impl Default for Detector {
    fn default() -> Self {
        Self {
            scan: Scan::default(),
            nodes: std::array::from_fn(|_| None),
            store_next: 0,
            check_next: 0,
            checked: 0,
            last_node: None,
            selected: None,
            last_change: 0.0,
            enabled: false,
        }
    }
}

impl Detector {
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(
        &mut self,
        enabled: bool,
        host_time: f64,
        eyes: Position,
        geometry: &mut impl Geometry,
    ) -> Option<RoomChange> {
        if !enabled {
            self.enabled = false;
            return None;
        }
        if !self.enabled {
            self.scan = Scan::default();
            self.last_change = 0.0;
            self.enabled = true;
        }
        if (host_time - self.last_change).abs() < 0.25 {
            return None;
        }
        for _ in 0..3 {
            self.scan.step(eyes, geometry);
        }
        let room = self.scan.ready?;
        let checks = if self.checked >= 40 { 0 } else { 40 };
        for _ in 0..checks {
            self.checked += 1;
            let index = self.check_next;
            self.check_next = (index + 1) % 40;
            let Some(node) = &mut self.nodes[index] else {
                continue;
            };
            let range = if node.room.outside {
                1200
            } else {
                480.min((node.room.width * 5).min(node.room.length))
            };
            let distance = distance(self.scan.origin, node.origin);
            if distance > range as f32 {
                continue;
            }
            let hit = geometry.trace(
                self.scan.origin,
                node.origin,
                TraceKind::WorldAndStaticProps,
            );
            node.visible = !hit.hit && distance <= 480.0;
            if node.visible {
                break;
            }
        }
        if self.checked < 40 {
            return None;
        }
        let mut selected = None;
        let mut area = 0;
        for (index, node) in self.nodes.iter().enumerate() {
            let Some(node) = node else {
                continue;
            };
            if node.visible && node.room.outside == room.outside {
                let candidate_area = node.room.width.wrapping_mul(node.room.length);
                if candidate_area > area {
                    selected = Some(index);
                    area = candidate_area;
                }
            }
        }
        let change = if let Some(index) = selected {
            if self.selected != Some(index) {
                self.last_change = host_time;
            }
            self.last_node = Some(index);
            self.selected = Some(index);
            Some(RoomChange {
                node: index,
                created: None,
            })
        } else if self.new_room(room) {
            let index = self.store_next;
            self.store_next = (index + 1) % 40;
            self.nodes[index] = Some(Node {
                room,
                origin: self.scan.origin,
                visible: false,
            });
            self.last_node = Some(index);
            self.selected = Some(index);
            self.last_change = host_time;
            Some(RoomChange {
                node: index,
                created: Some(room),
            })
        } else {
            None
        };
        self.checked = 0;
        self.scan.ready = None;
        for node in self.nodes.iter_mut().flatten() {
            node.visible = false;
        }
        change
    }

    fn new_room(&self, room: Room) -> bool {
        if let Some(old) = self.last_node.and_then(|index| self.nodes[index].as_ref()) {
            let horizontal = distance(
                [self.scan.origin[0], self.scan.origin[1], 0.0],
                [old.origin[0], old.origin[1], 0.0],
            );
            if horizontal <= 48.0 {
                return false;
            }
            let changed = [(room.width, old.room.width), (room.length, old.room.length)]
                .into_iter()
                .any(|(new, previous)| {
                    let ratio = if previous == 0 {
                        0.0
                    } else {
                        new as f32 / previous as f32
                    };
                    let ratio = if ratio > 1.0 {
                        (1.0 / f64::from(ratio)) as f32
                    } else {
                        ratio
                    };
                    1.0 - f64::from(ratio) >= 0.4
                });
            if !changed && old.room.outside == room.outside {
                return false;
            }
        }
        !(self.scan.low_ceiling != 0
            && self.scan.low_ceiling < room.height
            && f64::from(self.scan.low_ceiling as f32 / room.height as f32) < 0.8)
    }
}
