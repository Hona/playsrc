export type DevelopmentBuildCoherence = Readonly<{ ensure(): Promise<void> }>

export function createDevelopmentBuildCoherence(
  initialIdentity: string,
  currentIdentity: () => Promise<string>,
  prepare: (identity: string) => Promise<() => void>,
): DevelopmentBuildCoherence {
  let publishedIdentity = initialIdentity
  let pending: Promise<void> | undefined

  return Object.freeze({
    async ensure(): Promise<void> {
      if (pending) return pending
      const operation = (async () => {
        for (let replacements = 0; replacements < 8; replacements += 1) {
          const identity = await currentIdentity()
          if (identity === publishedIdentity) return
          const publish = await prepare(identity)
          // Compiling is asynchronous; a newer checkout must not be announced
          // with the old identity, even briefly to an already-open tab.
          if (await currentIdentity() !== identity) continue
          publish()
          publishedIdentity = identity
        }
        throw new Error("Development build changed continuously during replacement")
      })()
      pending = operation
      try {
        await operation
      } finally {
        if (pending === operation) pending = undefined
      }
    },
  })
}
