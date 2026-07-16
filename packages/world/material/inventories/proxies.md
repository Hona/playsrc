# Material Proxy Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 TF2 PC client build inputs `src/game/client/{client_base.vpc,client_econ_base.vpc,client_tf.vpc}`, public material-proxy interfaces, the registration declarations in the source groups below at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, exact target proxy-registry captures, and every proxy declaration in exact indexed VMT documents for one declared target-game content build. The captures, indexes, and VMT bytes are Missing.

Authority revision: SDK commit fixed; target registry revisions and content-build identities Missing.

Generator command: Missing.

Output path: `packages/world/material/inventories/proxies.md`.

Candidate item count: 84 ASCII-insensitive TF2-client registration identities in 37 source groups. Accepted item count: 0.

Each code span in `Registration identities` is one stable inventory item. `Evaluation input` names the state category required to reproduce the operation; it does not transfer game-state derivation, renderer state, or external-resource ownership to Material. Material owns declaration validation, source ordering, variable reads/writes, and the evaluated-state trace.

| Official source group | Registration identities | Evaluation input | Adjacent input owner |
|---|---|---|---|
| `src/game/client/AnimateSpecificTextureProxy.cpp` | `AnimateSpecificTexture` | Absolute time, texture frames, entity animation start | Generic entity snapshot |
| `src/game/client/C_MaterialModifyControl.cpp` | `MaterialModify`, `MaterialModifyAnimated` | Controller value, animation range, absolute time | Generic entity snapshot |
| `src/game/client/IsNPCProxy.cpp` | `IsNPC` | Entity classification | Generic entity snapshot |
| `src/game/client/ProxyHealth.cpp` | `Health` | Entity health range | Generic entity snapshot |
| `src/game/client/WaterLODMaterialProxy.cpp` | `WaterLOD` | Water LOD start/end | Map/application configuration |
| `src/game/client/WorldDimsProxy.cpp` | `WorldDims` | World bounds | Map |
| `src/game/client/alphamaterialproxy.cpp` | `Alpha` | Entity render alpha | Generic entity snapshot |
| `src/game/client/animatedentitytextureproxy.cpp` | `AnimatedEntityTexture` | Absolute time, entity animation start, texture frames | Generic entity snapshot |
| `src/game/client/animatedoffsettextureproxy.cpp` | `AnimatedOffsetTexture` | Absolute time, frame interval, texture frames | Application time source |
| `src/game/client/animatedtextureproxy.cpp` | `AnimatedTexture` | Absolute time, frame interval, texture frames | Application time source |
| `src/game/client/c_func_breakablesurf.cpp` | `BreakableSurface` | Breakable-surface state | Generic entity snapshot |
| `src/game/client/c_func_conveyor.cpp` | `ConveyorScroll` | Conveyor direction and speed | Generic entity snapshot |
| `src/game/client/camomaterialproxy.cpp` | `Camo` | Entity transform, view, scene capture inputs | Generic entity snapshot and Rendering |
| `src/game/client/clientshadowmgr.cpp` | `Shadow`, `ShadowModel` | Shadow texture and projection inputs | Rendering |
| `src/game/client/dummyproxy.cpp` | `Dummy` | No value input | Material |
| `src/game/client/econ/tool_items/custom_texture_cache.cpp` | `CustomSteamImageOnModel` | Selected external image binding | Game/application adapter |
| `src/game/client/entityoriginmaterialproxy.cpp` | `EntityOrigin`, `EntityOriginAlyx`, `Ep1IntroVortRefract` | Entity origin and effect state | Generic entity snapshot; game adapter for named effect state |
| `src/game/client/lampbeamproxy.cpp` | `lampbeam` | Lamp beam state | Generic entity snapshot |
| `src/game/client/lamphaloproxy.cpp` | `lamphalo` | Lamp halo state | Generic entity snapshot |
| `src/game/client/mathproxy.cpp` | `Add`, `Subtract`, `Multiply`, `Divide`, `Clamp`, `Sine`, `Equals`, `Frac`, `Int`, `LinearRamp`, `UniformNoise`, `GaussianNoise`, `Exponential`, `Abs`, `Empty`, `LessOrEqual`, `WrapMinMax`, `SelectFirstIfNonZero` | Material variables; `Sine` and `LinearRamp` also use absolute time; noise operations use supplied random samples | Material and application evaluation context |
| `src/game/client/matrixproxy.cpp` | `TextureTransform`, `MatrixRotate` | Material variables | Material |
| `src/game/client/particle_proxies.cpp` | `ParticleSphereProxy` | Particle sphere state | Particle |
| `src/game/client/proxyplayer.cpp` | `PlayerProximity`, `PlayerTeamMatch`, `PlayerView`, `PlayerSpeed`, `PlayerPosition`, `EntitySpeed`, `EntityRandom`, `PlayerLogo` | Player/entity/view state, supplied random value, or external logo binding | Generic entity snapshot; game/application adapter for player/logo selection |
| `src/game/client/proxypupil.cpp` | `Pupil` | Eye entity, view, and lighting inputs | Generic entity snapshot and Rendering |
| `src/game/client/replay/replayrenderer.cpp` | `accumbuff4sample` | Replay accumulation textures and weights | Replay and Rendering |
| `src/game/client/texturescrollmaterialproxy.cpp` | `TextureScroll` | Absolute time and material variables | Application time source and Material |
| `src/game/client/tf/c_baseobject.cpp` | `ObjectPower`, `building_invis` | TF2 building power and visibility state | TF2 game |
| `src/game/client/tf/c_tf_player.cpp` | `spy_invis`, `InvulnLevel`, `BurnLevel`, `YellowLevel`, `ModelGlowColor`, `CommunityWeapon`, `HeartbeatScale`, `BenefactorLevel`, `BuildingRescueLevel`, `ShieldFalloff`, `WheatlyEyeGlow`, `AnimatedWeaponSheen`, `StatTrakIllum`, `StatTrakDigit`, `StatTrakIcon`, `WeaponSkin` | TF2 player, item, condition, building, weapon, animation, and presentation-mapping state | TF2 game |
| `src/game/client/tf/teammaterialproxy.cpp` | `TeamTexture` | TF2 render-team state | TF2 game |
| `src/game/client/timematerialproxy.cpp` | `CurrentTime` | Absolute time | Application time source |
| `src/game/client/toggletextureproxy.cpp` | `ToggleTexture` | Entity texture-frame index and texture frames | Generic entity snapshot |
| `src/game/client/viewpostprocess.cpp` | `engine_post`, `MotionBlur` | View and post-process state | Rendering |
| `src/game/shared/econ/econ_wearable.cpp` | `ItemTintColor` | Item tint and team mapping | Game module |
| `src/game/shared/tf/tf_viewmodel.cpp` | `vm_invis`, `invis` | TF2 viewmodel and player visibility state | TF2 game |
| `src/game/shared/tf/tf_weapon_grenade_pipebomb.cpp` | `StickybombGlowColor` | TF2 projectile and team state | TF2 game |
| `src/game/shared/tf/tf_weapon_sniperrifle.cpp` | `SniperRifleCharge` | TF2 weapon charge state | TF2 game |
| `src/game/shared/tf/tf_weaponbase.cpp` | `weapon_invis` | TF2 weapon-owner visibility state | TF2 game |

