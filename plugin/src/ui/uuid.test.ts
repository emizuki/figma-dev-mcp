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

describe("randomUuid source selection", () => {
  const original = {
    randomUUID: globalThis.crypto.randomUUID,
    getRandomValues: globalThis.crypto.getRandomValues,
  }

  const override = (name: "randomUUID" | "getRandomValues", value: unknown) => {
    Object.defineProperty(globalThis.crypto, name, {
      configurable: true,
      value,
    })
  }

  afterEach(() => {
    override("randomUUID", original.randomUUID)
    override("getRandomValues", original.getRandomValues)
  })

  test("uses the host's randomUUID whenever it can be called", () => {
    const minted = "123e4567-e89b-42d3-a456-426614174777"
    let calls = 0
    override("randomUUID", () => {
      calls += 1
      return minted
    })

    expect(randomUuid()).toBe(minted)
    expect(calls).toBe(1)
  })

  test("falls back to getRandomValues, stamping the version and variant itself", () => {
    override("randomUUID", undefined)
    let calls = 0
    override("getRandomValues", (bytes: Uint8Array) => {
      calls += 1
      bytes.fill(0)
      return bytes
    })

    // Every byte is zero, so the only non-zero nibbles in the answer are the
    // ones `fromBytes` writes: version 4 and the RFC variant.
    expect(randomUuid()).toBe("00000000-0000-4000-8000-000000000000")
    expect(calls).toBe(1)
  })

  test("a randomUUID that throws when called is not fatal", () => {
    // Figma's iframe has been seen exposing a `randomUUID` that is a function
    // but refuses to run; the fallback has to take over rather than propagate.
    override("randomUUID", () => {
      throw new Error("blocked by the sandbox")
    })
    let calls = 0
    override("getRandomValues", (bytes: Uint8Array) => {
      calls += 1
      bytes.fill(0xff)
      return bytes
    })

    expect(randomUuid()).toBe("ffffffff-ffff-4fff-bfff-ffffffffffff")
    expect(calls).toBe(1)
  })

  test("with no web crypto at all, Math.random still produces a valid v4 UUID", () => {
    override("randomUUID", undefined)
    override("getRandomValues", undefined)

    const first = randomUuid()
    const second = randomUuid()
    expect(parseUuid(first)).toBe(first)
    expect(first).not.toBe(second)
    expect(first[14]).toBe("4")
    expect("89ab").toContain(first[19] ?? "")
  })
})
