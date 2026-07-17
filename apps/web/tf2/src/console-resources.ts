import type { ConsoleLimits, ConsoleResourceResolution } from "@playsrc/vgui"

const color = (red: number, green: number, blue: number, alpha = 255) =>
  Object.freeze([red, green, blue, alpha] as const)

const font = (logicalIdentity: string, family: string, size: number, weight: number) => Object.freeze({
  logicalIdentity,
  family,
  sizePxAt480: size,
  lineHeightPxAt480: size + 2,
  weight,
  style: "normal" as const,
})

export const consoleResources: ConsoleResourceResolution = Object.freeze({
  kind: "resolved",
  resources: Object.freeze({
    identity: "tf2-client-scheme-console-1d071b99",
    scheme: Object.freeze({
      logicalIdentity: "resource/clientscheme.res",
      tag: "ClientScheme",
      revision: "1d071b99def0405cbf73d97642a396e6dcbad1a7488f12696ca5dd62893c604c",
    }),
    localization: Object.freeze({
      logicalIdentity: "playsrc/application/tf2-console-english",
      language: "english",
      title: "Developer Console",
      submit: "Submit",
      entryAccessibleName: "Console command",
      historyAccessibleName: "Console output",
      completionAccessibleName: "Console completions",
    }),
    colors: Object.freeze({
      frameBackground: color(0, 0, 0, 196),
      titleText: color(178, 82, 22),
      historyBackground: color(0, 0, 0, 0),
      inputBackground: color(0, 0, 0, 0),
      inputText: color(178, 178, 178),
      completionBackground: color(0, 0, 0, 196),
      completionText: color(235, 226, 202),
      completionSelected: color(145, 73, 59),
      focus: color(178, 82, 22),
      normalOutput: color(178, 178, 178),
      developerOutput: color(178, 82, 22),
    }),
    fonts: Object.freeze({
      title: font("resource/clientscheme.res#fonts/defaultlarge", "Verdana", 18, 900),
      console: font("resource/clientscheme.res#fonts/default", "Verdana", 12, 900),
      completion: font("resource/clientscheme.res#fonts/defaultsmall", "Verdana", 12, 400),
    }),
    border: Object.freeze({
      logicalName: "ComboBoxBorder",
      color: color(235, 226, 202),
      widthPxAt480: 1,
      style: "solid" as const,
    }),
    frameTitleHeightPxAt480: 20,
  }),
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
  maxListeners: 8,
})

export const consoleResourceBlocker =
  "resource/gameui_english.txt is absent from the configured TF2 provider set; the console uses application-owned English labels while ClientScheme colors, fonts, and border values retain exact build 24207079 identities."
