import { expect, test } from "bun:test"
import { writeParticleCenters } from "../src/particle-attributes"

test("scalar particle centers preserve exact packed float32 values and neighboring quads", () => {
  for (const position of [[0, -0, 1], [Math.PI, 1e-40, -3.4028234663852886e38]] as const) {
    for (const orientation of [0, 1, 2]) {
      const expected = new Float32Array(48).fill(7), actual = expected.slice()
      for (let vertex = 0; vertex < 4; vertex++) { expected.set(position, 16 + vertex * 4); expected[16 + vertex * 4 + 3] = orientation }
      writeParticleCenters(actual, 16, position, orientation)
      expect(new Uint32Array(actual.buffer)).toEqual(new Uint32Array(expected.buffer))
    }
  }
})
