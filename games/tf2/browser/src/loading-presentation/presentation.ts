import type { Tf2GameUiState } from "../gameui"
import type { Tf2UiPanelDocument } from "../ui-resources"
import type { Tf2LoadingBackgroundResult } from "./background"

export type Tf2LoadingPresentationRequest =
  | Readonly<{ kind: "disconnect" }>
  | Readonly<{ kind: "dismiss-failure" }>

export type Tf2LoadingVguiOperation =
  | Readonly<{ kind: "mount"; resource: Tf2UiPanelDocument; modal: true }>
  | Readonly<{ kind: "bounds"; x: number; y: number; width: number; height: number }>
  | Readonly<{ kind: "failure-layout"; placement: "screen-center"; contentBottomPadding: 50; buttonGap: 6 }>
  | Readonly<{ kind: "status"; control: "InfoLabel"; text: string }>
  | Readonly<{ kind: "progress"; control: "Progress"; value: number }>
  | Readonly<{ kind: "button"; control: "CancelButton"; text: "#GameUI_Cancel" | "#GameUI_Close"; command: "Cancel" | "Close" }>
  | Readonly<{ kind: "unmount" }>

export type Tf2LoadingPresentationSnapshot = Readonly<{
  generation: number
  stateKind: Tf2GameUiState["kind"]
  background: Extract<Tf2LoadingBackgroundResult, { ok: true }> | null
  operations: readonly Tf2LoadingVguiOperation[]
}>

export type Tf2LoadingPresentation = Readonly<{
  update(generation: number, state: Tf2GameUiState, viewport: Readonly<{ width: number; height: number }>, background: Tf2LoadingBackgroundResult | null): Tf2LoadingPresentationSnapshot | null
  activate(command: "Cancel" | "Close"): Tf2LoadingPresentationRequest | null
  destroy(): Tf2LoadingPresentationSnapshot | null
}>

export type Tf2LoadingPresentationInput = Readonly<{
  loadingResource: Tf2UiPanelDocument
  failureResource: Tf2UiPanelDocument
}>

export function createTf2LoadingPresentation(input: Tf2LoadingPresentationInput): Tf2LoadingPresentation {
  let generation = 0
  let lastKey = ""
  let currentKind: Tf2GameUiState["kind"] = "main-menu"
  let destroyed = false

  const frozenSnapshot = (stateKind: Tf2GameUiState["kind"], background: Extract<Tf2LoadingBackgroundResult, { ok: true }> | null, operations: Tf2LoadingVguiOperation[]): Tf2LoadingPresentationSnapshot => Object.freeze({
    generation,
    stateKind,
    background,
    operations: Object.freeze(operations.map((operation) => Object.freeze(operation))),
  })

  return Object.freeze({
    update: (nextGeneration, state, viewport, resolved) => {
      if (destroyed || !Number.isSafeInteger(nextGeneration) || nextGeneration < generation) return null
      if (resolved && (!resolved.ok || resolved.generation !== nextGeneration)) return null
      generation = nextGeneration
      currentKind = state.kind
      const background = resolved?.ok ? resolved : null
      let operations: Tf2LoadingVguiOperation[]
      if (state.kind === "loading") {
        operations = [
          { kind: "mount", resource: input.loadingResource, modal: true },
          { kind: "bounds", x: viewport.width - 390, y: viewport.height - 122, width: 380, height: 112 },
          { kind: "status", control: "InfoLabel", text: state.statusText },
          { kind: "progress", control: "Progress", value: state.progress },
          { kind: "button", control: "CancelButton", text: "#GameUI_Cancel", command: "Cancel" },
        ]
      } else if (state.kind === "failure") {
        operations = [
          { kind: "mount", resource: input.failureResource, modal: true },
          { kind: "failure-layout", placement: "screen-center", contentBottomPadding: 50, buttonGap: 6 },
          { kind: "status", control: "InfoLabel", text: state.failure.extendedReason ? `${state.failure.reason}\n${state.failure.extendedReason}` : state.failure.reason },
          { kind: "button", control: "CancelButton", text: "#GameUI_Close", command: "Close" },
        ]
      } else operations = [{ kind: "unmount" }]
      const key = JSON.stringify({ generation, state, viewport, background, operations })
      if (key === lastKey) return null
      lastKey = key
      return frozenSnapshot(state.kind, background, operations)
    },
    activate: (command) => {
      if (destroyed) return null
      if (command === "Cancel" && currentKind === "loading") return Object.freeze({ kind: "disconnect" })
      if (command === "Close" && currentKind === "failure") return Object.freeze({ kind: "dismiss-failure" })
      return null
    },
    destroy: () => {
      if (destroyed) return null
      destroyed = true
      currentKind = "main-menu"
      return frozenSnapshot("main-menu", null, [{ kind: "unmount" }])
    },
  })
}
