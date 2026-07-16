# Rendering Presentation-Family Inventory

## Metadata

| Field | Value |
|---|---|
| Owning roadmap | [`../ROADMAP.md`](../ROADMAP.md) |
| Authority identity | Rendering behavior rows; W3C WebGPU Candidate Recommendation Draft initialization, resource, canvas, queue, color, loss, limit, error, and debugging contracts; Valve Source SDK 2013 render, material, model, client-leaf, view, lighting, decal, beam, sprite, interpolation, and client-entity contracts; accepted adjacent producer registries; exact declared-content indexes and visual-evidence manifest. |
| Authority revision | playsrc `1c2184c11353a618d8ef941c7f161e488346203e`; WebGPU `https://www.w3.org/TR/2026/CRD-webgpu-20260714/`; Valve Source SDK 2013 `88fa198fba3fb85d46d4c95018254693fdc3af0a`; producer registries, content indexes, and visual-evidence manifest Missing. |
| Generator command | Missing |
| Output path | `packages/presentation/rendering/inventories/presentation-families.md` |
| Candidate item count | 54 |
| Accepted item count | 0 |

This file is a hand-derived Candidate index of the 54 behavior-row identities. It is not generated or Accepted. Acceptance replaces this file with generator output for the same identities; the set union remains 54 denominator items rather than duplicating each roadmap row.

## Candidate Items

`Required seam` names an input producer or output consumer. It never transfers GPU, scene, view, interpolation, draw-preparation, pixel, or submission ownership out of Rendering.

