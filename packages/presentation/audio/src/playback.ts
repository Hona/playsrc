import { AudioError } from "./error"
import type { Listener, NeutralVoice, SoundSource } from "./source"

export type PcmResource = Readonly<{ identity: string; sampleRate: number; numberOfChannels: number; bits: number; length: number; duration: number; loopStartSeconds: number | null }>
export type SoundscapeSelection = Readonly<{ entity: number; soundscape: number; positionBits: number; positions: readonly (readonly [number, number, number])[] }>
export type AudioRandom = Readonly<{ nextUnit(): number; nextInteger(low: number, high: number): number }>
export type AudioEntity = Readonly<{ domain: 1 | 2; identity: number; origin: readonly [number, number, number] }>
export type AudioCapture = Readonly<{ pcm: ArrayBuffer; frames: number; differingSamples: number; uncoveredSamples: number; underruns: number; sampleRate: number }>
type Exports = WebAssembly.Exports & {
  memory: WebAssembly.Memory
  playsrc_audio_alloc(length: number): number
  playsrc_audio_free(pointer: number, length: number): void
  playsrc_audio_stage(): number
  playsrc_audio_resource(name: number, nameLength: number, bytes: number, length: number): number
  playsrc_audio_needed_documents(): number
  playsrc_audio_commit(): number
  playsrc_audio_frame(pointer: number, length: number): number
  playsrc_audio_scene(pointer: number, length: number): number
  playsrc_audio_start(pointer: number, length: number): number
  playsrc_audio_stop(identity: number): void
  playsrc_audio_reset(): void
  playsrc_audio_dispose(): void
  playsrc_audio_paint(frames: number): number
  playsrc_audio_pcm_data(): number
  playsrc_audio_pcm_count(): number
  playsrc_audio_active_count(): number
  playsrc_audio_active_data(): number
  playsrc_audio_voice_count(): number
  playsrc_audio_room(): number
  playsrc_audio_environment_starts(): number
  playsrc_audio_mp3_frames(): bigint
  playsrc_audio_soundscape(): number
  playsrc_audio_underwater(): number
  playsrc_audio_room_observation(): number
  playsrc_audio_output_length(): number
  playsrc_audio_output_data(): number
  playsrc_audio_error_length(): number
  playsrc_audio_error_data(): number
}
class Packet {
  readonly data: DataView
  at = 0
  constructor(readonly bytes: Uint8Array) { this.data = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) }
  u32(value: number) { this.data.setUint32(this.at, value, true); this.at += 4 }
  f32(value: number) { this.data.setFloat32(this.at, value, true); this.at += 4 }
  f64(value: number) { this.data.setFloat64(this.at, value, true); this.at += 8 }
  vector(value: readonly number[]) { for (const component of value) this.f32(component) }
  text(value: Uint8Array) { this.u32(value.length); this.bytes.set(value, this.at); this.at += value.length }
}
const encoded = new TextEncoder(), decoded = new TextDecoder("utf-8", { fatal: true })
const moduleCache = new Map<string, Promise<WebAssembly.Module>>()
const CAPACITY = 16384, AHEAD = 4408

