import { describe, expect, test } from "bun:test"

import { rendersVisibly } from "./visibility"

const node = (
  visible: boolean,
  parent?: Record<string, unknown>,
): Record<string, unknown> => ({ visible, parent })

describe("rendersVisibly", () => {
  test("a switched-on node with no parent renders", () => {
    expect(rendersVisibly({ visible: true })).toBe(true)
  })

  test("a switched-off node does not render", () => {
    expect(rendersVisibly({ visible: false })).toBe(false)
  })

  test("an absent visible is treated as switched on, matching the Figma default", () => {
    expect(rendersVisibly({})).toBe(true)
  })

  test("a switched-on node inside a switched-off parent does not render", () => {
    // The case a naive `node.visible` check misses, and the whole reason the
    // predicate walks the chain rather than reading one field.
    expect(rendersVisibly(node(true, node(false)))).toBe(false)
  })

  test("a switched-on node several levels under a switched-off ancestor does not render", () => {
    expect(
      rendersVisibly(node(true, node(true, node(true, node(false))))),
    ).toBe(false)
  })

  test("a non-record is treated as past the root rather than hidden", () => {
    expect(rendersVisibly(undefined)).toBe(true)
  })

  test("a chain longer than the walk limit is treated as not rendering", () => {
    // Fails safe: an unreadably deep chain is refused rather than assumed live.
    let current: Record<string, unknown> = { visible: true }
    for (let step = 0; step < 200; step += 1) {
      current = { visible: true, parent: current }
    }
    expect(rendersVisibly(current)).toBe(false)
  })
})
