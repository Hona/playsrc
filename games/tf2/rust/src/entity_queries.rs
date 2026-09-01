// Portions adapted from Valve's official Source SDK 2013.
// Copyright Valve Corporation, All rights reserved.
// See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt at the repository root.
use playsrc_collision::{EntityQuerySpace, Error, ErrorCode, Hull};
use playsrc_entity::EntityHandle;
use std::collections::BTreeMap;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Entity {
    Map(EntityHandle),
    Actor(u32),
    Projectile(u32),
    Building(u32),
    Follower { parent: EntityHandle, solid: u16 },
    StaticProp(u32),
}

impl Entity {
    fn token(self) -> u64 {
        match self {
            Self::Map(handle) => u64::from(handle.slot) | u64::from(handle.generation) << 16,
            Self::Actor(identity) => 1 << 61 | u64::from(identity),
            Self::Projectile(identity) => 2 << 61 | u64::from(identity),
            Self::Building(identity) => 3 << 61 | u64::from(identity),
            Self::Follower { parent, solid } => 4 << 61 | u64::from(solid) << 48 | u64::from(parent.generation) << 16 | u64::from(parent.slot),
            Self::StaticProp(index) => 5 << 61 | u64::from(index),
        }
    }

    fn from_token(token: u64) -> Self {
        match token >> 61 {
            0 => Self::Map(EntityHandle { slot: token as u16, generation: (token >> 16) as u32 }),
            1 => Self::Actor(token as u32),
            2 => Self::Projectile(token as u32),
            3 => Self::Building(token as u32),
            4 => Self::Follower { parent: EntityHandle { slot: token as u16, generation: (token >> 16) as u32 }, solid: ((token >> 48) & 0x1fff) as u16 },
            5 => Self::StaticProp(token as u32),
            _ => unreachable!("registered entity namespace"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub origin: [f32; 3],
    /// None denotes zero collision radius, independently of surrounding bounds.
    pub surrounding: Option<Hull>,
}

impl Bounds {
    pub fn nearest_and_center(origin: [f32; 3], local: Hull, angles: Option<[f32; 3]>, point: [f32; 3]) -> Result<([f32; 3], [f32; 3]), Error> {
        let center: [f32; 3] = std::array::from_fn(|axis| (local.mins[axis] + local.maxs[axis]) * 0.5);
        let Some(angles) = angles.filter(|angles| *angles != [0.0; 3]) else {
            return Ok((std::array::from_fn(|axis| (point[axis] - origin[axis]).clamp(local.mins[axis], local.maxs[axis]) + origin[axis]),
                std::array::from_fn(|axis| center[axis] + origin[axis])));
        };
        use playsrc_studio_model::{Float32, Vector3};
        let matrix = playsrc_studio_model::source_entity_transform(Vector3(origin.map(|value| Float32(value.to_bits()))),
            Vector3(angles.map(|value| Float32(value.to_bits())))).map_err(|_| failure(ErrorCode::NonFinite))?.0.map(|value| f32::from_bits(value.0));
        let delta: [f32; 3] = std::array::from_fn(|axis| point[axis] - matrix[axis * 4 + 3]);
        let nearest: [f32; 3] = std::array::from_fn(|axis| ((delta[0] * matrix[axis] + delta[1] * matrix[4 + axis]) + delta[2] * matrix[8 + axis]).clamp(local.mins[axis], local.maxs[axis]));
        let transform = |point: [f32; 3]| std::array::from_fn(|axis| {
            let row = &matrix[axis * 4..axis * 4 + 4];
            ((point[0] * row[0] + point[1] * row[1]) + point[2] * row[2]) + row[3]
        });
        Ok((transform(nearest), transform(center)))
    }

    pub fn collision(origin: [f32; 3], local: Hull, angles: Option<[f32; 3]>) -> Result<Self, Error> {
        if origin.into_iter().chain(local.mins).chain(local.maxs).any(|value| !value.is_finite()) {
            return Err(failure(ErrorCode::NonFinite));
        }
        if (0..3).any(|axis| local.mins[axis] > local.maxs[axis]) { return Err(failure(ErrorCode::InvalidHull)); }
        let diagonal: [f32; 3] = std::array::from_fn(|axis| local.maxs[axis] - local.mins[axis]);
        let radius = ((diagonal[0] * diagonal[0] + diagonal[1] * diagonal[1]) + diagonal[2] * diagonal[2]).sqrt() * 0.5;
        if radius == 0.0 { return Ok(Self { origin, surrounding: None }); }
        let surrounding = if let Some(angles) = angles.filter(|angles| *angles != [0.0; 3]) {
            use playsrc_studio_model::{Float32, Vector3};
            let matrix = playsrc_studio_model::source_entity_transform(Vector3(origin.map(|value| Float32(value.to_bits()))),
                Vector3(angles.map(|value| Float32(value.to_bits())))).map_err(|_| failure(ErrorCode::NonFinite))?.0.map(|value| f32::from_bits(value.0));
            let center: [f32; 3] = std::array::from_fn(|axis| (local.mins[axis] + local.maxs[axis]) * 0.5);
            let extent: [f32; 3] = std::array::from_fn(|axis| local.maxs[axis] - center[axis]);
            let world_center: [f32; 3] = std::array::from_fn(|axis| {
                let row = &matrix[axis * 4..axis * 4 + 4];
                ((center[0] * row[0] + center[1] * row[1]) + center[2] * row[2]) + row[3]
            });
            let world_extent: [f32; 3] = std::array::from_fn(|axis| {
                let row = &matrix[axis * 4..axis * 4 + 3];
                ((extent[0] * row[0]).abs() + (extent[1] * row[1]).abs()) + (extent[2] * row[2]).abs()
            });
            Hull { mins: std::array::from_fn(|axis| world_center[axis] - world_extent[axis]),
                maxs: std::array::from_fn(|axis| world_center[axis] + world_extent[axis]) }
        } else {
            // Unrotated bounds use direct addition, not a center/extents round trip.
            Hull { mins: std::array::from_fn(|axis| local.mins[axis] + origin[axis]),
                maxs: std::array::from_fn(|axis| local.maxs[axis] + origin[axis]) }
        };
        if surrounding.mins.into_iter().chain(surrounding.maxs).any(|value| !value.is_finite()) {
            return Err(failure(ErrorCode::NonFinite));
        }
        Ok(Self { origin, surrounding: Some(surrounding) })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Geometry {
    origin: [f32; 3],
    local: Hull,
    angles: Option<[f32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    handle: Option<u16>,
    lists: u16,
    dirty: bool,
    geometry: Option<Geometry>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Queries {
    space: EntityQuerySpace,
    entries: BTreeMap<Entity, Entry>,
    dirty: Vec<Entity>,
    updating: bool,
    maximum_entries: usize,
    maximum_updates: usize,
    static_identities: Vec<u64>,
    pub brushes_initialized: bool,
}

fn failure(code: ErrorCode) -> Error { Error { code, item: None, range: None } }

pub(crate) struct QueryWorld<'a, W> {
    pub world: &'a W,
    pub queries: RefCell<&'a mut Queries>,
}

impl<W> QueryWorld<'_, W> {
    fn flush(&self) -> Result<(), playsrc_movement::Error> {
        self.queries.borrow_mut().flush_bound().map_err(|_| playsrc_movement::Error::new(
            playsrc_movement::Operation::Trace, playsrc_movement::FailureKind::Malformed, "server entity partition update"))
    }
}

impl<W: playsrc_movement::Tracer> playsrc_movement::Tracer for QueryWorld<'_, W> {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace(start, end, hull, mask)
    }
    fn trace_without(&self, ignored: u64, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_without(ignored, start, end, hull, mask)
    }
    fn point_contents(&self, point: [f32; 3]) -> Result<u32, playsrc_movement::Error> {
        self.flush()?;
        self.world.point_contents(point)
    }
    fn impact_surface(&self, start: [f32; 3], end: [f32; 3], mask: u32) -> Result<Option<playsrc_movement::ImpactSurface>, playsrc_movement::Error> { self.world.impact_surface(start, end, mask) }
    fn support_velocity(&self, support: u64) -> Result<[f32; 3], playsrc_movement::Error> { self.world.support_velocity(support) }
    fn support_is_floating(&self, support: u64) -> Result<bool, playsrc_movement::Error> { self.world.support_is_floating(support) }
    fn conveyor_velocity(&self, support: u64) -> Result<Option<[f32; 3]>, playsrc_movement::Error> { self.world.conveyor_velocity(support) }
    fn is_world(&self, hit: u64) -> bool { self.world.is_world(hit) }
    fn surface_climbable(&self, hit: Option<u64>) -> bool { self.world.surface_climbable(hit) }
    fn mover_motion(&self, position: [f32; 3], hull: Hull, support: Option<u64>) -> Result<Option<playsrc_movement::MoverMotion>, playsrc_movement::Error> { self.world.mover_motion(position, hull, support) }
    fn overlaps_mover(&self, mover: u64, position: [f32; 3], hull: Hull) -> Result<bool, playsrc_movement::Error> { self.world.overlaps_mover(mover, position, hull) }
    fn observer_target(&self, target: u64) -> Result<Option<playsrc_movement::ObserverTarget>, playsrc_movement::Error> { self.world.observer_target(target) }
    fn movement_time(&self) -> Option<f32> { self.world.movement_time() }
}

impl<W: crate::GameplayWorld> crate::GameplayWorld for QueryWorld<'_, W> {
    fn trace_world(&self, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_world(start, end, hull, mask)
    }
    fn trace_brush(&self, model: usize, request: playsrc_collision::ObjectTraceRequest) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_brush(model, request)
    }
    fn trace_static(&self, identity: u64, start: [f32; 3], end: [f32; 3], hull: Hull, mask: u32) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_static(identity, start, end, hull, mask)
    }
    fn static_query_bounds(&self) -> Result<Vec<(u64, Hull)>, playsrc_movement::Error> { self.world.static_query_bounds() }
    fn trace_projectile_solid(&self, start: [f32; 3], end: [f32; 3], mask: u32) -> Result<crate::ProjectileSolidTrace, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_projectile_solid(start, end, mask)
    }
    fn trace_grenade_entities(&self, start: [f32; 3], end: [f32; 3], thrower: u32, hitboxes: &[crate::PosedPlayerHitbox]) -> Result<Option<crate::GrenadeEntityHit>, playsrc_movement::Error> {
        self.flush()?;
        self.world.trace_grenade_entities(start, end, thrower, hitboxes)
    }
    fn actor_query_state(&self, identity: u32, origin: [f32; 3], solid: bool) -> Result<(), playsrc_movement::Error> {
        self.queries.borrow_mut().actor_state(identity, origin, solid).map_err(|_| playsrc_movement::Error::new(
            playsrc_movement::Operation::Trace, playsrc_movement::FailureKind::Malformed, "actor entity partition update"))?;
        self.world.actor_query_state(identity, origin, solid)
    }
    fn bot_update_milliseconds(&self) -> f64 { self.world.bot_update_milliseconds() }
    fn has_player_hitbox_models(&self) -> bool { self.world.has_player_hitbox_models() }
    fn pose_player_hitboxes(&self, actors: &[crate::PlayerHitboxPose], tick: u64, interval: f32) -> Result<Vec<crate::PosedPlayerHitbox>, playsrc_movement::Error> { self.world.pose_player_hitboxes(actors, tick, interval) }
    fn combat_player(&self, identity: u32) -> Option<crate::map_runtime::CombatPlayerFacts> { self.world.combat_player(identity) }
    fn collision_snapshot_revision(&self) -> Option<u64> { self.world.collision_snapshot_revision() }
    fn overlaps_model_hull(&self, model: usize, origin: [f32; 3], position: [f32; 3], hull: Hull) -> Result<bool, playsrc_movement::Error> { self.world.overlaps_model_hull(model, origin, position, hull) }
    fn overlaps_transformed_model_hull(&self, model: usize, transform: playsrc_entity::Transform, position: [f32; 3], hull: Hull) -> Result<bool, playsrc_movement::Error> { self.world.overlaps_transformed_model_hull(model, transform, position, hull) }
}

