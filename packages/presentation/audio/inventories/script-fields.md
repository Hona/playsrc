# Audio Script-Field And Codec Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a candidate inventory, not generated output. It contains 59 candidate items: 43 runtime script-field identities, 10 resource decorators, and 6 codec dispositions. It contains 0 generated items and 0 accepted items. Candidate items contribute 0 items to the Audio completion denominator until one checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 sound-emitter, soundscape, sound-character, audio-source, sentence, and engine-sound contracts; Valve Developer Community `Soundscripts` and `Soundscape` field contracts; W3C Web Audio API Recommendation; TF2 build `24207079` exact sound-script, soundscape, sentence, and audio archive indexes |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; Valve Developer Community immutable page revisions Unknown, checked 16 July 2026; Web Audio API Recommendation 17 June 2021; `steam.inf` SHA-256 `b8d7c1eb4517a806d514087facf42e3d8f407bf14393ac5fdc5d4c69e40adc7f`; archive-index hashes recorded below |
| Generator command | Missing |
| Output path | `packages/presentation/audio/inventories/script-fields.md` |
| Owning roadmap | `packages/presentation/audio/ROADMAP.md` |
| Candidate item count | 59 |
| Generated item count | 0 |
| Accepted item count | 0 |

## Sound-Definition Manifest Fields

| Stable identity | Source spelling | Required observable effect | Candidate result |
|---|---|---|---|
| `sound-manifest.precache-file` | `precache_file` | Load the named sound-definition document in manifest order and mark its declared resources for precache. | Candidate required |
| `sound-manifest.preload-file` | `preload_file` | Load the named sound-definition document in manifest order and mark its declared resources for preload before level playback. | Candidate required |

## Sound-Entry Fields

Sound-entry lookup ignores ASCII case and retains source spelling, source document, and declaration order. Each entry resolves to one immutable definition before emission state is created.

| Stable identity | Source spelling | Required value and effect | Candidate result |
|---|---|---|---|
| `sound-entry.channel` | `channel` | One named channel from the channel inventory or one bounded numeric channel; no omitted or unknown channel is inferred from a wave path. | Candidate required |
| `sound-entry.volume` | `volume` | `VOL_NORM`, one finite scalar in `0..1`, or one source-ordered pair whose endpoints are within `0..1`, sampled once per emission. Descending pairs remain descending. | Candidate required |
| `sound-entry.pitch` | `pitch` | `PITCH_NORM`, `PITCH_LOW`, `PITCH_HIGH`, one integer scalar in `1..255`, or one source-ordered integer pair whose endpoints are within `1..255`, sampled once per emission. Descending pairs remain descending. | Candidate required |
| `sound-entry.wave` | `wave` | Append one decorated resource identity to the definition's ordered selection set. | Candidate required |
| `sound-entry.rndwave` | `rndwave` | Append one ordered, non-empty random-wave block to the definition's selection set. | Candidate required |
| `sound-entry.rndwave.wave` | `rndwave/wave` | One decorated resource identity retained at its block ordinal. | Candidate required |
| `sound-entry.attenuation` | `attenuation` | One attenuation alias, finite scalar, or source-ordered finite pair within `0..3.98`, converted to ordinary sound-level semantics. | Candidate required |
| `sound-entry.compatibility-attenuation` | `CompatibilityAttenuation` | One scalar attenuation value retained as the compatibility attenuation profile; intervals are Malformed. | Candidate required |
| `sound-entry.sound-level` | `soundlevel` | One symbolic sound level, one `SNDLVL_` token with a leading decimal value in `1..180`, one numeric scalar in `0..255`, or one source-ordered pair within `0..255`, sampled once per emission. | Candidate required |
| `sound-entry.owner-only` | `play_to_owner_only` | Boolean audience constraint requiring the local listener to match the supplied source-owner identity. | Candidate required |
| `sound-entry.delay-ms` | `delay_msec` | Integer milliseconds in `0..65,535`, added only when the start event supplies no absolute sound time. | Candidate required |

## Soundscape Manifest And Root Commands

| Stable identity | Source spelling | Required observable effect | Candidate result |
|---|---|---|---|
| `soundscape-manifest.file` | `file` | Load the named soundscape document in manifest order; a map-specific exact document may be appended once when absent from the manifest. | Candidate required |
| `soundscape.command.dsp` | `dsp` | Select the room-DSP input for the active top-level soundscape. | Candidate required |
| `soundscape.command.dsp-player` | `dsp_player` | Select the listener DSP input for the active top-level soundscape. | Candidate required |
| `soundscape.command.play-looping` | `playlooping` | Create or retarget one looping ambient or positioned layer. | Candidate required |
| `soundscape.command.play-random` | `playrandom` | Create one recurring random-sound schedule. | Candidate required |
| `soundscape.command.play-soundscape` | `playsoundscape` | Expand one named child soundscape under the supplied volume and position transforms. | Candidate required |
| `soundscape.command.sound-mixer` | `soundmixer` | Select one declared sound-mixer identity for the active top-level soundscape. | Candidate required |
| `soundscape.command.dsp-volume` | `dsp_volume` | Set the finite global DSP-send scale for the active top-level soundscape. | Candidate required |

