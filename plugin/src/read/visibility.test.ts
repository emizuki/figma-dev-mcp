import { describe, expect, test } from "bun:test"

import { rendersVisibly, visibilityOf } from "./visibility"

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

  test("a chain deeper than any real document still renders", () => {
    // The bound is the one way this predicate drops a node that genuinely
    // renders, and `get_selection` and `get_design_context` have no error slot
    // to say so. So it sits far above real documents: 500 ancestors is already
    // an order of magnitude past the deepest nesting measured in a production
    // file, and must still come back as rendering.
    let current: Record<string, unknown> = { visible: true }
    for (let step = 0; step < 500; step += 1) {
      current = { visible: true, parent: current }
    }
    expect(rendersVisibly(current)).toBe(true)
  })

  test("a chain longer than the walk limit is treated as not rendering", () => {
    // Fails safe: an unreadably deep chain is refused rather than assumed live.
    // The bound still exists — a cycle has to terminate somewhere — it just
    // sits where only a pathological tree can reach it.
    let current: Record<string, unknown> = { visible: true }
    for (let step = 0; step < 1100; step += 1) {
      current = { visible: true, parent: current }
    }
    expect(rendersVisibly(current)).toBe(false)
  })
})

describe("visibilityOf", () => {
  test("a live chain reports renders", () => {
    expect(visibilityOf({ visible: true, parent: { visible: true } })).toBe(
      "renders",
    )
  })

  test("a switched-off ancestor reports hidden, not undetermined", () => {
    expect(visibilityOf({ visible: true, parent: { visible: false } })).toBe(
      "hidden",
    )
  })

  test("a cyclic chain reports undetermined, not hidden", () => {
    const looped: Record<string, unknown> = { visible: true }
    looped.parent = looped
    expect(visibilityOf(looped)).toBe("undetermined")
  })

  test("a chain past the walk limit reports undetermined", () => {
    let current: Record<string, unknown> = { visible: true }
    for (let step = 0; step < 1100; step += 1) {
      current = { visible: true, parent: current }
    }
    expect(visibilityOf(current)).toBe("undetermined")
  })

  test("rendersVisibly answers false for both hidden and undetermined", () => {
    const looped: Record<string, unknown> = { visible: true }
    looped.parent = looped
    expect(rendersVisibly({ visible: false })).toBe(false)
    expect(rendersVisibly(looped)).toBe(false)
  })

  test("a cycle is cut at its own length rather than at the walk bound", () => {
    // Both answers are "undetermined", so the verdict alone cannot tell the
    // cycle watch from the bound. What the watch buys is host reads: it starts
    // recording at step 64 and a self-cycle repeats on the very next step, so
    // the walk asks for `parent` 65 times instead of the 1024 the bound allows.
    let parentReads = 0
    const looped: Record<string, unknown> = { visible: true }
    Object.defineProperty(looped, "parent", {
      configurable: true,
      enumerable: true,
      get() {
        parentReads += 1
        return looped
      },
    })

    expect(visibilityOf(looped)).toBe("undetermined")
    expect(parentReads).toBe(65)
  })
})