impl Queries {
    pub fn new(maximum_entries: usize, maximum_updates: usize) -> Result<Self, Error> {
        Ok(Self { space: EntityQuerySpace::new(maximum_entries, maximum_updates)?, entries: BTreeMap::new(),
            dirty: Vec::new(), updating: false, maximum_entries, maximum_updates, static_identities: Vec::new(), brushes_initialized: false })
    }

    pub fn register_static(&mut self, identity: u64, bounds: Hull) -> Result<(), Error> {
        if self.static_identities.contains(&identity) { return Err(failure(ErrorCode::DuplicateIdentity)); }
        if self.entries.len() >= self.maximum_entries { return Err(failure(ErrorCode::Limit)); }
        let index = u32::try_from(self.static_identities.len()).map_err(|_| failure(ErrorCode::Limit))?;
        let entity = Entity::StaticProp(index);
        let handle = self.space.create(entity.token(), bounds, 0x41)?;
        self.entries.insert(entity, Entry { handle: Some(handle), lists: 0x41, dirty: false, geometry: None });
        self.static_identities.push(identity);
        Ok(())
    }

    pub fn static_identity(&self, index: u32) -> Result<u64, Error> {
        self.static_identities.get(index as usize).copied().ok_or_else(|| failure(ErrorCode::InvalidReference))
    }

