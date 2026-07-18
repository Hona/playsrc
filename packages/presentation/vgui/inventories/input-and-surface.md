# VGUI Input And Surface Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a manually derived Candidate inventory. Its 70 items contribute 0 items to the VGUI completion denominator until a checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `IVGui`, `IPanel`, `IClientPanel`, `IInput`, `ISurface`, `ISystem`, `IImage`, `IBorder`, message-map, focus-navigation, and generic-control contracts; playsrc browser DOM contract |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; playsrc denominator review Missing |
| Generator command | Missing |
| Output path | `packages/presentation/vgui/inventories/input-and-surface.md` |
| Owning roadmap | `packages/presentation/vgui/ROADMAP.md` |
| Candidate item count | 70 |
| Generated item count | 0 |
| Accepted item count | 0 |

Drag and drop is excluded from the current target. The pinned TF2 and shared economy call sites contain no use of the panel drag/drop interface; browser file upload and product drag/drop belong to applications. A newly accepted game consumer changes this inventory and roadmap in the same ownership checkpoint.

## Panel Runtime And Scheduling

| Stable ID | Behavior | Required observable contract | Coverage classification |
|---|---|---|---|
| `IOS-001` | Runtime frame | Advance from an injected monotonic time through input-state rollover, focus changes, queued messages, tick signals, layout solve, animation, and DOM publication in one declared order. | Unsupported |
| `IOS-002` | Panel allocation and safe identity | Allocate one stable nonzero panel identity and invalidate every safe handle after destruction. | Unsupported |
| `IOS-003` | Parent and child lifetime | Maintain one acyclic parent, ordered children, child-added notification, optional parent-owned destruction, and explicit reparenting. | Unsupported |
| `IOS-004` | Visibility state | Keep local visibility distinct from effective ancestor visibility and remove invisible subtrees from paint and input eligibility. | Unsupported |
| `IOS-005` | Enabled and input-interest state | Keep enabled, mouse-input-enabled, and keyboard-input-enabled as independent states consumed by controls and routing. | Unsupported |
| `IOS-006` | Local, absolute, inset, and minimum geometry | Clamp size to its minimum, derive absolute coordinates through parent position and inset, and notify size changes once. | Unsupported |
| `IOS-007` | Clip rectangle | Intersect non-popup bounds with ancestor clips and insets and publish an empty, never inverted rectangle when fully clipped. | Unsupported |
| `IOS-008` | Stable z-order | Order siblings by signed z value and stable tie order; front and back moves change only the eligible equal-z range. | Unsupported |
| `IOS-009` | Popup order | Maintain one independent ordered popup list, topmost-popup state, visibility, and parent relationship. | Unsupported |
| `IOS-010` | Solve and layout | Solve parents before children, resolve sibling pins after the referenced sibling, apply scheme before layout, and run invalid layouts once before paint. | Unsupported |
| `IOS-011` | Queued and delayed messages | Own message data after posting, preserve queue order, dispatch due delayed messages by time then insertion order, and discard messages whose target identity is dead. | Unsupported |
| `IOS-012` | Tick and deferred deletion | Invoke registered ticks at their accepted intervals, permit safe registration changes during dispatch, and destroy marked panels only at the deletion commit point. | Unsupported |

## Input Routing

