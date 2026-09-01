macro_rules! native_sound_definitions {
    ($($name:ident = $code:literal, $identity:literal, $waves:literal;)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum SoundDefinition { Configured(u8), $($name,)* }
        impl SoundDefinition {
            pub const NATIVE: &'static [Self] = &[$(Self::$name,)*];
            pub const fn code(self) -> u8 { match self { Self::Configured(index) => 160 + index, $(Self::$name => $code,)* } }
            pub fn from_code(code: u8) -> Option<Self> { match code { $($code => Some(Self::$name),)* code if code >= 160 && usize::from(code - 160) < CONFIGURED_SOUNDS.len() => Some(Self::Configured(code - 160)), _ => None } }
            pub const fn identity(self) -> &'static str { match self { Self::Configured(index) => CONFIGURED_SOUNDS[index as usize].0, $(Self::$name => $identity,)* } }
            pub const fn wave_count(self) -> u8 { match self { Self::Configured(index) => CONFIGURED_SOUNDS[index as usize].2, $(Self::$name => $waves,)* } }
        }
    };
}

#[path = "audio_native.rs"]
mod native;
native::native_sounds!(native_sound_definitions);

include!("equipment-audio.generated.rs");

pub const FLAG_SOUNDS: &[SoundDefinition] = &[
    SoundDefinition::FlagEnemyStolen, SoundDefinition::FlagEnemyDropped,
    SoundDefinition::FlagEnemyCaptured, SoundDefinition::FlagEnemyReturned,
    SoundDefinition::FlagTeamStolen, SoundDefinition::FlagTeamDropped,
    SoundDefinition::FlagTeamCaptured, SoundDefinition::FlagTeamReturned, SoundDefinition::FlagSpawn,
];
pub const SOUND_PRECACHE_ABSENCES_PATH: &str = "playsrc/audio-precache-absences.txt";
pub const SOUND_PRECACHE_ABSENCES_HEADER: &str = "playsrc-audio-precache-absences-v1\n";

/// CTeamRoundTimer::Precache: setup and normal countdowns share the timer's
/// closure even on payload maps, which do not use the control-point runtime.
pub const TIMER_VOICE_SOUNDS: &[&str] = &[
    "Announcer.RoundEnds60seconds", "Announcer.RoundEnds30seconds", "Announcer.RoundEnds10seconds",
    "Announcer.RoundEnds5seconds", "Announcer.RoundEnds4seconds", "Announcer.RoundEnds3seconds",
    "Announcer.RoundEnds2seconds", "Announcer.RoundEnds1seconds",
    "Announcer.RoundBegins60Seconds", "Announcer.RoundBegins30Seconds", "Announcer.RoundBegins10Seconds",
    "Announcer.RoundBegins5Seconds", "Announcer.RoundBegins4Seconds", "Announcer.RoundBegins3Seconds",
    "Announcer.RoundBegins2Seconds", "Announcer.RoundBegins1Seconds",
    "Announcer.TimeAdded", "Announcer.TimeAddedForEnemy", "Announcer.TimeAwardedForTeam",
];
pub const TIMER_GENERAL_SOUNDS: &[&str] = &["Game.Overtime", "Game.YourTeamWon", "Game.YourTeamLost", "Game.Stalemate", "Ambient.Siren"];

#[cfg(test)]
mod native_tests {
    use super::*;
    #[test]
    fn native_codes_are_unique_and_preserve_medic_tail_order() {
        let mut codes = std::collections::BTreeSet::new();
        for definition in SoundDefinition::NATIVE {
            assert!(codes.insert(definition.code()));
            assert!(!(160..224).contains(&definition.code()));
            assert_eq!(SoundDefinition::from_code(definition.code()), Some(*definition));
            assert!((1..=16).contains(&definition.wave_count()));
        }
        assert_eq!(SoundDefinition::MedigunHealing.code(), 71);
        assert_eq!(SoundDefinition::MedigunCharged.code(), 73);
        assert_eq!(SoundDefinition::BonesawHitFlesh.code(), 74);
        assert_eq!(SoundDefinition::SyringeReload.code(), 76);
        assert_eq!(SoundDefinition::MinigunCritical.code(), 115);
        assert_eq!(SoundDefinition::HologramInterrupted.code(), 102);
    }
    #[test]
    fn configured_overrides_reuse_native_scripts_without_a_second_wave_cycle() {
        assert!(CONFIGURED_SOUNDS.len() <= 64);
        for (index, (name, _, _)) in CONFIGURED_SOUNDS.iter().enumerate() {
            assert!(SoundDefinition::NATIVE.iter().all(|definition| definition.identity() != *name));
            let definition = SoundDefinition::Configured(index as u8);
            assert_eq!(SoundDefinition::from_code(definition.code()), Some(definition));
        }
        for (_, code) in MELEE_CRITICAL_SOUNDS { assert!(SoundDefinition::from_code(*code).is_some()); }
    }

