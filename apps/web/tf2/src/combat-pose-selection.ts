import type { BotSnapshot, GameplayEvent } from "@playsrc/game-tf2-browser/codec"

/** A hidden player entity can still be the parent of a player_hurt effect. */
export function combatPoseSelection(bots: readonly BotSnapshot[], events: readonly GameplayEvent[], visible: (bot: BotSnapshot) => boolean) {
  const criticalTargets = new Set(events.filter(event => event.kind === 17 && event.auxiliary === 1 && event.subject !== 1 && event.values[2] > 0).map(event => event.subject))
  const drawn = new Set(bots.filter(bot => bot.lifecycle === 1 && visible(bot)).map(bot => bot.identity))
  const posed = bots.filter(bot => drawn.has(bot.identity) || criticalTargets.has(bot.identity))
  return Object.freeze({ posed, drawn, criticalTargets })
}
