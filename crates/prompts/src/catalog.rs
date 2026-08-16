use rmcp::model::{CacheScope, GetPromptResult, ListPromptsResult, Prompt, PromptMessage, Role};

pub const CACHE_TTL_MS: u64 = 86_400_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PromptDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

const PROMPTS: [PromptDefinition; 3] = [
    PromptDefinition {
        name: "prototype_flow_strategy",
        description: "Read-only prototype journey analysis using reactions, optional motion, and targeted node context.",
        body: include_str!("../bodies/prototype_flow_strategy.md"),
    },
    PromptDefinition {
        name: "read_design_strategy",
        description: "Token-efficient, read-only sequence for inspecting a Figma file with the 14-tool catalog.",
        body: include_str!("../bodies/read_design_strategy.md"),
    },
    PromptDefinition {
        name: "style_audit_strategy",
        description: "Bounded, report-only audit of raw values versus linked styles and variables.",
        body: include_str!("../bodies/style_audit_strategy.md"),
    },
];

pub fn prompt_definitions() -> &'static [PromptDefinition; 3] {
    &PROMPTS
}

pub fn prompt_by_name(name: &str) -> Option<&'static PromptDefinition> {
    PROMPTS.iter().find(|prompt| prompt.name == name)
}

pub fn prompts_catalog() -> ListPromptsResult {
    let prompts = PROMPTS
        .iter()
        .map(|prompt| Prompt::new(prompt.name, Some(prompt.description), None))
        .collect();
    ListPromptsResult::with_all_items(prompts)
        .with_ttl_ms(CACHE_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

pub fn get_prompt_result(name: &str) -> Option<GetPromptResult> {
    let prompt = prompt_by_name(name)?;
    Some(
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, prompt.body)])
            .with_description(prompt.description),
    )
}
