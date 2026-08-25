# TF2 Core State Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 class, loadout, economy schema, attribute manager, player condition, health/healing, damage/death, health-kit, ammo-pack and dropped-weapon contracts; configured TF2 class records and `scripts/items/items_game.txt`.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `24245096`; `scripts/items/items_game.txt` SHA-256 `4d1f15b63e63e3e897552cfb8042cccb99d2e233a0c8d8afd8734a3ea49d08da`; signature SHA-256 `353a124196f1218738b2d2d1982052b3900d71c8afac4428f35c13aaf5dbbccb`; `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`.

Generator command: `cargo run --locked --manifest-path games/tf2/rust/inventory-generator/Cargo.toml`

Output path: `games/tf2/inventories/core-state.md`

Item count: 322

Ordered stable-identity SHA-256: `7c31d0efd20320cc2810f26780bb43f2bd5c53b874b4209b4a4df6e95ef592ab`

Generation state: generated bounded inventory. This output enumerates only the selected core family; broader candidate records remain visible and unaccepted in the nine owning inventories.

Provider order: item runtime attributes override equal-definition static attributes; each query applies local attributes in retained order, providers in provider-vector order, then the owner. Provider-equals-initiator and weapon-provider-to-weapon-initiator edges are suppressed. Item-list queries bypass cache; non-item-list cache identity is exact hook plus input value.

Configured bounds: 64 players, 1,024 pickups, 4,096 commands per transition, 8,192 attribute entities, 4,096 attributes and 256 providers per attribute entity, 1,024 cache records per attribute entity, 64 healers, 128 damage-history entries, 19 class slots, three account slots, five condition words, 30-second dropped-pickup lifetime, three active dropped ammo packs per owner, and the SDK-observable 33 active dropped-weapon steady-state maximum produced by the pre-creation 32-item cleanup calculation.

Selected stock prefab closure contains 26 records: `weapon_baseflamethrower`, `weapon_newflame`, `weapon_bat`, `weapon_bonesaw`, `weapon_bottle`, `weapon_club`, `weapon_fireaxe`, `weapon_fists`, `weapon_flamethrower`, `weapon_grenade_launcher`, `weapon_invis`, `weapon_knife`, `weapon_medigun`, `weapon_minigun`, `weapon_pistol`, `weapon_revolver`, `weapon_rocketlauncher`, `weapon_sapper`, `weapon_scattergun`, `weapon_shotgun`, `weapon_shovel`, `weapon_smg`, `weapon_sniperrifle`, `weapon_stickybomb_launcher`, `weapon_syringegun`, `weapon_wrench`.

Configured class-record hashes: `scout` `f84dd59305afe06e9198a31f4b2f37ee6a06cc91e3e610f8e5ec6a5e8024979b`; `sniper` `f6cfd1320f8033abdd6dba1f32072a465fd198c1305e1f297b53a702337dd1ed`; `soldier` `3cae1b6da09c5ef26e04cc619bc4317b47bdcb4df838e50399cb10e7e078abb1`; `demoman` `28eb6c32971f16a52991327c2238e1993a22d9a9805c7ad1ec9282d36f538a4d`; `medic` `fe3cfdd879543984816530a67c3d5854f379b14fa8273dbb62c20f7d62922769`; `heavy` `3ee9abad4c25176a1922c9107ff403f9ab8bcede9a3d75810e9ffb703b79ac59`; `pyro` `fa20e5afacbde10379d89326ccca144c0652594f36c017176c6125feac06ab0f`; `spy` `31a4984abc14e92ad8225fa1cef643b618dd596b754debecae43630834db6ad6`; `engineer` `552dbf5a5bb1dc10dedfda1962bf143397bd82ffc72d2dff7a8f5c20f7957686`.

