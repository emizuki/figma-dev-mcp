export const MAX_DEPTH = 6
export const MAX_INPUT_IDS = 2_000
export const MAX_PAGE_IDS = 100
export const MAX_IDENTIFIER_BYTES = 256
export const MAX_QUERY_BYTES = 1_024
export const MAX_DISPLAY_TEXT_BYTES = 1_024
export const MAX_VISITED_NODES = 10_000
export const MAX_RETURNED_NODES = 2_000
export const MAX_TEXT_BYTES = 8 * 1_024 * 1_024
export const MAX_ENVELOPE_BYTES = 24 * 1_024 * 1_024
export const MAX_RASTER_SIDE = 4_096
export const MAX_RASTER_PIXELS = 16_000_000
export const MAX_RASTER_DECODED_BYTES = 12 * 1_024 * 1_024
export const MAX_RASTER_BASE64_BYTES = 16 * 1_024 * 1_024
export const MAX_SVG_BYTES = 4 * 1_024 * 1_024
export const MAX_IN_FLIGHT = 4
export const MAX_QUEUE = 16
export const INACTIVITY_TIMEOUT_SECS = 15
export const TOTAL_TIMEOUT_SECS = 120
export const HEARTBEAT_SECS = 5
export const STALE_SESSION_SECS = 20
export const IDLE_GRACE_SECS = 30
export const CANCEL_CHECK_BATCH = 100
export const U32_MAX = 4_294_967_295

export const BROKER_URL = "ws://localhost:3056"
export const RECONNECT_DELAYS_MS: readonly [250, 500, 1_000, 2_000, 5_000] = [
  250, 500, 1_000, 2_000, 5_000,
]
