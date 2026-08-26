import { describe, expect, test } from "bun:test"
import { WebGpuSubmissionBatch, withImmediateGpuSubmissions, type BatchedGpuQueue } from "../src/webgpu-submission-batch"

describe("ordered WebGPU queue submission batching", () => {
  test("submits readback copies before mapAsync and restores the outer batching scope on failure", () => {
    const operations: string[] = []
    const queue: BatchedGpuQueue = { submit: buffers => { operations.push(`submit:${buffers.join(",")}`) }, writeBuffer() {} }
    const batch = new WebGpuSubmissionBatch(queue)
    batch.begin()
    queue.submit(["world"])
    expect(() => withImmediateGpuSubmissions(queue, () => {
      queue.submit(["readback-copy"])
      operations.push("mapAsync")
      throw new Error("readback failed")
    })).toThrow("readback failed")
    queue.submit(["hud"])
    expect(operations).toEqual(["submit:world", "submit:readback-copy", "mapAsync"])
    batch.finish()
    expect(operations.at(-1)).toBe("submit:hud")
    batch.dispose()
    expect(() => withImmediateGpuSubmissions(queue, () => {})).toThrow("owner is unavailable")
  })
  test("combines adjacent ordered passes and snapshots reused submission arrays", () => {
    const operations: unknown[] = []
    const queue: BatchedGpuQueue = {
      submit: buffers => { operations.push(["submit", ...buffers]) },
      writeBuffer: (...values) => { operations.push(["write", ...values]) },
    }
    const original = queue.submit
    const batch = new WebGpuSubmissionBatch(queue)
    const reused: unknown[] = ["sky"]
    batch.begin()
    queue.submit(reused)
    reused[0] = "world"
    queue.submit(reused)
    reused[0] = null
    batch.finish()
    expect(operations).toEqual([["submit", "sky", "world"]])
    batch.dispose()
    expect(queue.submit).toBe(original)
  })

  test("flushes before every queue-timeline write and completion fence", async () => {
    const operations: string[] = []
    const queue: BatchedGpuQueue = {
      submit: buffers => { operations.push(`submit:${buffers.join(",")}`) },
      writeBuffer: () => { operations.push("write-buffer") },
      writeTexture: () => { operations.push("write-texture") },
      copyExternalImageToTexture: () => { operations.push("copy-external") },
      onSubmittedWorkDone: async () => { operations.push("fence") },
    }
    const batch = new WebGpuSubmissionBatch(queue)
    batch.begin()
    queue.submit(["world"]); queue.writeBuffer({})
    queue.submit(["viewmodel"]); queue.writeTexture!({})
    queue.submit(["hud"]); queue.copyExternalImageToTexture!({})
    queue.submit(["output"]); await queue.onSubmittedWorkDone!()
    batch.finish()
    expect(operations).toEqual([
      "submit:world", "write-buffer", "submit:viewmodel", "write-texture",
      "submit:hud", "copy-external", "submit:output", "fence",
    ])
  })
})
