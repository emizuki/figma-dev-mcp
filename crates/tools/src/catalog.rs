use crate::{contracts::*, names::ToolName};
use rmcp::model::{CacheScope, ListToolsResult, Tool, ToolAnnotations};
use schemars::JsonSchema;
use serde_json::{Map, Value};
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

pub const CACHE_TTL_MS: u64 = 86_400_000;

fn definition<I, O>(name: ToolName, description: &'static str) -> Tool
where
    I: JsonSchema + 'static,
    O: JsonSchema + 'static,
{
    let description = if name == ToolName::ListFiles {
        Cow::Borrowed(description)
    } else {
        Cow::Owned(format!(
            "{description} A connectionId is ephemeral and expires when its plugin socket reconnects."
        ))
    };
    Tool::new(name.as_str(), description, Arc::new(Map::new()))
        .with_input_schema::<I>()
        .with_output_schema::<O>()
        .with_annotations(
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .open_world(false),
        )
}

pub fn tools_catalog() -> ListToolsResult {
    let mut tools = vec![
        definition::<GetComponentsInput, GetComponentsResult>(
            ToolName::GetComponents,
            "Return components and component sets in a bounded node or page scope.",
        ),
        definition::<GetDesignContextInput, GetDesignContextResult>(
            ToolName::GetDesignContext,
            "Return an implementation-oriented bounded design tree.",
        ),
        definition::<GetDevModeDataInput, GetDevModeDataResult>(
            ToolName::GetDevModeData,
            "Return annotations, documentation, resources, and ownership metadata.",
        ),
        definition::<GetFontsInput, GetFontsResult>(
            ToolName::GetFonts,
            "Return fonts used by a bounded scope and their availability.",
        ),
        definition::<GetMetadataInput, GetMetadataResult>(
            ToolName::GetMetadata,
            "Return file, page, editor, and plugin capability metadata without descendants.",
        ),
        definition::<GetMotionInput, GetMotionResult>(
            ToolName::GetMotion,
            "Return bounded motion data for nodes when the Figma motion API is available.",
        ),
        definition::<GetNodesInput, GetNodesResult>(
            ToolName::GetNodes,
            "Fetch one or more nodes by opaque ID while preserving input order.",
        ),
        definition::<GetReactionsInput, GetReactionsResult>(
            ToolName::GetReactions,
            "Return prototype reactions and explicit target references.",
        ),
        definition::<GetScreenshotInput, GetScreenshotResult>(
            ToolName::GetScreenshot,
            "Render bounded nodes or the captured selection as raster or SVG assets. SVG source is always returned, with a `safe` verdict and, when unsafe, a `rejection` naming the rule that fired; safety never withholds the source. Treat an unsafe verdict as a caller decision: writing such source to disk can execute a `<script>` element if a browser later opens it. A node whose bounds enclose no area fails with `EMPTY_NODE_BOUNDS` in every format rather than returning an empty asset.",
        ),
        definition::<GetSelectionInput, GetSelectionResult>(
            ToolName::GetSelection,
            "Return the current selection with a requested detail level and bounded depth.",
        ),
        definition::<GetStylesInput, GetStylesResult>(
            ToolName::GetStyles,
            "Return styles referenced by a bounded scope, and the document's local styles. `selector` constrains only the `referenced` half; the `local` half is document-wide and ignores it. The default `both` therefore mixes a document-wide list with a scoped one.",
        ),
        definition::<GetVariablesInput, GetVariablesResult>(
            ToolName::GetVariables,
            "Return variable collections, modes, aliases, scopes, and code syntax.",
        ),
        definition::<ListFilesInput, ListFilesResult>(
            ToolName::ListFiles,
            "List live Figma connections; connection IDs expire when plugin sockets reconnect.",
        ),
        definition::<SearchNodesInput, SearchNodesResult>(
            ToolName::SearchNodes,
            "Search exactly one explicit page or node scope with bounded predicates.",
        ),
    ];
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    let tools = tools.into_iter().map(canonicalize_tool).collect();
    ListToolsResult::with_all_items(tools)
        .with_ttl_ms(CACHE_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

fn canonicalize_tool(mut tool: Tool) -> Tool {
    tool.input_schema = Arc::new(sort_object(tool.input_schema.as_ref().clone()));
    tool.output_schema = tool
        .output_schema
        .map(|schema| Arc::new(sort_object(schema.as_ref().clone())));
    tool
}

fn sort_object(object: Map<String, Value>) -> Map<String, Value> {
    object
        .into_iter()
        .map(|(key, value)| (key, sort_value(value)))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect()
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_value).collect()),
        Value::Object(object) => Value::Object(sort_object(object)),
        scalar => scalar,
    }
}
