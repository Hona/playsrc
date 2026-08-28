/** One clock for effects sampled by actual client renders, not publications. */
export type ClientRenderFrame = Readonly<{
  clientFrame: number
  acceptedClientFrame: number
  clientFrameSeconds: number
  reset: boolean
  accept(): void
}>
export type RecordedClientRenderFrame = Readonly<{ clientFrame: number; acceptedClientFrame: number; nowSeconds: number; clientFrameSeconds: number; reset: boolean }>

export function createClientRenderFrameClock(record?: (frame: RecordedClientRenderFrame) => void) {
  let accepted = 0, preparation = 0, suspension = 0, previous: number | null = null
  return {
    suspend() { previous = null; suspension++ },
    prepare(nowSeconds: number): ClientRenderFrame {
      const reset = previous === null
      const clientFrameSeconds = previous === null ? 0 : nowSeconds - previous
      if (!Number.isFinite(nowSeconds) || nowSeconds < 0 || !Number.isFinite(Math.fround(clientFrameSeconds)) || clientFrameSeconds < 0 || accepted >= 0xffff_ffff) {
        throw new RangeError("Invalid client render frame clock")
      }
      const ticket = ++preparation, epoch = suspension, clientFrame = accepted + 1
      return Object.freeze({ clientFrame, acceptedClientFrame: accepted, clientFrameSeconds, reset,
        accept() {
          if (ticket !== preparation || accepted >= clientFrame) throw new RangeError("Client render frame acceptance is out of order")
          previous = epoch === suspension ? nowSeconds : null
          accepted = clientFrame
          record?.({ clientFrame, acceptedClientFrame: clientFrame - 1, nowSeconds, clientFrameSeconds, reset })
        },
      })
    },
  }
}

/** Replay the original client-frame clock inputs at their real deadlines. No
 * synthetic delta, frame-rate multiplier, extra frame or clock reset is used. */
export class RecordedClientRenderFrames {
  #next = 0
  #closed = false
  readonly observations: Array<{ frame: number; due: number; began: number; lateMilliseconds: number }> = []
  constructor(readonly frames: readonly RecordedClientRenderFrame[],
    private readonly now = () => performance.now(), private readonly wait = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))) {
    if (!frames.length || frames.length > 16384) throw new Error("Invalid recorded client-frame count")
    let previous = -Infinity
    for (const [index, frame] of frames.entries()) {
      if (frame.clientFrame !== index + 1 || frame.acceptedClientFrame !== index || !Number.isFinite(frame.nowSeconds)
        || frame.nowSeconds < previous || !Number.isFinite(frame.clientFrameSeconds) || frame.clientFrameSeconds < 0
        || typeof frame.reset !== "boolean") throw new Error("Invalid recorded client-frame input")
      previous = frame.nowSeconds
    }
  }
  close() { this.#closed = true }
  get ended() { return this.#next === this.frames.length }
  async next(epoch: number) {
    const frame = this.frames[this.#next]
    if (this.#closed || !frame || !Number.isFinite(epoch)) throw new Error("Recorded client-frame owner unavailable")
    const due = epoch + frame.nowSeconds * 1000
    while (!this.#closed && this.now() < due) await this.wait(Math.min(20, due - this.now()))
    if (this.#closed) throw new Error("Recorded client-frame cancelled")
    const began = this.now()
    this.observations.push({ frame: frame.clientFrame, due, began, lateMilliseconds: began - due })
    return frame
  }
  accept(frame: number) {
    if (this.#closed || this.frames[this.#next]?.clientFrame !== frame) throw new Error("Recorded client-frame acceptance differs")
    this.#next++
  }
}
