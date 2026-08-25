type WindowEntry = {
  readonly root: HTMLElement
  readonly host: HTMLElement
  active: boolean
  modal: boolean
  topmost: boolean
  order: number
}

type ManagedLayer = Readonly<{ element: HTMLElement; zIndex: string }>

type WindowWorkspace = {
  readonly entries: Set<WindowEntry>
  readonly managedLayers: Map<HTMLElement, ManagedLayer>
  readonly managedInert: Map<HTMLElement, boolean>
  order: number
}

export type VguiWindowWorkspaceRegistration = Readonly<{
  activate(topmost?: boolean): void
  deactivate(): void
  setModal(modal: boolean): void
  setTopmost(topmost: boolean): void
  destroy(): void
}>

const workspaces = new WeakMap<Document, WindowWorkspace>()

function commonAncestor(entries: readonly WindowEntry[]): HTMLElement | null {
  let ancestor: HTMLElement | null = entries[0]?.root ?? null
  while (ancestor && !entries.every((entry) => entry.root === ancestor || ancestor!.contains(entry.root))) {
    ancestor = ancestor.parentElement
  }
  return ancestor
}

function layerFor(root: HTMLElement, ancestor: HTMLElement): HTMLElement {
  let element = root
  while (element.parentElement && element.parentElement !== ancestor) element = element.parentElement
  return element
}

function layerZIndex(element: HTMLElement): number {
  const view = element.ownerDocument.defaultView
  const computed = view && typeof view.getComputedStyle === "function" ? view.getComputedStyle(element).zIndex : element.style.zIndex
  const value = Number.parseInt(computed ?? "", 10)
  return Number.isSafeInteger(value) ? value : 0
}

function refresh(workspace: WindowWorkspace): void {
  for (const { element, zIndex } of workspace.managedLayers.values()) element.style.zIndex = zIndex
  workspace.managedLayers.clear()
  for (const [host, inert] of workspace.managedInert) host.inert = inert
  workspace.managedInert.clear()

  const active = [...workspace.entries].filter((entry) => entry.active)
  const modal = active.find((entry) => entry.modal)
  if (modal) {
    for (const entry of workspace.entries) {
      if (entry === modal || workspace.managedInert.has(entry.host)) continue
      workspace.managedInert.set(entry.host, entry.host.inert ?? false)
      entry.host.inert = true
    }
  }

  if (active.length < 2) return
  const ancestor = commonAncestor(active)
  if (!ancestor) return
  const layers = new Map<HTMLElement, WindowEntry>()
  for (const entry of active) {
    const element = layerFor(entry.root, ancestor)
    const existing = layers.get(element)
    if (!existing || Number(entry.modal) > Number(existing.modal)
      || (entry.modal === existing.modal && Number(entry.topmost) > Number(existing.topmost))
      || (entry.modal === existing.modal && entry.topmost === existing.topmost && entry.order > existing.order)) {
      layers.set(element, entry)
    }
  }
  if (layers.size < 2) return
  const ordered = [...layers].sort(([, left], [, right]) =>
    Number(left.modal) - Number(right.modal)
    || Number(left.topmost) - Number(right.topmost)
    || left.order - right.order)
  const base = Math.max(...ordered.map(([element]) => layerZIndex(element)))
  for (const [index, [element]] of ordered.entries()) {
    workspace.managedLayers.set(element, Object.freeze({ element, zIndex: element.style.zIndex ?? "" }))
    element.style.zIndex = String(base + index + 1)
  }
}

export function registerVguiWindowWorkspace(root: HTMLElement, host: HTMLElement): VguiWindowWorkspaceRegistration {
  const document = root.ownerDocument
  let workspace = workspaces.get(document)
  if (!workspace) {
    workspace = { entries: new Set(), managedLayers: new Map(), managedInert: new Map(), order: 0 }
    workspaces.set(document, workspace)
  }
  const current = workspace
  const entry: WindowEntry = { root, host, active: false, modal: false, topmost: false, order: 0 }
  current.entries.add(entry)
  let destroyed = false

  return Object.freeze({
    activate(topmost = false): void {
      if (destroyed) return
      entry.active = true
      entry.topmost = topmost
      entry.order = ++current.order
      refresh(current)
    },
    deactivate(): void {
      if (destroyed || !entry.active) return
      entry.active = false
      entry.modal = false
      entry.topmost = false
      refresh(current)
    },
    setModal(modal: boolean): void {
      if (destroyed || entry.modal === modal) return
      entry.modal = modal
      if (modal && !entry.active) {
        entry.active = true
        entry.order = ++current.order
      }
      refresh(current)
    },
    setTopmost(topmost: boolean): void {
      if (destroyed || entry.topmost === topmost) return
      entry.topmost = topmost
      refresh(current)
    },
    destroy(): void {
      if (destroyed) return
      destroyed = true
      current.entries.delete(entry)
      refresh(current)
      if (current.entries.size === 0) workspaces.delete(document)
    },
  })
}
