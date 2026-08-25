# Material

## Sample

```rust
let material = playsrc_material::resolve(&effective_vmt)?;
let state = playsrc_material::static_state(&material, texture_alpha_facts)?;
let evaluated = playsrc_material::evaluate_proxy_program(&material.proxy_program, &variables, &context)?;
let water = playsrc_material::water_material_output(&material)?;
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
- Emit shader-qualified alpha ownership plus a typed fragment-discard requirement. Alpha-tested fence/model fragments pass `GEQUAL` at their exact reference; SpriteCard output passes `GREATER 0.01`; rejected alpha-zero RGB never becomes an opaque black fragment.
- Emit one texture-use record per request with initial frame, transform, proxy-mutation facts, color read, and a join to caller-supplied Source sampling plus every authored mip/frame/face/slice. Undefined Water textures remain undefined, out-of-range frames and incomplete authored chains fail, and no generated mip or substitute request exists.
- Classify the encountered `SpriteCard` family independently from legacy `Sprite`: forced two-sided blend/depth/discard state, exact PC initializers, dual-sequence/frame-blend/depth-feather controls, and explicit viewport/camera/depth/sheet inputs. An absent process-selected depth-feather default remains a named missing input.
- Project `Water` into a complete LDR `Water_DX90` or HDR `Water_DX9_HDR` output with only defined base/normal/flow/environment/reflection/refraction bindings; base/normal/environment frames and transforms; scale/time/depth; reflection/refraction amounts, tints, Fresnel, blur, and blend factor; fog; force/LOD state; linked beneath/overlay materials; ordered proxy evaluation; and exact authored-plane/lightmap/environment/framebuffer/view/controller/time requirements. Material opacity follows the authored translucency flag only: a refraction framebuffer never turns the opaque, depth-writing Water shader into a transparent plane. Declaration metadata strings never create active textures or render targets.
- Project decal scale, alpha-test, and suppress-decal inputs needed by Map's bounded mark association without owning decal geometry.
- Assign exact sRGB, linear, compressed-HDR, normal-data, or format-dependent read intent to target texture roles and emit typed detail, SSBump, environment-map, alpha-test, blend, cull, depth, decal-offset, fog, wireframe, and vertex-input state.
- Compile and evaluate the target AnimatedTexture, Sine, Equals, TextureTransform, TextureScroll, and WaterLOD proxies in VMT order from supplied time, frame interval, texture frame counts, variables, and water-LOD values; malformed and unsupported declarations remain explicit no-operations.
- Emit complete target `VertexLitGeneric` model state for half-Lambert, self illumination, Phong exponent/mask/texture/factor/tint/boost/Fresnel/warp, rim light/mask, cloak, sheen, tangent, ambient/local-light, and camera requirements.
- Emit distinct `UnlitGeneric` StudioModel state with authored base/detail/environment bindings and no ambient-cube, local-light, normal, or tangent requirement; preserve scalar and two-axis detail scale through every consumer.
- Emit complete target `EyeRefract` texture, scalar/vector, Studio eye-parameter, ambient/local-light, and camera requirements while retaining effective shader defaults.
- Evaluate all 17 configured model proxy identities in VMT order. Generic texture/time/copy/multiply/compare/select operations use material variables; TF2 burn, invulnerability, tint, glow, yellow-level, invisibility, sheen, and weapon-skin operations consume only typed game-owned inputs.
- Bind each model texture role to caller-supplied VTF metadata, Source wrap/min/mag/mip filtering plus selected anisotropy level, and the complete authored `(mip, frame, face, slice)` plane sequence. Missing authored planes fail; generated mips are not an output.
- Resolve current model opacity, effective self-illumination/base-alpha ownership, alpha-test/blend/depth state, and potential/current framebuffer use from exact VTF alpha facts plus explicit current material alpha and cloak values; enumerate ambient cube, local lights, camera, Studio eye, environment, framebuffer, authored-plane, and game-proxy requirements without supplying fallback inputs.
- Build one manifest-ordered surface-property registry with stable file-closure identity and first-index/later-override semantics; retain independent `$surfaceprop`/`$surfaceprop2` names and resolve unknown authored names to the exact `default` record.

## Configured Evidence

`bun packages/world/material/scripts/water-content.ts` verifies the exact build-`24245096` `jump_beef` and `pl_upward` indexed Water VMT/VTF bytes, runs both packages' configured-content tests, validates all 540 shared-normal, 270 stock-normal, and nine stock-base authored planes, and records the two explicitly missing underwater-overlay dependencies.

`cargo test --locked -p playsrc-material --test exact_alpha_water -- --ignored` reads the configured `jump_beef` graph's 309 gameplay entries and verifies the fence, three glass patches, all 13 present marks, 55 model materials, 12 projectile-particle materials, both opaque refractive Water roots, the 60-frame/nine-mip normal-map chain, and the fixed semantic identity recorded in [`inventories/jump-beef-alpha-water.md`](inventories/jump-beef-alpha-water.md).

`bun packages/presentation/rendering/tests/water-map-headed.ts` visibly renders the exact `jump_beef` BSP through browser WASM and WebGPU; it verifies reflection/refraction/intersection order, Source fog depth alpha, above/below colors, frame-0/frame-30 normal animation, and 160 timed Water-transition frames below an 8.333 ms p95 budget.

## Non-Responsibilities

- Parsing KeyValues syntax or decoding VTF image formats.
- Owning browser or GPU resources.
- Implementing game-specific effect selection.

## Relationships

Consumes `vmt`, `vtf`, and `content`; supplies material descriptions to map and presentation modules.

## Completion

Complete when the declared shader, parameter, proxy, and material-state inventories are classified and verified.

## Licensing

The Source material-state behavior in `rust/src/{lib,model,alpha,water}.rs` is adapted from Valve's Source SDK 2013 and is governed by the Source 1 SDK License retained in this repository. All other original package material remains under the repository MIT license unless identified otherwise.
