# Material

## Sample

```rust
let material = playsrc_material::resolve(&effective_vmt)?;
```

## Objective

Resolve Source material documents and textures into runtime-neutral material behavior.

## Responsibilities

- Classify shaders, parameters, flags, proxies, animation, and material state.
- Resolve texture and material references through explicit inputs.
- Produce semantic descriptions usable by renderers without transferring material authority to them.
- Emit exact typed VTF dependency identities and built-in environment/render-target dispositions without invoking Content or decoding textures.
- Evaluate conditional material keys against an explicit PC LDR, integer-HDR, or float-HDR environment; retain active/inactive decisions; and apply active conditional values as replacements without activating unknown conditions.
- Select `Sky` LDR/HDR implementations and exactly one complete `$basetexture`, `$hdrbasetexture`, `$hdrcompressedtexture`, or `$hdrcompressedtexture0/1/2` role set. A missing HDR role is an error, not an LDR substitution.

## Non-Responsibilities

- Parsing KeyValues syntax or decoding VTF image formats.
- Owning browser or GPU resources.
- Implementing game-specific effect selection.

## Relationships

Consumes `vmt`, `vtf`, and `content`; supplies material descriptions to map and presentation modules.

## Completion

Complete when the declared shader, parameter, proxy, and material-state inventories are classified and verified.
