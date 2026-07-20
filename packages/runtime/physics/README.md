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

The first runtime slice is the exact configured-build VPhysics/IVP convex-body, fixed-step, gravity/drag/damping, continuous-contact, bounce, persistent-friction, sleep/wake, motion-disable/enable, offset-impulse, snapshot/rollback, sticky-projectile, and physical-prop behavior named by [`ROADMAP.md`](ROADMAP.md). Configured-target matrices now cover the selected motion, contact, surface, role, callback, sleep, impulse, world, model-PHY, issue-7426, and object-serialization paths. Implementation is Blocked because the target interface cannot serialize or import persistent contact/manifold/warm/sleep/island continuation: restoring the selected 160-byte sleeping object loses its friction snapshot and changes public transform bits. Exact rollback requires a target full-contact restore operation before derivation can pass and held-out vectors can be opened. The TF2 seam must then expand to zero-to-ten matched collisions, enable, force-at-point, sleep/wake, persistent-contact, and post-simulation facts.

Momentum Mod's archived public game code confirms a game-side polygon-body and deferred-disable integration pattern but delegates every solver transition to VPhysics and changes sticky policy. Direct configured-target comparison rejects current MIT VPhysics-Jolt for this slice: every compared body state differs, friction snapshots are invalid, wake/sleep callbacks are absent, and contact admission/order differs. No replacement solver, learned transition, or trace-specific trajectory is accepted.

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
