//! Read-only policy proof over source, catalog, wire, and traversal gates.

use std::{fs, path::PathBuf};

use figma_dev_mcp_protocol::wire::{BrokerToPlugin, ReadOperation};
use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};
use figma_dev_mcp_tools::tools_catalog;
use serde_json::{Value, json};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate sits in the workspace")
        .to_path_buf()
}

fn production_typescript(directory: PathBuf) -> String {
    let mut pending = vec![directory];
    let mut source = String::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).expect("plugin source directory is readable") {
            let entry = entry.expect("plugin source entry is readable");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "ts")
                && !path.to_string_lossy().ends_with(".test.ts")
                && !path.to_string_lossy().ends_with("environment.typecheck.ts")
            {
                source.push_str(&fs::read_to_string(&path).unwrap_or_else(|_| {
                    panic!("TypeScript source is readable: {}", path.display())
                }));
                source.push('\n');
            }
        }
    }
    source
}

fn code_lines(source: &str) -> impl Iterator<Item = &str> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('*'))
}

const MUTATION_DENYLIST: &[&str] = &[
    "loadAllPagesAsync",
    "loadFontAsync",
    "setCurrentPageAsync",
    "setRangeFontName",
    "installFont",
    "substituteFont",
    "importComponentByKeyAsync",
    "importComponentSetByKeyAsync",
    "addComponentProperty",
    "editComponentProperty",
    "deleteComponentProperty",
    "setProperties(",
    "setPluginData",
    "setRelaunchData",
    "createRectangle",
    "createFrame",
    "createText",
    "remove()",
    "applyAnimationStyle",
    "removeAnimationStyle",
    "applyManualKeyframeTrack",
    "removeManualKeyframeTrack",
    "setTimelineDuration",
];

const FORBIDDEN_INPUT_KEYS: &[&str] = &[
    "path",
    "filePath",
    "filepath",
    "filesystem",
    "fsPath",
    "command",
    "cmd",
    "argv",
    "method",
    "script",
    "eval",
    "url",
    "uri",
    "href",
    "endpoint",
    "host",
    "hostname",
    "socket",
    "network",
];

#[test]
fn snapshots_lock_tools_annotations_prompts_and_wire_variants() {
    let catalog = tools_catalog();
    let names: Vec<_> = catalog
        .tools
        .iter()
        .map(|tool| tool.name.as_ref())
        .collect();
    assert_eq!(names, TOOL_NAMES);
    assert_eq!(
        PROMPT_NAMES,
        [
            "prototype_flow_strategy",
            "read_design_strategy",
            "style_audit_strategy",
        ]
    );
    for tool in &catalog.tools {
        let annotations = tool.annotations.as_ref().expect("annotations");
        assert_eq!(annotations.read_only_hint, Some(true), "{}", tool.name);
        assert_eq!(annotations.destructive_hint, Some(false), "{}", tool.name);
        assert_eq!(annotations.open_world_hint, Some(false), "{}", tool.name);
    }

    let operations = [
        "get_metadata",
        "get_selection",
        "get_nodes",
        "search_nodes",
        "get_design_context",
        "get_styles",
        "get_variables",
        "get_components",
        "get_fonts",
        "get_dev_mode_data",
        "get_reactions",
        "get_motion",
        "get_screenshot",
    ];
    for tag in operations {
        let input = if tag == "get_screenshot" {
            json!({"format": "png", "selector": {"nodeId": "1:2"}})
        } else if tag == "get_nodes" {
            json!({"nodeIds": []})
        } else if tag == "search_nodes" {
            json!({"scope": {"pageId": "0:1"}, "query": "Card"})
        } else {
            json!({})
        };
        let operation: ReadOperation = serde_json::from_value(json!({
            "operation": tag,
            "input": input
        }))
        .unwrap();
        assert_eq!(serde_json::to_value(operation).unwrap()["operation"], tag);
    }
    assert!(
        serde_json::from_value::<ReadOperation>(json!({
            "operation": "applyAnimationStyle",
            "input": {}
        }))
        .is_err()
    );
}

