# `jump_beef` Brush Collision Evidence

Source identity: configured `maps/jump_beef.bsp`, SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`, Source-2013 BSP version 20, revision 731.

Collision-world SHA-256: `66d42c750648487669e1b9d7a1b36fc81e213624030f812667fb728ee61aa6ed` under domain `playsrc-collision-world-v2`.

## Complete source counts

- 476 BSP brushes and 123 BSP models.
- Model 0 owns 339 world brushes.
- Models 1 through 122 each have exactly one map entity occurrence and at least one collision brush. No inline model is unreferenced.
- Model 122 has zero render faces and collision brush 475 with contents `SOLID`.
- Occurrence classes are 56 `trigger_teleport`, 22 `trigger_multiple`, 22 `func_regenerate`, seven `func_brush`, five `func_button`, four `func_door`, three `func_respawnroom`, two `trigger_hurt`, and one `func_movelinear`.

## Ordinary solid divider and alpha fences

| Entity/model | Source class/state | Collision brushes | ORed contents | Initial ordinary-solid admission |
|---|---|---|---|---|
| `294/*109` | `func_brush`, `StartDisabled=0`, `Solidity=0` | 454 | `0x00000001` | enabled |
| `295/*110` | `func_brush`, `StartDisabled=0`, `Solidity=0` | 455 | `0x00000001` | enabled |
| `296/*111` | `func_brush`, `StartDisabled=0`, `Solidity=0` | 456 | `0x00000001` | enabled |
| `297/*112` | `func_brush`, `StartDisabled=0`, `Solidity=0` | 457 | `0x00000001` | enabled |
| `307/*113` | `func_brush`, `StartDisabled=0`, `Solidity=1` | 458 | `0x10000008` (`TRANSLUCENT|GRATE`) | disabled by explicit never-solid state |
| `322/*117` | `func_brush`, `StartDisabled=0`, `Solidity=1` | 468 | `0x10000008` (`TRANSLUCENT|GRATE`) | disabled by explicit never-solid state |
| `323/*118` | `func_brush`, `StartDisabled=0`, `Solidity=1` | 469 | `0x10000008` (`TRANSLUCENT|GRATE`) | disabled by explicit never-solid state |

Model 109's source transform is origin `[5300,1244,-2662.3]`, zero angles; local bounds are `[-1436,-4,-961.69995]..[1436,4,961.7001]`. Fixed `MASK_PLAYERSOLID` sweeps from Y 1100 to 1400 at X 5300/Z -2662.3 hit entity 294/model 109 for both TF2 hulls `[-24,-24,0]..[24,24,82]` and `[-24,-24,0]..[24,24,62]`. The result uses brush 454; no render triangle, texture alpha, or fallback box participates.

Configured snapshot evidence retains all seven records above in source order, binds the collision-world identity and revision `0x4d41505f42525553`, and retains the three never-solid alpha-fence records with `enabled=false` rather than deleting their source contents.
