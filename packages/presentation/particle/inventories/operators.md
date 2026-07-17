# Particle Operator Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

## Inventory Metadata

| Field | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a` public particle interfaces and Linux x86-64 particle library; TF2 content build `24207079` global and map particle manifests and their resolved PCF definitions |
| Authority revision | SDK particle-library SHA-256 `2700ff6cecb972db32b2b01a790cb28ccab2775db383d2e7ea6a32d9cdbb1f9e`; TF2 `steam.inf` SHA-256 `b8d7c1eb4517a806d514087facf42e3d8f407bf14393ac5fdc5d4c69e40adc7f`; `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9` |
| Generator command | Missing |
| Output path | `packages/presentation/particle/inventories/operators.md` |
| Item count | 95 Candidate; 0 Accepted |

This file is a manually derived Candidate and is not a generated inventory. Its items contribute 0 completion-denominator items until a checked-in generator, current output, target registry capture, and denominator review satisfy [`../../../../docs/roadmap-contract.md`](../../../../docs/roadmap-contract.md).

The registry candidate contains 4 renderer-data operators, 41 simulation operators, 39 initializers, 3 emitters, 3 force generators, and 5 constraints. Occurrence counts cover 228 distinct available PCF byte sequences selected by the global manifest or one of 233 configured TF2 map profiles. Nine selected map PCF references are Missing and do not contribute occurrence counts.

An item is Ready only when its ASCII-insensitive identity, accepted source spellings, category, multiplicity rule, obsolete state, complete typed parameter schema and defaults, parameter normalization, attribute reads and writes, initial-attribute reads, control-point reads and writes, per-system context, random samples, query inputs, ordering phase, strength envelope, state transition, render-data output, malformed behavior, and limit behavior have direct evidence. `TF2 occurrences` counts operator elements before system-name shadowing; repeated operators remain repeated operations.

## Renderer-Data Operators

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-REN-001 | `render_animated_sprites` | 10,134 | `render_animated_sprites` | Unsupported |
| PRT-REN-002 | `render_rope` | 315 | `render_rope` | Unsupported |
| PRT-REN-003 | `render_screen_velocity_rotate` | 35 | `render_screen_velocity_rotate` | Unsupported |
| PRT-REN-004 | `render_sprite_trail` | 1,062 | `render_sprite_trail` | Unsupported |

## Simulation Operators

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-OPR-001 | `Alpha Fade and Decay` | 5,498 | `Alpha Fade and Decay` (5,471); `alpha_fade` (27) | Unsupported |
| PRT-OPR-002 | `Alpha Fade In Random` | 4,557 | `Alpha Fade In Random` (4,555); `alpha_fade_in_random` (2) | Unsupported |
| PRT-OPR-003 | `Alpha Fade Out Random` | 4,733 | `Alpha Fade Out Random` (4,731); `alpha_fade_out_random` (2) | Unsupported |
| PRT-OPR-004 | `Color Fade` | 4,383 | `Color Fade` (4,380); `color_fade` (3) | Unsupported |
| PRT-OPR-005 | `Color Light from Control Point` | 10 | `Color Light From Control Point` (7); `Color Light from Control Point` (3) | Unsupported |
| PRT-OPR-006 | `Cull Random` | 1 | `Cull Random` | Unsupported |
| PRT-OPR-007 | `Cull relative to model` | 1 | `Cull relative to model` | Unsupported |
| PRT-OPR-008 | `Cull when crossing plane` | 75 | `Cull when crossing plane` | Unsupported |
| PRT-OPR-009 | `Lifespan Decay` | 6,057 | `Lifespan Decay` (6,049); `lifespan_decay` (8) | Unsupported |
| PRT-OPR-010 | `Lifespan Minimum Velocity Decay` | 0 | None | Unsupported |
| PRT-OPR-011 | `Movement Basic` | 9,912 | `Movement Basic` (9,878); `basic_movement` (34) | Unsupported |
| PRT-OPR-012 | `Movement Dampen Relative to Control Point` | 79 | `Movement Dampen Relative to Control Point` | Unsupported |
| PRT-OPR-013 | `Movement Follow CP` | 17 | `Movement Follow CP` | Unknown |
| PRT-OPR-014 | `Movement Lock to Bone` | 379 | `Movement Lock to Bone` | Unsupported |
| PRT-OPR-015 | `Movement Lock to Control Point` | 7,292 | `Movement Lock to Control Point` (7,258); `postion_lock_to_controlpoint` (34) | Unsupported |
| PRT-OPR-016 | `Movement Lock to Saved Position Along Path` | 9 | `Movement Lock to Saved Position Along Path` | Unknown |
| PRT-OPR-017 | `Movement Maintain Position Along Path` | 32 | `Movement Maintain Position Along Path` | Unsupported |
| PRT-OPR-018 | `Movement Match Particle Velocities` | 28 | `Movement Match Particle Velocities` | Unsupported |
| PRT-OPR-019 | `Movement Max Velocity` | 272 | `Movement Max Velocity` | Unsupported |
| PRT-OPR-020 | `Movement Rotate Particle Around Axis` | 1,073 | `Movement Rotate Particle Around Axis` | Unsupported |
| PRT-OPR-021 | `Noise Scalar` | 25 | `Noise Scalar` | Unsupported |
| PRT-OPR-022 | `Noise Vector` | 2 | `Noise Vector` | Unsupported |
| PRT-OPR-023 | `Oscillate Scalar` | 615 | `Oscillate Scalar` (593); `oscillate_scalar` (22) | Unsupported |
| PRT-OPR-024 | `Oscillate Vector` | 1,055 | `Oscillate Vector` (1,012); `oscillate_vector` (43) | Unsupported |
| PRT-OPR-025 | `Radius Scale` | 8,813 | `Radius Scale` (8,777); `radius_scale` (36) | Unsupported |
| PRT-OPR-026 | `Remap Control Point to Scalar` | 0 | None | Unsupported |
| PRT-OPR-027 | `Remap CP Speed to CP` | 19 | `Remap CP Speed to CP` | Unsupported |
| PRT-OPR-028 | `Remap Distance Between Two Control Points to Scalar` | 5 | `Remap Distance Between Two Control Points to Scalar` | Unsupported |
| PRT-OPR-029 | `Remap Distance to Control Point to Scalar` | 170 | `Remap Distance to Control Point to Scalar` | Unsupported |
| PRT-OPR-030 | `Remap Distance to Control Point to Vector` | 54 | `Remap Distance to Control Point to Vector` | Unknown |
| PRT-OPR-031 | `Remap Dot Product to Scalar` | 26 | `Remap Dot Product to Scalar` | Unsupported |
| PRT-OPR-032 | `Remap Scalar` | 70 | `Remap Scalar` | Unsupported |
| PRT-OPR-033 | `Rotation Basic` | 914 | `Rotation Basic` | Unsupported |
| PRT-OPR-034 | `Rotation Orient Relative to CP` | 28 | `Rotation Orient Relative to CP` | Unsupported |
| PRT-OPR-035 | `Rotation Orient to 2D Direction` | 5 | `Rotation Orient to 2D Direction` | Unsupported |
| PRT-OPR-036 | `Rotation Spin Roll` | 2,806 | `Rotation Spin Roll` (2,804); `rotation_spin` (2) | Unsupported |
| PRT-OPR-037 | `Rotation Spin Yaw` | 79 | `Rotation Spin Yaw` (77); `rotation_spin yaw` (2) | Unsupported |
| PRT-OPR-038 | `Set child control points from particle positions` | 759 | `Set child control points from particle positions` | Unsupported |
| PRT-OPR-039 | `Set Control Point Positions` | 477 | `Set Control Point Positions` | Unsupported |
| PRT-OPR-040 | `Set Control Point To Particles' Center` | 21 | `Set Control Point To Particles' Center` | Unsupported |
| PRT-OPR-041 | `Set Control Point To Player` | 60 | `Set Control Point To Player` | Unsupported |

## Initializers

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-INI-001 | `Alpha Random` | 8,891 | `Alpha Random` (8,860); `alpha_random` (31) | Unsupported |
| PRT-INI-002 | `Assign target CP` | 17 | `Assign target CP` | Unknown |
| PRT-INI-003 | `Color Random` | 9,613 | `Color Random` (9,598); `color_random` (15) | Unsupported |
| PRT-INI-004 | `Lifetime From Control Point Life Time` | 15 | `Lifetime From Control Point Life Time` | Unknown |
| PRT-INI-005 | `Lifetime From Sequence` | 304 | `Lifetime From Sequence` (100); `lifetime from sequence` (204) | Unsupported |
| PRT-INI-006 | `Lifetime from Time to Impact` | 0 | None | Unsupported |
| PRT-INI-007 | `Lifetime Pre-Age Noise` | 33 | `Lifetime Pre-Age Noise` | Unsupported |
| PRT-INI-008 | `Lifetime Random` | 10,620 | `Lifetime Random` (10,584); `lifetime_random` (36) | Unsupported |
| PRT-INI-009 | `Move Particles Between 2 Control Points` | 32 | `move particles between 2 control points` | Unsupported |
| PRT-INI-010 | `Position Along Path Random` | 228 | `Position Along Path Random` | Unsupported |
| PRT-INI-011 | `Position Along Path Sequential` | 154 | `Position Along Path Sequential` | Unsupported |
| PRT-INI-012 | `Position from Parent Cache` | 0 | None | Unsupported |
| PRT-INI-013 | `Position From Parent Particles` | 1,575 | `Position From Parent Particles` | Unsupported |
| PRT-INI-014 | `Position In CP Hierarchy` | 0 | None | Unsupported |
| PRT-INI-015 | `Position Modify Offset Random` | 6,231 | `Position Modify Offset Random` | Unsupported |
| PRT-INI-016 | `Position Modify Warp Random` | 150 | `Position Modify Warp Random` | Unsupported |
| PRT-INI-017 | `Position on Model Random` | 569 | `Position on Model Random` | Unsupported |
| PRT-INI-018 | `Position Within Box Random` | 505 | `Position Within Box Random` | Unsupported |
| PRT-INI-019 | `Position Within Sphere Random` | 8,510 | `Position Within Sphere Random` (8,498); `position_within_sphere` (12) | Unsupported |
| PRT-INI-020 | `Radius Random` | 9,713 | `Radius Random` (9,677); `radius_random` (36) | Unsupported |
| PRT-INI-021 | `Random position within a curved cylinder` | 1 | `Random position within a curved cylinder` | Unknown |
| PRT-INI-022 | `Remap Control Point to Scalar` | 28 | `Remap Control Point to Scalar` | Unsupported |
| PRT-INI-023 | `Remap Control Point to Vector` | 177 | `Remap Control Point to Vector` (164); `remap control point to Vector` (13) | Unsupported |
| PRT-INI-024 | `Remap Initial Distance to Control Point to Scalar` | 192 | `Remap Initial Distance to Control Point to Scalar` | Unsupported |
| PRT-INI-025 | `Remap Distance to Control Point to Vector` | 5 | `Remap Distance to Control Point to Vector` | Unknown |
| PRT-INI-026 | `Remap Initial Scalar` | 674 | `Remap Initial Scalar` (286); `remap initial scalar` (388) | Unsupported |
| PRT-INI-027 | `Remap Noise to Scalar` | 104 | `Remap Noise to Scalar` | Unsupported |
| PRT-INI-028 | `Remap Scalar to Vector` | 91 | `Remap Scalar to Vector` (84); `remap scalar to vector` (7) | Unsupported |
| PRT-INI-029 | `Rotation Random` | 6,275 | `Rotation Random` (6,248); `rotation_random` (27) | Unsupported |
| PRT-INI-030 | `Rotation Speed Random` | 301 | `Rotation Speed Random` | Unsupported |
| PRT-INI-031 | `Rotation Yaw Flip Random` | 2,321 | `Rotation Yaw Flip Random` | Unsupported |
| PRT-INI-032 | `Rotation Yaw Random` | 699 | `Rotation Yaw Random` | Unsupported |
| PRT-INI-033 | `Sequence Random` | 2,798 | `Sequence Random` | Unsupported |
| PRT-INI-034 | `Sequence Two Random` | 16 | `Sequence Two Random` | Unsupported |
| PRT-INI-035 | `Trail Length Random` | 1,131 | `Trail Length Random` (1,129); `trail_length_random` (2) | Unsupported |
| PRT-INI-036 | `Velocity Inherit from Control Point` | 126 | `Velocity Inherit from Control Point` | Unsupported |
| PRT-INI-037 | `Velocity Noise` | 906 | `Velocity Noise` (902); `Initial Velocity Noise` (4) | Unsupported |
| PRT-INI-038 | `Velocity Random` | 1,511 | `Velocity Random` | Unsupported |
| PRT-INI-039 | `Velocity Repulse from World` | 1 | `Velocity Repulse from World` | Unsupported |

## Emitters

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-EMI-001 | `emit noise` | 86 | `emit noise` | Unsupported |
| PRT-EMI-002 | `emit_continuously` | 9,483 | `emit_continuously` | Unsupported |
| PRT-EMI-003 | `emit_instantaneously` | 2,461 | `emit_instantaneously` | Unsupported |

## Force Generators

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-FOR-001 | `Pull towards control point` | 784 | `Pull towards control point` | Unsupported |
| PRT-FOR-002 | `random force` | 1,039 | `random force` | Unsupported |
| PRT-FOR-003 | `twist around axis` | 650 | `twist around axis` | Unsupported |

## Constraints

| Stable ID | Canonical identity | TF2 occurrences | Encountered spellings | Coverage classification |
|---|---|---:|---|---|
| PRT-CON-001 | `Collision via traces` | 591 | `Collision via traces` | Unsupported |
| PRT-CON-002 | `Constrain distance to control point` | 212 | `Constrain distance to control point` | Unsupported |
| PRT-CON-003 | `Constrain distance to path between two control points` | 387 | `Constrain distance to path between two control points` | Unsupported |
| PRT-CON-004 | `Prevent passing through a plane` | 58 | `Prevent passing through a plane` | Unsupported |
| PRT-CON-005 | `Prevent passing through static part of world` | 2 | `Prevent passing through static part of world` | Unsupported |

## Generation Contract

The future generator must enumerate the exact target registry by category and ASCII-insensitive identity; join every selected PCF operator element after accepted name normalization; emit complete schemas and semantic dependencies; retain zero-occurrence registrations, aliases, Unknown and Unsupported items; and fail on a duplicate registration, unregistered occurrence, category mismatch, unclassified parameter, target-capture mismatch, stale content identity, or item count other than 95. Two clean-work-directory runs must be byte-identical.
