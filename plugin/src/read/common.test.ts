import { describe, expect, test } from "bun:test"

import { detectCapabilities, loadPageIfNeeded } from "./common"

function installFigma(api: Record<string, unknown>) {
  ;(globalThis as typeof globalThis & { figma: unknown }).figma = api
}

describe("detectCapabilities", () => {
  test("annotations capability follows the annotations API, not the page", () => {
    // AnnotationsMixin belongs to scene nodes; PageNode does not extend it, so
    // probing currentPage can never be true. The API lives on figma itself.
    installFigma({ currentPage: {}, annotations: {}, variables: {} })
    expect(detectCapabilities().annotations).toBe(true)

    installFigma({ currentPage: {}, variables: {} })
    expect(detectCapabilities().annotations).toBe(false)
  })
})

describe("loadPageIfNeeded", () => {
  test("calls loadAsync on the page it was given", async () => {
    const receivers: unknown[] = []
    const page = {
      id: "0:1",
      type: "PAGE",
      async loadAsync(this: unknown) {
        receivers.push(this)
      },
    }

    const returned = await loadPageIfNeeded(page)

    expect(returned).toBe(page)
    expect(receivers).toEqual([page])
  })
})
