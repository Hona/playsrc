use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    fmt,
};

pub const MAGIC: u32 = 0xfeed_face;
pub const CURRENT_VERSION: u32 = 16;
pub const TF2_SUBVERSION: u32 = 2;
pub const NAV_MESH_CROUCH: u32 = 0x0000_0001;
pub const NAV_MESH_JUMP: u32 = 0x0000_0002;
pub const NAV_MESH_NO_JUMP: u32 = 0x0000_0008;
const GRID_CELL_SIZE: f32 = 300.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_areas: usize,
    pub max_connections: usize,
    pub max_hiding_spots: usize,
    pub max_encounters: usize,
    pub max_visible_areas: usize,
    pub max_ladders: usize,
    pub max_places: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_areas: 262_144,
            max_connections: 4_194_304,
            max_hiding_spots: 1_048_576,
            max_encounters: 4_194_304,
            max_visible_areas: 16_777_216,
            max_ladders: 65_536,
            max_places: 65_535,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Source,
    TeamFortress2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    Truncated,
    InvalidMagic,
    UnsupportedVersion,
    UnsupportedSubversion,
    BspSizeMismatch,
    InvalidPlace,
    InvalidArea,
    InvalidScalar,
    DuplicateArea,
    DuplicateHidingSpot,
    DuplicateLadder,
    MissingArea,
    MissingHidingSpot,
    MissingLadder,
    AreaLimit,
    ConnectionLimit,
    HidingSpotLimit,
    EncounterLimit,
    VisibilityLimit,
    LadderLimit,
    TrailingBytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub offset: usize,
    pub identity: Option<u32>,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?} at NAV byte {}", self.code, self.offset)?;
        if let Some(identity) = self.identity {
            write!(output, " (identity {identity})")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Direction {
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub const fn opposite(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HidingSpot {
    pub identity: u32,
    pub position: [f32; 3],
    pub flags: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncounterSpot {
    pub identity: u32,
    pub distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Encounter {
    pub from: u32,
    pub from_direction: u8,
    pub to: u32,
    pub to_direction: u8,
    pub spots: Vec<EncounterSpot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleArea {
    pub identity: u32,
    pub attributes: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub identity: u32,
    pub attributes: u32,
    pub northwest: [f32; 3],
    pub southeast: [f32; 3],
    pub northeast_z: f32,
    pub southwest_z: f32,
    pub connections: [Vec<u32>; 4],
    pub hiding_spots: Vec<HidingSpot>,
    pub encounters: Vec<Encounter>,
    pub place: Option<u16>,
    pub ladder_up: Vec<u32>,
    pub ladder_down: Vec<u32>,
    pub earliest_occupy: [f32; 2],
    pub light_intensity: [f32; 4],
    pub visible_areas: Vec<VisibleArea>,
    pub inherited_visibility: Option<u32>,
    pub game_attributes: u32,
}

impl Area {
    pub fn center(&self) -> [f32; 3] {
        [
            (self.northwest[0] + self.southeast[0]) / 2.0,
            (self.northwest[1] + self.southeast[1]) / 2.0,
            (self.northwest[2] + self.southeast[2]) / 2.0,
        ]
    }

    pub fn height(&self, x: f32, y: f32) -> f32 {
        let dx = self.southeast[0] - self.northwest[0];
        let dy = self.southeast[1] - self.northwest[1];
        if dx <= 0.0 || dy <= 0.0 {
            return self.northeast_z;
        }
        let u = ((x - self.northwest[0]) / dx).clamp(0.0, 1.0);
        let v = ((y - self.northwest[1]) / dy).clamp(0.0, 1.0);
        let north = self.northwest[2] + u * (self.northeast_z - self.northwest[2]);
        let south = self.southwest_z + u * (self.southeast[2] - self.southwest_z);
        north + v * (south - north)
    }

    pub fn closest_point(&self, position: [f32; 3]) -> [f32; 3] {
        let x = position[0].clamp(self.northwest[0], self.southeast[0]);
        let y = position[1].clamp(self.northwest[1], self.southeast[1]);
        [x, y, self.height(x, y)]
    }

    pub fn contains_xy(&self, position: [f32; 3]) -> bool {
        position[0] >= self.northwest[0]
            && position[0] <= self.southeast[0]
            && position[1] >= self.northwest[1]
            && position[1] <= self.southeast[1]
    }

    pub fn portal(&self, destination: &Self, direction: Direction) -> [f32; 3] {
        let (x, y) = match direction {
            Direction::North | Direction::South => {
                let left = self.northwest[0]
                    .max(destination.northwest[0])
                    .clamp(self.northwest[0], self.southeast[0]);
                let right = self.southeast[0]
                    .min(destination.southeast[0])
                    .clamp(self.northwest[0], self.southeast[0]);
                (
                    (left + right) / 2.0,
                    if direction == Direction::North {
                        self.northwest[1]
                    } else {
                        self.southeast[1]
                    },
                )
            }
            Direction::East | Direction::West => {
                let top = self.northwest[1]
                    .max(destination.northwest[1])
                    .clamp(self.northwest[1], self.southeast[1]);
                let bottom = self.southeast[1]
                    .min(destination.southeast[1])
                    .clamp(self.northwest[1], self.southeast[1]);
                (
                    if direction == Direction::West {
                        self.northwest[0]
                    } else {
                        self.southeast[0]
                    },
                    (top + bottom) / 2.0,
                )
            }
        };
        [x, y, self.height(x, y)]
    }

    pub fn connection_height_change(&self, destination: &Self, direction: Direction) -> f32 {
        let from = self.portal(destination, direction);
        let to = destination.portal(self, direction.opposite());
        to[2] - from[2]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ladder {
    pub identity: u32,
    pub width: f32,
    pub top: [f32; 3],
    pub bottom: [f32; 3],
    pub length: f32,
    pub direction: Direction,
    pub top_forward: Option<u32>,
    pub top_left: Option<u32>,
    pub top_right: Option<u32>,
    pub top_behind: Option<u32>,
    pub bottom_area: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub version: u32,
    pub subversion: u32,
    pub bsp_size: Option<u32>,
    pub analyzed: bool,
    pub places: Vec<String>,
    pub unnamed_areas: bool,
    pub areas: Vec<Area>,
    pub ladders: Vec<Ladder>,
    index: HashMap<u32, usize>,
    grid: Grid,
}

#[derive(Clone, Debug, PartialEq)]
struct Grid {
    minimum: [f32; 2],
    width: usize,
    height: usize,
    cells: Vec<Vec<usize>>,
}

impl Mesh {
    pub fn area(&self, identity: u32) -> Option<&Area> {
        self.index.get(&identity).map(|index| &self.areas[*index])
    }

    pub fn nearest_area(&self, position: [f32; 3]) -> Option<&Area> {
        let origin_x = self.grid.coordinate(position[0], 0, self.grid.width);
        let origin_y = self.grid.coordinate(position[1], 1, self.grid.height);
        let mut best: Option<(&Area, f32)> = None;
        let mut shift = 0;
        let mut limit = self.grid.width.max(self.grid.height);
        while shift <= limit {
            let min_x = origin_x.saturating_sub(shift);
            let max_x = origin_x.saturating_add(shift).min(self.grid.width - 1);
            let min_y = origin_y.saturating_sub(shift);
            let max_y = origin_y.saturating_add(shift).min(self.grid.height - 1);
            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    if x > min_x && x < max_x && y > min_y && y < max_y {
                        continue;
                    }
                    for &index in &self.grid.cells[x + y * self.grid.width] {
                        let area = &self.areas[index];
                        let distance = distance_squared(area.closest_point(position), position);
                        if best.is_none_or(|(prior, prior_distance)| {
                            distance
                                .total_cmp(&prior_distance)
                                .then_with(|| area.identity.cmp(&prior.identity))
                                .is_lt()
                        }) {
                            best = Some((area, distance));
                            limit = shift.saturating_add(1);
                        }
                    }
                }
            }
            shift += 1;
        }
        best.map(|(area, _)| area)
    }

    pub fn build_path<F>(&self, start: u32, goal: u32, mut cost: F) -> Option<Vec<u32>>
    where
        F: FnMut(&Area, &Area, Direction, f32) -> Option<f32>,
    {
        let start_index = *self.index.get(&start)?;
        let goal_index = *self.index.get(&goal)?;
        if start_index == goal_index {
            return Some(vec![start]);
        }
        let goal_position = self.areas[goal_index].center();
        let mut costs = vec![f32::INFINITY; self.areas.len()];
        let mut parents: Vec<Option<usize>> = vec![None; self.areas.len()];
        let mut closed = vec![false; self.areas.len()];
        let mut queue = BinaryHeap::new();
        let mut order = 0_u64;
        costs[start_index] = 0.0;
        queue.push(OpenArea {
            index: start_index,
            total: distance(self.areas[start_index].center(), goal_position),
            order,
        });
        while let Some(current) = queue.pop() {
            if closed[current.index] {
                continue;
            }
            if current.index == goal_index {
                let mut path = vec![goal];
                let mut at = current.index;
                while let Some(parent) = parents[at] {
                    path.push(self.areas[parent].identity);
                    at = parent;
                }
                path.reverse();
                return Some(path);
            }
            closed[current.index] = true;
            let from = &self.areas[current.index];
            for direction in Direction::ALL {
                for identity in &from.connections[direction as usize] {
                    let Some(&next_index) = self.index.get(identity) else {
                        continue;
                    };
                    if parents[current.index] == Some(next_index) || next_index == current.index {
                        continue;
                    }
                    let destination = &self.areas[next_index];
                    let length = distance(from.center(), destination.center());
                    let Some(edge) = cost(from, destination, direction, length) else {
                        continue;
                    };
                    if !edge.is_finite() || edge < 0.0 {
                        continue;
                    }
                    let next_cost =
                        (costs[current.index] + edge).max(costs[current.index] * 1.00001 + 0.00001);
                    if costs[next_index] <= next_cost {
                        continue;
                    }
                    costs[next_index] = next_cost;
                    parents[next_index] = Some(current.index);
                    closed[next_index] = false;
                    order += 1;
                    queue.push(OpenArea {
                        index: next_index,
                        total: next_cost + distance(destination.center(), goal_position),
                        order,
                    });
                }
            }
        }
        None
    }
}

impl Grid {
    fn new(areas: &[Area], maximum_areas: usize, reader: &Reader<'_>) -> Result<Self, Error> {
        let minimum = [
            areas
                .iter()
                .map(|area| area.northwest[0])
                .fold(f32::INFINITY, f32::min),
            areas
                .iter()
                .map(|area| area.northwest[1])
                .fold(f32::INFINITY, f32::min),
        ];
        let maximum = [
            areas
                .iter()
                .map(|area| area.southeast[0])
                .fold(f32::NEG_INFINITY, f32::max),
            areas
                .iter()
                .map(|area| area.southeast[1])
                .fold(f32::NEG_INFINITY, f32::max),
        ];
        let width = (((maximum[0] - minimum[0]) / GRID_CELL_SIZE) as usize)
            .checked_add(1)
            .ok_or_else(|| reader.error(ErrorCode::InvalidArea))?;
        let height = (((maximum[1] - minimum[1]) / GRID_CELL_SIZE) as usize)
            .checked_add(1)
            .ok_or_else(|| reader.error(ErrorCode::InvalidArea))?;
        let count = width
            .checked_mul(height)
            .filter(|count| *count <= maximum_areas.saturating_mul(16))
            .ok_or_else(|| reader.error(ErrorCode::InvalidArea))?;
        let mut grid = Self {
            minimum,
            width,
            height,
            cells: vec![Vec::new(); count],
        };
        for (index, area) in areas.iter().enumerate() {
            let min_x = grid.coordinate(area.northwest[0], 0, width);
            let max_x = grid.coordinate(area.southeast[0], 0, width);
            let min_y = grid.coordinate(area.northwest[1], 1, height);
            let max_y = grid.coordinate(area.southeast[1], 1, height);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    grid.cells[x + y * width].push(index);
                }
            }
        }
        Ok(grid)
    }

    fn coordinate(&self, value: f32, axis: usize, size: usize) -> usize {
        (((value - self.minimum[axis]) / GRID_CELL_SIZE) as usize).min(size - 1)
    }
}

#[derive(Clone, Copy, Debug)]
struct OpenArea {
    index: usize,
    total: f32,
    order: u64,
}

impl PartialEq for OpenArea {
    fn eq(&self, other: &Self) -> bool {
        self.total.to_bits() == other.total.to_bits() && self.order == other.order
    }
}
impl Eq for OpenArea {}
impl PartialOrd for OpenArea {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OpenArea {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .total
            .total_cmp(&self.total)
            .then_with(|| other.order.cmp(&self.order))
    }
}

fn distance_squared(left: [f32; 3], right: [f32; 3]) -> f32 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| (left - right) * (left - right))
        .sum()
}

fn distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    distance_squared(left, right).sqrt()
}

pub fn parse(
    bytes: &[u8],
    profile: Profile,
    expected_bsp_size: Option<u32>,
    limits: Limits,
) -> Result<Mesh, Error> {
    if bytes.len() > limits.max_bytes {
        return Err(Error {
            code: ErrorCode::InputLimit,
            offset: bytes.len(),
            identity: None,
        });
    }
    let mut reader = Reader { bytes, offset: 0 };
    if reader.u32()? != MAGIC {
        return Err(reader.error(ErrorCode::InvalidMagic));
    }
    let version = reader.u32()?;
    if !(4..=CURRENT_VERSION).contains(&version) {
        return Err(reader.error(ErrorCode::UnsupportedVersion));
    }
    let subversion = if version >= 10 { reader.u32()? } else { 0 };
    if profile == Profile::TeamFortress2 && subversion > TF2_SUBVERSION {
        return Err(reader.error(ErrorCode::UnsupportedSubversion));
    }
    let bsp_size = Some(reader.u32()?);
    if expected_bsp_size.is_some() && bsp_size != expected_bsp_size {
        return Err(reader.error(ErrorCode::BspSizeMismatch));
    }
    let analyzed = version >= 14 && reader.u8()? != 0;
    let mut places = Vec::new();
    let mut unnamed_areas = false;
    if version >= 5 {
        let count = usize::from(reader.u16()?);
        if count > limits.max_places {
            return Err(reader.error(ErrorCode::InvalidPlace));
        }
        for _ in 0..count {
            let length = usize::from(reader.u16()?);
            if length == 0 || length > 256 {
                return Err(reader.error(ErrorCode::InvalidPlace));
            }
            let value = reader.bytes(length)?;
            if value[length - 1] != 0 || value[..length - 1].contains(&0) {
                return Err(reader.error(ErrorCode::InvalidPlace));
            }
            let value = std::str::from_utf8(&value[..length - 1])
                .map_err(|_| reader.error(ErrorCode::InvalidPlace))?;
            places.push(value.to_owned());
        }
        if version > 11 {
            unnamed_areas = reader.u8()? != 0;
        }
    }
    let count = reader.u32()? as usize;
    if count == 0 {
        return Err(reader.error(ErrorCode::InvalidArea));
    }
    if count > limits.max_areas {
        return Err(reader.error(ErrorCode::AreaLimit));
    }
    let mut areas = Vec::with_capacity(count);
    let mut index = HashMap::with_capacity(count);
    let mut spots = HashMap::new();
    let mut total_connections = 0_usize;
    let mut total_hiding = 0_usize;
    let mut total_encounters = 0_usize;
    let mut total_visibility = 0_usize;
    for _ in 0..count {
        let identity = reader.u32()?;
        if identity == 0 {
            return Err(reader.error(ErrorCode::InvalidArea));
        }
        if index.insert(identity, areas.len()).is_some() {
            return Err(reader.identity_error(ErrorCode::DuplicateArea, identity));
        }
        let attributes = if version <= 8 {
            u32::from(reader.u8()?)
        } else if version < 13 {
            u32::from(reader.u16()?)
        } else {
            reader.u32()?
        };
        let northwest = reader.vector()?;
        let southeast = reader.vector()?;
        if northwest[0] > southeast[0] || northwest[1] > southeast[1] {
            return Err(reader.identity_error(ErrorCode::InvalidArea, identity));
        }
        let northeast_z = reader.f32()?;
        let southwest_z = reader.f32()?;
        let mut connections = std::array::from_fn(|_| Vec::new());
        for direction in Direction::ALL {
            let count = reader.u32()? as usize;
            total_connections = checked_count(
                total_connections,
                count,
                limits.max_connections,
                &reader,
                ErrorCode::ConnectionLimit,
            )?;
            let list = &mut connections[direction as usize];
            list.reserve(count);
            for _ in 0..count {
                let connection = reader.u32()?;
                if connection != identity {
                    list.push(connection);
                }
            }
        }
        let hiding_count = usize::from(reader.u8()?);
        total_hiding = checked_count(
            total_hiding,
            hiding_count,
            limits.max_hiding_spots,
            &reader,
            ErrorCode::HidingSpotLimit,
        )?;
        let mut hiding_spots = Vec::with_capacity(hiding_count);
        for _ in 0..hiding_count {
            let spot_identity = reader.u32()?;
            if spots.insert(spot_identity, identity).is_some() {
                return Err(reader.identity_error(ErrorCode::DuplicateHidingSpot, spot_identity));
            }
            hiding_spots.push(HidingSpot {
                identity: spot_identity,
                position: reader.vector()?,
                flags: reader.u8()?,
            });
        }
        if version < 15 {
            let approaches = usize::from(reader.u8()?);
            reader.bytes(
                approaches
                    .checked_mul(14)
                    .ok_or_else(|| reader.error(ErrorCode::Truncated))?,
            )?;
        }
        let encounter_count = reader.u32()? as usize;
        total_encounters = checked_count(
            total_encounters,
            encounter_count,
            limits.max_encounters,
            &reader,
            ErrorCode::EncounterLimit,
        )?;
        let mut encounters = Vec::with_capacity(encounter_count);
        for _ in 0..encounter_count {
            let from = reader.u32()?;
            let from_direction = reader.u8()?;
            let to = reader.u32()?;
            let to_direction = reader.u8()?;
            let spot_count = usize::from(reader.u8()?);
            let mut encounter_spots = Vec::with_capacity(spot_count);
            for _ in 0..spot_count {
                encounter_spots.push(EncounterSpot {
                    identity: reader.u32()?,
                    distance: f32::from(reader.u8()?) / 255.0,
                });
            }
            encounters.push(Encounter {
                from,
                from_direction,
                to,
                to_direction,
                spots: encounter_spots,
            });
        }
        let place = if version >= 5 {
            let place = reader.u16()?;
            if usize::from(place) > places.len() {
                return Err(reader.error(ErrorCode::InvalidPlace));
            }
            (place != 0).then(|| place - 1)
        } else {
            None
        };
        let mut ladder_up = Vec::new();
        let mut ladder_down = Vec::new();
        if version >= 7 {
            for list in [&mut ladder_up, &mut ladder_down] {
                let count = reader.u32()? as usize;
                total_connections = checked_count(
                    total_connections,
                    count,
                    limits.max_connections,
                    &reader,
                    ErrorCode::ConnectionLimit,
                )?;
                for _ in 0..count {
                    let ladder = reader.u32()?;
                    if !list.contains(&ladder) {
                        list.push(ladder);
                    }
                }
            }
        }
        let earliest_occupy = if version >= 8 {
            [reader.f32()?, reader.f32()?]
        } else {
            [0.0; 2]
        };
        let light_intensity = if version >= 11 {
            [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?]
        } else {
            [0.0; 4]
        };
        let mut visible_areas = Vec::new();
        let mut inherited_visibility = None;
        if version >= 16 {
            let count = reader.u32()? as usize;
            total_visibility = checked_count(
                total_visibility,
                count,
                limits.max_visible_areas,
                &reader,
                ErrorCode::VisibilityLimit,
            )?;
            visible_areas.reserve(count);
            for _ in 0..count {
                visible_areas.push(VisibleArea {
                    identity: reader.u32()?,
                    attributes: reader.u8()?,
                });
            }
            let inherited = reader.u32()?;
            inherited_visibility = (inherited != 0).then_some(inherited);
        }
        let game_attributes = if profile == Profile::TeamFortress2 && subversion > 1 {
            reader.u32()?
        } else {
            0
        };
        areas.push(Area {
            identity,
            attributes,
            northwest,
            southeast,
            northeast_z,
            southwest_z,
            connections,
            hiding_spots,
            encounters,
            place,
            ladder_up,
            ladder_down,
            earliest_occupy,
            light_intensity,
            visible_areas,
            inherited_visibility,
            game_attributes,
        });
    }
    let ladder_count = if version >= 6 {
        reader.u32()? as usize
    } else {
        0
    };
    if ladder_count > limits.max_ladders {
        return Err(reader.error(ErrorCode::LadderLimit));
    }
    let mut ladder_index = HashMap::with_capacity(ladder_count);
    let mut ladders = Vec::with_capacity(ladder_count);
    for _ in 0..ladder_count {
        let identity = reader.u32()?;
        if ladder_index.insert(identity, ladders.len()).is_some() {
            return Err(reader.identity_error(ErrorCode::DuplicateLadder, identity));
        }
        let width = reader.f32()?;
        let top = reader.vector()?;
        let bottom = reader.vector()?;
        let length = reader.f32()?;
        let direction = match reader.u32()? {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            3 => Direction::West,
            _ => return Err(reader.identity_error(ErrorCode::InvalidArea, identity)),
        };
        if version == 6 {
            reader.u8()?;
        }
        let mut area = || -> Result<Option<u32>, Error> {
            let value = reader.u32()?;
            Ok((value != 0).then_some(value))
        };
        ladders.push(Ladder {
            identity,
            width,
            top,
            bottom,
            length,
            direction,
            top_forward: area()?,
            top_left: area()?,
            top_right: area()?,
            top_behind: area()?,
            bottom_area: area()?,
        });
    }
    if reader.offset != bytes.len() {
        return Err(reader.error(ErrorCode::TrailingBytes));
    }
    for area in &mut areas {
        for identity in area.connections.iter().flatten() {
            if !index.contains_key(identity) {
                return Err(reader.identity_error(ErrorCode::MissingArea, *identity));
            }
        }
        for identity in area.ladder_up.iter().chain(&area.ladder_down) {
            if !ladder_index.contains_key(identity) {
                return Err(reader.identity_error(ErrorCode::MissingLadder, *identity));
            }
        }
        for encounter in &area.encounters {
            for identity in [encounter.from, encounter.to] {
                if !index.contains_key(&identity) {
                    return Err(reader.identity_error(ErrorCode::MissingArea, identity));
                }
            }
            for spot in &encounter.spots {
                if !spots.contains_key(&spot.identity) {
                    return Err(reader.identity_error(ErrorCode::MissingHidingSpot, spot.identity));
                }
            }
        }
        area.visible_areas
            .retain(|visible| index.contains_key(&visible.identity));
        if area.inherited_visibility == Some(area.identity) {
            return Err(reader.identity_error(ErrorCode::InvalidArea, area.identity));
        }
        if area
            .inherited_visibility
            .is_some_and(|identity| !index.contains_key(&identity))
        {
            area.inherited_visibility = None;
        }
    }
    for ladder in &ladders {
        for identity in [
            ladder.top_forward,
            ladder.top_left,
            ladder.top_right,
            ladder.top_behind,
            ladder.bottom_area,
        ]
        .into_iter()
        .flatten()
        {
            if !index.contains_key(&identity) {
                return Err(reader.identity_error(ErrorCode::MissingArea, identity));
            }
        }
    }
    let grid = Grid::new(&areas, limits.max_areas, &reader)?;
    Ok(Mesh {
        version,
        subversion,
        bsp_size,
        analyzed,
        places,
        unnamed_areas,
        areas,
        ladders,
        index,
        grid,
    })
}

fn checked_count(
    prior: usize,
    next: usize,
    limit: usize,
    reader: &Reader<'_>,
    code: ErrorCode,
) -> Result<usize, Error> {
    prior
        .checked_add(next)
        .filter(|value| *value <= limit)
        .ok_or_else(|| reader.error(code))
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn error(&self, code: ErrorCode) -> Error {
        Error {
            code,
            offset: self.offset,
            identity: None,
        }
    }

    fn identity_error(&self, code: ErrorCode, identity: u32) -> Error {
        Error {
            code,
            offset: self.offset,
            identity: Some(identity),
        }
    }

    fn bytes(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| self.error(ErrorCode::Truncated))?;
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, Error> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err(self.error(ErrorCode::InvalidScalar));
        }
        Ok(value)
    }

    fn vector(&mut self) -> Result<[f32; 3], Error> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type FixtureArea = ([f32; 3], [f32; 3], [Vec<u32>; 4], u32);

    fn fixture(areas: &[FixtureArea]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut u32 = |value: u32| bytes.extend(value.to_le_bytes());
        u32(MAGIC);
        u32(CURRENT_VERSION);
        u32(TF2_SUBVERSION);
        u32(128);
        bytes.push(1);
        bytes.extend(0_u16.to_le_bytes());
        bytes.push(1);
        bytes.extend((areas.len() as u32).to_le_bytes());
        for (index, (northwest, southeast, connections, tf)) in areas.iter().enumerate() {
            bytes.extend(((index + 1) as u32).to_le_bytes());
            bytes.extend(0_u32.to_le_bytes());
            for value in northwest.iter().chain(southeast) {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(northwest[2].to_le_bytes());
            bytes.extend(southeast[2].to_le_bytes());
            for list in connections {
                bytes.extend((list.len() as u32).to_le_bytes());
                for value in list {
                    bytes.extend(value.to_le_bytes());
                }
            }
            bytes.push(0);
            bytes.extend(0_u32.to_le_bytes());
            bytes.extend(0_u16.to_le_bytes());
            bytes.extend([0; 8]);
            bytes.extend([0; 8]);
            bytes.extend([0; 16]);
            bytes.extend([0; 8]);
            bytes.extend(tf.to_le_bytes());
        }
        bytes.extend(0_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn exact_tf_mesh_and_directional_path() {
        let bytes = fixture(&[
            (
                [0.0, 0.0, 0.0],
                [50.0, 50.0, 0.0],
                [vec![], vec![2], vec![3], vec![]],
                4,
            ),
            (
                [50.0, 0.0, 0.0],
                [100.0, 50.0, 0.0],
                [vec![], vec![], vec![4], vec![1]],
                0,
            ),
            (
                [0.0, 50.0, 0.0],
                [50.0, 100.0, 0.0],
                [vec![1], vec![4], vec![], vec![]],
                0,
            ),
            (
                [50.0, 50.0, 0.0],
                [100.0, 100.0, 0.0],
                [vec![2], vec![], vec![], vec![3]],
                2,
            ),
        ]);
        let mesh = parse(&bytes, Profile::TeamFortress2, Some(128), Limits::default()).unwrap();
        assert_eq!(mesh.areas.len(), 4);
        assert_eq!(mesh.area(1).unwrap().game_attributes, 4);
        assert_eq!(mesh.nearest_area([75.0, 80.0, 5.0]).unwrap().identity, 4);
        assert_eq!(
            mesh.build_path(1, 4, |_, _, _, length| Some(length)),
            Some(vec![1, 2, 4])
        );
        assert_eq!(
            mesh.build_path(1, 4, |_, next, _, length| (next.identity != 2)
                .then_some(length)),
            Some(vec![1, 3, 4])
        );
    }

    #[test]
    fn source_bilinear_height_and_portal() {
        let bytes = fixture(&[
            (
                [0.0, 0.0, 0.0],
                [100.0, 100.0, 20.0],
                [vec![], vec![2], vec![], vec![]],
                0,
            ),
            (
                [100.0, 25.0, 30.0],
                [200.0, 75.0, 30.0],
                [vec![], vec![], vec![], vec![1]],
                0,
            ),
        ]);
        let mesh = parse(&bytes, Profile::TeamFortress2, Some(128), Limits::default()).unwrap();
        let first = mesh.area(1).unwrap();
        let second = mesh.area(2).unwrap();
        assert_eq!(first.height(50.0, 50.0), 10.0);
        assert_eq!(first.portal(second, Direction::East), [100.0, 50.0, 10.0]);
        assert_eq!(
            first.connection_height_change(second, Direction::East),
            20.0
        );
    }

    #[test]
    fn source_spatial_grid_checks_the_adjacent_ring_and_breaks_distance_ties_by_identity() {
        let bytes = fixture(&[
            (
                [0.0, 0.0, 0.0],
                [200.0, 200.0, 0.0],
                [vec![], vec![], vec![], vec![]],
                0,
            ),
            (
                [300.0, 0.0, 0.0],
                [500.0, 200.0, 0.0],
                [vec![], vec![], vec![], vec![]],
                0,
            ),
            (
                [900.0, 0.0, 0.0],
                [1000.0, 200.0, 0.0],
                [vec![], vec![], vec![], vec![]],
                0,
            ),
        ]);
        let mesh = parse(&bytes, Profile::TeamFortress2, Some(128), Limits::default()).unwrap();
        assert_eq!(mesh.nearest_area([299.0, 100.0, 0.0]).unwrap().identity, 2);
        assert_eq!(mesh.nearest_area([250.0, 100.0, 0.0]).unwrap().identity, 1);
        assert_eq!(mesh.nearest_area([-500.0, 100.0, 0.0]).unwrap().identity, 1);
        assert_eq!(mesh.nearest_area([1200.0, 100.0, 0.0]).unwrap().identity, 3);
    }

    #[test]
    fn malformed_identity_size_and_references_are_rejected() {
        let bytes = fixture(&[(
            [0.0, 0.0, 0.0],
            [50.0, 50.0, 0.0],
            [vec![], vec![99], vec![], vec![]],
            0,
        )]);
        assert_eq!(
            parse(&bytes, Profile::TeamFortress2, Some(127), Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::BspSizeMismatch
        );
        assert_eq!(
            parse(&bytes, Profile::TeamFortress2, Some(128), Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::MissingArea
        );
        assert_eq!(
            parse(
                &bytes[..bytes.len() - 1],
                Profile::TeamFortress2,
                Some(128),
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::Truncated
        );
    }
}
