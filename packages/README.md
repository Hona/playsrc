# Packages

Reusable Source 1 modules live here. Every leaf package owns one mental model, presents an independently useful interface, and avoids game or product assumptions.

## Organization

| Group | Packages and exact scope |
|---|---|
| [`formats/`](formats/) | KeyValues, BSP, VPK, VTF, VMT, StudioModel, PHY, and DEM parsing. |
| [`world/`](world/) | Map assembly, material semantics, entity graphs and I/O, collision queries, and BSP visibility. |
| [`runtime/`](runtime/) | Player movement, rigid-body physics, deterministic simulation, multiplayer networking, and replay state. |
| [`presentation/`](presentation/) | Browser GPU rendering, Source particle behavior, and Source sound presentation. |

[`content/`](content/) resolves raw Source content before parsing. [`asset-store/`](asset-store/) stores immutable compiled playsrc output after compilation.

Group directories organize packages but contain no implementation and expose no interface. Package names remain independent of their physical grouping.
