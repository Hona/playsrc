import type {
  ClientDiagnosticResources,
  ConsoleBorderResource,
  ConsoleLimits,
  ConsoleResourceResolution,
} from "@playsrc/vgui"

const color = (red: number, green: number, blue: number, alpha = 255) =>
  Object.freeze([red, green, blue, alpha] as const)

const font = (
  logicalIdentity: string,
  family: string,
  size: number,
  weight: number,
  proportional = false,
  outlinePxAt480 = 0,
) => Object.freeze({
  logicalIdentity,
  family,
  sizePxAt480: size,
  lineHeightPxAt480: size,
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
      title: font("resource/sourceschemebase.res#fonts/defaultlarge", "Tahoma", 18, 0),
      console: font("resource/sourceschemebase.res#fonts/consoletext", "Lucida Console", 10, 500),
      entry: font("resource/sourceschemebase.res#fonts/default", "Tahoma", 16, 500),
      completion: font("resource/sourceschemebase.res#fonts/defaultsmall", "Tahoma", 12, 0),
      submit: font("resource/sourceschemebase.res#fonts/default", "Tahoma", 16, 500),
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
    ...font("resource/sourceschemebase.res#fonts/defaultfixedoutline", "Lucida Console", 10, 0, false, 1),
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

export const consoleResourceBlocker =
  "The configured console resolves its complete SourceScheme and English localization composition. Windows Tahoma and Lucida Console font bytes and target raster metrics are platform-owned inputs unavailable to the browser, so computed role, family, size, weight, spacing, and line-height values are exact while glyph-raster parity remains blocked."
