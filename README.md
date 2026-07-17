# playsrc

`playsrc` is a clean-room port of the Source 1 engine and its games to the web.

- Play Team Fortress 2 directly in your browser (soon CS:S, CS:GO, and maybe more).
- Use powerful, reusable tools to inspect, parse, compile, simulate, and present Source 1 content.
- Load declared BSP maps directly in a browser worker against exact BSP PAK and VPK content; prior GLB conversion is not required.
- Compose profile-selected HDR/LDR maps, TF2 Jump gameplay, bounded StudioModel artifacts, worker-executed PCFs, typed world-environment records, and Rust PVS draw candidates in the TF2 browser application.

TF2 is the first complete parity target. Counter-Strike: Source and legacy Source 1 Counter-Strike: Global Offensive follow through their own game modules. Source 2 is explicitly out of scope (for now..?!).

## Structure

```text
packages/     independently useful Source modules
games/        game behavior and game-owned rulesets
apps/         deployed web and service applications
tools/        developer and operator programs
infra/        hosting resources and environments
docs/         cross-cutting public documentation
```

Packages are grouped by exact responsibility:

| Area | Included behavior |
|---|---|
| `formats/` | KeyValues; BSP compiled maps; VPK archives; VTF textures; VMT material documents; StudioModel MDL, VVD, VTX, and ANI files; PHY physics assets; and Source DEM recordings. |
| `world/` | Canonical map assembly; resolved material semantics; entity identity, parenting, and ordered I/O; collision shapes, contents, ray traces, hull sweeps, and overlaps; BSP leaves, clusters, PVS, areas, portals, and occluders. |
| `runtime/` | Ground, air, water, ladder, crouch, jump, and stair movement; rigid bodies and constraints; deterministic ticks, commands, events, and snapshots; multiplayer replication and reconciliation; replay timelines and seeking. |
| `presentation/` | Browser GPU scenes, views, lighting, and frame presentation; Source particle definitions and simulation; sound scripts, channels, spatialization, mixing, and browser audio playback; Source VGUI panels, controls, schemes, localization binding, HUD animations, focus, input, and DOM/CSS presentation. |
| `content/` | Exact logical-path lookup across configured directories, VPK providers, BSP PAK providers, and reusable raw-source cache entries with declared mount precedence and provenance. |
| `asset-store/` | SHA-256-addressed immutable raw Source and derived objects; source, map-runtime, game, and application descriptors; catalogs; mutable channels; reachability validation; local storage; and remote synchronization. |

Every module README defines its objective, responsibilities, non-responsibilities, relationships, and completion criteria before implementation begins.

## Documents

- [Architecture](ARCHITECTURE.md)
- [Principles](PRINCIPLES.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Terminology](TERMINOLOGY.md)
- [Direct Source Runtime](docs/direct-source-runtime.md)
- [Native And WASM Contract](docs/native-wasm-contract.md)

## Development

The root Bun workspace validates `playsrc.local.json` before command dispatch. Commands resolve the repository independently of the caller's working directory:

```bash
bun run dev jump_beef
```

The Cargo workspace contains Rust crates inside their owning modules. Run `bun run setup` to install and verify the checked Rust toolchain under `sourceCacheDir` before building them.

## Scope

- Reusable Source 1 format, world, runtime, and presentation packages.
- Complete TF2 game behavior and game-owned rulesets, beginning with TF2 jump.
- Future CS:S and legacy Source 1 CS:GO games with their own rulesets.
- Browser products, future online multiplayer services, tools, asset publication, and infrastructure.
- Direct browser loading of declared BSP files and official VPK files through bounded WASM workers and exact ranged content providers.

Applications assemble modules. They do not reimplement Source, game, or ruleset behavior.

## Acknowledgements

Source, Team Fortress 2, Counter-Strike, and the Source SDK were created by Valve Corporation. playsrc is an independent fan project and is not affiliated with, endorsed by, sponsored by, or approved by Valve Corporation.

playsrc is developed and distributed free of charge as a non-commercial passion project.

Valve, Source, Team Fortress, Counter-Strike, Steam, and related names and trademarks belong to their respective owners.

[`bsp-to-glb`](https://github.com/Hona/bsp-to-glb) remains a great standalone tool for exporting Source BSP maps to GLB. playsrc does not require GLB for gameplay.

## License

Original playsrc code and documentation are available under the [MIT License](LICENSE). Code copied or adapted from Valve's Source 1 SDK remains under the Source 1 SDK License and its required notices. [`NOTICE.md`](NOTICE.md) defines the exact license boundary.
