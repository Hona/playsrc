# VGUI Resource Properties Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 58 property families contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it. Each family enumerates all spellings that share one observable contract; a spelling cannot move between families without a denominator change.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `Panel`, `EditablePanel`, `BuildGroup`, and generic-control resource consumers; configured TF2 public build `24245096`, patch `10828683`, `.res` identities |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; denominator review Missing |
| Generator command | Missing |
| Output path | `packages/presentation/vgui/inventories/resource-properties.md` |
| Owning roadmap | `packages/presentation/vgui/ROADMAP.md` |
| Candidate item count | 58 |
| Generated item count | 0 |
| Accepted item count | 0 |

The configured TF2 archive index contains 743 `resource/**/*.res` or `scripts/**/*.res` identities. Their parsed ordered trees contain 152,355 nodes. The most frequent scalar properties are `tall` 8,955, `ypos` 8,783, `wide` 8,745, `xpos` 8,694, `fieldName` 8,436, `ControlName` 8,271, `visible` 8,069, `zpos` 6,697, `enabled` 6,599, `font` 4,364, `AutoResize` 4,359, `labelText` 4,346, `PinCorner` 4,330, `textAlignment` 4,166, and `image` 1,858. The generator must enumerate properties from parsed KeyValues records; lexical counts are not accepted inventory input.

## Candidate Property Families

