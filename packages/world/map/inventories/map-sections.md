# Map Section Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

## Metadata

| Field | Value |
|---|---|
| Authority identity | playsrc Owner Registry and Map boundary; Valve Source SDK 2013 `src/public/bspfile.h`, `gamebspfile.h`, `worldsize.h`, `vphysics_interface.h`, `src/utils/vbsp/writebsp.cpp`, and named game-side map consumers; accepted producer registries; exact declared-content map indexes |
| Authority revision | playsrc `8ad1ca705dab2ec23c7d1deaf0ee84c5342357f9`; SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; producer registries and declared-content indexes Missing |
| Generator command | Missing |
| Output path | `packages/world/map/inventories/map-sections.md` |
| Item count | 27 candidate items |
| Accepted item count | 0 |
| Owning roadmap | `packages/world/map/ROADMAP.md` |
| Behavior owner | `packages/world/map` |

Every successful canonical map contains one descriptor for every accepted section identity in this order. An empty source universe does not remove a section. It produces an `Intentionally inert` zero-item section only under the row's exact empty rule. `packages/world/map` is the sole behavior owner of every inventory item and owns section identity, occurrence joins, and completeness. Each Ownership boundary cell names adjacent child semantics that remain excluded from the inventory item.

## Candidate Sections

| Order | Stable section identity | Coordinated content | Stable item identity rule | Ownership boundary | Empty rule |
|---:|---|---|---|---|---|
| 1 | `spatial.coordinate-system` | Source X-forward/east, Y-left/north, Z-up frame and pitch/yaw/roll convention | Singleton `source-xyz` | `packages/world/map` | Never empty; Missing is fatal. |
| 2 | `spatial.units` | One-inch position unit and degree angle unit | Singleton `source-inch-degree` | `packages/world/map` | Never empty; Missing is fatal. |
| 3 | `spatial.bounds` | Nominal coordinate interval, world gameplay bounds, and every brush-model bound | `coordinate-domain`, `world-gameplay`, then `brush-model:<BSP model index>` | `packages/world/map` | Never empty; model 0 and world gameplay bounds are required. |
| 4 | `geometry.world` | Model-0 face topology and source vertex, edge, surfedge, normal, primitive, material, lighting, leaf, and non-rasterized references | `face:<BSP face index>` | `packages/world/map` | Zero faces are Intentionally inert only when model 0 has an empty validated face range. |
| 5 | `geometry.displacements` | Parent-face occurrence plus displacement sample, neighbor, allowed-vertex, triangle-tag, material, lighting, collision, and visibility references | `displacement:<BSP displacement index>` | Occurrence and joins: `packages/world/map`; child semantics: named collision, visibility, and rendering owners | Empty BSP displacement set is Intentionally inert. |
| 6 | `geometry.brush-models` | World and `*N` submodel face ranges, head nodes, bounds, origins, entities, collision, and visibility policy references | `brush-model:<BSP model index>` | `packages/world/map` | Never empty; `brush-model:0` is required. |
| 7 | `lighting.world-lights` | Selected LDR or HDR world-light records and owning entity references | `world-light:<selected profile>:<BSP world-light index>` | Evaluation: `packages/presentation/rendering`; profile and identity coordination: `packages/world/map` | Empty selected world-light set is Intentionally inert. |
| 8 | `lighting.lightmaps` | Selected face sample ranges, lightmap extents/vectors, ambient records, and static/detail prop lighting references | `lightmap:<selected profile>:face:<BSP face index>` plus owner-supplied ambient and prop-light identities | Interpretation and atlasing: `packages/presentation/rendering`; reference coordination: `packages/world/map` | Intentionally inert only when the selected profile has no referenced samples or ambient/prop lighting. |
| 9 | `lighting.light-styles` | Every style referenced by selected faces, world lights, and detail-prop lighting | `light-style:<0-63>` | Animation and composition: `packages/world/entity` and `packages/presentation/rendering`; reference coordination: `packages/world/map` | Empty when no selected record references a style; otherwise every referenced style is required. |
| 10 | `resources.materials` | Every map material occurrence and exact current material artifact | `material-reference:<producer role>:<source index>`; duplicate logical paths remain distinct occurrences | `packages/world/material` | Intentionally inert only when no section contains a material occurrence. |
| 11 | `resources.models` | Every static/detail/dynamic/entity model occurrence and exact current StudioModel artifact | `model-reference:<producer role>:<source item identity>` | Parsing and model semantics: `packages/formats/studio-model`; occurrence: `packages/world/map` or `packages/world/entity` | Intentionally inert only when no section contains a model occurrence. |
| 12 | `instances.static-props` | Static-prop dictionary and instances, transforms, leaves, skin, solidity, flags, fades, model, collision, visibility, and selected lighting references | `static-prop:<game-lump entry identity>:<instance index>` | Placement and joins: `packages/world/map`; model, collision, visibility, and rendering semantics remain with child owners | Missing `sprp` and a validated zero-instance `sprp` are distinct Intentionally inert states. |
| 13 | `instances.detail-props` | Detail model/sprite dictionaries and instances, transforms, leaf, lighting, styles, orientation, shape, scale, sway, material, and model references | `detail-prop:<game-lump entry identity>:<instance index>` | Placement and joins: `packages/world/map`; model/material/rendering semantics remain with child owners | Missing `dprp` and a validated zero-instance `dprp` are distinct Intentionally inert states. |
| 14 | `marks.decals` | Map-authored decal entity occurrences and their material, entity, target, and projection-input references | Owner-supplied `decal:<entity item identity>` | Entity state: `packages/world/entity`; projection and pixels: `packages/presentation/rendering`; joins: `packages/world/map` | No decal occurrence is Intentionally inert. |
| 15 | `marks.overlays` | Compiled overlay and water-overlay occurrences, faces/models, material, render order, fade, and projection-input references | `overlay:<kind>:<BSP record index>`, where kind is `standard` or `water` | Projection and pixels: `packages/presentation/rendering`; joins: `packages/world/map` | Both overlay record sets empty is Intentionally inert. |
| 16 | `environment.cubemaps` | Cubemap sample origin, size, selected profile, exact texture logical identity, material references, and sampling inputs | `cubemap:<BSP cubemap index>` | Sampling and pixels: `packages/presentation/rendering`; texture semantics: `packages/world/material`; joins: `packages/world/map` | No cubemap samples is Intentionally inert; an encountered sample with a Missing texture is not. |
| 17 | `environment.sky` | Worldspawn sky identity, sky-surface occurrences, and six exact selected-profile material/resource references | Singleton `sky-2d` plus `sky-surface:<BSP face index>` | Entity value: `packages/world/entity`; resources: `packages/world/material`; presentation: `packages/presentation/rendering`; joins: `packages/world/map` | Intentionally inert only when no face or leaf requires 2D sky. |
| 18 | `environment.sky-3d` | Sky-camera occurrence, origin, scale, area, sky-marked world references, and fog reference | `sky-3d:<entity item identity>` | Entity behavior: `packages/world/entity` and `games/<game>`; presentation: `packages/presentation/rendering`; joins: `packages/world/map` | No sky-camera and no 3D-sky leaf requirement is Intentionally inert. |
| 19 | `environment.fog` | Main-view and 3D-sky fog controller occurrences plus geometry and area references | `fog:<entity item identity>` | `packages/world/entity`, selected game, and `packages/presentation/rendering` | No fog controller occurrence is Intentionally inert. |
| 20 | `environment.water` | Water faces, leaf-water records, water volumes, selected materials, collision/visibility references, and water-controller references | `water-surface:face:<BSP face index>` and `water-volume:<BSP leaf-water index>` | Volume occurrence and joins: `packages/world/map`; material, collision, visibility, entity, and rendering semantics remain with child owners | No water face, leaf-water record, or water contents occurrence is Intentionally inert. |
| 21 | `environment.ropes` | Rope entity occurrence, endpoints, attachments, material/model, collision, and entity references | `rope:<entity item identity>` | `packages/world/entity`, runtime simulation, selected game, and `packages/presentation/rendering` | No rope entity occurrence is Intentionally inert. |
| 22 | `environment.controllers` | Map environment-controller occurrences not owned by sky, fog, water, or rope sections, including lighting, shadow, tone-map, and detail controllers | `environment-controller:<entity item identity>` | `packages/world/entity`, selected game, and `packages/presentation/rendering` | No recognized controller occurrence is Intentionally inert; an unclassified environment class is Unknown. |
| 23 | `entities` | One current entity graph and every stable entity item required by another section | `entity:<entity-owner stable index>` | `packages/world/entity` and `games/<game>` | Never empty; exactly one worldspawn occurrence is required. |
| 24 | `collision` | One current collision-world root plus model, face, displacement, prop, trigger, contents, and surface join identities | `collision-root` plus collision-owner stable child identities | `packages/world/collision` | The root is required; its shape set may be empty only when the collision owner classifies it Intentionally inert. |
| 25 | `visibility` | One current visibility root plus leaf, cluster, area, portal, occluder, face, displacement, prop, and water join identities | `visibility-root` plus visibility-owner stable child identities | `packages/world/visibility` | The root is required; an empty visibility set must be classified by that owner. |
| 26 | `surface-properties` | Material-owned surface-property identities used by world faces, brush collision, props, models, decals, overlays, and water | `surface-property-reference:<producer role>:<source item identity>` | `packages/world/material`; collision, physics, audio, and game packages consume its identity | Intentionally inert only when no source item references a surface property. |
| 27 | `metadata` | Request identity, BSP profile/version/revision, worldspawn metadata, source provenance, selected lighting profile, direct/transitive input hashes, build-configuration hash, and compiler-behavior identity | Singleton `map-metadata` | `packages/world/map` | Never empty; Missing is fatal. |