| Stable identity | Authority record | Coverage classification |
|---|---|---|
| `class.scout` | `ETFClass=1 / scripts/playerclasses/scout.ctx f84dd59305afe06e9198a31f4b2f37ee6a06cc91e3e610f8e5ec6a5e8024979b / speed=400 health=125 armor=0 ammo=32/36/100/1/1 model=models/player/scout.mdl hwm=models/player/hwm/scout.mdl hands=models/weapons/c_models/c_scout_arms.mdl localize=TF_Class_Name_Scout weapons=TF_WEAPON_BAT,TF_WEAPON_PISTOL_SCOUT,TF_WEAPON_SCATTERGUN,,, grenades=TF_WEAPON_GRENADE_CALTROP,TF_WEAPON_GRENADE_CONCUSSION buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Scout.Death,Scout.CritDeath,Scout.MeleeDeath,Scout.ExplosionDeath / stock=13@0,23@1,0@2` | Handled: class record and stock spawn state |
| `class.sniper` | `ETFClass=2 / scripts/playerclasses/sniper.ctx f6cfd1320f8033abdd6dba1f32072a465fd198c1305e1f297b53a702337dd1ed / speed=300 health=125 armor=0 ammo=25/75/100/1/0 model=models/player/sniper.mdl hwm=models/player/hwm/sniper.mdl hands=models/weapons/c_models/c_sniper_arms.mdl localize=TF_Class_Name_Sniper weapons=TF_WEAPON_CLUB,TF_WEAPON_SMG,TF_WEAPON_SNIPERRIFLE,,, grenades=TF_WEAPON_GRENADE_NORMAL, buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Sniper.Death,Sniper.CritDeath,Sniper.MeleeDeath,Sniper.ExplosionDeath / stock=14@0,16@1,3@2` | Handled: class record and stock spawn state |
| `class.soldier` | `ETFClass=3 / scripts/playerclasses/soldier.ctx 3cae1b6da09c5ef26e04cc619bc4317b47bdcb4df838e50399cb10e7e078abb1 / speed=240 health=200 armor=0 ammo=20/32/100/1/1 model=models/player/soldier.mdl hwm=models/player/hwm/soldier.mdl hands=models/weapons/c_models/c_soldier_arms.mdl localize=TF_Class_Name_Soldier weapons=TF_WEAPON_SHOVEL,TF_WEAPON_SHOTGUN_SOLDIER,TF_WEAPON_ROCKETLAUNCHER,,, grenades=TF_WEAPON_GRENADE_NORMAL,TF_WEAPON_GRENADE_NAIL buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Soldier.Death,Soldier.CritDeath,Soldier.MeleeDeath,Soldier.ExplosionDeath / stock=18@0,10@1,6@2` | Handled: class record and stock spawn state |
| `class.demoman` | `ETFClass=4 / scripts/playerclasses/demoman.ctx 28eb6c32971f16a52991327c2238e1993a22d9a9805c7ad1ec9282d36f538a4d / speed=280 health=175 armor=0 ammo=16/24/100/1/1 model=models/player/demo.mdl hwm=models/player/hwm/demo.mdl hands=models/weapons/c_models/c_demo_arms.mdl localize=TF_Class_Name_Demoman weapons=TF_WEAPON_BOTTLE,TF_WEAPON_GRENADELAUNCHER,TF_WEAPON_PIPEBOMBLAUNCHER,,, grenades=TF_WEAPON_GRENADE_NORMAL,TF_WEAPON_GRENADE_MIRV_DEMOMAN buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Demoman.Death,Demoman.CritDeath,Demoman.MeleeDeath,Demoman.ExplosionDeath / stock=19@0,20@1,1@2` | Handled: class record and stock spawn state |
| `class.medic` | `ETFClass=5 / scripts/playerclasses/medic.ctx fe3cfdd879543984816530a67c3d5854f379b14fa8273dbb62c20f7d62922769 / speed=320 health=150 armor=0 ammo=150/150/100/0/0 model=models/player/medic.mdl hwm=models/player/hwm/medic.mdl hands=models/weapons/c_models/c_medic_arms.mdl localize=TF_Class_Name_Medic weapons=TF_WEAPON_BONESAW,TF_WEAPON_MEDIGUN,TF_WEAPON_SYRINGEGUN_MEDIC,,, grenades=TF_WEAPON_GRENADE_NORMAL,TF_WEAPON_GRENADE_HEAL buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Medic.Death,Medic.CritDeath,Medic.MeleeDeath,Medic.ExplosionDeath / stock=17@0,29@1,8@2` | Handled: class record and stock spawn state |
| `class.heavy` | `ETFClass=6 / scripts/playerclasses/heavyweapons.ctx 3ee9abad4c25176a1922c9107ff403f9ab8bcede9a3d75810e9ffb703b79ac59 / speed=230 health=300 armor=0 ammo=200/32/100/1/1 model=models/player/heavy.mdl hwm=models/player/hwm/heavy.mdl hands=models/weapons/c_models/c_heavy_arms.mdl localize=TF_Class_Name_HWGuy weapons=TF_WEAPON_FISTS,TF_WEAPON_SHOTGUN_HWG,TF_WEAPON_MINIGUN,,, grenades=TF_WEAPON_GRENADE_NORMAL,TF_WEAPON_GRENADE_MIRV buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Heavy.Death,Heavy.CritDeath,Heavy.MeleeDeath,Heavy.ExplosionDeath / stock=15@0,11@1,5@2` | Handled: class record and stock spawn state |
| `class.pyro` | `ETFClass=7 / scripts/playerclasses/pyro.ctx fa20e5afacbde10379d89326ccca144c0652594f36c017176c6125feac06ab0f / speed=300 health=175 armor=0 ammo=200/32/100/1/0 model=models/player/pyro.mdl hwm=models/player/hwm/pyro.mdl hands=models/weapons/c_models/c_pyro_arms.mdl localize=TF_Class_Name_Pyro weapons=TF_WEAPON_FIREAXE,TF_WEAPON_SHOTGUN_PYRO,TF_WEAPON_FLAMETHROWER,,, grenades=TF_WEAPON_GRENADE_NORMAL, buildables=,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Pyro.Death,Pyro.CritDeath,Pyro.MeleeDeath,Pyro.ExplosionDeath / stock=21@0,12@1,2@2` | Handled: class record and stock spawn state |
| `class.spy` | `ETFClass=8 / scripts/playerclasses/spy.ctx 31a4984abc14e92ad8225fa1cef643b618dd596b754debecae43630834db6ad6 / speed=320 health=125 armor=0 ammo=20/24/100/0/1 model=models/player/spy.mdl hwm=models/player/hwm/spy.mdl hands=models/weapons/c_models/c_spy_arms.mdl localize=TF_Class_Name_Spy weapons=TF_WEAPON_KNIFE,TF_WEAPON_REVOLVER,TF_WEAPON_PDA_SPY,TF_WEAPON_INVIS,, grenades=TF_WEAPON_GRENADE_NORMAL, buildables=OBJ_ATTACHMENT_SAPPER,,,,, animation-flags=0/0 camera=85,25,0 death-sounds=Spy.Death,Spy.CritDeath,Spy.MeleeDeath,Spy.ExplosionDeath / stock=24@1,4@2,735@4,27@5,30@6` | Handled: class record and stock spawn state |
| `class.engineer` | `ETFClass=9 / scripts/playerclasses/engineer.ctx 552dbf5a5bb1dc10dedfda1962bf143397bd82ffc72d2dff7a8f5c20f7957686 / speed=300 health=125 armor=0 ammo=32/200/200/0/0 model=models/player/engineer.mdl hwm=models/player/hwm/engineer.mdl hands=models/weapons/c_models/c_engineer_arms.mdl localize=TF_Class_Name_Engineer weapons=TF_WEAPON_WRENCH,TF_WEAPON_PISTOL,TF_WEAPON_SHOTGUN_PRIMARY,TF_WEAPON_PDA_ENGINEER_BUILD,TF_WEAPON_PDA_ENGINEER_DESTROY, grenades=TF_WEAPON_GRENADE_NORMAL,TF_WEAPON_GRENADE_EMP buildables=OBJ_SENTRYGUN,OBJ_DISPENSER,OBJ_TELEPORTER,,, animation-flags=0/0 camera=85,25,0 death-sounds=Engineer.Death,Engineer.CritDeath,Engineer.MeleeDeath,Engineer.ExplosionDeath / stock=9@0,22@1,7@2,28@4,25@5,26@6` | Handled: class record and stock spawn state |
| `loadout.class.0` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.1` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.2` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.3` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.4` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.5` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.6` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.7` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.8` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.9` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.10` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.11` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.12` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.13` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.14` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.15` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.16` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.17` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.class.18` | `loadout_positions_t` | Handled: class loadout slot |
| `loadout.account.0` | `account_loadout_positions_t` | Handled: account loadout slot |
| `loadout.account.1` | `account_loadout_positions_t` | Handled: account loadout slot |
| `loadout.account.2` | `account_loadout_positions_t` | Handled: account loadout slot |
| `item-definition:0` | `items:0 / TF_WEAPON_BAT / tf_weapon_bat` | Handled: stock item instance and loadout eligibility |
| `item-definition:1` | `items:1 / TF_WEAPON_BOTTLE / tf_weapon_bottle` | Handled: stock item instance and loadout eligibility |
| `item-definition:2` | `items:2 / TF_WEAPON_FIREAXE / tf_weapon_fireaxe` | Handled: stock item instance and loadout eligibility |
| `item-definition:3` | `items:3 / TF_WEAPON_CLUB / tf_weapon_club` | Handled: stock item instance and loadout eligibility |
| `item-definition:4` | `items:4 / TF_WEAPON_KNIFE / tf_weapon_knife` | Handled: stock item instance and loadout eligibility |
| `item-definition:5` | `items:5 / TF_WEAPON_FISTS / tf_weapon_fists` | Handled: stock item instance and loadout eligibility |
| `item-definition:6` | `items:6 / TF_WEAPON_SHOVEL / tf_weapon_shovel` | Handled: stock item instance and loadout eligibility |
| `item-definition:7` | `items:7 / TF_WEAPON_WRENCH / tf_weapon_wrench` | Handled: stock item instance and loadout eligibility |
| `item-definition:8` | `items:8 / TF_WEAPON_BONESAW / tf_weapon_bonesaw` | Handled: stock item instance and loadout eligibility |
| `item-definition:9` | `items:9 / TF_WEAPON_SHOTGUN_PRIMARY / tf_weapon_shotgun` | Handled: stock item instance and loadout eligibility |
| `item-definition:10` | `items:10 / TF_WEAPON_SHOTGUN_SOLDIER / tf_weapon_shotgun` | Handled: stock item instance and loadout eligibility |
| `item-definition:11` | `items:11 / TF_WEAPON_SHOTGUN_HWG / tf_weapon_shotgun` | Handled: stock item instance and loadout eligibility |
| `item-definition:12` | `items:12 / TF_WEAPON_SHOTGUN_PYRO / tf_weapon_shotgun` | Handled: stock item instance and loadout eligibility |
| `item-definition:13` | `items:13 / TF_WEAPON_SCATTERGUN / tf_weapon_scattergun` | Handled: stock item instance and loadout eligibility |
| `item-definition:14` | `items:14 / TF_WEAPON_SNIPERRIFLE / tf_weapon_sniperrifle` | Handled: stock item instance and loadout eligibility |
| `item-definition:15` | `items:15 / TF_WEAPON_MINIGUN / tf_weapon_minigun` | Handled: stock item instance and loadout eligibility |
| `item-definition:16` | `items:16 / TF_WEAPON_SMG / tf_weapon_smg` | Handled: stock item instance and loadout eligibility |
| `item-definition:17` | `items:17 / TF_WEAPON_SYRINGEGUN_MEDIC / tf_weapon_syringegun_medic` | Handled: stock item instance and loadout eligibility |
| `item-definition:18` | `items:18 / TF_WEAPON_ROCKETLAUNCHER / tf_weapon_rocketlauncher` | Handled: stock item instance and loadout eligibility |
| `item-definition:19` | `items:19 / TF_WEAPON_GRENADELAUNCHER / tf_weapon_grenadelauncher` | Handled: stock item instance and loadout eligibility |
| `item-definition:20` | `items:20 / TF_WEAPON_PIPEBOMBLAUNCHER / tf_weapon_pipebomblauncher` | Handled: stock item instance and loadout eligibility |
| `item-definition:21` | `items:21 / TF_WEAPON_FLAMETHROWER / tf_weapon_flamethrower` | Handled: stock item instance and loadout eligibility |
| `item-definition:22` | `items:22 / TF_WEAPON_PISTOL / tf_weapon_pistol` | Handled: stock item instance and loadout eligibility |
| `item-definition:23` | `items:23 / TF_WEAPON_PISTOL_SCOUT / tf_weapon_pistol` | Handled: stock item instance and loadout eligibility |
| `item-definition:24` | `items:24 / TF_WEAPON_REVOLVER / tf_weapon_revolver` | Handled: stock item instance and loadout eligibility |
| `item-definition:25` | `items:25 / TF_WEAPON_PDA_ENGINEER_BUILD / tf_weapon_pda_engineer_build` | Handled: stock item instance and loadout eligibility |
| `item-definition:26` | `items:26 / TF_WEAPON_PDA_ENGINEER_DESTROY / tf_weapon_pda_engineer_destroy` | Handled: stock item instance and loadout eligibility |
| `item-definition:27` | `items:27 / TF_WEAPON_PDA_SPY / tf_weapon_pda_spy` | Handled: stock item instance and loadout eligibility |
| `item-definition:28` | `items:28 / TF_WEAPON_BUILDER / tf_weapon_builder` | Handled: stock item instance and loadout eligibility |
| `item-definition:29` | `items:29 / TF_WEAPON_MEDIGUN / tf_weapon_medigun` | Handled: stock item instance and loadout eligibility |
| `item-definition:30` | `items:30 / TF_WEAPON_INVIS / tf_weapon_invis` | Handled: stock item instance and loadout eligibility |
| `item-definition:735` | `items:735 / TF_WEAPON_BUILDER_SPY / tf_weapon_builder` | Handled: stock item instance and loadout eligibility |
| `item-definition.default` | `items:default` | Handled: immutable selected schema default definition |
| `prefab:weapon_baseflamethrower` | `prefabs:weapon_baseflamethrower` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_newflame` | `prefabs:weapon_newflame` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_bat` | `prefabs:weapon_bat` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_bonesaw` | `prefabs:weapon_bonesaw` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_bottle` | `prefabs:weapon_bottle` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_club` | `prefabs:weapon_club` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_fireaxe` | `prefabs:weapon_fireaxe` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_fists` | `prefabs:weapon_fists` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_flamethrower` | `prefabs:weapon_flamethrower` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_grenade_launcher` | `prefabs:weapon_grenade_launcher` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_invis` | `prefabs:weapon_invis` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_knife` | `prefabs:weapon_knife` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_medigun` | `prefabs:weapon_medigun` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_minigun` | `prefabs:weapon_minigun` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_pistol` | `prefabs:weapon_pistol` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_revolver` | `prefabs:weapon_revolver` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_rocketlauncher` | `prefabs:weapon_rocketlauncher` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_sapper` | `prefabs:weapon_sapper` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_scattergun` | `prefabs:weapon_scattergun` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_shotgun` | `prefabs:weapon_shotgun` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_shovel` | `prefabs:weapon_shovel` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_smg` | `prefabs:weapon_smg` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_sniperrifle` | `prefabs:weapon_sniperrifle` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_stickybomb_launcher` | `prefabs:weapon_stickybomb_launcher` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_syringegun` | `prefabs:weapon_syringegun` | Handled: stock definition recursive prefab closure |
| `prefab:weapon_wrench` | `prefabs:weapon_wrench` | Handled: stock definition recursive prefab closure |
| `attribute-definition:15` | `attributes:15 / crit mod disabled / mult_crit_chance` | Handled: bounded core attribute definition |
| `attribute-definition:26` | `attributes:26 / max health additive bonus / add_maxhealth` | Handled: bounded core attribute definition |
| `attribute-definition:28` | `attributes:28 / crit mod disabled hidden / mult_crit_chance` | Handled: bounded core attribute definition |
| `attribute-definition:60` | `attributes:60 / dmg taken from fire reduced / mult_dmgtaken_from_fire` | Handled: bounded core attribute definition |
| `attribute-definition:61` | `attributes:61 / dmg taken from fire increased / mult_dmgtaken_from_fire` | Handled: bounded core attribute definition |
| `attribute-definition:62` | `attributes:62 / dmg taken from crit reduced / mult_dmgtaken_from_crit` | Handled: bounded core attribute definition |
| `attribute-definition:63` | `attributes:63 / dmg taken from crit increased / mult_dmgtaken_from_crit` | Handled: bounded core attribute definition |
| `attribute-definition:64` | `attributes:64 / dmg taken from blast reduced / mult_dmgtaken_from_explosions` | Handled: bounded core attribute definition |
| `attribute-definition:65` | `attributes:65 / dmg taken from blast increased / mult_dmgtaken_from_explosions` | Handled: bounded core attribute definition |
| `attribute-definition:66` | `attributes:66 / dmg taken from bullets reduced / mult_dmgtaken_from_bullets` | Handled: bounded core attribute definition |
| `attribute-definition:67` | `attributes:67 / dmg taken from bullets increased / mult_dmgtaken_from_bullets` | Handled: bounded core attribute definition |
| `attribute-definition:69` | `attributes:69 / health from healers reduced / mult_health_fromhealers` | Handled: bounded core attribute definition |
| `attribute-definition:70` | `attributes:70 / health from healers increased / mult_health_fromhealers` | Handled: bounded core attribute definition |
| `attribute-definition:108` | `attributes:108 / health from packs increased / mult_health_frompacks` | Handled: bounded core attribute definition |
| `attribute-definition:109` | `attributes:109 / health from packs decreased / mult_health_frompacks` | Handled: bounded core attribute definition |
| `attribute-definition:125` | `attributes:125 / max health additive penalty / add_maxhealth` | Handled: bounded core attribute definition |
| `attribute-definition:138` | `attributes:138 / dmg penalty vs players / mult_dmg_vs_players` | Handled: bounded core attribute definition |
| `attribute-definition:140` | `attributes:140 / hidden maxhealth non buffed / add_maxhealth_nonbuffed` | Handled: bounded core attribute definition |
| `attribute-definition:179` | `attributes:179 / minicrits become crits / minicrits_become_crits` | Handled: bounded core attribute definition |
| `attribute-definition:412` | `attributes:412 / dmg taken increased / mult_dmgtaken` | Handled: bounded core attribute definition |
| `attribute-definition:479` | `attributes:479 / overheal fill rate reduced / overheal_fill_rate` | Handled: bounded core attribute definition |
| `attribute-definition:491` | `attributes:491 / SET BONUS: dmg taken from crit reduced set bonus / mult_dmgtaken_from_crit` | Handled: bounded core attribute definition |
| `attribute-definition:492` | `attributes:492 / SET BONUS: dmg taken from fire reduced set bonus / mult_dmgtaken_from_fire` | Handled: bounded core attribute definition |
| `attribute-definition:503` | `attributes:503 / medigun bullet resist passive / medigun_bullet_resist_passive` | Handled: bounded core attribute definition |
| `attribute-definition:504` | `attributes:504 / medigun blast resist passive / medigun_blast_resist_passive` | Handled: bounded core attribute definition |
| `attribute-definition:505` | `attributes:505 / medigun fire resist passive / medigun_fire_resist_passive` | Handled: bounded core attribute definition |
| `attribute-definition:506` | `attributes:506 / medigun bullet resist deployed / medigun_bullet_resist_deployed` | Handled: bounded core attribute definition |
| `attribute-definition:507` | `attributes:507 / medigun blast resist deployed / medigun_blast_resist_deployed` | Handled: bounded core attribute definition |
| `attribute-definition:508` | `attributes:508 / medigun fire resist deployed / medigun_fire_resist_deployed` | Handled: bounded core attribute definition |
| `attribute-definition:516` | `attributes:516 / SET BONUS: dmg taken from bullets increased / mult_dmgtaken_from_bullets` | Handled: bounded core attribute definition |
| `attribute-definition:517` | `attributes:517 / SET BONUS: max health additive bonus / add_maxhealth` | Handled: bounded core attribute definition |
| `attribute-definition:526` | `attributes:526 / healing received bonus / mult_healing_received` | Handled: bounded core attribute definition |
| `attribute-definition:740` | `attributes:740 / reduced_healing_from_medics / mult_healing_from_medics` | Handled: bounded core attribute definition |
| `attribute-definition:794` | `attributes:794 / dmg taken from fire reduced on active / mult_dmgtaken_from_fire_active` | Handled: bounded core attribute definition |
| `attribute-definition:797` | `attributes:797 / dmg pierces resists absorbs / mod_pierce_resists_absorbs` | Handled: bounded core attribute definition |
| `attribute-definition:852` | `attributes:852 / mult_dmgtaken_active / mult_dmgtaken_active` | Handled: bounded core attribute definition |
| `attribute-definition:854` | `attributes:854 / mult_health_fromhealers_penalty_active / mult_health_fromhealers_penalty_active` | Handled: bounded core attribute definition |
| `attribute-definition:869` | `attributes:869 / crits_become_minicrits / crits_become_minicrits` | Handled: bounded core attribute definition |
| `schema-closure-attribute-definition:292` | `attributes:292 / kill eater score type / kill_eater_score_type` | Handled: retained stock prefab/item attribute; core advancement inert |
| `schema-closure-attribute-definition:293` | `attributes:293 / kill eater score type 2 / kill_eater_score_type_2` | Handled: retained stock prefab/item attribute; core advancement inert |
| `schema-closure-attribute-definition:495` | `attributes:495 / kill eater score type 3 / kill_eater_score_type_3` | Handled: retained stock prefab/item attribute; core advancement inert |
| `attribute-hook:add_maxhealth` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:add_maxhealth_nonbuffed` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_health_fromhealers` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_health_fromhealers_penalty_active` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_healing_received` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_health_frompacks` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_healing_from_medics` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:overheal_fill_rate` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmg_vs_players` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_crit` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_fire` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_fire_active` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_explosions` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_bullets` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_from_melee` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_dmgtaken_active` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mod_pierce_resists_absorbs` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:minicrits_become_crits` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:crits_become_minicrits` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `attribute-hook:mult_crit_chance` | `official SDK CALL_ATTRIB_HOOK site` | Handled: bounded core provider query |
| `condition:0` | `TF_COND_AIMING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:1` | `TF_COND_ZOOMED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:2` | `TF_COND_DISGUISING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:3` | `TF_COND_DISGUISED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:4` | `TF_COND_STEALTHED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:5` | `TF_COND_INVULNERABLE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:6` | `TF_COND_TELEPORTED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:7` | `TF_COND_TAUNTING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:8` | `TF_COND_INVULNERABLE_WEARINGOFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:9` | `TF_COND_STEALTHED_BLINK` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:10` | `TF_COND_SELECTED_TO_TELEPORT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:11` | `TF_COND_CRITBOOSTED` | Handled: generic lifecycle and core health/damage semantics |
| `condition:12` | `TF_COND_TMPDAMAGEBONUS` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:13` | `TF_COND_FEIGN_DEATH` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:14` | `TF_COND_PHASE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:15` | `TF_COND_STUNNED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:16` | `TF_COND_OFFENSEBUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:17` | `TF_COND_SHIELD_CHARGE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:18` | `TF_COND_DEMO_BUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:19` | `TF_COND_ENERGY_BUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:20` | `TF_COND_RADIUSHEAL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:21` | `TF_COND_HEALTH_BUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:22` | `TF_COND_BURNING` | Handled: generic lifecycle and core health/damage semantics |
| `condition:23` | `TF_COND_HEALTH_OVERHEALED` | Handled: generic lifecycle and core health/damage semantics |
| `condition:24` | `TF_COND_URINE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:25` | `TF_COND_BLEEDING` | Handled: generic lifecycle and core health/damage semantics |
| `condition:26` | `TF_COND_DEFENSEBUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:27` | `TF_COND_MAD_MILK` | Handled: generic lifecycle and core health/damage semantics |
| `condition:28` | `TF_COND_MEGAHEAL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:29` | `TF_COND_REGENONDAMAGEBUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:30` | `TF_COND_MARKEDFORDEATH` | Handled: generic lifecycle and core health/damage semantics |
| `condition:31` | `TF_COND_NOHEALINGDAMAGEBUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:32` | `TF_COND_SPEED_BOOST` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:33` | `TF_COND_CRITBOOSTED_PUMPKIN` | Handled: generic lifecycle and core health/damage semantics |
| `condition:34` | `TF_COND_CRITBOOSTED_USER_BUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:35` | `TF_COND_CRITBOOSTED_DEMO_CHARGE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:36` | `TF_COND_SODAPOPPER_HYPE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:37` | `TF_COND_CRITBOOSTED_FIRST_BLOOD` | Handled: generic lifecycle and core health/damage semantics |
| `condition:38` | `TF_COND_CRITBOOSTED_BONUS_TIME` | Handled: generic lifecycle and core health/damage semantics |
| `condition:39` | `TF_COND_CRITBOOSTED_CTF_CAPTURE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:40` | `TF_COND_CRITBOOSTED_ON_KILL` | Handled: generic lifecycle and core health/damage semantics |
| `condition:41` | `TF_COND_CANNOT_SWITCH_FROM_MELEE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:42` | `TF_COND_DEFENSEBUFF_NO_CRIT_BLOCK` | Handled: generic lifecycle and core health/damage semantics |
| `condition:43` | `TF_COND_REPROGRAMMED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:44` | `TF_COND_CRITBOOSTED_RAGE_BUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:45` | `TF_COND_DEFENSEBUFF_HIGH` | Handled: generic lifecycle and core health/damage semantics |
| `condition:46` | `TF_COND_SNIPERCHARGE_RAGE_BUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:47` | `TF_COND_DISGUISE_WEARINGOFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:48` | `TF_COND_MARKEDFORDEATH_SILENT` | Handled: generic lifecycle and core health/damage semantics |
| `condition:49` | `TF_COND_DISGUISED_AS_DISPENSER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:50` | `TF_COND_SAPPED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:51` | `TF_COND_INVULNERABLE_HIDE_UNLESS_DAMAGED` | Handled: generic lifecycle and core health/damage semantics |
| `condition:52` | `TF_COND_INVULNERABLE_USER_BUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:53` | `TF_COND_HALLOWEEN_BOMB_HEAD` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:54` | `TF_COND_HALLOWEEN_THRILLER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:55` | `TF_COND_RADIUSHEAL_ON_DAMAGE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:56` | `TF_COND_CRITBOOSTED_CARD_EFFECT` | Handled: generic lifecycle and core health/damage semantics |
| `condition:57` | `TF_COND_INVULNERABLE_CARD_EFFECT` | Handled: generic lifecycle and core health/damage semantics |
| `condition:58` | `TF_COND_MEDIGUN_UBER_BULLET_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:59` | `TF_COND_MEDIGUN_UBER_BLAST_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:60` | `TF_COND_MEDIGUN_UBER_FIRE_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:61` | `TF_COND_MEDIGUN_SMALL_BULLET_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:62` | `TF_COND_MEDIGUN_SMALL_BLAST_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:63` | `TF_COND_MEDIGUN_SMALL_FIRE_RESIST` | Handled: generic lifecycle and core health/damage semantics |
| `condition:64` | `TF_COND_STEALTHED_USER_BUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:65` | `TF_COND_MEDIGUN_DEBUFF` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:66` | `TF_COND_STEALTHED_USER_BUFF_FADING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:67` | `TF_COND_BULLET_IMMUNE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:68` | `TF_COND_BLAST_IMMUNE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:69` | `TF_COND_FIRE_IMMUNE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:70` | `TF_COND_PREVENT_DEATH` | Handled: generic lifecycle and core health/damage semantics |
| `condition:71` | `TF_COND_MVM_BOT_STUN_RADIOWAVE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:72` | `TF_COND_HALLOWEEN_SPEED_BOOST` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:73` | `TF_COND_HALLOWEEN_QUICK_HEAL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:74` | `TF_COND_HALLOWEEN_GIANT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:75` | `TF_COND_HALLOWEEN_TINY` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:76` | `TF_COND_HALLOWEEN_IN_HELL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:77` | `TF_COND_HALLOWEEN_GHOST_MODE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:78` | `TF_COND_MINICRITBOOSTED_ON_KILL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:79` | `TF_COND_OBSCURED_SMOKE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:80` | `TF_COND_PARACHUTE_ACTIVE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:81` | `TF_COND_BLASTJUMPING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:82` | `TF_COND_HALLOWEEN_KART` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:83` | `TF_COND_HALLOWEEN_KART_DASH` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:84` | `TF_COND_BALLOON_HEAD` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:85` | `TF_COND_MELEE_ONLY` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:86` | `TF_COND_SWIMMING_CURSE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:87` | `TF_COND_FREEZE_INPUT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:88` | `TF_COND_HALLOWEEN_KART_CAGE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:89` | `TF_COND_DONOTUSE_0` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:90` | `TF_COND_RUNE_STRENGTH` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:91` | `TF_COND_RUNE_HASTE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:92` | `TF_COND_RUNE_REGEN` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:93` | `TF_COND_RUNE_RESIST` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:94` | `TF_COND_RUNE_VAMPIRE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:95` | `TF_COND_RUNE_REFLECT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:96` | `TF_COND_RUNE_PRECISION` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:97` | `TF_COND_RUNE_AGILITY` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:98` | `TF_COND_GRAPPLINGHOOK` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:99` | `TF_COND_GRAPPLINGHOOK_SAFEFALL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:100` | `TF_COND_GRAPPLINGHOOK_LATCHED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:101` | `TF_COND_GRAPPLINGHOOK_BLEEDING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:102` | `TF_COND_AFTERBURN_IMMUNE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:103` | `TF_COND_RUNE_KNOCKOUT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:104` | `TF_COND_RUNE_IMBALANCE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:105` | `TF_COND_CRITBOOSTED_RUNE_TEMP` | Handled: generic lifecycle and core health/damage semantics |
| `condition:106` | `TF_COND_PASSTIME_INTERCEPTION` | Handled: generic lifecycle and core health/damage semantics |
| `condition:107` | `TF_COND_SWIMMING_NO_EFFECTS` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:108` | `TF_COND_PURGATORY` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:109` | `TF_COND_RUNE_KING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:110` | `TF_COND_RUNE_PLAGUE` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:111` | `TF_COND_RUNE_SUPERNOVA` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:112` | `TF_COND_PLAGUE` | Handled: generic lifecycle and core health/damage semantics |
| `condition:113` | `TF_COND_KING_BUFFED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:114` | `TF_COND_TEAM_GLOWS` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:115` | `TF_COND_KNOCKED_INTO_AIR` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:116` | `TF_COND_COMPETITIVE_WINNER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:117` | `TF_COND_COMPETITIVE_LOSER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:118` | `TF_COND_HEALING_DEBUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:119` | `TF_COND_PASSTIME_PENALTY_DEBUFF` | Handled: generic lifecycle and core health/damage semantics |
| `condition:120` | `TF_COND_GRAPPLED_TO_PLAYER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:121` | `TF_COND_GRAPPLED_BY_PLAYER` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:122` | `TF_COND_PARACHUTE_DEPLOYED` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:123` | `TF_COND_GAS` | Handled: generic lifecycle and core health/damage semantics |
| `condition:124` | `TF_COND_BURNING_PYRO` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:125` | `TF_COND_ROCKETPACK` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:126` | `TF_COND_LOST_FOOTING` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:127` | `TF_COND_AIR_CURRENT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:128` | `TF_COND_HALLOWEEN_HELL_HEAL` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:129` | `TF_COND_POWERUPMODE_DOMINANT` | Handled: generic lifecycle; specialized effect remains downstream |
| `condition:130` | `TF_COND_IMMUNE_TO_PUSHBACK` | Handled: generic lifecycle; specialized effect remains downstream |
| `pickup.health.small` | `item_healthkit_small` | Handled: ordinary pickup lifecycle and grant |
| `pickup.health.medium` | `item_healthkit_medium` | Handled: ordinary pickup lifecycle and grant |
| `pickup.health.full` | `item_healthkit_full` | Handled: ordinary pickup lifecycle and grant |
| `pickup.ammo.small` | `item_ammopack_small` | Handled: ordinary pickup lifecycle and grant |
| `pickup.ammo.medium` | `item_ammopack_medium` | Handled: ordinary pickup lifecycle and grant |
| `pickup.ammo.full` | `item_ammopack_full` | Handled: ordinary pickup lifecycle and grant |
| `pickup.ammo.dropped` | `tf_ammo_pack` | Handled: ordinary pickup lifecycle and grant |
| `pickup.weapon.dropped` | `tf_dropped_weapon` | Handled: ordinary pickup lifecycle and grant |
| `state.tick` | `CoreState::tick` | Handled: canonical core transition/snapshot field |
| `state.content-build` | `ItemSchema::content_build` | Handled: canonical core transition/snapshot field |
| `state.schema-hash` | `ItemSchema::schema_sha256` | Handled: canonical core transition/snapshot field |
| `player.lifecycle` | `TF_STATE_*` | Handled: canonical core transition/snapshot field |
| `player.team` | `TF_TEAM_*` | Handled: canonical core transition/snapshot field |
| `player.class` | `ETFClass` | Handled: canonical core transition/snapshot field |
| `player.desired-class` | `m_iDesiredPlayerClass` | Handled: canonical core transition/snapshot field |
| `player.health.current` | `m_iHealth` | Handled: canonical core transition/snapshot field |
| `player.health.maximum-buffable` | `GetMaxHealthForBuffing` | Handled: canonical core transition/snapshot field |
| `player.health.maximum` | `GetMaxHealth` | Handled: canonical core transition/snapshot field |
| `player.health.fraction` | `m_flHealFraction` | Handled: canonical core transition/snapshot field |
| `player.health.healers` | `m_aHealers` | Handled: canonical core transition/snapshot field |
| `player.health.overheal-decay` | `m_flBestOverhealDecayMult` | Handled: canonical core transition/snapshot field |
| `player.health.last-damage-time` | `m_flLastDamageTime` | Handled: canonical core transition/snapshot field |
| `player.ammo.primary` | `TF_AMMO_PRIMARY` | Handled: canonical core transition/snapshot field |
| `player.ammo.secondary` | `TF_AMMO_SECONDARY` | Handled: canonical core transition/snapshot field |
| `player.ammo.metal` | `TF_AMMO_METAL` | Handled: canonical core transition/snapshot field |
| `player.ammo.grenades1` | `TF_AMMO_GRENADES1` | Handled: canonical core transition/snapshot field |
| `player.ammo.grenades2` | `TF_AMMO_GRENADES2` | Handled: canonical core transition/snapshot field |
| `player.loadout.class-slots` | `loadout_positions_t` | Handled: canonical core transition/snapshot field |
| `player.loadout.account-slots` | `account_loadout_positions_t` | Handled: canonical core transition/snapshot field |
| `player.loadout.preset` | `GetNumAllowedItemPresets` | Handled: canonical core transition/snapshot field |
| `player.conditions.words` | `m_nPlayerCond{,Ex,Ex2,Ex3,Ex4}` | Handled: canonical core transition/snapshot field |
| `player.conditions.duration` | `m_ConditionData.m_flExpireTime` | Handled: canonical core transition/snapshot field |
| `player.conditions.provider` | `m_ConditionData.m_pProvider` | Handled: canonical core transition/snapshot field |
| `player.attributes.providers` | `CAttributeManager::m_Providers` | Handled: canonical core transition/snapshot field |
| `player.attributes.cache` | `CAttributeManager::m_CachedResults` | Handled: canonical core transition/snapshot field |
| `player.crit.bucket` | `m_flCritTokenBucket` | Handled: canonical core transition/snapshot field |
| `player.crit.history` | `m_DamageEvents` | Handled: canonical core transition/snapshot field |
| `player.weapon.active-slot` | `GetActiveTFWeapon` | Handled: canonical core transition/snapshot field |
| `pickup.lifecycle` | `CItem::{Respawn,Materialize}` | Handled: canonical core transition/snapshot field |
