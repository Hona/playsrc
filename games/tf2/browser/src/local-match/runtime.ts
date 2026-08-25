import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiRequest,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiViewport,
} from "@playsrc/vgui"
import type { BotDifficulty, BotQuotaMode } from "../codec"
import type { Tf2VguiResources } from "../ui-integration"
import {
  TF2_BOT_DIFFICULTIES,
  TF2_BOT_QUOTA_MODES,
  TF2_OFFLINE_PRACTICE_STORAGE_KEY,
  createTf2LocalMatchMaps,
  createTf2OfflinePracticeCatalog,
  tf2LocalMatchLaunch,
  type Tf2LocalMatchLaunch,
  type Tf2LocalMatchMap,
  type Tf2LocalMatchSettings,
  type Tf2OfflinePracticeCatalog,
} from "./model"

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 512, maxHierarchyDepth: 32, maxChildrenPerPanel: 128, maxResourceNodes: 4096,
  maxResourceDepth: 32, maxPropertiesPerPanel: 256, maxStringCodeUnits: 4095,
  maxTextCodeUnits: 65535, maxDialogVariables: 128, maxLocalizationTokens: 4096,
  maxSchemeColors: 1024, maxSchemeSettings: 2048, maxSchemeBorders: 512,
  maxSchemeImages: 2048, maxAnimationScripts: 16, maxAnimationSequences: 1024,
  maxAnimationCommands: 8192, maxActiveAnimations: 256, maxDelayedCommands: 256,
  maxQueuedMessages: 512, maxDiagnostics: 512, maxDomNodes: 2048, maxListeners: 64,
})

const PRACTICE_MODES = Object.freeze([
  { token: "#Gametype_CP", description: "#TF_GameModeDesc_CP", image: "illustrations/gamemode_cp", prefix: "cp_" },
  { token: "#Gametype_Koth", description: "#TF_GameModeDesc_Koth", image: "illustrations/gamemode_koth", prefix: "koth_" },
  { token: "#Gametype_Escort", description: "#TF_GameModeDesc_Escort", image: "illustrations/gamemode_payload", prefix: "pl_" },
] as const)

export type Tf2LocalMatchPresentation = Readonly<{
  show(entry: "training" | "create-server"): void
  hide(): void
  handleKey(event: Pick<KeyboardEvent, "code" | "preventDefault" | "stopImmediatePropagation">): boolean
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  snapshot(): Readonly<{
    visible: boolean
    entry: "training" | "create-server" | null
    page: "training-mode" | "practice-mode" | "practice-map" | "server" | "game"
    settings: Tf2LocalMatchSettings
    maps: readonly Tf2LocalMatchMap[]
    vgui: VguiRuntimeSnapshot
  }>
  destroy(): void
}>

export type Tf2LocalMatchPresentationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  configuredMaps: readonly string[]
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  storage?: Pick<Storage, "getItem" | "setItem">
  onVisibility(visible: boolean): void
  onLaunch(launch: Tf2LocalMatchLaunch): void
}>

const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null

