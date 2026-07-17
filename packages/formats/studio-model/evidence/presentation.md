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

Two artifact generations compare complete bytes and SHA-256. The fixed synthetic world artifact is 3,492 bytes with SHA-256 `f8cc817e20bfaba3c069d2cfd1d7cbd74564b18a56027d9c7425f452d0132613`. Decoding reconstructs every artifact field and canonical re-encoding must reproduce the input bytes exactly.

## Fixed TF2 matrix

The required public-build matrix is:

| Profile | Exact logical model inputs | Required state evidence |
|---|---|---|
| World player | `models/player/soldier.mdl`, `models/player/demo.mdl` | stand/idle, run, jump, crouch, primary fire; sequence/activity indexes, pose vectors, bone matrices, and attachments |
| World projectile | `models/weapons/w_models/w_rocket.mdl`, `models/weapons/w_models/w_stickybomb.mdl` | bind pose, skin/bodygroup/LOD selection, +X-forward basis, and attachment metadata |
| Viewmodel | `models/weapons/v_models/v_rocketlauncher_soldier.mdl`, `models/weapons/v_models/v_stickybomb_launcher_demo.mdl` | draw, idle, primary-fire recoil, return to idle, and reload timelines at the model-space matrix seam |

Each retained input requires logical path, byte length, SHA-256, provider identity, model dependency trace, selected VMT closure, typed VTF closure, artifact byte length, and artifact SHA-256. Run each artifact generation twice and require complete byte identity. Capture the named state timelines at fixed cycles `0`, `0.25`, `0.5`, `0.75`, and `1`; capture recoil return at every authored sequence boundary.

Configured TF2 closure previously stopped with `Malformed: InvalidIdentity in models/player/items/soldier/soldier_viking.mdl`. The focused regression now passes and the exact source-bundle command advances beyond that identity. It next stops with `Malformed: InvalidCount in models/player/soldier.mdl`; the fixed TF2 matrix therefore remains Blocked and no complete target timelines or parity result are claimed.
