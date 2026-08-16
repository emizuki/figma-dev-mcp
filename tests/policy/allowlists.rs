use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};

#[test]
fn mvp_tool_allowlist_is_sorted_closed_and_exact() {
    assert_eq!(
        TOOL_NAMES,
        [
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
        ]
    );
    assert_eq!(TOOL_NAMES.len(), 14);
    assert!(!TOOL_NAMES.contains(&"get_css"));
    assert!(!TOOL_NAMES.contains(&"get_tokens"));
    assert!(TOOL_NAMES.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn mvp_prompt_allowlist_is_sorted_closed_and_exact() {
    assert_eq!(
        PROMPT_NAMES,
        [
            "prototype_flow_strategy",
            "read_design_strategy",
            "style_audit_strategy",
        ]
    );
    assert!(PROMPT_NAMES.windows(2).all(|pair| pair[0] < pair[1]));
}