| Stable ID | Behavior | Required observable contract | Coverage classification |
|---|---|---|---|
| `IOS-013` | Input contexts | Maintain isolated root, focus, pointer, capture, modal, edge-state, repeat, and cursor state per context. | Unsupported |
| `IOS-014` | Hit testing | Traverse eligible visible panels from topmost popup and frontmost child to backmost child, honor clipping, and return one deepest panel. | Unsupported |
| `IOS-015` | Pointer enter and exit | Emit one exit before one enter when mouse-over identity changes; capture does not change the independently reported mouse-over identity. | Unsupported |
| `IOS-016` | Pointer movement | Coalesce browser movement to the current cursor position for the frame and route it to capture or mouse focus exactly once. | Unsupported |
| `IOS-017` | Mouse press | Record the edge and down state, route to capture or mouse focus, and permit the control to request focus or capture. | Unsupported |
| `IOS-018` | Double and triple press | Distinguish platform double press from VGUI triple-press synthesis and deliver only to controls that admit the corresponding event. | Unsupported |
| `IOS-019` | Mouse release and mismatch | Clear down state, route release to the capture or pressed control, report mismatched release, and release capture only under its declared initiating-button rule. | Unsupported |
| `IOS-020` | Mouse wheel | Route signed wheel delta to the current eligible mouse focus and then the control's declared parent chain. | Unsupported |
| `IOS-021` | Mouse state queries | Expose pressed, double-pressed, down, and released state for the current frame without consuming it. | Unsupported |
| `IOS-022` | Mouse capture | Route pointer movement and button events to one captured panel until explicit release, initiating-button completion, modal change, or panel deletion. | Unsupported |
| `IOS-023` | Cursor state | Map the inventoried Source cursor identities to CSS cursor state, preserve explicit position, visibility, and override, and release browser pointer lock through the application adapter. | Unsupported |
| `IOS-024` | Keyboard-focus calculation | Select the current eligible focus under the frontmost keyboard popup, active root, modal restrictions, and panel focus delegation. | Unsupported |
| `IOS-025` | Focus transition | Deliver loss before gain immediately, repaint both panels and their popup ancestors, then publish the new focus and move its eligible popup forward. | Unsupported |
| `IOS-026` | Tab and hotkey navigation | Traverse eligible controls by tab position and stable child order, remain inside a top-level focus group, and delegate label hotkeys to associated controls. | Unsupported |
| `IOS-027` | Directional navigation | Resolve named up, down, left, right, activate, back, and relay links while skipping invisible or disabled panels and terminating cycles. | Unsupported |
| `IOS-028` | Key-code press | Record pressed and down state and route the code to current calculated focus through the message chain. | Unsupported |
| `IOS-029` | Key-code typed | Record typed state and route semantic key activation separately from physical press. | Unsupported |
| `IOS-030` | Unicode text input | Route browser composition result or text input as Unicode scalar input independently from key codes. | Unsupported |
| `IOS-031` | Key release and state queries | Clear down state, record released state, route release, and expose pressed, typed, down, and released queries for the current frame. | Unsupported |
| `IOS-032` | Key repeat | Start repeat from an admitted key press, use injected time, stop on release or focus loss, and synthesize repeated presses in deterministic order. | Unsupported |
| `IOS-033` | Unhandled keys | Notify registered live listeners only after the focus message chain declines the key code. | Unsupported |
| `IOS-034` | Application modal surface | Restrict mouse and keyboard focus and messages to one modal panel and descendants until release or destruction. | Unsupported |
| `IOS-035` | Inclusive modal subtree | Route only into one subtree and notify the declared outside-click listener for an outside press. | Unsupported |
| `IOS-036` | Excluded modal subtree | Route normally except into one excluded subtree and notify the declared outside-click listener for an outside press. | Unsupported |
| `IOS-037` | IME language and mode state | Expose browser-supported input language, conversion mode, sentence mode, selected mode, and deterministic unsupported results. | Unsupported |
| `IOS-038` | IME composition and candidates | Preserve composition start, update, caret, commit, end, candidate list, page, selection, and candidate-window position for the focused text control. | Unsupported |

## Browser Surface

