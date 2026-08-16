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
    let source = production_typescript(project_root().join("plugin/src"));

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
