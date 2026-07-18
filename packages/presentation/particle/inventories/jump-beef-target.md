# `jump_beef` Projectile Particle Target

Authority: TF2 build `24207079`, exact configured `tf2_misc_dir.vpk` entries, and `jump_beef.psdb` SHA-256 `34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78`.

## PCF Inputs

| Logical path | Bytes | SHA-256 |
|---|---:|---|
| `particles/rockettrail.pcf` | 118,126 | `d6141fed629c3df3a6f4db190fe47fbbbf017662220d5804538edef453f42868` |
| `particles/rocketbackblast.pcf` | 5,922 | `a98a873842139ef24fac0331c2c8688478a85c62132ac8e86db6877babf0437f` |
| `particles/stickybomb.pcf` | 43,918 | `8496f1d12cd6cedff8f3db79f8df3c545184110333a9755f3cedf64ad3b303cf` |
| `particles/muzzle_flash.pcf` | 83,923 | `dae62e42e4e556b9b54073633870a7228f90155e45e3bbdf4006829c0eaef27b` |
| `particles/explosion.pcf` | 132,368 | `f0ebb89371c85113bdad6dc65fae0d484b1a1678f80e14ecea991ff2aaa13606` |

The selected roots are `rockettrail`, `rocketbackblast`, `stickybombtrail_red`, `stickybombtrail_blue`, `stickybomb_pulse_red`, `stickybomb_pulse_blue`, `muzzle_pipelauncher`, `ExplosionCore_Wall`, and `ExplosionCore_MidAir`.

Their complete 33-definition closure is `rockettrail`, `rockettrail_burst`, `rockettrail_fire`, `rocketbackblast`, `rocketbackblastsparks`, `stickybombtrail_red`, `stickybombtrail_blue`, `stickybomb_pulse_red`, `stickybomb_pulse_blue`, `muzzle_pipelauncher`, `muzzle_grenadelauncher_embers`, `muzzle_grenadelauncher_core`, `explosioncore_wall`, `explosion_debris001`, `explosion_dustup`, `explosion_dustup_2`, `explosion_coreflash`, `explosion_floatieembers`, `explosion_smoke_1`, `explosion_flash_1`, `explosion_flyingembers`, `explosion_flashup`, `explosioncore_midair`, `explosions_ma_coreflash`, `explosions_ma_dustup`, `explosions_ma_flash_1`, `explosions_ma_flashup`, `explosions_ma_floatieembers`, `explosions_ma_flyingembers`, `explosions_ma_smoke_1`, `explosions_ma_debris001`, `airburst_shockwave`, and `airburst_shockwave_d`.

The executable function set is exactly two renderers, eight operators, eleven initializers, two emitters, one force, and one constraint. The 33-definition closure contains 34 renderer, 136 operator, 197 initializer, 31 emitter, three force, five constraint, and 25 child occurrences.

## Executable Function Inventory

| Category | Identity | Occurrences |
|---|---|---:|
| Renderer | `render_animated_sprites` | 19 |
| Renderer | `render_sprite_trail` | 15 |
| Operator | `Alpha Fade and Decay` | 31 |
| Operator | `Color Fade` | 31 |
| Operator | `Movement Basic` | 28 |
| Operator | `Movement Lock to Control Point` | 5 |
| Operator | `Oscillate Scalar` | 3 |
| Operator | `Radius Scale` | 31 |
| Operator | `Rotation Basic` | 1 |
| Operator | `Rotation Spin Roll` | 6 |
| Initializer | `Alpha Random` | 24 |
| Initializer | `Color Random` | 31 |
| Initializer | `Lifetime Random` | 31 |
| Initializer | `Position Modify Offset Random` | 6 |
| Initializer | `Position Within Box Random` | 2 |
| Initializer | `Position Within Sphere Random` | 29 |
| Initializer | `Radius Random` | 26 |
| Initializer | `Rotation Random` | 22 |
| Initializer | `Rotation Speed Random` | 1 |
| Initializer | `Sequence Random` | 11 |
| Initializer | `Trail Length Random` | 14 |
| Emitter | `emit_continuously` | 16 |
| Emitter | `emit_instantaneously` | 15 |
| Force | `random force` | 3 |
| Constraint | `Collision via traces` | 5 |

