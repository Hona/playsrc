import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { decodeScreenshot, type DecodedScreenshot } from "./screenshot-pixels"
import { requireMacPageAdmission } from "./macos-page-admission"
import { requireStartupNative } from "./static-startup-gate"
import { selectionVisibleLatency } from "./selection-visible-latency"

export function nativeSelectionRect(image: Pick<DecodedScreenshot, "width" | "height">,
  pixels: { X: number; Y: number; Width: number; Height: number }, window: { left: number; top: number; width: number; height: number },
  facts: { screenX: number; screenY: number; outerWidth: number; outerHeight: number; innerWidth: number; innerHeight: number; bounds: { x: number; y: number; width: number; height: number } }) {
  if (pixels.X > window.left || pixels.Y > window.top || pixels.X + pixels.Width < window.left + window.width || pixels.Y + pixels.Height < window.top + window.height) {
    throw new Error("Native pixels must include the entire measured window; a cropped admission region is invalid")
  }
  const scale = image.width / pixels.Width, border = (facts.outerWidth - facts.innerWidth) / 2
  if (!(scale > 0) || Math.abs(image.height / pixels.Height - scale) > 1e-6) throw new Error("Native pixel scale differs by axis")
  const x = Math.ceil((facts.screenX - pixels.X + border + facts.bounds.x) * scale)
  const y = Math.ceil((facts.screenY - pixels.Y + facts.outerHeight - facts.innerHeight - border + facts.bounds.y) * scale)
  const width = Math.floor(facts.bounds.width * scale), height = Math.floor(facts.bounds.height * scale)
  if (x < 0 || y < 0 || x + width > image.width || y + height > image.height) throw new Error("Authored glyph rectangle escapes native pixels")
  return { x, y, width, height, scale }
}

export async function analyzeNativeSelectionPixels(directory: string) {
  const captures = JSON.parse(await readFile(path.join(directory, "selection-native.json"), "utf8"))
  const measurement = JSON.parse(await readFile(path.join(directory, "selection-measurement.json"), "utf8"))
  const { references, evidence, endedEpoch } = measurement
  const images = await Promise.all(captures.map(async (capture: any) => {
    if (capture.method === "windows-native-desktop") { requireStartupNative(capture.admission); requireStartupNative(capture.admissionAfter) }
    else { requireMacPageAdmission(capture.admission); if (capture.admissionAfter) requireMacPageAdmission(capture.admissionAfter) }
    const bytes = await readFile(path.join(directory, capture.file))
    if (bytes.byteLength !== capture.byteLength || createHash("sha256").update(bytes).digest("hex") !== capture.sha256) throw new Error("Native pixel provenance differs")
    return decodeScreenshot(bytes)
  }))
  const latencies = references.map((reference: any, index: number) => {
    const image = images[reference.index]!, facts = reference.facts
    const windows = captures[reference.index].method === "windows-native-desktop"
    if (!windows && captures[reference.index].admission.snapshot.screens.length !== 1) throw new Error("Native selection sample requires one exact primary display")
    if (captures.some((capture: any) => JSON.stringify(windows ? capture.nativeRecords[0].facts.bounds : capture.admission.page.bounds)
        !== JSON.stringify(windows ? captures[0].nativeRecords[0].facts.bounds : captures[0].admission.page.bounds))) throw new Error("Native sample geometry changed")
    const rectangles = captures.map((capture: any, index: number) => nativeSelectionRect(images[index]!,
      windows ? capture.admission.pixels.bounds : capture.admission.snapshot.screens[0],
      windows ? capture.nativeRecords[0].facts.bounds : { left: capture.admission.page.bounds.X, top: capture.admission.page.bounds.Y, width: capture.admission.page.bounds.Width, height: capture.admission.page.bounds.Height }, facts))
    const { x, y, width, height, scale } = rectangles[reference.index]!
    if (rectangles.some(rect => rect.width !== width || rect.height !== height || rect.scale !== scale)) throw new Error("Authored native sample scale changed")
    const colors = new Map<string, number[]>()
    for (let row = y; row < y + height; row++) for (let column = x; column < x + width; column++) {
      const at = (row * image.width + column) * image.channels, rgb = [...image.pixels.slice(at, at + 3)]
      // The configured TanLight glyph is [198,188,164] in native pixels; a
      // white-only mask incorrectly rejected its authored blue channel.
      if (rgb[0]! > 170 && rgb[1]! > 170 && rgb[2]! > 150) { const key = rgb.join(","); const points = colors.get(key) ?? []; points.push(at); colors.set(key, points) }
    }
    const mask = [...colors.values()].sort((a, b) => b.length - a.length)[0] ?? []
    if (mask.length <= 16 || mask.length >= width * height * 0.5) throw new Error(`${reference.scene} lacks distinct authored native glyph pixels`)
    const input = evidence.inputs[index], drawEpoch = facts.timeOrigin + facts.draw.at
    if (!input?.trusted || facts.draw.detail.scene !== reference.scene) throw new Error("Selection input/scene ownership is incomplete")
    if (reference.scene === "world" && (facts.draw.detail.lifecycle !== 1 || facts.draw.detail.class !== measurement.identity
      || facts.draw.detail.team !== (measurement.team === "red" ? 2 : 3))) throw new Error("World pixels do not belong to the selected alive player")
    const result = selectionVisibleLatency(input.inputEpoch, endedEpoch, captures.map((capture: any, at: number) => {
      const sampled = images[at]!, rectangle = rectangles[at]!
      const pixelsMatch = mask.filter(offset => {
        const pixel = offset / image.channels, column = pixel % image.width - x, row = Math.floor(pixel / image.width) - y
        const sampledOffset = ((rectangle.y + row) * sampled.width + rectangle.x + column) * sampled.channels
        return [0, 1, 2].every(channel => Math.abs(sampled.pixels[sampledOffset + channel]! - image.pixels[offset + channel]!) <= 2)
      }).length >= mask.length * 0.95
      // Historical records lacking the narrower native-call interval retain
      // their original conservative envelope; they are not re-timestamped.
      const interval = capture.pixelCaptureInterval ?? capture
      return { startedEpoch: interval.startedEpoch, endedEpoch: interval.endedEpoch,
        matches: !pixelsMatch ? false : interval.startedEpoch >= drawEpoch ? true : null }
    }))
    return { scene: reference.scene, input: input.name, glyphPixels: mask.length, reference: captures[reference.index].file,
      over250Milliseconds: result.upperMilliseconds === null || result.upperMilliseconds > 250, ...result }
  })
  await writeFile(path.join(directory, "selection-latency.json"), JSON.stringify(latencies, null, 2))
  return latencies
}
