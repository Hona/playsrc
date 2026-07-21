# Asset Service Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines denominator and evidence requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines Object, root manifest, Catalog, Channel, Asset store, CDN, Browser cache, current contract, and Complete. The parent [`../ROADMAP.md`](../ROADMAP.md) defines common process behavior.

## Completion Denominator

This leaf roadmap contains exactly 18 behavior rows. Its complete HTTP resource universe is the four hand-enumerated route families below; no generated inventory is required for that finite set.

The denominator is Not accepted. The Asset Store interface and remote deployment binding are not accepted, and no asset-service process, evidence record, or review record exists.

## Inputs

- One accepted parent service configuration whose public origin is `https://assets.playsrc.online` in production and whose local origin is an explicitly configured loopback listener.
- One current Asset Store read interface returning exact verified bytes and descriptors for objects, roots, and catalogs, plus exact channel-record bytes and revision. The service never constructs, mutates, publishes, repairs, or discovers these resources.
- Exactly these resource families:

| Resource identity | Methods | Successful selected representation |
|---|---|---|
| `/objects/sha256/{hash}` | `GET`, `HEAD` | Exact bytes for the lowercase 64-hex SHA-256 object descriptor. |
| `/roots/sha256/{hash}` | `GET`, `HEAD` | Exact object bytes only when the descriptor kind is `source-root`, `map-root`, `game-root`, or `application-root`. |
| `/catalogs/sha256/{hash}` | `GET`, `HEAD` | Exact object bytes only when the descriptor kind is `catalog`. |
| `/channels/{channel}` | `GET`, `HEAD` | Exact current canonical channel-record bytes and their SHA-256 revision. |

- `OPTIONS` is accepted on each exact route shape solely for CORS preflight, returns 204 with no content and `Cache-Control: no-store`, invokes no Asset Store operation, and appears in `Allow`. Every other method returns 405 with `Allow: GET, HEAD, OPTIONS`.
- A hash path parameter matching exactly `[0-9a-f]{64}` and the accepted Asset Store channel-name grammar and byte bound. Path segments are decoded once; encoded separators, empty segments, dot segments, backslashes, NUL, extra segments, query parameters, and fragments are invalid.
- RFC 9110 HTTP semantics, conditional requests, validators, and byte ranges; RFC 9111 caching; RFC 8246 `immutable`; RFC 9457 problem details; and RFC 9530 `Content-Digest` and `Repr-Digest` using only `sha-256`.
- The configured exact browser-origin allowlist. Resource reads are public, use no credentials, and return representation-invariant bytes when an `Authorization` or cookie field is absent or present.

## Outputs

- `200` with exact complete bytes, or `206` with one exact byte range. Immutable responses use the descriptor media type, `Content-Length`, `ETag: "<lowercase-object-sha256>"`, `Repr-Digest`, and `Accept-Ranges: bytes`. Channel responses use `Content-Type: application/vnd.playsrc.asset-channel+json`, `Content-Length`, and `ETag: "<lowercase-channel-revision-sha256>"`; they omit `Accept-Ranges`.
- `HEAD` with the same status and representation metadata that the corresponding `GET` would produce, excluding message content and `Content-Digest`.
- Immutable resources use `Cache-Control: public, max-age=31536000, immutable, no-transform`. Channel records use `Cache-Control: public, no-cache, must-revalidate, no-transform`. Error responses use `Cache-Control: no-store`.
- Full immutable `200` responses carry equal SHA-256 values in the descriptor, strong ETag, `Content-Digest`, and `Repr-Digest`. A `206` response carries `Content-Digest` for the returned content bytes and `Repr-Digest` for the complete selected representation.
- Structured request observations keyed by resource family, outcome, status, range class, byte count, cache-validation result, and bounded latency; resource hashes and channel names are trace fields only when sampled and are never metric labels.

## Invariants

