# Counter-Strike: Source Web Application Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines Application, Game, Ruleset, delivery status, evidence, and Complete. [`../ROADMAP.md`](../ROADMAP.md) defines the shared browser-application contract.

## Completion Denominator

This Future leaf roadmap contains exactly 5 behavior rows. Its target is activated only when the root roadmap activates the Counter-Strike: Source browser product and the CS:S game and ruleset-universe denominators are Accepted.

The denominator is Not accepted. No immutable CS:S content build, accepted game/ruleset denominator, selected application experience set, application catalog, service endpoint set, deployment environment, or review record exists.

## Inputs

- The accepted CS:S game interface and exact selected content build from [`../../../games/css/ROADMAP.md`](../../../games/css/ROADMAP.md).
- The accepted finite CS:S ruleset inventory and selected child interfaces from [`../../../games/css/rulesets/ROADMAP.md`](../../../games/css/rulesets/ROADMAP.md).
- One reviewed finite application-experience set assigning each route to exactly one game-owned ruleset or recorded-replay experience.
- The complete shared web configuration, browser matrix, limits, channel/catalog/root, lifecycle, input, settings, accessibility, observability, and deployment contracts from [`../ROADMAP.md`](../ROADMAP.md).
- Current Simulation or Replay, Networking where required, Rendering, Particle, Audio, VGUI, Asset Store/Service, and selected service interfaces.

## Outputs

- One immutable CS:S application root, catalog, canonical route grammar, selected experience identity, and product state.
- One active CS:S Simulation or Replay session with product-owned browser bindings and no duplicate authority.
- One product shell, Rendering canvas, VGUI mount, settings state, progress, failures, observations, and shutdown result satisfying the shared web contract.

## Invariants

- The application never selects a CS:S content build, ruleset, protocol, replay profile, map scenario, or experience by inference.
- Exactly one accepted CS:S game and one selected game-owned ruleset compose a gameplay session. Recorded replay uses Replay without gameplay resimulation.
- CS:S game/ruleset state, economy, weapons, objectives, movement, networking semantics, replay meaning, and presentation mappings remain outside the application.
- Future activation changes this roadmap in place with exact experiences, routes, roots, service dependencies, and evidence methods before implementation begins.
- No CS:S implementation is shared with legacy CS:GO or TF2 and no Source 2 identity can enter the catalog or route grammar.

## Ownership Exclusions

- CS:S game behavior belongs to `games/css`; mode lifecycle belongs to `games/css/rulesets/<ruleset>`.
- Generic formats, world, runtime, presentation, content, and asset behavior remains with packages. Service transport remains with service applications; provisioned resources remain with infrastructure.
- The application owns only future CS:S product composition, routes, product UI, browser adapters/resources, settings, catalogs, lifecycle, failures, and deployment configuration.
- Community records, rankings, course discovery, authentication, matchmaking, hosted servers, and online multiplayer require separately activated owners and cannot appear implicitly.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| `CSS-WEB-001` — One immutable CS:S target build and accepted game interface bind every application root, catalog entry, route, gameplay/replay composition, and evidence record. | The CS:S game roadmap records target build identity, content manifest, runtime captures, and current interface as unavailable. | **Target/application identity audit:** after game acceptance, compare every application/content/root identity and reject each one-field mismatch before product startup. | Blocked |
| `CSS-WEB-002` — One accepted finite application-experience set assigns every selected CS:S ruleset and recorded-replay experience to one canonical route and service dependency. | The CS:S ruleset inventory is Not accepted, all child roadmaps are absent, and no product selection decision exists. | **Experience/ruleset join:** join the accepted game/ruleset inventories to reviewed product selections; require exact count, one route, one owner, one root, and one evidence method per experience. | Blocked |
| `CSS-WEB-003` — One CS:S application root and catalog select only accepted content, maps, rulesets, replay profiles, and experiences; URL parsing cannot infer or override them. | No CS:S application root, catalog, route grammar, or Asset Service contract exists. | **Catalog/route conformance:** fixed accepted and mutated catalogs/URLs compare selected identities, order, direct load/history behavior, failures, and zero unlisted selection. | Blocked |
| `CSS-WEB-004` — Every activated CS:S route satisfies all shared web requirements on the exact browser matrix with product-specific settings, bindings, failures, accessibility, and deployment configuration. | Root activation, exact experience routes, content inputs, service endpoints, and current package/game interfaces are unavailable. | **Shared-contract application audit:** after activation, run every shared vector through every reviewed CS:S route/profile and compare product-specific state and resource ownership. | Blocked |
| `CSS-WEB-005` — Simulation or Replay, CS:S, one selected ruleset, presentation, assets, services, tools, and infrastructure integrate through one current interface with no duplicate game, ruleset, replay, rendering, or asset authority. | Required game, ruleset, package, service, and environment denominators are not accepted and no application implementation exists. | **Current-interface authority audit:** execute each accepted experience end-to-end and repository-audit every producer/consumer, duplicate transition, fallback, compatibility layer, legacy path, and stale artifact. | Blocked |

## Generated Inventories

No generated CS:S web-application inventory is accepted. Generated item count: 0. Accepted item count: 0.

The future application-experience inventory is blocked until the CS:S content build, game denominator, and ruleset inventory are Accepted. No output path or generator command is declared before activation review assigns them.

## Exit Criteria

The CS:S web application is Complete only when all of these predicates pass:

- The Future root target is Active, all 5 rows are Ready, and the denominator review is Accepted.
- One immutable target build, finite game/ruleset universe, finite application-experience set, route grammar, application root, catalog, service endpoint set, and deployment environment are current and accepted.
- Every experience passes the shared browser matrix and its fixed gameplay or replay, presentation, accessibility, failure, lifecycle, and shutdown comparisons.
- Every producer and consumer uses one current interface and no behavior is shared with TF2, legacy CS:GO, or Source 2.
- No required item remains Blocked, Unsupported, Unknown, Missing, Malformed, Partial, stale, duplicated, owner-conflicting, or dependent on a fallback, compatibility layer, or legacy path.

## Blockers

- **CS:S target and game denominator:** no immutable CS:S content build, exact configured provider, retained runtime capture set, accepted game inventory, or current game interface exists. Checked the CS:S game roadmap/inventory, root roadmap, local-configuration contract, and current game tree.
- **Ruleset and experience selection:** the eight candidate CS:S rulesets have no accepted authorities or child roadmaps; Competitive composition and Surf/Bhop/KZ target contracts are unresolved. No application owner can select an exact product set from unaccepted identities. Checked the ruleset roadmap/inventory and root Owner Registry.
- **Services and environments:** Asset Service, API Service, parent service, and Infrastructure have Draft denominators and no accepted endpoint/resource/deployment requirements for a CS:S product. Checked current service/infrastructure roadmaps and root dependencies.
- **Required package interfaces:** Simulation, Networking, Replay, Rendering, Particle, Audio, VGUI, Asset Store, Content, and required format/world packages are not accepted. Final application composition and evidence cannot proceed. Checked every named roadmap.
