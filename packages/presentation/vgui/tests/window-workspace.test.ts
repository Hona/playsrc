import { describe, expect, test } from "bun:test"
import { registerVguiWindowWorkspace } from "../src/window-workspace"
import { FakeDocument } from "./fake-dom"

describe("cross-root VGUI window workspace", () => {
  test("projects activation, topmost, modal input, and teardown across independent stacking roots", () => {
    const document = new FakeDocument()
    const workspace = document.createElement("main")
    const optionsLayer = document.createElement("section")
    const optionsMount = document.createElement("div")
    const optionsHost = document.createElement("div")
    const developerLayer = document.createElement("section")
    const developerHost = document.createElement("div")
    optionsLayer.style.zIndex = "30"
    developerLayer.style.zIndex = "40"
    optionsLayer.append(optionsMount)
    optionsMount.append(optionsHost)
    developerLayer.append(developerHost)
    workspace.append(optionsLayer, developerLayer)

    const options = registerVguiWindowWorkspace(optionsMount as unknown as HTMLElement, optionsHost as unknown as HTMLElement)
    const developer = registerVguiWindowWorkspace(developerLayer as unknown as HTMLElement, developerHost as unknown as HTMLElement)

    developer.activate()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["30", "40"])
    options.activate()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["42", "41"])

    options.setTopmost(true)
    developer.activate()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["42", "41"])
    developer.setModal(true)
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["41", "42"])
    expect((optionsHost as unknown as HTMLElement).inert).toBeTrue()
    expect((developerHost as unknown as HTMLElement).inert ?? false).toBeFalse()
    developer.setModal(false)
    expect((optionsHost as unknown as HTMLElement).inert).toBeFalse()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["42", "41"])

    options.deactivate()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["30", "40"])
    options.activate()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["42", "41"])
    developer.destroy()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["30", "40"])
    options.destroy()
    options.destroy()
    expect([optionsLayer.style.zIndex, developerLayer.style.zIndex]).toEqual(["30", "40"])
  })

  test("never promotes unrelated panels in a different document", () => {
    const firstDocument = new FakeDocument()
    const secondDocument = new FakeDocument()
    const firstRoot = firstDocument.createElement("section")
    const secondRoot = secondDocument.createElement("section")
    const firstHost = firstDocument.createElement("div")
    const secondHost = secondDocument.createElement("div")
    firstRoot.style.zIndex = "7"
    secondRoot.style.zIndex = "19"
    firstRoot.append(firstHost)
    secondRoot.append(secondHost)

    const first = registerVguiWindowWorkspace(firstRoot as unknown as HTMLElement, firstHost as unknown as HTMLElement)
    const second = registerVguiWindowWorkspace(secondRoot as unknown as HTMLElement, secondHost as unknown as HTMLElement)
    first.activate(true)
    second.activate()
    expect(firstRoot.style.zIndex).toBe("7")
    expect(secondRoot.style.zIndex).toBe("19")
    first.destroy()
    second.destroy()
  })
})
