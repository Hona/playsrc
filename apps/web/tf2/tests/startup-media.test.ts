import { expect, test } from "bun:test"
import { loadStartupMetadata, validateStartupMetadata } from "../src/startup-media"

const expected = { video: { width: 1440, height: 1080 }, durationMicroseconds: 10_051_000 }
const observed = { videoWidth: 1440, videoHeight: 1080, duration: 10.051, readyState: 1, networkState: 1 }

test("retains exact metadata validation with bounded numeric diagnostics, including nonfinite durations", () => {
  expect(() => validateStartupMetadata(observed, expected, 1, true)).not.toThrow()
  for (const changed of [{ duration: NaN }, { duration: Infinity }, { duration: 0 }, { duration: 10 }, { videoWidth: 0 }, { videoHeight: 720 }]) {
    expect(() => validateStartupMetadata({ ...observed, ...changed }, expected, 2, true)).toThrow(/Configured startup media metadata differs:.*"generation":2.*"expected":.*"observed":/)
  }
  expect(() => validateStartupMetadata(observed, expected, 3, false)).toThrow('"sourceMatches":false')
})

test("loads a new source before accepting metadata and ignores stale prior-source events", async () => {
  const target = new EventTarget() as HTMLVideoElement
  Object.assign(target, { ...observed, readyState: 4, currentSrc: "blob:old", src: "blob:old", load() {} })
  let completed = false
  const waiting = loadStartupMetadata(target, "blob:new", new AbortController().signal).then(() => { completed = true })
  target.dispatchEvent(new Event("loadedmetadata"))
  await Promise.resolve()
  expect(completed).toBe(false)
  expect(target.src).toBe("blob:new")
  Object.assign(target, { currentSrc: "blob:new" })
  target.dispatchEvent(new Event("loadedmetadata"))
  await waiting
  expect(completed).toBe(true)
})

test("cancels pending metadata preparation rather than retaining an old media listener", async () => {
  const target = new EventTarget() as HTMLVideoElement
  Object.assign(target, { currentSrc: "", load() {} })
  const controller = new AbortController()
  const pending = loadStartupMetadata(target, "blob:new", controller.signal)
  controller.abort()
  await expect(pending).rejects.toMatchObject({ name: "AbortError" })
  Object.assign(target, { currentSrc: "blob:new" })
  target.dispatchEvent(new Event("loadedmetadata"))
})
