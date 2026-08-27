import { describe, expect, test } from "bun:test"
import { deathNoticeGeometry, deathNoticeIcon, deathNoticeRow, retireDeathNotices } from "../../src/hud/deathnotice"
import { tf2HudAvailable as available, tf2HudUnavailable as unavailable } from "../../src/hud"
import type { Tf2HudKillfeedNotice } from "../../src/hud"
import { tf2DeathNoticeAssets } from "../../src/ui-resources/deathnotice.generated"

function notice(changes: Partial<Tf2HudKillfeedNotice> = {}): Tf2HudKillfeedNotice {
  return { killer: { identity: available(2), name: "RED", team: 2 }, victim: { identity: available(3), name: "BLU", team: 3 },
    assister: unavailable("not-applicable"), weaponIcon: available("scattergun"), weaponIdentity: available(4),
    customKill: 0, damageBits: 2, critical: false, selfInflicted: false, localPlayerInvolved: false,
    domination: false, revenge: false, silent: false, ...changes }
}
const row = (id: string, local = false, time = 0) => deathNoticeRow(id, time, notice({ localPlayerInvolved: local }), t => t)

describe("configured Source death notices", () => {
  test("retains authored prefixed atlas references, both polarities, and rectangles", () => {
    expect(tf2DeathNoticeAssets.contentBuild).toBe("24245096")
    expect(Object.keys(tf2DeathNoticeAssets.icons)).toHaveLength(595)
    for (const name of ["tf_projectile_rocket", "tf_projectile_pipe_remote", "tf_projectile_pipe", "quake_rl", "scattergun",
      "shotgun_primary", "shotgun_soldier", "shotgun_hwg", "shotgun_pyro", "pistol_scout", "pistol", "minigun",
      "sniperrifle", "headshot", "backstab", "crit", "skull_tf", "obj_sentrygun", "obj_sentrygun2", "obj_sentrygun3"]) {
      expect(deathNoticeIcon(`d_${name}`, false)?.name).toBe(`d_${name}`)
      expect(deathNoticeIcon(`d_${name}`, true)?.name).toBe(`dneg_${name}`)
    }
    expect(deathNoticeIcon("leaderboard_dominated", true)?.name).toBe("leaderboard_dominated")
    expect(deathNoticeIcon("d_not_a_weapon", false)).toBeNull()
    expect(deathNoticeRow("0", 0, notice({ weaponIcon: available("not_a_weapon") }), t => t).icon.name).toBe("d_skull_tf")
  })

  test("headshot/backstab override weapon art; only full critical damage supplies glow", () => {
    for (const [weapon, custom, expected] of [["sniperrifle", 1, "headshot"], ["sniperrifle", 51, "headshot"],
      ["ambassador", 1, "ambassador_headshot"], ["huntsman", 1, "huntsman_headshot"], ["knife", 2, "backstab"],
      ["sharp_dresser", 2, "sharp_dresser_backstab"]] as const) {
      const value = deathNoticeRow("0", 0, notice({ weaponIcon: available(weapon), customKill: custom, critical: true }), t => t)
      expect(value.icon.name).toBe(`d_${expected}`)
      expect(value.critIcon?.name).toBe("d_crit")
    }
    expect(row("mini").critIcon).toBeNull()
    const local = notice({ killer: { identity: available(1), name: "local", team: 2 }, localPlayerInvolved: true, critical: true })
    expect(deathNoticeRow("0", 0, local, t => t).critIcon?.name).toBe("dneg_crit")
  })

  test("assister name shares killer team text; assist-only highlight keeps the base crit polarity", () => {
    const value = deathNoticeRow("0", 0, notice({ assister: available({ identity: available(1), name: "Local", team: 2 }),
      localPlayerInvolved: true, critical: true }), t => t)
    expect(value.killer).toBe("RED + Local")
    expect(value.icon.name).toBe("dneg_scattergun")
    expect(value.critIcon?.name).toBe("d_crit")
  })

  test("world and self kills hide the killer, falling text follows the victim, saw overrides vehicle", () => {
    const fall = deathNoticeRow("fall", 0, notice({ selfInflicted: true, damageBits: 32, weaponIcon: available("world") }), t => t)
    expect(fall.killer).toBe("")
    expect(fall.info).toBe("#DeathMsg_Fall")
    expect(fall.icon.name).toBe("d_skull_tf")
    expect(deathNoticeRow("train", 0, notice({ selfInflicted: true, weaponIcon: available("tracktrain") }), t => t).icon.name).toBe("d_vehicle")
    expect(deathNoticeRow("saw", 0, notice({ selfInflicted: true, damageBits: 16 | 65536 }), t => t).icon.name).toBe("d_saw_kill")
    expect(deathNoticeRow("suicide", 0, notice({ selfInflicted: true, customKill: 6 }), t => t).info).toBe("#DeathMsg_Suicide")
  })

  test("strict curtime expiry, no fade, local priority, latest nonlocal always admitted", () => {
    const rows = [row("remote"), row("local", true)]
    expect(retireDeathNotices(rows, 6, 6, 4)).toBe(false)
    expect(rows.map(x => x.identity)).toEqual(["remote", "local"])
    expect(retireDeathNotices(rows, Math.fround(6.000001), 6, 4)).toBe(true)
    expect(rows.map(x => x.identity)).toEqual(["local"])
    expect(retireDeathNotices(rows, 12, 6, 4)).toBe(false)
    expect(retireDeathNotices(rows, Math.fround(12.000001), 6, 4)).toBe(true)
    const priority = [row("a", true), row("b"), row("c", true), row("d", true), row("e")]
    retireDeathNotices(priority, 1, 6, 4)
    expect(priority.map(x => x.identity)).toEqual(["a", "c", "d", "e"])
    const local = [row("a", true), row("b", true), row("c", true), row("d", true), row("e")]
    retireDeathNotices(local, 1, 6, 4)
    expect(local.map(x => x.identity)).toEqual(["b", "c", "d", "e"])
  })

  test("duration is evaluated for existing rows; backward time never expires or fades them", () => {
    const rows = [row("a", false, 100)]
    expect(retireDeathNotices(rows, 0, 6, 4)).toBe(false)
    expect(retireDeathNotices(rows, 101, 0, 4)).toBe(true)
    const negative = [row("a")]
    expect(retireDeathNotices(negative, 0, -1, 4)).toBe(true)
    const huge = [row("a", false, Math.fround(2 ** 24))]
    expect(retireDeathNotices(huge, Math.fround(2 ** 24 + 6), 6, 4)).toBe(false)
  })

  test("integer line layout rescales spacing with icon; rows compact and clip rather than ellipsize", () => {
    for (const height of [480, 600, 720, 768, 900, 1080, 844]) {
      const scale = height / 480, panelWidth = Math.trunc(628 * scale)
      const first = deathNoticeGeometry(row("a"), 0, panelWidth, scale, 2, 16 * scale, 4 * scale, 12, text => text.length * 7, true)
      const second = deathNoticeGeometry(row("b"), 1, panelWidth, scale, 2, 16 * scale, 4 * scale, 12, text => text.length * 7, true)
      expect(first.x + first.width).toBe(panelWidth)
      expect(first.y).toBe(Math.trunc(2 * scale))
      expect(second.y).toBe(Math.trunc(Math.trunc(2 * scale) + Math.trunc(16 * scale) + 4 * scale))
      expect(first.iconHeight).toBeLessThanOrEqual(Math.trunc(16 * scale) - Math.trunc(2 * scale))
    }
    const long = deathNoticeRow("long", 0, notice({ victim: { identity: available(3), team: 3, name: "W".repeat(128) } }), t => t)
    expect(deathNoticeGeometry(long, 0, 628, 1, 1, 16, 4, 12, s => s.length * 7, true).x).toBeLessThan(0)
  })
})
