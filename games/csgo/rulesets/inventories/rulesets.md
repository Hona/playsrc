# Legacy Source 1 CS:GO Ruleset Inventory

## Inventory Metadata

| Field | Value |
|---|---|
| Owning roadmap | [`../ROADMAP.md`](../ROADMAP.md) |
| Authority identity | Exact final-build first-party mode declarations; pinned stock SurfTimer, bhoptimer, and GOKZ contract manifests; current playsrc Owner Registry |
| Authority revision | Engine build `8802`; protocol `13881`; depot `731` manifest `1224088799001669801`; depot `732` manifest `6314304446937576250`; SurfTimer `1.1.4` commit `6a563cd2e8023049815d2b8b301bcda7e0d75afa`; bhoptimer `4.0.1` commit `e1be060c1c59529fd312bddc88fcd8e57e34a2c9`; GOKZ `3.7.0` commit `c56aa84f0581167bc5a2998e9f631382f67141be`; exact retained first-party bytes/captures and accepted community manifests Missing |
| Generator command | Missing |
| Output path | `games/csgo/rulesets/inventories/rulesets.md` |
| Candidate item count | 17 |
| Accepted item count | 0 |

This file is a manually derived candidate inventory, not generated output. Each stable identity equals one child-completion row in the owning aggregation roadmap. First-party items remain Blocked until exact final-build declarations and captures are retained. Community items remain Not started because their pinned revisions provide an available next action, but no child contract or accepted generated manifest exists.

## Candidate Items

| Stable identity | Future child roadmap | Complete child ownership | Authority or next requirement | Status |
|---|---|---|---|---|
| `rulesets.casual` | `games/csgo/rulesets/casual/ROADMAP.md` | Drop-in bomb/hostage match and round lifecycle; team/objective/economy/score policy; spawning; timeout; completion; winner; transition. | Exact final-build Casual declaration/configuration and retained target captures. | Blocked |
| `rulesets.competitive` | `games/csgo/rulesets/competitive/ROADMAP.md` | Long/short live-match lifecycle; warmup; rounds; halftime; economy/score; timeout; overtime/surrender disposition; completion; winner; transition. | Exact final-build Competitive declaration/configuration and retained target captures. | Blocked |
| `rulesets.wingman` | `games/csgo/rulesets/wingman/ROADMAP.md` | 2v2 single-site lifecycle; team limits; economy/score; timers; halftime; spawn restrictions; completion; winner; transition. | Exact final-build Wingman declaration/configuration and retained target captures. | Blocked |
| `rulesets.weapons-expert` | `games/csgo/rulesets/weapons-expert/ROADMAP.md` | Competitive-derived lifecycle plus per-player one-purchase weapon restrictions and reset. | Final-build support disposition, configuration, and retained target captures. | Blocked |
| `rulesets.arms-race` | `games/csgo/rulesets/arms-race/ROADMAP.md` | Respawn lifecycle; weapon progression; kill requirements; regression; leader state; golden-knife completion; winner; reset. | Exact final-build progression/configuration and retained target captures. | Blocked |
| `rulesets.demolition` | `games/csgo/rulesets/demolition/ROADMAP.md` | Bomb rounds; team weapon progression; kill/bonus upgrades; halftime; score; completion; winner; reset. | Exact final-build progression/configuration and retained target captures. | Blocked |
| `rulesets.deathmatch` | `games/csgo/rulesets/deathmatch/ROADMAP.md` | Warmup; spawn/respawn; team/FFA policy; loadouts; kill/bonus scoring; timer; winner; reset. | Exact final-build Deathmatch variant configuration and retained target captures. | Blocked |
| `rulesets.training` | `games/csgo/rulesets/training/ROADMAP.md` | Single-player lesson/course setup; forced team/loadout; scripted objectives; failure/reset; completion; terminal transition. | Exact final-build training map/script inventory and retained target captures. | Blocked |
| `rulesets.custom-baseline` | `games/csgo/rulesets/custom-baseline/ROADMAP.md` | Final-build Custom initialization and typed declared-map rule lifecycle only. | Exact final-build baseline configuration, declared map set, and retained target captures. | Blocked |
| `rulesets.guardian` | `games/csgo/rulesets/guardian/ROADMAP.md` | Cooperative mission setup; human team; bot challenge/waves; equipment/economy; retries; failure; completion; transition. | Exact final-build mission/configuration inventory and retained target captures. | Blocked |
| `rulesets.co-op-strike` | `games/csgo/rulesets/co-op-strike/ROADMAP.md` | Cooperative story mission; stage/objective gates; bot waves; checkpoint/respawn; failure; completion; terminal transition. | Exact final-build mission/script inventory and retained target captures. | Blocked |
| `rulesets.flying-scoutsman` | `games/csgo/rulesets/flying-scoutsman/ROADMAP.md` | SSG 08/knife loadout; no-buy economy; low-gravity air movement/accuracy policy; rounds; score; completion; reset. | Final-build skirmish routing/configuration and retained target captures. | Blocked |
| `rulesets.retakes` | `games/csgo/rulesets/retakes/ROADMAP.md` | Teams; site/pre-plant; blockers; loadout cards; retake rounds; score; first-to-eight completion; reset. | Exact final-build Retakes/card configuration and retained target captures. | Blocked |
| `rulesets.danger-zone` | `games/csgo/rulesets/danger-zone/ROADMAP.md` | Solo/squad setup; deployment; safe zones; loot/delivery/redeploy; survival objectives; elimination; winner; cleanup. | Exact final-build survival configuration/map scripts and retained target captures. | Blocked |
| `rulesets.surf` | `games/csgo/rulesets/surf/ROADMAP.md` | Stock SurfTimer session/course lifecycle; linear/staged zones; stages/checkpoints; tick timing; practice/invalidation; completion/result facts. | SurfTimer `1.1.4` commit `6a563cd2e8023049815d2b8b301bcda7e0d75afa`; generate and accept the stock dependency/configuration/zone/style contract. | Not started |
| `rulesets.bhop` | `games/csgo/rulesets/bhop/ROADMAP.md` | Stock bhoptimer session/track/style lifecycle; zones; checkpoints; tick timing; practice/invalidation; completion/result facts. | bhoptimer `4.0.1` commit `e1be060c1c59529fd312bddc88fcd8e57e34a2c9`; generate and accept the stock dependency/configuration/track/zone/style contract. | Not started |
| `rulesets.kz` | `games/csgo/rulesets/kz/ROADMAP.md` | GOKZ Vanilla/SimpleKZ/KZTimer course lifecycle; starts; checkpoints; teleports; tick timing; invalidation; completion/result facts. | GOKZ `3.7.0` commit `c56aa84f0581167bc5a2998e9f631382f67141be`; generate and accept the stock dependency/configuration/mode/course contract. | Not started |

## Inventory Boundary

- These 17 identities are the entire currently declared legacy CS:GO ruleset candidate universe.
- War Games is a catalog grouping; its final active children are Arms Race, Demolition, Flying Scoutsman, and Retakes.
- Team and free-for-all Deathmatch are explicit configurations inside one Deathmatch child.
- Long and short Competitive are explicit configurations inside one Competitive child. Premier queueing, map veto, rating, trust, and matchmaking remain outside the child.
- Custom Baseline cannot load an undeclared plugin or create an open-ended child universe.
- Surf, Bhop, and KZ own mode/course policy only. Generic Movement and legacy CS:GO movement specialization remain with their registered owners; durable records/rankings remain application/service behavior.
- Any additional legacy CS:GO ruleset requires one atomic update to this inventory, the aggregation roadmap, affected exclusions, child path, generator inputs, and denominator review.
