# Service Applications Roadmap

[`../../docs/roadmap-contract.md`](../../docs/roadmap-contract.md) defines aggregation, denominator, and evidence requirements. [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) defines Application, current contract, delivery status, bounds, backpressure, and Complete.

## Completion Denominator

This aggregation roadmap contains exactly 18 cross-child conformance rows. Its finite child universe is API, asset, matchmaking, and game-server services. API and asset services are Active; matchmaking and game-server services remain Future until the TF2 online-multiplayer root target is activated.

The denominator is Not accepted. No child has an accepted denominator, implementation, deployment binding, evidence record, or denominator review. Aggregate status is Blocked because the Active API and asset children have unavailable required inputs recorded in their roadmaps.

## Inputs

- One immutable service configuration containing exactly `service`, `environment`, `listener`, `publicOrigin`, `transportSecurity`, `trustedProxy`, `allowedOrigins`, `limits`, `timeouts`, `authentication`, `dependencies`, `telemetry`, and `deployment`. A child may add only fields declared by its roadmap. No field has a default in a deployed environment.
- `listener` contains network, address, port, and supported protocol set. `transportSecurity` is exactly `process-tls` or `trusted-edge-tls`; `trustedProxy` is required only for the latter and names the trusted network boundary and forwarded fields.
- `limits` contains positive finite maxima for connections, concurrent requests or sessions, queued work, header bytes, request bytes, response bytes when known before streaming, observation events, and in-flight memory. Each child declares additional domain limits.
- `timeouts` contains positive finite durations for header receipt, request receipt, handler execution, downstream calls, idle connections, keepalive, cancellation propagation, drain, and forced termination.
- One route manifest per child naming every method or frame type, path or transport identity, request and response schema, authentication class, authorization policy, cache policy, limits, timeout, backing owner, and observable error set.
- Provisioned listener, TLS, secret, dependency, telemetry, and routing identities supplied by Infrastructure; secret values are delivered at runtime and never enter configuration reports, logs, metrics, traces, or errors.
- RFC 9110 HTTP Semantics, RFC 9111 HTTP Caching, RFC 9457 Problem Details for HTTP APIs, RFC 8446 TLS 1.3, and the W3C Trace Context Recommendation dated 2021-11-23.
- The four child roadmaps and every accepted package, game, ruleset, application, tool, and infrastructure interface they name.

## Outputs

- One child process in exactly one state: `Configuring`, `Starting`, `Ready`, `Draining`, `Stopped`, or `Failed`.
- A bound set of child-owned network listeners and exact advertised public origins only after configuration and dependency validation succeeds.
- `GET /healthz` process-liveness and `GET /readyz` traffic-readiness responses on every HTTP listener. Both are unauthenticated, bounded, non-cacheable, and expose no dependency credentials or internal diagnostics.
- Complete responses or terminal transport outcomes for admitted work; a rejected or cancelled request publishes no partial domain mutation.
- RFC 9457 `application/problem+json` responses containing stable `type`, `title`, `status`, `detail`, `instance`, and `requestId` fields whenever HTTP can still carry a response.
- Structured logs, bounded metric series, and W3C-correlated traces keyed by service, environment, deployment, route, outcome, status, latency, and request or session identity.
- One redacted startup report, readiness transition stream, drain report, and terminal process result.

## Invariants

- Configuration is parsed and validated before dependency initialization or listener binding. Unknown fields, duplicate bindings, absent values, malformed origins, non-positive limits, inconsistent timeouts, unresolved secrets, and unprovisioned dependencies fail startup.
- The only successful lifecycle is `Configuring -> Starting -> Ready -> Draining -> Stopped`. Any pre-drain failure enters `Failed`; shutdown from `Configuring`, `Starting`, or `Failed` releases acquired resources and ends in `Stopped`.
- `/healthz` returns 200 only while the process event loop can accept probe work. `/readyz` returns 200 only in `Ready`; it returns 503 in every other reachable listener state. Readiness becomes false before drain stops admission.
- Externally advertised HTTP and bidirectional transports use TLS. In `trusted-edge-tls`, only the configured proxy boundary can supply client address, scheme, host, or credential-forwarding metadata; direct or untrusted forwarded fields are rejected.
- Every unit of work is charged before allocation against all applicable connection, byte, queue, concurrency, elapsed-time, and memory limits. A successful result is never truncated to satisfy a bound.
- Backpressure has one declared result per queue: delay, reject, or close. Services never silently discard mutating requests, assignments, admissions, lifecycle events, structured errors, or terminal observations.
- Cancellation and deadlines propagate to every downstream call. Completion after cancellation cannot publish a response or state transition that was not durably committed before cancellation won the declared race.
- Every route is exactly one of `public`, `service`, `player`, or `administrator`. Authentication validates the declared credential and its audience; authorization evaluates the authenticated principal against that route before calling a backing owner.
- Public routes do not change representation by credential presence. Credentials, session secrets, admission tokens, raw authorization headers, and unredacted personal data never enter URIs or observability output.
- Applications use one versionless current contract. A breaking route, schema, or transport change updates every producer and consumer in one checkpoint; no `/v1` branch, compatibility route, fallback handler, or legacy reader remains.

