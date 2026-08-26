import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiViewport,
} from "@playsrc/vgui"
import type { BuildingSnapshot, Snapshot, Tf2BuildingObject, Tf2BuildingRequest } from "./codec"
import type { Tf2VguiResources } from "./ui-integration"

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 512,
  maxHierarchyDepth: 32,
  maxChildrenPerPanel: 256,
  maxResourceNodes: 4096,
  maxResourceDepth: 32,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4095,
  maxTextCodeUnits: 65535,
  maxDialogVariables: 256,
  maxLocalizationTokens: 4096,
  maxSchemeColors: 1024,
  maxSchemeSettings: 2048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1024,
  maxAnimationCommands: 8192,
  maxActiveAnimations: 2048,
  maxDelayedCommands: 2048,
  maxQueuedMessages: 2048,
  maxDiagnostics: 2048,
  maxDomNodes: 2048,
  maxListeners: 64,
})

export const ENGINEER_BUILDINGS: readonly Readonly<{ object: Tf2BuildingObject; name: string; cost: number; status: string }>[] = Object.freeze([
  Object.freeze({ object: Object.freeze({ kind: 2, mode: 0 }), name: "sentry", cost: 130, status: "hud_obj_sentrygun" }),
  Object.freeze({ object: Object.freeze({ kind: 0, mode: 0 }), name: "dispenser", cost: 100, status: "hud_obj_dispenser" }),
  Object.freeze({ object: Object.freeze({ kind: 1, mode: 0 }), name: "tele_entrance", cost: 50, status: "hud_obj_tele_entrance" }),
  Object.freeze({ object: Object.freeze({ kind: 1, mode: 1 }), name: "tele_exit", cost: 50, status: "hud_obj_tele_exit" }),
])

const ICONS: Readonly<Record<string, string>> = Object.freeze({
  hud_menu_bg: "eng_build_bg",
  hud_menu_dispenser_build: "eng_build_dispenser_blueprint",
  hud_menu_item_bg: "eng_build_item",
  hud_menu_item_bg_outline: "eng_sel_item_active",
  hud_menu_sentry_build: "eng_build_sentry_blueprint",
  hud_menu_tele_entrance_build: "eng_build_tele_entrance_blueprint",
  hud_menu_tele_exit_build: "eng_build_tele_exit_blueprint",
  ico_build: "ico_build",
  ico_demolish: "ico_demolish",
  ico_key_blank: "ico_key_blank",
  ico_metal: "ico_metal_mask",
  obj_status_alert_background: "eng_status_area_tele_alrt",
  obj_status_alert_background_tall: "eng_status_area_sentry_alrt",
  obj_status_background_disabled: "eng_status_area_tele_disabled",
  obj_status_background_tall_disabled: "eng_status_area_sentry_disabled",
  obj_status_dispenser: "hud_obj_status_dispenser",
  obj_status_icon_sapper: "hud_obj_status_sapper",
  obj_status_icon_wrench: "eng_status_alert_ico_wrench",
  obj_status_sentrygun_1: "hud_obj_status_sentry_1",
  obj_status_sentrygun_2: "hud_obj_status_sentry_2",
  obj_status_sentrygun_3: "hud_obj_status_sentry_3",
  obj_status_tele_entrance: "hud_obj_status_tele_entrance",
  obj_status_tele_exit: "hud_obj_status_tele_exit",
  obj_status_upgrade_1: "hud_upgrade_1",
  obj_status_upgrade_2: "hud_upgrade_2",
  obj_status_upgrade_3: "hud_upgrade_3",
})

const scalar = (node: VguiResourceNode, name: string) => node.children.find(child => child.name.toLowerCase() === name.toLowerCase())?.value ?? null
const fieldName = (node: VguiResourceNode) => scalar(node, "fieldName") ?? node.name

