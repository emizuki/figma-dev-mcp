import { expect, test } from "bun:test"

// @ts-expect-error Bun tests do not expose iframe DOM globals.
type UnexpectedDocument = typeof document

// @ts-expect-error Bun tests do not expose Figma main globals.
type UnexpectedFigma = typeof figma

test("plugin test harness boots", () => {
  expect(true).toBe(true)
})
