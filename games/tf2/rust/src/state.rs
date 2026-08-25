use std::collections::BTreeMap;

use crate::{
    attribute::{AttributeEntity, AttributeError, AttributeGraph, ProviderKind, QueryValue},
    class::{AmmoLedger, PlayerClass, PlayerTeam},
    condition::{ConditionDuration, ConditionError, ConditionEvent, ConditionId, ConditionState},
    damage::{
        CritCheckInput, CritCheckResult, CritKind, CritState, CustomDamage, DamageError,
        DamageHistory, DamageHistoryInput, DamageInput, DamageModifiers, DamageResult,
        DamageSourceKind, DamageType, apply_damage,
    },
    health::{Healer, HealthConfiguration, HealthError, HealthState, HealthTick},
    pickup::{
        DroppedAmmo, DroppedWeaponState, Pickup, PickupError, PickupEvent, PickupKind,
        PickupLifecycle, PickupResult, retained_dropped_weapon_identities,
    },
    schema::{ItemInstance, ItemSchema, Loadout, LoadoutPosition, SchemaError},
};

pub const MAX_CORE_PLAYERS: usize = 64;
pub const MAX_CORE_PICKUPS: usize = 1_024;
pub const MAX_CORE_COMMANDS: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PlayerLifecycleState {
    Active = 0,
    Welcome = 1,
    Observer = 2,
    Dying = 3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CarriedWeaponState {
    pub clip: i16,
    pub reserve: u16,
    pub energy: f32,
    pub charge: f32,
    pub detonated: u16,
    pub effect_bar_regen_time: f32,
    pub next_primary_time: f32,
    pub next_secondary_time: f32,
    pub broken: bool,
    pub body: i32,
    pub meter: f32,
}

impl Default for CarriedWeaponState {
    fn default() -> Self {
        Self {
            clip: 0,
            reserve: 0,
            energy: 0.0,
            charge: 0.0,
            detonated: 0,
            effect_bar_regen_time: 0.0,
            next_primary_time: 0.0,
            next_secondary_time: 0.0,
            broken: false,
            body: 0,
            meter: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CorePlayer {
    pub identity: u32,
    pub lifecycle: PlayerLifecycleState,
    pub team: PlayerTeam,
    pub class: Option<PlayerClass>,
    pub desired_class: Option<PlayerClass>,
    pub health: Option<HealthState>,
    pub ammo: AmmoLedger,
    pub loadout: Option<Loadout>,
    pub active_slot: Option<LoadoutPosition>,
    pub weapon_states: BTreeMap<LoadoutPosition, CarriedWeaponState>,
    pub conditions: ConditionState,
    pub afterburn: Option<crate::pyro::Afterburn>,
    pub crit: BTreeMap<LoadoutPosition, CritState>,
    pub damage_history: DamageHistory,
    pub attribute_entity: u32,
    pub item_attribute_entities: BTreeMap<LoadoutPosition, u32>,
    pub taunting: bool,
    pub invisible: bool,
    pub active_weapon_allows_pickup: bool,
}

impl CorePlayer {
    pub fn new(identity: u32, attribute_entity: u32) -> Self {
        Self {
            identity,
            lifecycle: PlayerLifecycleState::Welcome,
            team: PlayerTeam::Unassigned,
            class: None,
            desired_class: None,
            health: None,
            ammo: AmmoLedger::default(),
            loadout: None,
            active_slot: None,
            weapon_states: BTreeMap::new(),
            conditions: ConditionState::default(),
            afterburn: None,
            crit: BTreeMap::new(),
            damage_history: DamageHistory::default(),
            attribute_entity,
            item_attribute_entities: BTreeMap::new(),
            taunting: false,
            invisible: false,
            active_weapon_allows_pickup: true,
        }
    }

    pub fn alive(&self) -> bool {
        self.lifecycle == PlayerLifecycleState::Active
            && self
                .health
                .as_ref()
                .is_some_and(|health| health.current > 0)
    }
}

#[derive(Clone, Debug)]
pub enum CoreCommand {
    SelectTeam {
        player: u32,
        team: PlayerTeam,
    },
    SelectClass {
        player: u32,
        class: PlayerClass,
    },
    Spawn {
        player: u32,
    },
    EquipItem {
        player: u32,
        position: LoadoutPosition,
        item: ItemInstance,
    },
    Observe {
        player: u32,
    },
    AddCondition {
        player: u32,
        condition: ConditionId,
        duration: ConditionDuration,
        provider: Option<u32>,
        competitive_summary_allowed: bool,
    },
    RemoveCondition {
        player: u32,
        condition: ConditionId,
        force: bool,
    },
    StartHealing {
        target: u32,
        healer_team: PlayerTeam,
        healer: Healer,
    },
    StopHealing {
        target: u32,
        healer: u32,
    },
    ImmediateHeal {
        target: u32,
        healer: u32,
        healer_team: PlayerTeam,
        amount: f32,
        ignore_maximum: bool,
        ignore_team: bool,
    },
    CriticalCheck {
        player: u32,
        position: LoadoutPosition,
        input: CritCheckInput,
    },
    Damage {
        input: DamageInput,
    },
    TouchPickup {
        player: u32,
        pickup: u32,
    },
    SetPickupDisabled {
        pickup: u32,
        disabled: bool,
    },
    AllowDroppedAmmoOwnerPickup {
        pickup: u32,
    },
}

#[derive(Clone, Debug)]
pub struct CoreTickInput {
    pub tick: u64,
    pub now: f32,
    pub tick_interval: f32,
    pub health_configuration: HealthConfiguration,
    pub commands: Vec<CoreCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoreEvent {
    TeamSelected {
        player: u32,
        team: PlayerTeam,
    },
    ClassSelected {
        player: u32,
        class: PlayerClass,
    },
    Spawned {
        player: u32,
        class: PlayerClass,
        team: PlayerTeam,
    },
    ItemEquipped {
        player: u32,
        position: LoadoutPosition,
        definition: u32,
        replaced_definition: Option<u32>,
    },
    Observing {
        player: u32,
    },
    Condition {
        player: u32,
        event: ConditionEvent,
    },
    HealingStarted {
        target: u32,
        healer: u32,
    },
    HealingStopped {
        target: u32,
        healer: u32,
        accumulated: f32,
    },
    Healed {
        target: u32,
        healer: u32,
        amount: i32,
    },
    HealthTick {
        player: u32,
        tick: HealthTick,
    },
    CriticalCheck {
        player: u32,
        result: CritCheckResult,
    },
    Damage {
        victim: u32,
        result: DamageResult,
    },
    Died {
        victim: u32,
    },
    PickupSpawned {
        pickup: u32,
    },
    Pickup {
        player: u32,
        result: PickupResult,
    },
    PickupLifecycle(PickupEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CorePlayerSnapshot {
    pub identity: u32,
    pub lifecycle: PlayerLifecycleState,
    pub team: PlayerTeam,
    pub class: Option<PlayerClass>,
    pub desired_class: Option<PlayerClass>,
    pub health: Option<HealthState>,
    pub ammo: AmmoLedger,
    pub loadout: Option<Loadout>,
    pub active_slot: Option<LoadoutPosition>,
    pub weapon_states: BTreeMap<LoadoutPosition, CarriedWeaponState>,
    pub conditions: [u32; 5],
    pub afterburn: Option<crate::pyro::Afterburn>,
    pub crit: BTreeMap<LoadoutPosition, CritState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoreSnapshot {
    pub tick: u64,
    pub content_build: u32,
    pub schema_sha256: String,
    pub players: Vec<CorePlayerSnapshot>,
    pub pickups: Vec<Pickup>,
    pub events: Vec<CoreEvent>,
}

#[derive(Clone, Debug)]
pub struct CoreState {
    tick: u64,
    schema: ItemSchema,
    attributes: AttributeGraph,
    players: BTreeMap<u32, CorePlayer>,
    pickups: BTreeMap<u32, Pickup>,
    next_pickup: u32,
    next_attribute_entity: u32,
    last_critical: BTreeMap<(u32, LoadoutPosition), CritCheckResult>,
}

impl CoreState {
    pub fn new(schema: ItemSchema) -> Self {
        Self {
            tick: 0,
            schema,
            attributes: AttributeGraph::default(),
            players: BTreeMap::new(),
            pickups: BTreeMap::new(),
            next_pickup: 1,
            next_attribute_entity: u32::MAX,
            last_critical: BTreeMap::new(),
        }
    }

    pub fn schema(&self) -> &ItemSchema {
        &self.schema
    }

    pub fn attributes(&self) -> &AttributeGraph {
        &self.attributes
    }

    pub fn attributes_mut(&mut self) -> &mut AttributeGraph {
        &mut self.attributes
    }

    pub fn player(&self, identity: u32) -> Option<&CorePlayer> {
        self.players.get(&identity)
    }

    pub fn pickup(&self, identity: u32) -> Option<&Pickup> {
        self.pickups.get(&identity)
    }

    pub fn add_player(&mut self, identity: u32) -> Result<(), CoreError> {
        if self.players.len() >= MAX_CORE_PLAYERS {
            return Err(CoreError::PlayerLimit);
        }
        if self.players.contains_key(&identity) {
            return Err(CoreError::DuplicatePlayer(identity));
        }
        self.attributes
            .insert(AttributeEntity::new(identity, ProviderKind::Player))
            .map_err(CoreError::Attribute)?;
        self.players
            .insert(identity, CorePlayer::new(identity, identity));
        Ok(())
    }

    pub fn add_pickup(&mut self, pickup: Pickup) -> Result<(), CoreError> {
        if self.pickups.len() >= MAX_CORE_PICKUPS {
            return Err(CoreError::PickupLimit);
        }
        if self.pickups.contains_key(&pickup.identity) {
            return Err(CoreError::DuplicatePickup);
        }
        self.next_pickup = self.next_pickup.max(pickup.identity.saturating_add(1));
        self.pickups.insert(pickup.identity, pickup);
        Ok(())
    }

    pub fn transition(&mut self, input: CoreTickInput) -> Result<CoreSnapshot, CoreError> {
        let mut candidate = self.clone();
        let snapshot = candidate.transition_inner(input)?;
        *self = candidate;
        Ok(snapshot)
    }

    fn transition_inner(&mut self, input: CoreTickInput) -> Result<CoreSnapshot, CoreError> {
        if input.tick != self.tick {
            return Err(CoreError::TickMismatch {
                expected: self.tick,
                actual: input.tick,
            });
        }
        if !input.now.is_finite()
            || !input.tick_interval.is_finite()
            || input.tick_interval <= 0.0
            || !input.health_configuration.validate()
        {
            return Err(CoreError::InvalidTick);
        }
        if input.commands.len() > MAX_CORE_COMMANDS {
            return Err(CoreError::CommandLimit);
        }
        self.last_critical.clear();
        let mut events = Vec::new();
        for command in input.commands {
            self.apply_command(input.now, command, &mut events)?;
        }

        for pickup in self.pickups.values_mut() {
            if let Some(event) = pickup.advance(input.now).map_err(CoreError::Pickup)? {
                events.push(CoreEvent::PickupLifecycle(event));
            }
        }

        let player_ids: Vec<_> = self.players.keys().copied().collect();
        for player_id in player_ids {
            if !self.players[&player_id].alive() {
                continue;
            }
            self.advance_afterburn(input.now, player_id, &mut events)?;
            if !self.players[&player_id].alive() {
                continue;
            }
            let healer_count = self.players[&player_id]
                .health
                .as_ref()
                .map_or(0, |health| health.healers.len());
            let condition_events = self
                .players
                .get_mut(&player_id)
                .expect("known player")
                .conditions
                .advance(input.tick_interval, healer_count)
                .map_err(CoreError::Condition)?;
            for event in condition_events {
                events.push(CoreEvent::Condition {
                    player: player_id,
                    event,
                });
            }

            let (from_healers, active_penalty, received) = (
                self.query_player_attribute(player_id, "mult_health_fromhealers", 1.0)?,
                self.query_active_attribute(
                    player_id,
                    "mult_health_fromhealers_penalty_active",
                    1.0,
                )?,
                self.query_active_attribute(player_id, "mult_healing_received", 1.0)?,
            );
            let player = self.players.get_mut(&player_id).expect("known player");
            let tick = player
                .health
                .as_mut()
                .expect("alive player has health")
                .advance(
                    input.now,
                    input.tick_interval,
                    input.health_configuration,
                    from_healers,
                    active_penalty,
                    received,
                    &mut player.conditions,
                )
                .map_err(CoreError::Health)?;
            for event in &tick.condition_events {
                events.push(CoreEvent::Condition {
                    player: player_id,
                    event: event.clone(),
                });
            }
            if tick.before != tick.after || !tick.condition_events.is_empty() {
                events.push(CoreEvent::HealthTick {
                    player: player_id,
                    tick,
                });
            }
        }

        self.pickups
            .retain(|_, pickup| pickup.lifecycle != PickupLifecycle::Removed);
        self.tick += 1;
        Ok(self.snapshot(events))
    }

    fn apply_command(
        &mut self,
        now: f32,
        command: CoreCommand,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        match command {
            CoreCommand::SelectTeam { player, team } => {
                let state = self.player_mut(player)?;
                if state.team != team {
                    state.team = team;
                    if !team.is_gameplay() {
                        state.lifecycle = if team == PlayerTeam::Spectator {
                            PlayerLifecycleState::Observer
                        } else {
                            PlayerLifecycleState::Welcome
                        };
                    }
                    events.push(CoreEvent::TeamSelected { player, team });
                }
            }
            CoreCommand::SelectClass { player, class } => {
                let state = self.player_mut(player)?;
                if state.desired_class != Some(class) {
                    state.desired_class = Some(class);
                    events.push(CoreEvent::ClassSelected { player, class });
                }
            }
            CoreCommand::Spawn { player } => self.spawn_player(player, events)?,
            CoreCommand::EquipItem {
                player,
                position,
                item,
            } => self.equip_item(player, position, item, events)?,
            CoreCommand::Observe { player } => {
                let state = self.player_mut(player)?;
                state.lifecycle = PlayerLifecycleState::Observer;
                state.health = None;
                for event in state.conditions.remove_all() {
                    events.push(CoreEvent::Condition { player, event });
                }
                events.push(CoreEvent::Observing { player });
            }
            CoreCommand::AddCondition {
                player,
                condition,
                duration,
                provider,
                competitive_summary_allowed,
            } => {
                let state = self.player_mut(player)?;
                if let Some(event) = state
                    .conditions
                    .add(
                        condition,
                        duration,
                        provider,
                        state.alive(),
                        competitive_summary_allowed,
                    )
                    .map_err(CoreError::Condition)?
                {
                    events.push(CoreEvent::Condition { player, event });
                }
            }
            CoreCommand::RemoveCondition {
                player,
                condition,
                force,
            } => {
                if let Some(event) = self.player_mut(player)?.conditions.remove(condition, force) {
                    events.push(CoreEvent::Condition { player, event });
                }
            }
            CoreCommand::StartHealing {
                target,
                healer_team,
                healer,
            } => {
                let state = self.player_mut(target)?;
                if !state.alive() || healer_team != state.team {
                    return Err(CoreError::HealingTargetRejected);
                }
                let healer_id = healer.identity;
                let condition_events = state
                    .health
                    .as_mut()
                    .expect("alive player has health")
                    .start_healing(healer, &mut state.conditions)
                    .map_err(CoreError::Health)?;
                events.push(CoreEvent::HealingStarted {
                    target,
                    healer: healer_id,
                });
                for event in condition_events {
                    events.push(CoreEvent::Condition {
                        player: target,
                        event,
                    });
                }
            }
            CoreCommand::StopHealing { target, healer } => {
                let state = self.player_mut(target)?;
                let Some(health) = state.health.as_mut() else {
                    return Ok(());
                };
                let (accumulated, condition_events) =
                    health.stop_healing(healer, &mut state.conditions);
                events.push(CoreEvent::HealingStopped {
                    target,
                    healer,
                    accumulated,
                });
                for event in condition_events {
                    events.push(CoreEvent::Condition {
                        player: target,
                        event,
                    });
                }
            }
            CoreCommand::ImmediateHeal {
                target,
                healer,
                healer_team,
                amount,
                ignore_maximum,
                ignore_team,
            } => {
                let received = self.query_active_attribute(target, "mult_healing_received", 1.0)?;
                let state = self.player_mut(target)?;
                if !state.alive() || (!ignore_team && state.team != healer_team) {
                    return Err(CoreError::HealingTargetRejected);
                }
                let amount = state
                    .health
                    .as_mut()
                    .expect("alive player has health")
                    .take_health(amount, ignore_maximum, received, &state.conditions)
                    .map_err(CoreError::Health)?;
                if amount > 0 {
                    events.push(CoreEvent::Healed {
                        target,
                        healer,
                        amount,
                    });
                }
            }
            CoreCommand::CriticalCheck {
                player,
                position,
                mut input,
            } => {
                if input.now != now {
                    return Err(CoreError::CritTimeMismatch);
                }
                input.chance_multiplier =
                    self.query_item_attribute(player, position, "mult_crit_chance", 1.0)?;
                let state = self.player_mut(player)?;
                if state
                    .loadout
                    .as_ref()
                    .and_then(|loadout| loadout.item(position))
                    .is_none()
                {
                    return Err(CoreError::EquipRejected);
                }
                input.guaranteed_critical |= state.conditions.is_crit_boosted();
                input.player_crit_multiplier = state
                    .damage_history
                    .crit_multiplier(input.now)
                    .map_err(CoreError::Damage)?;
                let result = state
                    .crit
                    .entry(position)
                    .or_default()
                    .check(input)
                    .map_err(CoreError::Damage)?;
                self.last_critical.insert((player, position), result);
                events.push(CoreEvent::CriticalCheck { player, result });
            }
            CoreCommand::Damage { mut input } => {
                if let Some(position) = input.weapon_position
                    && let Some(result) = self.last_critical.remove(&(input.attacker, position))
                {
                    input.crit = result.kind;
                }
                self.damage(now, input, events)?;
            }
            CoreCommand::TouchPickup { player, pickup } => {
                self.touch_pickup(now, player, pickup, events)?;
            }
            CoreCommand::SetPickupDisabled { pickup, disabled } => {
                self.pickups
                    .get_mut(&pickup)
                    .ok_or(CoreError::MissingPickup(pickup))?
                    .set_disabled(disabled);
            }
            CoreCommand::AllowDroppedAmmoOwnerPickup { pickup } => {
                self.pickups
                    .get_mut(&pickup)
                    .ok_or(CoreError::MissingPickup(pickup))?
                    .allow_owner_pickup();
            }
        }
        Ok(())
    }

    fn spawn_player(
        &mut self,
        player_id: u32,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let (team, class, attribute_entity) = {
            let player = self
                .players
                .get(&player_id)
                .ok_or(CoreError::MissingPlayer(player_id))?;
            (
                player.team,
                player.desired_class.ok_or(CoreError::ClassNotSelected)?,
                player.attribute_entity,
            )
        };
        if !team.is_gameplay() {
            return Err(CoreError::GameplayTeamRequired);
        }
        let loadout = Loadout::stock(&self.schema, class, team).map_err(CoreError::Schema)?;
        self.remove_item_attribute_entities(player_id)?;
        let provider_items = LoadoutPosition::ALL
            .into_iter()
            .filter_map(|position| loadout.item(position).cloned().map(|item| (position, item)))
            .collect::<Vec<_>>();
        for (position, item) in provider_items {
            self.install_item_attribute_entity(player_id, position, &item)?;
        }
        let add_max = self.query_attribute(attribute_entity, "add_maxhealth", 0.0)?;
        let add_nonbuffed =
            self.query_attribute(attribute_entity, "add_maxhealth_nonbuffed", 0.0)?;
        let mut weapon_states = BTreeMap::new();
        let mut crit = BTreeMap::new();
        for position in LoadoutPosition::ALL {
            if loadout.item(position).is_some() {
                weapon_states.insert(position, CarriedWeaponState::default());
                crit.insert(position, CritState::default());
            }
        }
        let active_slot = [
            LoadoutPosition::Primary,
            LoadoutPosition::Secondary,
            LoadoutPosition::Melee,
        ]
        .into_iter()
        .find(|position| loadout.item(*position).is_some());
        let state = self.player_mut(player_id)?;
        for event in state.conditions.remove_all() {
            events.push(CoreEvent::Condition {
                player: player_id,
                event,
            });
        }
        state.class = Some(class);
        state.lifecycle = PlayerLifecycleState::Active;
        state.health =
            Some(HealthState::spawn(class, add_max, add_nonbuffed).map_err(CoreError::Health)?);
        state.ammo = class.data().maximum_ammo;
        state.loadout = Some(loadout);
        state.active_slot = active_slot;
        state.weapon_states = weapon_states;
        state.afterburn = None;
        state.crit = crit;
        state.damage_history = DamageHistory::default();
        events.push(CoreEvent::Spawned {
            player: player_id,
            class,
            team,
        });
        Ok(())
    }

    fn equip_item(
        &mut self,
        player_id: u32,
        position: LoadoutPosition,
        item: ItemInstance,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let definition = item.definition;
        let replaced = {
            let player = self.player_mut(player_id)?;
            if !player.alive() {
                return Err(CoreError::EquipRejected);
            }
            player
                .loadout
                .as_mut()
                .ok_or(CoreError::ClassNotSelected)?
                .equip(position, item.clone())
                .map_err(CoreError::Schema)?
        };
        self.remove_item_attribute_entity(player_id, position)?;
        self.install_item_attribute_entity(player_id, position, &item)?;
        self.refresh_health_maximum(player_id)?;
        let player = self.player_mut(player_id)?;
        player.crit.insert(position, CritState::default());
        player
            .weapon_states
            .insert(position, CarriedWeaponState::default());
        events.push(CoreEvent::ItemEquipped {
            player: player_id,
            position,
            definition,
            replaced_definition: replaced.map(|item| item.definition),
        });
        Ok(())
    }

    fn damage(
        &mut self,
        now: f32,
        mut input: DamageInput,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        if input.source == DamageSourceKind::Player {
            let attacker = self
                .players
                .get(&input.attacker)
                .ok_or(CoreError::MissingPlayer(input.attacker))?;
            if attacker.team != input.attacker_team {
                return Err(CoreError::DamageIdentityMismatch);
            }
            if input.weapon_position.is_some_and(|position| {
                attacker
                    .loadout
                    .as_ref()
                    .and_then(|loadout| loadout.item(position))
                    .is_none()
            }) {
                return Err(CoreError::DamageIdentityMismatch);
            }
            input.attacker_conditions = attacker.conditions.clone();
        } else {
            input.attacker_conditions = ConditionState::default();
        }
        let victim = self
            .players
            .get(&input.victim)
            .ok_or(CoreError::MissingPlayer(input.victim))?;
        if victim.team != input.victim_team {
            return Err(CoreError::DamageIdentityMismatch);
        }
        let modifiers = self.damage_modifiers(&input)?;
        let result = {
            let victim = self.player_mut(input.victim)?;
            let health = victim.health.as_mut().ok_or(CoreError::VictimHasNoHealth)?;
            let result = apply_damage(
                victim.lifecycle == PlayerLifecycleState::Active,
                health,
                &mut victim.conditions,
                &input,
                modifiers,
            )
            .map_err(CoreError::Damage)?;
            if result.admitted {
                health.last_damage_time = now;
            }
            result
        };
        if result.admitted
            && result.death.is_none()
            && input.source == DamageSourceKind::Player
            && input.damage_type.contains(DamageType::IGNITE)
        {
            self.ignite(now, input.attacker, input.victim, events)?;
        }
        if input.source == DamageSourceKind::Player && result.admitted {
            let attacker = self.player_mut(input.attacker)?;
            attacker
                .damage_history
                .record(DamageHistoryInput {
                    now,
                    damage: result.pre_resistance_base_damage + result.pre_resistance_bonus_damage,
                    bonus_damage: result.pre_resistance_bonus_damage,
                    victim_previous_health: result.health_before,
                    lethal: result.death.is_some(),
                    damage_type: input.damage_type,
                    counts_toward_crit_rate: true,
                })
                .map_err(CoreError::Damage)?;
        }
        for condition_event in &result.condition_events {
            events.push(CoreEvent::Condition {
                player: input.victim,
                event: condition_event.clone(),
            });
        }
        events.push(CoreEvent::Damage {
            victim: input.victim,
            result: result.clone(),
        });
        if result.death.is_some() {
            self.kill_player(
                now,
                input.victim,
                input.attacker_team == input.victim_team,
                events,
            )?;
        }
        Ok(())
    }

    fn ignite(
        &mut self,
        now: f32,
        attacker: u32,
        victim: u32,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let state = self.player_mut(victim)?;
        if !state.alive()
            || state.conditions.contains(ConditionId::PHASE)
            || state
                .conditions
                .contains(ConditionId::PASSTIME_INTERCEPTION)
        {
            return Ok(());
        }
        let class = state.class.ok_or(CoreError::ClassNotSelected)?;
        let initial = state.afterburn.is_none();
        state.afterburn = Some(crate::pyro::Afterburn::ignite(
            state.afterburn,
            class,
            attacker,
            21,
            now,
        ));
        if initial {
            if let Some(event) = state
                .conditions
                .add(
                    ConditionId::BURNING,
                    ConditionDuration::Permanent,
                    Some(attacker),
                    true,
                    false,
                )
                .map_err(CoreError::Condition)?
            {
                events.push(CoreEvent::Condition {
                    player: victim,
                    event,
                });
            }
            if let Some(event) = state
                .conditions
                .add(
                    ConditionId::HEALING_DEBUFF,
                    ConditionDuration::Finite(crate::pyro::FLAME_INITIAL_AFTERBURN),
                    Some(attacker),
                    true,
                    false,
                )
                .map_err(CoreError::Condition)?
            {
                events.push(CoreEvent::Condition {
                    player: victim,
                    event,
                });
            }
        }
        Ok(())
    }

    fn advance_afterburn(
        &mut self,
        now: f32,
        victim: u32,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let Some(mut burn) = self
            .players
            .get(&victim)
            .and_then(|player| player.afterburn)
        else {
            return Ok(());
        };
        let damage = burn.advance(now);
        let expired = burn.duration <= 0.0;
        let player = self.player_mut(victim)?;
        player.afterburn = (!expired).then_some(burn);
        if expired && let Some(event) = player.conditions.remove(ConditionId::BURNING, true) {
            events.push(CoreEvent::Condition {
                player: victim,
                event,
            });
        }
        if let Some(amount) = damage {
            let attacker_team = self
                .players
                .get(&burn.attacker)
                .ok_or(CoreError::MissingPlayer(burn.attacker))?
                .team;
            let victim_team = self
                .player(victim)
                .ok_or(CoreError::MissingPlayer(victim))?
                .team;
            self.damage(
                now,
                DamageInput {
                    attacker: burn.attacker,
                    attacker_team,
                    attacker_conditions: ConditionState::default(),
                    source: DamageSourceKind::Player,
                    weapon_position: Some(LoadoutPosition::Primary),
                    victim,
                    victim_team,
                    base_damage: amount,
                    range_multiplier: 1.0,
                    damage_type: DamageType::BURN | DamageType::PREVENT_FORCE,
                    custom: CustomDamage::Burning,
                    crit: CritKind::None,
                    friendly_fire: false,
                    force_friendly_fire: false,
                    bypass_invulnerability: false,
                    force: [0.0; 3],
                },
                events,
            )?;
        }
        Ok(())
    }

    fn kill_player(
        &mut self,
        now: f32,
        victim: u32,
        suicide_or_team_damage: bool,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let (ammo_drop, weapon_drop) = {
            let player = self
                .players
                .get(&victim)
                .ok_or(CoreError::MissingPlayer(victim))?;
            let ammo = DroppedAmmo::from_death(victim, player.ammo);
            let weapon = player
                .active_slot
                .and_then(|slot| Some((slot, player.loadout.as_ref()?.item(slot)?.clone())))
                .map(|(slot, item)| {
                    let state = player.weapon_states.get(&slot).cloned().unwrap_or_default();
                    DroppedWeaponState {
                        owner: victim,
                        item,
                        clip: state.clip,
                        reserve: state.reserve,
                        energy: state.energy,
                        charge: if suicide_or_team_damage {
                            0.0
                        } else {
                            state.charge
                        },
                        detonated: state.detonated,
                        effect_bar_regen_time: state.effect_bar_regen_time,
                        next_primary_time: state.next_primary_time,
                        next_secondary_time: state.next_secondary_time,
                        broken: state.broken,
                        body: state.body,
                        meter: state.meter,
                    }
                });
            (ammo, weapon)
        };
        self.player_mut(victim)?.lifecycle = PlayerLifecycleState::Dying;
        events.push(CoreEvent::Died { victim });
        if let Some(weapon) = weapon_drop {
            for pickup in self.pickups.values_mut() {
                if let PickupKind::DroppedWeapon(existing) = &pickup.kind
                    && existing.owner == weapon.owner
                    && existing.item.item_id == weapon.item.item_id
                    && existing.item.definition == weapon.item.definition
                {
                    pickup.lifecycle = PickupLifecycle::Removed;
                }
            }
            let identity = self.allocate_pickup()?;
            self.pickups.insert(
                identity,
                Pickup::dropped(identity, PickupKind::DroppedWeapon(weapon), now)
                    .map_err(CoreError::Pickup)?,
            );
            events.push(CoreEvent::PickupSpawned { pickup: identity });
            self.enforce_dropped_weapon_limit();
        }
        let identity = self.allocate_pickup()?;
        self.pickups.insert(
            identity,
            Pickup::dropped(identity, PickupKind::DroppedAmmo(ammo_drop), now)
                .map_err(CoreError::Pickup)?,
        );
        events.push(CoreEvent::PickupSpawned { pickup: identity });
        self.enforce_dropped_ammo_limit(victim);

        let player = self.player_mut(victim)?;
        player.afterburn = None;
        for event in player.conditions.remove_all() {
            events.push(CoreEvent::Condition {
                player: victim,
                event,
            });
        }
        if let Some(health) = player.health.as_mut() {
            health.current = 0;
            health.healers.clear();
        }
        Ok(())
    }

    fn touch_pickup(
        &mut self,
        now: f32,
        player_id: u32,
        pickup_id: u32,
        events: &mut Vec<CoreEvent>,
    ) -> Result<(), CoreError> {
        let health_from_packs =
            self.query_player_attribute(player_id, "mult_health_frompacks", 1.0)?;
        let healing_received =
            self.query_active_attribute(player_id, "mult_healing_received", 1.0)?;
        let mut pickup = self
            .pickups
            .remove(&pickup_id)
            .ok_or(CoreError::MissingPickup(pickup_id))?;
        let result = {
            let schema = &self.schema;
            let player = self
                .players
                .get_mut(&player_id)
                .ok_or(CoreError::MissingPlayer(player_id))?;
            let class = player.class.ok_or(CoreError::ClassNotSelected)?;
            let alive = player.alive();
            let health = player.health.as_mut().ok_or(CoreError::VictimHasNoHealth)?;
            let loadout = player.loadout.as_mut().ok_or(CoreError::ClassNotSelected)?;
            pickup
                .touch(
                    now,
                    player_id,
                    class,
                    player.team,
                    alive,
                    player.taunting,
                    player.invisible,
                    player.active_weapon_allows_pickup,
                    health_from_packs,
                    healing_received,
                    schema,
                    health,
                    &mut player.ammo,
                    &mut player.conditions,
                    loadout,
                )
                .map_err(CoreError::Pickup)?
        };
        self.pickups.insert(pickup_id, pickup);
        if let Some(equipped) = &result.equipped {
            let position = equipped.picked_up.item.slot;
            self.remove_item_attribute_entity(player_id, position)?;
            self.install_item_attribute_entity(player_id, position, &equipped.picked_up.item)?;
            self.refresh_health_maximum(player_id)?;
            let dropped = &equipped.picked_up;
            let player = self.player_mut(player_id)?;
            player.weapon_states.insert(
                position,
                CarriedWeaponState {
                    clip: dropped.clip,
                    reserve: dropped.reserve,
                    energy: dropped.energy,
                    charge: dropped.charge,
                    detonated: dropped.detonated,
                    effect_bar_regen_time: dropped.effect_bar_regen_time,
                    next_primary_time: dropped.next_primary_time,
                    next_secondary_time: dropped.next_secondary_time,
                    broken: dropped.broken,
                    body: dropped.body,
                    meter: dropped.meter,
                },
            );
            player.crit.insert(position, CritState::default());
        }
        for event in &result.condition_events {
            events.push(CoreEvent::Condition {
                player: player_id,
                event: event.clone(),
            });
        }
        events.push(CoreEvent::Pickup {
            player: player_id,
            result,
        });
        Ok(())
    }

    fn query_player_attribute(
        &mut self,
        player: u32,
        hook: &str,
        input: f32,
    ) -> Result<f32, CoreError> {
        let entity = self
            .players
            .get(&player)
            .ok_or(CoreError::MissingPlayer(player))?
            .attribute_entity;
        self.query_attribute(entity, hook, input)
    }

    fn query_active_attribute(
        &mut self,
        player: u32,
        hook: &str,
        input: f32,
    ) -> Result<f32, CoreError> {
        let Some(position) = self
            .players
            .get(&player)
            .ok_or(CoreError::MissingPlayer(player))?
            .active_slot
        else {
            return Ok(input);
        };
        self.query_item_attribute(player, position, hook, input)
    }

    fn query_item_attribute(
        &mut self,
        player: u32,
        position: LoadoutPosition,
        hook: &str,
        input: f32,
    ) -> Result<f32, CoreError> {
        let entity = self
            .players
            .get(&player)
            .ok_or(CoreError::MissingPlayer(player))?
            .item_attribute_entities
            .get(&position)
            .copied()
            .ok_or(CoreError::EquipRejected)?;
        self.query_attribute(entity, hook, input)
    }

    fn damage_modifiers(&mut self, input: &DamageInput) -> Result<DamageModifiers, CoreError> {
        let weapon = (input.source == DamageSourceKind::Player)
            .then_some(input.weapon_position)
            .flatten();
        let outgoing_vs_players = if let Some(position) = weapon {
            self.query_item_attribute(input.attacker, position, "mult_dmg_vs_players", 1.0)?
        } else {
            1.0
        };
        let pierces_resists = if let Some(position) = weapon {
            self.query_item_attribute(input.attacker, position, "mod_pierce_resists_absorbs", 0.0)?
                != 0.0
        } else {
            false
        };
        let minicrits_become_crits = if let Some(position) = weapon {
            self.query_item_attribute(input.attacker, position, "minicrits_become_crits", 0.0)?
                != 0.0
        } else {
            false
        };
        let crits_become_minicrits = if let Some(position) = weapon {
            self.query_item_attribute(input.attacker, position, "crits_become_minicrits", 0.0)?
                != 0.0
        } else {
            false
        };
        Ok(DamageModifiers {
            outgoing_vs_players,
            critical_bonus_taken: self.query_player_attribute(
                input.victim,
                "mult_dmgtaken_from_crit",
                1.0,
            )?,
            fire_taken: self.query_player_attribute(
                input.victim,
                "mult_dmgtaken_from_fire",
                1.0,
            )? * self.query_active_attribute(
                input.victim,
                "mult_dmgtaken_from_fire_active",
                1.0,
            )?,
            blast_taken: self.query_player_attribute(
                input.victim,
                "mult_dmgtaken_from_explosions",
                1.0,
            )?,
            bullet_taken: self.query_player_attribute(
                input.victim,
                "mult_dmgtaken_from_bullets",
                1.0,
            )?,
            melee_taken: self.query_player_attribute(
                input.victim,
                "mult_dmgtaken_from_melee",
                1.0,
            )?,
            general_taken: self.query_player_attribute(input.victim, "mult_dmgtaken", 1.0)?,
            active_taken: self.query_active_attribute(input.victim, "mult_dmgtaken_active", 1.0)?,
            pierces_resists,
            minicrits_become_crits,
            crits_become_minicrits,
        })
    }

    fn query_attribute(&mut self, entity: u32, hook: &str, input: f32) -> Result<f32, CoreError> {
        match self
            .attributes
            .query_numeric(&self.schema, entity, entity, hook, input, false)
            .map_err(CoreError::Attribute)?
            .value
        {
            QueryValue::Numeric(value) => Ok(value),
            QueryValue::String(_) => Err(CoreError::AttributeType),
        }
    }

    fn install_item_attribute_entity(
        &mut self,
        player_id: u32,
        position: LoadoutPosition,
        item: &ItemInstance,
    ) -> Result<(), CoreError> {
        let identity = self.allocate_attribute_entity()?;
        let kind = if matches!(
            position,
            LoadoutPosition::Primary
                | LoadoutPosition::Secondary
                | LoadoutPosition::Melee
                | LoadoutPosition::Utility
                | LoadoutPosition::Building
                | LoadoutPosition::Pda
                | LoadoutPosition::Pda2
        ) {
            ProviderKind::Weapon
        } else {
            ProviderKind::Generic
        };
        self.attributes
            .insert(AttributeEntity::new(identity, kind))
            .map_err(CoreError::Attribute)?;
        self.attributes
            .set_attributes(
                identity,
                item.ordered_attributes(&self.schema)
                    .map_err(CoreError::Schema)?,
            )
            .map_err(CoreError::Attribute)?;
        let player_attribute = self
            .players
            .get(&player_id)
            .ok_or(CoreError::MissingPlayer(player_id))?
            .attribute_entity;
        self.attributes
            .provide_to(identity, player_attribute)
            .map_err(CoreError::Attribute)?;
        self.attributes
            .set_owner(identity, Some(player_attribute))
            .map_err(CoreError::Attribute)?;
        self.player_mut(player_id)?
            .item_attribute_entities
            .insert(position, identity);
        Ok(())
    }

    fn remove_item_attribute_entity(
        &mut self,
        player_id: u32,
        position: LoadoutPosition,
    ) -> Result<(), CoreError> {
        let identity = self
            .player_mut(player_id)?
            .item_attribute_entities
            .remove(&position);
        if let Some(identity) = identity {
            self.attributes
                .remove(identity)
                .map_err(CoreError::Attribute)?;
        }
        Ok(())
    }

    fn remove_item_attribute_entities(&mut self, player_id: u32) -> Result<(), CoreError> {
        let positions: Vec<_> = self
            .players
            .get(&player_id)
            .ok_or(CoreError::MissingPlayer(player_id))?
            .item_attribute_entities
            .keys()
            .copied()
            .collect();
        for position in positions {
            self.remove_item_attribute_entity(player_id, position)?;
        }
        Ok(())
    }

    fn refresh_health_maximum(&mut self, player_id: u32) -> Result<(), CoreError> {
        let (class, attribute_entity) = {
            let player = self
                .players
                .get(&player_id)
                .ok_or(CoreError::MissingPlayer(player_id))?;
            (
                player.class.ok_or(CoreError::ClassNotSelected)?,
                player.attribute_entity,
            )
        };
        let add_max = self.query_attribute(attribute_entity, "add_maxhealth", 0.0)?;
        let add_nonbuffed =
            self.query_attribute(attribute_entity, "add_maxhealth_nonbuffed", 0.0)?;
        let health = self
            .player_mut(player_id)?
            .health
            .as_mut()
            .ok_or(CoreError::VictimHasNoHealth)?;
        health.maximum_for_buffing = (class.data().maximum_health as f32 + add_max) as i32;
        health.maximum = ((health.maximum_for_buffing as f32 + add_nonbuffed) as i32).max(1);
        Ok(())
    }

    fn player_mut(&mut self, identity: u32) -> Result<&mut CorePlayer, CoreError> {
        self.players
            .get_mut(&identity)
            .ok_or(CoreError::MissingPlayer(identity))
    }

    fn allocate_pickup(&mut self) -> Result<u32, CoreError> {
        if self.pickups.len() >= MAX_CORE_PICKUPS {
            return Err(CoreError::PickupLimit);
        }
        let identity = self.next_pickup;
        self.next_pickup = self
            .next_pickup
            .checked_add(1)
            .ok_or(CoreError::PickupIdentityExhausted)?;
        Ok(identity)
    }

    fn allocate_attribute_entity(&mut self) -> Result<u32, CoreError> {
        loop {
            let identity = self.next_attribute_entity;
            self.next_attribute_entity = self
                .next_attribute_entity
                .checked_sub(1)
                .ok_or(CoreError::AttributeIdentityExhausted)?;
            if self.attributes.entity(identity).is_none() {
                return Ok(identity);
            }
        }
    }

    fn enforce_dropped_weapon_limit(&mut self) {
        let pickups: Vec<_> = self.pickups.values().cloned().collect();
        let retained = retained_dropped_weapon_identities(&pickups);
        for pickup in self.pickups.values_mut() {
            if matches!(pickup.kind, PickupKind::DroppedWeapon(_))
                && !retained.contains(&pickup.identity)
            {
                pickup.lifecycle = PickupLifecycle::Removed;
            }
        }
    }

    fn enforce_dropped_ammo_limit(&mut self, owner: u32) {
        let mut active = self
            .pickups
            .values()
            .filter(|pickup| {
                pickup.lifecycle != PickupLifecycle::Removed
                    && matches!(&pickup.kind, PickupKind::DroppedAmmo(ammo) if ammo.owner == owner)
            })
            .map(|pickup| (pickup.creation_time, pickup.identity))
            .collect::<Vec<_>>();
        active.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        let remove_count = active.len().saturating_sub(3);
        for (_, identity) in active.into_iter().take(remove_count) {
            self.pickups
                .get_mut(&identity)
                .expect("selected dropped ammo exists")
                .lifecycle = PickupLifecycle::Removed;
        }
    }

    fn snapshot(&self, events: Vec<CoreEvent>) -> CoreSnapshot {
        CoreSnapshot {
            tick: self.tick + 1,
            content_build: self.schema.content_build(),
            schema_sha256: self.schema.schema_sha256().into(),
            players: self
                .players
                .values()
                .map(|player| CorePlayerSnapshot {
                    identity: player.identity,
                    lifecycle: player.lifecycle,
                    team: player.team,
                    class: player.class,
                    desired_class: player.desired_class,
                    health: player.health.clone(),
                    ammo: player.ammo,
                    loadout: player.loadout.clone(),
                    active_slot: player.active_slot,
                    weapon_states: player.weapon_states.clone(),
                    conditions: player.conditions.words(),
                    afterburn: player.afterburn,
                    crit: player.crit.clone(),
                })
                .collect(),
            pickups: self.pickups.values().cloned().collect(),
            events,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoreError {
    TickMismatch { expected: u64, actual: u64 },
    InvalidTick,
    CommandLimit,
    PlayerLimit,
    PickupLimit,
    DuplicatePlayer(u32),
    MissingPlayer(u32),
    DuplicatePickup,
    MissingPickup(u32),
    PickupIdentityExhausted,
    AttributeIdentityExhausted,
    GameplayTeamRequired,
    ClassNotSelected,
    VictimHasNoHealth,
    HealingTargetRejected,
    DamageIdentityMismatch,
    CritTimeMismatch,
    EquipRejected,
    AttributeType,
    Schema(SchemaError),
    Attribute(AttributeError),
    Condition(ConditionError),
    Health(HealthError),
    Damage(DamageError),
    Pickup(PickupError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        damage::{CritKind, CustomDamage, DamageType},
        pickup::PickupSize,
        schema::{ITEM_SCHEMA_SHA256, ITEM_SCHEMA_SIGNATURE_SHA256, SchemaInput, SchemaNode},
    };

    fn stock_schema() -> ItemSchema {
        let mut definitions: BTreeMap<u32, (u8, Vec<&str>)> = BTreeMap::new();
        for class in PlayerClass::ALL {
            for stock in class.data().stock_items {
                let entry = definitions
                    .entry(stock.definition)
                    .or_insert_with(|| (stock.slot, Vec::new()));
                assert_eq!(entry.0, stock.slot);
                entry.1.push(class.data().source_name);
            }
        }
        let mut items = definitions
            .into_iter()
            .map(|(index, (slot, classes))| {
                let position = LoadoutPosition::try_from(slot).unwrap();
                SchemaNode::object(
                    index.to_string(),
                    vec![
                        SchemaNode::scalar("name", format!("stock-{index}")),
                        SchemaNode::scalar("item_class", "tf_weapon"),
                        SchemaNode::scalar("item_slot", slot_name(position)),
                        SchemaNode::scalar("item_quality", "normal"),
                        SchemaNode::scalar("min_ilevel", "1"),
                        SchemaNode::scalar("max_ilevel", "1"),
                        SchemaNode::object(
                            "used_by_classes",
                            classes
                                .into_iter()
                                .map(|class| SchemaNode::scalar(class, "1"))
                                .collect(),
                        ),
                    ],
                )
            })
            .collect::<Vec<_>>();
        items.insert(
            0,
            SchemaNode::object(
                "default",
                vec![
                    SchemaNode::scalar("name", "default"),
                    SchemaNode::scalar("item_class", "tf_wearable"),
                    SchemaNode::scalar("item_slot", "melee"),
                ],
            ),
        );
        ItemSchema::compose(SchemaInput {
            content_build: 24_245_096,
            schema_sha256: ITEM_SCHEMA_SHA256.into(),
            signature_sha256: ITEM_SCHEMA_SIGNATURE_SHA256.into(),
            game_info: [
                ("first_valid_class", "1"),
                ("last_valid_class", "9"),
                ("account_class_index", "16"),
                ("account_first_valid_item_slot", "0"),
                ("account_last_valid_item_slot", "3"),
                ("first_valid_item_slot", "0"),
                ("last_valid_item_slot", "18"),
                ("num_item_presets", "4"),
            ]
            .into_iter()
            .map(|(key, value)| SchemaNode::scalar(key, value))
            .collect(),
            prefabs: Vec::new(),
            attributes: vec![
                SchemaNode::object(
                    "26",
                    vec![
                        SchemaNode::scalar("name", "max health additive bonus"),
                        SchemaNode::scalar("attribute_class", "add_maxhealth"),
                        SchemaNode::scalar("description_format", "value_is_additive"),
                        SchemaNode::scalar("stored_as_integer", "0"),
                    ],
                ),
                SchemaNode::object(
                    "526",
                    vec![
                        SchemaNode::scalar("name", "healing received bonus"),
                        SchemaNode::scalar("attribute_class", "mult_healing_received"),
                        SchemaNode::scalar("description_format", "value_is_percentage"),
                        SchemaNode::scalar("stored_as_integer", "0"),
                    ],
                ),
            ],
            items,
        })
        .unwrap()
    }

    fn slot_name(position: LoadoutPosition) -> &'static str {
        match position {
            LoadoutPosition::Primary => "primary",
            LoadoutPosition::Secondary => "secondary",
            LoadoutPosition::Melee => "melee",
            LoadoutPosition::Utility => "utility",
            LoadoutPosition::Building => "building",
            LoadoutPosition::Pda => "pda",
            LoadoutPosition::Pda2 => "pda2",
            LoadoutPosition::Head => "head",
            LoadoutPosition::Misc => "misc",
            LoadoutPosition::Action => "action",
            LoadoutPosition::Misc2 => "misc2",
            LoadoutPosition::Taunt => "taunt",
            LoadoutPosition::Taunt2 => "taunt2",
            LoadoutPosition::Taunt3 => "taunt3",
            LoadoutPosition::Taunt4 => "taunt4",
            LoadoutPosition::Taunt5 => "taunt5",
            LoadoutPosition::Taunt6 => "taunt6",
            LoadoutPosition::Taunt7 => "taunt7",
            LoadoutPosition::Taunt8 => "taunt8",
        }
    }

    fn tick(tick: u64, commands: Vec<CoreCommand>) -> CoreTickInput {
        CoreTickInput {
            tick,
            now: tick as f32,
            tick_interval: 0.1,
            health_configuration: HealthConfiguration::default(),
            commands,
        }
    }

    #[test]
    fn team_class_stock_spawn_and_observer_lifecycle_are_atomic() {
        let mut state = CoreState::new(stock_schema());
        state.add_player(1).unwrap();
        let snapshot = state
            .transition(tick(
                0,
                vec![
                    CoreCommand::SelectTeam {
                        player: 1,
                        team: PlayerTeam::Red,
                    },
                    CoreCommand::SelectClass {
                        player: 1,
                        class: PlayerClass::Scout,
                    },
                    CoreCommand::Spawn { player: 1 },
                ],
            ))
            .unwrap();
        let player = &snapshot.players[0];
        assert_eq!(player.lifecycle, PlayerLifecycleState::Active);
        assert_eq!(player.health.as_ref().unwrap().current, 125);
        assert_eq!(player.ammo, PlayerClass::Scout.data().maximum_ammo);
        assert_eq!(
            player
                .loadout
                .as_ref()
                .unwrap()
                .class_slots()
                .iter()
                .flatten()
                .count(),
            3
        );

        state
            .transition(tick(1, vec![CoreCommand::Observe { player: 1 }]))
            .unwrap();
        assert_eq!(
            state.player(1).unwrap().lifecycle,
            PlayerLifecycleState::Observer
        );
        let before = state.player(1).unwrap().clone();
        assert!(matches!(
            state.transition(tick(3, Vec::new())),
            Err(CoreError::TickMismatch { .. })
        ));
        assert_eq!(state.player(1).unwrap().lifecycle, before.lifecycle);
    }

    #[test]
    fn flame_damage_ignites_stacks_ticks_and_preserves_pyro_immunity() {
        let mut state = CoreState::new(stock_schema());
        for (identity, team, class) in [
            (1, PlayerTeam::Red, PlayerClass::Pyro),
            (2, PlayerTeam::Blue, PlayerClass::Scout),
            (3, PlayerTeam::Blue, PlayerClass::Pyro),
        ] {
            state.add_player(identity).unwrap();
            state
                .transition(tick(
                    state.tick,
                    vec![
                        CoreCommand::SelectTeam {
                            player: identity,
                            team,
                        },
                        CoreCommand::SelectClass {
                            player: identity,
                            class,
                        },
                        CoreCommand::Spawn { player: identity },
                    ],
                ))
                .unwrap();
        }
        let fire = |victim| DamageInput {
            attacker: 1,
            attacker_team: PlayerTeam::Red,
            attacker_conditions: ConditionState::default(),
            source: DamageSourceKind::Player,
            weapon_position: Some(LoadoutPosition::Primary),
            victim,
            victim_team: PlayerTeam::Blue,
            base_damage: 13.0,
            range_multiplier: 1.0,
            damage_type: DamageType::BURN | DamageType::IGNITE,
            custom: CustomDamage::None,
            crit: CritKind::None,
            friendly_fire: false,
            force_friendly_fire: false,
            bypass_invulnerability: false,
            force: [0.0; 3],
        };
        state
            .transition(tick(
                state.tick,
                vec![CoreCommand::Damage { input: fire(2) }],
            ))
            .unwrap();
        let scout = state.player(2).unwrap();
        assert!(scout.conditions.contains(ConditionId::BURNING));
        assert_eq!(scout.afterburn.unwrap().duration, 3.4);
        assert_eq!(scout.health.as_ref().unwrap().current, 112);
        state.transition(tick(state.tick, Vec::new())).unwrap();
        assert_eq!(
            state.player(2).unwrap().health.as_ref().unwrap().current,
            108
        );

        state
            .transition(tick(
                state.tick,
                vec![CoreCommand::Damage { input: fire(3) }],
            ))
            .unwrap();
        assert_eq!(state.player(3).unwrap().afterburn.unwrap().duration, 0.25);
        let pyro_health = state.player(3).unwrap().health.as_ref().unwrap().current;
        state.transition(tick(state.tick, Vec::new())).unwrap();
        assert_eq!(
            state.player(3).unwrap().health.as_ref().unwrap().current,
            pyro_health
        );
        assert!(
            !state
                .player(3)
                .unwrap()
                .conditions
                .contains(ConditionId::BURNING)
        );
    }

    #[test]
    fn healing_damage_death_drop_and_respawn_share_one_transition() {
        let mut state = CoreState::new(stock_schema());
        state.add_player(1).unwrap();
        state.add_player(2).unwrap();
        for player in [1, 2] {
            state
                .transition(tick(
                    state.tick,
                    vec![
                        CoreCommand::SelectTeam {
                            player,
                            team: if player == 1 {
                                PlayerTeam::Red
                            } else {
                                PlayerTeam::Blue
                            },
                        },
                        CoreCommand::SelectClass {
                            player,
                            class: PlayerClass::Scout,
                        },
                        CoreCommand::Spawn { player },
                    ],
                ))
                .unwrap();
        }
        let damage = DamageInput {
            attacker: 1,
            attacker_team: PlayerTeam::Red,
            attacker_conditions: ConditionState::default(),
            source: DamageSourceKind::Player,
            weapon_position: Some(LoadoutPosition::Primary),
            victim: 2,
            victim_team: PlayerTeam::Blue,
            base_damage: 200.0,
            range_multiplier: 1.0,
            damage_type: DamageType::BLAST,
            custom: CustomDamage::None,
            crit: CritKind::None,
            friendly_fire: false,
            force_friendly_fire: false,
            bypass_invulnerability: false,
            force: [0.0, 0.0, 100.0],
        };
        let snapshot = state
            .transition(tick(
                state.tick,
                vec![CoreCommand::Damage { input: damage }],
            ))
            .unwrap();
        let victim = snapshot
            .players
            .iter()
            .find(|player| player.identity == 2)
            .unwrap();
        assert_eq!(victim.lifecycle, PlayerLifecycleState::Dying);
        assert_eq!(victim.health.as_ref().unwrap().current, 0);
        assert_eq!(snapshot.pickups.len(), 2);

        state
            .transition(tick(state.tick, vec![CoreCommand::Spawn { player: 2 }]))
            .unwrap();
        assert_eq!(
            state.player(2).unwrap().lifecycle,
            PlayerLifecycleState::Active
        );
        assert_eq!(
            state.player(2).unwrap().health.as_ref().unwrap().current,
            125
        );
    }

    #[test]
    fn pickup_and_post_entity_health_condition_phase_are_ordered() {
        let mut state = CoreState::new(stock_schema());
        state.add_player(1).unwrap();
        state
            .add_pickup(
                Pickup::map(
                    50,
                    PickupKind::Health(PickupSize::Small),
                    Some(PlayerTeam::Red),
                    0.0,
                )
                .unwrap(),
            )
            .unwrap();
        state
            .transition(tick(
                0,
                vec![
                    CoreCommand::SelectTeam {
                        player: 1,
                        team: PlayerTeam::Red,
                    },
                    CoreCommand::SelectClass {
                        player: 1,
                        class: PlayerClass::Scout,
                    },
                    CoreCommand::Spawn { player: 1 },
                    CoreCommand::AddCondition {
                        player: 1,
                        condition: ConditionId::BURNING,
                        duration: ConditionDuration::Finite(10.0),
                        provider: Some(2),
                        competitive_summary_allowed: false,
                    },
                ],
            ))
            .unwrap();
        state
            .player_mut(1)
            .unwrap()
            .health
            .as_mut()
            .unwrap()
            .current = 100;
        let snapshot = state
            .transition(tick(
                1,
                vec![CoreCommand::TouchPickup {
                    player: 1,
                    pickup: 50,
                }],
            ))
            .unwrap();
        assert!(
            !state
                .player(1)
                .unwrap()
                .conditions
                .contains(ConditionId::BURNING)
        );
        assert_eq!(
            state.player(1).unwrap().health.as_ref().unwrap().current,
            125
        );
        assert!(
            snapshot
                .events
                .iter()
                .any(|event| matches!(event, CoreEvent::Pickup { .. }))
        );
    }

    #[test]
    fn every_selected_class_and_gameplay_team_spawns_exact_stock_state() {
        for class in PlayerClass::ALL {
            for team in [PlayerTeam::Red, PlayerTeam::Blue] {
                let mut state = CoreState::new(stock_schema());
                state.add_player(1).unwrap();
                let snapshot = state
                    .transition(tick(
                        0,
                        vec![
                            CoreCommand::SelectTeam { player: 1, team },
                            CoreCommand::SelectClass { player: 1, class },
                            CoreCommand::Spawn { player: 1 },
                        ],
                    ))
                    .unwrap();
                let player = &snapshot.players[0];
                assert_eq!(player.team, team);
                assert_eq!(player.class, Some(class));
                assert_eq!(
                    player.health.as_ref().unwrap().current,
                    class.data().maximum_health
                );
                assert_eq!(player.ammo, class.data().maximum_ammo);
                assert_eq!(
                    player
                        .loadout
                        .as_ref()
                        .unwrap()
                        .class_slots()
                        .iter()
                        .flatten()
                        .count(),
                    class.data().stock_items.len()
                );
            }
        }
    }

    #[test]
    fn command_failure_rolls_back_complete_state_and_journal() {
        let mut state = CoreState::new(stock_schema());
        state.add_player(1).unwrap();
        state
            .transition(tick(
                0,
                vec![
                    CoreCommand::SelectTeam {
                        player: 1,
                        team: PlayerTeam::Red,
                    },
                    CoreCommand::SelectClass {
                        player: 1,
                        class: PlayerClass::Scout,
                    },
                    CoreCommand::Spawn { player: 1 },
                ],
            ))
            .unwrap();
        let before = state.snapshot(Vec::new());
        let error = state.transition(tick(
            1,
            vec![
                CoreCommand::AddCondition {
                    player: 1,
                    condition: ConditionId::BURNING,
                    duration: ConditionDuration::Finite(5.0),
                    provider: Some(2),
                    competitive_summary_allowed: false,
                },
                CoreCommand::TouchPickup {
                    player: 1,
                    pickup: 999,
                },
            ],
        ));
        assert_eq!(error, Err(CoreError::MissingPickup(999)));
        assert_eq!(state.snapshot(Vec::new()), before);
    }

    #[test]
    fn equipped_item_provider_invalidates_queries_and_updates_health_maximum() {
        use crate::schema::{ItemAttribute, ItemAttributeValue, ItemInstanceInput, ItemQuality};

        let schema = stock_schema();
        let mut state = CoreState::new(schema.clone());
        state.add_player(1).unwrap();
        state
            .transition(tick(
                0,
                vec![
                    CoreCommand::SelectTeam {
                        player: 1,
                        team: PlayerTeam::Red,
                    },
                    CoreCommand::SelectClass {
                        player: 1,
                        class: PlayerClass::Scout,
                    },
                    CoreCommand::Spawn { player: 1 },
                ],
            ))
            .unwrap();
        let item = ItemInstance::create(
            &schema,
            ItemInstanceInput {
                item_id: 100,
                definition: 13,
                quality: Some(ItemQuality::Normal),
                level: 1,
                style: 0,
                paint: None,
                team: PlayerTeam::Red,
                class: PlayerClass::Scout,
                runtime_attributes: vec![
                    ItemAttribute {
                        definition: 26,
                        value: ItemAttributeValue::Numeric(50.0),
                    },
                    ItemAttribute {
                        definition: 526,
                        value: ItemAttributeValue::Numeric(0.5),
                    },
                ],
            },
        )
        .unwrap();
        state
            .transition(tick(
                1,
                vec![CoreCommand::EquipItem {
                    player: 1,
                    position: LoadoutPosition::Primary,
                    item,
                }],
            ))
            .unwrap();
        let player = state.player(1).unwrap();
        assert_eq!(player.health.as_ref().unwrap().maximum_for_buffing, 175);
        assert_eq!(player.health.as_ref().unwrap().maximum, 175);
        assert_eq!(player.health.as_ref().unwrap().current, 125);
        assert_eq!(player.item_attribute_entities.len(), 3);
        state
            .player_mut(1)
            .unwrap()
            .health
            .as_mut()
            .unwrap()
            .current = 100;
        state
            .transition(tick(
                2,
                vec![CoreCommand::ImmediateHeal {
                    target: 1,
                    healer: 2,
                    healer_team: PlayerTeam::Red,
                    amount: 20.0,
                    ignore_maximum: false,
                    ignore_team: false,
                }],
            ))
            .unwrap();
        assert_eq!(
            state.player(1).unwrap().health.as_ref().unwrap().current,
            110
        );
    }

    #[test]
    fn critical_checks_are_per_weapon_time_bound_and_journaled() {
        use crate::damage::{CritCheckInput, CritKind, WeaponCritKind};

        let mut state = CoreState::new(stock_schema());
        state.add_player(1).unwrap();
        state
            .transition(tick(
                0,
                vec![
                    CoreCommand::SelectTeam {
                        player: 1,
                        team: PlayerTeam::Red,
                    },
                    CoreCommand::SelectClass {
                        player: 1,
                        class: PlayerClass::Scout,
                    },
                    CoreCommand::Spawn { player: 1 },
                ],
            ))
            .unwrap();
        let input = CritCheckInput {
            now: 1.0,
            kind: WeaponCritKind::SingleShot,
            random_crits_enabled: false,
            can_fire_critical: true,
            guaranteed_critical: true,
            raw_damage: 10.0,
            projectiles_per_shot: 1.0,
            fire_delay: 0.5,
            player_crit_multiplier: 99.0,
            chance_multiplier: 99.0,
            roll: None,
        };
        let snapshot = state
            .transition(tick(
                1,
                vec![CoreCommand::CriticalCheck {
                    player: 1,
                    position: LoadoutPosition::Primary,
                    input,
                }],
            ))
            .unwrap();
        assert!(snapshot.events.iter().any(|event| {
            matches!(
                event,
                CoreEvent::CriticalCheck {
                    result: CritCheckResult {
                        kind: CritKind::Full,
                        random: false,
                        ..
                    },
                    ..
                }
            )
        }));
        assert_eq!(
            state
                .player(1)
                .unwrap()
                .crit
                .get(&LoadoutPosition::Primary)
                .unwrap()
                .token_bucket,
            crate::damage::CRIT_BUCKET_DEFAULT
        );
        assert_eq!(
            state.transition(tick(
                2,
                vec![CoreCommand::CriticalCheck {
                    player: 1,
                    position: LoadoutPosition::Primary,
                    input: CritCheckInput { now: 3.0, ..input },
                }],
            )),
            Err(CoreError::CritTimeMismatch)
        );
    }
}