    #[test]
    fn payload_alerts_cycle_all_ten_waves_and_restore_without_truncation() {
        let mut sounds = SoundSelection::new();
        for definition in [SoundDefinition::CartWarning,SoundDefinition::CartFinalWarning] {
            for ordinal in 0..10 { assert_eq!(sounds.original_ordinal(definition,0,true),ordinal); }
            assert_eq!(sounds.available_count(definition),10);
            for ordinal in 0..7 { assert_eq!(sounds.original_ordinal(definition,0,true),ordinal); }
        }
        let state=sounds.state();
        assert_eq!(state.payload_warning_available,[0x380;2]);
        let mut restored=SoundSelection::new();
        assert!(restored.restore(state));
        assert_eq!(restored.original_ordinal(SoundDefinition::CartWarning,1,true),8);
        assert_eq!(restored.original_ordinal(SoundDefinition::CartFinalWarning,2,true),9);
        let mut invalid=state;invalid.payload_warning_available[0]=0x400;
        assert!(!restored.restore(invalid));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponSoundSlot { Single, Double, Burst, Reload, Special1, Special2, Special3, MeleeMiss, MeleeHit, MeleeHitWorld, Empty, Deploy }

impl WeaponSoundSlot {
    pub const fn key(self) -> &'static str { match self {
        Self::Single => "sound_single_shot", Self::Burst => "sound_burst", Self::Reload => "sound_reload",
        Self::Double => "sound_double_shot",
        Self::Special1 => "sound_special1", Self::Special2 => "sound_special2", Self::Special3 => "sound_special3",
        Self::MeleeMiss => "sound_melee_miss", Self::MeleeHit => "sound_melee_hit", Self::MeleeHitWorld => "sound_melee_hit_world",
        Self::Empty => "sound_empty", Self::Deploy => "sound_deploy",
    } }
}

impl SoundDefinition {
    /// Announcers and objective patches retain the map's existing precache gate.
    pub const fn map_scoped(self) -> bool {
        matches!(self, Self::FlagEnemyStolen | Self::FlagEnemyDropped | Self::FlagEnemyCaptured | Self::FlagEnemyReturned
            | Self::FlagTeamStolen | Self::FlagTeamDropped | Self::FlagTeamCaptured | Self::FlagTeamReturned | Self::FlagSpawn
            | Self::TeamWon | Self::TeamLost | Self::RoundEnds60 | Self::RoundEnds30 | Self::RoundEnds10
            | Self::RoundEnds5 | Self::RoundEnds4 | Self::RoundEnds3 | Self::RoundEnds2 | Self::RoundEnds1 | Self::Overtime
            | Self::PointSuccess | Self::PointFailure | Self::PointCaptured | Self::PointContested | Self::PointContestedNeutral
            | Self::PointEnabled | Self::RoundBegins5 | Self::RoundBegins4 | Self::RoundBegins3 | Self::RoundBegins2
            | Self::RoundBegins1 | Self::Stalemate | Self::CaptureWarn | Self::HologramStart | Self::HologramStop | Self::HologramMove | Self::HologramInterrupted
            | Self::RoundBegins60 | Self::RoundBegins30 | Self::RoundBegins10 | Self::RoundStartSiren | Self::TimeAdded | Self::TimeAddedForEnemy | Self::TimeAwardedForTeam | Self::EndRoundScored
            | Self::CartWarning | Self::CartFinalWarning | Self::CartAlarm | Self::CartAlarmSingle)
    }
    pub fn configured(name: &str) -> Option<Self> {
        Self::NATIVE.iter().find(|definition| definition.identity() == name).copied()
            .or_else(|| CONFIGURED_SOUNDS.iter().position(|(candidate, _, _)| *candidate == name).map(|index| Self::Configured(index as u8)))
    }

