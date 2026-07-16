# Game-Server Service Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines denominator and inventory requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines gameplay authority, simulation tick, Object, root manifest, Application, current contract, delivery status, and Complete. The parent [`../ROADMAP.md`](../ROADMAP.md) defines common process behavior.

## Completion Denominator

This Future leaf roadmap contains exactly 19 behavior rows. Planned generated inventories for hosted process profiles and management/gameplay transports each contain 0 candidate, generated, and accepted items and contribute 0 denominator items. The current denominator is 19 rows and is Not accepted.

The target uses one fresh gameplay process for one assigned session and terminates that process after the session. Process reuse, multiple concurrent sessions per process, root hot-swap, map hot-swap, and ruleset hot-swap are outside the current contract.

## Inputs

- One accepted parent service configuration and a provisioned compute/process adapter supporting atomic allocation claims, exact executable launch, isolated working state, listener binding, monotonic health observation, graceful signal, forced termination, exit status, cancellation, and resource measurements.
- One generated immutable `ServerProcessProfile` naming game, game-owned ruleset and policy, native Simulation executable object descriptor, game source root, map source, optional derived map root, optional ruleset artifact descriptors, Networking protocol/schema identity, concrete gameplay transports and ports, tick interval, player capacity, environment identity, CPU/memory/file-descriptor/network limits, startup deadline, health cadence/failure threshold, drain deadline, termination deadline, and evidence identity.
- One allocation request containing stable request and selection identities, exact process-profile identity, immutable root descriptors, requested capacity, and cancellation. Repeated identical requests are idempotent; conflicting identity reuse is rejected.
- One matchmaking-owned assignment and roster-admission interface, plus one gameplay-process control interface exposing configuration acceptance, listener readiness, health heartbeat, session attachment, session end, and terminal result.
- Accepted current Asset Store, Simulation, Networking, game, and ruleset interfaces. The service passes identities and capabilities to the process but cannot parse assets, advance simulation, decode gameplay packets, or change rules.
- A monotonic clock and finite limits for pending allocations, starting/ready/allocated processes, queued observations, process output, restarts, health failures, drain work, retained terminal records, and aggregate compute.
- Agones 1.59.0 public GameServer lifecycle, health, and atomic allocation documentation revised 2026-07-08 as architecture comparison input; playsrc does not inherit its Kubernetes resource schema or provider state.

## Outputs

- One server instance in exactly one state: `Starting`, `Ready`, `Allocated`, `Active`, `Draining`, `Terminating`, `Terminated`, or `Failed`.
- One immutable allocation record containing request, process profile, instance, process, executable, roots, transport endpoints, resource limits, allocation revision, health identity, and timestamps.
- One scoped process capability that can report health and session lifecycle but cannot allocate itself, change roots, or create another gameplay authority.
- One session attachment record joining exactly one matchmaking assignment and roster to the allocated process.
- Ordered lifecycle, resource, transport, health, exit, drain, and failure observations plus one bounded terminal report.

## Invariants

- The only normal lifecycle is `Starting -> Ready -> Allocated -> Active -> Draining -> Terminating -> Terminated`. Startup or runtime faults enter `Failed`, then cleanup performs `Terminating -> Terminated` without returning the instance to Ready.
- Allocation atomically claims one `Ready` instance for one request. A process is never observable as allocated to two requests, and a failed claim changes neither process nor request state.
- Executable, game, ruleset, map, roots, networking profile, ports, limits, and environment are immutable from launch through termination. Every supplied descriptor is validated and retained before process creation.
- `Ready` requires successful process configuration, root validation, listener binding, current-interface identity checks, and a fresh health heartbeat. A port-open process without these facts is not Ready.
- One allocated process accepts exactly one assignment attachment. The attached process contains the sole gameplay authority for that session; service state, health probes, transports, and matchmaking cannot advance gameplay.
- Drain withdraws Ready/assignment eligibility before signalling the process. Existing active gameplay receives the configured graceful deadline; no new player admission is accepted after drain begins.
- A process that exits, misses its accepted health threshold, changes an immutable identity, exceeds a hard resource limit, or loses required transport enters `Failed` once and is never restarted under the same instance identity.
- Termination is idempotent. Graceful completion precedes force; after the force deadline no process, listener, capability, session attachment, temporary state, or allocation claim remains.

## Ownership Exclusions

