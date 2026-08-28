import { rename } from "node:fs/promises"

/** Windows may reject replacement while the live owner briefly reads its
 * lease. Retry the same atomic rename, never unlink/empty the authoritative
 * lease or change its token. Permanent faults still abort within250ms. */
export async function replaceProfileLeaseFile(source: string, destination: string, options: {
  platform?: string
  replace?: typeof rename
  now?: () => number
  wait?: (milliseconds: number) => Promise<unknown>
} = {}): Promise<void> {
  const replace = options.replace ?? rename, now = options.now ?? (() => performance.now())
  const wait = options.wait ?? (milliseconds => Bun.sleep(milliseconds)), platform = options.platform ?? process.platform
  const deadline = now() + 250
  for (;;) {
    try { await replace(source, destination); return }
    catch (error) {
      const remaining = deadline - now()
      if (platform !== "win32" || remaining <= 0 || !["EPERM", "EACCES", "EBUSY"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error
      await wait(Math.min(10, remaining))
    }
  }
}
