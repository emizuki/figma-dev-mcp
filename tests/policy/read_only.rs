//! Read-only policy proof over source, catalog, and wire shapes.

use std::{fs, path::PathBuf};

use figma_dev_mcp_prompts::{RESOURCE_URI_PREFIX, resource_uri};
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

/// No production plugin file spells a canonical error message.
///
/// The messages are generated into `shared/error-catalog.ts` from the Rust
/// protocol, and Rust refuses any frame whose message is not canonical for its
/// code — so a hand-copied string that drifts does not produce a wrong field,
/// it drops the session at decode time.
///
/// This is a test rather than a claim in a comment because the claim was made
/// three times during this consolidation and was wrong each time: a copy
/// survived in `ui/relay.ts`, then another in `read/motion.ts`, each found only
/// by someone reading the diff. A grep is a poor tool for a general property,
/// but it is a better one than a sentence nobody can check.
#[test]
fn production_plugin_source_spells_no_canonical_error_message() {
    let generated = workspace_root().join("plugin/src/shared/error-catalog.ts");
    let catalog = fs::read_to_string(&generated).expect("generated catalog is readable");
    let messages: Vec<&str> = catalog
        .lines()
        .filter_map(|line| line.split_once(": \"")?.1.strip_suffix("\","))
        .collect();
    assert!(
        messages.len() >= 17,
        "expected the generated catalog to yield its messages, parsed {}",
        messages.len()
    );

    let mut pending = vec![workspace_root().join("plugin/src")];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).expect("plugin source directory is readable") {
            let path = entry.expect("plugin source entry is readable").path();
            let name = path.to_string_lossy().to_string();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "ts")
                || name.ends_with(".test.ts")
                || name.ends_with("environment.typecheck.ts")
                || name.ends_with("shared/error-catalog.ts")
            {
                continue;
            }
            let source = fs::read_to_string(&path).expect("TypeScript source is readable");
            for message in &messages {
                assert!(
                    !source.contains(message),
                    "{name} spells the canonical message {message:?} instead of \
                     importing CANONICAL_MESSAGES from shared/error-catalog"
                );
            }
        }
    }
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
    for assignment in [
        "currentPage",
        "selection",
        "fontName",
        "skipInvisibleInstanceChildren",
    ] {
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

/// True if `source` defines the identifier `installFigma` — as a
/// `function installFigma(`, or as a `const`/`let`/`var installFigma =` —
/// rather than merely mentioning a longer name that happens to start with it
/// (`installFigmaHost`) or calling it.
///
/// Known limit: this only recognises those two syntactic shapes. A builder
/// written as an object property (`{ installFigma: () => {} }`), a class
/// method (`class H { installFigma() {} }`), or a plain property assignment
/// (`globals.installFigma = () => {}`) defines the same identifier but is not
/// detected — name-matching cannot fix that, and widening the shapes forever
/// is not this predicate's job. Such a builder still has to install a host to
/// do anything, so it is normally still caught, just by `assigns_figma` and
/// the import assertion below rather than by this one.
fn defines_install_figma(source: &str) -> bool {
    const DEFINING_KEYWORDS: [&str; 4] = ["function", "const", "let", "var"];
    let is_ident_char = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$';

    source.match_indices("installFigma").any(|(index, needle)| {
        let after = source[index + needle.len()..].chars().next();
        if after.is_some_and(is_ident_char) {
            return false; // e.g. installFigmaHost
        }
        let before = source[..index].trim_end_matches(char::is_whitespace);
        DEFINING_KEYWORDS.iter().any(|keyword| {
            before.ends_with(keyword)
                && before.len() >= keyword.len()
                && before[..before.len() - keyword.len()]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !is_ident_char(ch))
        })
    })
}

/// True if `source` assigns `figma` a value, either through a receiver
/// (`(...).figma = {...}`) or bare (`figma = {...}`). `==`/`===` comparisons
/// do not count, and a longer identifier that merely contains `figma`
/// (`figmaType`, `myFigma`) does not either.
///
/// Known limit: only the dotted-or-bare `figma =` form is recognised, which
/// is the repo's one idiom (`(globalThis as ...).figma = api`, used by both
/// the harness and `navigation.test.ts`). `globalThis["figma"] = ...` and
/// `Object.assign(globalThis, { figma })` are not detected.
fn assigns_figma(source: &str) -> bool {
    let is_ident_char = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$';
    source.match_indices("figma").any(|(index, needle)| {
        let before = source[..index].chars().next_back();
        if before.is_some_and(is_ident_char) {
            return false; // e.g. myFigma
        }
        let after = source[index + needle.len()..].chars().next();
        if after.is_some_and(is_ident_char) {
            return false; // e.g. figmaType
        }
        let after = source[index + needle.len()..].trim_start_matches(char::is_whitespace);
        after.starts_with('=') && !after.starts_with("==")
    })
}