- Simulation owns gameplay advancement, tick ordering, commands, snapshots, and one authority. Games and rulesets own gameplay and mode behavior.
- Networking owns transport-neutral protocol, codecs, replication, acknowledgement, rates, and reconciliation. This service owns concrete native listeners, carrier adapters, endpoint publication, and process-side connection lifetime.
- Matchmaking owns queue selection, player credentials, assignment, and admission policy. This service owns atomic process reservation and verifies a supplied assignment before one attachment.
- Asset Store owns roots, bytes, integrity, retention, and publication. This service selects only descriptors already named by an accepted process profile and never mutates a channel or root.
- Infrastructure owns compute pools, schedulers, network resources, ports, firewalls, DNS, certificates, autoscaling resources, and secret delivery. This service owns allocation and process behavior over configured adapters.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every hosted game/ruleset/root/network composition appears once in the generated process-profile inventory with exact executable, transport, limits, health, drain, and evidence fields. | Required online compositions and package interfaces are not accepted; item count is 0. | **Process-profile regeneration:** join accepted products, games, rulesets, roots, native executables, Networking profiles, transports, environments, and Owner Registry; compare stable identities and complete fields. | Blocked |
| Every management and gameplay transport appears once in the generated transport inventory with concrete carrier, address class, protocol owner, auth class, limits, timeout, and endpoint consumer. | No management API, native gameplay carrier, or hosted network binding is selected; item count is 0. | **Transport regeneration and conformance:** generate from accepted Matchmaking, Networking, process, and infrastructure contracts and execute one connection vector per item. | Blocked |
| Allocation validates request identity, exact process profile, immutable roots, requested capacity, and cancellation before claiming compute; identical retries return one result and conflicting retries change nothing. | No allocator, profile, or request schema exists. | **Allocation validation/idempotency matrix:** mutate every field and race identical/conflicting retries; compare adapter calls, claim, result identity, state, and observations. | Blocked |
| Concurrent allocations atomically reserve at most one compatible Ready instance per request and one request per instance, with deterministic eligible-instance ordering. | No Ready pool, atomic claim, or ordering policy exists. | **Multi-replica allocation schedules:** permute allocators, storage transactions, candidate order, cancellation, and capacity; compare winners and complete instance/request states with an independent model. | Not started |
| Startup creates one isolated process from the exact executable descriptor, immutable configuration, resource limits, and process capability, with no ambient path or undeclared environment input. | No native Simulation executable contract, compute adapter, or process launcher exists. | **Launch seam record:** use fixed profiles and instrument argv-free typed configuration, environment, mounts, descriptors, handles, limits, and process tree; compare exact calls and reject ambient inputs. | Blocked |
| Root selection verifies and retains the exact game, map, and required artifact closure before launch, then passes immutable descriptors without channel following or hot-swap. | The Asset Store root and retention interfaces are not accepted. | **Root-binding integration:** race channel changes and mutate every descriptor/closure node; compare selected identities, retention calls, launch calls, and zero channel or fallback lookup after allocation. | Blocked |
| The child process accepts exactly the profile's current Simulation, game, ruleset, and Networking identities before exposing any gameplay listener. | Required package/game/ruleset interfaces and executable handshake are not accepted. | **Interface-handshake table:** cross matching and one-field-mismatched identities; compare listener visibility, process state, error, cleanup, and absence of compatibility negotiation. | Blocked |
| A server becomes Ready only after root validation, process handshake, all declared listener bindings, resource self-check, and a fresh health heartbeat complete within the startup deadline. | No readiness handshake or accepted startup deadline exists. | **Readiness phase trace:** fail and delay each prerequisite independently; compare state, allocation eligibility, endpoints, cleanup, and exact terminal reason. | Blocked |
| Health heartbeats are monotonic, identity-bound, and required at the profile cadence; threshold failure, process exit, identity change, or hard resource violation enters Failed exactly once. | No process health protocol, thresholds, or resource monitor exists. | **Virtual-clock health schedules:** vary heartbeat timing, duplicates, stale identity, pauses, exits, CPU/memory/descriptor/network limits, and recovery; compare state and one failure event. | Blocked |
| Successful allocation publishes only the exact healthy instance endpoints and allocation revision after the Ready-to-Allocated claim is durable. | No allocation record, transport binding, or Matchmaking integration exists. | **Publication atomicity injection:** fail before and after each claim/record/response boundary and race readers; compare no endpoint before commit and one immutable record after success. | Blocked |
| Session attachment verifies one matchmaking assignment, roster, server audience, roots, profile, expiry, and allocation revision and commits at most once. | No accepted Matchmaking assignment or attachment interface exists. | **Attachment claim matrix:** valid and one-field-mutated assignments plus duplicate, expiry, drain, wrong instance, and concurrent attach compare one record or exact denial and zero gameplay mutation. | Blocked |
| Concrete gameplay transports carry complete Networking-owned payloads and lifecycle signals without decoding, reordering, coalescing, or altering protocol semantics. | No accepted Networking interface or concrete hosted transport exists. | **Carrier adapter conformance:** run identical payload schedules through every accepted carrier; compare bytes, boundaries, timestamps, close/cancellation results, backpressure, and Networking call trace. | Blocked |
| Active state begins only after the gameplay process confirms one attached authoritative session; service operations cannot submit commands, advance ticks, read mutable internals, or publish snapshots independently. | No hosted gameplay process or authority capability exists. | **Authority-capability audit:** execute a fixed session through process controls, compare Simulation authority identity, and repository-audit service APIs for tick, command, snapshot-mutation, game, or ruleset behavior. | Blocked |
| Drain atomically withdraws allocation and admission eligibility, notifies Matchmaking, and gives the active process exactly the configured graceful session deadline. | No drain state or peer notifications exist. | **Drain race schedules:** drain in every lifecycle state while allocation, attachment, admission, disconnect, normal end, and repeated drain race; compare eligibility, notifications, process signal, and state. | Blocked |
| Normal session completion records the terminal gameplay-process fact, closes admission and transports, then proceeds through graceful termination without reusing the process. | No session-end control interface or process termination exists. | **Normal-end lifecycle trace:** fixed empty and populated sessions compare close order, final Networking/Simulation facts, observations, retained report, handles, and no Ready transition. | Blocked |
| Termination sends one graceful request, waits the profile deadline, escalates once to force when required, reaps the complete process tree, and is idempotent from every state. | No process termination implementation or adapter exists. | **Termination fault matrix:** terminate from each state with responsive, stuck, child-leaking, signal-failing, and already-exited processes; compare calls, deadlines, exit result, resources, and repeated outcome. | Blocked |
| Crash, health failure, resource exhaustion, listener loss, and adapter failure publish one Failed result, revoke endpoints and capabilities, release attachments/claims, and never restart the same identity. | No failure containment or cleanup implementation exists. | **Failure injection at every process boundary:** compare one failure transition, endpoint withdrawal, peer notifications, process tree, root retention release, claims, and bounded terminal report. | Blocked |
| Pending allocations, processes by state, connections, bytes, queues, observations, process output, CPU, memory, descriptors, network, deadlines, and aggregate compute enforce accepted limits and backpressure. | Exact hosted-process and environment limits are not accepted. | **Capacity/resource matrix:** exercise every limit at minus one, equal, and plus one with mixed lifecycle states; compare admission, process results, memory/compute measurements, ordering, and cleanup. | Blocked |
| Lifecycle errors, logs, metrics, and traces are bounded and correlated across allocation, process, session, and Matchmaking identities; every consumer uses one current contract with no duplicate launcher, allocator, authority, fallback, or compatibility path. | No implementation, observability schema, deployment, or consumer integration exists. | **End-to-end hosted-session and repository audit:** run fixed allocate-to-terminate sessions with faults and high identity cardinality; compare observations/redaction and search all producers/consumers for duplicate or legacy paths. | Blocked |

