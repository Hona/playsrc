import { expect, test } from "bun:test"
import { weaponParticleColorRequests } from "../../../games/tf2/browser/src/presentation"

// Run the actual application effect mapper without constructing its DOM/UI.
// Only field access is supplied; no branch or effect expression is rewritten.
async function mapper() {
  const source = await Bun.file(new URL("../../../apps/web/tf2/src/runtime.ts", import.meta.url)).text()
  const begin = source.indexOf("  #pyroParticles(snapshot: Snapshot):"), end = source.indexOf("\n  #command():", begin)
  if (begin < 0 || end < begin) throw new Error("Application particle owner seam differs")
  const method = source.slice(begin, end)
  const code = new Bun.Transpiler({ loader: "ts", target: "bun" }).transformSync(`class OfflineMapper {
    #attachmentTransforms = new Map([15, 98].map(weapon => [weapon, new Map([["muzzle", { position: [1,2,3], orientation: [0,0,0,1] }]])]));
    #pyroFlameEffect; #pyroEffectSerial = 0; #manmelterChargeEffect;
    run(snapshot) { return this.#pyroParticles(snapshot); }
    ${method}
  }`)
  return new (new Function("weaponParticleColorRequests", `${code}; return OfflineMapper`)(weaponParticleColorRequests))()
}

const snapshot = (changes: object = {}) => ({ tick: 10n, class: 7, weapon: 98, team: 2, lifecycle: 1, flameFiring: false, flamePoints: [], activities: [], events: [], ...changes })

test("flamethrowerfire128 owner is reached by Manmelter absorption, not stock flamethrower or mere charge", async () => {
  const owner = await mapper()
  const started = (value: any[]) => value.filter(request => request.kind === "start").map(request => request.system)
  expect(started(owner.run(snapshot({ class: 3, weapon: 1, events: [{ kind: 24, subject: 3 }] })))).toEqual([])
  expect(started(owner.run(snapshot({ weapon: 15, flameFiring: true })))).toEqual(["new_flame"])
  const charging = owner.run(snapshot({ events: [{ kind: 24, subject: 1 }] }))
  expect(started(charging)).toEqual(["drg_manmelter_vacuum"])
  const absorbed = owner.run(snapshot({ events: [{ kind: 24, subject: 3 }] }))
  expect(started(absorbed)).toEqual(["drg_manmelter_vacuum_flames"])
  expect(absorbed.find((request: any) => request.kind === "start")).toMatchObject({ launcherIdentity: 98, ownerIdentity: 1, team: "red", attachment: { entityIdentity: 98, name: "muzzle" } })
  expect(absorbed.some((request: any) => request.kind === "set-control-point")).toBe(true)
})

test("holster, class change and death stop the sustained owner once without erasing the one-shot absorption effect", async () => {
  for (const changes of [{ weapon: 15 }, { class: 3, weapon: 1 }, { lifecycle: 0 }]) {
    const owner = await mapper()
    owner.run(snapshot({ events: [{ kind: 24, subject: 1 }] }))
    const absorbed = owner.run(snapshot({ events: [{ kind: 24, subject: 3 }] })).find((request: any) => request.kind === "start")
    const stopped = owner.run(snapshot(changes)).filter((request: any) => request.kind === "stop")
    expect(stopped).toHaveLength(1)
    expect(stopped[0]).toMatchObject({ projectileIdentity: 98, immediate: false })
    expect(stopped[0].effectIdentity).not.toBe(absorbed.effectIdentity)
    expect(owner.run(snapshot(changes)).filter((request: any) => request.kind === "stop")).toHaveLength(0)
  }
})
