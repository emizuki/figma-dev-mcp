use std::collections::BTreeSet;

const TOOL_LIKE_VERBS: &[&str] = &[
    "add",
    "apply",
    "bind",
    "call",
    "convert",
    "create",
    "delete",
    "edit",
    "export",
    "generate",
    "get",
    "import",
    "insert",
    "install",
    "list",
    "load",
    "move",
    "patch",
    "put",
    "remove",
    "rename",
    "replace",
    "run",
    "save",
    "scan",
    "search",
    "set",
    "substitute",
    "swap",
    "update",
    "write",
];

/// Backticked snake_case identifiers whose first segment is a tool-like verb.
pub fn extracted_tool_references(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let token = &rest[..end];
        rest = &rest[end + 1..];
        if let Some(name) = tool_like_identifier(token) {
            names.insert(name.to_owned());
        }
    }
    names
}

fn tool_like_identifier(token: &str) -> Option<&str> {
    let ident = token
        .split(|ch: char| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'))
        .next()
        .unwrap_or("");
    let mut parts = ident.split('_');
    let verb = parts.next()?;
    if TOOL_LIKE_VERBS.contains(&verb)
        && parts.next().is_some()
        && !ident.starts_with('_')
        && !ident.ends_with('_')
        && !ident.contains("__")
        && ident
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        Some(ident)
    } else {
        None
    }
}
