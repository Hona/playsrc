# Physics

## Sample

```rust
use playsrc_physics::{EnvironmentConfig, PerformanceSettings, PhysicsEnvironment};

let mut environment = PhysicsEnvironment::new(
    EnvironmentConfig {
        random_seed: 1,
        gravity: [0.0, 0.0, -800.0],
        air_density: 2.0,
        timestep: 0.015,
        max_bodies: 4096,
        max_events: 4096,
        performance: PerformanceSettings::default(),
    },
    surface_registry,
)?;
environment.create_body(body)?;
environment.wake(body_identity)?;
environment.simulate(0.015)?;
let state = environment.body(body_identity).unwrap().published()?;
let contacts = environment.friction_contacts(body_identity)?;
```

## Objective

Port Source 1 rigid-body simulation exactly, with TF2 as the first target. [`ROADMAP.md`](ROADMAP.md) defines the behavior and acceptance scope.

## Current Target

`PhysicsEnvironment` owns the fixed-boundary event queue, motion phases, spatial subscriptions, retained convex pairs, overlap retries, ordered simulation islands, contact controllers, and quiet-state transitions. Ordinary body creation and `simulate` drive discovery and retained contacts; callers do not supply collision times or support features.

Contact response preserves separate active and queued velocity. Normal groups use retained-active dense solves and a bounded complementarity search with incremental factor updates. Tangent response retains authored frames, pressure, coordinates, energy, and feature-recheck state. Impact graphs, recursive hulls, pinning, world material tables and collision-solver policies are environment-owned. TF2 integration and browser gameplay acceptance remain pending.

The environment consumes immutable Collision shapes with exact authored points, packed directed edges, hierarchy, center, inertia, radii and drag data, plus the Material surface registry. Missing authored data is an error; Physics does not parse assets or manufacture substitute shapes.

Source-facing force, torque, velocity and contact queries preserve their own unit conversions and arithmetic widths. `friction_contacts` returns ordered body-relative contact data. `set_event_reporting` enables friction event reporting; both bodies' callback flags control admission. Query-normal normalization is distinct from the solver's contact normal.

Snapshots retain body, pair, controller, contact, cache, clock, random, deferred deletion and reporting state. Restore validates ownership and bounds before replacing the live environment. The initial random state and submission duration are explicit. Object archives reconstruct getter-space state separately from full environment continuation. Full cross-environment transfer remains tracked work.

`create_fluid` turns an authored volume body into a fluid controller. Intrusion, dormant membership, buoyancy, pressure and current history remain in the environment; water does not generate ordinary rigid contact impulses. Model volume and buoyancy overrides are separate body properties.

`advance_boundary`, `advance_events_before`, and explicit pair scheduling support bounded lower-level verification. Normal gameplay should use the environment-owned submission path. `contact_banks` and `statistics` expose bounded diagnostic state without adding another simulation owner.

## Responsibilities

- Own Source-domain body motion, continuous collision, response, contact lifetime and simulation ordering.
- Preserve authored shape/material contracts, deterministic continuation and rejected-operation safeguards.
- Expose physical facts and ordered events to simulation and entities.

## Non-Responsibilities

- Player command movement, game damage rules, weapon behavior and presentation.
- Parsing BSP, PHY, models or material documents.
- Selecting game effects, audio, particles or rendering resources.

## Completion

The crate is partial. Retained body/contact histories and numerical kernels are independently verified; complete dynamic lifecycle, held-out worlds, playable sticky integration and headed performance acceptance are still required.
