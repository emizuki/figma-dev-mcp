use std::{fs, path::PathBuf};

fn workspace_file(path: &str) -> String {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate must be inside the workspace")
        .to_path_buf();
    fs::read_to_string(workspace.join(path)).expect("workspace source must be readable")
}

#[test]
fn public_contract_wrappers_keep_protocol_types_behind_crate_private_conversions() {
    let common = workspace_file("crates/tools/src/contracts/common.rs");
    assert!(common.contains("pub struct $name($inner);"));
    assert!(common.contains("pub(crate) fn into_protocol"));
    assert!(common.contains("pub(crate) fn from_protocol"));
    assert!(!common.contains("pub struct $name(pub $inner);"));
    assert!(!common.contains("impl From<$name> for $inner"));
    assert!(!common.contains("impl From<$inner> for $name"));
    assert!(!common.contains("#[allow(dead_code)]"));
    assert!(!common.contains("#[macro_export]"));

    let visual = workspace_file("crates/tools/src/contracts/visual.rs");
    assert!(visual.contains("pub struct GetScreenshotInput(domain::GetScreenshotInput);"));
    assert!(visual.contains("pub(crate) fn into_protocol"));
    assert!(!visual.contains("pub struct GetScreenshotInput(pub domain::GetScreenshotInput);"));

    let dispatch = workspace_file("crates/tools/src/dispatch.rs");
    assert!(!dispatch.contains("public.0"));
    assert!(dispatch.contains("name: ToolName"));
    assert!(dispatch.contains("ToolName::GetMetadata"));
    assert!(!dispatch.contains("\"get_metadata\" =>"));
}
