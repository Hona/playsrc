import { expect, test } from "bun:test"
import { PixelFeedbackLedger } from "../src/legacy-visuals"

test("a newly submitted proxy cannot overwrite an unread completed sample count",()=>{
  const ledger=new PixelFeedbackLedger()
  const pending={source:6,submission:1,visible:-1,possible:-1,clipFraction:1}
  ledger.submit(pending)
  expect(ledger.consume()).toEqual([pending])
  const complete={...pending,visible:17,possible:32}
  ledger.complete(complete)
  const next={...pending,submission:2,clipFraction:0.75}
  ledger.submit(next)
  // The real call order is GPU completion -> render the next frame -> prepare
  // native visibility. Losing the completed record here starves the fader.
  expect(ledger.snapshot()).toEqual([complete])
  expect(ledger.consume()).toEqual([complete])
  expect(ledger.consume()).toEqual([next])
  ledger.complete({...next,visible:0,possible:28})
  expect(ledger.consume()[0]).toMatchObject({submission:2,visible:0,possible:28,clipFraction:0.75})
  ledger.clear()
  expect(ledger.consume()).toEqual([])
})
