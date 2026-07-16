# API Service Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines denominator, inventory, and evidence requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines Catalog, Application, current contract, delivery status, and Complete. The parent [`../ROADMAP.md`](../ROADMAP.md) defines the common service-process contract.

## Completion Denominator

This leaf roadmap contains exactly 13 behavior rows. The current named product-data endpoint set is empty: no accepted Active application contract names data required from this service. Operational `/healthz` and `/readyz` routes belong to the parent conformance contract and are not product-data endpoints.

The planned generated endpoint inventory contains 0 candidate, generated, or accepted items and contributes 0 denominator items. The current denominator is 13 rows and is Not accepted. Endpoint enumeration, backing interfaces, and consumer integration are Blocked; no catch-all or speculative product endpoint is admitted in their place.

## Inputs

- The accepted parent service configuration and lifecycle interface.
- One generated endpoint manifest containing only product-neutral data required by at least two accepted applications. Each item names stable endpoint identity, exact method and path, path/query/header/body schemas, response schema and media type, cache policy, authentication class, authorization action, limits, timeout, backing owner/interface identity, consumers, and error set.
- Accepted application declarations proving each endpoint's consumers and the application owner of catalog entries, labels, ordering, and experience selection.
- Accepted backing package or application interfaces. The API service receives typed values and cannot parse Source formats, query unconfigured files, reinterpret gameplay, alter catalogs, or become storage authority.
- RFC 9110 request, response, conditional, method, and status semantics; RFC 9111 caching; and RFC 9457 problem details.
- The current asset-service boundary: raw immutable catalog bytes are served only by `apps/services/assets`; an API endpoint can expose a named product-neutral catalog query only when accepted consumers and a distinct backing interface require it.

## Outputs

- One complete typed response for the exact selected endpoint, or one parent-contract problem response or transport outcome.
- Canonical UTF-8 JSON for JSON endpoints, with exact media type, deterministic field presence and ordering contract, and no undeclared envelope fields.
- Strong validators and explicit cache controls only when the endpoint manifest identifies an immutable or revision-addressed representation.
- Structured endpoint observations keyed by stable endpoint identity and outcome, never by unbounded product-data values.

## Invariants

- The dispatch table is generated exclusively from the accepted endpoint inventory. Unknown paths return 404; a known path with a disallowed method returns 405 and its exact `Allow` value. Prefix, suffix, wildcard, proxy, GraphQL, arbitrary-query, and pass-through routes are absent.
- Every accepted endpoint has one backing behavior owner. Response assembly selects and serializes owner-returned fields but cannot add inferred domain facts, issue additional discovery queries, or repair owner errors.
- Route validation completes before authentication-dependent authorization and before the backing call. Duplicate scalar query parameters, unknown parameters, invalid UTF-8, invalid percent encoding, noncanonical identifiers, and undeclared body bytes are rejected.
- Product data remains versionless. A schema or endpoint change updates every accepted consumer and the generated inventory in one checkpoint.
- A product-specific endpoint required by only one application belongs to that application or a separately declared product service, not this shared API.

## Ownership Exclusions

