# Material Parameter And Flag Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 public material and shader interfaces plus PC shader declarations at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, exact target shader-registry captures, and every entry in exact indexed VMT documents for one declared TF2, CS:S, and legacy Source 1 CS:GO content build. The captures, indexes, and VMT bytes are Missing.

Authority revision: SDK commit fixed; target registry revisions and content-build identities Missing.

Generator command: Missing.

Output path: `packages/world/material/inventories/parameters.md`.

Candidate item count: 534: 484 parameter identities and 50 flag identities. Accepted item count: 0.

The 484 parameter identities comprise 13 standard slots and 471 shader-local `(ASCII-insensitive name, declared type)` pairs. The pinned source contains 820 active PC shader-local declaration occurrences, 468 distinct names, 471 name/type pairs, and 505 distinct name/type/declared-default contracts. Per-shader expansion, effective initializer defaults, VMT occurrences, and coverage classifications remain generation outputs.

## Parameter Types

The finite declared type universe has 14 values from `ShaderParamType_t`: `TEXTURE`, `INTEGER`, `COLOR`, `VEC2`, `VEC3`, `VEC4`, `ENVMAP` (obsolete), `FLOAT`, `BOOL`, `FOURCC`, `MATRIX`, `MATERIAL`, `STRING`, and `MATRIX4X2`.

Resolved runtime variables use the finite `MaterialVarType_t` universe: `FLOAT`, `STRING`, `VECTOR`, `TEXTURE`, `INT`, `FOURCC`, `UNDEFINED`, `MATRIX`, and `MATERIAL`. The selected shader contract determines conversion from declared parameter type to runtime variable type and vector component count.

## Standard Parameter Slots

These 13 identities occur before shader-local parameters in every selected shader registry. `Declared default` is registry metadata. `Effective initialization` is separately captured because shader initialization can replace an undefined slot with a different value.

| Stable identity | VMT/runtime name | Declared type | Declared default | State role |
|---|---|---|---|---|
| `standard:FLAGS` | `$flags` | `INTEGER` | `0` | Effective author/runtime flag bits |
| `standard:FLAGS_DEFINED` | `$flags_defined` | `INTEGER` | `0` | Bits explicitly defined by material input |
| `standard:FLAGS2` | `$flags2` | `INTEGER` | `0` | Shader-derived state bits |
| `standard:FLAGS_DEFINED2` | `$flags_defined2` | `INTEGER` | `0` | Derived-state definition mask |
| `standard:COLOR` | `$color` | `COLOR` | `[1 1 1]` | Primary color modulation |
| `standard:ALPHA` | `$alpha` | `FLOAT` | `1.0` | Alpha modulation |
| `standard:BASETEXTURE` | `$basetexture` | `TEXTURE` | `shadertest/BaseTexture` | Primary texture reference |
| `standard:FRAME` | `$frame` | `INTEGER` | `0` | Primary texture frame |
| `standard:BASETEXTURETRANSFORM` | `$basetexturetransform` | `MATRIX` | `center .5 .5 scale 1 1 rotate 0 translate 0 0` | Primary texture transform |
| `standard:FLASHLIGHTTEXTURE` | `$flashlighttexture` | `TEXTURE` | `effects/flashlight001` | Flashlight texture reference |
| `standard:FLASHLIGHTTEXTUREFRAME` | `$flashlighttextureframe` | `INTEGER` | `0` | Flashlight texture frame |
| `standard:COLOR2` | `$color2` | `COLOR` | `[1 1 1]` | Secondary color modulation |
| `standard:SRGBTINT` | `$srgbtint` | `COLOR` | `[1 1 1]` | sRGB-profile tint modulation |

## Shader-Local Parameter Identities

Each code span below is one stable `(name, type)` candidate. A name can occur under more than one type and can have multiple declared defaults in different shader registries. The accepted generator expands every candidate by shader identity, parameter index, declaration flags, declared default, effective initializer value, content occurrence, and semantic disposition.

