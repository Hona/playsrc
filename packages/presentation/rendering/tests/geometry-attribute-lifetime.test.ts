import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import Geometries from "three/src/renderers/common/Geometries.js"
import { installGeometryAttributeLifetime } from "../src/geometry-attribute-lifetime"

test("geometry retirement retains uploaded attributes without consulting the retired first draw", () => {
  const deleted: object[] = [], info = { memory: { geometries: 0 } }
  const manager = new Geometries({ delete(attribute: object) { deleted.push(attribute) } }, info)
  const original = manager.initGeometry, restore = installGeometryAttributeLifetime(manager)
  const geometry = new THREE.BoxGeometry(), attributes = [geometry.getAttribute("position"), geometry.getAttribute("uv")]
  const index = geometry.index!
  const draw = { geometry, getAttributes() { return attributes } }
  manager.initGeometry(draw)
  expect(info.memory.geometries).toBe(1)
  // The render object and all its node/scene references may now be retired.
  draw.getAttributes = () => { throw new Error("retired draw retained by geometry") }
  geometry.dispose()
  expect(deleted).toEqual([index, ...attributes])
  expect(info.memory.geometries).toBe(0)
  expect(manager._geometryDisposeListeners.size).toBe(0)
  restore()
  expect(manager.initGeometry).toBe(original)
})
