import { describe, expect, test } from "bun:test"
import {
  initializeDeveloperConsole,
  SOURCE_CONSOLE_CEILINGS,
  type ConsoleCatalog,
  type ConsoleLimits,
  type ConsoleRequest,
  type ConsoleResourceResolution,
  type ConsoleResources,
  type DeveloperConsole,
  type DeveloperConsoleConfiguration,
} from "../src"
import { byName, click, createRoot, descendants, FakeDocument, FakeElement, input, key } from "./fake-dom"

const color = (red: number, green: number, blue: number, alpha = 255) =>
  Object.freeze([red, green, blue, alpha] as const)

function resources(identity = "vgui/console/test-resources"): ConsoleResources {
  const font = (logicalIdentity: string, family: string, size: number) =>
    Object.freeze({
      logicalIdentity,
      family,
      sizePxAt480: size,
      lineHeightPxAt480: size + 2,
      weight: 500,
      style: "normal" as const,
    })
  return Object.freeze({
    identity,
    scheme: Object.freeze({
      logicalIdentity: "resource/sourcescheme.res",
      tag: "SourceScheme",
      revision: "synthetic-1",
    }),
    localization: Object.freeze({
      logicalIdentity: "resource/gameui_english.txt",
      language: "english",
      title: "Developer Console",
      submit: "Submit",
      entryAccessibleName: "Console command",
      historyAccessibleName: "Console output",
      completionAccessibleName: "Console completions",
    }),
    colors: Object.freeze({
      frameBackground: color(40, 41, 42),
      titleText: color(250, 251, 252),
      historyBackground: color(10, 11, 12),
      inputBackground: color(20, 21, 22),
      inputText: color(230, 231, 232),
      completionBackground: color(30, 31, 32),
      completionText: color(220, 221, 222),
      completionSelected: color(70, 80, 90),
      focus: color(255, 190, 20),
      normalOutput: color(216, 222, 211),
      developerOutput: color(196, 181, 80),
    }),
    fonts: Object.freeze({
      title: font("resource/fonts/title.ttf", "Title Test", 12),
      console: font("resource/fonts/console.ttf", "Console Test", 14),
      completion: font("resource/fonts/small.ttf", "Small Test", 10),
    }),
    border: Object.freeze({
      logicalName: "DepressedButtonBorder",
      color: color(100, 101, 102),
      widthPxAt480: 1,
      style: "inset" as const,
    }),
    frameTitleHeightPxAt480: 20,
  })
}

function catalog(revision = "catalog-1"): ConsoleCatalog {
  return Object.freeze({
    revision,
    items: Object.freeze([
      Object.freeze({ kind: "command" as const, name: "map", disposition: "visible" as const, acceptsSuggestions: true }),
      Object.freeze({ kind: "command" as const, name: "mat_reload", disposition: "visible" as const, acceptsSuggestions: false }),
      Object.freeze({ kind: "convar" as const, name: "mat_wireframe", disposition: "visible" as const, displayValue: "0" }),
      Object.freeze({ kind: "command" as const, name: "map_hidden", disposition: "hidden" as const, acceptsSuggestions: false }),
      Object.freeze({ kind: "convar" as const, name: "map_dev", disposition: "development" as const, displayValue: "1" }),
    ]),
  })
}

function limits(overrides: Partial<ConsoleLimits> = {}): ConsoleLimits {
  const value = {
    maxInputUtf8Bytes: SOURCE_CONSOLE_CEILINGS.maxInputUtf8Bytes,
    maxHistoryItems: SOURCE_CONSOLE_CEILINGS.maxHistoryItems,
    maxCatalogItems: 128,
    maxCatalogItemUtf8Bytes: 255,
    maxCompletionItems: SOURCE_CONSOLE_CEILINGS.maxCompletionItems,
    maxCompletionItemUtf8Bytes: SOURCE_CONSOLE_CEILINGS.maxCompletionItemUtf8Bytes,
    maxVisibleCompletionItems: SOURCE_CONSOLE_CEILINGS.maxVisibleCompletionItems,
    maxOutputBatchSegments: 16,
    maxOutputBatchUtf8Bytes: 1024,
    maxOutputSegments: 32,
    maxOutputUtf8Bytes: 4096,
    maxDiagnostics: 16,
    maxDomNodes: 64,
    maxListeners: 8,
    ...overrides,
  }
  value.maxDomNodes = Math.max(
    value.maxDomNodes,
    10 + value.maxOutputSegments + value.maxVisibleCompletionItems,
  )
  return Object.freeze(value)
}