## Ownership Exclusions

- Each child service owns its process, route manifest, concrete transports, request validation, authentication integration, authorization enforcement, response assembly, and observations. This aggregation owns only cross-child conformance and status derivation.
- Packages own reusable data, storage, simulation, networking, and presentation behavior. A service calls those interfaces and cannot reinterpret or repair their results.
- Games and game-owned rulesets own gameplay and mode behavior. A hosted process cannot acquire a second gameplay authority.
- Web applications own product routing, browser state, catalog contents, user-visible errors, and reconnect UI.
- `tools/playsrc` owns repeated local and release command orchestration. Services own the long-running processes started by those commands.
- Infrastructure owns compute, storage, CDN, DNS, certificates, network policy, secret delivery, telemetry sinks, and environment provisioning. Services own application-specific binding validation and behavior after binding.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every child rejects an incomplete, unknown, duplicate, inconsistent, or unbound configuration before acquiring a listener and emits one redacted failure. | No service configuration schema or validator exists. | **Configuration rejection matrix:** mutate every common and child field at missing, valid, boundary, and invalid values; compare listener calls, acquired resources, redacted report, and exact failure. | Not started |
| Startup initializes telemetry, credentials, backing dependencies, route tables, and listeners in declared order and becomes Ready only after every required self-check passes. | No service startup implementation exists. | **Startup phase trace:** inject failure and cancellation before and after every phase; compare calls, reverse cleanup, listener visibility, state, and readiness exactly. | Not started |
| `GET /healthz` reports process liveness without dependency probing, domain work, authentication, caching, or sensitive detail. | No health endpoint exists. | **Liveness endpoint vectors:** query every process state and inject dependency failure, event-loop stall, overload, and shutdown; compare status, body, headers, latency bound, and zero downstream calls. | Not started |
| `GET /readyz` reports 200 only while new work can be admitted and changes to 503 before startup failure, dependency loss requiring withdrawal, or drain. | No readiness endpoint or readiness state exists. | **Readiness transition trace:** exercise every lifecycle edge and dependency-health policy; compare transition order, status/body, admission gate, and retained observations. | Not started |
| Shutdown withdraws readiness, stops new admission, cancels queued work, lets admitted work finish within `drainTimeout`, then closes transports and forces remaining work before `terminationTimeout`. | No coordinated service shutdown exists. | **Drain schedule:** use idle, queued, active, streaming, stuck, cancelled, and repeated-signal workloads; compare admission results, completion order, forced closures, resource counts, and terminal report. | Not started |
| Each public listener proves either process-owned TLS or one exact trusted TLS edge; cleartext external traffic and untrusted forwarding metadata are rejected. | No service listener or trust-boundary implementation exists. | **Transport-security conformance:** direct TLS, trusted edge, spoofed forwarding, wrong host, certificate rotation, protocol downgrade, and cleartext probes compare accepted authority and rejection exactly. | Not started |
| Routing accepts only a manifest-declared method or frame type, normalized target, media type, origin policy, and complete request shape; no catch-all handler reaches domain code. | No service route manifests or dispatchers exist. | **Route-manifest differential:** generate accepted and one-field-invalid requests for every route; compare selected identity, decoded input, owner calls, 404/405/406/415 outcomes, and absence of fallback dispatch. | Not started |
| Every connection, request, response, stream, header, body, queue, observation, and memory allocation is rejected at its configured maximum plus one before over-bound work. | No common service limit accounting exists. | **Boundary and allocation matrix:** test every configured maximum at minus one, equal, and plus one with allocation instrumentation; compare result, prior state, bytes, and elapsed bound. | Not started |
| Child queues and workers enforce declared concurrency and backpressure while preserving serial-equivalent state and exact admitted-work order. | No service queue, worker, or backpressure implementation exists. | **Deterministic saturation schedules:** permute producers and worker completion at empty, full, and overfull states; compare admissions, queue order, state, responses, and memory use. | Not started |
| Header, request, handler, downstream, idle, keepalive, drain, and termination deadlines use a monotonic clock and produce their distinct declared outcomes. | No service timeout implementation exists. | **Virtual-clock deadline vectors:** advance each clock boundary independently and race completion, cancellation, peer close, and shutdown; compare winner, state, response, cleanup, and observation order. | Not started |
| Cancellation closes or releases all queued, downstream, streaming, and transport work without leaking a task or publishing a post-cancellation partial result. | No cross-service cancellation contract exists. | **Cancellation injection:** cancel at every asynchronous boundary for each child transport; compare committed state, response visibility, downstream cancellation, open handles, and retained buffers. | Not started |
| Authentication is applied only to routes declaring `service`, `player`, or `administrator`, validates credential type, issuer, audience, expiry, and binding, and never logs credential material. | No service authentication provider or route declaration exists. | **Credential matrix:** cross route class with missing, valid, expired, wrong-audience, replayed, malformed, and rotated credentials; compare principal, challenge, owner-call count, and redaction. | Not started |
| Authorization runs after authentication and before owner invocation, uses the route's exact action and resource identity, and returns a stable denial without revealing resource existence beyond policy. | No service authorization policy or enforcement exists. | **Authorization decision table:** cross principals, actions, resources, absent resources, tenant boundaries, and policy reloads; compare allow/deny, status, owner calls, and disclosed fields. | Not started |
| Every HTTP failure uses the declared status and RFC 9457 type; a failure after response commitment terminates the transport and records the same request identity instead of writing a second response. | No shared service error contract exists. | **Fault-to-wire matrix:** inject every validation, auth, capacity, timeout, dependency, integrity, and post-commit stream fault; compare status, exact problem bytes or terminal close, redaction, and one error observation. | Not started |
| Structured logs record one bounded start and one bounded completion event per admitted operation plus lifecycle transitions, with stable keys and no secret or unbounded payload. | No service logging schema or sink integration exists. | **Golden log comparison:** run fixed lifecycle and request schedules with malicious fields and credentials; compare ordered canonical records, truncation classifications, correlation, and redaction. | Not started |
| Metrics expose bounded-cardinality lifecycle, traffic, latency, byte, queue, rejection, timeout, cancellation, dependency, and failure measurements without user, object, ticket, session, or trace identities as labels. | No service metrics contract exists. | **Metric cardinality audit:** run high-identity workloads and compare series keys/count, counter deltas, histogram observations, lifecycle gauges, and enabled/disabled behavior. | Not started |
| Traces parse or restart W3C trace context at the trust boundary, create one server span and declared downstream spans, and never let sampling or instrumentation change service results. | No service tracing implementation exists. | **Trace-context vectors:** valid, absent, malformed, oversized, sampled, unsampled, trusted, and untrusted headers compare spans and propagated fields; disabled tracing must produce identical service output. | Not started |
| Every child exposes one versionless route contract and validates application-specific deployment bindings against provisioned identities before Ready; all producers and consumers use that same contract. | No child implementation, route contract, or accepted infrastructure binding exists. | **Current-contract deployment audit:** exercise each child in every accepted environment and repository-audit routes, schemas, bindings, consumers, fallbacks, compatibility branches, and stale deployment artifacts. | Not started |

