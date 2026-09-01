// Portions adapted from Valve's official Source SDK 2013.
// Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
use crate::{Error, ErrorCode, Hull, error};
use std::cmp::Reverse;
use std::collections::BTreeMap;

const SERVER_LISTS: u16 = 0x53;

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    identity: u64,
    bounds: Option<Hull>,
    lists: u16,
    hidden: bool,
    order: u64,
}

/// Persistent server-side entity-query state, including insertion history.
/// Cloning retains the complete continuation independently of other readers.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityQuerySpace {
    entries: Vec<Option<Entry>>,
    available: Vec<u16>,
    order: u64,
    maximum: usize,
    visits: usize,
}

impl EntityQuerySpace {
    pub fn new(maximum_entries: usize, maximum_visits: usize) -> Result<Self, Error> {
        if maximum_entries == 0 || maximum_entries > usize::from(u16::MAX) || maximum_visits == 0 {
            return Err(error(ErrorCode::Limit, None));
        }
        Ok(Self {
            entries: Vec::new(),
            available: Vec::new(),
            order: 0,
            maximum: maximum_entries,
            visits: maximum_visits,
        })
    }

    pub fn create(&mut self, identity: u64, bounds: Hull, lists: u16) -> Result<u16, Error> {
        validate_lists(lists)?;
        let bounds = expanded(bounds)?;
        let order = if lists == 0 {
            self.order
        } else {
            self.order
                .checked_add(1)
                .ok_or_else(|| error(ErrorCode::Limit, None))?
        };
        let handle = self.allocate(identity)?;
        let entry = self.entries[usize::from(handle)]
            .as_mut()
            .expect("allocated entry");
        entry.lists = lists;
        if lists != 0 {
            entry.bounds = Some(clamped(bounds));
            entry.order = order;
        }
        self.order = order;
        Ok(handle)
    }

    /// Allocates an unplaced handle; listing it does not provide geometric bounds.
    pub fn allocate(&mut self, identity: u64) -> Result<u16, Error> {
        if self
            .entries
            .iter()
            .flatten()
            .any(|entry| entry.identity == identity)
        {
            return Err(error(ErrorCode::DuplicateIdentity, None));
        }
        if self.available.is_empty() && self.entries.len() >= self.maximum {
            return Err(error(ErrorCode::Limit, None));
        }
        let entry = Some(Entry {
            identity,
            bounds: None,
            lists: 0,
            hidden: false,
            order: 0,
        });
        let handle = if let Some(handle) = self.available.pop() {
            self.entries[usize::from(handle)] = entry;
            handle
        } else {
            let handle = self.entries.len() as u16;
            self.entries.push(entry);
            handle
        };
        Ok(handle)
    }

    pub fn destroy(&mut self, handle: u16) -> Result<(), Error> {
        self.entry(handle)?;
        self.entries[usize::from(handle)] = None;
        self.available.push(handle);
        Ok(())
    }

