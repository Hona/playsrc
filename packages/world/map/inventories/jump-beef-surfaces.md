# `jump_beef` World Surface And Mark Inventory

Source identity: configured `maps/jump_beef.bsp`, 33,379,388 bytes, SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`, Source-2013 BSP version 20, revision 731.

Dependency closure identity: 317-entry `PSDB` source bundle, 120,412,213 bytes, SHA-256 `c8ccea4035c5e75e26ffc0855a425ff4139f079f35ab9abd09e22990726f03d5`. The bundle contains unchanged exact bytes selected from the active BSP PAK and declared TF2/HL2 VPK providers.

## Surface totals

- Selected HDR faces: 3,793.
- Face vertices: 16,291.
- Source triangles: 8,705.
- Texdata material identities: 14.
- Selected-face material occurrences: 3,793; drawable occurrences: 2,992. Sky and trigger faces remain canonical non-rasterized surfaces rather than debug materials.
- Canonical UV origin is top-left. Each UV is the BSP texinfo texture-vector dot product divided by the texdata mapping width or height. VTF row zero is top; Map performs no row inversion.
- Each surface separately retains texinfo index, stored BSP plane, face-side bit, texture/lightmap vectors, mapping dimensions, and oriented render normals.

## Ordered texdata materials

| Index | Logical path | Mapping | Faces | Drawn | Effective shader | Exact root VMT SHA-256 |
|---:|---|---:|---:|---:|---|---|
| 0 | `materials/WOOD/WOOD_FLOOR002.vmt` | 1024×1024 | 584 | 584 | `LightmappedGeneric` | `6fa0548ebc9fd728b7c6e266517921a3b8670d1f198fa086c08a3c892c3e3967` |
| 1 | `materials/TOOLS/TOOLSNODRAW.vmt` | 64×64 | 0 | 0 | `LightmappedGeneric` | `354c52037c197e7c6de2ce92f97caaaaf695fa84812610959b91a69db2210ff7` |
| 2 | `materials/WOOD/WALL020B.vmt` | 1024×1024 | 1,511 | 1,511 | `LightmappedGeneric`, SSBump | `52cce0f07819f7539d085af040bfb1eb17f2e8b60244343d12b0ef45d6d82857` |
| 3 | `materials/WOOD/WALL007A.vmt` | 1024×1024 | 694 | 694 | `LightmappedGeneric` | `04eb647d4ee116aed94f472876dcb6132242143136e5143c139185485ed35c95` |
| 4 | `materials/WOOD/WALL015B.vmt` | 1024×1024 | 78 | 78 | `LightmappedGeneric` | `837ae6bd98e4e29a2e5e3c94e78506d498bdfb8682936327ef0ef3f8e673065f` |
| 5 | `materials/TOOLS/TOOLSSKYBOX.vmt` | 128×128 | 91 | 0 | `LightmappedGeneric`, compile-sky | `965ec64ab9377988519b76326f492bc3178fbe964ae9c5d29d36feae26b6a20d` |
| 6 | `materials/CONCRETE/CONCRETEWALL047A.vmt` | 512×512 | 12 | 12 | `LightmappedGeneric` | `9c70132190af82e13f9a3f8e20f5524b94dc885424f2de575c73ba7c014980c4` |
| 7 | `materials/TOOLS/TOOLSTRIGGER.vmt` | 64×64 | 710 | 0 | `LightmappedGeneric`, translucent compile-trigger | `05b87cd379759d5edb12185136eb4cffd9a185af799fe6a4aca42dff9e4e5f07` |
| 8 | `materials/METAL/METALFENCE007A.vmt` | 512×512 | 50 | 50 | `LightmappedGeneric`, alpha test `GEQUAL 0.35` | `ee3c58d2a7d56cb33c34f1bad03a72a1df4b73eadde0561bfe20d4af21cc8255` |
| 9 | `materials/maps/jump_beef/water/water_2fort_expensive_-4787_3137_-2159.vmt` | 256×256 | 4 | 4 | `Water` patch | `2824deec4ec65df3ee5d5ef8e4c3419145ff5ff33479fe005518b859376f6335` |
| 10 | `materials/maps/jump_beef/glass/glasswindow002a_12672_539_-2562.vmt` | 512×512 | 36 | 36 | translucent `LightmappedGeneric` patch | `061c7d431d173108f5ab1169a68c8a5451f71791631ab2818c9200f3bad414e0` |
| 11 | `materials/maps/jump_beef/glass/glasswindow002a_-4787_3137_-2159.vmt` | 512×512 | 12 | 12 | translucent `LightmappedGeneric` patch | `c941a7abbc26a982cbdc6d80135f7863a8bf0d052394c272325794410dff542e` |
| 12 | `materials/maps/jump_beef/glass/glasswindow002a_12672_683_-4448.vmt` | 512×512 | 7 | 7 | translucent `LightmappedGeneric` patch | `500bfab3af7b24efafd832b7ef25355b2c19eb08d0b13ba407d4751144aa697e` |
| 13 | `materials/water/water_2fort_beneath.vmt` | 256×256 | 4 | 4 | `Water`, below-water | `118cae4c43eda381491f99c0753fbc8963b35c2de787ee58194d3d7feaa028c8` |

Patch bases are exact `materials/water/water_2fort_expensive.vmt` at SHA-256 `5f61b7786628a7e267419a7b709548102c115f2ef1f468bd3e3dc73aa6349806` and `materials/glass/glasswindow002a.vmt` at SHA-256 `4d245323e238a2823ef992dd3a62d424f56d1270ebd1b98d1cc03d318be5d04e`. The empty `Proxies` object in the water patch does not delete inherited proxy children.

## Marks

- Entity records: 39 ordered `infodecal` entities.
- Material references: `signs/number_00` through `signs/number_09`, `signs/arrow_lt_blue`, `signs/arrow_rt_blue`, `signs/arrow_up_blue`, and missing `decals/custom/interro_ad`.
- Dispositions: 38 projected and one missing material. Collision receiver selection removes the four false ineligible results produced by cross-model render-polygon search.
- Geometry: 73 fragments, 292 vertices.
- Every fragment uses the stored BSP plane normal for basis and is moved exactly 0.1 Source units along that normal after clipping. Number materials use scale 1; arrows use scale 0.25. Mapping dimensions remain floating point after scale.
- Standard overlay, overlay-fade, and water-overlay inputs are empty. Their empty outputs are intentionally inert.

Door-number entities 220, 221, and 222 select current entities/models `216/*93`, `217/*94`, and `218/*95` respectively. Every retained fragment belongs to its collision-selected model. The canonical `playsrc-map-mark-stream-v2` identity covers all 39 source-ordered records: disposition/kind/activation/lifetime flags, material path, receiver entity/model/local origin/transform, target faces, and count-framed fragment model/face/position/UV/triangle streams. Its SHA-256 is `dc240ad45952f19150071cf235b433dcd1d035fd3c2f3afad55e9bd1f84d26c7`.

## Exact execution

- `bun packages/world/map/scripts/verify-parity.ts` validates package tests, both configured inventories, formatting, and stable Clippy.
- Collision world identity is `66d42c750648487669e1b9d7a1b36fc81e213624030f812667fb728ee61aa6ed`.
- Mark stream identity is `dc240ad45952f19150071cf235b433dcd1d035fd3c2f3afad55e9bd1f84d26c7`.
- Environment source counts are 284 clusters, 1,775 nodes, 1,899 leaves, 91 sky faces, three cubemaps, 16 profile-qualified water surfaces, one water volume, 39 marks, 73 mark fragments, and two controllers.
- Application-owned runtime and presentation payload identities are not Map evidence and are not retained here.
