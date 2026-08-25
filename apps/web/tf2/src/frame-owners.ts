import type { ApplicationView } from "./runtime"

export const GAME_UI_FRAME_OWNER = 1
export const LOADING_FRAME_OWNER = 2
export const HUD_FRAME_OWNER = 4
export const OPTIONS_FRAME_OWNER = 8

export function visibleFrameOwners(
  view: Pick<ApplicationView, "phase" | "gameUi" | "optionsVisible">,
  gameUiRevealed: boolean,
): number {
  if (view.phase === "Closed") return 0
  const gameUi = gameUiRevealed && view.gameUi !== "in-game"
  const loading = view.gameUi === "loading" || view.gameUi === "failure"
  const hud = view.gameUi === "in-game" || view.gameUi === "pause"
  const options = view.optionsVisible === true
  return Number(gameUi) * GAME_UI_FRAME_OWNER
    | Number(loading) * LOADING_FRAME_OWNER
    | Number(hud) * HUD_FRAME_OWNER
    | Number(options) * OPTIONS_FRAME_OWNER
}
