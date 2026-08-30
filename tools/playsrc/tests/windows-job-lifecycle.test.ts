import { expect, test } from "bun:test"
import { mkdir } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import path from "node:path"
import { loadLocalConfig } from "../src/config"

test.skipIf(process.platform !== "win32")("native lifecycle: zero background UI in every branch, exactly one interactive decision, closure before dispatch and teardown before completion", async () => {
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, "evidence/windows-job-lifecycle-tests", randomUUID())
  await mkdir(directory, { recursive: true })
  const child = Bun.spawn(["powershell.exe", "-NoProfile", "-NonInteractive", "-File", path.join(import.meta.dir, "fixtures/windows-job-lifecycle.ps1"), "-Directory", directory], { windowsHide: true, stdout: "pipe", stderr: "pipe" })
  const [output, errors, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
  expect({ code, errors }).toEqual({ code: 0, errors: "" })
  expect(JSON.parse(output)).toEqual({ cases: 32, backgroundUiInvocations: 0, testOnly: true })
}, 30_000)
