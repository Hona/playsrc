import { AudioError } from "./error"
import { supportsSimd128 } from "@playsrc/wasm"

/** Reject an unsupported target before compiling or instantiating audio. */
export function compileAudioModule(bytes: ArrayBuffer): Promise<WebAssembly.Module> {
  if (bytes.byteLength > 8 * 1024 * 1024) throw new AudioError("Capacity", "Audio module exceeds its bound")
  if (!supportsSimd128()) throw new AudioError("BrowserFailure", "This browser lacks required standard WebAssembly SIMD128 support")
  return WebAssembly.compile(bytes)
}
