#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundDefinition {
    RocketSingle,
    OriginalSingle,
    StickySingle,
    RocketExplosion,
    OriginalExplosion,
    StickyExplosion,
    MinigunWindUp,
    MinigunWindDown,
    MinigunSpin,
    MinigunFire,
    ShotgunSingle,
    FistMiss,
    FistHitWorld,
    FistHitFlesh,
}

impl SoundDefinition {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::RocketSingle => "Weapon_RPG.Single",
            Self::OriginalSingle => "Weapon_QuakeRPG.Single",
            Self::StickySingle => "Weapon_StickyBombLauncher.Single",
            Self::RocketExplosion => "BaseExplosionEffect.Sound",
            Self::OriginalExplosion => "Weapon_QuakeRPG.Explode",
            Self::StickyExplosion => "Weapon_Grenade_Pipebomb.Explode",
            Self::MinigunWindUp => "Weapon_Minigun.WindUp",
            Self::MinigunWindDown => "Weapon_Minigun.WindDown",
            Self::MinigunSpin => "Weapon_Minigun.Spin",
            Self::MinigunFire => "Weapon_Minigun.Fire",
            Self::ShotgunSingle => "Weapon_Shotgun.Single",
            Self::FistMiss => "Weapon_Fist.Miss",
            Self::FistHitWorld => "Weapon_Fist.HitWorld",
            Self::FistHitFlesh => "Weapon_Fist.HitFlesh",
        }
    }

    pub(crate) const fn wave_count(self) -> u8 {
        match self {
            Self::RocketExplosion | Self::StickyExplosion | Self::FistHitFlesh => 3,
            Self::FistMiss | Self::FistHitWorld => 2,
            Self::RocketSingle
            | Self::OriginalSingle
            | Self::StickySingle
            | Self::OriginalExplosion
            | Self::MinigunWindUp
            | Self::MinigunWindDown
            | Self::MinigunSpin
            | Self::MinigunFire
            | Self::ShotgunSingle => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundQueryPhase {
    Inspect,
    Emit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundSamples {
    pub volume: f32,
    pub pitch: f32,
    pub wave: u8,
    pub sound_level: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioEventIdentity {
    WeaponSingle,
    ExplosionSpecial1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioSourceKind {
    Entity,
    World,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEvent {
    pub tick: u64,
    pub ordinal: u16,
    pub identity: AudioEventIdentity,
    pub definition: SoundDefinition,
    pub source_kind: AudioSourceKind,
    pub source_identity: u32,
    pub owner_identity: Option<u32>,
    pub position: [f32; 3],
    pub samples: SoundSamples,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SoundSelectionState {
    pub rocket_explosion_available: u8,
    pub sticky_explosion_available: u8,
    pub fist_miss_available: u8,
    pub fist_hit_world_available: u8,
    pub fist_hit_flesh_available: u8,
}

#[derive(Clone, Copy)]
struct WaveCycle {
    available: u8,
    mask: u8,
}

impl WaveCycle {
    const ALL: u8 = 0b111;

    const fn new(mask: u8) -> Self {
        Self {
            available: mask,
            mask,
        }
    }

    fn available_count(&mut self) -> u8 {
        if self.available == 0 {
            self.available = self.mask;
        }
        self.available.count_ones() as u8
    }

    fn original_ordinal(&mut self, rank: u8, consume: bool) -> u8 {
        self.available_count();
        let mut remaining = rank;
        for ordinal in 0..3 {
            let bit = 1_u8 << ordinal;
            if self.available & bit == 0 {
                continue;
            }
            if remaining == 0 {
                if consume {
                    self.available &= !bit;
                }
                return ordinal;
            }
            remaining -= 1;
        }
        unreachable!("rank came from the available wave count")
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SoundSelection {
    rocket_explosion: WaveCycle,
    sticky_explosion: WaveCycle,
    fist_miss: WaveCycle,
    fist_hit_world: WaveCycle,
    fist_hit_flesh: WaveCycle,
}

impl SoundSelection {
    pub(crate) const fn new() -> Self {
        Self {
            rocket_explosion: WaveCycle::new(0b111),
            sticky_explosion: WaveCycle::new(0b111),
            fist_miss: WaveCycle::new(0b11),
            fist_hit_world: WaveCycle::new(0b11),
            fist_hit_flesh: WaveCycle::new(0b111),
        }
    }

    pub(crate) fn state(self) -> SoundSelectionState {
        SoundSelectionState {
            rocket_explosion_available: self.rocket_explosion.available,
            sticky_explosion_available: self.sticky_explosion.available,
            fist_miss_available: self.fist_miss.available,
            fist_hit_world_available: self.fist_hit_world.available,
            fist_hit_flesh_available: self.fist_hit_flesh.available,
        }
    }

    pub(crate) fn restore(&mut self, state: SoundSelectionState) -> bool {
        if state.rocket_explosion_available & !WaveCycle::ALL != 0
            || state.sticky_explosion_available & !WaveCycle::ALL != 0
            || state.fist_miss_available & !0b11 != 0
            || state.fist_hit_world_available & !0b11 != 0
            || state.fist_hit_flesh_available & !WaveCycle::ALL != 0
        {
            return false;
        }
        self.rocket_explosion.available = state.rocket_explosion_available;
        self.sticky_explosion.available = state.sticky_explosion_available;
        self.fist_miss.available = state.fist_miss_available;
        self.fist_hit_world.available = state.fist_hit_world_available;
        self.fist_hit_flesh.available = state.fist_hit_flesh_available;
        true
    }

    pub(crate) fn available_count(&mut self, definition: SoundDefinition) -> u8 {
        self.cycle(definition).available_count()
    }

    pub(crate) fn original_ordinal(
        &mut self,
        definition: SoundDefinition,
        rank: u8,
        consume: bool,
    ) -> u8 {
        self.cycle(definition).original_ordinal(rank, consume)
    }

    fn cycle(&mut self, definition: SoundDefinition) -> &mut WaveCycle {
        match definition {
            SoundDefinition::RocketExplosion => &mut self.rocket_explosion,
            SoundDefinition::StickyExplosion => &mut self.sticky_explosion,
            SoundDefinition::FistMiss => &mut self.fist_miss,
            SoundDefinition::FistHitWorld => &mut self.fist_hit_world,
            SoundDefinition::FistHitFlesh => &mut self.fist_hit_flesh,
            _ => unreachable!("only configured random-wave definitions have selection state"),
        }
    }
}
