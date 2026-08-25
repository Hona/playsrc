import { describe, expect, test } from "bun:test"
import { FakeDocument, createRoot, descendants } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { Tf2HudScopePresentation, tf2ScopeGeometry } from "../../src/hud-integration/scope"
import { createTf2AuthoredScope, tf2AuthoredScope } from "../../src/ui-resources/scope"
import { configuredTf2AuthoredScopeInput } from "../../src/ui-resources/scope.generated"

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
    scope.publish(true, 150, { width: 1280, height: 720 })
    expect(charge.dataset.charge).toBe("1")
    scope.hide()
    expect(nodes.find(value => value.dataset.tf2Scope === "authored")!.style.display).toBe("none")
    scope.destroy()
    expect(root.children.length).toBe(0)
  })
})
