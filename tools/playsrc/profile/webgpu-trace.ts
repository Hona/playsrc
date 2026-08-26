import type { ChromiumTraceEvent } from "./compositor-truth"

type NativeEvent = ChromiumTraceEvent & Readonly<{ ph?: string; tts?: number; tdur?: number }>
type Slice = NativeEvent & Readonly<{ ts: number; dur: number }>
type Window = Readonly<{ startedMicroseconds: number; endedMicroseconds: number }>

/** Normalize synchronous Dawn B/E spans without treating asynchronous fences as
 * thread execution or joining independent GPU processes. */
export function nativeTraceSlices(events: readonly NativeEvent[]): Slice[] {
  const slices: Slice[] = []
  const stacks = new Map<string, NativeEvent[]>()
  for (const event of events.filter(e => Number.isFinite(e.ts)).toSorted((a, b) => a.ts! - b.ts!)) {
    if (event.ph === "X" && Number.isFinite(event.dur) && event.dur! >= 0) slices.push(event as Slice)
    if (event.ph !== "B" && event.ph !== "E") continue
    if (event.pid === undefined || event.tid === undefined) continue
    const key = `${event.pid}:${event.tid}`
    const stack = stacks.get(key) ?? []
    if (event.ph === "B") { stack.push(event); stacks.set(key, stack); continue }
    const begin = stack.pop()
    if (!begin || (event.name && begin.name !== event.name)) continue
    const cpu = Number.isFinite(begin.tts) && Number.isFinite(event.tts) ? event.tts! - begin.tts! : undefined
    slices.push({ ...begin, ph: "X", ts: begin.ts!, dur: event.ts! - begin.ts!, ...(cpu !== undefined && cpu >= 0 ? { tdur: cpu } : {}) })
  }
  return slices
}

export function summarizeWebGpuTrace(events: readonly NativeEvent[], window: Window) {
  if (!Number.isFinite(window.startedMicroseconds) || !Number.isFinite(window.endedMicroseconds)
    || window.endedMicroseconds <= window.startedMicroseconds) throw new Error("WebGPU sampling window is invalid")
  const threads = new Map(events.filter(e => e.name === "thread_name").map(e => [`${e.pid}:${e.tid}`, e.args?.name]))
  const slices = nativeTraceSlices(events)
  const commands = slices.filter(e => e.name === "WebGPU" && threads.get(`${e.pid}:${e.tid}`) === "CrGpuMain"
    && e.ts < window.endedMicroseconds && e.ts + e.dur > window.startedMicroseconds)
  const span = (e: Slice) => ({
    name: e.name ?? "unknown", pid: e.pid, tid: e.tid, startedMicroseconds: e.ts,
    ...(typeof e.args?.label === "string" ? { label: e.args.label } : {}),
    wallMilliseconds: e.dur / 1000,
    // tdur describes the complete span. Never prorate CPU to a clipped window.
    threadCpuMilliseconds: Number.isFinite(e.tdur) && e.tdur! >= 0 ? e.tdur! / 1000 : null,
  })
  return {
    evidence: commands.length ? "chromium-gpu-main-command-execution" : "unavailable",
    commands: commands.length,
    maximumStartedWithinActiveMilliseconds: commands.filter(e => e.ts >= window.startedMicroseconds).reduce((max, e) => Math.max(max, e.dur / 1000), 0),
    maximumOverlappingMilliseconds: commands.reduce((max, e) => Math.max(max, e.dur / 1000), 0),
    maximumActiveOverlapMilliseconds: commands.reduce((max, e) => Math.max(max,
      (Math.min(e.ts + e.dur, window.endedMicroseconds) - Math.max(e.ts, window.startedMicroseconds)) / 1000), 0),
    longest: commands.toSorted((a, b) => b.dur - a.dur).slice(0, 12).map(command => {
      const dawn = slices.filter(e => e.pid === command.pid && e.tid === command.tid && e.cat?.includes("dawn")
        && e.ts >= command.ts && e.ts + e.dur <= command.ts + command.dur)
      const byName = new Map<string, Slice[]>()
      for (const event of dawn) {
        const name = event.name ?? "unknown", list = byName.get(name) ?? []
        list.push(event); byName.set(name, list)
      }
      return {
        ...span(command),
        activeOverlapMilliseconds: (Math.min(command.ts + command.dur, window.endedMicroseconds) - Math.max(command.ts, window.startedMicroseconds)) / 1000,
        // Names can nest inside each other: never sum across rows. Within a
        // single name use interval union, not recursive inclusive duration.
        namedWork: [...byName].map(([name, list]) => {
          let end = -Infinity, microseconds = 0
          for (const event of list.toSorted((a, b) => a.ts - b.ts)) {
            microseconds += Math.max(0, event.ts + event.dur - Math.max(end, event.ts))
            end = Math.max(end, event.ts + event.dur)
          }
          return { name, count: list.length, unionMilliseconds: microseconds / 1000 }
        }).sort((a, b) => b.unionMilliseconds - a.unionMilliseconds).slice(0, 24),
        dawn: dawn.toSorted((a, b) => b.dur - a.dur).slice(0, 16).map(span),
      }
    }),
  }
}
