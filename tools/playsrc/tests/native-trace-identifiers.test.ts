import { expect, test } from "bun:test"
import { gzipSync } from "node:zlib"
import { decodeRawTrace } from "../profile/compositor-evidence"

test("native uint64 surface identities remain distinct beyond JS integer precision", () => {
  const events = decodeRawTrace(gzipSync('{"traceEvents":[{"name":"Graphics.Pipeline","ts":417462769262,"args":{"surface_frame_trace_id":370248778665928881}},{"name":"Graphics.Pipeline","ts":417462769263,"args":{"surface_frame_trace_id":370248778665928882,"aggregated_surface_frame_trace_ids":[370248778665928881,370248778665928882]}}]}'))
  expect(events[0]!.ts).toBe(417462769262)
  expect(events[0]!.args!.surface_frame_trace_id).toBe("370248778665928881")
  expect(events[1]!.args!.surface_frame_trace_id).toBe("370248778665928882")
  expect(events[1]!.args!.aggregated_surface_frame_trace_ids).toEqual(["370248778665928881", "370248778665928882"])
})
