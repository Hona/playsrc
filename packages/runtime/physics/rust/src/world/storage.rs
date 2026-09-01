use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct CoreStorage {
    pub(super) next: u64,
    free: Vec<u64>,
}
impl Default for CoreStorage {
    fn default() -> Self {
        Self {
            next: 1,
            free: Vec::new(),
        }
    }
}
impl CoreStorage {
    pub(super) fn acquire(&mut self) -> Result<u64, EnvironmentError> {
        if let Some(slot) = self.free.pop() {
            return Ok(slot);
        }
        let slot = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(EnvironmentError::ClockOverflow)?;
        Ok(slot)
    }
    pub(super) fn release(&mut self, slot: u64) -> Result<(), EnvironmentError> {
        if slot == 0 || slot >= self.next || self.free.contains(&slot) {
            return Err(EnvironmentError::SnapshotMismatch);
        }
        self.free.push(slot);
        Ok(())
    }
    pub(super) fn valid(&self, bodies: &[RigidBody], maximum: usize) -> bool {
        if self.next == 0
            || self.next - 1 > maximum as u64
            || self
                .free
                .len()
                .checked_add(bodies.len())
                .is_none_or(|count| count as u64 != self.next - 1)
        {
            return false;
        }
        let mut slots = std::collections::BTreeSet::new();
        self.free
            .iter()
            .copied()
            .chain(bodies.iter().map(|body| body.storage_identity))
            .all(|slot| slot > 0 && slot < self.next && slots.insert(slot))
    }
}
pub(super) fn pair_keys(
    bodies: &[RigidBody],
    cores: [u64; 2],
) -> Result<[u64; 2], EnvironmentError> {
    let mut keys = [0; 2];
    for side in 0..2 {
        keys[side] = bodies
            .iter()
            .find(|body| body.core_identity == cores[side])
            .ok_or(EnvironmentError::MissingBody)?
            .storage_identity;
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn released_storage_is_lifo_while_core_lifetimes_remain_distinct() {
        let (mut world, _) = super::super::tests::automatic_pair_world(false);
        let archive = world.serialize_body(1).unwrap();
        let old_core = world.body(1).unwrap().core_identity;
        let old_slot = world.body(1).unwrap().storage_identity;
        world.destroy_body(1).unwrap();
        world.unserialize_body(3, &archive, true).unwrap();
        assert_ne!(world.body(3).unwrap().core_identity, old_core);
        assert_eq!(world.body(3).unwrap().storage_identity, old_slot);
        let snapshot = world.snapshot();
        world.restore(snapshot.clone()).unwrap();
        let mut duplicate = snapshot.clone();
        duplicate.bodies[0].storage_identity = duplicate.bodies[1].storage_identity;
        assert_eq!(
            world.restore(duplicate),
            Err(EnvironmentError::SnapshotMismatch)
        );
        assert_eq!(world.snapshot(), snapshot);
        let mut slots = CoreStorage::default();
        assert_eq!(slots.acquire().unwrap(), 1);
        assert_eq!(slots.acquire().unwrap(), 2);
        slots.release(1).unwrap();
        slots.release(2).unwrap();
        assert!(slots.valid(&[], 2));
        let before = slots.clone();
        assert_eq!(slots.release(1), Err(EnvironmentError::SnapshotMismatch));
        assert_eq!(slots, before);
        assert_eq!(slots.acquire().unwrap(), 2);
        assert_eq!(slots.acquire().unwrap(), 1);
        assert_eq!(slots.acquire().unwrap(), 3);
    }
}
