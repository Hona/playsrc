import { test, expect } from "bun:test"
import { selectionLoadingControl } from "../profile/selection-loading-control"

test("loading control observes transfers including zero/cache/failure without altering network state", async () => {
  const calls: string[] = [], events = new Map<string, (event: any) => void>()
  const control = await selectionLoadingControl({ send: async (name: string) => { calls.push(name) },
    on: (name: string, callback: (event: any) => void) => events.set(name, callback) } as any)
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
  } finally { control.stop() }
})
