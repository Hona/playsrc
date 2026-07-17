# `jump_beef` Visibility View Inventory

Source identity: configured `maps/jump_beef.bsp`, decoded SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`.

The immutable topology contains 284 clusters, 1,775 nodes, 1,899 leaves, eight areas, no referenced directed area portal, and no clip-portal vertices.

Immutable visibility-world identity: `174dcf0de59c4bf4684eaddb75be5ff493e7d7bbf4341914f07aecf38583e737`.

## Fixed camera

Camera `[5328,3376,-3067.2099609375]` resolves to leaf 11, cluster 0, area 1, with no sky flag.

- PVS clusters: `0,1,2,3,4,5,6,7,87,88,89`.
- Expanded 36-byte PVS row SHA-256: `d2a3d8b87ce57cd1130b4859eba3ee8953d38393335477fde747daca3c8feabc`.
- Front-to-back visible leaves: `11,15,21,24,18,29,27,31,317,321,323`.
- Ordered little-endian-u32 leaf-stream SHA-256: `eb4f6d6e2fa4fed5c82c94a53d606f292113a85ba2cf62041f4662b6c90a7206`.
- Deduplicated world-face candidates: 91.
- Ordered little-endian-u32 face-stream SHA-256: `41220f9df4d2ce5b1bad4b7da5aa6652807d107d5d650d4dbc361ab46f3a97d3`.
- Empty-candidate, area-revision-zero view-cache identity: `076d3192fc01dfe25335e56a9f73d14c0410be2ba437a35289748cc8c6295090`.

The water-volume bounds `[-5216,2304,-2416]..[-4448,3792,-2160]` enumerate leaves `652,659,660,661,662,663,668,671,674,675,882,883,885,886,894,900,911`. The ordered little-endian-u64 stream has SHA-256 `5b01a4ea18748108c1c795e614b5c3d262bc4a8ec9b7e38a6387221ecdcf8289`.

## Point candidates

| Point | Leaf | Cluster | Area | Fixed-camera candidate |
|---|---:|---:|---:|---|
| `[-4787,3137,-2159]` | 660 | 187 | 5 | No |
| `[12672,539,-2562]` | 90 | 27 | 2 | No |
| `[12672,683,-4448]` | 90 | 27 | 2 | No |
| `[0,0,0]` | 821 | -1 | 0 | No; an origin query here enters outside-world mode |

Candidate ordering is kind then immutable identity. Bounds-to-leaf linking, area-state revision, sorted unique origin clusters, outside/bypass mode, and candidate-set identity are cache-key inputs; renderer frusta, materials, LOD, and draw state are excluded.