## Evaluation Contract

- Registry lookup ignores ASCII case and preserves declaration spelling. Unknown names remain `Unknown`; no name is selected by prefix, edit distance, or source file.
- Every proxy receives its complete VMT declaration subtree. Argument lookup, required arguments, optional defaults, variable references, component selectors, accepted variable types, and initialization failure are proxy-qualified contracts.
- Proxies execute in VMT source order. Every operation reads the variable state produced by preceding operations. Repeated registrations remain repeated operations.
- Time is supplied as absolute presentation seconds plus frame interval. Entity and game data are immutable snapshots. Random values are supplied as a deterministic ordered sample stream. External images, shadows, particle values, replay values, and post-process values arrive through typed adapters.
- A game-owned adapter derives game-specific values. Material applies those values to declared material variables and records the write; it does not inspect gameplay state directly.
- Missing required input is `Missing`, invalid arguments or values are `Malformed`, a registered but unimplemented operation is `Unsupported`, and an unregistered identity is `Unknown`. Every such declaration records its no-operation disposition; later declarations continue only when the proxy-qualified target contract requires continuation.

## Occurrence State

Per-game proxy occurrence counts, argument-key/value shapes, initialization defaults, repeated sequences, source hashes, and coverage classifications are Missing. The official TF2 source candidate does not establish the complete CS:S or legacy Source 1 CS:GO proxy registries.

## Generation Contract

The future checked-in generator must:

1. Evaluate the pinned TF2 PC client build inputs and enumerate every active material-proxy registration with registration identity, source group, argument schema, default values, variable reads/writes, input dependencies, and game ownership.
2. Capture and hash the exact target registry for each declared game and fail on any source/capture mismatch, duplicate ASCII-insensitive registration, or registration without one owner.
3. Enumerate every proxy declaration in indexed effective VMT documents, retaining material logical identity, provenance, source hash, declaration order, repeated identity, complete argument subtree, registry result, and coverage classification.
4. Emit unknown and unsupported declarations as items instead of omitting them. Record exact per-content-build occurrence and sequence counts.
5. Sort registry items by ASCII-insensitive identity and occurrence records by material logical identity plus source order; emit byte-identically from fixed inputs.

Acceptance requires exact registry captures and content indexes for all three target games, one checked-in generator command, two byte-identical clean-work-directory runs, 0 unclassified proxy declarations, exact game-input ownership, and denominator review metadata satisfying `docs/roadmap-contract.md`.
