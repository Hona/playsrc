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
import {
  TF2_MAIN_MENU_STATE,
  transitionTf2GameUi,
  type Tf2GameUiEvent,
  type Tf2GameUiRequest,
  type Tf2GameUiState,
  type Tf2GameUiTransition,
  type Tf2MenuButton,
} from "../gameui"
import type { Tf2VguiResources } from "../ui-integration"
import type { Tf2PresentationRandom } from "../ui-integration"
import { tf2GameUiBaseBackground } from "./base-background"

export type Tf2GameUiIntegrationDiagnostic = Readonly<{
  code: "UnsupportedResourceObject" | "MissingCodeControl" | "VguiRejected" | "InactiveCommand"
  subject: string
}>

export type Tf2GameUiIntegration = Readonly<{
  state(): Tf2GameUiState
  snapshot(): VguiRuntimeSnapshot
  diagnostics(): readonly Tf2GameUiIntegrationDiagnostic[]
  dispatch(event: Tf2GameUiEvent): Tf2GameUiTransition
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  destroy(): void
}>

export type Tf2GameUiIntegrationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  presentation: Readonly<{
    random: Tf2PresentationRandom
    activeHoliday: "none" | "summer" | "halloween" | "fullmoon" | "christmas"
    activeWar: string | null
    activeOperation: boolean
    freeTrial: boolean
  }>
  onRequest(request: Tf2GameUiRequest): void
}>

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 2_048,
  maxHierarchyDepth: 64,
  maxChildrenPerPanel: 512,
  maxResourceNodes: 4_096,
  maxResourceDepth: 64,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4_095,
  maxTextCodeUnits: 65_535,
  maxDialogVariables: 512,
  maxLocalizationTokens: 4_096,
  maxSchemeColors: 1_024,
  maxSchemeSettings: 2_048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2_048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1_024,
  maxAnimationCommands: 8_192,
  maxActiveAnimations: 2_048,
  maxDelayedCommands: 2_048,
  maxQueuedMessages: 4_096,
  maxDiagnostics: 4_096,
  maxDomNodes: 8_192,
  maxListeners: 64,
})

const OBJECT_PROPERTIES = new Set(["button", "upbutton", "downbutton", "slider", "scrollbar", "tabskv"])
const MAIN_MENU_PATH = "resource/ui/mainmenuoverride.res"
const DASHBOARD_PATH = "resource/ui/matchmakingdashboard.res"
const DASHBOARD_PLAYLIST_PATH = "resource/ui/matchmakingdashboardplaylist.res"
const PLAYLIST_PATH = "resource/ui/matchmakingplaylist.res"
const PLAYLIST_ENTRY_PATH = "resource/ui/mainmenuplaylistentry.res"

export function tf2CharacterImageVisible(state: Tf2GameUiState, backgroundUsesCharacterImage: boolean): boolean {
  return state.kind === "main-menu" && backgroundUsesCharacterImage
}

export function tf2MainMenuAspectCondition(viewport: VguiViewport): "if_wider" | "if_taller" {
  return viewport.width / viewport.height >= 1.6 ? "if_wider" : "if_taller"
}

const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const cloneNode = (node: VguiResourceNode, children = node.children): VguiResourceNode => Object.freeze({
  name: node.name,
  value: node.value,
  condition: node.condition,
  children: Object.freeze(children),
})

function mustApply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function panelByName(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  const folded = name.toLowerCase()
  return runtime.snapshot().panels.find((panel) => panel.name.toLowerCase() === folded && (parent === undefined || panel.parent === parent))?.id ?? null
}

function syntheticDocument(source: VguiResourceDocument, identity: string, root: VguiResourceNode): VguiResourceDocument {
  return Object.freeze({ logicalIdentity: `${source.logicalIdentity}/${identity}`.toLowerCase(), revision: source.revision, root })
}
function resourceBlock(root: VguiResourceNode, name: string): VguiResourceNode | null {
  if (root.name.toLowerCase() === name.toLowerCase()) return root
  for (const child of root.children) {
    if (child.value !== null) continue
    const found = resourceBlock(child, name)
    if (found) return found
  }
  return null
}

