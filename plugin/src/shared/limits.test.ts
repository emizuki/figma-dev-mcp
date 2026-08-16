import { describe, expect, test } from "bun:test"

import {
  CANCEL_CHECK_BATCH,
  HEARTBEAT_SECS,
  IDLE_GRACE_SECS,
  INACTIVITY_TIMEOUT_SECS,
  MAX_DEPTH,
  MAX_DISPLAY_TEXT_BYTES,
  MAX_ENVELOPE_BYTES,
  MAX_IDENTIFIER_BYTES,
  MAX_IN_FLIGHT,
  MAX_INPUT_IDS,
  MAX_PAGE_IDS,
  MAX_QUERY_BYTES,
  MAX_QUEUE,
  MAX_RASTER_BASE64_BYTES,
  MAX_RASTER_DECODED_BYTES,
  MAX_RASTER_PIXELS,
  MAX_RASTER_SIDE,
  MAX_RETURNED_NODES,
  MAX_SVG_BYTES,
  MAX_TEXT_BYTES,
  MAX_VISITED_NODES,
  STALE_SESSION_SECS,
  TOTAL_TIMEOUT_SECS,
} from "./limits"

describe("reviewed resource ceilings", () => {
  test("TypeScript fixtures match the Rust production constants", () => {
    expect(MAX_DEPTH).toBe(6)
    expect(MAX_INPUT_IDS).toBe(2_000)
    expect(MAX_PAGE_IDS).toBe(100)
    expect(MAX_IDENTIFIER_BYTES).toBe(256)
    expect(MAX_QUERY_BYTES).toBe(1_024)
    expect(MAX_DISPLAY_TEXT_BYTES).toBe(1_024)
    expect(MAX_VISITED_NODES).toBe(10_000)
    expect(MAX_RETURNED_NODES).toBe(2_000)
    expect(MAX_TEXT_BYTES).toBe(8 * 1024 * 1024)
    expect(MAX_ENVELOPE_BYTES).toBe(24 * 1024 * 1024)
    expect(MAX_RASTER_SIDE).toBe(4_096)
    expect(MAX_RASTER_PIXELS).toBe(16_000_000)
    expect(MAX_RASTER_DECODED_BYTES).toBe(12 * 1024 * 1024)
    expect(MAX_RASTER_BASE64_BYTES).toBe(16 * 1024 * 1024)
    expect(MAX_SVG_BYTES).toBe(4 * 1024 * 1024)
    expect(MAX_IN_FLIGHT).toBe(4)
    expect(MAX_QUEUE).toBe(16)
    expect(INACTIVITY_TIMEOUT_SECS).toBe(15)
    expect(TOTAL_TIMEOUT_SECS).toBe(120)
    expect(HEARTBEAT_SECS).toBe(5)
    expect(STALE_SESSION_SECS).toBe(20)
    expect(IDLE_GRACE_SECS).toBe(30)
    expect(CANCEL_CHECK_BATCH).toBe(100)
  })
})
