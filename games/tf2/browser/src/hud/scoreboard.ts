import type { ScoreboardSnapshot, Tf2Team } from "../codec"
import { tf2HudAvailable, tf2HudUnavailable } from "./bindings"
import type { Tf2HudScoreboard, Tf2ScoreboardCounters, Tf2ScoreboardPlayer } from "./contract"

const sources = new WeakMap<Tf2HudScoreboard, Readonly<{ authority: ScoreboardSnapshot; localTeam: Tf2Team }>>()

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
  previous?: Tf2HudScoreboard,
): Tf2HudScoreboard {
  const source = previous && sources.get(previous)
  if (previous && source?.authority === authority && source.localTeam === localTeam
    && previous.visible === visible && previous.mapName === mapName && previous.pingAsText === pingAsText) return previous
  const retain = (result: Tf2HudScoreboard): Tf2HudScoreboard => {
    if (Object.isFrozen(authority) && Object.isFrozen(authority.players) && authority.players.every(Object.isFrozen)) {
      sources.set(result, { authority, localTeam })
    } else sources.delete(result)
    return result
  }
  const players: Tf2ScoreboardPlayer[] = []
  const spectators: string[] = []
  const retained = previous === undefined ? undefined : new Map(previous.players.map((player) => [player.identity, player]))
  for (const player of authority.players) {
    if (player.team === 0 || player.team === 1) {
      spectators.push(player.name)
      continue
    }
    const enemy = (localTeam === 2 && player.team === 3) || (localTeam === 3 && player.team === 2)
    const prior = retained?.get(player.identity)
    const priorCounters = prior?.counters.kind === "available" ? prior.counters.value : undefined
    const visibleClass = !enemy && localTeam !== 0
    if (prior !== undefined && prior.name === player.name && prior.team === player.team
      && prior.score === player.score && prior.alive === player.alive
      && (visibleClass ? prior.class.kind === "available" && prior.class.value === player.class : prior.class.kind === "unavailable")
      && (player.fake ? prior.ping.kind === "available" && prior.ping.value === "bot" : prior.ping.kind === "unavailable")
      && priorCounters?.kills === player.kills && priorCounters.deaths === player.deaths
      && priorCounters.captures === player.captures && priorCounters.damage === player.damage) {
      players.push(prior)
      continue
    }
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
  if (previous !== undefined && previous.visible === visible && previous.mapName === mapName
    && previous.pingAsText === pingAsText && previous.red.score === authority.redScore
    && previous.red.playerCount === authority.redCount && previous.blue.score === authority.blueScore
    && previous.blue.playerCount === authority.blueCount
    && previous.players.length === players.length && previous.players.every((player, index) => player === players[index])
    && previous.spectators.length === spectators.length && previous.spectators.every((name, index) => name === spectators[index])
    && (localTeam === 2 || localTeam === 3 ? previous.selectedPlayer.kind === "available" && previous.selectedPlayer.value === 1 : previous.selectedPlayer.kind === "unavailable")) {
    return retain(previous)
  }
  return retain(Object.freeze({
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
  }))
}