## `playlooping` Fields

| Stable identity | Source spelling | Required value and effect | Candidate result |
|---|---|---|---|
| `soundscape.loop.volume` | `volume` | Finite scalar or source-ordered pair within `0..1`, multiplied by the inherited soundscape volume. | Candidate required |
| `soundscape.loop.pitch` | `pitch` | Integer scalar or source-ordered pair within `1..255`, sampled once when the layer starts. | Candidate required |
| `soundscape.loop.wave` | `wave` | One decorated looping resource identity. | Candidate required |
| `soundscape.loop.position` | `position` | Integer local-position slot after inherited position offset and override rules. | Candidate required |
| `soundscape.loop.attenuation` | `attenuation` | Finite scalar or source-ordered pair within `0..3.98`, converted to the positioned layer's sound level. | Candidate required |
| `soundscape.loop.sound-level` | `soundlevel` | Symbolic sound level, numeric scalar, or source-ordered pair within `0..255` for a positioned layer. | Candidate required |
| `soundscape.loop.suppress-on-restore` | `suppress_on_restore` | Boolean preventing a restored timeline from restarting the layer. | Candidate required |

## `playrandom` Fields

| Stable identity | Source spelling | Required value and effect | Candidate result |
|---|---|---|---|
| `soundscape.random.volume` | `volume` | Finite scalar or source-ordered pair within `0..1`, multiplied by the inherited soundscape volume at each emission. | Candidate required |
| `soundscape.random.pitch` | `pitch` | Integer scalar or source-ordered pair within `1..255`, sampled at each emission. | Candidate required |
| `soundscape.random.attenuation` | `attenuation` | Finite scalar or source-ordered pair within `0..3.98`, converted to the positioned emission's sound level. | Candidate required |
| `soundscape.random.sound-level` | `soundlevel` | Symbolic sound level, numeric scalar, or source-ordered pair within `0..255`, sampled at each positioned emission. | Candidate required |
| `soundscape.random.time` | `time` | Non-negative finite scalar or source-ordered pair controlling the first and subsequent due times. | Candidate required |
| `soundscape.random.rndwave` | `rndwave` | One ordered, non-empty random-wave block. | Candidate required |
| `soundscape.random.rndwave.wave` | `rndwave/wave` | One decorated resource identity retained at its block ordinal. | Candidate required |
| `soundscape.random.position` | `position` | Integer local-position slot or `random`; inherited explicit overrides take precedence over `random`. | Candidate required |
| `soundscape.random.suppress-on-restore` | `suppress_on_restore` | Boolean preventing a restored timeline from scheduling the random layer. | Candidate required |

## `playsoundscape` Fields

| Stable identity | Source spelling | Required value and effect | Candidate result |
|---|---|---|---|
| `soundscape.child.volume` | `volume` | Finite scalar or source-ordered pair within `0..1`, multiplied by the inherited soundscape volume. | Candidate required |
| `soundscape.child.position` | `position` | Integer offset added to every child local-position index. | Candidate required |
| `soundscape.child.position-override` | `positionoverride` | Integer slot replacing all child positioned and ambient locations unless an ancestor already fixed the override. | Candidate required |
| `soundscape.child.ambient-position-override` | `ambientpositionoverride` | Integer slot replacing child ambient locations unless an ancestor already fixed that override. | Candidate required |
| `soundscape.child.name` | `name` | One ASCII-insensitive soundscape identity expanded at this command ordinal. | Candidate required |
| `soundscape.child.sound-level` | `soundlevel` | Recognized child field with no sound-level effect and one deterministic diagnostic. | Candidate required |

## Resource Decorators

Decorators form an ordered prefix. The undecorated remainder is the exact resource or sentence identity supplied to Content or sentence lookup.

| Stable identity | Character | Required effect | Candidate result |
|---|---:|---|---|
| `decorator.stream` | `*` | Use streaming resource lifetime. | Candidate required |
| `decorator.user-voice` | `?` | Bind the source to an explicit decoded user-voice stream. | Candidate required |
| `decorator.sentence` | `!` | Resolve a sentence identity rather than a file resource. | Candidate required |
| `decorator.dry` | `#` | Bypass room DSP while retaining master and mixer gains. | Candidate required |
| `decorator.doppler` | `>` | Interpret stereo channels as approaching and departing components under supplied source direction. | Candidate required |
| `decorator.directional` | `<` | Interpret stereo channels as front- and rear-facing components. | Candidate required |
| `decorator.distance-variant` | `^` | Interpret stereo channels as near and far components. | Candidate required |
| `decorator.omnidirectional` | `@` | Apply distance gain without directional panning. | Candidate required |
| `decorator.spatial-stereo` | `)` | Retain stereo content while applying positioned stereo spatialization. | Candidate required |
| `decorator.fast-pitch` | `}` | Select the lower-quality pitch-resampling profile. | Candidate required |

## Codec Dispositions

