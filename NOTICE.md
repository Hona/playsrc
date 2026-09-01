# Licensing Notice

## Original playsrc Material

The MIT License in [`LICENSE`](LICENSE) applies to original playsrc code and documentation unless a file or package states another license.

MIT permits commercial use, modification, distribution, sublicensing, and sale of the original playsrc material. The project's free-of-charge, non-commercial operation does not add a restriction to the MIT License.

## Valve Source 1 SDK Material

Code copied or adapted from Valve's official Source 1 SDK is not relicensed under MIT. It remains subject to the Source 1 SDK License, Valve's copyright notice, and every third-party notice required by that license.

Before a public checkpoint introduces copied or adapted Source 1 SDK code, that checkpoint must:

1. Identify every affected file or package.
2. Preserve the applicable Valve copyright and license notice.
3. Add the Source 1 SDK `LICENSE` and `thirdpartylegalnotices.txt` to the public repository.
4. Ensure distributed source and object-code forms satisfy the Source 1 SDK License, including its free-of-charge distribution requirement.

Independently authored code informed only by public contracts, documented formats, or observed behavior remains under the MIT License unless its file states another license.

The world construction and collision-policy portions of `games/tf2/rust/src/rigid_world.rs` and `games/tf2/rust/src/rigid_world/policy.rs`, projectile orchestration in `games/tf2/rust/src/rigid_projectiles.rs`, the `source_transform_components` extraction in `packages/formats/studio-model/rust/src/presentation.rs`, and the `clear_velocity_and_contact_strain` orchestration in `packages/runtime/physics/rust/src/world/contacts.rs`, adapt official Source SDK 2013 code. Their applicable terms and notices are retained in [`LICENSE.source-sdk-2013`](LICENSE.source-sdk-2013) and [`thirdpartylegalnotices.txt`](thirdpartylegalnotices.txt).

The reload-frame orchestration in `games/tf2/rust/src/weapon.rs` also adapts the official Source SDK 2013 weapon frame and reload-mode paths under the same SDK terms above.

The Demoman pipe-count and launcher-charge HUD orchestration in `games/tf2/browser/src/hud-integration/runtime.ts` and charge-progress computation in `games/tf2/rust/src/weapon.rs` adapt the official Source SDK 2013 under the same SDK terms above.

The Stickybomb Launcher charge sound and pullback activity lifecycle in `games/tf2/rust/src/lib.rs` and `games/tf2/rust/src/weapon.rs` also adapt the official Source SDK 2013 under the same SDK terms above.

## Trademarks And Affiliation

Source, Team Fortress 2, Counter-Strike, Steam, Valve, and their logos and trademarks belong to Valve Corporation or their respective owners.

playsrc is an independent fan project. It is not affiliated with, endorsed by, sponsored by, or approved by Valve Corporation.