- The service resolves only the exact route identity. It never maps a URL to an operating-system path, lists storage, appends an extension, decodes a second time, tries another resource family, or falls back to a public directory, Source cache, work directory, CDN, or browser cache.
- Object, root, and catalog responses are byte-preserving and content-encoding-free. No compression, transcoding, JSON reserialization, media-type sniffing, range coalescing, or edge transformation can change the selected bytes.
- Roots and catalogs occupy Asset Store object bytes once. Typed routes validate kind and do not create another stored representation or a second truth source.
- Preconditions are evaluated in RFC 9110 order against one strong ETag snapshot. `If-None-Match` can return 304; failed `If-Match` returns 412. A matching strong `If-Range` permits range service; an absent or nonmatching validator returns the full representation.
- Exactly one byte range is accepted. Multiple or malformed range sets return 400. A syntactically valid range with no selected bytes returns 416 and `Content-Range: bytes */<complete-length>`. `Range` on `HEAD` or a channel route is ignored.
- An Asset Store `IntegrityFailure` exposes no resource bytes. If detected before commitment it returns one stable 500 problem; if detected or an I/O failure occurs after commitment, the service terminates the stream and records the same request and descriptor identity.
- Missing, malformed, and wrong-kind resources are distinct internal outcomes. Malformed identities return 400; absent or wrong-kind typed resources return 404 without disclosing alternate typed routes.

## Ownership Exclusions

- `packages/asset-store` owns object identity, descriptors, exact bytes, kind validation, roots, catalogs, channels, integrity, retention, storage, publication, and remote synchronization.
- Applications own catalog entries and selected roots. The API service owns accepted product-data queries; it does not duplicate raw immutable catalog delivery.
- Content and compiler owners produce source and derived descriptors. The asset service cannot compile, resolve Source logical paths, infer a missing dependency, or extract a VPK entry.
- Browsers and CDNs may cache responses but never establish origin existence or integrity. Infrastructure owns CDN behavior, origin routing, TLS, storage bindings, and transformation prohibition.

## Behavior Families

