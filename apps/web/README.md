# Web Applications

## Objective

Deliver browser products assembled from playsrc modules.

## Responsibilities

- Own product routing, UI, input integration, browser workers, resource lifetime, and application composition.
- Resolve declared map sources, invoke coarse WASM compilation on cache misses, and consume verified raw/derived objects plus presentation state through package interfaces.

## Non-Responsibilities

- Reimplementing Source formats, gameplay, rulesets, rendering semantics, or asset storage.
- Requiring a prebuilt GLB or server-generated map package before a declared BSP can load.

Each child is one deployable browser product.