function merge(base: VguiResourceNode, override: VguiResourceNode): VguiResourceNode {
  const replacements = new Map(override.children.map(child => [child.name.toLowerCase(), child]))
  const children = base.children.map(child => {
    const next = replacements.get(child.name.toLowerCase())
    if (!next) return child
    replacements.delete(child.name.toLowerCase())
    return child.value === null && next.value === null ? merge(child, next) : next
  })
  return Object.freeze({ ...base, children: Object.freeze([...children, ...replacements.values()]) })
}

function iconNode(node: VguiResourceNode): VguiResourceNode {
  const isIcon = scalar(node, "ControlName") === "CIconPanel"
  const children = node.children.flatMap(child => {
    if (child.name.toLowerCase() === "font" && child.value === "HudMenuNumberFont") return [Object.freeze({ ...child, value: "Default" })]
    if (isIcon && child.name.toLowerCase() === "controlname") return [Object.freeze({ ...child, value: "ImagePanel" })]
    if (isIcon && child.name.toLowerCase() === "icon") {
      const image = child.value && ICONS[child.value.toLowerCase()]
      if (!image) return []
      return [Object.freeze({ ...child, name: "image", value: `../hud/${image}` })]
    }
    if (isIcon && child.name.toLowerCase() === "iconcolor") return [Object.freeze({ ...child, name: "drawcolor", value: child.value })]
    return [child.value === null ? iconNode(child) : child]
  })
  return Object.freeze({ ...node, children: Object.freeze(children) })
}

function apply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function document(source: VguiResourceDocument, suffix: string, children: readonly VguiResourceNode[]): VguiResourceDocument {
  return Object.freeze({ logicalIdentity: `${source.logicalIdentity}/${suffix}`.toLowerCase(), revision: source.revision,
    root: Object.freeze({ ...source.root, children: Object.freeze(children.map(iconNode)) }) })
}

function panel(runtime: VguiRuntime, parent: VguiPanelId, name: string): VguiPanelId {
  const value = runtime.snapshot().panels.find(candidate => candidate.parent === parent && candidate.name.toLowerCase() === name.toLowerCase())
  if (!value) throw new Error(`Authored Engineer panel unavailable: ${name}`)
  return value.id
}

export type Tf2EngineerPresentation = Readonly<{
  publish(snapshot: Snapshot): void
  menu(): "build" | "destroy" | null
  select(slot: number): Tf2BuildingRequest | null
  setViewport(viewport: VguiViewport): void
  frame(seconds: number): void
  destroy(): void
}>