The active migration checkpoint implements loopback `GET`/`HEAD`/`OPTIONS` object and channel routes, strict identities, verified object bytes, strong ETags, immutable/channel cache policy, conditional object reads, whole/open/suffix ranges, RFC 9530 SHA-256 fields, bounded no-store problems, CORS, and `/readyz` over the local Asset Store. The production PoC exposes only immutable objects through a direct R2 Custom Domain with provider ETags, exact-body browser SHA-256 verification, R2 CORS, Cache Rules, and Smart Tiered Cache; this bounded delivery path does not satisfy the complete four-family Asset Service contract.

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Dispatch exactly the four declared resource families and their `GET`/`HEAD` methods without a filesystem, prefix, extension, or alternate-resource fallback. | No asset-service router exists. | **Route-space exhaustive vectors:** vary family, method, segment count, slash, case, encoding, query, traversal, and unknown path; compare selected family, status, `Allow`, and zero unintended store calls. | Not started |
| Validate hash and channel identities before any Asset Store call and pass one canonical typed identity to that interface. | No path validator or accepted channel bound exists. | **Identity and call-spy matrix:** test every grammar boundary, noncanonical hash, encoded separator, Unicode, control byte, and maximum-plus-one name; compare typed call or exact 400 with zero storage access. | Not started |
| `GET /objects/sha256/{hash}` streams complete verified bytes or one exact HTTP range for immutable source and derived objects, including unchanged BSP/VPK objects and encoded resource chunks selected by a source root. | The loopback service serves verified immutable objects; the production R2 origin serves canonical object keys. Neither path has source-root/chunk consumer conformance or the complete accepted service interface. | **Whole/ranged object integration:** empty, one-byte, BSP, VPK directory/segment, identity/deflate chunk, derived, large streamed, missing, and corrupt objects compare requested ranges, response bytes, descriptor fields, store-call trace, and independent SHA-256. | Blocked |
| `GET /roots/sha256/{hash}` returns exact bytes only for source, map, game, or application roots and rejects every other object kind as not found. | No accepted typed root-read interface exists. | **Root-kind table:** exercise every accepted object kind, wrong kind, missing descriptor, malformed manifest, and corrupt bytes; compare status, metadata, content, and store validation trace. | Blocked |
| `GET /catalogs/sha256/{hash}` returns exact bytes only for catalog objects without interpreting application-owned entries. | No accepted typed catalog-read interface exists. | **Catalog opacity vectors:** vary canonical entries and references, wrong kind, missing, and corrupt inputs; compare exact bytes and prove zero entry parsing or transformation in the service. | Blocked |
| `GET /channels/{channel}` reads one atomic channel snapshot and returns its exact canonical bytes and strong revision without following the target. | No accepted channel-read interface or service implementation exists. | **Channel snapshot schedules:** race repeated reads with atomic changes and compare every response to one complete prior or next record, exact ETag, zero target reads, and no mixed bytes. | Blocked |
| Every successful `HEAD` matches the corresponding `GET` status and representation fields while producing no message content or storage-byte stream beyond metadata verification required by the store. | No HEAD implementation exists. | **GET/HEAD differential:** run all four families across full, conditional, missing, wrong-kind, and corrupt cases; compare status/headers and require an empty HEAD body and bounded store calls. | Blocked |
| Immutable and channel responses emit their exact declared cache controls, while every error is `no-store`; no response relies on heuristic freshness. | No asset cache-header implementation exists. | **Cache-policy table:** compare every family, status, conditional result, and range result through origin and a conforming test cache; require exact directives and no stale channel reuse. | Not started |
| Strong ETag preconditions produce exact 200, 304, or 412 outcomes against one resource snapshot without reading or emitting bytes when not required. | No conditional-request implementation exists. | **RFC 9110 precondition matrix:** cross `If-Match`, `If-None-Match`, weak/strong tags, wildcard, lists, changed channel revision, and method; compare evaluation order, store reads, status, headers, and body. | Not started |
| One satisfiable byte range on an immutable resource returns exact 206 content; suffix, open-ended, empty-resource, unsatisfiable, malformed, multiple, and `If-Range` cases follow the declared outcomes. | No range implementation exists. | **Byte-range boundary vectors:** compare every split of fixed empty and non-empty objects plus very large offsets with an independent slicer; verify exact 200/206/400/416 status, fields, and bytes. | Not started |
| Full and partial responses expose RFC 9530 SHA-256 integrity fields with `Repr-Digest` bound to complete bytes and `Content-Digest` bound to transmitted content bytes. | No HTTP integrity metadata implementation exists and descriptor integration is unaccepted. | **Independent digest comparison:** recompute digests for empty, full, every range form, HEAD, and channel responses; compare Structured Field bytes, ETag relation, and corruption failures exactly. | Blocked |
| Exact-route `OPTIONS` returns the declared no-store 204 preflight without storage access; every other unknown method returns 405 with `Allow: GET, HEAD, OPTIONS`; configured browser origins receive only the declared public-read CORS methods and headers, and credentials never vary bytes. | No method or CORS policy exists. | **Method/CORS cross-product:** vary method, origin, preflight headers, cookies, authorization, and credential mode; compare status, `Allow`, CORS fields, `Vary`, body identity, and store calls. | Not started |
| Malformed identities return 400, missing or wrong-kind resources return 404, unsatisfied preconditions return 412, and unavailable ranges return 416, all as bounded no-store problem responses. | No asset error mapping exists. | **Resource-failure decision table:** inject every validation and store outcome for every route; compare public status/type, `Content-Range` where required, redaction, and zero fallback reads. | Not started |
| Asset Store integrity, descriptor, and I/O failures publish no invalid bytes and map to one pre-commit problem or one post-commit stream termination. | No store integration or streaming fault barrier exists. | **Fault injection at every byte boundary:** corrupt descriptors and bytes, fail before headers and after each chunk, and compare delivered prefix, terminal transport, owner error, logs, and open handles. | Blocked |
| Peer cancellation, service drain, and downstream cancellation stop reads and streaming promptly without changing Asset Store state or retaining buffers. | No cancellation-aware asset streaming exists. | **Cancellation schedule:** cancel before read, before headers, at each stream chunk, after completion, and during drain; compare transmitted bytes, store signal, released memory, response outcome, and observations. | Not started |
| Connection, request, range, stream, concurrent-read, queued-read, in-flight-byte, response-time, and observation bounds apply before excess allocation with declared rejection or backpressure. | Exact Asset Store object and service deployment maxima are not accepted. | **Saturation/resource matrix:** run mixed full/range/HEAD/channel reads at every accepted maximum and maximum-plus-one; compare admission, order, memory, descriptors, elapsed time, and serial-equivalent bytes. | Blocked |
| Local and deployed origins implement the same routes, bytes, status, validators, cache, range, integrity, cancellation, and error behavior for the same Asset Store snapshot. | The direct R2 PoC binds `assets.playsrc.online`, immutable object keys, exact-origin CORS, Cache Rules, and Smart Tiered Cache. It omits roots, catalogs, Channels, digest fields, hash ETags, exact error mapping, cancellation observations, and dual-origin evidence. | **Dual-origin conformance:** serve one fixed store through loopback and isolated deployed origin; compare complete response transcripts byte-for-byte except declared Date/server transport fields. | In progress |
| Every web application, tool, CDN origin, and evidence harness reads the current routes, and no path-addressed static mirror, API catalog duplicate, fallback server, compatibility route, or stale resource map remains. | No current service implementation or consumer integration exists. | **Current-interface integration audit:** exercise all named producers and consumers, compare descriptor identities, and repository-audit routes, static mounts, duplicate catalog delivery, fallback reads, version branches, and stale artifacts. | Blocked |