- Web applications own catalogs, product-specific data, UI projections, route navigation, and user-visible recovery. The API owns only accepted shared network endpoints.
- The Asset Store owns immutable catalog envelopes and bytes; the asset service owns raw catalog HTTP reads. The API does not expose a second raw-catalog route.
- Tempus map, course, record, ranking, demo, account, and external-integration semantics belong to the Tempus application or a future explicitly registered Tempus service. They do not enter this shared API implicitly.
- Packages, games, and rulesets own domain calculations and state transitions. Infrastructure owns databases, queues, compute, DNS, and TLS resources.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every shared product-data endpoint required by Active applications appears exactly once in the generated endpoint inventory, and no single-product or ownerless endpoint appears. | No accepted web-application roadmap names a shared endpoint; the candidate and accepted item counts are 0. | **Consumer-to-endpoint regeneration:** join accepted application requirements to backing owner interfaces; compare stable endpoint identities and require at least two consumers, one owner, and one exact route per item. | Blocked |
| The generated route table dispatches only each inventoried method and exact normalized path, returning 404 or 405 without invoking another route or backing owner. | No endpoint inventory, route table, or API process exists. | **Exhaustive route differential:** generate every route plus wrong method, segment, slash, case, encoding, prefix, suffix, and unknown path; compare endpoint identity, `Allow`, status, and owner-call count. | Blocked |
| Each request validates all declared path, query, header, media-type, and body fields atomically and rejects duplicate, unknown, over-bound, malformed, or noncanonical input. | No API request schemas or validator exist. | **Schema mutation matrix:** mutate each field independently at valid, boundary, invalid, duplicate, absent, unknown, and over-bound values; compare typed input or exact problem response. | Not started |
| An admitted request invokes exactly one current backing-owner operation with the validated typed input and never performs domain discovery or a second implementation. | No endpoint has an accepted backing interface. | **Owner seam spy:** for every inventory item, compare API input with owner-call bytes, count, cancellation, returned result, and repository absence of duplicate domain logic. | Blocked |
| Response assembly emits only the endpoint's declared fields and canonical representation, preserving owner classifications and stable ordering without changing product truth. | No response schemas or backing results exist. | **Canonical response vectors:** permute owner result storage/order and compare exact response bytes, media type, field omissions, classification mapping, and repeated-run identity. | Blocked |
| Endpoint cache controls, strong ETags, preconditions, and 304 responses follow the inventory policy and never cache authenticated or mutable data as public. | No API cache policy implementation exists. | **Cache and validator matrix:** cross endpoint policies with authorization, `If-None-Match`, changed revisions, intermediaries, and stale entries; compare status, headers, body, and owner calls under RFC 9110/9111. | Not started |
| Authentication and authorization run exactly as declared per endpoint; public endpoints are representation-invariant to credentials and protected endpoints never expose data before both checks pass. | No endpoint authentication classes or backing authorization actions are declared. | **Endpoint access matrix:** cross every inventoried route with credential and policy outcomes; compare challenge, denial, cache headers, owner calls, response bytes, and redaction. | Blocked |
| Client cancellation and the configured request deadline cancel the backing operation and response serialization, with one declared winner when completion races cancellation. | No API operation or cancellation plumbing exists. | **Virtual-clock cancellation races:** cancel before dispatch, during owner work, during serialization, and after commitment; compare committed result, transport outcome, owner signal, cleanup, and observations. | Not started |
| API concurrency, queue, request-byte, response-byte, and in-flight-memory bounds reject excess work with stable backpressure and no result reordering. | No API limit accounting or queue exists. | **Saturation and resource schedule:** run fixed endpoint mixes at every configured boundary; compare admissions, order, 413/429/503 outcomes, `Retry-After` when declared, memory, and serial-equivalent results. | Not started |
| Every validation, authentication, authorization, capacity, timeout, cancellation, owner, and serialization failure maps to one stable RFC 9457 type without exposing internal data. | No API problem-type registry or error mapper exists. | **Error cross-product:** inject every declared failure for every endpoint; compare status, type, title, bounded detail, request identity, headers, and redaction exactly. | Not started |
| Browser requests use the configured exact origin allowlist, methods, and headers; preflight never grants credentials or a route absent from the manifest. | No API CORS policy exists. | **CORS conformance table:** cross allowed/disallowed/absent/null origins, simple and preflight methods, requested headers, credentials, and cache variation; compare exact response fields and dispatch count. | Not started |
| API logs, metrics, and traces use stable endpoint identities and bounded outcomes, correlate backing calls, and never label or record unbounded product data or credentials. | No API observability implementation exists. | **Golden observation audit:** execute fixed success/failure/saturation workloads with high-cardinality values; compare records, series count, trace linkage, sampling invariance, and redaction. | Not started |
| All accepted applications call the same versionless endpoint contracts, and every endpoint is backed by its current owner with no raw-catalog duplicate, proxy catch-all, fallback, or compatibility route. | Required application roadmaps, endpoint inventory, and backing interfaces are not accepted. | **Producer-consumer integration audit:** exercise every generated endpoint from every named consumer and repository-audit route registrations, schemas, catalog reads, proxies, fallbacks, and version branches. | Blocked |

## Generated Inventories

No generated API inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Generated items | Accepted items |
|---|---|---|---|---:|---:|---:|
| `apps/services/api/inventories/product-data-endpoints.md` | Accepted Active web-application service requirements joined to current backing-owner interfaces and the root Owner Registry | Application denominators Missing; backing-interface acceptance Missing | Missing | 0 | 0 | 0 |

The future generator is owned by `tools/playsrc`. It must retain each discovered requirement with consumers, owner, route, schemas, policy, limits, errors, and coverage classification; reject a requirement with fewer than two applications, no owner, duplicate route, catch-all shape, product-specific semantics, or raw asset-resource duplication; and fail on count or authority changes. No command name is declared before that operation exists.

## Exit Criteria

The API service is Complete only when all of these predicates pass:

- All 13 behavior rows are Ready.
- The endpoint inventory is current, non-empty when an Active application requires this service, generated by one checked-in command, and Accepted with exact item count and review metadata.
- Every item has at least two accepted application consumers, one backing owner, one exact versionless route, complete schemas, policies, limits, errors, and integration evidence.
- Parent lifecycle, security, bounds, backpressure, timeout, cancellation, structured-error, logging, metric, tracing, and deployment-binding conformance passes.
- No endpoint is a catch-all, arbitrary proxy/query surface, single-product behavior owner, raw asset duplicate, fallback, compatibility branch, or legacy route.
- No required item, owner, consumer, input, dependency, decision, evidence method, or behavior remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Named shared endpoint set:** the Draft TF2 roadmap declares no shared API request. The Draft Tempus roadmap declares 12 exact Tempus API v0 GET operations, but each has one product consumer and Tempus-specific semantics, so none satisfies this shared owner's endpoint rule. Neither child denominator is accepted. Checked both application roadmaps and READMEs, the web group roadmap, API README, root roadmap, Asset Store roadmap, and current Owner Registry. The current endpoint item count remains 0 rather than inventing a catch-all.
- **Catalog transport boundary:** the API README names product-neutral catalogs while the asset-service README names stored catalog delivery. The current boundary is raw immutable catalog bytes through the asset service and only a separately accepted multi-application catalog query through the API. No accepted consumer currently declares the latter.
- **Tempus proxy ownership:** the Draft Tempus roadmap assigns deployed Tempus API and DEM proxy transport to this service, while this owner's README excludes Tempus-specific semantics and this roadmap admits only shared multi-application product data. The web and service owners must assign the exact 12 API operations and DEM-download transport either to a separately registered Tempus service or to an amended owner contract before either denominator can be accepted.
- **Backing interfaces:** no accepted owner interface supplies a named shared product-data query to the API. The Asset Store, TF2, TF2 ruleset, and application roadmaps are Draft or absent, so endpoint schemas and errors cannot be accepted.
- **Deployment binding:** the Infrastructure roadmap is Draft and defines no accepted API compute, TLS edge, routing, secret delivery, telemetry sinks, or dependency bindings for `api.playsrc.online`.