#[test]
fn plugin_source_denies_mutation_private_and_motion_write_apis() {
    let source = production_typescript(workspace_root().join("plugin/src"));
    assert!(
        !source.contains("plugin/dist"),
        "source scan must not depend on a pre-existing build"
    );
    for forbidden in MUTATION_DENYLIST {
        assert!(
            !source.contains(forbidden),
            "production plugin source contains forbidden surface {forbidden}"
        );
    }
    for assignment in ["currentPage", "selection", "fontName"] {
        let dotted = format!(".{assignment} =");
        let figma = format!("figma.{assignment} =");
        assert!(
            code_lines(&source).all(|line| {
                !(line.contains(&dotted) || line.contains(&figma)) || line.contains("==")
            }),
            "production plugin source must not assign {assignment}"
        );
    }
}

#[test]
fn manifest_is_dev_mode_inspect_dynamic_page_loopback() {
    let source = fs::read_to_string(workspace_root().join("plugin/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_str(&source).unwrap();
    assert_eq!(manifest["editorType"], json!(["dev"]));
    assert_eq!(manifest["capabilities"], json!(["inspect"]));
    assert_eq!(manifest["documentAccess"], "dynamic-page");
    assert_eq!(
        manifest["networkAccess"]["allowedDomains"],
        json!(["ws://localhost:3056"])
    );
}

#[test]
fn input_schemas_reject_filesystem_command_and_network_targets() {
    for tool in tools_catalog().tools {
        let schema = Value::Object((*tool.input_schema).clone());
        let mut names = Vec::new();
        collect_property_names(&schema, &mut names);
        for key in names {
            assert!(
                !FORBIDDEN_INPUT_KEYS.contains(&key.as_str()),
                "{} input schema accepts forbidden property {key}",
                tool.name
            );
        }
    }
}

fn collect_property_names(schema: &Value, names: &mut Vec<String>) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        names.extend(properties.keys().cloned());
        for value in properties.values() {
            collect_property_names(value, names);
        }
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            for branch in branches {
                collect_property_names(branch, names);
            }
        }
    }
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for value in defs.values() {
            collect_property_names(value, names);
        }
    }
    if let Some(items) = schema.get("items") {
        collect_property_names(items, names);
    }
}

#[test]
fn write_shaped_mcp_and_wire_requests_are_rejected() {
    for operation in [
        "apply_style",
        "set_selection",
        "create_rectangle",
        "applyAnimationStyle",
        "setTimelineDuration",
        "get_css",
        "get_tokens",
    ] {
        assert!(
            serde_json::from_value::<ReadOperation>(json!({
                "operation": operation,
                "input": {}
            }))
            .is_err(),
            "{operation} must not be a wire variant"
        );
        assert!(
            serde_json::from_value::<BrokerToPlugin>(json!({
                "type": "request",
                "requestId": "plugin-1",
                "deadlineMs": 100,
                "target": {},
                "operation": {"operation": operation, "input": {}}
            }))
            .is_err(),
            "{operation} must not decode as a broker request"
        );
    }
    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request",
            "requestId": "plugin-1",
            "deadlineMs": 100,
            "target": {},
            "operation": {"operation": "unknown_variant", "input": {}}
        }))
        .is_err()
    );
}

#[test]
fn origin_socket_and_rpc_boundaries_stay_raw_tcp_and_null_origin() {
    let ws = fs::read_to_string(workspace_root().join("crates/broker/src/ws.rs")).unwrap();
    assert!(ws.contains("Origin"));
    assert!(ws.contains("null"));
    let rpc = fs::read_to_string(workspace_root().join("crates/broker/src/rpc.rs")).unwrap();
    assert!(
        !rpc.contains("WebSocket") && !rpc.contains("tokio_tungstenite"),
        "frontend RPC must stay raw TCP"
    );
    assert!(rpc.contains("encode_frame") || rpc.contains("read_frame"));
}

