# TF2 Attributes Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: configured `scripts/items/items_game.txt` `attributes` records; Valve Source SDK 2013 `src/game/shared/econ/attribute_manager.{h,cpp}`, `econ_item_schema.{h,cpp}`, `econ_item_view.{h,cpp}`, TF2 shared/server/client attribute-hook consumers, and TF2 item/weapon/player/building providers.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 content build `10822003`; item-schema SHA-256 `47900e0d174971625a76625fe311a012910031171d0b121ff5f628078c83214d`.

Generator command: Missing

Bounded core-state generator: `cargo run --locked --manifest-path games/tf2/rust/inventory-generator/Cargo.toml`; its selected output is [`core-state.md`](core-state.md). It selects only definitions and hook identities consumed by bounded class health, healing, damage, crit, resistance, and pickup behavior. The remaining definitions and hooks remain visible here and unaccepted.

Output path: `games/tf2/inventories/attributes.md`

Candidate item count: 1,346

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

The candidate contains 843 schema definitions and 503 distinct TF2 hook identities extracted from 757 hook call sites. Definition stable identity is `attribute-definition:<numeric-key>`. Hook stable identity is `attribute-hook:<exact-stringized-hook-name>`. The sorted hook-name set has SHA-256 `8e9075f829043bc1e8e1d21ecbb54d7fc3e383e6d38c79fcba0809c4498f49ed`.

## Definition Identity Set

These inclusive ranges enumerate all 843 definition keys:

```text
1-28, 30-209, 211-212, 214-253, 255-289, 292-315, 317-341, 343-422, 424-431,
433-479, 481-482, 484, 488-501, 503-522, 524-528, 532-537, 539-540, 542-551,
554, 556-557, 600, 602, 606-610, 612-622, 630, 632-634, 636-647, 651, 661-662,
669-671, 674-676, 681, 684, 687-696, 698-705, 708-712, 719, 723-754, 760, 762,
772-801, 804-835, 837-848, 851-856, 859-863, 865-881, 1000-1009, 1030,
2000-2032, 2034-2046, 2048-2059, 2062-2079, 3000-3018
```

Every definition must retain numeric identity, name, attribute class, storage type, description format, effect type, hidden state, networking/export flags, default value where declared, and required gameplay disposition. A definition with no direct hook remains an inventory item and receives `Intentionally inert`, a data-driven consumer, or an exact excluded owner; it is never silently omitted.

## Application Order Contract

For each of the 503 hook identities, the generated item must record all SDK call sites and establish this observable order:

1. Validate the hook, initiator, provider graph, item list, and value type.
2. Apply the queried entity's own static and dynamic attributes in stored iteration order using the definition's combination rule.
3. Traverse eligible providers in provider order, suppress weapon-to-weapon loopback, and apply each provider once.
4. Traverse the eligible owner only after local/provider application.
5. Preserve integer conversion, float bit interpretation, string replacement, item-list reporting, cache lookup, cache invalidation, and provision-parity behavior.
6. Publish one final typed value or a classified failure; a failed query cannot publish a partially transformed value.

## Generation Contract

The future generator must parse all schema definitions and every `CALL_ATTRIB_HOOK_*` use under TF2 shared, server, and client code; normalize macro-expanded names without case folding; join hook names to schema attribute classes and provider roles; emit definition and hook records in numeric/name order; and fail on a duplicate key, duplicate name with conflicting type, unresolved hook, unknown combination rule, provider cycle, malformed value, missing content input, or item-count mismatch.
