# playsrc Roadmap

[`docs/roadmap-contract.md`](docs/roadmap-contract.md) defines the normative roadmap schema, aggregation rules, evidence requirements, inventory metadata, and denominator review gate. [`TERMINOLOGY.md`](TERMINOLOGY.md) defines every delivery status and coverage classification used here.

## Completion Denominator

The project roadmap contains 12 named targets. The TF2-first completion denominator contains the 9 Active targets. Project completion contains all 12 targets, including the 3 Future targets after their denominators pass review. A Future target does not block TF2-first completion and cannot be reported Complete before activation and denominator acceptance.

| Target | Lifecycle | Required roadmap aggregation |
|---|---|---|
| Reusable package set | Active | All 23 package leaf roadmaps plus the format-universe roadmap |
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
- Game-specific behavior belongs to `games/<game>/`; mode lifecycle belongs to `games/<game>/rulesets/<ruleset>/`.
- Browser product integration belongs to `apps/web/<product>/`; service process and transport integration belongs to `apps/services/<service>/`.
- Reusable operations belong to packages; command orchestration belongs to `tools/`; provisioned resources belong to `infra/`.
- PoC migration decisions are excluded from public completion denominators and delivery statuses.
- Counter-Strike: Source and legacy Source 1 CS:GO remain Future. Source 2 and undeclared games, products, rulesets, services, tools, and environments are Excluded.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Reusable package set has one accepted finite denominator for every declared format, world, runtime, presentation, content, and asset-store owner. | The format-universe roadmap and 20 format, world, runtime, content, and asset-store leaf roadmaps are Draft and contain Blocked requirements; the 3 presentation leaf roadmaps are absent. | Aggregation audit of all registered package roadmap review records, Ready rows, inventories, and exit criteria. | Blocked |
| TF2 game behavior composes completed generic packages through one accepted TF2 denominator. | The TF2 behavior denominator has not been written or accepted. | Aggregation audit of the accepted TF2 roadmap, package dependencies, integration evidence, and TF2 exit criteria. | Not started |
| TF2 jump owns a finite course and timed-run ruleset over completed TF2 behavior. | The TF2 ruleset universe and jump denominator have not been written or accepted. | Aggregation audit of ruleset-inventory authority, TF2 jump Ready rows, gameplay vectors, and product integration. | Not started |
| Recorded TF2 demos produce authoritative replay state and a declared browser replay experience without gameplay resimulation. | Replay leaf roadmaps are absent and no web application owns the replay experience. | Fixed-demo timeline, seek, snapshot, event, and presentation comparisons plus application integration evidence. | Blocked |
| The TF2 browser product delivers its finite declared experiences through completed modules without duplicate authority. | The TF2 application experience denominator and required service endpoint set have not been written or accepted. | Browser captures and integration records for every accepted experience, content build, browser target, and module boundary. | Not started |
| The Tempus browser product delivers its finite declared TF2 jump, map, course, record, ranking, and external-integration experiences. | The Tempus application experience denominator and external interface inventory have not been written or accepted. | Browser captures and fixed API integration records for every accepted Tempus experience and failure state. | Not started |
| Shared HTTP delivery exposes the finite API and immutable asset resources required by Active products. | API endpoint and asset-resource denominators have not been written or accepted. | Endpoint vectors, bounded-request evidence, immutable-cache and range comparisons, and product-consumer integration records. | Not started |
| Every repeated supported developer or operator operation has one deterministic tool command. | The supported-operation inventory and both tool denominators have not been written or accepted. | Clean-workspace command records with fixed configuration, outputs, process ownership, failure behavior, and repeated-result comparisons. | Not started |
| Every environment required by an Active application can be provisioned, observed, changed, and retired from checked definitions. | The environment and resource inventories have not been written or accepted. | Plan/apply/destroy records against isolated declared environments plus application binding and observability checks. | Not started |
| TF2 online multiplayer carries player commands and authoritative snapshots between browser clients and hosted gameplay servers while preserving one gameplay authority. | The target is Future; no implementation, integration, or evidence work has begun for its unresolved session, ruleset, transport, replication, or hosting requirements. | Multi-client controlled sessions comparing authoritative ticks, prediction reconciliation, disconnect/rejoin behavior, bounds, backpressure, and server lifecycle. | Not started |
| The Counter-Strike: Source browser product composes a finite CS:S game and ruleset universe. | The target is Future; no implementation, integration, or evidence work has begun for its unresolved game, ruleset, experience, service, or environment requirements. | Accepted leaf-roadmap aggregation plus fixed gameplay, replay, presentation, browser, service, and environment evidence for every declared experience. | Not started |
| The legacy Source 1 CS:GO browser product composes a finite legacy CS:GO game and ruleset universe without Source 2 behavior. | The target is Future; no implementation, integration, or evidence work has begun for its unresolved game, ruleset, experience, service, or environment requirements. | Accepted leaf-roadmap aggregation plus fixed gameplay, replay, presentation, browser, service, and environment evidence for every declared experience. | Not started |

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

- **Package denominators:** the format-universe roadmap and 20 format, world, runtime, content, and asset-store leaf roadmaps are Draft and contain Blocked requirements; the 3 presentation leaf roadmaps are absent. Checked every current package roadmap and inventory, `packages/**/README.md`, `ARCHITECTURE.md`, and this root roadmap.
- **TF2 denominators:** TF2 behavior, TF2 ruleset-universe, and TF2 jump roadmaps do not exist. Checked `games/tf2/**/README.md` and the current root roadmap.
- **Replay application owner:** `packages/runtime/replay` owns replay authority and names replay applications as consumers, but `apps/web/tf2` and `apps/web/tempus` do not own a recorded-replay experience and no dedicated replay application is declared. Checked those three owner READMEs, `apps/web/README.md`, and `ARCHITECTURE.md`.
- **Active product denominators:** the TF2, Tempus, API, asset-service, tool-operation, and infrastructure environment universes are not finite. Checked their owner READMEs and the current root roadmap.
- **Online multiplayer denominator:** session types, selected TF2 rulesets, browser transports, replicated-state inventory, matchmaking behavior, server lifecycle, and hosted environments are not declared as finite sets. Checked networking, simulation, TF2 web, matchmaking, game-server, and infrastructure READMEs.
- **Future game denominators:** CS:S and legacy Source 1 CS:GO game, ruleset, browser-experience, service, and environment universes are not finite. Checked both game trees, both application READMEs, and `ARCHITECTURE.md`.
