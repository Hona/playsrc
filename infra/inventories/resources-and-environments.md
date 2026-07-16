# Infrastructure Resources And Environments Inventory

Status: Draft. This inventory is not accepted and does not enter the Infrastructure completion denominator.

## Metadata

| Field | Value |
|---|---|
| Authority identity | playsrc `ROADMAP.md`, `ARCHITECTURE.md`, `TERMINOLOGY.md`, `infra/README.md`, the four Active application and service READMEs, and `packages/asset-store/ROADMAP.md` at the revision below |
| Authority revision | playsrc commit `690cc3a05449383119c0a4272395a8b034f74d45` |
| Generator command | Blocked: no checked-in generator or Accepted deployment-requirement manifests exist |
| Output path | `infra/inventories/resources-and-environments.md` |
| Item count | 27 |
| Accepted item count | 0 |
| Owner | `infra` |
| Owning roadmap | `infra/ROADMAP.md` |

## Active Candidate Items

These 27 candidate items are the finite union of currently declared Active environment, domain, application, service, asset-delivery, observability, deployment-identity, and cost-control requirements. They remain candidates because their provider shape, hosted environment, dependencies, limits, and bindings are not accepted.

| Identity | Kind | Declared consumer or target | Required infrastructure result | Missing acceptance input | Coverage |
|---|---|---|---|---|---|
| `environment.local` | Environment | Local development workflow and local Asset Service origin | Record local as an execution environment with no provisioned cloud resource, provider credential, remote state, public DNS, or shared mutable binding. | Checked environment admission contract and generator | Intentionally inert |
| `dns.zone.playsrc.online` | DNS authority | All declared `playsrc.online` hostnames | Manage the accepted authoritative zone boundary, records, DNSSEC policy, access, drift, and protection without assuming registrar ownership. | Zone custody, provider, registrar boundary, DNSSEC, identities, and teardown policy | Unsupported |
| `dns.record.tf2` | DNS record | `tf2.playsrc.online`; Active TF2 web application | Resolve the hostname only to the accepted TF2 application route with declared type, target, TTL, and replacement order. | Accepted TF2 deployment target, environment, provider, record type, target, and TTL | Unsupported |
| `dns.record.tempus` | DNS record | `tempus.playsrc.online`; Active Tempus web application | Resolve the hostname only to the accepted Tempus application route with declared type, target, TTL, and replacement order. | Accepted Tempus deployment target, environment, provider, record type, target, and TTL | Unsupported |
| `dns.record.assets` | DNS record | `assets.playsrc.online`; Active Asset Service and CDN | Resolve the hostname only to the accepted asset-delivery route with declared type, target, TTL, and replacement order. | Accepted Asset Service deployment and CDN target, environment, provider, record type, and TTL | Unsupported |
| `dns.record.api` | DNS record | `api.playsrc.online`; Active API service | Resolve the hostname only to the accepted API route with declared type, target, TTL, and replacement order. | Accepted API deployment target, environment, provider, record type, target, and TTL | Unsupported |
| `tls.binding.tf2` | TLS binding | `tf2.playsrc.online` | Bind and renew a certificate covering only the accepted TF2 route names under the accepted key-custody and protocol policy. | Certificate authority, challenge, grouping, key custody, protocol floor, renewal, revocation, and owner | Unsupported |
| `tls.binding.tempus` | TLS binding | `tempus.playsrc.online` | Bind and renew a certificate covering only the accepted Tempus route names under the accepted key-custody and protocol policy. | Certificate authority, challenge, grouping, key custody, protocol floor, renewal, revocation, and owner | Unsupported |
| `tls.binding.assets` | TLS binding | `assets.playsrc.online` | Bind and renew a certificate covering only the accepted asset route names under the accepted key-custody and protocol policy. | Certificate authority, challenge, grouping, key custody, protocol floor, renewal, revocation, and owner | Unsupported |
| `tls.binding.api` | TLS binding | `api.playsrc.online` | Bind and renew a certificate covering only the accepted API route names under the accepted key-custody and protocol policy. | Certificate authority, challenge, grouping, key custody, protocol floor, renewal, revocation, and owner | Unsupported |
| `route.tf2` | Public route | Active TF2 web application | Route the accepted host and paths to one healthy TF2 application deployment without cross-product fallback. | Accepted route, hosting target, listener, health, replacement, and failure contract | Unsupported |
| `route.tempus` | Public route | Active Tempus web application | Route the accepted host and paths to one healthy Tempus application deployment without cross-product fallback. | Accepted route, hosting target, listeners, dependencies, health, replacement, and failure contract | Unsupported |
| `route.assets` | Public route | Active Asset Service and CDN | Route accepted asset requests to the CDN and exact origin according to Asset Service cache, range, metadata, and failure behavior. | Accepted resource paths, methods, cache, range, origin, bound, and health contract | Unsupported |
| `route.api` | Public route | Active API service | Route accepted API requests to healthy API compute under its exact listener, authentication-resource, bound, and failure contract. | Accepted endpoint, listener, health, authentication-resource, request-bound, and failure contract | Unsupported |
| `compute.web.tf2` | Web hosting or compute | Active TF2 web application | Host one immutable TF2 application build with exact bindings, capacity, replacement, health, and telemetry. | Accepted application build target, static-or-process decision, environment, configuration, bounds, and telemetry | Unsupported |
| `compute.web.tempus` | Web hosting or compute | Active Tempus web application | Host one immutable Tempus application build with exact service and asset bindings, capacity, replacement, health, and telemetry. | Accepted application build target, static-or-process decision, environment, dependencies, bounds, and telemetry | Unsupported |
| `compute.service.api` | Service compute | Active API service | Run the accepted API process with exact listener, identity, configuration, secrets, network, capacity, scaling, health, and telemetry. | Accepted API endpoint and deployment manifests plus hosted environment | Unsupported |
| `compute.service.assets` | Service compute | Active Asset Service | Run the accepted asset-origin process with exact storage and CDN bindings, listener, identity, request bounds, scaling, health, and telemetry. | Accepted Asset Service resource and deployment manifests plus hosted environment | Unsupported |
| `delivery.assets-cdn` | CDN | Active Asset Service and web applications | Mirror only published Asset Store resources under the accepted origin, cache, range, method, key, and observability contract. | Accepted Asset Service contract, CDN provider, origin, cache policy, bounds, and environment | Unsupported |
| `storage.asset-objects` | Immutable-object storage | Asset Store remote adapter and Asset Service origin | Provide create-if-absent immutable writes, exact origin reads, strong read-after-write, integrity verification, retention, encryption, and deletion denial. | Accepted remote adapter, provider capability revision, storage identity, retention, encryption, region, and access | Unsupported |
| `storage.asset-channels` | Mutable Channel storage | Asset Store remote adapter and Asset Service origin | Provide exact reads and conditional record replacement under policy separate from immutable objects; deny infrastructure target selection. | Accepted remote adapter, conditional primitive, namespace, retention, recovery, encryption, region, and access | Unsupported |
| `observability.logs` | Log resource | All Active deployments and infrastructure operations | Retain accepted structured application, service, provider-audit, access, deployment, drift, recovery, teardown, and cost events under exact redaction and bounds. | Log provider, fields, redaction, retention, ingestion bounds, readers, and cost limit | Unsupported |
| `observability.metrics` | Metric resource | All Active deployments and infrastructure resources | Retain accepted availability, latency, error, saturation, capacity, scaling, storage, CDN, DNS, TLS, operation, and cost series under exact units and cardinality bounds. | Metric provider, indicators, units, labels, windows, cardinality, retention, and readers | Unsupported |
| `observability.alerts` | Alert resource | Named operators for every Active target | Evaluate accepted rules and route firing and recovery events with exact thresholds, windows, deduplication, escalation, and tests. | Availability targets, rules, contacts, escalation, quieting, provider, and evidence route | Unsupported |
| `observability.dashboards` | Dashboard resource | Named operators and maintainers for every Active target | Present checked environment, deployment, route, resource, availability, capacity, alert, and cost queries without cross-environment data. | Audiences, panels, queries, units, refresh, provider, access, and environment set | Unsupported |
| `identity.deployment` | Deployment identity family | Infrastructure plan, apply, drift, restore, and teardown operations | Issue distinct least-privilege identities per hosted environment and attribute every provider action to one approved operation. | Identity provider, hosted environments, provider actions, operator roles, emergency policy, and audit destination | Unsupported |
| `cost.control` | Cost-control resource | Every Active environment and shared resource | Attribute charges and enforce accepted budgets, forecasts, anomalies, untagged-resource policy, and post-teardown residual-cost checks. | Provider pricing identity, environment set, budgets, categories, forecast window, thresholds, and owners | Unsupported |

