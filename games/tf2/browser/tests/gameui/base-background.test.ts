import { expect, test } from "bun:test"
import { TF2_MAIN_MENU_STATE } from "../../src/gameui"
import type { Tf2GameUiState } from "../../src/gameui"
import { tf2GameUiBaseBackground } from "../../src/gameui-integration/base-background"
import type { Tf2GameUiBackgroundDescriptor } from "../../src/ui-integration"

const descriptor: Tf2GameUiBackgroundDescriptor = Object.freeze({
  identity: "tf2-gameui-background-test", contentBuild: "24245096",
  source: Object.freeze({ logicalPath: "scripts/chapterbackgrounds.txt", byteLength: 1, sha256: "0".repeat(64) }),
  defaultChapter: 1, backgroundName: "background_2fort",
  variants: Object.freeze(["standard", "widescreen"].map((aspect, index) => Object.freeze({
    aspect: aspect as "standard" | "widescreen", image: `../console/background_2fort${index ? "_widescreen" : ""}`,
    material: `materials/console/background_2fort${index ? "_widescreen" : ""}.vmt`, materialSha256: `${index}`.repeat(64),
    texture: `materials/console/background_2fort${index ? "_widescreen" : ""}.vtf`, textureSha256: `${index}`.repeat(64), width: 1, height: 1,
  }))),
})

test("selects one opaque stretched base background independently from the character", () => {
  expect(tf2GameUiBaseBackground(descriptor, TF2_MAIN_MENU_STATE, { width: 1280, height: 720, devicePixelRatio: 1 })).toMatchObject({
    visible: true, alpha: 255, geometry: "stretch", bounds: { x: 0, y: 0, width: 1280, height: 720 },
    variant: { aspect: "widescreen", image: "../console/background_2fort_widescreen" },
  })
  expect(tf2GameUiBaseBackground(descriptor, TF2_MAIN_MENU_STATE, { width: 1192, height: 1339, devicePixelRatio: 2 })).toMatchObject({
    visible: true, bounds: { x: 0, y: 0, width: 1192, height: 1339 }, variant: { aspect: "standard" },
  })
})

test("hides the ordinary base background outside the disconnected Main Menu", () => {
  for (const kind of ["in-game", "pause", "loading", "failure", "disconnecting"] as const) {
    const state = kind === "loading" ? Object.freeze({ kind, mapIdentity: "jump_beef", phase: "connecting", progress: 0, statusText: "" })
      : kind === "failure" ? Object.freeze({ kind, reason: "failure", extendedReason: "" })
        : kind === "disconnecting" ? Object.freeze({ kind, origin: "pause" as const, mapIdentity: "jump_beef" })
          : Object.freeze({ kind })
    expect(tf2GameUiBaseBackground(descriptor, state as Tf2GameUiState, { width: 1280, height: 720, devicePixelRatio: 1 }).visible).toBeFalse()
  }
})
