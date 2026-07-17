# StudioModel Presentation Evidence

## Automated conformance

Run `cargo test -p playsrc-studio-model --lib` from the repository root.

The fixed synthetic vectors prove:

- authored ANI identity selection, ANI hash retention, sequence animation grids, bone weights, animation sections, and integer-frame decoding;
- authored-depth-first include requests, include animation/sequence composition, root-bone remaps, companion closure, missing optional PHY disposition, and cycle rejection;
- ordered MDL material-directory candidates, VTX per-LOD replacement candidates, first-present VMT selection, caller-supplied VMT include closure, typed VTF requests, missing dependencies, and SHA-256 rejection;
- byte-identical `PSMP` world artifacts, distinct viewmodel artifacts, full decode/re-encode identity, content hash identity, trailing-byte rejection, and strict encoded-size limits below 64 MiB;
- bind/frame/pose-grid interpolation, shortest-path quaternion blending, ordered delta layers, parent matrix concatenation, skinning matrices, attachment transforms, skin/bodygroup/LOD material selection, Source +X/+Y/+Z model basis, and world/viewmodel activity indexes;
- `NotPresent` or `RetainedNotEvaluated` records for axis interpolation, quaternion interpolation, jiggle, aim-at-bone, aim-at-attachment, IK, flex, sequence autolayers, and unknown procedural types; malformed known procedural references fail before output;
- cancellation plus animation-sample and artifact bounds at the accepted boundary and one below it.
- nested include closure for `models/player/items/soldier/soldier_viking.mdl`, including an authored leading/trailing material directory, root-relative texture name, empty ANI name, block-zero directory, exact VVD/VTX/PHY `Load::Needs`, include chain, supplied companion composition, and canonical artifact round trip.
- the configured Soldier sequence-index-2 forward declaration at descriptor `24672..24884`: label `user_ref`, flags `STUDIO_OVERRIDE`, zero blends, `[0, 0]` blend dimensions, non-dereferenced empty-child offsets, and exact failing count range `24728..24732`; ordinary zero-blend and nonempty-child mutations remain Malformed.
- dense-frame ownership fallback: a fixed constant-channel vector fails under its dense retained size, fits under the unchanged limit as a tagged compact frame block, samples identically, round-trips canonically, and still fails one byte below its compact retained size;
- shared dependency accounting: repeated occurrences of one logical path/hash/length charge source bytes once while retaining every requester/material/role occurrence; a changed hash charges a distinct immutable source.

Two artifact generations compare complete bytes and SHA-256. The fixed synthetic world artifact is 3,492 bytes with SHA-256 `f8cc817e20bfaba3c069d2cfd1d7cbd74564b18a56027d9c7425f452d0132613`. Decoding reconstructs every artifact field and canonical re-encoding must reproduce the input bytes exactly.

## Fixed TF2 matrix

The required public-build matrix is:

| Profile | Exact logical model inputs | Required state evidence |
|---|---|---|
| World player | `models/player/soldier.mdl`, `models/player/demo.mdl` | stand/idle, run, jump, crouch, primary fire; sequence/activity indexes, pose vectors, bone matrices, and attachments |
| World projectile | `models/weapons/w_models/w_rocket.mdl`, `models/weapons/w_models/w_stickybomb.mdl` | bind pose, skin/bodygroup/LOD selection, +X-forward basis, and attachment metadata |
| Viewmodel | `models/weapons/v_models/v_rocketlauncher_soldier.mdl`, `models/weapons/v_models/v_stickybomb_launcher_demo.mdl` | draw, idle, primary-fire recoil, return to idle, and reload timelines at the model-space matrix seam |

Each retained input requires logical path, byte length, SHA-256, provider identity, model dependency trace, selected VMT closure, typed VTF closure, artifact byte length, and artifact SHA-256. Run each artifact generation twice and require complete byte identity. Capture the named state timelines at fixed cycles `0`, `0.25`, `0.5`, `0.75`, and `1`; capture recoil return at every authored sequence boundary.

Configured TF2 closure previously stopped first at the nested Soldier item identity and then at the empty `user_ref` forward declaration in `models/player/soldier.mdl`. Both focused regressions now pass. The exact command produces 130 entries, 44,132,628 bytes, and bundle SHA-256 `9777cb4e36ba0cb37e1142a203d3235c9096f5aab6f487b1e1f66dd72d79137b`. Complete target model-space timelines remain Missing, so no presentation parity result is claimed.

Complete configured player artifacts use the same `PSMP` version and ordinary limits. Soldier retains 934 animations, 41,033 logical frames, and 3,528,831 bone samples in a 41,423,542-byte artifact with SHA-256 `54801064af8c1d9559db236d640cfcf31821c50f8fffa5c0f45cfcbc9600ead0`. Demoman retains 852 animations, 39,476 logical frames, and 3,237,029 bone samples in a 39,322,364-byte artifact with SHA-256 `e254f6010dce2ef4df830ca4239fd73e548476fa497febd6df2b26a9dd903be6`. Two generations and complete decode/re-encode compare byte-identically for both; cycle `0.5` sampling of sequence 0 emits 86 Soldier and 84 Demoman skinning matrices.
