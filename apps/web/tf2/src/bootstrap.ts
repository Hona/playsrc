/** This shell must not import the renderer, generated WASM, or UI resources.
 * A failed module graph otherwise leaves the static document completely blank. */
export async function bootstrapApplication(shell: HTMLElement, load: () => Promise<unknown>): Promise<void> {
  try {
    await load()
    shell.remove()
  } catch {
    shell.setAttribute("role", "alert")
    shell.querySelector("h1")!.textContent = "Unable to start TF2"
    shell.querySelector("p")!.textContent = "The application module could not load. Check the connection, then reload."
  }
}

if (typeof document !== "undefined") {
  const shell = document.getElementById("bootstrap-status")
  if (shell) void bootstrapApplication(shell, () => import("./main"))
}
