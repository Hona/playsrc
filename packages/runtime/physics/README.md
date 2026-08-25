# Physics

## Sample

```rust
use playsrc_physics::{AuthoredFace, FeatureTopology};

let topology = FeatureTopology::new(collision_points, &authored_faces)?;
let edge = topology.edge_id(0).unwrap();
let ordered_candidates = topology.walk(edge, separation)?;
```

## Objective

Advance Source-style rigid bodies, constraints, and physical interactions deterministically.

## Current Target

The first runtime slice is the exact configured-build convex-body, fixed-step, gravity/drag/damping, continuous-contact, bounce, persistent-friction, sleep/wake, motion-disable/enable, offset-impulse, snapshot/rollback, sticky-projectile, and physical-prop behavior named by [`ROADMAP.md`](ROADMAP.md). The independently buildable Rust crate currently owns Collision-supplied compact points, authored face order, packed reciprocal directed-edge links, ordered opposite/previous feature fans, target-width strict projection scoring, virtual-edge identity, bounded topology validation, and shared phase-knot contact projection. It does not parse PHY payloads or claim a completed rigid-body solver. Configured-target matrices cover motion, contact, surface, role, callback, sleep, impulse, world, model-PHY, issue-7426, and object-serialization paths. Implementation remains active until one generic solver matches every derivation vector, then every sealed held-out vector. Target continuation is reconstructed by replaying complete command histories; playsrc snapshots retain solver-owned contacts, manifolds, warm friction, sleep, ordering, and clock state. The TF2 seam must expand to zero-to-ten matched collisions, enable, force-at-point, sleep/wake, persistent-contact, and post-simulation facts in the same breaking migration.

Momentum Mod's archived public game code confirms a game-side polygon-body and deferred-disable integration pattern but delegates every solver transition to VPhysics and changes sticky policy. Direct configured-target comparison rejects current MIT VPhysics-Jolt as the complete solver for this slice: every compared body state differs, friction snapshots are invalid, wake/sleep callbacks are absent, and contact admission/order differs. Its licensed adapter and shape-conversion work may be reused only where field-level differentials establish the selected behavior. No replacement solver, learned transition, or trace-specific trajectory is accepted.

## Responsibilities

- Own authored compact contact-feature topology, directed feature walks, phase-knot contact projection, rigid-body state, integration, contacts, constraints, sleeping, and physical material response.
- Consume collision shapes without redefining collision parsing or queries.
- Expose physical events and state to entities and simulation.

## Non-Responsibilities

- Player movement, game damage rules, or visual ragdoll presentation.
- Parsing PHY files.
- Owning gameplay authority outside physical state.

## Relationships

Consumes `collision` and PHY-derived data; simulation coordinates physics with entities and game behavior.

## Completion

Complete when the declared physical behavior family is deterministic, integrated, and supported by credible evidence.
