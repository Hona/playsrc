# VGUI Scheme Properties Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 56 items contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `IScheme`, `ISurface`, `IBorder`, `IImage`, and generic-control scheme consumers; configured TF2 public build `24207079`, patch `10822003`, scheme identities |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; denominator review Missing |
| Generator command | Missing |
| Output path | `packages/presentation/vgui/inventories/scheme-properties.md` |
| Owning roadmap | `packages/presentation/vgui/ROADMAP.md` |
| Candidate item count | 56 |
| Generated item count | 0 |
| Accepted item count | 0 |

The configured archive index contains `resource/ClientScheme.res`, `resource/SourceScheme.res`, `resource/ChatScheme.res`, and `resource/PDAControlPanelScheme.res`. Named colors, settings, fonts, files, and borders remain game-owned content identities; this inventory owns their generic schema and lookup behavior.

## Candidate Scheme Properties

| Stable ID | Construct or property | Required observable semantics | Coverage classification |
|---|---|---|---|
| `SCH-001` | Scheme document composition | Consume one parsed KeyValues document after ordered `#base` composition and preserve provenance for every contributing logical identity. | Unsupported |
| `SCH-002` | `Scheme` root, tag, and `Name` | Bind one logical file to one caller-supplied scheme tag; repeated loads of the same current identity return the same live scheme. | Unsupported |
| `SCH-003` | Section order and repeated entries | Preserve parsed order and apply the current KeyValues composition rule before registry construction. | Unsupported |
| `SCH-004` | Conditions and resolution suffixes | Apply only caller-declared platform, resolution, `_minmode`, accessibility, and browser conditions before registry construction. | Unsupported |
| `SCH-005` | `Colors` | Build one ASCII-insensitive ordered named-color registry. | Unsupported |
| `SCH-006` | `BaseSettings` | Build one ASCII-insensitive named string registry used by generic controls. | Unsupported |
| `SCH-007` | `Fonts` | Build named normal and proportional font aliases from ordered candidate glyph sets. | Unsupported |
| `SCH-008` | `CustomFontFiles` | Resolve declared font logical identities and optional family and language-range metadata. | Unsupported |
| `SCH-009` | `BitmapFontFiles` | Bind one symbolic bitmap-font name to one exact font logical identity. | Unsupported |
| `SCH-010` | `Borders` | Build one ordered named-border registry containing aliases and concrete border definitions. | Unsupported |
| `SCH-011` | Color RGBA literal | Parse exactly four bounded integer channels and preserve 8-bit RGBA output. | Unsupported |
| `SCH-012` | Color alias | Resolve a color value through another `Colors` or `BaseSettings` identity without changing the requested name. | Unsupported |
| `SCH-013` | Missing or cyclic color alias | Return `Missing` or `Malformed` with the complete alias chain; never substitute an unrelated color. | Unsupported |
| `SCH-014` | Base-setting lookup | Return the exact stored string for a named setting; the consumer owns numeric, color, font, or path interpretation. | Unsupported |
| `SCH-015` | Font alias identity | Create distinct normal and proportional handles for each font name while preserving inverse name lookup. | Unsupported |
| `SCH-016` | Ordered font candidates | Select the first candidate whose conditions and resolution interval admit the current viewport. | Unsupported |
| `SCH-017` | `yres` | Parse an inclusive minimum and optional maximum viewport-height interval; a lone value selects that exact height. | Unsupported |
| `SCH-018` | `name` in a font candidate | Resolve the declared system, custom, or bitmap family identity. | Unsupported |
| `SCH-019` | `tall`, `tall_lodef`, `tall_hidef` | Select the resolution-processed requested height and scale it only for proportional aliases lacking a `yres` interval. | Unsupported |
| `SCH-020` | `weight`, `weight_lodef`, `weight_hidef` | Select the resolution-processed numeric font weight. | Unsupported |
| `SCH-021` | `blur` | Apply a non-negative glyph blur radius after proportional scaling when applicable. | Unsupported |
| `SCH-022` | `scanlines` | Apply a non-negative scanline parameter after proportional scaling when applicable. | Unsupported |
| `SCH-023` | `scalex`, `scaley`, `scalex_lodef`, `scaley_lodef` | Select and proportionally scale bitmap glyph axes independently. | Unsupported |
| `SCH-024` | `range` | Parse an inclusive Unicode scalar interval and reject reversed, invalid, or out-of-range endpoints. | Unsupported |
| `SCH-025` | `italic` | Set italic font presentation when the selected browser font adapter supports it. | Unsupported |
| `SCH-026` | `underline` | Set underline font presentation. | Unsupported |
| `SCH-027` | `strikeout` | Set strikeout font presentation. | Unsupported |
| `SCH-028` | `symbol` | Select symbol-font character mapping without text-language fallback. | Unsupported |
| `SCH-029` | `antialias` | Request antialiased glyph presentation and record whether the browser adapter satisfies it. | Unsupported |
| `SCH-030` | `dropshadow` | Apply the declared one-glyph shadow treatment when supported. | Unsupported |
| `SCH-031` | `outline` | Apply the declared glyph outline treatment when supported. | Unsupported |
| `SCH-032` | `additive` | Mark the font for additive draw mode without changing glyph metrics. | Unsupported |
| `SCH-033` | `custom` | Disable browser family fallback for a declared custom family. | Unsupported |
| `SCH-034` | `bitmap` | Resolve glyphs through the named bitmap-font file and bitmap metrics. | Unsupported |
| `SCH-035` | `rotary` | Retain the declared rotary-font flag and classify adapter support explicitly. | Unsupported |
| `SCH-036` | `textcolor` font metadata | Retain the metadata for a consumer that declares it; otherwise classify it `Intentionally inert`. | Unsupported |
| `SCH-037` | `isproportional` font metadata | Retain the metadata while the requested font handle remains the sole proportional-selection authority. | Unsupported |
| `SCH-038` | Scalar custom-font entry | Treat the scalar value as one exact custom-font logical identity. | Unsupported |
| `SCH-039` | `font` in a custom-font block | Treat the value as the exact custom-font logical identity for the block. | Unsupported |
| `SCH-040` | `name` in a custom-font block | Bind the loaded font to the declared family name. | Unsupported |
| `SCH-041` | Language child and `range` | Apply the selected language child only and bind its Unicode interval to the named custom family. | Unsupported |
| `SCH-042` | Bitmap-font symbolic key and scalar value | Bind the symbolic key to one exact bitmap-font logical identity. | Unsupported |
| `SCH-043` | Scalar border alias | Resolve the named border to another border identity while retaining cycle and missing-reference diagnostics. | Unsupported |
| `SCH-044` | Border with absent `bordertype` | Present ordered side-line records, insets, and background type. | Unsupported |
| `SCH-045` | `bordertype` = `image` | Present one image over the requested border rectangle using image-border tiling policy. | Unsupported |
| `SCH-046` | `bordertype` = `scalable_image` | Present one image as a nine-slice border using source and destination corner dimensions. | Unsupported |
| `SCH-047` | `inset` | Parse left, top, right, and bottom integer insets and expose them to panel layout and foreground paint. | Unsupported |
| `SCH-048` | `backgroundtype` | Select filled, textured, or rounded-corner panel background behavior. | Unsupported |
| `SCH-049` | `Left`, `Top`, `Right`, `Bottom` | Preserve side order and each side's ordered line records. | Unsupported |
| `SCH-050` | `color` in a side line | Resolve one named or literal color for that line. | Unsupported |
| `SCH-051` | `offset` in a side line | Parse the line's start and end offsets relative to its side. | Unsupported |
| `SCH-052` | `image`, `tiled`, `paintfirst` in an image border | Resolve one image and select tiling plus before-background or after-foreground paint order. | Unsupported |
| `SCH-053` | `src_corner_height`, `src_corner_width`, `draw_corner_height`, `draw_corner_width`, `color` in a scalable image border | Configure nine-slice source extents, destination extents, and tint. | Unsupported |
| `SCH-054` | Named image lookup and hardware-filter flag | Cache by exact logical image identity plus filter selection and expose image dimensions, frame count, tint, and rotation. | Unsupported |
| `SCH-055` | Proportional scale and normalization | Scale from the 640×480 base by viewport height and truncate toward zero; normalization applies the inverse scale and truncation. | Unsupported |
| `SCH-056` | Reload, malformed data, missing data, and limits | Rebuild registries atomically under accepted byte, entry, alias-depth, font, border, image, texture, and diagnostic limits. | Unsupported |

## Generation Contract

The future generator must read the pinned public interfaces and generic control lookups, parsed scheme documents from exact declared-content indexes, the accepted font and image format inventories, and the accepted Owner Registry. It emits these stable IDs in numeric order and separately records every named color, base setting, font, custom font, bitmap font, border, and image occurrence with its game owner.

Generation fails on a new schema spelling without a family, duplicate or cyclic alias, invalid value domain, unclassified adapter requirement, missing format owner, missing logical dependency, changed stable ID, or item-count mismatch. Missing and malformed dependencies remain visible in the output or generator diagnostics.
