# `jump_beef` Alpha, Translucency, SpriteCard, And Water Inventory

Source identity: configured build `24207079` `jump_beef.psdb`, 317 exact entries, 120,412,213 bytes, SHA-256 `c8ccea4035c5e75e26ffc0855a425ff4139f079f35ab9abd09e22990726f03d5`. Its typed closure contains 108 VMTs and 126 VTFs. The VMT set contains 55 selected model materials, 12 projectile-particle materials, 13 present mark materials, the world fence and glass families, and both effective Water roots.

The 42 base VTFs declaring one-bit or eight-bit alpha divide exactly into 19 model, 13 mark, seven particle, one fence, one glass, and one non-rasterized trigger texture. Vertex alpha additionally drives all 12 particle materials, including five whose base VTF does not declare alpha.

## World And Marks

| Effective material identities | Occurrences | Alpha owner | Static state |
|---|---:|---|---|
| `materials/METAL/METALFENCE007A.vmt` | 50 faces | Base VTF alpha test | `GEQUAL 0.35`; blending disabled; depth test/write enabled; back cull |
| `materials/maps/jump_beef/glass/glasswindow002a_{-4787_3137_-2159,12672_539_-2562,12672_683_-4448}.vmt` | 12, 36, and 7 faces | Base VTF opacity | source-alpha/one-minus-source-alpha; depth test enabled; depth write disabled; back cull |
| `materials/signs/number_{00,01,02,03,04,05,06,07,08,09}.vmt` | 36 entities total | Base VTF opacity plus vertex alpha | source-alpha/one-minus-source-alpha; depth test enabled; depth write disabled; decal offset; back cull |
| `materials/signs/arrow_{lt_blue,rt_blue,up_blue}.vmt` | one entity each | Base VTF opacity plus vertex alpha | source-alpha/one-minus-source-alpha; depth test enabled; depth write disabled; decal offset; back cull |
| `materials/TOOLS/TOOLSTRIGGER.vmt` | 710 canonical faces, zero draws | Base VTF opacity | Classified translucent compile-trigger input; never promoted to drawable output |

The fence root VMT SHA-256 is `ee3c58d2a7d56cb33c34f1bad03a72a1df4b73eadde0561bfe20d4af21cc8255`; its 512×512, ten-mip, one-frame DXT5 base VTF SHA-256 is `51431161fb9ad7aeb6d8df5d8f50a16f0005e08ba403bda55cf36b2883ddf293` and its mip-zero plane contains 160,677 alpha-zero texels. The shared glass root and base VTF hashes are `4d245323e238a2823ef992dd3a62d424f56d1270ebd1b98d1cc03d318be5d04e` and `a79d35afaa7643a8f5e672394a03ded73876f716f93c27bd3cf2d3ebfef11970`. Every present mark mip-zero plane contains alpha-zero texels; their exact VMT/VTF identities and hashes are fixed by the source closure and [`../../formats/vtf/inventories/jump-beef-world-textures.md`](../../formats/vtf/inventories/jump-beef-world-textures.md).

## Models

The selected model set has 55 VMT identities and 71 source VTF identities. Complete VMT/VTF hashes, semantic roles, and authored `(mip, frame, face, slice)` chains have SHA-256 `05c7869e3f78b03b2c9f05ebdb9a8ec8f9895a8a8d3ff37530f3e0d26f617033`.

Nineteen base VTFs declare alpha. Their ownership is exhaustive:

- Alpha test: `models/player/{demo/demoman_{blue,red}_zombie_{alphatest,invun},soldier/soldier_{blue,red}_zombie_{alphatest,invun}}.vmt` (eight).
- Phong mask: `models/player/items/soldier/medals{,_blue}.vmt`, `models/player/soldier/soldier_hands.vmt`, `models/weapons/c_models/c_rocketlauncher/c_rocketlauncher{,_gold}.vmt`, `models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher{,_gold}.vmt`, and `models/weapons/w_rocketlauncher/w_rocket01.vmt` (eight).
- Tint mask: `models/player/items/soldier/soldier_viking{,_blue}.vmt` (two).
- Self-illumination mask: `models/props_2fort/frog001.vmt` (one).

