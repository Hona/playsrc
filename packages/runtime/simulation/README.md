# Simulation

```rust
use playsrc_simulation::{Configuration, DEFAULT_TICK_INTERVAL, FixedStepHost};

let configuration = Configuration::new(DEFAULT_TICK_INTERVAL, limits)?;
let mut host = FixedStepHost::new(configuration, simulation_adapter);
host.observe(monotonic_seconds)?; // establishes the wall-clock baseline
host.submit(&canonical_command_bytes)?;
let report = host.observe(next_monotonic_seconds)?;
let publications = host.drain_publications();
```

## Objective

Provide the single deterministic authority that advances composed Source gameplay state.

## Responsibilities

- Filter ordinary host elapsed time to `[0.001,0.1]`, accumulate binary32 duration in a binary64 remainder, and execute every selected fixed tick without a second catch-up cap.
- Own ordered command staging, pause/suspension, clock reversal, bounded output backpressure, interpolation state, faults, and shutdown.
- Publish one immutable final snapshot per completed host frame and every ordered tick event batch; fast and delayed consumers receive byte-identical publication streams while capacity remains available.
- Coordinate world, entity, movement, physics, game, and ruleset behavior in a defined order.
- Move one gameplay adapter into a caller-selected browser worker or native process without creating a second authority.

## Non-Responsibilities

- Reimplementing movement, physics, entities, games, or rulesets.
- Owning network transport, replay authority, or presentation.
- Choosing DOM visibility policy, operating-system sleep/spin policy, developer/replay time overrides, or clock-drift correction.
- Calling WASM once per tiny domain operation.

## Relationships

`Simulation` is the runtime-neutral gameplay seam. `FixedStepHost` owns pacing and invokes one adapter serially. Applications supply finite monotonic samples and explicit suspension; presentation drains immutable publications and never gates gameplay while configured output capacity remains.

The fixed tick/time contracts correspond to Valve Source SDK 2013 `src/public/{const.h,globalvars_base.h}`. Game adapters retain the per-tick ordering established by `src/game/server/{gameinterface.cpp,player.cpp,player_command.cpp}`.

## Verification

- `bun packages/runtime/simulation/scripts/verify.ts` runs formatting, debug/release traces and tests, stable Clippy, and the bounded release benchmark smoke profile.
- `bun packages/runtime/simulation/scripts/benchmark.ts full` runs the fixed 20-warmup/200-measurement profile and emits one machine-readable report containing work counts, latency distributions, queue depths, bytes, allocations, and stall recovery.

## Completion

Fixed-step host pacing is complete when all pacing rows in [`ROADMAP.md`](ROADMAP.md) remain Ready. The broader gameplay-authority denominator remains independently incomplete until composition, domain phases, canonical snapshots, native/WASM adapters, and every producer/consumer use this interface.
