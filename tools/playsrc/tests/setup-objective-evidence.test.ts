import { expect, test } from "bun:test"
import { observeSetupObjectiveContacts } from "../profile/setup-objective-evidence"

test("rendered contact shorter than a coarse timer sample remains retained without changing publication", () => {
  const profile: Record<string, any> = { round: { state: 4, waitingForPlayers: false, inSetup: true }, bots: [{tick:"100"}] }
  observeSetupObjectiveContacts(profile)
  const touching = {points:[{playerCounts:[0,1],owner:2}]}, empty = {points:[{playerCounts:[0,0],owner:2}]}
  profile.controlPoints = touching
  expect(profile.setupObjectiveContacts).toEqual([])
  profile.round = {...profile.round, inSetup:false}
  profile.controlPoints = touching
  profile.controlPoints = empty
  expect(profile.controlPoints).toBe(empty)
  expect(profile.setupObjectiveContacts).toHaveLength(1)
  expect(profile.setupObjectiveContacts[0]).toMatchObject({tick:"100",points:touching,round:profile.round})
  for(let i=0;i<100;i++)profile.controlPoints=touching
  expect(profile.setupObjectiveContacts).toHaveLength(16)
})
