//! Production plugin source must stay read-only.

use std::{fs, path::PathBuf};

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate has a workspace parent")
        .to_path_buf()
}

fn production_typescript(directory: PathBuf) -> String {
    let mut pending = vec![directory];
    let mut source = String::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("plugin source directory is readable") {
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
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with("*"))
}

fn has_property_assignment(source: &str, property: &str) -> bool {
    let dotted = format!(".{property} =");
    let figma = format!("figma.{property} =");
    code_lines(source)
        .any(|line| (line.contains(&dotted) || line.contains(&figma)) && !line.contains("=="))
}

#[test]
fn plugin_source_rejects_unbounded_page_font_and_mutation_surfaces() {
    let source = production_typescript(crate::read_only::plugin_source_root());

    for forbidden in [
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
        "applyAnimationStyle",
        "removeAnimationStyle",
        "applyManualKeyframeTrack",
        "removeManualKeyframeTrack",
        "setTimelineDuration",
    ] {
        assert!(
            !source.contains(forbidden),
            "production plugin source contains forbidden surface {forbidden}"
        );
    }

    assert!(
        !has_property_assignment(&source, "currentPage"),
        "production plugin source must not assign figma.currentPage"
    );
    assert!(
        !has_property_assignment(&source, "selection"),
        "production plugin source must not assign selection"
    );
    assert!(
        !has_property_assignment(&source, "fontName"),
        "production plugin source must not assign fontName"
    );
    assert!(
        code_lines(&source).all(|line| !line.contains("currentPage.selection =")),
        "production plugin source must not assign currentPage.selection"
    );
}

/// The `plugin/src` walk reaches real files, and both predicates this module
/// applies to them still fire.
///
/// Pointing the walk at an empty directory left the policy suite at
/// 31 passed / 0 failed: seventeen `!contains` assertions and four
/// `has_property_assignment` assertions, all true of the empty string.
///
/// **Which mutations this catches.** Both the walk-root redirect and this
/// module's own filter narrowing — the first only because the scan and the
/// control share one `plugin_source_root()`. With two roots the redirect left
/// the suite at 41 passed / 0 failed, the control walking the real tree while
/// the scan read nothing.
///
/// The measurement that makes the planted samples necessary:
/// `has_property_assignment` returns **false on every one of the 32 production
/// files**, for all four properties. It is only ever asked about text that does
/// not assign, so its `==` exemption never evaluates, and neither does
/// `code_lines`'s comment filter — the whole predicate could be replaced with
/// `false` and nothing above would notice.
#[test]
fn the_plugin_source_walk_reaches_real_files_and_the_assignment_predicate_fires() {
    let root = project_root();
    let paths = crate::read_only::production_typescript_paths(
        &root,
        &crate::read_only::plugin_source_root(),
    );
    assert!(
        paths.len() >= crate::read_only::PRODUCTION_SOURCE_FILE_FLOOR,
        "the plugin/src walk opened {} production files, fewer than the {} this \
         sweep measured: {paths:?}",
        paths.len(),
        crate::read_only::PRODUCTION_SOURCE_FILE_FLOOR
    );
    for anchor in crate::read_only::PRODUCTION_SOURCE_ANCHORS {
        assert!(
            paths.iter().any(|path| path == anchor),
            "the plugin/src walk no longer reaches {anchor}; it was renamed, \
             moved, or the walk's filter stopped matching it"
        );
    }
    let source = production_typescript(crate::read_only::plugin_source_root());
    assert!(
        source.contains("figma.") && source.contains("export"),
        "the walk read no production TypeScript, so every forbidden surface it \
         reports absent is absent from nothing"
    );

    // The predicate, in both directions, on planted text.
    assert!(
        has_property_assignment("figma.currentPage = page", "currentPage"),
        "has_property_assignment no longer detects a bare figma.* assignment, \
         so the four assertions above would pass over source that makes one"
    );
    assert!(
        has_property_assignment("host.selection = []", "selection"),
        "has_property_assignment no longer detects a receiver-dotted assignment"
    );
    assert!(
        !has_property_assignment("if (figma.currentPage == page) {}", "currentPage"),
        "has_property_assignment lost its comparison exemption and would now \
         fire on a read"
    );
    assert!(
        !has_property_assignment("// figma.currentPage = page", "currentPage"),
        "code_lines no longer drops comment lines, so a commented-out example \
         would fail the scan"
    );
    assert_eq!(
        code_lines("// comment\n\n * doc\n  real = 1\n").collect::<Vec<_>>(),
        ["real = 1"],
        "code_lines no longer trims and filters the way the assignment scan \
         assumes"
    );
}
