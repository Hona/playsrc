# TF2 Items Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 `src/game/shared/econ/econ_item_schema.{h,cpp}`, `econ_item_system.{h,cpp}`, `econ_item_view.{h,cpp}`, `econ_entity.{h,cpp}`, and TF2 `tf_item_schema.{h,cpp}`, `tf_item_inventory.{h,cpp}`, `tf_item_constants.{h,cpp}`; configured logical path `scripts/items/items_game.txt`.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `10822003`; `scripts/items/items_game.txt` SHA-256 `47900e0d174971625a76625fe311a012910031171d0b121ff5f628078c83214d`; signature SHA-256 `2a9de0701878250a20329bf8bd2b974e54f19dd18ba709778736e4828f7daad6`.

Generator command: Missing

Bounded core-state generator: `cargo run --locked --manifest-path games/tf2/rust/inventory-generator/Cargo.toml`; its selected output is [`core-state.md`](core-state.md). It selects the stock item definitions, their complete recursive prefab closure, the 19 class positions, and the three account positions. All other candidate records remain visible here and unaccepted.

Output path: `games/tf2/inventories/items.md`

Candidate item count: 13,396

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

The candidate contains 13,374 direct schema records outside the separately owned `attributes` section plus 22 SDK loadout positions. A schema-record stable identity is `<top-level-section>:<direct-child-key>:<one-based-duplicate-occurrence>`. Ordered stable identities have SHA-256 `248e308f044f19b9b7b8897d6c479dcf94c7e7a48e21c014267a1a064cc1ac5a`.

## Schema Sections

| Section | Direct records | TF2-game disposition |
|---|---:|---|
| `game_info` | 8 | Parse class, account, slot, and preset bounds used by loadout validation |
| `qualities` | 16 | Preserve item quality identity consumed by item state and presentation mappings |
| `colors` | 22 | Preserve descriptive color identities; gameplay advancement is intentionally inert |
| `rarities` | 9 | Preserve rarity identity; gameplay advancement is intentionally inert |
| `equip_regions_list` | 31 | Build ordered equip-region bit assignments, including repeated `shared` records by occurrence |
| `equip_conflicts` | 2 | Build symmetric conflict masks before equip validation |
| `quest_objective_conditions` | 236 | Classify as product/account progression inputs; gameplay advancement is intentionally inert unless an Active product declares quest behavior |
| `item_series_types` | 2 | Preserve series identity; gameplay advancement is intentionally inert |
| `item_collections` | 66 | Preserve collection identity; gameplay advancement is intentionally inert |
| `operations` | 18 | Preserve operation identity; gameplay advancement is intentionally inert |
| `prefabs` | 226 | Resolve ordered prefab inheritance before item parsing |
| `items` | 11,490 | Resolve every definition, including the `default` template and 11,489 numeric definitions |
| `item_criteria_templates` | 2 | Preserve criteria identity; gameplay advancement is intentionally inert unless selected by a loadout rule |
| `random_attribute_templates` | 136 | Preserve template identity; random account-generation behavior is excluded |
| `lootlist_job_template_definitions` | 32 | Preserve identity; loot generation is excluded |
| `item_sets` | 73 | Resolve set membership and gameplay attributes before provider application |
| `client_loot_lists` | 233 | Preserve identity; loot selection is excluded |
| `revolving_loot_lists` | 182 | Preserve identity; loot rotation is excluded |
| `recipes` | 133 | Preserve identity; crafting is excluded |
| `achievement_rewards` | 68 | Preserve identity; award progression is excluded from the current target |
| `attribute_controlled_attached_particles` | 5 | Resolve TF2 effect mappings; Particle owns execution |
| `armory_data` | 3 | Preserve identity; store/armory behavior is excluded |
| `item_levels` | 14 | Preserve level-band identity; gameplay advancement is intentionally inert |
| `kill_eater_score_types` | 94 | Preserve score-type identity; account-stat persistence is excluded |
| `mvm_maps` | 7 | Route mode-only behavior to [`../rulesets/ROADMAP.md`](../rulesets/ROADMAP.md) |
| `mvm_tours` | 5 | Preserve identity; tour progression is excluded |
| `matchmaking_categories` | 5 | Preserve identity; matchmaking owns selection behavior |
| `maps` | 15 | Preserve schema map identity; map and ruleset selection are excluded |
| `master_maps_list` | 231 | Preserve identity; product catalog and matchmaking behavior are excluded |
| `steam_packages` | 5 | Preserve identity; package commerce is excluded |
| `string_lookups` | 2 | Resolve schema lookup aliases before dependent records |
| `community_market_item_remaps` | 2 | Preserve identity; market behavior is excluded |
| `war_definitions` | 1 | Preserve identity; account event progression is excluded |

