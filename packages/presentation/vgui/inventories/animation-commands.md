# VGUI Animation Commands Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 66 items contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `AnimationController`, `PanelAnimationVar`, and panel property-converter contracts; configured TF2 public build `24207079`, patch `10822003`, HUD-animation manifest and script |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; `scripts/hudanimations_manifest.txt` SHA-256 `927b61008081eafac302db71d32d75c63b4c9ed1b23c4bd5b8725826733c1acc`; `scripts/hudanimations_tf.txt` SHA-256 `cffca69ab872c358d8afb566e78acde022062e87b4fdcfa1779e77dd28ad52ff`; denominator review Missing |
| Generator command | Missing |
| Output path | `packages/presentation/vgui/inventories/animation-commands.md` |
| Owning roadmap | `packages/presentation/vgui/ROADMAP.md` |
| Candidate item count | 66 |
| Generated item count | 0 |
| Accepted item count | 0 |

The configured TF2-specific script contains 232 sequence declarations and 990 command occurrences across 9 command spellings. Those are game-owned occurrence identities. This inventory owns the complete generic language and runtime behavior.

## Script And Sequence Behavior

| Stable ID | Identity | Required observable semantics | Coverage classification |
|---|---|---|---|
| `ANIM-001` | Script-set loading | Load the manifest's exact ordered script identities; the first accepted script clears prior definitions and later scripts extend or replace them. | Unsupported |
| `ANIM-002` | `event` sequence declaration | Bind one ASCII-insensitive sequence name to its ordered command list and calculated duration. | Unsupported |
| `ANIM-003` | Duplicate sequence name | Replace the prior sequence deterministically with the later accepted declaration. | Unsupported |
| `ANIM-004` | Sequence condition | Include or omit the complete sequence from one explicit condition environment before scheduling. | Unsupported |
| `ANIM-005` | Command condition | Include or omit that command without changing the order of retained commands. | Unsupported |
| `ANIM-006` | Scoped sequence start | Resolve target panels under the supplied parent, cancel prior cancelable work for the same sequence and parent, then queue commands in script order. | Unsupported |
| `ANIM-007` | Stop and cancel lifecycle | Cancel only work selected by sequence, panel, variable, parent, and cancelability rules; deleted panels cannot receive later work. | Unsupported |
| `ANIM-008` | Virtual-time update | Process due delayed commands in queue order, then sample active animations at the same supplied time. | Unsupported |
| `ANIM-009` | Viewport-size change | Finish active visual interpolation, re-evaluate proportional position and size targets, reload selected scripts, and preserve deterministic command order. | Unsupported |

## Commands

| Stable ID | Script command | Required operands and effect | Coverage classification |
|---|---|---|---|
| `ANIM-010` | `Animate` | Target panel, animation variable, target value, interpolator and optional parameter, start delay, and duration; interpolate only a registered compatible variable. | Unsupported |
| `ANIM-011` | `RunEvent` | Sequence name and delay; start the named sequence under the same parent and cancelability context. | Unsupported |
| `ANIM-012` | `RunEventChild` | Child name, sequence name, and delay; resolve the child recursively and start the sequence under that child. | Unsupported |
| `ANIM-013` | `StopEvent` | Sequence name and delay; remove queued and active cancelable work for that sequence under the current parent. | Unsupported |
| `ANIM-014` | `StopAnimation` | Panel name, variable name, and delay; remove another sequence's active animation of that variable on that panel. | Unsupported |
| `ANIM-015` | `StopPanelAnimations` | Panel name and delay; remove other sequences' active animations on that panel. | Unsupported |
| `ANIM-016` | `SetFont` | Panel name, registered font variable, scheme font identity, and delay; set the variable through its converter. | Unsupported |
| `ANIM-017` | `SetTexture` | Panel name, registered texture variable, texture logical identity, and delay; set the variable through its converter. | Unsupported |
| `ANIM-018` | `SetString` | Panel name, registered string variable, string value, and delay; set the exact value through its converter. | Unsupported |
| `ANIM-019` | `FireCommand` | Delay and command text; emit one typed VGUI command at the scheduled time. | Unsupported |
| `ANIM-020` | `PlaySound` | Delay and sound logical identity; emit one typed audio request without owning playback. | Unsupported |
| `ANIM-021` | `SetVisible` | Panel name, boolean value, and delay; set panel visibility at the scheduled time. | Unsupported |
| `ANIM-022` | `SetInputEnabled` | Panel name, boolean value, and delay; set mouse and keyboard input eligibility together. | Unsupported |

## Interpolators

| Stable ID | Script identity | Required normalized-time transform | Coverage classification |
|---|---|---|---|
| `ANIM-023` | `Linear` and every other interpolator token | Preserve normalized time unchanged when the token is not one of the eight named nonlinear forms. | Unsupported |
| `ANIM-024` | `Accel` | Square normalized time. | Unsupported |
| `ANIM-025` | `Deaccel` | Use the non-negative square root of normalized time. | Unsupported |
| `ANIM-026` | `Spline` | Apply the declared simple ease-in/ease-out spline. | Unsupported |
| `ANIM-027` | `Pulse` | Apply the parameterized cosine pulse while producing the exact target at completion. | Unsupported |
| `ANIM-028` | `Flicker` | Select start or target from a deterministic injected random stream and the declared probability parameter. | Unsupported |
| `ANIM-029` | `Bias` | Apply the declared bias curve with a finite parameter in its accepted domain. | Unsupported |
| `ANIM-030` | `Gain` | Apply the declared gain curve with a finite parameter in its accepted domain. | Unsupported |
| `ANIM-031` | `Bounce` | Apply the three-interval damped bounce curve and settle exactly on the target. | Unsupported |

