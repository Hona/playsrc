import { initializeVguiRuntime, type VguiOperation, type VguiPanelId, type VguiRuntime, type VguiViewport } from "@playsrc/vgui"
import type { Tf2UiResourceNode } from "../ui-resources"
import { tf2ClassPresentation } from "../class"
import type { Tf2Class } from "../codec"
import type { Tf2VguiResources } from "../ui-integration"
import type { Tf2EquipmentState, Tf2EquippedItem, Tf2SupportedItem } from "./types"
import { attachEquipmentSurface } from "./surface"

export type Tf2EquipmentPreview = Readonly<{ class: Tf2Class; equippedItems: readonly Tf2EquippedItem[]; fov: number; origin: readonly [number, number, number]; angles: readonly [number, number, number]; bounds: Readonly<{ x: number; y: number; width: number; height: number }> }>
export type Tf2EquipmentPresentationRequest = Readonly<{
  root: HTMLElement; resources: Tf2VguiResources; viewport: VguiViewport; reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>; random: Readonly<{ nextUnit(): number }>
  modelSurface?: HTMLCanvasElement
  onEquip(playerClass: Tf2Class, slot: number, definitionIndex: number | null): Promise<Tf2EquipmentState>
  onClose(): void
  onPreview(preview: Tf2EquipmentPreview | null): void
}>

const scalar = (node: Tf2UiResourceNode, name: string): string | undefined => node.children.find(child => child.name.toLowerCase() === name.toLowerCase())?.value ?? undefined
const block = (node: Tf2UiResourceNode, name: string): Tf2UiResourceNode => {
  const value = node.children.find(child => child.name.toLowerCase() === name.toLowerCase() && child.value === null)
  if (!value) throw new Error(`Missing authored equipment control ${name}`)
  return value
}
const QUALITY_SUFFIX = ["", "_1", "_2", "_Vintage", "_3", "_4", "_Unique", "_Community", "_Developer", "_SelfMade", "_Customized", "_Strange", "_Completed", "_Haunted", "_Collectors", "_PaintkitWeapon"] as const
const QUALITY_COLOR = ["Normal", "rarity1", "rarity2", "Vintage", "rarity3", "rarity4", "Unique", "Community", "Developer", "SelfMade", "Customized", "Strange", "Completed", "Haunted", "Collectors", "PaintkitWeapon"] as const

export class Tf2EquipmentPresentation {
  readonly #runtime: VguiRuntime
  readonly #request: Tf2EquipmentPresentationRequest
  readonly #localization: ReadonlyMap<string, string>
  #viewport: VguiViewport
  #state: Tf2EquipmentState | undefined
  #class: Tf2Class = 1
  #page: "classes" | "loadout" | "backpack" | "slot" = "classes"
  #slot = 0
  #visible = false
  #busy = false
  #pageNumber = 0
  #content: VguiPanelId | undefined
  #releaseSurface: (() => void) | undefined
  #selected: number | null = null
  readonly #itemPanels = new Map<VguiPanelId, Tf2SupportedItem>()
  #hover: { panel: VguiPanelId; item: Tf2SupportedItem; since: number } | undefined
  #tooltip: VguiPanelId | undefined

