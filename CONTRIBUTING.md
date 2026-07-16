# Contributing

## Scope

playsrc targets Source 1. TF2 is the first complete parity target. Source 2 changes are out of scope.

## Package Boundaries

- Put behavior in the package that owns the Source domain.
- Keep packages independently useful.
- Do not introduce product assumptions into format, content, map, model, material, physics, or simulation packages.
- Keep game-specific behavior in game adapters and mode-specific behavior in rulesets.
- Update every producer and consumer when changing a contract.

## Process

Work follows:

```text
RESEARCH -> TRACK -> IMPLEMENT
```

- Research the complete behavior family and relevant prior art.
- Track the comprehensive work in the owning roadmap before implementation.
- Implement the complete subsystem or category rather than the narrowest visible patch.
- Remove replaced, duplicated, fallback, and legacy paths in the same change.

## Breaking Changes

There are no external consumers yet. Breaking changes are preferred over compatibility code. Maintain one current contract, update all callers together, and regenerate stale development artifacts.

## Verification

- Add unit tests when behavior is deterministic and reasonably testable.
- Use integration evidence for package and subsystem boundaries.
- Use controlled captures or fair manual inspection for visual behavior.
- Do not force TDD, brittle tests, or implementation-detail assertions.
- Do not weaken valid evidence to make new code pass.

## Roadmaps

- Update the owning package roadmap with every implementation.
- Compare Source/TF2 behavior with playsrc behavior explicitly.
- Do not claim percentages without a complete denominator.
- Large universes should use generated inventories rather than manually duplicated lists.

## Commits

- Keep commits coherent by domain or complete vertical slice.
- Do not combine unrelated cleanup with behavior changes.
- Document unresolved behavior as unsupported or unknown rather than hiding it.

## Terminology

The canonical project glossary is [TERMINOLOGY.md](TERMINOLOGY.md). Use its terms consistently in code, documentation, issues, reviews, and roadmaps.