/// True if `source` has an `import` line naming the shared harness module, as
/// opposed to merely containing the string anywhere (a stale comment left
/// behind by a migration away from it, for instance).
fn imports_figma_harness(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("import") && trimmed.contains("/tests/figma-harness\"")
    })
}

const COMMON_TEST_PATH: &str = "plugin/src/read/common.test.ts";

/// Read tests migrated onto the shared harness by this plan. Not exhaustive
/// of every read test — see the exemptions below — but if the walk stops
/// seeing one of these, something moved, was renamed, or was deleted, and the
/// other assertions here would otherwise report success over whatever is
/// left, `common.test.ts` included.
const HARNESS_MIGRATED_FILES: [&str; 9] = [
    "plugin/src/read/components.test.ts",
    "plugin/src/read/dev-mode.test.ts",
    "plugin/src/read/fonts.test.ts",
    "plugin/src/read/motion.test.ts",
    "plugin/src/read/reactions.test.ts",
    "plugin/src/read/render.test.ts",
    "plugin/src/read/search.test.ts",
    "plugin/src/read/styles.test.ts",
    "plugin/src/read/variables.test.ts",
];

/// Every read test installs its fake Figma one of two sanctioned ways —
/// importing the shared builder in `plugin/tests/figma-harness.ts`, or,
/// solely `common.test.ts`, defining its own `installFigma` because that file
/// tests capability detection when a capability is absent and needs a host
/// the harness, which always builds a complete one, cannot express — plus one
/// documented exception below (`navigation.test.ts`).
///
/// Whether a file installs a host at all is derived — `defines_install_figma`
/// or `assigns_figma` — not listed, so a future test file that never touches
/// `figma` needs no entry here and drops out on its own, while a future file
/// that does install one and skips the harness is caught automatically
/// instead of requiring someone to remember to add it to an exemption list.
/// Known limit shared by both predicates: name- and syntax-matching, not
/// parsing, so an unrecognised spelling of either can still slip through:
/// `defines_install_figma`'s and `assigns_figma`'s doc comments say which.
///
/// Only two exact repo-relative paths are named. `COMMON_TEST_PATH` is the
/// positive control below. `plugin/src/read/navigation.test.ts` installs its
/// hosts by hand, inline, 36 times over — with no shared builder, one
/// block-scoped helper covering one of those sites aside — the same defect
/// class the harness was built to retire, just never folded into this
/// migration. It is exempted from the import requirement, not because it has
/// no host to migrate, but because migrating a file this size belongs in its
/// own change, not inside this test's fix. Matching on the full relative path
/// rather than the bare file name means a future nested
/// `plugin/src/read/**/navigation.test.ts` or `.../common.test.ts` does not
/// inherit either exemption for free.
///
/// This makes five assertions: the walk actually reaches `common.test.ts`
/// (otherwise every check below could pass over an empty or renamed
/// directory); it also reaches every file in `HARNESS_MIGRATED_FILES`
/// (otherwise the first floor alone would still pass with only
/// `common.test.ts` left); `common.test.ts` still defines its own host, so
/// its exemption cannot rot into a dead branch that always passes; no other
/// file defines its own builder; and every file that installs a host and
/// is not named above imports the shared harness.
#[test]
fn read_tests_share_one_figma_harness() {
    const NAMED_EXEMPT_FROM_HARNESS_IMPORT: [&str; 1] = ["plugin/src/read/navigation.test.ts"];

    let root = workspace_root();
    let mut pending = vec![root.join("plugin/src/read")];
    let mut seen_paths = Vec::new();
    let mut found_common_test = false;
    let mut common_test_defines_its_own_host = false;
    let mut own_builder_offenders = Vec::new();
    let mut missing_import_offenders = Vec::new();

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read test directory is readable") {
            let path = entry.expect("read test entry is readable").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.ends_with(".test.ts") {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .expect("read test path is inside the workspace")
                .to_string_lossy()
                .into_owned();
            seen_paths.push(relative.clone());

            let source = fs::read_to_string(&path).expect("test source is readable");
            let defines_own_builder = defines_install_figma(&source);

            if relative == COMMON_TEST_PATH {
                found_common_test = true;
                common_test_defines_its_own_host = defines_own_builder;
                continue;
            }
            if defines_own_builder {
                own_builder_offenders.push(relative.clone());
            }

            let installs_a_host = defines_own_builder || assigns_figma(&source);
            if installs_a_host
                && !NAMED_EXEMPT_FROM_HARNESS_IMPORT.contains(&relative.as_str())
                && !imports_figma_harness(&source)
            {
                missing_import_offenders.push(relative);
            }
        }
    }
    own_builder_offenders.sort();
    missing_import_offenders.sort();

    let missing_migrated_files: Vec<&str> = HARNESS_MIGRATED_FILES
        .into_iter()
        .filter(|expected| !seen_paths.iter().any(|seen| seen == expected))
        .collect();

    assert!(
        found_common_test,
        "expected the walk to find {COMMON_TEST_PATH}; the walk or its \
         exemption list has drifted from the files on disk"
    );
    assert!(
        missing_migrated_files.is_empty(),
        "expected the walk to find these files this plan migrated onto the \
         shared harness, but they were renamed, moved, or deleted: \
         {missing_migrated_files:?}"
    );
    assert!(
        common_test_defines_its_own_host,
        "common.test.ts must still define its own installFigma: it installs \
         a deliberately incomplete host to test capability detection, which \
         the shared harness cannot express, and this is the positive control \
         proving the definition predicate still fires"
    );
    assert!(
        own_builder_offenders.is_empty(),
        "these read tests define their own Figma builder instead of \
         importing plugin/tests/figma-harness: {own_builder_offenders:?}"
    );
    assert!(
        missing_import_offenders.is_empty(),
        "these read tests do not import the shared Figma harness \
         (plugin/tests/figma-harness): {missing_import_offenders:?}"
    );
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
fn the_read_dispatcher_mutates_no_process_global_host_state() {
    // The traversal gate used to flip `skipInvisibleInstanceChildren` off for
    // the exclusive `includeHidden` path and back on afterward. With that flag
    // gone there is no exclusive mode left to protect, so the dispatcher has
    // no reason to touch that switch — or any other Figma write API — at all.
    //
    // That assignment is no longer checked here: it used to be, but checking
    // only `dispatch.ts` scoped the guard to the one file the flag happened to
    // live in before this branch, not to the flag itself. Now that the gate
    // that owned it is gone, nothing pins `skipInvisibleInstanceChildren` to
    // any particular module, so `plugin_source_denies_mutation_private_and_motion_write_apis`
    // checks it across all of `plugin/src` instead, the same way it already
    // checks `currentPage`, `selection`, and `fontName`. This test keeps only
    // the `MUTATION_DENYLIST` half, which is still a real assertion specific
    // to the dispatcher: no other write surface has snuck into the one
    // function every read request passes through.
    let dispatch =
        fs::read_to_string(workspace_root().join("plugin/src/main/dispatch.ts")).unwrap();
    for forbidden in MUTATION_DENYLIST {
        assert!(
            !dispatch.contains(forbidden),
            "dispatch.ts contains forbidden write surface {forbidden}"
        );
    }
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

/// The body of one `async fn` in a Rust source, from its signature up to the
/// next item. Returns an empty string when the handler is absent.
fn handler_body<'a>(source: &'a str, name: &str) -> &'a str {
    let Some((_, after)) = source.split_once(&format!("async fn {name}(")) else {
        return "";
    };
    let end = ["\n    async fn ", "\n}", "\nfn "]
        .iter()
        .filter_map(|boundary| after.find(boundary))
        .min()
        .unwrap_or(after.len());
    &after[..end]
}