## Built-In Animation Variables

| Stable ID | Variable identity | Required value and scaling behavior | Coverage classification |
|---|---|---|---|
| `ANIM-032` | `Position` | Two coordinates with independent right, center, relative-panel, and proportional resolution. | Unsupported |
| `ANIM-033` | `Size` | Width and height pair, both proportionally scaled when the controller is proportional. | Unsupported |
| `ANIM-034` | `FgColor` | Four-channel foreground color. | Unsupported |
| `ANIM-035` | `BgColor` | Four-channel background color. | Unsupported |
| `ANIM-036` | `XPos` | One horizontal coordinate with right, center, relative-panel, and proportional resolution. | Unsupported |
| `ANIM-037` | `YPos` | One vertical coordinate with bottom, center, relative-panel, and proportional resolution. | Unsupported |
| `ANIM-038` | `Wide` | One width value, proportionally scaled when selected. | Unsupported |
| `ANIM-039` | `Tall` | One height value, proportionally scaled when selected. | Unsupported |
| `ANIM-040` | `ModelPos` | Three finite model-position values handed to a registered game-owned presentation control. | Unsupported |

## Property Converters

| Stable ID | Type identity | Required conversion | Coverage classification |
|---|---|---|---|
| `ANIM-041` | `float` | Parse, read, interpolate, and write one finite scalar. | Unsupported |
| `ANIM-042` | `int` | Parse and write one integer; interpolated values use the declared integer conversion at publication. | Unsupported |
| `ANIM-043` | `Color` | Resolve literal or scheme color and read or write four channels. | Unsupported |
| `ANIM-044` | `bool` | Accept `true`, `false`, or integer boolean forms and publish one boolean. | Unsupported |
| `ANIM-045` | `char` | Read and write one bounded registered character array. | Unsupported |
| `ANIM-046` | `string` | Read and write one bounded registered string array. | Unsupported |
| `ANIM-047` | `HFont` | Resolve one scheme font identity for the panel's proportional mode. | Unsupported |
| `ANIM-048` | `vgui::HFont` | Apply the same font conversion under the qualified type identity. | Unsupported |
| `ANIM-049` | `proportional_float` | Scale and normalize one float through the current scheme viewport. | Unsupported |
| `ANIM-050` | `proportional_int` | Scale and normalize one integer through the current scheme viewport. | Unsupported |
| `ANIM-051` | `proportional_xpos` | Scale one horizontal coordinate and retain screen-relative position semantics. | Unsupported |
| `ANIM-052` | `proportional_ypos` | Scale one vertical coordinate and retain screen-relative position semantics. | Unsupported |
| `ANIM-053` | `proportional_width` | Scale one width against the current scheme viewport. | Unsupported |
| `ANIM-054` | `proportional_height` | Scale one height against the current scheme viewport. | Unsupported |
| `ANIM-055` | `textureid` | Resolve one texture logical identity, retain its current texture handle, and release replaced resources. | Unsupported |

## Relative Alignments

| Stable ID | Token | Required reference point | Coverage classification |
|---|---|---|---|
| `ANIM-056` | `nw` | Refer to the target panel's top-left point. | Unsupported |
| `ANIM-057` | `n` | Refer to the target panel's top-center point. | Unsupported |
| `ANIM-058` | `ne` | Refer to the target panel's top-right point. | Unsupported |
| `ANIM-059` | `w` | Refer to the target panel's center-left point. | Unsupported |
| `ANIM-060` | `c` | Refer to the target panel's center point. | Unsupported |
| `ANIM-061` | `e` | Refer to the target panel's center-right point. | Unsupported |
| `ANIM-062` | `sw` | Refer to the target panel's bottom-left point. | Unsupported |
| `ANIM-063` | `s` | Refer to the target panel's bottom-center point. | Unsupported |
| `ANIM-064` | `se` | Refer to the target panel's bottom-right point. | Unsupported |
| `ANIM-065` | Malformed or unknown script item | Report script identity, sequence, command index, token span, expected operand, and classification; publish no partial script set. | Unsupported |
| `ANIM-066` | Limits and determinism | Enforce script bytes, tokens, sequences, commands, active animations, delayed commands, lookup depth, text, and diagnostics before allocation or queue mutation. | Unsupported |

## Generation Contract

The future generator must read the pinned SDK animation declarations, the accepted UI-script format inventory, exact declared-game HUD-animation manifests and scripts, the generated control and resource-property inventories, and the current Owner Registry. It emits stable IDs in numeric order and records every sequence and command occurrence under its game owner.

Generation fails on a new command, interpolator, built-in variable, converter, alignment, malformed form, or occurrence without one owner; a duplicate identity; a missing manifest dependency; a changed stable ID; or an item-count mismatch.
