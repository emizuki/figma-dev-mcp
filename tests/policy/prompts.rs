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

/// The prompt-body scan reads a distinct body per name, and the two predicates
/// it applies to them still fire.
///
/// `prompt_bodies_name_only_allowlisted_tools_and_reject_removed_or_mutating_guidance`
/// has a real positive control — the union of extracted references must name
/// four core read tools — but the union is not the scan. Making `prompt_body`
/// return `read_design_strategy.md` for all three names left the policy suite
/// at 31 passed / 0 failed: that one body alone names all fourteen tools, so
/// the union assertion is satisfied while two of the three bodies are never
/// scanned for forbidden identifiers or mutating guidance at all.
///
/// Measured gate frequencies, which are why the checks below look the way they
/// do:
///
/// - `contains_ident` is exercised by the real bodies in the rejecting
///   direction — all three contain the substring `get_node` inside `get_nodes`
///   and it correctly answers no — but never in the accepting direction.
/// - `instructs`'s negation-prefix branch fires **exactly once** across the
///   twelve phrases and three bodies: `prototype_flow_strategy` says "Do not
///   create connector nodes". That single occurrence is the only thing standing
///   between the phrase list and a red suite, so it is pinned here by name.
#[test]
fn the_prompt_body_scan_reads_a_distinct_body_per_name_and_its_predicates_fire() {
    let bodies: Vec<(&str, String)> = PROMPT_NAMES
        .iter()
        .map(|name| (*name, prompt_body(name)))
        .collect();
    assert_eq!(bodies.len(), 3);

    for (index, (name, body)) in bodies.iter().enumerate() {
        for (other_name, other) in &bodies[index + 1..] {
            assert!(
                body != other,
                "{name} and {other_name} resolved to the same text; the scan is \
                 reading one body several times and the other bodies are never \
                 examined"
            );
        }
        assert!(
            !body.trim().is_empty(),
            "{name} body is empty, so every forbidden identifier is trivially \
             absent from it"
        );
        assert!(
            !extracted_tool_references(body).is_empty(),
            "{name} names no allowlisted tool at all; the union assertion in the \
             test above can be satisfied by one body while this one is blank of \
             anything the scan would inspect"
        );
        // The rejecting direction of contains_ident, on real input.
        assert!(
            body.contains("get_node"),
            "{name} no longer contains the substring `get_node` (inside \
             `get_nodes`), which is the only thing exercising the identifier \
             boundary check on real text"
        );
        assert!(
            !contains_ident(body, "get_node"),
            "contains_ident now reports the removed tool `get_node` inside \
             `get_nodes` in {name}"
        );
    }

    // The accepting direction, which nothing real exercises.
    assert!(
        contains_ident("call `get_document` here", "get_document"),
        "contains_ident no longer reports a forbidden identifier that is \
         actually present, so the 23-name denylist above forbids nothing"
    );

    // The one live negation, by name, plus the accepting direction.
    let flow = prompt_body("prototype_flow_strategy").to_ascii_lowercase();
    assert!(
        flow.contains("create connector"),
        "prototype_flow_strategy no longer says `create connector`; that phrase \
         is the only occurrence of any forbidden phrase in any body, and \
         without it `instructs` is never asked a question it could answer wrong"
    );
    assert!(
        !instructs(&flow, "create connector"),
        "instructs lost the negation exemption that keeps `Do not create \
         connector nodes` from reading as an instruction"
    );
    assert!(
        instructs("you may create connector nodes", "create connector"),
        "instructs no longer reports a plain instruction, so the twelve-phrase \
         list above forbids nothing"
    );
}
