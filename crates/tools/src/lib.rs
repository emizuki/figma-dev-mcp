//! Public MCP tool contracts, deterministic catalog, and broker-backed service.

mod catalog;
mod content;
mod contracts;
mod dispatch;
mod names;
mod observability;
mod service;

pub use catalog::{CACHE_TTL_MS, tools_catalog};
pub use content::{
    AccountedImage, EnvelopeContext, account_batch_images, account_call_tool_result,
    account_screenshot_result, reject_oversize_frame, serialize_mcp_envelope,
    structured_with_image,
};
pub use contracts::*;
pub use service::McpService;
