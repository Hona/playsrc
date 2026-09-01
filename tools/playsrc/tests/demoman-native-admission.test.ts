import { expect, test } from "bun:test"
import { macWindowClick } from "../profile/macos-window-click"

test("native click rejects incomplete targets before compiling or sending input", async () => {
  await expect(macWindowClick("unused", 0, 1, { X: 0, Y: 0, Width: 1280, Height: 720 })).rejects.toThrow("exact process")
  await expect(macWindowClick("unused", 1, 0, { X: 0, Y: 0, Width: 1280, Height: 720 })).rejects.toThrow("exact process")
  await expect(macWindowClick("unused", 1, 1, { X: NaN, Y: 0, Width: 1280, Height: 720 })).rejects.toThrow("finite bounds")
})

test("Demoman evidence uses read-only native admission instead of forcing focus", async () => {
  const fixture = await Bun.file(new URL("../profile/demoman-test.ts", import.meta.url)).text()
  expect(fixture).toContain("startupConsoleIdle(local.sourceCacheDir) < 2_000")
  expect(fixture).toContain("requireStartupNative")
  expect(fixture).toContain("guardStartupInput")
  expect(fixture).toContain('PLAYSRC_PROFILE_MANAGED !== "1"')
  expect(fixture).not.toMatch(/bringToFront|bring_to_front|SetForegroundWindow|requestPointerLock\s*=/)
  expect(fixture).toContain("macWindowClick")
  expect(fixture).toContain("point.currentDocument")
  const nativeClick = await Bun.file(new URL("../profile/macos-window-click.m", import.meta.url)).text()
  expect(nativeClick).toContain("CGPreflightPostEventAccess")
  expect(nativeClick).toContain("frontmostApplication.processIdentifier != pid")
  expect(nativeClick).toContain("CGRectEqualToRect(bounds, expected)")
  expect(nativeClick).toContain("CGRectIntersectsRect(bounds, expected)")
  expect(nativeClick).toContain("CGEventPost(kCGHIDEventTap, event)")
  expect(nativeClick).not.toMatch(/activateWithOptions|orderFront|AXUIElementSetAttributeValue|CGRequestPostEventAccess/)
  for (const name of ["demoman-bottle", "demoman-grenade", "demoman-physics"]) {
    const source = await Bun.file(new URL(`../profile/${name}.profile.ts`, import.meta.url)).text()
    expect(source).toContain('from "./demoman-test"')
    expect(source).toContain("nativeGameplay.lockPointer()")
    expect(source).not.toMatch(/bringToFront|bring_to_front|dispatchEvent\(new (MouseEvent|KeyboardEvent)|Object\.defineProperty/)
  }
})
