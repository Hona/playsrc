import { writeParticleCenters } from "../../../packages/presentation/rendering/src/particle-attributes"
import { loadLocalConfig } from "./config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import path from "node:path"
const config = await loadLocalConfig()
const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
const lock = await acquireHeadedProfileLock(lockPath, "particle-packing-offline", 5000)
try {
  const array = new Float32Array(16), position = Object.freeze([1.1, 2.2, 3.3]) as readonly [number, number, number]
  const generic = () => { for (let vertex = 0; vertex < 4; vertex++) { array.set(position, vertex * 4); array[vertex * 4 + 3] = 1 } }
  const scalar = () => writeParticleCenters(array, 0, position, 1)
  const milliseconds = []
  for (const write of [generic, scalar]) {
    for (let index = 0; index < 10000; index++) write()
    const start = performance.now()
    for (let index = 0; index < 500000; index++) write()
    milliseconds.push(performance.now() - start)
  }
  console.log(JSON.stringify({ engine: process.versions, iterations: 500000, milliseconds, values: [...array], visiblePerformanceAcceptance: false }))
} finally { await releaseHeadedProfileLock(lockPath, lock.token) }