Every listed occurrence uses its retained PCF parameter values. Target validation rejects another executable identity or parameter type. The three `Oscillate Scalar` declarations contain five legacy authoring attributes absent from the executable registry contract; those attributes remain typed and classified as inert rather than changing runtime state.

## Definition Execution Inventory

`R/O/I/E/F/C` is the ordered renderer/operator/initializer/emitter/force/constraint occurrence count. Child names retain declaration order and every target child delay is `0` seconds.

| Definition | Material | R/O/I/E/F/C | Children |
|---|---|---:|---|
| `rockettrail` | `effects/rocketrailsmoke.vmt` | `1/4/6/1/0/0` | `rockettrail_burst`, `rockettrail_fire` |
| `rockettrail_burst` | `effects/brightglow_y_nomodel.vmt` | `1/5/6/1/0/0` | — |
| `rockettrail_fire` | `effects/sc_brightglow_y_nomodel.vmt` | `1/4/5/1/0/0` | — |
| `rocketbackblast` | `effects/smokelit2/smoke2lit.vmt` | `1/5/5/1/0/0` | `rocketbackblastsparks` |
| `rocketbackblastsparks` | `effects/brightglow_y_nomodel.vmt` | `1/4/5/1/0/0` | — |
| `stickybombtrail_red` | `effects/sc_softglow.vmt` | `1/4/5/1/0/0` | — |
| `stickybombtrail_blue` | `effects/sc_softglow.vmt` | `1/4/5/1/0/0` | — |
| `stickybomb_pulse_red` | `effects/circle2.vmt` | `1/5/5/1/0/0` | — |
| `stickybomb_pulse_blue` | `effects/circle2.vmt` | `1/4/5/1/0/0` | — |
| `muzzle_pipelauncher` | `effects/softglow_translucent.vmt` | `1/4/6/1/0/0` | `muzzle_grenadelauncher_embers`, `muzzle_grenadelauncher_core` |
| `muzzle_grenadelauncher_embers` | `effects/brightglow_y_nomodel.vmt` | `1/5/6/1/1/0` | — |
| `muzzle_grenadelauncher_core` | `effects/brightglow_y_nomodel.vmt` | `1/5/6/1/0/0` | — |
| `ExplosionCore_Wall` | `effects/softglow.vmt` | `0/0/0/0/0/0` | `Explosion_Debris001`, `Explosion_Dustup`, `Explosion_CoreFlash`, `Explosion_FloatieEmbers`, `Explosion_Smoke_1`, `Explosion_Flash_1`, `Explosion_FlyingEmbers`, `Explosion_Flashup` |
| `Explosion_Debris001` | `effects/debris/debris_chunk.vmt` | `1/4/7/1/0/1` | — |
| `Explosion_Dustup` | `effects/softglow_translucent.vmt` | `1/5/8/1/0/0` | `Explosion_Dustup_2` |
| `Explosion_Dustup_2` | `effects/softglow_translucent.vmt` | `2/4/7/1/0/0` | — |
| `Explosion_CoreFlash` | `effects/softglow.vmt` | `1/5/8/1/0/0` | — |
| `Explosion_FloatieEmbers` | `effects/brightglow_y_nomodel.vmt` | `1/4/6/1/1/1` | — |
| `Explosion_Smoke_1` | `effects/smokelit2/smoke2lit.vmt` | `1/5/8/1/0/0` | — |
| `Explosion_Flash_1` | `effects/sc_brightglow_y_nomodel.vmt` | `1/3/7/1/0/0` | — |
| `Explosion_FlyingEmbers` | `effects/circle2.vmt` | `2/4/6/1/0/1` | — |
| `Explosion_Flashup` | `effects/softglow.vmt` | `1/5/8/1/0/0` | — |
| `ExplosionCore_MidAir` | `particle/smoke1/smoke1.vmt` | `0/0/0/0/0/0` | `Explosions_MA_coreflash`, `Explosions_MA_Dustup`, `Explosions_MA_Flash_1`, `Explosions_MA_Flashup`, `Explosions_MA_FloatieEmbers`, `Explosions_MA_FlyingEmbers`, `Explosions_MA_Smoke_1`, `Explosions_MA_Debris001`, `airburst_shockwave` |
| `Explosions_MA_coreflash` | `effects/softglow.vmt` | `1/5/8/1/0/0` | — |
| `Explosions_MA_Dustup` | `effects/softglow_translucent.vmt` | `1/5/8/1/0/0` | `Explosion_Dustup_2` |
| `Explosions_MA_Flash_1` | `effects/sc_brightglow_y_nomodel.vmt` | `1/3/6/1/0/0` | — |
| `Explosions_MA_Flashup` | `effects/softglow.vmt` | `1/5/8/1/0/0` | — |
| `Explosions_MA_FloatieEmbers` | `effects/brightglow_y_nomodel.vmt` | `1/4/6/1/1/1` | — |
| `Explosions_MA_FlyingEmbers` | `effects/circle2.vmt` | `2/4/6/1/0/1` | — |
| `Explosions_MA_Smoke_1` | `effects/smokelit2/smoke2lit.vmt` | `1/5/8/1/0/0` | — |
| `Explosions_MA_Debris001` | `effects/debris/debris_chunk.vmt` | `1/5/8/1/0/0` | — |
| `airburst_shockwave` | `effects/circle4.vmt` | `1/4/5/1/0/0` | `airburst_shockwave_d` |
| `airburst_shockwave_d` | `effects/circle3.vmt` | `1/4/4/1/0/0` | — |

