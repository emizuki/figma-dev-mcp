import { afterEach, describe, expect, test } from "bun:test"

import { parseUuid } from "../shared/validation"
import { randomUuid } from "./uuid"

describe("randomUuid", () => {
  const originalRandomUUID = globalThis.crypto.randomUUID

  afterEach(() => {
    Object.defineProperty(globalThis.crypto, "randomUUID", {
      configurable: true,
      value: originalRandomUUID,
    })
  })

  test("returns a parseable UUID when randomUUID is missing", () => {
    Object.defineProperty(globalThis.crypto, "randomUUID", {
      configurable: true,
      value: undefined,
    })

    const first = randomUuid()
    const second = randomUuid()
    expect(parseUuid(first)).toBe(first.toLowerCase())
    expect(parseUuid(second)).toBe(second.toLowerCase())
    expect(first).not.toBe(second)
  })
})