function configuration(
  requests: ConsoleRequest[],
  overrides: Partial<DeveloperConsoleConfiguration> = {},
): DeveloperConsoleConfiguration {
  return Object.freeze({
    runtimeIdentity: "console-test",
    limits: limits(),
    resources: Object.freeze({ kind: "resolved" as const, resources: resources() }),
    catalog: catalog(),
    viewport: Object.freeze({ width: 1280, height: 720, devicePixelRatio: 2 }),
    reducedMotion: false,
    onRequest: (request: ConsoleRequest) => requests.push(request),
    ...overrides,
  })
}

function create(
  requests: ConsoleRequest[] = [],
  overrides: Partial<DeveloperConsoleConfiguration> = {},
): DeveloperConsole {
  const initialized = initializeDeveloperConsole(configuration(requests, overrides))
  if (!initialized.ok) throw new Error(`initialization failed: ${initialized.diagnostic.code}`)
  return initialized.console
}

function mounted(
  requests: ConsoleRequest[] = [],
  overrides: Partial<DeveloperConsoleConfiguration> = {},
): Readonly<{ console: DeveloperConsole; root: FakeElement; document: FakeDocument }> {
  const document = new FakeDocument()
  const root = createRoot(document)
  const developerConsole = create(requests, overrides)
  expect(developerConsole.apply({ kind: "mount", root: root as unknown as HTMLElement }).ok).toBe(true)
  return { console: developerConsole, root, document }
}

describe("developer console initialization, resources, and direct DOM", () => {
  test("rejects missing, malformed, and inconsistent required inputs before mounting", () => {
    const requests: ConsoleRequest[] = []
    const missing: ConsoleResourceResolution = Object.freeze({
      kind: "missing",
      logicalIdentity: "resource/sourceschemebase.res",
    })
    expect(initializeDeveloperConsole(configuration(requests, { resources: missing }))).toMatchObject({
      ok: false,
      diagnostic: { code: "MissingResource", subject: "resource/sourceschemebase.res" },
    })

    const malformed = Object.freeze({
      kind: "resolved" as const,
      resources: { ...resources(), colors: { ...resources().colors, normalOutput: [0, 0, 0, 256] } },
    }) as unknown as ConsoleResourceResolution
    expect(initializeDeveloperConsole(configuration(requests, { resources: malformed }))).toMatchObject({
      ok: false,
      diagnostic: { code: "MalformedResource" },
    })

    expect(
      initializeDeveloperConsole(
        configuration(requests, { limits: limits({ maxInputUtf8Bytes: 256 }) }),
      ),
    ).toMatchObject({ ok: false, diagnostic: { code: "InvalidLimits" } })
    expect(
      initializeDeveloperConsole(
        configuration(requests, { viewport: { width: 0, height: 720, devicePixelRatio: 2 } }),
      ),
    ).toMatchObject({ ok: false, diagnostic: { code: "InvalidViewport" } })
  })

  test("mounts one VGUI-owned direct subtree with scheme, layout, and accessibility state", () => {
    const { console: developerConsole, root, document } = mounted()
    expect(root.children).toHaveLength(2)
    expect(root.children[0].tagName).toBe("STYLE")
    expect(root.children[1].dataset.vguiOwner).toBe("playsrc")
    expect(descendants(root).map((node) => node.dataset.vguiControl).filter(Boolean)).toEqual([
      "Frame",
      "Label",
      "Panel",
      "RichText",
      "TextEntry",
      "Button",
      "Menu",
    ])

    const frame = byName(root, "GameConsole")
    const entry = byName(root, "ConsoleEntry")
    const history = byName(root, "ConsoleHistory")
    expect(frame.getAttribute("role")).toBe("dialog")
    expect(frame.getAttribute("aria-label")).toBe("Developer Console")
    expect(history.getAttribute("role")).toBe("log")
    expect(history.getAttribute("aria-live")).toBe("polite")
    expect(entry.getAttribute("aria-label")).toBe("Console command")
    expect(entry.getAttribute("aria-controls")).toBe(byName(root, "CompletionList").id)
    expect(frame.style.left).toBe("616px")
    expect(frame.style.top).toBe("96px")
    expect(frame.style.width).toBe("640px")
    expect(frame.style.height).toBe("528px")

    expect(developerConsole.apply({ kind: "activate" }).ok).toBe(true)
    expect(developerConsole.snapshot()).toMatchObject({ visible: true, focused: true, foregroundRevision: 1 })
    expect(document.activeElement).toBe(entry)
    expect(developerConsole.apply({ kind: "hide" }).ok).toBe(true)
    expect(developerConsole.snapshot()).toMatchObject({ visible: false, focused: false })
  })

  test("atomically replaces resources and preserves geometry across DPR-only changes", () => {
    const { console: developerConsole, root } = mounted()
    const frame = byName(root, "GameConsole")
    const original = [frame.style.left, frame.style.top, frame.style.width, frame.style.height]
    expect(
      developerConsole.apply({
        kind: "replace-resources",
        resolution: { kind: "missing", logicalIdentity: "resource/sourceschemebase.res" },
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "MissingResource" } })
    expect(developerConsole.snapshot().resourceIdentity).toBe("vgui/console/test-resources")

    expect(
      developerConsole.apply({
        kind: "replace-resources",
        resolution: {
          kind: "resolved",
          resources: {
            ...resources("vgui/console/malformed"),
            border: { ...resources().border, widthPxAt480: -1 },
          },
        },
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "MalformedResource" } })
    expect(developerConsole.snapshot().resourceIdentity).toBe("vgui/console/test-resources")

    expect(
      developerConsole.apply({
        kind: "replace-resources",
        resolution: { kind: "resolved", resources: resources("vgui/console/replacement") },
      }).ok,
    ).toBe(true)
    expect(developerConsole.snapshot().resourceIdentity).toBe("vgui/console/replacement")
    expect(root.children[1].dataset.resourceIdentity).toBe("vgui/console/replacement")

    expect(
      developerConsole.apply({
        kind: "set-viewport",
        viewport: { width: 1280, height: 720, devicePixelRatio: 3 },
      }).ok,
    ).toBe(true)
    expect([frame.style.left, frame.style.top, frame.style.width, frame.style.height]).toEqual(original)
    expect(root.children[1].dataset.devicePixelRatio).toBe("3")

    expect(
      developerConsole.apply({
        kind: "set-viewport",
        viewport: { width: 160, height: 100, devicePixelRatio: 1 },
      }).ok,
    ).toBe(true)
    expect(Number.parseInt(String(frame.style.left))).toBeGreaterThanOrEqual(0)
    expect(Number.parseInt(String(frame.style.top))).toBeGreaterThanOrEqual(0)
    expect(Number.parseInt(String(frame.style.left)) + Number.parseInt(String(frame.style.width))).toBeLessThanOrEqual(160)
    expect(Number.parseInt(String(frame.style.top)) + Number.parseInt(String(frame.style.height))).toBeLessThanOrEqual(100)

    developerConsole.apply({ kind: "set-reduced-motion", reduced: true })
    expect(root.children[1].dataset.reducedMotion).toBe("true")
  })
})

