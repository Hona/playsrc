# Material Shader Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 PC standard-shader build inputs `src/materialsystem/stdshaders/{stdshader_dx9_inc.vpc,*.cpp}` and public shader registration macros at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, exact target-game shader-registry captures, and exact indexed VMT roots for one declared TF2, CS:S, and legacy Source 1 CS:GO content build. The captures, indexes, and VMT bytes are Missing.

Authority revision: SDK commit fixed; target registry revisions and content-build identities Missing.

Generator command: Missing.

Output path: `packages/world/material/inventories/shaders.md`.

Candidate item count: 130 PC registration identities: 89 implementations and 41 aliases. Accepted item count: 0.

Registration identity ignores ASCII case. The retained spelling below is the official declaration spelling. An alias is a registry item with its own accepted input identity and one declared selection target; it is not a second Material implementation.

## Implementation Identities

The following 89 stable identities are declared by the selected PC standard-shader build:

- A–D: `accumbuff4sample`, `accumbuff5sample`, `Aftershock_dx9`, `Bik`, `Bloom`, `BlurFilterX`, `BlurFilterY`, `BufferClearObeyStencil_DX9`, `Cable_DX9`, `Cloak_DX90`, `Cloud_dx9`, `color_projection`, `ColorCorrection`, `Compositor`, `Core_DX90`, `DebugMorphAccumulator`, `DebugMRTTexture`, `DebugTextureView_dx9`, `DecalBaseTimesLightmapAlphaBlendSelfIllum_DX9`, `DecalModulate_dx9`, `DepthWrite`, `Downsample`, `Downsample_nohdr`.
- E–I: `Engine_Post_dx9`, `EyeGlint_dx9`, `EyeRefract_dx9`, `Eyes_dx9`, `FilmDust_dx9`, `FilmGrain_dx9`, `floatcombine`, `floatcombine_autoexpose`, `floattoscreen`, `floattoscreen_vanilla`, `HDRCombineTo16Bit`, `HDRSelectRange`, `hsl_filmgrain_pass1`, `hsl_filmgrain_pass2`, `HSV`, `IntroScreenSpaceEffect`.
- L–P: `LightmappedGeneric`, `LightmappedReflective_DX90`, `Modulate_DX9`, `MonitorScreen_DX9`, `MorphAccumulate_DX9`, `MorphWeight_DX9`, `MotionBlur_dx9`, `Occlusion_DX9`, `ParticleLitGeneric_DX9`, `ParticleSphere_DX9`, `Portal_DX90`, `PortalRefract_dx9`, `PortalStaticOverlay`, `pyro_vision`.
- R–S: `Refract_DX90`, `Sample4x4`, `Sample4x4_Blend`, `screenspace_general_dx9`, `sfm_blurfilterx_shader`, `sfm_blurfiltery_shader`, `sfm_downsample_shader`, `sfm_integercombine_shader`, `Shadow`, `ShadowBuild_DX9`, `ShadowModel_DX9`, `ShatteredGlass`, `showz`, `Sky_DX9`, `Sky_HDR_DX9`, `Sprite_DX9`, `Spritecard`.
- T–Z: `Teeth_DX9`, `TreeLeaf`, `UnlitGeneric`, `UnlitTwoTexture_DX9`, `VertexLitGeneric`, `VolumeClouds_dx9`, `VortWarp_DX9`, `vr_distort_hud`, `vr_distort_texture`, `warp`, `Water_DX90`, `Water_DX9_HDR`, `WindowImposter_DX90`, `Wireframe_DX9`, `WorldTwoTextureBlend`, `WorldVertexAlpha`, `WorldVertexTransition_DX9`, `WriteStencil_DX9`, `WriteZ_DX9`.

## Alias Identities

