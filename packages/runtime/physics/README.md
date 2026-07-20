# Physics

## Sample

```ts
import { createPhysicsWorld } from "@playsrc/physics"

const physics = createPhysicsWorld(collision)
physics.step(tickInterval)
```

```rust
let mut physics = playsrc_physics::World::new(collision);
physics.step(tick_interval)?;
```

## Objective

Advance Source-style rigid bodies, constraints, and physical interactions deterministically.

## Current Target

The first runtime slice is the exact configured-build VPhysics/IVP convex-body, fixed-step, gravity/drag/damping, continuous-contact, bounce, persistent-friction, sleep/wake, motion-disable/enable, offset-impulse, snapshot/rollback, sticky-projectile, and physical-prop behavior named by [`ROADMAP.md`](ROADMAP.md). Configured-target matrices cover the selected motion, contact, surface, role, callback, sleep, impulse, world, model-PHY, issue-7426, and object-serialization paths. Implementation remains active until one generic solver matches every derivation vector, then every sealed held-out vector. Target continuation is reconstructed by replaying complete command histories; playsrc snapshots retain playsrc-owned contacts, manifolds, warm friction, sleep, ordering, and clock state. The TF2 seam must expand to zero-to-ten matched collisions, enable, force-at-point, sleep/wake, persistent-contact, and post-simulation facts in the same breaking migration.

Momentum Mod's archived public game code confirms a game-side polygon-body and deferred-disable integration pattern but delegates every solver transition to VPhysics and changes sticky policy. Direct configured-target comparison rejects current MIT VPhysics-Jolt as the complete solver for this slice: every compared body state differs, friction snapshots are invalid, wake/sleep callbacks are absent, and contact admission/order differs. Its licensed adapter and shape-conversion work may be reused only where field-level differentials establish the selected behavior. No replacement solver, learned transition, or trace-specific trajectory is accepted.

## Responsibilities

- Own rigid-body state, integration, contacts, constraints, sleeping, and physical material response.
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
