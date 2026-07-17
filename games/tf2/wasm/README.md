# TF2 WASM Binding

The module compiles one BSP/configuration pair through Map, Material, VTF, StudioModel v2, Particle, Entity, Collision, and Visibility and initializes one generation-bound gameplay session at the first ordered valid `info_player_teamspawn`. Handles retain canonical map/presentation bytes, exact decoded model resources, Particle sheets, visibility/collision worlds, gameplay state, bounded phase outputs, and explicit disposal.

`playsrc_compile_map` accepts LDR profile `0` or HDR profile `1`. It returns classified failure for malformed BSP, unknown profile, failed canonical compilation, missing/malformed spawn, failed Entity runtime, incomplete lighting, missing material/profile dependencies, or missing model dependencies. `playsrc_spawn_copy` emits exactly 40 little-endian `PSIV` version-1 bytes containing entity ordinal, Hammer ID or `0xffffffff`, origin, and angles.

`playsrc_game_advance` accepts one 40-byte `PCMD` v2 command and 1–64 ticks, clones the complete session, emits one bounded `PSSN` v4 snapshot, and commits only after every transition and byte validates. V4 appends a final-tick `PMTK` record containing mode, crouch, grounded, wish, jump, hull, step, query/contact/event counts, and mover result.

`playsrc_model_transact` executes one bounded batch of explicit model/activity/time/skin/bodygroup/LOD requests through StudioModel and returns exact timing, cycles, posed vertices, tangents, attachments, and client presentation events. `playsrc_particle_transact` resolves exact VTF sheet samples before emitting PSPR v2 render records.

`playsrc_jump_configure` atomically installs one 52–65,536-byte `PJMP` version-1 linear course whose map hash equals the session BSP and whose trigger identities resolve in the Entity world. `playsrc_runtime_count` exposes button, door, movelinear, multiple, hurt, push, catapult, teleport, regenerate, and teleport-destination counts for fixed map evidence. Any stale handle, malformed command/course, failed transition, or snapshot overrun preserves the prior live session and snapshot.
