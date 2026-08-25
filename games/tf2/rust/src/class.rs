use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PlayerClass {
    Scout = 1,
    Sniper = 2,
    Soldier = 3,
    Demoman = 4,
    Medic = 5,
    Heavy = 6,
    Pyro = 7,
    Spy = 8,
    Engineer = 9,
}

impl PlayerClass {
    pub const ALL: [Self; 9] = [
        Self::Scout,
        Self::Sniper,
        Self::Soldier,
        Self::Demoman,
        Self::Medic,
        Self::Heavy,
        Self::Pyro,
        Self::Spy,
        Self::Engineer,
    ];

    pub const fn source_number(self) -> u8 {
        self as u8
    }

    pub const fn data(self) -> &'static ClassData {
        &CLASS_DATA[self as usize - 1]
    }

    pub const fn standing_eye_height(self) -> f32 {
        match self {
            Self::Scout => 65.0,
            Self::Sniper | Self::Medic | Self::Heavy | Self::Spy => 75.0,
            Self::Soldier | Self::Demoman | Self::Pyro | Self::Engineer => 68.0,
        }
    }
}

impl TryFrom<u8> for PlayerClass {
    type Error = ClassIdentityError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|class| *class as u8 == value)
            .ok_or(ClassIdentityError(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassIdentityError(pub u8);

impl fmt::Display for ClassIdentityError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "class {} is not one of the nine playable TF2 classes",
            self.0
        )
    }
}

impl std::error::Error for ClassIdentityError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PlayerTeam {
    Unassigned = 0,
    Spectator = 1,
    Red = 2,
    Blue = 3,
}

impl PlayerTeam {
    pub const fn source_number(self) -> u8 {
        self as u8
    }

    pub const fn is_gameplay(self) -> bool {
        matches!(self, Self::Red | Self::Blue)
    }

