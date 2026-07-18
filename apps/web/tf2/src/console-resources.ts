import type {
  ClientDiagnosticResources,
  ConsoleBorderResource,
  ConsoleLimits,
  ConsoleResourceResolution,
  LocalPlatformFontCapability,
  LocalPlatformFontTarget,
  VguiDesktopPlatform,VguiFontFileIdentity,VguiResolvedFontRequest,VguiSchemeDocument,VguiSchemeNode,VguiFontSetMount,
} from "@playsrc/vgui"
import { admitLocalPlatformFonts,mountVguiFontSet,resolveVguiSchemeFonts } from "@playsrc/vgui"

const consolePlatformFontTarget: LocalPlatformFontTarget = Object.freeze({
  identity: "tf2/source-scheme/e9159a98/windows-console-fonts",
  requiredPlatform: "windows",
  faces: Object.freeze([
    Object.freeze({
      identity: "tahoma-normal",
      localName: "Tahoma",
      browserFamily: "playsrc-tf2-tahoma-normal-e9159a98",
      weight: 0,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0000, 0xffff] as const),
    }),
    Object.freeze({
      identity: "tahoma-medium",
      localName: "Tahoma",
      browserFamily: "playsrc-tf2-tahoma-medium-e9159a98",
      weight: 500,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0000, 0xffff] as const),
    }),
    Object.freeze({
      identity: "lucida-console-normal-latin",
      localName: "Lucida Console",
      browserFamily: "playsrc-tf2-lucida-console-normal-e9159a98",
      weight: 0,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0000, 0x00ff] as const),
    }),
    Object.freeze({
      identity: "lucida-console-normal-extended",
      localName: "Tahoma",
      browserFamily: "playsrc-tf2-lucida-console-normal-e9159a98",
      weight: 0,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0100, 0xffff] as const),
    }),
    Object.freeze({
      identity: "lucida-console-medium-latin",
      localName: "Lucida Console",
      browserFamily: "playsrc-tf2-lucida-console-medium-e9159a98",
      weight: 500,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0000, 0x00ff] as const),
    }),
    Object.freeze({
      identity: "lucida-console-medium-extended",
      localName: "Tahoma",
      browserFamily: "playsrc-tf2-lucida-console-medium-e9159a98",
      weight: 500,
      style: "normal" as const,
      unicodeRange: Object.freeze([0x0100, 0xffff] as const),
    }),
  ]),
})

export const consolePlatformFonts: LocalPlatformFontCapability =
  await admitLocalPlatformFonts(consolePlatformFontTarget)

const browserFamily = (identity: string): string | null =>
  consolePlatformFonts.kind === "supported"
    ? consolePlatformFonts.faces.find((face) => face.identity === identity)?.browserFamily ?? null
    : null

const color = (red: number, green: number, blue: number, alpha = 255) =>
  Object.freeze([red, green, blue, alpha] as const)

const font = (
  logicalIdentity: string,
  family: string,
  faceIdentity: string,
  size: number,
  weight: number,
  proportional = false,
  outlinePxAt480 = 0,
) => Object.freeze({
  logicalIdentity,
  family,
  browserFamily: browserFamily(faceIdentity),
  sizePxAt480: size,
  lineHeightPxAt480: size + outlinePxAt480 * 2,
  weight,
  style: "normal" as const,
  proportional,
  outlinePxAt480,
})

const blank = color(0, 0, 0, 0)
const borderShade = color(96, 90, 78, 90)

const border = (
  logicalName: string,
  inset: readonly [number, number, number, number],
  width = 1,
): ConsoleBorderResource => Object.freeze({
  logicalName,
  colors: Object.freeze({ left: borderShade, top: borderShade, right: borderShade, bottom: borderShade }),
  widthsPxAt480: Object.freeze([width, width, width, width]),
  insetPxAt480: Object.freeze(inset),
  proportional: false,
})

const frameBorder: ConsoleBorderResource = Object.freeze({
  logicalName: "FrameBorder",
  colors: Object.freeze({ left: blank, top: blank, right: blank, bottom: blank }),
  widthsPxAt480: Object.freeze([0, 0, 0, 0]),
  insetPxAt480: Object.freeze([0, 0, 0, 0]),
  proportional: false,
})

const scheme = Object.freeze({
  logicalIdentity: "resource/sourcescheme.res+resource/sourceschemebase.res",
  tag: "Tracker",
  revision: "e9159a983557dea91b7030b382cce9ee7521c6f4de904107013bdcb47c4a732e",
})