describe("developer console output and submission", () => {
  test("preserves normal, developer, and explicit-color order, retention, scrolling, and clear", () => {
    const { console: developerConsole, root } = mounted([], {
      limits: limits({ maxOutputSegments: 4, maxOutputUtf8Bytes: 258 }),
    })
    expect(
      developerConsole.apply({
        kind: "append-output",
        segments: [
          { kind: "normal", text: "one" },
          { kind: "developer", text: "two" },
          { kind: "color", text: "three", color: color(1, 2, 3, 4) },
        ],
      }).ok,
    ).toBe(true)
    expect(developerConsole.snapshot().output.map((segment) => segment.kind)).toEqual([
      "normal",
      "developer",
      "color",
    ])
    const history = byName(root, "ConsoleHistory")
    expect(history.textContent).toBe("onetwothree")
    expect(history.children.map((node) => node.dataset.outputKind)).toEqual(["normal", "developer", "color"])
    expect(history.children[2].style.color).toBe("rgba(1, 2, 3, 0.01568627450980392)")
    expect(history.scrollTop).toBe(history.scrollHeight)

    expect(
      developerConsole.apply({
        kind: "append-output",
        segments: [
          { kind: "normal", text: "four" },
          { kind: "normal", text: "five" },
        ],
      }).ok,
    ).toBe(true)
    expect(developerConsole.snapshot().output.map((segment) => segment.text)).toEqual(["two", "three", "four", "five"])

    expect(developerConsole.apply({ kind: "clear-output" }).ok).toBe(true)
    expect(developerConsole.snapshot().output).toEqual([])
    expect(history.textContent).toBe("")
  })

  test("rejects malformed and limit-plus-one batches without partial publication", () => {
    const { console: developerConsole } = mounted([], {
      limits: limits({ maxOutputBatchSegments: 3, maxOutputBatchUtf8Bytes: 258 }),
    })
    expect(
      developerConsole.apply({
        kind: "append-output",
        segments: Array.from({ length: 4 }, () => ({ kind: "normal" as const, text: "x" })),
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "OutputLimit" } })
    expect(developerConsole.snapshot().output).toEqual([])
    expect(
      developerConsole.apply({
        kind: "append-output",
        segments: [{ kind: "color", text: "x", color: [0, 0, 0, 256] as unknown as readonly [number, number, number, number] }],
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "OutputLimit" } })
    expect(developerConsole.snapshot().output).toEqual([])
  })

  test("submits exact empty, boundary, and Unicode text while enforcing the 255-byte entry ceiling", () => {
    const requests: ConsoleRequest[] = []
    const { console: developerConsole, root } = mounted(requests)
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    const submit = byName(root, "ConsoleSubmit")

    click(submit)
    expect(requests.at(-1)).toMatchObject({
      kind: "submission",
      text: "",
      maxExecutionUtf8Bytes: 510,
      maxExecutionArguments: 64,
    })
    expect(developerConsole.snapshot().output.map((segment) => segment.text).join("")).toBe("] \n")
    expect(developerConsole.snapshot().history).toEqual([])

    const boundary = "x".repeat(255)
    input(entry, boundary)
    expect(developerConsole.snapshot().entryText).toBe(boundary)
    const rejected = input(entry, `${boundary}x`)
    expect(rejected.defaultPrevented).toBe(false)
    expect(entry.value).toBe(boundary)
    expect(developerConsole.snapshot().diagnostics.at(-1)?.code).toBe("InputLimit")
    key(entry, "Enter")
    expect(requests.at(-1)).toMatchObject({ kind: "submission", text: boundary })

    const unicode = `${"é".repeat(125)}😀`
    expect(new TextEncoder().encode(unicode)).toHaveLength(254)
    input(entry, unicode)
    key(entry, "Enter")
    expect(requests.at(-1)).toMatchObject({ kind: "submission", text: unicode })

    input(entry, "valid")
    input(entry, String.fromCharCode(0xd800))
    expect(entry.value).toBe("valid")
    expect(developerConsole.snapshot().diagnostics.at(-1)?.code).toBe("MalformedText")
  })

  test("deduplicates bounded history and restores the partially typed draft", () => {
    const { console: developerConsole, root } = mounted([], {
      limits: limits({ maxHistoryItems: 3 }),
    })
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    for (const text of ["map first   ", "status", "MAP FIRST", "third", "fourth"]) {
      input(entry, text)
      key(entry, "Enter")
    }
    expect(developerConsole.snapshot().history).toEqual(["MAP FIRST", "third", "fourth"])

    input(entry, "partially typed")
    key(entry, "ArrowUp")
    expect(entry.value).toBe("fourth")
    key(entry, "ArrowUp")
    expect(entry.value).toBe("third")
    key(entry, "ArrowDown")
    expect(entry.value).toBe("fourth")
    key(entry, "ArrowDown")
    expect(entry.value).toBe("partially typed")
    expect(developerConsole.snapshot().historyCursor).toBeNull()
  })
})

