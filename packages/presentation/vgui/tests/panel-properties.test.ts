import { expect, test } from "bun:test"
import { PanelProperties } from "../src/panel-properties"

test("installed property reads preserve exact getters, first folded key, duplicate updates and order", () => {
  const properties = new PanelProperties([["WiDe", "10"], ["wide", "20"], ["WiDe", "30"], ["İ", "unicode"], ["empty", ""]])
  expect(properties.get("wide")).toBe("20")
  expect(properties.get("WIDE")).toBeUndefined()
  expect(properties.first("WIDE")).toBe("30")
  expect(properties.first("i")).toBeNull()
  expect(properties.first("EMPTY")).toBe("")
  expect(properties.signature()).toBe(JSON.stringify([...properties]))
  properties.delete("WiDe")
  expect(properties.first("wide")).toBe("20")
  properties.set("WiDe", "40")
  expect(properties.first("wide")).toBe("20")
  expect(properties.signature()).toBe(JSON.stringify([...properties]))
})

test("paint and publication share one property serialization, invalidated only by actual edits", () => {
  const properties = new PanelProperties([["image", 'a"\\b'], ["frame", "0"]])
  let iterations = 0
  const iterate = properties[Symbol.iterator].bind(properties)
  properties[Symbol.iterator] = () => { iterations++; return iterate() }
  const initial = properties.signature()
  expect(properties.signature()).toBe(initial)
  properties.set("frame", "0")
  properties.delete("absent")
  expect(properties.signature()).toBe(initial)
  expect(iterations).toBe(1)
  properties.set("frame", "1")
  expect(properties.signature()).not.toBe(initial)
  expect(iterations).toBe(2)
  properties.set("frame", "0")
  expect(properties.signature()).toBe(initial)
  properties.delete("image")
  expect(properties.signature()).toBe('[["frame","0"]]')
  properties.clear()
  expect(properties.signature()).toBe("[]")
  const cleared = iterations
  properties.clear()
  expect(properties.signature()).toBe("[]")
  expect(iterations).toBe(cleared)
})
