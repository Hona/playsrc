# VGUI Controls Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 51 items contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 VGUI build-factory declarations and public control contracts; configured TF2 public build `24245096`, patch `10828683`, VGUI resource control identities |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; denominator review Missing |
| Generator command | Missing |
| Output path | `packages/presentation/vgui/inventories/controls.md` |
| Owning roadmap | `packages/presentation/vgui/ROADMAP.md` |
| Candidate item count | 51 |
| Generated item count | 0 |
| Accepted item count | 0 |

The first 41 identities are the generic build-factory names declared by the SDK control library. The final 10 are generic code-created controls required by the owning roadmap. Parsed resources from TF2 public build `24245096`, patch `10828683`, contain 8,271 `ControlName` occurrences with 195 ASCII-insensitive identities: 28 match these generic factories and 167 non-factory identities require package, game, or application classification.

The GUI/HUD selected generic resource identities occur as follows: `Panel` 69, `EditablePanel` 975, `Label` 663, `ImagePanel` 1,276, `Button` 114, `TextEntry` 41, `RichText` 3, `Frame` 69, `ScrollBar` 15, `Slider` 3, `ComboBox` 56, `Menu` 8, `CheckButton` 102, `RadioButton` 55, `ProgressBar` 7, `ListPanel` 4, and `URLLabel` 1. `MenuItem`, `PropertySheet`, `PropertyPage`, `MessageBox`, and `QueryBox` have zero resource-factory occurrences and remain required code-created controls.

## Candidate Controls

