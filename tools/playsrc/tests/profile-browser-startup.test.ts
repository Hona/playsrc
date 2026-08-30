import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { effectiveProfileBrowserLaunch } from "../src/profile-browser"

test("the leased server uses the child's explicit channel without dropping launch arguments", () => {
  const args = ["--enable-precise-memory-info"]
  expect(effectiveProfileBrowserLaunch({ channel: "msedge", launchOptions: { args } }, { PLAYSRC_PROFILE_BROWSER_CHANNEL: "chrome" })).toEqual({ channel: "chrome", args })
  expect(effectiveProfileBrowserLaunch({ channel: "msedge" }, {})).toEqual({ channel: "msedge" })
  expect(effectiveProfileBrowserLaunch({}, {})).toEqual({})
})

test("failed browser startup closes its control pipe instead of exhausting the profile deadline", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "playsrc-browser-failure-"))
  let child: ReturnType<typeof Bun.spawn> | undefined
  try {
    const preload = path.join(directory, "no-browser.cjs")
    // No browser is launched. Exercise the actual Node transport with only its
    // launch dependency replaced by the missing-executable failure boundary.
    await writeFile(preload, `const Module=require('node:module'),load=Module._load;Module._load=function(name,...args){if(name==='@playwright/test')return {chromium:{launchServer:async()=>{throw new Error('fixture executable missing')}}};return load.call(this,name,...args)};`)
    const node = Bun.which("node")
    expect(node).not.toBeNull()
    child = Bun.spawn([node!, "--require", preload, path.resolve(import.meta.dir, "../src/profile-browser-server.cjs"), "{}"], { stdin: "pipe", stdout: "pipe", stderr: "pipe" })
    let timeout: ReturnType<typeof setTimeout> | undefined
    try {
      const code = await Promise.race([child.exited, new Promise<never>((_, reject) => { timeout = setTimeout(() => reject(new Error("Failed startup retained its open control pipe")), 2000) })])
      expect(code).toBe(1)
      expect(await new Response(child.stderr).text()).toContain("fixture executable missing")
      expect(await new Response(child.stdout).text()).toBe("")
    } finally { clearTimeout(timeout) }
  } finally { child?.kill(); await rm(directory, { recursive: true, force: true }) }
})
