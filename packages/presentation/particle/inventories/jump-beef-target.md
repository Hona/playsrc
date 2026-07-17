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

The executable function set is exactly two renderers (`render_animated_sprites`, `render_sprite_trail`), eight operators, eleven initializers, two emitters, `random force`, and `Collision via traces`, as enumerated in the package roadmap target rows.

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
