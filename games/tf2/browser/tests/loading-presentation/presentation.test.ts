import { describe, expect, test } from "bun:test"
import { TF2_LOCAL_LOADING_PHASES, TF2_MAIN_MENU_STATE, transitionTf2GameUi, type Tf2GameUiState, type Tf2LoadingState } from "../../src/gameui"
import { createTf2LoadingPresentation, resolveTf2LoadingBackground, TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "../../src/loading-presentation"
import { tf2UiResources } from "../../src/ui-resources"

const resource = (path: string) => tf2UiResources.panels.find((panel) => panel.source.logicalPath === path)!
const loadingResource = resource("resource/loadingdialognobanner.res")
const failureResource = resource("resource/loadingdialogerror.res")
const background = resolveTf2LoadingBackground({ generation: 1, mapIdentity: "jump_beef", viewport: { width: 1_280, height: 720 }, mapPhotoLookups: TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS.map((location) => ({ location, outcome: "missing" as const })), backingMaterial: TF2_STAMP_BACKGROUND.material, backingTexture: TF2_STAMP_BACKGROUND.texture })
const loading = (): Tf2LoadingState => transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "loading-started", mapIdentity: "jump_beef" }).state as Tf2LoadingState

describe("TF2 loading presentation operations", () => {
  test("maps every owner milestone directly without synthetic progress", () => {
    const presentation = createTf2LoadingPresentation({ loadingResource, failureResource })
    let state: Tf2GameUiState = loading()
    for (const phase of TF2_LOCAL_LOADING_PHASES.slice(1)) {
      state = transitionTf2GameUi(state, { kind: "loading-progress", phase: phase.identity }).state
      const snapshot = presentation.update(1, state, { width: 1_280, height: 720 }, background)
      expect(snapshot?.operations.find((operation) => operation.kind === "progress")).toEqual({ kind: "progress", control: "Progress", value: (state as Tf2LoadingState).progress })
      expect(snapshot?.operations.find((operation) => operation.kind === "status")).toEqual({ kind: "status", control: "InfoLabel", text: (state as Tf2LoadingState).statusText })
    }
  })

  test("emits exact desktop and tall-viewport dialog geometry and suppresses repeats", () => {
    const presentation = createTf2LoadingPresentation({ loadingResource, failureResource })
    expect(presentation.update(1, loading(), { width: 1_280, height: 720 }, background)?.operations.find((operation) => operation.kind === "bounds"))
      .toEqual({ kind: "bounds", x: 890, y: 598, width: 380, height: 112 })
    expect(presentation.update(1, loading(), { width: 390, height: 844 }, background)?.operations.find((operation) => operation.kind === "bounds"))
      .toEqual({ kind: "bounds", x: 0, y: 722, width: 380, height: 112 })
    expect(presentation.update(1, loading(), { width: 390, height: 844 }, background)).toBeNull()
  })

  test("rejects stale generations and routes Cancel, failure Close, success, and destroy", () => {
    const presentation = createTf2LoadingPresentation({ loadingResource, failureResource })
    const load = loading()
    expect(presentation.update(2, load, { width: 1_280, height: 720 }, null)?.stateKind).toBe("loading")
    expect(presentation.update(1, load, { width: 1_280, height: 720 }, null)).toBeNull()
    expect(presentation.activate("Cancel")).toEqual({ kind: "disconnect" })

    const failed = transitionTf2GameUi(load, { kind: "loading-failed", reason: "Failed", extendedReason: "Detail" }).state
    const failure = presentation.update(2, failed, { width: 1_280, height: 720 }, null)
    expect(failure?.operations).toContainEqual({ kind: "button", control: "CancelButton", text: "#GameUI_Close", command: "Close" })
    expect(failure?.operations).toContainEqual({ kind: "status", control: "InfoLabel", text: "Failed\nDetail" })
    expect(presentation.activate("Close")).toEqual({ kind: "dismiss-failure" })

    const game = transitionTf2GameUi(load, { kind: "loading-succeeded" }).state
    expect(presentation.update(3, game, { width: 1_280, height: 720 }, null)?.operations).toEqual([{ kind: "unmount" }])
    expect(presentation.destroy()).toBeNull()
    expect(presentation.destroy()).toBeNull()
  })
})