## Generated Inventories

No generated inventory is required. The four resource families are finite, hand-enumerated in Inputs, and each is covered by explicit behavior rows. Accepted generated inventory count: 0.

## Exit Criteria

The Asset service is Complete only when all of these predicates pass:

- All 18 behavior rows are Ready.
- The accepted Asset Store interface supplies exact descriptors, typed root/catalog validation, channel snapshots, bytes, errors, cancellation, and numeric bounds consumed by this service.
- Every route passes complete, HEAD, conditional, range, missing, wrong-kind, corruption, cancellation, saturation, local-origin, and deployed-origin comparisons.
- Immutable resource bytes remain identical across Asset Store, origin, CDN, and browser retrieval under the declared integrity check; channel reads always expose one atomic revision.
- Parent process, transport-security, authentication declaration, authorization declaration, structured-error, observability, and deployment-binding conformance passes.
- All consumers use these four current resource families and no duplicate static mirror, alternate catalog transport, fallback lookup, compatibility path, or legacy server remains.
- No required input, dependency, decision, evidence method, value, behavior, or integration remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Asset Store acceptance:** the Asset Store roadmap is Draft; its descriptor, typed root/catalog read, channel snapshot, integrity error, cancellation, and object-bound contracts have no accepted implementation or review. Checked its README, roadmap, object-kind candidate inventory, and the asset-service README.
- **Numeric service profile:** maximum object and manifest bytes, range offsets, concurrent reads, queue depth, in-flight bytes, stream duration, and aggregate memory depend on unaccepted Asset Store and deployment bounds. RFC 9110 defines semantics but intentionally does not select these application limits.
- **Deployed origin:** the production PoC selects direct R2, TLS, exact-origin CORS, canonical immutable paths, Cache Rules, and Smart Tiered Cache. It does not implement or evidence the complete Asset Service route, validator, digest, error, cancellation, saturation, and observability contract.
- **Consumer integration:** the Draft TF2 and Tempus web roadmaps require channel, application-root, catalog, map-root, and immutable-object delivery, but neither denominator is accepted. Their final request sets and deployment bindings remain unavailable. This prevents current-interface evidence but does not prevent HTTP behavior implementation against synthetic stores.