export async function createSourceAudioSystem(context: AudioContext, moduleUrl: URL, resources: ReadonlyMap<string, Uint8Array>, random: AudioRandom, diagnostics = false) {
  if (context.sampleRate !== 44100 || !context.audioWorklet || typeof AudioWorkletNode !== "function" || !crossOriginIsolated) {
    throw new AudioError("BrowserFailure", "Source audio requires a 44.1 kHz isolated audio device")
  }
  let module = moduleCache.get(moduleUrl.href)
  if (!module) {
    module = fetch(moduleUrl).then(async response => {
      if (!response.ok) throw new AudioError("BrowserFailure", "Audio module is unavailable")
      const bytes = await response.arrayBuffer()
      if (bytes.byteLength > 8 * 1024 * 1024) throw new AudioError("Capacity", "Audio module exceeds its bound")
      return WebAssembly.compile(bytes)
    })
    moduleCache.set(moduleUrl.href, module)
    void module.catch(() => moduleCache.delete(moduleUrl.href))
  }
  const instance = await WebAssembly.instantiate(await module, { playsrc_audio: {
    random_float: (low: number, high: number) => Math.fround(low + Math.fround(Math.fround(high - low) * Math.fround(random.nextUnit()))),
    random_integer: (low: number, high: number) => random.nextInteger(low, high),
  } })
  const wasm = instance.exports as Exports
  const buffer = new SharedArrayBuffer(32 + CAPACITY * 8)
  const control = new Int32Array(buffer, 0, 8), samples = new Float32Array(buffer, 32)
  await context.audioWorklet.addModule(new URL("./output-worklet.js", import.meta.url))
  const output = new AudioWorkletNode(context, "playsrc-output", { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [2], processorOptions: { buffer, capacity: CAPACITY, diagnostics } })
  output.connect(context.destination)
  let closed = false, failure: Error | undefined, epoch = 0, previousTime: number | undefined, pending = false, ready = false
  let inventory = new Map<string, PcmResource>()
  const queued: Array<{ voice: NeutralVoice; source: SoundSource } | { stop: number }> = []
  let capture: { id: number; base: number; parts: Int16Array[]; frames: number; resolve(value: AudioCapture): void; reject(error: Error): void; timer: ReturnType<typeof setTimeout> } | undefined
  let captureSequence = 0
  const metrics = { paintCalls: 0, extraPaintCalls: 0, paintedFrames: 0, paintMilliseconds: 0, maximumPaintMilliseconds: 0 }
  const lifecycle: Record<string, unknown>[] = []
  let lastPaint: number | undefined
  let nextExtraUpdate = 0
  const record = (event: string, fields: Record<string, unknown> = {}) => {
    if (diagnostics && lifecycle.length < 256) lifecycle.push({ event, wallTime: performance.now(), audioTime: context.currentTime, epoch, pending, ready, contextState: context.state, ...fields })
  }
  output.onprocessorerror = () => {
    failure = new AudioError("BrowserFailure", "Source audio device processor failed")
    if (capture) { clearTimeout(capture.timer); capture.reject(failure); capture = undefined }
  }

  function bytes(pointer: number, length: number): Uint8Array {
    if (!Number.isSafeInteger(pointer) || pointer < 0 || !Number.isSafeInteger(length) || length < 0 || pointer > wasm.memory.buffer.byteLength - length) throw new AudioError("BrowserFailure", "Audio module memory bounds")
    return new Uint8Array(wasm.memory.buffer, pointer, length)
  }
  function check(value: number, operation: string): void {
    if (value === 1) return
    const length = wasm.playsrc_audio_error_length()
    const reason = decoded.decode(bytes(wasm.playsrc_audio_error_data() >>> 0, length))
    throw new AudioError("BrowserFailure", `${operation}: ${reason}`)
  }
  function withBytes<T>(input: Uint8Array, action: (pointer: number, length: number) => T): T {
    const pointer = wasm.playsrc_audio_alloc(input.length) >>> 0
    if (!pointer && input.length > 0) throw new AudioError("Capacity", "Audio input allocation failed")
    try { bytes(pointer, input.length).set(input); return action(pointer, input.length) }
    finally { wasm.playsrc_audio_free(pointer, input.length) }
  }
  function result(): Uint8Array { return bytes(wasm.playsrc_audio_output_data() >>> 0, wasm.playsrc_audio_output_length()).slice() }
  function reader(value: Uint8Array) {
    const data = new DataView(value.buffer, value.byteOffset, value.byteLength); let at = 0
    return { number() { if (at + 4 > value.length) throw new AudioError("MalformedResource", "Truncated audio metadata"); const number = data.getUint32(at, true); at += 4; return number },
      text() { const length = this.number(); if (length > 1024 || at + length > value.length) throw new AudioError("MalformedResource", "Audio metadata string bound"); const text = decoded.decode(value.subarray(at, at + length)); at += length; return text },
      done() { if (at !== value.length) throw new AudioError("MalformedResource", "Trailing audio metadata") } }
  }
  function flushDevice(): void {
    record("retire", { read: Atomics.load(control, 0) >>> 0, write: Atomics.load(control, 1) >>> 0 })
    Atomics.store(control, 4, 0); epoch = (epoch + 1) >>> 0; Atomics.store(control, 2, epoch)
    previousTime = undefined; pending = false; ready = false; queued.length = 0
    lastPaint = undefined
    if (capture) {
      clearTimeout(capture.timer); output.port.postMessage({ cancelCapture: capture.id })
      capture.reject(new AudioError("Closed", "Audio capture crossed map ownership")); capture = undefined
    }
  }
  function replace(input: ReadonlyMap<string, Uint8Array>): ReadonlyMap<string, PcmResource> {
    if (closed) throw new AudioError("Closed", "Audio system is closed")
    check(wasm.playsrc_audio_stage(), "Stage audio")
    const uploaded = new Set<string>()
    const upload = (identity: string, required: boolean) => {
      if (uploaded.has(identity)) return
      const resource = input.get(identity)
      if (!resource) { if (required) throw new AudioError("MissingResource", `Audio dependency ${identity} is missing`); return }
      withBytes(encoded.encode(identity), (name, nameLength) => withBytes(resource, (pointer, length) => check(wasm.playsrc_audio_resource(name, nameLength, pointer, length), identity)))
      uploaded.add(identity)
    }
    upload("derived/soundscape-map.pssm", true); upload("scripts/soundscapes_manifest.txt", true)
    check(wasm.playsrc_audio_needed_documents(), "Resolve audio documents")
    const documents = reader(result()), documentCount = documents.number()
    if (documentCount > 256) throw new AudioError("Capacity", "Audio document count exceeds bound")
    for (let index = 0; index < documentCount; index++) upload(documents.text(), false)
    documents.done()
    for (const identity of input.keys()) if (identity.startsWith("sound/") && /\.(wav|mp3)$/u.test(identity)) upload(identity, true)
    check(wasm.playsrc_audio_commit(), "Commit audio map")
    const info = reader(result()), count = info.number(), next = new Map<string, PcmResource>()
    if (count > 4096) throw new AudioError("Capacity", "Audio resource count exceeds bound")
    for (let index = 0; index < count; index++) {
      const identity = info.text(), sampleRate = info.number(), numberOfChannels = info.number(), bits = info.number(), length = info.number(), loop = info.number()
      next.set(identity, Object.freeze({ identity, sampleRate, numberOfChannels, bits, length, duration: length / sampleRate, loopStartSeconds: loop === 0xffff_ffff ? null : loop / sampleRate }))
    }
    info.done(); flushDevice(); inventory = next; return inventory
  }
  function start(voice: NeutralVoice, source: SoundSource): void {
    const decorators: Record<string, string> = { stream: "*", voice: "?", sentence: "!", dry: "#", doppler: ">", directional: "<", "distance-variant": "^", omni: "@", "spatial-stereo": ")", "fast-pitch": "}" }
    const wave = encoded.encode(voice.decorators.map(value => decorators[value]).join("") + voice.resource.replace(/^sound\//u, ""))
    const sourceClass = encoded.encode(source.sourceClass)
    const packet = new Packet(new Uint8Array(88 + wave.length + sourceClass.length))
    packet.u32(voice.identity); packet.text(wave); packet.f32(voice.volume); packet.u32(voice.pitch); packet.u32(voice.soundLevel)
    const player = source.sourceClass === "player" || source.sourceClass === "tf_player"
    packet.u32(Number(source.kind === "local-listener" || source.kind === "ui" || source.kind === "entity" && player && source.identity === 1))
    packet.u32(source.kind === "entity" ? player ? 1 : 2 : 0); packet.u32(source.kind === "entity" ? source.identity : 0)
    packet.u32(Number(source.origin !== null))
    packet.vector(source.origin ?? [0, 0, 0]); packet.f32(source.radius); packet.u32(voice.channel)
    packet.f32(voice.offsetSeconds); packet.f32(Math.max(0, voice.startTimeSeconds - context.currentTime))
    packet.u32(Number(voice.envelope !== undefined))
    if (voice.envelope) { packet.f32(voice.envelope.from); packet.f32(voice.envelope.to); packet.f32(voice.envelope.seconds) }
    packet.text(sourceClass)
    withBytes(packet.bytes.subarray(0, packet.at), (pointer, length) => check(wasm.playsrc_audio_start(pointer, length), "Start audio voice"))
  }
  function paint(): void {
    if ((Atomics.load(control, 6) >>> 0) !== epoch) return
    const read = Atomics.load(control, 0) >>> 0, write = Atomics.load(control, 1) >>> 0, available = (write - read) >>> 0
    if (available > CAPACITY) throw new AudioError("BrowserFailure", "Audio ring ownership differs")
    const frames = Math.max(0, AHEAD - available) & ~3
    if (frames === 0) return
    const started = performance.now(); check(wasm.playsrc_audio_paint(frames), "Paint audio")
    const count = wasm.playsrc_audio_pcm_count()
    if (count !== frames * 2) throw new AudioError("BrowserFailure", "Audio paint frame count differs")
    const pcm = new Int16Array(wasm.memory.buffer, wasm.playsrc_audio_pcm_data() >>> 0, count)
    if (capture) capture.parts.push(pcm.slice())
    for (let frame = 0; frame < frames; frame++) {
      const at = ((write + frame) & (CAPACITY - 1)) * 2
      samples[at] = pcm[frame * 2]! / 32768; samples[at + 1] = pcm[frame * 2 + 1]! / 32768
    }
    Atomics.store(control, 1, (write + frames) | 0); Atomics.store(control, 4, 1)
    if (diagnostics) {
      const now = context.currentTime
      if (lastPaint === undefined || now - lastPaint > 0.08) record("paint-gap", { previousAudioTime: lastPaint, read, write, frames, previousAvailable: available, activeVoices: wasm.playsrc_audio_voice_count() })
      lastPaint = now
    }
    const duration = performance.now() - started
    metrics.paintCalls++; metrics.paintedFrames += frames; metrics.paintMilliseconds += duration; metrics.maximumPaintMilliseconds = Math.max(metrics.maximumPaintMilliseconds, duration)
    nextExtraUpdate = performance.now() + 90
  }
  output.port.onmessage = ({ data }) => {
    if (data.deviceGap) { record("device-gap", data.deviceGap); return }
    const current = capture
    if (!current || data.captureId !== current.id) return
    capture = undefined; clearTimeout(current.timer)
    if (data.error || !(data.capture instanceof ArrayBuffer) || data.epoch !== epoch || data.capture.byteLength !== current.frames * 4) {
      current.reject(new AudioError("BrowserFailure", data.error ?? "Audio capture framing differs")); return
    }
    const actual = new Int16Array(data.capture), offset = (((data.startRead >>> 0) - current.base) >>> 0) * 2
    if (!(data.gaps instanceof ArrayBuffer) || !Number.isInteger(data.gapCount) || data.gapCount < 0 || data.gapCount % 2 !== 0 || data.gapCount * 4 > data.gaps.byteLength) {
      current.reject(new AudioError("BrowserFailure", "Audio capture gaps are malformed")); return
    }
    const gaps = new Uint32Array(data.gaps, 0, data.gapCount)
    let end = 0, missing = 0
    for (let at = 0; at < gaps.length; at += 2) {
      if (gaps[at]! < end || gaps[at + 1] === 0 || gaps[at]! + gaps[at + 1]! > current.frames) { current.reject(new AudioError("BrowserFailure", "Audio capture gap range differs")); return }
      end = gaps[at]! + gaps[at + 1]!; missing += gaps[at + 1]!
    }
    if (missing !== data.underruns) { current.reject(new AudioError("BrowserFailure", "Audio capture gap count differs")); return }
    let part = 0, cursor = offset, differingSamples = 0, uncoveredSamples = 0, gap = 0
    for (let frame = 0; frame < current.frames; frame++) {
      if (gap < gaps.length && frame === gaps[gap]) {
        for (let index = frame * 2; index < (frame + gaps[gap + 1]!) * 2; index++) differingSamples += Number(actual[index] !== 0)
        frame += gaps[gap + 1]! - 1; gap += 2; continue
      }
      for (let channel = 0; channel < 2; channel++) {
        while (part < current.parts.length && cursor >= current.parts[part]!.length) { cursor -= current.parts[part]!.length; part++ }
        if (part === current.parts.length) uncoveredSamples++
        else differingSamples += Number(current.parts[part]![cursor++] !== actual[frame * 2 + channel])
      }
    }
    current.resolve(Object.freeze({ pcm: data.capture, frames: current.frames, differingSamples, uncoveredSamples, underruns: data.underruns, sampleRate: 44100 }))
  }

  try { replace(resources) }
  catch (error) { output.disconnect(); output.port.close(); wasm.playsrc_audio_dispose(); throw error }
  return Object.freeze({
    resources: () => inventory as ReadonlyMap<string, PcmResource>, replace,
    playNeutral(voice: NeutralVoice, source: SoundSource) {
      if (closed) throw new AudioError("Closed", "Audio system is closed")
      if (queued.length >= 4096) throw new AudioError("Capacity", "Audio command queue exceeded")
      queued.push({ voice, source })
    },
    stop(identity: number) {
      if (!Number.isSafeInteger(identity) || identity < 1 || identity > 0xffff_ffff) throw new AudioError("MalformedEvent", "Invalid audio voice identity")
      if (!closed) { if (queued.length >= 4096) throw new AudioError("Capacity", "Audio command queue exceeded"); queued.push({ stop: identity }) }
    },
    activeVoices(): readonly number[] {
      if (closed) return Object.freeze([])
      const count = wasm.playsrc_audio_active_count()
      const active = new Set(new Uint32Array(wasm.memory.buffer, wasm.playsrc_audio_active_data() >>> 0, count))
      for (const command of queued) if ("stop" in command) active.delete(command.stop); else active.add(command.voice.identity)
      return Object.freeze([...active].sort((a, b) => a - b))
    },
    frame(selection: SoundscapeSelection, listener: Listener, gameTime: number, hostTime: number, volume: number, entities: readonly AudioEntity[]): Readonly<{ input: ArrayBuffer; accept(reply: ArrayBuffer): void }> | undefined {
      if (failure) throw failure
      if (closed || context.state !== "running") { previousTime = undefined; return undefined }
      if (pending) throw new AudioError("BrowserFailure", "Overlapping audio scene transaction")
      for (const command of queued.splice(0)) if ("stop" in command) wasm.playsrc_audio_stop(command.stop); else start(command.voice, command.source)
      if (selection.positions.length !== 8) throw new AudioError("MalformedEvent", "Soundscape positions differ")
      if (entities.length > 512) throw new AudioError("Capacity", "Audio entity count exceeds bound")
      const packet = new Packet(new Uint8Array(168 + entities.length * 20))
      packet.u32(selection.entity); packet.u32(selection.soundscape); packet.u32(selection.positionBits)
      for (const position of selection.positions) packet.vector(position)
      packet.vector(listener.origin); packet.vector(listener.forward); packet.vector(listener.right)
      packet.f32(previousTime === undefined ? 0 : hostTime - previousTime); packet.f32(gameTime); packet.f64(hostTime); packet.f32(volume)
      packet.u32(entities.length)
      for (const entity of entities) { packet.u32(entity.domain); packet.u32(entity.identity); packet.vector(entity.origin) }
      withBytes(packet.bytes, (pointer, length) => check(wasm.playsrc_audio_frame(pointer, length), "Update soundscape"))
      previousTime = hostTime; pending = true
      const owner = epoch
      return Object.freeze({ input: result().buffer as ArrayBuffer, accept(reply: ArrayBuffer) {
        if (closed || owner !== epoch) return
        withBytes(new Uint8Array(reply), (pointer, length) => check(wasm.playsrc_audio_scene(pointer, length), "Accept audio scene"))
        pending = false; ready = true; paint()
      } })
    },
    pump() {
      if (failure) throw failure
      if (!closed && ready && context.state === "running") paint()
    },
    extraUpdate() {
      if (performance.now() >= nextExtraUpdate && !closed && ready && context.state === "running") {
        const before = metrics.paintCalls
        paint()
        metrics.extraPaintCalls += Number(metrics.paintCalls !== before)
      }
    },
    reset() { if (!closed) { wasm.playsrc_audio_reset(); flushDevice() } },
    async resume() {
      if (closed) throw new AudioError("Closed", "Audio system is closed")
      previousTime = undefined; await context.resume()
      if (closed) throw new AudioError("Closed", "Audio system is closed")
      if (context.state !== "running") throw new AudioError("Suspended", "Audio context did not enter running state")
    },
    async close() { if (closed) return; closed = true; flushDevice(); output.disconnect(); output.port.close(); wasm.playsrc_audio_dispose(); if (context.state !== "closed") await context.close() },
    stats() {
      const pointer = wasm.playsrc_audio_room_observation() >>> 0
      const observation = pointer ? Array.from(new Float32Array(wasm.memory.buffer, pointer, 12)) : null
      return Object.freeze({ ...metrics, contextState: context.state, sampleRate: context.sampleRate, channels: 2,
      renderedFrames: Atomics.load(control, 5) >>> 0, underrunFrames: Atomics.load(control, 3) >>> 0, queuedFrames: ((Atomics.load(control, 1) >>> 0) - (Atomics.load(control, 0) >>> 0)) >>> 0,
      activeVoices: wasm.playsrc_audio_voice_count(), room: wasm.playsrc_audio_room(), wasmBytes: wasm.memory.buffer.byteLength,
      soundscape: wasm.playsrc_audio_soundscape(), environmentStarts: wasm.playsrc_audio_environment_starts(), mp3Frames: Number(wasm.playsrc_audio_mp3_frames()),
      underwater: wasm.playsrc_audio_underwater() !== 0, roomObservation: observation,
      decodedBytes: [...inventory.values()].reduce((total, clip) => total + clip.length * clip.numberOfChannels * 2, 0), epoch,
      ...(diagnostics ? { lifecycle: lifecycle.slice() } : {}) }) },
    capture(frames = 220500): Promise<AudioCapture> {
      if (closed || context.state !== "running" || capture || captureSequence >= 0xffff_ffff || (Atomics.load(control, 6) >>> 0) !== epoch || !Number.isInteger(frames) || frames < 1 || frames > 441000) return Promise.reject(new AudioError("MalformedEvent", "Audio capture is unavailable"))
      const base = Atomics.load(control, 0) >>> 0, end = Atomics.load(control, 1) >>> 0, buffered = (end - base) >>> 0
      const initial = new Int16Array(buffered * 2)
      for (let frame = 0; frame < buffered; frame++) { const at = ((base + frame) & (CAPACITY - 1)) * 2; initial[frame * 2] = samples[at]! * 32768; initial[frame * 2 + 1] = samples[at + 1]! * 32768 }
      return new Promise((resolve, reject) => {
        const id = ++captureSequence
        const timer = setTimeout(() => {
          if (capture?.id !== id) return
          capture = undefined; output.port.postMessage({ cancelCapture: id })
          reject(new AudioError("BrowserFailure", "Audio capture timed out"))
        }, 12_000)
        capture = { id, base, parts: [initial], frames, resolve, reject, timer }
        output.port.postMessage({ captureId: capture.id, captureFrames: frames })
      })
    },
  })
}
