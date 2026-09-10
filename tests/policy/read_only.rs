//! Read-only policy proof over source, catalog, and wire shapes.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use super::{contains_ident, instructs, workspace_root};
use figma_dev_mcp_prompts::{RESOURCE_URI_PREFIX, resource_uri};
use figma_dev_mcp_protocol::wire::{BrokerToPlugin, ReadOperation};
use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};
use figma_dev_mcp_tools::tools_catalog;
use serde_json::{Value, json};

/// Whether `production_typescript` would open this path: a `.ts` file that is
/// neither a test nor the typecheck fixture. Extracted so the walk and the
/// positive controls below cannot end up with two spellings of one rule; the
/// controls additionally tie themselves to the walk by byte count, so a
/// divergence would still be caught if this were ever inlined again.
fn is_production_typescript(path: &Path) -> bool {
    let name = path.to_string_lossy();
    path.extension().is_some_and(|extension| extension == "ts")
        && !name.ends_with(".test.ts")
        && !name.ends_with("environment.typecheck.ts")
}

/// The production TypeScript files a `production_typescript` walk over
/// `directory` would read, as sorted repo-relative paths.
///
/// `production_typescript` returns only concatenated text, and text cannot
/// answer the question a positive control has to ask: *did the walk reach
/// anything, and did it reach the file this rule is about?* A walk that opens
/// zero files returns `""`, and `!"".contains(forbidden)` is true for every
/// forbidden string there is.
pub(crate) fn production_typescript_paths(root: &Path, directory: &Path) -> Vec<String> {
    let mut pending = vec![directory.to_path_buf()];
    let mut found = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).expect("plugin source directory is readable") {
            let path = entry.expect("plugin source entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else if is_production_typescript(&path) {
                found.push(
                    path.strip_prefix(root)
                        .expect("plugin source path is inside the workspace")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found.sort();
    found
}

/// The four production modules every `plugin/src` denylist scan below claims
/// to cover — one per subtree, each chosen because it is the file that would
/// carry the surface its scan forbids. Naming real paths rather than counting
/// means a control also fails when one of them moves, which is the thing worth
/// noticing.
pub(crate) const PRODUCTION_SOURCE_ANCHORS: [&str; 4] = [
    "plugin/src/main/dispatch.ts",
    "plugin/src/read/navigation.ts",
    "plugin/src/shared/result-validation.ts",
    "plugin/src/ui/socket.ts",
];

/// The number of production TypeScript files under `plugin/src` when this
/// sweep measured it. A floor, not an equality: the failure being pinned is a
/// walk that *shrinks* — a rename to `.mts`, a filter that stops matching, a
/// directory that moves — not one that grows.
pub(crate) const PRODUCTION_SOURCE_FILE_FLOOR: usize = 32;

/// The one root every production-source policy scan in this suite walks, and
/// the one every positive control below re-walks.
///
/// This exists so the two share a root rather than each computing its own.
/// With two roots, redirecting a scan's walk at an empty directory leaves its
/// control still looking at real files, and the control reports success over a
/// scan that read nothing — which is the exact failure this task was written to
/// close, reappearing one level up. Sharing the root is an extraction: it moves
/// no assertion and changes no scan's behaviour, and it is what makes the
/// walk-root mutation turn a control red.
pub(crate) fn plugin_source_root() -> PathBuf {
    workspace_root().join("plugin/src")
}

/// The generated error catalog, named once and read by both the scan that
/// exempts it and the control that checks the scan still reaches it.
///
/// It is the one production file
/// `production_plugin_source_spells_no_canonical_error_message` skips — it
/// spells every canonical message by construction — and it is also that scan's
/// positive control, the one file where the containment search must be able to
/// answer yes. Sharing the constant is what makes a *broadened* exemption
/// catchable: widening it is how that scan would quietly stop visiting files,
/// and the control's "the walk still reaches this exact path" assertion stops
/// matching at the same moment.
pub(crate) const GENERATED_ERROR_CATALOG: &str = "plugin/src/shared/error-catalog.ts";

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
    let generated = workspace_root().join(GENERATED_ERROR_CATALOG);
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

    // The walk is `production_typescript_paths`, shared with the control below
    // and with every other production-source scan in this suite, rather than a
    // fourth hand-written copy of the same root, recursion and filter. That is
    // an extraction, not a rewrite: the skip condition it replaces was
    // De Morgan-identical to `is_production_typescript`, and the set of files
    // opened is unchanged — 31, verified by instrumenting both versions and
    // diffing the lists. What it buys is that narrowing the root, the recursion
    // or the filter is now caught by the control, which it was not while this
    // walk was its own.
    let root = workspace_root();
    for relative in production_typescript_paths(&root, &plugin_source_root()) {
        if relative == GENERATED_ERROR_CATALOG {
            continue;
        }
        let path = root.join(&relative);
        let name = path.to_string_lossy().to_string();
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

#[test]
fn plugin_source_denies_mutation_private_and_motion_write_apis() {
    let source = production_typescript(plugin_source_root());
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

/// Strips the surrounding quotes from a module specifier sitting at the end
/// of an import statement — the last quoted run in `statement`, with any
/// trailing `;` and whitespace ignored. `None` if the statement does not end
/// in a quoted specifier, which is how a statement spread over several lines
/// reports "not finished yet".
fn trailing_module_specifier(statement: &str) -> Option<&str> {
    let statement = statement.trim_end().trim_end_matches(';').trim_end();
    let quote = statement
        .chars()
        .next_back()
        .filter(|ch| *ch == '"' || *ch == '\'')?;
    let body = &statement[..statement.len() - quote.len_utf8()];
    let open = body.rfind(quote)?;
    Some(&body[open + quote.len_utf8()..])
}

/// Normalises a path lexically — no filesystem access, no symlink
/// resolution — so `plugin/src/read/../../tests/figma-harness` collapses to
/// `plugin/tests/figma-harness`.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// True if `source` imports the *shared* harness — the actual module at
/// `plugin/tests/figma-harness.ts` — as opposed to merely mentioning the
/// string somewhere (a stale comment left behind by a migration away from it,
/// for instance) or importing some other module whose path happens to end the
/// same way.
///
/// The specifier is resolved lexically against `directory`, the importing
/// file's own directory, and compared to the harness's real location. A decoy
/// at `plugin/src/read/tests/figma-harness.ts`, imported as
/// `"./tests/figma-harness"`, therefore does *not* satisfy this — a suffix
/// match would have accepted it.
///
/// Statements spread over several lines are handled: a line that opens an
/// `import` without reaching its specifier is joined to the lines that follow
/// until one ends in a quoted specifier, so the multi-line form Prettier
/// produces once an import list grows is recognised.
///
/// Known limits, both of which fail *closed* — an unrecognised import reads
/// as "does not import the harness", so the assertions below fire rather than
/// pass quietly. Only relative specifiers are resolved: this repo configures
/// no path aliases, so a bare-specifier or aliased import of the harness
/// would not be recognised. And the statement must begin at the start of a
/// line with the word `import`; `await import(...)` and
/// `require("...")` are not.
fn imports_figma_harness(source: &str, directory: &Path, root: &Path) -> bool {
    let harness = root.join("plugin/tests/figma-harness");
    let mut statement = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if statement.is_empty() {
            if !trimmed.starts_with("import") {
                continue;
            }
            statement.push_str(trimmed);
        } else {
            statement.push(' ');
            statement.push_str(trimmed);
        }
        let Some(specifier) = trailing_module_specifier(&statement) else {
            // Not finished: either a still-open multi-line import, or a
            // statement that ended without a specifier. Drop the latter so a
            // malformed line cannot swallow the rest of the file.
            if statement.trim_end().ends_with(';') {
                statement.clear();
            }
            continue;
        };
        let resolved = specifier.starts_with('.')
            && normalize_lexically(&directory.join(specifier)) == harness;
        statement.clear();
        if resolved {
            return true;
        }
    }
    false
}

const COMMON_TEST_PATH: &str = "plugin/src/read/common.test.ts";

/// Read tests migrated onto the shared harness by this plan. Not exhaustive
/// of every read test — see the exemptions below — but each of these is held
/// to two things: the walk must still reach it (if the walk stops seeing one,
/// something moved, was renamed, or was deleted, and the other assertions
/// here would otherwise report success over whatever is left, `common.test.ts`
/// included), and it must import the shared harness, unconditionally.
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
/// tests capability detection across arbitrary combinations of absent keys,
/// and the harness's `omit*` options cover only the few specific keys some
/// read path needed — plus one documented exception below
/// (`navigation.test.ts`).
///
/// Two rules drive the import requirement, and they answer different
/// questions — neither subsumes the other:
///
/// **Derived.** A file that installs a host textually — `defines_install_figma`
/// or `assigns_figma` — must import the harness. Nothing is listed, so a
/// future test file that never touches `figma` needs no entry here and drops
/// out on its own, while a future file that *does* hand-roll a host and skips
/// the harness is caught without anyone remembering to add it to a list.
///
/// **Unconditional.** Every path in `HARNESS_MIGRATED_FILES` must import the
/// harness, whether or not it looks like it installs anything. The derived
/// rule cannot carry this: after the migration none of the nine installs a
/// host textually any more — they get one purely through the import — so
/// `installs_a_host` is false for all of them and the derived rule never
/// fires on the very files this test exists to pin. Without the unconditional
/// rule, a migrated file could drift back off the harness by any route and
/// stay green.
///
/// Known limits. Both predicates are name- and syntax-matching, not parsing,
/// so an unrecognised spelling of either can still slip through:
/// `defines_install_figma`'s and `assigns_figma`'s doc comments say which
/// (`Object.assign(globalThis, { figma })` is one that does).
/// Structurally: **the walk opens only `*.test.ts`**. A builder moved into a
/// sibling module that is not a test file — `plugin/src/read/fake-host.ts`,
/// say — is never read, so neither predicate can see it. That route is closed
/// for the nine by the unconditional rule (dropping the harness import to
/// take it is itself the failure), but a *new* `*.test.ts` file importing a
/// hand-rolled builder from such a sibling is outside this test's reach.
/// Resolving imports to decide whether a specifier reaches a host builder is
/// a different, larger job than this scan does.
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
/// This makes six assertions: the walk actually reaches `common.test.ts`
/// (otherwise every check below could pass over an empty or renamed
/// directory); it also reaches every file in `HARNESS_MIGRATED_FILES`
/// (otherwise the first floor alone would still pass with only
/// `common.test.ts` left); `common.test.ts` still defines its own host, so
/// its exemption cannot rot into a dead branch that always passes; no other
/// file defines its own builder; every file in `HARNESS_MIGRATED_FILES`
/// imports the shared harness; and every file that installs a host and is
/// not named above imports it too.
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
    let mut migrated_without_import_offenders = Vec::new();

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

            let file_directory = path
                .parent()
                .expect("read test file has a parent directory")
                .to_path_buf();
            let imports_harness = imports_figma_harness(&source, &file_directory, &root);

            // Unconditional: a migrated file must import the harness however
            // it now gets its host. Nothing derived from the file's own text
            // can stand in for this — see the doc comment above.
            if HARNESS_MIGRATED_FILES.contains(&relative.as_str()) && !imports_harness {
                migrated_without_import_offenders.push(relative.clone());
            }

            let installs_a_host = defines_own_builder || assigns_figma(&source);
            if installs_a_host
                && !NAMED_EXEMPT_FROM_HARNESS_IMPORT.contains(&relative.as_str())
                && !imports_harness
            {
                missing_import_offenders.push(relative);
            }
        }
    }
    own_builder_offenders.sort();
    missing_import_offenders.sort();
    migrated_without_import_offenders.sort();

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
        migrated_without_import_offenders.is_empty(),
        "these read tests were migrated onto the shared Figma harness \
         (plugin/tests/figma-harness) and no longer import it, so they have \
         drifted back onto a Figma model of their own: \
         {migrated_without_import_offenders:?}"
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

// ---------------------------------------------------------------------------
// Positive controls.
//
// Every scan above is a denial: it reads something and asserts a forbidden
// string is absent. The failure mode of a denial is not "it fires on compliant
// code" — the repository is the fixture, so that would go red on the spot. It
// is **passing on nothing**: a walk that opens zero files returns `""`, and
// `!"".contains(anything)` is true. From the outside a scan that found nothing
// wrong and a scan that looked at nothing are the same green tick.
//
// Each control below therefore asserts two things: that the scan reached a
// *named real file* in this repository, and that the predicate it applies
// still fires on a case that should trip it. Where a predicate's discriminating
// branch never evaluates on the real inputs, the control says so with the
// number that was measured, because a rule gated on a condition is only as good
// as how often that condition holds.
// ---------------------------------------------------------------------------

/// The `plugin/src` walk behind `plugin_source_denies_mutation_private_and_motion_write_apis`
/// reaches real files, and the denylist it carries is not empty.
///
/// Measured: the walk opens 32 production `.ts` files; `MUTATION_DENYLIST`
/// carries 23 entries. Three separate emptyings each left the whole policy
/// suite at 31 passed / 0 failed before this test existed — redirecting the
/// walk root to an empty directory, narrowing the filter so it matches no
/// file, and replacing the constant with `&[]`.
///
/// **All three are caught, and the first one only because the root is shared.**
/// The scan and this control both read `plugin_source_root()`. When they each
/// computed their own, redirecting the *scan's* root left this control walking
/// the real tree — 41 passed / 0 failed, a control reporting success over a
/// scan that had read nothing. The filter narrowing is caught by the byte-sum
/// below: `production_typescript` re-implements the filter, so if the two
/// implementations disagree their totals disagree with them.
#[test]
fn the_mutation_denylist_scan_reaches_real_plugin_source_and_its_list_is_not_empty() {
    let root = workspace_root();
    let paths = production_typescript_paths(&root, &plugin_source_root());
    assert!(
        paths.len() >= PRODUCTION_SOURCE_FILE_FLOOR,
        "the plugin/src walk opened {} production TypeScript files, fewer than \
         the {PRODUCTION_SOURCE_FILE_FLOOR} this sweep measured; a scan that \
         reaches nothing passes every denial it makes: {paths:?}",
        paths.len()
    );
    for anchor in PRODUCTION_SOURCE_ANCHORS {
        assert!(
            paths.iter().any(|path| path == anchor),
            "the plugin/src walk no longer reaches {anchor}; it was renamed, \
             moved, or the walk's filter stopped matching it"
        );
    }

    // Tie the control to the scan. `production_typescript` re-implements the
    // filter; if the two ever disagree the byte totals disagree with them, so
    // this control cannot go on passing over a walk that has narrowed.
    let source = production_typescript(plugin_source_root());
    let expected: usize = paths
        .iter()
        .map(|path| {
            fs::read_to_string(root.join(path))
                .expect("production TypeScript is readable")
                .len()
                + 1
        })
        .sum();
    assert_eq!(
        source.len(),
        expected,
        "production_typescript read a different set of files than \
         production_typescript_paths listed, so this control no longer speaks \
         for the scan it is controlling"
    );

    assert_eq!(
        MUTATION_DENYLIST.len(),
        23,
        "MUTATION_DENYLIST carried 23 entries when this sweep measured it; a \
         loop over an emptied list asserts nothing and stays green"
    );
    assert!(
        MUTATION_DENYLIST.contains(&"createRectangle"),
        "MUTATION_DENYLIST no longer names createRectangle, the node-creation \
         surface a read-only plugin exists to not have"
    );
    assert!(
        MUTATION_DENYLIST.contains(&"setTimelineDuration"),
        "MUTATION_DENYLIST no longer names setTimelineDuration, the Motion \
         write surface the same scan was extended to cover"
    );

    // The predicate itself. Every one of the 32 files is clean, so the
    // `contains` search never returns true on real input — it is asserted
    // false 23 times per file and nothing here ever proves it can say yes.
    for forbidden in MUTATION_DENYLIST {
        let planted = format!("const x = 1\nawait figma.{forbidden}\n");
        assert!(
            planted.contains(forbidden),
            "the denylist scan's own substring search failed to find \
             {forbidden} in a line that spells it"
        );
    }
}

/// The canonical-error-message scan reaches both of its inputs: the generated
/// catalog it parses messages out of, and the production files it looks for
/// them in.
///
/// The catalog half already had a floor (`messages.len() >= 17`, and 17 is
/// exactly what the catalog yields today). The walk half had none: pointing it
/// at an empty directory left the policy suite at 31 passed / 0 failed.
///
/// **The scan and this control now share the whole walk**, not just its root:
/// both call `production_typescript_paths(&root, &plugin_source_root())`, and
/// both name the exempted file with `GENERATED_ERROR_CATALOG`. So every way of
/// making that scan see less is caught here, each measured:
///
/// | emptying | before sharing | now |
/// |---|---|---|
/// | walk root redirected at an empty tree | 41 / 0 | 37 / 4 |
/// | shared filter narrowed to a extension nothing has | 41 / 0 | 37 / 4 |
/// | walk stops recursing into subdirectories | — | 37 / 4 |
/// | exemption broadened so it matches nothing real | — | 39 / 2 |
/// | exemption redirected at a different production file | — | 40 / 1, and the red one is the scan itself, which then reads the catalog and finds all 17 messages in it |
///
/// The scan reached this state in two steps and the first one was wrong. It
/// began with its own inline copy of the root, the recursion and the filter,
/// and this comment claimed the tie was impossible because "the inline walk
/// builds no concatenated text to compare against" — reasoning from the one
/// tie the denylist scan next door happens to use, and generalising from it.
/// A reviewer closed it in three commands: the inline skip condition was
/// De Morgan-identical to `is_production_typescript`, so the walk could simply
/// *be* the shared one. Behaviour-preserving, and verified as such rather than
/// asserted — instrumented at both versions, both open the same 31 files.
///
/// The lesson is the task's own, one level up: **an impossibility claim needs
/// a demonstration exactly as much as a coverage claim does.** This one had
/// none, and it was wrong.
#[test]
fn the_canonical_message_scan_reaches_the_catalog_and_every_production_file() {
    let root = workspace_root();
    let generated = root.join(GENERATED_ERROR_CATALOG);
    let catalog = fs::read_to_string(&generated).expect("generated catalog is readable");
    let messages: Vec<&str> = catalog
        .lines()
        .filter_map(|line| line.split_once(": \"")?.1.strip_suffix("\","))
        .collect();
    assert_eq!(
        messages.len(),
        17,
        "the generated catalog yielded {} messages, not the 17 this sweep \
         measured; the parse has drifted from the generator's output",
        messages.len()
    );

    // The positive control for the containment predicate. The catalog is the
    // one file exempted from the scan precisely because it spells every
    // message — so it is also the one file that proves the search can say yes.
    // Without this, the exemption could rot into a branch that always passes.
    for message in &messages {
        assert!(
            catalog.contains(message),
            "the scan's substring search did not find {message:?} in the very \
             file it was parsed out of"
        );
    }

    let paths = production_typescript_paths(&root, &plugin_source_root());
    assert!(
        paths.len() >= PRODUCTION_SOURCE_FILE_FLOOR,
        "the canonical-message walk opened {} production files, fewer than the \
         {PRODUCTION_SOURCE_FILE_FLOOR} this sweep measured",
        paths.len()
    );
    for anchor in PRODUCTION_SOURCE_ANCHORS {
        assert!(
            paths.iter().any(|path| path == anchor),
            "the canonical-message walk no longer reaches {anchor}"
        );
    }
    assert!(
        paths.iter().any(|path| path == GENERATED_ERROR_CATALOG),
        "the walk must still reach {GENERATED_ERROR_CATALOG} — it is skipped by \
         an explicit exemption, not by being invisible to the walk, and the \
         scan and this assertion name it with the same constant so a broadened \
         exemption stops matching here too"
    );
}

/// The input-schema scan collects property names from every nesting level it
/// claims to reach.
///
/// `collect_property_names` recurses through `properties`, `oneOf`/`anyOf`/
/// `allOf`, `$defs` and `items`. Measured over the 14 real schemas those
/// branches are entered 58, 37, 13 and 19 times respectively, for 89 names in
/// total — but discarding every collected name left the policy suite at
/// 31 passed / 0 failed, because the forbidden-key loop then has nothing to
/// loop over.
///
/// `nodeIds`, `pageIds` and `selection` are the interesting names: on eleven of
/// the fourteen tools they are reachable *only* through `$defs` and a `oneOf`
/// branch, so asserting them pins the deep recursion rather than the shallow
/// `properties` pass that a narrowed implementation would keep.
///
/// One measured vacuity is recorded rather than fixed: `list_files` has no
/// properties at all, so the forbidden-key check over it is empty by
/// construction. That is a property of the schema, not of the scan.
#[test]
fn the_input_schema_scan_collects_names_from_every_level_it_claims_to_reach() {
    let mut total = 0usize;
    let mut list_files_names = Vec::new();
    let mut tools_with_deep_only_names = 0usize;
    let mut tools_reaching_selection = 0usize;
    let mut deep_only_union: Vec<String> = Vec::new();
    for tool in tools_catalog().tools {
        let schema = Value::Object((*tool.input_schema).clone());
        let mut names = Vec::new();
        collect_property_names(&schema, &mut names);
        total += names.len();
        let shallow: Vec<String> = schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| properties.keys().cloned().collect())
            .unwrap_or_default();
        let deep_only: Vec<&String> = names
            .iter()
            .filter(|name| !shallow.contains(name))
            .collect();
        if !deep_only.is_empty() {
            tools_with_deep_only_names += 1;
        }
        if deep_only.iter().any(|name| *name == "selection") {
            tools_reaching_selection += 1;
        }
        for name in deep_only {
            if !deep_only_union.contains(name) {
                deep_only_union.push(name.clone());
            }
        }
        if tool.name.as_ref() == "list_files" {
            list_files_names = names;
        }
    }
    assert!(
        total >= 89,
        "the input-schema scan collected {total} property names across the \
         catalog, fewer than the 89 this sweep measured; a scan that collects \
         nothing accepts every forbidden key there is"
    );
    deep_only_union.sort();
    assert_eq!(
        deep_only_union,
        ["nodeId", "nodeIds", "pageId", "pageIds", "selection"],
        "these are the property names no schema exposes at its top level — the \
         recursion through $defs and oneOf is the only thing that reaches them, \
         so losing them is how a narrowed scan would look"
    );
    assert_eq!(
        tools_with_deep_only_names, 10,
        "10 of the 14 schemas carried names reachable only below the top-level \
         properties map when this sweep measured it, and they are what makes \
         the recursion load-bearing"
    );
    assert_eq!(
        tools_reaching_selection, 9,
        "9 schemas reached `selection` only through the selector union"
    );
    assert!(
        list_files_names.is_empty(),
        "list_files gained input properties; it had none when this sweep \
         measured it, which is why the forbidden-key check over it is vacuous \
         and recorded as such"
    );

    assert_eq!(
        FORBIDDEN_INPUT_KEYS.len(),
        19,
        "FORBIDDEN_INPUT_KEYS carried 19 entries when this sweep measured it"
    );

    // The predicate fires at depth. A forbidden key planted under
    // $defs -> oneOf -> properties -> items -> properties is reported, so a
    // recursion that quietly stopped descending could not stay green.
    let planted = json!({
        "$defs": {
            "target": {
                "oneOf": [
                    {"properties": {"nodeId": {"type": "string"}}},
                    {"properties": {"batch": {"items": {"properties": {"filePath": {"type": "string"}}}}}}
                ]
            }
        },
        "properties": {"connectionId": {"type": "string"}}
    });
    let mut planted_names = Vec::new();
    collect_property_names(&planted, &mut planted_names);
    for expected in ["connectionId", "nodeId", "batch", "filePath"] {
        assert!(
            planted_names.iter().any(|name| name == expected),
            "collect_property_names missed {expected} in a schema that nests it \
             under $defs, oneOf, properties and items: {planted_names:?}"
        );
    }
    assert!(
        FORBIDDEN_INPUT_KEYS.contains(&"filePath"),
        "the planted key must be one the scan would actually reject, or this \
         control proves nothing about the rule"
    );
}