## Texture Inputs

| Logical path | Bytes | SHA-256 | Sheet sequences / frames |
|---|---:|---|---:|
| `materials/effects/smoke/smokelit.vtf` | 176,420 | `332b3af22bf82027dd1131a9c4b6c855a8bb897f0383240ab9a29f5d5be491c4` | 4 / 20 |
| `materials/effects/brightglow_y.vtf` | 11,144 | `fec40b9e31f00b730973d63d154da57cb398d695acc00c569f98cbbfddc3f6c2` | 0 / 0 |
| `materials/effects/softglow.vtf` | 11,168 | `70f7f6225f06a03e4e1bd56796827fb0a0f875f38cdfa96ecad15ae3b07b2bfc` | 0 / 0 |
| `materials/effects/softglow_translucent.vtf` | 22,112 | `0970f336b2da7387c1292e87cf3848ffbd5224e28e0e2ea0d2e7760acb01d636` | 0 / 0 |
| `materials/effects/smokelit2/smoke2lit.vtf` | 175,440 | `2b75c9b3021a9df30b4ad897ad1d9cdea287024fc6dbf3db0642ec20bfec55c4` | 5 / 5 |
| `materials/effects/circle2.vtf` | 43,936 | `a7a7922b845df9846d424f787cb259e2e1de8c6dcc195f4f90af1fafdb16b864` | 0 / 0 |
| `materials/effects/circle3.vtf` | 87,640 | `236cf73d60d24088204070aa26a98aa9f48692d677e50b00a3e6d1436d72932c` | 0 / 0 |
| `materials/effects/circle4.vtf` | 87,640 | `cb6f6c674e114351b2f0cf5c53dc7476e229dcedab109f270cd692fb042f9c89` | 0 / 0 |
| `materials/effects/debris/debris_chunk.vtf` | 44,460 | `33edd989d4a7035d285b9e43d95adc6aa5b9c9d02eea9d99d33dea8ab60e894f` | 6 / 6 |
| `materials/particle/smoke1/smoke1.vtf` | 351,084 | `6564bf414fd789a7620f68dc92716a788a8856dd6c136ffbf1a36847526eae4e` | 16 / 16 |

Absent sheet resources are explicit single-image full-texture inputs; they are not missing animation data. Sheet-bearing textures retain each authored clamp flag, duration, frame duration, four image rectangles, and sequence number through typed VTF output before Particle samples them.

## Material Presentation Inputs

