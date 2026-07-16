# Matchmaking Service Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines denominator and inventory requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines gameplay authority, Application, delivery status, bounds, backpressure, current contract, and Complete. The parent [`../ROADMAP.md`](../ROADMAP.md) defines common process behavior.

## Completion Denominator

This Future leaf roadmap contains exactly 18 behavior rows. Planned generated inventories for queue profiles and concrete client/peer endpoints each contain 0 candidate, generated, and accepted items and contribute 0 denominator items. The current denominator is 18 rows and is Not accepted.

Party matchmaking is not declared by an Active or Future product contract and is outside the current denominator. Adding parties requires one explicit denominator change that defines party identity, membership authority, leader permissions, atomic queueing, partial disconnect, cancellation, and assignment behavior before implementation.

## Inputs

- One accepted parent service configuration and deployment binding.
- One generated immutable queue profile per selectable online-multiplayer composition. A profile names game, ruleset, in-session policy, region set and ordering, player-count bounds, join-in-progress policy, skill/latency selectors when declared, immutable game/map/application roots, networking profile, server process profile, queue capacity, ticket lifetime, selection cadence, allocation deadline, admission deadline, reconnect policy, and every numeric bound.
- One authenticated solo player identity and one immutable queue request naming exactly one queue profile, client build/root identities, transport capabilities, region measurements, and an idempotency identity. No party value is accepted in the current contract.
- One current game-server allocation interface accepting a process profile and immutable roots and returning one atomically reserved server identity, transport endpoints, process health identity, and allocation result.
- One current Networking admission interface and game-owned/ruleset-owned compatibility declarations. Matchmaking can compare declared identities and policies but cannot decode gameplay messages or alter game state.
- A monotonic clock, deterministic selection epoch identity, cancellation signals, and accepted finite limits for tickets, queues, candidates, selections, allocations, admissions, sessions, observations, and retained terminal records.
- Open Match 1.8.0 public Ticket, Pool, Match, Assignment, cancellation, and assignment-watching contracts as architecture comparison input; playsrc defines its own current interface and does not inherit Open Match extensions or storage shape.

## Outputs

- One ticket in exactly one state: `Queued`, `Selecting`, `Allocating`, `Assigned`, `Admitted`, `Active`, `Cancelled`, `Expired`, `Failed`, or `Ended`.
- One immutable assignment containing ticket, player, queue profile, selection epoch, server allocation, immutable roots, transport endpoints, admission credential identity and expiry, assignment time, and reconnect deadline when declared.
- One admission decision and one matchmaking-session record. Matchmaking session state tracks roster admission and attachment only; it contains no gameplay state, objective state, score, tick, or authoritative snapshot.
- Ordered lifecycle observations and stable terminal outcomes for cancellation, expiry, incompatible input, capacity, allocation failure, admission failure, disconnect, server loss, and normal end.

## Invariants

- One player has at most one nonterminal ticket. An idempotent repeated create with identical bytes returns that ticket; identity reuse with different bytes is a conflict.
- Tickets enter selection only from `Queued`. One selection epoch sees one immutable ordered ticket snapshot; a ticket can occur in at most one proposed match and one allocation attempt at a time.
- Queue profile compatibility is exact. Missing, changed, or mismatched game, ruleset, root, build, networking, region, capacity, or transport identity never falls back to another queue.
- Cancellation wins until assignment commit. Assignment commit wins a simultaneous later cancellation and returns the assignment; a client can then decline admission through the declared admission operation. The race is serialized by ticket revision.
- A ticket becomes `Assigned` only after one healthy server allocation is durably reserved. Allocation failure returns every still-valid ticket to `Queued` or moves it to `Failed` according to the queue profile's exact retry disposition.
- Admission credentials are audience-, assignment-, player-, server-, and expiry-bound. Credential verification and authorization cannot create a second assignment or gameplay authority.
- `Active` means the game-server service confirmed attachment of the admitted roster to one gameplay process. Matchmaking never starts, advances, pauses, restores, or terminates gameplay directly.
- Terminal states are immutable. Retention expiry removes a terminal record only under the accepted data-retention policy and cannot resurrect a ticket or assignment.

## Ownership Exclusions

