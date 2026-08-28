import { expect, test } from "bun:test"
import { writeProfileLease } from "../src/profile-lease"

function fixture(failures: number, current: string | null = "owner", code = "EPERM") {
  const calls: string[] = [], sources: string[] = [], writes: string[] = []
  const error = Object.assign(new Error("sharing violation"), { code })
  const fs = {
    async writeFile(_path: string, data: string, options: any) { expect(options.flag).toBe("wx"); calls.push("write"); writes.push(data) },
    async rename(source: string, destination: string) { calls.push("rename"); sources.push(source); expect(destination).toBe("record.lease"); if (failures-- > 0) throw error },
    async readFile() { calls.push("read"); if (current === null) throw Object.assign(new Error("absent"), { code: "ENOENT" }); return JSON.stringify({ token: current }) },
    async pause() { calls.push("pause") },
    async rm(source: string) { expect(source).toBe(sources[0]!); calls.push("cleanup") },
  }
  return { fs: fs as any, calls, writes, sources, error }
}

test("transient Windows lease sharing retries the same atomic publication and unchanged expiry", async () => {
  const state = fixture(2)
  await writeProfileLease("record", "owner", 500, state.fs)
  expect(state.calls).toEqual(["write", "rename", "read", "pause", "rename", "read", "pause", "rename", "cleanup"])
  expect(state.writes).toHaveLength(1); expect(new Set(state.sources).size).toBe(1)
  expect(JSON.parse(state.writes[0]!).token).toBe("owner")
})

test("persistent failures, foreign ownership and unrelated errors remain failures with owned cleanup", async () => {
  for (const [failures, current, code, renames] of [[3, "owner", "EPERM", 3], [1, "foreign", "EPERM", 1], [1, "owner", "EACCES", 1]] as const) {
    const state = fixture(failures, current, code)
    await expect(writeProfileLease("record", "owner", 0, state.fs)).rejects.toThrow(current === "foreign" ? "lease changed" : "sharing violation")
    expect(state.sources).toHaveLength(renames); expect(state.calls.at(-1)).toBe("cleanup")
    expect(state.writes).toHaveLength(1)
  }
})

test("initial creation can retry an absent destination without changing lease content", async () => {
  const state = fixture(1, null)
  await writeProfileLease("record", "owner", 0, state.fs)
  expect(state.sources).toHaveLength(2); expect(state.writes).toHaveLength(1)
})
