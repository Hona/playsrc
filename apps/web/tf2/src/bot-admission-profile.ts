type AdmissionProfile = { botAdmissionActive?: boolean; botAdmission?: unknown[]; botAdmissionDropped?: number }

export function botAdmissionProfile(): AdmissionProfile | undefined {
  const profile = (globalThis as typeof globalThis & { __playsrcProfile?: AdmissionProfile }).__playsrcProfile
  return profile?.botAdmissionActive ? profile : undefined
}

export function recordBotAdmission(profile: AdmissionProfile, stage: string, tick: bigint, detail: unknown): void {
  const records = profile.botAdmission ??= []
  if (records.length >= 4096) { profile.botAdmissionDropped = (profile.botAdmissionDropped ?? 0) + 1; return }
  records.push({ at: performance.now(), stage, tick: tick.toString(), detail })
}