#[test]
fn documentation_states_the_exact_strategy_resource_uris() {
    let docs = operator_documentation();
    for name in PROMPT_NAMES {
        let uri = resource_uri(name);
        assert!(
            docs.contains(&uri),
            "operator docs must name exact strategy resource {uri}"
        );
    }
    let readme = require_file("README.md");
    let advertised = readme.matches(RESOURCE_URI_PREFIX).count();
    assert_eq!(
        advertised,
        PROMPT_NAMES.len(),
        "README must advertise exactly the {} strategy resources",
        PROMPT_NAMES.len()
    );
}

#[test]
fn serving_a_strategy_resource_never_reaches_the_broker() {
    let service = require_file("crates/tools/src/service.rs");
    for handler in ["list_resources", "read_resource"] {
        let body = handler_body(&service, handler);
        assert!(!body.is_empty(), "service must handle resources/{handler}");
        assert!(
            !body.contains("self.broker"),
            "{handler} must serve compiled-in text, not a plugin round trip"
        );
    }

    let resources = require_file("crates/prompts/src/resources.rs");
    for forbidden in ["broker", "BrokerClient", "fs::", "File::", "std::net"] {
        assert!(
            !resources.contains(forbidden),
            "the strategy resource catalog must stay static text ({forbidden})"
        );
    }
    assert!(
        resources.contains("include_str!") || resources.contains("prompt_definitions"),
        "the strategy resource catalog must reuse the prompt bodies"
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
