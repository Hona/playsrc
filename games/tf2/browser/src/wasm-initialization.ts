export type WasmInitializationStage = "integrity" | "shared-memory" | "instantiate" | "exports" | "thread-pool" | "reply-memory"

const ERROR_NAMES = new Set(["Error", "TypeError", "RangeError", "CompileError", "LinkError", "RuntimeError", "SecurityError", "NotSupportedError", "DataCloneError", "AbortError"])

export class WasmInitializationError extends Error {
  constructor(readonly stage: WasmInitializationStage, detail: string) {
    super(`initialize/${stage}: ${detail}`)
    this.name = "WasmInitializationError"
  }
}

/** Preserve the authenticated threaded runtime; report which operation failed
 * without exporting exception text, stack traces, URLs or filesystem paths. */
export async function initializeAuthenticatedWasm<T extends { memory: WebAssembly.Memory }>(request: Readonly<{
  bytes: ArrayBuffer
  expectedSha256: string
  threads: number
  isolated: boolean
  sharedArrayBuffer: typeof SharedArrayBuffer | undefined
  replyBytes: number
  instantiate(bytes: ArrayBuffer): Promise<T>
  validateExports(candidate: T): boolean
  startThreadPool(threads: number): Promise<unknown>
}>): Promise<Readonly<{ candidate: T; actual: string; mailbox: SharedArrayBuffer; modelOwnership: Int32Array }>> {
  let stage: WasmInitializationStage = "integrity"
  const facts = `bytes=${request.bytes.byteLength},threads=${request.threads},isolated=${Number(request.isolated)},SAB=${Number(!!request.sharedArrayBuffer)}`
  try {
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", request.bytes))
    const actual = Array.from(digest, value => value.toString(16).padStart(2, "0")).join("")
    if (actual !== request.expectedSha256) throw new WasmInitializationError(stage, "SHA256 mismatch")
    stage = "shared-memory"
    const SharedMemory = request.sharedArrayBuffer
    if (!request.isolated || !SharedMemory) throw new WasmInitializationError(stage, `Cross-origin isolated shared memory unavailable (${facts})`)
    stage = "instantiate"
    const candidate = await request.instantiate(request.bytes)
    stage = "exports"
    if (!(candidate.memory instanceof WebAssembly.Memory)) throw new WasmInitializationError(stage, "WebAssembly.Memory export missing")
    if (!request.validateExports(candidate)) throw new WasmInitializationError(stage, "Required runtime function export missing")
    stage = "shared-memory"
    if (!(candidate.memory.buffer instanceof SharedMemory)) throw new WasmInitializationError(stage, "Runtime memory is not shared")
    stage = "thread-pool"
    await request.startThreadPool(request.threads)
    stage = "reply-memory"
    const mailbox = new SharedMemory(request.replyBytes)
    const modelOwnership = new Int32Array(new SharedMemory(64 * Int32Array.BYTES_PER_ELEMENT))
    return Object.freeze({ candidate, actual, mailbox, modelOwnership })
  } catch (error) {
    if (error instanceof WasmInitializationError) throw error
    const name = error !== null && typeof error === "object" && "name" in error && typeof error.name === "string" && ERROR_NAMES.has(error.name) ? error.name : "Exception"
    throw new WasmInitializationError(stage, `${name} (${facts})`)
  }
}
