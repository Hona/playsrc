import { describe, expect, test } from "bun:test"
import { FakeDocument, createRoot, descendants } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { Tf2HudScopePresentation, tf2ScopeGeometry } from "../../src/hud-integration/scope"
import { createTf2AuthoredScope, tf2AuthoredScope } from "../../src/ui-resources/scope"
import { configuredTf2AuthoredScopeInput } from "../../src/ui-resources/scope.generated"
import { decodeScreenshot } from "../../../../../tools/playsrc/profile/screenshot-pixels"
import { evaluateSourceRefractPixel } from "../../../../../packages/presentation/rendering/src/source-refract"

describe("authored Source Sniper scope and charge HUD", () => {
  test("retains exact configured refraction quadrants, normal, tint, and two charge textures", () => {
    expect(tf2AuthoredScope.quadrants.map(value => value.logicalPath)).toEqual([
      "materials/hud/scope_sniper_ul.vmt", "materials/hud/scope_sniper_ur.vmt",
      "materials/hud/scope_sniper_lr.vmt", "materials/hud/scope_sniper_ll.vmt",
    ])
    expect(tf2AuthoredScope.chargeMaterial.logicalPath).toBe("materials/hud/sniperscope_numbers.vmt")
    expect(tf2AuthoredScope.chargeMask.source.logicalPath).toBe("materials/hud/sniperscope_numbers2.vtf")
    expect(() => createTf2AuthoredScope({ ...(configuredTf2AuthoredScopeInput as object), contentBuild: "0" })).toThrow()
  })

  test("uses integer Source 4:3 scope extents and retained widescreen/tall black blocks", () => {
    expect(tf2ScopeGeometry(1280, 720)).toEqual({ left: 160, top: 0, right: 1120, bottom: 720, middleX: 640, middleY: 360 })
    expect(tf2ScopeGeometry(390, 844)).toEqual({ left: 0, top: 276, right: 390, bottom: 568, middleX: 195, middleY: 422 })
  })

  test("tint RGB is not an opaque decal; normal alpha owns refraction and blending", () => {
    const rgba = (name: "normal" | "tint", u: number, v: number) => {
      const mip = tf2AuthoredScope[name].mips[0]!
      const image = decodeScreenshot(Buffer.from(mip.pngDataUrl.split(",")[1]!, "base64"))
      const offset = (Math.floor(v * image.height) * image.width + Math.floor(u * image.width)) * 4
      return Array.from(image.pixels.slice(offset, offset + 4))
    }
    const centerNormal = rgba("normal", .9, .9)
    const centerTint = rgba("tint", .9, .9)
    expect(centerNormal).toEqual([128, 128, 254, 128])
    expect(centerTint).toEqual([134, 130, 120, 255])
    const linear = (byte: number) => { const c = byte / 255; return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4 }
    const pixel = evaluateSourceRefractPixel({
      state: { refractAmount: .1, refractTint: [1, 1, 1], blurAmount: 1, ignoreDepth: true },
      coordinate: [.45, .45], normal: centerNormal.map(v => v / 255) as [number, number, number, number],
      tintTexture: centerTint.slice(0, 3).map(linear) as [number, number, number], sample: () => [.8, .4, .2, 1],
    })
    expect(pixel.rgba[3]).toBe(128 / 255)
    expect(pixel.rgba[0]).toBeCloseTo(.8 * .9999999 * 2 * linear(134), 8)
    expect(pixel.warpedCoordinate[0]).toBeCloseTo(.45 + (128 / 255 * 2 - 1) * (128 / 255) * .1, 10)
    expect(rgba("normal", 0, 0)[3]).toBe(255)
    expect(rgba("tint", 0, 0).slice(0, 3)).toEqual([0, 0, 0])
    expect(tf2AuthoredScope.normal.mips).toHaveLength(9)
    expect(tf2AuthoredScope.tint.mips).toHaveLength(10)
    expect(tf2AuthoredScope.normal.clampS && tf2AuthoredScope.normal.clampT).toBe(true)
    expect(tf2AuthoredScope.chargeBase.clampT).toBe(false)
    expect(tf2AuthoredScope.chargeBase.mips).toHaveLength(1)
    expect(tf2AuthoredScope.chargeMask.mips).toHaveLength(1)
  })

  test("publishes exact one-pixel upper-right overlap and SniperRifleCharge material transform", () => {
    const document = new FakeDocument()
    const root = createRoot(document)
    const scope = new Tf2HudScopePresentation(root as unknown as HTMLElement)
    scope.publish(true, 0, { width: 1280, height: 720 })
    const nodes = descendants(root)
    const topRight = nodes.find(value => value.dataset.scopeQuadrant === "ur")!
    expect(topRight.style.left).toBe("639px")
    expect(topRight.style.top).toBe("0px")
    expect(topRight.style.width).toBe("481px")
    expect(topRight.style.height).toBe("361px")
    const charge = nodes.find(value => value.dataset.tf2ScopeCharge === "authored")!
    expect(charge.style.left).toBe("736px")
    expect(charge.style.top).toBe("264px")
    expect(charge.style.width).toBe("96px")
    expect(charge.style.height).toBe("192px")
    expect(charge.dataset.charge).toBe("0")
    const initial = scope.materialFrame()!
    expect(initial.draws[1]!.bounds).toEqual([639, 0, 481, 361])
    expect(initial.draws[1]!.uv).toEqual([1 - .5 / 256, .5 / 256, .5 / 256, 1 - .5 / 256])
    expect(initial.draws[8]!.uv[1]).toBeCloseTo(1.775)
    expect(initial.draws[8]!.uv[3]).toBeCloseTo(2.025)
    scope.publish(true, 150, { width: 1280, height: 720 })
    expect(charge.dataset.charge).toBe("1")
    expect(scope.materialFrame()!.draws[8]!.uv).toEqual([0, .975, 1, 1.225])
    const full = scope.materialFrame()
    scope.publish(true, 150, { width: 1280, height: 720 })
    expect(scope.materialFrame()).toBe(full)
    scope.hide()
    expect(scope.materialFrame()).toBeUndefined()
    expect(nodes.find(value => value.dataset.tf2Scope === "authored")!.style.display).toBe("none")
    scope.destroy()
    expect(root.children.length).toBe(0)
  })

  test("resizing and reentry replace geometry without retaining old scope pixels", () => {
    const root = createRoot(new FakeDocument())
    const scope = new Tf2HudScopePresentation(root as unknown as HTMLElement)
    for (const [width, height] of [[1280, 720], [801, 721], [390, 844], [1024, 768]]) {
      scope.publish(true, 150, { width: width!, height: height! })
      const frame = scope.materialFrame()!
      expect(frame.draws).toHaveLength(9)
      expect(frame.draws.slice(4, 8).every(draw => draw.bounds[2] >= 0 && draw.bounds[3] >= 0)).toBe(true)
      scope.hide()
      scope.publish(true, 150, { width: width!, height: height! })
      expect(scope.materialFrame()!.draws).toEqual(frame.draws)
    }
    scope.setViewport({ width: 1280, height: 720 })
    expect(scope.materialFrame()!.draws[1]!.bounds).toEqual([639, 0, 481, 361])
    scope.destroy()
  })
})
