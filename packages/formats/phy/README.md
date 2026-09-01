# PHY

## Sample

```rust
let file = playsrc_phy::parse_standalone(&bytes, profile, limits)?;
let bsp_payload = playsrc_phy::parse_payload(
    collision_bytes,
    keydata_bytes,
    solid_count,
    profile,
    limits,
)?;
```

## Objective

Parse Source 1 PHY resources into runtime-neutral collision asset data.

## Responsibilities

- Validate serialized physics headers, solids, constraints, and associated metadata.
- Represent collision geometry and physical properties without runtime-engine assumptions.
- Preserve unsupported records explicitly.
- Decode modern and legacy compact polygon partitions into Source-space convex points and triangles while retaining every exact solid body and triangle record.
- Retain authored hierarchy nodes and convex headers, including enclosing hulls and subtree links. Terminal `convexes` reference the shared `geometries` array; enclosing hulls do not become extra collision pieces.
- Preserve exact NUL-terminated keydata bytes and expose ordered nested block/scalar views without applying physics defaults.

## Non-Responsibilities

- Advancing rigid bodies or solving constraints.
- Performing world traces or player movement.
- Applying game-specific prop behavior.

## Relationships

Supplies decoded physics assets to `collision`, `physics`, map compilation, and game modules.

## Completion

Complete when the declared PHY behavior family is bounded, represented, and verified independently of a physics runtime.