/// A well-formed broker request carrying an allowlisted operation still
/// decodes, and decodes to what was sent.
///
/// `write_shaped_mcp_and_wire_requests_are_rejected` asserts that seven
/// write-shaped operations fail to decode as a `BrokerToPlugin` — inside an
/// envelope the test writes by hand. Misspelling one envelope key
/// (`requestId` → `requestIdTypo`) makes *every* one of those decodes fail for
/// that reason instead, and the policy suite stayed at 31 passed / 0 failed.
/// The refusals were green while nothing was being refused for the reason
/// under test.
#[test]
fn a_broker_request_carrying_an_allowlisted_operation_still_decodes_intact() {
    let frame = json!({
        "type": "request",
        "requestId": "plugin-1",
        "deadlineMs": 100,
        "target": {},
        "operation": {"operation": "get_metadata", "input": {}}
    });
    let decoded: BrokerToPlugin = serde_json::from_value(frame.clone()).expect(
        "the envelope write_shaped_mcp_and_wire_requests_are_rejected writes must \
         decode when its operation is allowlisted; if it does not, every \
         refusal that test makes is satisfied by the envelope rather than by \
         the operation",
    );
    assert_eq!(
        serde_json::to_value(&decoded).expect("a decoded broker request re-serializes"),
        frame,
        "the broker request did not survive the round trip intact"
    );

    // The same envelope with each remaining read operation, so a refusal in
    // the test above cannot be satisfied by an operation tag the wire format
    // has silently stopped accepting either.
    for tag in TOOL_NAMES {
        if tag == "list_files" {
            continue; // served by the broker, never sent to the plugin
        }
        let input = match tag {
            "get_screenshot" => json!({"format": "png", "selector": {"nodeId": "1:2"}}),
            "get_nodes" => json!({"nodeIds": []}),
            "search_nodes" => json!({"scope": {"pageId": "0:1"}, "query": "Card"}),
            _ => json!({}),
        };
        let frame = json!({
            "type": "request",
            "requestId": "plugin-1",
            "deadlineMs": 100,
            "target": {},
            "operation": {"operation": tag, "input": input}
        });
        let decoded: BrokerToPlugin = serde_json::from_value(frame.clone())
            .unwrap_or_else(|error| panic!("{tag} must decode as a broker request: {error}"));
        let round_tripped = serde_json::to_value(&decoded).expect("re-serializes");
        // Not an equality: several inputs carry serialized defaults the sender
        // did not write (`get_design_context` gains `dedupeComponents: false`).
        // What must hold is that nothing the sender *did* write was dropped or
        // altered — accepted is not the same as arrived intact.
        for (key, value) in frame.as_object().expect("the frame is an object") {
            if key == "operation" {
                assert_eq!(
                    round_tripped["operation"]["operation"], value["operation"],
                    "{tag} decoded as a different operation than it was sent as"
                );
                for (field, sent) in value["input"].as_object().expect("input is an object") {
                    assert_eq!(
                        &round_tripped["operation"]["input"][field], sent,
                        "{tag} lost or altered the input field {field} it was sent with"
                    );
                }
            } else {
                assert_eq!(
                    &round_tripped[key], value,
                    "{tag} lost or altered the envelope field {key}"
                );
            }
        }
    }
}

