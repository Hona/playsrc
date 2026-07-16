# TF2 Networked State Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 TF2 shared/server/client network-table, send/receive property, and prediction declarations; SDK `tf_usermessages.cpp`; configured TF2 `resource/modevents.res`.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `10822003`; `resource/modevents.res` SHA-256 `662cd625113f980d4c60a11b88e6cb30ab44480e367f71a8ab4ff47c1e4ba2c0`.

Generator command: Missing

Output path: `games/tf2/inventories/networked-state.md`

Candidate item count: 2,318

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

SDK declaration stable identity is `<category>:<SDK-relative-path>:<one-based-line>:<macro>` at the pinned revision. Event identity is `event:<name>:<one-based-duplicate-occurrence>`. User-message identity is `usermessage:<name>`.

| Category | Candidate items | Ordered-identity SHA-256 | Required record |
|---|---:|---|---|
| Network-table declarations | 223 | `f15b6d0016ef8c970031bbe6005cdeca3f1a325c1a7e11a4d6188890c855c0f5` | Table/class identity, base table, direction, owning TF2 state, and generic transport handoff |
| Send/receive property declarations | 1,492 | `0b0f802238d8eaa784cb613cf51deb38a3bb68a7309029df43f708a2700c9c7b` | Logical field, type, bits/range/flags/proxy, exclusions, arrays/tables, authority, recipients, and replay meaning |
| Prediction-table declarations | 93 | `659b78cb5bb3b27d8e8811ccc6e5a90ae332591aa13b05200c73173c6a612402` | Predicted owner, base prediction state, reconciliation scope, and error policy |
| Prediction-field declarations | 106 | `8affa066655180ea626c4012860a48feb98a82132231f4611fc4e8715c2d1595` | Logical field, type, tolerance declaration, send-table relationship, and replay/reset behavior |
| TF2 modevent records | 325 | `69a444d711c1c3b9969f0ed9d3dd92c475ede4842a73774501671397f58dcef5` | Ordered event name occurrence, typed fields, producer, consumers, reliability/locality, and sole behavior owner |
| User-message registrations | 79 | `7e54b27ef208bd4a3bbb1687ddca1eed05ee2ccb388ff0a4911f332c55dfb082` | Name, fixed or variable byte contract, producer, consumer, and gameplay/presentation disposition |

`resource/modevents.res` contains 325 records and 324 unique names because `scout_slamdoll_landed` is declared twice. Both occurrences remain candidates until generation records their exact merge/disposition.

TF2 owns canonical field meaning, production from TF2 state, prediction policy, event/message payload meaning, and replay interpretation. Networking owns table flattening, encoding, transport, baseline/delta state, acknowledgement, recipient delivery, and reconciliation mechanics. Demo and Replay own recorded-byte decoding and timeline advancement. Presentation owners consume typed TF2 presentation facts.

## Generation Contract

The future generator must preprocess TF2 client/server builds, pair send/receive declarations into logical fields without losing direction-specific differences, expand table macros and arrays, join prediction fields, parse every modevent field and user-message size expression, assign one owner, and fail on an unmatched pair, conflicting type/quantization, unknown proxy, duplicate event conflict, unbounded variable payload, unresolved ruleset event, or item-count mismatch.
