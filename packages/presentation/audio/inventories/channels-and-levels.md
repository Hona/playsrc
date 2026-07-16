# Audio Channels And Levels Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a candidate inventory, not generated output. It contains 47 candidate items: 11 channel identities, 30 symbolic sound-level identities, and 6 attenuation aliases. It contains 0 generated items and 0 accepted items.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `src/public/soundflags.h`, `src/public/SoundParametersInternal.cpp`, `src/public/SoundEmitterSystem/isoundemittersystembase.h`, `src/public/engine/IEngineSound.h`, and TF2 build `10822003` indexed sound definitions |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 `steam.inf` SHA-256 `b8d7c1eb4517a806d514087facf42e3d8f407bf14393ac5fdc5d4c69e40adc7f` |
| Generator command | Missing |
| Output path | `packages/presentation/audio/inventories/channels-and-levels.md` |
| Owning roadmap | `packages/presentation/audio/ROADMAP.md` |
| Candidate item count | 47 |
| Generated item count | 0 |
| Accepted item count | 0 |

## Channels

The first eight values are network-representable sound-event channels. `CHAN_REPLACE` is an emission selector rather than retained active-channel state. `CHAN_VOICE_BASE` and `CHAN_USER_BASE` are range boundaries; a game owns the meaning of each game-specific value at or above `CHAN_USER_BASE`.

| Stable identity | SDK identity | Value | Required channel behavior | Candidate result |
|---|---|---:|---|---|
| `channel.replace` | `CHAN_REPLACE` | -1 | Replace the matching source's non-stream and non-voice retained channel selected at emission time. | Candidate required |
| `channel.auto` | `CHAN_AUTO` | 0 | Allocate a dynamic non-exclusive voice; another `CHAN_AUTO` start does not replace it solely by source and channel. | Candidate required |
| `channel.weapon` | `CHAN_WEAPON` | 1 | Retain weapon-channel identity and replace a prior matching source/channel unless overwrite is prohibited. | Candidate required |
| `channel.voice` | `CHAN_VOICE` | 2 | Retain primary acted-voice identity and protect it from ordinary capacity stealing. | Candidate required |
| `channel.item` | `CHAN_ITEM` | 3 | Retain item-channel identity and replace a prior matching source/channel unless overwrite is prohibited. | Candidate required |
| `channel.body` | `CHAN_BODY` | 4 | Retain body-channel identity and replace a prior matching source/channel unless overwrite is prohibited. | Candidate required |
| `channel.stream` | `CHAN_STREAM` | 5 | Retain streamed-file identity and protect active playback from ordinary capacity stealing. | Candidate required |
| `channel.static` | `CHAN_STATIC` | 6 | Allocate static-area lifetime for ambient, looping, and explicitly static playback. | Candidate required |
| `channel.voice-2` | `CHAN_VOICE2` | 7 | Retain secondary acted-voice identity and protect it from ordinary capacity stealing. | Candidate required |
| `channel.voice-base` | `CHAN_VOICE_BASE` | 8 | Mark the first channel reserved for decoded network voice instances. | Candidate required |
| `channel.user-base` | `CHAN_USER_BASE` | 136 | Mark the first game-owned channel value; Audio retains the integer while the selected game owns its semantic label. | Candidate required |

TF2 build `10822003` base sound definitions contain all eight named script channels: `CHAN_AUTO` 70 occurrences, `CHAN_WEAPON` 337, `CHAN_VOICE` 5,812, `CHAN_ITEM` 42, `CHAN_BODY` 113, `CHAN_STREAM` 1, `CHAN_STATIC` 1,991, and `CHAN_VOICE2` 588.

## Symbolic Sound Levels

Every symbolic lookup ignores ASCII case. Aliases with the same numeric value remain separate accepted source spellings and produce the same canonical decibel value.