| Stable ID | Control identity | Required generic behavior | Coverage classification |
|---|---|---|---|
| `CTRL-001` | `AnalogBar` | Bind a scalar or dialog variable to a bounded analog fill and emit deterministic fill geometry. | Unsupported |
| `CTRL-002` | `ContinuousAnalogBar` | Present the analog value as one continuous region rather than segmented units. | Unsupported |
| `CTRL-003` | `AnimatingImagePanel` | Resolve an ordered image-frame series and advance frames from the declared frame rate and visibility lifetime. | Unsupported |
| `CTRL-004` | `CBitmapImagePanel` | Present one bitmap image with color, alignment, filtering, and aspect-ratio policy. | Unsupported |
| `CTRL-005` | `Button` | Maintain default, armed, depressed, selected, enabled, keyboard, mouse, command, and action-signal behavior. | Supported |
| `CTRL-006` | `CheckButton` | Toggle one boolean value, paint checked state, and expose checkbox focus and accessibility state. | Supported |
| `CTRL-007` | `CircularProgressBar` | Clip foreground and background images to a declared angular progress interval. | Unsupported |
| `CTRL-008` | `ComboBox` | Coordinate text entry, drop button, popup menu, selected item, keyboard navigation, and change signals. | Partial |
| `CTRL-009` | `CControllerMap` | Bind named controller inputs to icon, text, and command records without owning game-command meaning. | Unsupported |
| `CTRL-010` | `Divider` | Present one deterministic horizontal or vertical divider using scheme state. | Unsupported |
| `CTRL-011` | `EditablePanel` | Load and apply child resources, create controls, maintain dialog variables, and delegate focus within one panel subtree. | Supported |
| `CTRL-012` | `ExpandButton` | Toggle expanded state and expose its glyph, command, and action signal. | Unsupported |
| `CTRL-013` | `GraphPanel` | Present caller-supplied graph samples through bounded surface primitives without owning sample production. | Unsupported |
| `CTRL-014` | `ImagePanel` | Resolve and present one image with scaling, tiling, positioning, rotation, fill, border, and draw-color state. | Supported |
| `CTRL-015` | `Label` | Present localized or literal text plus images with alignment, wrapping, insets, hotkey association, and auto-size behavior. | Supported |
| `CTRL-016` | `ListPanel` | Maintain ordered columns, rows, sorting, selection, keyboard navigation, images, and scroll state. | Partial |
| `CTRL-017` | `ListViewPanel` | Maintain an icon-and-label item view with deterministic ordering, selection, keyboard navigation, and scrolling. | Unsupported |
| `CTRL-018` | `Menu` | Maintain ordered menu items, separators, cascades, keyboard navigation, selection, popup lifetime, and dismissal. | Partial |
| `CTRL-019` | `MenuBar` | Coordinate ordered menu buttons and one active popup menu. | Unsupported |
| `CTRL-020` | `MenuButton` | Combine button state with deterministic menu opening, focus transfer, and dismissal. | Unsupported |
| `CTRL-021` | `MenuItem` | Present command, check, cascade, accelerator, armed, and enabled state inside a menu. | Partial |
| `CTRL-022` | `MessageBox` | Present a modal title, message, accepted button set, result signal, and close lifetime. | Supported |
| `CTRL-023` | `Panel` | Provide the base lifetime, hierarchy, geometry, state, paint, input, message, scheme, layout, and animation-variable contract. | Partial |
| `CTRL-024` | `PanelListPanel` | Lay out an ordered panel list with bounded vertical scrolling and optional scrollbar hiding. | Unsupported |
| `CTRL-025` | `ProgressBar` | Bind explicit or dialog-variable progress to deterministic segmented fill and remaining-time text. | Partial |
| `CTRL-026` | `ContinuousProgressBar` | Present progress as one continuous bounded region. | Unsupported |
| `CTRL-027` | `RadioButton` | Maintain one selected member in a tab-position group and emit deterministic checked signals. | Supported |
| `CTRL-028` | `RichText` | Present styled, selectable, scrollable text with insertion, links, copy, and deterministic line layout. | Partial |
| `CTRL-029` | `RotatingProgressBar` | Present one image rotated between declared angles with bounded approach and origin state. | Unsupported |
| `CTRL-030` | `ScalableImagePanel` | Present a nine-slice image with independent source and destination corner dimensions. | Unsupported |
| `CTRL-031` | `ScrollBar` | Coordinate decrement button, increment button, slider, range window, value, orientation, auto-hide, and signals. | Partial |
| `CTRL-032` | `ScrollBar_Vertical` | Instantiate `ScrollBar` with vertical orientation through its resource identity. | Partial |
| `CTRL-033` | `ScrollBar_Horizontal` | Instantiate `ScrollBar` with horizontal orientation through its resource identity. | Partial |
| `CTRL-034` | `SectionedListPanel` | Maintain ordered sections, columns, rows, sorting, selection, images, and scrolling. | Unsupported |
| `CTRL-035` | `Slider` | Maintain bounded scalar value, ticks, thumb geometry, keyboard steps, pointer drag, labels, and change signals. | Partial |
| `CTRL-036` | `TextEntry` | Maintain editable Unicode text, selection, caret, scrolling, clipboard, numeric-only mode, IME, and submit/change signals. | Partial |
| `CTRL-037` | `ToggleButton` | Preserve button interaction while latching selected state after activation. | Unsupported |
| `CTRL-038` | `TreeView` | Maintain stable hierarchical items, expansion, selection, keyboard navigation, images, and scrolling. | Unsupported |
| `CTRL-039` | `CTreeViewListControl` | Present a tree through list-style rows while retaining tree expansion and selection state. | Unsupported |
| `CTRL-040` | `URLLabel` | Present label interaction and emit a typed external-navigation request after URL validation and localization binding. | Supported |
| `CTRL-041` | `CvarToggleCheckButton` | Bind checkbox state to an injected configuration value without owning configuration policy. | Unsupported |
| `CTRL-042` | `ScrollBarSlider` | Maintain slider range, range window, thumb geometry, drag, page movement, borders, and movement signals for a scrollbar. | Unsupported |
| `CTRL-043` | `ScrollableEditablePanel` | Combine one editable child panel with one resource-configured scrollbar and deterministic viewport layout. | Unsupported |
| `CTRL-044` | `PropertySheet` | Maintain ordered tabs, one active page, page transitions, keyboard navigation, tab sizing, and apply/reset signals. | Partial |
| `CTRL-045` | `PropertyPage` | Provide one property-sheet page with show, hide, reset, apply, and tab-activation messages. | Supported |
| `CTRL-046` | `Frame` | Present a popup frame with title bar, client inset, focus appearance, move, resize, close, minimize, and modal behavior. | Partial |
| `CTRL-047` | `Tooltip` | Present delayed localized text for one owner, remain outside hit testing, and dismiss on owner, focus, or pointer-state changes. | Unsupported |
| `CTRL-048` | `FocusNavGroup` | Own default button, current default button, current focus, tab traversal, and hotkey delegation for one subtree. | Unsupported |
| `CTRL-049` | `AnimationController` | Parse, schedule, cancel, and advance the inventoried HUD animation commands against panel animation variables. | Unsupported |
| `CTRL-050` | `HTML` | Present an embedded document viewport and emit typed navigation, popup, clipboard, and external-open requests without owning product routing or network policy. | Unsupported |
| `CTRL-051` | `QueryBox` | Present a two-button modal query, map Escape to cancel, emit the configured result payload, close, and release modal state deterministically. | Supported |

## Generation Contract

The future generator must read the pinned SDK build-factory declarations, the named public control contracts, the accepted root Owner Registry, and exact declared-game `ControlName` occurrences. It emits package-owned controls in stable-ID order and emits every other occurrence with its game, application, or unresolved owner in generator diagnostics.

Generation fails on a duplicate factory name, case-insensitive identity collision, omitted generic control, unclassified content occurrence, missing owner, changed stable ID, or item-count mismatch. `Unsupported`, `Unknown`, `Malformed`, `Missing`, and owner-conflict discoveries are retained rather than omitted.
