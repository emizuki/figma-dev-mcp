//! Protocol types shared by the Figma plugin and the MCP server.

mod deferred;

pub mod domain;
pub mod error;
pub mod limits;
pub mod rpc;
pub mod wire;

/// The complete public MCP tool catalog for the MVP.
pub const TOOL_NAMES: [&str; 14] = [
    "get_components",
    "get_design_context",
    "get_dev_mode_data",
    "get_fonts",
    "get_metadata",
    "get_motion",
    "get_nodes",
    "get_reactions",
    "get_screenshot",
    "get_selection",
    "get_styles",
    "get_variables",
    "list_files",
    "search_nodes",
];

/// The complete public MCP prompt catalog for the MVP.
pub const PROMPT_NAMES: [&str; 3] = [
    "prototype_flow_strategy",
    "read_design_strategy",
    "style_audit_strategy",
];
