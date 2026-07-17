import { encodeJumpCourse, type JumpCourseZone } from "@playsrc/game-tf2-browser/codec"
const checkpoints = [
  87, 94, 96, 101, 106, 111, 116, 120, 124, 129, 134, 138, 141, 144, 239, 241, 246, 250, 253,
] as const
export function jumpBeefCourse(hash: string) {
  const zones: JumpCourseZone[] = [
    { identity: 1, triggerEntity: 316, kind: "start", index: 1 },
    ...checkpoints.map((triggerEntity, index) => ({
      identity: index + 2,
      triggerEntity,
      kind: "checkpoint" as const,
      index: index + 1,
    })),
    { identity: checkpoints.length + 2, triggerEntity: 257, kind: "end", index: 1 },
  ]
  return encodeJumpCourse(0x6a756d705f626565n, hash, zones)
}