    pub(crate) fn physical_impact_volume(self) -> Option<f32> {
        match self {
            Self::DefaultImpactHard | Self::DefaultImpactSoft => Some(0.6),
            Self::GrenadeImpactHard | Self::GrenadeImpactSoft => Some(1.0),
            Self::SolidMetalImpactHard => Some(0.4),
            Self::SolidMetalImpactSoft => Some(0.6),
            Self::FleshImpactHard => Some(0.8),
            Self::FleshImpactSoft => Some(0.6),
            _ => None,
        }
    }

    pub fn equipment_override(self, definition: u32) -> Self {
        let key = match self {
            Self::GrenadeSingle => "sound_single_shot",
            Self::GrenadeCritical => "sound_burst",
            Self::GrenadeReload => "sound_reload",
            Self::GrenadeModeSwitch => "sound_special3",
            Self::BatMiss | Self::ShovelMiss | Self::KukriMiss | Self::FireAxeMiss | Self::WrenchMiss | Self::BottleMiss | Self::BonesawMiss | Self::KnifeMiss | Self::FistMiss => "sound_melee_miss",
            Self::BatHitFlesh | Self::ShovelHitFlesh | Self::KukriHitFlesh | Self::FireAxeHitFlesh | Self::WrenchHitFlesh | Self::BottleHitFlesh | Self::BonesawHitFlesh | Self::KnifeHitFlesh | Self::FistHitFlesh => "sound_melee_hit",
            Self::BatHitWorld | Self::ShovelHitWorld | Self::KukriHitWorld | Self::FireAxeHitWorld | Self::WrenchHitWorld | Self::BottleHitWorld | Self::BonesawHitWorld | Self::KnifeHitWorld | Self::FistHitWorld => "sound_melee_hit_world",
            Self::RocketSingle | Self::OriginalSingle | Self::StickySingle | Self::ScattergunSingle | Self::PistolSingle | Self::ShotgunSingle | Self::SniperSingle | Self::SmgSingle | Self::RevolverSingle | Self::SyringeSingle => "sound_single_shot",
            Self::MinigunFire => "sound_double_shot",
            Self::ScattergunCritical | Self::ShotgunCritical | Self::PistolCritical | Self::SmgCritical | Self::SniperCritical | Self::RevolverCritical | Self::MinigunCritical => "sound_burst",
            Self::MinigunEmpty => "sound_empty",
            Self::ScattergunReload | Self::PistolReload | Self::ShotgunReload | Self::SmgReload | Self::RevolverReload | Self::SyringeReload => "sound_reload",
            Self::MinigunWindUp => "sound_special1",
            Self::MinigunWindDown => "sound_special2",
            Self::MinigunSpin => "sound_special3",
            _ => return self,
        };
        self.item_sound(definition, key)
    }

    pub fn equipment_slot(self, definition: u32, slot: WeaponSoundSlot) -> Self { self.item_sound(definition, slot.key()) }

    fn item_sound(self, definition: u32, key: &str) -> Self {
        let Some(name) = crate::equipment::presentation(definition).and_then(|item| item.sound_overrides.iter().find(|(candidate, _)| *candidate == key)).map(|(_, name)| *name) else { return self; };
        if name == self.identity() { return self; }
        Self::configured(name).expect("generated sound closure")
    }