## Generated Inventories

No generated Game-Server inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Generated items | Accepted items |
|---|---|---|---|---:|---:|---:|
| `apps/services/game-servers/inventories/process-profiles.md` | Accepted hosted product sessions, game/ruleset compositions, native Simulation executables, Asset Store roots, Networking profiles, environments, and Owner Registry | TF2 online-multiplayer target Future; required denominators Missing | Missing | 0 | 0 | 0 |
| `apps/services/game-servers/inventories/transports.md` | Accepted management peers, Networking carrier requirements, process control interface, and infrastructure bindings | Matchmaking, Networking, process, and infrastructure contracts Missing | Missing | 0 | 0 | 0 |

The future generator is owned by `tools/playsrc`. It must retain every discovered profile and transport with stable identity, sole owner, exact bounds, and coverage classification; fail on mutable roots, process reuse, duplicate carrier, unknown owner, missing authority identity, unbounded resource, or count change; and emit both outputs atomically. No command name is declared before implementation exists.

## Exit Criteria

The Game-Server service is Complete only when all of these predicates pass after Future activation:

- All 19 behavior rows are Ready.
- Both inventories are current, non-empty, generated, Accepted, and record exact item counts and review metadata.
- Every process profile passes allocation, startup, root binding, current-interface handshake, readiness, health, attachment, transport, authority, drain, termination, crash, saturation, and observation evidence.
- Matchmaking, Asset Store, Simulation, Networking, games, rulesets, tools, and infrastructure use one current interface and one gameplay authority per attached session.
- Every process uses immutable roots and one session, is never reused or hot-swapped, and leaves no process, listener, capability, attachment, claim, or temporary state after termination.
- Parent process, transport-security, authentication, authorization, structured-error, observability, and deployment-binding conformance passes in every hosted environment.
- No required item, owner, profile, transport, input, dependency, decision, evidence method, or behavior remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Hosted process profiles:** the TF2 online-multiplayer target is Future and no accepted session, game/ruleset composition, native Simulation executable, immutable root set, Networking profile, player capacity, or resource limit can populate a process profile. Checked the root roadmap, TF2 and ruleset roadmaps, Simulation and Networking roadmaps, Asset Store roadmap, and Game-Server README.
- **Allocation and attachment peers:** the Matchmaking roadmap is Draft and its implementation is absent, so no accepted assignment, credential, reservation, attachment, cancellation, or cleanup interface is available.
- **Concrete transports:** Networking owns a Draft transport-neutral profile, but no native gameplay carrier, management API, port set, endpoint publication contract, TLS boundary, or process-control transport is accepted.
- **Compute and environment adapter:** the Draft Infrastructure roadmap keeps game-server resources outside its Active inventory and defines no accepted compute pools, schedulers, isolation, port allocation, firewalls, process credentials, resource monitors, autoscaling, telemetry sinks, or isolated hosted-session evidence environment.
- **Health and termination bounds:** startup deadline, heartbeat cadence, health failure threshold, graceful drain, force deadline, retained output, process-tree, and aggregate compute bounds have no accepted product or environment profile.
