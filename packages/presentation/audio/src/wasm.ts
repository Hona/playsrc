import { AudioError } from "./error"

/** Validate the actual module, not a browser name or its WebGPU capability. */
export function compileAudioModule(bytes: ArrayBuffer): Promise<WebAssembly.Module> {
  if (bytes.byteLength > 8 * 1024 * 1024) throw new AudioError("Capacity", "Audio module exceeds its bound")
  if (!WebAssembly.validate(bytes)) throw new AudioError("BrowserFailure", "Audio module is invalid or this browser lacks required standard WebAssembly SIMD128 support")
  return WebAssembly.compile(bytes)
}