/// The read dispatcher's source still names every operation it dispatches.
///
/// `the_read_dispatcher_mutates_no_process_global_host_state` reads
/// `plugin/src/main/dispatch.ts` and asserts 23 forbidden surfaces are absent.
/// Pointing that read at an empty file left the policy suite at
/// 31 passed / 0 failed: the `unwrap` catches a *missing* file, nothing caught
/// a file with nothing in it, or a dispatcher that had moved out from under
/// the path while a stub kept the name.
#[test]
fn the_read_dispatcher_source_still_names_every_operation_it_dispatches() {
    let dispatch = fs::read_to_string(workspace_root().join("plugin/src/main/dispatch.ts"))
        .expect("the read dispatcher source is readable");
    for name in TOOL_NAMES {
        if name == "list_files" {
            assert!(
                !dispatch.contains(name),
                "list_files is served by the broker and must not appear in the \
                 plugin's read dispatcher"
            );
            continue;
        }
        assert!(
            dispatch.contains(name),
            "dispatch.ts no longer names {name}; the file the mutation scan \
             reads is not the dispatcher any more, so every absence it asserts \
             is an absence from the wrong file"
        );
    }
}

/// The operator-documentation scan reaches all four documents, and the
/// predicates it applies to them still fire.
///
/// Three separate emptyings each left the policy suite at 31 passed / 0 failed:
/// replacing the concatenated corpus with `""` for
/// `documentation_forbids_local_export_instructions_and_unadvertised_product_tools`,
/// emptying the `OPERATOR_DOCS` enumeration under
/// `documentation_required_operator_files_exist`, and emptying
/// `SEVEN_LOCAL_VERIFICATION_COMMANDS` — whose test is named *all seven* and
/// counted none. Emptying `DIAGNOSTIC_CALL_NAMES` was green too.
///
/// The measurement that makes the `instructs` half necessary: across 59903
/// bytes of operator documentation, **all eight** forbidden phrases occur
/// **zero** times. So `instructs` is only ever asked about text that does not
/// contain the phrase, its negation-prefix branch never evaluates, and the
/// whole predicate could be replaced by `false` without the suite noticing.
/// Only a planted sample exercises it.
#[test]
fn the_operator_documentation_scan_reaches_all_four_files_and_instructs_still_fires() {
    assert_eq!(
        OPERATOR_DOCS,
        [
            "README.md",
            "docs/setup.md",
            "docs/testing.md",
            "docs/manual-acceptance.md",
        ],
        "OPERATOR_DOCS is the enumeration seven tests scan; an emptied or \
         narrowed list turns each of them into a loop over nothing"
    );

    let combined = operator_documentation();
    let mut expected = 0usize;
    for relative in OPERATOR_DOCS {
        let body = require_file(relative);
        assert!(
            !body.trim().is_empty(),
            "{relative} is empty, so every phrase the scans forbid is trivially \
             absent from it"
        );
        assert!(
            combined.contains(&body),
            "operator_documentation() does not contain the text of {relative}; \
             the corpus the doc scans read is not the four documents"
        );
        expected += body.len() + 1;
    }
    assert_eq!(
        combined.len(),
        expected,
        "operator_documentation() is not the concatenation of exactly the four \
         OPERATOR_DOCS"
    );

    // `instructs`, in both directions, on planted text. Nothing in the real
    // corpus exercises either branch.
    //
    // `instructs` is now a single shared implementation in `super` (formerly
    // duplicated here and in `prompts.rs` with two different exemption
    // lists — a six-prefix one here and a three-prefix one there, which
    // disagreed on the same sentence). The full three-prefix exemption list
    // is pinned by `the_shared_instructs_exempts_exactly_three_negation_prefixes`
    // below; this block only pins that the doc scan actually reaches a live
    // `instructs` call along the plain-instruction and `do not`/`never` paths.
    assert!(
        instructs("you may save to a path when exporting", "save to a path"),
        "instructs no longer reports a plain instruction, so every phrase the \
         doc scan forbids would pass unread"
    );
    assert!(
        !instructs("do not save to a path", "save to a path"),
        "instructs lost its negation exemption"
    );
    assert!(
        !instructs("never save to a path", "save to a path"),
        "instructs lost the `never` half of its negation exemption, so a \
         document that forbids a phrase would read as instructing it"
    );

    // `contains_ident` is exercised by the real corpus in the opposite
    // direction — it is what keeps `get_nodes` from reading as `get_node` —
    // but only ever answers "no" to the forbidden names, so pin both sides.
    assert!(
        contains_ident("run list_files first", "list_files"),
        "contains_ident no longer reports an identifier that is present, so the \
         14 tool names and 3 prompt names the doc scan requires would all read \
         as missing"
    );
    assert!(
        !contains_ident("run list_files_v2 first", "list_files"),
        "contains_ident lost its trailing-boundary check and now matches a \
         longer identifier that merely starts with the name"
    );
    assert!(
        !contains_ident("read durationMsField", "durationMs"),
        "contains_ident lost its trailing-boundary check for the forbidden \
         direction: `durationMsField` would now read as documenting durationMs"
    );

    assert_eq!(
        SEVEN_LOCAL_VERIFICATION_COMMANDS.len(),
        7,
        "the test that checks these is named for the count and never checked it"
    );
    assert_eq!(
        DIAGNOSTIC_CALL_NAMES.len(),
        3,
        "DIAGNOSTIC_CALL_NAMES carried 3 entries when this sweep measured it; \
         an emptied list forbids no unadvertised diagnostic"
    );
    assert!(
        DIAGNOSTIC_CALL_NAMES.contains(&"test_missing_capability"),
        "DIAGNOSTIC_CALL_NAMES no longer names test_missing_capability, the \
         diagnostic the README table rule was written about"
    );
}

