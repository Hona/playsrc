# StudioModel Serialized Record-Family Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/studio.h`, `src/public/optimize.h`, `src/public/materialsystem/hardwareverts.h`, and `src/common/studiobyteswap.cpp` at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, plus exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes. Configured TF2 public build `24207079` and its VPK indexes are available; the CS:S and legacy CS:GO indexes remain Missing.

Generator command: Missing.

Candidate item count: 82. Accepted item count: 0.

Each stable identity is one serialized record family or primitive table with one validation and output contract. Runtime cache structures, GPU resources, entity animation state, and semantic effects are not inventory items.

| Stable identity | File family | Encoded records | Runtime-neutral output contract | Adjacent semantic owner |
|---|---|---|---|---|
| `mdl.header` | MDL | 408-byte `IDST` primary header | Signature, profile/version, checksum, internal-name bytes, declared length, eye/illumination positions, hull/view bounds, flags, table descriptors, root-LOD fields, flex scale, mass, contents, and source range | Games, collision, and rendering interpret stored policy fields |
| `mdl.secondary-header` | MDL | Optional secondary header reached from the primary header | Presence, source range, source-transform descriptor, illumination attachment, maximum eye deflection, linear-bone descriptor, alternate name, bone-flex-driver descriptor, and reserved bytes | Rendering interprets illumination and eye behavior |
| `mdl.strings` | MDL | Fixed and relative NUL-terminated byte strings | Exact bytes, terminator, base record, relative/absolute source range, and optional derived Source logical identity | Content resolves logical identities; games interpret labels |
| `mdl.bones` | MDL | Ordered bone records and name-sorted bone-index table | Parent graph, controller indexes, base transforms, compression scales, pose-to-bone transform, alignment, flags, physics-bone reference, surface property, contents, and sorted lookup bytes | Games/rendering produce pose state; collision interprets physics fields |
| `mdl.procedural-axis` | MDL | Axis-interpolation procedural-bone payload | Control bone, axis, six positions, six quaternions, and exact source range | Rendering evaluates procedural pose |
| `mdl.procedural-quaternion` | MDL | Quaternion-interpolation header and ordered trigger records | Control bone, trigger count/order, inverse tolerance, trigger quaternion, result position/quaternion, and exact ranges | Rendering evaluates procedural pose |
| `mdl.procedural-jiggle` | MDL | Jiggle/boing procedural-bone payload | Raw flags and every length, mass, stiffness, damping, constraint, friction, bounce, and boing scalar | Rendering evaluates presentation-only jiggle state |
| `mdl.procedural-aim` | MDL | Aim-at-bone or aim-at-attachment payload | Declared type, parent, aim target, aim/up vectors, base position, and source range | Rendering evaluates procedural pose |
| `mdl.linear-bones` | MDL | Linear-bone header and parallel bone-indexed arrays | Bone count; ordered flags, parents, positions, quaternions, rotations, pose transforms, scales, and alignments | Rendering consumes skeleton data |
| `mdl.source-bone-transforms` | MDL | Source-bone transform records | Name bytes plus pre/post 3×4 transforms in source order | Model tools and rendering consume transforms |
| `mdl.bone-controllers` | MDL | Bone-controller records | Bone, motion type, start/end, rest byte, input field, reserved bytes, and source range | Games supply controller state; rendering applies it |
| `mdl.bone-flex-drivers` | MDL | Bone-flex-driver headers and ordered control records | Bone index, component, flex-controller index, min/max mapping, control order, and reserved bytes | Rendering applies controller values |
| `mdl.hitbox-sets` | MDL | Hitbox-set records | Set name, ordered hitbox range, empty-set state, and source range | Collision and games select sets |
| `mdl.hitboxes` | MDL | Hitbox records | Bone, group, minimum/maximum vectors, name bytes, reserved bytes, and source range | Collision and games interpret damage geometry |
| `mdl.animation-descriptors` | MDL | Local animation descriptor records | Name, fps bits, flags, frame count, movement descriptor, block/index, IK descriptors, local-hierarchy descriptor, section descriptor, zero-frame descriptor, and source range | Games/replay choose animations; rendering evaluates poses |
| `mdl.animation-block-directory` | MDL | Animation-block file identity and ordered start/end pairs | Exact ANI logical identity, block indexes beginning at 1, byte ranges, empty ranges, and source descriptor ranges | Content retrieves ANI bytes |
| `mdl.animation-sections` | MDL | Ordered block/index pairs for frame sections | Section identity, frame span, external/local block choice, final-frame section, and referenced byte range | Rendering requests decoded integer frames |
| `mdl.animation-tracks` | MDL | Per-bone linked track headers and raw Vector48/Quaternion48/Quaternion64 payloads | Bone, flags, next offset, source encoding, exact compressed bits, decoded local transform, and terminator state | Rendering blends decoded local transforms |
| `mdl.animation-values` | MDL | Position/rotation value-pointer tables and RLE run/value streams | Axis offsets, ordered `(valid,total)` runs, exact signed values, covered frames, scale, decoded values, and source ranges | Rendering interpolates/blends values |
| `mdl.zero-frames` | MDL | Per-animation saved position/rotation spans | Span, count, bone-flag-selected Vector48/Quaternion64 records, decoded values, and exact source range | Rendering uses decoded saved frames |
| `mdl.movements` | MDL | Piecewise animation movement records | End frame, motion flags, endpoint velocities, yaw, direction, accumulated position, order, and source range | Games own entity movement decisions |
| `mdl.ik-rules` | MDL/ANI | Inline or external IK-rule records | Index, type, chain, bone, slot, geometry, transform, influence window, contact/drop/top, attachment bytes, and source location | Rendering solves IK; games own target state |
| `mdl.ik-errors` | MDL/ANI | Uncompressed IK errors or compressed six-channel error headers and value streams | Encoding identity, scale, offsets, frame coverage, position/quaternion errors, and exact ranges | Rendering solves IK |
| `mdl.local-hierarchies` | MDL/ANI | Local hierarchy records and compressed local-animation errors | Bone, new parent, influence window, first frame, compressed channel data, reserved bytes, and exact ranges | Rendering applies hierarchy changes |
| `mdl.sequence-descriptors` | MDL | Local sequence descriptor records | Label/activity-name bytes, raw activity fields, flags, bounds, blend count/grid, parameter fields, fades, nodes, phases, next sequence, pose, cycle pose, child descriptors, and source range | Games choose sequences and map activities |
| `mdl.sequence-blend-indices` | MDL | Signed-16 animation-index grid | Exact two-dimensional shape, source order, local animation references, and source range | Rendering blends selected animations |
| `mdl.sequence-pose-keys` | MDL | Binary32 pose-key arrays | Parameter axis, animation coordinate, exact binary32 bits, source order, and source range | Games/rendering supply and apply pose parameters |
| `mdl.sequence-bone-weights` | MDL | Binary32 per-bone sequence-weight arrays | Bone identity, exact weight bits, shared-range identity, and source order | Rendering weights pose accumulation |
| `mdl.sequence-events` | MDL | Ordered animation-event records | Cycle bits, raw event ID, type, 64 option bytes, optional event-name bytes, source order, and range | Games, audio, and particles own event effects |
| `mdl.sequence-autolayers` | MDL | Ordered autolayer records | Local sequence/pose references, flags, start/peak/tail/end bits, order, and source range | Games/rendering choose and blend layers |
| `mdl.sequence-ik-locks` | MDL | Sequence and autoplay IK-lock records | Chain, position/local-rotation weights, flags, reserved bytes, containing sequence/autoplay identity, and range | Rendering solves IK |
| `mdl.sequence-activity-modifiers` | MDL | Ordered activity-modifier name records | Exact modifier-name bytes, order, duplicates, and source range | Games map activities and modifiers |
| `mdl.transition-graph` | MDL | Node-name offset table and `N × N` transition-byte matrix | Ordered node names, matrix dimensions, every directed transition byte, and exact ranges | Games choose transitions |
| `mdl.pose-parameters` | MDL | Local pose-parameter descriptors | Name bytes, raw flags, start/end/loop bits, order, and source range | Games/replay supply values; rendering applies them |
| `mdl.attachments` | MDL | Local attachment records | Name, raw flags, bone, 3×4 local transform, reserved bytes, order, and source range | Games/rendering use attachment transforms |
| `mdl.textures` | MDL | Texture/material-name records | Raw name bytes, flags, usage fields, reserved/runtime-pointer bytes as encoded, texture index, and source range | VMT/world-material owners interpret materials |
| `mdl.material-directories` | MDL | Ordered offsets to material-directory strings | Raw directory bytes, order, duplicates, exact ranges, and derived material candidate prefixes | Content/world material resolve candidates |
| `mdl.skin-table` | MDL | Rectangular signed-16 texture-reference table | Family/slot dimensions, every texture index, source order, and source range | Games choose skin family; rendering applies materials |
| `mdl.bodyparts` | MDL | Bodypart records | Name, model count, base value, ordered model descriptor, and source range | Games choose bodygroup values |
| `mdl.models` | MDL | Model records nested under bodyparts | Fixed name bytes, type, bounding radius, mesh descriptor, vertex count/offset, tangent offset, attachment/eyeball descriptors, and reserved bytes | Rendering consumes model alternatives |
| `mdl.meshes` | MDL | Mesh records nested under models | Material slot, parent-model relative reference, vertex count/offset, flex descriptor, material operation fields, mesh ID, center, per-LOD vertex counts, and reserved bytes | Rendering consumes geometry/material bindings |
| `mdl.eyeballs` | MDL | Eyeball records | Name, bone, origin, radius/offset, orientation vectors, texture, iris scale, upper/lower flex references/targets, non-FACS flag, and reserved bytes | Rendering evaluates eye presentation |
| `mdl.mouths` | MDL | Mouth records | Bone, forward vector, flex descriptor, order, and source range | Audio/rendering interpret mouth state |
| `mdl.flex-descriptors` | MDL | Flex/FACS-name records | Name bytes, local index, order, and source range | Rendering owns flex values |
| `mdl.flex-controllers` | MDL | Flex-controller records | Type/name bytes, stored local-to-global value, min/max bits, order, and source range | Games/rendering supply controller values |
| `mdl.flex-controller-ui` | MDL | Flex-controller UI/remap records | Name, referenced controller offsets, remap type, stereo state, derived controller references, reserved bytes, and range | Applications/rendering expose and apply controls |
| `mdl.flex-rules` | MDL | Flex-rule headers and ordered opcode/operand records | Destination flex, operation count/order, raw opcode, exact operand bits, known stack contract, unknown-op classification, and source ranges | Rendering evaluates rules from supplied controller values |
| `mdl.mesh-flexes` | MDL | Mesh flex headers | Descriptor, four target bits, vertex descriptor, pair, vertex-animation type, reserved bytes, containing mesh, and source range | Rendering applies morph targets |
| `mdl.vertex-animations` | MDL | Normal 16-byte or wrinkle 18-byte vertex-animation records | Mesh-local vertex, speed, side, source encoding, position/normal deltas, optional wrinkle delta, scale, order, and exact range | Rendering applies morph deltas |
| `mdl.include-models` | MDL | Ordered include-model label/name records | Label bytes, exact model logical identity, order, dependency result, and local-to-root remap view | Content resolves bytes; games/rendering consume composed records |
| `mdl.keyvalues` | MDL | Model-level and sequence-level byte ranges | Exact bytes, containing record, empty state, source range, and KeyValues parse-request boundary | `packages/formats/keyvalues` owns KV1 syntax; games/tools own fields |
| `mdl.collision-references` | MDL | Surface-property strings, physics-bone indexes, mass, contents, hull/view bounds, checksum, and derived PHY identity | Raw values, expected PHY checksum, exact logical identity, and dependency state without PHY decoding | PHY, collision, physics, and games own meaning |
| `mdl.unknown-records` | MDL | Unknown flags/types, reserved words, padding, and unconsumed declared-length bytes | Exact byte ranges and one coverage classification per value/range | A semantic owner must be assigned before use |
| `vvd.header` | VVD | 64-byte `IDSV` version-4 header | Signature, version, checksum, LOD count, eight LOD vertex counts, fixup count/offset, vertex offset, tangent offset, and source range | StudioModel owns structural matching |
| `vvd.fixups` | VVD | Ordered 12-byte fixup records | LOD threshold, source vertex start/count, derived destination range per root LOD, and source range | StudioModel owns vertex selection |
| `vvd.bone-weights` | VVD | Embedded three-weight/three-bone/influence-count record | Exact binary32 weight bits, bone bytes, influence count, sum classification, and containing vertex | Rendering owns skinning |
| `vvd.vertices` | VVD | Ordered 48-byte vertex records | Source index, bone weights, exact position/normal/UV bits, selected-LOD destination indexes, and source range | Rendering consumes geometry |
| `vvd.tangents` | VVD | Separate ordered 16-byte Vector4 records | Source vertex index, four exact binary32 bits, presence state, and source range | Rendering consumes tangent basis |
| `vvd.unknown-records` | VVD | Unknown ID/version, bytes between known ranges, overlaps, padding, and trailing bytes | Exact ranges/relations/bytes and coverage classification | A semantic owner must be assigned before use |
| `vtx.header` | VTX | 36-byte packed version-7 header | Extension identity, version, vertex-cache size, bone ceilings, checksum, LOD count, material-list descriptor, bodypart descriptor, and source range | Rendering chooses device profile; StudioModel parses bytes |
| `vtx.material-replacement-lists` | VTX | One packed replacement-list header per LOD | LOD identity, replacement count/offset, order, empty state, and source range | World material/rendering consume replacement choices |
| `vtx.material-replacements` | VTX | Packed replacement records plus relative name strings | Signed-16 material ID, exact replacement-name bytes, order, duplicates, and source ranges | World material resolves VMT identity |
| `vtx.bodyparts` | VTX | Packed bodypart headers | Ordered model descriptor, matching MDL bodypart identity, and source range | StudioModel owns structural matching |
| `vtx.models` | VTX | Packed model headers | Ordered LOD descriptor, matching MDL model identity, and source range | StudioModel owns structural matching |
| `vtx.lods` | VTX | Packed model-LOD headers | LOD identity, mesh descriptor, exact switch-point bits, shared-table relation, and source range | Rendering selects LOD |
| `vtx.meshes` | VTX | Packed mesh headers | Ordered strip-group descriptor, raw mesh flags, matching MDL mesh identity, and source range | Rendering consumes topology flags |
| `vtx.strip-groups` | VTX | Packed strip-group headers | Vertex/index/strip descriptors, raw flags, source ranges, and containing hierarchy | Rendering consumes topology groups |
| `vtx.vertices` | VTX | Packed 9-byte optimized vertex records | Bone-weight indexes, influence count, original MDL mesh-vertex ID, bone IDs, order, and source range | Rendering consumes skinning mappings |
| `vtx.indices` | VTX | Ordered unsigned-16 strip-group-local indexes | Exact value, group-local vertex reference, order, duplicate/degenerate state, and source range | Rendering consumes topology |
| `vtx.strips` | VTX | Packed 27-byte strip records | Index/vertex subranges, bone count, list/strip flags, bone-state descriptor, order, and source range | Rendering consumes primitive topology |
| `vtx.bone-state-changes` | VTX | Packed hardware-bone remap records | Hardware ID, new bone ID, containing strip, order, and source range | Rendering applies hardware-profile remaps |
| `vtx.unknown-records` | VTX | Unknown flags, bytes between packed ranges, overlaps, padding, and tails | Exact ranges/relations/bytes and coverage classification | A semantic owner must be assigned before use |
| `ani.header` | ANI | 408-byte `IDAG` header | Signature, profile/version, declared length, exact reserved/header bytes, and source range | StudioModel owns identity validation |
| `ani.animation-blocks` | ANI | MDL-indexed external animation-block byte ranges | Block index, declared start/end, exact bytes, referenced/unreferenced state, and source range | Content supplies bytes; StudioModel decodes referenced data |
| `ani.animation-tracks` | ANI | External per-bone tracks and raw/RLE/zero-frame payloads | Same typed output as MDL-local animation tracks, with ANI source identity and block-relative range | Rendering consumes decoded local transforms |
| `ani.ik-payloads` | ANI | External IK rules and compressed/uncompressed error streams | Same typed output as MDL-local IK records, with block and ANI source identity | Rendering solves IK |
| `ani.local-hierarchy-payloads` | ANI | External local-hierarchy records and compressed local-animation errors | Same typed output as MDL-local hierarchy records, with block and ANI source identity | Rendering applies hierarchy changes |
| `ani.unknown-records` | ANI | Unreferenced blocks, unknown payload bytes, gaps, alignment padding, and tails | Exact ranges/bytes, reference relation, and coverage classification | A semantic owner must be assigned before use |
| `vhv.header` | VHV | 40-byte packed version-2 header | Version, checksum, vertex flags, stride, total vertex count, mesh count, reserved words, and source range | Rendering interprets vertex flags/format |
| `vhv.meshes` | VHV | Ordered 28-byte packed mesh headers | LOD, vertex count, byte offset, reserved words, derived byte range, overlap relation, and source range | World map associates map instance; rendering consumes records |
| `vhv.vertices` | VHV | Four BGRA bytes per declared `VERTEX_COLOR` vertex | Mesh/local index, exact blue/green/red/alpha bytes, stride, shared/overlap relation, and source range | Rendering consumes static-prop lighting samples |
| `vhv.unknown-records` | VHV | Unknown flags/strides, reserved words, gaps, overlap bytes, padding, and tails | Exact ranges/relations/bytes and coverage classification | A semantic owner must be assigned before use |

## Boundary Audit

- Runtime virtual-model caches, mesh buffers, material pointers, thin vertices, autoplay caches, and activity lookup tables are not serialized record families. Include composition is a roadmap behavior over serialized records.
- `IDCV` thin VVD data is an internal runtime cache representation and is excluded from accepted Source input.
- PHY bytes and records belong to `packages/formats/phy`; StudioModel inventory contains only MDL collision references.
- VMT/VTF bytes, GPU buffers, scene nodes, entity pose state, sequence selection, IK results, flex values, audio/particle effects, and rendered pixels are not StudioModel serialized records.
- A record absent from this candidate inventory is retained as `<file-family>.unknown-records` and blocks inventory acceptance until generation assigns it a stable identity, owner, and contract.