## Future Activation Candidates

These 13 stable candidates are explicitly excluded from the 27-item count and current denominator. Activation requires the root lifecycle to become Active and the owning application or service deployment denominator to be Accepted before generation.

| Identity | Future owner or target | Activation requirement |
|---|---|---|
| `dns.record.css` | `css.playsrc.online` | Active CS:S browser product and Accepted CS:S deployment manifest |
| `tls.binding.css` | `css.playsrc.online` | Accepted hostname, route, certificate, and key-custody requirements |
| `route.css` | CS:S web application | Accepted route, hosting, listener, health, capacity, and failure requirements |
| `compute.web.css` | CS:S web application | Accepted application-build and deployment manifest |
| `dns.record.csgo` | `csgo.playsrc.online` | Active legacy Source 1 CS:GO browser product and Accepted deployment manifest |
| `tls.binding.csgo` | `csgo.playsrc.online` | Accepted hostname, route, certificate, and key-custody requirements |
| `route.csgo` | Legacy Source 1 CS:GO web application | Accepted route, hosting, listener, health, capacity, and failure requirements |
| `compute.web.csgo` | Legacy Source 1 CS:GO web application | Accepted application-build and deployment manifest |
| `dns.record.servers` | `servers.playsrc.online` | Active online multiplayer service route and Accepted service deployment manifest |
| `tls.binding.servers` | `servers.playsrc.online` | Accepted hostname, route, certificate, and key-custody requirements |
| `route.servers` | Matchmaking or game-server service selected by the Accepted route contract | Active owning service and Accepted route, listener, health, capacity, and failure requirements |
| `compute.service.matchmaking` | Matchmaking service | Active matchmaking target and Accepted queue, process, data, network, scaling, and deployment requirements |
| `compute.service.game-servers` | Game-server service | Active online multiplayer target and Accepted pool, network, capacity, scaling, drain, and deployment requirements |