#[test]
fn the_shared_instructs_exempts_exactly_three_negation_prefixes() {
    assert!(
        instructs("you may save to a path when exporting", "save to a path"),
        "a plain statement must read as an instruction"
    );
    for exempt in ["do not", "never", "without"] {
        assert!(
            !instructs(&format!("{exempt} save to a path"), "save to a path"),
            "`{exempt}` must be exempt"
        );
    }
    for not_exempt in ["does not", "no", "not", "cannot"] {
        assert!(
            instructs(&format!("{not_exempt} save to a path"), "save to a path"),
            "`{not_exempt}` must NOT be exempt — widening the list weakens the scan"
        );
    }
}

const ACCEPTANCE_RECORD: &str = "docs/acceptance-sweep.md";

/// Repo-relative paths of every source file a record citation could name.
/// Build output, dependency trees and the git directory are skipped.
fn repository_files(root: &Path) -> Vec<String> {
    const SKIPPED: [&str; 5] = ["target", "node_modules", ".git", "dist", ".superpowers"];
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if path.is_dir() {
                if !SKIPPED.contains(&name.as_str()) {
                    pending.push(path);
                }
            } else if name.ends_with(".rs") || name.ends_with(".ts") {
                found.push(
                    path.strip_prefix(root)
                        .expect("repository path is inside the workspace")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found.sort();
    found
}

/// Every `<path>.rs:<line>` / `<path>.ts:<line>` citation in `record`, in
/// order, as `(path fragment, line number)`. The path fragment is whatever
/// path-shaped run of characters immediately precedes the colon, so both a
/// bare `common.rs:446` and a qualified `domain/common.rs:446` are recognised
/// — and only the second of those resolves, which is the point.
fn file_line_citations(record: &str) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for line in record.lines() {
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b':' {
                index += 1;
                continue;
            }
            let mut end = index + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > index + 1 {
                let mut start = index;
                while start > 0
                    && matches!(bytes[start - 1],
                        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/')
                {
                    start -= 1;
                }
                let path = &line[start..index];
                if (path.ends_with(".rs") || path.ends_with(".ts"))
                    && let Ok(number) = line[index + 1..end].parse::<usize>()
                {
                    found.push((path.to_owned(), number));
                }
            }
            index = end.max(index + 1);
        }
    }
    found
}

/// Every `file:line` citation in the acceptance record resolves to a line that
/// exists and carries something.
///
/// The record's own citation convention says a line number survives only where
/// it "has been checked against the tree" — a claim with no verification story,
/// in a file whose 53 surviving citations all point into one 2884-line source
/// that is still being edited. Nothing ran that check; this does.
///
/// It checks what is mechanically checkable and no more: the cited path
/// resolves to exactly one file in the tree, the line number is in range, and
/// the line is not blank or a lone brace. It cannot check that the line means
/// what the row says it means. What it does catch is the decay that actually
/// happens — the cited file is edited, every number after the edit point
/// shifts, and the citations quietly stop pointing at anything.
///
/// Measured on the record as this task found it: 123 citation occurrences, 54
/// distinct, 53 of them into `result-validation.ts`. All three predicates
/// return "fine" on all 54, so none of them is exercised by the repository and
/// each needs the planted control below. The one that would *not* have passed
/// is worth naming: the record's single `common.rs` citation was written as a
/// bare basename, and two tracked files carry that name — it is qualified to
/// `domain/common.rs` now, and the assertion that the ambiguity is real keeps
/// the resolution rule from being decorative.
///
/// **Known limit, measured rather than assumed — and the direction matters.**
/// Simulating a uniform shift of the 53 `result-validation.ts` citations and
/// counting how many land on a blank line or a lone bracket:
///
/// | lines | **deletion** above (cited `n` now shows old `n+k`) | **insertion** above (cited `n` now shows old `n−k`) |
/// |---|---|---|
/// | 1 | 0 of 53 — **silent** | 2 of 53 — fires |
/// | 2 | 0 of 53 — **silent** | 33 of 53 — fires |
/// | 3 | 2 of 53 — fires | 30 of 53 — fires |
/// | 5 | 41 of 53 — fires | 18 of 53 — fires |
///
/// So the hole is on **deletion**, at one and two lines, and insertions are
/// caught from a single line up. Both confirmed against the real tree, not
/// only simulated: inserting one blank line at the top of
/// `result-validation.ts` turns this red, and deleting one line from the top
/// leaves the suite green. Note that a five-line change fires in *both*
/// columns, so the five-line experiment does not distinguish the directions
/// and cannot be used to confirm either — which is how the direction came to
/// be stated backwards here in the first place.
///
/// It is a tripwire with a measured hole, not a verifier. Saying exactly which
/// hole is the point — "these citations have been checked against the tree"
/// was the claim with no verification story that this test exists to replace,
/// and replacing it with a second unchecked claim would be no better.
#[test]
fn every_file_line_citation_in_the_acceptance_record_resolves_to_a_real_line() {
    let root = workspace_root();
    let record = require_file(ACCEPTANCE_RECORD);
    let citations = file_line_citations(&record);

    assert!(
        citations.len() >= 100,
        "the citation parser found {} file:line citations in \
         {ACCEPTANCE_RECORD}, fewer than the 123 this sweep measured; a parser \
         that finds none verifies none",
        citations.len()
    );
    let into_result_validation = citations
        .iter()
        .filter(|(path, _)| path.ends_with("result-validation.ts"))
        .count();
    assert!(
        into_result_validation >= 50,
        "only {into_result_validation} citations point into result-validation.ts, \
         fewer than the 53 the convention was written about"
    );

    let files = repository_files(&root);
    assert!(
        files.len() >= 100,
        "the repository walk found {} source files; citation resolution over an \
         empty tree resolves nothing and would report every citation broken",
        files.len()
    );

    let mut unresolved = Vec::new();
    let mut out_of_range = Vec::new();
    let mut points_at_nothing = Vec::new();
    for (path, number) in &citations {
        let suffix = format!("/{path}");
        let matched: Vec<&String> = files
            .iter()
            .filter(|file| *file == path || file.ends_with(&suffix))
            .collect();
        let [single] = matched.as_slice() else {
            unresolved.push(format!("{path}:{number} -> {matched:?}"));
            continue;
        };
        let body = fs::read_to_string(root.join(single)).expect("cited source is readable");
        let lines: Vec<&str> = body.lines().collect();
        if *number == 0 || *number > lines.len() {
            out_of_range.push(format!(
                "{path}:{number} (the file has {} lines)",
                lines.len()
            ));
            continue;
        }
        let text = lines[number - 1].trim();
        if text.is_empty() || matches!(text, "}" | "};" | "});" | "{" | ")" | ");" | "]" | "],") {
            points_at_nothing.push(format!("{path}:{number} -> {text:?}"));
        }
    }
    assert!(
        unresolved.is_empty(),
        "these {ACCEPTANCE_RECORD} citations name no file, or name a path that \
         several files share — a citation that cannot be resolved cannot be \
         checked: {unresolved:?}"
    );
    assert!(
        out_of_range.is_empty(),
        "these {ACCEPTANCE_RECORD} citations point past the end of the file \
         they name; the file was edited and the record was not: {out_of_range:?}"
    );
    assert!(
        points_at_nothing.is_empty(),
        "these {ACCEPTANCE_RECORD} citations point at a blank line or a lone \
         bracket, which is what a citation looks like after the lines above it \
         moved: {points_at_nothing:?}"
    );

    // Planted controls. None of the three rules above is exercised by the
    // record as it stands, so each is proved here on a case that trips it.
    assert_eq!(
        file_line_citations("see `domain/common.rs:446` and result-validation.ts:1090 for both"),
        vec![
            ("domain/common.rs".to_owned(), 446),
            ("result-validation.ts".to_owned(), 1090)
        ],
        "the citation parser stopped recognising the two forms the record uses"
    );
    assert!(
        file_line_citations("a table cell reading 260 / 0 and a port 127.0.0.1:3056").is_empty(),
        "the citation parser is matching things that are not citations"
    );
    assert!(
        files
            .iter()
            .filter(|file| file.ends_with("/common.rs"))
            .count()
            >= 2,
        "two tracked files were named common.rs when this sweep measured it, \
         which is what makes the resolve-to-exactly-one rule load-bearing \
         rather than decorative; if only one remains, this control no longer \
         proves the rule can say no"
    );
    let cited = root.join("plugin/src/shared/result-validation.ts");
    let body = fs::read_to_string(&cited).expect("the most-cited source is readable");
    assert!(
        body.lines().count() >= 2000,
        "result-validation.ts has {} lines; the 53 citations into it were \
         written against a file of 2884",
        body.lines().count()
    );
}
