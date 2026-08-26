import {
  isVguiGenericResourcePropertySupported,
  VGUI_GENERIC_CONTROL_NAMES,
  type VguiControlRegistration,
  type VguiImagePresentation,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiScheme,
} from "@playsrc/vgui"
import { FakeDocument, createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import {
  initializeTf2GameUiIntegration,
  type Tf2GameUiIntegration,
} from "../../src/gameui-integration"
import type { Tf2GameUiRequest } from "../../src/gameui"
import {
  initializeTf2LoadingVguiRuntime,
  type Tf2LoadingPresentationRequest,
  type Tf2LoadingVguiRuntime,
} from "../../src/loading-presentation"
import type { Tf2VguiResources } from "../../src/ui-integration"
import { tf2UiResources, type Tf2UiResourceNode } from "../../src/ui-resources"

const genericControls = new Set<string>(VGUI_GENERIC_CONTROL_NAMES)

function convert(node: Tf2UiResourceNode): VguiResourceNode {
  const control = node.children.find((child) => child.name.toLowerCase() === "controlname" && child.value !== null)?.value
  return Object.freeze({
    name: node.name,
    value: node.value,
    condition: node.condition?.token ?? null,
    children: Object.freeze(node.children.filter((child) =>
      !/(?:_lodef|_minmode)$/iu.test(child.name)
      && !/^(?:border|border_override|background-texture|paintbackgroundtype|drawcolor|frame|image)$/iu.test(child.name)
      && !(child.value !== null && control && genericControls.has(control)
        && !isVguiGenericResourcePropertySupported(control as never, child.name)),
    ).map(convert)),
  })
}

function createResources(): Tf2VguiResources {
  const documents = new Map<string, VguiResourceDocument>()
  for (const panel of tf2UiResources.panels) {
    const root = panel.roots[0]
    if (root && panel.source.sha256) {
      documents.set(panel.source.logicalPath, Object.freeze({
        logicalIdentity: panel.source.logicalPath,
        revision: panel.source.sha256,
        root: convert(root),
      }))
    }
  }

  const accepted = new Map<string, Set<string>>()
  const visit = (node: Tf2UiResourceNode): void => {
    const control = node.children.find((child) => child.name.toLowerCase() === "controlname" && child.value !== null)?.value
    if (control) {
      const properties = accepted.get(control) ?? new Set<string>()
      for (const child of node.children) {
        if (child.value !== null && child.name.toLowerCase() !== "controlname") properties.add(child.name)
      }
      accepted.set(control, properties)
    }
    for (const child of node.children) if (child.value === null) visit(child)
  }
  for (const panel of tf2UiResources.panels) for (const root of panel.roots) visit(root)

  const menuProperties = new Set(tf2UiResources.panels
    .filter((panel) => panel.domain === "main-menu")
    .flatMap((panel) => panel.roots.flatMap((root) => root.children
      .flatMap((block) => block.children.filter((child) => child.value !== null).map((child) => child.name)))))
  accepted.set("CTFMatchmakingDashboard", menuProperties)
  accepted.set("CTFPlaylistPanel", menuProperties)
  accepted.set("CHudMainMenuOverride", new Set(["update_url", "blog_url", "button_x_offset", "button_y", "button_y_delta"]))

  const customControls: VguiControlRegistration[] = tf2UiResources.controls
    .filter((control) => !genericControls.has(control.name))
    .map((control) => Object.freeze({
      name: control.name,
      baseControl: control.name === "CTeamMenu" ? "Frame"
        : control.name === "CCreateMultiplayerGameServerPage" ? "PropertyPage"
        : /button/iu.test(control.name) ? "Button" : /image|class/iu.test(control.name) ? "ImagePanel" : "EditablePanel",
      element: /button/iu.test(control.name) ? "button" : "div",
      role: null,
      focusable: /button/iu.test(control.name),
      animationVariables: Object.freeze([]),
      acceptedProperties: Object.freeze([...(accepted.get(control.name) ?? [])]),
    }))
  for (const name of ["CHudMainMenuOverride", "CTFMatchmakingDashboard", "CTFPlaylistPanel"]) {
    if (customControls.some((control) => control.name === name)) continue
    customControls.push(Object.freeze({
      name,
      baseControl: "EditablePanel",
      element: "div",
      role: null,
      focusable: false,
      animationVariables: Object.freeze([]),
      acceptedProperties: Object.freeze([...(accepted.get(name) ?? [])]),
    }))
  }

  const images: VguiImagePresentation[] = [...Map.groupBy(tf2UiResources.images, (image) => image.configuredValue.toLowerCase()).values()]
    .map((entries, index) => {
      const image = entries[0]!
      return Object.freeze({
        name: image.configuredValue,
        logicalIdentity: `materials/vgui/gameui-transitions/${index}.vtf`,
        revision: image.material?.sha256 ?? image.identity,
        browserUrl: "data:image/png;base64,AA==",
        width: image.textures[0]?.width ?? 1,
        height: image.textures[0]?.height ?? 1,
        frames: image.textures[0]?.frames ?? 1,
        hardwareFiltered: false,
      })
    })
  for (const [index, name] of ["../console/background_2fort", "../console/background_2fort_widescreen"].entries()) {
    images.push(Object.freeze({
      name,
      logicalIdentity: `materials/console/background_2fort${index ? "_widescreen" : ""}.vmt`,
      revision: `background-${index}`,
      browserUrl: "data:image/png;base64,AA==",
      width: 1,
      height: 1,
      frames: 1,
      hardwareFiltered: false,
    }))
  }

  const clientScheme: VguiScheme = Object.freeze({
    identity: "tf2-gameui-transitions",
    revision: tf2UiResources.identity,
    tag: "ClientScheme",
    colors: Object.freeze([
      { name: "TanLight", value: "235 226 202 255" },
      { name: "TanDark", value: "117 107 94 255" },
      { name: "Black", value: "46 43 42 255" },
      { name: "TransparentBlack", value: "0 0 0 196" },
    ]),
    settings: Object.freeze([]),
    fonts: Object.freeze([...new Set(tf2UiResources.schemes.flatMap((scheme) => scheme.fontDefinitions.map((font) => font.name)))]
      .map((name) => Object.freeze({
        name,
        cssFamily: "sans-serif",
        sizePx: 12,
        lineHeightPx: 12,
        weight: 400,
        style: "normal" as const,
        available: true,
        measure: () => Object.freeze({ width: 0, height: 12 }),
      }))),
    borders: Object.freeze([]),
    images: Object.freeze(images),
  })

  return Object.freeze({
    identity: tf2UiResources.identity,
    descriptor: tf2UiResources,
    clientScheme,
    sourceScheme: clientScheme,
    localization: Object.freeze({
      identity: "tf2-gameui-transitions",
      revision: "1",
      language: "english",
      tokens: Object.freeze(tf2UiResources.localization.tokens.flatMap((token) => {
        if (!["#LoadingMap", "#Gametype_Escort", "#TF_Matchmaking_HeaderModeSelect"].includes(token.name)
          && !token.name.startsWith("#MMenu_PlayList_")) return []
        const definition = token.definitions.at(-1)
        return definition
          ? [Object.freeze({ name: token.name.replace(/^#/u, ""), value: definition.value })]
          : []
      })),
    }),
    animations: Object.freeze({ identity: "tf2-gameui-transitions", revision: "1", scripts: Object.freeze([]), activeConditions: Object.freeze([]) }),
    activeConditions: Object.freeze(["WIN32", "OSX", "POSIX"]),
    customControls: Object.freeze(customControls),
    gameUiBackground: Object.freeze({
      identity: "tf2-gameui-background-test",
      contentBuild: tf2UiResources.contentBuild,
      source: Object.freeze({ logicalPath: "scripts/chapterbackgrounds.txt", byteLength: 1, sha256: "0".repeat(64) }),
      defaultChapter: 1 as const,
      backgroundName: "background_2fort",
      variants: Object.freeze(["standard", "widescreen"].map((aspect, index) => Object.freeze({
        aspect: aspect as "standard" | "widescreen",
        image: `../console/background_2fort${index ? "_widescreen" : ""}`,
        material: `materials/console/background_2fort${index ? "_widescreen" : ""}.vmt`,
        materialSha256: `${index}`.repeat(64),
        texture: `materials/console/background_2fort${index ? "_widescreen" : ""}.vtf`,
        textureSha256: `${index}`.repeat(64),
        width: 1,
        height: 1,
      }))),
    }),
    diagnostics: Object.freeze([]),
    document(logicalPath) {
      const document = documents.get(logicalPath.toLowerCase())
      if (!document) throw new Error(`Missing configured document ${logicalPath}`)
      return document
    },
    panelDocument(logicalPath) {
      const document = tf2UiResources.panels.find((panel) => panel.source.logicalPath === logicalPath.toLowerCase())
      if (!document) throw new Error(`Missing configured panel ${logicalPath}`)
      return document
    },
    destroy() {},
  })
}

export type Tf2GameUiTransitionFixture = Readonly<{
  gameUi: Tf2GameUiIntegration
  loading: Tf2LoadingVguiRuntime
  requests: Tf2GameUiRequest[]
  loadingRequests: Tf2LoadingPresentationRequest[]
  resources: Tf2VguiResources
  document: FakeDocument
}>

export function createTf2GameUiTransitionFixture(nowSeconds: () => number = () => 0): Tf2GameUiTransitionFixture {
  const document = new FakeDocument()
  const resources = createResources()
  const viewport = { width: 1_280, height: 720, devicePixelRatio: 1 }
  const requests: Tf2GameUiRequest[] = []
  const loadingRequests: Tf2LoadingPresentationRequest[] = []
  const random = {
    nextUnit: () => 0,
    nextInteger: (minimum: number) => minimum,
    snapshot: () => Object.freeze({ seed: 0, state: 0, current: 0, shuffle: Object.freeze([]), draws: 0 }),
    restore() {},
  }
  const gameUi = initializeTf2GameUiIntegration({
    root: createRoot(document) as unknown as HTMLElement,
    resources,
    viewport,
    reducedMotion: true,
    clock: { nowSeconds },
    random,
    presentation: { random, activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false },
    onRequest: (request) => requests.push(request),
  })
  const loading = initializeTf2LoadingVguiRuntime({
    root: createRoot(document) as unknown as HTMLElement,
    resources,
    viewport,
    reducedMotion: true,
    clock: { nowSeconds },
    random,
    onRequest: (request) => loadingRequests.push(request),
  })
  return Object.freeze({ gameUi, loading, requests, loadingRequests, resources, document })
}
