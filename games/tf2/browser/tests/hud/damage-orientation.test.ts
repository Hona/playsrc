import { describe, expect, test } from "bun:test"
import { Tf2HudDamagePresentation } from "../../src/hud-integration/damage"
import type { Tf2HudBinding } from "../../src/hud"

// A DOM sink for the fast geometry/lifecycle loop, not visual acceptance. The
// headed damage-indicator profile checks the configured texture in gameplay.
function fixture() {
  const children: any[] = []
  const document = { createElement: (tag: string) => tag === "canvas"
    ? { getContext: () => ({ putImageData() {} }), toDataURL: () => "data:image/png;base64," }
    : { dataset: {}, style: {}, remove() { children.splice(children.indexOf(this), 1) } } }
  const root = { ownerDocument: document, append: (element: any) => children.push(element) }
  let yaw = 0
  const eye = [100, -200, 75] as const
  const previous = globalThis.ImageData
  globalThis.ImageData = class {} as typeof ImageData
  let presentation: Tf2HudDamagePresentation
  try {
    presentation = new Tf2HudDamagePresentation(root as unknown as HTMLElement, {
      material: "materials/vgui/damageindicator.vmt", texture: { width: 128, height: 64, rgba: new Uint8Array(128 * 64 * 4) },
      eyePosition: () => eye, yawDegrees: () => yaw, random: () => .5,
    })
  } finally { globalThis.ImageData = previous }
  return { children, presentation, eye, yaw: (value: number) => { yaw = value },
    hit(delta: readonly number[], now = 0, scale = 50) {
      presentation.publish({ commands: [{ kind: "damage-indicator", scale, lifetimeSeconds: 1 + scale / 100,
        direction: delta.map((v, axis) => v + eye[axis]!), tick: 1n, ordinal: 0 }] } as unknown as Tf2HudBinding, now)
    } }
}

describe("TF2 damage textured quad orientation", () => {
  for (const yaw of [0, 90, 227]) for (const bearing of [0, 45, 90, 135, 180, 225, 270, 315]) {
    test(`world bearing ${bearing}, camera yaw ${yaw}: SDK UV corners and screen Y`, () => {
      const f = fixture()
      const angle = bearing * Math.PI / 180
      f.hit([300 * Math.cos(angle), 300 * Math.sin(angle), 800])
      // Changing the camera after publication must reorient the retained hit.
      f.yaw(yaw)
      for (const viewport of [{ width: 1280, height: 720 }, { width: 997, height: 613 }]) {
        f.presentation.frame(.1, viewport)
        const s = f.children[0].style
        const width = parseFloat(s.width), height = parseFloat(s.height)
        const cx = parseFloat(s.left) + width / 2, cy = parseFloat(s.top) + height / 2
        const css = Number(s.transform.match(/rotate\((.*)rad\)/)[1])
        const relative = (bearing - yaw) * Math.PI / 180
        const radius = 120 * viewport.height / 480
        expect(cx).toBeCloseTo(viewport.width / 2 - radius * Math.sin(relative), 8)
        expect(cy).toBeCloseTo(viewport.height / 2 - radius * Math.cos(relative), 8)
        // SDK DrawDamageIndicator: axis[0]=(cos(-rotation),sin(-rotation)),
        // axis[1]=(-axis[0].y,axis[0].x); UVs (0,0),(1,0),(1,1),(0,1).
        for (const [u, v] of [[0, 0], [1, 0], [1, 1], [0, 1]]) {
          const x = (u! - .5) * width, y = (v! - .5) * height
          expect(cx + x * Math.cos(css) - y * Math.sin(css)).toBeCloseTo(cx + x * Math.cos(relative) + y * Math.sin(relative), 8)
          expect(cy + x * Math.sin(css) + y * Math.cos(css)).toBeCloseTo(cy - x * Math.sin(relative) + y * Math.cos(relative), 8)
        }
      }
    })
  }

  test("keeps damage scale, travel, fade, overlapping lifetimes and cleanup", () => {
    const f = fixture()
    f.hit([100, 0, -500], 1, 50)
    f.hit([0, -100, 500], 1.1, 100)
    f.presentation.frame(1.1, { width: 1280, height: 720 })
    expect(f.children).toHaveLength(2)
    expect(parseFloat(f.children[0].style.width)).toBe(82.5)
    expect(parseFloat(f.children[0].style.height)).toBe(90)
    expect(parseFloat(f.children[0].style.top) + 45).toBeCloseTo(180)
    expect(parseFloat(f.children[1].style.left) + 75).toBeCloseTo(850)
    f.presentation.frame(2.275, { width: 1280, height: 720 })
    expect(Number(f.children[0].style.opacity)).toBeCloseTo(.5)
    f.presentation.frame(2.5, { width: 1280, height: 720 })
    expect(f.children).toHaveLength(1)
    f.presentation.reset()
    expect(f.children).toHaveLength(0)
  })

  test("a vertical source keeps the SDK's horizontal projection rather than tilting the quad", () => {
    for (const z of [-1000, 1000]) {
      const f = fixture()
      f.hit([0, 0, z])
      f.presentation.frame(.1, { width: 1280, height: 720 })
      expect(parseFloat(f.children[0].style.left) + 82.5 / 2).toBe(640)
      expect(parseFloat(f.children[0].style.top) + 90 / 2).toBe(180)
      expect(f.children[0].style.transform).toBe("rotate(0rad)")
    }
  })
})
