//! Policy tests.

use std::path::PathBuf;

mod allowlists;
mod manifest;
mod plugin_source;
mod prompts;
mod public_contracts;
mod read_only;

/// The workspace root, derived from this test crate's own manifest
/// directory. Was defined identically in `prompts.rs` and `read_only.rs`
/// (differing only in their `.expect()` wording); consolidated here for the
/// same reason as `instructs` — two copies of one fact are one too many.
pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate sits in the workspace")
        .to_path_buf()
}

/// Does `haystack` contain `needle` as a whole identifier — not as a
/// substring of a longer one? Was defined identically in `prompts.rs` and
/// `read_only.rs`; consolidated here alongside `workspace_root`.
pub(crate) fn contains_ident(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        !before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

/// Does `haystack` instruct `phrase`, or negate it?
///
/// Exempts exactly three prefixes. The list is deliberately short: call sites
/// assert `!instructs(..)`, so every added exemption makes the scan weaker,
/// not stricter. `not` in particular is a suffix of `cannot`, `do not` and
/// `does not`, so exempting it would swallow cases nobody chose to exempt.
///
/// Pinned by `the_shared_instructs_exempts_exactly_three_negation_prefixes`.
pub(crate) fn instructs(haystack: &str, phrase: &str) -> bool {
    haystack.match_indices(phrase).any(|(index, _)| {
        let prefix = haystack[..index].trim_end();
        !prefix.ends_with("do not") && !prefix.ends_with("never") && !prefix.ends_with("without")
    })
}
