# VHV Format

`playsrc-vhv` parses canonical little-endian PC Source 1 hardware vertex-lighting files into immutable typed headers, ordered mesh streams, BGRA8 Source vertex-light colors, exact source ranges, and a SHA-256 source identity.

The package accepts one explicit `source-pc-v2-color-bgra8888` profile and a caller-supplied expected MDL checksum. Parsing is bounded by caller limits and rejects unsupported versions or vertex layouts, checksum disagreement, malformed ranges or counts, noncanonical reserved bytes, gaps, overlaps, padding, alpha, alignment, and trailing bytes.

The package does not resolve providers, associate `sp_<index>.vhv` names with map occurrences, parse MDL/VVD/VTX/BSP, select an LOD, interpret lighting for rendering, or create GPU resources.

Run the configured `pl_upward` inventory twice and require byte-identical output:

```sh
bun packages/formats/vhv/scripts/verify-configured.ts
```

The command writes metadata only to `sourceCacheDir/evidence/vhv/pl_upward-inventory.json`. It never writes Source asset bytes into the repository.

[`ROADMAP.md`](ROADMAP.md) defines the complete selected format family and evidence contract.