  constructor(request: Tf2EquipmentPresentationRequest) {
    this.#request = request; this.#viewport = request.viewport
    this.#localization = new Map(request.resources.localization.tokens.map(token => [token.name.replace(/^#/, "").toLowerCase(), token.value]))
    const initialized = initializeVguiRuntime({ runtimeIdentity: "tf2-equipment", root: request.root,
      rootControl: { control: "EditablePanel", name: "EquipmentViewport" }, viewport: request.viewport,
      limits: { maxPanels: 512, maxHierarchyDepth: 32, maxChildrenPerPanel: 128, maxResourceNodes: 4096, maxResourceDepth: 32, maxPropertiesPerPanel: 256,
        maxStringCodeUnits: 4095, maxTextCodeUnits: 65535, maxDialogVariables: 128, maxLocalizationTokens: 4096, maxSchemeColors: 1024, maxSchemeSettings: 2048,
        maxSchemeBorders: 512, maxSchemeImages: 2048, maxAnimationScripts: 16, maxAnimationSequences: 1024, maxAnimationCommands: 8192,
        maxActiveAnimations: 256, maxDelayedCommands: 256, maxQueuedMessages: 512, maxDiagnostics: 512, maxDomNodes: 2048, maxListeners: 64 },
      clock: request.clock, random: request.random, scheme: request.resources.clientScheme, localization: request.resources.localization,
      animationScripts: request.resources.animations, customControls: request.resources.customControls, reducedMotion: request.reducedMotion,
      onRequest: value => { if (value.kind === "command") this.#command(value.command) },
    })
    if (!initialized.ok) throw new Error(`Equipment UI ${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#apply({ kind: "set-panel-state", panel: 1, visible: false, proportional: true })
    request.root.addEventListener("pointerover", this.#enterItem)
    request.root.addEventListener("focusin", this.#enterItem)
    request.root.addEventListener("pointerout", this.#leaveItem)
    request.root.addEventListener("focusout", this.#leaveItem)
  }

  #apply(operation: VguiOperation): VguiPanelId | undefined {
    const result = this.#runtime.apply(operation)
    if (!result.ok) throw new Error(`Equipment UI ${result.diagnostic.code}:${result.diagnostic.subject}`)
    return result.panel
  }
  #create(parent: VguiPanelId, control: string, name: string, properties: Record<string, string>): VguiPanelId {
    return this.#apply({ kind: "create-panel", parent, control, name, properties: Object.entries(properties).map(([name, value]) => ({ name, value })) })!
  }
  #localize(value: string): string { return this.#localization.get(value.replace(/^#/, "").toLowerCase()) ?? value }
  #button(parent: VguiPanelId, name: string, text: string, command: string, x: string, y: string, width = "100", height = "25", extra: Record<string, string> = {}): VguiPanelId {
    return this.#create(parent, "CExButton", name, { xpos: x, ypos: y, wide: width, tall: height, labelText: text, command,
      font: "HudFontSmallBold", textAlignment: "center", border: "Econ.Button.Border.Default", sound_depressed: "UI/buttonclick.wav", sound_released: "UI/buttonclickrelease.wav", ...extra })
  }

  show(state: Tf2EquipmentState, playerClass?: Tf2Class): void {
    this.#state = state; this.#visible = true; this.#selected = null; this.#pageNumber = 0
    this.#page = playerClass ? "loadout" : "classes"; if (playerClass) this.#class = playerClass
    this.#request.root.style.display = "block"
    this.#apply({ kind: "set-panel-state", panel: 1, visible: true })
    this.#render()
  }
  visible(): boolean { return this.#visible }
  snapshot() { return { visible: this.#visible, page: this.#page, class: this.#class, vgui: this.#runtime.snapshot() } }
  hide(): void {
    this.#clearTooltip()
    this.#visible = false; this.#releaseSurface?.(); this.#releaseSurface = undefined
    this.#apply({ kind: "set-panel-state", panel: 1, visible: false }); this.#request.root.style.display = "none"
    this.#request.onPreview(null); this.#request.onClose()
  }
  #render(): void {
    if (!this.#visible || !this.#state) return
    this.#clearTooltip()
    this.#itemPanels.clear()
    this.#releaseSurface?.(); this.#releaseSurface = undefined
    if (this.#page !== "loadout") this.#request.onPreview(null)
    this.#runtime.deferPresentation(() => {
      if (this.#content !== undefined) this.#apply({ kind: "delete-panel", panel: this.#content, deferred: false })
      const page = this.#page
      const path = page === "classes" ? "resource/ui/charinfoloadoutsubpanel.res" : page === "loadout" ? "resource/ui/classloadoutpanel.res" : "resource/ui/econ/backpackpanel.res"
      const document = this.#request.resources.panelDocument(path).roots[0]!
      const authored = document.children[0]!
      const root = this.#create(1, "EditablePanel", "LocalEquipment", { xpos: "0", ypos: "0", wide: "f0", tall: "f0", paintbackground: "1", bgcolor_override: scalar(authored, "bgcolor_override")! })
      this.#content = root
      this.#create(root, "CExLabel", "EquipmentTitle", { xpos: "c-280", ypos: "15", wide: "500", tall: "30", font: "HudFontMediumBold",
        labelText: page === "classes" ? "#SelectClassLoadout" : page === "backpack" ? "#BackpackTitle" : tf2ClassPresentation(this.#class).displayName })
      this.#button(root, "BackButton", "#TF_BackCarat", "back", "c-288", "r40")
      if (page !== "backpack") this.#button(root, "BackpackButton", "#BackpackTitle", "backpack", "c175", "r40", "115")
      if (page === "classes") {
        for (const identity of [1, 3, 7, 4, 6, 9, 5, 2, 8] as const) {
          const source = block(document, identity === 6 ? "heavyweapons" : tf2ClassPresentation(identity).name)
          const button = this.#button(root, `Class${identity}`, "", `class ${identity}`, scalar(source, "xpos")!, scalar(source, "ypos")!, scalar(source, "wide")!, scalar(source, "tall")!)
          const image = this.#create(button, "ImagePanel", "ClassImage", { xpos: "0", ypos: "0", wide: scalar(source, "wide")!, tall: scalar(source, "tall")!, image: scalar(source, "activeimage")!, scaleImage: "1" })
          this.#apply({ kind: "set-panel-state", panel: image, mouseInput: false, keyboardInput: false })
        }
        return
      }
      if (page === "loadout") {
        // CClassLoadoutPanel's normal and Spy model-panel ordering.
        const slots = this.#class === 8 ? [1, 2, 6, 4, 7, 8, 10, 9] : [0, 1, 2, ...(this.#class === 9 ? [5] : []), 7, 8, 10, 9]
        const equipped = this.#state!.classes[this.#class - 1]!.items
        slots.forEach((slot, index) => {
          const item = equipped.find(item => item.slot === slot)
          const supported = this.#state!.inventory.find(value => value.item.definitionIndex === item?.definitionIndex)
          const right = slot >= 7
          const row = right ? [7, 8, 10, 9].indexOf(slot) : index
          this.#item(root, supported, `slot ${slot}`, `c${right ? scalar(authored, "item_xpos_offcenter_b")! : scalar(authored, "item_xpos_offcenter_a")!}`, String(Number(scalar(authored, "item_ypos")) + row * Number(scalar(authored, "item_ydelta"))), block(authored, "modelpanels_kv"), true)
        })
        const model = block(document, "classmodelpanel")
        const surface = this.#create(root, "CTFPlayerModelPanel", "EquipmentPlayer", Object.fromEntries(["xpos", "ypos", "wide", "tall"].map(key => [key, scalar(model, key)!])))
        const bounds = this.#runtime.snapshot().panels.find(panel => panel.id === surface)!.bounds
        if (this.#request.modelSurface) this.#releaseSurface = attachEquipmentSurface(this.#runtime, surface, this.#request.modelSurface, this.#viewport, bounds)
        const settings = block(model, "model")
        const vector = (prefix: string) => [Number(scalar(settings, `${prefix}_x`)), Number(scalar(settings, `${prefix}_y`)), Number(scalar(settings, `${prefix}_z`))] as const
        this.#request.onPreview({ class: this.#class, equippedItems: equipped, bounds, fov: Number(scalar(model, "fov")), origin: vector("origin"), angles: vector("angles") })
        return
      }
      const inventory = this.#pageItems()
      const template = block(authored, "modelpanels_kv")
      const width = Number(scalar(template, "wide")), height = Number(scalar(template, "tall"))
      for (let cell = 0; cell < 50; cell++) {
        const item = inventory[this.#pageNumber * 50 + cell]
        this.#item(root, item, item ? `item ${item.item.definitionIndex}` : "", `c${Number(scalar(authored, "item_backpack_offcenter_x")) + (cell % 10) * (width + Number(scalar(authored, "item_backpack_xdelta")))}`,
          String(Number(scalar(authored, "item_ypos")) + Math.floor(cell / 10) * (height + Number(scalar(authored, "item_backpack_ydelta")))), template, false)
      }
      if (page === "slot") this.#button(root, "UnequipButton", "UNEQUIP", "unequip", "c-50", "r40", "120")
      const pages = Math.ceil(inventory.length / 50)
      if (pages > 1) {
        this.#button(root, "PrevPage", "<", "prev", "c-288", "288", "25", "20")
        this.#button(root, "NextPage", ">", "next", "c260", "288", "25", "20")
      }
    })
  }

  #item(parent: VguiPanelId, item: Tf2SupportedItem | undefined, command: string, x: string, y: string, template: Tf2UiResourceNode, name: boolean): void {
    const selected = item?.item.definitionIndex === this.#selected
    const button = this.#button(parent, `Item${command.replaceAll(" ", "-") || `${x}-${y}`}`, "", command, x, y, scalar(template, "wide")!, scalar(template, "tall")!, {
      border: selected ? "BackpackItemSelectedBorder" : `BackpackItemBorder${QUALITY_SUFFIX[item?.item.quality ?? 0]}`,
      tooltiptext: "",
    })
    if (!item) { this.#apply({ kind: "set-panel-state", panel: button, enabled: command !== "" }); return }
    this.#itemPanels.set(button, item)
    this.#apply({ kind: "mutate-control", panel: button, mutation: { accessibleName: item.displayName, description: item.description.map(line => line.text).join("\n") } })
    const width = Number(scalar(template, "model_wide")), height = Number(scalar(template, "model_tall"))
    const image = this.#create(button, "ImagePanel", "ItemIcon", { xpos: scalar(template, "model_xpos") ?? String((Number(scalar(template, "wide")) - width) / 2), ypos: scalar(template, "model_ypos")!, wide: String(width), tall: String(height), image: `../${item.image}`, scaleImage: "1" })
    this.#apply({ kind: "set-panel-state", panel: image, mouseInput: false, keyboardInput: false })
    if (name) {
      const text = item.displayName
      const available = Number(scalar(template, "wide")) * this.#viewport.height / 480
      const large = this.#request.resources.clientScheme.fonts.find(font => font.name === "ItemFontNameLarge")
      const font = (large?.measure?.(text, null).width ?? Infinity) <= available ? "ItemFontNameLarge" : "ItemFontNameSmall"
      const label = this.#create(button, "CExLabel", "ItemName", { xpos: "0", ypos: scalar(template, "text_ypos")!, wide: scalar(template, "wide")!, tall: "16", textAlignment: "center", font,
        fgcolor: `QualityColor${QUALITY_COLOR[item.item.quality]}`, labelText: item.displayName, zpos: "2" })
      this.#apply({ kind: "set-panel-state", panel: label, mouseInput: false, keyboardInput: false })
    } else if (this.#state?.classes.some(value => value.items.some(equipped => equipped.itemId === item.item.itemId))) {
      const authored = block(block(this.#request.resources.panelDocument("resource/ui/econ/itemmodelpanel.res").roots[0]!, "MainContentsContainer"), "equippedlabel")
      const props = Object.fromEntries(["font", "wide", "tall", "labelText", "textAlignment", "fgcolor", "bgcolor_override", "PaintBackgroundType", "zpos"].map(key => [key, scalar(authored, key)!]))
      props.xpos = String(Number(scalar(template, "wide")) - Number(props.wide) - Number(scalar(template, "inset_eq_x")))
      props.ypos = String(Number(scalar(template, "tall")) - Number(props.tall) - Number(scalar(template, "inset_eq_y")))
      const label = this.#create(button, "CExLabel", "EquippedLabel", props)
      this.#apply({ kind: "set-panel-state", panel: label, mouseInput: false, keyboardInput: false })
    }
    if (item.item.attributes.some(attribute => attribute.definition === 134)) {
      const authored = block(block(this.#request.resources.panelDocument("resource/ui/econ/itemmodelpanel.res").roots[0]!, "MainContentsContainer"), "is_unusual_icon")
      const wide = Number(scalar(authored, "wide"))
      const icon = this.#create(button, "ImagePanel", "UnusualIcon", { xpos: String(Number(scalar(template, "wide")) - 1 - wide), ypos: "1", wide: String(wide),
        tall: scalar(authored, "tall")!, zpos: scalar(authored, "zpos")!, image: "viewmode_unusual", scaleImage: "1" })
      this.#apply({ kind: "set-panel-state", panel: icon, mouseInput: false, keyboardInput: false })
    }
  }

  #findItem(target: EventTarget | null): { panel: VguiPanelId; item: Tf2SupportedItem } | undefined {
    let element = target as HTMLElement | null
    while (element && element !== this.#request.root) {
      const panel = Number(element.dataset?.vguiPanel)
      const item = this.#itemPanels.get(panel)
      if (item) return { panel, item }
      element = element.parentElement
    }
    return undefined
  }
  readonly #enterItem = (event: Event): void => {
    if (!this.#visible) return
    const item = this.#findItem(event.target)
    if (item?.panel === this.#hover?.panel) return
    this.#clearTooltip()
    if (item) this.#hover = { ...item, since: this.#request.clock.nowSeconds() }
  }
  readonly #leaveItem = (event: Event): void => {
    const next = this.#findItem((event as MouseEvent).relatedTarget)
    if (next?.panel !== this.#hover?.panel) this.#clearTooltip()
  }
  #clearTooltip(): void {
    this.#hover = undefined
    if (this.#tooltip !== undefined) { this.#apply({ kind: "delete-panel", panel: this.#tooltip, deferred: false }); this.#tooltip = undefined }
  }
  #showTooltip(): void {
    const hover = this.#hover
    if (!hover || this.#tooltip !== undefined || this.#content === undefined) return
    const itemPanel = this.#runtime.snapshot().panels.find(panel => panel.id === hover.panel)
    if (!itemPanel) return
    const scale = this.#viewport.height / 480
    const authored = block(this.#request.resources.panelDocument(this.#page === "loadout" ? "resource/ui/classloadoutpanel.res" : "resource/ui/econ/backpackpanel.res").roots[0]!, "mouseoveritempanel")
    const contents = this.#request.resources.panelDocument("resource/ui/econ/itemmodelpanel.res").roots[0]!
    const configuration = block(contents, "mouseoveritempanel")
    const labels = block(contents, "MainContentsContainer")
    const width = Number(scalar(authored, "wide")), textWidth = Number(scalar(configuration, "text_wide")), textX = Number(scalar(configuration, "text_xpos"))
    const nameFont = scalar(block(labels, "namelabel"), "font")!
    const attributeFont = scalar(block(authored, "attriblabel"), "font")!
    const measure = (fontName: string, text: string): number => {
      const font = this.#request.resources.clientScheme.fonts.find(font => font.name === fontName)
      if (!font) throw new Error(`Missing authored equipment font ${fontName}`)
      const metrics = font.metricsForViewport?.(this.#viewport.height) ?? font
      const measured = (metrics.measure ?? font.measure)?.(text, textWidth * scale)
      if (!measured) throw new Error(`Equipment font measurement is unavailable: ${fontName}`)
      return measured.height / scale
    }
    const lines = [{ text: hover.item.displayName, color: `QualityColor${QUALITY_COLOR[hover.item.item.quality]}`, font: nameFont },
      ...hover.item.description.map(line => ({ ...line, font: attributeFont }))]
    const heights = lines.map(line => measure(line.font, line.text))
    const top = Number(scalar(configuration, "text_ypos")), height = top + heights.reduce((sum, value) => sum + value, 0) + Number(scalar(authored, "padding_height"))
    const item = { x: itemPanel.bounds.x / scale, y: itemPanel.bounds.y / scale, width: itemPanel.bounds.width / scale, height: itemPanel.bounds.height / scale }
    const candidates = [[item.x + item.width / 2 - width / 2, item.y + item.height + 4],
      [item.x - width - 4, item.y - height / 2], [item.x + item.width + 4, item.y - height / 2],
      [item.x - width + 18, item.y - 7], [item.x + item.width - 20, item.y - 7], [item.x + item.width / 2 - width / 2, item.y - height - 4]]
    let x = 0, y = 0
    for (const candidate of candidates) {
      x = candidate[0]!; y = candidate[1]!
      let valid = true
      if (x < 0) x = 0
      else if (x + width > this.#viewport.width / scale) {
        const shifted = this.#viewport.width / scale - width
        if (shifted >= 0) x = shifted; else valid = false
      }
      if (y < 0) y = 0
      else if (y + height + 32 > 480) {
        const shifted = item.y - height - 4
        if (shifted >= 0) y = shifted; else valid = false
      }
      if (valid && !(x < item.x + item.width && x + width > item.x && y < item.y + item.height && y + height > item.y)) break
    }
    this.#runtime.deferPresentation(() => {
      const panel = this.#create(this.#content!, "EditablePanel", "ItemTooltip", { xpos: String(x), ypos: String(y), wide: String(width), tall: String(height),
        zpos: scalar(authored, "zpos")!, border: `BackpackItemMouseOverBorder${QUALITY_SUFFIX[hover.item.item.quality]}`, paintborder: "1", paintbackground: "1", PaintBackgroundType: "2" })
      this.#tooltip = panel
      this.#apply({ kind: "set-panel-state", panel, mouseInput: false, keyboardInput: false })
      let at = top
      lines.forEach((line, index) => {
        const label = this.#create(panel, "CExLabel", index === 0 ? "ItemTooltipName" : `ItemDescription${index}`, { xpos: String(textX), ypos: String(at), wide: String(textWidth), tall: String(heights[index]),
          font: line.font, labelText: line.text, fgcolor: line.color, textAlignment: "center", centerwrap: "1", wrap: "1" })
        this.#apply({ kind: "set-panel-state", panel: label, mouseInput: false, keyboardInput: false })
        at += heights[index]!
      })
    })
  }
  #pageItems() {
    const inventory = this.#state!.inventory
    return this.#page === "slot" ? inventory.filter(item => item.classSlots.some(slot => slot.class === this.#class && slot.slot === this.#slot)) : inventory
  }
  #command(command: string): void {
    if (!this.#visible || this.#busy) return
    if (command === "back") {
      if (this.#page === "classes") { this.hide(); return }
      this.#page = this.#page === "slot" ? "loadout" : "classes"
    } else if (command === "backpack") { this.#page = "backpack"; this.#pageNumber = 0 }
    else if (command.startsWith("class ")) { this.#class = Number(command.slice(6)) as Tf2Class; this.#page = "loadout" }
    else if (command.startsWith("slot ")) { this.#slot = Number(command.slice(5)); this.#page = "slot"; this.#pageNumber = 0 }
    else if (command === "prev") this.#pageNumber = Math.max(0, this.#pageNumber - 1)
    else if (command === "next") this.#pageNumber = Math.min(Math.max(0, Math.ceil(this.#pageItems().length / 50) - 1), this.#pageNumber + 1)
    else if (command === "unequip" || command.startsWith("item ")) {
      const definition = command === "unequip" ? null : Number(command.slice(5))
      if (this.#page === "backpack" && definition !== null) {
        const item = this.#state!.inventory.find(item => item.item.definitionIndex === definition)!
        const slot = item.classSlots.find(slot => slot.class === this.#class) ?? item.classSlots[0]!
        this.#class = slot.class; this.#slot = slot.slot; this.#selected = definition; this.#page = "slot"; this.#pageNumber = 0
      } else {
        this.#busy = true
        void this.#request.onEquip(this.#class, this.#slot, definition).then(state => { this.#state = state; this.#page = "loadout"; this.#selected = null })
          .finally(() => { this.#busy = false; this.#render() })
        return
      }
    } else return
    this.#render()
  }
  handleKey(event: Pick<KeyboardEvent, "code" | "preventDefault" | "stopImmediatePropagation">): boolean {
    if (!this.#visible || event.code !== "Escape") return false
    this.#command("back"); event.preventDefault(); event.stopImmediatePropagation(); return true
  }
  frame(timeSeconds: number): void {
    if (!this.#visible) return
    this.#apply({ kind: "frame", timeSeconds })
    if (this.#hover && timeSeconds >= this.#hover.since + 0.1) this.#showTooltip()
  }
  setViewport(viewport: VguiViewport): void { this.#viewport = viewport; this.#apply({ kind: "set-viewport", viewport }); this.#render() }
  destroy(): void {
    this.#releaseSurface?.(); this.#request.onPreview(null)
    this.#request.root.removeEventListener("pointerover", this.#enterItem); this.#request.root.removeEventListener("focusin", this.#enterItem)
    this.#request.root.removeEventListener("pointerout", this.#leaveItem); this.#request.root.removeEventListener("focusout", this.#leaveItem)
    this.#itemPanels.clear(); this.#hover = undefined; this.#apply({ kind: "destroy" })
  }
}