| Alias identity | Declared target | Official declaration |
|---|---|---|
| `Aftershock` | `Aftershock_dx9` | `aftershock.cpp` |
| `BufferClearObeyStencil` | `BufferClearObeyStencil_DX9` | `BufferClearObeyStencil_dx9.cpp` |
| `Cable` | `Cable_DX9` | `cable_dx9.cpp` |
| `Cloak` | `Cloak_DX90` | `cloak.cpp` |
| `Cloud` | `Cloud_dx9` | `cloud_dx9.cpp` |
| `Core` | `Core_DX90` | `core_dx9.cpp` |
| `DebugTextureView` | `DebugTextureView_dx9` | `DebugTextureView.cpp` |
| `DecalBaseTimesLightmapAlphaBlendSelfIllum` | `DecalBaseTimesLightmapAlphaBlendSelfIllum_DX9` | `DecalBaseTimesLightmapAlphaBlendSelfIllum_dx9.cpp` |
| `DecalModulate` | `DecalModulate_DX9` | `DecalModulate_dx9.cpp` |
| `Engine_Post` | `Engine_Post_dx9` | `Engine_Post_dx9.cpp` |
| `EyeGlint` | `EyeGlint_dx9` | `eyeglint_dx9.cpp` |
| `EyeRefract` | `EyeRefract_dx9` | `eye_refract.cpp` |
| `eyes` | `Eyes_dx9` | `eyes_dx9.cpp` |
| `FilmDust` | `FilmDust_dx9` | `filmdust_dx8_dx9.cpp` |
| `FilmGrain` | `FilmGrain_dx9` | `filmgrain_dx8_dx9.cpp` |
| `LightmappedReflective` | `LightmappedReflective_DX90` | `lightmappedreflective.cpp` |
| `Modulate` | `Modulate_DX9` | `modulate_dx9.cpp` |
| `MonitorScreen` | `MonitorScreen_DX9` | `MonitorScreen_dx9.cpp` |
| `MorphAccumulate` | `MorphAccumulate_DX9` | `morphaccumulate_dx9.cpp` |
| `MorphWeight` | `MorphWeight_DX9` | `morphweight_dx9.cpp` |
| `MotionBlur` | `MotionBlur_dx9` | `motion_blur_dx9.cpp` |
| `Occlusion` | `Occlusion_DX9` | `occlusion_dx9.cpp` |
| `ParticleSphere` | `ParticleSphere_DX9` | `particlesphere_dx9.cpp` |
| `Portal` | `Portal_DX90` | `portal.cpp` |
| `PortalRefract` | `PortalRefract_dx9` | `portal_refract.cpp` |
| `Refract` | `Refract_DX90` | `refract.cpp` |
| `screenspace_general` | `screenspace_general_dx9` | `screenspace_general.cpp` |
| `ShadowBuild` | `ShadowBuild_DX9` | `shadowbuild_dx9.cpp` |
| `ShadowModel` | `ShadowModel_DX9` | `shadowmodel_dx9.cpp` |
| `Sky` | `Sky_HDR_DX9` | `sky_hdr_dx9.cpp` |
| `Sprite` | `Sprite_DX9` | `sprite_dx9.cpp` |
| `Teeth` | `Teeth_DX9` | `teeth.cpp` |
| `UnlitTwoTexture` | `UnlitTwoTexture_DX9` | `unlittwotexture_dx9.cpp` |
| `VolumeClouds` | `VolumeClouds_dx9` | `volume_clouds.cpp` |
| `VortWarp` | `VortWarp_dx9` | `vortwarp_dx9.cpp` |
| `Water` | `Water_DX9_HDR` | `water.cpp` |
| `WindowImposter` | `WindowImposter_DX90` | `windowimposter_dx90.cpp` |
| `Wireframe` | `Wireframe_DX9` | `wireframe_dx9.cpp` |
| `WorldVertexTransition` | `WorldVertexTransition_DX9` | `worldvertextransition.cpp` |
| `WriteStencil` | `WriteStencil_DX9` | `writestencil_dx9.cpp` |
| `WriteZ` | `WriteZ_DX9` | `writez_dx9.cpp` |

## Occurrence State

Per-game VMT occurrence counts, procedural-material occurrence counts, selected aliases, effective implementations, and source hashes are Missing. No candidate registration is classified `Handled`, `Intentionally inert`, `Unsupported`, or `Unknown` for a target content build until generation joins the registry with every indexed VMT root and procedural material producer.

## Generation Contract

The future checked-in generator must:

1. Evaluate the pinned PC standard-shader build inputs, including build conditions, implementation declarations, alias declarations, and shader-declared selection edges.
2. Capture and hash the exact target registry for each declared content build and fail on any disagreement with the source candidate instead of choosing one authority silently.
3. Enumerate every indexed VMT root and every declared procedural-material producer, retaining logical identity, provenance, source hash, source spelling, selected registration, and complete selection chain.
4. Emit one item per ASCII-insensitive registration identity, preserving declaration spelling and classifying implementation, alias, content occurrence count, semantic support, and every conflicting registration.
5. Sort by ASCII-insensitive registration identity and emit this file byte-identically from fixed inputs.

Acceptance requires exact registry captures and content indexes for all three target games, one checked-in generator command, two byte-identical clean-work-directory runs, 0 unregistered shader occurrences, 0 conflicting registrations, and denominator review metadata satisfying `docs/roadmap-contract.md`.
