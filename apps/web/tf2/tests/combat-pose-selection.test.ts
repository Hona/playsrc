import { expect, test } from "bun:test"
import { combatPoseSelection } from "../src/combat-pose-selection"
import type { BotSnapshot, GameplayEvent } from "@playsrc/game-tf2-browser/codec"

test("critical attachments pose existing dead players without drawing them or unrelated hidden players", () => {
  const bots = [1, 2, 3, 4].map(identity => ({ identity, lifecycle: identity === 2 ? 2 : 1 })) as BotSnapshot[]
  const events = [{ kind: 17, subject: 2, auxiliary: 1, values: [10, 0, 1, 0] },
    { kind: 17, subject: 5, auxiliary: 1, values: [10, 0, 2, 0] }] as GameplayEvent[]
  const result = combatPoseSelection(bots, events, bot => bot.identity === 3)
  expect(result.posed.map(bot => bot.identity)).toEqual([2, 3])
  expect([...result.drawn]).toEqual([3])
  expect([...result.criticalTargets]).toEqual([2, 5])
  expect(combatPoseSelection(bots, [], () => false).posed).toEqual([])
})
