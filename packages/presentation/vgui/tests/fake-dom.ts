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
  readonly repeat: boolean
  readonly isComposing: boolean
  readonly keyCode: number
  readonly button: number
  readonly buttons: number
  readonly pointerId: number
  readonly clientX: number
  readonly clientY: number

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
      repeat?: boolean
      isComposing?: boolean
      keyCode?: number
      button?: number
      buttons?: number
      pointerId?: number
      clientX?: number
      clientY?: number
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
    this.repeat = init.repeat ?? false
    this.isComposing = init.isComposing ?? false
    this.keyCode = init.keyCode ?? 0
    this.button = init.button ?? 0
    this.buttons = init.buttons ?? 0
    this.pointerId = init.pointerId ?? 1
    this.clientX = init.clientX ?? 0
    this.clientY = init.clientY ?? 0
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
  writes = 0

  constructor() {
    return new Proxy(this, { set(target, property, value) {
      if (property !== "writes") target.writes += 1
      return Reflect.set(target, property, value)
    } })
  }

  private readonly properties = new Map<string, string>()

  setProperty(name: string, value: string): void {
    this.writes += 1
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
  appendCalls = 0
  attributeWrites = 0
  textWrites = 0
  private ownText = ""
  private readonly attributes = new Map<string, string>()
  private readonly listeners = new Map<string, FakeListener[]>()
  private readonly capturedPointers = new Set<number>()

  constructor(
    readonly ownerDocument: FakeDocument,
    readonly tagName: string,
  ) {}

  get childNodes(): readonly FakeElement[] {
    return this.children
  }
  get parentNode(): FakeElement | null { return this.parentElement }
  get nextSibling(): FakeElement | null {
    return this.parentElement?.children[this.parentElement.children.indexOf(this) + 1] ?? null
  }
  insertBefore(node: FakeElement, next: FakeElement | null): void {
    node.remove()
    const index = next === null ? this.children.length : this.children.indexOf(next)
    if (index < 0) throw new Error("insertBefore reference is not a child")
    node.parentElement = this
    this.children.splice(index, 0, node)
  }

  get textContent(): string {
    return this.ownText + this.children.map((child) => child.textContent).join("")
  }

  set textContent(value: string) {
    this.textWrites += 1
    for (const child of this.children) child.parentElement = null
    this.children.length = 0
    this.ownText = String(value ?? "")
  }

  get scrollHeight(): number {
    return Math.max(0, this.children.length * 20)
  }

  append(...nodes: FakeElement[]): void {
    for (const node of nodes) {
      this.appendCalls += 1
      node.remove()
      node.parentElement = this
      this.children.push(node)
    }
  }

  prepend(...nodes: FakeElement[]): void {
    for (let index = nodes.length - 1; index >= 0; index -= 1) {
      const node = nodes[index]!
      node.remove()
      node.parentElement = this
      this.children.unshift(node)
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

  getBoundingClientRect(): Readonly<{ left: number; top: number; right: number; bottom: number; width: number; height: number }> {
    let left = Number.parseInt(String(this.style.left ?? "0"), 10) || 0
    let top = Number.parseInt(String(this.style.top ?? "0"), 10) || 0
    for (let parent = this.parentElement; parent; parent = parent.parentElement) {
      left += Number.parseInt(String(parent.style.left ?? "0"), 10) || 0
      top += Number.parseInt(String(parent.style.top ?? "0"), 10) || 0
    }
    const width = Number.parseInt(String(this.style.width ?? "0"), 10) || 0
    const height = Number.parseInt(String(this.style.height ?? "0"), 10) || 0
    return { left, top, right: left + width, bottom: top + height, width, height }
  }

  setAttribute(name: string, value: string): void {
    this.attributeWrites += 1
    const text = String(value)
    this.attributes.set(name, text)
    if (name === "id") this.id = text
  }

  getAttribute(name: string): string | null {
    if (name === "id" && this.id) return this.id
    return this.attributes.get(name) ?? null
  }

  removeAttribute(name: string): void {
    this.attributeWrites += 1
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
    if (previous) previous.dispatchEvent(new FakeEvent("focusout", { relatedTarget: this, bubbles: true }))
    this.ownerDocument.activeElement = this
    this.dispatchEvent(new FakeEvent("focus"))
    this.dispatchEvent(new FakeEvent("focusin", { bubbles: true }))
  }

  blur(): void {
    if (this.ownerDocument.activeElement !== this) return
    this.ownerDocument.activeElement = null
    this.dispatchEvent(new FakeEvent("focusout", { relatedTarget: null, bubbles: true }))
  }

  setPointerCapture(pointerId: number): void {
    this.capturedPointers.add(pointerId)
  }

  hasPointerCapture(pointerId: number): boolean {
    return this.capturedPointers.has(pointerId)
  }

  releasePointerCapture(pointerId: number): void {
    if (!this.capturedPointers.delete(pointerId)) return
    this.dispatchEvent(new FakeEvent("lostpointercapture", { pointerId }))
  }
}

export class FakeWindow {
  private readonly listeners = new Map<string, FakeListener[]>()

  addEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type) ?? []
    listeners.push(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type) ?? []
    const index = listeners.indexOf(listener)
    if (index >= 0) listeners.splice(index, 1)
  }

  dispatchEvent(event: FakeEvent): boolean {
    for (const listener of [...(this.listeners.get(event.type) ?? [])]) listener(event)
    return !event.defaultPrevented
  }
}

export class FakeDocument {
  activeElement: FakeElement | null = null
  hidden = false
  readonly defaultView = new FakeWindow()
  private readonly listeners = new Map<string, FakeListener[]>()

  createElement(tag: string): FakeElement {
    return new FakeElement(this, tag.toUpperCase())
  }

  createElementNS(_namespace: string, tag: string): FakeElement {
    return this.createElement(tag)
  }

  addEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type) ?? []
    listeners.push(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: FakeListener): void {
    const listeners = this.listeners.get(type) ?? []
    const index = listeners.indexOf(listener)
    if (index >= 0) listeners.splice(index, 1)
  }

  dispatchEvent(event: FakeEvent): boolean {
    for (const listener of [...(this.listeners.get(event.type) ?? [])]) listener(event)
    return !event.defaultPrevented
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

export function pointer(
  element: FakeElement,
  type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
  clientX: number,
  clientY: number,
  init: Readonly<{ pointerId?: number; button?: number; buttons?: number }> = {},
): FakeEvent {
  const event = new FakeEvent(type, {
    bubbles: true,
    clientX,
    clientY,
    pointerId: init.pointerId ?? 1,
    button: init.button ?? 0,
    buttons: init.buttons ?? (type === "pointerup" || type === "pointercancel" ? 0 : 1),
  })
  element.dispatchEvent(event)
  return event
}
