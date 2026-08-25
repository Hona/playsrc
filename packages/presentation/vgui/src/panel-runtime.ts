import { VGUI_CSS } from "./style"
import { registerVguiWindowWorkspace, type VguiWindowWorkspaceRegistration } from "./window-workspace"

type VguiDomLimits = Readonly<{
  maxDomNodes: number
  maxListeners: number
}>

type ListenerRecord = Readonly<{
  target: EventTarget
  type: string
  listener: EventListener
  options?: AddEventListenerOptions | boolean
}>

export type VguiControlKind =
  | "Frame"
  | "Panel"
  | "Label"
  | "RichText"
  | "TextEntry"
  | "Button"
  | "Menu"
  | "MenuItem"

export class VguiControl {
  readonly children: VguiControl[] = []

  constructor(
    private readonly runtime: VguiDomRuntime,
    readonly kind: VguiControlKind,
    readonly name: string,
    readonly element: HTMLElement,
    readonly parent: VguiControl | null,
  ) {}

  append(child: VguiControl): void {
    if (child.parent !== this) throw new Error("VGUI child parent differs")
    this.children.push(child)
    this.element.append(child.element)
  }

  removeChild(child: VguiControl): void {
    const index = this.children.indexOf(child)
    if (index >= 0) this.children.splice(index, 1)
  }

  setBounds(x: number, y: number, width: number, height: number): void {
    this.element.style.left = `${x}px`
    this.element.style.top = `${y}px`
    this.element.style.width = `${width}px`
    this.element.style.height = `${height}px`
  }

  setVisible(visible: boolean): void {
    if (this.element.hidden === visible) this.element.hidden = !visible
    this.element.setAttribute("aria-hidden", visible ? "false" : "true")
  }

  moveToFront(): void {
    this.element.parentElement?.append(this.element)
  }

  destroy(): void {
    this.runtime.releaseControl(this)
  }
}

export class VguiDomRuntime {
  readonly document: Document
  readonly host: HTMLElement
  readonly workspace: VguiWindowWorkspaceRegistration
  private readonly style: HTMLStyleElement
  private readonly nodes = new Set<Node>()
  private readonly listeners: ListenerRecord[] = []
  private destroyed = false

  constructor(
    readonly root: HTMLElement,
    private readonly limits: VguiDomLimits,
  ) {
    this.document = root.ownerDocument
    this.reserveNodes(2)
    this.style = this.document.createElement("style")
    this.style.dataset.playsrcVgui = "developer-console"
    this.style.textContent = VGUI_CSS
    this.host = this.document.createElement("div")
    this.host.className = "playsrc-vgui-root"
    this.host.dataset.vguiOwner = "playsrc"
    this.workspace = registerVguiWindowWorkspace(root, this.host)
    this.nodes.add(this.style)
    this.nodes.add(this.host)
    try {
      root.append(this.style, this.host)
    } catch (error) {
      this.workspace.destroy()
      this.host.remove()
      this.style.remove()
      this.nodes.clear()
      throw error
    }
  }

  createControl(
    kind: VguiControlKind,
    name: string,
    tag: keyof HTMLElementTagNameMap,
    parent: VguiControl | null,
  ): VguiControl {
    this.assertLive()
    this.reserveNodes(1)
    const element = this.document.createElement(tag)
    element.classList.add("playsrc-vgui-control")
    element.dataset.vguiControl = kind
    element.dataset.vguiName = name
    this.nodes.add(element)
    const control = new VguiControl(this, kind, name, element, parent)
    if (parent) parent.append(control)
    else this.host.append(element)
    return control
  }

  createOwnedNode(tag: keyof HTMLElementTagNameMap): HTMLElement {
    this.assertLive()
    this.reserveNodes(1)
    const node = this.document.createElement(tag)
    this.nodes.add(node)
    return node
  }

  releaseOwnedNode(node: Node): void {
    if (!this.nodes.delete(node)) return
    if ("remove" in node && typeof node.remove === "function") node.remove()
  }

  releaseControl(control: VguiControl): void {
    for (const child of [...control.children].reverse()) child.destroy()
    control.children.length = 0
    control.parent?.removeChild(control)
    this.releaseOwnedNode(control.element)
  }

  listen(
    target: EventTarget,
    type: string,
    listener: EventListener,
    options?: AddEventListenerOptions | boolean,
  ): void {
    this.assertLive()
    if (this.listeners.length >= this.limits.maxListeners) throw new Error("ListenerLimit")
    target.addEventListener(type, listener, options)
    this.listeners.push(Object.freeze({ target, type, listener, options }))
  }

  counts(): Readonly<{ nodes: number; listeners: number }> {
    return Object.freeze({ nodes: this.nodes.size, listeners: this.listeners.length })
  }

  destroy(): void {
    if (this.destroyed) return
    this.destroyed = true
    for (const record of this.listeners.splice(0).reverse()) {
      record.target.removeEventListener(record.type, record.listener, record.options)
    }
    this.workspace.destroy()
    this.host.remove()
    this.style.remove()
    this.nodes.clear()
  }

  private reserveNodes(count: number): void {
    if (this.nodes.size + count > this.limits.maxDomNodes) throw new Error("DomLimit")
  }

  private assertLive(): void {
    if (this.destroyed) throw new Error("VGUI runtime is destroyed")
  }
}