| Stable identity | SDK identity | Decibel value | Candidate result |
|---|---|---:|---|
| `level.none` | `SNDLVL_NONE` | 0 | Candidate required |
| `level.20db` | `SNDLVL_20dB` | 20 | Candidate required |
| `level.25db` | `SNDLVL_25dB` | 25 | Candidate required |
| `level.30db` | `SNDLVL_30dB` | 30 | Candidate required |
| `level.35db` | `SNDLVL_35dB` | 35 | Candidate required |
| `level.40db` | `SNDLVL_40dB` | 40 | Candidate required |
| `level.45db` | `SNDLVL_45dB` | 45 | Candidate required |
| `level.50db` | `SNDLVL_50dB` | 50 | Candidate required |
| `level.55db` | `SNDLVL_55dB` | 55 | Candidate required |
| `level.idle` | `SNDLVL_IDLE` | 60 | Candidate required |
| `level.60db` | `SNDLVL_60dB` | 60 | Candidate required |
| `level.65db` | `SNDLVL_65dB` | 65 | Candidate required |
| `level.static` | `SNDLVL_STATIC` | 66 | Candidate required |
| `level.70db` | `SNDLVL_70dB` | 70 | Candidate required |
| `level.normal` | `SNDLVL_NORM` | 75 | Candidate required |
| `level.75db` | `SNDLVL_75dB` | 75 | Candidate required |
| `level.80db` | `SNDLVL_80dB` | 80 | Candidate required |
| `level.talking` | `SNDLVL_TALKING` | 80 | Candidate required |
| `level.85db` | `SNDLVL_85dB` | 85 | Candidate required |
| `level.90db` | `SNDLVL_90dB` | 90 | Candidate required |
| `level.95db` | `SNDLVL_95dB` | 95 | Candidate required |
| `level.100db` | `SNDLVL_100dB` | 100 | Candidate required |
| `level.105db` | `SNDLVL_105dB` | 105 | Candidate required |
| `level.110db` | `SNDLVL_110dB` | 110 | Candidate required |
| `level.120db` | `SNDLVL_120dB` | 120 | Candidate required |
| `level.130db` | `SNDLVL_130dB` | 130 | Candidate required |
| `level.gunfire` | `SNDLVL_GUNFIRE` | 140 | Candidate required |
| `level.140db` | `SNDLVL_140dB` | 140 | Candidate required |
| `level.150db` | `SNDLVL_150dB` | 150 | Candidate required |
| `level.180db` | `SNDLVL_180dB` | 180 | Candidate required |

Regular event sound levels occupy the integer domain `0..255`; `256..511` is the compatibility-attenuation domain. A sound-script token beginning `SNDLVL_` consumes its leading decimal integer when that value is 1 through 180; trailing nonnumeric text does not change that parsed value. A numeric sound-script value accepts one scalar or ordered interval; the parser does not require membership in the symbolic table. Every network-origin level is validated against its nine-bit field before Audio receives the event.

TF2 build `10822003` base sound definitions contain 50 distinct source spellings and 9,195 `soundlevel` occurrences. The occurrence set includes symbolic aliases, custom `SNDLVL_<integer>` values, and one numeric scalar.

## Attenuation Aliases

| Stable identity | SDK identity | Scalar | Required conversion | Candidate result |
|---|---|---:|---|---|
| `attenuation.none` | `ATTN_NONE` | 0.0 | Convert to non-attenuated sound level 0. | Candidate required |
| `attenuation.normal` | `ATTN_NORM` | 0.8 | Convert through the declared attenuation-to-level function. | Candidate required |
| `attenuation.idle` | `ATTN_IDLE` | 2.0 | Convert through the declared attenuation-to-level function. | Candidate required |
| `attenuation.static` | `ATTN_STATIC` | 1.25 | Convert through the declared attenuation-to-level function. | Candidate required |
| `attenuation.ricochet` | `ATTN_RICOCHET` | 1.5 | Convert through the declared attenuation-to-level function. | Candidate required |
| `attenuation.gunfire` | `ATTN_GUNFIRE` | 0.27 | Convert through the declared attenuation-to-level function. | Candidate required |

Numeric attenuation is finite and non-negative. Network compatibility attenuation cannot exceed `3.98`. Unknown named channels, levels, and attenuation aliases are `Unknown`; they never select a similarly named item or a default channel or level.

## Generation Contract

The future generator is owned by `tools/playsrc`. It must extract every named constant and conversion boundary from the pinned SDK, compare the result with the Audio contract manifest, and attach every indexed sound-definition and sound-mixer occurrence from each accepted content build to one listed channel, level, attenuation alias, or numeric-domain record.

Generation fails on a changed value, duplicate stable identity, new named constant, out-of-domain numeric value, unknown source spelling, unowned game-specific channel, stale authority, or item-count mismatch. It emits exactly 47 items in this order unless the same reviewed change updates the roadmap denominator.
