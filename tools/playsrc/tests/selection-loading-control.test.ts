import { test, expect } from "bun:test"
import { selectionLoadingControl } from "../profile/selection-loading-control"

test("loading control observes transfers including zero/cache/failure without altering network state", async () => {
  const calls: string[] = [], events = new Map<string, (event: any) => void>()
  const control = await selectionLoadingControl({ send: async (name: string) => { calls.push(name) },
    on: (name: string, callback: (event: any) => void) => events.set(name, callback), off: (name: string) => events.delete(name) } as any)
  try {
    control.boundary("map-request")
    events.get("Network.requestWillBeSent")!({ requestId: "1", timestamp: 1, wallTime: 100,
      request: { url: "http://127.0.0.1:1555/objects/abc?private=secret", method: "GET" }, type: "Fetch" })
    events.get("Network.responseReceived")!({ requestId: "1", timestamp: 2,
      response: { status: 200, fromDiskCache: true, protocol: "http/1.1" } })
    events.get("Network.requestServedFromCache")!({ requestId: "1" })
    events.get("Network.loadingFinished")!({ requestId: "1", timestamp: 3, encodedDataLength: 0 })
    const report = control.stop()
    expect(calls).toEqual(["Network.enable"])
    expect(report.requests[0]).toMatchObject({ path: "/objects/abc", diskCache: true, servedFromCache: true, encodedBytes: 0 })
    expect(JSON.stringify(report)).not.toContain("secret")
    expect(report.boundaries[0].name).toBe("map-request")
    expect(report.pressure.length).toBeGreaterThanOrEqual(2)
    expect(report.unfinishedRequests).toBe(0)
    expect(report.complete).toBe(true)
    expect(events.size).toBe(0)
    expect(control.stop()).toBe(report)
  } finally { control.stop() }
})

test("loading control never retains inline image bodies and bounds all interval ledgers", async () => {
  const events = new Map<string, (event: any) => void>()
  const control = await selectionLoadingControl({ send: async () => {}, on: (name: string, callback: (event: any) => void) => events.set(name, callback), off: (name: string) => events.delete(name) } as any)
  try {
    const request = events.get("Network.requestWillBeSent")!, finish = events.get("Network.loadingFinished")!
    for (let i = 0; i < 4100; i++) {
      request({ requestId: String(i), timestamp: i, wallTime: i, request: { url: "data:image/png;base64,PRIVATE_IMAGE_BODY", method: "GET" }, type: "Image" })
      finish({ requestId: String(i), timestamp: i + 1, encodedDataLength: 0 })
    }
    for (let i = 0; i < 40; i++) control.boundary(`phase-${i}`)
    const result = control.stop()
    expect(result.requests).toHaveLength(4096)
    expect(result.requests[0]).toMatchObject({ path: null, scheme: "data:", encodedBytes: 0 })
    expect(JSON.stringify(result)).not.toContain("PRIVATE_IMAGE_BODY")
    expect(result.boundaries).toHaveLength(32)
    expect(result.pressure.length).toBeLessThanOrEqual(256)
    expect(result.dropped).toEqual({ requests: 4, boundaries: 8, pressure: 0 })
    expect(result.complete).toBe(false)
    expect(result.unfinishedRequests).toBe(0)
    expect(() => control.boundary("late")).toThrow("stopped")
  } finally { control.stop() }
})
