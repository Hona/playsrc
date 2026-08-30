import { expect, spyOn, test } from "bun:test"
import { compileAudioModule } from "../src/wasm"

// () -> (), v128.const zero; drop. Standard SIMD, no relaxed instructions.
const simd = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,23,1,21,0,253,12,...Array(16).fill(0),26,11]).buffer

test("audio validates actual SIMD bytecode before compiling, independently of GPU or browser identity", async () => {
  expect(WebAssembly.validate(simd)).toBe(true)
  expect(await compileAudioModule(simd)).toBeInstanceOf(WebAssembly.Module)
  expect(compileAudioModule(new ArrayBuffer(0))).rejects.toBeInstanceOf(WebAssembly.CompileError)
  expect(() => compileAudioModule(new ArrayBuffer(8 * 1024 * 1024 + 1))).toThrow("exceeds its bound")
})

test("an unsupported SIMD target is rejected without compiling the audio module", () => {
  const validate = spyOn(WebAssembly, "validate").mockReturnValue(false)
  const compile = spyOn(WebAssembly, "compile")
  try {
    expect(() => compileAudioModule(simd)).toThrow("lacks required standard WebAssembly SIMD128")
    expect(compile).not.toHaveBeenCalled()
  } finally { validate.mockRestore(); compile.mockRestore() }
})
