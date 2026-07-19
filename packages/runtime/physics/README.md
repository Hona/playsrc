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

The first runtime slice is the exact configured-build VPhysics/IVP convex-body, fixed-step, gravity/drag/damping, continuous-contact, bounce, persistent-friction, sleep/wake, motion-disable/enable, offset-impulse, snapshot/rollback, sticky-projectile, and physical-prop behavior named by [`ROADMAP.md`](ROADMAP.md). It is Blocked until a legally usable matching solver contract or complete controlled target specification, a checked native oracle corpus, a zero-to-ten-collision result seam with enable/force/post-simulation state, one exact physical-prop trace subject, and the adjacent Collision shape/contact and physical-surface producers exist.

Momentum Mod's archived public game code confirms a game-side polygon-body and deferred-disable integration pattern but delegates every solver transition to VPhysics and changes sticky policy. The MIT VPhysics-Jolt and Bullet adapters demonstrate interface architecture over different solvers; they do not establish IVP trajectories, contacts, friction, sleeping, or ordering. No replacement solver, learned transition, or trace-specific trajectory is accepted.

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
