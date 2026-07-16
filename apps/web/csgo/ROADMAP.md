# Legacy Source 1 CS:GO Web Application Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines Application, Game, Ruleset, delivery status, evidence, and Complete. [`../ROADMAP.md`](../ROADMAP.md) defines the shared browser-application contract.

## Completion Denominator

This Future leaf roadmap contains exactly 5 behavior rows. Its target is activated only when the root roadmap activates the legacy Source 1 CS:GO browser product and the legacy CS:GO game and ruleset-universe denominators are Accepted.

The denominator is Not accepted. Exact target depot bytes/captures, accepted game/ruleset denominators, selected application experiences, application catalog, service endpoints, deployment environments, and a review record are unavailable.

## Inputs

- The exact legacy Source 1 CS:GO `1.38.8.1` target and accepted game interface from [`../../../games/csgo/ROADMAP.md`](../../../games/csgo/ROADMAP.md).
- The accepted finite legacy CS:GO ruleset inventory and selected child interfaces from [`../../../games/csgo/rulesets/ROADMAP.md`](../../../games/csgo/rulesets/ROADMAP.md).
- One reviewed finite application-experience set assigning each route to exactly one game-owned ruleset or recorded-replay experience.
- The complete shared web configuration, browser matrix, limits, channel/catalog/root, lifecycle, input, settings, accessibility, observability, and deployment contracts from [`../ROADMAP.md`](../ROADMAP.md).
- Current Simulation or Replay, final-protocol Networking and Demo where required, Rendering, Particle, Audio, VGUI, NAV when accepted gameplay requires it, Asset Store/Service, and selected service interfaces.

## Outputs

- One immutable legacy CS:GO application root, catalog, canonical route grammar, selected experience identity, and product state.
- One active legacy CS:GO Simulation or Replay session with product-owned browser bindings and no duplicate authority.
- One product shell, Rendering canvas, VGUI mount, settings state, progress, failures, observations, and shutdown result satisfying the shared web contract.

## Invariants

- Every gameplay, replay, protocol, content, catalog, and evidence input is fixed to legacy Source 1 CS:GO `1.38.8.1`; no earlier build or Source 2 identity can enter.
- Exactly one accepted legacy CS:GO game and one selected game-owned ruleset compose a gameplay session. Recorded replay uses Replay without gameplay resimulation.
- Game/ruleset state, items, economy, weapons, bots, objectives, movement, networking semantics, replay meaning, and presentation mappings remain outside the application.
- Future activation changes this roadmap in place with exact experiences, routes, roots, service dependencies, and evidence methods before implementation begins.
- No legacy CS:GO implementation is shared with CS:S or TF2 and no same-name ruleset is imported from another game.

## Ownership Exclusions

