# Map WASM Binding

The binding exposes one coarse complete-map compilation call over transferred BSP/configuration bytes, stable generation-checked result handles, exact payload/error/hash reads, and explicit disposal. It invokes the same BSP, Map, Entity, Collision, and Visibility Rust crates as native callers. Worker termination is the current cancellation boundary; no JavaScript callback runs inside parsing or compilation.
