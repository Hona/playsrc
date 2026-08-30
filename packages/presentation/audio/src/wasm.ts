import { AudioError } from "./error"

// (module (func (drop (v128.const i32x4 0 0 0 0))))
// Check standard SIMD without validating the complete audio module twice.
const SIMD128 = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,23,1,21,0,253,12,...Array(16).fill(0),26,11])

/** Reject an unsupported target before compiling or instantiating audio. */
export function compileAudioModule(bytes: ArrayBuffer): Promise<WebAssembly.Module> {
  if (bytes.byteLength > 8 * 1024 * 1024) throw new AudioError("Capacity", "Audio module exceeds its bound")
  if (typeof WebAssembly === "undefined" || !WebAssembly.validate(SIMD128)) throw new AudioError("BrowserFailure", "This browser lacks required standard WebAssembly SIMD128 support")
  return WebAssembly.compile(bytes)
}
