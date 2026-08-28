// Runs inside the headed page. Observe rendered publications, not a one-second
// timer that can miss a capper killed between samples. No simulation mutation.
export function observeSetupObjectiveContacts(profile: Record<string, any>) {
  const contacts: Array<{ at: number; tick: string | null; points: unknown; round: unknown }> = []
  let current: any
  Object.defineProperty(profile, "controlPoints", {
    configurable: true,
    enumerable: true,
    get: () => current,
    set: (points: any) => {
      current = points
      const round = profile.round
      if (contacts.length < 16 && round?.state === 4 && !round.waitingForPlayers && !round.inSetup
        && points?.points.some((point: any) => point.playerCounts[1] > 0 || point.owner === 3)) {
        contacts.push({ at: performance.now(), tick: profile.bots?.[0]?.tick ?? null, points, round })
      }
    },
  })
  profile.setupObjectiveContacts = contacts
}
