# Counter-Strike: Source Ruleset Inventory

## Inventory Metadata

| Field | Value |
|---|---|
| Owning roadmap | [`../ROADMAP.md`](../ROADMAP.md) |
| Authority identity | Current playsrc CS:S declarations plus selected immutable standard, competitive, surf, bhop, and KZ target manifests |
| Authority revision | playsrc `1c2184c11353a618d8ef941c7f161e488346203e`; all external target revisions Missing |
| Generator command | Missing |
| Output path | `games/css/rulesets/inventories/rulesets.md` |
| Candidate item count | 8 |
| Accepted item count | 0 |

This file is a manually derived candidate inventory, not generated output. Each stable identity equals one child-completion row in the owning aggregation roadmap. An item remains Blocked until its immutable target authority, complete child contract, evidence method, and composition boundary are accepted.

## Candidate Items

| Stable identity | Future child roadmap | Complete child ownership | Missing authority or decision | Status |
|---|---|---|---|---|
| `rulesets.standard-bomb-defusal` | `games/css/rulesets/standard-bomb-defusal/ROADMAP.md` | Bomb-round lifecycle; plant/defuse objective policy; score; spawn/restriction policy; timeout; completion; winner; next-round transition. | Selected CS:S build/configuration and retained bomb-round target captures. | Blocked |
| `rulesets.standard-hostage-rescue` | `games/css/rulesets/standard-hostage-rescue/ROADMAP.md` | Hostage-round lifecycle; rescue objective policy and threshold; score; spawn/restriction policy; timeout; completion; winner; next-round transition. | Selected CS:S build/configuration and retained hostage-round target captures. | Blocked |
| `rulesets.standard-assassination` | `games/css/rulesets/standard-assassination/ROADMAP.md` | VIP selection/escort lifecycle; safety/death/timeout outcomes; score; restrictions; completion; winner; next-round transition. | Current-target support disposition, selected configuration, and retained target captures. | Blocked |
| `rulesets.standard-escape` | `games/css/rulesets/standard-escape/ROADMAP.md` | Escaper eligibility; escape ratio/threshold lifecycle; team rotation; score; restrictions; timeout; completion; winner; next-round transition. | Current-target support disposition, selected configuration, and retained target captures. | Blocked |
| `rulesets.competitive` | `games/css/rulesets/competitive/ROADMAP.md` | One complete competitive match/round lifecycle and policy. | Immutable policy revision and decision between complete selectable ruleset versus standard-ruleset-owned configuration. | Blocked |
| `rulesets.surf` | `games/css/rulesets/surf/ROADMAP.md` | Surf session/course lifecycle; starts; checkpoints; finishes; tick timing; spawn/restriction policy; invalidation; completion; result facts. | Selected organization/release, course schema, movement configuration, timer contract, evidence corpus, and records boundary. | Blocked |
| `rulesets.bhop` | `games/css/rulesets/bhop/ROADMAP.md` | Bhop session/course lifecycle; starts; checkpoints; finishes; tick timing; spawn/restriction policy; invalidation; completion; result facts. | Selected organization/release, course schema, movement configuration, timer contract, evidence corpus, and records boundary. | Blocked |
| `rulesets.kz` | `games/css/rulesets/kz/ROADMAP.md` | KZ session/course lifecycle; starts; checkpoints; teleports; finishes; tick timing; spawn/restriction policy; invalidation; completion; result facts. | Selected organization/release, course schema, movement configuration, timer contract, evidence corpus, and records boundary. | Blocked |

## Inventory Boundary

- These eight identities are the entire currently declared CS:S ruleset candidate universe.
- A standard scenario is a complete ruleset, not a shared implementation with another game.
- Competitive cannot remain an overlay candidate; acceptance must resolve it to one complete selectable child or remove it and update the standard children in the same denominator change.
- Surf, bhop, and KZ own mode/course policy only. Generic movement and CS:S movement specialization remain with their registered owners.
- Any additional CS:S ruleset requires an atomic update to this inventory, the aggregation roadmap, affected ownership exclusions, child path, generator inputs, and denominator review.