#[test]
fn every_node_scoped_operation_acquires_the_traversal_gate() {
    let dispatch =
        fs::read_to_string(workspace_root().join("plugin/src/main/dispatch.ts")).unwrap();
    for required in [
        "get_selection: \"read\"",
        "get_nodes: \"read\"",
        "search_nodes: \"read\"",
        "get_design_context: \"includeHiddenWhenRequested\"",
        "get_styles: \"read\"",
        "get_variables: \"read\"",
        "get_components: \"read\"",
        "get_fonts: \"read\"",
        "get_dev_mode_data: \"read\"",
        "get_reactions: \"read\"",
        "get_motion: \"read\"",
        "get_screenshot: \"read\"",
        "get_metadata: \"none\"",
    ] {
        assert!(
            dispatch.contains(required),
            "TRAVERSAL_POLICY must include {required}"
        );
    }
    assert!(dispatch.contains("gate.read") || dispatch.contains("return gate.read"));
    assert!(dispatch.contains("gate.includeHidden"));
}

const OPERATOR_DOCS: &[&str] = &[
    "README.md",
    "docs/setup.md",
    "docs/testing.md",
    "docs/manual-acceptance.md",
];

const DIAGNOSTIC_CALL_NAMES: &[&str] = &[
    "test_missing_capability",
    "test_streaming_elicitation",
    "test_logging_tool",
];

const SEVEN_LOCAL_VERIFICATION_COMMANDS: &[&str] = &[
    "cargo fmt --all -- --check",
    "cargo clippy --workspace --all-targets --all-features -- -D warnings",
    "cargo test --workspace --all-features",
    "(cd plugin && bun install --frozen-lockfile)",
    "(cd plugin && bun run format:check && bun run typecheck && bun run build && bun run test)",
    "(cd conformance && bun install --frozen-lockfile)",
    "./scripts/run-conformance.sh",
];

fn require_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    assert!(path.is_file(), "required document missing: {relative}");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{relative} must be readable: {error}"))
}

fn operator_documentation() -> String {
    let mut combined = String::new();
    for relative in OPERATOR_DOCS {
        combined.push_str(&require_file(relative));
        combined.push('\n');
    }
    combined
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
        !prefix.ends_with("do not")
            && !prefix.ends_with("does not")
            && !prefix.ends_with("never")
            && !prefix.ends_with("without")
            && !prefix.ends_with("no")
            && !prefix.ends_with("not")
    })
}

fn table_first_cell_identifiers(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') {
                return None;
            }
            let cell = trimmed
                .trim_start_matches('|')
                .split('|')
                .next()
                .unwrap_or("")
                .trim();
            let name = cell.strip_prefix('`')?.strip_suffix('`')?;
            if name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_') && name.contains('_') {
                Some(name.to_owned())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn documentation_required_operator_files_exist() {
    for relative in OPERATOR_DOCS {
        let _ = require_file(relative);
    }
}

#[test]
fn documentation_states_exact_ports_tools_and_prompts() {
    let docs = operator_documentation();
    for port in ["127.0.0.1:3056", "127.0.0.1:3057"] {
        assert!(
            docs.contains(port),
            "operator docs must name exact port {port}"
        );
    }
    for tool in TOOL_NAMES {
        assert!(
            contains_ident(&docs, tool),
            "operator docs must name exact tool {tool}"
        );
    }
    for prompt in PROMPT_NAMES {
        assert!(
            contains_ident(&docs, prompt),
            "operator docs must name exact prompt {prompt}"
        );
    }

    let readme = require_file("README.md");
    let table_tools: Vec<String> = table_first_cell_identifiers(&readme)
        .into_iter()
        .filter(|name| {
            TOOL_NAMES.contains(&name.as_str()) || DIAGNOSTIC_CALL_NAMES.contains(&name.as_str())
        })
        .collect();
    let mut unique_tools = table_tools.clone();
    unique_tools.sort();
    unique_tools.dedup();
    assert_eq!(
        unique_tools,
        TOOL_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
        "README tool tables must list exactly the 14 product tools"
    );
    for diagnostic in DIAGNOSTIC_CALL_NAMES {
        assert!(
            !table_tools.iter().any(|name| name == diagnostic),
            "README tool tables must not list unadvertised diagnostic {diagnostic}"
        );
        assert!(
            !contains_ident(&readme, diagnostic),
            "README must not present {diagnostic} as a product tool"
        );
    }

    let table_prompts: Vec<String> = table_first_cell_identifiers(&readme)
        .into_iter()
        .filter(|name| PROMPT_NAMES.contains(&name.as_str()))
        .collect();
    let mut unique_prompts = table_prompts;
    unique_prompts.sort();
    unique_prompts.dedup();
    assert_eq!(
        unique_prompts,
        PROMPT_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
        "README prompt tables must list exactly the three product prompts"
    );
}