const shallow = (node: VguiResourceNode): VguiResourceNode => Object.freeze({
  ...node,
  children: Object.freeze(node.children.filter((child) => child.value !== null || child.name.toLowerCase() === "controlname")),
})

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`TF2 local match ${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function panel(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((value) => value.name.toLowerCase() === name.toLowerCase()
    && (parent === undefined || value.parent === parent))?.id ?? null
}

function derived(source: VguiResourceDocument, suffix: string, nodes: readonly VguiResourceNode[]): VguiResourceDocument {
  return Object.freeze({
    logicalIdentity: `${source.logicalIdentity}/${suffix}`.toLowerCase(),
    revision: source.revision,
    root: Object.freeze({ ...source.root, children: Object.freeze(nodes) }),
  })
}

function children(
  runtime: VguiRuntime,
  owner: VguiPanelId,
  source: VguiResourceDocument,
  blocks: readonly VguiResourceNode[],
  conditions: readonly string[],
): void {
  const selected = blocks.filter((block) => block.value === null && scalar(block, "ControlName") !== null)
  if (selected.length === 0) return
  const names = new Set<string>()
  const distinct = selected.filter((block) => {
    const name = (scalar(block, "fieldName") ?? block.name).toLowerCase()
    if (names.has(name)) return false
    names.add(name)
    return true
  })
  apply(runtime, {
    kind: "replace-resource",
    parent: owner,
    document: derived(source, `level-${owner}`, distinct.map(shallow)),
    selection: { activeConditions: conditions, resolutionSuffixes: ["_hidef"] },
  })
  for (const block of distinct) {
    const child = panel(runtime, scalar(block, "fieldName") ?? block.name, owner)
    if (child !== null) children(runtime, child, source, block.children, conditions)
  }
}

function control(runtime: VguiRuntime, name: string): VguiPanelId {
  const identity = panel(runtime, name)
  if (identity === null) throw new Error(`TF2 local match authored control is unavailable: ${name}`)
  return identity
}

class Presentation implements Tf2LocalMatchPresentation {
  readonly #request: Tf2LocalMatchPresentationRequest
  readonly #practice: Tf2OfflinePracticeCatalog
  readonly #maps: readonly Tf2LocalMatchMap[]
  readonly #trainingMount: HTMLElement
  readonly #serverMount: HTMLElement
  readonly #training: VguiRuntime
  readonly #server: VguiRuntime
  readonly #localization: ReadonlyMap<string, string>
  #visible = false
  #entry: "training" | "create-server" | null = null
  #page: "training-mode" | "practice-mode" | "practice-map" | "server" | "game" = "training-mode"
  #practiceMode = 0
  #practiceMap = 0
  #trainingDialog = 0
  #trainingContainer = 0
  #serverDialog = 0
  #serverSheet = 0
  #serverPage = 0
  #gamePage = 0
  #settings: Tf2LocalMatchSettings
  #practiceSettings: Tf2LocalMatchSettings
  #serverSettings: Tf2LocalMatchSettings
  #destroyed = false

  constructor(request: Tf2LocalMatchPresentationRequest) {
    this.#request = request
    this.#practice = createTf2OfflinePracticeCatalog(
      request.resources.panelDocument("resource/offline_practice.res"),
      request.configuredMaps,
    )
    this.#maps = createTf2LocalMatchMaps(request.configuredMaps, this.#practice)
    if (this.#maps.length === 0) throw new Error("TF2 local match configured maps are unavailable")
    this.#localization = new Map(request.resources.localization.tokens.map((token) => [token.name.toLowerCase(), token.value]))
    this.#settings = Object.freeze({
      mapIdentity: this.#practice.maps[0]?.identity ?? this.#maps[0]!.identity,
      difficulty: this.#practice.defaults.difficulty,
      playerCount: this.#practice.defaults.suggestedPlayers,
      quotaMode: "normal",
    })
    this.#restore()
    this.#practiceSettings = this.#settings
    this.#serverSettings = Object.freeze({
      mapIdentity: this.#maps[0]!.identity,
      difficulty: 1,
      playerCount: 0,
      quotaMode: "normal",
    })
    this.#trainingMount = request.root.ownerDocument.createElement("div")
    this.#serverMount = request.root.ownerDocument.createElement("div")
    for (const [mount, identity] of [[this.#trainingMount, "training"], [this.#serverMount, "create-server"]] as const) {
      mount.dataset.tf2LocalMatchMount = identity
      mount.style.position = "absolute"
      mount.style.inset = "0"
      mount.style.width = "100%"
      mount.style.height = "100%"
      mount.style.display = "none"
      request.root.append(mount)
    }
    this.#training = this.#initialize("training", this.#trainingMount)
    this.#server = this.#initialize("create-server", this.#serverMount)
    this.#buildTraining()
    this.#buildServer()
  }

  #initialize(identity: "training" | "create-server", root: HTMLElement): VguiRuntime {
    const request = this.#request
    const initialized = initializeVguiRuntime({
      runtimeIdentity: `tf2-local-match-${identity}`,
      root,
      rootControl: { control: "EditablePanel", name: identity === "training" ? "TrainingViewport" : "CreateServerViewport" },
      viewport: request.viewport,
      limits: LIMITS,
      clock: request.clock,
      random: request.random,
      scheme: identity === "training" ? request.resources.clientScheme : request.resources.sourceScheme,
      localization: request.resources.localization,
      animationScripts: request.resources.animations,
      customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion,
      onRequest: (value) => this.#vguiRequest(identity, value),
    })
    if (!initialized.ok) throw new Error(`TF2 local match ${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    return initialized.runtime
  }

  #buildTraining(): void {
    const runtime = this.#training
    const conditions = this.#request.resources.activeConditions
    runtime.deferPresentation(() => {
      const source = this.#request.resources.document("resource/ui/training/main.res")
      const owner = source.root.children.find((block) => block.name === "TrainingDialog")
      const container = source.root.children.find((block) => block.name === "Container")
      if (!owner || !container) throw new Error("TF2 authored training dialog is malformed")
      this.#trainingDialog = apply(runtime, { kind: "create-panel", parent: 1, control: "CTrainingDialog", name: "TrainingDialog" })!
      apply(runtime, { kind: "set-panel-state", panel: this.#trainingDialog, proportional: true })
      apply(runtime, { kind: "replace-resource", parent: 1, document: derived(source, "dialog", [shallow(owner)]), selection: { activeConditions: conditions, resolutionSuffixes: ["_hidef"] } })
      children(runtime, this.#trainingDialog, source, [container], conditions)
      this.#trainingContainer = control(runtime, "Container")
      const modeOwner = control(runtime, "ModeSelectionPanel")
      const modeSource = this.#request.resources.document("resource/ui/training/modeselection/modeselection.res")
      children(runtime, modeOwner, modeSource, modeSource.root.children, conditions)
      const modePanel = this.#request.resources.document("resource/ui/training/modeselection/modepanel.res")
      for (const name of ["BasicTrainingPanel", "OfflinePracticePanel"] as const) {
        const selected = control(runtime, name)
        children(runtime, selected, modePanel, modePanel.root.children, conditions)
        const block = modeSource.root.children.find((value) => value.name === name)!
        for (const variable of ["modename", "description"] as const) {
          apply(runtime, { kind: "set-dialog-variable", panel: selected, name: variable, value: this.#localize(scalar(block, variable) ?? "") })
        }
        const image = panel(runtime, "Image", panel(runtime, "ModeInfoContainer", selected)!)
        if (image !== null) apply(runtime, { kind: "mutate-control", panel: image, mutation: { image: scalar(block, "image") ?? "" } })
        const button = panel(runtime, "StartButton", selected)
        if (button !== null) {
          apply(runtime, { kind: "mutate-control", panel: button, mutation: { command: scalar(block, "startcommand") } })
          apply(runtime, { kind: "set-panel-state", panel: button, enabled: name === "OfflinePracticePanel" })
        }
      }
      const practiceModes = this.#request.resources.document("resource/ui/training/offlinepractice/practicemodeselection.res")
      children(runtime, control(runtime, "OfflinePractice_ModeSelectionPanel"), practiceModes, practiceModes.root.children, conditions)
      const practiceMaps = this.#request.resources.document("resource/ui/training/offlinepractice/mapselection.res")
      children(runtime, control(runtime, "OfflinePractice_MapSelectionPanel"), practiceMaps, practiceMaps.root.children, conditions)
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "DifficultyComboBox"), mutation: {
        editable: false,
        items: TF2_BOT_DIFFICULTIES.map((text, id) => ({ id, text, enabled: true })),
        activeIndex: this.#settings.difficulty,
      } })
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "NumPlayersTextEntry"), mutation: { text: String(this.#settings.playerCount) } })
      apply(runtime, { kind: "set-panel-state", panel: 1, visible: false })
    })
  }

  #create(runtime: VguiRuntime, parent: VguiPanelId, controlName: string, name: string,
    bounds: Readonly<{ x: number; y: number; width: number; height: number }>,
    properties: readonly Readonly<{ name: string; value: string }>[] = []): VguiPanelId {
    const identity = apply(runtime, { kind: "create-panel", parent, control: controlName, name, properties })!
    apply(runtime, { kind: "set-bounds", panel: identity, bounds })
    return identity
  }

  #buildServer(): void {
    const runtime = this.#server
    runtime.deferPresentation(() => {
      const viewport = this.#request.viewport
      this.#serverDialog = this.#create(runtime, 1, "Frame", "CreateMultiplayerGameDialog", {
        x: Math.max(0, Math.trunc((viewport.width - 348) / 2)),
        y: Math.max(0, Math.trunc((viewport.height - 460) / 2)),
        width: 348,
        height: 460,
      }, [
        { name: "title", value: "#GameUI_CreateServer" },
        { name: "moveable", value: "1" },
        { name: "sizeable", value: "0" },
      ])
      this.#serverSheet = this.#create(runtime, this.#serverDialog, "PropertySheet", "CreateServerSheet", { x: 8, y: 31, width: 332, height: 392 })
      this.#serverPage = this.#create(runtime, this.#serverSheet, "CCreateMultiplayerGameServerPage", "ServerPage", { x: 0, y: 28, width: 332, height: 364 })
      const server = this.#request.resources.document("resource/createmultiplayergameserverpage.res")
      children(runtime, this.#serverPage, server, server.root.children.filter((block) => block.name !== "ServerPage"), this.#request.resources.activeConditions)
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "MapList"), mutation: {
        editable: false,
        items: this.#maps.map((map, id) => ({ id, text: map.identity, enabled: true })),
        activeIndex: Math.max(0, this.#maps.findIndex((map) => map.identity === this.#settings.mapIdentity)),
      } })
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "VisibilityType"), mutation: {
        editable: false,
        items: [{ id: 0, text: "Local", enabled: true }],
        activeIndex: 0,
      } })
      this.#gamePage = this.#create(runtime, this.#serverSheet, "PropertyPage", "GameplayPage", { x: 0, y: 28, width: 332, height: 364 })
      const gameplay = this.#request.resources.document("resource/createmultiplayergamegameplaypage.res")
      children(runtime, this.#gamePage, gameplay, gameplay.root.children, this.#request.resources.activeConditions)
      const options = control(runtime, "GameOptions")
      const definitions = this.#request.resources.document("resource/ui/training/offlinepractice/mapselection.res")
      const authored = definitions.root.children
        .filter((block) => ["DifficultyLabel", "DifficultyComboBox", "PlayersLabel", "NumPlayersTextEntry"].includes(block.name))
        .map((block) => Object.freeze({
          ...block,
          children: Object.freeze(block.children.map((property) => property.name.toLowerCase() === "font"
            ? Object.freeze({ ...property, value: "Default" })
            : property)),
        }))
      children(runtime, options, definitions, authored, this.#request.resources.activeConditions)
      for (const [name, bounds] of [
        ["DifficultyLabel", { x: 8, y: 12, width: 130, height: 24 }],
        ["DifficultyComboBox", { x: 145, y: 12, width: 150, height: 24 }],
        ["PlayersLabel", { x: 8, y: 47, width: 130, height: 24 }],
        ["NumPlayersTextEntry", { x: 145, y: 47, width: 50, height: 24 }],
      ] as const) apply(runtime, { kind: "set-bounds", panel: panel(runtime, name, options)!, bounds })
      const difficulty = panel(runtime, "DifficultyComboBox", options)!
      const count = panel(runtime, "NumPlayersTextEntry", options)!
      apply(runtime, { kind: "mutate-control", panel: difficulty, mutation: {
        editable: false,
        items: TF2_BOT_DIFFICULTIES.map((text, id) => ({ id, text, enabled: true })),
        activeIndex: this.#settings.difficulty,
      } })
      apply(runtime, { kind: "mutate-control", panel: count, mutation: { text: String(this.#settings.playerCount) } })
      this.#create(runtime, options, "Label", "TeamFillLabel", { x: 8, y: 82, width: 130, height: 24 }, [{ name: "labelText", value: "Team fill:" }])
      const fill = this.#create(runtime, options, "ComboBox", "TeamFillComboBox", { x: 145, y: 82, width: 150, height: 24 }, [{ name: "editable", value: "0" }])
      apply(runtime, { kind: "mutate-control", panel: fill, mutation: {
        editable: false,
        items: TF2_BOT_QUOTA_MODES.map((value, id) => ({ id, text: value, enabled: true })),
        activeIndex: TF2_BOT_QUOTA_MODES.indexOf(this.#settings.quotaMode),
      } })
      apply(runtime, { kind: "mutate-control", panel: this.#serverSheet, mutation: {
        items: [{ id: this.#serverPage, text: "#GameUI_Server", enabled: true }, { id: this.#gamePage, text: "#GameUI_Game", enabled: true }],
        activeIndex: 0,
      } })
      this.#create(runtime, this.#serverDialog, "Button", "StartButton", { x: 178, y: 428, width: 76, height: 24 }, [
        { name: "labelText", value: "#GameUI_Start" },
        { name: "command", value: "Start" },
        { name: "actionsignallevel", value: "1" },
      ])
      this.#create(runtime, this.#serverDialog, "Button", "CancelButton", { x: 260, y: 428, width: 76, height: 24 }, [
        { name: "labelText", value: "#PropertyDialog_Cancel" },
        { name: "command", value: "Cancel" },
        { name: "actionsignallevel", value: "1" },
      ])
      apply(runtime, { kind: "set-panel-state", panel: this.#gamePage, visible: false })
      apply(runtime, { kind: "set-panel-state", panel: 1, visible: false })
    })
  }

  #restore(): void {
    const value = this.#request.storage?.getItem(TF2_OFFLINE_PRACTICE_STORAGE_KEY)
    if (!value) return
    try {
      const parsed = JSON.parse(value) as Record<string, unknown>
      const difficulty = parsed.tf_bot_difficulty
      const count = parsed.tf_bot_quota
      const map = parsed.map
      if (!Number.isSafeInteger(difficulty) || Number(difficulty) < 0 || Number(difficulty) > 3
        || !Number.isSafeInteger(count) || Number(count) < 1 || Number(count) > 31
        || typeof map !== "string" || !this.#maps.some((candidate) => candidate.identity === map)) return
      this.#settings = Object.freeze({
        ...this.#settings,
        difficulty: difficulty as BotDifficulty,
        playerCount: count as number,
        mapIdentity: map,
      })
    } catch {}
  }

  #save(): void {
    this.#request.storage?.setItem(TF2_OFFLINE_PRACTICE_STORAGE_KEY, JSON.stringify({
      tf_bot_quota: this.#settings.playerCount,
      tf_bot_difficulty: this.#settings.difficulty,
      map: this.#settings.mapIdentity,
    }))
  }

  #localize(token: string): string {
    return token.startsWith("#") ? this.#localization.get(token.slice(1).toLowerCase()) ?? token : token
  }

  #visiblePracticeMaps(): readonly Tf2LocalMatchMap[] {
    const mode = PRACTICE_MODES[this.#practiceMode]!
    return this.#practice.maps.filter((map) => map.identity.startsWith(mode.prefix))
  }

  #presentTraining(): void {
    const runtime = this.#training
    runtime.deferPresentation(() => {
      for (const [name, visible] of [
        ["ModeSelectionPanel", this.#page === "training-mode"],
        ["OfflinePractice_ModeSelectionPanel", this.#page === "practice-mode"],
        ["OfflinePractice_MapSelectionPanel", this.#page === "practice-map"],
        ["BasicTraining_ClassSelectionPanel", false],
        ["BasicTraining_ClassDetailsPanel", false],
      ] as const) apply(runtime, { kind: "set-panel-state", panel: control(runtime, name), visible })
      const first = this.#page === "training-mode"
      const back = control(runtime, "BackButton")
      const cancel = control(runtime, "CancelButton")
      const container = runtime.snapshot().panels.find((item) => item.id === this.#trainingContainer)!
      const cancelState = runtime.snapshot().panels.find((item) => item.id === cancel)!
      const backState = runtime.snapshot().panels.find((item) => item.id === back)!
      apply(runtime, { kind: "set-panel-state", panel: back, visible: !first })
      apply(runtime, { kind: "set-bounds", panel: cancel, bounds: {
        ...cancelState.bounds,
        x: first ? Math.trunc((container.bounds.width - cancelState.bounds.width) / 2) : Math.trunc(container.bounds.width / 2) + 5,
      } })
      if (!first) apply(runtime, { kind: "set-bounds", panel: back, bounds: {
        ...backState.bounds,
        x: Math.trunc(container.bounds.width / 2) - backState.bounds.width - 5,
      } })
      if (this.#page === "training-mode") {
        apply(runtime, { kind: "set-dialog-variable", panel: this.#trainingContainer, name: "title", value: this.#localize("#TF_Training_Title") })
        apply(runtime, { kind: "set-dialog-variable", panel: this.#trainingContainer, name: "subtitle", value: "" })
        return
      }
      const mode = PRACTICE_MODES[this.#practiceMode]!
      if (this.#page === "practice-mode") {
        const owner = control(runtime, "OfflinePractice_ModeSelectionPanel")
        apply(runtime, { kind: "set-dialog-variable", panel: this.#trainingContainer, name: "title", value: this.#localize("#TR_PracticeModeSelectTitle") })
        apply(runtime, { kind: "set-dialog-variable", panel: owner, name: "gamemode", value: this.#localize(mode.token) })
        apply(runtime, { kind: "set-dialog-variable", panel: owner, name: "description", value: this.#localize(mode.description) })
        apply(runtime, { kind: "set-dialog-variable", panel: owner, name: "curpage", value: `${this.#practiceMode + 1}/${PRACTICE_MODES.length}` })
        apply(runtime, { kind: "mutate-control", panel: control(runtime, "GameModeImagePanel"), mutation: { image: mode.image } })
        apply(runtime, { kind: "set-panel-state", panel: control(runtime, "SelectCurrentGameModeButton"), enabled: this.#visiblePracticeMaps().length > 0 })
        return
      }
      const maps = this.#visiblePracticeMaps()
      const map = maps[this.#practiceMap]
      if (!map) throw new Error("TF2 offline practice selected map is unavailable")
      this.#settings = Object.freeze({ ...this.#settings, mapIdentity: map.identity })
      const owner = control(runtime, "OfflinePractice_MapSelectionPanel")
      const title = this.#localize("#TR_PracticeMapSelectTitle").replace("%gametype%", this.#localize(mode.token))
      apply(runtime, { kind: "set-dialog-variable", panel: this.#trainingContainer, name: "title", value: title })
      apply(runtime, { kind: "set-dialog-variable", panel: owner, name: "mapname", value: map.displayName })
      apply(runtime, { kind: "set-dialog-variable", panel: owner, name: "curpage", value: `${this.#practiceMap + 1}/${maps.length}` })
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "MapImagePanel"), mutation: { image: `training/screenshots/${map.identity}` } })
      const suggested = this.#localize("#TF_OfflinePractice_NumPlayers")
        .replace("%s1", String(map.minimumPlayers)).replace("%s2", String(map.maximumPlayers))
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "SuggestedPlayerCountLabel"), mutation: { text: suggested } })
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "DifficultyComboBox"), mutation: { activeIndex: this.#settings.difficulty } })
      apply(runtime, { kind: "mutate-control", panel: control(runtime, "NumPlayersTextEntry"), mutation: { text: String(this.#settings.playerCount) } })
    })
  }

  #stage(entry: "training" | "create-server"): void {
    const runtime = entry === "training" ? this.#training : this.#server
    const owner = entry === "training" ? control(runtime, "OfflinePractice_MapSelectionPanel") : control(runtime, "GameOptions")
    const snapshots = new Map(runtime.snapshot().panels.map((item) => [item.id, item]))
    const difficulty = snapshots.get(panel(runtime, "DifficultyComboBox", owner)!)?.state.activeIndex
    const rawCount = snapshots.get(panel(runtime, "NumPlayersTextEntry", owner)!)?.text ?? ""
    const parsed = Number.parseInt(rawCount, 10)
    const playerCount = Number.isSafeInteger(parsed)
      ? Math.max(entry === "training" ? 1 : 0, Math.min(31, parsed))
      : entry === "training" ? 1 : 0
    let quotaMode = this.#settings.quotaMode
    let mapIdentity = this.#settings.mapIdentity
    if (entry === "create-server") {
      const selectedMap = snapshots.get(control(runtime, "MapList"))?.state.activeIndex
      const selectedMode = snapshots.get(control(runtime, "TeamFillComboBox"))?.state.activeIndex
      if (selectedMap !== null && selectedMap !== undefined) mapIdentity = this.#maps[selectedMap]?.identity ?? mapIdentity
      if (selectedMode !== null && selectedMode !== undefined) quotaMode = TF2_BOT_QUOTA_MODES[selectedMode] ?? quotaMode
    }
    this.#settings = Object.freeze({
      mapIdentity,
      difficulty: difficulty !== null && difficulty !== undefined && difficulty >= 0 && difficulty <= 3
        ? difficulty as BotDifficulty : this.#settings.difficulty,
      playerCount,
      quotaMode,
    })
    if (entry === "training") this.#practiceSettings = this.#settings
    else this.#serverSettings = this.#settings
  }

  #vguiRequest(entry: "training" | "create-server", request: VguiRequest): void {
    if (request.kind === "message" && request.source !== null) {
      if (entry === "create-server" && request.source === this.#serverSheet && request.message.name === "PageChanged") {
        const index = this.#server.snapshot().panels.find((value) => value.id === this.#serverSheet)?.state.activeIndex
        this.#setServerPage(index === 1 ? "game" : "server")
      }
      return
    }
    if (request.kind !== "command") return
    if (request.command === "cancel" || request.command === "Cancel" || request.command === "Close") {
      if (entry === "training" && this.#page === "practice-map") { this.#stage(entry); this.#save() }
      this.hide()
      return
    }
    if (entry === "create-server" && request.command === "Start") {
      this.#launch(entry)
      return
    }
    if (entry !== "training") return
    if (request.command === "offlinepracticeselected") {
      this.#practiceMode = 0
      this.#page = "practice-mode"
      this.#presentTraining()
    } else if (request.command === "selectcurrentgamemode" && this.#visiblePracticeMaps().length > 0) {
      this.#practiceMap = 0
      this.#page = "practice-map"
      this.#presentTraining()
    } else if (request.command === "startofflinepractice") this.#launch(entry)
    else if (request.command === "prevpage") {
      if (this.#page === "practice-map") { this.#stage(entry); this.#save(); this.#page = "practice-mode" }
      else if (this.#page === "practice-mode") this.#page = "training-mode"
      else { this.hide(); return }
      this.#presentTraining()
    } else if (request.command === "gonext" || request.command === "goprev") {
      const direction = request.command === "gonext" ? 1 : -1
      if (this.#page === "practice-mode") {
        this.#practiceMode = (this.#practiceMode + direction + PRACTICE_MODES.length) % PRACTICE_MODES.length
      } else if (this.#page === "practice-map") {
        const count = this.#visiblePracticeMaps().length
        this.#practiceMap = (this.#practiceMap + direction + count) % count
      }
      this.#presentTraining()
    }
  }

  #setServerPage(page: "server" | "game"): void {
    this.#page = page
    this.#server.deferPresentation(() => {
      apply(this.#server, { kind: "set-panel-state", panel: this.#serverPage, visible: page === "server" })
      apply(this.#server, { kind: "set-panel-state", panel: this.#gamePage, visible: page === "game" })
    })
  }

  #launch(entry: "training" | "create-server"): void {
    this.#stage(entry)
    const map = this.#maps.find((candidate) => candidate.identity === this.#settings.mapIdentity)
    if (!map) throw new Error("TF2 local match selected map is unavailable")
    const launch = tf2LocalMatchLaunch(entry, this.#settings, map)
    if (entry === "training") this.#save()
    this.hide()
    this.#request.onLaunch(launch)
  }

  show(entry: "training" | "create-server"): void {
    if (this.#destroyed) throw new Error("TF2 local match presentation is destroyed")
    this.#entry = entry
    this.#visible = true
    this.#settings = entry === "training" ? this.#practiceSettings : this.#serverSettings
    this.#trainingMount.style.display = entry === "training" ? "block" : "none"
    this.#serverMount.style.display = entry === "create-server" ? "block" : "none"
    apply(this.#training, { kind: "set-panel-state", panel: 1, visible: entry === "training" })
    apply(this.#server, { kind: "set-panel-state", panel: 1, visible: entry === "create-server" })
    apply(this.#server, { kind: "set-panel-state", panel: this.#serverDialog, visible: entry === "create-server" })
    if (entry === "training") {
      this.#page = "training-mode"
      this.#presentTraining()
    } else {
      this.#page = "server"
      const options = control(this.#server, "GameOptions")
      apply(this.#server, { kind: "mutate-control", panel: control(this.#server, "MapList"), mutation: {
        activeIndex: Math.max(0, this.#maps.findIndex((map) => map.identity === this.#settings.mapIdentity)),
      } })
      apply(this.#server, { kind: "mutate-control", panel: panel(this.#server, "DifficultyComboBox", options)!, mutation: {
        activeIndex: this.#settings.difficulty,
      } })
      apply(this.#server, { kind: "mutate-control", panel: panel(this.#server, "NumPlayersTextEntry", options)!, mutation: {
        text: String(this.#settings.playerCount),
      } })
      apply(this.#server, { kind: "mutate-control", panel: control(this.#server, "TeamFillComboBox"), mutation: {
        activeIndex: TF2_BOT_QUOTA_MODES.indexOf(this.#settings.quotaMode),
      } })
      apply(this.#server, { kind: "mutate-control", panel: this.#serverSheet, mutation: { activeIndex: 0 } })
      this.#setServerPage("server")
    }
    this.#request.onVisibility(true)
  }

  hide(): void {
    if (!this.#visible) return
    this.#visible = false
    this.#entry = null
    this.#trainingMount.style.display = "none"
    this.#serverMount.style.display = "none"
    apply(this.#training, { kind: "set-panel-state", panel: 1, visible: false })
    apply(this.#server, { kind: "set-panel-state", panel: 1, visible: false })
    this.#request.onVisibility(false)
  }

  handleKey(event: Pick<KeyboardEvent, "code" | "preventDefault" | "stopImmediatePropagation">): boolean {
    if (!this.#visible || event.code !== "Escape") return false
    event.preventDefault()
    event.stopImmediatePropagation()
    if (this.#entry === "training" && this.#page === "practice-map") { this.#stage("training"); this.#save() }
    this.hide()
    return true
  }

  frame(timeSeconds: number): void {
    if (!this.#visible || this.#entry === null) return
    apply(this.#entry === "training" ? this.#training : this.#server, { kind: "frame", timeSeconds })
    if (this.#entry === "create-server") {
      const current = this.#server.snapshot().panels.find((item) => item.id === this.#serverSheet)?.state.activeIndex
      const selected = current === 1 ? "game" : "server"
      if (selected !== this.#page) this.#setServerPage(selected)
    }
  }

  setViewport(viewport: VguiViewport): void {
    apply(this.#training, { kind: "set-viewport", viewport })
    apply(this.#server, { kind: "set-viewport", viewport })
    const current = this.#server.snapshot().panels.find((item) => item.id === this.#serverDialog)!
    apply(this.#server, { kind: "set-bounds", panel: this.#serverDialog, bounds: {
      ...current.bounds,
      x: Math.max(0, Math.trunc((viewport.width - current.bounds.width) / 2)),
      y: Math.max(0, Math.trunc((viewport.height - current.bounds.height) / 2)),
    } })
    if (this.#entry === "training") this.#presentTraining()
  }

  snapshot(): ReturnType<Tf2LocalMatchPresentation["snapshot"]> {
    return Object.freeze({
      visible: this.#visible,
      entry: this.#entry,
      page: this.#page,
      settings: this.#settings,
      maps: this.#entry === "training" ? Object.freeze([...this.#visiblePracticeMaps()]) : this.#maps,
      vgui: (this.#entry === "create-server" ? this.#server : this.#training).snapshot(),
    })
  }

  destroy(): void {
    if (this.#destroyed) return
    if (this.#visible) this.hide()
    this.#destroyed = true
    apply(this.#training, { kind: "destroy" })
    apply(this.#server, { kind: "destroy" })
    this.#trainingMount.remove()
    this.#serverMount.remove()
  }
}

export function initializeTf2LocalMatchPresentation(request: Tf2LocalMatchPresentationRequest): Tf2LocalMatchPresentation {
  return Object.freeze(new Presentation(request))
}