export const consoleResources: ConsoleResourceResolution = Object.freeze({
  kind: "resolved",
  resources: Object.freeze({
    identity: "tf2-source-scheme-console-e9159a98",
    scheme,
    localization: Object.freeze({
      logicalIdentity: "resource/vgui_english.txt",
      language: "english",
      title: "Console",
      submit: "Submit",
      closeAccessibleName: "Close console",
      entryAccessibleName: "Console command",
      historyAccessibleName: "Console output",
      completionAccessibleName: "Console completions",
    }),
    colors: Object.freeze({
      frameBackground: color(60, 56, 53),
      frameBackgroundUnfocused: color(60, 56, 53, 190),
      titleBackground: blank,
      titleBackgroundUnfocused: blank,
      titleText: color(236, 227, 203, 150),
      titleTextUnfocused: color(201, 188, 162, 150),
      historyBackground: color(0, 0, 0, 128),
      inputBackground: color(0, 0, 0, 128),
      inputText: color(251, 236, 203, 150),
      inputSelectionBackground: color(156, 82, 33),
      inputSelectionText: color(0, 0, 0),
      inputCursor: color(221, 221, 221),
      completionBackground: color(39, 36, 34),
      completionText: color(255, 255, 255),
      completionArmedBackground: color(255, 155, 0),
      completionArmedText: color(0, 0, 0),
      submitBackground: color(201, 188, 162, 150),
      submitText: color(60, 56, 53),
      submitArmedBackground: color(236, 227, 203, 150),
      submitArmedText: color(60, 56, 53),
      submitDepressedBackground: color(201, 188, 162, 150),
      submitDepressedText: color(60, 56, 53),
      closeButton: color(236, 227, 203, 150),
      closeButtonUnfocused: color(255, 255, 255, 192),
      focus: color(0, 0, 0, 196),
      normalOutput: color(221, 221, 221),
      developerOutput: color(255, 255, 255),
    }),
    fonts: Object.freeze({
      title: font("resource/sourceschemebase.res#fonts/defaultlarge", "Tahoma", "tahoma-normal", 18, 0),
      console: font("resource/sourceschemebase.res#fonts/consoletext", "Lucida Console", "lucida-console-medium-latin", 10, 500),
      entry: font("resource/sourceschemebase.res#fonts/default", "Tahoma", "tahoma-medium", 16, 500),
      completion: font("resource/sourceschemebase.res#fonts/defaultsmall", "Tahoma", "tahoma-normal", 12, 0),
      submit: font("resource/sourceschemebase.res#fonts/default", "Tahoma", "tahoma-medium", 16, 500),
    }),
    borders: Object.freeze({
      frame: frameBorder,
      history: border("DepressedButtonBorder->BaseBorder->DepressedBorder", [0, 0, 1, 1]),
      entry: border("DepressedButtonBorder->BaseBorder->DepressedBorder", [0, 0, 1, 1]),
      submit: border("ButtonBorder->RaisedBorder", [0, 0, 1, 1]),
      submitDepressed: border("ButtonDepressedBorder", [2, 1, 1, 1]),
      completion: border("MenuBorder->RaisedBorder", [0, 0, 1, 1]),
    }),
    layout: Object.freeze({
      frameMinimumWidthPx: 128,
      frameMinimumHeightPx: 66,
      frameFocusTransitionSeconds: 0.3,
      clientMinimumWidthPx: 100,
      clientMinimumHeightPx: 100,
      clientInsetXPx: 8,
      clientInsetYPxAt480: 6,
      titleTextInsetXPxAt480: 16,
      titleTextInsetYPxAt480: 9,
      titleBackgroundInsetPxAt480: 5,
      titleBackgroundBottomPxAt480: 28,
      captionHeightPxAt480: 23,
      captionTitleBorderPxAt480: 7,
      clientTitleGapPxAt480: 1,
      closeButtonInsetRightPxAt480: 5,
      closeButtonInsetTopPxAt480: 8,
      closeButtonOffsetPxAt480: 20,
      closeButtonSizePxAt480: 18,
      closeGlyphSizePx: 14,
      resizeGripPxAt480: 5,
      resizeCornerPxAt480: 8,
      resizeBottomRightPxAt480: 18,
      consoleInsetPxAt480: 8,
      historyTopOffsetPxAt480: 4,
      historyDrawOffsetXPx: 3,
      historyDrawOffsetYPx: 1,
      entryHeightPxAt480: 24,
      entryInsetPxAt480: 4,
      entryDrawOffsetXPxAt480: 3,
      entryDrawOffsetYPxAt480: 1,
      submitWidthPxAt480: 64,
      submitInsetPxAt480: 7,
      completionTextInsetPxAt480: 6,
      completionRowPaddingPxAt480: 2,
    }),
  }),
})

