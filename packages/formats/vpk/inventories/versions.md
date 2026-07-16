# VPK Version And Archive-Layout Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

## Metadata

| Field | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `src/public/vpklib/packedstore.h`; Valve Developer Community `VPK (file format)` revision `496391`; exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes and directory-file bytes |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; VDC oldid `496391`; three target content-build revisions are Missing |
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
| `tf2-layout` | One selected TF2 content build | Candidate split families are `tf2_lv`, `tf2_textures`, `tf2_sound_vo_<language>`, `tf2_sound_misc`, `tf2_misc`, `hl2_textures`, `hl2_sound_vo_<language>`, `hl2_sound_misc`, `hl2_misc`, and `platform_misc`; exact selected families, languages, segment indexes, hashes, versions, and section profiles must come from the configured archive index and directory bytes | Source SDK `game/mod_tf/gameinfo.txt`; public depot 441 manifest `1804278129270892792`; exact configured target index is Missing | Blocked: no accepted TF2 content build and archive index |
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
- Exact configured TF2, CS:S, and legacy Source 1 CS:GO archive indexes and directory-file bytes are Missing.
- `playsrc.local.json` is Missing; the current example has no CS:S or legacy CS:GO root fields.
- No checked-in generator command exists.
