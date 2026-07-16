# Format Packages

## Objective

Interpret Source 1 files and recorded streams as bounded typed data.

## Responsibilities

- Validate format identifiers, versions, offsets, counts, and limits.
- Preserve information required by later semantic modules.
- Report malformed, unsupported, and unknown input explicitly.

## Non-Responsibilities

- Gameplay, game-specific behavior, rendering, and product behavior.
- Finding logical resources across mounted content.
- Publishing compiled artifacts.

Each child is an independently useful package. This directory is only a navigational group.

## Packages

| Package | Exact responsibility |
|---|---|
| [`keyvalues/`](keyvalues/) | KeyValues keys, scalar values, nested objects, repeated keys, ordering, and format-level directives. |
| [`bsp/`](bsp/) | BSP headers, versions, lump directories, encoded lump records, and embedded map data. |
| [`vpk/`](vpk/) | VPK directory trees, segmented archives, entry metadata, integrity data, and exact entry reads. |
| [`vtf/`](vtf/) | VTF headers, resource tables, formats, frames, faces, slices, mip levels, and decoded pixels. |
| [`vmt/`](vmt/) | VMT shader names, parameters, proxies, references, and document-level composition. |
| [`studio-model/`](studio-model/) | Coordinated MDL, VVD, VTX, and ANI geometry, skeleton, sequence, animation, attachment, bodygroup, skin, LOD, and flex data. |
| [`phy/`](phy/) | PHY solids, collision geometry, constraints, and physical metadata. |
| [`demo/`](demo/) | Source DEM headers, commands, ticks, lengths, and encoded record payloads. |
