# TF2 Objectives Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 TF2 objective, flag, control-point, train-watcher, objective-resource, timer, round-win, and game-type declarations; configured `tf.fgd` objective classes.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `10822003`; `tf.fgd` SHA-256 `e8985f85da985c11e22e4a68b8ea5031bf775f3f471a8d5d4db622e1e7204a49`.

Generator command: Missing

Output path: `games/tf2/inventories/objectives.md`

Candidate item count: 32

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

## Objective Primitives

| Stable identity | SDK/map identity | TF2-game contract | Ruleset contract |
|---|---|---|---|
| `objective.entity.flag` | `item_teamflag` | Own flag state, carrier/drop/return facts, team eligibility hooks, and replicated fields | Own pickup, capture, scoring, return timing, and win meaning |
| `objective.entity.flag-return-icon` | `item_teamflag_return_icon` | Own state-derived presentation mapping | Select visibility policy |
| `objective.entity.capture-zone` | `func_capturezone` | Own TF2 filter inputs and capture request output | Decide whether the request completes an objective |
| `objective.entity.flag-detection-zone` | `func_flagdetectionzone` | Own carrier touch facts | Decide mode response |
| `objective.entity.flag-alert` | `func_flag_alert` | Own carrier/team touch facts and entity outputs | Decide alert use |
| `objective.entity.respawn-flag` | `func_respawnflag` | Own flag reset request | Decide when reset is legal |
| `objective.entity.control-point` | `team_control_point` | Own point index, owner, lock, cappers, model, and replicated state | Own capture prerequisites, score, and completion |
| `objective.entity.control-point-round` | `team_control_point_round` | Own point grouping and availability inputs | Own round selection and completion |
| `objective.entity.control-point-master` | `team_control_point_master` | Own ordered point graph and change facts | Own mode progression and win decision |
| `objective.entity.capture-area` | `trigger_capture_area` | Own touching/capper facts and capture-progress inputs | Own eligibility, rate, block, and completion policy |
| `objective.entity.train-watcher` | `team_train_watcher` | Own train position/progress/checkpoint facts | Own payload progression, rollback, score, and win policy |
| `objective.entity.cart-dispenser` | `mapobj_cart_dispenser` | Own TF2 dispenser behavior attached to an objective mover | Own activation and team policy |
| `objective.entity.resource` | `tf_objective_resource` | Own canonical TF2 objective snapshot and replication mapping | Supply mode-owned values |
| `objective.entity.round-win` | `game_round_win` | Own validated win request and event fields | Decide when to issue it |
| `objective.entity.round-timer` | `team_round_timer` | Own timer primitive inputs/outputs and TF2 mapping | Own start, pause, add-time, expiry, overtime, and completion policy |
| `objective.entity.cp-timer` | `tf_logic_cp_timer` | Own TF2 control-point timer bridge | Own mode lifecycle response |
| `objective.entity.intermission` | `point_intermission` | Own validated terminal-state request | Own eligibility and timing |

## Flag Types

The seven stable items are numeric `ETFFlagType` order: `TF_FLAGTYPE_CTF`, `TF_FLAGTYPE_ATTACK_DEFEND`, `TF_FLAGTYPE_TERRITORY_CONTROL`, `TF_FLAGTYPE_INVADE`, `TF_FLAGTYPE_RESOURCE_CONTROL`, `TF_FLAGTYPE_ROBOT_DESTRUCTION`, and `TF_FLAGTYPE_PLAYER_DESTRUCTION`. TF2 owns each flag state's type-dependent primitive behavior; [`../rulesets/ROADMAP.md`](../rulesets/ROADMAP.md) owns mode lifecycle and scoring.

## Game-Type Routing

The eight routing items are `TF_GAMETYPE_CTF`, `TF_GAMETYPE_CP`, `TF_GAMETYPE_ESCORT`, `TF_GAMETYPE_ARENA`, `TF_GAMETYPE_MVM`, `TF_GAMETYPE_RD`, `TF_GAMETYPE_PASSTIME`, and `TF_GAMETYPE_PD`. They are stable TF2 routing identities, not ruleset implementations. `TF_GAMETYPE_UNDEFINED` and `TF_GAMETYPE_COUNT` are excluded sentinels.

## Generation Contract

The future generator must join SDK enums, FGD entities, runtime bindings, keyvalues, inputs, outputs, replicated fields, and objective-resource fields; preserve numeric order; assign every state transition either to TF2 primitive behavior or one exact ruleset owner; and fail on an unclassified objective entity, missing field, conflicting owner, duplicate numeric value, or item-count mismatch.
