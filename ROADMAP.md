# playsrc Roadmap

[`docs/roadmap-contract.md`](docs/roadmap-contract.md) defines the normative roadmap schema, aggregation rules, evidence requirements, inventory metadata, and denominator review gate. [`TERMINOLOGY.md`](TERMINOLOGY.md) defines every delivery status and coverage classification used here.

## Completion Denominator

The project roadmap contains 12 named targets. The TF2-first completion denominator contains the 9 Active targets. Project completion contains all 12 targets, including the 3 Future targets after their denominators pass review. A Future target does not block TF2-first completion and cannot be reported Complete before activation and denominator acceptance.

| Target | Lifecycle | Required roadmap aggregation |
|---|---|---|
| Reusable package set | Active | All 24 package leaf roadmaps plus the format-universe roadmap |
| TF2 game | Active | Reusable package set and `games/tf2/ROADMAP.md` |
| TF2 jump | Active | TF2 game, `games/tf2/rulesets/ROADMAP.md`, and `games/tf2/rulesets/jump/ROADMAP.md` |
| Recorded TF2 replay | Active | Demo, networking, replay, TF2 recorded-state interpretation, presentation, and an unresolved web-application owner |
| TF2 browser product | Active | TF2 game, selected TF2 rulesets, web TF2 application, API and asset services required by its declared experiences, tools, and infrastructure |
| Tempus browser product | Active | TF2 jump, web Tempus application, API and asset services required by its declared experiences, tools, and infrastructure |
| Shared HTTP delivery | Active | API service, asset service, asset store, content consumed by declared endpoints, tools, and infrastructure |
| Developer and operator tools | Active | Tools aggregation, playsrc tool, inspector, and every module interface invoked by a declared operation |
| Hosting infrastructure | Active | Infrastructure roadmap and every resource or environment required by an Active application target |
| TF2 online multiplayer | Future | TF2 browser product, simulation, networking, selected TF2 rulesets, matchmaking, game-server service, tools, and infrastructure |
| Counter-Strike: Source browser product | Future | Reusable package set, CS:S game, accepted CS:S ruleset universe, web CS:S application, required services, tools, and infrastructure |
| Legacy Source 1 CS:GO browser product | Future | Reusable package set, legacy CS:GO game, accepted legacy CS:GO ruleset universe, web legacy CS:GO application, required services, tools, and infrastructure |

## Inputs

- The current-contract roadmap rows and exit criteria at every path in the Owner Registry.
- Accepted generated inventories identified by authority identity, authority revision, generator command, output path, and item count.
- Evidence records satisfying [`docs/roadmap-contract.md`](docs/roadmap-contract.md) for every Ready leaf row and cross-module exit criterion.
- The lifecycle and dependency set declared for each root target in this file.

## Outputs

- One derived delivery status for each of the 12 root targets.
- A TF2-first completion decision over exactly the 9 Active targets.
- A project completion decision over exactly all 12 targets.
- Exact Blocked records for every unresolved owner, denominator, dependency, or evidence method.

## Invariants

- A leaf behavior has exactly one owner. Referencing one leaf roadmap from multiple root targets does not duplicate ownership.
- Root targets aggregate leaf results and own only cross-module exit criteria; they do not restate module behavior.
- Active, Future, and Excluded are target lifecycle classifications, not delivery statuses.
- A root target is Ready only when every named child target is Complete and every root exit criterion for that target passes.
- No PoC implementation, migration item, artifact schema, or capability claim changes a playsrc delivery status.
- Source 2 never enters a denominator, inventory, evidence claim, interface, or compatibility requirement.
- The repository retains one current denominator. A requirement change updates that denominator in place and does not create an interface-version branch.

## Ownership Exclusions

- Module behavior inventories belong to the leaf owner paths in the Owner Registry.
- Generic Source panel, control, resource, scheme, localization-binding, focus, input, animation, and developer-console presentation semantics belong to `packages/presentation/vgui`; applications own mount roots, browser lifecycle and policy, and typed adapters, while games own game-specific values, mappings, command meanings, and effects.
- Game-specific behavior belongs to `games/<game>/`; mode lifecycle belongs to `games/<game>/rulesets/<ruleset>/`.
- Browser product integration belongs to `apps/web/<product>/`; service process and transport integration belongs to `apps/services/<service>/`.
- Reusable operations belong to packages; command orchestration belongs to `tools/`; provisioned resources belong to `infra/`.
- PoC migration decisions are excluded from public completion denominators and delivery statuses.
- Counter-Strike: Source and legacy Source 1 CS:GO remain Future. Source 2 and undeclared games, products, rulesets, services, tools, and environments are Excluded.

