# Roadmap Contract

This document is normative for every playsrc roadmap. [`../TERMINOLOGY.md`](../TERMINOLOGY.md) defines roadmap item, completion denominator, exit criteria, delivery statuses, coverage classifications, evidence, inventory, current contract, and Complete. This document links those definitions and does not replace them.

## Leaf Roadmap Schema

Every behavior-owning roadmap contains these sections in this order:

```markdown
# <Owner> Roadmap

## Completion Denominator
## Inputs
## Outputs
## Invariants
## Ownership Exclusions
## Behavior Families
## Generated Inventories
## Exit Criteria
## Blockers
```

Every behavior row uses exactly this table shape:

```markdown
| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
```

`Status` contains exactly one delivery status defined by [`TERMINOLOGY.md`](../TERMINOLOGY.md): `Not started`, `In progress`, `Blocked`, or `Ready`. Coverage classifications are recorded in behavior evidence or generated inventories and never replace delivery status.

The completion denominator is the union of the roadmap's behavior rows and every item in an accepted generated inventory named by those rows. Prose goals, examples, PoC migration tasks, and unaccepted inventories are not denominator items.

## Aggregation Roadmaps

An aggregation roadmap names child roadmaps and cross-child exit criteria. It does not restate or own leaf behavior.

An aggregate target derives its status in this order:

1. `Blocked` when the next required action is unavailable for the exact reason recorded by the aggregate or any required child.
2. `Ready` when every required child is Complete and every aggregate exit criterion passes.
3. `In progress` when at least one required child or aggregate exit criterion has begun and no blocker prevents the next action.
4. `Not started` otherwise.

Aggregate status is never calculated from counts, ratios, averages, weights, points, or percentages. One non-Ready required child prevents aggregate readiness.

## Ready Evidence

Every Ready behavior row links evidence that records all applicable fields below. A field is inapplicable only when the evidence record names the field and explains why it cannot affect the predicate.

| Field | Required record |
|---|---|
| Claim | Exact roadmap row and observable predicate established by the evidence |
| Target authority | Authority name, stable identity, revision or content build, and exact target inputs |
| playsrc identity | Repository commit and the current interface or artifact identity under test |
| Inputs | Logical identities, provenance, and content hashes for every consumed fixed input |
| Configuration | Build flags, runtime configuration, numeric bounds, and feature selections affecting output |
| Environment | Operating system, CPU architecture, runtime version, and browser, GPU, and driver when they affect output |
| Procedure | Checked-in command or fixed manual procedure and the ordered operations performed |
| Comparison | Compared target and playsrc outputs, comparison method, acceptance predicate, and numeric tolerance when equality is not exact |
| Result | Actual observed values, pass or fail result, execution time, and evidence artifact location |
| Integration | Required producers and consumers exercised through the current interface |

Evidence for a manual inspection or capture also includes every field required by the corresponding [`TERMINOLOGY.md`](../TERMINOLOGY.md) definition. A test name without retained inputs, procedure, comparison predicate, and result is not Ready evidence.

## Generated Inventories

A generated inventory records these mandatory metadata fields:

| Field | Contract |
|---|---|
| Authority identity | Exact upstream schema, manifest, registry, archive index, content build, or public contract enumerated |
| Authority revision | Immutable revision, content-build identity, content hash, or `Unknown` when the authority exposes no revision |
| Generator command | One checked-in stable command that regenerates the inventory from declared configuration |
| Output path | Repository-relative path of the generated inventory |
| Item count | Exact number of denominator items emitted by that output |

The generated output also records its owning roadmap and one stable identity per item. Generation fails rather than silently omitting malformed, unknown, missing, or unsupported entries. Those entries remain in the inventory with the coverage classification defined by [`TERMINOLOGY.md`](../TERMINOLOGY.md).

An inventory is current only when its recorded authority revision, generator command, and output agree with the repository state. A stale artifact cannot satisfy a denominator. Hand-edited generated output is invalid and must be regenerated.

## Denominator Review Gate

A denominator is Accepted only when one review record in its roadmap records `Accepted`, reviewer identity, review date, reviewed commit, and a passing result for every predicate below:

1. **Finite authority:** Every row is individually enumerated, or belongs to a generated inventory with all mandatory metadata and an exact item count.
2. **Single ownership:** Every item names one behavior owner; every adjacent behavior names its different owner under Ownership Exclusions.
3. **Boundary-complete contract:** Inputs, outputs, state, ordering, errors, invariants, limits, and exclusions required to observe each item are explicit.
4. **Authority coverage:** The roadmap names every authority checked, its identity or revision, and every unresolved conflict.
5. **Evidence feasibility:** Every row names a comparison method capable of establishing its observable predicate.
6. **Integration closure:** Every required producer, consumer, application, game, ruleset, tool, service, and infrastructure dependency appears in the roadmap or a named child roadmap.
7. **Unknown closure:** Every unresolved owner, behavior, input, dependency, decision, or evidence method is a Blocked item naming every source already checked.
8. **Current-contract consistency:** No row requires a fallback, legacy path, compatibility layer, interface-version branch, or stale generated artifact.

`Draft`, absence of a review record, or any failed predicate means Not accepted. A Not accepted child prevents aggregate readiness; the aggregate delivery status still derives exclusively from the rules in Aggregation Roadmaps.

## Requirement Changes

A newly discovered required behavior is added immediately to its sole owning roadmap. The same change updates affected inputs, outputs, invariants, exclusions, inventories, exit criteria, producers, consumers, and aggregate dependencies. Its initial delivery status follows [`TERMINOLOGY.md`](../TERMINOLOGY.md).

The repository keeps one current denominator. Git history preserves prior contracts; the working tree does not retain versioned roadmap branches, aliases, old readers, or compatibility entries.

An ownership conflict blocks every affected item until the root Owner Registry and both affected roadmap exclusions identify one owner. Moving ownership removes the item from the prior owner and adds it to the new owner in the same checkpoint.

## Migration Separation

PoC locations, reusable code, omissions, and `Copy`, `Adapt`, `Reimplement`, or `Discard` decisions are migration coordination data. They do not appear in public behavior rows, count as implementation, alter delivery status, or define the target denominator.
