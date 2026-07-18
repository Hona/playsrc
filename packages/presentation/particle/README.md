# Particle

## Sample

```ts
import { createParticleSystem } from "@playsrc/particle"

const particles = createParticleSystem(rustKernel, suppliedPcfResources)
const renderBatch = particles.advance({ bytes: orderedEventAndAdvanceBatch })
```

## Objective

Represent and advance Source particle definitions independently of game and GPU implementations.

## Responsibilities

- Parse caller-supplied Source 1 binary-v2/PCF-v1 bytes into bounded typed DMX graphs without filesystem or VPK access.
- Resolve ASCII-insensitive definition names, exact UUIDs, source-order replacement, ordered functions, child systems, and material dependencies.
- Advance the 33-system stock TF2 rocket/sticky closure in Rust from explicit events, the Source particle random table and four-lane stream, timestamps, control points, shared collision-plane batches, lifecycle commands, and ten-step limits.
- Combine caller-supplied Material shader/blend/color-space facts with typed VTF sheets, sample Source's 1,024-entry timelines, derive exact trail endpoint/width/length, and produce one immutable version-3 batch containing finite bounds plus complete 436-byte sprite/trail records.

## Non-Responsibilities

- Selecting TF2-specific effects for gameplay events.
- Owning GPU draw resources or gameplay state.
- Hiding unsupported operators behind visual approximations.
- Resolving logical paths, VMT/VTF bytes, model attachments, or collision truth.

## Relationships

Game modules bind gameplay events to effects; Content supplies exact PCF bytes; Collision supplies bounded trace results; rendering presents decoded particle output without reinterpreting particle semantics.

## Completion

Complete when the declared particle definition and behavior inventories are classified, implemented, and fairly verified.
- Execute bounded browser-worker PCF transactions over exact projectile mapper requests and return one compact bounds-and-render-item batch per frame; supplied Collision batches remain the only world-contact input.
- Validate the exact configured nine-root, 33-definition timeline through natural death or graceful stop and hash every output field across 126 fixed batches.
- Retain one fixed bounds-state visual-region identity for each of the nine projectile roots across the 14 configured timeline samples.

`rust/src/source_random.rs` is adapted from Valve's public Source SDK 2013 particle artifact and is governed by [`SOURCE-1-SDK-LICENSE.txt`](SOURCE-1-SDK-LICENSE.txt); the remaining original package material is MIT-licensed.
