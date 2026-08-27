import { expect, test } from "bun:test"
import { botAdmissionProfile, recordBotAdmission } from "../src/bot-admission-profile"

test("admission presentation evidence is opt-in and retained within one explicit bound", () => {
  const host = globalThis as typeof globalThis & { __playsrcProfile?: { botAdmissionActive?: boolean; botAdmission?: unknown[]; botAdmissionDropped?: number } }
  const prior = host.__playsrcProfile
  try {
    host.__playsrcProfile = {}
    expect(botAdmissionProfile()).toBeUndefined()
    expect(host.__playsrcProfile.botAdmission).toBeUndefined()
    host.__playsrcProfile.botAdmissionActive = true
    const profile = botAdmissionProfile()!
    for (let index = 0; index < 4100; index++) recordBotAdmission(profile, "publication", BigInt(index), { actors: [{ actor: 2 }] })
    expect(profile.botAdmission).toHaveLength(4096)
    expect(profile.botAdmissionDropped).toBe(4)
    expect(profile.botAdmission?.[0]).toMatchObject({ stage: "publication", tick: "0", detail: { actors: [{ actor: 2 }] } })
  } finally {
    if (prior === undefined) delete host.__playsrcProfile
    else host.__playsrcProfile = prior
  }
})
