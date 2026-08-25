import { configuredTf2UiResourceInput } from "./configured.generated"
import { TF2_CONTENT_BUILD } from "../content-build"
import {
  tf2UiResourceBounds,
  type Tf2UiCommandCategory,
  type Tf2UiAdvancedOption,
  type Tf2UiKeyboardAction,
  type Tf2UiCommandDescriptor,
  type Tf2UiCondition,
  type Tf2UiControlDescriptor,
  type Tf2UiBorderDescriptor,
  type Tf2UiDependency,
  type Tf2UiFontDescriptor,
  type Tf2UiImageDescriptor,
  type Tf2UiLocalizationToken,
  type Tf2UiOwner,
  type Tf2UiPropertyDescriptor,
  type Tf2UiProvider,
  type Tf2UiResourceDescriptor,
  type Tf2UiResourceDiagnosticCode,
  type Tf2UiResourceDomain,
  type Tf2UiResourceNode,
  type Tf2UiResourceResolution,
  type Tf2UiResourceSource,
  type Tf2UiSchemeDescriptor,
  type Tf2UiSchemeEntry,
} from "./types"

type RecordValue = Record<string, unknown>

const SHA256 = /^[0-9a-f]{64}$/u
const LOGICAL_PATH = /^[a-z0-9][a-z0-9./_-]{0,1023}$/u
const IDENTITY = /^[a-z0-9][a-z0-9._-]{0,255}$/u
const DOMAINS = new Set<Tf2UiResourceDomain>([
  "main-menu",
  "loading",
  "scheme",
  "scheme-base",
  "hud",
  "class-selection",
  "team-selection",
  "animation-manifest",
  "animation-script",
  "options",
  "localization",
])
const ALLOWED_MISSING_SOURCES = new Set([
  "resource/loadingdialogdualprogress.res",
  "cfg/user.scr",
])
const utf8 = new TextEncoder()

const VGUI_CONTROLS = new Set([
  "Button",
  "CheckButton",
  "ComboBox",
  "ContinuousProgressBar",
  "Divider",
  "EditablePanel",
  "Frame",
  "FrameSystemButton",
  "HTML",
  "ImagePanel",
  "Label",
  "Menu",
  "Panel",
  "ProgressBar",
  "ScalableImagePanel",
  "ScrollBar",
  "ScrollableEditablePanel",
  "SectionedListPanel",
  "TextEntry",
  "URLLabel",
])

const SETTINGS_CONTROLS = new Set([
  "BuildModeDialog",
  "CCvarNegateCheckButton",
  "CCvarSlider",
  "CCvarToggleCheckButton",
  "CGammaDialog",
  "CLabeledCommandComboBox",
  "COptionsSubMultiplayer",
  "COptionsSubVideoAdvancedDlg",
])

const GAMEUI_CONTROLS = new Set(["CLoadingDialog", "CPanelListPanel", "URLButton"])

const GENERIC_PROPERTIES = new Set([
  "actionsignallevel", "alpha", "armedbgcolor_override", "armedfgcolor_override", "autoresize",
  "auto_wide_tocontents", "background", "bgcolor_override", "border", "brighttext", "centerwrap",
  "command", "controlname", "default", "defaultbgcolor_override", "defaultfgcolor_override",
  "depressedfgcolor_override", "dulltext", "editable", "enabled", "fgcolor", "fgcolor_override",
  "fieldname", "font", "image", "image2", "keyboardinputenabled", "labeltext", "lefttext",
  "maxchars", "mouseinputenabled", "navactivate", "navdown", "navleft", "navright", "navtorelay",
  "navup", "paintbackground", "paintbackgroundtype", "paintborder", "pincorner",
  "pin_corner_to_sibling", "pin_to_sibling_corner", "proportionaltoparent", "righttext",
  "roundedcorners", "scaleimage", "selected", "settitlebarvisible", "sound_depressed",
  "sound_released", "tabposition", "tall", "textalignment", "texthidden", "textinsetx", "textinsety",
  "tileimage", "title", "tooltip", "tooltiptext", "unicode", "use_proportional_insets", "useparentbg",
  "visible", "wide", "wrap", "xpos", "ypos", "zpos",
])

