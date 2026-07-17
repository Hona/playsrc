# Particle

## Sample

```ts
import { createParticleSystem } from "@playsrc/particle"

const particles = createParticleSystem(rustKernel, suppliedPcfResources)
const renderItems = particles.advance({ bytes: orderedEventAndAdvanceBatch })
```

## Objective

Represent and advance Source particle definitions independently of game and GPU implementations.

## Responsibilities

- Parse caller-supplied Source 1 binary-v2/PCF-v1 bytes into bounded typed DMX graphs without filesystem or VPK access.
- Resolve ASCII-insensitive definition names, exact UUIDs, source-order replacement, ordered functions, child systems, and material dependencies.
- Advance the 33-system stock TF2 rocket/sticky closure in Rust from explicit events, seeds, timestamps, control points, and supplied collision batches.
- Produce bounded runtime-neutral sprite/trail records and one compact binary browser transfer per advancement phase.

## Non-Responsibilities

- Selecting TF2-specific effects for gameplay events.
- Owning GPU draw resources or gameplay state.
- Hiding unsupported operators behind visual approximations.
- Resolving logical paths, VMT/VTF bytes, model attachments, or collision truth.

## Relationships

Game modules bind gameplay events to effects; Content supplies exact PCF bytes; Collision supplies bounded trace results; rendering presents decoded particle output without reinterpreting particle semantics.

## Completion

Complete when the declared particle definition and behavior inventories are classified, implemented, and fairly verified.
