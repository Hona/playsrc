# Material

## Sample

```rust
let material = playsrc_material::resolve(&effective_vmt)?;
let state = playsrc_material::static_state(&material, texture_alpha_facts)?;
let evaluated = playsrc_material::evaluate_proxy_program(&material.proxy_program, &variables, &context)?;
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
- Project `Water` inputs into typed above-water, fog, reflection, refraction, environment-cubemap, normal-map, bottom-material, cheap-water, and animation/proxy facts without creating renderer resources.
- Project decal scale, alpha-test, and suppress-decal inputs needed by Map's bounded mark association without owning decal geometry.
- Assign exact sRGB, linear, compressed-HDR, normal-data, or format-dependent read intent to target texture roles and emit typed detail, SSBump, environment-map, alpha-test, blend, cull, depth, decal-offset, fog, wireframe, and vertex-input state.
- Compile and evaluate the target AnimatedTexture, Sine, Equals, TextureTransform, TextureScroll, and WaterLOD proxies in VMT order from supplied time, frame interval, texture frame counts, variables, and water-LOD values; malformed and unsupported declarations remain explicit no-operations.
- Emit complete target `VertexLitGeneric` model state for half-Lambert, self illumination, Phong exponent/mask/texture/factor/tint/boost/Fresnel/warp, rim light/mask, cloak, sheen, tangent, ambient/local-light, and camera requirements.
- Emit complete target `EyeRefract` texture, scalar/vector, Studio eye-parameter, ambient/local-light, and camera requirements while retaining effective shader defaults.
- Evaluate all 17 configured model proxy identities in VMT order. Generic texture/time/copy/multiply/compare/select operations use material variables; TF2 burn, invulnerability, tint, glow, yellow-level, invisibility, sheen, and weapon-skin operations consume only typed game-owned inputs.
- Bind each model texture role to caller-supplied VTF metadata, Source wrap/min/mag/mip filtering plus selected anisotropy level, and the complete authored `(mip, frame, face, slice)` plane sequence. Missing authored planes fail; generated mips are not an output.
- Resolve current model opacity, effective self-illumination/base-alpha ownership, alpha-test/blend/depth state, and potential/current framebuffer use from exact VTF alpha facts plus explicit current material alpha and cloak values; enumerate ambient cube, local lights, camera, Studio eye, environment, framebuffer, authored-plane, and game-proxy requirements without supplying fallback inputs.

## Non-Responsibilities

- Parsing KeyValues syntax or decoding VTF image formats.
- Owning browser or GPU resources.
- Implementing game-specific effect selection.

## Relationships

Consumes `vmt`, `vtf`, and `content`; supplies material descriptions to map and presentation modules.

## Completion

Complete when the declared shader, parameter, proxy, and material-state inventories are classified and verified.

## Licensing

The Source material-state behavior in `rust/src/lib.rs` and `rust/src/model.rs` is adapted from Valve's Source SDK 2013 and is governed by the Source 1 SDK License retained in this repository. All other original package material remains under the repository MIT license unless identified otherwise.