    pub fn move_bounds(&mut self, handle: u16, bounds: Hull) -> Result<(), Error> {
        let bounds = expanded(bounds)?;
        let entry = self.entry(handle)?;
        if entry.lists == 0 || entry.bounds == Some(bounds) {
            return Ok(());
        }
        let order = self
            .order
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::Limit, None))?;
        let entry = self.entries[usize::from(handle)]
            .as_mut()
            .expect("validated entry");
        entry.bounds = Some(clamped(bounds));
        entry.order = order;
        self.order = order;
        Ok(())
    }

    pub fn change_lists(&mut self, handle: u16, remove: u16, insert: u16) -> Result<(), Error> {
        validate_lists(remove | insert)?;
        self.entry(handle)?;
        let entry = self.entries[usize::from(handle)]
            .as_mut()
            .expect("validated entry");
        entry.lists = (entry.lists & !remove) | insert;
        Ok(())
    }

    pub fn set_hidden(&mut self, handle: u16, hidden: bool) -> Result<(), Error> {
        self.entry(handle)?;
        self.entries[usize::from(handle)]
            .as_mut()
            .expect("validated entry")
            .hidden = hidden;
        Ok(())
    }

    /// The Source sphere enumeration is a box query, not a nearest-point sphere test.
    /// Game filtering precedes the result limit; distance tests remain caller-owned.
    pub fn sphere(
        &self,
        origin: [f32; 3],
        radius: f32,
        lists: u16,
        maximum_results: usize,
        accepts: impl FnMut(u64) -> bool,
    ) -> Result<Vec<u64>, Error> {
        validate_lists(lists)?;
        if !radius.is_finite() || origin.iter().any(|value| !value.is_finite()) {
            return Err(error(ErrorCode::NonFinite, None));
        }
        if !(0.0..=16384.0).contains(&radius) {
            return Err(error(ErrorCode::InvalidRange, None));
        }
        if origin
            .iter()
            .any(|value| !(-32768.0..=32768.0).contains(value))
        {
            return Err(error(ErrorCode::Unsupported, None));
        }
        let query = Hull {
            mins: origin.map(|value| (value - radius).clamp(-16384.0, 16384.0)),
            maxs: origin.map(|value| (value + radius).clamp(-16384.0, 16384.0)),
        };
        self.box_query(query, lists, maximum_results, accepts)
    }

    fn box_query(&self, query: Hull, lists: u16, maximum_results: usize, mut accepts: impl FnMut(u64) -> bool) -> Result<Vec<u64>, Error> {
        if lists == 0 || maximum_results == 0 {
            return Ok(Vec::new());
        }
        let mut ranked = Vec::new();
        for (visits, entry) in self.entries.iter().flatten().enumerate() {
            if visits >= self.visits {
                return Err(error(ErrorCode::Limit, None));
            }
            let Some(bounds) = entry.bounds else {
                continue;
            };
            if entry.hidden
                || entry.lists & lists == 0
                || (0..3).any(|axis| {
                    bounds.mins[axis] > query.maxs[axis] || bounds.maxs[axis] < query.mins[axis]
                })
            {
                continue;
            }
            let extent = (0..3)
                .map(|axis| bounds.maxs[axis] - bounds.mins[axis])
                .fold(0.0_f32, f32::max);
            let mut level = 0_u8;
            let mut span = 256.0_f32;
            while extent > span && level < 3 {
                level += 1;
                span *= 4.0;
            }
            // An entity is first encountered in its first intersected cell;
            // newer insertions precede older ones within that cell.
            let cell: [i32; 3] = std::array::from_fn(|axis| {
                (((bounds.mins[axis] + 16384.0) / span).floor() as i32)
                    .max(((query.mins[axis] + 16384.0) / span).floor() as i32)
            });
            ranked.push((level, cell, Reverse(entry.order), entry.identity));
        }
        ranked.sort_unstable();
        Ok(ranked
            .into_iter()
            .map(|entry| entry.3)
            .filter(|identity| accepts(*identity))
            .take(maximum_results)
            .collect())
    }

    pub fn ray(&self, start: [f32; 3], end: [f32; 3], lists: u16, maximum_results: usize,
        accepts: impl FnMut(u64) -> bool) -> Result<Vec<u64>, Error> {
        validate_lists(lists)?;
        if start.into_iter().chain(end).any(|value| !value.is_finite()) { return Err(error(ErrorCode::NonFinite, None)); }
        let delta: [f32; 3] = std::array::from_fn(|axis| end[axis] - start[axis]);
        if delta.iter().any(|value| !value.is_finite()) { return Err(error(ErrorCode::NonFinite, None)); }
        self.ray_delta(start, delta, lists, maximum_results, accepts)
    }

    fn ray_delta(&self, start: [f32; 3], delta: [f32; 3], lists: u16, maximum_results: usize,
        accepts: impl FnMut(u64) -> bool) -> Result<Vec<u64>, Error> {
        if delta.iter().map(|value| value * value).sum::<f32>() == 0.0 {
            return self.sphere(start.map(|value| value.clamp(-16384.0, 16384.0)), 0.0, lists, maximum_results, accepts);
        }
        if lists == 0 || maximum_results == 0 { return Ok(Vec::new()); }
        let Some((start, end, delta)) = clipped_ray(start, delta) else { return Ok(Vec::new()); };
        let shifted = start.map(|value| value + 16384.0);
        let shifted_end = end.map(|value| value + 16384.0);
        let mut cell = shifted.map(|value| (value / 256.0).floor() as i32);
        let mut crossings = Vec::new();
        for axis in 0..3 {
            if shifted[axis] == shifted_end[axis] { continue; }
            let step = if delta[axis] < 0.0 { -1 } else { 1 };
            let reciprocal = if step < 0 { -(1.0 / delta[axis]) } else { 1.0 / delta[axis] };
            let distance = if step < 0 { shifted[axis] - (cell[axis] as f32 * 256.0) }
                else { (cell[axis] + 1) as f32 * 256.0 - shifted[axis] };
            let mut time = distance * reciprocal;
            let interval = 256.0 * reciprocal;
            if time.is_nan() || interval <= 0.0 { return Err(error(ErrorCode::NonFinite, None)); }
            while time < 1.0 {
                if crossings.len() >= self.visits { return Err(error(ErrorCode::Limit, None)); }
                crossings.push((time, axis, step));
                let next = time + interval;
                if next <= time { return Err(error(ErrorCode::Limit, None)); }
                time = next;
            }
        }
        crossings.sort_by(|left, right| left.0.partial_cmp(&right.0).expect("finite crossing").then_with(|| right.1.cmp(&left.1)));
        let mut cells = BTreeMap::new();
        let mut ordinal = 0_usize;
        let mut visit = |cell: [i32; 3]| {
            for level in 0..4_u8 {
                let key = (level, cell.map(|value| value >> (u32::from(level) * 2)));
                if let std::collections::btree_map::Entry::Vacant(entry) = cells.entry(key) {
                    entry.insert(ordinal);
                    ordinal += 1;
                }
            }
        };
        visit(cell);
        for (_, axis, step) in crossings { cell[axis] += step; visit(cell); }
        self.ranked_cells(&cells, start, delta, [0.0; 3], lists, maximum_results, accepts)
    }

    pub fn sweep(&self, start: [f32; 3], end: [f32; 3], hull: Hull, lists: u16, maximum_results: usize,
        accepts: impl FnMut(u64) -> bool) -> Result<Vec<u64>, Error> {
        validate_lists(lists)?;
        if start.into_iter().chain(end).chain(hull.mins).chain(hull.maxs).any(|value| !value.is_finite()) { return Err(error(ErrorCode::NonFinite, None)); }
        if (0..3).any(|axis| hull.mins[axis] > hull.maxs[axis]) { return Err(error(ErrorCode::InvalidHull, None)); }
        let delta: [f32; 3] = std::array::from_fn(|axis| end[axis] - start[axis]);
        let start: [f32; 3] = std::array::from_fn(|axis| start[axis] + (hull.mins[axis] + hull.maxs[axis]) * 0.5);
        let extent: [f32; 3] = std::array::from_fn(|axis| (hull.maxs[axis] - hull.mins[axis]) * 0.5);
        if start.into_iter().chain(delta).chain(extent).any(|value| !value.is_finite()) { return Err(error(ErrorCode::NonFinite, None)); }
        if extent.iter().map(|value| value * value).sum::<f32>() < 1e-6 {
            return self.ray_delta(start, delta, lists, maximum_results, accepts);
        }
        if delta.iter().map(|value| value * value).sum::<f32>() == 0.0 {
            return self.box_query(clamped(Hull { mins: std::array::from_fn(|axis| start[axis] - extent[axis]), maxs: std::array::from_fn(|axis| start[axis] + extent[axis]) }), lists, maximum_results, accepts);
        }
        if lists == 0 || maximum_results == 0 { return Ok(Vec::new()); }
        let Some((start, end, delta)) = clipped_ray(start, delta) else { return Ok(Vec::new()); };
        let edges = [std::array::from_fn::<_, 3, _>(|axis| start[axis] - extent[axis]), std::array::from_fn(|axis| start[axis] + extent[axis])];
        let mut ranges = edges.map(|point| point.map(|value| ((value + 16384.0) / 256.0).floor() as i32));
        let mut cells = BTreeMap::new();
        let mut ordinal = 0;
        let mut visit = |level: u8, low: [i32; 3], high: [i32; 3]| -> Result<(), Error> {
            for x in low[0]..=high[0] { for y in low[1]..=high[1] { for z in low[2]..=high[2] {
                if ordinal >= self.visits { return Err(error(ErrorCode::Limit, None)); }
                cells.entry((level, [x, y, z])).or_insert(ordinal);
                ordinal += 1;
            } } }
            Ok(())
        };
        for level in 0..4_u8 { visit(level, ranges[0].map(|value| value >> (level * 2)), ranges[1].map(|value| value >> (level * 2)))?; }
        let end_low: [i32; 3] = std::array::from_fn(|axis| ((end[axis] - extent[axis] + 16384.0) / 256.0).floor() as i32);
        let end_high: [i32; 3] = std::array::from_fn(|axis| ((end[axis] + extent[axis] + 16384.0) / 256.0).floor() as i32);
        if !(0..3).all(|axis| end_low[axis] >= ranges[0][axis] && end_high[axis] <= ranges[1][axis]) {
            let mut crossings = Vec::new();
            for axis in 0..3 {
                if delta[axis] == 0.0 { continue; }
                let step = if delta[axis] < 0.0 { -1 } else { 1 };
                let inverse = if step < 0 { -(1.0 / delta[axis]) } else { 1.0 / delta[axis] };
                let interval = 256.0 * inverse;
                for trailing in [false, true] {
                    let edge = usize::from((step > 0) != trailing);
                    let shifted = edges[edge][axis] + 16384.0;
                    let distance = if step < 0 { shifted - ranges[edge][axis] as f32 * 256.0 }
                        else { -shifted - (ranges[edge][axis] + 1) as f32 * -256.0 };
                    if distance > delta[axis].abs() { continue; }
                    let mut time = distance * inverse;
                    while time < 1.0 {
                        if crossings.len() >= self.visits { return Err(error(ErrorCode::Limit, None)); }
                        crossings.push((time, trailing, axis, step, edge));
                        let next = time + interval;
                        if next <= time { return Err(error(ErrorCode::Limit, None)); }
                        time = next;
                    }
                }
            }
            crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite crossing").then_with(|| a.1.cmp(&b.1)).then_with(|| b.2.cmp(&a.2)));
            for (_, trailing, axis, step, edge) in crossings {
                let old = ranges[edge][axis];
                ranges[edge][axis] += step;
                if trailing { continue; }
                for level in 0..4_u8 {
                    let shift = level * 2;
                    let coordinate = ranges[edge][axis] >> shift;
                    if level > 0 && coordinate == old >> shift { continue; }
                    let mut low = ranges[0].map(|value| value >> shift);
                    let mut high = ranges[1].map(|value| value >> shift);
                    low[axis] = coordinate; high[axis] = coordinate;
                    visit(level, low, high)?;
                }
            }
        }
        self.ranked_cells(&cells, start, delta, extent, lists, maximum_results, accepts)
    }

    fn ranked_cells(&self, cells: &BTreeMap<(u8, [i32; 3]), usize>, start: [f32; 3], delta: [f32; 3], sweep_extent: [f32; 3],
        lists: u16, maximum_results: usize, mut accepts: impl FnMut(u64) -> bool) -> Result<Vec<u64>, Error> {
        let mut ranked = Vec::new();
        for (visits, entry) in self.entries.iter().flatten().enumerate() {
            if visits >= self.visits { return Err(error(ErrorCode::Limit, None)); }
            let Some(bounds) = entry.bounds else { continue; };
            if entry.hidden || entry.lists & lists == 0 { continue; }
            let extent = (0..3).map(|axis| bounds.maxs[axis] - bounds.mins[axis]).fold(0.0_f32, f32::max);
            let mut level = 0_u8;
            let mut span = 256.0_f32;
            while extent > span && level < 3 { level += 1; span *= 4.0; }
            let low = bounds.mins.map(|value| ((value + 16384.0) / span).floor() as i32);
            let high = bounds.maxs.map(|value| ((value + 16384.0) / span).floor() as i32);
            let mut first = None;
            for x in low[0]..=high[0] { for y in low[1]..=high[1] { for z in low[2]..=high[2] {
                if let Some(&order) = cells.get(&(level, [x, y, z])) { first = Some(first.map_or(order, |old: usize| old.min(order))); }
            } } }
            let Some(first) = first else { continue; };
            let expanded = Hull { mins: std::array::from_fn(|axis| bounds.mins[axis] - sweep_extent[axis]), maxs: std::array::from_fn(|axis| bounds.maxs[axis] + sweep_extent[axis]) };
            if intersects(start, delta, expanded) { ranked.push((first, Reverse(entry.order), entry.identity)); }
        }
        ranked.sort_unstable();
        Ok(ranked.into_iter().map(|entry| entry.2).filter(|identity| accepts(*identity)).take(maximum_results).collect())
    }

    fn entry(&self, handle: u16) -> Result<&Entry, Error> {
        self.entries
            .get(usize::from(handle))
            .and_then(Option::as_ref)
            .ok_or_else(|| error(ErrorCode::InvalidReference, Some(usize::from(handle))))
    }
}

