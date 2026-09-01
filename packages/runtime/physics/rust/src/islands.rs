use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreMovement {
    Moving,
    Slow,
    Calm,
    Dormant,
    Discovering,
}
impl CoreMovement {
    pub fn is_simulated(self) -> bool {
        matches!(self, Self::Moving | Self::Slow | Self::Calm)
    }
    pub fn can_collide(self) -> bool {
        self != Self::Dormant
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IslandController {
    pub identity: u64,
    pub priority: i32,
    /// Empty for independent force controllers. Order matters for dependent controllers.
    pub associated: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerRoster {
    pub controller: u64,
    pub cores: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationIsland {
    pub identity: u64,
    pub cores: Vec<u64>,
    pub controllers: Vec<ControllerRoster>,
    pub active: bool,
    pub connectivity_dirty: bool,
    pub activity: crate::MotionActivity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IslandCore {
    island: u64,
    controllers: Vec<u64>,
    movement: CoreMovement,
    immovable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationIslands {
    cores: BTreeMap<u64, IslandCore>,
    controllers: BTreeMap<u64, IslandController>,
    islands: BTreeMap<u64, SimulationIsland>,
    active: Vec<u64>,
    dormant: Vec<u64>,
    next_island: u64,
    maximum_cores: usize,
    maximum_controllers: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IslandError {
    InvalidLimit,
    Capacity,
    Identity,
    MissingCore,
    MissingController,
    MissingIsland,
    MissingBinding,
    ControllerInUse,
    InvalidAssociation,
    InvalidState,
}
impl fmt::Display for IslandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimit => "simulation island limits must be positive",
            Self::Capacity => "simulation island capacity exhausted",
            Self::Identity => "simulation island identity is already present or exhausted",
            Self::MissingCore => "simulation island core is absent",
            Self::MissingController => "simulation island controller is absent",
            Self::MissingIsland => "simulation island is absent",
            Self::MissingBinding => "simulation island controller binding is absent",
            Self::ControllerInUse => "simulation controller still has core bindings",
            Self::InvalidAssociation => {
                "dependent controller crosses inconsistent island membership"
            }
            Self::InvalidState => "simulation island state is inconsistent",
        })
    }
}
impl std::error::Error for IslandError {}

impl SimulationIslands {
    pub fn new(maximum_cores: usize, maximum_controllers: usize) -> Result<Self, IslandError> {
        if maximum_cores == 0 || maximum_controllers == 0 {
            return Err(IslandError::InvalidLimit);
        }
        Ok(Self {
            cores: BTreeMap::new(),
            controllers: BTreeMap::new(),
            islands: BTreeMap::new(),
            active: Vec::new(),
            dormant: Vec::new(),
            next_island: 1,
            maximum_cores,
            maximum_controllers,
        })
    }
    pub fn active(&self) -> &[u64] {
        &self.active
    }
    pub fn phase_order(&self) -> impl Iterator<Item = u64> + '_ {
        let (prefix, tail) = self.active.split_at(self.active.len().saturating_sub(3));
        prefix.iter().chain(tail.iter().rev()).copied()
    }
    pub fn dormant(&self) -> &[u64] {
        &self.dormant
    }
    pub fn island(&self, id: u64) -> Option<&SimulationIsland> {
        self.islands.get(&id)
    }
    pub fn island_of(&self, core: u64) -> Option<u64> {
        self.cores.get(&core).map(|c| c.island)
    }
    pub fn core_controllers(&self, core: u64) -> Option<&[u64]> {
        self.cores.get(&core).map(|c| c.controllers.as_slice())
    }
    pub fn movement(&self, core: u64) -> Option<CoreMovement> {
        self.cores.get(&core).map(|c| c.movement)
    }
    pub fn controller(&self, id: u64) -> Option<&IslandController> {
        self.controllers.get(&id)
    }
    pub fn core_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.cores.keys().copied()
    }
    pub fn is_immovable(&self, core: u64) -> Option<bool> {
        self.cores.get(&core).map(|value| value.immovable)
    }
    pub fn set_movement(&mut self, core: u64, movement: CoreMovement) -> Result<(), IslandError> {
        self.cores
            .get_mut(&core)
            .ok_or(IslandError::MissingCore)?
            .movement = movement;
        Ok(())
    }
    pub fn activity_mut(&mut self, id: u64) -> Result<&mut crate::MotionActivity, IslandError> {
        Ok(&mut self
            .islands
            .get_mut(&id)
            .ok_or(IslandError::MissingIsland)?
            .activity)
    }

    pub fn register_core(&mut self, core: u64, immovable: bool) -> Result<u64, IslandError> {
        if self.cores.contains_key(&core) {
            return Err(IslandError::Identity);
        }
        if self.cores.len() == self.maximum_cores {
            return Err(IslandError::Capacity);
        }
        let id = self.allocate_island(false)?;
        self.islands.get_mut(&id).unwrap().cores.push(core);
        self.cores.insert(
            core,
            IslandCore {
                island: id,
                controllers: Vec::new(),
                movement: CoreMovement::Dormant,
                immovable,
            },
        );
        Ok(id)
    }
    pub fn register_controller(&mut self, controller: IslandController) -> Result<(), IslandError> {
        if self.controllers.contains_key(&controller.identity) {
            return Err(IslandError::Identity);
        }
        if self.controllers.len() == self.maximum_controllers {
            return Err(IslandError::Capacity);
        }
        if controller
            .associated
            .iter()
            .any(|id| !self.cores.contains_key(id))
        {
            return Err(IslandError::MissingCore);
        }
        self.controllers.insert(controller.identity, controller);
        Ok(())
    }
    pub fn remove_controller(&mut self, controller: u64) -> Result<(), IslandError> {
        if !self.controllers.contains_key(&controller) {
            return Err(IslandError::MissingController);
        }
        if self
            .cores
            .values()
            .any(|c| c.controllers.contains(&controller))
        {
            return Err(IslandError::ControllerInUse);
        }
        self.controllers.remove(&controller);
        Ok(())
    }
    pub fn attach(&mut self, core: u64, controller: u64) -> Result<(), IslandError> {
        let island = self.island_of(core).ok_or(IslandError::MissingCore)?;
        if !self.controllers.contains_key(&controller) {
            return Err(IslandError::MissingController);
        }
        let c = &self.cores[&core];
        let unit = &self.islands[&island];
        if c.controllers.len() >= u16::MAX as usize
            || unit
                .controllers
                .iter()
                .find(|r| r.controller == controller)
                .is_some_and(|r| r.cores.len() >= u16::MAX as usize)
            || (unit.controllers.len() >= u16::MAX as usize
                && !unit.controllers.iter().any(|r| r.controller == controller))
        {
            return Err(IslandError::Capacity);
        }
        self.cores
            .get_mut(&core)
            .unwrap()
            .controllers
            .push(controller);
        let roster = &mut self.islands.get_mut(&island).unwrap().controllers;
        if let Some(entry) = roster.iter_mut().rev().find(|r| r.controller == controller) {
            entry.cores.push(core);
        } else {
            roster.push(ControllerRoster {
                controller,
                cores: vec![core],
            });
        }
        self.sort_roster(island);
        Ok(())
    }
    pub fn detach(&mut self, core: u64, controller: u64) -> Result<(), IslandError> {
        let c = self.cores.get(&core).ok_or(IslandError::MissingCore)?;
        let index = c
            .controllers
            .iter()
            .rposition(|id| *id == controller)
            .ok_or(IslandError::MissingBinding)?;
        let island = c.island;
        let roster = &self.islands[&island].controllers;
        let ri = roster
            .iter()
            .rposition(|r| r.controller == controller)
            .ok_or(IslandError::InvalidState)?;
        let ci = roster[ri]
            .cores
            .iter()
            .rposition(|id| *id == core)
            .ok_or(IslandError::InvalidState)?;
        self.cores.get_mut(&core).unwrap().controllers.remove(index);
        let roster = &mut self.islands.get_mut(&island).unwrap().controllers;
        roster[ri].cores.remove(ci);
        if roster[ri].cores.is_empty() {
            roster.remove(ri);
        }
        Ok(())
    }

    /// Returns the island requiring synchronous core revival, if any.
    pub fn announce(&mut self, controller: u64) -> Result<Option<u64>, IslandError> {
        let mut next = self.clone();
        let result = next.announce_inner(controller)?;
        next.validate()?;
        *self = next;
        Ok(result)
    }
    fn announce_inner(&mut self, controller: u64) -> Result<Option<u64>, IslandError> {
        let associated = self
            .controllers
            .get(&controller)
            .ok_or(IslandError::MissingController)?
            .associated
            .clone();
        let mut reference = None;
        let mut simulated = false;
        let mut merged = false;
        for core in associated.into_iter().rev() {
            let current = self.cores.get(&core).ok_or(IslandError::MissingCore)?;
            if current.immovable {
                continue;
            }
            simulated |= current.movement.is_simulated();
            let unit = current.island;
            if let Some(reference) = reference {
                if reference != unit {
                    self.transfer(reference, unit)?;
                    merged = true;
                }
            } else {
                reference = Some(unit);
            }
            self.attach(core, controller)?;
        }
        if let Some(unit) = reference {
            if merged {
                self.rebuild(unit)?;
            }
            if simulated {
                return Ok(Some(unit));
            }
        }
        Ok(None)
    }
    /// A non-silent caller synchronously revives the returned island after removal.
    pub fn withdraw(&mut self, controller: u64, silent: bool) -> Result<Option<u64>, IslandError> {
        let mut next = self.clone();
        let associated = next
            .controllers
            .get(&controller)
            .ok_or(IslandError::MissingController)?
            .associated
            .clone();
        let mut unit = None;
        for core in associated.into_iter().rev() {
            next.detach(core, controller)?;
            unit = next.island_of(core);
        }
        if let Some(id) = unit {
            next.islands.get_mut(&id).unwrap().connectivity_dirty = true;
        }
        next.validate()?;
        *self = next;
        Ok(if silent { None } else { unit })
    }
    /// Revival callbacks run before moving a dormant island onto the active list.
    pub fn activate(&mut self, island: u64) -> Result<(), IslandError> {
        let unit = self
            .islands
            .get_mut(&island)
            .ok_or(IslandError::MissingIsland)?;
        if unit.active {
            return Ok(());
        }
        unit.active = true;
        remove_id(&mut self.dormant, island)?;
        self.active.insert(0, island);
        Ok(())
    }
    pub fn freeze(&mut self, island: u64) -> Result<Vec<u64>, IslandError> {
        let unit = self
            .islands
            .get_mut(&island)
            .ok_or(IslandError::MissingIsland)?;
        let cores = unit.cores.iter().rev().copied().collect::<Vec<_>>();
        if unit.active {
            unit.active = false;
            remove_id(&mut self.active, island)?;
            self.dormant.insert(0, island);
        }
        for core in &cores {
            self.cores.get_mut(core).unwrap().movement = CoreMovement::Dormant;
        }
        Ok(cores)
    }
    pub fn remove_core(&mut self, core: u64) -> Result<(), IslandError> {
        let c = self.cores.get(&core).ok_or(IslandError::MissingCore)?;
        if self
            .controllers
            .values()
            .any(|v| v.associated.contains(&core))
        {
            return Err(IslandError::ControllerInUse);
        }
        let island = c.island;
        let controllers = c.controllers.clone();
        for controller in controllers.into_iter().rev() {
            self.detach(core, controller)?;
        }
        self.cores.remove(&core);
        let unit = self.islands.get_mut(&island).unwrap();
        remove_id(&mut unit.cores, core)?;
        if unit.cores.is_empty() {
            self.unlink(island)?;
            self.islands.remove(&island);
        }
        Ok(())
    }
    pub fn rebuild(&mut self, island: u64) -> Result<(), IslandError> {
        let cores = &self
            .islands
            .get(&island)
            .ok_or(IslandError::MissingIsland)?
            .cores;
        let mut roster: Vec<ControllerRoster> = Vec::new();
        let mut indices = BTreeMap::new();
        for core in cores.iter().rev() {
            for controller in self
                .cores
                .get(core)
                .ok_or(IslandError::MissingCore)?
                .controllers
                .iter()
                .rev()
            {
                if !self.controllers.contains_key(controller) {
                    return Err(IslandError::MissingController);
                }
                let index = *indices.entry(*controller).or_insert_with(|| {
                    let index = roster.len();
                    roster.push(ControllerRoster {
                        controller: *controller,
                        cores: Vec::new(),
                    });
                    index
                });
                roster[index].cores.push(*core);
                if roster[index].cores.len() > u16::MAX as usize {
                    return Err(IslandError::Capacity);
                }
            }
        }
        if roster.len() > u16::MAX as usize {
            return Err(IslandError::Capacity);
        }
        roster.sort_by_key(|r| self.controllers[&r.controller].priority);
        self.islands.get_mut(&island).unwrap().controllers = roster;
        Ok(())
    }
    pub(crate) fn join(&mut self, recipient: u64, donor: u64) -> Result<(), IslandError> {
        if recipient == donor {
            return Ok(());
        }
        let mut candidate = self.clone();
        candidate.transfer(recipient, donor)?;
        candidate.rebuild(recipient)?;
        *self = candidate;
        Ok(())
    }
    pub(crate) fn set_associated(
        &mut self,
        controller: u64,
        cores: Vec<u64>,
    ) -> Result<(), IslandError> {
        if cores
            .iter()
            .enumerate()
            .any(|(i, core)| cores[..i].contains(core) || !self.cores.contains_key(core))
        {
            return Err(IslandError::InvalidAssociation);
        }
        self.controllers
            .get_mut(&controller)
            .ok_or(IslandError::MissingController)?
            .associated = cores;
        Ok(())
    }
    pub fn resolve_connectivity(&mut self, island: u64) -> Result<Vec<u64>, IslandError> {
        let mut next = self.clone();
        next.rebuild(island)?;
        let created = next.split_inner(island)?;
        next.islands
            .get_mut(&island)
            .ok_or(IslandError::MissingIsland)?
            .connectivity_dirty = false;
        next.validate()?;
        *self = next;
        Ok(created)
    }
    pub(crate) fn request_connectivity_check(&mut self, core: u64) -> Result<(), IslandError> {
        if self.is_immovable(core).ok_or(IslandError::MissingCore)? {
            return Ok(());
        }
        let island = self.island_of(core).ok_or(IslandError::MissingCore)?;
        self.islands
            .get_mut(&island)
            .ok_or(IslandError::MissingIsland)?
            .connectivity_dirty = true;
        Ok(())
    }
    fn split_inner(&mut self, island: u64) -> Result<Vec<u64>, IslandError> {
        let unit = self
            .islands
            .get(&island)
            .ok_or(IslandError::MissingIsland)?;
        let mut parents = unit
            .cores
            .iter()
            .map(|c| (*c, *c))
            .collect::<BTreeMap<_, _>>();
        for binding in unit.controllers.iter().rev() {
            let related = &self.controllers[&binding.controller].associated;
            if related.iter().any(|c| !parents.contains_key(c)) {
                return Err(IslandError::InvalidAssociation);
            }
            if let Some(first) = related.first() {
                let root = find_root(&mut parents, *first);
                for core in related.iter().rev() {
                    let other = find_root(&mut parents, *core);
                    if root != other {
                        parents.insert(other, root);
                    }
                }
            }
        }
        let Some(first) = unit.cores.first().copied() else {
            return Ok(Vec::new());
        };
        let original = find_root(&mut parents, first);
        let mut selected = unit
            .cores
            .iter()
            .copied()
            .map(|c| find_root(&mut parents, c))
            .find(|root| *root != original);
        let mut created = Vec::new();
        while let Some(root) = selected {
            let id = self.allocate_island(true)?;
            created.push(id);
            let mut moved = Vec::new();
            let mut retained = Vec::new();
            let mut first_remaining = None;
            let mut multiple = false;
            for core in &self.islands[&island].cores {
                let current = find_root(&mut parents, *core);
                if current == root {
                    moved.push(*core);
                } else {
                    retained.push(*core);
                    if let Some(first) = first_remaining {
                        multiple |= first != current;
                    } else {
                        first_remaining = Some(current);
                    }
                }
            }
            if moved.is_empty() || retained.is_empty() {
                return Err(IslandError::InvalidAssociation);
            }
            self.islands.get_mut(&island).unwrap().cores = retained;
            for core in &moved {
                self.cores.get_mut(core).unwrap().island = id;
            }
            self.islands.get_mut(&id).unwrap().cores = moved;
            self.rebuild(id)?;
            selected = if multiple { first_remaining } else { None };
        }
        if !created.is_empty() {
            self.rebuild(island)?;
        }
        Ok(created)
    }
    fn transfer(&mut self, recipient: u64, donor: u64) -> Result<(), IslandError> {
        let moved = self
            .islands
            .get(&donor)
            .ok_or(IslandError::MissingIsland)?
            .cores
            .clone();
        let target = self
            .islands
            .get_mut(&recipient)
            .ok_or(IslandError::MissingIsland)?;
        if target.cores.len() + moved.len() > u16::MAX as usize {
            return Err(IslandError::Capacity);
        }
        target.cores.extend_from_slice(&moved);
        for core in moved {
            self.cores.get_mut(&core).unwrap().island = recipient;
        }
        self.unlink(donor)?;
        self.islands.remove(&donor);
        Ok(())
    }
    fn sort_roster(&mut self, island: u64) {
        self.islands
            .get_mut(&island)
            .unwrap()
            .controllers
            .sort_by_key(|r| self.controllers[&r.controller].priority);
    }
    fn allocate_island(&mut self, active: bool) -> Result<u64, IslandError> {
        let id = self.next_island;
        self.next_island = id.checked_add(1).ok_or(IslandError::Identity)?;
        self.islands.insert(
            id,
            SimulationIsland {
                identity: id,
                cores: Vec::new(),
                controllers: Vec::new(),
                active,
                connectivity_dirty: false,
                activity: crate::MotionActivity::default(),
            },
        );
        if active {
            self.active.insert(0, id);
        } else {
            self.dormant.insert(0, id);
        }
        Ok(id)
    }
    fn unlink(&mut self, id: u64) -> Result<(), IslandError> {
        if self
            .islands
            .get(&id)
            .ok_or(IslandError::MissingIsland)?
            .active
        {
            remove_id(&mut self.active, id)
        } else {
            remove_id(&mut self.dormant, id)
        }
    }
    pub fn validate(&self) -> Result<(), IslandError> {
        if self.cores.len() > self.maximum_cores
            || self.controllers.len() > self.maximum_controllers
        {
            return Err(IslandError::Capacity);
        }
        let mut listed = BTreeSet::new();
        for (active, ids) in [(true, &self.active), (false, &self.dormant)] {
            for id in ids {
                if !listed.insert(*id) || self.islands.get(id).is_none_or(|u| u.active != active) {
                    return Err(IslandError::InvalidState);
                }
            }
        }
        if listed.len() != self.islands.len() {
            return Err(IslandError::InvalidState);
        }
        let mut core_ids = BTreeSet::new();
        for (id, unit) in &self.islands {
            if unit.identity != *id || unit.cores.is_empty() || *id >= self.next_island {
                return Err(IslandError::InvalidState);
            }
            let mut expected = BTreeMap::<(u64, u64), usize>::new();
            for core in &unit.cores {
                if !core_ids.insert(*core) {
                    return Err(IslandError::InvalidState);
                }
                let c = self.cores.get(core).ok_or(IslandError::MissingCore)?;
                if c.island != *id {
                    return Err(IslandError::InvalidState);
                }
                for controller in &c.controllers {
                    if !self.controllers.contains_key(controller) {
                        return Err(IslandError::MissingController);
                    }
                    *expected.entry((*controller, *core)).or_default() += 1;
                }
            }
            let mut actual = BTreeMap::<(u64, u64), usize>::new();
            let mut controls = BTreeSet::new();
            let mut previous = None;
            for entry in &unit.controllers {
                let priority = self
                    .controllers
                    .get(&entry.controller)
                    .ok_or(IslandError::MissingController)?
                    .priority;
                if entry.cores.is_empty()
                    || !controls.insert(entry.controller)
                    || previous.is_some_and(|p| p > priority)
                {
                    return Err(IslandError::InvalidState);
                }
                previous = Some(priority);
                for core in &entry.cores {
                    *actual.entry((entry.controller, *core)).or_default() += 1;
                }
            }
            if actual != expected {
                return Err(IslandError::InvalidState);
            }
        }
        if core_ids.len() != self.cores.len() {
            return Err(IslandError::InvalidState);
        }
        Ok(())
    }
}

fn remove_id(ids: &mut Vec<u64>, id: u64) -> Result<(), IslandError> {
    let index = ids
        .iter()
        .rposition(|v| *v == id)
        .ok_or(IslandError::InvalidState)?;
    ids.remove(index);
    Ok(())
}
fn find_root(parents: &mut BTreeMap<u64, u64>, id: u64) -> u64 {
    let mut root = id;
    while parents[&root] != root {
        root = parents[&root];
    }
    let mut current = id;
    while current != root {
        let next = parents[&current];
        parents.insert(current, root);
        current = next;
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_dispatch_preserves_prefix_then_reverses_the_final_three_islands() {
        let expected: [&[u64]; 9] = [
            &[],
            &[1],
            &[1, 2],
            &[1, 2, 3],
            &[4, 1, 2, 3],
            &[5, 4, 1, 2, 3],
            &[6, 5, 4, 1, 2, 3],
            &[7, 6, 5, 4, 1, 2, 3],
            &[8, 7, 6, 5, 4, 1, 2, 3],
        ];
        let mut graph = SimulationIslands::new(8, 8).unwrap();
        for (count, expected) in expected.into_iter().enumerate() {
            if count != 0 {
                let island = graph.register_core(count as u64, false).unwrap();
                graph.activate(island).unwrap();
            }
            let before = graph.clone();
            assert_eq!(graph.phase_order().collect::<Vec<_>>(), expected);
            assert_eq!(graph, before);
        }
    }
    fn graph() -> SimulationIslands {
        let mut graph = SimulationIslands::new(8, 8).unwrap();
        graph
            .register_controller(IslandController {
                identity: 10,
                priority: 1000,
                associated: vec![],
            })
            .unwrap();
        for core in 1..=4 {
            graph.register_core(core, false).unwrap();
            graph.attach(core, 10).unwrap();
        }
        graph
    }
    #[test]
    fn fusion_and_multicomponent_split_preserve_target_list_order() {
        let mut graph = graph();
        graph
            .register_controller(IslandController {
                identity: 20,
                priority: 2000,
                associated: vec![1, 2, 3, 4],
            })
            .unwrap();
        assert_eq!(graph.announce(20).unwrap(), None);
        let unit = graph.island_of(1).unwrap();
        assert_eq!(graph.island(unit).unwrap().cores, [4, 3, 2, 1]);
        graph.withdraw(20, true).unwrap();
        let created = graph.resolve_connectivity(unit).unwrap();
        assert_eq!(created.len(), 3);
        assert_eq!(graph.island(unit).unwrap().cores, [1]);
        assert_eq!(
            created
                .iter()
                .map(|id| graph.island(*id).unwrap().cores.clone())
                .collect::<Vec<_>>(),
            [vec![3], vec![4], vec![2]]
        );
        graph.validate().unwrap();
    }
    #[test]
    fn duplicate_bindings_remove_last_stably_and_failed_commands_are_atomic() {
        let mut graph = graph();
        graph
            .register_controller(IslandController {
                identity: 11,
                priority: 1000,
                associated: vec![],
            })
            .unwrap();
        graph.attach(1, 11).unwrap();
        graph.attach(1, 10).unwrap();
        graph.detach(1, 10).unwrap();
        assert_eq!(graph.core_controllers(1).unwrap(), [10, 11]);
        let before = graph.clone();
        assert_eq!(graph.detach(2, 11), Err(IslandError::MissingBinding));
        assert_eq!(graph, before);
        assert_eq!(
            graph.remove_controller(10),
            Err(IslandError::ControllerInUse)
        );
        assert_eq!(graph, before);
        graph.validate().unwrap();
    }
}
