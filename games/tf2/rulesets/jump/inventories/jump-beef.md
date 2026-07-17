# `jump_beef` Tempus Zone-Identity Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

This fixed inventory pins the public Tempus identity facts for configured BSP SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`. It does not invent zone geometry, trigger mappings, topology edges, or run behavior.

## Capture Identity

- Public contract: Tempus API v0 OpenAPI, 33,411 bytes, SHA-256 `3d7bf3b65d4e74a2ae3b12f5c6b45a1cf003f29b6dc35882cccac12c8eb59d99`.
- Request: `GET https://tempus2.xyz/api/v0/maps/name/jump_beef/fullOverview2`.
- Response header time: `Fri, 17 Jul 2026 17:41:26 GMT`.
- Response identity: 8,004 bytes, SHA-256 `24d6f390d2d7b5eeb7ab46323ce770b88506aa1b7fc739d2c4d9e5c5d4518b83`.
- Canonical identity subset: `map_info`, `zone_counts`, and all five zone fields sorted by `(type, zoneindex, id)`; 3,857 bytes; SHA-256 `b140813677b5b1b64777500b6ac972d5008338d39120b64db4eb75018bb827f5`.
- Map product identity: ID `1`, name `jump_beef`, date added `1396524945.047`.

The capture contains exactly 49 zone identities. The API response is mutable product data; a later response is a different input until reviewed and pinned.

## Course-Facing Identities

| Type | Index | Zone ID | Custom name | Established fact |
|---|---:|---:|---|---|
| `map` | 1 | 1 | — | One map-run identity exists. |
| `map_end` | 1 | 10 | — | One map-end identity exists. |
| `course` | 1 | 2 | — | Course-run identity 1 exists. |
| `course` | 2 | 3 | — | Course-run identity 2 exists. |
| `course` | 3 | 9 | — | Course-run identity 3 exists. |
| `course_end` | 1 | 4 | — | Course-end identity 1 exists. |
| `course_end` | 2 | 8 | — | Course-end identity 2 exists. |
| `checkpoint` | 1 | 6283 | — | Checkpoint identity 1 exists. |
| `checkpoint` | 2 | 6284 | — | Checkpoint identity 2 exists. |
| `checkpoint` | 3 | 6285 | — | Checkpoint identity 3 exists. |
| `checkpoint` | 4 | 6286 | — | Checkpoint identity 4 exists. |
| `checkpoint` | 5 | 6287 | — | Checkpoint identity 5 exists. |
| `checkpoint` | 6 | 6288 | — | Checkpoint identity 6 exists. |
| `bonus` | 1 | 1839 | — | Bonus-run identity 1 exists. |
| `bonus_end` | 1 | 1840 | — | Bonus-end identity 1 exists. |
| `trick` | 1 | 5146 | `Wallpogo` | Trick-result identity 1 exists; the name defines no behavior. |
| `trick` | 2 | 10063 | `moo` | Trick-result identity 2 exists; the name defines no behavior. |

The absence of a `linear` identity and the presence of three `course` identities establish that this capture is not the current playsrc single-linear-course input. The API does not establish checkpoint scope, course adjacency, final-course termination, concurrent-run ordering, or any contact transition.

## Miscellaneous Identities

| Index | Zone ID | Custom name |
|---:|---:|---|
| 1 | 2836 | `Cow 1` |
| 2 | 2837 | `Cow 2` |
| 3 | 2843 | `TWO` |
| 4 | 3253 | `2-3 Gap` |
| 5 | 3255 | — |
| 6 | 5144 | `Wallpogo cancel` |
| 7 | 5145 | `Wallpogo End` |
| 8 | 5147 | `Wallpogo start` |
| 9 | 8185 | — |
| 10 | 10042 | `cow 1` |
| 11 | 10043 | `cow 2` |
| 12 | 10044 | `cow 3` |
| 13 | 10045 | `cow 4` |
| 14 | 10046 | — |
| 15 | 10047 | `cow 6` |
| 16 | 10048 | `cow 7` |
| 17 | 10049 | `cow 8` |
| 18 | 10050 | `cow 5` |
| 19 | 10051 | `cow 9` |
| 20 | 10052 | `cow 10` |
| 21 | 10053 | `cow 11` |
| 22 | 10054 | `cow 12` |
| 23 | 10055 | `cow 13` |
| 24 | 10056 | `cow 14` |
| 25 | 10057 | `cow 15` |
| 26 | 10058 | `cow 16` |
| 27 | 10059 | `cow 17` |
| 28 | 10060 | `cow 18` |
| 29 | 10061 | `cow 19` |
| 30 | 10062 | `cow 20` |
| 31 | 10064 | `moo start` |
| 32 | 10065 | `moo end` |

Every miscellaneous identity and name is retained verbatim. Names never create start, end, cancel, checkpoint, teleport, or trick semantics.

## Required Missing Data

The OpenAPI and captured response omit every zone bound, supplemental trigger identity, zone-to-trigger relation, Collision shape identity, checkpoint scope, topology edge, enabled state, teleport destination, and transition rule. The public supplemental trigger plugin consumes a runtime-supplied `triggers.kv` containing trigger IDs and AABB bounds, but no checked public source supplies that file for `jump_beef` or maps its trigger IDs to the 49 API zone IDs.

The configured BSP contains 22 map-authored `trigger_multiple` entities used for hint/text I/O. They are not Tempus zones. A course definition that reclassifies those entity ordinals as Tempus starts, checkpoints, or ends is invalid evidence.

Exact course activation remains Blocked until an immutable zone export supplies the missing geometry/mapping and an accepted timer contract supplies checkpoint, run, reset/restart, save/restore, validity, and completion behavior.
