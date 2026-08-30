import { expect, test } from "bun:test"
import { initializeAuthenticatedWasm } from "../src/wasm-initialization"

async function fixture() {
  const bytes = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]).buffer
  const expectedSha256 = Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)), value => value.toString(16).padStart(2, "0")).join("")
  const calls: string[] = []
  const candidate = { memory: new WebAssembly.Memory({ initial: 1, maximum: 2, shared: true }) }
  return { calls, request: { bytes, expectedSha256, threads: 2, isolated: true, sharedArrayBuffer: SharedArrayBuffer, replyBytes: 32,
    instantiate: async () => { calls.push("instantiate"); return candidate },
    validateExports: () => { calls.push("exports"); return true },
    startThreadPool: async () => { calls.push("threads") },
  } }
}

test("authenticates before instantiating and retains threaded shared reply storage", async () => {
  const { request, calls } = await fixture()
  const result = await initializeAuthenticatedWasm(request)
  expect(calls).toEqual(["instantiate", "exports", "threads"])
  expect(result.actual).toBe(request.expectedSha256)
  expect(result.mailbox.byteLength).toBe(32)
  expect(result.modelOwnership.buffer instanceof SharedArrayBuffer).toBe(true)
  expect(result.modelOwnership.length).toBe(64)
})

test("a hash mismatch is not relabelled as missing WebAssembly or a thread failure", async () => {
  const { request, calls } = await fixture()
  await expect(initializeAuthenticatedWasm({ ...request, expectedSha256: "0".repeat(64) })).rejects.toThrow("initialize/integrity: SHA256 mismatch")
  expect(calls).toEqual([])
})

test("distinguishes isolation, shared-memory, exports, instantiation and thread-pool failures", async () => {
  const { request } = await fixture()
  for (const changed of [{ isolated: false }, { sharedArrayBuffer: undefined }]) {
    await expect(initializeAuthenticatedWasm({ ...request, ...changed })).rejects.toThrow("initialize/shared-memory: Cross-origin isolated shared memory unavailable")
  }
  await expect(initializeAuthenticatedWasm({ ...request, validateExports: () => false })).rejects.toThrow("initialize/exports: Required runtime function export missing")
  await expect(initializeAuthenticatedWasm({ ...request, instantiate: async () => ({ memory: null! }) })).rejects.toThrow("initialize/exports: WebAssembly.Memory export missing")
  await expect(initializeAuthenticatedWasm({ ...request, instantiate: async () => ({ memory: new WebAssembly.Memory({ initial: 1 }) }) })).rejects.toThrow("initialize/shared-memory: Runtime memory is not shared")
  for (const name of ["CompileError", "LinkError", "RangeError", "RuntimeError"]) {
    await expect(initializeAuthenticatedWasm({ ...request, instantiate: async () => { throw { name, message: "private URL and path" } } })).rejects.toThrow(`initialize/instantiate: ${name}`)
  }
  await expect(initializeAuthenticatedWasm({ ...request, startThreadPool: async () => { throw new TypeError("private worker URL") } })).rejects.toThrow("initialize/thread-pool: TypeError")
  try { await initializeAuthenticatedWasm({ ...request, startThreadPool: async () => { throw { name: "private", message: "secret" } } }) }
  catch (error) { expect(String(error)).not.toMatch(/private|secret/); expect(String(error)).toContain("initialize/thread-pool: Exception") }
})
