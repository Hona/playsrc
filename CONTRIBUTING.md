# Contributing

## Scope

playsrc targets Source 1. TF2 is the first complete parity target. Counter-Strike: Source and legacy Source 1 CS:GO follow through their own game modules. Source 2 formats, behavior, terminology, and compatibility are out of scope.

## Find The Owner

Place work in the module that owns its mental model:

| Location | Ownership |
|---|---|
| `packages/formats` | Source file and recording formats |
| `packages/world` | Runtime-neutral world semantics and queries |
| `packages/runtime` | Movement, physics, simulation, networking, and replay |
| `packages/presentation` | Rendering, particles, and audio |
| `packages/content` | Raw Source logical-path resolution |
| `packages/asset-store` | Immutable compiled objects, roots, catalogs, and channels |
| `games/<game>` | Behavior belonging to one game |
| `games/<game>/rulesets` | Mode behavior belonging to that game |
| `apps` | Deployed product and network applications |
| `tools` | Developer and operator programs |
| `infra` | Hosting resources and environments |

Every module README defines its objective, responsibilities, non-responsibilities, relationships, and completion criteria. Update that definition when ownership changes.

Do not introduce game or product assumptions into generic packages. Do not extract shared behavior merely because two modes use the same name.

## Colocation

Keep schemas, fixtures, examples, generated inventories, and detailed roadmaps with the module that owns them. Use root `docs` only for knowledge that genuinely crosses several modules.

Direct runtime loading follows [`docs/direct-source-runtime.md`](docs/direct-source-runtime.md). Do not add a gameplay path that requires GLB, extracted VPK trees, application `public/` assets, or prior manual map conversion. Native and WASM callers use the same owning Rust compiler implementation and derived-cache identity.

Native/browser integration follows [`docs/native-wasm-contract.md`](docs/native-wasm-contract.md). A JavaScript/WASM call must perform one complete domain phase or bounded batch. Do not expose per-face, per-trace, per-entity, per-particle, or per-resource calls across that seam.

## Process

Work follows:

```text
RESEARCH -> TRACK -> IMPLEMENT
```

- Research the complete behavior family, prior playsrc work, public documentation, and the official Source SDK where applicable.
- Track the comprehensive behavior family in the owning roadmap before implementation.
- Implement the complete selected mental model rather than the narrowest visible symptom.
- Update every producer and consumer when changing an interface.
- Remove replaced, duplicated, fallback, and legacy paths in the same change.

The official Source SDK may be cited publicly. Code copied or adapted from it must preserve the applicable Source 1 SDK license, copyright, and required notices.

## Migration

Existing proof-of-concept code is migration material, not an interface to preserve.

- Copy isolated implementation that already fits its new owner.
- Adapt useful behavior hidden behind poor interfaces.
- Reimplement tangled application-specific or duplicate-authority paths.
- Discard compatibility paths, obsolete fallbacks, stale generated artifacts, and excessive test infrastructure.

There are no external consumers yet. Do not add compatibility readers, aliases, migrations, schema-version branches, or deprecated paths.

## Verification

- Add focused tests when parser, transform, serialization, or gameplay behavior is deterministic and reasonably testable.
- Use integration evidence for module seams.
- Use controlled captures or fair manual inspection for visual and experiential behavior.
- Do not require TDD, broad regression harnesses, brittle golden outputs, or implementation-detail assertions.
- Do not weaken valid evidence to make an implementation pass.

## Roadmaps

- Compare Source or game behavior with playsrc behavior explicitly.
- Check work only after implementation and fair evidence are complete.
- Do not claim percentages without a complete denominator.
- Generate large authoritative inventories rather than duplicating them manually.
- Add newly discovered required behavior to the owning checklist before calling a target complete.

## Commits

- Keep commits coherent by module or complete vertical behavior family.
- Do not combine unrelated cleanup with behavior changes.
- Document unresolved behavior as unsupported or unknown rather than hiding it.
- Regenerate stale artifacts after current contracts change.

## Terminology

The canonical project glossary is [TERMINOLOGY.md](TERMINOLOGY.md). Use its terms consistently in code, documentation, issues, reviews, and roadmaps.
