import { describe, expect, test } from "bun:test"

import { detectCapabilities } from "./common"

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