| Identity | Presentation family | Finite target | Required seam | Current coverage |
|---|---|---|---|---|
| `REN-001` | Renderer initialization | Validate one complete creation request and publish exactly one Ready renderer or one atomic failure. | WebGPU adapter/device; applications supply canvas and profile. | Unsupported |
| `REN-002` | Canvas and device lifecycle | Own one configured WebGPU canvas context and one live device generation through deterministic disposal. | HTML/WebGPU canvas contracts. | Unsupported |
| `REN-003` | Context loss | Stop submission, invalidate the lost generation, rebuild leased scenes under the identical profile, or fail terminally. | WebGPU `GPUDevice.lost`; immutable scene inputs. | Unsupported |
| `REN-004` | Immutable asset loading | Validate and load one exact map-root object closure atomically without placeholders or discovery. | Map, Asset Store, Content. | Missing |
| `REN-005` | Resource ownership | Ledger every CPU/GPU resource, hash-key share, scene lease, retirement, and destruction. | WebGPU resource lifetime; scene consumers. | Unsupported |
| `REN-006` | Upload bounds | Enforce caller/device allocation, alignment, byte, operation, pending-work, and per-frame budgets before mutation. | WebGPU limits and queues. | Unsupported |
| `REN-007` | World geometry | Preserve face topology, winding, attributes, material, model, and source identities in bounded draw buffers. | BSP and Map. | Missing |
| `REN-008` | Displacements | Draw accepted displacement topology and classifications through all declared receiver and pass roles. | BSP, Map, Material, Visibility. | Missing |
| `REN-009` | Brush models | Render world model zero and transformed `*N` models with current state, lighting, decals, shadows, and visibility. | Map, Entity, Visibility. | Missing |
| `REN-010` | Static props | Present each static occurrence with transform, skin, fade, lighting, LOD, decals, shadows, and instancing. | Map, StudioModel, Material, Visibility. | Missing |
| `REN-011` | Dynamic props | Present revisioned transforms and selections without advancing Physics or gameplay. | Entity, Physics, selected game, StudioModel, Visibility. | Missing |
| `REN-012` | Studio models | Assemble accepted model geometry, skeleton, materials, skins, bodyparts, LODs, flexes, decals, and attachments. | StudioModel and Material. | Missing |
| `REN-013` | Bones | Evaluate bounded local/world bone matrices and skin vertices from supplied pose inputs and discontinuities. | StudioModel; selected game, Simulation, or Replay. | Missing |
| `REN-014` | Animations | Sample selected sequences, cycles, blends, layers, pose parameters, flexes, and presentation events in declared order. | StudioModel; selected game, Simulation, or Replay. | Missing |
| `REN-015` | Skins | Resolve exactly the supplied skin family and slot to current Material output. | StudioModel, Material, selected game. | Missing |
| `REN-016` | Bodygroups | Select exactly one declared submodel value per bodypart and retain empty/error dispositions. | StudioModel, selected game. | Missing |
| `REN-017` | LODs | Select model and shadow LOD from camera metric, compiled restrictions, switch values, and stable hysteresis. | StudioModel and Visibility inputs. | Missing |
| `REN-018` | Material binding | Convert one evaluated runtime-neutral Material state into exact GPU pipeline, bindings, uniforms, and pass state. | Material. | Missing |
| `REN-019` | Texture binding | Upload and bind every declared texture subresource, interpretation, sampler, and selected frame exactly. | VTF, Material, immutable object bytes. | Missing |
| `REN-020` | Lightmaps | Bind profile-specific flat/directional atlas data and evaluate normal/SSBump contribution per face. | BSP, Map, Material. | Missing |
| `REN-021` | LDR | Evaluate only selected LDR lighting and one accepted LDR reconstruction/output path. | Map lighting profile and visual-evidence manifest. | Missing |
| `REN-022` | HDR | Preserve radiance through lighting/blending and apply only the accepted HDR exposure/tone/output path. | Map lighting profile and visual-evidence manifest. | Missing |
| `REN-023` | Light styles | Sample every active style at presentation time and scale all associated contributions. | Map and Entity style state. | Missing |
| `REN-024` | Dynamic lights | Select and evaluate bounded point/spot lights across world, displacement, brush, and model draws. | Entity, selected game, Visibility. | Missing |
| `REN-025` | Shadows | Render accepted projected, render-to-texture, and depth-texture shadow families with exact caster/receiver state. | Entity/game shadow state, Map, StudioModel, Visibility. | Missing |
| `REN-026` | Cubemaps | Select and sample the exact map cubemap occurrence and profile without sky/environment substitution. | Map and Material. | Missing |
| `REN-027` | Reflections | Render isolated planar reflection views with reflected camera, visibility, fog, clipping, and target state. | Material, Map, Visibility. | Missing |
| `REN-028` | 2D sky | Render the selected six-face sky around the camera with exact face orientation, seams, depth, and visibility. | Map and Material. | Missing |
| `REN-029` | 3D sky | Render the sky-camera world as a separate transformed, area-filtered, fogged view before the main world. | Map, Entity, Visibility, selected game. | Missing |
| `REN-030` | Fog | Sample supplied main, sky, and water transition state and apply directional color blend, distance/density, and per-view restoration. | Entity and Map environment associations. | Missing |
| `REN-031` | Water | Classify camera/surface state and render the declared cheap/reflection/refraction/intersection/composition graph. | Map, Material, Entity, Visibility. | Missing |
| `REN-032` | Decals | Draw supplied world/displacement/brush/prop/model decal projection state, lifetime, fade, receiver, and pass order. | Map, Entity, StudioModel, Material. | Missing |
| `REN-033` | Overlays | Draw canonical compiled/water overlay fragments once with basis, lightmap, order, fade, activation, and receiver identity. | Map and Material. | Missing |
| `REN-034` | Detail props | Present detail models/sprites with occurrence, orientation, animation, sway, lighting/style, fade, leaf, and interleaving. | Map, StudioModel, Material, Visibility. | Missing |
| `REN-035` | Ropes | Convert supplied rope segments, endpoints, subdivision, width, materials, scroll, color, and light into bounded draws. | Entity supplies rope state; Material supplies semantics. | Missing |
| `REN-036` | Beams | Draw every accepted point/entity/ring/spline/follow/halo beam input without creating or advancing beam truth. | Entity and selected game mappings. | Missing |
| `REN-037` | Sprites | Emit camera-oriented sprite quads with exact frame, material, orientation, scale, render mode, color, depth, and blend. | Entity, selected game, StudioModel/material resources. | Missing |
| `REN-038` | Glows | Present layered glows from explicit source and view-qualified visibility/occlusion samples. | Entity/game mappings, Material, Visibility. | Missing |
| `REN-039` | Particles as render input | Consume immutable advanced particle data and emit bounded primitive draws without effect selection or state mutation. | Particle and selected game mappings. | Missing |
| `REN-040` | Viewmodels | Render accepted first-person model state with separate projection/depth, pose, handedness, and post-world order. | StudioModel and selected game presentation mappings. | Missing |
| `REN-041` | Transparent ordering | Interleave all translucent families back-to-front by visibility leaves and stable family tie keys; draw ignore-depth last. | Visibility supplies ordered leaves/candidates. | Unsupported |
| `REN-042` | Depth and stencil | Implement clear/load/store, comparison, write, bias, clip, viewmodel range, and nested-state restoration. | WebGPU render passes; Material depth state. | Unsupported |
| `REN-043` | Draw culling | Apply frustum, area, fade, LOD, map/GPU occlusion, material, and alpha decisions in one fixed order. | Visibility supplies candidates and map decisions. | Unsupported |
| `REN-044` | Visibility input | Validate exact map/view/revision result identity and consume candidates without PVS decode or BSP traversal. | Visibility. | Missing |
| `REN-045` | Cameras | Derive Source-consistent perspective, orthographic, off-center, explicit, pixel, and viewmodel matrices. | Applications select cameras; Rendering owns mathematics. | Unsupported |
| `REN-046` | View state | Isolate and restore per-view camera, viewport, target, clear, visibility, fog, clip, draw, copy, and debug state. | All presentation producers; canvas consumer. | Unsupported |
| `REN-047` | Presentation interpolation | Sample only supplied snapshot pairs/policies/fractions and snap at every supplied discontinuity. | Simulation/Replay snapshots and selected-game policy. | Unsupported |
| `REN-048` | Frame phases | Execute one frozen phase order ending in exactly one queue submission or no submission on failure. | All Rendering inputs; WebGPU queue. | Unsupported |
| `REN-049` | Frame pacing | Bound browser frame opportunities and in-flight work while recording pacing and never advancing authority state. | Browser animation-frame callback; applications select policy. | Unsupported |
| `REN-050` | Resizing | Atomically replace bounded canvas and dependent attachment generations and update camera/view state. | HTML/WebGPU canvas sizing; applications supply CSS size and DPR. | Unsupported |
| `REN-051` | Color output | Keep data/linear/sRGB roles distinct and apply exactly one selected LDR/HDR transfer to declared canvas output. | Material, WebGPU canvas color, evidence manifest. | Missing |
| `REN-052` | Debugging views | Emit the finite declared wireframe/normal/lightmap/ID/overdraw/depth/target/culling/resource views without state contamination. | Inspector consumes outputs. | Unsupported |
| `REN-053` | Unsupported presentation states | Classify every encountered family and reject required non-handled state before scene activation or submission. | Every producer registry and declared-content index. | Unsupported |
| `REN-054` | Aligned visual evidence | Bind every visual claim to exact content, state, time, camera, viewport, profile, hashes, environment, comparison, and tolerance. | Tools orchestrate target and playsrc captures. | Missing |

## Generation Contract

The future generator must:

1. Read one checked rendering-contract manifest containing exactly `REN-001` through `REN-054` in numeric order.
2. Validate the accepted Owner Registry and every `Required seam` against current producer and consumer registries.
3. Pin the W3C WebGPU and Valve Source SDK authority identities in Metadata.
4. Consume exact archive indexes for one declared content build of TF2, CS:S, and legacy Source 1 CS:GO, plus the accepted browser/GPU and visual-evidence manifests.
5. Retain one Handled, Intentionally inert, Unsupported, Malformed, Unknown, or Missing classification for every discovered input value under its sole family owner.
6. Fail on an omitted or duplicate family, non-contiguous identity, changed owner, unknown seam, unclassified discovery, stale authority, missing evidence method, or count other than 54.
7. Emit this path byte-identically from identical inputs without machine paths, timestamps, locale-dependent order, or asynchronous-completion order.
