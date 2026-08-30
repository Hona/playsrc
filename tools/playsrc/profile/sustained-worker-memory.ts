import type { Worker } from "@playwright/test"

/** Rayon helpers may be inside a blocking WASM/Atomics wait. They neither own
 * the gameplay memory publication nor accept ordinary Runtime.evaluate there. */
export async function sustainedWorkerMemory(workers: readonly Pick<Worker, "url" | "evaluate">[], timeoutMilliseconds = 5000) {
  const owners = workers.filter(worker => worker.url().includes("gameplay-worker"))
  if (owners.length !== 1) throw new Error("Sustained memory requires one gameplay Worker owner")
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    const memory = await Promise.race([
      owners[0]!.evaluate(() => ({ url: location.href, memory: (globalThis as any).__playsrcWorkerMemory ?? null })),
      new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Gameplay Worker memory read exceeded its deadline")), timeoutMilliseconds) }),
    ])
    return { owner: memory, unqueriedWorkerUrls: workers.filter(worker => worker !== owners[0]).map(worker => worker.url()) }
  } finally { clearTimeout(timer) }
}
