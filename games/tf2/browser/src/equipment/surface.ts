import type { VguiPanelId, VguiRect, VguiRuntime, VguiViewport } from "@playsrc/vgui"

/** A model panel clips a full-viewport renderer; it never rescales that renderer. */
export function attachEquipmentSurface(runtime: VguiRuntime, panel: VguiPanelId, canvas: HTMLCanvasElement, viewport: VguiViewport, bounds: VguiRect): () => void {
  const parent = canvas.parentElement, next = canvas.nextSibling, css = canvas.style.cssText
  const wrapper = canvas.ownerDocument.createElement("div")
  wrapper.dataset.equipmentViewportClip = "true"
  Object.assign(wrapper.style, { position: "absolute", inset: "0", overflow: "hidden", width: "100%", height: "100%", pointerEvents: "none" })
  Object.assign(canvas.style, { position: "absolute", width: `${viewport.width}px`, height: `${viewport.height}px`,
    left: `${-bounds.x}px`, top: `${-bounds.y}px`, backgroundColor: "transparent", visibility: "visible", pointerEvents: "none" })
  wrapper.append(canvas)
  const detach = runtime.attachSurface(panel, wrapper)
  let attached = true
  return () => {
    if (!attached) return
    attached = false
    detach()
    canvas.style.cssText = css
    if (parent) parent.insertBefore(canvas, next?.parentNode === parent ? next : null)
    else canvas.remove()
    wrapper.remove()
  }
}
