import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiPanelSnapshot,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiSectionedListItem,
  type VguiViewport,
} from "@playsrc/vgui"
import {
  adaptSessionHud,
  bindTf2Hud,
  bindTf2HudAction,
  TF2_HUD_DYNAMIC_IMAGES,
  TF2_SCOREBOARD_CLASS_IMAGES,
  TF2_SCOREBOARD_IMAGES,
  TF2_GROUPED_CONDITION_PANELS,
  TF2_INDEPENDENT_CONDITION_PANELS,
  tf2HudAvailable,
  tf2HudUnavailable,
  type SessionHudContext,
  type SessionSimulationPublication,
  type Tf2HudAction,
  type Tf2HudAvailability,
  type Tf2HudBinding,
  type Tf2HudCommand,
  type Tf2HudCrosshair,
  type Tf2HudPanelValue,
  type Tf2HudScoreboard,
  type Tf2HudSnapshot,
  type Tf2ScoreboardPlayer,
} from "../hud"
import type { CaptureObjectives, RoundSnapshot, Tf2Team } from "../codec"
import type { Tf2VguiResources } from "../ui-integration"
import { Tf2HudCrosshairPresentation } from "./crosshair"
import { Tf2HudScopePresentation } from "./scope"

export type Tf2HudIntegrationDiagnostic = Readonly<{
  code: "VguiRejected" | "PanelUnavailable" | "ValueUnavailable" | "UnsupportedPanelValue" | "AnimationUnavailable"
  subject: string
}>

export type Tf2HudIntegrationSnapshot = Readonly<{
  binding: Tf2HudBinding | null
  vgui: VguiRuntimeSnapshot
  diagnostics: readonly Tf2HudIntegrationDiagnostic[]
  animationTrace: readonly string[]
}>

export type Tf2HudIntegrationProbe = Readonly<{
  panels: readonly VguiPanelSnapshot[]
  animationTrace: readonly string[]
}>

export type Tf2HudIntegration = Readonly<{
  publish(publication: SessionSimulationPublication, context: SessionHudContext): Tf2HudBinding
  action(action: Tf2HudAction): Tf2HudAvailability<Tf2HudCommand>
  frame(timeSeconds: number): void
  setViewport(viewport: VguiViewport): void
  probe(): Tf2HudIntegrationProbe
  snapshot(): Tf2HudIntegrationSnapshot
  setPlayerClassUsePlayerModel(value: boolean): void
  setCrosshair(value: Tf2HudCrosshair): void
  setScoreboardVisibility(visible: boolean): void
  reset(reason: "map-replaced" | "disconnect"): void
  destroy(): void
}>

export type Tf2HudIntegrationRequest = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onCommand(command: Tf2HudCommand): void
}>

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 512,
  maxHierarchyDepth: 32,
  maxChildrenPerPanel: 256,
  maxResourceNodes: 2_048,
  maxResourceDepth: 32,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4_095,
  maxTextCodeUnits: 65_535,
  maxDialogVariables: 256,
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
  maxQueuedMessages: 2_048,
  maxDiagnostics: 2_048,
  maxDomNodes: 2_048,
  maxListeners: 64,
})

const HUD_LAYOUT = "scripts/hudlayout.res"
const HUD_CLASS = "resource/ui/hudplayerclass.res"
const HUD_HEALTH = "resource/ui/hudplayerhealth.res"
const HUD_AMMO = "resource/ui/hudammoweapons.res"
const HUD_WEAPONS = "resource/ui/hudweaponselection.res"
const HUD_MATCH_STATUS = "resource/ui/hudmatchstatus.res"
const HUD_TIME_PANEL = "resource/ui/hudobjectivetimepanel.res"
const HUD_WAITING_PANEL = "resource/ui/waitingforplayerspanel.res"
const HUD_SCOREBOARD = "resource/ui/scoreboard.res"
const HUD_OBJECTIVE_FLAGS = "resource/ui/hudobjectiveflagpanel.res"
const HUD_FLAG_STATUS = "resource/ui/flagstatus.res"
const HUD_WIN_PANEL = "resource/ui/winpanel.res"
const HUD_NOTIFICATION_BASE = "resource/ui/notifications/base_notification.res"
const FLAG_STATUS_IMAGES = Object.freeze([
  "../hud/objectives_flagpanel_ico_flag_home",
  "../hud/objectives_flagpanel_ico_flag_moving",
  "../hud/objectives_flagpanel_ico_flag_dropped",
])
const NOTIFICATION_FILES = Object.freeze([
  "your_flag_taken", "your_flag_dropped", "your_flag_returned", "your_flag_captured",
  "enemy_flag_taken", "enemy_flag_dropped", "enemy_flag_returned", "enemy_flag_captured",
  "touching_enemy_ctf_cap",
])
const HUD_SPY_METER = "resource/ui/huditemeffectmeter_spy.res"
const HUD_SPY_DISGUISE_MENU = "resource/ui/disguise_menu/hudmenuspydisguise.res"
const HUD_MEDIC_CHARGE = "resource/ui/hudmediccharge.res"
const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const node = (name: string, children: readonly VguiResourceNode[]): VguiResourceNode => Object.freeze({ name, value: null, condition: null, children: Object.freeze(children) })
const document = (source: VguiResourceDocument, suffix: string, root: VguiResourceNode): VguiResourceDocument => Object.freeze({
  logicalIdentity: `${source.logicalIdentity}/${suffix}`.toLowerCase(),
  revision: source.revision,
  root,
})

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function find(runtime: VguiRuntime, name: string, parent?: VguiPanelId): VguiPanelId | null {
  return runtime.snapshot().panels.find((panel) => panel.name.toLowerCase() === name.toLowerCase() && (parent === undefined || panel.parent === parent))?.id ?? null
}

function resourceChildren(block: VguiResourceNode): readonly VguiResourceNode[] {
  return block.children.filter((child) => child.value === null && scalar(child, "ControlName") !== null)
}

function shallow(block: VguiResourceNode): VguiResourceNode {
  return Object.freeze({ ...block, children: Object.freeze(block.children.filter((child) => child.value !== null || child.name.toLowerCase() === "controlname")) })
}

function applyChildren(runtime: VguiRuntime, parent: VguiPanelId, source: VguiResourceDocument, blocks: readonly VguiResourceNode[], activeConditions: readonly string[]): void {
  const selected = blocks.filter((block) => block.value === null && scalar(block, "ControlName") !== null)
  if (selected.length === 0) return
  apply(runtime, {
    kind: "replace-resource",
    parent,
    document: document(source, `children-${parent}`, node(source.root.name, selected.map(shallow))),
    selection: { activeConditions, resolutionSuffixes: ["_hidef"] },
  })
  for (const block of selected) {
    const child = find(runtime, scalar(block, "fieldName") ?? block.name, parent)
    if (child !== null) applyChildren(runtime, child, source, resourceChildren(block), activeConditions)
  }
}

