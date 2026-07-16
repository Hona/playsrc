# Visibility

## Objective

Determine potentially visible Source world state independently of a renderer.

## Responsibilities

- Represent BSP leaves, clusters, visibility sets, areas, portals, and occluders.
- Evaluate declared visibility state and map-provided visibility data.
- Supply bounded visibility results to presentation and networking consumers.

## Non-Responsibilities

- Drawing visible objects or owning GPU occlusion queries.
- Defining gameplay relevance or network replication policy.
- Parsing unrelated BSP semantics.

## Relationships

Consumes BSP visibility data and dynamic world state; supplies visibility decisions to rendering and runtime modules.

## Completion

Complete when the declared Source visibility families are represented and verified independently of presentation.
