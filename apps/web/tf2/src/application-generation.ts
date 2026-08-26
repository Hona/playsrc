const HASH = /^[0-9a-f]{64}$/
const STORAGE_KEY = "playsrc.tf2.application-generation.v1"
const MAX_UPGRADES = 3

export function resourceGenerationMatches(
  configuration: Readonly<{ wasm: Readonly<{ sha256: string }>; targets: readonly Readonly<{ target: string; objects: Readonly<{ resources: Readonly<{ sha256: string }> }> }>[] }>,
  wasmSha256: string,
  resourceRoots: Readonly<Record<string, string>>,
): boolean {
  return configuration.wasm.sha256 === wasmSha256
    && configuration.targets.every((target) => Object.hasOwn(resourceRoots, target.target)
      && resourceRoots[target.target] === target.objects.resources.sha256)
}

export type ApplicationGenerationRecovery = Readonly<{
  ensure(expectedBuild: string, generationMismatch?: boolean): Promise<boolean>
  complete(): void
}>

export function createApplicationGenerationRecovery(options: Readonly<{
  currentBuild: string
  storage: Pick<Storage, "getItem" | "setItem">
  visible(): boolean
  whenVisible(): Promise<void>
  reload(): void
}>): ApplicationGenerationRecovery {
  let pending: Promise<boolean> | undefined
  return Object.freeze({
    complete(): void {
      // A matching startup alone does not prove convergence. Only actual map Ready
      // ends the episode; failed boot loops must retain their retry budget.
      if (options.storage.getItem(STORAGE_KEY) !== null) options.storage.setItem(STORAGE_KEY, "[]")
    },
    async ensure(expectedBuild: string, staleWorker = false): Promise<boolean> {
      if (!HASH.test(options.currentBuild) || !HASH.test(expectedBuild)) {
        throw new Error("Application generation identity is invalid")
      }
      if (expectedBuild === options.currentBuild && !staleWorker) return true
      if (pending) return pending
      const operation = (async () => {
        const identity = `${options.currentBuild}:${expectedBuild}`
        const value = options.storage.getItem(STORAGE_KEY)
        if (value !== null && value.length > MAX_UPGRADES * 132 + 1) throw new Error("Application generation recovery state is invalid")
        let previous: unknown
        try { previous = value === null ? [] : JSON.parse(value) }
        catch { throw new Error("Application generation recovery state is invalid") }
        if (!Array.isArray(previous) || previous.length > MAX_UPGRADES || previous.some((entry) => typeof entry !== "string" || !/^[0-9a-f]{64}:[0-9a-f]{64}$/.test(entry))) {
          throw new Error("Application generation recovery state is invalid")
        }
        const current = previous.filter((entry) => entry.startsWith(`${options.currentBuild}:`))
        if (current.includes(identity) || current.length >= MAX_UPGRADES) {
          throw new Error("Application generation upgrade did not converge")
        }
        options.storage.setItem(STORAGE_KEY, JSON.stringify([...current, identity]))
        if (!options.visible()) await options.whenVisible()
        options.reload()
        return false
      })()
      pending = operation
      try { return await operation }
      finally { if (pending === operation) pending = undefined }
    },
  })
}
