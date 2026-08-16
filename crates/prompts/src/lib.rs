//! MCP prompt catalog for `figma-dev-mcp`.

mod catalog;
mod validation;

pub use catalog::{
    CACHE_TTL_MS, PromptDefinition, get_prompt_result, prompt_by_name, prompt_definitions,
    prompts_catalog,
};
pub use validation::extracted_tool_references;
