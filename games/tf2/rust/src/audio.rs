#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundDefinition {
    RocketSingle,
    OriginalSingle,
    StickySingle,
    RocketExplosion,
    OriginalExplosion,
    StickyExplosion,
    ScattergunSingle,
    PistolSingle,
    BatMiss,
    BatHitFlesh,
    BatHitWorld,
    ScattergunReload,
    PistolReload,

    ShotgunSingle,
    ShotgunReload,
    ShovelMiss,
    ShovelHitFlesh,
    ShovelHitWorld,
    MinigunWindUp,
    MinigunWindDown,
    MinigunSpin,
    MinigunFire,
    FistMiss,
    FistHitWorld,
    FistHitFlesh,
    SniperSingle,
    SmgSingle,
    KukriMiss,
    KukriHitFlesh,
    KukriHitWorld,
    SmgReload,
    ShotgunEmpty,
    PistolEmpty,
    WrenchMiss,
    WrenchHitFlesh,
    WrenchHitWorld,
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
            Self::ScattergunSingle => "Weapon_Scatter_Gun.Single",
            Self::PistolSingle => "Weapon_Pistol.Single",
            Self::BatMiss => "Weapon_Bat.Miss",
            Self::BatHitFlesh => "Weapon_Bat.HitFlesh",
            Self::BatHitWorld => "Weapon_Bat.HitWorld",
            Self::ScattergunReload => "Weapon_Scatter_Gun.WorldReload",
            Self::PistolReload => "Weapon_Pistol.WorldReload",

            Self::ShotgunSingle => "Weapon_Shotgun.Single",
            Self::ShotgunReload => "Weapon_Shotgun.WorldReload",
            Self::ShovelMiss => "Weapon_Shovel.Miss",
            Self::ShovelHitFlesh => "Weapon_Shovel.HitFlesh",
            Self::ShovelHitWorld => "Weapon_Shovel.HitWorld",
            Self::MinigunWindUp => "Weapon_Minigun.WindUp",
            Self::MinigunWindDown => "Weapon_Minigun.WindDown",
            Self::MinigunSpin => "Weapon_Minigun.Spin",
            Self::MinigunFire => "Weapon_Minigun.Fire",
            Self::FistMiss => "Weapon_Fist.Miss",
            Self::FistHitWorld => "Weapon_Fist.HitWorld",
            Self::FistHitFlesh => "Weapon_Fist.HitFlesh",
            Self::ShotgunEmpty => "Weapon_Shotgun.Empty",
            Self::PistolEmpty => "Weapon_Pistol.ClipEmpty",
            Self::WrenchMiss => "Weapon_Wrench.Miss",
            Self::WrenchHitFlesh => "Weapon_Wrench.HitFlesh",
            Self::WrenchHitWorld => "Weapon_Wrench.HitWorld",
            Self::SniperSingle => "Weapon_SniperRifle.Single",
            Self::SmgSingle => "Weapon_SMG.Single",
            Self::KukriMiss => "Weapon_Machete.Miss",
            Self::KukriHitFlesh => "Weapon_Machete.HitFlesh",
            Self::KukriHitWorld => "Weapon_Machete.HitWorld",
            Self::SmgReload => "Weapon_SMG.WorldReload",
        }
    }

    pub(crate) const fn wave_count(self) -> u8 {
        match self {
            Self::RocketExplosion
            | Self::StickyExplosion
            | Self::ShovelHitFlesh
            | Self::FistHitFlesh
            | Self::KukriHitFlesh
            | Self::WrenchHitFlesh => 3,
            Self::BatHitWorld
            | Self::ShovelHitWorld
            | Self::FistMiss
            | Self::FistHitWorld
            | Self::KukriHitWorld => 2,

            Self::RocketSingle
            | Self::OriginalSingle
            | Self::StickySingle
            | Self::OriginalExplosion
            | Self::ScattergunSingle
            | Self::PistolSingle
            | Self::BatMiss
            | Self::BatHitFlesh
            | Self::ScattergunReload
            | Self::PistolReload
            | Self::ShotgunSingle
            | Self::ShotgunReload
            | Self::ShovelMiss
            | Self::MinigunWindUp
            | Self::MinigunWindDown
            | Self::MinigunSpin
            | Self::MinigunFire => 1,
            Self::SniperSingle
            | Self::SmgSingle
            | Self::KukriMiss
            | Self::SmgReload
            | Self::ShotgunEmpty
            | Self::PistolEmpty
            | Self::WrenchMiss
            | Self::WrenchHitWorld => 1,
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
    pub bat_hit_world_available: u8,

    pub shovel_hit_world_available: u8,
    pub shovel_hit_flesh_available: u8,
    pub fist_miss_available: u8,
    pub fist_hit_world_available: u8,
    pub fist_hit_flesh_available: u8,
    pub wrench_hit_flesh_available: u8,
    pub kukri_hit_flesh_available: u8,
    pub kukri_hit_world_available: u8,
}

#[derive(Clone, Copy)]
struct WaveCycle {
    available: u8,
    all: u8,
}

impl WaveCycle {
    const THREE: u8 = 0b111;
    const TWO: u8 = 0b011;

    const fn new(all: u8) -> Self {
        Self {
            available: all,
            all,
        }
    }

    fn available_count(&mut self) -> u8 {
        if self.available == 0 {
            self.available = self.all;
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
    bat_hit_world: WaveCycle,

    shovel_hit_world: WaveCycle,
    shovel_hit_flesh: WaveCycle,
    fist_miss: WaveCycle,
    fist_hit_world: WaveCycle,
    fist_hit_flesh: WaveCycle,
    wrench_hit_flesh: WaveCycle,
    kukri_hit_flesh: WaveCycle,
    kukri_hit_world: WaveCycle,
}

impl SoundSelection {
    pub(crate) const fn new() -> Self {
        Self {
            rocket_explosion: WaveCycle::new(WaveCycle::THREE),
            sticky_explosion: WaveCycle::new(WaveCycle::THREE),
            bat_hit_world: WaveCycle::new(WaveCycle::TWO),

            shovel_hit_world: WaveCycle::new(WaveCycle::TWO),
            shovel_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            fist_miss: WaveCycle::new(WaveCycle::TWO),
            fist_hit_world: WaveCycle::new(WaveCycle::TWO),
            fist_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            wrench_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            kukri_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            kukri_hit_world: WaveCycle::new(WaveCycle::TWO),
        }
    }

    pub(crate) fn state(self) -> SoundSelectionState {
        SoundSelectionState {
            rocket_explosion_available: self.rocket_explosion.available,
            sticky_explosion_available: self.sticky_explosion.available,
            bat_hit_world_available: self.bat_hit_world.available,

            shovel_hit_world_available: self.shovel_hit_world.available,
            shovel_hit_flesh_available: self.shovel_hit_flesh.available,
            fist_miss_available: self.fist_miss.available,
            fist_hit_world_available: self.fist_hit_world.available,
            fist_hit_flesh_available: self.fist_hit_flesh.available,
            wrench_hit_flesh_available: self.wrench_hit_flesh.available,
            kukri_hit_flesh_available: self.kukri_hit_flesh.available,
            kukri_hit_world_available: self.kukri_hit_world.available,
        }
    }

    pub(crate) fn restore(&mut self, state: SoundSelectionState) -> bool {
        if state.rocket_explosion_available & !WaveCycle::THREE != 0
            || state.sticky_explosion_available & !WaveCycle::THREE != 0
            || state.bat_hit_world_available & !WaveCycle::TWO != 0
            || state.shovel_hit_world_available & !WaveCycle::TWO != 0
            || state.shovel_hit_flesh_available & !WaveCycle::THREE != 0
            || state.fist_miss_available & !WaveCycle::TWO != 0
            || state.fist_hit_world_available & !WaveCycle::TWO != 0
            || state.fist_hit_flesh_available & !WaveCycle::THREE != 0
            || state.wrench_hit_flesh_available & !WaveCycle::THREE != 0
            || state.kukri_hit_flesh_available & !WaveCycle::THREE != 0
            || state.kukri_hit_world_available & !WaveCycle::TWO != 0
        {
            return false;
        }
        self.rocket_explosion.available = state.rocket_explosion_available;
        self.sticky_explosion.available = state.sticky_explosion_available;
        self.bat_hit_world.available = state.bat_hit_world_available;

        self.shovel_hit_world.available = state.shovel_hit_world_available;
        self.shovel_hit_flesh.available = state.shovel_hit_flesh_available;
        self.fist_miss.available = state.fist_miss_available;
        self.fist_hit_world.available = state.fist_hit_world_available;
        self.fist_hit_flesh.available = state.fist_hit_flesh_available;
        self.wrench_hit_flesh.available = state.wrench_hit_flesh_available;
        self.kukri_hit_flesh.available = state.kukri_hit_flesh_available;
        self.kukri_hit_world.available = state.kukri_hit_world_available;

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
            SoundDefinition::BatHitWorld => &mut self.bat_hit_world,

            SoundDefinition::ShovelHitWorld => &mut self.shovel_hit_world,
            SoundDefinition::ShovelHitFlesh => &mut self.shovel_hit_flesh,
            SoundDefinition::FistMiss => &mut self.fist_miss,
            SoundDefinition::FistHitWorld => &mut self.fist_hit_world,
            SoundDefinition::FistHitFlesh => &mut self.fist_hit_flesh,
            SoundDefinition::WrenchHitFlesh => &mut self.wrench_hit_flesh,
            SoundDefinition::KukriHitFlesh => &mut self.kukri_hit_flesh,
            SoundDefinition::KukriHitWorld => &mut self.kukri_hit_world,

            _ => unreachable!("only configured random-wave definitions have selection state"),
        }
    }
}