function applyPanelResource(runtime: VguiRuntime, panel: VguiPanelId, source: VguiResourceDocument, activeConditions: readonly string[]): void {
  const snapshot = runtime.snapshot().panels.find((candidate) => candidate.id === panel)!
  const self = source.root.children.find((block) => (scalar(block, "fieldName") ?? block.name).toLowerCase() === snapshot.name.toLowerCase())
  if (self && snapshot.parent !== null) {
    apply(runtime, {
      kind: "replace-resource",
      parent: snapshot.parent,
      document: document(source, `self-${panel}`, node(source.root.name, [shallow(self)])),
      selection: { activeConditions, resolutionSuffixes: ["_hidef"] },
    })
  }
  applyChildren(runtime, panel, source, source.root.children.filter((block) => block !== self), activeConditions)
}

class Integration implements Tf2HudIntegration {
  readonly #runtime: VguiRuntime
  readonly #crosshair: Tf2HudCrosshairPresentation
  readonly #scope: Tf2HudScopePresentation
  readonly #resources: Tf2VguiResources
  readonly #onCommand: (command: Tf2HudCommand) => void
  readonly #cloakLabel: string | undefined
  readonly #diagnostics: Tf2HudIntegrationDiagnostic[] = []
  readonly #diagnosticSubjects = new Set<string>()
  readonly #animationTrace: string[] = []
  readonly #baseBounds = new Map<string, Readonly<{ x: number; y: number; width: number; height: number }>>()
  readonly #panels = new Map<string, VguiPanelId>()
  readonly #publishedValues = new Map<string, string>()
  readonly #localization: ReadonlyMap<string, string>
  readonly #scoreboardProperties: ReadonlyMap<string, string>
  #scoreboardFingerprint = ""
  readonly #deathNotices: Array<{ panel: VguiPanelId; expires: bigint }> = []
  #previous: Tf2HudAvailability<Tf2HudSnapshot> = tf2HudUnavailable("initial")
  #binding: Tf2HudBinding | null = null
  #viewport: VguiViewport
  #objective?: Readonly<{
    root: VguiPanelId
    panel: VguiPanelId
    redFlag: VguiPanelId
    blueFlag: VguiPanelId
    redStatus: VguiPanelId
    blueStatus: VguiPanelId
    redArrow: VguiPanelId
    blueArrow: VguiPanelId
    carried: VguiPanelId
    outline: VguiPanelId
    captureArrow: VguiPanelId
    playingTo: VguiPanelId
    playingToBackground: VguiPanelId
    notification: VguiPanelId
    notificationBackground: VguiPanelId
    notificationLabel: VguiPanelId
  }>
  #notificationDeadline = 0n
  #objectiveCarrying = false
  #winPanel?: VguiPanelId
  #roundPanels?: Readonly<{
    match: VguiPanelId
    timer: VguiPanelId
    value: VguiPanelId
    waiting: VguiPanelId
    timerChildren: ReadonlyMap<string, VguiPanelId>
    waitingChildren: ReadonlyMap<string, VguiPanelId>
  }>
  #destroyed = false

  constructor(request: Tf2HudIntegrationRequest) {
    this.#resources = request.resources
    this.#onCommand = request.onCommand
    this.#cloakLabel = request.resources.localization.tokens.find(token => token.name.toLowerCase() === "tf_cloak")?.value
    this.#viewport = Object.freeze({ ...request.viewport })
    this.#localization = new Map(request.resources.localization.tokens.map((token) => [`#${token.name.replace(/^#/u, "").toLowerCase()}`, token.value]))
    const scoreboardDocument = request.resources.document(HUD_SCOREBOARD)
    const scoreboardRoot = scoreboardDocument.root.children.find((block) => (scalar(block, "fieldName") ?? block.name).toLowerCase() === "scoreinfo")
    if (!scoreboardRoot) throw new Error("TF2 scoreboard authored root is unavailable")
    this.#scoreboardProperties = new Map(scoreboardRoot.children.filter((child) => child.value !== null).map((child) => [child.name.toLowerCase(), child.value!]))
    const availableImages = new Set(request.resources.clientScheme.images.map((image) => image.name.toLowerCase()))
    const missingImages = [...TF2_HUD_DYNAMIC_IMAGES, ...TF2_SCOREBOARD_IMAGES].filter((image) => !availableImages.has(image.toLowerCase()))
    if (missingImages.length > 0) throw new Error(`TF2 HUD dynamic images are unavailable: ${missingImages.join(",")}`)
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "tf2-hud",
      root: request.root,
      rootControl: { control: "EditablePanel", name: "HudViewport" },
      viewport: request.viewport,
      limits: LIMITS,
      clock: request.clock,
      random: request.random,
      scheme: request.resources.clientScheme,
      localization: request.resources.localization,
      animationScripts: request.resources.animations,
      customControls: request.resources.customControls,
      reducedMotion: request.reducedMotion,
      onRequest: () => {},
    })
    if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    this.#runtime = initialized.runtime
    this.#runtime.deferPresentation(() => {
    apply(this.#runtime, { kind: "set-panel-state", panel: 1, proportional: true, mouseInput: false, keyboardInput: false })
    const roots = [
      ["HudPlayerStatus", "CTFHudElement"],
      ["HudWeaponAmmo", "CTFHudElement"],
      ["HudWeaponSelection", "CTFHudElement"],
      ["HudItemEffectMeter", "CTFHudElement"],
      ["HudMenuSpyDisguise", "CTFHudElement"],
      ["HudCrosshair", "CTFHudElement"],
      ["HudDeathNotice", "CTFHudElement"],
      ["HudMedicCharge", "CTFHudElement"],
    ] as const
    for (const [name, control] of roots) apply(this.#runtime, { kind: "create-panel", parent: 1, control, name })
    const layout = request.resources.document(HUD_LAYOUT)
    const selectedLayout = layout.root.children.filter((block) => roots.some(([name]) => name === (scalar(block, "fieldName") ?? block.name)))
    apply(this.#runtime, {
      kind: "replace-resource",
      parent: 1,
      document: document(layout, "selected", node(layout.root.name, selectedLayout.map(shallow))),
      selection: { activeConditions: request.resources.activeConditions, resolutionSuffixes: ["_hidef"] },
    })
    const status = find(this.#runtime, "HudPlayerStatus")!
    const playerClass = apply(this.#runtime, { kind: "create-panel", parent: status, control: "CTFHudElement", name: "HudPlayerClass" })!
    const playerHealth = apply(this.#runtime, { kind: "create-panel", parent: status, control: "CTFHudElement", name: "HudPlayerHealth" })!
    apply(this.#runtime, { kind: "create-panel", parent: playerHealth, control: "CTFHealthPanel", name: "PlayerStatusHealthImage" })
    applyPanelResource(this.#runtime, playerClass, request.resources.document(HUD_CLASS), request.resources.activeConditions)
    applyPanelResource(this.#runtime, playerHealth, request.resources.document(HUD_HEALTH), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudWeaponAmmo")!, request.resources.document(HUD_AMMO), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudWeaponSelection")!, request.resources.document(HUD_WEAPONS), request.resources.activeConditions)
    const scoreboard = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFClientScoreBoardDialog", name: "scoreinfo" })!
    const scoreboardNames = new Set<string>()
    const normalizedBlocks = scoreboardDocument.root.children.map((block) => {
      if (block.value !== null) return block
      const identity = (scalar(block, "fieldName") ?? block.name).toLowerCase()
      if (!scoreboardNames.has(identity)) {
        scoreboardNames.add(identity)
        return identity === "scoreinfo" && block.name.toLowerCase() !== identity
          ? Object.freeze({ ...block, name: "scoreinfo" })
          : block
      }
      return Object.freeze({
        ...block,
        children: Object.freeze(block.children.map((child) => child.name.toLowerCase() === "fieldname"
          ? Object.freeze({ ...child, value: block.name })
          : child)),
      })
    })
    applyPanelResource(this.#runtime, scoreboard,
      document(scoreboardDocument, "authored", node(scoreboardDocument.root.name, normalizedBlocks)),
      request.resources.activeConditions)
    apply(this.#runtime, { kind: "set-panel-state", panel: scoreboard, visible: false, z: 100 })
    for (const name of ["LocalPlayerDuelStatsPanel", "ButtonLegend", "ButtonLegendBG", "MvMScoreboard"]) {
      const panel = find(this.#runtime, name)
      if (panel !== null) apply(this.#runtime, { kind: "set-panel-state", panel, visible: false })
    }
    applyPanelResource(this.#runtime, find(this.#runtime, "HudItemEffectMeter")!, request.resources.document(HUD_SPY_METER), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudMenuSpyDisguise")!, request.resources.document(HUD_SPY_DISGUISE_MENU), request.resources.activeConditions)
    applyPanelResource(this.#runtime, find(this.#runtime, "HudMedicCharge")!, request.resources.document(HUD_MEDIC_CHARGE), request.resources.activeConditions)
    const panels = this.#runtime.snapshot().panels
    for (const panel of panels) {
      if (!this.#panels.has(panel.name.toLowerCase())) this.#panels.set(panel.name.toLowerCase(), panel.id)
      apply(this.#runtime, { kind: "set-panel-state", panel: panel.id, mouseInput: false, keyboardInput: false })
    }
    this.#captureBaseBounds(panels)
    })
    this.#crosshair = new Tf2HudCrosshairPresentation(request.root)
    this.#scope = new Tf2HudScopePresentation(request.root)
  }

  #initializeObjectives() {
    if (this.#objective) return this.#objective
    const root = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFHudElement", name: "HudObjectiveStatus" })!
    const notification = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFHudElement", name: "NotificationPanel" })!
    const layout = this.#resources.document(HUD_LAYOUT)
    const selected = layout.root.children.filter((block) => ["HudObjectiveStatus", "NotificationPanel"].includes(scalar(block, "fieldName") ?? block.name))
    apply(this.#runtime, {
      kind: "replace-resource",
      parent: 1,
      document: document(layout, "ctf-objectives", node(layout.root.name, selected.map(shallow))),
      selection: { activeConditions: this.#resources.activeConditions, resolutionSuffixes: ["_hidef"] },
    })
    const panel = apply(this.#runtime, { kind: "create-panel", parent: root, control: "EditablePanel", name: "ObjectiveStatusFlagPanel" })!
    applyPanelResource(this.#runtime, panel, this.#resources.document(HUD_OBJECTIVE_FLAGS), this.#resources.activeConditions)
    const redFlag = find(this.#runtime, "RedFlag", panel)!
    const blueFlag = find(this.#runtime, "BlueFlag", panel)!
    const flagDocument = this.#resources.document(HUD_FLAG_STATUS)
    applyPanelResource(this.#runtime, redFlag, flagDocument, this.#resources.activeConditions)
    applyPanelResource(this.#runtime, blueFlag, flagDocument, this.#resources.activeConditions)
    applyPanelResource(this.#runtime, notification, this.#resources.document(HUD_NOTIFICATION_BASE), this.#resources.activeConditions)
    const required = (name: string, parent: VguiPanelId): VguiPanelId => {
      const value = find(this.#runtime, name, parent)
      if (value === null) throw new Error(`TF2 CTF HUD panel ${name} is missing`)
      return value
    }
    const redStatus = required("StatusIcon", redFlag), blueStatus = required("StatusIcon", blueFlag)
    const redArrow = required("Arrow", redFlag), blueArrow = required("Arrow", blueFlag)
    apply(this.#runtime, { kind: "mutate-control", panel: redArrow, mutation: { image: "../hud/objectives_flagpanel_compass_red" } })
    apply(this.#runtime, { kind: "mutate-control", panel: blueArrow, mutation: { image: "../hud/objectives_flagpanel_compass_blue" } })
    apply(this.#runtime, { kind: "set-panel-state", panel: notification, visible: false, mouseInput: false, keyboardInput: false })
    const objective = Object.freeze({
      root, panel, redFlag, blueFlag, redStatus, blueStatus, redArrow, blueArrow,
      carried: required("CarriedImage", panel),
      outline: required("OutlineImage", panel),
      captureArrow: required("CaptureFlag", panel),
      playingTo: required("PlayingTo", panel),
      playingToBackground: required("PlayingToBG", panel),
      notification,
      notificationBackground: required("Notification_Background", notification),
      notificationLabel: required("Notification_Label", notification),
    })
    for (const value of this.#runtime.snapshot().panels) {
      this.#panels.set(value.name.toLowerCase(), value.id)
      apply(this.#runtime, { kind: "set-panel-state", panel: value.id, mouseInput: false, keyboardInput: false })
    }
    apply(this.#runtime, { kind: "set-panel-state", panel: objective.outline, visible: false })
    this.#captureBaseBounds()
    this.#objective = objective
    return objective
  }

  #initializeRoundPanels() {
    if (this.#roundPanels) return this.#roundPanels
    const match = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFHudElement", name: "HudMatchStatus" })!
    const waiting = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFHudElement", name: "WaitingForPlayersPanel" })!
    const layout = this.#resources.document(HUD_LAYOUT)
    const roots = layout.root.children.filter((value) => ["HudMatchStatus", "WaitingForPlayersPanel"].includes(scalar(value, "fieldName") ?? value.name))
    if (roots.length !== 2) throw new Error("TF2 authored round HUD root layout is unavailable")
    apply(this.#runtime, {
      kind: "replace-resource", parent: 1,
      document: document(layout, "round-roots", node(layout.root.name, roots.map(shallow))),
      selection: { activeConditions: this.#resources.activeConditions, resolutionSuffixes: ["_hidef"] },
    })
    const timer = apply(this.#runtime, { kind: "create-panel", parent: match, control: "EditablePanel", name: "ObjectiveStatusTimePanel" })!
    const matchResource = this.#resources.document(HUD_MATCH_STATUS)
    const authoredTimer = matchResource.root.children.find((value) => (scalar(value, "fieldName") ?? value.name) === "ObjectiveStatusTimePanel")
    if (!authoredTimer) throw new Error("TF2 authored round timer position is unavailable")
    apply(this.#runtime, {
      kind: "replace-resource", parent: match,
      document: document(matchResource, "round-timer-position", node(matchResource.root.name, [shallow(authoredTimer)])),
      selection: { activeConditions: this.#resources.activeConditions, resolutionSuffixes: ["_hidef"] },
    })
    const value = apply(this.#runtime, { kind: "create-panel", parent: timer, control: "CExLabel", name: "TimePanelValue" })!
    applyChildren(this.#runtime, timer, matchResource, resourceChildren(authoredTimer), this.#resources.activeConditions)
    applyPanelResource(this.#runtime, timer, this.#resources.document(HUD_TIME_PANEL), this.#resources.activeConditions)
    applyPanelResource(this.#runtime, waiting, this.#resources.document(HUD_WAITING_PANEL), this.#resources.activeConditions)
    const timerChildren = new Map<string, VguiPanelId>()
    const waitingChildren = new Map<string, VguiPanelId>()
    for (const panel of this.#runtime.snapshot().panels) {
      this.#panels.set(panel.name.toLowerCase(), panel.id)
      if (panel.parent === timer) timerChildren.set(panel.name.toLowerCase(), panel.id)
      if (panel.parent === waiting) waitingChildren.set(panel.name.toLowerCase(), panel.id)
      apply(this.#runtime, { kind: "set-panel-state", panel: panel.id, mouseInput: false, keyboardInput: false })
    }
    this.#captureBaseBounds()
    this.#roundPanels = Object.freeze({ match, timer, value, waiting, timerChildren, waitingChildren })
    return this.#roundPanels
  }

  #publishRound(round: RoundSnapshot, team: Tf2Team): void {
    if (!round.waitingForPlayers && !round.timer && !this.#roundPanels) return
    const panels = this.#initializeRoundPanels()
    const timerVisible = round.waitingForPlayers || (round.timer !== null && round.timer.showInHud && !round.timer.disabled)
    const setVisible = (panel: VguiPanelId, visible: boolean, name: string): void => {
      this.#objectiveValue(`round-visible:${name}`, String(visible), { kind: "set-panel-state", panel, visible })
    }
    setVisible(panels.match, timerVisible, "match")
    setVisible(panels.timer, timerVisible, "timer")
    setVisible(panels.waiting, round.waitingForPlayers, "waiting")
    const seconds = Math.max(0, Math.floor(round.waitingForPlayers ? round.waitingRemaining ?? 0 : round.timer?.remaining ?? 0))
    const text = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
    this.#objectiveValue("round-time", text, { kind: "mutate-control", panel: panels.value, mutation: { text } })
    const background = panels.timerChildren.get("timepanelbg")
    if (background !== undefined) {
      const image = `../hud/objectives_timepanel_${team === 2 ? "red" : "blue"}_bg`
      this.#objectiveValue("round-team-background", image, { kind: "mutate-control", panel: background, mutation: { image } })
    }
    for (const [name, visible] of [
      ["WaitingForPlayersLabel", round.waitingForPlayers],
      ["WaitingForPlayersBG", round.waitingForPlayers],
      ["SetupLabel", round.inSetup && !round.waitingForPlayers],
      ["SetupBG", round.inSetup && !round.waitingForPlayers],
      ["OvertimeLabel", round.inOvertime],
      ["OvertimeBG", round.inOvertime],
      ["SuddenDeathLabel", round.state === 7],
      ["SuddenDeathBG", round.state === 7],
      ["ServerTimeLimitLabel", false],
      ["ServerTimeLimitLabelBG", false],
    ] as const) {
      const panel = panels.timerChildren.get(name.toLowerCase())
      if (panel !== undefined) setVisible(panel, visible, `timer-${name}`)
    }
    const aboutToEnd = round.waitingRemaining !== null && round.waitingRemaining <= 10
    for (const [name, visible] of [
      ["WaitingForPlayersLabel", !aboutToEnd],
      ["WaitingForPlayersEndingLabel", aboutToEnd],
    ] as const) {
      const panel = panels.waitingChildren.get(name.toLowerCase())
      if (panel !== undefined) setVisible(panel, visible, `waiting-${name}`)
    }
  }

  #publishWinPanel(round: RoundSnapshot | undefined, objectives: CaptureObjectives | undefined): void {
    const winner = round?.winningTeam ?? objectives?.winner ?? null
    if (winner === null) {
      if (this.#winPanel !== undefined) {
        this.#objectiveValue("ctf-win:visible", "false", { kind: "set-panel-state", panel: this.#winPanel, visible: false })
      }
      return
    }
    let panel = this.#winPanel
    if (panel === undefined) {
      panel = apply(this.#runtime, { kind: "create-panel", parent: 1, control: "CTFHudElement", name: "WinPanel" })!
      const layout = this.#resources.document(HUD_LAYOUT)
      const selected = layout.root.children.find((value) => (scalar(value, "fieldName") ?? value.name) === "WinPanel")
      if (!selected) throw new Error("TF2 authored round-win layout is unavailable")
      apply(this.#runtime, {
        kind: "replace-resource",
        parent: 1,
        document: document(layout, "ctf-win-panel", node(layout.root.name, [shallow(selected)])),
        selection: { activeConditions: this.#resources.activeConditions, resolutionSuffixes: ["_hidef"] },
      })
      applyPanelResource(this.#runtime, panel, this.#resources.document(HUD_WIN_PANEL), this.#resources.activeConditions)
      this.#winPanel = panel
      for (const value of this.#runtime.snapshot().panels) {
        this.#panels.set(value.name.toLowerCase(), value.id)
        apply(this.#runtime, { kind: "set-panel-state", panel: value.id, mouseInput: false, keyboardInput: false })
      }
      this.#captureBaseBounds()
    }
    this.#objectiveValue("ctf-win:visible", "true", { kind: "set-panel-state", panel, visible: true })
    const localized = (name: string): string => this.#resources.localization.tokens
      .find((value) => value.name.toLowerCase() === name.replace(/^#/u, "").toLowerCase())?.value ?? name
    const team = winner === 2 ? "RED" : "BLU"
    const winning = localized("#Winpanel_TeamWins")
      .replace("%s1", team)
      .replace("%s2", localized("#Winpanel_Team1"))
    const reasonToken = round?.winReason === 4 ? "#Winreason_DefendedUntilTimeLimit"
      : round?.winReason === 2 ? "#Winreason_OpponentsDead"
        : round?.winReason === 5 ? "#Winreason_Stalemate"
          : objectives?.captureLimit === 1 ? "#Winreason_FlagCaptureLimit_One" : "#Winreason_FlagCaptureLimit"
    const reason = localized(reasonToken)
      .replace("%s1", team)
      .replace("%s2", String(objectives?.captureLimit ?? 0))
    for (const [name, value] of [
      ["WinningTeamLabel", winning],
      ["AdvancingTeamLabel", ""],
      ["WinReasonLabel", reason],
      ["DetailsLabel", ""],
      ["TopPlayersLabel", localized(winner === 2 ? "#Winpanel_RedMVPs" : "#Winpanel_BlueMVPs")],
      ["redteamname", "RED"],
      ["blueteamname", "BLU"],
      ["redteamscore", round?.redScore ?? objectives?.redScore ?? 0],
      ["blueteamscore", round?.blueScore ?? objectives?.blueScore ?? 0],
    ] as const) {
      this.#objectiveValue(`ctf-win:${name}`, String(value), { kind: "set-dialog-variable", panel, name, value })
    }
  }

  #objectiveValue(identity: string, value: string, operation: VguiOperation): void {
    if (this.#publishedValues.get(identity) === value) return
    apply(this.#runtime, operation)
    this.#publishedValues.set(identity, value)
  }

  #publishObjectives(objectives: CaptureObjectives, player: number, team: Tf2Team, tick: bigint): void {
    const panels = this.#initializeObjectives()
    const carried = objectives.flags.find((flag) => flag.carrier === player)
    const visible = (panel: VguiPanelId, value: boolean, name: string): void => {
      this.#objectiveValue(`ctf-visible:${name}`, String(value), { kind: "set-panel-state", panel, visible: value })
    }
    const image = (panel: VguiPanelId, value: string, name: string): void => {
      this.#objectiveValue(`ctf-image:${name}`, value, { kind: "mutate-control", panel, mutation: { image: value } })
    }
    visible(panels.root, true, "root")
    for (const flag of objectives.flags) {
      const red = flag.team === 2
      const status = red ? panels.redStatus : panels.blueStatus
      const panel = red ? panels.redFlag : panels.blueFlag
      const name = red ? "red" : "blue"
      visible(panel, !carried && (!flag.disabled || flag.visibleWhenDisabled), name)
      image(status, FLAG_STATUS_IMAGES[flag.status]!, `${name}-status`)
    }
    visible(panels.carried, carried !== undefined, "carried")
    visible(panels.captureArrow, carried !== undefined, "capture-arrow")
    if (this.#objectiveCarrying !== (carried !== undefined)) {
      this.#objectiveCarrying = carried !== undefined
      visible(panels.outline, true, "outline")
      try {
        apply(this.#runtime, { kind: "start-animation-sequence", parent: 1, sequence: "FlagOutline", cancelable: true })
        this.#animationTrace.push(`${tick}:ctf:viewport:FlagOutline`)
      } catch {
        this.#diagnostic("AnimationUnavailable", "viewport:FlagOutline")
      }
    }
    if (carried) {
      const enemy = team === 2 ? "blue" : "red"
      image(panels.carried, `${carried.icon}_${enemy}`, "carried")
      image(panels.captureArrow, `../hud/objectives_flagpanel_compass_${team === 2 ? "red" : "blue"}`, "capture-arrow")
    }
    const limited = objectives.captureLimit > 0
    visible(panels.playingTo, limited, "playing-to")
    visible(panels.playingToBackground, limited, "playing-to-background")
    for (const [name, value] of [
      ["redscore", limited ? objectives.redCaptures : objectives.redScore],
      ["bluescore", limited ? objectives.blueCaptures : objectives.blueScore],
      ["rounds", objectives.captureLimit],
    ] as const) {
      this.#objectiveValue(`ctf-variable:${name}`, String(value), { kind: "set-dialog-variable", panel: panels.panel, name, value })
    }
    for (const event of objectives.events) {
      if (event.kind !== 4 || event.team !== team || event.player === player) continue
      const filename = NOTIFICATION_FILES[event.detail]
      if (!filename) throw new Error(`TF2 CTF HUD notification ${event.detail} is invalid`)
      const resource = this.#resources.panelDocument(`resource/ui/notifications/notify_${filename}_${team === 2 ? "red" : "blue"}.res`)
      const root = resource.roots[0]
      const background = root?.children.find((child) => child.name === "Notification_Background")
      const label = root?.children.find((child) => child.name === "Notification_Label")
      const backgroundImage = background?.children.find((child) => child.name.toLowerCase() === "image")?.value
      const token = label?.children.find((child) => child.name.toLowerCase() === "labeltext")?.value
      if (!backgroundImage || !token) throw new Error(`TF2 CTF HUD notification ${filename} is incomplete`)
      image(panels.notificationBackground, backgroundImage, "notification-background")
      const localized = this.#resources.localization.tokens.find((item) => item.name.toLowerCase() === token.slice(1).toLowerCase())?.value ?? token
      this.#objectiveValue("ctf-notification-label", localized, { kind: "mutate-control", panel: panels.notificationLabel, mutation: { text: localized } })
      this.#notificationDeadline = tick + 200n
      visible(panels.notification, true, "notification")
    }
    if (tick >= this.#notificationDeadline) visible(panels.notification, false, "notification")
  }

  #diagnostic(code: Tf2HudIntegrationDiagnostic["code"], subject: string): void {
    const identity = `${code}:${subject}`
    if (this.#diagnosticSubjects.has(identity) || this.#diagnostics.length >= LIMITS.maxDiagnostics) return
    this.#diagnosticSubjects.add(identity)
    this.#diagnostics.push(Object.freeze({ code, subject }))
  }

  #captureBaseBounds(panels = this.#runtime.snapshot().panels): void {
    for (const panel of panels) this.#baseBounds.set(panel.name, panel.bounds)
  }

  #value(value: Tf2HudPanelValue): void {
    const panel = this.#panels.get(value.panel.toLowerCase()) ?? null
    if (panel === null) {
      this.#diagnostic("PanelUnavailable", `${value.kind}:${value.panel}`)
      return
    }
    const identity = value.kind === "dialog-variable"
      ? `${value.kind}:${value.panel}:${value.variable}`
      : value.kind === "scalar" || value.kind === "color"
        ? `${value.kind}:${value.panel}:${value.property}`
        : `${value.kind}:${value.panel}`
    const fingerprint = JSON.stringify(value.value)
    if (this.#publishedValues.get(identity) === fingerprint) return
    if (value.kind === "visible") {
      apply(this.#runtime, { kind: "set-panel-state", panel, visible: value.value })
      this.#publishedValues.set(identity, fingerprint)
      return
    }
    if (value.value.kind === "unavailable") {
      this.#diagnostic("ValueUnavailable", `${value.kind}:${value.panel}:${value.value.reason}`)
      return
    }
    if (value.kind === "dialog-variable") {
      const available = value.value.value
      const rendered = typeof available === "object"
        ? (this.#localization.get(available.token.toLowerCase()) ?? available.token)
          .replace(/%s([1-9])/gu, (_match, index: string) => String(available.parameters[Number(index) - 1] ?? ""))
        : available
      if (value.panel === "ItemEffectMeterLabel" && value.variable === "labelText") {
        if (!this.#cloakLabel) throw new Error("Authored Spy cloak localization is unavailable")
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: { text: this.#cloakLabel } })
      } else {
        apply(this.#runtime, { kind: "set-dialog-variable", panel, name: value.variable, value: rendered })
      }
    } else if (value.kind === "image") {
      try { apply(this.#runtime, { kind: "mutate-control", panel, mutation: { image: value.value.value } }) }
      catch { this.#diagnostic("ValueUnavailable", `image:${value.panel}:${value.value.value}`); return }
    } else if (value.kind === "color") {
      const color = Object.freeze([...value.value.value]) as readonly [number, number, number, number]
      apply(this.#runtime, { kind: "mutate-control", panel, mutation: value.property.toLowerCase() === "drawcolor" ? { drawColor: color } : { foregroundColor: color } })
    } else if (value.kind === "scalar") {
      if (value.property === "fill") {
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: {
          imageFill: value.value.value,
          image: value.value.value <= 0 ? "hud/health_dead" : "hud/health_color",
        } })
      } else if (value.property === "progress") {
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: { progress: value.value.value } })
      } else if (value.property === "boundsAdjustment") {
        const base = this.#baseBounds.get(value.panel)
        if (!base) {
          this.#diagnostics.push(Object.freeze({ code: "PanelUnavailable", subject: `bounds:${value.panel}` }))
          return
        }
        const amount = value.value.value
        apply(this.#runtime, { kind: "set-bounds", panel, bounds: { x: base.x - amount, y: base.y - amount, width: base.width + amount * 2, height: base.height + amount * 2 } })
      } else {
        apply(this.#runtime, { kind: "mutate-control", panel, mutation: { scalarProperties: { [value.property]: value.value.value } } })
      }
    }
    this.#publishedValues.set(identity, fingerprint)
  }

  #scoreboardColumnWidth(name: string): number {
    const value = Number(this.#scoreboardProperties.get(name))
    if (!Number.isSafeInteger(value) || value < 0) throw new Error(`TF2 scoreboard authored width is invalid: ${name}`)
    return Math.trunc(value * this.#viewport.height / 480)
  }

  #scoreboardPlayer(player: Tf2ScoreboardPlayer, pingAsText: boolean): VguiSectionedListItem {
    const dead = player.alive ? "" : "_d"
    const cells: Record<string, string | Readonly<{ kind: "image"; image: string }>> = {
      medal: "",
      avatar: "",
      spacer: "",
      name: player.connection === "connected"
        ? player.name
        : player.connection === "disconnected" ? "#TF_MM_PlayerLostConnection" : "#TF_MM_PlayerConnecting",
      killstreak: player.killstreak ? String(player.killstreak) : "",
      killstreak_image: "",
      dominating: "",
      nemesis: "",
      score: String(player.score),
      class: player.class.kind === "available"
        ? Object.freeze({ kind: "image", image: `${TF2_SCOREBOARD_CLASS_IMAGES[player.class.value - 1]}${dead}` })
        : "",
      ping: "",
    }
    if (player.ping.kind === "available") {
      const ping = player.ping.value
      if (pingAsText) cells.ping = ping === "bot" ? "#TF_Scoreboard_Bot" : ping > 0 ? String(ping) : ""
      else if (ping === "bot") cells.ping = Object.freeze({
        kind: "image",
        image: `../hud/scoreboard_ping_bot_${player.team === 2 ? "red" : "blue"}${dead}`,
      })
      else if (ping > 0) cells.ping = Object.freeze({
        kind: "image",
        image: `../hud/scoreboard_ping_${ping < 125 ? "low" : ping < 200 ? "med" : ping < 275 ? "high" : "very_high"}${dead}`,
      })
    }
    const local = player.identity === 1
    const foregroundColor = Object.freeze(player.alive
      ? player.team === 2 ? [255, 64, 64, 255] : [153, 204, 255, 255]
      : player.team === 2
        ? local ? [182, 75, 75, 255] : [135, 83, 83, 255]
        : local ? [123, 153, 187, 255] : [81, 97, 129, 255]) as readonly [number, number, number, number]
    return Object.freeze({
      id: player.identity,
      section: 0,
      cells: Object.freeze(cells),
      enabled: true,
      foregroundColor,
      backgroundColor: Object.freeze([0, 0, 0, 80]) as readonly [number, number, number, number],
    })
  }

  #publishScoreboard(value: Tf2HudScoreboard): void {
    const fingerprint = JSON.stringify(value)
    if (this.#scoreboardFingerprint === fingerprint) return
    this.#scoreboardFingerprint = fingerprint
    const root = this.#panels.get("scoreinfo")
    if (root === undefined) throw new Error("TF2 scoreboard authored root is absent")
    const widths = {
      medal: this.#scoreboardColumnWidth("medal_column_width"),
      avatar: this.#scoreboardColumnWidth("avatar_width"),
      spacer: this.#scoreboardColumnWidth("spacer"),
      killstreak: this.#scoreboardColumnWidth("killstreak_width"),
      killstreakImage: this.#scoreboardColumnWidth("killstreak_image_width"),
      nemesis: this.#scoreboardColumnWidth("nemesis_width"),
      score: this.#scoreboardColumnWidth("score_width"),
      class: this.#scoreboardColumnWidth("class_width"),
      ping: this.#scoreboardColumnWidth("ping_width"),
    }
    for (const [name, team] of [["RedPlayerList", 2], ["BluePlayerList", 3]] as const) {
      const id = this.#panels.get(name.toLowerCase())
      if (id === undefined) throw new Error(`TF2 scoreboard authored list is absent: ${name}`)
      const panel = this.#runtime.snapshotPanels([id])[0]!
      const reserved = widths.medal + widths.avatar + widths.spacer + widths.killstreak
        + widths.killstreakImage + widths.nemesis * 2 + widths.score + widths.class + widths.ping + 10
      const nameWidth = Math.max(0, panel.bounds.width - reserved)
      const image = 0x02, center = 0x08, right = 0x10
      const columns = [
        { name: "medal", text: "", flags: image | center, width: widths.medal },
        { name: "avatar", text: "", flags: image, width: widths.avatar },
        { name: "spacer", text: "", flags: 0, width: widths.spacer },
        { name: "name", text: "#TF_Scoreboard_Name", flags: 0, width: nameWidth },
        { name: "killstreak", text: "", flags: right, width: widths.killstreak },
        { name: "killstreak_image", text: "", flags: image, width: widths.killstreakImage },
        { name: "dominating", text: "", flags: image | center, width: widths.nemesis },
        { name: "nemesis", text: "", flags: image | center, width: widths.nemesis },
        { name: "score", text: "#TF_Scoreboard_Score", flags: right, width: widths.score },
        { name: "class", text: "", flags: image | right, width: widths.class },
        { name: "ping", text: value.pingAsText ? "#TF_Scoreboard_Ping" : "", flags: value.pingAsText ? right : image | right, width: widths.ping },
      ]
      const sectionedItems = value.players.filter((player) => player.team === team)
        .map((player) => this.#scoreboardPlayer(player, value.pingAsText))
      const selected = sectionedItems.findIndex((player) => value.selectedPlayer.kind === "available" && player.id === value.selectedPlayer.value)
      apply(this.#runtime, { kind: "mutate-control", panel: id, mutation: {
        sections: [Object.freeze({ id: 0, name: "", alwaysVisible: true, minimumHeight: 0, columns: Object.freeze(columns) })],
        sectionedItems,
        activeIndex: selected >= 0 ? selected : null,
      } })
    }
    const spectators = value.spectators.length === 0 ? "" : (
      this.#localization.get((value.spectators.length === 1 ? "#ScoreBoard_Spectator" : "#ScoreBoard_Spectators").toLowerCase()) ?? ""
    ).replace("%s1", String(value.spectators.length)).replace("%s2", value.spectators.join(", "))
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "spectators", value: spectators })
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "waitingtoplay", value: value.waitingToPlay.join(", ") })
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "mapname", value: value.mapName })
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "server", value: "" })
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "servertime", value: "" })
    const local = value.players.find((player) => player.identity === 1)
    apply(this.#runtime, { kind: "set-dialog-variable", panel: root, name: "playername", value: local?.name ?? "" })
    const stats = this.#panels.get("localplayerstatspanel")
    if (stats !== undefined) {
      const counters = local?.counters.kind === "available" ? local.counters.value : null
      for (const name of ["kills", "deaths", "assists", "destruction", "captures", "defenses", "dominations", "revenge", "healing", "invulns", "teleports", "headshots", "backstabs", "bonus", "support", "damage"] as const) {
        apply(this.#runtime, { kind: "set-dialog-variable", panel: stats, name, value: counters?.[name] ?? "" })
      }
      apply(this.#runtime, { kind: "set-dialog-variable", panel: stats, name: "gametype", value: value.gameType.kind === "available" ? this.#localization.get(value.gameType.value.toLowerCase()) ?? value.gameType.value : "" })
    }
  }

  #applyValues(binding: Tf2HudBinding): void {
    for (const value of binding.values) this.#value(value)
    if (binding.scoreboard.kind === "available") this.#publishScoreboard(binding.scoreboard.value)
    this.#crosshair.publish(binding, this.#viewport)
  }

  publish(publication: SessionSimulationPublication, context: SessionHudContext): Tf2HudBinding {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    return this.#runtime.deferPresentation(() => {
    const adapted = adaptSessionHud(this.#previous, publication, context)
    const binding = bindTf2Hud(adapted)
    this.#applyValues(binding)
    const scoped = publication.snapshot.class === 2 && publication.snapshot.weapon === 12
      && (publication.snapshot.conditions[0] & 2) !== 0
    if (scoped) {
      const state = publication.snapshot.loadout.find(value => value.weapon === 12) as { chargedDamage?: number } | undefined
      if (!state || state.chargedDamage === undefined) throw new Error("TF2 Sniper scope has no authoritative charge")
      this.#scope.publish(true, state.chargedDamage, this.#viewport)
    } else {
      this.#scope.hide()
    }
    const round = publication.snapshot.round
    if (round) this.#publishRound(round, publication.snapshot.team)
    const objectives = publication.snapshot.objectives
    if (objectives) {
      this.#publishObjectives(objectives, context.playerIdentity, publication.snapshot.team, publication.snapshot.tick)
    } else if (this.#objective) {
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#objective.root, visible: false })
      apply(this.#runtime, { kind: "set-panel-state", panel: this.#objective.notification, visible: false })
    }
    this.#publishWinPanel(round, objectives ?? undefined)
    for (let index = this.#deathNotices.length - 1; index >= 0; index -= 1) {
      const notice = this.#deathNotices[index]!
      if (notice.expires <= publication.snapshot.tick) {
        apply(this.#runtime, { kind: "delete-panel", panel: notice.panel, deferred: false })
        this.#deathNotices.splice(index, 1)
      }
    }
    for (const command of binding.commands) {
      if (command.kind !== "killfeed-notice") continue
      const parent = this.#panels.get("huddeathnotice")
      if (parent === undefined) continue
      if (this.#deathNotices.length === 4) {
        apply(this.#runtime, { kind: "delete-panel", panel: this.#deathNotices.shift()!.panel, deferred: false })
      }
      const label = apply(this.#runtime, { kind: "create-panel", parent, control: "Label",
        name: `DeathNotice${command.tick}_${command.ordinal}` })!
      apply(this.#runtime, { kind: "set-panel-state", panel: parent, visible: true })
      apply(this.#runtime, { kind: "set-bounds", panel: label, bounds: { x: 0, y: this.#deathNotices.length * 20,
        width: Math.min(this.#viewport.width, 420), height: 28 } })
      apply(this.#runtime, { kind: "mutate-control", panel: label, mutation: {
        text: `${command.notice.killer.name}  ${command.notice.weaponIcon.kind === "available" ? command.notice.weaponIcon.value : ""}  ${command.notice.victim.name}` } })
      this.#deathNotices.push({ panel: label, expires: command.tick + BigInt(command.notice.localPlayerInvolved ? 800 : 400) })
    }
    const medicRoot = find(this.#runtime, "HudMedicCharge")
    if (medicRoot !== null) {
      const source = publication.snapshot as typeof publication.snapshot & { medigunCharge: number; medigunReleasing: boolean }
      const visible = source.class === 5 && source.lifecycle === 1 && (source.weapon === 20 || source.weapon === 21)
      apply(this.#runtime, { kind: "set-panel-state", panel: medicRoot, visible })
      if (visible) {
        const charge = source.medigunCharge
        if (!Number.isFinite(charge) || charge < 0 || charge > 1) throw new Error("Medi Gun HUD charge is not authoritative")
        apply(this.#runtime, { kind: "set-dialog-variable", panel: medicRoot, name: "charge", value: Math.floor(charge * 100) })
        const meter = find(this.#runtime, "ChargeMeter", medicRoot)
        if (meter !== null) apply(this.#runtime, { kind: "mutate-control", panel: meter, mutation: { scalarProperties: { progress: charge } } })
        const background = find(this.#runtime, "Background", medicRoot)
        if (background !== null) apply(this.#runtime, { kind: "mutate-control", panel: background, mutation: { image: source.team === 2 ? "../hud/medic_charge_red_bg" : "../hud/medic_charge_blue_bg" } })
        for (const name of ["IndividualChargesLabel", "ChargeMeter1", "ChargeMeter2", "ChargeMeter3", "ChargeMeter4", "ResistIcon"]) {
          const panel = find(this.#runtime, name, medicRoot)
          if (panel !== null) apply(this.#runtime, { kind: "set-panel-state", panel, visible: false })
        }
      }
    }
    for (const animation of binding.animations) {
      const parent = animation.target === "viewport" ? 1 : find(this.#runtime, animation.target)
      if (parent === null) {
        this.#diagnostic("AnimationUnavailable", `${animation.target}:${animation.sequence}`)
        continue
      }
      try {
        apply(this.#runtime, { kind: "start-animation-sequence", parent, sequence: animation.sequence, cancelable: true })
        this.#animationTrace.push(`${animation.tick}:${animation.ordinal}:${animation.target}:${animation.sequence}`)
      } catch {
        this.#diagnostic("AnimationUnavailable", `${animation.target}:${animation.sequence}`)
      }
    }
    this.#previous = tf2HudAvailable(binding.facts)
    this.#binding = binding
    return binding
    })
  }

  action(action: Tf2HudAction): Tf2HudAvailability<Tf2HudCommand> {
    if (!this.#binding) return tf2HudUnavailable("initial")
    const command = bindTf2HudAction(this.#binding.facts, action)
    if (command.kind === "available") this.#onCommand(command.value)
    return command
  }

  frame(timeSeconds: number): void { apply(this.#runtime, { kind: "frame", timeSeconds }) }
  setViewport(viewport: VguiViewport): void {
    if (viewport.width === this.#viewport.width
      && viewport.height === this.#viewport.height
      && viewport.devicePixelRatio === this.#viewport.devicePixelRatio) return
    this.#runtime.deferPresentation(() => {
      apply(this.#runtime, { kind: "set-viewport", viewport })
      this.#viewport = Object.freeze({ ...viewport })
      this.#captureBaseBounds()
      if (this.#binding) {
        this.#publishedValues.clear()
        this.#scoreboardFingerprint = ""
        this.#applyValues(this.#binding)
      }
    })
  }
  probe(): Tf2HudIntegrationProbe {
    const panels = [
      "PlayerStatusHealthImage", "HudWeaponAmmo", "HudWeaponAmmoBG", "modelpanel0",
      "PlayerStatusClassImage", "PlayerStatusClassImageBG", "classmodelpanel", "classmodelpanelBG",
      "PlayerStatusSpyImage", "PlayerStatusSpyOutlineImage", "PlayerStatus_WheelOfDoom", "scoreinfo",
      "HudObjectiveStatus", "ObjectiveStatusFlagPanel", "BlueFlag", "RedFlag", "CarriedImage", "CaptureFlag", "BlueScore", "RedScore", "PlayingTo", "NotificationPanel", "Notification_Label", "WinPanel", "WinningTeamLabel", "WinReasonLabel",
      "scoreinfo", "RedPlayerList", "BluePlayerList", "Spectators", "Kills", "Deaths",
      ...TF2_GROUPED_CONDITION_PANELS.map((item) => item.panel),
      ...TF2_INDEPENDENT_CONDITION_PANELS.map((item) => item.panel),
    ]
      .map((name) => this.#panels.get(name.toLowerCase()))
      .filter((panel): panel is VguiPanelId => panel !== undefined)
    return Object.freeze({
      panels: this.#runtime.snapshotPanels(panels),
      animationTrace: Object.freeze([...this.#animationTrace]),
    })
  }
  snapshot(): Tf2HudIntegrationSnapshot {
    return Object.freeze({
      binding: this.#binding,
      vgui: this.#runtime.snapshot(),
      diagnostics: Object.freeze([...this.#diagnostics]),
      animationTrace: Object.freeze([...this.#animationTrace]),
    })
  }
  setPlayerClassUsePlayerModel(value: boolean): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    if (typeof value !== "boolean") throw new Error("TF2 HUD player-model setting is invalid")
    const current = this.#binding
    if (!current || current.facts.player.kind !== "available") return
    const currentPlayer = current.facts.player.value
    this.#runtime.deferPresentation(() => {
      const player = Object.freeze({ ...currentPlayer, playerClassUsePlayerModel: value })
      const snapshot: Tf2HudSnapshot = Object.freeze({ ...current.facts, player: tf2HudAvailable(player) })
      const binding = bindTf2Hud(Object.freeze({
        previous: tf2HudUnavailable<Tf2HudSnapshot>("replay-discontinuity"),
        snapshot,
        events: Object.freeze([]),
      }))
      this.#publishedValues.clear()
      this.#applyValues(binding)
      this.#previous = tf2HudAvailable(binding.facts)
      this.#binding = binding
    })
  }
  setCrosshair(value: Tf2HudCrosshair): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    const current = this.#binding
    if (!current || current.facts.player.kind !== "available") return
    const previousPlayer = current.facts.player.value
    this.#runtime.deferPresentation(() => {
      const player = Object.freeze({ ...previousPlayer, crosshair: tf2HudAvailable(value) })
      const snapshot: Tf2HudSnapshot = Object.freeze({ ...current.facts, player: tf2HudAvailable(player) })
      const binding = bindTf2Hud(Object.freeze({
        previous: tf2HudUnavailable<Tf2HudSnapshot>("replay-discontinuity"),
        snapshot,
        events: Object.freeze([]),
      }))
      this.#applyValues(binding)
      this.#previous = tf2HudAvailable(binding.facts)
      this.#binding = binding
    })
  }

  setScoreboardVisibility(visible: boolean): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    const current = this.#binding
    if (!current || current.scoreboard.kind !== "available" || current.scoreboard.value.visible === visible) return
    this.#runtime.deferPresentation(() => {
      const scoreboard = tf2HudAvailable(Object.freeze({ ...current.scoreboard.value, visible }))
      const snapshot: Tf2HudSnapshot = Object.freeze({ ...current.facts, scoreboard })
      const binding = bindTf2Hud(Object.freeze({
        previous: tf2HudUnavailable<Tf2HudSnapshot>("replay-discontinuity"),
        snapshot,
        events: Object.freeze([]),
      }))
      this.#applyValues(binding)
      this.#previous = tf2HudAvailable(binding.facts)
      this.#binding = binding
    })
  }

  reset(reason: "map-replaced" | "disconnect"): void {
    if (this.#destroyed) throw new Error("TF2 HUD integration is destroyed")
    void reason
    this.#runtime.deferPresentation(() => {
      const unavailable = tf2HudUnavailable<never>("replay-discontinuity")
      const snapshot: Tf2HudSnapshot = Object.freeze({
        tick: 0n,
        player: unavailable,
        scoreboard: unavailable,
        freezePanel: unavailable,
      })
      const binding = bindTf2Hud(Object.freeze({ previous: unavailable, snapshot, events: Object.freeze([]) }))
      this.#publishedValues.clear()
      this.#scoreboardFingerprint = ""
      this.#applyValues(binding)
      this.#previous = unavailable
      this.#binding = null
      this.#scope.hide()
      this.#animationTrace.length = 0
      if (this.#objective) {
        apply(this.#runtime, { kind: "set-panel-state", panel: this.#objective.root, visible: false })
        apply(this.#runtime, { kind: "set-panel-state", panel: this.#objective.notification, visible: false })
      }
      if (this.#winPanel !== undefined) {
        apply(this.#runtime, { kind: "set-panel-state", panel: this.#winPanel, visible: false })
      }
      if (this.#roundPanels) {
        apply(this.#runtime, { kind: "set-panel-state", panel: this.#roundPanels.match, visible: false })
        apply(this.#runtime, { kind: "set-panel-state", panel: this.#roundPanels.waiting, visible: false })
      }
      this.#notificationDeadline = 0n
      this.#objectiveCarrying = false
      for (const notice of this.#deathNotices) apply(this.#runtime, { kind: "delete-panel", panel: notice.panel, deferred: false })
      this.#deathNotices.length = 0
    })
  }
  destroy(): void {
    if (this.#destroyed) return
    this.#destroyed = true
    this.#crosshair.destroy()
    this.#scope.destroy()
    apply(this.#runtime, { kind: "destroy" })
  }
}

export function initializeTf2HudIntegration(request: Tf2HudIntegrationRequest): Tf2HudIntegration {
  return Object.freeze(new Integration(request))
}
