# `jump_beef` World Environment Inventory

Source identity: configured `maps/jump_beef.bsp`, decoded SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`, Source-2013 BSP version 20, revision 731.

## Selected environment

| Output | Exact source facts |
|---|---|
| 2D sky | Worldspawn `skyname=sky_day01_01`; selected HDR materials are `materials/skybox/sky_day01_01_hdr{rt,lf,bk,ft,up,dn}.vmt` in that order. |
| Controllers | Entity 170 is `light_environment`; entity 360 is `water_lod_control` with start `1000` and end `2000`. No `env_fog_controller`, `sky_camera`, `shadow_control`, or `env_tonemap_controller` occurs. |
| Lighting provenance | Explicit HDR profile; 3,793 faces, 3,896,843 RGBExp32 samples, 73 world lights, 1,899 ambient indexes, and 9,014 ambient samples. Map retains these inputs without exposure or tone mapping. |

The environment light retains `_light=255 255 255 2000`, `_ambient=255 255 255 2000`, `_lightHDR=-1 -1 -1 1`, `_ambientHDR=-1 -1 -1 1`, both HDR scales `1`, angles/pitch `0`, and `SunSpreadAngle=0`.

## Cubemaps

| Index | Integer origin | Encoded size | LDR request | HDR request |
|---:|---|---:|---|---|
| 0 | `[-4787,3137,-2159]` | 0 | `materials/maps/jump_beef/c-4787_3137_-2159.vtf` | `materials/maps/jump_beef/c-4787_3137_-2159.hdr.vtf` |
| 1 | `[12672,539,-2562]` | 0 | `materials/maps/jump_beef/c12672_539_-2562.vtf` | `materials/maps/jump_beef/c12672_539_-2562.hdr.vtf` |
| 2 | `[12672,683,-4448]` | 0 | `materials/maps/jump_beef/c12672_683_-4448.vtf` | `materials/maps/jump_beef/c12672_683_-4448.hdr.vtf` |

Each resource is 32×32 with six mips. The VTF source contains seven faces; the render-neutral sample request names right, left, back, front, up, and down for every mip. The three LDR bytes share SHA-256 `627caf57bfe16e869a64b282d6bc39663ff4682cde2c5b244772a617db2a353a`. HDR indexes 0, 1, and 2 have SHA-256 `b3f13af032931bd21fc6f9c873f07b3718b1dc2a8c86ccebdca514e8826ab7e8`, `c389984d1f4f1941b36da93e18c012d965fe91e96e16d03a1dda637cea814f19`, and `a5e761098727e2babe50e59fa27ef293d0b49d4293f824a64474c4ccd07026c9`. The fixed camera `[5328,3376,-3067.2099609375]` selects index 1. Missing selected bytes are `Missing`; neither sky nor another profile is queried.

## Water

| Fact | Exact value |
|---|---|
| Materials | Index 9 `maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159`; index 13 `water/water_2fort_beneath.vmt` |
| Surface occurrences | 16 total: eight LDR and eight HDR; each profile contains four surface and four beneath occurrences |
| Selected surfaces | Eight under explicit HDR |
| Leaf-water records | One: surface Z `-2160`, minimum Z `-2416`, surface texinfo 454 |
| Member leaves | 663, 675, 886, 911 |
| Member clusters/area | Clusters 188, 191, 252, 262; area 5 |
| Leaf contents/distance | Each member is `0x10000020`; BSP lump 46 contains exactly 1,899 `u16` values, each member is 0, and 1,860 leaves retain sentinel 65,535 |
| Volume bounds | `[-5216,2304,-2416]` through `[-4448,3792,-2160]` |
| Volume plane | normal `[0,0,1]`, distance `-2160` |

The surface material's declared environment path selects cubemap sample 0 under both profiles and explicitly binds reflection and refraction. Surface material 9 joins bottom material 13. The exact beneath material reports `aboveWater=false`, explicitly binds refraction, and has no effective environment or reflection parameter; all eight beneath profile surfaces retain `environment=None` and `reflection=false`. The eight above-water profile surfaces and the volume's surface binding retain declared sample 0. The volume's bottom binding and an underwater view plan retain `environment=None`, `reflection=false`, and `refraction=true` without rejection or fallback. View planning accepts an eye leaf plus ordered PVS/area/frustum-qualified candidate leaves and explicit platform policy. It emits no-water/cheap or Source-ordered reflection, refraction, main, and intersection plans with categorical fog/sky/entity admission, forced reflection leaf, mirrored camera, and two-unit clip-plane requests.

## Marks

The entity graph contains 39 ordered `infodecal` records. Exact Collision world/snapshot receiver evidence classifies 38 projected and one missing material, producing 73 bounded fragments and 292 vertices. Door-number entities 220, 221, and 222 select `216/*93`, `217/*94`, and `218/*95`. Every receiver polygon is clipped against the decal unit square in Source plane-basis order, and every clipped position is moved 0.1 Source units along its stored BSP plane normal while retaining a categorical decal polygon-offset request. The complete receiver/geometry stream defined in [`jump-beef-surfaces.md`](jump-beef-surfaces.md) has SHA-256 `dc240ad45952f19150071cf235b433dcd1d035fd3c2f3afad55e9bd1f84d26c7`. Standard overlay and water-overlay lumps are empty. Empty compiled mark lumps are intentionally inert; an unknown nonempty version or malformed whole record is not.
