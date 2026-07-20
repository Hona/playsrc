# VGUI Scheme Properties Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 56 items contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `IScheme`, `ISurface`, `IBorder`, `IImage`, and generic-control scheme consumers; configured TF2 public build `24245096`, patch `10828683`, scheme identities |
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
| `SCH-001` | Scheme document composition | Consume exact parsed KeyValues documents, compose ordered relative `#base` edges recursively with derived values winning, and preserve every contributing logical identity. | Supported |
| `SCH-002` | `Scheme` root, tag, and `Name` | Bind one logical file to one caller-supplied scheme tag; repeated loads of the same current identity return the same live scheme. | Unsupported |
| `SCH-003` | Section order and repeated entries | Preserve parsed order and apply case-sensitive recursive base matching before registry construction. | Supported |
| `SCH-004` | Conditions and resolution suffixes | Apply `$DECK`, `$X360`, `$WIN32`, `$WINDOWS`, `$OSX`, `$LINUX`, `$POSIX`, and leading negation during document filtering; apply desktop `_minmode` only when selected. | Supported |
| `SCH-005` | `Colors` | Build one ASCII-insensitive ordered named-color registry. | Unsupported |
| `SCH-006` | `BaseSettings` | Build one ASCII-insensitive named string registry used by generic controls. | Unsupported |
| `SCH-007` | `Fonts` | Resolve distinct named normal and proportional requests from ordered candidate glyph sets. | Supported |
| `SCH-008` | `CustomFontFiles` | Resolve scalar and named exact font identities plus selected-language family ranges. | Supported |
| `SCH-009` | `BitmapFontFiles` | Bind one symbolic bitmap-font name to one exact bitmap identity and retain its adapter requirement. | Supported |
| `SCH-010` | `Borders` | Build one ordered named-border registry containing aliases and concrete border definitions. | Unsupported |
| `SCH-011` | Color RGBA literal | Parse exactly four bounded integer channels and preserve 8-bit RGBA output. | Unsupported |
| `SCH-012` | Color and base-setting lookup | Treat a `Colors` value as the terminal channel string; recursively resolve aliases only through `BaseSettings` without changing the requested name. | Supported |
| `SCH-013` | Missing, malformed, or cyclic base-setting color alias | Return `Missing` or `Malformed` with the complete alias chain; never reinterpret a non-channel `Colors` value or substitute an unrelated color. | Supported |
| `SCH-014` | Base-setting lookup | Return the exact stored string for a named setting; the consumer owns numeric, color, font, or path interpretation. | Unsupported |
| `SCH-015` | Font alias identity | Create distinct normal and proportional handles for each font name while preserving inverse name lookup. | Unsupported |
| `SCH-016` | Ordered font candidates | Select the first condition-retained candidate whose nonzero `yres` minimum admits the viewport, regardless of later source admission. | Supported |
| `SCH-017` | `yres` | Parse decimal minimum and maximum; copy a lone nonzero minimum to the maximum; treat minimum zero as no interval filter while retaining a nonzero maximum as a proportional-scaling guard. | Supported |
| `SCH-018` | `name` in a font candidate | Resolve the declared local, custom-file, or bitmap symbolic family identity. | Supported |
| `SCH-019` | `tall`, `tall_lodef`, `tall_hidef` | Use desktop `tall` after condition filtering; desktop scheme loading never processes `_lodef` or `_hidef`; scale only proportional aliases without an active `yres` pair. | Supported |
| `SCH-020` | `weight`, `weight_lodef`, `weight_hidef` | Use desktop `weight` after condition filtering and preserve the Source integer even when a browser adapter cannot express it. | Supported |
| `SCH-021` | `blur` | Retain a non-negative glyph blur radius after proportional scaling when applicable and apply it to supplied RGBA glyphs. | Supported |
| `SCH-022` | `scanlines` | Retain a non-negative scanline interval after proportional scaling when applicable and apply it to supplied RGBA glyphs. | Supported |
| `SCH-023` | `scalex`, `scaley`, `scalex_lodef`, `scaley_lodef` | Use desktop `scalex` and `scaley`, then proportionally scale bitmap axes independently when applicable. | Supported |
| `SCH-024` | `range` | Ignore candidate-local `range`; parse the selected-language custom-font `range` as an inclusive BMP interval and swap reversed endpoints. | Supported |
| `SCH-025` | `italic` | Retain italic selection and pass it to the selected adapter. | Supported |
| `SCH-026` | `underline` | Set underline font presentation. | Unsupported |
| `SCH-027` | `strikeout` | Set strikeout font presentation. | Unsupported |
| `SCH-028` | `symbol` | Select symbol-font character mapping without text-language fallback. | Unsupported |
| `SCH-029` | `antialias` | Retain the request only when the selected surface declares antialias support and keep browser/native raster disposition separate. | Supported |
| `SCH-030` | `dropshadow` | Apply the declared one-pixel down-right shadow to supplied RGBA glyphs when the surface supports it. | Supported |
| `SCH-031` | `outline` | Apply the declared one-pixel square-neighbor outline to supplied RGBA glyphs when the surface supports it. | Supported |
| `SCH-032` | `additive` | Retain additive draw mode without changing glyph metrics. | Supported |
| `SCH-033` | `custom` | Retain custom-family behavior and prohibit browser family fallback outside declared surface ranges. | Supported |
| `SCH-034` | `bitmap` | Resolve the named exact bitmap file over `U+0000-00FF` and require an explicit bitmap adapter and metrics. | Supported |
| `SCH-035` | `rotary` | Retain rotary selection and apply its fixed center line to supplied RGBA glyphs. | Supported |
| `SCH-036` | `textcolor` font metadata | Retain the metadata for a consumer that declares it; otherwise classify it `Intentionally inert`. | Unsupported |
| `SCH-037` | `isproportional` font metadata | Retain the metadata while the requested font handle remains the sole proportional-selection authority. | Unsupported |
| `SCH-038` | Scalar custom-font entry | Treat the scalar value as one exact custom-font logical identity and use supplied SFNT families. | Supported |
| `SCH-039` | `font` in a custom-font block | Treat the value as the exact custom-font logical identity for the block. | Supported |
| `SCH-040` | `name` in a custom-font block | Bind the loaded font to the declared family name. | Supported |
| `SCH-041` | Language child and `range` | Apply the selected language child only and bind its BMP interval to the named custom family. | Supported |
| `SCH-042` | Bitmap-font symbolic key and scalar value | Bind the symbolic key to one exact bitmap-font logical identity. | Supported |
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
| `SCH-055` | Proportional scale and normalization | Font requests scale from the 640×480 base by viewport height and truncate toward zero; generic inverse normalization remains Unsupported. | Partial |
| `SCH-056` | Reload, malformed data, missing data, and limits | Rebuild registries atomically under accepted byte, entry, alias-depth, font, border, image, texture, and diagnostic limits. | Unsupported |

## Generation Contract

The future generator must read the pinned public interfaces and generic control lookups, parsed scheme documents from exact declared-content indexes, the accepted font and image format inventories, and the accepted Owner Registry. It emits these stable IDs in numeric order and separately records every named color, base setting, font, custom font, bitmap font, border, and image occurrence with its game owner.

Generation fails on a new schema spelling without a family, duplicate or cyclic alias, invalid value domain, unclassified adapter requirement, missing format owner, missing logical dependency, changed stable ID, or item-count mismatch. Missing and malformed dependencies remain visible in the output or generator diagnostics.
