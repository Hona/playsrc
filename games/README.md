# Games

Game modules implement behavior belonging to one Source 1 title. They compose generic packages without placing game differences into those packages.

## Organization

| Game | Objective |
|---|---|
| [`tf2/`](tf2/) | First complete parity target. |
| [`css/`](css/) | Future Counter-Strike: Source target. |
| [`csgo/`](csgo/) | Future legacy Source 1 Counter-Strike: Global Offensive target. |

Each game owns its classes, items, weapons, entities, movement differences, replicated state, game rules, and presentation mappings. Rulesets are nested under their game because identically named modes do not imply shared behavior.