- Games and rulesets own gameplay compatibility facts, teams, rosters after admission, mode lifecycle, scoring, and completion. Matchmaking owns pre-session queue, selection, assignment, and admission policy only.
- Networking owns transport-neutral protocol, replicated state, command/snapshot codecs, acknowledgement, and reconciliation. Matchmaking owns credential and endpoint delivery, not packet semantics.
- The game-server service owns process-profile availability, atomic allocation, process startup, health, drain, termination, transport binding, and session attachment. Matchmaking requests allocation and associates the returned reservation with tickets.
- Web applications own queue UI, user cancellation intent, status display, reconnect UI, and client transport lifecycle.
- Infrastructure owns databases, queues, compute, routing, regions, secret storage, and scaling resources. Matchmaking owns state-machine and ordering semantics over configured adapters.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every selectable online composition appears once in the generated queue-profile inventory with exact compatibility fields, ordering, timeouts, retry policy, and bounds. | The online session, ruleset, transport, region, and profile universes are not accepted; item count is 0. | **Queue-profile regeneration:** join accepted product sessions, game/ruleset compositions, Networking profiles, server profiles, roots, and regions; compare stable identities and require one complete profile per selectable composition. | Blocked |
| Every client and service-to-service operation appears once in the generated endpoint inventory with concrete transport, schema, auth class, timeout, and owner. | No online client transport, matchmaking origin, or peer endpoint is selected; item count is 0. | **Endpoint regeneration and conformance:** generate from accepted browser, game-server, identity, and infrastructure contracts; compare exact operations and execute one wire vector per item. | Blocked |
| Player authentication produces one stable principal accepted by the selected queue profile; wrong issuer, audience, build, ban/admission state, or expired credential queues nothing. | No player identity or authentication provider is declared. | **Authentication/admission table:** cross credential and profile states; compare principal, denial, ticket-store calls, challenge, redaction, and zero queued state. | Blocked |
| Ticket creation validates one solo-player request and idempotency identity atomically, returning the existing identical ticket or a distinct conflict without duplicate queue membership. | No queue profile, endpoint, ticket state, or store exists. | **Create/idempotency schedules:** submit identical, conflicting, concurrent, malformed, incompatible, and over-capacity requests; compare ticket identity/revision, queue state, response, and observations. | Blocked |
| Queue insertion, selection eligibility, and stable ordering follow profile priority, enqueue time, and ticket identity, independent of worker, storage, or arrival batching order. | No queue or selection ordering implementation exists. | **Permutation model comparison:** permute calls, transactions, workers, storage iteration, and selection cadence for one ticket set; compare eligible order and complete state after each epoch. | Not started |
| One selection epoch snapshots eligible tickets, applies only the queue profile's declared predicates, and proposes disjoint bounded rosters without mutating gameplay or hidden ratings. | No selection implementation or accepted profile predicates exist. | **Selection differential:** fixed ticket universes cover every predicate boundary, tie, insufficient roster, and overlapping candidate; compare proposals with a small independent model and owner-call trace. | Blocked |
| Concurrent epochs, retries, and service replicas cannot select, allocate, assign, or admit one ticket twice; failed proposals release every uncommitted claim. | No durable ticket revisions, claims, or transactions exist. | **Adversarial transaction schedules:** interleave selection, lease expiry, replica loss, allocation, assignment, and retry; compare one winner, released claims, revisions, and terminal state exactly. | Not started |
| Cancellation from `Queued` or `Selecting` removes eligibility atomically, while the declared revision race chooses exactly cancellation or committed assignment from `Allocating`. | No cancellation endpoint or ticket state exists. | **Cancellation race model:** cancel before, during, and after every selection/allocation commit point, including duplicates and timeouts; compare one terminal/assigned result, allocation cleanup, and observations. | Blocked |
| Ticket lifetime, allocation deadline, admission deadline, and reconnect deadline use monotonic time and produce exactly `Expired`, requeued, `Failed`, or `Ended` according to the profile. | No accepted timeout values or timeout state machine exists. | **Virtual-clock lifecycle:** advance every threshold at minus one, equal, and plus one while racing success, cancellation, disconnect, and shutdown; compare transitions and resource release. | Blocked |
| A selected roster requests exactly one compatible game-server allocation and passes only the accepted process profile, immutable roots, roster capacity, and selection identity. | No accepted game-server allocation interface or process profile exists. | **Allocation seam integration:** compare selected roster/profile bytes with allocator calls; inject capacity, timeout, conflict, cancellation, and wrong-profile results and compare ticket recovery exactly. | Blocked |
| Assignment commits atomically to every roster ticket only after reservation succeeds; partial ticket update, duplicate assignment, or hidden endpoint substitution is impossible. | No assignment transaction or server reservation exists. | **Multi-ticket commit fault injection:** fail before and after each durable write and race another assignment; compare all-or-none ticket states, reservation cleanup, endpoint bytes, and retries. | Blocked |
| One short-lived admission credential is issued per assigned player and binds the exact assignment, server, profile, roots, audience, and expiry without exposing signing material. | No credential authority, schema, or server verifier exists. | **Credential claim matrix:** independently verify valid and one-claim-mutated credentials, replay, rotation, expiry, wrong server, and cancellation; compare acceptance and redacted observations. | Blocked |
| Admission accepts each assigned player at most once within capacity and deadline, rejects nonmembers and stale credentials, and cannot alter team, class, loadout, or gameplay state. | No admission endpoint or accepted game-server attachment interface exists. | **Roster admission schedules:** permute joins, duplicates, expiry, cancellation, server drain, capacity, and malicious claims; compare admitted roster, denials, server calls, and absence of gameplay mutations. | Blocked |
| A matchmaking session advances `Assigned -> Admitted -> Active -> Ended` only from complete server attachment facts; failure and cancellation transitions release all owned credentials and records. | No session state or attachment callback exists. | **Session lifecycle model:** execute every legal edge and single illegal edge with partial admission, disconnect, server failure, and normal end; compare state, cleanup, callbacks, and terminal record. | Blocked |
| Reconnect is available only when an accepted queue profile declares a positive reconnect deadline and the same player, assignment, server, roots, and live session remain valid. | No product declares reconnect policy or credential behavior. | **Reconnect decision table:** cross deadline, identity, server/session health, replaced assignment, drain, and repeated attempts; compare refreshed credential, denial, roster state, and no new match. | Blocked |
| Ticket, queue, selection, allocation, admission, session, observation, concurrency, byte, and memory limits apply before excess state, with explicit delay or rejection backpressure. | No matchmaking limits or backpressure implementation exists. | **Capacity and slow-consumer schedules:** exercise every accepted limit at minus one, equal, and plus one; compare queue order, 429/503 or transport result, retry metadata, state, and memory. | Not started |
| Every lifecycle and peer failure returns one stable structured outcome and emits bounded logs, metrics, and traces without player, ticket, assignment, or credential identities as metric labels. | No matchmaking errors or observability schema exists. | **Failure/observation golden vectors:** inject every declared error under high identity cardinality; compare public outcome, ordered records, metric series, trace links, redaction, and sampling invariance. | Not started |
| Drain stops ticket creation and new selection, resolves or cancels in-flight allocations by declared ownership, preserves committed assignments, and integrates one current contract with clients and game servers. | No matchmaking process, endpoints, clients, or game-server integration exists. | **Drain and current-interface audit:** drain at every lifecycle state, compare ownership cleanup and client results, then repository-audit duplicate queues, allocators, admission paths, fallbacks, and compatibility branches. | Blocked |

