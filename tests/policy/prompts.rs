use figma_dev_mcp_prompts::extracted_tool_references;
use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};
use std::collections::BTreeSet;
use std::{fs, path::PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate must sit in the workspace")
        .to_path_buf()
}

fn prompt_body(name: &str) -> String {
    let path = workspace_root().join(format!("crates/prompts/bodies/{name}.md"));
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("prompt body {} must exist: {error}", path.display()))
}

fn contains_ident(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

fn instructs(haystack: &str, phrase: &str) -> bool {
    haystack.match_indices(phrase).any(|(index, _)| {
        let prefix = haystack[..index].trim_end();
        !prefix.ends_with("do not") && !prefix.ends_with("never") && !prefix.ends_with("without")
    })
}

#[test]
fn prompt_bodies_name_only_allowlisted_tools_and_reject_removed_or_mutating_guidance() {
    assert_eq!(
        PROMPT_NAMES,
        [
            "prototype_flow_strategy",
            "read_design_strategy",
            "style_audit_strategy",
        ]
    );

    let mut referenced = BTreeSet::new();
    for name in PROMPT_NAMES {
        let body = prompt_body(name);
        assert!(!body.trim().is_empty(), "{name} body must not be empty");
        let tools = extracted_tool_references(&body);
        for tool in &tools {
            assert!(
                TOOL_NAMES.contains(&tool.as_str()),
                "{name} references `{tool}`, which is outside the 14-tool allowlist"
            );
        }
        referenced.extend(tools);

        for forbidden in [
            "get_document",
            "get_node",
            "get_nodes_info",
            "scan_text_nodes",
            "scan_nodes_by_types",
            "get_viewport",
            "export_frames_to_pdf",
            "save_screenshots",
            "get_css",
            "get_tokens",
            "get_pages",
            "get_variable_defs",
            "get_local_components",
            "get_motion_styles",
            "get_node_motion",
            "apply_style_to_node",
            "applyAnimationStyle",
            "removeAnimationStyle",
            "applyManualKeyframeTrack",
            "removeManualKeyframeTrack",
            "setTimelineDuration",
            "loadAllPagesAsync",
            "durationMs",
        ] {
            assert!(
                !contains_ident(&body, forbidden),
                "{name} must not mention removed or mutating identifier {forbidden}"
            );
        }

        let lower = body.to_ascii_lowercase();
        for phrase in [
            "write to disk",
            "write a local",
            "local filesystem",
            "filesystem path",
            "save to a path",
            "save screenshots",
            "export frames",
            "create connector",
            "change prototype",
            "mutate figma",
            "bind the variable",
            "apply the style to",
        ] {
            assert!(
                !instructs(&lower, phrase),
                "{name} must not instruct {phrase}"
            );
        }
    }

    assert!(
        referenced.contains("list_files")
            && referenced.contains("get_design_context")
            && referenced.contains("get_reactions")
            && referenced.contains("get_styles"),
        "prompt catalog must actually name core read tools: {referenced:?}"
    );
}

#[test]
fn tool_reference_extraction_is_limited_to_backticked_snake_case_verbs() {
    let sample = "\
        Use `list_files` then `get_nodes()` and `get_design_context(depth: 2)`.\n\
        Ignore `CAPABILITY_UNAVAILABLE`, `includeAvailableStyles`, `durationMs`,\n\
        `dedupeComponents`, `read_design_strategy`, and bare get_css.\n\
        Reject `get_document` and `scan_text_nodes` if a body ever names them.\n\
    ";
    let extracted = extracted_tool_references(sample);
    assert_eq!(
        extracted,
        BTreeSet::from([
            "get_design_context".into(),
            "get_document".into(),
            "get_nodes".into(),
            "list_files".into(),
            "scan_text_nodes".into(),
        ])
    );
    assert!(!extracted.iter().any(|name| name == "get_css"));
    assert!(!extracted.iter().any(|name| name == "durationMs"));
    assert!(!extracted.iter().any(|name| name == "read_design_strategy"));
}
