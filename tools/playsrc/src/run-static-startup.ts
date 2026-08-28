import { spawn, execFile } from "node:child_process"
import { mkdir } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import { profileNodeExecutable } from "./profile-browser"
import { requireWindowsProfileConsole } from "../profile/windows-desktop"

const started=Date.now(),config=await loadLocalConfig()
requireWindowsProfileConsole()
const lockPath=path.join(config.sourceCacheDir,"evidence/tf2-browser-performance/chromium-profile.lock")
const lock=await acquireHeadedProfileLock(lockPath,"static-startup",15_000)
try {
  const directory=path.join(config.sourceCacheDir,"profiles/static-startup",`${started}-${lock.token}`)
  await mkdir(directory,{recursive:true})
  const child=spawn(profileNodeExecutable(),[path.join(repositoryRoot,"node_modules/@playwright/test/cli.js"),"test","--config=tools/playsrc/static-startup.config.ts","--output",path.join(directory,"results")],{cwd:repositoryRoot,stdio:"inherit",detached:process.platform!=="win32",env:{...process.env,PLAYSRC_PROFILE_MANAGED:"1",PLAYSRC_PROFILE_RUN_DIRECTORY:directory}})
  const terminate=(force=false)=>{
    if(!child.pid||child.exitCode!==null)return
    if(process.platform==="win32")execFile("taskkill.exe",["/PID",String(child.pid),"/T",...(force?["/F"]:[])],{timeout:5000},()=>{})
    else {try{process.kill(-child.pid,force?"SIGKILL":"SIGTERM")}catch(error){if((error as NodeJS.ErrnoException).code!=="ESRCH")throw error}}
  }
  const stop=setTimeout(()=>terminate(),Math.max(1,170_000-(Date.now()-started)))
  const hardStop=setTimeout(()=>terminate(true),Math.max(1,178_000-(Date.now()-started)))
  try {process.exitCode=await new Promise<number>((resolve,reject)=>{child.once("error",reject);child.once("exit",code=>resolve(code??1))})}
  finally {clearTimeout(stop);clearTimeout(hardStop)}
} finally {await releaseHeadedProfileLock(lockPath,lock.token)}