    pub fn contains(&self, entity: Entity) -> bool { self.entries.contains_key(&entity) }

    pub fn solid(&self, entity: Entity) -> Result<bool, Error> {
        self.entries.get(&entity).map(|entry| entry.lists & 1 != 0).ok_or_else(|| failure(ErrorCode::InvalidReference))
    }

    pub fn collision_geometry(&self, entity: Entity) -> Result<(playsrc_entity::Transform, Hull), Error> {
        let geometry = self.entries.get(&entity).and_then(|entry| entry.geometry.as_ref()).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        Ok((playsrc_entity::Transform { origin: geometry.origin, angles: geometry.angles.unwrap_or([0.0; 3]) }, geometry.local))
    }

    pub fn register(&mut self, entity: Entity) -> Result<(), Error> {
        if entity == Entity::Map(EntityHandle::NULL) { return Err(failure(ErrorCode::InvalidReference)); }
        if let Entity::Follower { parent, solid } = entity {
            if parent == EntityHandle::NULL { return Err(failure(ErrorCode::InvalidReference)); }
            if solid >= 8192 { return Err(failure(ErrorCode::Limit)); }
        }
        if self.entries.contains_key(&entity) { return Err(failure(ErrorCode::DuplicateIdentity)); }
        if self.entries.len() >= self.maximum_entries { return Err(failure(ErrorCode::Limit)); }
        self.entries.insert(entity, Entry { handle: None, lists: 0, dirty: false, geometry: None });
        Ok(())
    }