## Behavior Families

The current TF2 browser integration consumes PSMP v3 facing, PMRQ v5/PMPO v4 frames, PMST v2 alpha, PENV v3 sky/mark/Water, PVIS v3 PVS/frustum view, PSSN v8 combat/Entity/locker, Collision v2 brush, ordered projectile event batches, and PPTX v2 graceful-stop outputs through one WASM/browser/Rendering/application contract and checked native/headed workflow. Rendering additionally enforces per-face visibility, exact typed model/environment requirements, inert dynamic-light/shadow families, atomic frame/resource contracts, and aligned-capture input validation; absent lightcache/eye, Water tangent/projected-depth/underwater-overlay, shadow, explicit-command-encoder backend, and target-capture inputs remain Missing or Unsupported rather than producing fallback pixels.

The integrated package foundations add a 322-item all-class TF2 core, protocol-3 DEM framing, protocol-24 recorded-state codecs, deterministic zero-Simulation replay sessions, and typed Source entity/value/template/timer/trigger/mover/contact/save-restore behavior. The compact Soldier/Demoman direct-map session remains the sole browser gameplay authority because no complete all-class adapter exists. Recorded replay remains product-blocked on the TF2 recorded-state decoder, presentation mapping, and application upload/control path. Current-build VPhysics sticky results, native glyph-raster comparison, remaining opaque world mip chains, and the Tempus core/`jump_beef` zone contract remain explicit blockers; no solver, fallback font, inferred course, replay resimulation, water substitute, or model-specific facing repair is permitted.

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Reusable package set has one accepted finite denominator for every declared format, world, runtime, presentation, content, and asset-store owner. | The format-universe roadmap and all 24 package leaf roadmaps are Draft and contain Blocked requirements. | Aggregation audit of all registered package roadmap review records, Ready rows, inventories, and exit criteria. | Blocked |
| TF2 game behavior composes completed generic packages through one accepted TF2 denominator. | The TF2 roadmap and inventories are Draft and contain Blocked requirements. | Aggregation audit of the accepted TF2 roadmap, package dependencies, integration evidence, and TF2 exit criteria. | Blocked |
| TF2 jump owns a finite course and timed-run ruleset over completed TF2 behavior. | The TF2 ruleset-universe and jump roadmaps are Draft and contain Blocked requirements. | Aggregation audit of ruleset-inventory authority, TF2 jump Ready rows, gameplay vectors, and product integration. | Blocked |
| Recorded TF2 demos produce authoritative replay state and a declared browser replay experience without gameplay resimulation. | Demo, Networking, and Replay implement bounded recorded-state foundations and deterministic seek/restore with zero Simulation calls. The TF2 recorded-state decoder, game-owned presentation mapping, upload/control implementation, and exact originating build identity for the retained provider DEM remain absent. | Fixed-demo timeline, seek, snapshot, event, and presentation comparisons plus application integration evidence. | Blocked |
| The TF2 browser product loads declared BSP sources directly, resolves exact PAK/VPK dependencies, compiles missing runtime data in WASM, and delivers its finite experiences without duplicate authority. | The TF2 web-application roadmap and its required source, cache, compiler, and service roadmaps are Draft and contain Blocked requirements. | Empty/warm cache browser captures and integration records for every accepted experience, content build, browser target, source/range request, derived identity, and module boundary. | Blocked |
| The Tempus browser product discovers declared maps without prior playsrc conversion, compiles them on demand, and delivers its finite TF2 jump, course, record, ranking, and external-integration experiences. | The Tempus web-application roadmap and its required source, cache, compiler, and service roadmaps are Draft and contain Blocked requirements. | Empty/warm cache browser captures and fixed API integration records for every accepted Tempus experience and failure state. | Blocked |
| Shared HTTP delivery exposes immutable raw BSP/VPK objects, reproducible derived objects, descriptors, catalogs, channels, and the finite API resources required by Active products. | The API, asset-store, and asset-service roadmaps are Draft and contain Blocked requirements. | Whole/range endpoint vectors, immutable-cache comparisons, source/derived hash verification, and product-consumer integration records. | Blocked |
| Every repeated supported developer or operator operation has one deterministic tool command, while browser gameplay remains independent of prior native compilation. | The tool aggregation, playsrc tool, and inspector roadmaps are Draft and contain Blocked requirements. | Clean-workspace direct-source, cache-population, GLB-export, process-ownership, failure, and repeated-result command records. | Blocked |
| Every environment required by an Active application can be provisioned, observed, changed, and retired from checked definitions. | The infrastructure roadmap and resource inventory are Draft and contain Blocked requirements. | Plan/apply/destroy records against isolated declared environments plus application binding and observability checks. | Blocked |
| TF2 online multiplayer carries player commands and authoritative snapshots between browser clients and hosted gameplay servers while preserving one gameplay authority. | The target is Future; networking, matchmaking, game-server, TF2, selected-ruleset, browser, tool, and infrastructure roadmaps are Draft and contain Blocked requirements. | Multi-client controlled sessions comparing authoritative ticks, prediction reconciliation, disconnect/rejoin behavior, bounds, backpressure, and server lifecycle. | Blocked |
| The Counter-Strike: Source browser product composes a finite CS:S game and ruleset universe. | The target is Future; CS:S game, ruleset, browser, service, tool, and infrastructure roadmaps are Draft and contain Blocked requirements. | Accepted leaf-roadmap aggregation plus fixed gameplay, replay, presentation, browser, service, and environment evidence for every declared experience. | Blocked |
| The legacy Source 1 CS:GO browser product composes a finite legacy CS:GO game and ruleset universe without Source 2 behavior. | The target is Future; legacy CS:GO game, ruleset, browser, service, tool, and infrastructure roadmaps are Draft and contain Blocked requirements. | Accepted leaf-roadmap aggregation plus fixed gameplay, replay, presentation, browser, service, and environment evidence for every declared experience. | Blocked |