- `BOOL` (55): `$aaenable`, `$abovewater`, `$basetexture2noenvmap`, `$basetexturenoenvmap`, `$blendframes`, `$blendtintbybasealpha`, `$bloomenable`, `$blurrefract`, `$cloakpassenabled`, `$color_depth`, `$debug_mode`, `$detail_alpha_mask_base_texture`, `$distancealpha`, `$distancealphafromdetail`, `$emissiveblendenabled`, `$enablesrgb`, `$fadeoutonsilhouette`, `$flashlightnolambert`, `$fleshdebugforcefleshon`, `$fleshinteriorenabled`, `$forcealphawrite`, `$forcecheap`, `$forceexpensive`, `$glow`, `$ignorevertexcolors`, `$intro`, `$masked`, `$mod2x`, `$nofresnel`, `$nolowendlightmap`, `$nosrgb`, `$outline`, `$phong`, `$phongalbedotint`, `$raytracesphere`, `$reflectentities`, `$rimlight`, `$rimmask`, `$scaleedgesoftnessbasedonscreenres`, `$scaleoutlinesoftnessbasedonscreenres`, `$seamless_base`, `$seamless_detail`, `$selfillumfresnel`, `$separatedetailuvs`, `$sheenpassenabled`, `$showalpha`, `$softedges`, `$spheretexkillcombo`, `$treeswaystatic`, `$unlit`, `$use_fb_texture`, `$useinstancing`, `$usingpixelshader`, `$vertexcolormodulate`, `$writez`.
- `COLOR` (19): `$cloakcolortint`, `$color`, `$colortint`, `$detailtint`, `$emissiveblendtint`, `$envmaptint`, `$fleshbordertint`, `$fleshsubsurfacetint`, `$fogcolor`, `$glowcolor`, `$outlinecolor`, `$reflecttint`, `$refracttint`, `$scroll1`, `$scroll2`, `$selfillumtint`, `$sheenmaptint`, `$silhouettecolor`, `$tint`.
- `FLOAT` (148): `$addbasetexture2`, `$addself`, `$alpha`, `$alpha2`, `$alphasharpenfactor`, `$alphatested`, `$alphatestreference`, `$autoexpose_max`, `$autoexpose_min`, `$blendtintcoloroverbase`, `$bloomamount`, `$bloomexponent`, `$bluramount`, `$c0_w`, `$c0_x`, `$c0_y`, `$c0_z`, `$c1_w`, `$c1_x`, `$c1_y`, `$c1_z`, `$c2_w`, `$c2_x`, `$c2_y`, `$c2_z`, `$c3_w`, `$c3_x`, `$c3_y`, `$c3_z`, `$cheapwaterenddistance`, `$cheapwaterstartdistance`, `$cloakfactor`, `$contrast`, `$contrast_correction`, `$corneabumpstrength`, `$deltascale`, `$depthblendscale`, `$detailblendfactor`, `$detailscale`, `$diffuse_base`, `$diffuse_white`, `$dilation`, `$dof_max`, `$dof_power`, `$dof_start_distance`, `$edge_softness`, `$edgesoftnessend`, `$edgesoftnessstart`, `$emissiveblendstrength`, `$endfadesize`, `$envmapcontrast`, `$envmapfresnel`, `$envmapsaturation`, `$eyeballradius`, `$falloffamount`, `$falloffdistance`, `$falloffoffset`, `$farfadeinterval`, `$fleshbordernoisescale`, `$fleshbordersoftness`, `$fleshborderwidth`, `$fleshglobalopacity`, `$fleshglossbrightness`, `$fleshscrollspeed`, `$flowmaptexcoordoffset`, `$fogend`, `$fogstart`, `$fresnelpower`, `$fresnelreflection`, `$glossiness`, `$glowalpha`, `$glowend`, `$glowstart`, `$glowx`, `$glowy`, `$gray_power`, `$groundmax`, `$groundmin`, `$hdrcolorscale`, `$heat_haze_scale`, `$illumfactor`, `$lightmap_gradients`, `$maxdistance`, `$maxlight`, `$maxreflectivity`, `$maxsize`, `$minlight`, `$minreflectivity`, `$minsize`, `$noise_scale`, `$outlinealpha`, `$outlineend0`, `$outlineend1`, `$outlinestart0`, `$outlinestart1`, `$overbrightfactor`, `$parallaxstrength`, `$phongboost`, `$phongexponent`, `$phongexponentfactor`, `$portalcolorscale`, `$portalopenamount`, `$portalstatic`, `$reflectamount`, `$reflectblendfactor`, `$refractamount`, `$rimlightboost`, `$rimlightexponent`, `$saturation`, `$seamless_scale`, `$selfillum_envmapmask_alpha`, `$sharpness`, `$sheenmapmaskoffsetx`, `$sheenmapmaskoffsety`, `$sheenmapmaskscalex`, `$sheenmapmaskscaley`, `$silhouettethickness`, `$startfadesize`, `$staticamount`, `$stripe_lm_scale`, `$time`, `$time_scale`, `$treeswayfalloffexp`, `$treeswayheight`, `$treeswayradius`, `$treeswayscrumblefalloffexp`, `$treeswayscrumblefrequency`, `$treeswayscrumblespeed`, `$treeswayscrumblestrength`, `$treeswayspeed`, `$treeswayspeedhighwindmultiplier`, `$treeswayspeedlerpend`, `$treeswayspeedlerpstart`, `$treeswaystartheight`, `$treeswaystartradius`, `$treeswaystrength`, `$unlitfactor`, `$vignette_min_bright`, `$vignette_power`, `$warpparam`, `$waterdepth`, `$weight0`, `$weight1`, `$weight2`, `$weight3`, `$weight_default`, `$woodcut`, `$zoomanimateseq2`.
- `FOURCC` (1): `$lights`.
- `INTEGER` (87): `$addoverblend`, `$alpha_blend`, `$alpha_blend_color_overlay`, `$alphadepth`, `$alphamasktextureframe`, `$ambientonly`, `$basemapalphaphongmask`, `$bloomtintenable`, `$bluramount`, `$bumpframe`, `$bumpframe2`, `$clearalpha`, `$clearcolor`, `$cleardepth`, `$combine_mode`, `$copyalpha`, `$corecolortextureframe`, `$cstrike`, `$depthblend`, `$detailblendmode`, `$detailframe`, `$disable_color_writes`, `$dualsequence`, `$effect`, `$envmapframe`, `$envmapmaskframe`, `$extractgreenalpha`, `$flowmapframe`, `$frame2`, `$fullbright`, `$gammacolorread`, `$hudtranslucent`, `$hudundistort`, `$invertphongmask`, `$irisframe`, `$linearread_basetexture`, `$linearread_texture1`, `$linearread_texture2`, `$linearread_texture3`, `$linearwrite`, `$maskedblending`, `$maxlumframeblend1`, `$maxlumframeblend2`, `$mode`, `$mrtindex`, `$nocolorwrite`, `$nodiffusebumplighting`, `$nowritez`, `$num_lookups`, `$orientation`, `$receiveflashlight`, `$refracttinttextureframe`, `$renderfixz`, `$selector0`, `$selector1`, `$selector10`, `$selector11`, `$selector12`, `$selector13`, `$selector14`, `$selector15`, `$selector2`, `$selector3`, `$selector4`, `$selector5`, `$selector6`, `$selector7`, `$selector8`, `$selector9`, `$selfillumtextureframe`, `$sequence_blend_mode`, `$sheenindex`, `$sheenmapmaskdirection`, `$sheenmapmaskframe`, `$splinetype`, `$spriteorientation`, `$spriterendermode`, `$ssbump`, `$stage`, `$staticblendtextureframe`, `$textureinputcount`, `$treesway`, `$usealternateviewmatrix`, `$userendertarget`, `$vertex_lit`, `$vertexalphatest`, `$x360appchooser`.
- `MATERIAL` (1): `$translucent_material`.
- `MATRIX` (8): `$alternateviewmatrix`, `$blendmasktransform`, `$bumptransform`, `$bumptransform2`, `$detailtexturetransform`, `$envmapmasktransform`, `$texture2transform`, `$texturetransform`.
- `MATRIX4X2` (4): `$textransform0`, `$textransform1`, `$textransform2`, `$textransform3`.
- `STRING` (1): `$pixshader`.
- `TEXTURE` (91): `$albedo`, `$alphamasktexture`, `$ambientoccltexture`, `$basetexture`, `$basetexture2`, `$basetexture3`, `$blendmodulatetexture`, `$bloomtexture`, `$blurredtexture`, `$blurtexture`, `$bumpcompress`, `$bumpmap`, `$bumpmap2`, `$bumpmask`, `$bumpstretch`, `$canvas`, `$cbtexture`, `$cloudalphatexture`, `$colorbar`, `$compress`, `$corecolortexture`, `$corneatexture`, `$crtexture`, `$delta`, `$detail`, `$distortmap`, `$dust_texture`, `$emissiveblendbasetexture`, `$emissiveblendflowtexture`, `$emissiveblendtexture`, `$envmap`, `$envmapmask`, `$exposure_texture`, `$fbtexture`, `$fleshbordertexture1d`, `$fleshcubetexture`, `$fleshinteriornoisetexture`, `$fleshinteriortexture`, `$fleshnormaltexture`, `$fleshsubsurfacetexture`, `$flowmap`, `$frame_texture`, `$frametexture`, `$glint`, `$grain`, `$grain_texture`, `$hdrbasetexture`, `$hdrcompressedtexture`, `$hdrcompressedtexture0`, `$hdrcompressedtexture1`, `$hdrcompressedtexture2`, `$input`, `$input_texture`, `$iris`, `$lightmap`, `$lightwarptexture`, `$noisetexture`, `$normalmap`, `$normalmap2`, `$originaltexture`, `$phongexponenttexture`, `$phongwarptexture`, `$portalcolortexture`, `$portalmasktexture`, `$ramptexture`, `$reflecttexture`, `$refracttexture`, `$refracttinttexture`, `$selfillummap`, `$selfillummask`, `$selfillumtexture`, `$sheenmap`, `$sheenmapmask`, `$sidespeed`, `$sourcemrtrendertarget`, `$srctexture0`, `$srctexture1`, `$srctexture2`, `$srctexture3`, `$staticblendtexture`, `$stretch`, `$stripetexture`, `$texture0`, `$texture1`, `$texture2`, `$texture3`, `$texture4`, `$vignette_texture`, `$vignette_tile`, `$warptexture`, `$ytexture`.
- `VEC2` (11): `$base_step_range`, `$basetextureoffset`, `$basetexturescale`, `$canvas_step_range`, `$cloudscale`, `$emissiveblendscrollvector`, `$flowmapscrollrate`, `$gray_step`, `$lightmap_step_range`, `$maskscale`, `$scale`.
- `VEC3` (25): `$ambientocclcolor`, `$canvas_color_end`, `$canvas_color_start`, `$canvas_scale`, `$color`, `$dimensions`, `$entityorigin`, `$eyeorigin`, `$eyeup`, `$forward`, `$hsv_correction`, `$leafcenter`, `$light_color`, `$light_position`, `$phongfresnelranges`, `$phongtint`, `$spriteorigin`, `$stripe_color`, `$stripe_fade_normal1`, `$stripe_fade_normal2`, `$stripe_scale`, `$texadjustlevels0`, `$texadjustlevels1`, `$texadjustlevels2`, `$texadjustlevels3`.
- `VEC4` (20): `$aainternal1`, `$aainternal2`, `$aainternal3`, `$bloomamount`, `$channel_select`, `$distortbounds`, `$flesheffectcenterradius1`, `$flesheffectcenterradius2`, `$flesheffectcenterradius3`, `$flesheffectcenterradius4`, `$glintu`, `$glintv`, `$hslnoisescale`, `$irisu`, `$irisv`, `$motionblurinternal`, `$noisescale`, `$scalebias`, `$selfillumfresnelminmaxexp`, `$weights`.