const commandClassifications: Readonly<Record<string, readonly [Tf2UiCommandCategory, Tf2UiOwner]>> = Object.freeze({
  "%button_command%": ["service", "service"],
  Advanced: ["application", "settings"],
  Apply: ["application", "settings"],
  Cancel: ["application", "application"],
  ChangeKey: ["application", "settings"],
  ClearKey: ["application", "settings"],
  Close: ["application", "application"],
  Defaults: ["application", "settings"],
  ImportSprayImage: ["application", "application"],
  Login: ["service", "service"],
  MinimizeToSysTray: ["unsupported", "unsupported"],
  OK: ["application", "application"],
  Ok: ["application", "application"],
  OpenAchievementsDialog: ["service", "service"],
  OpenLoadSingleplayerCommentaryDialog: ["unsupported", "unsupported"],
  OpenMutePlayerDialog: ["application", "application"],
  OpenOptionsDialog: ["application", "settings"],
  OpenReportPlayerDialog: ["service", "service"],
  ShowThirdPartyAudioCredits: ["application", "application"],
  TestMicrophone: ["application", "application"],
  TestSpeakers: ["application", "application"],
  callvote: ["gameplay", "tf2"],
  cancelmenu: ["gameplay", "tf2"],
  "jointeam auto": ["gameplay", "tf2"],
  "jointeam blue": ["gameplay", "tf2"],
  "jointeam red": ["gameplay", "tf2"],
  "jointeam spectate": ["gameplay", "tf2"],
  comp_access_info: ["service", "service"],
  create_server: ["gameplay", "application"],
  "engine OpenSteamWorkshopDialog": ["service", "service"],
  "engine bug": ["unsupported", "unsupported"],
  "engine cl_coach_find_coach": ["service", "service"],
  "engine cl_coach_toggle": ["service", "service"],
  "engine open_charinfo": ["service", "service"],
  "engine open_store": ["service", "service"],
  "engine replay_reloadbrowser": ["unsupported", "unsupported"],
  "engine vr_toggle": ["unsupported", "unsupported"],
  exitreplayeditor: ["gameplay", "tf2"],
  find_game: ["service", "service"],
  join_party_match: ["service", "service"],
  leave_queue: ["service", "service"],
  manage_queues: ["service", "service"],
  motd_hide: ["service", "service"],
  motd_next: ["service", "service"],
  motd_prev: ["service", "service"],
  motd_show: ["service", "service"],
  motd_viewurl: ["external", "external"],
  noti_hide: ["service", "service"],
  noti_show: ["service", "service"],
  open_rank_type_menu: ["service", "service"],
  opentf2options: ["application", "settings"],
  play_casual: ["service", "service"],
  play_community: ["service", "service"],
  play_competitive: ["service", "service"],
  play_event: ["service", "service"],
  play_mvm: ["service", "service"],
  play_training: ["unsupported", "unsupported"],
  questlog: ["service", "service"],
  queue_logo_clicked: ["service", "service"],
  quit: ["application", "application"],
  resume_game: ["gameplay", "application"],
  safemode_leave: ["application", "settings"],
  safemode_save_settings: ["application", "settings"],
  showpromocodes: ["service", "service"],
  toggle_chat: ["application", "application"],
  view_newuser_forums: ["external", "external"],
  watch_stream: ["external", "external"],
})

