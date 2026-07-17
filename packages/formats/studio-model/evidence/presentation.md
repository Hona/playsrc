# StudioModel Presentation Evidence

## Commands

- `cargo test -p playsrc-studio-model --lib` runs bounded synthetic parser, composition, artifact, transform, timing, event, autolayer, and pose vectors.
- `bun packages/formats/studio-model/scripts/verify-parity.ts` validates `playsrc.local.json`, regenerates the exact `jump_beef` source bundle through configured TF2 public build `24207079`, then runs the package-owned locked evidence runner. The runner resolves the ten target roots and every MDL/VVD/DX90.VTX/ANI/PHY/include/VMT/VTF dependency from the bundle, generates each `PSMP` v2 artifact twice, requires byte identity, decodes and canonically re-encodes it, samples every declared timeline cycle, and compares all fixed hashes below.

The configured bundle contains 294 entries and 112,112,616 bytes with SHA-256 `34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78`. The declared BSP is 33,379,388 bytes with SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`.

## Exact target matrix

Timeline SHA-256 covers activity bytes, composed sequence index, FPS, weighted frame count, CPS, duration, and every skinning/attachment matrix at cycles `0`, `0.25`, `0.5`, `0.75`, and `1`. Props/projectiles hash sequence 0 at cycle 0. Material manifests contain the complete selected VMT include closure and typed VTF records; artifacts retain every dependency occurrence and immutable hash.

| Model | MDL SHA-256 | PSMP bytes | PSMP SHA-256 | Timeline SHA-256 |
|---|---|---:|---|---|
| `models/props_2fort/cow001_reference.mdl` | `33b6fdd5a60a146f865157d488baf0d7b945c956e7cca7bfb13cf80e9d5e726e` | 52,403 | `af674c5d95989d65479de833c81418f815b9d4112db308c3c3c4e652686fa9f5` | `65a9a91c0b9bc6342a4343db11f8bae8b8ae34c8d4e02273e7c90471572fd6dd` |
| `models/props_2fort/frog.mdl` | `6ec4727763b46d37b7aabd85e210c33be1eac694b402fa101551fd1ad3378f78` | 120,661 | `56db74a275f5db8c066b325bbbd3e8f91a8327965ca21c1210eec38c417bd8e6` | `65a9a91c0b9bc6342a4343db11f8bae8b8ae34c8d4e02273e7c90471572fd6dd` |
| `models/props_gameplay/resupply_locker.mdl` | `cfca762077d5b1f252ccd448e904de4f9207da925215e0487f857c7f707a1bd2` | 998,429 | `8356e2c21ec854a3ec5a92596518642b73827023877dd59802753bdeab6c3c6c` | `bcaff50ed60d571f2eba900b1a98f009c43b638bc69c00ae53f331651846fd43` |
| `models/player/items/soldier/soldier_viking.mdl` | `2f7f2f7aab04188977985195378e18285204d71e037abc9d5da49b8b140ba561` | 258,490 | `73bf0c80b29130a2b93c9c7f162db5d7875b46c04ce6f9c91d053702eefaeaa3` | `9b08336afb4e9eb30508f8bad1b519e3d7679e59a10b31c16adc577124f02226` |
| `models/player/soldier.mdl` | `62bc48c0fa4ac4166151633087feadef103a2d86d4412639fe1adddba91de219` | 41,473,885 | `32f86ecbd068953f677358d53d5021ee987357d705abd37214ae4aef7387fd87` | `0b07964a4ffc54a45c80df4daf7dff2eb5cb088da6abc16c306ea9853211be7d` |
| `models/player/demo.mdl` | `b8fe45619b062197a975798310d703ec59f8c1ab53f901c462eed5e0fd34ef93` | 39,357,818 | `824da007e9d4ea47e430be3145c0cdcb3f8b982adf467e105998b4fd02ae03b1` | `75251b83cdc507c123fdd731d4b94733b6dcd9f752e50d7297862fb62cfec221` |
| `models/weapons/w_models/w_rocket.mdl` | `c5856b209922950a29e183245976ae76305f4b66aae1997fbe36c41b7d0f1a84` | 60,431 | `93d6da2b1914b56efac46eb7de751a0713ea9c377ef504bbbe9d02e27b61302d` | `6770d4c90a26b5d55316a396ea5b6b4fbe7cd093ebf4ec534c990ce64846d4e0` |
| `models/weapons/w_models/w_stickybomb.mdl` | `b577c7f40fec381f6a42128782effe52df76788ca245e82072f76c8d6fda3791` | 145,017 | `0a1d519d3ada6fe49e0ab9ab5d160de626ec39e4b85204c0e0ecbe3d057e2ab4` | `a4fc0efa81ceb888399cdf12b1c07caed644616c180a0f2b6fe053f81766f3d6` |
| `models/weapons/v_models/v_rocketlauncher_soldier.mdl` | `0eca831c2733188494763419c0e3ca6971fdc2715519f1a1e248d832e1509738` | 547,213 | `a29079d338c25eea18a2e947d1c58dc4b193938ccf07e672a5961590a48be6eb` | `d86487b69a77adbe0d454192b092fb8ea6c36ae2eb4475ba01e4f28ea2e34882` |
| `models/weapons/v_models/v_stickybomb_launcher_demo.mdl` | `1cbb38e6908762b0255437604ca4f7266ac9958b4a334456b665a6d9a9f15a84` | 929,616 | `1d13b532c529270a2b0eabf0b87b284b49d4fc6dcc45dd18b5d061bd9c47f2d0` | `e09068d6d4657ee260ab1eb5c8fbf735ac6527421ba11216cda592146726cb14` |

## Transform and vertex contract

- Cow and frog carry the static-prop header flag. Their world descriptor requires bone zero to equal the Source entity matrix. Every other target concatenates sampled model-space bones below that matrix.
- Positions, normals, tangent-S vectors, tangent W handedness, and UV pairs remain byte-identical to VVD values. The descriptor states +X forward, +Y left, +Z up and authored U-right/V-down UVs. No axis swap, texture-coordinate inversion, orientation offset, or model-name branch exists.
- The 33 exact target `prop_dynamic` occurrences produce transform SHA-256 `7a4eff4a2d9ca0892b6f576d21df4d44d03e03f957499c20245740b21b4edee6` from entity index, model identity, and all 12 `AngleMatrix` binary32 values. This includes the tilted frog at origin `[5368, -1792, -6640]`, angles `[55.9871, 178.212, -1.48216]`.

## Materials, skins, bodygroups, LODs, and PHY

The first selected materials are `cow001.vmt`, `frog001.vmt`, `resupply_locker.vmt`, `soldier_viking.vmt`, `soldier_red.vmt`, `demoman_red.vmt`, `w_rocket01.vmt`, `w_stickybomb_red.vmt`, `soldier_sleeves_red.vmt`, and `demoman_hands.vmt` under their exact `materials/models/**` identities. The locker additionally selects `models/items/ammo_box2.vmt`, `ammo_box1.vmt`, and `medkit.vmt`; those manifests select the no-alpha base textures and `item_selfillum.vtf`. Stickybomb red/blue manifests select `*_noalpha.vtf` plus `w_stickybomb_selfillum.vtf`. These exact choices prevent base-alpha data from becoming unintended model transparency.

Cow/frog have LOD 0 and missing optional PHY; the locker has LODs 0–4 and present PHY; Viking and sticky have LODs 0–2 and present PHY; Soldier/Demoman have LODs 0–6 and present PHY; rocket has LOD 0 and present PHY; both viewmodels have LOD 0 and missing optional PHY. Every skin table and bodypart-model cardinality is part of the PSMP hash and checked before artifact output.

## Animation and viewmodel contract

- Soldier and Demoman timelines cover primary stand, run, crouch, crouch-walk, jump-start, jump-float, jump-land, and stand attack. Exact authored autolayers execute in order; directional locomotion uses the authored 3×3 pose grid. Sequence FPS/CPS/duration and all cycles are in each timeline hash.
- Soldier viewmodel sequences are draw 25 frames, idle 41, fire 31, reload-start 16, looping reload 26, and reload-finish 24 at 30 FPS. Demoman viewmodel sequences are draw 31, idle 51, fire 19, reload-start 11, reload 21, and reload-end 17 at 30 FPS.
- Soldier draw event 5004 occurs at cycle `1/24`; Soldier reload events occur at `0.08` and `0.4`. Demoman draw event 5004 occurs at `1/30`; Demoman reload events occur at `0.1` and `0.6`. Event records retain fixed options/name bytes. Presentation queries preserve authored order and end-before-start loop ordering without executing effects.
- The viewmodel descriptor fixes default horizontal-4:3 FOV `54`, minimum `0.1`, maximum `179.9`, near plane `1`, caller-supplied world far plane, depth range `[0, 0.1]`, post-world opaque-then-translucent ordering, and optional view-space-Y handedness reflection.

IK rules/locks, flexes, and procedural bones remain explicit `RetainedNotEvaluated` families. Timeline hashes are the deterministic StudioModel sequence/grid/autolayer model-space seam before those separately classified effects; they do not claim post-IK or flex parity.
