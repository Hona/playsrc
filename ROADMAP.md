# Roadmap

This roadmap compares observable Source/TF2 behavior with playsrc. No percentages are used until an authoritative denominator exists.

## Module Status

| Module | Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|---|
| Contracts | Complete domain contracts | Not started | — | Not started |
| Content | Mounted Source content lookup | Not started | — | Not started |
| BSP | Compiled map parsing | Not started | — | Not started |
| Materials | VMT/VTF semantics | Not started | — | Not started |
| Models | StudioModel semantics | Not started | — | Not started |
| Entities | Entity graph and I/O | Not started | — | Not started |
| Collision | Brush and physics collision | Not started | — | Not started |
| Visibility | BSP visibility and occlusion | Not started | — | Not started |
| Simulation | Deterministic gameplay primitives | Not started | — | Not started |
| Rendering | Source presentation | Not started | — | Not started |
| Pipeline | Repeatable compilation and deployment | Not started | — | Not started |

Detailed package, game, ruleset, and inventory roadmaps live under [`roadmap/`](roadmap/README.md).

## Global Completion

- [ ] Every required module defines its Source/TF2 comparison universe.
- [ ] Every encountered value is handled, inert, unsupported, malformed, missing, or unknown.
- [ ] Required unsupported and unknown behavior is eliminated for the declared game target.
- [ ] Gameplay, replay, and rendering authorities are integrated without duplication.
- [ ] Representative real content passes fair verification.
- [ ] Runtime and compilation performance are bounded and measured.
- [ ] TF2 game roadmap exit criteria are complete.