## Owner Registry

An Aggregation roadmap defines a finite child universe or cross-child exit criteria and owns no leaf behavior. A Behavior roadmap owns the exact behavior behind one module interface.

| Owner | Kind | Lifecycle | Future `ROADMAP.md` path |
|---|---|---|---|
| Format universe | Aggregation | Active | `packages/formats/ROADMAP.md` |
| KeyValues | Behavior | Active | `packages/formats/keyvalues/ROADMAP.md` |
| BSP | Behavior | Active | `packages/formats/bsp/ROADMAP.md` |
| VPK | Behavior | Active | `packages/formats/vpk/ROADMAP.md` |
| VTF | Behavior | Active | `packages/formats/vtf/ROADMAP.md` |
| VMT | Behavior | Active | `packages/formats/vmt/ROADMAP.md` |
| Studio model | Behavior | Active | `packages/formats/studio-model/ROADMAP.md` |
| PHY | Behavior | Active | `packages/formats/phy/ROADMAP.md` |
| Demo | Behavior | Active | `packages/formats/demo/ROADMAP.md` |
| Map | Behavior | Active | `packages/world/map/ROADMAP.md` |
| Material | Behavior | Active | `packages/world/material/ROADMAP.md` |
| Entity | Behavior | Active | `packages/world/entity/ROADMAP.md` |
| Collision | Behavior | Active | `packages/world/collision/ROADMAP.md` |
| Visibility | Behavior | Active | `packages/world/visibility/ROADMAP.md` |
| Movement | Behavior | Active | `packages/runtime/movement/ROADMAP.md` |
| Physics | Behavior | Active | `packages/runtime/physics/ROADMAP.md` |
| Simulation | Behavior | Active | `packages/runtime/simulation/ROADMAP.md` |
| Networking | Behavior | Active | `packages/runtime/networking/ROADMAP.md` |
| Replay | Behavior | Active | `packages/runtime/replay/ROADMAP.md` |
| Rendering | Behavior | Active | `packages/presentation/rendering/ROADMAP.md` |
| Particle | Behavior | Active | `packages/presentation/particle/ROADMAP.md` |
| Audio | Behavior | Active | `packages/presentation/audio/ROADMAP.md` |
| VGUI | Behavior | Active | `packages/presentation/vgui/ROADMAP.md` |
| Content | Behavior | Active | `packages/content/ROADMAP.md` |
| Asset store | Behavior | Active | `packages/asset-store/ROADMAP.md` |
| TF2 game | Behavior | Active | `games/tf2/ROADMAP.md` |
| TF2 ruleset universe | Aggregation | Active | `games/tf2/rulesets/ROADMAP.md` |
| TF2 jump | Behavior | Active | `games/tf2/rulesets/jump/ROADMAP.md` |
| CS:S game | Behavior | Future | `games/css/ROADMAP.md` |
| CS:S ruleset universe | Aggregation | Future | `games/css/rulesets/ROADMAP.md` |
| Legacy CS:GO game | Behavior | Future | `games/csgo/ROADMAP.md` |
| Legacy CS:GO ruleset universe | Aggregation | Future | `games/csgo/rulesets/ROADMAP.md` |
| Web applications | Aggregation | Active | `apps/web/ROADMAP.md` |
| TF2 web application | Behavior | Active | `apps/web/tf2/ROADMAP.md` |
| Tempus web application | Behavior | Active | `apps/web/tempus/ROADMAP.md` |
| CS:S web application | Behavior | Future | `apps/web/css/ROADMAP.md` |
| Legacy CS:GO web application | Behavior | Future | `apps/web/csgo/ROADMAP.md` |
| Service applications | Aggregation | Active | `apps/services/ROADMAP.md` |
| API service | Behavior | Active | `apps/services/api/ROADMAP.md` |
| Asset service | Behavior | Active | `apps/services/assets/ROADMAP.md` |
| Matchmaking service | Behavior | Future | `apps/services/matchmaking/ROADMAP.md` |
| Game-server service | Behavior | Future | `apps/services/game-servers/ROADMAP.md` |
| Tools | Aggregation | Active | `tools/ROADMAP.md` |
| playsrc tool | Behavior | Active | `tools/playsrc/ROADMAP.md` |
| Inspector | Behavior | Active | `tools/inspector/ROADMAP.md` |
| Infrastructure | Behavior | Active | `infra/ROADMAP.md` |

