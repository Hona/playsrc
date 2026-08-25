import type { ScoreboardSnapshot, Tf2Team } from "../codec"
import { tf2HudAvailable, tf2HudUnavailable } from "./bindings"
import type { Tf2HudScoreboard, Tf2ScoreboardCounters, Tf2ScoreboardPlayer } from "./contract"

function counters(kills: number, deaths: number, captures: number, damage: number): Tf2ScoreboardCounters {
  return Object.freeze({
    kills,
    deaths,
    assists: 0,
    destruction: 0,
    captures,
    defenses: 0,
    dominations: 0,
    revenge: 0,
    healing: 0,
    invulns: 0,
    teleports: 0,
    headshots: 0,
    backstabs: 0,
    bonus: 0,
    support: 0,
    damage,
  })
}

export function adaptTf2Scoreboard(
  authority: ScoreboardSnapshot,
  localTeam: Tf2Team,
  visible: boolean,
  mapName: string,
  pingAsText: boolean,
): Tf2HudScoreboard {
  const players: Tf2ScoreboardPlayer[] = []
  const spectators: string[] = []
  for (const player of authority.players) {
    if (player.team === 0 || player.team === 1) {
      spectators.push(player.name)
      continue
    }
    const enemy = (localTeam === 2 && player.team === 3) || (localTeam === 3 && player.team === 2)
    players.push(Object.freeze({
      identity: player.identity,
      name: player.name,
      team: player.team,
      connection: "connected",
      score: player.score,
      alive: player.alive,
      class: !enemy && localTeam !== 0 ? tf2HudAvailable(player.class) : tf2HudUnavailable("not-applicable"),
      ping: player.fake ? tf2HudAvailable("bot" as const) : tf2HudUnavailable("missing-source-fact"),
      killstreak: 0,
      activeDominations: 0,
      relationship: "none",
      counters: tf2HudAvailable(counters(player.kills, player.deaths, player.captures, player.damage)),
    }))
  }
  players.sort((left, right) => right.score - left.score || right.identity - left.identity)
  return Object.freeze({
    visible,
    mapName,
    gameType: mapName.startsWith("ctf_")
      ? tf2HudAvailable("#Gametype_CTF" as const)
      : mapName.startsWith("pl_")
        ? tf2HudAvailable("#Gametype_Escort" as const)
        : tf2HudUnavailable("not-applicable"),
    pingAsText,
    red: Object.freeze({ team: 2, localizedName: "RED", score: authority.redScore, playerCount: authority.redCount }),
    blue: Object.freeze({ team: 3, localizedName: "BLU", score: authority.blueScore, playerCount: authority.blueCount }),
    players: Object.freeze(players),
    spectators: Object.freeze(spectators),
    waitingToPlay: Object.freeze([]),
    selectedPlayer: localTeam === 2 || localTeam === 3
      ? tf2HudAvailable(1)
      : tf2HudUnavailable("not-applicable"),
  })
}