The accepted file-codec set contains exactly four items. MIDI is an indexed non-playable resource discovery. The encoded network-voice profile is unresolved.

| Stable identity | Container and encoded samples | Required result | TF2 build occurrence | Candidate result |
|---|---|---|---:|---|
| `codec.wave-pcm-u8` | RIFF/WAVE format 1, mono unsigned 8-bit PCM | Decode byte `u` to `(u - 128) / 128`, with exact channel, rate, frame, chunk, and loop metadata. | 15 files | Candidate required |
| `codec.wave-pcm-s16` | RIFF/WAVE format 1, mono or stereo signed 16-bit little-endian PCM | Decode integer `s` to `s / 32768`, with exact channel, rate, frame, chunk, loop, and sentence metadata. | 2,800 files | Candidate required |
| `codec.wave-ms-adpcm` | RIFF/WAVE format 2, mono Microsoft ADPCM | Decode validated coefficient and block data to signed 16-bit samples, then map each `s` to `s / 32768`. | 2 files | Candidate required |
| `codec.mpeg-layer-iii` | MPEG-1, MPEG-2, or MPEG-2.5 Layer III with optional ID3 prefix | Decode to canonical mono or stereo float samples in `[-1,1]`; expose exact output rate, channel count, duration, and stream progress. | 13,140 files | Candidate required |
| `codec.midi-non-playable` | Standard MIDI resource | Return deterministic `Unsupported` without creating a voice or decoder. | 1 file | Candidate required |
| `codec.network-voice` | Selected live or recorded voice profile | Decode only the exact profile selected by Networking and the application; no profile identity is accepted yet. | Not an archive-file codec | Candidate Blocked |

## TF2 Build `24207079` Occurrence Audit

The occurrence audit does not add denominator items. It records exact inputs that the future generator must reproduce.

- `scripts/game_sounds_manifest.txt` SHA-256 `6231fedc12c9b347a9bbc584c33137bed73803b2d84ec0a2a53b997070990dfa` declares 15 `precache_file` entries and 1 `preload_file` entry.
- The 16 base documents contain 9,218 sound-entry occurrences, 9,142 ASCII-insensitive identities, 46 duplicated identities, and 17,622 wave occurrences. Base field occurrences are: `channel` 8,954; `volume` 9,211; `pitch` 8,155; `soundlevel` 9,195; `attenuation` 2; root `wave` 7,639; `rndwave` 1,608; nested `wave` 9,983.
- The four SDK-declared MvM override documents contain 5,867 entry occurrences. Applying them in declared order leaves 12,959 ASCII-insensitive identities in the MvM registry.
- `scripts/soundscapes_manifest.txt` SHA-256 `0ecc70e34ffabda0a660791881911ffcd130322ddbbf87446fa31cecf3c93e24` declares 39 documents containing 153 unique soundscapes, 543 `playlooping` commands, 164 `playrandom` commands, and 2 `playsoundscape` commands.
- `scripts/sentences.txt` SHA-256 `69362ec39126d394f29c7541e9d5c257de3b859ebeea75f6be0b54cdfb5c9c4d` declares no TF2 sentences.
- The four indexed TF2 archive families contain 15,958 `sound/` resources: 2,817 WAVE, 13,140 MP3, and 1 MIDI. The WAVE set contains 2,815 PCM files and 2 Microsoft ADPCM files; 297 WAVE files declare a usable loop start and 51 contain sentence metadata.
- Exact decorated script lookup resolves 16,827 wave occurrences inside the four indexed TF2 archive families. Another 795 occurrences representing 530 unique logical paths require providers declared by `gameinfo.txt` outside the configured `tf2Dir`; they remain unresolved rather than being classified Missing.
- Archive-index SHA-256 values are `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9` (`tf2_misc_dir.vpk`), `dfbcc92beb6e9dd86994ad37be8bc7d8d7da66d01d2f4d441accdab776894bd9` (`tf2_sound_misc_dir.vpk`), `f9b0518925cd7b7b3b4373214e8fc9b8552a631bbe43661d8ff111d1fc539076` (`tf2_sound_vo_english_dir.vpk`), and `291719bce05f0d82e6fb20961e631c0dd3967a7fe5b11cb374ed56c25312337e` (`tf2_textures_dir.vpk`).

## Generation Contract

The future generator is owned by `tools/playsrc`. It must consume immutable SDK snapshots, the accepted Audio contract manifest, the exact configured provider plan and archive indexes, declared browser decoder profiles, and the selected network-voice profile. It emits all 59 stable identities in this order, then attaches every indexed sound entry, wave occurrence, soundscape command, mixer input, codec occurrence, loop declaration, and sentence declaration to exactly one item without adding those occurrences to the item count.

Generation fails on a changed field spelling, duplicate stable identity, empty random-wave block, unknown decorator, unknown codec, unresolved accepted provider, unclassified duplicate sound identity, invalid interval, malformed container, missing semantic owner, stale authority, or item-count mismatch. Unknown, Unsupported, Malformed, and Missing discoveries remain visible in output diagnostics and are never silently omitted.