## Generated Inventories

No generated inventory has been accepted. Current inventory count: 0.

Every future inventory must satisfy the metadata and review requirements in [`docs/roadmap-contract.md`](docs/roadmap-contract.md). An inventory item is part of a denominator only after its authority, revision, generator command, output path, and item count are recorded and its owning roadmap review is Accepted.

## Exit Criteria

The TF2-first target is Complete only when all of these predicates pass:

- All 9 Active target rows are Ready.
- Every Active leaf roadmap in the Owner Registry has an Accepted denominator and passes its exit criteria.
- Every generated inventory consumed by an Active target is current and accepted.
- Every required producer and consumer uses the current interface and artifact shape.
- The gameplay, replay, and presentation authority invariants hold in every Active product.
- No Active requirement remains Blocked, Unsupported, Unknown, Missing, or Partial.

The project is Complete only when the TF2-first criteria pass, all 3 Future targets have been activated, and all 12 target rows are Ready.

## Blockers

- **Package denominators:** the format-universe roadmap and all 24 package leaf roadmaps are Draft and contain Blocked requirements. Checked every current package roadmap and inventory, `packages/**/README.md`, `ARCHITECTURE.md`, and this root roadmap.
- **TF2 denominators:** TF2 behavior, TF2 ruleset-universe, and TF2 jump roadmaps are Draft and contain Blocked requirements. Checked every TF2 roadmap and inventory plus the current root roadmap.
- **Recorded replay composition:** Demo, Networking, and Replay own implemented transport-neutral recorded-state foundations, and the TF2 web roadmap declares the recorded-replay consumer role. No TF2 recorded-state decoder/presentation mapper or upload/control implementation exists, and the retained provider DEM does not expose its originating Steam build identity. Checked the three package roadmaps, TF2 game/application roadmaps, fixed evidence manifest, and application tree.
- **Active product denominators:** TF2, Tempus, API, asset-service, tool-operation, and infrastructure roadmaps are Draft and contain Blocked requirements. Checked every application, service, tool, and infrastructure roadmap and inventory.
- **Online multiplayer denominator:** networking, matchmaking, game-server, TF2, selected-ruleset, browser, tool, and infrastructure roadmaps are Draft and contain Blocked requirements. Session types and selected TF2 rulesets remain unresolved.
- **Future game denominators:** CS:S and legacy Source 1 CS:GO game, ruleset, browser-experience, service, tool, and environment roadmaps are Draft and contain Blocked requirements.
