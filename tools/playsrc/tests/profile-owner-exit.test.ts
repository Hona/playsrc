import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"

test("an expired owner exits even when a closed development service leaves referenced handles", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-exit-"))
  const metadata = path.join(directory, "owner.json")
  const source = path.resolve(import.meta.dir, "../src")
  await writeFile(`${metadata}.lease`, JSON.stringify({ schema: "playsrc-profile-owner-lease-v1", token: "expired-owner", expiresAt: 0 }))
  const child = Bun.spawn([process.execPath, "-e", `
    import { mock } from "bun:test";
    mock.module(${JSON.stringify(path.join(source,"config.ts"))},()=>({repositoryRoot:${JSON.stringify(directory)},loadLocalConfig:async()=>({})}));
    mock.module(${JSON.stringify(path.join(source,"profile-identity.ts"))},()=>({generatedProfileIdentity:async()=>"generated"}));
    mock.module(${JSON.stringify(path.join(source,"dev.ts"))},()=>({startDevelopment:async()=>{
      setInterval(()=>{},1000);
      return {url:"http://127.0.0.1:4173",startup:{totalMilliseconds:0},close:async()=>{}};
    }}));
    process.argv[2]="cp_gorge";
    await import(${JSON.stringify(path.join(source,"profile-owner.ts"))});
  `], { env: { ...process.env, PLAYSRC_PROFILE_OWNER_TOKEN: "expired-owner", PLAYSRC_PROFILE_SOURCE_IDENTITY: "source", PLAYSRC_PROFILE_OWNER_PATH: metadata }, stdout: "ignore", stderr: "pipe" })
  let deadline: ReturnType<typeof setTimeout> | undefined
  try {
    const result = await Promise.race([child.exited, new Promise<string>(resolve => { deadline=setTimeout(()=>resolve("stranded after close"),3000) })])
    expect(result).toBe(0)
  } finally {
    if (deadline) clearTimeout(deadline)
    if (child.exitCode === null) { child.kill("SIGKILL"); await child.exited }
    await rm(directory,{recursive:true,force:true})
  }
}, 10_000)
