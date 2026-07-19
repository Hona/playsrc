# Selected Source Map Foundation Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Generated; reviewed for the source-map-foundations checkpoint.

| Field | Value |
|---|---|
| SDK revision | `88fa198fba3fb85d46d4c95018254693fdc3af0a` |
| Configured BSP | `maps/jump_beef.bsp`, SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959` |
| Generator | `cargo run -p playsrc-entity --bin generate-source-map-inventory` |
| Selected behavior items | 22 |
| Configured class identities | 24 |

## Selected Generic Behaviors

| Stable identity | Exact selected contract | Evidence status |
|---|---|---|
| `field.typed-projection-and-writable-input` | SDK datamap key/input fields and variant conversion matrix. | Ready |
| `hierarchy.attachments` | SetParent, attachment snap/maintain-offset, ClearParent and live attachment transforms. | Ready |
| `template.point-template` | 16 expressions, captured relative transforms, prototypes, fixups and atomic ForceSpawn. | Ready |
| `logic.timer` | fixed/random interval, alternating output and complete timer inputs. | Ready |
| `trigger.contacts` | multiple/hurt/push/catapult/teleport accepted-contact state and typed effects. | Ready |
| `mover.func-button` | linear button endpoints, lock, wait, damage and output context. | Ready |
| `mover.func-rot-button` | angular button endpoints and block/carry request. | Ready |
| `mover.momentary-rot-button` | positioned angular endpoint and immediate/current transform. | Ready |
| `mover.func-door` | linear door endpoints, wait, lock, block and reversal. | Ready |
| `mover.func-door-rotating` | angular door endpoints, start position, wait, block and reversal. | Ready |
| `mover.func-movelinear` | position input, speed replacement and endpoint outputs. | Ready |
| `mover.func-rotating` | continuous angular speed, start/stop/toggle/reverse and current transform. | Ready |
| `mover.func-plat` | top/bottom endpoints, toggle, auto-return and block reversal. | Ready |
| `mover.func-platrot` | synchronized top/bottom translation and rotation. | Ready |
| `mover.func-train` | path-corner progression, wait/retrigger/teleport and block state. | Ready |
| `mover.func-tracktrain` | path-track links, direction/speed inputs, look-ahead segments and current transform. | Ready |
| `collision.pusher-current-transform` | linear/angular hierarchy carry, reverse-order block and whole-proposal rollback. | Ready |
| `collision.trigger-contact-producer` | moving brush/box trigger enter/stay/exit and swept crossing. | Ready |
| `lifecycle.func-breakable` | health inputs, normalized health output, break, nonsolid state and delayed removal. | Ready |
| `lifecycle.prop-dynamic` | draw/collision/animation request and completion state. | Ready |
| `lifecycle.ordinary-pickup` | cache interaction, game admission, touch output, respawn/materialize/remove. | Ready |
| `snapshot.selected-state` | version-5 Entity, version-2 pusher and version-1 contact continuation state. | Ready |

## Configured Class Occurrences

| Classname | Count | Owner disposition |
|---|---:|---|
| `func_brush` | 7 | Generic Entity |
| `func_button` | 5 | Generic Entity |
| `func_door` | 4 | Generic Entity |
| `func_movelinear` | 1 | Generic Entity |
| `func_regenerate` | 22 | Selected game |
| `func_respawnroom` | 3 | Selected game |
| `game_text` | 51 | Selected game/presentation |
| `info_observer_point` | 1 | Selected game/presentation |
| `info_player_teamspawn` | 10 | Selected game |
| `info_teleport_destination` | 25 | Generic Entity |
| `infodecal` | 39 | Map/presentation |
| `item_ammopack_full` | 3 | Selected game |
| `light` | 41 | Map/presentation |
| `light_environment` | 1 | Map/presentation |
| `light_spot` | 30 | Map/presentation |
| `logic_auto` | 1 | Generic Entity |
| `prop_dynamic` | 33 | Generic Entity |
| `team_round_timer` | 1 | Selected game |
| `tf_gamerules` | 1 | Selected game |
| `trigger_hurt` | 2 | Generic Entity |
| `trigger_multiple` | 22 | Generic Entity |
| `trigger_teleport` | 56 | Generic Entity |
| `water_lod_control` | 1 | Map/presentation |
| `worldspawn` | 1 | Generic Entity |

Configured totals: 361 entities, 3,674 ordered key/value pairs, 66 parsed output actions, and one malformed output action. No configured classname is selected by spelling: generic classes are fixed above; every other class remains selected-game, presentation/map, or intentionally inert input.
