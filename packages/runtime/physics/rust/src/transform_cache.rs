use crate::{CoreOrientation, CoreTransformState, ProjectionKnot, TopologyError};
use std::{collections::BTreeMap, fmt};

pub const TRANSFORM_CACHE_CAPACITY: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CachedTransform {
    pub core_position: [f64; 3],
    pub orientation: CoreOrientation,
    pub object: ProjectionKnot,
}

impl CoreTransformState {
    pub fn sample(self) -> Result<CachedTransform, TopologyError> {
        let (orientation, core) = ProjectionKnot::sample_core(
            self.position,
            self.prior_orientation,
            self.next_orientation,
            self.projection_velocity,
            self.core_time,
            self.environment_time,
            self.inverse_step,
        )?;
        Ok(CachedTransform {
            core_position: core.position,
            orientation,
            object: self.object_frame.object_pose(core)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheActivity {
    Simulated,
    Inactive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformCacheError {
    Exhausted,
    MissingOwner,
    Pinned,
    NotPinned,
    Uninitialized,
    Projection(TopologyError),
}

impl fmt::Display for TransformCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => f.write_str("all shared transform slots are pinned"),
            Self::MissingOwner => f.write_str("body has no shared transform slot"),
            Self::Pinned => f.write_str("shared transform slot is pinned"),
            Self::NotPinned => f.write_str("shared transform slot is not pinned"),
            Self::Uninitialized => {
                f.write_str("time code does not admit initial transform projection")
            }
            Self::Projection(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TransformCacheError {}
impl From<TopologyError> for TransformCacheError {
    fn from(value: TopologyError) -> Self {
        Self::Projection(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Entry {
    owner: Option<u64>,
    pinned: bool,
    time_code: u32,
    transform: Option<CachedTransform>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransformCache {
    entries: Box<[Entry]>,
    owners: BTreeMap<u64, u8>,
    cursor: u8,
}

impl Default for TransformCache {
    fn default() -> Self {
        Self {
            entries: vec![Entry::default(); TRANSFORM_CACHE_CAPACITY].into_boxed_slice(),
            owners: BTreeMap::new(),
            cursor: 0,
        }
    }
}

impl TransformCache {
    pub fn cursor(&self) -> u8 {
        self.cursor
    }
    pub fn slot(&self, owner: u64) -> Option<u8> {
        self.owners.get(&owner).copied()
    }
    pub fn owners(&self) -> impl Iterator<Item = u64> + '_ {
        self.owners.keys().copied()
    }
    pub fn current(&self, owner: u64) -> Option<(u32, CachedTransform)> {
        let slot = self.slot(owner)?;
        let entry = self.entries[usize::from(slot)];
        Some((entry.time_code, entry.transform?))
    }

    pub fn resolve(
        &mut self,
        owner: u64,
        activity: CacheActivity,
        time_code: u32,
        state: CoreTransformState,
    ) -> Result<CachedTransform, TransformCacheError> {
        let existing = self.slot(owner);
        let slot = match existing {
            Some(slot) => slot,
            None => (0..TRANSFORM_CACHE_CAPACITY)
                .map(|offset| self.cursor.wrapping_add(offset as u8))
                .find(|&slot| !self.entries[usize::from(slot)].pinned)
                .ok_or(TransformCacheError::Exhausted)?,
        };
        let mut entry = self.entries[usize::from(slot)];
        if existing.is_none() {
            entry.time_code = 0;
        }
        let refresh = match activity {
            CacheActivity::Inactive => existing.is_none(),
            CacheActivity::Simulated => (time_code as i32) > (entry.time_code as i32),
        };
        if refresh {
            entry.transform = Some(state.sample()?);
            entry.time_code = time_code;
        }
        let transform = entry.transform.ok_or(TransformCacheError::Uninitialized)?;
        if existing.is_none() {
            if let Some(evicted) = entry.owner {
                self.owners.remove(&evicted);
            }
            self.owners.insert(owner, slot);
            self.cursor = slot.wrapping_add(1);
        }
        entry.owner = Some(owner);
        self.entries[usize::from(slot)] = entry;
        Ok(transform)
    }

    pub fn pin(&mut self, owner: u64) -> Result<(), TransformCacheError> {
        let slot = self.slot(owner).ok_or(TransformCacheError::MissingOwner)?;
        let entry = &mut self.entries[usize::from(slot)];
        if entry.pinned {
            return Err(TransformCacheError::Pinned);
        }
        entry.pinned = true;
        Ok(())
    }

    pub fn release(&mut self, owner: u64) -> Result<(), TransformCacheError> {
        let slot = self.slot(owner).ok_or(TransformCacheError::MissingOwner)?;
        let entry = &mut self.entries[usize::from(slot)];
        if !entry.pinned {
            return Err(TransformCacheError::NotPinned);
        }
        entry.pinned = false;
        Ok(())
    }

    pub fn invalidate(&mut self, owner: u64) -> Result<(), TransformCacheError> {
        let Some(slot) = self.slot(owner) else {
            return Ok(());
        };
        let entry = &mut self.entries[usize::from(slot)];
        if entry.pinned {
            return Err(TransformCacheError::Pinned);
        }
        entry.owner = None;
        self.owners.remove(&owner);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectFrame;

    fn state() -> CoreTransformState {
        let orientation = CoreOrientation {
            quaternion: [0.0, 0.0, 0.0, 1.0],
        };
        CoreTransformState {
            object_frame: ObjectFrame::identity(),
            position: [-0.0, 2.0, 3.0],
            prior_orientation: orientation,
            next_orientation: orientation,
            projection_velocity: [1.0, 0.0, 0.0],
            core_time: 0.0,
            environment_time: 0.0,
            inverse_step: 66.0,
        }
    }

    #[test]
    fn full_pinned_pool_fails_without_mutation_and_releases_its_exact_slot() {
        let mut cache = TransformCache::default();
        for owner in 0..TRANSFORM_CACHE_CAPACITY as u64 {
            cache
                .resolve(owner, CacheActivity::Inactive, 1, state())
                .unwrap();
            cache.pin(owner).unwrap();
        }
        let before = cache.clone();
        assert_eq!(
            cache.resolve(256, CacheActivity::Simulated, 2, state()),
            Err(TransformCacheError::Exhausted)
        );
        assert_eq!(cache.invalidate(17), Err(TransformCacheError::Pinned));
        assert_eq!(cache.pin(17), Err(TransformCacheError::Pinned));
        assert_eq!(cache, before);
        cache.release(17).unwrap();
        assert_eq!(cache.release(17), Err(TransformCacheError::NotPinned));
        cache
            .resolve(256, CacheActivity::Inactive, 1, state())
            .unwrap();
        assert_eq!(cache.slot(256), Some(17));
        assert_eq!(cache.slot(17), None);
        assert_eq!(cache.cursor(), 18);
    }

    #[test]
    fn invalid_replacement_keeps_eviction_state_and_cache_payload_atomic() {
        let mut cache = TransformCache::default();
        for owner in 0..256 {
            cache
                .resolve(owner, CacheActivity::Inactive, 1, state())
                .unwrap();
        }
        let before = cache.clone();
        assert!(
            cache
                .resolve(
                    256,
                    CacheActivity::Simulated,
                    2,
                    CoreTransformState {
                        inverse_step: 0.0,
                        ..state()
                    }
                )
                .is_err()
        );
        assert_eq!(cache, before);
        cache.invalidate(0).unwrap();
        assert_eq!(cache.cursor(), 0);
        let reused = cache
            .resolve(256, CacheActivity::Simulated, 0, state())
            .unwrap();
        assert_eq!(reused.object.position[0].to_bits(), (-0.0_f64).to_bits());
        assert_eq!(cache.current(256).unwrap().0, 0);
        let mut fresh = TransformCache::default();
        assert_eq!(
            fresh.resolve(0, CacheActivity::Simulated, 0, state()),
            Err(TransformCacheError::Uninitialized)
        );
        assert_eq!(fresh, TransformCache::default());
    }

    #[test]
    fn inactive_and_nonadvancing_signed_codes_retain_the_original_transform() {
        let mut cache = TransformCache::default();
        let first = cache
            .resolve(0, CacheActivity::Simulated, 7, state())
            .unwrap();
        let changed = CoreTransformState {
            environment_time: 0.01,
            ..state()
        };
        for code in [7, 6, 0, 0xffff_ffff, 0x8000_0000] {
            assert_eq!(
                cache
                    .resolve(0, CacheActivity::Simulated, code, changed)
                    .unwrap(),
                first
            );
        }
        assert_eq!(
            cache
                .resolve(0, CacheActivity::Inactive, 8, changed)
                .unwrap(),
            first
        );
        assert_ne!(
            cache
                .resolve(0, CacheActivity::Simulated, 8, changed)
                .unwrap(),
            first
        );
        assert_eq!(cache.cursor(), 1);
    }
}