## Required Categories Not Yet Emitted

The generator must emit additional stable items when Accepted inputs require them. None is counted while its exact identity and owner are unavailable:

- Hosted development, staging, or production environments. Channel names do not emit environment items.
- Network boundaries, private origins, state backend, provider account or project, and regional or global placement resources.
- Runtime identities, operator roles, access policies, secret identities, and configuration bindings.
- Databases, queues, mutable-data backups, restore environments, and recovery resources required by Accepted services.
- Per-workload scaling resources, provider quota reservations, and capacity controls not intrinsic to an emitted compute or storage resource.

## Generation Contract

The future checked-in generator must consume immutable revisions of:

1. The root target lifecycle and Owner Registry.
2. Every Accepted application and service roadmap plus its machine-readable deployment manifest.
3. The Accepted Asset Store remote-adapter and Asset Service origin contracts.
4. The Accepted environment, provider, state, location, naming, network, identity, and access decisions.
5. The secret-identity registry without secret values.
6. The accepted capacity, scaling, telemetry, recovery, teardown, and cost policies.

It must then:

1. Reject an unaccepted authority, unresolved owner, duplicate semantic identity, unknown lifecycle, missing revision, missing dependency, cycle, Future resource in an Active set, or cross-environment reference.
2. Emit `environment.local` with an empty provisioned-resource set and emit hosted environments only when independently declared.
3. Join each Active domain, application, service, and Asset Store requirement to exact environment resources and emit shared resources once.
4. Emit each item with semantic identity, kind, sole owner, environment, provider kind and locked revision, dependencies, location, network, bounds, scaling, binding identities, secret-reference identities, telemetry, backup and restore policy, protection, cost attribution, and coverage classification.
5. Sort by semantic identity, record exact candidate and accepted counts, and reproduce this output byte-for-byte from the same revisions.
6. Fail without replacing the prior inventory when an input is malformed, Missing, Unknown, Unsupported, conflicting, or stale.

## Acceptance Blockers

- Accept deployment denominators and machine-readable deployment manifests for TF2, Tempus, API, and Asset Service.
- Accept hosted environment identities, infrastructure engine, providers and revisions, state backend, locations, DNS and TLS ownership, and evidence environments.
- Accept the Asset Store remote adapter and Asset Service origin, CDN, cache, range, bound, and telemetry contracts.
- Accept network, identity, access, secret-reference, configuration-binding, capacity, scaling, observability, recovery, teardown, and cost policies.
- Implement one checked-in generator and verify byte-identical output on two runs.
- Review the emitted item set and record an Accepted denominator review in `infra/ROADMAP.md`.
