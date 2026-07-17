import { describe, expect, test } from "bun:test"
import {
  admitLocalPlatformFonts,
  classifyBrowserPlatform,
  type LocalPlatformFontAdapter,
  type LocalPlatformFontTarget,
} from "../src"

const target: LocalPlatformFontTarget = Object.freeze({
  identity: "test/windows-console-fonts",
  requiredPlatform: "windows",
  faces: Object.freeze([
    Object.freeze({
      identity: "tahoma-normal",
      localName: "Tahoma",
      browserFamily: "playsrc-test-composite",
      weight: 0,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0000, 0x00ff] as const),
    }),
    Object.freeze({
      identity: "lucida-console-medium",
      localName: "Lucida Console",
      browserFamily: "playsrc-test-composite",
      weight: 500,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0100, 0xffff] as const),
    }),
  ]),
})

function adapter(
  overrides: Partial<LocalPlatformFontAdapter> = {},
): LocalPlatformFontAdapter {
  return Object.freeze({
    reportedPlatform: "Windows",
    async loadLocalFace(request) { return Object.freeze({ identity: request.identity }) },
    addLoadedFace() {},
    removeLoadedFace() {},
    ...overrides,
  })
}

describe("browser-local platform fonts", () => {
  test("classifies only explicit desktop platform reports", () => {
    expect(classifyBrowserPlatform("Win32")).toBe("windows")
    expect(classifyBrowserPlatform("Windows")).toBe("windows")
    expect(classifyBrowserPlatform("MacIntel")).toBe("macos")
    expect(classifyBrowserPlatform("Linux x86_64")).toBe("linux")
    expect(classifyBrowserPlatform("Linux Android")).toBe("other")
    expect(classifyBrowserPlatform(null)).toBe("other")
  })

  test("rejects a non-target host before probing installed faces", async () => {
    const loaded: string[] = []
    const result = await admitLocalPlatformFonts(target, adapter({
      reportedPlatform: "MacIntel",
      async loadLocalFace(request) {
        loaded.push(request.identity)
        return request
      },
    }))
    expect(result).toEqual({
      kind: "unsupported",
      targetIdentity: target.identity,
      platform: "macos",
      reason: "unsupported-platform",
      unadmittedFaceIdentities: ["tahoma-normal", "lucida-console-medium"],
    })
    expect(loaded).toEqual([])
  })

  test("loads every isolated local face but publishes none when one is unavailable", async () => {
    const loaded: string[] = []
    const published: unknown[] = []
    const result = await admitLocalPlatformFonts(target, adapter({
      async loadLocalFace(request) {
        loaded.push(request.identity)
        if (request.identity === "lucida-console-medium") throw new Error("NetworkError")
        return Object.freeze({ identity: request.identity })
      },
      addLoadedFace(face) { published.push(face) },
    }))
    expect(result).toEqual({
      kind: "unsupported",
      targetIdentity: target.identity,
      platform: "windows",
      reason: "local-face-unavailable",
      unadmittedFaceIdentities: ["lucida-console-medium"],
    })
    expect(loaded).toEqual(["tahoma-normal", "lucida-console-medium"])
    expect(published).toEqual([])
  })

  test("publishes the complete loaded set in declared order", async () => {
    const events: string[] = []
    const result = await admitLocalPlatformFonts(target, adapter({
      async loadLocalFace(request) {
        events.push(`load:${request.identity}`)
        return Object.freeze({ identity: request.identity })
      },
      addLoadedFace(face) { events.push(`add:${(face as { identity: string }).identity}`) },
    }))
    expect(events).toEqual([
      "load:tahoma-normal",
      "load:lucida-console-medium",
      "add:tahoma-normal",
      "add:lucida-console-medium",
    ])
    expect(result).toEqual({
      kind: "supported",
      targetIdentity: target.identity,
      platform: "windows",
      faces: target.faces,
    })
  })

  test("rolls back already-published faces when publication fails", async () => {
    const events: string[] = []
    const result = await admitLocalPlatformFonts(target, adapter({
      addLoadedFace(face) {
        const identity = (face as { identity: string }).identity
        events.push(`add:${identity}`)
        if (identity === "lucida-console-medium") throw new Error("publication failed")
      },
      removeLoadedFace(face) { events.push(`remove:${(face as { identity: string }).identity}`) },
    }))
    expect(result).toEqual({
      kind: "unsupported",
      targetIdentity: target.identity,
      platform: "windows",
      reason: "font-set-publication-failed",
      unadmittedFaceIdentities: ["tahoma-normal", "lucida-console-medium"],
    })
    expect(events).toEqual(["add:tahoma-normal", "add:lucida-console-medium", "remove:tahoma-normal"])
  })

  test("rejects overlapping ranges within one browser family selection", async () => {
    const loaded: string[] = []
    const invalid = Object.freeze({
      ...target,
      faces: Object.freeze([
        target.faces[0],
        Object.freeze({
          ...target.faces[0],
          identity: "overlap",
          localName: "Lucida Console",
          unicodeRange: Object.freeze([0x0080, 0x01ff] as const),
        }),
      ]),
    })
    const result = await admitLocalPlatformFonts(invalid, adapter({
      async loadLocalFace(request) {
        loaded.push(request.identity)
        return request
      },
    }))
    expect(result).toEqual({
      kind: "unsupported",
      targetIdentity: target.identity,
      platform: "windows",
      reason: "invalid-target",
      unadmittedFaceIdentities: [],
    })
    expect(loaded).toEqual([])
  })
})