- Legacy CS:GO game behavior belongs to `games/csgo`; mode lifecycle belongs to `games/csgo/rulesets/<ruleset>`.
- Generic formats, world, runtime, presentation, content, and asset behavior remains with packages. Service transport remains with service applications; provisioned resources remain with infrastructure.
- The application owns only future legacy CS:GO product composition, routes, product UI, browser adapters/resources, settings, catalogs, lifecycle, failures, and deployment configuration.
- Premier queueing/veto, ratings, trust/Prime policy, Game Coordinator behavior, matchmaking, hosted servers, durable community records/rankings, and online multiplayer require separately activated owners and cannot appear implicitly.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| `CSGO-WEB-001` — Exact retained `1.38.8.1` target bytes and an accepted game interface bind every application root, catalog entry, route, gameplay/replay composition, and evidence record. | Target identifiers are fixed, but exact depot bytes, hashes, controlled captures, game inventory, and current game interface are unavailable. | **Target/application identity audit:** after game acceptance, compare every application/content/protocol/root identity and reject each one-field mismatch before product startup. | Blocked |
| `CSGO-WEB-002` — One accepted finite application-experience set assigns every selected first-party or pinned community ruleset and recorded-replay experience to one canonical route and service dependency. | The 17-item ruleset inventory is Not accepted, no child roadmap exists, and first-party/community contract inputs remain incomplete. | **Experience/ruleset join:** join accepted game/ruleset inventories to reviewed product selections; require exact count, one route, one owner, one root, and one evidence method per experience. | Blocked |
| `CSGO-WEB-003` — One legacy CS:GO application root and catalog select only accepted final-build content, maps, rulesets, replay profiles, and experiences; URL parsing cannot infer or override them. | No application root, catalog, route grammar, final-build Asset Service contract, or selected experience set exists. | **Catalog/route conformance:** fixed accepted and mutated catalogs/URLs compare selected identities, order, direct load/history behavior, failures, and zero unlisted selection. | Blocked |
| `CSGO-WEB-004` — Every activated legacy CS:GO route satisfies all shared web requirements on the exact browser matrix with product-specific settings, bindings, failures, accessibility, and deployment configuration. | Root activation, exact experience routes, target inputs, service endpoints, and current package/game interfaces are unavailable. | **Shared-contract application audit:** after activation, run every shared vector through every reviewed legacy CS:GO route/profile and compare product-specific state and resource ownership. | Blocked |
| `CSGO-WEB-005` — Simulation or Replay, legacy CS:GO, one selected ruleset, final-protocol networking/demo, presentation, assets, services, tools, and infrastructure integrate through one current interface with no duplicate authority. | Required game, ruleset, package, NAV, service, and environment denominators are not accepted and no application implementation exists. | **Current-interface authority audit:** execute each accepted experience end-to-end and repository-audit every producer/consumer, duplicate transition, fallback, compatibility layer, legacy path, and stale artifact. | Blocked |

## Generated Inventories

No generated legacy CS:GO web-application inventory is accepted. Generated item count: 0. Accepted item count: 0.

The future application-experience inventory is blocked until exact target bytes/captures and the game/ruleset inventories are Accepted. No output path or generator command is declared before activation review assigns them.

## Exit Criteria

The legacy Source 1 CS:GO web application is Complete only when all of these predicates pass:

- The Future root target is Active, all 5 rows are Ready, and the denominator review is Accepted.
- Exact `1.38.8.1` target inputs, finite game/ruleset universe, finite application-experience set, route grammar, application root, catalog, service endpoint set, and deployment environment are current and accepted.
- Every experience passes the shared browser matrix and its fixed gameplay or replay, final-protocol, presentation, accessibility, failure, lifecycle, and shutdown comparisons.
- Every producer and consumer uses one current interface and no behavior is shared with TF2, CS:S, or Source 2.
- No required item remains Blocked, Unsupported, Unknown, Missing, Malformed, Partial, stale, duplicated, owner-conflicting, or dependent on a fallback, compatibility layer, or legacy path.

## Blockers

- **Exact target and game denominator:** final-build identifiers are known, but exact depot bytes/indexes/hashes, controlled runtime captures, accepted game inventory, NAV owner, bot evidence, and current game interface are unavailable. Checked the legacy CS:GO game roadmap/inventory, root roadmap, local-configuration contract, and current game tree.
- **Ruleset and experience selection:** the 17 candidate rulesets have no Complete child roadmaps; final-build first-party inputs and accepted community configuration/course manifests are unavailable. No application owner can select an exact product set from unaccepted identities. Checked the ruleset roadmap/inventory and root Owner Registry.
- **Services and environments:** Asset Service, API Service, parent service, matchmaking/game-server services, and Infrastructure have Draft denominators and no accepted endpoint/resource/deployment requirements for this Future product. Checked current service/infrastructure roadmaps and root dependencies.
- **Required package interfaces:** Simulation, Networking, Demo, Replay, Rendering, Particle, Audio, VGUI, NAV, Asset Store, Content, and required format/world packages are not accepted. Final application composition and evidence cannot proceed. Checked every named roadmap and the format-universe owner registry.