    pub fn melee_critical(definition: u32) -> Option<Self> {
        MELEE_CRITICAL_SOUNDS.iter().find(|(item, _)| *item == definition).and_then(|(_, code)| Self::from_code(*code))
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
    PhysicsImpact,
    PlayerFeedback,
    WeaponSingle,
    ExplosionSpecial1,
    ItemPickup,
    ItemMaterialize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioSourceKind {
    Entity,
    Projectile,
    World,
    LocalListener,
    ControlPoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioAction {
    Play,
    PlayAtPitch(f32),
    PlayAtVolume(f32),
    FadeIn(f32),
    FadeOut(f32),
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEvent {
    pub action: AudioAction,
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
    pub payload_warning_available: [u16; 2],
    pub configured_available: [u8; 64],
    pub projectile_unlock_available: [u8; 6],
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
    pub fire_axe_hit_world_available: u8,
    pub fire_axe_hit_flesh_available: u8,
    pub flag_enemy_stolen_available: u8,
    pub flag_enemy_dropped_available: u8,
    pub flag_enemy_captured_available: u8,
    pub flag_enemy_returned_available: u8,
    pub flag_team_dropped_available: u8,
    pub bottle_hit_flesh_available: u8,
    pub bottle_hit_world_available: u8,
    pub knife_hit_flesh_available: u8,
    pub bonesaw_hit_flesh_available: u8,
    pub bonesaw_hit_world_available: u8,
    pub overtime_available: u8,
    pub control_point_available: u16,
}

macro_rules! wave_cycle {
($name:ident, $mask:ty) => {
#[derive(Clone, Copy)]
struct $name { available: $mask, all: $mask }
impl $name {
    const fn new(all: $mask) -> Self {
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
        for ordinal in 0..<$mask>::BITS as u8 {
            let bit = (1 as $mask) << ordinal;
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
};
}
wave_cycle!(WaveCycle, u8);
wave_cycle!(WideWaveCycle, u16);
impl WaveCycle {
    const FOUR: u8 = 0b1111;
    const THREE: u8 = 0b111;
    const TWO: u8 = 0b011;
}

#[derive(Clone, Copy)]
pub(crate) struct SoundSelection {
    payload_warnings: [WideWaveCycle; 2],
    configured: [WaveCycle; 64],
    projectile_unlock: [WaveCycle; 6],
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
    fire_axe_hit_world: WaveCycle,
    fire_axe_hit_flesh: WaveCycle,
    flag_enemy_stolen: WaveCycle,
    flag_enemy_dropped: WaveCycle,
    flag_enemy_captured: WaveCycle,
    flag_enemy_returned: WaveCycle,
    flag_team_dropped: WaveCycle,
    bottle_hit_flesh: WaveCycle,
    bottle_hit_world: WaveCycle,
    knife_hit_flesh: WaveCycle,
    bonesaw_hit_flesh: WaveCycle,
    bonesaw_hit_world: WaveCycle,
    overtime: WaveCycle,
    control_point: [WaveCycle; 6],
}

impl SoundSelection {
    pub(crate) fn new() -> Self {
        Self {
            payload_warnings: [WideWaveCycle::new(0x3ff); 2],
            configured: std::array::from_fn(|index| WaveCycle::new(CONFIGURED_SOUNDS.get(index).map_or(0, |(_, _, count)| ((1_u16 << count) - 1) as u8))),
            projectile_unlock: [WaveCycle::new(WaveCycle::THREE), WaveCycle::new(WaveCycle::THREE),
                WaveCycle::new(WaveCycle::THREE), WaveCycle::new(WaveCycle::THREE),
                WaveCycle::new(WaveCycle::FOUR), WaveCycle::new(WaveCycle::THREE)],
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
            fire_axe_hit_world: WaveCycle::new(WaveCycle::TWO),
            fire_axe_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            flag_enemy_stolen: WaveCycle::new(WaveCycle::FOUR),
            flag_enemy_dropped: WaveCycle::new(WaveCycle::TWO),
            flag_enemy_captured: WaveCycle::new(WaveCycle::THREE),
            flag_enemy_returned: WaveCycle::new(WaveCycle::THREE),
            flag_team_dropped: WaveCycle::new(WaveCycle::TWO),
            bottle_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            bottle_hit_world: WaveCycle::new(WaveCycle::THREE),
            knife_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            bonesaw_hit_flesh: WaveCycle::new(WaveCycle::THREE),
            bonesaw_hit_world: WaveCycle::new(WaveCycle::TWO),
            overtime: WaveCycle::new(WaveCycle::FOUR),
            control_point: [WaveCycle::new(WaveCycle::TWO), WaveCycle::new(WaveCycle::THREE), WaveCycle::new(WaveCycle::TWO), WaveCycle::new(WaveCycle::FOUR), WaveCycle::new(WaveCycle::TWO), WaveCycle::new(WaveCycle::THREE)],
        }
    }

    pub(crate) fn state(self) -> SoundSelectionState {
        SoundSelectionState {
            payload_warning_available: self.payload_warnings.map(|cycle| cycle.available),
            configured_available: self.configured.map(|cycle| cycle.available),
            projectile_unlock_available: self.projectile_unlock.map(|cycle| cycle.available),
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
            fire_axe_hit_world_available: self.fire_axe_hit_world.available,
            fire_axe_hit_flesh_available: self.fire_axe_hit_flesh.available,
            flag_enemy_stolen_available: self.flag_enemy_stolen.available,
            flag_enemy_dropped_available: self.flag_enemy_dropped.available,
            flag_enemy_captured_available: self.flag_enemy_captured.available,
            flag_enemy_returned_available: self.flag_enemy_returned.available,
            flag_team_dropped_available: self.flag_team_dropped.available,
            bottle_hit_flesh_available: self.bottle_hit_flesh.available,
            bottle_hit_world_available: self.bottle_hit_world.available,
            knife_hit_flesh_available: self.knife_hit_flesh.available,
            bonesaw_hit_flesh_available: self.bonesaw_hit_flesh.available,
            bonesaw_hit_world_available: self.bonesaw_hit_world.available,
            overtime_available: self.overtime.available,
            control_point_available: u16::from(self.control_point[0].available) | (u16::from(self.control_point[1].available)<<2) | (u16::from(self.control_point[2].available)<<5) | (u16::from(self.control_point[3].available)<<7) | (u16::from(self.control_point[4].available)<<11) | (u16::from(self.control_point[5].available)<<13),
        }
    }

    pub(crate) fn restore(&mut self, state: SoundSelectionState) -> bool {
        if state.payload_warning_available.iter().any(|mask| mask & !0x3ff != 0) { return false; }
        if self.configured.iter().zip(state.configured_available).any(|(cycle, value)| value & !cycle.all != 0) { return false; }
        if state.projectile_unlock_available.into_iter().enumerate().any(|(index, available)|
            available & !(if index == 4 { WaveCycle::FOUR } else { WaveCycle::THREE }) != 0)
            || state.rocket_explosion_available & !WaveCycle::THREE != 0
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
            || state.fire_axe_hit_world_available & !WaveCycle::TWO != 0
            || state.fire_axe_hit_flesh_available & !WaveCycle::THREE != 0
            || state.flag_enemy_stolen_available & !WaveCycle::FOUR != 0
            || state.flag_enemy_dropped_available & !WaveCycle::TWO != 0
            || state.flag_enemy_captured_available & !WaveCycle::THREE != 0
            || state.flag_enemy_returned_available & !WaveCycle::THREE != 0
            || state.flag_team_dropped_available & !WaveCycle::TWO != 0
            || state.bottle_hit_flesh_available & !WaveCycle::THREE != 0
            || state.bottle_hit_world_available & !WaveCycle::THREE != 0
            || state.knife_hit_flesh_available & !WaveCycle::THREE != 0
            || state.bonesaw_hit_flesh_available & !WaveCycle::THREE != 0
            || state.bonesaw_hit_world_available & !WaveCycle::TWO != 0
            || state.overtime_available & !WaveCycle::FOUR != 0
        {
            return false;
        }
        for (cycle, mask) in self.payload_warnings.iter_mut().zip(state.payload_warning_available) { cycle.available = mask; }
        self.rocket_explosion.available = state.rocket_explosion_available;
        for (cycle, available) in self.projectile_unlock.iter_mut().zip(state.projectile_unlock_available) {
            cycle.available = available;
        }
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
        self.bottle_hit_flesh.available = state.bottle_hit_flesh_available;
        self.bottle_hit_world.available = state.bottle_hit_world_available;

        self.fire_axe_hit_world.available = state.fire_axe_hit_world_available;
        self.fire_axe_hit_flesh.available = state.fire_axe_hit_flesh_available;
        self.flag_enemy_stolen.available = state.flag_enemy_stolen_available;
        self.flag_enemy_dropped.available = state.flag_enemy_dropped_available;
        self.flag_enemy_captured.available = state.flag_enemy_captured_available;
        self.flag_enemy_returned.available = state.flag_enemy_returned_available;
        self.flag_team_dropped.available = state.flag_team_dropped_available;
        self.knife_hit_flesh.available = state.knife_hit_flesh_available;
        self.bonesaw_hit_flesh.available = state.bonesaw_hit_flesh_available;
        self.bonesaw_hit_world.available = state.bonesaw_hit_world_available;
        self.overtime.available = state.overtime_available;
        for (index, (shift, mask)) in [(0,3),(2,7),(5,3),(7,15),(11,3),(13,7)].into_iter().enumerate() {
            self.control_point[index].available = ((state.control_point_available >> shift) & mask) as u8;
        }
        for (cycle, value) in self.configured.iter_mut().zip(state.configured_available) { cycle.available = value; }
        true
    }

    pub(crate) fn available_count(&mut self, definition: SoundDefinition) -> u8 {
        if matches!(definition, SoundDefinition::CartWarning | SoundDefinition::CartFinalWarning) {
            return self.payload_warnings[usize::from(definition == SoundDefinition::CartFinalWarning)].available_count();
        }
        self.cycle(definition).available_count()
    }

    pub(crate) fn original_ordinal(
        &mut self,
        definition: SoundDefinition,
        rank: u8,
        consume: bool,
    ) -> u8 {
        if matches!(definition, SoundDefinition::CartWarning | SoundDefinition::CartFinalWarning) {
            return self.payload_warnings[usize::from(definition == SoundDefinition::CartFinalWarning)].original_ordinal(rank, consume);
        }
        self.cycle(definition).original_ordinal(rank, consume)
    }

    fn cycle(&mut self, definition: SoundDefinition) -> &mut WaveCycle {
        match definition {
            SoundDefinition::Configured(index) => &mut self.configured[index as usize],
            SoundDefinition::DirectHitExplosion => &mut self.projectile_unlock[0],
            SoundDefinition::BlackBoxExplosion => &mut self.projectile_unlock[1],
            SoundDefinition::AirStrikeSingle => &mut self.projectile_unlock[2],
            SoundDefinition::AirStrikeExplosion => &mut self.projectile_unlock[3],
            SoundDefinition::FlareExplosion => &mut self.projectile_unlock[4],
            SoundDefinition::FlarePlayerImpact => &mut self.projectile_unlock[5],
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
            SoundDefinition::BottleHitFlesh => &mut self.bottle_hit_flesh,
            SoundDefinition::BottleHitWorld => &mut self.bottle_hit_world,

            SoundDefinition::FireAxeHitWorld => &mut self.fire_axe_hit_world,
            SoundDefinition::FireAxeHitFlesh => &mut self.fire_axe_hit_flesh,
            SoundDefinition::FlagEnemyStolen => &mut self.flag_enemy_stolen,
            SoundDefinition::FlagEnemyDropped => &mut self.flag_enemy_dropped,
            SoundDefinition::FlagEnemyCaptured => &mut self.flag_enemy_captured,
            SoundDefinition::FlagEnemyReturned => &mut self.flag_enemy_returned,
            SoundDefinition::FlagTeamDropped => &mut self.flag_team_dropped,
            SoundDefinition::KnifeHitFlesh => &mut self.knife_hit_flesh,
            SoundDefinition::BonesawHitFlesh => &mut self.bonesaw_hit_flesh,
            SoundDefinition::BonesawHitWorld => &mut self.bonesaw_hit_world,
            SoundDefinition::Overtime => &mut self.overtime,
            SoundDefinition::PointSuccess => &mut self.control_point[0],
            SoundDefinition::PointContested => &mut self.control_point[1],
            SoundDefinition::PointContestedNeutral => &mut self.control_point[2],
            SoundDefinition::PointEnabled => &mut self.control_point[3],
            SoundDefinition::CaptureWarn => &mut self.control_point[4],
            SoundDefinition::TimeAwardedForTeam => &mut self.control_point[5],
            _ => unreachable!("only configured random-wave definitions have selection state"),
        }
    }
}