The obsolete `ENVMAP` declared type has 0 shader-local candidates in the pinned PC build. The 27 names with more than one source declaration contract are `$albedo`, `$alphatestreference`, `$basetexture`, `$basetexture2`, `$bloomamount`, `$bluramount`, `$bumpmap`, `$bumpmask`, `$cloakfactor`, `$color`, `$detail`, `$detailscale`, `$distortmap`, `$envmap`, `$envmapframe`, `$envmapmask`, `$envmapmaskframe`, `$fbtexture`, `$lightwarptexture`, `$normalmap`, `$phongexponent`, `$pixshader`, `$refractamount`, `$seamless_scale`, `$selfillumtint`, `$texture2`, and `$time`. Acceptance requires shader-qualified defaults and types; no cross-shader value is selected by name alone.

## Author And Runtime Flags

These 31 items are the complete `MaterialVarFlags_t` bit universe. A VMT spelling of `—` means the bit is set only through a typed runtime operation, not an authored scalar entry.

| Bit | Stable identity | VMT spelling | Runtime-neutral meaning |
|---:|---|---|---|
| 0 | `flag:DEBUG` | `$debug` | Debug-state request |
| 1 | `flag:NO_DEBUG_OVERRIDE` | `$no_fullbright` | Prohibit debug replacement |
| 2 | `flag:NO_DRAW` | `$no_draw` | No draw output |
| 3 | `flag:USE_IN_FILLRATE_MODE` | `$use_in_fillrate_mode` | Eligible for fill-rate mode |
| 4 | `flag:VERTEXCOLOR` | `$vertexcolor` | Consume vertex color |
| 5 | `flag:VERTEXALPHA` | `$vertexalpha` | Consume vertex alpha |
| 6 | `flag:SELFILLUM` | `$selfillum` | Enable self illumination |
| 7 | `flag:ADDITIVE` | `$additive` | Select additive blend family |
| 8 | `flag:ALPHATEST` | `$alphatest` | Enable alpha test |
| 9 | `flag:MULTIPASS` | `$multipass` | Require multiple semantic passes |
| 10 | `flag:ZNEARER` | `$znearer` | Select nearer depth comparison |
| 11 | `flag:MODEL` | `$model` | Material is used on model geometry |
| 12 | `flag:FLAT` | `$flat` | Flat debug presentation request |
| 13 | `flag:NOCULL` | `$nocull` | Disable face culling |
| 14 | `flag:NOFOG` | `$nofog` | Disable fog participation |
| 15 | `flag:IGNOREZ` | `$ignorez` | Disable depth test and writes |
| 16 | `flag:DECAL` | `$decal` | Decal depth/polygon-offset role |
| 17 | `flag:ENVMAPSPHERE` | `$envmapsphere` | Sphere-space environment map |
| 18 | `flag:NOALPHAMOD` | `$noalphamod` | Ignore ordinary alpha modulation |
| 19 | `flag:ENVMAPCAMERASPACE` | `$envmapcameraspace` | Camera-space environment map |
| 20 | `flag:BASEALPHAENVMAPMASK` | `$basealphaenvmapmask` | Base alpha supplies environment mask |
| 21 | `flag:TRANSLUCENT` | `$translucent` | Explicit translucency request |
| 22 | `flag:NORMALMAPALPHAENVMAPMASK` | `$normalmapalphaenvmapmask` | Normal-map alpha supplies environment mask |
| 23 | `flag:NEEDS_SOFTWARE_SKINNING` | `$softwareskin` | Require software-skinned vertices |
| 24 | `flag:OPAQUETEXTURE` | `$opaquetexture` | Ignore base-texture translucency |
| 25 | `flag:ENVMAPMODE` | `$envmapmode` | Environment-map mode request |
| 26 | `flag:SUPPRESS_DECALS` | `$nodecal` | Suppress decals on this material |
| 27 | `flag:HALFLAMBERT` | `$halflambert` | Half-Lambert diffuse response |
| 28 | `flag:WIREFRAME` | `$wireframe` | Wireframe raster role |
| 29 | `flag:ALLOWALPHATOCOVERAGE` | `$allowalphatocoverage` | Permit alpha-to-coverage |
| 30 | `flag:IGNORE_ALPHA_MODULATION` | — | Ignore runtime alpha modulation |