    pub fn mark_dirty(&mut self, entity: Entity) -> Result<(), Error> {
        let entry = self.entries.get_mut(&entity).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        if !entry.dirty {
            entry.dirty = true;
            self.dirty.push(entity);
        }
        Ok(())
    }

    pub fn set_origin(&mut self, entity: Entity, origin: &mut [f32; 3], next: [f32; 3]) -> Result<(), Error> {
        if *origin != next {
            self.mark_dirty(entity)?;
            if let Some(geometry) = &mut self.entries.get_mut(&entity).expect("validated entity").geometry { geometry.origin = next; }
            *origin = next;
        }
        Ok(())
    }

    pub fn bind_bounds(&mut self, entity: Entity, origin: [f32; 3], local: Hull, angles: Option<[f32; 3]>) -> Result<(), Error> {
        let geometry = Geometry { origin, local, angles };
        let entry = self.entries.get_mut(&entity).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        if entry.geometry.as_ref() != Some(&geometry) {
            entry.geometry = Some(geometry);
            self.mark_dirty(entity)?;
        }
        Ok(())
    }

    pub fn set_transform(&mut self, entity: Entity, transform: playsrc_entity::Transform) -> Result<(), Error> {
        let geometry = self.entries.get(&entity).and_then(|entry| entry.geometry.as_ref()).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        self.bind_bounds(entity, transform.origin, geometry.local, geometry.angles.map(|_| transform.angles))
    }

    pub fn actor_state(&mut self, identity: u32, origin: [f32; 3], solid: bool) -> Result<(), Error> {
        let entity = Entity::Actor(identity);
        if !self.entries.contains_key(&entity) { self.register(entity)?; }
        self.bind_bounds(entity, origin, crate::STANDING_PLAYER_HULL, None)?;
        self.set_solid(entity, solid)
    }

