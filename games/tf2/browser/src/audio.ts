/** The application bundle owns this generated, main-thread audio module. */
export function tf2AudioModuleUrl(): URL {
  return new URL("./wasm-generated/audio_wasm.wasm", import.meta.url)
}