Particle receives these facts from Material; it does not parse VMT. `Additive` means source-alpha/one blending. `Alpha` means source-alpha/one-minus-source-alpha blending. Every base texture is sampled as sRGB and every particle tint is a linear vertex multiplier.

| Material | Shader family | Blend |
|---|---|---|
| `effects/rocketrailsmoke.vmt` | SpriteCard | Alpha |
| `effects/brightglow_y_nomodel.vmt` | Mesh sprite | Additive |
| `effects/sc_brightglow_y_nomodel.vmt` | SpriteCard | Additive |
| `effects/smokelit2/smoke2lit.vmt` | SpriteCard | Alpha |
| `effects/sc_softglow.vmt` | SpriteCard | Additive |
| `effects/circle2.vmt` | Mesh sprite | Additive |
| `effects/softglow_translucent.vmt` | Mesh sprite | Alpha |
| `effects/debris/debris_chunk.vmt` | SpriteCard | Alpha |
| `effects/softglow.vmt` | Mesh sprite | Additive |
| `particle/smoke1/smoke1.vmt` | SpriteCard | Alpha |
| `effects/circle4.vmt` | SpriteCard | Alpha |
| `effects/circle3.vmt` | SpriteCard | Alpha |

None of the target SpriteCard materials enables dual-sequence rendering. SpriteCard animation ignores `animation_fit_lifetime`; mesh-sprite animation applies it. A SpriteCard material suppresses `render_sprite_trail`, while every target trail occurrence resolves a mesh-sprite material.

## Deterministic Timeline Evidence

`cargo test -p playsrc-particle --test exact_content -- --ignored` reads only the configured `jump_beef.psdb`, verifies the byte length and fixed FNV-1a identity of all five PCFs and ten VTFs above, advances with seed `1337 + root ordinal`, control point 0 at `[10,20,30]`, identity orientation, no-hit collision results, camera `[100,50,25]`, and `0.05`-second maximum steps. `rockettrail` receives a graceful stop at `1.00`; every other root reaches natural completion. Each cell is the complete resolved render-record count.

| Root | 0.00 | 0.05 | 0.10 | 0.15 | 0.20 | 0.25 | 0.50 | 1.00 | 1.50 | 2.00 | 2.50 | 3.00 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `rockettrail` | 1 | 20 | 40 | 61 | 81 | 90 | 128 | 195 | 81 | 3 | 0 | 0 |
| `rocketbackblast` | 0 | 15 | 18 | 16 | 14 | 13 | 9 | 6 | 6 | 0 | 0 | 0 |
| `stickybombtrail_red` | 0 | 0 | 0 | 0 | 0 | 12 | 76 | 102 | 102 | 102 | 26 | 0 |
| `stickybombtrail_blue` | 0 | 0 | 0 | 0 | 0 | 12 | 76 | 102 | 102 | 102 | 26 | 0 |
| `stickybomb_pulse_red` | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `stickybomb_pulse_blue` | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `muzzle_pipelauncher` | 0 | 25 | 30 | 17 | 17 | 16 | 14 | 0 | 0 | 0 | 0 | 0 |
| `ExplosionCore_Wall` | 0 | 153 | 149 | 127 | 111 | 101 | 79 | 56 | 27 | 11 | 0 | 0 |
| `ExplosionCore_MidAir` | 0 | 159 | 155 | 138 | 119 | 104 | 77 | 58 | 26 | 11 | 0 | 0 |

The FNV-1a 64 identity of the 126 concatenated version-3 batches at `0.00`, `0.05`, `0.10`, `0.15`, `0.20`, `0.25`, `0.50`, `1.00`, `1.50`, `2.00`, `2.50`, `3.00`, `3.50`, and `4.00`, in table order, is `602405073279b21f`. The batch bytes include bounds and every ordered primitive identity, material shader/blend/color-space fact, current/prior/trail position, radius, roll, yaw, tint, quantized alpha, sequence, primary/secondary sheet sample, frame rectangles, frame blend, animation mode/rate, trail scale/width/length, orientation, sort key, and stable tie identity. Empty batches retain the exact versioned header.
