# TF2 Cvars Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 TF2 shared, server, and client declarations whose active source text constructs a `ConVar`; inherited generic cvars are inputs only when a TF2 consumer reads them and remain owned by their generic package.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`.

Generator command: Missing

Output path: `games/tf2/inventories/cvars.md`

Candidate item count: 1,278

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

Stable identity is the exact console name. The candidate derives from 1,281 declarations and 1,278 unique names; `sv_vote_issue_kick_allowed`, `tf_casual_welcome_hide`, and `tf_comp_welcome_hide` each have two declarations. The sorted unique-name set has SHA-256 `7f9640ff764c3657a86aeebe844335a7cfa748437aad1c6713dcf0a83297ba4d`.

| Exact name predicate | Unique names |
|---|---:|
| Starts with `tf_` | 1,050 |
| Starts with `cl_` or `_cl_` | 64 |
| Starts with `sv_` | 38 |
| Starts with `bot_` | 24 |
| Starts with `hud_` | 22 |
| Starts with `training_` | 18 |
| Starts with `mp_` | 17 |
| Matches none of the seven predicates above | 45 |

Every generated item must record declaring paths, duplicate declarations, type/default/min/max/flags/help text, TF2 read sites, write/change callbacks, replication/prediction relevance, unit, owner, deterministic snapshot rule, and required coverage. Client UI preferences, diagnostics, bots, matchmaking, account progression, mode-only policy, and presentation controls remain in the inventory with an exact excluded or delegated owner.

Gameplay advancement consumes one immutable validated TF2 cvar snapshot per tick. It never reads an ambient mutable registry midway through a transition. A replicated cvar change takes effect only at its declared simulation boundary and emits the exact state/event consequence required by its consumers.

## Generation Contract

The future generator must preprocess the TF2 client and server targets, merge exact duplicate names only after proving identical contracts, join declarations to every read and callback, classify generic and mode ownership, emit stable names in byte order, and fail on conflicting defaults/flags/bounds, an unclassified gameplay read, a dynamic unnamed cvar, an unresolved owner, or item-count mismatch.