function applyResourceTree(
  runtime: VguiRuntime,
  parent: VguiPanelId,
  source: VguiResourceDocument,
  root: VguiResourceNode,
  diagnostics: Tf2GameUiIntegrationDiagnostic[],
  activeConditions: readonly string[],
  resolutionSuffixes: readonly string[],
): void {
  const existing = new Set(runtime.snapshot().panels.filter((panel) => panel.parent === parent).map((panel) => panel.name.toLowerCase()))
  const controls: VguiResourceNode[] = []
  for (const block of root.children) {
    if (block.value !== null) continue
    const controlName = scalar(block, "ControlName")
    if (!controlName && !existing.has(block.name.toLowerCase())) {
      diagnostics.push(Object.freeze({ code: "MissingCodeControl", subject: `${source.logicalIdentity}:${block.name}` }))
      continue
    }
    const shallow: VguiResourceNode[] = []
    for (const property of block.children) {
      if (property.value !== null || property.name.toLowerCase() === "controlname" || OBJECT_PROPERTIES.has(property.name.toLowerCase())) {
        shallow.push(property)
      } else if (!scalar(property, "ControlName")) {
        if (property.name === "MOTD_TextPanel" || property.name === "Notifications_Control") continue
        diagnostics.push(Object.freeze({ code: "UnsupportedResourceObject", subject: `${source.logicalIdentity}:${block.name}:${property.name}` }))
      }
    }
    controls.push(cloneNode(block, shallow))
  }
  const document = syntheticDocument(source, `level-${parent}`, cloneNode(root, controls))
  try {
    mustApply(runtime, { kind: "replace-resource", parent, document, selection: { activeConditions, resolutionSuffixes } })
  } catch (error) {
    diagnostics.push(Object.freeze({ code: "VguiRejected", subject: error instanceof Error ? error.message : source.logicalIdentity }))
    throw error
  }
  for (const block of root.children) {
    if (block.value !== null) continue
    const child = panelByName(runtime, scalar(block, "fieldName") ?? block.name, parent)
    if (child === null) continue
    const parentControl = runtime.snapshot().panels.find((panel) => panel.id === parent)?.control
    if (scalar(block, "ControlName") === "ImagePanel" && (parentControl === "CExImageButton" || parentControl === "CExButton" || parentControl === "Button")) {
      mustApply(runtime, { kind: "set-panel-state", panel: child, mouseInput: false, keyboardInput: false })
    }
    const nested = block.children.filter((node) => node.value === null && scalar(node, "ControlName") !== null)
    if (nested.length === 0) continue
    applyResourceTree(runtime, child, source, cloneNode(block, nested), diagnostics, activeConditions, resolutionSuffixes)
  }
}

function createCodeControl(runtime: VguiRuntime, parent: VguiPanelId, control: string, name: string): VguiPanelId {
  return panelByName(runtime, name, parent) ?? mustApply(runtime, { kind: "create-panel", parent, control, name })!
}

const PLAYLIST_CONTROLS: Readonly<Record<Tf2MenuButton["identity"], string>> = Object.freeze({
  "special-event": "EventEntry",
  casual: "CasualEntry",
  competitive: "CompetitiveEntry",
  "mann-vs-machine": "MvMEntry",
  "community-servers": "ServerBrowserEntry",
  training: "TrainingEntry",
  "create-server": "CreateServerEntry",
  "find-game": "FindAGameButton",
  quit: "QuitButton",
  resume: "ResumeButton",
  disconnect: "DisconnectButton",
  items: "CharacterSetupButton",
  store: "GeneralStoreButton",
  options: "SettingsButton",
  "advanced-options": "TF2SettingsButton",
  "new-user-forum": "NewUserForumsButton",
  "cancel-loading": "CancelButton",
})

class Integration implements Tf2GameUiIntegration {
  readonly #runtime: VguiRuntime
  readonly #resources: Tf2VguiResources
  readonly #onRequest: (request: Tf2GameUiRequest) => void
  readonly #diagnostics: Tf2GameUiIntegrationDiagnostic[] = []
  readonly #baseVisibility = new Map<VguiPanelId, boolean>()
  #baseBackground: VguiPanelId | null = null
  #baseBackgroundImage: string | null = null
  #viewport: VguiViewport
  #mainMenu: VguiPanelId | null = null
  #mainMenuConditions: string[] = []
  #state: Tf2GameUiState = TF2_MAIN_MENU_STATE
  #playlistActive = false
  #destroyed = false

