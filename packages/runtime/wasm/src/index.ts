// (module (func (drop (v128.const i32x4 0 0 0 0))))
// Probe standard SIMD before instantiation without validating the entire large
// gameplay module twice. WebAssembly compilation still validates its full body.
const SIMD128 = new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,23,1,21,0,253,12,...Array(16).fill(0),26,11])

export function supportsSimd128(): boolean {
  return typeof WebAssembly !== "undefined" && WebAssembly.validate(SIMD128)
}