export const diagnosticResources: ClientDiagnosticResources = Object.freeze({
  identity: "tf2-source-scheme-client-diagnostics-e9159a98",
  scheme,
  font: Object.freeze({
    ...font("resource/sourceschemebase.res#fonts/defaultfixedoutline", "Lucida Console", "lucida-console-normal-latin", 10, 0, false, 1),
  }),
  colors: Object.freeze({
    goodFps: color(0, 255, 0),
    warningFps: color(255, 255, 0),
    badFps: color(255, 0, 0),
    position: color(255, 255, 255),
  }),
  panelWidthPx: 300,
  panelPaddingPx: 2,
  panelHeightPaddingPx: 8,
  lineGapPx: 2,
  maximumLines: 4,
})

export const consoleLimits: ConsoleLimits = Object.freeze({
  maxInputUtf8Bytes: 255,
  maxHistoryItems: 100,
  maxCatalogItems: 64,
  maxCatalogItemUtf8Bytes: 255,
  maxCompletionItems: 64,
  maxCompletionItemUtf8Bytes: 63,
  maxVisibleCompletionItems: 10,
  maxOutputBatchSegments: 16,
  maxOutputBatchUtf8Bytes: 4096,
  maxOutputSegments: 256,
  maxOutputUtf8Bytes: 65_536,
  maxDiagnostics: 64,
  maxDomNodes: 300,
  maxListeners: 17,
})

export const consoleResourceBlocker = consolePlatformFonts.kind === "supported"
  ? "Windows Tahoma and Lucida Console faces loaded through isolated browser local() sources. Native TF2 build-24207079 non-antialiased GDI target captures remain absent, so browser glyph-raster parity remains blocked."
  : `TF2 console platform fonts unsupported: ${consolePlatformFonts.reason} on ${consolePlatformFonts.platform}; unadmitted faces are ${consolePlatformFonts.unadmittedFaceIdentities.join(", ") || "none"}. No browser fallback glyphs are presented.`