    pub const fn is_enemy(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Red, Self::Blue) | (Self::Blue, Self::Red)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AmmoType {
    Primary = 1,
    Secondary = 2,
    Metal = 3,
    Grenades1 = 4,
    Grenades2 = 5,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AmmoLedger {
    pub primary: u16,
    pub secondary: u16,
    pub metal: u16,
    pub grenades1: u16,
    pub grenades2: u16,
}

impl AmmoLedger {
    pub const fn get(self, kind: AmmoType) -> u16 {
        match kind {
            AmmoType::Primary => self.primary,
            AmmoType::Secondary => self.secondary,
            AmmoType::Metal => self.metal,
            AmmoType::Grenades1 => self.grenades1,
            AmmoType::Grenades2 => self.grenades2,
        }
    }

    pub fn set(&mut self, kind: AmmoType, value: u16) {
        match kind {
            AmmoType::Primary => self.primary = value,
            AmmoType::Secondary => self.secondary = value,
            AmmoType::Metal => self.metal = value,
            AmmoType::Grenades1 => self.grenades1 = value,
            AmmoType::Grenades2 => self.grenades2 = value,
        }
    }

    pub fn give(&mut self, maximum: Self, kind: AmmoType, requested: u16) -> u16 {
        let before = self.get(kind);
        let after = before.saturating_add(requested).min(maximum.get(kind));
        self.set(kind, after);
        after - before
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Buildable {
    Sentry,
    Dispenser,
    Teleporter,
    Sapper,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StockItem {
    pub definition: u32,
    pub slot: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassData {
    pub identity: &'static str,
    pub source_name: &'static str,
    pub class_record_name: &'static str,
    pub model: &'static str,
    pub hwm_model: &'static str,
    pub hand_model: &'static str,
    pub localization: &'static str,
    pub maximum_speed: f32,
    pub maximum_health: i32,
    pub maximum_armor: i32,
    pub maximum_ammo: AmmoLedger,
    pub weapon_declarations: &'static [&'static str],
    pub grenade_declarations: [&'static str; 2],
    pub stock_items: &'static [StockItem],
    pub buildables: &'static [Buildable],
    pub disable_airwalk: bool,
    pub disable_new_jump: bool,
    pub third_person_camera: [f32; 3],
    pub death_sounds: [&'static str; 4],
    pub content_sha256: &'static str,
}

const SCOUT_WEAPONS: &[&str] = &[
    "TF_WEAPON_BAT",
    "TF_WEAPON_PISTOL_SCOUT",
    "TF_WEAPON_SCATTERGUN",
];
const SNIPER_WEAPONS: &[&str] = &["TF_WEAPON_CLUB", "TF_WEAPON_SMG", "TF_WEAPON_SNIPERRIFLE"];
const SOLDIER_WEAPONS: &[&str] = &[
    "TF_WEAPON_SHOVEL",
    "TF_WEAPON_SHOTGUN_SOLDIER",
    "TF_WEAPON_ROCKETLAUNCHER",
];
const DEMOMAN_WEAPONS: &[&str] = &[
    "TF_WEAPON_BOTTLE",
    "TF_WEAPON_GRENADELAUNCHER",
    "TF_WEAPON_PIPEBOMBLAUNCHER",
];
const MEDIC_WEAPONS: &[&str] = &[
    "TF_WEAPON_BONESAW",
    "TF_WEAPON_MEDIGUN",
    "TF_WEAPON_SYRINGEGUN_MEDIC",
];
const HEAVY_WEAPONS: &[&str] = &[
    "TF_WEAPON_FISTS",
    "TF_WEAPON_SHOTGUN_HWG",
    "TF_WEAPON_MINIGUN",
];
const PYRO_WEAPONS: &[&str] = &[
    "TF_WEAPON_FIREAXE",
    "TF_WEAPON_SHOTGUN_PYRO",
    "TF_WEAPON_FLAMETHROWER",
];
const SPY_WEAPONS: &[&str] = &[
    "TF_WEAPON_KNIFE",
    "TF_WEAPON_REVOLVER",
    "TF_WEAPON_PDA_SPY",
    "TF_WEAPON_INVIS",
];
const ENGINEER_WEAPONS: &[&str] = &[
    "TF_WEAPON_WRENCH",
    "TF_WEAPON_PISTOL",
    "TF_WEAPON_SHOTGUN_PRIMARY",
    "TF_WEAPON_PDA_ENGINEER_BUILD",
    "TF_WEAPON_PDA_ENGINEER_DESTROY",
];

const SCOUT_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 13,
        slot: 0,
    },
    StockItem {
        definition: 23,
        slot: 1,
    },
    StockItem {
        definition: 0,
        slot: 2,
    },
];
const SNIPER_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 14,
        slot: 0,
    },
    StockItem {
        definition: 16,
        slot: 1,
    },
    StockItem {
        definition: 3,
        slot: 2,
    },
];
const SOLDIER_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 18,
        slot: 0,
    },
    StockItem {
        definition: 10,
        slot: 1,
    },
    StockItem {
        definition: 6,
        slot: 2,
    },
];
const DEMOMAN_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 19,
        slot: 0,
    },
    StockItem {
        definition: 20,
        slot: 1,
    },
    StockItem {
        definition: 1,
        slot: 2,
    },
];
const MEDIC_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 17,
        slot: 0,
    },
    StockItem {
        definition: 29,
        slot: 1,
    },
    StockItem {
        definition: 8,
        slot: 2,
    },
];
const HEAVY_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 15,
        slot: 0,
    },
    StockItem {
        definition: 11,
        slot: 1,
    },
    StockItem {
        definition: 5,
        slot: 2,
    },
];
const PYRO_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 21,
        slot: 0,
    },
    StockItem {
        definition: 12,
        slot: 1,
    },
    StockItem {
        definition: 2,
        slot: 2,
    },
];
const SPY_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 24,
        slot: 1,
    },
    StockItem {
        definition: 4,
        slot: 2,
    },
    StockItem {
        definition: 735,
        slot: 4,
    },
    StockItem {
        definition: 27,
        slot: 5,
    },
    StockItem {
        definition: 30,
        slot: 6,
    },
];
const ENGINEER_ITEMS: &[StockItem] = &[
    StockItem {
        definition: 9,
        slot: 0,
    },
    StockItem {
        definition: 22,
        slot: 1,
    },
    StockItem {
        definition: 7,
        slot: 2,
    },
    StockItem {
        definition: 28,
        slot: 4,
    },
    StockItem {
        definition: 25,
        slot: 5,
    },
    StockItem {
        definition: 26,
        slot: 6,
    },
];

const NO_BUILDABLES: &[Buildable] = &[];
const SPY_BUILDABLES: &[Buildable] = &[Buildable::Sapper];
const ENGINEER_BUILDABLES: &[Buildable] = &[
    Buildable::Sentry,
    Buildable::Dispenser,
    Buildable::Teleporter,
];

