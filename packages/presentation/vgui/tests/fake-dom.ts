type FakeListener = (event: FakeEvent) => void

export class FakeEvent {
  target: FakeElement | null = null
  currentTarget: FakeElement | null = null
  defaultPrevented = false
  readonly bubbles: boolean
  readonly relatedTarget: FakeElement | null
  readonly key: string
  readonly code: string
  readonly altKey: boolean
  readonly ctrlKey: boolean
  readonly metaKey: boolean
  readonly shiftKey: boolean

  constructor(
    readonly type: string,
    init: Readonly<{
      bubbles?: boolean
      relatedTarget?: FakeElement | null
      key?: string
      code?: string
      altKey?: boolean
      ctrlKey?: boolean
      metaKey?: boolean
      shiftKey?: boolean
    }> = {},
  ) {
    this.bubbles = init.bubbles ?? false
    this.relatedTarget = init.relatedTarget ?? null
    this.key = init.key ?? ""
    this.code = init.code ?? ""
    this.altKey = init.altKey ?? false
    this.ctrlKey = init.ctrlKey ?? false
    this.metaKey = init.metaKey ?? false
    this.shiftKey = init.shiftKey ?? false
  }

  preventDefault(): void {
    this.defaultPrevented = true
  }

  stopPropagation(): void {
    this.propagationStopped = true
  }

  propagationStopped = false
}

class FakeClassList {
  constructor(private readonly element: FakeElement) {}

  add(...names: string[]): void {
    const classes = new Set(this.element.className.split(/\s+/u).filter(Boolean))
    for (const name of names) classes.add(name)
    this.element.className = [...classes].join(" ")
  }

  contains(name: string): boolean {
    return this.element.className.split(/\s+/u).includes(name)
  }
}

class FakeStyle {
  [name: string]: unknown

  private readonly properties = new Map<string, string>()

  setProperty(name: string, value: string): void {
    this.properties.set(name, value)
  }

  getPropertyValue(name: string): string {
    return this.properties.get(name) ?? ""
  }
}

export class FakeElement {
  readonly nodeType = 1
  readonly dataset: Record<string, string> = {}
  readonly style = new FakeStyle()
  readonly classList = new FakeClassList(this)
  readonly children: FakeElement[] = []
  parentElement: FakeElement | null = null
  className = ""
  hidden = false
  id = ""
  tabIndex = 0
  value = ""
  scrollTop = 0
  private ownText = ""
  private readonly attributes = new Map<string, string>()
  private readonly listeners = new Map<string, FakeListener[]>()

  constructor(
    readonly ownerDocument: FakeDocument,
    readonly tagName: string,
  ) {}

  get childNodes(): readonly FakeElement[] {
    return this.children
  }

  get textContent(): string {
    return this.ownText + this.children.map((child) => child.textContent).join("")
  }

  set textContent(value: string) {
    for (const child of this.children) child.parentElement = null
    this.children.length = 0
    this.ownText = String(value ?? "")
  }

  get scrollHeight(): number {
    return Math.max(0, this.children.length * 20)
  }

  append(...nodes: FakeElement[]): void {
    for (const node of nodes) {
      node.remove()
      node.parentElement = this
      this.children.push(node)
    }
  }

  replaceChildren(...nodes: FakeElement[]): void {
    for (const child of this.children) child.parentElement = null
    this.children.length = 0
    this.ownText = ""
    this.append(...nodes)
  }

  remove(): void {
    if (!this.parentElement) return
    const index = this.parentElement.children.indexOf(this)
    if (index >= 0) this.parentElement.children.splice(index, 1)
    this.parentElement = null
  }

  contains(node: FakeElement): boolean {
    for (let current: FakeElement | null = node; current; current = current.parentElement) {
      if (current === this) return true
    }
    return false
  }

  setAttribute(name: string, value: string): void {
    const text = String(value)
    this.attributes.set(name, text)
    if (name === "id") this.id = text
  }

  getAttribute(name: string): string | null {
    if (name === "id" && this.id) return this.id
    return this.attributes.get(name) ?? null
  }

  removeAttribute(name: string): void {
    this.attributes.delete(name)
    if (name === "id") this.id = ""
  }

  addEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type) ?? []
    listeners.push(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type)
    if (!listeners) return
    const index = listeners.indexOf(listener)
    if (index >= 0) listeners.splice(index, 1)
  }

  dispatchEvent(event: FakeEvent): boolean {
    if (!event.target) event.target = this
    event.currentTarget = this
    for (const listener of [...(this.listeners.get(event.type) ?? [])]) listener(event)
    if (event.bubbles && !event.propagationStopped) this.parentElement?.dispatchEvent(event)
    return !event.defaultPrevented
  }

  focus(): void {
    const previous = this.ownerDocument.activeElement
    if (previous === this) return
    if (previous) previous.dispatchEvent(new FakeEvent("focusout", { relatedTarget: this }))
    this.ownerDocument.activeElement = this
    this.dispatchEvent(new FakeEvent("focus"))
  }

  blur(): void {
    if (this.ownerDocument.activeElement !== this) return
    this.ownerDocument.activeElement = null
    this.dispatchEvent(new FakeEvent("focusout", { relatedTarget: null }))
  }
}

export class FakeDocument {
  activeElement: FakeElement | null = null

  createElement(tag: string): FakeElement {
    return new FakeElement(this, tag.toUpperCase())
  }
}

export function createRoot(document = new FakeDocument()): FakeElement {
  return document.createElement("main")
}

export function byName(root: FakeElement, name: string): FakeElement {
  const result = descendants(root).find((element) => element.dataset.vguiName === name)
  if (!result) throw new Error(`missing VGUI node ${name}`)
  return result
}

export function descendants(root: FakeElement): FakeElement[] {
  return root.children.flatMap((child) => [child, ...descendants(child)])
}

export function input(element: FakeElement, value: string): FakeEvent {
  element.value = value
  const event = new FakeEvent("input")
  element.dispatchEvent(event)
  return event
}

export function key(
  element: FakeElement,
  value: string,
  init: Omit<ConstructorParameters<typeof FakeEvent>[1], "key"> = {},
): FakeEvent {
  const event = new FakeEvent("keydown", { ...init, key: value })
  element.dispatchEvent(event)
  return event
}

export function click(element: FakeElement): FakeEvent {
  const event = new FakeEvent("click", { bubbles: true })
  element.dispatchEvent(event)
  return event
}