#[test]
fn documentation_covers_dev_mode_import_connection_selection_and_no_daemon() {
    let setup = require_file("docs/setup.md");
    let readme = require_file("README.md");
    let combined = format!("{readme}\n{setup}");
    assert!(
        combined.contains("Dev Mode"),
        "setup docs must describe Dev Mode import"
    );
    assert!(
        combined.contains("plugin/manifest.json"),
        "setup docs must import plugin/manifest.json"
    );
    assert!(
        combined.contains("development plugin"),
        "setup docs must say the companion is imported as a development plugin"
    );
    assert!(
        setup.contains("cargo build --release"),
        "setup docs must include cargo build --release"
    );
    assert!(
        setup.contains("stdio"),
        "setup docs must configure the MCP client over stdio"
    );
    assert!(
        contains_ident(&setup, "list_files"),
        "setup docs must tell operators to use list_files"
    );
    assert!(
        combined.contains("exactly one") && contains_ident(&combined, "connectionId"),
        "docs must state the single-file connection-selection rule"
    );
    assert!(
        combined.contains("AMBIGUOUS_CONNECTION"),
        "docs must name AMBIGUOUS_CONNECTION for omitted ids with several files"
    );
    assert!(
        combined.contains("without a separate daemon"),
        "docs must state no-daemon startup wording"
    );
}

#[test]
fn documentation_lists_all_seven_local_verification_commands() {
    let testing = require_file("docs/testing.md");
    for command in SEVEN_LOCAL_VERIFICATION_COMMANDS {
        assert!(
            testing.contains(command),
            "docs/testing.md must list verification command: {command}"
        );
    }
}

#[test]
fn documentation_states_svg_source_readonly_limits_and_origin_threat_model() {
    let docs = operator_documentation();
    for required in [
        "SVG source",
        "viewBox",
        "image/svg+xml",
        "UNSAFE_SVG",
        "read-only",
        "Origin: null",
        "not authentication",
    ] {
        assert!(
            docs.contains(required),
            "operator docs must state {required}"
        );
    }
    assert!(
        docs.contains("seconds"),
        "operator docs must state Motion times are seconds"
    );
    assert!(
        !contains_ident(&docs, "durationMs"),
        "operator docs must not document durationMs"
    );
    assert!(
        docs.to_ascii_lowercase().contains("local filesystem")
            || docs.to_ascii_lowercase().contains("local export"),
        "operator docs must state the local side-effect limitation"
    );
    assert!(
        docs.contains("same local") || docs.contains("same operating-system user"),
        "operator docs must include the origin threat-model caveat"
    );
}

#[test]
fn documentation_forbids_local_export_instructions_and_unadvertised_product_tools() {
    let docs = operator_documentation();
    let lower = docs.to_ascii_lowercase();
    for phrase in [
        "save screenshots",
        "export frames",
        "write a local file",
        "write to disk",
        "save to a path",
        "export to a path",
        "save_screenshots",
        "export_frames_to_pdf",
    ] {
        assert!(
            !instructs(&lower, phrase),
            "operator docs must not instruct {phrase}"
        );
    }
}