## Candidate Cross-Section Edges

The generator must classify every edge emitted by a producer. At minimum it validates these finite edge families:

- world face to brush model, vertices, normals, primitive, material, displacement, lightmap, leaf, overlay, collision, and visibility identities;
- displacement to parent face, sample ranges, neighbors, material, lightmap, collision, and visibility identities;
- entity to world/brush model, decal, sky, fog, water, rope, controller, material, model, collision, and visibility identities;
- static/detail prop to dictionary entry, model or material, leaves, lighting/style, collision, and visibility identities;
- water to face, leaf-water, material, collision, visibility, fog, and controller identities;
- selected lighting profile to faces, samples, world lights, ambient records, detail/static prop lighting, cubemaps, and sky resources;
- every non-metadata section descriptor to its exact direct dependency artifacts and the singleton metadata identity; `metadata` has no self-edge.

An edge with no accepted owner or target identity remains in generated output as Unknown and blocks acceptance. Generation fails rather than omitting it.

## Acceptance Blockers

- Implement one checked-in generator owned by `tools/playsrc`.
- Accept current machine-readable section, producer, item-identity, dependency-role, artifact-media-type, and edge declarations.
- Supply exact archive indexes for one declared TF2, CS:S, and legacy Source 1 CS:GO content build and retain every map discovery by logical path and SHA-256.
- Resolve every producer contract and artifact media type named in the owning roadmap.
- Run the generator twice from clean workspaces and require byte-identical output containing exactly 27 stable section identities.
- Record an Accepted denominator review with reviewer, date, reviewed commit, authority revisions, generator command, output hash, and all roadmap-contract predicates passing.