function object(value: unknown): value is RecordValue {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function failure(code: Tf2UiResourceDiagnosticCode, subject: string): Tf2UiResourceResolution {
  return Object.freeze({ ok: false, diagnostic: Object.freeze({ code, subject }) })
}

function textWithinBound(value: unknown): value is string {
  return typeof value === "string" && utf8.encode(value).byteLength <= tf2UiResourceBounds.maximumStringBytes
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}

function controlOwner(name: string): Tf2UiOwner | null {
  if (VGUI_CONTROLS.has(name)) return "vgui"
  if (SETTINGS_CONTROLS.has(name)) return "settings"
  if (GAMEUI_CONTROLS.has(name)) return "gameui"
  if (/^(?:C(?:AutoFittingLabel|AvatarImagePanel|CompetitiveAccessInfoPanel|CurrencyStatusPanel|CyclingAdContainerPanel|DashboardPartyMember|EmbeddedItemModelPanel|EventPlayListEntry|ExButton|ExImageButton|ExLabel|ExplanationPopup|IconPanel|ImagePanel|ItemModelPanel|MainMenuNotificationsControl|ModelPanel|PlayListEntry|PvPRankPanel|SteamFriendsListPanel|TeamMenu|TFArrowPanel|TFBadgePanel|TFClientScoreBoardDialog|TFHudMannVsMachineScoreboard|TFClassImage|TFClassTipsItemPanel|TFClassTipsPanel|TFFlagStatus|TFFooter|TFImagePanel|TFLogoPanel|TFParticlePanel|TFPlayerModelPanel|TFProgressBar|TFTeamButton|TFTeamStatus)|PanelListPanel)$/u.test(name)) return "tf2"
  return null
}

function domainOwner(domain: Tf2UiResourceDomain): Tf2UiOwner {
  if (domain === "loading") return "gameui"
  if (domain === "options") return "settings"
  if (domain === "scheme" || domain === "scheme-base" || domain === "animation-manifest" || domain === "animation-script" || domain === "localization") return "vgui"
  return "tf2"
}

function basePropertyName(name: string): string {
  return name.toLowerCase().replace(/_(?:hidef|lodef|minmode)$/u, "")
}

function deepFreeze<T>(value: T, seen = new Set<object>()): T {
  if (typeof value !== "object" || value === null || seen.has(value)) return value
  seen.add(value)
  for (const child of Object.values(value)) deepFreeze(child, seen)
  return Object.freeze(value)
}

type NodeWalk = Readonly<{ node: Tf2UiResourceNode; path: string; depth: number }>

function validateNodes(value: unknown, source: string):
  | Readonly<{ ok: true; roots: readonly Tf2UiResourceNode[]; count: number }>
  | Readonly<{ ok: false; code: Tf2UiResourceDiagnosticCode; subject: string }> {
  if (!Array.isArray(value)) return { ok: false, code: "MalformedResource", subject: `${source}:document` }
  const roots = value as unknown[]
  const stack = roots.map((node, index) => ({ node, path: `/${index}`, depth: 1 }))
  const seen = new Set<object>()
  let count = 0
  while (stack.length > 0) {
    const current = stack.pop()!
    if (!object(current.node) || seen.has(current.node)) return { ok: false, code: "MalformedResource", subject: `${source}${current.path}` }
    seen.add(current.node)
    count += 1
    if (count > tf2UiResourceBounds.maximumNodes || current.depth > tf2UiResourceBounds.maximumDepth) {
      return { ok: false, code: "BoundExceeded", subject: `${source}${current.path}` }
    }
    if (!textWithinBound(current.node.name) || (current.node.value !== null && !textWithinBound(current.node.value))) {
      return { ok: false, code: "MalformedResource", subject: `${source}${current.path}:text` }
    }
    if (current.node.scalarKind !== null && !textWithinBound(current.node.scalarKind)) {
      return { ok: false, code: "MalformedResource", subject: `${source}${current.path}:scalarKind` }
    }
    if (current.node.condition !== null) {
      const condition = current.node.condition
      if (
        !object(condition)
        || !textWithinBound(condition.token)
        || !textWithinBound(condition.symbol)
        || typeof condition.negated !== "boolean"
        || !["BeforeValue", "AfterScalar"].includes(condition.placement as string)
      ) return { ok: false, code: "MalformedResource", subject: `${source}${current.path}:condition` }
    }
    if (!Array.isArray(current.node.children)) return { ok: false, code: "MalformedResource", subject: `${source}${current.path}:children` }
    if (
      (current.node.value === null && current.node.scalarKind !== null)
      || (current.node.value !== null && (current.node.scalarKind === null || current.node.children.length !== 0))
    ) {
      return { ok: false, code: "MalformedResource", subject: `${source}${current.path}:value` }
    }
    current.node.children.forEach((child: unknown, index: number) => stack.push({
      node: child,
      path: `${current.path}/${index}`,
      depth: current.depth + 1,
    }))
  }
  return { ok: true, roots: value as readonly Tf2UiResourceNode[], count }
}

function sourceLedger(resources: readonly RecordValue[]): string {
  return resources.map((resource) => `${resource.logicalPath}=${resource.sha256 ?? "missing"}`).join("\n")
}

function dependency(value: unknown): Tf2UiDependency | null {
  if (!object(value) || typeof value.logicalPath !== "string" || !["found", "missing"].includes(value.outcome as string)) return null
  if (!Array.isArray(value.checkedLocations)) return null
  if (value.outcome === "found") {
    if (!Number.isSafeInteger(value.byteLength) || (value.byteLength as number) <= 0 || typeof value.sha256 !== "string" || !SHA256.test(value.sha256)) return null
  } else if (value.byteLength !== null || value.sha256 !== null) return null
  return value as unknown as Tf2UiDependency
}

function collectWalks(roots: readonly Tf2UiResourceNode[]): readonly NodeWalk[] {
  const output: NodeWalk[] = []
  const walk = (nodes: readonly Tf2UiResourceNode[], parent: string, depth: number) => {
    const occurrences = new Map<string, number>()
    for (const node of nodes) {
      const folded = node.name.toLowerCase()
      const occurrence = occurrences.get(folded) ?? 0
      occurrences.set(folded, occurrence + 1)
      const path = `${parent}/${node.name}[${occurrence}]`
      output.push({ node, path, depth })
      walk(node.children, path, depth + 1)
    }
  }
  walk(roots, "", 1)
  return output
}

export function createTf2UiResourceDescriptor(input: unknown): Tf2UiResourceResolution {
  if (!object(input) || input.schema !== "playsrc-tf2-ui-resources-v1" || input.contentBuild !== TF2_CONTENT_BUILD.contentBuild) {
    return failure("InvalidInput", "root")
  }
  if (
    typeof input.sourceLedger !== "string"
    || typeof input.sourceLedgerSha256 !== "string"
    || !SHA256.test(input.sourceLedgerSha256)
    || !Array.isArray(input.providers)
    || !Array.isArray(input.resources)
    || !Array.isArray(input.uniqueControls)
    || !Array.isArray(input.codeLocalizationTokens)
    || !Array.isArray(input.images)
    || !Array.isArray(input.fonts)
    || !Array.isArray(input.advancedOptions)
    || !Array.isArray(input.keyboardActions)
  ) return failure("InvalidInput", "root fields")
  if (input.providers.length > tf2UiResourceBounds.maximumProviders || input.resources.length > tf2UiResourceBounds.maximumSources) {
    return failure("BoundExceeded", "providers/resources")
  }

  const providerIdentities = new Set<string>()
  const providers: Tf2UiProvider[] = []
  for (let index = 0; index < input.providers.length; index += 1) {
    const value = input.providers[index]
    if (
      !object(value)
      || value.order !== index
      || typeof value.identity !== "string"
      || !IDENTITY.test(value.identity)
      || providerIdentities.has(value.identity)
      || !["vpk", "directory"].includes(value.kind as string)
      || typeof value.revision !== "string"
      || !Array.isArray(value.pathIds)
      || !value.pathIds.every(textWithinBound)
      || typeof value.configuredLocation !== "string"
      || !value.configuredLocation.startsWith("tf2-install/")
    ) return failure("InvalidInput", `provider:${index}`)
    providerIdentities.add(value.identity)
    providers.push(value as unknown as Tf2UiProvider)
  }

  const resourceIdentities = new Set<string>()
  const resources: Tf2UiResourceSource[] = []
  const rawResources: RecordValue[] = []
  let totalNodes = 0
  let retainedBytes = 0
  for (let index = 0; index < input.resources.length; index += 1) {
    const value = input.resources[index]
    if (!object(value) || !DOMAINS.has(value.domain as Tf2UiResourceDomain) || typeof value.logicalPath !== "string" || !LOGICAL_PATH.test(value.logicalPath)) {
      return failure("MalformedResource", `source:${index}`)
    }
    if (
      !Number.isSafeInteger(value.roots)
      || (value.roots as number) < 0
      || !Number.isSafeInteger(value.nodes)
      || (value.nodes as number) < 0
      || !Array.isArray(value.directives)
      || !value.directives.every(textWithinBound)
      || !Array.isArray(value.checkedLocations)
      || !value.checkedLocations.every(textWithinBound)
    ) return failure("MalformedResource", `${value.logicalPath}:metadata`)
    if (resourceIdentities.has(value.logicalPath)) return failure("DuplicateIdentity", value.logicalPath)
    resourceIdentities.add(value.logicalPath)
    rawResources.push(value)
    if (value.outcome === "missing") {
      if (!ALLOWED_MISSING_SOURCES.has(value.logicalPath)) return failure("MissingRequiredResource", value.logicalPath)
      if (value.sha256 !== null || value.byteLength !== null || value.document !== null || !Array.isArray(value.checkedLocations) || value.checkedLocations.length === 0) {
        return failure("MalformedResource", value.logicalPath)
      }
    } else if (value.outcome === "found") {
      if (
        !Number.isSafeInteger(value.byteLength)
        || (value.byteLength as number) <= 0
        || typeof value.sha256 !== "string"
        || !SHA256.test(value.sha256)
        || typeof value.providerIdentity !== "string"
        || !providerIdentities.has(value.providerIdentity)
        || typeof value.providerRevision !== "string"
        || !["Vpk", "Directory"].includes(value.providerKind as string)
      ) return failure("MalformedResource", value.logicalPath)
      retainedBytes += value.byteLength as number
      if (retainedBytes > tf2UiResourceBounds.maximumRetainedSourceBytes) return failure("BoundExceeded", "retained source bytes")
      if (value.document !== null) {
        const validated = validateNodes(value.document, value.logicalPath)
        if (!validated.ok) return failure(validated.code, validated.subject)
        totalNodes += validated.count
      } else if (value.encoding !== "opaque-producer-input") return failure("MalformedResource", `${value.logicalPath}:document`)
    } else return failure("MalformedResource", `${value.logicalPath}:outcome`)
    resources.push({
      domain: value.domain,
      logicalPath: value.logicalPath,
      outcome: value.outcome,
      byteLength: value.byteLength,
      sha256: value.sha256,
      providerIdentity: value.providerIdentity,
      providerKind: value.providerKind,
      providerRevision: value.providerRevision,
      encoding: value.encoding,
      roots: value.roots,
      nodes: value.nodes,
      directives: value.directives,
      document: value.document,
      checkedLocations: value.checkedLocations,
    } as Tf2UiResourceSource)
  }
  if (totalNodes > tf2UiResourceBounds.maximumNodes) return failure("BoundExceeded", "nodes")
  if (sourceLedger(rawResources) !== input.sourceLedger) return failure("ChangedSource", "source ledger")

  const controlCounts = new Map<string, number>()
  const properties: Tf2UiPropertyDescriptor[] = []
  const commands: Tf2UiCommandDescriptor[] = []
  const tokenOccurrences = new Map<string, { name: string; count: number }>()
  for (const source of resources) {
    if (!source.document) continue
    const walks = collectWalks(source.document)
    for (const { node, path } of walks) {
      if (node.name.toLowerCase() === "controlname" && node.value !== null) {
        controlCounts.set(node.value, (controlCounts.get(node.value) ?? 0) + 1)
      }
      if ((node.name.toLowerCase() === "command" || node.name.toLowerCase() === "button_command") && node.value !== null) {
        const classification = commandClassifications[node.value]
          ?? (/^select (?:[1-9]|12)$/u.test(node.value) || node.value === "resetclass"
            ? ["gameplay", "tf2"] as const
            : node.value === "vguicancel" || node.value === "close"
              ? ["application", "application"] as const
              : node.value === "openloadout"
                ? ["service", "service"] as const
                : undefined)
        if (!classification) return failure("UnclassifiedCommand", node.value)
        commands.push(Object.freeze({
          identity: `command-${String(commands.length + 1).padStart(4, "0")}`,
          sourceLogicalPath: source.logicalPath,
          nodePath: path,
          command: node.value,
          category: classification[0],
          capabilityOwner: classification[1],
          executable: false,
        }))
      }
      if (node.value?.startsWith("#") && node.value.length > 1) {
        const folded = node.value.slice(1).toLowerCase()
        const prior = tokenOccurrences.get(folded)
        tokenOccurrences.set(folded, { name: prior?.name ?? node.value, count: (prior?.count ?? 0) + 1 })
      }
      if (!["scheme", "scheme-base", "localization", "animation-script"].includes(source.domain)) {
        const owner = GENERIC_PROPERTIES.has(basePropertyName(node.name)) ? "vgui" : domainOwner(source.domain)
        properties.push(Object.freeze({
          identity: `property-${String(properties.length + 1).padStart(5, "0")}`,
          sourceLogicalPath: source.logicalPath,
          nodePath: path,
          name: node.name,
          kind: node.value === null ? "object" : "scalar",
          owner,
          condition: node.condition,
        }))
      }
    }
  }
  if (properties.length > tf2UiResourceBounds.maximumProperties || commands.length > tf2UiResourceBounds.maximumCommands) {
    return failure("BoundExceeded", "properties/commands")
  }

  const declaredControls = new Set((input.uniqueControls as unknown[]).filter((value): value is string => typeof value === "string"))
  for (const control of controlCounts.keys()) declaredControls.add(control)
  const controls: Tf2UiControlDescriptor[] = []
  for (const name of [...declaredControls].sort(compareText)) {
    const owner = controlOwner(name)
    if (!owner) return failure("UnclassifiedControl", name)
    controls.push(Object.freeze({
      identity: `control-${String(controls.length + 1).padStart(3, "0")}`,
      name,
      owner,
      sourceOccurrences: controlCounts.get(name) ?? 0,
    }))
  }
  if (controls.length > tf2UiResourceBounds.maximumControls) return failure("BoundExceeded", "controls")

  const localizationSources = resources.filter((source) => source.domain === "localization")
  const definitions = new Map<string, Tf2UiLocalizationToken["definitions"][number][]>()
  for (const source of localizationSources) {
    if (!source.document) continue
    for (const root of source.document) {
      for (const child of root.children) {
        if (child.name.toLowerCase() !== "tokens") continue
        for (const token of child.children) {
          if (token.value === null) return failure("MalformedResource", `${source.logicalPath}:${token.name}`)
          const folded = token.name.toLowerCase()
          const values = definitions.get(folded) ?? []
          values.push(Object.freeze({ sourceLogicalPath: source.logicalPath, value: token.value, condition: token.condition }))
          definitions.set(folded, values)
        }
      }
    }
  }
  const images: Tf2UiImageDescriptor[] = []
  for (const value of input.images) {
    if (
      !object(value)
      || typeof value.identity !== "string"
      || !textWithinBound(value.configuredValue)
      || !["content-vtf", "procedural-material", "missing-material", "missing-texture", "unsupported-pic"].includes(value.classification as string)
      || !Array.isArray(value.textures)
    ) return failure("InvalidInput", "image")
    const material = value.material === null ? null : dependency(value.material)
    if (value.material !== null && material === null) return failure("InvalidInput", `${value.identity}:material`)
    for (const texture of value.textures) {
      if (!object(texture) || dependency(texture.source) === null) return failure("InvalidInput", `${value.identity}:texture`)
    }
    images.push(Object.freeze({ ...value, owner: "vgui" }) as unknown as Tf2UiImageDescriptor)
  }
  if (images.length > tf2UiResourceBounds.maximumImages) return failure("BoundExceeded", "images")

  const fonts: Tf2UiFontDescriptor[] = []
  for (const value of input.fonts) {
    if (
      !object(value)
      || typeof value.identity !== "string"
      || !textWithinBound(value.configuredValue)
      || !["content-sfnt", "content-bitmap", "scheme-reference", "missing-font"].includes(value.classification as string)
      || (value.source !== null && dependency(value.source) === null)
    ) return failure("InvalidInput", "font")
    fonts.push(Object.freeze({ ...value, owner: "vgui" }) as unknown as Tf2UiFontDescriptor)
  }
  if (fonts.length > tf2UiResourceBounds.maximumFonts) return failure("BoundExceeded", "fonts")

  const advancedOptions: Tf2UiAdvancedOption[] = []
  const advancedIdentities = new Set<string>()
  for (const value of input.advancedOptions) {
    if (!object(value)
      || typeof value.identity !== "string" || !textWithinBound(value.identity) || advancedIdentities.has(value.identity.toLowerCase())
      || typeof value.category !== "string" || !textWithinBound(value.category)
      || typeof value.prompt !== "string" || !textWithinBound(value.prompt)
      || (value.tooltip !== null && (typeof value.tooltip !== "string" || !textWithinBound(value.tooltip)))
      || !["BOOL", "NUMBER", "STRING", "LIST", "SLIDER"].includes(value.kind as string)
      || !Array.isArray(value.choices)
      || value.choices.some((choice) => !object(choice) || typeof choice.label !== "string" || !textWithinBound(choice.label) || typeof choice.value !== "string" || !textWithinBound(choice.value))
      || (value.minimum !== null && !Number.isFinite(value.minimum))
      || (value.maximum !== null && !Number.isFinite(value.maximum))
      || typeof value.contentDefault !== "string" || !textWithinBound(value.contentDefault)) return failure("InvalidInput", "advanced option")
    if ((value.kind === "NUMBER" || value.kind === "SLIDER") !== (value.minimum !== null && value.maximum !== null)
      || ((value.kind === "LIST") !== (value.choices.length > 0))) return failure("InvalidInput", `${value.identity}:advanced shape`)
    advancedIdentities.add(value.identity.toLowerCase())
    advancedOptions.push(Object.freeze({ ...value, choices: Object.freeze(value.choices.map((choice) => Object.freeze({ ...choice }))) }) as Tf2UiAdvancedOption)
  }
  if (advancedOptions.length !== 88) return failure("ChangedSource", "advanced option count")
  const keyboardActions: Tf2UiKeyboardAction[] = []
  for (const value of input.keyboardActions) {
    if (!object(value) || !Number.isSafeInteger(value.section) || (value.section as number) < 1
      || typeof value.sectionName !== "string" || !textWithinBound(value.sectionName)
      || typeof value.binding !== "string" || !textWithinBound(value.binding)
      || typeof value.description !== "string" || !textWithinBound(value.description)) return failure("InvalidInput", "keyboard action")
    keyboardActions.push(Object.freeze({ ...value }) as Tf2UiKeyboardAction)
  }
  if (keyboardActions.length !== 70) return failure("ChangedSource", "keyboard action count")

  const addLocalizationOccurrence = (name: string): void => {
    if (!name.startsWith("#") || name.length < 2) return
    const folded = name.slice(1).toLowerCase()
    const prior = tokenOccurrences.get(folded)
    tokenOccurrences.set(folded, { name: prior?.name ?? name, count: (prior?.count ?? 0) + 1 })
  }
  if (!input.codeLocalizationTokens.every((value) => typeof value === "string" && textWithinBound(value) && value.startsWith("#"))) {
    return failure("InvalidInput", "code localization tokens")
  }
  for (const token of input.codeLocalizationTokens) addLocalizationOccurrence(token as string)
  for (const row of advancedOptions) {
    addLocalizationOccurrence(row.category)
    addLocalizationOccurrence(row.prompt)
    if (row.tooltip) addLocalizationOccurrence(row.tooltip)
    for (const choice of row.choices) addLocalizationOccurrence(choice.label)
  }
  for (const row of keyboardActions) {
    addLocalizationOccurrence(row.sectionName)
    addLocalizationOccurrence(row.description)
  }
  const localizationTokens: Tf2UiLocalizationToken[] = [...tokenOccurrences]
    .sort(([left], [right]) => compareText(left, right))
    .map(([folded, occurrence], index) => {
      const tokenDefinitions = definitions.get(folded) ?? []
      return Object.freeze({
        identity: `localization-${String(index + 1).padStart(4, "0")}`,
        name: occurrence.name,
        status: tokenDefinitions.length > 0 ? "resolved" : "missing",
        occurrences: occurrence.count,
        definitions: Object.freeze(tokenDefinitions),
        owner: "vgui" as const,
      })
    })
  if (localizationTokens.length > tf2UiResourceBounds.maximumLocalizationTokens) return failure("BoundExceeded", "localization")

  const animationManifest = resources.find((source) => source.logicalPath === "scripts/hudanimations_manifest.txt")
  if (!animationManifest?.document) return failure("MissingRequiredResource", "scripts/hudanimations_manifest.txt")
  const compositionOrder = collectWalks(animationManifest.document)
    .filter(({ node }) => node.name.toLowerCase() === "file" && node.value !== null)
    .map(({ node }) => node.value!)
  const animationScripts = compositionOrder.map((logicalPath) => resources.find((source) => source.logicalPath === logicalPath.toLowerCase()))
  if (animationScripts.some((source) => !source || source.outcome !== "found")) return failure("MissingRequiredResource", "HUD animation composition")

  const missingDependencies = [
    ...resources.filter((source) => source.outcome === "missing").map((source) => source.logicalPath),
    ...images.filter((image) => image.classification.startsWith("missing-")).map((image) => `image:${image.configuredValue}`),
    ...fonts.filter((font) => font.classification === "missing-font").map((font) => `font:${font.configuredValue}`),
    ...localizationTokens.filter((token) => token.status === "missing").map((token) => `localization:${token.name}`),
  ]

  const panels = resources
    .filter((source): source is Tf2UiResourceSource & { domain: "main-menu" | "loading" | "hud" | "class-selection" | "team-selection" | "options"; document: readonly Tf2UiResourceNode[] } =>
      ["main-menu", "loading", "hud", "class-selection", "team-selection", "options"].includes(source.domain) && source.document !== null)
    .map((source) => Object.freeze({
      identity: `panel-document-${source.logicalPath.replace(/[^a-z0-9]+/gu, "-").replace(/^-|-$/gu, "")}`,
      domain: source.domain,
      source,
      roots: source.document,
    }))

  const borders: Tf2UiBorderDescriptor[] = []
  const schemeSources = resources
    .filter((source): source is Tf2UiResourceSource & { document: readonly Tf2UiResourceNode[] } =>
      (source.domain === "scheme" || source.domain === "scheme-base") && source.document !== null)
  const malformedScheme = schemeSources.find((source) => !source.document.some((root) => root.name.toLowerCase() === "scheme"))
  if (malformedScheme) return failure("MalformedResource", `${malformedScheme.logicalPath}:scheme root`)
  const schemes: Tf2UiSchemeDescriptor[] = schemeSources
    .map((source, schemeIndex) => {
      const schemeRoot = source.document.find((root) => root.name.toLowerCase() === "scheme")!
      const entries = (sectionName: string, kind: string): Tf2UiSchemeEntry[] => {
        const output: Tf2UiSchemeEntry[] = []
        for (const section of schemeRoot.children.filter((node) => node.name.toLowerCase() === sectionName)) {
          for (const node of section.children) {
            output.push(Object.freeze({
              identity: `scheme-${String(schemeIndex + 1).padStart(2, "0")}-${kind}-${String(output.length + 1).padStart(4, "0")}`,
              name: node.name,
              node,
            }))
          }
        }
        return output
      }
      const colors = entries("colors", "color")
      const fontDefinitions = entries("fonts", "font")
      const borderEntries = entries("borders", "border")
      for (const entry of borderEntries) {
        borders.push(Object.freeze({
          identity: `border-${String(borders.length + 1).padStart(4, "0")}`,
          sourceLogicalPath: source.logicalPath,
          name: entry.name,
          node: entry.node,
          owner: "vgui",
        }))
      }
      const directory = source.logicalPath.slice(0, source.logicalPath.lastIndexOf("/") + 1)
      const baseLogicalPaths = source.directives
        .filter((directive) => directive.toLowerCase().startsWith("base:"))
        .map((directive) => {
          const target = directive.slice(directive.indexOf(":") + 1).replaceAll("\\", "/").toLowerCase()
          return target.includes("/") ? target : `${directory}${target}`
        })
      return Object.freeze({
        identity: `scheme-${String(schemeIndex + 1).padStart(2, "0")}`,
        source,
        baseLogicalPaths: Object.freeze(baseLogicalPaths),
        colors: Object.freeze(colors),
        fontDefinitions: Object.freeze(fontDefinitions),
        borders: Object.freeze(borderEntries),
      })
    })

  const descriptor: Tf2UiResourceDescriptor = {
    schema: "playsrc-tf2-ui-resources-v1",
    identity: `tf2-ui-${TF2_CONTENT_BUILD.contentBuild}-${input.sourceLedgerSha256.slice(0, 16)}`,
    game: "tf2",
    contentBuild: TF2_CONTENT_BUILD.contentBuild,
    sourceLedger: input.sourceLedger,
    providers,
    sources: resources,
    panels,
    schemes,
    borders,
    localization: {
      language: "english",
      sources: localizationSources,
      tokens: localizationTokens,
    },
    images,
    fonts,
    advancedOptions,
    keyboardActions,
    animation: {
      manifest: animationManifest,
      scripts: animationScripts as Tf2UiResourceSource[],
      compositionOrder,
    },
    controls,
    properties,
    commands,
    missingDependencies,
    conditions: {
      platforms: ["windows", "macos", "linux"],
      languages: ["english", "%language%"],
      resolutions: ["default", "minmode", "lodef", "hidef"],
      aspect: ["if_taller", "if_wider"],
      session: ["menu", "in-game", "replay"],
    },
    bounds: tf2UiResourceBounds,
  }
  return Object.freeze({ ok: true, descriptor: deepFreeze(descriptor) })
}

export function classifyTf2UiCommand(command: string): Readonly<{
  category: Tf2UiCommandCategory
  capabilityOwner: Tf2UiOwner
  executable: false
}> | null {
  const classification = commandClassifications[command]
  return classification
    ? Object.freeze({ category: classification[0], capabilityOwner: classification[1], executable: false })
    : null
}

const configuredResolution = createTf2UiResourceDescriptor(configuredTf2UiResourceInput)
if (!configuredResolution.ok) {
  throw new Error(`${configuredResolution.diagnostic.code}:${configuredResolution.diagnostic.subject}`)
}

export const tf2UiResources = configuredResolution.descriptor