fn clipped_ray(mut start: [f32; 3], mut delta: [f32; 3]) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let mut end: [f32; 3] = std::array::from_fn(|axis| start[axis] + delta[axis]);
    let inside = |point: [f32; 3]| point.into_iter().all(|value| (-16384.0..=16384.0).contains(&value));
    let start_inside = inside(start);
    let end_inside = inside(end);
    if !start_inside && !end_inside { return None; }
    if !start_inside {
        for axis in 0..3 {
            if delta[axis].abs() < 1e-10 { continue; }
            let time = if start[axis] < -16384.0 && delta[axis] > 0.0 {
                Some((-16379.0 - start[axis]) / delta[axis])
            } else if start[axis] > 16384.0 && delta[axis] < 0.0 {
                Some((start[axis] - 16379.0) / -delta[axis])
            } else { None };
            if let Some(time) = time { start = std::array::from_fn(|axis| start[axis] + time * delta[axis]); }
        }
        delta = std::array::from_fn(|axis| end[axis] - start[axis]);
    } else if !end_inside {
        for axis in 0..3 {
            if delta[axis].abs() < 1e-10 { continue; }
            let time = if end[axis] < -16384.0 && delta[axis] < 0.0 {
                Some((start[axis] - -16379.0) / -delta[axis])
            } else if end[axis] > 16384.0 && delta[axis] > 0.0 {
                Some((start[axis] - -16379.0) / delta[axis])
            } else { None };
            if let Some(time) = time { end = std::array::from_fn(|axis| start[axis] + time * delta[axis]); }
        }
        delta = std::array::from_fn(|axis| end[axis] - start[axis]);
    }
    Some((start, end, delta))
}

