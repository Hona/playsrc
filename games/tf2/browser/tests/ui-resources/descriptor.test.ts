import { describe, expect, test } from "bun:test"
import { configuredTf2UiResourceInput } from "../../src/ui-resources/configured.generated"
import {
  classifyTf2UiCommand,
  createTf2UiResourceDescriptor,
  tf2UiResourceBounds,
  tf2UiResources,
} from "../../src/ui-resources"

const cloneInput = (): any => structuredClone(configuredTf2UiResourceInput)

describe("configured TF2 UI resource descriptor", () => {
  test("binds the exact configured provider and selected source closure", () => {
    expect(tf2UiResources.identity).toBe("tf2-ui-24207079-4a097b1e805d9ce1")
    expect(tf2UiResources.providers).toHaveLength(14)
    expect(tf2UiResources.sources).toHaveLength(49)
    expect(tf2UiResources.panels).toHaveLength(35)
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/mainmenuoverride.res")?.sha256)
      .toBe("5f628eb8ec62ea557cf49bc13e587b48c5e7ebd480742e1795e1653c5ae8ed92")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/clientscheme.res")?.sha256)
      .toBe("1d071b99def0405cbf73d97642a396e6dcbad1a7488f12696ca5dd62893c604c")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "scripts/hudlayout.res")?.sha256)
      .toBe("1f18cb73d9ef54ff79ea208c9996db0655ac731b2ee8e9a82ff63a4b697f400f")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/optionssubkeyboard.res")?.sha256)
      .toBe("99cc7c486fa19b58c76842a8ce5abf8b65ff13fbbd097f5eb7e195783b570902")
    expect(tf2UiResources.animation.compositionOrder).toEqual([
      "scripts/hudanimations.txt",
      "scripts/hudanimations_tf.txt",
    ])
    expect(tf2UiResources.schemes.map((scheme) => [
      scheme.source.logicalPath,
      scheme.colors.length,
      scheme.fontDefinitions.length,
      scheme.borders.length,
    ])).toEqual([
      ["resource/clientscheme.res", 154, 164, 188],
      ["resource/sourcescheme.res", 24, 8, 0],
      ["resource/sourceschemebase.res", 11, 48, 19],
    ])
    expect(tf2UiResources.borders).toHaveLength(207)
  })

  test("is deterministic and deeply immutable", () => {
    const first = createTf2UiResourceDescriptor(cloneInput())
    const second = createTf2UiResourceDescriptor(cloneInput())
    expect(first.ok).toBe(true)
    expect(second.ok).toBe(true)
    if (!first.ok || !second.ok) return
    expect(first.descriptor.identity).toBe(second.descriptor.identity)
    expect(first.descriptor.commands).toEqual(second.descriptor.commands)
    expect(Object.isFrozen(first.descriptor)).toBe(true)
    expect(Object.isFrozen(first.descriptor.sources)).toBe(true)
    expect(Object.isFrozen(first.descriptor.panels[0]!.roots[0]!)).toBe(true)
  })

  test("retains every selected inventory with no unclassified item", () => {
    expect(tf2UiResources.controls).toHaveLength(52)
    expect(tf2UiResources.properties).toHaveLength(11_077)
    expect(tf2UiResources.commands).toHaveLength(79)
    expect(tf2UiResources.localization.tokens).toHaveLength(454)
    expect(tf2UiResources.localization.tokens.find((token) => token.name === "#Valve_Move_Forward")?.definitions[0]?.value).toBe("Move forward")
    expect(tf2UiResources.localization.tokens.find((token) => token.name === "#TF_OptionCategory_Combat")?.definitions[0]?.value).toBe("Combat Options")
    expect(tf2UiResources.images).toHaveLength(249)
    expect(tf2UiResources.fonts).toHaveLength(48)
    expect(tf2UiResources.advancedOptions).toHaveLength(88)
    expect(tf2UiResources.keyboardActions).toHaveLength(70)
    expect(new Set(tf2UiResources.keyboardActions.map((row) => row.binding.toLowerCase())).size).toBe(65)
    expect(Object.fromEntries([...Map.groupBy(tf2UiResources.advancedOptions, (row) => row.kind)].map(([kind, rows]) => [kind, rows.length]))).toEqual({
      BOOL: 55,
      SLIDER: 13,
      LIST: 12,
      NUMBER: 5,
      STRING: 3,
    })
    expect(tf2UiResources.controls.every((control) => control.owner !== "unsupported")).toBe(true)
    expect(tf2UiResources.properties.every((property) => property.owner !== "unsupported")).toBe(true)
    expect(tf2UiResources.commands.every((command) => command.executable === false)).toBe(true)
  })

  test("retains exact authoritative absences instead of fallbacks", () => {
    expect(tf2UiResources.missingDependencies).toEqual([
      "resource/loadingdialogdualprogress.res",
      "cfg/user.scr",
      "image:logos/UI/spray",
      "image:vgui/hud/icon_commentary",
      "localization:#Replay_SaveReplay",
      "localization:#Steam_ValidLoginRequired",
      "localization:#VAC_ConnectingToSecureServer",
      "localization:#VAC_ConnectionIssuesSupport_Title",
      "localization:#VAC_ConnectionIssuesSupportSite",
      "localization:#VAC_ConnectionIssuesSupportURL",
      "localization:#VAC_ConnectionRefusedDetail",
    ])
    expect(tf2UiResources.images.filter((image) => image.classification === "missing-material").map((image) => image.configuredValue))
      .toEqual(["logos/UI/spray", "vgui/hud/icon_commentary"])
    expect(tf2UiResources.fonts.some((font) => font.classification === "missing-font")).toBe(false)
  })

  test("retains platform, language, resolution, aspect, and session condition domains", () => {
    expect(tf2UiResources.conditions).toEqual({
      platforms: ["windows", "macos", "linux"],
      languages: ["english", "%language%"],
      resolutions: ["default", "minmode", "lodef", "hidef"],
      aspect: ["if_taller", "if_wider"],
      session: ["menu", "in-game", "replay"],
    })
    const conditions = tf2UiResources.properties.flatMap((property) => property.condition ? [property.condition.token] : [])
    expect(conditions.length).toBeGreaterThan(0)
  })
})

