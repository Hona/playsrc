# Tools Roadmap

[`../docs/roadmap-contract.md`](../docs/roadmap-contract.md) defines aggregation, evidence, inventory, and denominator-review requirements. [`../TERMINOLOGY.md`](../TERMINOLOGY.md) defines Tool, delivery status, current contract, and Complete.

## Completion Denominator

This aggregation roadmap contains exactly the 2 child-completion rows below: the playsrc tool and the Inspector. It owns no leaf executable behavior. The denominator is Not accepted because neither child denominator has an Accepted review.

## Inputs

- The current [`playsrc/ROADMAP.md`](playsrc/ROADMAP.md) and [`inspector/ROADMAP.md`](inspector/ROADMAP.md), including every behavior row, generated-inventory requirement, blocker, review record, and exit criterion.
- The root Owner Registry and every accepted owner interface invoked by either child.
- Evidence records satisfying the roadmap contract for each Ready child row and every cross-child exit criterion.

## Outputs

- One derived delivery status for the playsrc tool child and one for the Inspector child.
- One Tools completion decision over exactly those 2 children.
- An exact Blocked record for any unavailable child interface, inventory, evidence method, or review requirement.

## Invariants

- Child roadmaps own executable behavior. This aggregation owns only child inclusion and cross-child closure.
- A behavior referenced by both children retains its package, game, application, infrastructure, or single tool owner; aggregation never duplicates it.
- The Tools target is Ready only when both children are Complete and every cross-child exit criterion passes.
- Child status is derived from its complete denominator and never from counts, ratios, or another child's progress.

## Ownership Exclusions

- `tools/playsrc` owns command orchestration, owned process trees, operation reports, and invocation of public owner interfaces.
- `tools/inspector` owns read-only interactive diagnostic presentation through those interfaces.
- Packages, games, rulesets, applications, and infrastructure retain every semantic behavior, state transition, artifact contract, deployment definition, and evidence predicate behind the invoked interface.
- Automated verification orchestration belongs to the playsrc tool. Interactive evidence display belongs to Inspector. Evidence claims and acceptance predicates remain with the roadmap owner they support.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every repeated supported developer or operator operation has one complete deterministic command through the playsrc tool child. | The playsrc tool has a 20-row Draft denominator, no implementation, 10 Blocked rows, and no Accepted review. | **Child-completion audit:** compare the accepted playsrc tool denominator, command surface, delegated owner interfaces, Ready evidence, and exit criteria with the root Owner Registry; require every child predicate to pass. | Blocked |
| Every declared diagnostic view reads current owner outputs through one complete Inspector child without creating another authority. | Inspector has a 16-row Draft denominator, no implementation, 15 Blocked owner-view rows, and no Accepted review. | **Child-completion audit:** compare the accepted Inspector denominator, 23 owner/evidence seams, Ready evidence, authority isolation, and exit criteria; require every child predicate to pass. | Blocked |

## Generated Inventories

This aggregation owns no generated inventory. The playsrc tool enumerates its 20 operation families directly; Inspector enumerates its 16 view families and 23 required seams directly. Generated domain inventories retain their leaf owners and enter those owners' denominators, not this aggregation denominator.

## Exit Criteria

The Tools target is Complete only when all of these predicates pass:

- Both Behavior Families rows are Ready.
- Both child denominators have Accepted review records and pass every child exit criterion.
- Every public owner interface invoked by a supported command or view is current, accepted, and exercised by integration evidence.
- Verification orchestration and interactive inspection share owner outputs without sharing mutable tool authority or duplicating evidence predicates.
- No required child behavior, interface, inventory, evidence method, or cross-child seam remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Child denominator acceptance:** neither [`playsrc/ROADMAP.md`](playsrc/ROADMAP.md) nor [`inspector/ROADMAP.md`](inspector/ROADMAP.md) has an Accepted review record. Checked both child roadmaps, the root Owner Registry, and [`../docs/roadmap-contract.md`](../docs/roadmap-contract.md).
- **Invoked owner interfaces:** package, Active application, Active service, and infrastructure roadmaps are Draft or Not accepted and expose no accepted process, deployment, environment-binding, or release implementation. Checked every current package roadmap, `apps/**/ROADMAP.md`, `infra/ROADMAP.md`, both child roadmaps, and the root Owner Registry.
