# TF2 Classes Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 `src/game/shared/tf/tf_shareddefs.{h,cpp}`, `tf_classdata.{h,cpp}`, and `tf_playerclass_shared.{h,cpp}`; TF2 content-build class records `scripts/playerclasses/{scout,sniper,soldier,demoman,medic,heavyweapons,pyro,spy,engineer}.ctx` resolved from `tf2_misc_dir.vpk`.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `10822003`; class-record SHA-256 values are recorded below.

Generator command: Missing

Bounded core-state generator: `cargo run --locked --manifest-path games/tf2/rust/inventory-generator/Cargo.toml`; its selected output is [`core-state.md`](core-state.md). It must retain all nine identities and exact configured class-record hashes without claiming that the broader class-record field inventory is accepted.

Output path: `games/tf2/inventories/classes.md`

Candidate item count: 9

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

Each candidate identity is one playable class. Speed is inches per second. Ammo columns are the class-record maxima for primary, secondary, metal, grenade 1, and grenade 2. Stock equipment is recorded in class-data order; item-schema loadout selection can replace it.

| Stable identity | SDK identity | Speed | Health | Ammo `P/S/M/G1/G2` | Stock equipment | Buildables | Class-record SHA-256 | Required coverage |
|---|---|---:|---:|---|---|---|---|---|
| `class.scout` | `TF_CLASS_SCOUT = 1` | 400 | 125 | `32/36/100/1/1` | bat, pistol, scattergun; caltrop and concussion grenade declarations | None | `f84dd59305afe06e9198a31f4b2f37ee6a06cc91e3e610f8e5ec6a5e8024979b` | Handled |
| `class.sniper` | `TF_CLASS_SNIPER = 2` | 300 | 125 | `25/75/100/1/0` | club, SMG, sniper rifle; normal grenade declaration | None | `f6cfd1320f8033abdd6dba1f32072a465fd198c1305e1f297b53a702337dd1ed` | Handled |
| `class.soldier` | `TF_CLASS_SOLDIER = 3` | 240 | 200 | `20/32/100/1/1` | shovel, shotgun, rocket launcher; normal and nail grenade declarations | None | `3cae1b6da09c5ef26e04cc619bc4317b47bdcb4df838e50399cb10e7e078abb1` | Handled |
| `class.demoman` | `TF_CLASS_DEMOMAN = 4` | 280 | 175 | `16/24/100/1/1` | bottle, grenade launcher, pipebomb launcher; normal and MIRV grenade declarations | None | `28eb6c32971f16a52991327c2238e1993a22d9a9805c7ad1ec9282d36f538a4d` | Handled |
| `class.medic` | `TF_CLASS_MEDIC = 5` | 320 | 150 | `150/150/100/0/0` | bonesaw, medigun, syringe gun; normal and heal grenade declarations | None | `fe3cfdd879543984816530a67c3d5854f379b14fa8273dbb62c20f7d62922769` | Handled |
| `class.heavy` | `TF_CLASS_HEAVYWEAPONS = 6` | 230 | 300 | `200/32/100/1/1` | fists, shotgun, minigun; normal and MIRV grenade declarations | None | `3ee9abad4c25176a1922c9107ff403f9ab8bcede9a3d75810e9ffb703b79ac59` | Handled |
| `class.pyro` | `TF_CLASS_PYRO = 7` | 300 | 175 | `200/32/100/1/0` | fire axe, shotgun, flamethrower; normal grenade declaration | None | `fa20e5afacbde10379d89326ccca144c0652594f36c017176c6125feac06ab0f` | Handled |
| `class.spy` | `TF_CLASS_SPY = 8` | 320 | 125 | `20/24/100/0/1` | knife, revolver, disguise PDA, invisibility watch; normal grenade declaration | sapper | `31a4984abc14e92ad8225fa1cef643b618dd596b754debecae43630834db6ad6` | Handled |
| `class.engineer` | `TF_CLASS_ENGINEER = 9` | 300 | 125 | `32/200/200/0/0` | wrench, pistol, shotgun, build PDA, destroy PDA; normal and EMP grenade declarations | sentry, dispenser, teleporter | `552dbf5a5bb1dc10dedfda1962bf143397bd82ffc72d2dff7a8f5c20f7957686` | Handled |

All nine class records also carry model, high-detail model, hand model, localization identity, zero armor, third-person camera offset, and four death-sound mappings. The future generator must emit those fields without truncation and compare every consumed resource identity.

`TF_CLASS_UNDEFINED`, `TF_CLASS_RANDOM`, and `TF_CLASS_CIVILIAN` are classified declarations but are not playable-class inventory items. Civilian has a content record and is excluded because the SDK playable-class predicate and normal-class iterator stop before `TF_CLASS_CIVILIAN`.

## Generation Contract

The future generator must read the pinned SDK enum and parser contract, resolve the ten declared class records through the configured content index, decrypt them through the official class-data contract, emit exactly the nine playable identities in numeric order, retain the Civilian exclusion, and fail on a missing record, hash change, duplicate class number, unknown field, malformed number, unresolved weapon/buildable identity, or item-count mismatch.
