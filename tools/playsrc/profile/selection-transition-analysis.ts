import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { decodeScreenshot } from "./screenshot-pixels"
import { requireMacPageAdmission } from "./macos-page-admission"
import { requireStartupNative } from "./static-startup-gate"
import { selectionVisibleLatency } from "./selection-visible-latency"

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
    const screen = windows ? captures[reference.index].admission.pixels.bounds : captures[reference.index].admission.snapshot.screens[0]
    if (!windows && captures[reference.index].admission.snapshot.screens.length !== 1) throw new Error("Native selection sample requires one exact primary display")
    if (images.some(image => image.width !== images[0]!.width || image.height !== images[0]!.height || image.channels !== images[0]!.channels)
      || captures.some((capture: any) => JSON.stringify(windows ? capture.nativeRecords[0].facts.bounds : capture.admission.page.bounds)
        !== JSON.stringify(windows ? captures[0].nativeRecords[0].facts.bounds : captures[0].admission.page.bounds))) throw new Error("Native sample geometry changed")
    const scale = image.width / screen.Width, border = (facts.outerWidth - facts.innerWidth) / 2
    const x = Math.ceil((facts.screenX - screen.X + border + facts.bounds.x) * scale), y = Math.ceil((facts.screenY - screen.Y + facts.outerHeight - facts.innerHeight - border + facts.bounds.y) * scale)
    const width = Math.floor(facts.bounds.width * scale), height = Math.floor(facts.bounds.height * scale)
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
    const result = selectionVisibleLatency(input.inputEpoch, endedEpoch, captures.map((capture: any, at: number) => {
      const pixelsMatch = mask.filter(offset => [0, 1, 2].every(channel => Math.abs(images[at]!.pixels[offset + channel]! - image.pixels[offset + channel]!) <= 2)).length >= mask.length * 0.95
      // Historical records lacking the narrower native-call interval retain
      // their original conservative envelope; they are not re-timestamped.
      const interval = capture.pixelCaptureInterval ?? capture
      return { startedEpoch: interval.startedEpoch, endedEpoch: interval.endedEpoch,
        matches: !pixelsMatch ? false : interval.startedEpoch >= drawEpoch ? true : null }
    }))
    return { scene: reference.scene, input: input.name, glyphPixels: mask.length, reference: captures[reference.index].file, ...result }
  })
  await writeFile(path.join(directory, "selection-latency.json"), JSON.stringify(latencies, null, 2))
  return latencies
}
