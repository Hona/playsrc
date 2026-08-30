import { expect, test } from "bun:test"
import { bootstrapApplication } from "../src/bootstrap"

function shell() {
  const heading = { textContent: "Starting TF2" }, detail = { textContent: "Loading the application…" }
  const state = { removed: false, role: "status" }
  return { heading, detail, state, element: {
    querySelector: (selector: string) => selector === "h1" ? heading : detail,
    setAttribute: (_key: string, value: string) => { state.role = value },
    remove: () => { state.removed = true },
  } as unknown as HTMLElement }
}

test("the independent shell remains actionable when the main module graph fails", async () => {
  const view = shell()
  await bootstrapApplication(view.element, async () => { throw new Error("private path/URL") })
  expect(view.state).toEqual({ removed: false, role: "alert" })
  expect(view.heading.textContent).toBe("Unable to start TF2")
  expect(view.detail.textContent).toContain("module could not load")
  expect(view.detail.textContent).not.toContain("private")
})

test("normal startup removes only its own bootstrap shell", async () => {
  const view = shell()
  await bootstrapApplication(view.element, async () => {})
  expect(view.state.removed).toBe(true)
})