## Generated Inventories

No generated service-group inventory is required. The four-child universe is hand-enumerated above and matches the root Owner Registry. Accepted generated inventory count: 0.

## Exit Criteria

The Service Applications aggregation is Complete only when all of these predicates pass:

- All 18 cross-child rows are Ready.
- API and asset services are Complete for the Active TF2-first target; matchmaking and game-server services are Complete after their Future target is activated.
- Every child route, transport, backing owner, authentication class, authorization policy, limit, timeout, deployment binding, and consumer appears in an accepted child denominator.
- Every accepted environment passes startup, health, readiness, saturation, cancellation, drain, transport-security, error, and observability evidence through provisioned bindings.
- Every producer and consumer uses the same versionless current contracts and no service reimplements package, game, ruleset, application, tool, or infrastructure behavior.
- No required child, owner, input, dependency, decision, evidence method, behavior, or integration remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Active product endpoint contract:** the Draft TF2 web roadmap names no shared product-data endpoint. The Draft Tempus web roadmap names 12 Tempus-specific upstream operations and assigns their deployed proxy transport to the API service, conflicting with the API owner's explicit exclusion of Tempus-specific semantics and single-product endpoints. Neither Draft is accepted. Checked both web roadmaps and READMEs, the web group roadmap, the root roadmap, and the API README.
- **Asset-store interface:** the Asset Store roadmap is Draft and its object, root, catalog, channel, descriptor, error, and numeric-bound contracts are not accepted. The asset child cannot complete its backing integration. Checked the Asset Store README, roadmap, object-kind candidate inventory, and asset-service README.
- **Online-multiplayer interfaces:** session types, selected TF2 rulesets, player identity, concrete browser/server transports, queue profiles, replicated-state interfaces, and hosted-process profiles are not accepted. Checked the root roadmap, TF2 and ruleset roadmaps, Simulation and Networking roadmaps, and both Future service READMEs.
- **Deployment bindings:** the Infrastructure roadmap is Draft and supplies no accepted service compute, trusted TLS edges, certificate identities, network policy, secret delivery, telemetry sinks, or matchmaking public/internal origin. Checked `ARCHITECTURE.md`, the Infrastructure README and roadmap, the root roadmap, and all service READMEs.
- **Adjacent roadmap synchronization:** the root roadmap still states that the API and asset denominators have not been written; the web group and both Active web children still state that API or asset roadmaps do not exist; and the Networking roadmap still states that this game-server roadmap does not exist. Those owners must replace the stale statements after reviewing these Drafts. Checked every named roadmap and the current Owner Registry.
