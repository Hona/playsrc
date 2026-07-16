# Packages

Reusable Source 1 modules live here. Every leaf package owns one mental model, presents an independently useful interface, and avoids game or product assumptions.

## Organization

| Group | Objective |
|---|---|
| [`formats/`](formats/) | Interpret Source files and recorded data. |
| [`world/`](world/) | Represent and query a runtime-neutral Source world. |
| [`runtime/`](runtime/) | Advance or transport authoritative and replay state. |
| [`presentation/`](presentation/) | Present world, gameplay, and replay state. |

[`content/`](content/) resolves raw Source content before parsing. [`asset-store/`](asset-store/) stores immutable compiled playsrc output after compilation.

Group directories organize packages but contain no implementation and expose no interface. Package names remain independent of their physical grouping.
