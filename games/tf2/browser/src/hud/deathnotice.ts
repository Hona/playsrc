import type { Tf2HudKillfeedNotice } from "./contract"
import { tf2DeathNoticeAssets } from "../ui-resources/deathnotice.generated"

export type DeathNoticeIcon = Readonly<{ atlas: string; x: number; y: number; width: number; height: number }>
const icons: Readonly<Record<string, DeathNoticeIcon>> = tf2DeathNoticeAssets.icons

// CHudBaseDeathNotice::GetIcon: inverted art is optional, the ordinary icon
// is tried next, and only a missing death identifier uses the TF skull.
export function deathNoticeIcon(name: string, inverted: boolean): Readonly<{ name: string; icon: DeathNoticeIcon }> | null {
  const selected = inverted && name.startsWith("d_") && icons[`dneg_${name.slice(2)}`]
    ? `dneg_${name.slice(2)}` : name
  return icons[selected] ? { name: selected, icon: icons[selected]! } : null
}

export type DeathNoticeRow = Readonly<{
  identity: string
  creationTime: number
  notice: Tf2HudKillfeedNotice
  killer: string
  victim: string
  icon: NonNullable<ReturnType<typeof deathNoticeIcon>>
  critIcon: ReturnType<typeof deathNoticeIcon>
  info: string
}>

export function deathNoticeRow(identity: string, creationTime: number, notice: Tf2HudKillfeedNotice,
  localize: (token: string) => string): DeathNoticeRow {
  let name = notice.weaponIcon.kind === "available" ? `d_${notice.weaponIcon.value}`.slice(0, 31) : "d_world"
  let info = ""
  if (notice.selfInflicted && (notice.damageBits & 32) !== 0) info = localize("#DeathMsg_Fall")
  else if (notice.selfInflicted && ((notice.damageBits & 16) !== 0 || name === "d_tracktrain")) name = "d_vehicle"
  switch (notice.customKill) {
    case 2: name = name === "d_sharp_dresser" ? "d_sharp_dresser_backstab" : "d_backstab"; break
    case 1:
    case 51:
      name = name === "d_ambassador" ? "d_ambassador_headshot"
        : name === "d_huntsman" ? "d_huntsman_headshot" : "d_headshot"
      break
    case 3:
      if (notice.killer.identity.kind === "available" && notice.victim.identity.kind === "available"
        && notice.killer.identity.value === notice.victim.identity.value) { name = "d_firedeath"; info = "" }
      break
    case 6:
      info = localize(notice.selfInflicted ? "#DeathMsg_Suicide"
        : notice.assister.kind === "available" ? "#DeathMsg_AssistedSuicide_Multiple" : "#DeathMsg_AssistedSuicide")
      break
  }
  if ((notice.damageBits & 65536) !== 0) name = "d_saw_kill"
  const killer = notice.selfInflicted ? "" : notice.killer.name
  return Object.freeze({ identity, creationTime, notice,
    killer: notice.assister.kind === "available" ? `${killer} + ${notice.assister.value.name}` : killer,
    victim: notice.victim.name,
    icon: deathNoticeIcon(name, notice.localPlayerInvolved) ?? deathNoticeIcon("d_skull_tf", notice.localPlayerInvolved)!,
    // The base handler selects crit art before the TF assister highlight update.
    critIcon: notice.critical ? deathNoticeIcon("d_crit", notice.killer.identity.kind === "available" && notice.killer.identity.value === 1
      || notice.victim.identity.kind === "available" && notice.victim.identity.value === 1) : null,
    info,
  })
}

export function deathNoticeRivalries(row: DeathNoticeRow, localize: (token: string) => string): DeathNoticeRow[] {
  const result: DeathNoticeRow[] = []
  for (const [flag, token] of [[row.notice.domination, "#Msg_Dominating"], [row.notice.revenge, "#Msg_Revenge"]] as const) {
    if (!flag) continue
    const involved = row.notice.killer.identity.kind === "available" && row.notice.killer.identity.value === 1
      || row.notice.victim.identity.kind === "available" && row.notice.victim.identity.value === 1
    result.push(Object.freeze({ ...row, identity: `${row.identity}_${token.slice(1)}`, killer: row.notice.killer.name,
      notice: Object.freeze({ ...row.notice, selfInflicted: false, localPlayerInvolved: involved }),
      icon: deathNoticeIcon("leaderboard_dominated", false)!, critIcon: null, info: localize(token) }))
  }
  return result
}

// Source has no fade or row animation. Expiry is strictly greater-than, in
// simulation curtime, and the duration cvar is read even for existing notices.
export function retireDeathNotices(rows: DeathNoticeRow[], curtime: number, duration: number, maximum: number): boolean {
  let changed = false
  for (let i = rows.length - 1; i >= 0; i--) {
    const row = rows[i]!
    if (curtime > Math.fround(row.creationTime + Math.fround(duration * (row.notice.localPlayerInvolved ? 2 : 1)))) {
      rows.splice(i, 1); changed = true
    }
  }
  let needed = rows.length - maximum
  // Preserve the SDK forward-index removal order, including adjacent skips.
  for (let i = 0; i < rows.length - 1 && needed > 0; i++) {
    if (!rows[i]!.notice.localPlayerInvolved) { rows.splice(i, 1); needed--; changed = true }
  }
  while (rows.length > maximum) { rows.shift(); changed = true }
  return changed
}

export type DeathNoticeGeometry = Readonly<{
  x: number; y: number; width: number; height: number
  killerX: number; victimX: number; infoX: number; textY: number
  iconX: number; iconY: number; iconWidth: number; iconHeight: number
}>

export function deathNoticeGeometry(row: DeathNoticeRow, index: number,
  panelWidth: number, scale: number, horizontalScale: number, lineHeight: number, spacing: number, fontTall: number,
  measure: (text: string) => number, rightJustify: boolean): DeathNoticeGeometry {
  const margin = Math.trunc(10 * horizontalScale), space = measure(" ")
  const tall = Math.trunc(lineHeight), desired = tall - Math.trunc(2 * scale)
  const factor = Math.fround(desired / row.icon.icon.height)
  const iconWidth = Math.trunc(Math.fround(row.icon.icon.width * factor))
  const iconHeight = Math.trunc(Math.fround(row.icon.icon.height * factor))
  // Source scales the spacing along with the texture's width, not separately.
  const iconWide = Math.trunc(Math.fround((row.icon.icon.width + space) * factor))
  const killerWide = row.killer ? measure(row.killer) + space : 0
  const victimWide = measure(row.victim) + space
  const infoWide = row.info ? measure(row.info) + space : 0
  const width = killerWide + iconWide + victimWide + infoWide + margin * 2
  const x = rightJustify ? panelWidth - width : 0
  const y = Math.trunc(2 * scale) + (tall + Math.trunc(spacing)) * index
  const iconX = x + margin + killerWide
  return { x, y, width, height: tall, killerX: x + margin,
    iconX, iconY: y + Math.trunc((tall - iconHeight) / 2), iconWidth, iconHeight,
    textY: y + Math.trunc((tall - fontTall) / 2),
    victimX: iconX + iconWide + (row.notice.selfInflicted ? 0 : infoWide),
    infoX: iconX + iconWide + (row.notice.selfInflicted ? victimWide : 0) }
}
