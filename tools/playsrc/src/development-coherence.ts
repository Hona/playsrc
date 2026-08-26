export type DevelopmentBuildCoherence = Readonly<{ ensure(): Promise<void> }>

export function createDevelopmentBuildCoherence(
  initialIdentity: string,
  currentIdentity: () => Promise<string>,
  replace: (identity: string) => Promise<void>,
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
          await replace(identity)
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