| Stable ID | Behavior | Required observable contract | Coverage classification |
|---|---|---|---|
| `IOS-039` | Current paint context | Push one panel-local origin and optional inset, intersect the active clip, and restore the exact prior context on pop. | Unsupported |
| `IOS-040` | Panel paint sequence | Paint early border when selected, background, foreground, children from back to front, late border otherwise, then post-child foreground. | Unsupported |
| `IOS-041` | Scissor state | Represent the solved clip rectangle in CSS overflow or a browser surface clip and restore it across nested paint contexts. | Unsupported |
| `IOS-042` | Alpha multiplication | Multiply panel alpha through ancestors and apply the resulting bounded value uniformly to that panel's paint and descendants. | Unsupported |
| `IOS-043` | Filled rectangles | Present one or an ordered array of half-open axis-aligned filled rectangles under current color, alpha, origin, and clip. | Unsupported |
| `IOS-044` | Outlined rectangles | Present a one-unit four-edge outline without filling its interior. | Unsupported |
| `IOS-045` | Lines and polylines | Present ordered line segments under current color and clip with deterministic endpoint treatment. | Unsupported |
| `IOS-046` | Rectangle fades | Present horizontal or vertical linear alpha interpolation between declared endpoint alphas. | Unsupported |
| `IOS-047` | Circles and polygons | Present bounded outlined circles and ordered textured or untextured polygons with declared clipping. | Unsupported |
| `IOS-048` | Texture identity lifecycle | Allocate stable texture identities, validate identity use, bind one current texture, and destroy bytes and browser resources exactly once. | Unsupported |
| `IOS-049` | File-backed texture | Resolve one exact logical image identity with filter and reload selection; no path discovery or alternate extension search occurs. | Unsupported |
| `IOS-050` | Procedural and updated texture | Upload bounded RGBA/BGRA bytes, dimensions, format, and subregions only after complete validation. | Unsupported |
| `IOS-051` | Textured geometry | Present full rectangles, subrectangles, lines, polylines, and polygons with exact normalized texture coordinates. | Unsupported |
| `IOS-052` | Image object | Maintain position, content size, draw size, tint, frame, texture identity, quarter-turn rotation, paint, and eviction state. | Unsupported |
| `IOS-053` | Text draw state | Maintain current font, color, position, scale, and draw mode and advance text position after each printed glyph. | Unsupported |
| `IOS-054` | Character metrics | Exact supplied profiles return requested/actual height, ascent, maximum width, face-keyed A/B/C widths, character width, and additive state; browser-native exact metrics remain a typed requirement. | Partial |
| `IOS-055` | Text measurement and kerning | Exact supplied profiles measure UTF-16 code units with immediate-context kerning, cross-face suppression, newline height, ampersand skipping, and upward width rounding; browser shaping remains explicitly unverified. | Partial |
| `IOS-056` | Font resource lifecycle | Verify exact custom/external/bitmap bytes, construct private range families, publish atomically, release partial/publication failures, and destroy successful mounts exactly once; character precaching remains Unsupported. | Partial |
| `IOS-057` | Popup creation and ordering | Create popup DOM under the VGUI root, preserve parent ownership, maintain front/back/topmost order, and keep taskbar and native-window flags inert in browsers. | Unsupported |
| `IOS-058` | Embedded root and workspace | Bind one application-supplied root element and explicit workspace rectangle; VGUI never creates or discovers the application shell. | Unsupported |
| `IOS-059` | Screen and proportional base | Expose the current CSS-pixel viewport and the fixed 640×480 proportional base independently from device-pixel ratio. | Unsupported |
| `IOS-060` | Browser resize | Apply one resize observation atomically, invalidate affected schemes and layouts, emit the old and new dimensions, and repaint once. | Unsupported |
| `IOS-061` | Device-pixel ratio | Keep layout in CSS pixels, update raster and image selection inputs on ratio change, and avoid changing panel geometry solely from ratio change. | Unsupported |
| `IOS-062` | Direct DOM and CSS | Create, update, order, and remove VGUI-owned elements directly; no Preact vnode or framework reconciler enters the subtree. | Unsupported |
| `IOS-063` | DOM cleanup | Remove listeners, observers, timers, queued work, font and image references, popup entries, capture, focus, accessibility relationships, and DOM nodes on destruction. | Unsupported |

## Browser Integration And Semantics

| Stable ID | Behavior | Required observable contract | Coverage classification |
|---|---|---|---|
| `IOS-064` | Browser event adapter | Normalize pointer, mouse, wheel, keyboard, before-input, composition, focus, visibility, resize, and cancellation events into the VGUI input contract. | Unsupported |
| `IOS-065` | Accessibility mapping | Map labels, images, buttons, checkboxes, radios, text fields, sliders, progress controls, menus, lists, trees, tabs, dialogs, state, value, name, description, and relationships to one deterministic accessibility tree. | Unsupported |
| `IOS-066` | Reduced motion | In explicit reduced-motion mode, publish each visual interpolation's endpoint when its start time arrives while preserving command delays, event order, visibility changes, input changes, and sequence completion time. | Unsupported |
| `IOS-067` | Clipboard and system seam | Use injected clock and clipboard adapters and emit typed external-open, user-config, cursor-lock, and audio requests; product policy and persistence remain outside VGUI. | Unsupported |
| `IOS-068` | Bounds and error atomicity | Enforce panel, depth, child, message, tick, input, text, font, texture, DOM-node, observer, timer, byte, and diagnostic limits before mutation. | Unsupported |
| `IOS-069` | Commands and action signals | Route typed control commands and owned action-signal payloads to ordered live targets without invoking gameplay or product policy. | Unsupported |
| `IOS-070` | Message maps | Dispatch zero-, one-, two-, and record-parameter messages by registered name and declared type through derived-to-base maps, rejecting type mismatch before invocation. | Unsupported |

## Generation Contract

The future generator must read the pinned public VGUI interfaces and generic-control contracts, the accepted controls and resource-property inventories, the browser adapter contract, the current Owner Registry, and declared TF2 consumer call sites. It emits stable IDs in numeric order and records every supported browser event and surface operation under one semantic owner.

Generation fails on an omitted interface operation, duplicate stable ID, unclassified browser event, missing accessibility mapping, missing cleanup obligation, undeclared limit, unassigned adapter request, changed stable ID, or item-count mismatch.
