# `jump_beef` Entity Runtime Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

Source identity: configured `maps/jump_beef.bsp`, decoded SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`.

This fixed inventory is evidence input, not a generated denominator. Every count is over the ordered entity-lump records retained by Entity.

| Record | Count | Entity runtime disposition |
|---|---:|---|
| All entities | 361 | Allocate one ordered live slot/generation handle per accepted definition. |
| Ordered key/value pairs | 3,674 | Retain exact pair bytes and order. |
| Parsed output actions | 66 | Allocate stable action identities and reverse-declaration fire order. |
| Malformed output actions | 1 | Retain raw pair and malformed classification; never schedule it. |
| `func_button` | 5 | Generic button lock/toggle/mover state and output requests. |
| `func_door` | 4 | Generic door lock/open/close/wait/block/mover state and output requests. |
| `func_movelinear` | 1 | Generic endpoint/speed/position mover requests. |
| `func_brush` | 7 | Generic enabled/visible/solid state requests. |
| `logic_auto` | 1 | Selected load output followed by `OnMapSpawn`. |
| `trigger_multiple` | 22 | Filtered enter/stay/exit contacts, base touch outputs, and wait-gated `OnTrigger`. |
| `trigger_hurt` | 2 | Generic contacts and typed hurt requests; damage remains game-owned. |
| `trigger_teleport` | 56 | Generic contacts and typed teleport requests; destination application remains game-owned. |

| Output identity | Action count |
|---|---:|
| `OnDamaged` | 9 |
| `OnFullyClosed` | 1 |
| `OnFullyOpen` | 1 |
| `OnLoadGame` | 1 |
| `OnMapSpawn` | 4 |
| `OnStartTouch` | 51 |

The map contains no `logic_relay`, `math_counter`, `logic_case`, `trigger_push`, or `trigger_catapult` definition. Those generic primitives use fixed synthetic timelines because this map supplies no instance evidence.
