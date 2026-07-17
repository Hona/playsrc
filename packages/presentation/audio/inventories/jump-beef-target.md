# `jump_beef` Projectile Audio Target

Authority: TF2 build `24207079`, `scripts/game_sounds_manifest.txt` SHA-256 `6231fedc12c9b347a9bbc584c33137bed73803b2d84ec0a2a53b997070990dfa`, `scripts/game_sounds_weapons.txt` SHA-256 `84b3706bf7ac5a73fdbede55f2623f7b49e4cb4460ae0e13446f5ec563e6f6e1`, and exact configured `tf2_sound_misc_dir.vpk` resources.

| Definition | Channel | Level | Volume | Pitch | Decorator | Ordered waves |
|---|---:|---:|---:|---:|---|---|
| `Weapon_RPG.Single` | `CHAN_WEAPON` (1) | 94 dB | 1 | 100 | spatial stereo `)` | `weapons/rocket_shoot.wav` |
| `Weapon_StickyBombLauncher.Single` | `CHAN_WEAPON` (1) | 94 dB | 1 | 100 | spatial stereo `)` | `weapons/stickybomblauncher_shoot.wav` |
| `BaseExplosionEffect.Sound` | `CHAN_WEAPON` (1) | 95 dB | 1 | 100 | spatial stereo `)` | `weapons/explode2.wav`, `weapons/explode3.wav`, `weapons/explode1.wav` |
| `Weapon_Grenade_Pipebomb.Explode` | `CHAN_WEAPON` (1) | 95 dB | 1 | 100 | spatial stereo `)` | `weapons/pipe_bomb1.wav`, `weapons/pipe_bomb2.wav`, `weapons/pipe_bomb3.wav` |

| Logical path | Channels / format | Rate / frames | Bytes | SHA-256 |
|---|---|---|---:|---|
| `sound/weapons/rocket_shoot.wav` | 2 / PCM16 | 44,100 / 177,421 | 709,728 | `a1559b6daaf13cfa5562688217bf24a0f3e0c6afb4122962e5deab81572675a0` |
| `sound/weapons/stickybomblauncher_shoot.wav` | 2 / PCM16 | 44,100 / 18,816 | 75,416 | `564a4dd961bcebf59d346b3b68199ebfc069122e47bcaa8c8376f75b3b15a724` |
| `sound/weapons/explode1.wav` | 2 / PCM16 | 44,100 / 355,328 | 1,421,442 | `7dcb3d7a642d8524dd10eaa1b6a7fe339d9601e51c8969e3a49b0a3274767087` |
| `sound/weapons/explode2.wav` | 2 / PCM16 | 44,100 / 355,328 | 1,421,464 | `0115c57915598744dc0fea5e4a6b87a2643fb7f54b065a094225aee54b790144` |
| `sound/weapons/explode3.wav` | 2 / PCM16 | 44,100 / 333,848 | 1,335,544 | `ecd8de0ff0cc303fb454be585768ed352bca46747fc1fbb3586a7aa12311afcd` |
| `sound/weapons/pipe_bomb1.wav` | 2 / PCM16 | 44,100 / 177,152 | 708,738 | `f93ec405b4e307de4b139b44f86a44a010620944759241637c2efa87fcf7b92a` |
| `sound/weapons/pipe_bomb2.wav` | 2 / PCM16 | 44,100 / 177,152 | 708,738 | `f93ec405b4e307de4b139b44f86a44a010620944759241637c2efa87fcf7b92a` |
| `sound/weapons/pipe_bomb3.wav` | 2 / PCM16 | 44,100 / 177,152 | 708,738 | `f93ec405b4e307de4b139b44f86a44a010620944759241637c2efa87fcf7b92a` |

All eight resources are non-looping RIFF/WAVE PCM. The configured `jump_beef` BSP entity lump contains zero `ambient_generic`, `env_soundscape`, `env_soundscape_triggerable`, `trigger_soundscape`, and `env_soundscape_proxy` entities. The target therefore has no map-authored environmental voice producer; Audio still retains world/entity/UI/local-listener source distinctions for supplied events.