| Stable ID | Property spellings or construct | Required observable semantics | Coverage classification |
|---|---|---|---|
| `RES-001` | Resource root and child blocks | Preserve source child order and repeated blocks; apply only object-valued control blocks. | Unsupported |
| `RES-002` | Child block key, `fieldName` | Use the block key as the pre-apply panel identity; apply `fieldName` as the resulting panel name when present. | Unsupported |
| `RES-003` | `ControlName` | Resolve one ASCII-insensitive registered factory identity before applying any child settings. | Unsupported |
| `RES-004` | Unregistered `ControlName` | Return `Unknown` with resource identity, block identity, and requested control; never substitute `Panel`. | Unsupported |
| `RES-005` | `xpos` | Parse integer, right-edge `r`, center `c`, self-relative, parent-relative, and proportional position forms without changing source order. | Unsupported |
| `RES-006` | `ypos` | Parse integer, bottom-edge `r`, center `c`, self-relative, parent-relative, and proportional position forms without changing source order. | Unsupported |
| `RES-007` | `wide` | Parse integer, full-parent `f`, proportional-parent, proportional-self, and aspect-relative width forms. | Unsupported |
| `RES-008` | `tall` | Parse integer, full-parent `f`, proportional-parent, proportional-self, and aspect-relative height forms. | Unsupported |
| `RES-009` | `zpos` | Set signed sibling z-order while preserving stable order among equal values. | Unsupported |
| `RES-010` | `proportionalToParent` | Select the parent bounds rather than the root viewport as the alignment and fill reference. | Unsupported |
| `RES-011` | `usetitlesafe` | Apply the selected title-safe inset to eligible outer edges after position parsing and before final size publication. | Unsupported |
| `RES-012` | `AutoResize`, `PinnedCornerOffsetX`, `PinnedCornerOffsetY`, `UnpinnedCornerOffsetX`, `UnpinnedCornerOffsetY` | Preserve the declared pin and unpinned offsets and recompute bounds after parent-size changes. | Unsupported |
| `RES-013` | `PinCorner` | Select top-left, top-right, bottom-left, or bottom-right pin behavior for parent resize. | Unsupported |
| `RES-014` | `pin_to_sibling`, `pin_corner_to_sibling`, `pin_to_sibling_corner` | Resolve one sibling by name and align the declared corners after sibling geometry is solved. | Unsupported |
| `RES-015` | `navUp`, `navDown`, `navLeft`, `navRight`, `navToRelay`, `navActivate`, `navBack` | Resolve named navigation links lazily, skip ineligible panels, and terminate cycles deterministically. | Unsupported |
| `RES-016` | `visible` | Apply explicit visible state; an absent value retains the control default. | Unsupported |
| `RES-017` | `enabled` | Apply enabled state and expose the corresponding control and accessibility state. | Unsupported |
| `RES-018` | `mouseinputenabled` | Admit or exclude the panel subtree from pointer hit testing and dispatch. | Unsupported |
| `RES-019` | `keyboardinputenabled` | Admit or exclude the panel subtree from keyboard-focus selection and key dispatch. | Unsupported |
| `RES-020` | `tabPosition`, `TabPosition`, `SubTabPosition` | Establish stable tab and radio-group ordering without deriving order from DOM insertion alone. | Unsupported |
| `RES-021` | `tooltiptext` | Bind literal or localized tooltip text to the owning control. | Unsupported |
| `RES-022` | `paintbackground` | Enable or disable background painting independently from foreground and border painting. | Unsupported |
| `RES-023` | `paintborder` | Enable or disable border painting independently from background and foreground painting. | Unsupported |
| `RES-024` | `border`, `ButtonBorder`, `border_override`, `normalborder_override`, `activeborder_override` | Resolve the named scheme border for the applicable normal, button, tab, or active state. | Unsupported |
| `RES-025` | `IgnoreScheme` | Reapply explicit resource values after scheme defaults for properties that permit resource override. | Unsupported |
| `RES-026` | `actionsignallevel` | Add the ancestor at the exact non-negative parent distance as an action-signal target. | Unsupported |
| `RES-027` | `RoundedCorners` | Select the four-bit rounded-corner mask used by background presentation. | Unsupported |
| `RES-028` | `ForceStereoRenderToFrameBuffer` | Retain the presentation request for the application surface adapter without changing world rendering. | Unsupported |
| `RES-029` | Condition blocks supplied by the caller | Promote values from every active named condition in caller order; inactive blocks remain unapplied. | Unsupported |
| `RES-030` | Resolution suffixes, `_minmode`, declared browser layout conditions | Apply exact selected overrides before control creation; VGUI never infers an undeclared condition. | Unsupported |
| `RES-031` | Dialog-variable store and `variable`, `analogValue` | Bind typed string, wide string, integer, and float values and notify the owning panel plus its direct children atomically. | Unsupported |
| `RES-032` | `labelText`, `text`, `title`, `URLText`, leading `#`, and `%name%` dialog placeholders | Preserve unlocalized text, resolve localization tokens, then substitute supplied dialog variables without mutating the token table. | Unsupported |
| `RES-033` | Control-registered overridable color names | Accept an RGBA value or scheme-color identity only for a color property registered by the control. | Unsupported |
| `RES-034` | `alpha`, `PaintBackgroundType`, `Texture1`, `Texture2`, `Texture3`, `Texture4` | Apply the base panel animation variables and texture identities before paint traversal. | Unsupported |
| `RES-035` | `font`, `dulltext`, `brighttext`, `allcaps` | Resolve label font and text-color state, then apply uppercase transformation to presentation text only. | Unsupported |
| `RES-036` | `textAlignment`, `textinsetx`, `textinsety`, `use_proportional_insets`, `associate` | Apply one of nine alignments, exact insets, and optional associated-control focus delegation. | Unsupported |
| `RES-037` | `wrap`, `centerwrap`, `auto_wide_tocontents`, `auto_tall_tocontents` | Compute line breaks and content-driven dimensions from the selected font and available width. | Unsupported |
| `RES-038` | `image`, `drawcolor`, `fillcolor`, `border` on image controls | Resolve the image and apply draw tint, optional fill, and optional scheme border. | Unsupported |
| `RES-039` | `scaleImage`, `scaleProportional`, `scaleAmount`, `tileImage`, `tileHorizontally`, `tileVertically`, `positionImage`, `rotation` | Apply the exact image sizing, tiling, placement, and quarter-turn rotation policy. | Unsupported |
| `RES-040` | `imagecolor`, `imageAlignment`, `preserveAspectRatio`, `filtered` | Apply bitmap-image tint, one of nine alignments, aspect preservation, and filter selection. | Unsupported |
| `RES-041` | `src_corner_height`, `src_corner_width`, `draw_corner_height`, `draw_corner_width` | Map one source image to deterministic nine-slice destination regions. | Unsupported |
| `RES-042` | `frames`, `anim_framerate`, `image` on `AnimatingImagePanel` | Resolve the numbered frame set and advance it from the declared positive frame rate. | Unsupported |
| `RES-043` | `command`, `default`, `selected`, `stayselectedonclick`, `stay_armed_on_click` | Bind button command, default-button eligibility, initial selection, and post-click armed or selected state. | Unsupported |
| `RES-044` | `button_activation_type`, `sound_armed`, `sound_depressed`, `sound_released` | Select press-versus-release activation and emit typed sound requests at the declared state transitions. | Unsupported |
| `RES-045` | `smallcheckimage`, `TabPosition`, `SubTabPosition` on check and radio controls | Select check glyph scale and deterministic radio-group membership. | Unsupported |
| `RES-046` | `Button`, `border_override` on combo and menu composites | Configure the composite drop button and popup border while retaining composite ownership. | Unsupported |
| `RES-047` | `editable`, `maxchars`, `NumericInputOnly`, `selectallonfirstfocus`, `textHidden`, `unicode` | Configure text mutation, code-unit limit, numeric admission, first-focus selection, masking, and Unicode mode. | Unsupported |
| `RES-048` | `text`, `textfile`, `maxchars`, `scrollbar` on `RichText` | Select inline or resolved text input, bounded content, and scrollbar visibility without searching for files. | Unsupported |
| `RES-049` | `rangeMin`, `rangeMax`, `numTicks`, `thumbwidth`, `leftText`, `rightText` | Configure slider domain, ticks, thumb dimension, and endpoint labels; reject an invalid domain. | Unsupported |
| `RES-050` | `progress`, `variable`, `analogValue` | Set explicit progress or bind one dialog variable to progress or analog presentation. | Unsupported |
| `RES-051` | `bg_image`, `fg_image`, `start_degrees`, `end_degrees`, `approach_speed`, `rot_origin_x_percent`, `rot_origin_y_percent`, `rotating_x`, `rotating_y`, `rotating_wide`, `rotating_tall` | Configure circular and rotating progress image geometry and angular state. | Unsupported |
| `RES-052` | `nobuttons`, `UpButton`, `DownButton`, `Slider`, `Scrollbar` | Configure scrollbar child controls and the nested scrollbar block of a scrollable editable panel. | Unsupported |
| `RES-053` | `sectiongap`, `linegap`, `linespacing`, `show_columns`, `autohide_scrollbar` | Configure sectioned-list spacing, column visibility, and panel-list scrollbar behavior. | Unsupported |
| `RES-054` | `tabwidth`, `tabskv`, `transition_time`, `tabxindent`, `tabxdelta`, `tabxfittotext`, `tabheight`, `tabheight_small`, `yoffset` | Configure property-sheet tab creation, dimensions, page offset, and transition timing. | Unsupported |
| `RES-055` | `title_font`, `settitlebarvisible`, `setclosebuttonvisible`, `clientinsetx_override`, `titlecolor`, `messagecolor`, `buttontextcolor`, `button_margin`, `button_separator`, `footer_tall`, `activity_indent`, `WizardWide`, `WizardTall` | Configure frame, message-dialog, and wizard presentation without changing dialog command policy. | Unsupported |
| `RES-056` | `name`, `icon`, `text`, `command` on `CControllerMap`; `URLText` | Bind controller-map display records and validated external-navigation text. | Unsupported |
| `RES-057` | Malformed scalar, unknown property, missing reference, and unsupported property | Classify each encountered value explicitly; no invalid property can partially mutate a panel. | Unsupported |
| `RES-058` | Aggregate resource limits and apply order | Validate bytes, depth, nodes, controls, strings, references, and diagnostics before atomically replacing one resource-owned subtree. | Unsupported |

## Generation Contract

The future generator must read parsed KeyValues records from every accepted generic control resource consumer, the pinned SDK property consumers, the accepted control inventory, the accepted condition registry, and exact declared-game resource indexes. It emits one row per accepted property family in stable-ID order and records every encountered spelling, value domain, consuming control, and semantic owner.

Generation fails on an unassigned spelling, one spelling in multiple families, unknown control owner, missing value domain, omitted malformed case, changed stable ID, unclassified content occurrence, or item-count mismatch. Hand editing is invalid after generation exists.
