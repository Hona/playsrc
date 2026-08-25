import { describe, expect, test } from "bun:test"
import bundleManifest from "../../../../../tools/source-bundle/tf2-ui.generated.json" with { type: "json" }
import { TF2_CLASS_IMAGES, TF2_HUD_DYNAMIC_IMAGES } from "../../src/hud"
import { configuredTf2UiResourceInput } from "../../src/ui-resources/configured.generated"
import {
  classifyTf2UiCommand,
  createTf2AuthoredCrosshairDescriptor,
  createTf2UiResourceDescriptor,
  tf2AuthoredCrosshairs,
  tf2UiResourceBounds,
  tf2UiResources,
} from "../../src/ui-resources"

const cloneInput = (): any => structuredClone(configuredTf2UiResourceInput)

describe("configured TF2 UI resource descriptor", () => {
  test("exports every code-selected HUD image through the generated source-bundle closure", () => {
    const staticImages = new Set(tf2UiResources.images.map((image) => image.configuredValue.toLowerCase()))
    const dependencies = new Map(bundleManifest.dependencies.map((dependency) => [dependency.logicalPath, dependency]))
    const dynamic = new Map(bundleManifest.dynamicImages.map((image) => [image.configuredValue.toLowerCase(), image.material]))
    expect(bundleManifest.dynamicImages).toHaveLength(2)
    for (const image of Object.values(TF2_CLASS_IMAGES).flatMap((images) => Object.values(images))) {
      const record = tf2UiResources.images.find((candidate) => candidate.configuredValue === image)
      expect(record?.classification, image).toBe("content-vtf")
      expect(record?.material?.sha256, image).toMatch(/^[0-9a-f]{64}$/u)
      expect(record?.textures, image).toHaveLength(1)
      expect(dependencies.get(record!.material!.logicalPath)?.kinds, image).toContain("material")
      expect(dependencies.get(record!.textures[0]!.source.logicalPath)?.kinds, image).toContain("texture")
      expect(dynamic.has(image.toLowerCase()), image).toBeFalse()
    }
    for (const image of TF2_HUD_DYNAMIC_IMAGES) {
      const folded = image.toLowerCase()
      expect(staticImages.has(folded) || dynamic.has(folded), image).toBeTrue()
      if (dynamic.has(folded)) {
        expect(dynamic.get(folded)).toBe(`materials/${image.replace(/^\.\.\//u, "").toLowerCase()}.vmt`)
      }
    }
  })

  test("binds the exact configured provider and selected source closure", () => {
    expect(tf2UiResources.identity).toBe("tf2-ui-24245096-825cf32936fa863e")
    expect(tf2UiResources.providers).toHaveLength(14)
    expect(tf2UiResources.sources).toHaveLength(81)
    expect(tf2UiResources.panels).toHaveLength(67)
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/statsummary.res")?.sha256)
      .toBe("bf146199fcd7aec0a5467752853b89ead6f882d11533de383c09561bd3455903")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/mainmenuoverride.res")?.sha256)
      .toBe("5f628eb8ec62ea557cf49bc13e587b48c5e7ebd480742e1795e1653c5ae8ed92")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/clientscheme.res")?.sha256)
      .toBe("1d071b99def0405cbf73d97642a396e6dcbad1a7488f12696ca5dd62893c604c")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "scripts/hudlayout.res")?.sha256)
      .toBe("1f18cb73d9ef54ff79ea208c9996db0655ac731b2ee8e9a82ff63a4b697f400f")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/optionssubkeyboard.res")?.sha256)
      .toBe("99cc7c486fa19b58c76842a8ce5abf8b65ff13fbbd097f5eb7e195783b570902")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/scoreboard.res")?.sha256)
      .toBe("ed6e7d1619dcfa7423d00cf77c19026d7fe00c6a2bca634bb742d20d870b3e52")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/teammenu.res"))
      .toMatchObject({ domain: "team-selection", byteLength: 14_354, sha256: "650867b7928f728ff6fce107745018223f3a19b1193b4c7f5fae54e391f102a0" })
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/hudobjectivetimepanel.res")?.sha256)
      .toBe("cae1af088a6fbe3ca6607d3e756ed2cf7e3d8f574ef18492a20f29cba7591500")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/waitingforplayerspanel.res")?.sha256)
      .toBe("04c80baaff2d42a8ae6155f240a33c8e687861b8df5d04f09c3a333b87f00b4f")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/hudobjectiveflagpanel.res")?.sha256)
      .toBe("8affbaac643b77e07e3561497d4f6cbc1abd919d497c92fe0a683015d9adc683")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/flagstatus.res")?.sha256)
      .toBe("69b56afcf854afe54cd745054f1f720540933a23838bf8f77f34d780091176ca")
    expect(tf2UiResources.sources.find((source) => source.logicalPath === "resource/ui/winpanel.res")?.sha256)
      .toBe("e03fe37c651b1958fedd58b8062a88cc76a7728a6b1d98f21e61af96bbb68089")
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
    expect(tf2UiResources.controls).toHaveLength(69)
    expect(tf2UiResources.properties).toHaveLength(17_253)
    expect(tf2UiResources.commands).toHaveLength(99)
    expect(tf2UiResources.localization.tokens).toHaveLength(586)
    expect(tf2UiResources.localization.tokens.find((token) => token.name === "#Valve_Move_Forward")?.definitions[0]?.value).toBe("Move forward")
    expect(tf2UiResources.localization.tokens.find((token) => token.name === "#TF_OptionCategory_Combat")?.definitions[0]?.value).toBe("Combat Options")
    expect(tf2UiResources.images).toHaveLength(361)
    expect(tf2UiResources.images.find((image) => image.configuredValue === "maps/menu_photos_pl_upward")?.material?.sha256)
      .toBe("79ca3d5e39f80c8d18c79eb63fd9b457a359e2a2db147c426eb7814a2cd1101e")
    expect(tf2UiResources.fonts).toHaveLength(77)
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
      "localization:#CMenu_ClassHighlightPanel_Title",
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

describe("authored TF2 crosshair content closure", () => {
  test("retains the exact transparent default icon and visible decrypted weapon icons", () => {
    expect(tf2AuthoredCrosshairs.contentBuild).toBe("24245096")
    expect(tf2AuthoredCrosshairs.iconSource).toMatchObject({
      logicalPath: "scripts/mod_textures.txt",
      sha256: "33a38ee5a1ffe71d461d7ea0d8317e08b512aaf69ad26f2922fb5da07e443b0c",
    })
    expect(tf2AuthoredCrosshairs.stock).toMatchObject({
      file: "",
      crop: { x: 32, y: 0, width: 32, height: 32 },
      material: { logicalPath: "materials/sprites/crosshairs.vmt", sha256: "ebb03a5623c41393c07e1ce9c18be187faedc5418a144af3bf1a21b3bc60b36f" },
      texture: { logicalPath: "materials/sprites/crosshairs.vtf", sha256: "e38c69d9c961a0bf8e39043c73d6d9f322d8138c7b4c05fc2b9dfee52d828b59" },
    })
    expect(tf2AuthoredCrosshairs.weapons.map((weapon) => ({
      identities: weapon.weaponIdentities,
      script: weapon.source.logicalPath,
      crop: weapon.crosshair.crop,
      autoaim: weapon.autoaim?.crop,
    }))).toEqual([
      { identities: [1, 2], script: "scripts/tf_weapon_rocketlauncher.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [3], script: "scripts/tf_weapon_pipebomblauncher.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [4], script: "scripts/tf_weapon_scattergun.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [5], script: "scripts/tf_weapon_pistol_scout.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [6], script: "scripts/tf_weapon_bat.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
     { identities: [7], script: "scripts/tf_weapon_shotgun_soldier.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [8], script: "scripts/tf_weapon_shovel.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [9], script: "scripts/tf_weapon_minigun.ctx", crop: { x: 64, y: 64, width: 64, height: 64 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [10], script: "scripts/tf_weapon_shotgun_hwg.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [11], script: "scripts/tf_weapon_fists.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [12], script: "scripts/tf_weapon_sniperrifle.ctx", crop: { x: 64, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [13], script: "scripts/tf_weapon_smg.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [14], script: "scripts/tf_weapon_club.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [17], script: "scripts/tf_weapon_bottle.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [18], script: "scripts/tf_weapon_grenadelauncher.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [40], script: "scripts/tf_weapon_shotgun_primary.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [41], script: "scripts/tf_weapon_pistol.ctx", crop: { x: 0, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [42], script: "scripts/tf_weapon_wrench.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },

      { identities: [15], script: "scripts/tf_weapon_flamethrower.ctx", crop: { x: 32, y: 32, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
      { identities: [16], script: "scripts/tf_weapon_fireaxe.ctx", crop: { x: 64, y: 0, width: 32, height: 32 }, autoaim: { x: 0, y: 48, width: 24, height: 24 } },
    ])
  })

  test("admits exactly the sorted paired authored styles and every animation frame", () => {
    expect(tf2AuthoredCrosshairs.styles.map((style) => [style.file, style.frames.length])).toEqual([
      ["crosshair1", 11], ["crosshair2", 11], ["crosshair3", 1], ["crosshair4", 1],
      ["crosshair5", 1], ["crosshair6", 11], ["crosshair7", 1], ["default", 1],
    ])
    for (const style of tf2AuthoredCrosshairs.styles) {
      expect(style.material.logicalPath).toBe(`materials/vgui/crosshairs/${style.file}.vmt`)
      expect(style.texture.logicalPath).toBe(`materials/vgui/crosshairs/${style.file}.vtf`)
      expect(style.frames.every((frame, index) => frame.index === index && frame.pngDataUrl.startsWith("data:image/png;base64,"))).toBe(true)
    }
    expect(Object.isFrozen(tf2AuthoredCrosshairs.styles[0]!.frames)).toBe(true)
  })

  test("rejects mismatched source crops, duplicate weapons, and missing style pairs", () => {
    const cropped = structuredClone(tf2AuthoredCrosshairs)
    cropped.stock.crop!.x = 128
    expect(() => createTf2AuthoredCrosshairDescriptor(cropped)).toThrow("source crop exceeds")

    const duplicate = structuredClone(tf2AuthoredCrosshairs)
    duplicate.weapons[1]!.weaponIdentities[0] = 1
    expect(() => createTf2AuthoredCrosshairDescriptor(duplicate)).toThrow("duplicated")

    const missing = structuredClone(tf2AuthoredCrosshairs)
    missing.styles[0]!.material.logicalPath = "materials/vgui/crosshairs/missing.vmt"
    expect(() => createTf2AuthoredCrosshairDescriptor(missing)).toThrow("exact material/texture pair")
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