export function initializeTf2EngineerPresentation(request: Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  reducedMotion: boolean
}>): Tf2EngineerPresentation {
  const initialized = initializeVguiRuntime({ runtimeIdentity: "tf2-engineer-buildings", root: request.root,
    rootControl: { control: "EditablePanel", name: "EngineerViewport" }, viewport: request.viewport, limits: LIMITS,
    clock: request.clock, random: request.random, scheme: request.resources.clientScheme,
    localization: request.resources.localization, animationScripts: request.resources.animations,
    customControls: request.resources.customControls, reducedMotion: request.reducedMotion, onRequest: () => {} })
  if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
  const runtime = initialized.runtime
  const selection = { activeConditions: request.resources.activeConditions, resolutionSuffixes: request.resources.resolutionSuffixes }
  let previous: Snapshot | undefined
  let statusRootId=0,accountRootId=0,accountBackgroundId=0
  let menuFingerprint=""
  let menu: "build" | "destroy" | null = null
  const menuPanels = new Map<string, VguiPanelId>()
  const cards = new Map<string, VguiPanelId>()
  const statusCards = new Map<number, VguiPanelId>()
  const statusPanels = new Map<string, VguiPanelId>()
  const statusFingerprints = new Map<number,string>()
  runtime.deferPresentation(() => {
    apply(runtime, { kind: "set-panel-state", panel: 1, proportional: true, mouseInput: false, keyboardInput: false })
    for (const name of ["HudMenuEngyBuild", "HudMenuEngyDestroy", "BuildingStatus_Engineer", "CHudAccountPanel"]) {
      apply(runtime, { kind: "create-panel", parent: 1, control: "EditablePanel", name })
    }
    const layout = request.resources.document("scripts/hudlayout.res")
    const roots = layout.root.children.filter(node => ["HudMenuEngyBuild", "HudMenuEngyDestroy", "BuildingStatus_Engineer", "CHudAccountPanel"].includes(fieldName(node)))
      .map(value => Object.freeze({ ...value, children: Object.freeze([
        Object.freeze({ name: "ControlName", value: "EditablePanel", condition: null, children: Object.freeze([]) }), ...value.children,
      ]) }))
    apply(runtime, { kind: "replace-resource", parent: 1, document: document(layout, "engineer", roots), selection })
    for (const [kind, rootName, folder] of [["build", "HudMenuEngyBuild", "build_menu"], ["destroy", "HudMenuEngyDestroy", "destroy_menu"]] as const) {
      const parent = panel(runtime, 1, rootName)
      menuPanels.set(kind, parent)
      const authored = request.resources.document(`resource/ui/${folder}/${rootName.toLowerCase()}.res`)
      apply(runtime, { kind: "replace-resource", parent, document: document(authored, "root", authored.root.children), selection })
      for (const [index, entry] of ENGINEER_BUILDINGS.entries()) {
        const variants = kind === "build" ? ["active", "already_built", "cant_afford", "unavailable"] : ["active", "inactive"]
        for (const variant of variants) {
          const panelName = `${variant}_item_${index + 1}`
          const target = panel(runtime, parent, panelName)
          const specific = request.resources.document(`resource/ui/${folder}/${entry.name}_${variant}.res`)
          let children = specific.root.children
          if (kind === "build") {
            const base = request.resources.document(`resource/ui/${folder}/base_${variant}.res`)
            const overrides = new Map(specific.root.children.map(child => [child.name.toLowerCase(), child]))
            children = base.root.children.map(child => {
              const override = overrides.get(child.name.toLowerCase())
              return override ? merge(child, override) : child
            })
          }
          apply(runtime, { kind: "replace-resource", parent: target, document: document(specific, `${variant}-${index}`, children), selection })
          apply(runtime, { kind: "set-dialog-variable", panel: target, name: "metal", value: entry.cost })
          const number = runtime.snapshot().panels.find(candidate => candidate.parent === target && candidate.name === "NumberLabel")
          if (number) apply(runtime, { kind: "set-panel-state", panel: number.id, text: String(index + 1) })
          cards.set(`${kind}:${index}:${variant}`, target)
        }
      }
      apply(runtime, { kind: "set-panel-state", panel: parent, visible: false })
    }
    const accountRoot=panel(runtime,1,"CHudAccountPanel")
    accountRootId=accountRoot
    const account=request.resources.document("resource/ui/hudaccountpanel.res")
    apply(runtime,{kind:"replace-resource",parent:accountRoot,document:document(account,"account",account.root.children.filter(value=>scalar(value,"ControlName")!==null)),selection})
    apply(runtime,{kind:"set-panel-state",panel:accountRoot,visible:false})
    accountBackgroundId=panel(runtime,accountRoot,"AccountBG")
    const statusRoot=panel(runtime,1,"BuildingStatus_Engineer")
    statusRootId=statusRoot
    let statusY=9
    const applyStatusChildren=(parent:VguiPanelId,source:VguiResourceDocument,nodes:readonly VguiResourceNode[],suffix:string):void=>{
      const blocks=nodes.filter(value=>value.value===null&&scalar(value,"ControlName")!==null)
      if(blocks.length===0)return
      const shallow=blocks.map(value=>Object.freeze({...value,children:Object.freeze(value.children.filter(child=>child.value!==null))}))
      apply(runtime,{kind:"replace-resource",parent,document:document(source,suffix,shallow),selection})
      for(const value of blocks){const id=panel(runtime,parent,fieldName(value));statusPanels.set(`${suffix}:${fieldName(value)}`,id);applyStatusChildren(id,source,value.children,`${suffix}/${fieldName(value)}`)}
    }
    for(const [index,entry] of ENGINEER_BUILDINGS.entries()){
      const source=request.resources.document(`resource/ui/${entry.status}.res`)
      const self=source.root.children.find(value=>fieldName(value)==="BuildingStatusItem")
      if(!self)throw new Error(`Authored Engineer building status root unavailable: ${entry.name}`)
      const width=Number(scalar(self,"wide")??"160"),height=Number(scalar(self,"tall")??"31")
      const card=apply(runtime,{kind:"create-panel",parent:statusRoot,control:"EditablePanel",name:`EngineerStatus_${entry.name}`})!
      const scale=request.viewport.height/480
      apply(runtime,{kind:"set-bounds",panel:card,bounds:{x:Math.trunc(9*scale),y:Math.trunc(statusY*scale),width:Math.trunc(width*scale),height:Math.trunc(height*scale)}})
      statusCards.set(index,card)
      applyStatusChildren(card,source,source.root.children.filter(value=>value!==self),`status-${index}`)
      statusY+=height
    }
    apply(runtime, { kind: "set-panel-state", panel: statusRoot, visible: false })
  })
  const show = (identity: VguiPanelId, visible: boolean) => apply(runtime, { kind: "set-panel-state", panel: identity, visible })
  return Object.freeze({
    publish(snapshot) {
      if (previous?.tick === snapshot.tick) return
      const prior=previous
      previous = snapshot
      const next = snapshot.class !== 9 ? null : snapshot.weapon === 43 ? "build" : snapshot.weapon === 44 ? "destroy" : null
      if (menu !== next) {
        if (menu) show(menuPanels.get(menu)!, false)
        if (next) show(menuPanels.get(next)!, true)
        menu = next
        request.root.dataset.engineerMenu = menu ?? "none"
      }
      if(!prior||prior.metal!==snapshot.metal)request.root.dataset.metal = String(snapshot.metal)
      if(!prior||prior.buildings.length!==snapshot.buildings.length)request.root.dataset.buildings = String(snapshot.buildings.length)
      const visible=snapshot.class===9&&snapshot.team>=2&&snapshot.lifecycle===1
      if(!prior||prior.class!==snapshot.class||prior.team!==snapshot.team||prior.lifecycle!==snapshot.lifecycle){show(statusRootId,visible);show(accountRootId,visible)}
      if(!prior||prior.metal!==snapshot.metal)apply(runtime,{kind:"set-dialog-variable",panel:accountRootId,name:"metal",value:snapshot.metal})
      if(!prior||prior.team!==snapshot.team)apply(runtime,{kind:"mutate-control",panel:accountBackgroundId,mutation:{image:`../hud/misc_ammo_area_${snapshot.team===2?"red":"blue"}`}})
      runtime.deferPresentation(()=>{
        for(const[index,entry]of ENGINEER_BUILDINGS.entries()){
          const building=snapshot.buildings.find(value=>value.object.kind===entry.object.kind&&value.object.mode===entry.object.mode)
          const fingerprint=building?`${building.phase}:${building.level}:${building.health}:${building.maximumHealth}:${building.upgradeMetal}:${building.shells}:${building.maximumShells}:${building.timesUsed}`:"absent"
          if(statusFingerprints.get(index)===fingerprint)continue
          statusFingerprints.set(index,fingerprint)
          const prefix=`status-${index}`
          const notBuilt=statusPanels.get(`${prefix}:NotBuiltPanel`),built=statusPanels.get(`${prefix}:BuiltPanel`)
          if(notBuilt)show(notBuilt,!building)
          if(built)show(built,Boolean(building))
          if(building){
            const buildingPanel=statusPanels.get(`${prefix}/BuiltPanel:BuildingPanel`),runningPanel=statusPanels.get(`${prefix}/BuiltPanel:RunningPanel`)
            if(buildingPanel)show(buildingPanel,building.phase===0||building.phase===2)
            if(runningPanel)show(runningPanel,building.phase!==0&&building.phase!==2)
            for(let level=1;level<=3;level++){const icon=statusPanels.get(`${prefix}/BuiltPanel:Icon_Upgrade_${level}`);if(icon)show(icon,building.level===level)}
            const upgrade=statusPanels.get(`${prefix}/BuiltPanel/RunningPanel:Upgrade`)
            if(upgrade)apply(runtime,{kind:"mutate-control",panel:upgrade,mutation:{progress:building.upgradeMetal/200}})
            const health=statusPanels.get(`${prefix}/BuiltPanel:Health`)
            if(health)apply(runtime,{kind:"mutate-control",panel:health,mutation:{scalarProperties:{health:building.health,maximumHealth:building.maximumHealth}}})
            apply(runtime,{kind:"set-dialog-variable",panel:statusCards.get(index)!,name:"timesused",value:building.timesUsed})
          }
        }
      })
      if (!menu) return
      const nextFingerprint=`${menu}:${snapshot.metal}:${snapshot.buildings.map(building=>`${building.object.kind}.${building.object.mode}`).join(",")}`
      if(nextFingerprint===menuFingerprint)return
      menuFingerprint=nextFingerprint
      runtime.deferPresentation(() => {
        for (const [index, entry] of ENGINEER_BUILDINGS.entries()) {
          const existing = snapshot.buildings.find(building => building.object.kind === entry.object.kind && building.object.mode === entry.object.mode)
          if (menu === "build") {
            const selected = existing ? "already_built" : snapshot.metal < entry.cost ? "cant_afford" : "active"
            for (const variant of ["active", "already_built", "cant_afford", "unavailable"]) show(cards.get(`build:${index}:${variant}`)!, variant === selected)
          } else {
            for (const variant of ["active", "inactive"]) show(cards.get(`destroy:${index}:${variant}`)!, variant === (existing ? "active" : "inactive"))
          }
        }
      })
    },
    menu: () => menu,
    select(slot) {
      const entry = ENGINEER_BUILDINGS[slot - 1]
      if (!menu || !entry || !previous) return null
      const existing = previous.buildings.some(building => building.object.kind === entry.object.kind && building.object.mode === entry.object.mode)
      if (menu === "build" && !existing && previous.metal >= entry.cost) return Object.freeze({ action: "build", object: entry.object })
      if (menu === "destroy" && existing) return Object.freeze({ action: "destroy", object: entry.object })
      return null
    },
    setViewport(viewport) { apply(runtime, { kind: "set-viewport", viewport }) },
    frame(seconds) { if (menu) apply(runtime,{kind:"frame",timeSeconds:seconds}) },
    destroy() { runtime.destroy(); request.root.replaceChildren() },
  })
}

export function buildingModel(snapshot: BuildingSnapshot): string {
  const { kind } = snapshot.object
  if (kind === 2) {
    if (snapshot.phase === 0) return "models/buildables/sentry1_heavy.mdl"
    if (snapshot.phase === 2) return `models/buildables/sentry${snapshot.level}_heavy.mdl`
    if (snapshot.level === 1) return "models/buildables/sentry1.mdl"
    return `models/buildables/sentry${snapshot.level}.mdl`
  }
  if (kind === 0) {
    const level = snapshot.level === 1 ? "" : `_lvl${snapshot.level}`
    const light = snapshot.phase === 0 || snapshot.phase === 2 ? "" : "_light"
    return `models/buildables/dispenser${level}${light}.mdl`
  }
  return `models/buildables/teleporter${snapshot.phase === 0 || snapshot.phase === 2 ? "" : "_light"}.mdl`
}

export function blueprintModel(object: Tf2BuildingObject): string {
  return object.kind === 2 ? "models/buildables/sentry1_blueprint.mdl"
    : object.kind === 0 ? "models/buildables/dispenser_blueprint.mdl"
      : `models/buildables/teleporter_blueprint_${object.mode === 0 ? "enter" : "exit"}.mdl`
}
