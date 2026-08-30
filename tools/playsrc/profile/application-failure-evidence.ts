const owners = new WeakMap<object, (error: Error) => Promise<void>>()

/** Evidence owns only bounded pre-teardown retention, never failure admission. */
export function retainBeforeApplicationFailure(page: object, retain: (error: Error) => Promise<void>): () => void {
  if (owners.has(page)) throw new Error("Application failure evidence already has an owner")
  owners.set(page, retain)
  return () => owners.delete(page)
}

export async function rejectAfterApplicationFailureEvidence(page: object, error: Error, milliseconds = 8000): Promise<never> {
  const retain = owners.get(page)
  owners.delete(page)
  let deadline: ReturnType<typeof setTimeout> | undefined
  try {
    if (retain) await Promise.race([retain(error), new Promise<never>((_, reject) => {
      deadline = setTimeout(() => reject(new Error("Failed application evidence retention exceeded deadline")), milliseconds)
    })])
  } catch (retentionError) { error.message += `\nEvidence retention: ${String(retentionError)}` }
  finally { clearTimeout(deadline) }
  throw error
}