    pub fn flush_bound(&mut self) -> Result<(), Error> {
        self.flush(0x13, |_| false, |queries, entity| {
            let geometry = queries.entries.get(&entity).and_then(|entry| entry.geometry.as_ref())
                .ok_or_else(|| failure(ErrorCode::InvalidReference))?;
            Bounds::collision(geometry.origin, geometry.local, geometry.angles)
        })
    }

    pub fn set_lists(&mut self, entity: Entity, lists: u16) -> Result<(), Error> {
        if lists & !0x13 != 0 { return Err(failure(ErrorCode::Unsupported)); }
        let entry = self.entries.get_mut(&entity).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        if let Some(handle) = entry.handle { self.space.change_lists(handle, 0x13, lists)?; }
        entry.lists = lists;
        Ok(())
    }

    pub fn set_solid(&mut self, entity: Entity, solid: bool) -> Result<(), Error> {
        let lists = if solid { 0x11 } else { 0 };
        if self.entries.get(&entity).ok_or_else(|| failure(ErrorCode::InvalidReference))?.lists != lists {
            self.set_lists(entity, lists)?;
            self.mark_dirty(entity)?;
        }
        Ok(())
    }

    pub fn destroy(&mut self, entity: Entity) -> Result<(), Error> {
        let entry = self.entries.get(&entity).ok_or_else(|| failure(ErrorCode::InvalidReference))?;
        if let Some(handle) = entry.handle { self.space.destroy(handle)?; }
        self.entries.remove(&entity);
        self.dirty.retain(|pending| *pending != entity);
        Ok(())
    }

    /// Called inside the owning Session transaction, before a server game query.
    pub fn flush(&mut self, lists: u16, mut setting_up_bones: impl FnMut(Entity) -> bool,
        mut bounds: impl FnMut(&mut Self, Entity) -> Result<Bounds, Error>) -> Result<(), Error> {
        if lists & 0x13 == 0 || self.updating { return Ok(()); }
        self.updating = true;
        let result = (|| {
            let mut deferred = Vec::new();
            let mut updates = 0;
            while !self.dirty.is_empty() {
                let mut batch = std::mem::take(&mut self.dirty);
                while let Some(entity) = batch.pop() {
                    updates += 1;
                    if updates > self.maximum_updates { return Err(failure(ErrorCode::Limit)); }
                    let Some(entry) = self.entries.get_mut(&entity) else { continue; };
                    if setting_up_bones(entity) { deferred.push(entity); continue; }
                    if !entry.dirty { continue; }
                    entry.dirty = false;
                    let handle = match entry.handle {
                        Some(handle) => handle,
                        None => {
                            let handle = self.space.allocate(entity.token())?;
                            self.space.change_lists(handle, 0, entry.lists)?;
                            entry.handle = Some(handle);
                            handle
                        }
                    };
                    if entry.lists != 0 {
                        let value = bounds(self, entity)?;
                        if self.entries.contains_key(&entity) {
                            let bounds = value.surrounding.map_or(Hull { mins: value.origin, maxs: value.origin }, |bounds|
                                Hull { mins: bounds.mins.map(|value| value - 1.0), maxs: bounds.maxs.map(|value| value + 1.0) });
                            self.space.move_bounds(handle, bounds)?;
                        }
                    }
                }
                if self.dirty.is_empty() { self.dirty = batch; break; }
            }
            let entries = &self.entries;
            self.dirty.extend(deferred.into_iter().filter(|entity| entries.contains_key(entity)));
            Ok(())
        })();
        self.updating = false;
        result
    }

    pub fn sphere(&self, origin: [f32; 3], radius: f32, lists: u16, maximum: usize,
        mut accepts: impl FnMut(Entity) -> bool) -> Result<Vec<Entity>, Error> {
        Ok(self.space.sphere(origin, radius, lists, maximum, |token| accepts(Entity::from_token(token)))?
            .into_iter().map(Entity::from_token).collect())
    }

    pub fn ray(&self, start: [f32; 3], end: [f32; 3], lists: u16, maximum: usize,
        mut accepts: impl FnMut(Entity) -> bool) -> Result<Vec<Entity>, Error> {
        Ok(self.space.ray(start, end, lists, maximum, |token| accepts(Entity::from_token(token)))?
            .into_iter().map(Entity::from_token).collect())
    }