  #configureBaseBackground(viewport: VguiViewport): void {
    if (this.#baseBackground === null) throw new Error("TF2 GameUI base-background panel is missing")
    const presentation = tf2GameUiBaseBackground(this.#resources.gameUiBackground, this.#state, viewport)
    const variant = presentation.variant
    mustApply(this.#runtime, { kind: "set-bounds", panel: this.#baseBackground, bounds: presentation.bounds })
    if (this.#baseBackgroundImage !== variant.image) {
      mustApply(this.#runtime, { kind: "mutate-control", panel: this.#baseBackground, mutation: { image: variant.image } })
      this.#baseBackgroundImage = variant.image
    }
  }

  #selectCharacter(request: Tf2GameUiIntegrationRequest, parent: VguiPanelId): void {
    const source = this.#resources.panelDocument("scripts/characterbackgrounds.txt")
    const root = source.roots[0]
    if (!root) throw new Error("Configured character background root is missing")
    const rawScalar = (value: typeof root, name: string): string | null => value.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
    const candidates: string[] = []
    for (const character of root.children) {
      const image = rawScalar(character, "image")
      if (!image) continue
      let weight = Number(rawScalar(character, "weight") ?? 1)
      const holiday = (rawScalar(character, "holiday_restriction") ?? "none").toLowerCase()
      const war = rawScalar(character, "war_restriction")
      const operation = Number(rawScalar(character, "operation") ?? 0) !== 0
      if (!Number.isSafeInteger(weight) || weight < 0 || weight > 1_024) throw new Error(`Configured character weight is invalid: ${character.name}`)
      if (request.presentation.activeWar !== null) {
        if (war !== request.presentation.activeWar) weight = 0
      } else if (holiday !== "none") {
        weight = holiday === request.presentation.activeHoliday || (holiday === "halloween_or_fullmoon" && (request.presentation.activeHoliday === "halloween" || request.presentation.activeHoliday === "fullmoon")) ? Math.max(weight, 6) : 0
      } else if (request.presentation.activeOperation && !operation) weight = 0
      else if (["summer", "halloween", "fullmoon", "christmas"].includes(request.presentation.activeHoliday)) weight = 0
      for (let index = 0; index < weight; index += 1) candidates.push(image)
    }
    if (candidates.length === 0) throw new Error("Configured character background selection is empty")
    const selected = candidates[request.presentation.random.nextInteger(0, candidates.length - 1)]!
    const image = panelByName(this.#runtime, "TFCharacterImage", parent)
    if (image === null) throw new Error("Configured character image panel is missing")
    mustApply(this.#runtime, { kind: "mutate-control", panel: image, mutation: { image: selected } })
  }

  #configureScrollers(mainMenu: VguiPanelId): void {
    const source = this.#resources.document(MAIN_MENU_PATH)
    for (const [scrollerName, childName] of [["MOTD_TextScroller", "MOTD_TextPanel"], ["Notifications_Scroller", "Notifications_Control"]] as const) {
      const scroller = panelByName(this.#runtime, scrollerName)
      const block = resourceBlock(source.root, childName)
      if (scroller === null || !block) throw new Error(`Configured scrollable panel ${scrollerName}:${childName} is missing`)
      createCodeControl(this.#runtime, scroller, "EditablePanel", childName)
      applyResourceTree(this.#runtime, scroller, source, cloneNode(source.root, [block]), this.#diagnostics, this.#resources.activeConditions, ["_hidef"])
    }
    void mainMenu
  }

  #applyOwned(logicalPath: string, owner: VguiPanelId, conditions: readonly string[] = []): void {
    const document = this.#resources.document(logicalPath)
    const snapshot = this.#runtime.snapshot().panels.find((panel) => panel.id === owner)!
    const self = document.root.children.find((block) => (scalar(block, "fieldName") ?? block.name).toLowerCase() === snapshot.name.toLowerCase())
    if (self && snapshot.parent !== null) {
      const shallow = cloneNode(self, self.children.filter((property) => property.value !== null || property.name.toLowerCase() === "controlname"))
      applyResourceTree(this.#runtime, snapshot.parent, document, cloneNode(document.root, [shallow]), this.#diagnostics, [...this.#resources.activeConditions, ...conditions], ["_hidef"])
    }
    applyResourceTree(this.#runtime, owner, document, cloneNode(document.root, document.root.children.filter((block) => block !== self)), this.#diagnostics, [...this.#resources.activeConditions, ...conditions], ["_hidef"])
  }

  constructor(request: Tf2GameUiIntegrationRequest) {
    this.#viewport = request.viewport
    this.#resources = request.resources
    this.#onRequest = request.onRequest
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-gameui",
      root: request.root,
      rootControl: { control: "EditablePanel", name: "GameUiRoot" },
      viewport: request.viewport,
      limits: LIMITS,
      clock: request.clock,
      random: request.random,
      scheme: request.resources.clientScheme,
      localization: request.resources.localization,
      animationScripts: request.resources.animations,
      customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion,
      onRequest: (value) => this.#vguiRequest(value),
    })
    if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#runtime.deferPresentation(() => {
      const baseBackground = createCodeControl(this.#runtime, 1, "ImagePanel", "GameUiBaseBackground")
      this.#baseBackground = baseBackground
      mustApply(this.#runtime, { kind: "set-panel-state", panel: baseBackground, mouseInput: false, keyboardInput: false, z: -100 })
      this.#configureBaseBackground(request.viewport)
      const mainMenu = createCodeControl(this.#runtime, 1, "CHudMainMenuOverride", "MainMenuOverride")
      this.#mainMenu = mainMenu
      mustApply(this.#runtime, { kind: "set-panel-state", panel: mainMenu, proportional: true })
      createCodeControl(this.#runtime, mainMenu, "ImagePanel", "TFCharacterImage")
      const dashboard = createCodeControl(this.#runtime, 1, "CTFMatchmakingDashboard", "MMDashboard")
      const playlistContainer = createCodeControl(this.#runtime, 1, "EditablePanel", "ExpandableList")
      const playlist = createCodeControl(this.#runtime, playlistContainer, "CTFPlaylistPanel", "playlist")
      const conditions = [tf2MainMenuAspectCondition(request.viewport)]
      if (request.presentation.activeHoliday !== "none") conditions.push(`if_${request.presentation.activeHoliday}`)
      let characterBackground = true
      if (request.presentation.activeHoliday === "halloween") {
        const selected = request.presentation.random.nextInteger(0, 5)
        conditions.push(`if_halloween_${selected}`)
        characterBackground = selected !== 3 && selected !== 4
      } else if (request.presentation.activeHoliday === "christmas") {
        conditions.push(`if_christmas_${request.presentation.random.nextInteger(0, 1)}`)
      }
      this.#mainMenuConditions = conditions
      this.#applyOwned(MAIN_MENU_PATH, mainMenu, conditions)
      this.#configureScrollers(mainMenu)
      if (characterBackground) this.#selectCharacter(request, mainMenu)
      else {
        const image = panelByName(this.#runtime, "TFCharacterImage", mainMenu)
        if (image !== null) mustApply(this.#runtime, { kind: "set-panel-state", panel: image, visible: false })
      }
      this.#applyOwned(DASHBOARD_PATH, dashboard)
      this.#applyOwned(DASHBOARD_PLAYLIST_PATH, playlistContainer)
      this.#applyOwned(PLAYLIST_PATH, playlist, ["if_wider"])
      this.#configurePlaylist()
      this.#disableUnownedControls()
      this.#hideUnavailablePanels()
      for (const identity of [1, mainMenu]) mustApply(this.#runtime, { kind: "set-panel-state", panel: identity, mouseInput: false, keyboardInput: false })
      this.#captureBaseVisibility()
      this.#presentState()
    })
  }

  #captureBaseVisibility(owner?: VguiPanelId): void {
    const panels = this.#runtime.snapshot().panels
    if (owner === undefined) this.#baseVisibility.clear()
    const byId = new Map(panels.map((panel) => [panel.id, panel]))
    const owned = (panel: typeof panels[number]): boolean => {
      if (owner === undefined || panel.id === owner) return true
      let current = panel
      while (current.parent !== null) {
        if (current.parent === owner) return true
        const parent = byId.get(current.parent)
        if (!parent) return false
        current = parent
      }
      return false
    }
    for (const panel of panels) if (owned(panel)) this.#baseVisibility.set(panel.id, panel.visible)
  }

  #apply(logicalPath: string, parent: VguiPanelId, conditions: readonly string[] = []): void {
    const document = this.#resources.document(logicalPath)
    applyResourceTree(this.#runtime, parent, document, document.root, this.#diagnostics, [...this.#resources.activeConditions, ...conditions], ["_hidef"])
  }

  #configurePlaylist(): void {
    const buttons = TF2_MAIN_MENU_STATE.panels.find((panel) => panel.identity === "play-list")!.buttons
    for (const button of buttons) {
      const entryName = PLAYLIST_CONTROLS[button.identity]
      const entry = panelByName(this.#runtime, entryName)
      if (entry === null) continue
      this.#apply(PLAYLIST_ENTRY_PATH, entry)
      const mode = panelByName(this.#runtime, "ModeButton", entry)
      if (mode !== null) {
        mustApply(this.#runtime, { kind: "mutate-control", panel: mode, mutation: { text: button.text, command: button.sourceCommand } })
        mustApply(this.#runtime, { kind: "set-panel-state", panel: mode, enabled: button.capability.kind === "request", mouseInput: true })
      }
      if (button.visibility === "event-conditional") mustApply(this.#runtime, { kind: "set-panel-state", panel: entry, visible: false })
    }
  }

  #disableUnownedControls(): void {
    const active = new Set(["OpenOptionsDialog", "opentf2options", "view_newuser_forums", "quit", "resume_game", "Cancel", "find_game", "play_training", "create_server"])
    const snapshots = this.#runtime.snapshot().panels
    for (const command of this.#resources.descriptor.commands) {
      if (active.has(command.command)) continue
      const segments = command.nodePath.split("/")
      const controlName = /^(.*)\[\d+\]$/u.exec(segments.at(-2) ?? "")?.[1]
      if (!controlName) continue
      const panel = snapshots.find((candidate) => candidate.name === controlName)
      if (panel) mustApply(this.#runtime, { kind: "set-panel-state", panel: panel.id, enabled: false })
    }
  }

  #hideUnavailablePanels(): void {
    const names = new Set([
      "CycleRankTypeButton", "RankTooltipPanel", "RankPanel", "RankModelPanel", "NoGCMessage", "NoGCImage", "RankBorder",
      "Notifications_ShowButtonPanel", "Notifications_Panel", "WatchStreamButton", "QuestLogButton",
      "MOTD_ShowButtonPanel", "MOTD_Panel", "VRBGPanel", "VRModeButton", "FriendsContainer", "EventPromo",
      "ShowPromoCodesButton", "CharacterSetupButton", "GeneralStoreButton", "StoreHasNewItemsImage",
      "ReportPlayerButton", "CallVoteButton", "MutePlayersButton", "RequestCoachButton",
      "AchievementsButton", "CommentaryButton", "CoachPlayersButton", "WorkshopButton", "ReplayButton", "ReportBugButton",
      "SettingsButtonSDK", "TF2SettingsButtonSDK", "icon_generator", "ToggleChatButton", "QueueContainer", "JoinPartyLobbyContainer",
      ...Array.from({ length: 6 }, (_, index) => `PartySlot${index}`),
    ])
    for (const panel of this.#runtime.snapshot().panels) {
      if (names.has(panel.name)) mustApply(this.#runtime, { kind: "set-panel-state", panel: panel.id, visible: false })
    }
  }

  #vguiRequest(request: VguiRequest): void {
    if (request.kind !== "command") return
    const panel = this.#runtime.snapshot().panels.find((candidate) => candidate.id === request.panel)
    const command = request.command
    let button: Tf2MenuButton["identity"] | null = null
    if (command === "OpenOptionsDialog") button = "options"
    else if (command === "opentf2options") button = "advanced-options"
    else if (command === "view_newuser_forums") button = "new-user-forum"
    else if (command === "resume_game") button = "resume"
    else if (command === "Cancel") button = "cancel-loading"
    else if (command === "find_game") button = "find-game"
    else if (command === "play_training") button = "training"
    else if (command === "create_server") button = "create-server"
    else if (command === "quit") button = panel?.name === "DisconnectButton" ? "disconnect" : "quit"
    if (button === null) {
      this.#diagnostics.push(Object.freeze({ code: "InactiveCommand", subject: command }))
      return
    }
    this.dispatch({ kind: "activate-button", button })
  }

  #presentState(): void {
    const mainMenu = this.#state.kind === "main-menu"
    const menu = mainMenu || this.#state.kind === "pause"
    const gameplay = this.#state.kind === "in-game"
    const before = this.#runtime.snapshot().panels
    const byId = new Map(before.map((panel) => [panel.id, panel]))
    const byName = new Map<string, typeof before[number]>()
    const visibility = new Map(before.map((panel) => [panel.id, panel.visible]))
    for (const panel of before) if (!byName.has(panel.name.toLowerCase())) byName.set(panel.name.toLowerCase(), panel)
    const setVisible = (panel: VguiPanelId, visible: boolean): void => {
      if (visibility.get(panel) === visible) return
      mustApply(this.#runtime, { kind: "set-panel-state", panel, visible })
      visibility.set(panel, visible)
    }
    const dashboard = before.find((panel) => panel.name === "MMDashboard")
    const mainOverride = before.find((panel) => panel.name === "MainMenuOverride")
    const resume = before.find((panel) => panel.name === "ResumeButton")
    const disconnect = before.find((panel) => panel.name === "DisconnectButton")
    const findGame = before.find((panel) => panel.name === "FindAGameButton")
    const quit = before.find((panel) => panel.name === "QuitButton")
    if (this.#baseBackground !== null) setVisible(this.#baseBackground, tf2GameUiBaseBackground(this.#resources.gameUiBackground, this.#state, this.#viewport).visible)
    const descendsFrom = (panelId: VguiPanelId, ancestorId: VguiPanelId): boolean => {
      let current = byId.get(panelId)
      while (current?.parent !== null) {
        if (current?.parent === ancestorId) return true
        current = current?.parent === undefined ? undefined : byId.get(current.parent)
      }
      return false
    }
    if (dashboard) {
      for (const panel of before.filter((candidate) => descendsFrom(candidate.id, dashboard.id))) {
        const pauseControls = [resume, disconnect, findGame].filter((value) => value !== undefined)
        const pauseControl = pauseControls.some((control) =>
          panel.id === control.id || descendsFrom(panel.id, control.id) || descendsFrom(control.id, panel.id),
        )
        setVisible(panel.id, this.#state.kind === "pause" ? pauseControl : (this.#baseVisibility.get(panel.id) ?? panel.visible))
      }
    }
    if (mainOverride) {
      for (const panel of before.filter((candidate) => descendsFrom(candidate.id, mainOverride.id))) {
        setVisible(panel.id, panel.name === "TFCharacterImage"
          ? tf2CharacterImageVisible(this.#state, this.#baseVisibility.get(panel.id) ?? panel.visible)
          : menu ? (this.#baseVisibility.get(panel.id) ?? panel.visible) : false)
      }
    }
    for (const name of ["MainMenuOverride", "MMDashboard", "TopBar", "ExpandableList", "playlist", "EventEntry", "CasualEntry", "CompetitiveEntry", "MvMEntry", "ServerBrowserEntry", "TrainingEntry", "CreateServerEntry"]) {
      const panel = byName.get(name.toLowerCase())
      if (panel) setVisible(panel.id, (name === "MainMenuOverride" || name === "MMDashboard" || name === "TopBar" ? menu : menu && this.#playlistActive) && name !== "EventEntry")
    }
    const expandable = byName.get("expandablelist")
    if (expandable) {
      const x = this.#playlistActive && menu ? this.#viewport.width - expandable.bounds.width : this.#viewport.width
      if (expandable.bounds.x !== x) mustApply(this.#runtime, { kind: "set-bounds", panel: expandable.id, bounds: { ...expandable.bounds, x } })
    }
    const states: Readonly<Record<string, boolean>> = Object.freeze({
      QuitButton: this.#state.kind === "main-menu",
      ResumeButton: this.#state.kind === "pause",
      DisconnectButton: this.#state.kind === "pause",
      FindAGameButton: menu,
      GeneralStoreButton: false,
      CharacterSetupButton: false,
      SettingsButton: menu,
      SettingsButtonSDK: false,
      TF2SettingsButton: menu,
      TF2SettingsButtonSDK: false,
      NewUserForumsButton: menu,
    })
    for (const [name, visible] of Object.entries(states)) {
      const panel = byName.get(name.toLowerCase())
      if (panel) setVisible(panel.id, visible)
    }
    const offset = this.#state.kind === "pause" ? disconnect : quit
    if (offset && findGame) {
      const findBounds = { ...findGame.bounds, x: offset.bounds.x - findGame.bounds.width - 1 }
      if (findGame.bounds.x !== findBounds.x) mustApply(this.#runtime, { kind: "set-bounds", panel: findGame.id, bounds: findBounds })
      if (resume) {
        const resumeX = findBounds.x - resume.bounds.width - 1
        if (resume.bounds.x !== resumeX) mustApply(this.#runtime, { kind: "set-bounds", panel: resume.id, bounds: { ...resume.bounds, x: resumeX } })
      }
    }
    setVisible(1, menu || this.#state.kind === "loading" || this.#state.kind === "failure")
    if (gameplay) mustApply(this.#runtime, { kind: "request-focus", panel: null })
  }

  state(): Tf2GameUiState { return this.#state }
  snapshot(): VguiRuntimeSnapshot { return this.#runtime.snapshot() }
  diagnostics(): readonly Tf2GameUiIntegrationDiagnostic[] { return Object.freeze([...this.#diagnostics]) }

  dispatch(event: Tf2GameUiEvent): Tf2GameUiTransition {
    if (this.#destroyed) throw new Error("TF2 GameUI integration is destroyed")
    const transition = transitionTf2GameUi(this.#state, event)
    if (transition.request?.kind === "show-play-list") this.#playlistActive = true
    else if (transition.request?.kind === "show-local-match" || transition.state.kind === "loading" || transition.state.kind === "disconnecting") this.#playlistActive = false
    this.#state = transition.state
    this.#runtime.deferPresentation(() => this.#presentState())
    if (transition.request && transition.request.kind !== "show-play-list") this.#onRequest(transition.request)
    return transition
  }

  frame(timeSeconds: number): void { mustApply(this.#runtime, { kind: "frame", timeSeconds }) }
  setViewport(viewport: VguiViewport): void {
    const aspectCondition = tf2MainMenuAspectCondition(viewport)
    const priorAspect = this.#mainMenuConditions.find((condition) => condition === "if_wider" || condition === "if_taller")
    this.#runtime.deferPresentation(() => {
      mustApply(this.#runtime, { kind: "set-viewport", viewport })
      this.#viewport = viewport
      this.#configureBaseBackground(viewport)
      if (this.#mainMenu !== null && priorAspect !== aspectCondition) {
        this.#mainMenuConditions = [aspectCondition, ...this.#mainMenuConditions.filter((condition) => condition !== "if_wider" && condition !== "if_taller")]
        this.#applyOwned(MAIN_MENU_PATH, this.#mainMenu, this.#mainMenuConditions)
        this.#configureScrollers(this.#mainMenu)
        this.#disableUnownedControls()
        this.#hideUnavailablePanels()
        this.#captureBaseVisibility(this.#mainMenu)
      }
      this.#presentState()
    })
  }
  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    mustApply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2GameUiIntegration(request: Tf2GameUiIntegrationRequest): Tf2GameUiIntegration {
  return Object.freeze(new Integration(request))
}
