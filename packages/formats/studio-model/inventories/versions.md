# StudioModel Version Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, the public MDL/ANI/VVD/VTX/VHV contracts, the candidate parent format inventory, and exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes. Configured TF2 public build `24207079` and its VPK indexes are available; the parent inventory is Not accepted and the CS:S/legacy CS:GO indexes remain Missing.

Generator command: Missing.

Candidate item count: 8. Accepted item count: 0.

All accepted profiles use little-endian PC fields. A caller selects one profile before parsing. An MDL profile does not select a VTX extension for the caller and does not imply that VVD, VTX, ANI, VHV, or PHY bytes exist.

| Profile identity | File identity and fixed header | Companion/version contract | Version-specific behavior | Declared-target authority |
|---|---|---|---|---|
| `mdl-ani-pc-v44` | Root `.mdl`: `IDST`, MDL version 44, 408-byte primary header. Optional `.ani`: `IDAG`, ANI version 44, 408-byte header. | Geometry-bearing MDL requires `vvd-pc-v4` and one `vtx-pc-v7` identity. External animation blocks require matching ANI. | SDK conversion treats pre-46 animation descriptors as the older index contract and treats a nonzero section table as incompatible. Pre-47 zero-frame fields are not accepted as animation data. | SDK historical conversion contract; exact TF2/CS:S/legacy-CS:GO occurrence is Missing. |
| `mdl-ani-pc-v45` | Root `.mdl`: `IDST`, MDL version 45, 408-byte primary header. Optional `.ani`: `IDAG`, ANI version 45, 408-byte header. | Geometry-bearing MDL requires `vvd-pc-v4` and one `vtx-pc-v7` identity. External animation blocks require matching ANI. | The SDK identifies sectioned version-45 ANI data as incompatible with current animation indexes and disables those descriptors during conversion. The package must retain and classify this condition; it cannot claim decoded animation parity until declared-content behavior is resolved. | SDK historical conversion contract; exact declared-content occurrence and required sectioned-ANI output are Missing. |
| `mdl-ani-pc-v46` | Root `.mdl`: `IDST`, MDL version 46, 408-byte primary header. Optional `.ani`: `IDAG`, ANI version 46, 408-byte header. | Geometry-bearing MDL requires `vvd-pc-v4` and one `vtx-pc-v7` identity. External animation blocks require matching ANI. | Animation section indexes use the post-45 contract. Pre-47 zero-frame fields are cleared by the SDK conversion contract. | SDK historical conversion contract; exact declared-content occurrence is Missing. |
| `mdl-ani-pc-v47` | Root `.mdl`: `IDST`, MDL version 47, 408-byte primary header. Optional `.ani`: `IDAG`, ANI version 47, 408-byte header. | Geometry-bearing MDL requires `vvd-pc-v4` and one `vtx-pc-v7` identity. External animation blocks require matching ANI. | Nonzero zero-frame indexes are identified as incompatible and cleared by the SDK conversion contract. | SDK historical conversion contract; exact declared-content occurrence is Missing. |
| `mdl-ani-pc-v48` | Root `.mdl`: `IDST`, MDL version 48, 408-byte primary header. Optional `.ani`: `IDAG`, ANI version 48, 408-byte header. | Geometry-bearing MDL requires `vvd-pc-v4` and one `vtx-pc-v7` identity. External animation blocks require matching ANI. | Current SDK layout includes fixed-point flex scale, optional secondary header, zero-frame records, source-bone transforms, linear bones, and bone-flex drivers. | Official SDK `STUDIO_VERSION == 48`; exact occurrence by declared content build is Missing. |
| `vvd-pc-v4` | `.vvd`: `IDSV`, version 4, 64-byte header. | Checksum equals MDL; 1 through 8 LOD counts; optional 12-byte fixups; 48-byte vertices; separate 16-byte tangents. | `IDCV` thin-vertex cache data is not accepted Source input. | Official SDK `MODEL_VERTEX_FILE_VERSION == 4`; exact occurrence by declared content build is Missing. |
| `vtx-pc-v7` | `.vtx`, `.dx80.vtx`, `.dx90.vtx`, or `.sw.vtx`: no magic, version 7, 36-byte packed header. | Checksum equals MDL; LOD/bodypart/model/mesh cardinalities match MDL and VVD. Extension identity is retained and selected explicitly. | Packed records use one-byte alignment and contain hardware limits, material replacements, strip groups, vertices, unsigned-16 indices, strips, and bone-state changes. | Official SDK `OPTIMIZED_MODEL_FILE_VERSION == 7`; exact extension occurrence by declared content build is Missing. |
| `vhv-pc-v2` | Map-owned `.vhv`: no magic, version 2, 40-byte packed header; each mesh header is 28 bytes. | Checksum equals the associated MDL supplied by the map owner. Accepted records use `VERTEX_COLOR`, four-byte BGRA stride, and 512-byte-aligned payload start/end. | The package preserves header/mesh records, padding, and exact BGRA bytes; rendering owns lighting use. Another flag/stride combination remains Unknown until a generated declared-content inventory assigns it. | Official SDK `VHV_VERSION == 2` and `src/utils/vrad/vradstaticprops.cpp`; exact declared-content occurrence is Missing. |

## Blocked And Excluded Identities

- Candidate legacy CS:GO VVC remains Blocked in the parent inventory. Its extension, identifier, version, checksum relation, record layout, package owner, and declared-build occurrence are unresolved; it is not an item in this candidate inventory.
- `.xbox.vtx`, `.360.mdl`, `.360.ani`, `.360.vvd`, `.360.vtx`, and `.360.vhv` are excluded because Xbox 360 is not a declared platform.
- `IDCV` thin VVD data is an internal runtime cache representation, not accepted Source content.
- PHY is owned by `packages/formats/phy`; its format versions are not StudioModel inventory items.
- GoldSrc MDL and Source 2 model/container identities are excluded categorically.
