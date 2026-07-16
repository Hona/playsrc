# Material

## Sample

```ts
import { loadMaterial } from "@playsrc/material"

const material = await loadMaterial(content, "materials/concrete/concretefloor001a.vmt")
```

```rust
let material = playsrc_material::load(&content, "materials/concrete/concretefloor001a.vmt")?;
```

## Objective

Resolve Source material documents and textures into runtime-neutral material behavior.

## Responsibilities

- Classify shaders, parameters, flags, proxies, animation, and material state.
- Resolve texture and material references through explicit inputs.
- Produce semantic descriptions usable by renderers without transferring material authority to them.

## Non-Responsibilities

- Parsing KeyValues syntax or decoding VTF image formats.
- Owning browser or GPU resources.
- Implementing game-specific effect selection.

## Relationships

Consumes `vmt`, `vtf`, and `content`; supplies material descriptions to map and presentation modules.

## Completion

Complete when the declared shader, parameter, proxy, and material-state inventories are classified and verified.
