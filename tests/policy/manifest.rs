use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

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
                source.push_str(&fs::read_to_string(path).expect("TypeScript source is readable"));
                source.push('\n');
            }
        }
    }
    source
}

fn read_plugin_bundles(plugin_directory: &std::path::Path) -> std::io::Result<(String, String)> {
    let dist = plugin_directory.join("dist");
    let main = fs::read_to_string(dist.join("code.js"))?;
    let ui = fs::read_to_string(dist.join("index.html"))?;
    Ok((main, ui))
}

#[test]
fn manifest_is_the_exact_read_only_loopback_surface() {
    let path = project_root().join("plugin/manifest.json");
    let source = fs::read_to_string(path).expect("plugin manifest exists");
    let actual: Value = serde_json::from_str(&source).expect("plugin manifest is valid JSON");
    let expected = json!({
      "name": "Figma Dev MCP",
      "id": "figma-dev-mcp",
      "api": "1.0.0",
      "main": "dist/code.js",
      "ui": "dist/index.html",
      "editorType": ["dev"],
      "capabilities": ["inspect"],
      "documentAccess": "dynamic-page",
      "networkAccess": {
        "allowedDomains": ["ws://localhost:3056"],
        "reasoning": "Connects only to the local figma-dev-mcp broker for read-only developer inspection."
      }
    });
    assert_eq!(actual, expected);
}

#[test]
fn plugin_contexts_keep_network_and_figma_apis_separate() {
    let root = project_root().join("plugin/src");
    let main = production_typescript(root.join("main"));
    let ui = production_typescript(root.join("ui"));
    assert!(
        !main.contains("WebSocket"),
        "controller must not construct WebSockets"
    );
    assert!(
        !ui.contains("figma."),
        "iframe transport must not access the Figma API"
    );
}

#[test]
fn bundle_policy_requires_both_artifacts() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "figma-dev-mcp-policy-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("isolated policy fixture directory is created");

    assert!(read_plugin_bundles(&directory).is_err());
    fs::create_dir(directory.join("dist")).expect("fixture dist directory is created");
    assert!(read_plugin_bundles(&directory).is_err());
    fs::write(directory.join("dist/code.js"), "main").expect("main fixture is written");
    assert!(read_plugin_bundles(&directory).is_err());
    fs::write(directory.join("dist/index.html"), "ui").expect("UI fixture is written");
    assert_eq!(
        read_plugin_bundles(&directory).expect("both fixture bundles are readable"),
        ("main".to_owned(), "ui".to_owned())
    );

    fs::remove_dir_all(directory).expect("isolated policy fixture is removed");
}

#[test]
fn production_dispatch_is_closed_and_removed_operations_stay_absent() {
    let source = production_typescript(project_root().join("plugin/src"));
    for forbidden in [
        "get_css",
        "get_tokens",
        "handlers[",
        "eval(",
        "new Function(",
        "createRectangle(",
        "setPluginData(",
    ] {
        assert!(
            !source.contains(forbidden),
            "production plugin source contains forbidden surface {forbidden}"
        );
    }

    // Bundle scans live in plugin/tests/policy/bundle.test.ts so this crate
    // stays green from a clean source checkout without plugin/dist.
    let _ = read_plugin_bundles;
}