## Generated Inventories

No generated Matchmaking inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Generated items | Accepted items |
|---|---|---|---|---:|---:|---:|
| `apps/services/matchmaking/inventories/queue-profiles.md` | Accepted online product sessions, game/ruleset compositions, Networking profiles, game-server process profiles, immutable roots, regions, and current Owner Registry | TF2 online-multiplayer target Future; required denominators Missing | Missing | 0 | 0 | 0 |
| `apps/services/matchmaking/inventories/endpoints.md` | Accepted browser/client, identity, game-server, administration, and infrastructure transport contracts | Client and peer contracts Missing | Missing | 0 | 0 | 0 |

The future generator is owned by `tools/playsrc`. It must retain every discovered profile and endpoint with sole owner and coverage classification; fail on an undeclared party field, duplicate route/profile, missing compatibility identity, missing bound, unknown owner, or count change; and emit both outputs atomically. No command name is declared before implementation exists.

## Exit Criteria

The Matchmaking service is Complete only when all of these predicates pass after Future activation:

- All 18 behavior rows are Ready.
- Both inventories are current, non-empty, generated, Accepted, and record exact item counts and review metadata.
- Every accepted queue profile passes authentication, creation, ordering, selection, claim, cancellation, timeout, allocation, assignment, admission, session, reconnect, saturation, and drain evidence.
- Browser clients, identity verification, game-server allocation/attachment, Networking admission, games, rulesets, tools, and infrastructure use one current interface without duplicate authority.
- Parent process, transport-security, structured-error, observability, and deployment-binding conformance passes in every declared hosted environment.
- No party behavior exists unless a later accepted denominator defines it completely.
- No required item, owner, profile, endpoint, input, dependency, decision, evidence method, or behavior remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Future session universe:** the TF2 online-multiplayer target is Future and has no accepted session types, ruleset selections, player counts, regions, roots, transports, reconnect policy, queue policy, or bounds. Checked the root roadmap, TF2 application README, TF2 ruleset-universe roadmap, Networking roadmap, and Matchmaking README.
- **Player identity and authorization:** no account/identity owner, credential issuer, audience, ban/admission authority, signing-key binding, or browser authentication flow is registered. Checked the root Owner Registry, application and service READMEs, and Infrastructure README.
- **Game-server interface:** the game-server child has no accepted allocation, reservation, credential-verification, attachment, health, endpoint, or cleanup contract. The required process-profile inventory has 0 accepted items.
- **Concrete endpoint and origin:** no browser matchmaking transport, service-to-service transport, public/internal origin, TLS binding, data store, queue, region, or evidence environment is accepted. The Draft Infrastructure roadmap correctly keeps matchmaking resources outside its Active inventory. Checked `ARCHITECTURE.md`, the Networking and Infrastructure roadmaps, and all service READMEs.
