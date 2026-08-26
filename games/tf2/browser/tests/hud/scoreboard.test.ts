import { describe, expect, test } from "bun:test"
import type { ScoreboardSnapshot } from "../../src/codec"
import { adaptTf2Scoreboard } from "../../src/hud/scoreboard"

function roster(count: number): ScoreboardSnapshot {
  const players = Array.from({ length: count }, (_, index) => Object.freeze({
    identity: index + 1,
    name: index === 0 ? "Player" : `Bot ${index}`,
    team: index % 2 === 0 ? 2 as const : 3 as const,
    class: (index % 9 + 1) as 1,
    alive: true,
    fake: index !== 0,
    score: count - index,
    kills: 0,
    deaths: 0,
    captures: 0,
    damage: 0,
  }))
  return Object.freeze({ redScore: 1, blueScore: 2, redCount: Math.ceil(count / 2), blueCount: Math.floor(count / 2), players: Object.freeze(players) }) as ScoreboardSnapshot
}

describe("retained authoritative TF2 scoreboard presentation", () => {
  test.each([16, 24])("retains all %i immutable player rows and the complete scoreboard across unchanged publications", (count) => {
    const source = roster(count)
    const first = adaptTf2Scoreboard(source, 2, true, "pl_upward", false)
    const next = adaptTf2Scoreboard(source, 2, true, "pl_upward", false, first)
    expect(next).toBe(first)
    expect(next.players).toHaveLength(count)
    expect(next.red.playerCount + next.blue.playerCount).toBe(count)
  })

  test("replaces only changed player facts while preserving authored ordering, class visibility, and teammates", () => {
    const source = roster(24)
    const first = adaptTf2Scoreboard(source, 2, false, "ctf_2fort", false)
    const changed = Object.freeze({ ...source, players: Object.freeze(source.players.map((player) => player.identity === 4 ? Object.freeze({ ...player, score: 99, kills: 2, damage: 350 }) : player)) })
    const next = adaptTf2Scoreboard(changed, 2, true, "ctf_2fort", false, first)
    expect(next).not.toBe(first)
    expect(next.visible).toBeTrue()
    expect(next.players[0]?.identity).toBe(4)
    expect(next.players.find((player) => player.identity === 1)).toBe(first.players.find((player) => player.identity === 1))
    expect(next.players.find((player) => player.identity === 4)).not.toBe(first.players.find((player) => player.identity === 4))
    expect(next.players.find((player) => player.identity === 4)?.class.kind).toBe("unavailable")
    expect(next.players.find((player) => player.identity === 3)?.class.kind).toBe("available")
  })
})
