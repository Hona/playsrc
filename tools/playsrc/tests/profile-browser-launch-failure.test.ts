import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import os from "node:os"
import { profileNodeExecutable } from "../src/profile-browser"

test("failed native browser launch does not leave its Node stdin owner alive", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-launch-failure-"))
  const preload = path.join(directory, "preload.cjs")
  // Mock only the launch error. This test opens no browser and is not UI evidence.
  await writeFile(preload, `const M=require('node:module'),load=M._load;M._load=function(name,...args){return name==='@playwright/test'?{chromium:{launchServer:async()=>{throw Error('missing executable fixture')}}}:load.call(this,name,...args)}`)
  const child = Bun.spawn([profileNodeExecutable(), "--require", preload, path.resolve(import.meta.dir, "../src/profile-browser-server.cjs"), "{}"], { stdin: "pipe", stdout: "pipe", stderr: "pipe" })
  const timer = setTimeout(() => child.kill(), 2_000)
  try {
    expect(await child.exited).toBe(1)
    expect(await new Response(child.stderr).text()).toContain("missing executable fixture")
  } finally { clearTimeout(timer); child.kill(); await child.exited; await rm(directory, { recursive: true, force: true }) }
})
