import { expect, test } from "bun:test"
import { compileAudioModule } from "../src/wasm"

// () -> (), v128.const zero; drop. Standard SIMD, no relaxed instructions.
const simd = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,23,1,21,0,253,12,...Array(16).fill(0),26,11]).buffer

test("audio validates actual SIMD bytecode before compiling, independently of GPU or browser identity", async () => {
  expect(WebAssembly.validate(simd)).toBe(true)
  expect(await compileAudioModule(simd)).toBeInstanceOf(WebAssembly.Module)
  expect(() => compileAudioModule(new ArrayBuffer(0))).toThrow("invalid or this browser lacks")
  expect(() => compileAudioModule(new ArrayBuffer(8 * 1024 * 1024 + 1))).toThrow("exceeds its bound")
})