#[test]
fn documentation_splits_stdio_evidence_from_official_lifecycle_smoke() {
    let testing = require_file("docs/testing.md");
    let lower = testing.to_ascii_lowercase();
    for required in [
        "production stdio",
        "test-only http adapter",
        "lifecycle smoke",
        "2026-07-28",
        "2025-11-25",
    ] {
        assert!(
            lower.contains(required),
            "docs/testing.md must explain the evidence split with {required}"
        );
    }
    for forbidden in [
        "full upstream suite",
        "full official suite",
        "full conformance suite",
    ] {
        assert!(
            !lower.contains(forbidden),
            "docs/testing.md must not call the two official scenarios the {forbidden}"
        );
    }
    assert!(
        testing.contains("FIGMA_DEV_MCP_LOG"),
        "docs/testing.md must document stderr log controls"
    );
    assert!(
        testing.contains("stderr"),
        "docs/testing.md must say logs go to stderr"
    );
    assert!(
        lower.contains("node text") && lower.contains("screenshot") && lower.contains("variable"),
        "docs/testing.md must say design content is not logged"
    );
}

#[test]
fn documentation_manual_acceptance_has_nine_spec_scenarios() {
    let manual = require_file("docs/manual-acceptance.md");
    for required in [
        "list_files",
        "connectionId",
        "truncat",
        "Disconnect",
        "plugin data",
        "relaunch",
        "local export",
        "read_design_strategy",
        "prototype_flow_strategy",
        "style_audit_strategy",
        "viewBox",
        "Figma desktop version",
        "plugin build hash",
        "binary version",
    ] {
        assert!(
            manual.contains(required),
            "docs/manual-acceptance.md must record {required}"
        );
    }
    let checkboxes = manual
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]")
        })
        .count();
    assert!(
        checkboxes >= 9,
        "docs/manual-acceptance.md must contain a checkbox for each of the nine spec scenarios, found {checkboxes}"
    );
}

#[test]
fn documentation_gitignore_keeps_lockfiles_and_snapshots_tracked() {
    let gitignore = require_file(".gitignore");
    for required in [
        "/target/",
        "/plugin/node_modules/",
        "/plugin/dist/",
        "/conformance/node_modules/",
    ] {
        assert!(
            gitignore.contains(required),
            ".gitignore must ignore {required}"
        );
    }
    assert!(
        gitignore.to_ascii_lowercase().contains("do not ignore")
            || gitignore.to_ascii_lowercase().contains("must not ignore"),
        ".gitignore must state that lockfiles and contract snapshots stay tracked"
    );
    for tracked in [
        "Cargo.lock",
        "plugin/bun.lock",
        "conformance/bun.lock",
        "tests/contracts/fixtures",
        "tests/contracts/snapshots",
    ] {
        let ignored = gitignore.lines().any(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with('#') && (trimmed == tracked || trimmed == format!("/{tracked}"))
        });
        assert!(
            !ignored,
            ".gitignore must not ignore tracked path {tracked}"
        );
    }
}

#[test]
fn documentation_ci_pins_runtimes_and_never_publishes() {
    let ci = require_file(".github/workflows/ci.yml");
    for job in [
        "rust-static:",
        "rust-tests:",
        "plugin:",
        "policy:",
        "conformance:",
    ] {
        assert!(ci.contains(job), "CI must define job {job}");
    }
    assert!(ci.contains("1.95.0"), "CI must pin Rust 1.95.0");
    assert!(ci.contains("1.3.14"), "CI must pin Bun 1.3.14");
    assert!(
        ci.contains("frozen-lockfile"),
        "CI must install with frozen lockfiles"
    );
    assert!(
        ci.contains("Cargo.lock")
            && ci.contains("plugin/bun.lock")
            && ci.contains("conformance/bun.lock"),
        "CI cache keys must include Cargo.lock, plugin/bun.lock, and conformance/bun.lock"
    );
    let lower = ci.to_ascii_lowercase();
    for forbidden in [
        "cargo publish",
        "npm publish",
        "softprops/action-gh-release",
        "actions/upload-artifact",
        "actions/upload-release-asset",
    ] {
        assert!(
            !lower.contains(forbidden),
            "CI must never publish an artifact or package ({forbidden})"
        );
    }
}
