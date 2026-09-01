use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnerId(usize);

impl OwnerId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipError {
    CapacityExceeded,
    InvalidOwner,
    OwnerReleased,
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded => {
                formatter.write_str("retained contact-owner capacity exceeded")
            }
            Self::InvalidOwner => formatter.write_str("retained contact-owner identity is invalid"),
            Self::OwnerReleased => {
                formatter.write_str("retained contact owner was already released")
            }
        }
    }
}

impl std::error::Error for OwnershipError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerSlots {
    capacity: usize,
    next: usize,
    released: Vec<usize>,
    active: Vec<bool>,
}

impl OwnerSlots {
    pub const fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next: 0,
            released: Vec::new(),
            active: Vec::new(),
        }
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn active_count(&self) -> usize {
        self.next - self.released.len()
    }

    pub fn allocate(&mut self) -> Result<OwnerId, OwnershipError> {
        if let Some(index) = self.released.pop() {
            self.active[index] = true;
            return Ok(OwnerId(index));
        }
        if self.next >= self.capacity {
            return Err(OwnershipError::CapacityExceeded);
        }
        let index = self.next;
        self.active.push(true);
        self.next += 1;
        Ok(OwnerId(index))
    }

    pub fn release(&mut self, owner: OwnerId) -> Result<(), OwnershipError> {
        let Some(active) = self.active.get_mut(owner.0) else {
            return Err(OwnershipError::InvalidOwner);
        };
        if !*active {
            return Err(OwnershipError::OwnerReleased);
        }
        *active = false;
        self.released.push(owner.0);
        Ok(())
    }

    pub fn owner(&self, index: usize) -> Result<OwnerId, OwnershipError> {
        match self.active.get(index) {
            Some(true) => Ok(OwnerId(index)),
            Some(false) => Err(OwnershipError::OwnerReleased),
            None => Err(OwnershipError::InvalidOwner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnerSlots, OwnershipError};

    #[test]
    fn released_owner_slots_are_reused_in_reverse_release_order() {
        let mut slots = OwnerSlots::new(4);
        let first = slots.allocate().unwrap();
        let second = slots.allocate().unwrap();
        let third = slots.allocate().unwrap();
        slots.release(first).unwrap();
        slots.release(second).unwrap();
        assert_eq!(slots.allocate().unwrap(), second);
        assert_eq!(slots.allocate().unwrap(), first);
        assert_eq!(slots.allocate().unwrap().index(), 3);
        assert_eq!(slots.active_count(), 4);
        assert_eq!(slots.allocate(), Err(OwnershipError::CapacityExceeded));
        assert_eq!(slots.owner(third.index()).unwrap(), third);
    }

    #[test]
    fn released_owners_and_exhausted_zero_capacity_are_rejected() {
        let mut slots = OwnerSlots::new(1);
        let owner = slots.allocate().unwrap();
        slots.release(owner).unwrap();
        assert_eq!(slots.release(owner), Err(OwnershipError::OwnerReleased));
        assert_eq!(
            slots.owner(owner.index()),
            Err(OwnershipError::OwnerReleased)
        );
        assert_eq!(slots.owner(1), Err(OwnershipError::InvalidOwner));
        assert_eq!(
            OwnerSlots::new(0).allocate(),
            Err(OwnershipError::CapacityExceeded)
        );
    }
}
