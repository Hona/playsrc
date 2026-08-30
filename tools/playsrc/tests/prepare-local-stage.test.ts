import { expect, test } from "bun:test"
import { prepareLocalStage } from "../src/prepare-local-stage"
import { localJobCommand, parseLocalPreparationStage } from "../src/local-job-command"

test("bounded local jobs select only one declared preparation stage, never arbitrary compiler arguments", () => {
  for (const args of [["wasm"], ["producer"], ["resources", "pl_upward"]]) {
    expect(localJobCommand(["build-stage", ...args])).toEqual({ command: ["tools/playsrc/src/prepare-local-stage.ts", ...args], interactive: false })
  }
  for (const args of [[], ["wasm", "pl_upward"], ["resources"], ["resources", "../bad"], ["resources", "pl_upward", "--release"], ["cargo", "--offline"], ["producer;bad"]]) {
    expect(() => parseLocalPreparationStage(args)).toThrow()
    expect(() => localJobCommand(["build-stage", ...args])).toThrow()
  }
})

test("stage orchestration calls existing verified build owners serially and rejects failure or identity drift", async () => {
  const calls: string[] = [], config = { tf2Dir: "tf", sourceCacheDir: "cache", assetDir: "assets" }
  let identity = "a".repeat(64), fail = false
  const deps = {
    identity: async () => identity,
    wasm: async (input: typeof config) => { expect(input).toBe(config); calls.push("wasm"); if (fail) throw new Error("compiler lock timeout"); return "wasm" },
    producer: async () => { calls.push("producer"); return { generatorSha256: "b".repeat(64) } as any },
    resources: async (_input: typeof config, target: string) => { calls.push(target); return { report: { graphDescriptor: { sha256: "c".repeat(64) } } } as any },
  }
  for (const args of [["wasm"], ["producer"], ["resources", "pl_upward"]]) {
    const report = await prepareLocalStage(config, parseLocalPreparationStage(args), deps)
    expect(report.identity).toBe(identity); expect(report.finishedAt).toBeGreaterThanOrEqual(report.startedAt)
  }
  expect(calls).toEqual(["wasm", "producer", "pl_upward"])
  fail = true
  await expect(prepareLocalStage(config, { kind: "wasm" }, deps)).rejects.toThrow("compiler lock timeout")
  await expect(prepareLocalStage(config, { kind: "wasm" }, { ...deps, wasm: async () => { identity = "d".repeat(64); return "changed" } })).rejects.toThrow("Build inputs changed")
})
