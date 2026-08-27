/** One clock for effects sampled by actual client renders, not publications. */
export type ClientRenderFrame = Readonly<{
  clientFrame: number
  acceptedClientFrame: number
  clientFrameSeconds: number
  accept(): void
}>

export function createClientRenderFrameClock() {
  let accepted = 0, preparation = 0, suspension = 0, previous: number | null = null
  return {
    suspend() { previous = null; suspension++ },
    prepare(nowSeconds: number): ClientRenderFrame {
      const clientFrameSeconds = previous === null ? 0 : nowSeconds - previous
      if (!Number.isFinite(nowSeconds) || nowSeconds < 0 || !Number.isFinite(Math.fround(clientFrameSeconds)) || clientFrameSeconds < 0 || accepted >= 0xffff_ffff) {
        throw new RangeError("Invalid client render frame clock")
      }
      const ticket = ++preparation, epoch = suspension, clientFrame = accepted + 1
      return Object.freeze({ clientFrame, acceptedClientFrame: accepted, clientFrameSeconds,
        accept() {
          if (ticket !== preparation || accepted >= clientFrame) throw new RangeError("Client render frame acceptance is out of order")
          previous = epoch === suspension ? nowSeconds : null
          accepted = clientFrame
        },
      })
    },
  }
}
