import { describe, expect, test } from "bun:test"
import { tokenizeSourceCommand } from "../src/console-command"

describe("Source developer-console command tokenization", () => {
  test("preserves quoted bot names, empty arguments, and adjacent quoted tokens", () => {
    expect(tokenizeSourceCommand('bot_teleport "Hat-Wearing MAN" 1 2 3 0 90 0'))
      .toEqual(["bot_teleport", "Hat-Wearing MAN", "1", "2", "3", "0", "90", "0"])
    expect(tokenizeSourceCommand('cl_crosshair_file ""')).toEqual(["cl_crosshair_file", ""])
    expect(tokenizeSourceCommand('"foo"bar')).toEqual(["foo", "bar"])
    expect(tokenizeSourceCommand('say "unfinished name')).toEqual(["say", "unfinished name"])
  })

  test("matches Source whitespace, line comments, and the six default break characters", () => {
    expect(tokenizeSourceCommand("\talpha // ignore this line\n beta{}()':gamma"))
      .toEqual(["alpha", "beta", "{", "}", "(", ")", "'", ":", "gamma"])
    expect(tokenizeSourceCommand("one\0ignored")).toEqual(["one"])
    expect(Object.isFrozen(tokenizeSourceCommand("one two"))).toBe(true)
  })
})