pub const CLASS_DATA: [ClassData; 9] = [
    ClassData {
        identity: "class.scout",
        source_name: "scout",
        class_record_name: "scout",
        model: "models/player/scout.mdl",
        hwm_model: "models/player/hwm/scout.mdl",
        hand_model: "models/weapons/c_models/c_scout_arms.mdl",
        localization: "TF_Class_Name_Scout",
        maximum_speed: 400.0,
        maximum_health: 125,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 32,
            secondary: 36,
            metal: 100,
            grenades1: 1,
            grenades2: 1,
        },
        weapon_declarations: SCOUT_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_CALTROP", "TF_WEAPON_GRENADE_CONCUSSION"],
        stock_items: SCOUT_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Scout.Death",
            "Scout.CritDeath",
            "Scout.MeleeDeath",
            "Scout.ExplosionDeath",
        ],
        content_sha256: "f84dd59305afe06e9198a31f4b2f37ee6a06cc91e3e610f8e5ec6a5e8024979b",
    },
    ClassData {
        identity: "class.sniper",
        source_name: "sniper",
        class_record_name: "sniper",
        model: "models/player/sniper.mdl",
        hwm_model: "models/player/hwm/sniper.mdl",
        hand_model: "models/weapons/c_models/c_sniper_arms.mdl",
        localization: "TF_Class_Name_Sniper",
        maximum_speed: 300.0,
        maximum_health: 125,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 25,
            secondary: 75,
            metal: 100,
            grenades1: 1,
            grenades2: 0,
        },
        weapon_declarations: SNIPER_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", ""],
        stock_items: SNIPER_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Sniper.Death",
            "Sniper.CritDeath",
            "Sniper.MeleeDeath",
            "Sniper.ExplosionDeath",
        ],
        content_sha256: "f6cfd1320f8033abdd6dba1f32072a465fd198c1305e1f297b53a702337dd1ed",
    },
    ClassData {
        identity: "class.soldier",
        source_name: "soldier",
        class_record_name: "soldier",
        model: "models/player/soldier.mdl",
        hwm_model: "models/player/hwm/soldier.mdl",
        hand_model: "models/weapons/c_models/c_soldier_arms.mdl",
        localization: "TF_Class_Name_Soldier",
        maximum_speed: 240.0,
        maximum_health: 200,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 20,
            secondary: 32,
            metal: 100,
            grenades1: 1,
            grenades2: 1,
        },
        weapon_declarations: SOLDIER_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", "TF_WEAPON_GRENADE_NAIL"],
        stock_items: SOLDIER_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Soldier.Death",
            "Soldier.CritDeath",
            "Soldier.MeleeDeath",
            "Soldier.ExplosionDeath",
        ],
        content_sha256: "3cae1b6da09c5ef26e04cc619bc4317b47bdcb4df838e50399cb10e7e078abb1",
    },
    ClassData {
        identity: "class.demoman",
        source_name: "demoman",
        class_record_name: "demoman",
        model: "models/player/demo.mdl",
        hwm_model: "models/player/hwm/demo.mdl",
        hand_model: "models/weapons/c_models/c_demo_arms.mdl",
        localization: "TF_Class_Name_Demoman",
        maximum_speed: 280.0,
        maximum_health: 175,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 16,
            secondary: 24,
            metal: 100,
            grenades1: 1,
            grenades2: 1,
        },
        weapon_declarations: DEMOMAN_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", "TF_WEAPON_GRENADE_MIRV_DEMOMAN"],
        stock_items: DEMOMAN_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Demoman.Death",
            "Demoman.CritDeath",
            "Demoman.MeleeDeath",
            "Demoman.ExplosionDeath",
        ],
        content_sha256: "28eb6c32971f16a52991327c2238e1993a22d9a9805c7ad1ec9282d36f538a4d",
    },
    ClassData {
        identity: "class.medic",
        source_name: "medic",
        class_record_name: "medic",
        model: "models/player/medic.mdl",
        hwm_model: "models/player/hwm/medic.mdl",
        hand_model: "models/weapons/c_models/c_medic_arms.mdl",
        localization: "TF_Class_Name_Medic",
        maximum_speed: 320.0,
        maximum_health: 150,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 150,
            secondary: 150,
            metal: 100,
            grenades1: 0,
            grenades2: 0,
        },
        weapon_declarations: MEDIC_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", "TF_WEAPON_GRENADE_HEAL"],
        stock_items: MEDIC_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Medic.Death",
            "Medic.CritDeath",
            "Medic.MeleeDeath",
            "Medic.ExplosionDeath",
        ],
        content_sha256: "fe3cfdd879543984816530a67c3d5854f379b14fa8273dbb62c20f7d62922769",
    },
    ClassData {
        identity: "class.heavy",
        source_name: "heavy",
        class_record_name: "heavyweapons",
        model: "models/player/heavy.mdl",
        hwm_model: "models/player/hwm/heavy.mdl",
        hand_model: "models/weapons/c_models/c_heavy_arms.mdl",
        localization: "TF_Class_Name_HWGuy",
        maximum_speed: 230.0,
        maximum_health: 300,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 200,
            secondary: 32,
            metal: 100,
            grenades1: 1,
            grenades2: 1,
        },
        weapon_declarations: HEAVY_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", "TF_WEAPON_GRENADE_MIRV"],
        stock_items: HEAVY_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Heavy.Death",
            "Heavy.CritDeath",
            "Heavy.MeleeDeath",
            "Heavy.ExplosionDeath",
        ],
        content_sha256: "3ee9abad4c25176a1922c9107ff403f9ab8bcede9a3d75810e9ffb703b79ac59",
    },
    ClassData {
        identity: "class.pyro",
        source_name: "pyro",
        class_record_name: "pyro",
        model: "models/player/pyro.mdl",
        hwm_model: "models/player/hwm/pyro.mdl",
        hand_model: "models/weapons/c_models/c_pyro_arms.mdl",
        localization: "TF_Class_Name_Pyro",
        maximum_speed: 300.0,
        maximum_health: 175,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 200,
            secondary: 32,
            metal: 100,
            grenades1: 1,
            grenades2: 0,
        },
        weapon_declarations: PYRO_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", ""],
        stock_items: PYRO_ITEMS,
        buildables: NO_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Pyro.Death",
            "Pyro.CritDeath",
            "Pyro.MeleeDeath",
            "Pyro.ExplosionDeath",
        ],
        content_sha256: "fa20e5afacbde10379d89326ccca144c0652594f36c017176c6125feac06ab0f",
    },
    ClassData {
        identity: "class.spy",
        source_name: "spy",
        class_record_name: "spy",
        model: "models/player/spy.mdl",
        hwm_model: "models/player/hwm/spy.mdl",
        hand_model: "models/weapons/c_models/c_spy_arms.mdl",
        localization: "TF_Class_Name_Spy",
        maximum_speed: 320.0,
        maximum_health: 125,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 20,
            secondary: 24,
            metal: 100,
            grenades1: 0,
            grenades2: 1,
        },
        weapon_declarations: SPY_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", ""],
        stock_items: SPY_ITEMS,
        buildables: SPY_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Spy.Death",
            "Spy.CritDeath",
            "Spy.MeleeDeath",
            "Spy.ExplosionDeath",
        ],
        content_sha256: "31a4984abc14e92ad8225fa1cef643b618dd596b754debecae43630834db6ad6",
    },
    ClassData {
        identity: "class.engineer",
        source_name: "engineer",
        class_record_name: "engineer",
        model: "models/player/engineer.mdl",
        hwm_model: "models/player/hwm/engineer.mdl",
        hand_model: "models/weapons/c_models/c_engineer_arms.mdl",
        localization: "TF_Class_Name_Engineer",
        maximum_speed: 300.0,
        maximum_health: 125,
        maximum_armor: 0,
        maximum_ammo: AmmoLedger {
            primary: 32,
            secondary: 200,
            metal: 200,
            grenades1: 0,
            grenades2: 0,
        },
        weapon_declarations: ENGINEER_WEAPONS,
        grenade_declarations: ["TF_WEAPON_GRENADE_NORMAL", "TF_WEAPON_GRENADE_EMP"],
        stock_items: ENGINEER_ITEMS,
        buildables: ENGINEER_BUILDABLES,
        disable_airwalk: false,
        disable_new_jump: false,
        third_person_camera: [85.0, 25.0, 0.0],
        death_sounds: [
            "Engineer.Death",
            "Engineer.CritDeath",
            "Engineer.MeleeDeath",
            "Engineer.ExplosionDeath",
        ],
        content_sha256: "552dbf5a5bb1dc10dedfda1962bf143397bd82ffc72d2dff7a8f5c20f7957686",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_identity_and_class_record_matrix_is_exact() {
        assert_eq!(
            PlayerClass::ALL.map(PlayerClass::source_number),
            [1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        assert!(PlayerClass::try_from(0).is_err());
        assert!(PlayerClass::try_from(10).is_err());
        assert_eq!(
            PlayerClass::ALL.map(PlayerClass::standing_eye_height),
            [65.0, 75.0, 68.0, 68.0, 75.0, 75.0, 68.0, 75.0, 68.0]
        );
        assert_eq!(
            [
                PlayerTeam::Unassigned,
                PlayerTeam::Spectator,
                PlayerTeam::Red,
                PlayerTeam::Blue,
            ]
            .map(PlayerTeam::source_number),
            [0, 1, 2, 3]
        );
        assert_eq!(PlayerClass::Scout.data().maximum_speed, 400.0);
        assert_eq!(PlayerClass::Heavy.data().maximum_health, 300);
        assert_eq!(PlayerClass::Engineer.data().maximum_ammo.metal, 200);
        assert_eq!(PlayerClass::Spy.data().buildables, &[Buildable::Sapper]);
    }

    #[test]
    fn ammo_grants_cap_and_report_only_the_applied_delta() {
        let maximum = PlayerClass::Soldier.data().maximum_ammo;
        let mut ammo = AmmoLedger {
            primary: 19,
            ..AmmoLedger::default()
        };
        assert_eq!(ammo.give(maximum, AmmoType::Primary, 10), 1);
        assert_eq!(ammo.primary, 20);
        assert_eq!(ammo.give(maximum, AmmoType::Primary, 1), 0);
    }
}