The `attributes` section contains 843 records and belongs to [`attributes.md`](attributes.md).

## Item-Definition Identity Set

`item-definition.default` plus the numeric identities in these inclusive ranges form all 11,490 item definitions:

```text
0-30, 35-61, 94-111, 115-118, 120-148, 150-155, 158-167, 169-175, 177-185,
189-216, 219-234, 237, 239-255, 259-284, 286-292, 294-299, 302-319, 321-327,
329-349, 351, 354-365, 377-384, 386-395, 397-406, 408-417, 420, 422-427,
429-454, 457, 459-463, 465-468, 470-474, 477-486, 489-493, 496-528, 533-572,
574-623, 625-671, 673, 675, 680-699, 701-704, 707-709, 711-713, 717-722,
725-727, 729-741, 743-746, 751-821, 823-848, 850-859, 863-1035, 1037-1040,
1057-1124, 1126-1127, 1132-1146, 1149-1206, 1899-2026, 2028-2180, 2500-2571,
5000-5014, 5018, 5020-5023, 5026-5046, 5048-5057, 5060-5068, 5070-5087,
5500, 5600-5633, 5635-5661, 5700-5735, 5737-5781, 5783-5784, 5789-5814,
5816-5869, 5871-5873, 5875-5980, 5999-6013, 6015-6016, 6018-6026, 6028,
6032-6039, 6041-6042, 6048, 6051-6065, 6500, 6502-6524, 6526-6688, 8000-8996,
9029-9258, 9260-9999, 10002-10800, 10805-11155, 11171-11661, 11667-13980,
13982-15092, 15094-15158, 16102, 16104-16106, 16109, 16112-16114, 16120, 16122,
16130, 16139, 16143-16144, 16151, 16160-16161, 16163, 16300-16310, 16390-16391,
17200-17215, 17217-17218, 17220-17221, 17223-17226, 17228, 17230, 17232, 17234-17273,
17275-17287, 17289-17297, 17400-17442, 18000-18005, 18500-19174, 20000-20003,
20005-20009, 25000-25052, 30000-30087, 30089-30101, 30103-30110, 30112-30165,
30167-30173, 30175-30183, 30185-30187, 30189-30200, 30203-30208, 30211-30243,
30245, 30247-30249, 30251-30261, 30263-30270, 30273-30290, 30292-30369,
30371-30379, 30388-30431, 30467, 30469-30484, 30486-30536, 30538-30559, 30561,
30563-30564, 30567, 30569-30576, 30578, 30580-30584, 30586-30593, 30595-30607,
30609, 30614-30616, 30618, 30621, 30623, 30625-30629, 30631, 30633-30637, 30640,
30643-30655, 30658, 30661-30673, 30675-30676, 30680-30682, 30684-30686, 30693,
30698, 30700, 30704, 30706-30708, 30716-30724, 30726-30728, 30733, 30735-30759,
30761-30763, 30767-30771, 30773, 30775, 30777, 30779-30780, 30785-30786,
30788-30789, 30792-30801, 30803-30827, 30829-30831, 30833, 30835-30836,
30838-30840, 30842-30846, 30848-30849, 30853, 30856-30859, 30862-30863,
30866-30869, 30871-30923, 30928-30930, 30936-30937, 30939-30940, 30945,
30954-30955, 30958-30960, 30964, 30969, 30971-31157, 31160-31203, 31207-31233,
31236-31237, 31239, 31241-31286, 31288-31329, 31331-31349, 31351-31352,
31354-31489, 31491-31516, 31518-31547, 31549-31629
```

## Loadout Positions

The 22 loadout items are class positions `primary`, `secondary`, `melee`, `utility`, `building`, `pda`, `pda2`, `head`, `misc`, `action`, `misc2`, and `taunt` through `taunt8`, followed by account positions `account1`, `account2`, and `account3`. Numeric identities are the SDK enum values; invalid and count sentinels are excluded.

## Generation Contract

The future generator must parse the complete schema through the accepted KeyValues contract, retain source order and duplicate occurrence, apply prefab overlays in declared order, resolve every cross-reference, emit the 34 top-level section counts including the separately owned attribute handoff, classify every direct record, and fail on a signature/hash mismatch, missing section, duplicate numeric definition, unresolved prefab/item/attribute/region/set reference, malformed value, unknown gameplay field, or item-count mismatch.
