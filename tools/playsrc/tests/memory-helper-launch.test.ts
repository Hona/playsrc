import { expect, test } from "bun:test"

type Launch = { executable: string; arguments: string[]; options: Record<string, unknown> }
const input = [{ id: 41, type: "browser" }, { id: 82, type: "GPU" }, { id: 41, type: "browser" },
  ...[0, -1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1, "123"].map(id => ({ id, type: "invalid" }))]
const windowsRows = [{ Id: 41, WorkingSet64: 1024, PrivateMemorySize64: 768 }, { Id: 82, WorkingSet64: 2048, PrivateMemorySize64: 1536 }]

// Exercise the actual local functions, not a second command builder. Importing
// their Playwright modules would register headed scenarios; extracting only
// these closed helpers lets launch-option tests run without any process/UI.
async function helper(file: string, name: string, dependency: string, execute: (...args: any[]) => any, platform: string) {
  const source = await Bun.file(new URL(`../profile/${file}`, import.meta.url)).text()
  const compiled = new Bun.Transpiler({ loader: "ts" }).transformSync(source)
  const start = compiled.indexOf(`function ${name}(`), end = compiled.indexOf("\n}", start)
  if (start < 0 || end < start) throw new Error("Memory helper seam changed")
  const declaration = compiled.slice(start, end + 2)
  return new Function(dependency, "process", `return (${name === "residentProcesses" ? "async " : ""}${declaration})`)(execute, { platform }) as (processes: unknown[]) => any
}

for (const scenario of [
  { file: "three-map-load.profile.ts", name: "residentProcesses", dependency: "executeFile", timeout: 2_000, asynchronous: true },
  { file: "ctf-2fort-performance.profile.ts", name: "processMemory", dependency: "spawnSync", timeout: 10_000, asynchronous: false },
]) {
  test(`${scenario.file} launches only bounded console-free numeric-PID Windows telemetry`, async () => {
    const launches: Launch[] = []
    const read = await helper(scenario.file, scenario.name, scenario.dependency, (executable, arguments_, options) => {
      launches.push({ executable, arguments: arguments_, options })
      return { status: 0, stdout: JSON.stringify(windowsRows), stderr: "" }
    }, "win32")
    const result = await read(input)
    expect(launches).toHaveLength(1)
    expect(launches[0]!.executable).toBe("powershell")
    expect(launches[0]!.arguments.slice(0, 2)).toEqual(["-NoProfile", "-Command"])
    expect(launches[0]!.arguments[2]).toContain("Get-Process -Id 41,82 -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64")
    expect(launches[0]!.arguments[2]).not.toMatch(/-Name|Infinity|NaN|invalid|123/)
    expect(launches[0]!.options).toEqual({ timeout: scenario.timeout, windowsHide: true, ...(scenario.asynchronous ? {} : { encoding: "utf8" }) })
    const rows = scenario.asynchronous ? result : result.processes
    expect(rows.map((row: any) => [row.id, row.residentBytes, row.privateBytes])).toEqual([[41, 1024, 768], [82, 2048, 1536]])
    expect(rows.map((row: any) => row.type ?? row.role)).toEqual(["browser", "GPU"])
    if (!scenario.asynchronous) expect(result.residentBytes).toBe(3072)
    launches.length = 0
    await read(input.filter(value => value.type === "invalid"))
    expect(launches).toHaveLength(0)
  })

  test(`${scenario.file} preserves ps RSS units, PID scope and bounded completion`, async () => {
    const launches: Launch[] = []
    const read = await helper(scenario.file, scenario.name, scenario.dependency, (executable, arguments_, options) => {
      launches.push({ executable, arguments: arguments_, options })
      return { status: 0, stdout: "41 10\n82 20\n", stderr: "" }
    }, "darwin")
    const result = await read(input)
    expect(launches).toEqual([{ executable: "ps", arguments: ["-o", "pid=,rss=", "-p", "41,82"], options: { timeout: scenario.timeout, ...(scenario.asynchronous ? {} : { encoding: "utf8" }) } }])
    const rows = scenario.asynchronous ? result : result.processes
    expect(rows.map((row: any) => [row.id, row.residentBytes, row.privateBytes])).toEqual([[41, 10240, null], [82, 20480, null]])
  })
}

test("three-map timeout is propagated; 2Fort process failure remains an error, not memory evidence", async () => {
  const timeout = Object.assign(new Error("memory helper timed out"), { code: "ETIMEDOUT" })
  const three = await helper("three-map-load.profile.ts", "residentProcesses", "executeFile", () => Promise.reject(timeout), "win32")
  await expect(three(input)).rejects.toBe(timeout)
  const fort = await helper("ctf-2fort-performance.profile.ts", "processMemory", "spawnSync", () => ({ status: null, stdout: "", stderr: "memory helper timed out" }), "win32")
  expect(fort(input)).toEqual({ processes: [], roles: {}, residentBytes: 0, error: "memory helper timed out" })
})

test("helper launch fixes do not change the headed scenario or its sampling schedule", async () => {
  const three = await Bun.file(new URL("../profile/three-map-load.profile.ts", import.meta.url)).text()
  const fort = await Bun.file(new URL("../profile/ctf-2fort-performance.profile.ts", import.meta.url)).text()
  expect(three).toContain("const SAMPLE_MILLISECONDS = 2_000")
  expect(three).toContain("setInterval(() => { void sampleProcesses() }, 250)")
  expect(fort).toContain("const seconds = profileSampleSeconds()")
  for (const source of [three, fort]) {
    expect(source).toContain('from "./application-test"')
    expect(source).not.toMatch(/headless:\s*true|SetForegroundWindow|SetPriorityClass/)
  }
  for (const configuration of ["playwright.three-map-load.config.ts", "playwright.profile.config.ts"]) {
    expect(await Bun.file(new URL(`../../../${configuration}`, import.meta.url)).text()).toContain("export default headedProfileConfiguration({")
  }
  expect(await Bun.file(new URL("../profile/profile-config.ts", import.meta.url)).text()).toContain("headless: false")
})
