# Native And WASM Contract

This document defines the execution and data boundary between Rust-owned Source behavior and TypeScript-owned browser integration.

## Ownership

Rust owns:

- Bounded KeyValues, BSP, VPK, VTF, VMT, StudioModel, PHY, and DEM parsing.
- Map dependency discovery and canonical compilation.
- Geometry, displacement, lightmap, collision, and visibility preparation.
- Movement, rigid-body physics, gameplay simulation, prediction transitions, replay decoding, and network codecs.
- Particle definition evaluation and particle-state advancement.
- Hashing, canonical serialization, and derived-cache identities.

TypeScript owns:

- Fetch and HTTP byte-range adapters.
- Worker creation, readiness, cancellation, termination, and failure reporting.
- HTTP cache and IndexedDB adapters.
- Three.js GPU resources, command submission, canvas lifecycle, and browser frame opportunities.
- Web Audio resources and browser permission lifecycle.
- VGUI DOM/CSS presentation.
- Preact product shells, routes, product state, and browser input.
- Concrete browser networking transports.

TypeScript never reimplements Rust-owned Source behavior. Rust never owns DOM, GPU, browser audio, URL routing, or network transport objects.

## Execution Topology

```text
Preact shell and browser adapters
    <-> typed worker messages
module worker
    <-> coarse generated bindings
Rust/WASM module
```

Heavy Rust/WASM work never executes on the browser main thread. Native tools and servers call the same Rust crates without the worker or WASM adapters.

## Interface Rule

One interface call performs one complete domain phase or bounded batch. Call count cannot scale directly with face count, entity count, trace count, particle count, VPK entry count, or another unbounded content cardinality.

Prohibited JS loops include:

```ts
for (const face of faces) wasm.compileFace(face)
for (const trace of traces) wasm.trace(trace)
for (const entity of entities) wasm.updateEntity(entity)
for (const particle of particles) wasm.updateParticle(particle)
```

Accepted interfaces include:

```ts
compiler.beginMap(mapSource)
compiler.supplyObjects(objectBatch)
compiler.advance(workBudget)
compiler.readResult()

simulation.submitCommands(commandBatch)
simulation.advance(tickCount)
simulation.readSnapshot()
```

## Map Compilation Protocol

Map compilation uses bounded dependency rounds:

1. TypeScript supplies the verified `MapSource` and initial BSP bytes or byte ranges.
2. Rust parses available bytes and emits one ordered batch of dependency requests.
3. TypeScript resolves requests through Content providers and supplies one object batch.
4. Rust advances every dependency made ready by that batch and emits the next request batch.
5. Steps 3 and 4 repeat until compilation succeeds, fails, is cancelled, or reaches a declared bound.
6. Rust emits canonical serialized objects, runtime descriptors, direct renderer buffers, coverage classifications, and diagnostics in one result batch.

A dependency request contains stable request identity, requesting object, logical path or byte range, expected representation, accepted providers, priority, and byte bound. A response contains request identity, object identity, exact bytes, provenance, or one classified error.

No Rust call invokes JavaScript once per dependency. No JavaScript callback runs from inside a parser loop.

## Simulation Protocol

Commands are submitted as ordered batches covering one or more players and ticks. `advance(tickCount)` performs complete fixed-tick phases inside Rust. `readSnapshot()` returns one packed immutable snapshot and ordered event batch.

Rendering and VGUI consume snapshots. They never query individual authoritative fields through repeated WASM calls during a frame.

Prediction and server authority call the same Rust transitions with different declared input/state roles. Networking transports commands and snapshots but never owns movement or gameplay behavior.

## Data Representation

Cross-boundary data uses:

- Transferable `ArrayBuffer` ownership for large immutable or single-owner batches.
- Typed-array views over packed numeric buffers.
- Compact binary descriptors with explicit schema, byte order, lengths, offsets, counts, and limits.
- Stable integer handles for Rust-owned long-lived state.
- Explicit `dispose` or release operations for every long-lived handle.

Canonical maps, snapshots, meshes, lightmaps, collision data, and particle state never cross the boundary as large JSON object graphs.

Strings cross only when they are required logical identities, diagnostics, localization keys, or user-facing text. Repeated strings use tables or stable IDs.

## Memory

The caller transferring an `ArrayBuffer` relinquishes access until ownership is explicitly returned. Shared memory is excluded from the initial contract. It may be introduced only after retained profiling proves that transferable batches cannot satisfy an accepted latency or throughput bound.

Rust validates every offset, length, count, alignment, handle generation, and total byte bound before access. TypeScript treats every returned buffer as immutable unless its descriptor explicitly transfers mutable ownership.

## Cancellation And Failure

Every long-running operation has one cancellation identity checked at bounded Rust work intervals. Cancellation returns no partial authoritative result. Completed immutable cache objects remain valid; operation-owned temporary buffers and handles are released.

Errors cross as typed codes plus bounded structured fields. Rust panic text, JavaScript stack traces, private paths, raw pointers, credentials, and unrestricted source bytes never become public diagnostics.

## Performance Evidence

Each external interface records:

- Calls per operation or simulation tick.
- Bytes transferred in each direction.
- Transfer and serialization duration.
- Rust execution duration.
- Worker queue depth and wait duration.
- Peak WASM memory.
- Peak browser-owned buffer memory.

An interface is rejected when call count scales with an unbounded content cardinality or boundary overhead prevents an accepted operation bound. Optimization changes batching or representation; it never moves Source behavior into TypeScript.