## Derived State Flags

These 19 items are the independent nonzero bits in `MaterialVarFlags2_t`. `LIGHTING_UNLIT` is the zero lighting value. `LIGHTING_MASK` is the aggregate of bits 1, 2, and 3 and is not an additional item.

| Bit | Stable identity | Runtime-neutral meaning |
|---:|---|---|
| 1 | `flag2:LIGHTING_VERTEX_LIT` | Vertex-lit lighting model |
| 2 | `flag2:LIGHTING_LIGHTMAP` | Lightmapped lighting model |
| 3 | `flag2:LIGHTING_BUMPED_LIGHTMAP` | Bumped-lightmap lighting model |
| 4 | `flag2:DIFFUSE_BUMPMAPPED_MODEL` | Bump-mapped model diffuse lighting |
| 5 | `flag2:USES_ENV_CUBEMAP` | Requires map environment cubemap |
| 6 | `flag2:NEEDS_TANGENT_SPACES` | Requires tangent-space vertices |
| 7 | `flag2:NEEDS_SOFTWARE_LIGHTING` | Requires software lighting input |
| 8 | `flag2:BLEND_WITH_LIGHTMAP_ALPHA` | Blend with lightmap alpha |
| 9 | `flag2:NEEDS_BAKED_LIGHTING_SNAPSHOTS` | Requires baked-lighting variants |
| 10 | `flag2:USE_FLASHLIGHT` | Uses flashlight state |
| 11 | `flag2:USE_FIXED_FUNCTION_BAKED_LIGHTING` | Uses fixed-function baked lighting |
| 12 | `flag2:NEEDS_FIXED_FUNCTION_FLASHLIGHT` | Requires fixed-function flashlight state |
| 13 | `flag2:USE_EDITOR` | Uses editor presentation inputs |
| 14 | `flag2:NEEDS_POWER_OF_TWO_FRAME_BUFFER_TEXTURE` | Requires power-of-two framebuffer input |
| 15 | `flag2:NEEDS_FULL_FRAME_BUFFER_TEXTURE` | Requires full framebuffer input |
| 16 | `flag2:IS_SPRITECARD` | Sprite-card semantic family |
| 17 | `flag2:USES_VERTEXID` | Requires vertex identifier input |
| 18 | `flag2:SUPPORTS_HW_SKINNING` | Supports hardware-skinned vertices |
| 19 | `flag2:SUPPORTS_FLASHLIGHT` | Supports flashlight presentation |

## Generation Contract

The future checked-in generator must:

1. Evaluate the pinned PC shader build and enumerate each selected shader's complete effective parameter registry in index order, including standard slots, local declarations, overrides, declared metadata defaults, declaration flags, and effective initialized values under each accepted selection environment.
2. Capture and hash target shader registries and fail on a source/capture mismatch, duplicate parameter index, unknown declared type, non-finite initialized value, or shader-qualified default conflict.
3. Enumerate every indexed effective VMT entry and retain logical identity, source hash, source spelling, selected shader, condition result, typed value, flag bit, semantic owner, and coverage classification.
4. Preserve unknown parameter and flag-shaped occurrences as inventory items; never collapse them into a generic ignored count.
5. Sort stable parameter identities by standard/local scope, ASCII-insensitive name, declared type, shader identity, and parameter index; sort flags by flag set and bit.

Acceptance requires exact target registries and content indexes for all three games, one checked-in generator command, two byte-identical clean-work-directory runs, exact per-shader defaults and initializers, exact per-build occurrence counts, 0 unclassified entries, and denominator review metadata satisfying `docs/roadmap-contract.md`.