At fully visible alpha modulation and cloak factor zero, all 55 materials classify opaque, eight retain alpha test, and five require the current framebuffer for authored sheen. Current draw-state identity is `12770b45364a035c81a0fd96fdd84dd762d97540a640bbfbb44db290d7b2014d`.

## Projectile Particles

| Material | Shader | Blend | Base-alpha flag |
|---|---|---|---|
| `effects/rocketrailsmoke.vmt` | SpriteCard | source alpha | eight-bit |
| `effects/brightglow_y_nomodel.vmt` | UnlitGeneric mesh sprite | additive | none |
| `effects/sc_brightglow_y_nomodel.vmt` | SpriteCard | additive | none |
| `effects/smokelit2/smoke2lit.vmt` | SpriteCard | source alpha | eight-bit |
| `effects/sc_softglow.vmt` | SpriteCard | additive | none |
| `effects/circle2.vmt` | UnlitGeneric mesh sprite | additive | none |
| `effects/softglow_translucent.vmt` | UnlitGeneric mesh sprite | source alpha | eight-bit |
| `effects/debris/debris_chunk.vmt` | SpriteCard | source alpha | eight-bit |
| `effects/softglow.vmt` | UnlitGeneric mesh sprite | additive | none |
| `particle/smoke1/smoke1.vmt` | SpriteCard | source alpha | eight-bit |
| `effects/circle4.vmt` | SpriteCard | source alpha | eight-bit |
| `effects/circle3.vmt` | SpriteCard | source alpha | eight-bit |

All eight SpriteCard materials use shader-output alpha test `GREATER 0.01`, two-sided culling, and no depth write. None enables dual sequence. `particle/smoke1/smoke1.vmt` selects authored depth blend `1` in the configured non-low-fill environment; `circle3`, `circle4`, `debris_chunk`, and `sc_softglow` author `0`; `rocketrailsmoke`, `sc_brightglow_y_nomodel`, and `smoke2lit` require the caller-supplied process-selected SpriteCard default.

## Water

| Identity | Effective shader | Occurrences | Defined bindings | Proxy order |
|---|---|---:|---|---|
| `materials/maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159.vmt` composed with `materials/water/water_2fort_expensive.vmt` | `Water_DX90` in LDR; `Water_DX9_HDR` in HDR | four per profile | normal, declared map cubemap, reflection framebuffer, refraction framebuffer, beneath material | AnimatedTexture, Sine, Sine, Equals, Equals, TextureTransform, WaterLOD |
| `materials/water/water_2fort_beneath.vmt` | `Water_DX90` in LDR; `Water_DX9_HDR` in HDR | four per profile | normal, refraction framebuffer, underwater overlay; environment and reflection absent | AnimatedTexture, TextureScroll, WaterLOD |

The patch, surface base, and beneath VMT SHA-256 values are `2824deec4ec65df3ee5d5ef8e4c3419145ff5ff33479fe005518b859376f6335`, `5f61b7786628a7e267419a7b709548102c115f2ef1f468bd3e3dc73aa6349806`, and `118cae4c43eda381491f99c0753fbc8963b35c2de787ee58194d3d7feaa028c8`. Both use `materials/water/tfwater001_normal.vtf`, SHA-256 `7b5de49340bfe1ec2f1e37d771289d42773414f130767b5632ca29467494c017`: VTF 7.3, 256×256, nine authored mips, 60 frames, one face, BGR888, normal-map flag, repeat wrap, and no generated resource.

The surface is above water with reflection `0.25`, refraction `0.32`, fog `{51 43 13}` from `1` through `400`, and a linked beneath material. The beneath material is below water with no environment/reflection binding, refraction `0.5`, tint `[0.95 1.0 0.97]`, fog `{92 100 80}` from `-350` through `1050`, blur enabled, and overlay `effects/water_warp_2fort`. WaterLOD supplies `1000..2000`. No configured Water VMT defines a base texture or flow map; neither role is inferred from another texture.

The fixed semantic vector concatenates fence reference, 204,175 mark alpha-zero texels, model alpha/ownership counts, SpriteCard/blend/default counts, Water proxy counts, the 540 normal-map subresource identities, and frame `30` at one second. Its SHA-256 is `ccac4488cf11a82cd43511bdf83300bf799b3788ad0448481736d5118cc64f2a`.
