# World Packages

## Objective

Represent and query a runtime-neutral Source 1 world after format parsing.

## Responsibilities

- Own semantic map, material, entity, collision, and visibility behavior.
- Provide representations usable by compilation, gameplay, replay, and presentation.
- Keep game-specific behavior in `games`.

## Non-Responsibilities

- Parsing unrelated Source file containers.
- Owning gameplay authority or GPU presentation.

Each child is an independently useful package. This directory is only a navigational group.

## Packages

| Package | Exact responsibility |
|---|---|
| [`map/`](map/) | Map-wide references and assembly across geometry, lighting, materials, models, entities, collision, visibility, and environment data. |
| [`material/`](material/) | Resolved shaders, parameters, flags, proxies, animation, texture references, and runtime-neutral material state. |
| [`entity/`](entity/) | Entity identities, keyvalues, references, parenting, ordered outputs, inputs, lifecycle, and generic state transitions. |
| [`collision/`](collision/) | World, brush, prop, trigger, and model shapes plus contents tests, ray traces, hull sweeps, overlaps, and contacts. |
| [`visibility/`](visibility/) | BSP leaves, clusters, PVS, areas, areaportals, occluders, and dynamic visibility state. |