describe("developer console completion and focus input", () => {
  test("sorts visible prefix matches, presents convar values, and cycles with Tab and Shift-Tab", () => {
    const { console: developerConsole, root } = mounted()
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    input(entry, "m")
    expect(developerConsole.snapshot().completion).toMatchObject({
      source: "catalog",
      labels: ["map", "mat_reload", "mat_wireframe 0"],
      visible: true,
    })
    expect(byName(root, "CompletionList").children.map((item) => item.textContent)).toEqual([
      "map",
      "mat_reload",
      "mat_wireframe 0",
    ])

    const tab = key(entry, "Tab")
    expect(tab.defaultPrevented).toBe(true)
    expect(entry.value).toBe("map ")
    key(entry, "Tab")
    expect(entry.value).toBe("mat_reload ")
    key(entry, "Tab", { shiftKey: true })
    expect(entry.value).toBe("map ")
  })

  test("bounds the visible popup while retaining complete keyboard cycling", () => {
    const many: ConsoleCatalog = Object.freeze({
      revision: "many",
      items: Object.freeze(
        Array.from({ length: 12 }, (_, index) =>
          Object.freeze({
            kind: "command" as const,
            name: `command_${String(index).padStart(2, "0")}`,
            disposition: "visible" as const,
            acceptsSuggestions: false,
          }),
        ),
      ),
    })
    const { console: developerConsole, root } = mounted([], { catalog: many })
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    input(entry, "c")
    expect(developerConsole.snapshot().completion.labels).toHaveLength(12)
    const popup = byName(root, "CompletionList")
    expect(popup.children).toHaveLength(10)
    expect(popup.children.at(-1)?.textContent).toBe("…")
    for (let index = 0; index < 12; index += 1) key(entry, "Tab")
    expect(entry.value).toBe("command_11 ")
  })

  test("accepts ordered owner suggestions, filters dispositions, and rejects stale or over-bound results", () => {
    const requests: ConsoleRequest[] = []
    const { console: developerConsole, root } = mounted(requests)
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    input(entry, "map j")
    const request = requests.at(-1)
    expect(request).toMatchObject({
      kind: "completion",
      catalogRevision: "catalog-1",
      commandName: "map",
      partialText: "map j",
      maxItems: 64,
      maxItemUtf8Bytes: 63,
    })
    if (!request || request.kind !== "completion") throw new Error("missing completion request")
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: {
          requestId: request.requestId,
          catalogRevision: request.catalogRevision,
          suggestions: [
            { text: "map jump_z", disposition: "visible" },
            { text: "map jump_hidden", disposition: "hidden" },
            { text: "map jump_a", disposition: "visible" },
          ],
        },
      }).ok,
    ).toBe(true)
    expect(developerConsole.snapshot().completion.labels).toEqual(["map jump_z", "map jump_a"])
    const first = byName(root, "Completion0")
    click(first)
    expect(entry.value).toBe("map jump_z ")
    expect(developerConsole.snapshot().completion.visible).toBe(false)

    input(entry, "map x")
    const staleRequest = requests.at(-1)
    if (!staleRequest || staleRequest.kind !== "completion") throw new Error("missing stale request")
    developerConsole.apply({ kind: "replace-catalog", catalog: catalog("catalog-2") })
    expect(requests).toContainEqual(
      expect.objectContaining({
        kind: "completion-cancelled",
        requestId: staleRequest.requestId,
        reason: "catalog-replaced",
      }),
    )
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: { requestId: staleRequest.requestId, catalogRevision: "catalog-1", suggestions: [] },
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "StaleCompletion" } })

    input(entry, "map y")
    const boundaryRequest = requests.at(-1)
    if (!boundaryRequest || boundaryRequest.kind !== "completion") throw new Error("missing boundary request")
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: {
          requestId: boundaryRequest.requestId,
          catalogRevision: boundaryRequest.catalogRevision,
          suggestions: [{ text: "x".repeat(63), disposition: "visible" }],
        },
      }).ok,
    ).toBe(true)

    input(entry, "map z")
    const invalidRequest = requests.at(-1)
    if (!invalidRequest || invalidRequest.kind !== "completion") throw new Error("missing invalid request")
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: {
          requestId: invalidRequest.requestId,
          catalogRevision: invalidRequest.catalogRevision,
          suggestions: [{ text: "x".repeat(64), disposition: "visible" }],
        },
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "InvalidCompletion" } })

    input(entry, "map q")
    const countBoundary = requests.at(-1)
    if (!countBoundary || countBoundary.kind !== "completion") throw new Error("missing count-boundary request")
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: {
          requestId: countBoundary.requestId,
          catalogRevision: countBoundary.catalogRevision,
          suggestions: Array.from({ length: 64 }, (_, index) => ({
            text: `map q${index}`,
            disposition: "visible" as const,
          })),
        },
      }).ok,
    ).toBe(true)
    expect(developerConsole.snapshot().completion.labels).toHaveLength(64)

    input(entry, "map r")
    const countOverflow = requests.at(-1)
    if (!countOverflow || countOverflow.kind !== "completion") throw new Error("missing count-overflow request")
    expect(
      developerConsole.apply({
        kind: "apply-completion",
        result: {
          requestId: countOverflow.requestId,
          catalogRevision: countOverflow.catalogRevision,
          suggestions: Array.from({ length: 65 }, (_, index) => ({
            text: `map r${index}`,
            disposition: "visible" as const,
          })),
        },
      }),
    ).toMatchObject({ ok: false, diagnostic: { code: "InvalidCompletion" } })
  })

  test("keeps entry focus for popup selection and emits a typed hide request for unmodified Backquote", () => {
    const requests: ConsoleRequest[] = []
    const { console: developerConsole, root, document } = mounted(requests)
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    input(entry, "m")
    expect(document.activeElement).toBe(entry)
    const backquote = key(entry, "`", { code: "Backquote" })
    expect(backquote.defaultPrevented).toBe(true)
    expect(requests.at(-1)).toMatchObject({
      kind: "visibility",
      operation: "hide",
      reason: "entry-backquote",
    })
    expect(developerConsole.snapshot().visible).toBe(true)
    developerConsole.apply({ kind: "hide" })
    expect(developerConsole.snapshot().visible).toBe(false)

    developerConsole.apply({ kind: "activate" })
    const count = requests.length
    const modified = key(entry, "~", { code: "Backquote", altKey: true })
    expect(modified.defaultPrevented).toBe(false)
    expect(requests).toHaveLength(count)
  })
})

