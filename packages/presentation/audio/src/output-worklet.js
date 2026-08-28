/* Browser device boundary only. Source computation stays in Rust. */
registerProcessor("playsrc-output", class extends AudioWorkletProcessor {
  constructor(options) {
    super()
    const { buffer, capacity } = options.processorOptions
    if (sampleRate !== 44100 || !(buffer instanceof SharedArrayBuffer) || !Number.isInteger(capacity)
      || capacity < 8192 || capacity > 1048576 || (capacity & (capacity - 1)) !== 0 || buffer.byteLength !== 32 + capacity * 8) {
      throw new Error("Invalid Source audio device buffer")
    }
    this.control = new Int32Array(buffer, 0, 8)
    this.samples = new Float32Array(buffer, 32)
    this.mask = capacity - 1
    this.epoch = Atomics.load(this.control, 2) >>> 0
    this.capture = null
    this.diagnostics = options.processorOptions.diagnostics === true
    this.gap = null
    this.diagnosticCount = 0
    this.lastNonzeroTime = null
    this.port.onmessage = ({ data }) => {
      if (data.cancelCapture !== undefined) {
        if (this.capture?.id === data.cancelCapture) this.capture = null
        return
      }
      if (!Number.isInteger(data.captureId) || data.captureId < 1 || data.captureId > 0xffff_ffff || !Number.isInteger(data.captureFrames) || data.captureFrames < 1 || data.captureFrames > 441000 || this.capture) {
        this.port.postMessage({ captureId: data.captureId, error: "Invalid audio capture request" }); return
      }
      this.capture = { id: data.captureId, samples: new Float32Array(data.captureFrames * 2), at: 0,
        startRead: Atomics.load(this.control, 0) >>> 0, epoch: Atomics.load(this.control, 2) >>> 0,
        gaps: new Uint32Array(8192), gapCount: 0, missing: 0 }
    }
  }
  process(_inputs, outputs) {
    const [left, right] = outputs[0]
    const epoch = Atomics.load(this.control, 2) >>> 0
    if (epoch !== this.epoch) {
      // Only the consumer writes its cursor. Acknowledge discarding old PCM
      // before the producer can reuse any slot for the replacement map.
      Atomics.store(this.control, 0, Atomics.load(this.control, 1))
      this.epoch = epoch
      this.lastNonzeroTime = null
      Atomics.store(this.control, 6, epoch | 0)
    }
    const read = Atomics.load(this.control, 0) >>> 0
    const written = Atomics.load(this.control, 1) >>> 0
    const enabled = Atomics.load(this.control, 4) !== 0
    const available = (written - read) >>> 0
    if (available > this.mask + 1) throw new Error("Audio device ownership overflow")
    const count = enabled ? Math.min(left.length, available) : 0
    for (let frame = 0; frame < count; frame++) {
      const at = ((read + frame) & this.mask) * 2
      left[frame] = this.samples[at]; right[frame] = this.samples[at + 1]
    }
    left.fill(0, count); right.fill(0, count)
    if (epoch !== (Atomics.load(this.control, 2) >>> 0)) { left.fill(0); right.fill(0); return true }
    Atomics.store(this.control, 0, (read + count) | 0)
    Atomics.add(this.control, 5, left.length)
    if (enabled && count < left.length) Atomics.add(this.control, 3, left.length - count)
    if (this.diagnostics) {
      for (let frame = 0; frame < count; frame++) {
        if (left[frame] !== 0 || right[frame] !== 0) this.lastNonzeroTime = currentTime + frame / sampleRate
      }
      if (enabled && count < left.length) {
        if (!this.gap) this.gap = { start: currentTime + count / sampleRate, epoch, read, written, lastNonzeroTime: this.lastNonzeroTime, frames: 0 }
        this.gap.frames += left.length - count
      } else if (this.gap) {
        if (this.diagnosticCount++ < 64) this.port.postMessage({ deviceGap: { ...this.gap, end: currentTime, resumedEnabled: enabled, resumedRead: read, resumedWrite: written } })
        this.gap = null
      }
    }
    if (this.capture) {
      const capture = this.capture
      if (capture.epoch !== epoch) { this.capture = null; this.port.postMessage({ captureId: capture.id, error: "Audio capture crossed map ownership" }); return true }
      const frames = Math.min(left.length, capture.samples.length / 2 - capture.at)
      if (count < frames) {
        const start = capture.at + count, missing = frames - count
        if (capture.gapCount > 0 && capture.gaps[capture.gapCount - 2] + capture.gaps[capture.gapCount - 1] === start) {
          capture.gaps[capture.gapCount - 1] += missing
        } else {
          if (capture.gapCount + 2 > capture.gaps.length) { this.capture = null; this.port.postMessage({ captureId: capture.id, error: "Audio capture gap bound exceeded" }); return true }
          capture.gaps[capture.gapCount++] = start; capture.gaps[capture.gapCount++] = missing
        }
        capture.missing += missing
      }
      for (let frame = 0; frame < frames; frame++) {
        capture.samples[(capture.at + frame) * 2] = left[frame]
        capture.samples[(capture.at + frame) * 2 + 1] = right[frame]
      }
      capture.at += frames
      if (capture.at * 2 === capture.samples.length) {
        this.capture = null
        this.port.postMessage({ captureId: capture.id, capture: capture.samples.buffer, startRead: capture.startRead, epoch,
          underruns: capture.missing, gaps: capture.gaps.buffer, gapCount: capture.gapCount }, [capture.samples.buffer, capture.gaps.buffer])
      }
    }
    return true
  }
})
