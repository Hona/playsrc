import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { createHash } from "node:crypto"
import { decodeScreenshot } from "./screenshot-pixels"
import { nativeSelectionRect } from "./selection-transition-analysis"
import { selectionVisibleLatency } from "./selection-visible-latency"

export async function analyzeEquipmentNavigation(directory: string) {
  const { captures, references } = JSON.parse(await readFile(path.join(directory, "equipment-native.json"), "utf8"))
  const images = await Promise.all(captures.map(async (capture: any) => {
    const bytes = await readFile(path.join(directory, capture.file))
    if (bytes.length !== capture.byteLength || createHash("sha256").update(bytes).digest("hex") !== capture.sha256) throw new Error("Equipment pixel provenance differs")
    return decodeScreenshot(bytes)
  }))
  const latencies = references.map((reference: any) => {
    const image = images[reference.index]!
    const rects = captures.map((capture: any, index: number) => nativeSelectionRect(images[index]!, capture.admission.pixels.bounds, capture.nativeRecords[0].facts.bounds, reference.facts))
    const expected = rects[reference.index]!
    let distinct = 0
    for (let y = 0; y < expected.height; y++) for (let x = 0; x < expected.width; x++) {
      const at = ((expected.y + y) * image.width + expected.x + x) * image.channels
      if (image.pixels[at]! > 100) distinct++
    }
    if (distinct <= 16) throw new Error(`${reference.name} lacks authored native pixels`)
    const samples = captures.map((capture: any, index: number) => {
      const sampled = images[index]!, rect = rects[index]!
      let different = 0
      for (let y = 0; y < expected.height; y++) for (let x = 0; x < expected.width; x++) {
        const a = ((expected.y + y) * image.width + expected.x + x) * image.channels, b = ((rect.y + y) * sampled.width + rect.x + x) * sampled.channels
        if ([0, 1, 2].some(channel => Math.abs(image.pixels[a + channel]! - sampled.pixels[b + channel]!) > 2)) different++
      }
      return { ...capture.admission.pixels, matches: different < expected.width * expected.height * 0.01 }
    })
    return { name: reference.name, input: reference.facts.input, distinctPixels: distinct,
      ...selectionVisibleLatency(reference.facts.input.inputEpoch, samples.at(-1)!.endedEpoch, samples) }
  })
  await writeFile(path.join(directory, "equipment-latency.json"), JSON.stringify(latencies, null, 2))
  return latencies
}
