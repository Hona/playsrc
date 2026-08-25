use crate::{
    class::{AmmoLedger, AmmoType, PlayerClass, PlayerTeam},
    condition::{ConditionEvent, ConditionId, ConditionState},
    health::{HealthError, HealthState},
    schema::{ItemInstance, ItemSchema, Loadout, LoadoutPosition},
};

pub const MAP_PICKUP_RESPAWN_SECONDS: f32 = 10.0;
pub const DROPPED_PICKUP_LIFETIME_SECONDS: f32 = 30.0;
pub const MAX_DROPPED_WEAPONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PickupSize {
    Small = 0,
    Medium = 1,
    Full = 2,
}

impl PickupSize {
    pub const fn ratio(self) -> f32 {
        match self {
            Self::Small => 0.2,
            Self::Medium => 0.5,
            Self::Full => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DroppedAmmo {
    pub owner: u32,
    pub ratio: f32,
    pub primary: u16,
    pub secondary: u16,
    pub metal: u16,
    pub owner_pickup_allowed: bool,
}

impl DroppedAmmo {
    pub fn from_death(owner: u32, donor_ammo: AmmoLedger) -> Self {
        Self {
            owner,
            ratio: 0.5,
            primary: donor_ammo.primary.max(5),
            secondary: donor_ammo.secondary.max(5),
            metal: donor_ammo.metal.clamp(5, 100),
            owner_pickup_allowed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DroppedWeaponState {
    pub owner: u32,
    pub item: ItemInstance,
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

impl DroppedWeaponState {
    pub fn validate(&self) -> bool {
        self.energy.is_finite()
            && self.charge.is_finite()
            && self.effect_bar_regen_time.is_finite()
            && self.next_primary_time.is_finite()
            && self.next_secondary_time.is_finite()
            && self.meter.is_finite()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickupKind {
    Health(PickupSize),
    Ammo(PickupSize),
    DroppedAmmo(DroppedAmmo),
    DroppedWeapon(DroppedWeaponState),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupLifecycle {
    Available,
    Respawning { materialize_time: f32 },
    Removed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pickup {
    pub identity: u32,
    pub kind: PickupKind,
    pub team: Option<PlayerTeam>,
    pub disabled: bool,
    pub lifecycle: PickupLifecycle,
    pub creation_time: f32,
}

impl Pickup {
    pub fn map(
        identity: u32,
        kind: PickupKind,
        team: Option<PlayerTeam>,
        creation_time: f32,
    ) -> Result<Self, PickupError> {
        if !creation_time.is_finite()
            || team.is_some_and(|team| !team.is_gameplay())
            || matches!(
                kind,
                PickupKind::DroppedAmmo(_) | PickupKind::DroppedWeapon(_)
            )
        {
            return Err(PickupError::InvalidPickup);
        }
        Ok(Self {
            identity,
            kind,
            team,
            disabled: false,
            lifecycle: PickupLifecycle::Available,
            creation_time,
        })
    }

    pub fn dropped(
        identity: u32,
        kind: PickupKind,
        creation_time: f32,
    ) -> Result<Self, PickupError> {
        if !creation_time.is_finite()
            || !matches!(
                kind,
                PickupKind::DroppedAmmo(_) | PickupKind::DroppedWeapon(_)
            )
            || matches!(&kind, PickupKind::DroppedAmmo(ammo) if !ammo.ratio.is_finite() || ammo.ratio < 0.0)
            || matches!(&kind, PickupKind::DroppedWeapon(weapon) if !weapon.validate())
        {
            return Err(PickupError::InvalidPickup);
        }
        Ok(Self {
            identity,
            kind,
            team: None,
            disabled: false,
            lifecycle: PickupLifecycle::Available,
            creation_time,
        })
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn allow_owner_pickup(&mut self) {
        if let PickupKind::DroppedAmmo(ammo) = &mut self.kind {
            ammo.owner_pickup_allowed = true;
        }
    }

    pub fn advance(&mut self, now: f32) -> Result<Option<PickupEvent>, PickupError> {
        if !now.is_finite() {
            return Err(PickupError::NonFiniteInput);
        }
        if let PickupKind::DroppedAmmo(ammo) = &mut self.kind
            && now >= self.creation_time + 0.75
        {
            ammo.owner_pickup_allowed = true;
        }
        match self.lifecycle {
            PickupLifecycle::Respawning { materialize_time } if now >= materialize_time => {
                self.lifecycle = PickupLifecycle::Available;
                Ok(Some(PickupEvent::Materialized {
                    pickup: self.identity,
                }))
            }
            PickupLifecycle::Available
                if matches!(
                    self.kind,
                    PickupKind::DroppedAmmo(_) | PickupKind::DroppedWeapon(_)
                ) && now >= self.creation_time + DROPPED_PICKUP_LIFETIME_SECONDS =>
            {
                self.lifecycle = PickupLifecycle::Removed;
                Ok(Some(PickupEvent::Expired {
                    pickup: self.identity,
                }))
            }
            _ => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn touch(
        &mut self,
        now: f32,
        player: u32,
        class: PlayerClass,
        team: PlayerTeam,
        alive: bool,
        taunting: bool,
        invisible: bool,
        can_pickup_weapon: bool,
        health_from_packs_multiplier: f32,
        healing_received_multiplier: f32,
        schema: &ItemSchema,
        health: &mut HealthState,
        ammo: &mut AmmoLedger,
        conditions: &mut ConditionState,
        loadout: &mut Loadout,
    ) -> Result<PickupResult, PickupError> {
        if !now.is_finite()
            || !health_from_packs_multiplier.is_finite()
            || health_from_packs_multiplier < 0.0
            || !healing_received_multiplier.is_finite()
            || healing_received_multiplier < 0.0
        {
            return Err(PickupError::NonFiniteInput);
        }
        if self.lifecycle != PickupLifecycle::Available {
            return Ok(PickupResult::denied(
                self.identity,
                PickupDenial::Unavailable,
            ));
        }
        if self.disabled {
            return Ok(PickupResult::denied(self.identity, PickupDenial::Disabled));
        }
        if !alive || health.current <= 0 {
            return Ok(PickupResult::denied(self.identity, PickupDenial::Dead));
        }
        if self.team.is_some_and(|required| required != team) {
            return Ok(PickupResult::denied(self.identity, PickupDenial::WrongTeam));
        }

        let mut grants = Vec::new();
        let mut condition_events = Vec::new();
        let mut equipped = None;
        let consumed = match &self.kind {
            PickupKind::Health(size) => {
                let cleansed = [
                    ConditionId::BURNING,
                    ConditionId::BLEEDING,
                    ConditionId::PLAGUE,
                ]
                .into_iter()
                .filter(|condition| conditions.contains(*condition))
                .collect::<Vec<_>>();
                let requested =
                    (health.maximum as f32 * size.ratio()).ceil() * health_from_packs_multiplier;
                let applied = health
                    .take_health(requested, false, healing_received_multiplier, conditions)
                    .map_err(PickupError::Health)?;
                if applied > 0 {
                    grants.push(PickupGrant::Health(applied));
                }
                for condition in cleansed {
                    if let Some(event) = conditions.remove(condition, false) {
                        condition_events.push(event);
                    }
                }
                applied > 0 || !condition_events.is_empty()
            }
            PickupKind::Ammo(size) => {
                grant_map_ammo(class, *size, ammo, &mut grants);
                !grants.is_empty()
            }
            PickupKind::DroppedAmmo(dropped) => {
                if dropped.owner == player && !dropped.owner_pickup_allowed {
                    return Ok(PickupResult::denied(
                        self.identity,
                        PickupDenial::OwnerLocked,
                    ));
                }
                grant_dropped_ammo(class, dropped, ammo, &mut grants);
                !grants.is_empty()
            }
            PickupKind::DroppedWeapon(dropped) => {
                if taunting {
                    return Ok(PickupResult::denied(self.identity, PickupDenial::Taunting));
                }
                if class == PlayerClass::Spy
                    && (invisible || conditions.contains(ConditionId::DISGUISED))
                {
                    return Ok(PickupResult::denied(self.identity, PickupDenial::SpyHidden));
                }
                if !can_pickup_weapon {
                    return Ok(PickupResult::denied(
                        self.identity,
                        PickupDenial::ActiveWeaponBlocked,
                    ));
                }
                let Some(definition) = schema.definition(dropped.item.definition) else {
                    return Err(PickupError::InvalidDroppedWeapon);
                };
                if !definition.usable_by.contains(&class) {
                    return Ok(PickupResult::denied(
                        self.identity,
                        PickupDenial::ClassMismatch,
                    ));
                }
                if !matches!(
                    dropped.item.slot,
                    LoadoutPosition::Primary | LoadoutPosition::Secondary | LoadoutPosition::Melee
                ) {
                    return Ok(PickupResult::denied(
                        self.identity,
                        PickupDenial::ClassMismatch,
                    ));
                }
                if loadout.item(dropped.item.slot).is_none() {
                    return Ok(PickupResult::denied(
                        self.identity,
                        PickupDenial::NoReplacement,
                    ));
                }
                let mut picked_up = dropped.item.clone();
                picked_up.class = class;
                let replaced = loadout
                    .equip(picked_up.slot, picked_up.clone())
                    .map_err(|_| PickupError::InvalidDroppedWeapon)?;
                equipped = Some(WeaponPickup {
                    picked_up: DroppedWeaponState {
                        item: picked_up,
                        ..dropped.clone()
                    },
                    replaced: replaced.expect("replacement was checked"),
                });
                true
            }
        };
        if !consumed {
            return Ok(PickupResult::denied(self.identity, PickupDenial::Full));
        }
        self.lifecycle = if matches!(self.kind, PickupKind::Health(_) | PickupKind::Ammo(_)) {
            PickupLifecycle::Respawning {
                materialize_time: now + MAP_PICKUP_RESPAWN_SECONDS,
            }
        } else {
            PickupLifecycle::Removed
        };
        Ok(PickupResult {
            pickup: self.identity,
            consumed: true,
            denial: None,
            grants,
            condition_events,
            equipped,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickupDenial {
    Unavailable,
    Disabled,
    Dead,
    WrongTeam,
    OwnerLocked,
    Full,
    Taunting,
    SpyHidden,
    ActiveWeaponBlocked,
    ClassMismatch,
    NoReplacement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PickupGrant {
    Health(i32),
    Ammo { kind: AmmoType, amount: u16 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponPickup {
    pub picked_up: DroppedWeaponState,
    pub replaced: ItemInstance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickupResult {
    pub pickup: u32,
    pub consumed: bool,
    pub denial: Option<PickupDenial>,
    pub grants: Vec<PickupGrant>,
    pub condition_events: Vec<ConditionEvent>,
    pub equipped: Option<WeaponPickup>,
}

impl PickupResult {
    fn denied(pickup: u32, denial: PickupDenial) -> Self {
        Self {
            pickup,
            consumed: false,
            denial: Some(denial),
            grants: Vec::new(),
            condition_events: Vec::new(),
            equipped: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupEvent {
    Materialized { pickup: u32 },
    Expired { pickup: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PickupError {
    InvalidPickup,
    InvalidDroppedWeapon,
    NonFiniteInput,
    Health(HealthError),
}

pub fn retained_dropped_weapon_identities(pickups: &[Pickup]) -> Vec<u32> {
    let mut dropped: Vec<_> = pickups
        .iter()
        .filter(|pickup| {
            pickup.lifecycle != PickupLifecycle::Removed
                && matches!(pickup.kind, PickupKind::DroppedWeapon(_))
        })
        .collect();
    dropped.sort_by(|left, right| {
        left.creation_time
            .total_cmp(&right.creation_time)
            .then(left.identity.cmp(&right.identity))
    });
    dropped
        .into_iter()
        .rev()
        .take(MAX_DROPPED_WEAPONS + 1)
        .map(|pickup| pickup.identity)
        .collect()
}

fn grant_map_ammo(
    class: PlayerClass,
    size: PickupSize,
    ammo: &mut AmmoLedger,
    grants: &mut Vec<PickupGrant>,
) {
    let maximum = class.data().maximum_ammo;
    for kind in [AmmoType::Primary, AmmoType::Secondary, AmmoType::Metal] {
        grant_ammo(
            ammo,
            maximum,
            kind,
            (maximum.get(kind) as f32 * size.ratio()).ceil() as u16,
            grants,
        );
    }
    if class == PlayerClass::Engineer {
        grant_ammo(
            ammo,
            maximum,
            AmmoType::Grenades1,
            (maximum.grenades1 as f32 * size.ratio()).ceil() as u16,
            grants,
        );
    }
}

fn grant_dropped_ammo(
    class: PlayerClass,
    dropped: &DroppedAmmo,
    ammo: &mut AmmoLedger,
    grants: &mut Vec<PickupGrant>,
) {
    let maximum = class.data().maximum_ammo;
    grant_ammo(
        ammo,
        maximum,
        AmmoType::Primary,
        (maximum.primary as f32 * dropped.ratio).ceil() as u16,
        grants,
    );
    grant_ammo(
        ammo,
        maximum,
        AmmoType::Secondary,
        (maximum.secondary as f32 * dropped.ratio).ceil() as u16,
        grants,
    );
    grant_ammo(ammo, maximum, AmmoType::Metal, dropped.metal, grants);
    if class == PlayerClass::Engineer {
        grant_ammo(
            ammo,
            maximum,
            AmmoType::Grenades1,
            (maximum.grenades1 as f32 * dropped.ratio).ceil() as u16,
            grants,
        );
    }
}

fn grant_ammo(
    ammo: &mut AmmoLedger,
    maximum: AmmoLedger,
    kind: AmmoType,
    requested: u16,
    grants: &mut Vec<PickupGrant>,
) {
    let amount = ammo.give(maximum, kind, requested);
    if amount > 0 {
        grants.push(PickupGrant::Ammo { kind, amount });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        condition::ConditionDuration,
        schema::{
            ITEM_SCHEMA_SHA256, ITEM_SCHEMA_SIGNATURE_SHA256, ItemInstanceInput, ItemQuality,
            ItemSchema, LoadoutPosition, SchemaInput, SchemaNode,
        },
    };

    fn target(class: PlayerClass) -> (HealthState, AmmoLedger, ConditionState) {
        (
            HealthState::spawn(class, 0.0, 0.0).unwrap(),
            AmmoLedger::default(),
            ConditionState::default(),
        )
    }

    fn empty_loadout(class: PlayerClass) -> Loadout {
        Loadout::empty(class, 0).unwrap()
    }

    fn touch(
        pickup: &mut Pickup,
        class: PlayerClass,
        player: u32,
        health: &mut HealthState,
        ammo: &mut AmmoLedger,
        conditions: &mut ConditionState,
        loadout: &mut Loadout,
    ) -> PickupResult {
        pickup
            .touch(
                0.0,
                player,
                class,
                PlayerTeam::Red,
                true,
                false,
                false,
                true,
                1.0,
                1.0,
                &one_item_schema(),
                health,
                ammo,
                conditions,
                loadout,
            )
            .unwrap()
    }

    #[test]
    fn health_pack_size_full_and_condition_cleanup_matrix_is_exact() {
        for (size, expected) in [
            (PickupSize::Small, 25),
            (PickupSize::Medium, 63),
            (PickupSize::Full, 100),
        ] {
            let (mut health, mut ammo, mut conditions) = target(PlayerClass::Scout);
            health.current = 25;
            let mut pickup = Pickup::map(1, PickupKind::Health(size), None, 0.0).unwrap();
            let result = touch(
                &mut pickup,
                PlayerClass::Scout,
                1,
                &mut health,
                &mut ammo,
                &mut conditions,
                &mut empty_loadout(PlayerClass::Scout),
            );
            assert_eq!(result.grants, vec![PickupGrant::Health(expected.min(100))]);
            assert!(matches!(
                pickup.lifecycle,
                PickupLifecycle::Respawning {
                    materialize_time: 10.0
                }
            ));
        }
        let (mut health, mut ammo, mut conditions) = target(PlayerClass::Scout);
        for condition in [
            ConditionId::BURNING,
            ConditionId::BLEEDING,
            ConditionId::PLAGUE,
        ] {
            conditions
                .add(condition, ConditionDuration::Permanent, None, true, false)
                .unwrap();
        }
        let mut pickup = Pickup::map(2, PickupKind::Health(PickupSize::Small), None, 0.0).unwrap();
        let result = touch(
            &mut pickup,
            PlayerClass::Scout,
            1,
            &mut health,
            &mut ammo,
            &mut conditions,
            &mut empty_loadout(PlayerClass::Scout),
        );
        assert!(result.consumed);
        assert_eq!(result.condition_events.len(), 3);
    }

    #[test]
    fn map_ammo_size_and_full_denial_matrix_is_exact_for_every_class() {
        for class in PlayerClass::ALL {
            for size in [PickupSize::Small, PickupSize::Medium, PickupSize::Full] {
                let (mut health, mut ammo, mut conditions) = target(class);
                let mut pickup = Pickup::map(1, PickupKind::Ammo(size), None, 0.0).unwrap();
                let result = touch(
                    &mut pickup,
                    class,
                    1,
                    &mut health,
                    &mut ammo,
                    &mut conditions,
                    &mut empty_loadout(class),
                );
                assert!(result.consumed);
                assert_eq!(
                    ammo.primary,
                    (class.data().maximum_ammo.primary as f32 * size.ratio()).ceil() as u16
                );
            }
            let (mut health, _, mut conditions) = target(class);
            let mut ammo = class.data().maximum_ammo;
            let mut pickup = Pickup::map(1, PickupKind::Ammo(PickupSize::Full), None, 0.0).unwrap();
            let result = touch(
                &mut pickup,
                class,
                1,
                &mut health,
                &mut ammo,
                &mut conditions,
                &mut empty_loadout(class),
            );
            assert_eq!(result.denial, Some(PickupDenial::Full));
        }
    }

    #[test]
    fn dropped_ammo_uses_receiver_maximum_metal_contents_and_owner_lock() {
        let dropped = DroppedAmmo::from_death(
            1,
            AmmoLedger {
                primary: 1,
                secondary: 2,
                metal: 200,
                ..AmmoLedger::default()
            },
        );
        assert_eq!((dropped.primary, dropped.secondary), (5, 5));
        assert_eq!(dropped.metal, 100);
        let mut pickup = Pickup::dropped(1, PickupKind::DroppedAmmo(dropped), 0.0).unwrap();
        let (mut health, mut ammo, mut conditions) = target(PlayerClass::Soldier);
        let result = touch(
            &mut pickup,
            PlayerClass::Soldier,
            1,
            &mut health,
            &mut ammo,
            &mut conditions,
            &mut empty_loadout(PlayerClass::Soldier),
        );
        assert_eq!(result.denial, Some(PickupDenial::OwnerLocked));
        pickup.allow_owner_pickup();
        let result = touch(
            &mut pickup,
            PlayerClass::Soldier,
            1,
            &mut health,
            &mut ammo,
            &mut conditions,
            &mut empty_loadout(PlayerClass::Soldier),
        );
        assert!(result.consumed);
        assert_eq!((ammo.primary, ammo.secondary, ammo.metal), (10, 16, 100));
    }

    fn one_item_schema() -> ItemSchema {
        let item = |index: u32, class: &str| {
            SchemaNode::object(
                index.to_string(),
                vec![
                    SchemaNode::scalar("name", format!("item-{index}")),
                    SchemaNode::scalar("item_class", "tf_weapon"),
                    SchemaNode::scalar("item_slot", "primary"),
                    SchemaNode::scalar("item_quality", "normal"),
                    SchemaNode::scalar("min_ilevel", "1"),
                    SchemaNode::scalar("max_ilevel", "1"),
                    SchemaNode::object("used_by_classes", vec![SchemaNode::scalar(class, "1")]),
                ],
            )
        };
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
            attributes: Vec::new(),
            items: vec![
                SchemaNode::object(
                    "default",
                    vec![
                        SchemaNode::scalar("name", "default"),
                        SchemaNode::scalar("item_class", "tf_wearable"),
                        SchemaNode::scalar("item_slot", "melee"),
                    ],
                ),
                item(1, "soldier"),
                item(2, "soldier"),
            ],
        })
        .unwrap()
    }

    #[test]
    fn dropped_weapon_replaces_one_slot_and_retains_runtime_fields() {
        let schema = one_item_schema();
        let old = ItemInstance::create(
            &schema,
            ItemInstanceInput {
                item_id: 1,
                definition: 1,
                quality: Some(ItemQuality::Normal),
                level: 1,
                style: 0,
                paint: None,
                team: PlayerTeam::Red,
                class: PlayerClass::Soldier,
                runtime_attributes: Vec::new(),
            },
        )
        .unwrap();
        let new = ItemInstance::create(
            &schema,
            ItemInstanceInput {
                item_id: 2,
                definition: 2,
                quality: Some(ItemQuality::Normal),
                level: 1,
                style: 0,
                paint: None,
                team: PlayerTeam::Blue,
                class: PlayerClass::Soldier,
                runtime_attributes: Vec::new(),
            },
        )
        .unwrap();
        let mut loadout = Loadout::empty(PlayerClass::Soldier, 0).unwrap();
        loadout
            .equip(LoadoutPosition::Primary, old.clone())
            .unwrap();
        let state = DroppedWeaponState {
            owner: 2,
            item: new.clone(),
            clip: 3,
            reserve: 11,
            energy: 5.0,
            charge: 0.5,
            detonated: 2,
            effect_bar_regen_time: 4.0,
            next_primary_time: 6.0,
            next_secondary_time: 7.0,
            broken: true,
            body: 8,
            meter: 9.0,
        };
        let mut pickup = Pickup::dropped(1, PickupKind::DroppedWeapon(state.clone()), 0.0).unwrap();
        let (mut health, mut ammo, mut conditions) = target(PlayerClass::Soldier);
        let result = touch(
            &mut pickup,
            PlayerClass::Soldier,
            1,
            &mut health,
            &mut ammo,
            &mut conditions,
            &mut loadout,
        );
        assert_eq!(result.equipped.as_ref().unwrap().picked_up, state);
        assert_eq!(result.equipped.as_ref().unwrap().replaced, old);
        assert_eq!(loadout.item(LoadoutPosition::Primary), Some(&new));
    }

    #[test]
    fn respawn_expiry_and_dropped_weapon_limit_are_bounded() {
        let mut pickup = Pickup::map(1, PickupKind::Ammo(PickupSize::Small), None, 0.0).unwrap();
        pickup.lifecycle = PickupLifecycle::Respawning {
            materialize_time: 10.0,
        };
        assert!(pickup.advance(9.99).unwrap().is_none());
        assert_eq!(
            pickup.advance(10.0).unwrap(),
            Some(PickupEvent::Materialized { pickup: 1 })
        );

        let schema = one_item_schema();
        let item = ItemInstance::create(
            &schema,
            ItemInstanceInput {
                item_id: 1,
                definition: 1,
                quality: None,
                level: 1,
                style: 0,
                paint: None,
                team: PlayerTeam::Red,
                class: PlayerClass::Soldier,
                runtime_attributes: Vec::new(),
            },
        )
        .unwrap();
        let mut pickups = Vec::new();
        for identity in 0..34 {
            pickups.push(
                Pickup::dropped(
                    identity,
                    PickupKind::DroppedWeapon(DroppedWeaponState {
                        owner: 1,
                        item: item.clone(),
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
                    }),
                    identity as f32,
                )
                .unwrap(),
            );
        }
        let retained = retained_dropped_weapon_identities(&pickups);
        assert_eq!(retained.len(), 33);
        assert!(!retained.contains(&0));
    }

    #[test]
    fn disabled_team_dead_owner_materialize_and_expiry_boundaries_are_exact() {
        let schema = one_item_schema();
        let (mut health, mut ammo, mut conditions) = target(PlayerClass::Soldier);
        let mut loadout = empty_loadout(PlayerClass::Soldier);
        let mut pickup = Pickup::map(
            1,
            PickupKind::Ammo(PickupSize::Small),
            Some(PlayerTeam::Blue),
            0.0,
        )
        .unwrap();
        let result = pickup
            .touch(
                0.0,
                1,
                PlayerClass::Soldier,
                PlayerTeam::Red,
                true,
                false,
                false,
                true,
                1.0,
                1.0,
                &schema,
                &mut health,
                &mut ammo,
                &mut conditions,
                &mut loadout,
            )
            .unwrap();
        assert_eq!(result.denial, Some(PickupDenial::WrongTeam));
        pickup.team = Some(PlayerTeam::Red);
        pickup.set_disabled(true);
        let result = pickup
            .touch(
                0.0,
                1,
                PlayerClass::Soldier,
                PlayerTeam::Red,
                true,
                false,
                false,
                true,
                1.0,
                1.0,
                &schema,
                &mut health,
                &mut ammo,
                &mut conditions,
                &mut loadout,
            )
            .unwrap();
        assert_eq!(result.denial, Some(PickupDenial::Disabled));

        let mut dropped = Pickup::dropped(
            2,
            PickupKind::DroppedAmmo(DroppedAmmo::from_death(1, AmmoLedger::default())),
            0.0,
        )
        .unwrap();
        dropped.advance(0.749).unwrap();
        assert!(
            matches!(&dropped.kind, PickupKind::DroppedAmmo(ammo) if !ammo.owner_pickup_allowed)
        );
        dropped.advance(0.75).unwrap();
        assert!(
            matches!(&dropped.kind, PickupKind::DroppedAmmo(ammo) if ammo.owner_pickup_allowed)
        );
        assert!(dropped.advance(29.999).unwrap().is_none());
        assert_eq!(
            dropped.advance(30.0).unwrap(),
            Some(PickupEvent::Expired { pickup: 2 })
        );
    }
}
