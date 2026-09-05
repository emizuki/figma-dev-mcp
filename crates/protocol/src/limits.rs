//! Reviewed, non-configurable upper limits for public protocol boundaries.

pub const MAX_DEPTH: u8 = 6;
pub const MAX_INPUT_IDS: usize = 2_000;
pub const MAX_PAGE_IDS: usize = 100;
pub const MAX_IDENTIFIER_BYTES: usize = 256;
pub const MAX_QUERY_BYTES: usize = 1_024;
pub const MAX_SEARCH_CURSOR_BYTES: usize = 64 * 1_024;
pub const MAX_DISPLAY_TEXT_BYTES: usize = 1_024;
pub const MAX_VISITED_NODES: usize = 10_000;
pub const MAX_RETURNED_NODES: usize = 2_000;
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ENVELOPE_BYTES: usize = 24 * 1024 * 1024;
pub const MAX_RASTER_SIDE: u32 = 4_096;
pub const MAX_RASTER_PIXELS: u64 = 16_000_000;
pub const MAX_RASTER_DECODED_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_RASTER_BASE64_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_IN_FLIGHT: usize = 4;
pub const MAX_QUEUE: usize = 16;
pub const INACTIVITY_TIMEOUT_SECS: u64 = 15;
pub const TOTAL_TIMEOUT_SECS: u64 = 120;
pub const HEARTBEAT_SECS: u64 = 5;
pub const STALE_SESSION_SECS: u64 = 20;
pub const IDLE_GRACE_SECS: u64 = 30;
/// How long a call waits for the first election to install a backend before
/// giving up. The measured race is ~80µs — the MCP service starts answering
/// before `supervise` finishes electing — so this is orders of magnitude more
/// than the real case needs. It is capped so that a genuinely stuck election
/// still surfaces as an error rather than as latency: election retries from
/// 100ms to a 5s ceiling and can stay stuck for minutes.
pub const BACKEND_READY_MS: u64 = 1_000;
