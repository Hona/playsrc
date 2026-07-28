# Format Packages

## Objective

Interpret the finite Source 1 format universe required by Team Fortress 2, Counter-Strike: Source, legacy Source 1 Counter-Strike: Global Offensive, map compilation, gameplay, replay, rendering, particles, audio, and declared tools as bounded typed data.

## Responsibilities

- Validate format identifiers, versions, offsets, counts, and limits.
- Preserve information required by later semantic modules.
- Report malformed, unsupported, and unknown input explicitly.
- Maintain one format owner for every accepted format identity.

## Non-Responsibilities

- Gameplay, game-specific behavior, rendering, and product behavior.
- Finding logical resources across mounted content.
- Publishing compiled artifacts.

Each child is an independently useful package. This directory is only a navigational group.

[`ROADMAP.md`](ROADMAP.md) owns the format-universe denominator and package assignment. [`inventories/formats.md`](inventories/formats.md) is the current 62-item candidate inventory. It is not accepted because five format decisions lack authority, no checked-in generator exists, and 18 proposed leaf owners are absent from the root Owner Registry.

## Packages

| Package | Exact responsibility |
|---|---|
| [`keyvalues/`](keyvalues/) | KeyValues keys, scalar values, nested objects, repeated keys, ordering, and format-level directives. |
| [`bsp/`](bsp/) | BSP headers, versions, lump directories, encoded lump records, and embedded map data. |
| [`vpk/`](vpk/) | VPK directory trees, segmented archives, entry metadata, integrity data, and exact entry reads. |
| [`vtf/`](vtf/) | VTF headers, resource tables, formats, frames, faces, slices, mip levels, and decoded pixels. |
| [`vmt/`](vmt/) | VMT shader names, parameters, proxies, references, and document-level composition. |
| [`studio-model/`](studio-model/) | Coordinated MDL, VVD, VTX, and ANI geometry, skeleton, sequence, animation, attachment, bodygroup, skin, LOD, and flex data. |
| [`vhv/`](vhv/) | VHV hardware vertex-light headers, ordered mesh streams, Source vertex-light BGRA8 records, ranges, checksums, and source identities. |
| [`phy/`](phy/) | PHY solids, collision geometry, constraints, and physical metadata. |
| [`demo/`](demo/) | Source DEM headers, commands, ticks, lengths, and encoded record payloads. |

These nine package paths are the current public leaves. Proposed leaves remain roadmap decisions until the root Owner Registry accepts them; no proposed package exists by implication.