    pub fn sweep(&self, start: [f32; 3], end: [f32; 3], hull: Hull, lists: u16, maximum: usize,
        mut accepts: impl FnMut(Entity) -> bool) -> Result<Vec<Entity>, Error> {
        Ok(self.space.sweep(start, end, hull, lists, maximum, |token| accepts(Entity::from_token(token)))?
            .into_iter().map(Entity::from_token).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_movement::Tracer;
    use crate::GameplayWorld;

    #[test]
    fn static_props_use_engine_padding_and_stay_out_of_the_non_static_list() {
        let mut queries = Queries::new(16, 64).unwrap();
        let identity = 0x8000_0000_0000_0007;
        queries.register_static(identity, Hull { mins: [128.0; 3], maxs: [128.0; 3] }).unwrap();
        assert_eq!(queries.static_identity(0).unwrap(), identity);
        assert!(queries.sphere([128.0; 3], 0.0, 0x10, 16, |_| true).unwrap().is_empty());
        assert_eq!(queries.sphere([128.03; 3], 0.0, 0x40, 16, |_| true).unwrap(), [Entity::StaticProp(0)]);
        assert!(queries.sphere([128.04; 3], 0.0, 1, 16, |_| true).unwrap().is_empty());
        queries.flush_bound().unwrap();
        let clone = queries.clone();
        assert_eq!(clone, queries);
        assert_eq!(queries.register_static(identity, Hull { mins: [0.0; 3], maxs: [1.0; 3] }).unwrap_err().code, ErrorCode::DuplicateIdentity);
        assert_eq!(clone, queries);
        let follower = Entity::Follower { parent: EntityHandle { slot: 65534, generation: u32::MAX }, solid: 8191 };
        assert_eq!(Entity::from_token(follower.token()), follower);
        assert_ne!(follower.token(), Entity::Map(EntityHandle { slot: 65534, generation: u32::MAX }).token());
    }

    struct Probe(std::cell::Cell<usize>);
    impl Tracer for Probe {
        fn trace(&self, start: [f32; 3], end: [f32; 3], _: Hull, _: u32) -> Result<playsrc_movement::Trace, playsrc_movement::Error> {
            self.0.set(self.0.get() + 1);
            Ok(playsrc_movement::Trace { fraction: 1.0, start_solid: false, all_solid: false, end, normal: None, hit: None, contents: u32::from(start == end) })
        }
        fn support_velocity(&self, _: u64) -> Result<[f32; 3], playsrc_movement::Error> { Ok([1.0, 2.0, 3.0]) }
        fn support_is_floating(&self, _: u64) -> Result<bool, playsrc_movement::Error> { Ok(true) }
        fn movement_time(&self) -> Option<f32> { Some(7.0) }
        fn is_world(&self, hit: u64) -> bool { hit == 42 }
    }
    impl GameplayWorld for Probe {
        fn overlaps_model_hull(&self, _: usize, _: [f32; 3], _: [f32; 3], _: Hull) -> Result<bool, playsrc_movement::Error> { Ok(true) }
        fn collision_snapshot_revision(&self) -> Option<u64> { Some(17) }
    }

    #[test]
    fn trace_adapter_flushes_geometry_only_at_queries_and_preserves_environment_methods() {
        let mut queries = Queries::new(16, 64).unwrap();
        let probe = Probe(std::cell::Cell::new(0));
        let world = QueryWorld { world: &probe, queries: RefCell::new(&mut queries) };
        world.actor_query_state(1, [128.0; 3], true).unwrap();
        assert!(world.queries.borrow().sphere([128.0; 3], 0.0, 16, 16, |_| true).unwrap().is_empty());
        assert_eq!(world.support_velocity(42).unwrap(), [1.0, 2.0, 3.0]);
        assert!(world.support_is_floating(42).unwrap());
        assert_eq!(world.movement_time(), Some(7.0));
        assert!(world.is_world(42));
        assert!(!world.is_world(0));
        assert_eq!(world.collision_snapshot_revision(), Some(17));
        assert!(world.overlaps_model_hull(1, [0.0; 3], [0.0; 3], Hull { mins: [0.0; 3], maxs: [0.0; 3] }).unwrap());
        assert_eq!(probe.0.get(), 0);
        world.trace([0.0; 3], [1.0; 3], Hull { mins: [0.0; 3], maxs: [0.0; 3] }, 1).unwrap();
        assert_eq!(world.queries.borrow().sphere([128.0; 3], 0.0, 16, 16, |_| true).unwrap(), [Entity::Actor(1)]);
        world.actor_query_state(1, [128.0; 3], false).unwrap();
        assert!(world.queries.borrow().sphere([128.0; 3], 0.0, 16, 16, |_| true).unwrap().is_empty());
        world.actor_query_state(1, [256.0; 3], true).unwrap();
        world.point_contents([0.0; 3]).unwrap();
        assert_eq!(probe.0.get(), 2);
        assert_eq!(world.queries.borrow().sphere([256.0; 3], 0.0, 16, 16, |_| true).unwrap(), [Entity::Actor(1)]);
    }

    #[test]
    fn invalid_pending_geometry_fails_before_the_underlying_query() {
        let mut queries = Queries::new(16, 64).unwrap();
        let probe = Probe(std::cell::Cell::new(0));
        let world = QueryWorld { world: &probe, queries: RefCell::new(&mut queries) };
        world.actor_query_state(1, [f32::NAN, 0.0, 0.0], true).unwrap();
        assert!(world.point_contents([0.0; 3]).is_err());
        assert_eq!(probe.0.get(), 0);
    }

    fn point() -> Bounds { Bounds { origin: [128.0; 3], surrounding: None } }
    fn register(queries: &mut Queries, entity: Entity) {
        queries.register(entity).unwrap();
        queries.set_lists(entity, 0x11).unwrap();
        queries.mark_dirty(entity).unwrap();
    }
    fn all(queries: &Queries) -> Vec<Entity> { queries.sphere([128.0; 3], 8.0, 0x10, 32, |_| true).unwrap() }

    #[test]
    fn collision_bounds_keep_zero_radius_and_unrotated_float_order() {
        let point = Bounds::collision([128.0; 3], Hull { mins: [10.0; 3], maxs: [10.0; 3] }, Some([0.0, 90.0, 0.0])).unwrap();
        assert_eq!(point.origin, [128.0; 3]);
        assert_eq!(point.surrounding, None);
        let thin = Hull { mins: [0.0; 3], maxs: [0.0025, 2.0, 2.0] };
        let unrotated = Bounds::collision([8192.0, 0.0, 0.0], thin, Some([0.0; 3])).unwrap().surrounding.unwrap();
        assert_eq!(unrotated.maxs[0].to_bits(), (8192.0_f32 + 0.0025).to_bits());
        let rotated = Bounds::collision([128.0, -64.0, 32.0], Hull { mins: [-10.25, 3.125, -4.5], maxs: [20.5, 8.75, 3.25] }, Some([0.0, 90.0, 0.0])).unwrap().surrounding.unwrap();
        assert_eq!(rotated, Hull { mins: [119.25, -74.25, 27.5], maxs: [124.875, -43.5, 35.25] });
    }

    #[test]
    fn dirty_batches_preserve_first_mark_and_process_new_marks_after_the_detached_batch() {
        let mut queries = Queries::new(32, 128).unwrap();
        let a = Entity::Actor(1);
        let b = Entity::Projectile(1);
        let c = Entity::Map(EntityHandle { slot: 1, generation: 3 });
        let d = Entity::Building(1);
        for entity in [a, b, c] { register(&mut queries, entity); }
        queries.mark_dirty(a).unwrap();
        queries.register(d).unwrap();
        queries.set_lists(d, 0x11).unwrap();
        let mut visits = Vec::new();
        queries.flush(0x10, |_| false, |queries, entity| {
            visits.push(entity);
            if entity == c { queries.mark_dirty(d)?; }
            queries.flush(0x10, |_| panic!("nested update"), |_, _| panic!("nested bounds"))?;
            Ok(point())
        }).unwrap();
        assert_eq!(visits, [c, b, a, d]);
        assert_eq!(all(&queries), [d, a, b, c]);
        assert_eq!(queries.sphere([128.0; 3], 8.0, 0x10, 1, |entity| entity == b).unwrap(), [b]);
        let mut restored = queries.clone();
        queries.mark_dirty(b).unwrap();
        queries.flush(0x10, |_| false, |_, _| Ok(point())).unwrap();
        assert_eq!(all(&queries), all(&restored));
        restored.destroy(a).unwrap();
        assert_eq!(all(&queries), [d, a, b, c]);
        assert_eq!(all(&restored), [d, b, c]);
    }

    #[test]
    fn bone_setup_deferral_requeues_after_all_detached_batches_and_skips_deleted_generations() {
        let mut queries = Queries::new(32, 128).unwrap();
        let a = Entity::Actor(1);
        let b = Entity::Projectile(1);
        let old = Entity::Map(EntityHandle { slot: 4, generation: 1 });
        let new = Entity::Map(EntityHandle { slot: 4, generation: 2 });
        for entity in [a, b, old] { register(&mut queries, entity); }
        queries.destroy(old).unwrap();
        register(&mut queries, new);
        let mut visits = Vec::new();
        queries.flush(0x10, |entity| entity == b || entity == new, |_, entity| { visits.push(entity); Ok(point()) }).unwrap();
        assert_eq!(visits, [a]);
        assert_eq!(all(&queries), [a]);
        visits.clear();
        queries.flush(0x10, |_| false, |_, entity| { visits.push(entity); Ok(point()) }).unwrap();
        assert_eq!(visits, [b, new]);
        assert_eq!(all(&queries), [new, b, a]);
    }

    #[test]
    fn bounds_computation_can_redirty_the_same_entity_without_losing_the_next_batch() {
        let mut queries = Queries::new(32, 128).unwrap();
        let entity = Entity::Projectile(1);
        register(&mut queries, entity);
        let mut visits = 0;
        queries.flush(0x10, |_| false, |queries, entity| {
            visits += 1;
            if visits == 1 { queries.mark_dirty(entity)?; }
            Ok(point())
        }).unwrap();
        assert_eq!(visits, 2);
        assert!(queries.dirty.is_empty());
        assert_eq!(all(&queries), [entity]);
    }

    #[test]
    fn game_bounds_expand_one_unit_but_zero_radius_uses_the_origin() {
        let mut queries = Queries::new(32, 128).unwrap();
        let point_entity = Entity::Actor(1);
        let box_entity = Entity::Projectile(1);
        for entity in [point_entity, box_entity] { register(&mut queries, entity); }
        queries.flush(0x10, |_| false, |_, entity| Ok(Bounds { origin: [0.0; 3],
            surrounding: (entity == box_entity).then_some(Hull { mins: [-2.0; 3], maxs: [2.0; 3] }) })).unwrap();
        assert_eq!(queries.sphere([3.02, 0.0, 0.0], 0.0, 0x10, 32, |_| true).unwrap(), [box_entity]);
        assert!(queries.sphere([3.04, 0.0, 0.0], 0.0, 0x10, 32, |_| true).unwrap().is_empty());
        assert!(!queries.sphere([0.04, 0.0, 0.0], 0.0, 0x10, 32, |_| true).unwrap().contains(&point_entity));
    }

    #[test]
    fn listing_an_unplaced_handle_does_not_invent_bounds_or_refresh_while_unlisted() {
        let mut queries = Queries::new(32, 128).unwrap();
        let entity = Entity::Projectile(1);
        queries.register(entity).unwrap();
        queries.mark_dirty(entity).unwrap();
        queries.flush(0x10, |_| false, |_, _| panic!("unlisted bounds")).unwrap();
        assert!(queries.entries[&entity].handle.is_some());
        queries.set_lists(entity, 0x11).unwrap();
        assert!(all(&queries).is_empty());
        queries.mark_dirty(entity).unwrap();
        queries.flush(0x10, |_| false, |_, _| Ok(point())).unwrap();
        assert_eq!(all(&queries), [entity]);
        queries.set_lists(entity, 0).unwrap();
        queries.mark_dirty(entity).unwrap();
        queries.flush(0x10, |_| false, |_, _| panic!("unlisted bounds")).unwrap();
        assert!(all(&queries).is_empty());
        queries.set_lists(entity, 0x11).unwrap();
        assert_eq!(all(&queries), [entity]);
    }
}
