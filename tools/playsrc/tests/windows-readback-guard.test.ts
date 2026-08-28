import { expect, test } from "bun:test"
import { randomUUID } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"

test.skipIf(process.platform!=="win32")("native readback helpers enforce independent lifetime and allocation limits", async()=>{
  const {sourceCacheDir}=await loadLocalConfig(),directory=path.join(sourceCacheDir,"evidence","tf2-perf-selection-pipeline",`readback-guard-${randomUUID()}`)
  await mkdir(directory,{recursive:true})
  const lockPath=path.join(sourceCacheDir,"evidence","tf2-browser-performance","chromium-profile.lock")
  const lock=await acquireHeadedProfileLock(lockPath,"readback-guard-regression",100000)
  const records=[]
  try {
    const text='#< CLIXML\r\n'+JSON.stringify({result:null,launchError:"retained failure"})+'\r\n'
    for(const mode of ["deadline","memory","plain","plain-utf16"]){
      const receipt=path.join(directory,`${mode}.json`),started=Date.now()
      const input=path.join(directory,`${mode}.log`)
      await writeFile(input,mode==="plain-utf16"?Buffer.from('\ufeff'+text,'utf16le'):text)
      const child=Bun.spawn(["powershell.exe","-NoProfile","-NonInteractive","-File",path.join(import.meta.dir,"fixtures/windows-readback-guard.ps1"),
        "-Guard",path.join(repositoryRoot,"tools/playsrc/windows-readback-guard.cs"),"-Receipt",receipt,"-Mode",mode.startsWith("plain")?"plain":mode,
        "-Reader",path.join(repositoryRoot,"tools/playsrc/windows-readback.ps1"),"-InputFile",input],{stdout:"pipe",stderr:"pipe",windowsHide:true})
      const timeout=setTimeout(()=>child.kill(),8000)
      const [code,stdout,stderr]=await Promise.all([child.exited,new Response(child.stdout).text(),new Response(child.stderr).text()])
      clearTimeout(timeout)
      const fault=await readFile(receipt,"utf8").then(JSON.parse).catch(()=>null)
      records.push({mode,pid:child.pid,code,stdout,stderr,fault,milliseconds:Date.now()-started})
      await writeFile(path.join(directory,"results.json"),JSON.stringify({lock,records},null,2))
      if(mode==="deadline"||mode==="memory"){
        expect(code).toBe(124);expect(fault.pid).toBe(child.pid);expect(fault.reason).toBe(mode==="deadline"?"deadline":"private-memory-limit")
      }else{expect(code).toBe(0);expect(fault).toBeNull();const result=JSON.parse(stdout);expect(result.launchError).toBe(text);expect(result.providerProperties).toBe(0)}
      expect(Date.now()-started).toBeLessThan(8000)
    }
    console.log(JSON.stringify({directory,records}))
  }finally{await releaseHeadedProfileLock(lockPath,lock.token)}
},160000)
