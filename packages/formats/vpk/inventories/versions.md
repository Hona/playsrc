# VPK Version And Archive-Layout Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

## Metadata

| Field | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `src/public/vpklib/packedstore.h`; Valve Developer Community `VPK (file format)` revision `496391`; exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes and directory-file bytes |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; VDC oldid `496391`; configured TF2 product version `10822003`; CS:S and legacy CS:GO target revisions are Missing |
| Generator command | Missing |
| Output path | `packages/formats/vpk/inventories/versions.md` |
| Candidate item count | 7 |
| Accepted item count | 0 |
| Owner | `packages/formats/vpk` |

Candidate outcome counts: 2 Required, 2 Excluded, and 3 Blocked.

## Decisions

| Identity | Scope | Header or layout contract | Authority | Outcome |
|---|---|---|---|---|
| `source1-headerless` | VPK directory bytes without `0x55AA1234` and a version field | Historical tree-first layout with no versioned header | VDC `VPK (file format)` oldid `496391`; the candidate format-universe row enumerates only versions 1 and 2 | Excluded: no declared target content build requires the headerless layout |
| `source1-v1` | Signature `0x55AA1234`, version 1 | 12-byte header, exact `TreeSize`, then tree and directory-contained data; no version 2 integrity or signature sections | VDC oldid `496391`; Source SDK public VPK interface | Required, pending format-universe acceptance |
| `source1-v2` | Signature `0x55AA1234`, version 2 | 28-byte header; exact tree, file-data, archive-MD5, other-MD5, and signature section lengths; sections may be absent only through their accepted zero-length form | VDC oldid `496391`; Source SDK `packedstore.h` hash and signature interface | Required, pending format-universe acceptance |
| `source2-v3` | Source 2 VPK version 3 and Source 2 archive/resource behavior | Different version and resource ecosystem | [`../../../../TERMINOLOGY.md`](../../../../TERMINOLOGY.md) Source 2 exclusion | Excluded: Source 2 never enters playsrc |
| `tf2-layout` | Configured TF2 product version `10822003` | The configured TF root contains four exact split v2 families: `tf2_misc` has 102,288 entries, segments 000–027, and directory SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; `tf2_textures` has 29,210 entries, segments 000–107, and directory SHA-256 `291719bce05f0d82e6fb20961e631c0dd3967a7fe5b11cb374ed56c25312337e`; `tf2_sound_misc` has 3,230 entries, segments 000–011, and directory SHA-256 `dfbcc92beb6e9dd86994ad37be8bc7d8d7da66d01d2f4d441accdab776894bd9`; `tf2_sound_vo_english` has 12,728 entries, segments 000–002, and directory SHA-256 `f9b0518925cd7b7b3b4373214e8fc9b8552a631bbe43661d8ff111d1fc539076`. Every family has zero preload and embedded entries, a 48-byte other-MD5 section, a 296-byte signature section, verified self-MD5 values, and a verified signature. Shared HL2/platform roots and `tf2_lv` are not present beneath the configured root. | Exact configured `steam.inf`, directory-file bytes, VPK parser output, and SHA-256 values; Source SDK `game/mod_tf/gameinfo.txt` defines the broader candidate search paths | Blocked: the format-universe decision and generator are not accepted, and the configured contract cannot resolve shared HL2/platform families |
| `css-layout` | One selected CS:S content build | Candidate split families are `cstrike_<language>`, `cstrike_pak`, shared `hl2_*`, and `platform_misc`; public depot manifest `6941588918651947824` records `cstrike_pak_000.vpk` through `cstrike_pak_012.vpk` plus `cstrike_pak_dir.vpk`, but the selected target families, hashes, versions, and section profiles must come from configured bytes | Public CS:S gameinfo contract and depot 241 manifest `6941588918651947824`; exact configured target index is Missing | Blocked: no accepted CS:S content build or local root contract |
| `legacy-csgo-layout` | One selected legacy Source 1 CS:GO content build | `csgo/pak01_dir.vpk` plus its referenced `pak01_<index>.vpk` segments is a candidate family; regional, language, standalone, version, section-profile, segment-index, and hash decisions require the selected legacy build | Public depot 731 records and VDC oldid `496391`; exact configured legacy archive index is Missing | Blocked: no accepted legacy CS:GO content build or local root contract |

## Generation Contract

The future checked-in generator must:

1. Read the accepted format-universe VPK decision and reject a different version family.
2. Read exact configured archive indexes for one selected TF2, CS:S, and legacy Source 1 CS:GO content build without scanning an installation.
3. Resolve each indexed directory file and referenced segment by exact identity, recording content-build identity, logical source identity, byte length, and SHA-256.
4. Parse each directory file through the VPK package and emit signature, version, standalone/split layout, section lengths, signing state, and every referenced segment index.
5. Sort stable identities by game, archive family, and segment index; fail on a missing, duplicate, malformed, unknown, or unsupported discovery instead of omitting it.
6. Emit this path byte-identically on two clean-work-directory runs from the same declared inputs.

## Acceptance Blockers

- The format-universe VPK decision is Not accepted.
- The configured TF2 root establishes four TF2-owned archive families but not shared HL2/platform families.
- The local configuration contract has no CS:S or legacy CS:GO root fields, so both target indexes remain Missing.
- No checked-in generator command exists.
