import { expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { acquireHeadedProfileLock, delegatedHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"

test("the console child borrows one live checked owner without acquiring or releasing another lock", async () => {
  const directory=await mkdtemp(path.join(os.tmpdir(),"playsrc-console-lock-")),file=path.join(directory,"profile.lock")
  const startedAt=Date.now(),owner=await acquireHeadedProfileLock(file,"selection-transition",1000)
  const request={...owner,pid:process.pid,startedAt}
  try {
    const before=await readFile(file,"utf8")
    expect(await delegatedHeadedProfileLock(file,JSON.stringify(request),process.cwd(),"selection-transition")).toEqual(request)
    expect(await readFile(file,"utf8")).toBe(before)
    expect(await delegatedHeadedProfileLock(file,undefined,process.cwd(),"selection-transition")).toBeUndefined()
    await expect(delegatedHeadedProfileLock(file,JSON.stringify({...request,token:"another-owner"}),process.cwd(),"selection-transition")).rejects.toThrow("live local-job")
    await expect(delegatedHeadedProfileLock(file,JSON.stringify(request),directory,"selection-transition")).rejects.toThrow("live local-job")
    await expect(delegatedHeadedProfileLock(file,JSON.stringify({...request,startedAt:0}),process.cwd(),"selection-transition")).rejects.toThrow("live local-job")
    await expect(delegatedHeadedProfileLock(file,JSON.stringify(request),process.cwd(),"different-profile")).rejects.toThrow("live local-job")
    await releaseHeadedProfileLock(file,owner.token)
    await expect(delegatedHeadedProfileLock(file,JSON.stringify(request),process.cwd(),"selection-transition")).rejects.toThrow("live local-job")
  } finally {await releaseHeadedProfileLock(file,owner.token).catch(()=>{});await rm(directory,{recursive:true,force:true})}
})
