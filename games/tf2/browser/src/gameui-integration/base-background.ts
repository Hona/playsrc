import type { VguiViewport } from "@playsrc/vgui"
import type { Tf2GameUiState } from "../gameui"
import type { Tf2GameUiBackgroundDescriptor } from "../ui-integration"

export type Tf2GameUiBaseBackgroundPresentation = Readonly<{
  visible: boolean
  alpha: 255
  geometry: "stretch"
  bounds: Readonly<{ x: 0; y: 0; width: number; height: number }>
  variant: Tf2GameUiBackgroundDescriptor["variants"][number]
}>

export function tf2GameUiBaseBackground(
  descriptor: Tf2GameUiBackgroundDescriptor,
  state: Tf2GameUiState,
  viewport: VguiViewport,
): Tf2GameUiBaseBackgroundPresentation {
  const aspect = viewport.width / viewport.height >= 1.5999 ? "widescreen" : "standard"
  const variant = descriptor.variants.find((candidate) => candidate.aspect === aspect)
  if (!variant) throw new Error(`TF2 GameUI base-background ${aspect} variant is missing`)
  return Object.freeze({
    visible: state.kind === "main-menu",
    alpha: 255,
    geometry: "stretch",
    bounds: Object.freeze({ x: 0, y: 0, width: viewport.width, height: viewport.height }),
    variant,
  })
}