fn intersects(start: [f32; 3], delta: [f32; 3], bounds: Hull) -> bool {
    let mut enter = -f32::MAX;
    let mut leave = f32::MAX;
    for axis in 0..3 {
        let low = bounds.mins[axis] - start[axis];
        let high = bounds.maxs[axis] - start[axis];
        let before = low > 0.0;
        let after = high < 0.0;
        let end_before = delta[axis] < low;
        let end_after = delta[axis] > high;
        if before && end_before || after && end_after { return false; }
        if before != end_before || after != end_after {
            let inverse = if delta[axis] != 0.0 { 1.0 / delta[axis] } else { f32::MAX };
            let a = low * inverse;
            let b = high * inverse;
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
    }
    enter.max(0.0) <= leave.min(1.0)
}

fn validate_lists(lists: u16) -> Result<(), Error> {
    if lists & !SERVER_LISTS != 0 {
        Err(error(ErrorCode::Unsupported, None))
    } else {
        Ok(())
    }
}

fn expanded(bounds: Hull) -> Result<Hull, Error> {
    if bounds
        .mins
        .into_iter()
        .chain(bounds.maxs)
        .any(|value| !value.is_finite())
    {
        return Err(error(ErrorCode::NonFinite, None));
    }
    if (0..3).any(|axis| bounds.mins[axis] > bounds.maxs[axis]) {
        return Err(error(ErrorCode::InvalidHull, None));
    }
    Ok(Hull {
        mins: bounds.mins.map(|value| value - 0.03125),
        maxs: bounds.maxs.map(|value| value + 0.03125),
    })
}

fn clamped(bounds: Hull) -> Hull {
    Hull { mins: bounds.mins.map(|value| value.clamp(-16384.0, 16384.0)),
        maxs: bounds.maxs.map(|value| value.clamp(-16384.0, 16384.0)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rays_interleave_size_tiers_as_cells_are_entered() {
        let mut space = EntityQuerySpace::new(16, 4096).unwrap();
        space.create(1, Hull { mins: [298.0, -2.0, -2.0], maxs: [302.0, 2.0, 2.0] }, 17).unwrap();
        space.create(2, Hull { mins: [-1000.0; 3], maxs: [1000.0; 3] }, 17).unwrap();
        let before = space.clone();
        assert_eq!(space.ray([0.0; 3], [512.0, 0.0, 0.0], 1, 16, |_| true).unwrap(), [2, 1]);
        assert_eq!(space.ray([512.0, 0.0, 0.0], [0.0; 3], 1, 16, |_| true).unwrap(), [2, 1]);
        assert_eq!(space.ray([0.0; 3], [512.0, 0.0, 0.0], 1, 1, |identity| identity == 1).unwrap(), [1]);
        assert_eq!(space, before);
    }

    #[test]
    fn ray_world_edge_admission_is_not_symmetric_segment_clipping() {
        let mut space = EntityQuerySpace::new(16, 4096).unwrap();
        space.create(1, Hull { mins: [16384.0, 0.0, 0.0], maxs: [16384.0, 0.0, 0.0] }, 17).unwrap();
        assert_eq!(space.ray([5000.0, 0.0, 0.0], [20000.0, 0.0, 0.0], 1, 16, |_| true).unwrap(), [1]);
        assert!(space.ray([20000.0, 0.0, 0.0], [0.0; 3], 1, 16, |_| true).unwrap().is_empty());
        assert!(space.ray([-20000.0, 0.0, 0.0], [20000.0, 0.0, 0.0], 1, 16, |_| true).unwrap().is_empty());
    }
    #[test]
    fn bounds_crossing_the_partition_world_are_clamped_and_reinserted_on_repeat_moves() {
        let mut space = EntityQuerySpace::new(16, 16).unwrap();
        let bounds = Hull { mins: [-16408.0, -2.0, -2.0], maxs: [-16360.0, 2.0, 2.0] };
        let first = space.create(1, bounds, 17).unwrap();
        let second = space.create(2, bounds, 17).unwrap();
        let query = |space: &EntityQuerySpace| space.sphere([-16384.0, 0.0, 0.0], 8.0, 16, 16, |_| true).unwrap();
        assert_eq!(query(&space), [2, 1]);
        space.move_bounds(first, bounds).unwrap();
        assert_eq!(query(&space), [1, 2]);
        space.move_bounds(second, bounds).unwrap();
        assert_eq!(query(&space), [2, 1]);
        space.create(3, Hull { mins: [20000.0, 0.0, 0.0], maxs: [20000.0, 0.0, 0.0] }, 17).unwrap();
        assert_eq!(space.sphere([16384.0, 0.0, 0.0], 0.0, 16, 16, |_| true).unwrap(), [3]);
    }
    #[test]
    fn cell_rounding_and_world_query_clamping_are_not_geometric_shortcuts() {
        let mut space = EntityQuerySpace::new(128, 128).unwrap();
        let point = |x| Hull {
            mins: [x, -0.03125, 0.0],
            maxs: [x, -0.03125, 0.0],
        };
        space.create(20, point(0.0), 16).unwrap();
        space
            .create(10, point(f32::from_bits(0x3cff_ffff)), 16)
            .unwrap();
        assert_eq!(
            space.sphere([0.0; 3], 128.0, 16, 512, |_| true).unwrap(),
            [20, 10]
        );
        assert_eq!(
            space.sphere([0.0; 3], 128.0, 16, 1, |id| id == 10).unwrap(),
            [10]
        );
        space.create(30, point(16384.0), 16).unwrap();
        assert_eq!(
            space
                .sphere([32768.0, 0.0, 0.0], 0.0, 16, 512, |_| true)
                .unwrap(),
            [30]
        );
    }

    #[test]
    fn rejected_mutations_preserve_history_and_recycled_handles() {
        let mut space = EntityQuerySpace::new(2, 2).unwrap();
        let bounds = Hull {
            mins: [-1.0; 3],
            maxs: [1.0; 3],
        };
        let first = space.create(10, bounds, 16).unwrap();
        let second = space.create(20, bounds, 16).unwrap();
        assert_eq!(
            space.sphere([0.0; 3], 2.0, 16, 2, |_| true).unwrap(),
            [20, 10]
        );
        let before = space.clone();
        assert!(space.create(30, bounds, 16).is_err());
        assert!(
            space
                .move_bounds(
                    first,
                    Hull {
                        mins: [f32::NAN; 3],
                        ..bounds
                    }
                )
                .is_err()
        );
        assert!(space.change_lists(first, 0, 4).is_err());
        assert!(space.destroy(u16::MAX).is_err());
        assert_eq!(space, before);
        let mut branch = space.clone();
        branch.destroy(first).unwrap();
        branch.create(30, bounds, 16).unwrap();
        assert_eq!(
            branch.sphere([0.0; 3], 2.0, 16, 512, |_| true).unwrap(),
            [30, 20]
        );
        assert_eq!(space, before);
        space.move_bounds(first, bounds).unwrap();
        assert_eq!(space, before);
        space.destroy(first).unwrap();
        space.destroy(second).unwrap();
        assert_eq!(space.create(30, bounds, 16).unwrap(), second);
        assert_eq!(space.create(40, bounds, 16).unwrap(), first);
    }
}