describe("developer console lifecycle and cleanup", () => {
  test("replaces roots, cancels pending completion, and leaves no old nodes or listeners", () => {
    const requests: ConsoleRequest[] = []
    const document = new FakeDocument()
    const firstRoot = createRoot(document)
    const secondRoot = createRoot(document)
    const developerConsole = create(requests)
    developerConsole.apply({ kind: "mount", root: firstRoot as unknown as HTMLElement })
    developerConsole.apply({ kind: "activate" })
    input(byName(firstRoot, "ConsoleEntry"), "map j")
    const pending = requests.at(-1)
    if (!pending || pending.kind !== "completion") throw new Error("missing pending completion")

    expect(
      developerConsole.apply({ kind: "replace-root", root: secondRoot as unknown as HTMLElement }).ok,
    ).toBe(true)
    expect(firstRoot.children).toHaveLength(0)
    expect(secondRoot.children).toHaveLength(2)
    expect(requests.at(-1)).toMatchObject({
      kind: "completion-cancelled",
      requestId: pending.requestId,
      reason: "root-replaced",
    })
    expect(developerConsole.snapshot()).toMatchObject({
      lifecycle: "mounted",
      visible: true,
      focused: true,
      ownedResources: { observers: 0, timers: 0, listeners: 6 },
    })
  })

  test("repeated mount and destroy retains zero nodes, listeners, observers, timers, or late callbacks", () => {
    for (let iteration = 0; iteration < 25; iteration += 1) {
      const requests: ConsoleRequest[] = []
      const root = createRoot()
      const developerConsole = create(requests)
      developerConsole.apply({ kind: "append-output", segments: [{ kind: "normal", text: `run ${iteration}\n` }] })
      developerConsole.apply({ kind: "mount", root: root as unknown as HTMLElement })
      developerConsole.apply({ kind: "activate" })
      input(byName(root, "ConsoleEntry"), "map j")
      developerConsole.apply({ kind: "destroy" })
      expect(root.children).toHaveLength(0)
      expect(developerConsole.snapshot()).toMatchObject({
        lifecycle: "destroyed",
        visible: false,
        focused: false,
        ownedResources: { nodes: 0, listeners: 0, observers: 0, timers: 0 },
      })
      expect(developerConsole.apply({ kind: "destroy" }).ok).toBe(true)
      expect(developerConsole.apply({ kind: "activate" })).toMatchObject({
        ok: false,
        diagnostic: { code: "Destroyed" },
      })
    }
  })

  test("contains request sink failures and keeps immutable caller snapshots", () => {
    const sourceCatalog = {
      revision: "mutable",
      items: [{ kind: "command" as const, name: "map", disposition: "visible" as const, acceptsSuggestions: true }],
    }
    const { console: developerConsole, root } = mounted([], {
      catalog: sourceCatalog,
      onRequest: () => {
        throw new Error("owner failed")
      },
    })
    sourceCatalog.revision = "changed"
    sourceCatalog.items[0].name = "changed"
    developerConsole.apply({ kind: "activate" })
    const entry = byName(root, "ConsoleEntry")
    input(entry, "map x")
    expect(developerConsole.snapshot().catalogRevision).toBe("mutable")
    expect(developerConsole.snapshot().diagnostics.at(-1)?.code).toBe("RequestSinkFailure")
    expect(developerConsole.snapshot().entryText).toBe("map x")
  })
})
