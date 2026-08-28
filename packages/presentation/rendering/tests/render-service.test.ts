import { expect, test } from "bun:test"
import { installRenderService } from "../src/render-service"

test("render service preserves receiver, every argument and exact draw order", () => {
  const calls: unknown[] = []
  let installed: Function | null = null
  const backend = {
    getRenderObjectFunction: () => installed,
    setRenderObjectFunction: (value: Function | null) => { installed = value },
    renderObject(...args: unknown[]) { expect(this).toBe(backend); calls.push(args) },
  }
  installRenderService(backend as any, () => calls.push("service"))
  const args = Array.from({ length: 8 }, () => ({}))
  installed!(...args, "reflection")
  installed!(...args, "refraction")
  expect(calls).toEqual(["service", [...args, "reflection"], "service", [...args, "refraction"]])
  expect((calls[1] as unknown[])[0]).toBe(args[0])
})

test("an already installed draw owner remains the draw authority", () => {
  let draws = 0, serviced = 0
  let draw: Function | null = () => { draws++ }
  const backend = { getRenderObjectFunction: () => draw, setRenderObjectFunction: (value: Function | null) => { draw = value },
    renderObject() { throw new Error("must not bypass the installed owner") } }
  installRenderService(backend as any, () => { serviced++ })
  draw!()
  expect([draws, serviced]).toEqual([1, 1])
})

test("pipeline compilation uses the same service without double servicing a delegated draw", () => {
  let draws = 0, serviced = 0
  let installed: Function | null = (...args: unknown[]) => backend.renderObject(...args)
  const backend = { getRenderObjectFunction: () => installed, setRenderObjectFunction: (value: Function | null) => { installed = value },
    renderObject(..._args: unknown[]) { draws++ } }
  installRenderService(backend as any, () => { serviced++ })
  // Three compileAsync selects this entry directly.
  backend.renderObject()
  installed!()
  expect([draws, serviced]).toEqual([2, 2])
})