describe("TF2 UI command and capability inventory", () => {
  test("classifies every category without execution", () => {
    expect(classifyTf2UiCommand("resume_game")).toEqual({ category: "gameplay", capabilityOwner: "application", executable: false })
    expect(classifyTf2UiCommand("OpenOptionsDialog")).toEqual({ category: "application", capabilityOwner: "settings", executable: false })
    expect(classifyTf2UiCommand("play_casual")).toEqual({ category: "service", capabilityOwner: "service", executable: false })
    expect(classifyTf2UiCommand("view_newuser_forums")).toEqual({ category: "external", capabilityOwner: "external", executable: false })
    expect(classifyTf2UiCommand("play_training")).toEqual({ category: "unsupported", capabilityOwner: "unsupported", executable: false })
    expect(classifyTf2UiCommand("not-configured")).toBeNull()
    expect(new Set(tf2UiResources.commands.map((command) => command.category))).toEqual(
      new Set(["gameplay", "application", "service", "external", "unsupported"]),
    )
  })

  test("preserves configured Casual, Competitive, MvM, Community, Training, and local-server commands", () => {
    const commands = new Set(tf2UiResources.commands.map((command) => command.command))
    for (const command of [
      "play_casual",
      "play_competitive",
      "play_mvm",
      "play_community",
      "play_training",
      "create_server",
    ]) expect(commands.has(command)).toBe(true)
  })
})

describe("TF2 UI descriptor admission", () => {
  test("rejects a changed configured source ledger", () => {
    const input = cloneInput()
    input.resources[0].sha256 = "0".repeat(64)
    expect(createTf2UiResourceDescriptor(input)).toEqual({
      ok: false,
      diagnostic: { code: "ChangedSource", subject: "source ledger" },
    })
  })

  test("rejects a newly missing required resource", () => {
    const input = cloneInput()
    const source = input.resources.find((value: any) => value.logicalPath === "resource/clientscheme.res")
    Object.assign(source, {
      outcome: "missing",
      byteLength: null,
      sha256: null,
      providerIdentity: null,
      providerKind: null,
      providerRevision: null,
      document: null,
      checkedLocations: ["configured"],
    })
    expect(createTf2UiResourceDescriptor(input)).toEqual({
      ok: false,
      diagnostic: { code: "MissingRequiredResource", subject: "resource/clientscheme.res" },
    })
  })

  test("rejects malformed parsed resources atomically", () => {
    const input = cloneInput()
    input.resources[0].document[0].value = "invalid-object-scalar"
    const result = createTf2UiResourceDescriptor(input)
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.diagnostic.code).toBe("MalformedResource")
  })

  test("rejects bounds before descriptor publication", () => {
    const input = cloneInput()
    input.providers = Array.from({ length: tf2UiResourceBounds.maximumProviders + 1 }, () => ({}))
    expect(createTf2UiResourceDescriptor(input)).toEqual({
      ok: false,
      diagnostic: { code: "BoundExceeded", subject: "providers/resources" },
    })
  })
})
