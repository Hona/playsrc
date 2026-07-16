# Roadmap

The roadmap will define the finite checklists whose completion establishes each declared playsrc target. No percentages are used without complete denominators.

## Completion Contract

A declared target is complete only when:

- Every required behavior belongs to an owning module checklist.
- Every required checklist item is implemented through all necessary producers and consumers.
- Every completed item has credible evidence.
- No required behavior remains unsupported or unknown.
- Cross-module integration and target exit criteria are complete.

Newly discovered required behavior is added to the owning checklist before completion is claimed.

## Current State

No implementation has been migrated into the new architecture. Detailed roadmaps and generated inventories will be colocated with their owning packages, games, rulesets, applications, tools, or infrastructure modules.

| Owner | Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|---|
| [Format packages](packages/formats/) | Required Source file and recording formats | Not started | - | Not started |
| [World packages](packages/world/) | Canonical maps, materials, entities, collision, and visibility | Not started | - | Not started |
| [Runtime packages](packages/runtime/) | Movement, physics, simulation, networking, and replay | Not started | - | Not started |
| [Presentation packages](packages/presentation/) | Rendering, particles, and audio | Not started | - | Not started |
| [Content](packages/content/) | Exact mounted Source content resolution | Not started | - | Not started |
| [Asset store](packages/asset-store/) | Immutable objects, roots, catalogs, channels, and publication | Not started | - | Not started |
| [TF2](games/tf2/) | Complete Team Fortress 2 behavior | Not started | - | Not started |
| [TF2 jump](games/tf2/rulesets/jump/) | Complete TF2 jump rules | Not started | - | Not started |
| [Web applications](apps/web/) | Declared browser products | Not started | - | Not started |
| [Service applications](apps/services/) | Declared network applications | Not started | - | Not started |
| [Tools](tools/) | Repeatable operation and inspection | Not started | - | Not started |
| [Infrastructure](infra/) | Declared hosting resources and environments | Not started | - | Not started |

The next roadmap pass will establish the complete behavior families and generated inventories under each owner before implementation migration begins.
