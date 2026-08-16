use tracing_subscriber::EnvFilter;

const ENV_NAME: &str = "FIGMA_DEV_MCP_LOG";
const FAMILY: &[&str] = &[
    "figma_dev_mcp",
    "figma_dev_mcp_broker",
    "figma_dev_mcp_tools",
    "figma_dev_mcp_protocol",
    "figma_dev_mcp_prompts",
];

pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter(filter_from_env())
        .with_writer(std::io::stderr)
        .init();
}

/// Compile `FIGMA_DEV_MCP_LOG` the same way production `init` does.
pub fn filter_from_env() -> EnvFilter {
    filter_from_directive(std::env::var(ENV_NAME).ok().as_deref())
}

/// When `directive` is a bare level, enable this crate family at that level and
/// keep `rmcp` at `info` so debug never dumps design payloads.
pub fn filter_from_directive(directive: Option<&str>) -> EnvFilter {
    match directive.map(str::trim).filter(|value| !value.is_empty()) {
        None => EnvFilter::new("info"),
        Some(value) if is_bare_level(value) => EnvFilter::new(crate_family_filter(value)),
        Some(value) => EnvFilter::try_new(value).unwrap_or_else(|_| EnvFilter::new("info")),
    }
}

fn is_bare_level(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "error" | "warn" | "info" | "debug" | "trace" | "off"
    )
}

fn crate_family_filter(level: &str) -> String {
    let mut filter = String::from("info");
    for target in FAMILY {
        filter.push(',');
        filter.push_str(target);
        filter.push('=');
        filter.push_str(level);
    }
    filter.push_str(",rmcp=info");
    filter
}

#[cfg(test)]
mod tests {
    use super::{crate_family_filter, filter_from_directive};

    #[test]
    fn bare_debug_enables_this_crate_family_and_keeps_rmcp_at_info() {
        let compiled = crate_family_filter("debug");
        assert!(compiled.contains("figma_dev_mcp_tools=debug"));
        assert!(compiled.contains("figma_dev_mcp_broker=debug"));
        assert!(compiled.contains("rmcp=info"));
        let filter = filter_from_directive(Some("debug"));
        let rendered = filter.to_string();
        assert!(
            rendered.contains("rmcp=info"),
            "bare debug must pin rmcp: {rendered}"
        );
    }
}