export type ResolvedConsoleResources=Readonly<{console:ConsoleResourceResolution;diagnostics:ClientDiagnosticResources;fontSet:VguiFontSetMount|null;blocker:string}>
const fontPaths=["resource/tf2build.ttf","resource/halflife2.ttf","resource/hl2ep2.ttf","resource/marlett.ttf","resource/linux_fonts/dejavusans.ttf","resource/linux_fonts/dejavusans-bold.ttf","resource/linux_fonts/dejavusans-boldoblique.ttf","resource/linux_fonts/dejavusans-oblique.ttf","resource/linux_fonts/liberationsans-regular.ttf","resource/linux_fonts/liberationsans-bold.ttf","resource/linux_fonts/liberationmono-regular.ttf","resource/linux_fonts/firasans-regular.ttf"] as const
const hash=async(bytes:Uint8Array)=>Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256",bytes.slice().buffer)),v=>v.toString(16).padStart(2,"0")).join("")
function schemeDocument(identity:string,bytes:Uint8Array):Promise<VguiSchemeDocument>{
  const source=new TextDecoder().decode(bytes).replace(/\/\/[^\n\r]*/gu,"");const tokens=[...source.matchAll(/"((?:\\.|[^"\\])*)"|\[[^\]]+\]|[{}]|[^\s{}"]+/gu)].map(m=>m[1]??m[0]!);let at=0;const bases:string[]=[]
  const node=():VguiSchemeNode=>{const name=tokens[at++]!;let condition:string|null=null;if(tokens[at]?.startsWith("["))condition=tokens[at++]!;if(tokens[at]==="{"){at++;const children:VguiSchemeNode[]=[];while(tokens[at]!=="}")children.push(node());at++;return Object.freeze({name,value:null,condition,children:Object.freeze(children)})}const value=tokens[at++]!;if(tokens[at]?.startsWith("["))condition=tokens[at++]!;return Object.freeze({name,value,condition,children:Object.freeze([])})}
  const roots:VguiSchemeNode[]=[];while(at<tokens.length){if(tokens[at]?.toLowerCase()==="#base"){at++;const target=tokens[at++]!.toLowerCase();bases.push(target.startsWith("resource/")?target:`resource/${target}`)}else roots.push(node())}const root=roots.find(value=>value.name.toLowerCase()==="scheme");if(!root)throw new Error(`VGUI scheme root missing: ${identity}`);return hash(bytes).then(sha256=>Object.freeze({logicalIdentity:identity,sha256,baseLogicalIdentities:Object.freeze(bases),root}))
}
function sfnt(bytes:Uint8Array){const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),u16=(o:number)=>view.getUint16(o,false),u32=(o:number)=>view.getUint32(o,false),tables=u16(4);let base=0;for(let i=0;i<tables;i++){const at=12+i*16;if(new TextDecoder().decode(bytes.subarray(at,at+4))==="name")base=u32(at+8)}if(!base)throw new Error("SFNT name table missing");const count=u16(base+2),strings=u16(base+4),families=new Set<string>();for(let i=0;i<count;i++){const at=base+6+i*12,platform=u16(at),name=u16(at+6);if(name!==1)continue;const length=u16(at+8),offset=base+strings+u16(at+10),raw=bytes.subarray(offset,offset+length);let value="";if(platform===0||platform===3){for(let p=0;p<raw.length;p+=2)value+=String.fromCharCode((raw[p]!<<8)|raw[p+1]!)}else value=String.fromCharCode(...raw);if(value)families.add(value)}return{version:`sfnt-16.16:${u32(0)}`,families:Object.freeze([...families])}}
function platform():VguiDesktopPlatform{const value=(navigator.platform??"").toLowerCase();if(value.startsWith("mac"))return"macos";if(value.startsWith("win"))return"windows";if(value.startsWith("linux"))return"linux";throw new Error("Unsupported VGUI desktop platform")}
export async function resolveConfiguredConsoleResources(entries:ReadonlyMap<string,Uint8Array>,viewportHeight:number):Promise<ResolvedConsoleResources>{
  const documents=await Promise.all(["resource/sourcescheme.res","resource/sourceschemebase.res"].map(id=>{const bytes=entries.get(id);if(!bytes)throw new Error(`Missing ${id}`);return schemeDocument(id,bytes)}));const files:VguiFontFileIdentity[]=[];const supplies:any[]=[];for(const logicalIdentity of fontPaths){const bytes=entries.get(logicalIdentity);if(!bytes)throw new Error(`Missing ${logicalIdentity}`);const bitmap=logicalIdentity.endsWith(".vbf"),meta=bitmap?{version:`vbf-sha256:${await hash(bytes)}`,families:Object.freeze(["Buttons"])}:sfnt(bytes),sha256=await hash(bytes),file=Object.freeze({kind:bitmap?"bitmap" as const:"content" as const,logicalIdentity,sha256,byteLength:bytes.length,version:meta.version,families:meta.families});files.push(file);supplies.push(Object.freeze({...file,bytes}))}const p=platform(),lookups=[{identity:"title",name:p==="macos"?"UiBold":"DefaultLarge",proportional:false},{identity:"console",name:"ConsoleText",proportional:false},{identity:"entry",name:"Default",proportional:false},{identity:"completion",name:"DefaultSmall",proportional:false},{identity:"diagnostic",name:"DefaultFixedOutline",proportional:false}];const resolution=resolveVguiSchemeFonts({schemeLogicalIdentity:"resource/sourcescheme.res",documents,fontFiles:files,localFonts:[],context:{platform:p,viewportHeight,language:"english",minMode:false,steamDeck:false,surfaceFeatures:{antialias:true,dropShadow:true,outline:true}},lookups});if(!resolution.ok)throw new Error(`${resolution.diagnostic.code}:${resolution.diagnostic.subject}`);const mounted=await mountVguiFontSet({identity:`tf2/source-scheme/${p}-24207079`,fonts:resolution.fonts,byteSupplies:supplies,profiles:[]}),capability=mounted.ok?mounted.fontSet.snapshot().capability:null;const selected=(id:string)=>{const request=resolution.fonts.find(v=>v.identity===id)!,family=capability?.kind==="supported"?capability.fonts.find(v=>v.identity===id)?.browserFamily??null:null;return Object.freeze({logicalIdentity:`resource/sourcescheme.res#fonts/${request.schemeFontName.toLowerCase()}`,family:request.family,browserFamily:family,sizePxAt480:request.requestedHeight,lineHeightPxAt480:request.requestedHeight+Number(request.effects.outline)*2,weight:request.weight,style:request.effects.italic?"italic" as const:"normal" as const,proportional:request.proportional,outlinePxAt480:Number(request.effects.outline)})};if(consoleResources.kind!=="resolved")throw new Error("base resources");const scheme=Object.freeze({logicalIdentity:resolution.documentLogicalIdentities.join("+"),tag:"Tracker",revision:resolution.schemeSha256}),resources=Object.freeze({...consoleResources.resources,scheme,fonts:Object.freeze({title:selected("title"),console:selected("console"),entry:selected("entry"),completion:selected("completion"),submit:selected("entry")})}),diagnostics=Object.freeze({...diagnosticResources,scheme,font:selected("diagnostic")});return Object.freeze({console:Object.freeze({kind:"resolved" as const,resources}),diagnostics,fontSet:mounted.ok?mounted.fontSet:null,blocker:`The configured console resolves its complete SourceScheme for ${p}; exact native metrics and raster comparison remain required.`})}
