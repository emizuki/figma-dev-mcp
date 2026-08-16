use super::common::{
    protocol_input_wrapper, protocol_object_output_wrapper, protocol_output_wrapper,
    protocol_schema_wrapper,
};
use figma_dev_mcp_protocol::domain;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::{Value, json};
use std::borrow::Cow;

protocol_schema_wrapper!(ListFilesInput, domain::ListFilesInput);
protocol_output_wrapper!(ListFilesResult, domain::ListFilesResult);
protocol_input_wrapper!(GetMetadataInput, domain::GetMetadataInput);
protocol_output_wrapper!(GetMetadataResult, domain::GetMetadataResult);
protocol_input_wrapper!(GetSelectionInput, domain::GetSelectionInput);
protocol_object_output_wrapper!(GetSelectionResult, domain::GetSelectionResult);
protocol_input_wrapper!(GetNodesInput, domain::GetNodesInput);
protocol_object_output_wrapper!(GetNodesResult, domain::GetNodesResult);
protocol_output_wrapper!(SearchNodesResult, domain::SearchNodesResult);
protocol_input_wrapper!(GetDesignContextInput, domain::GetDesignContextInput);
protocol_object_output_wrapper!(GetDesignContextResult, domain::GetDesignContextResult);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SearchNodesInput(domain::SearchNodesInput);

impl SearchNodesInput {
    pub(crate) fn into_protocol(self) -> domain::SearchNodesInput {
        self.0
    }
}

impl<'de> Deserialize<'de> for SearchNodesInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut input = domain::SearchNodesInput::deserialize(deserializer)?;
        if input
            .query
            .name
            .as_ref()
            .is_some_and(|term| term.value.as_str().trim().is_empty())
        {
            return Err(D::Error::custom("name must be non-empty after trimming"));
        }
        if input
            .query
            .text
            .as_ref()
            .is_some_and(|term| term.value.as_str().trim().is_empty())
        {
            return Err(D::Error::custom("text must be non-empty after trimming"));
        }
        if !input.query.node_types.is_empty() {
            let mut types = Vec::with_capacity(input.query.node_types.len());
            for type_name in input.query.node_types.as_slice() {
                let trimmed = type_name.as_str().trim();
                if trimmed.is_empty() {
                    return Err(D::Error::custom(
                        "nodeTypes must be non-empty after trimming",
                    ));
                }
                types.push(domain::NodeTypeName::try_from(trimmed).map_err(D::Error::custom)?);
            }
            input.query.node_types =
                domain::NodeTypeList::try_from(types).map_err(D::Error::custom)?;
        }
        let has_name = input
            .query
            .name
            .as_ref()
            .is_some_and(|term| !term.value.as_str().trim().is_empty());
        let has_text = input
            .query
            .text
            .as_ref()
            .is_some_and(|term| !term.value.as_str().trim().is_empty());
        if !has_name && input.query.node_types.is_empty() && !has_text {
            return Err(D::Error::custom(
                "search query must include name, nodeTypes, or text",
            ));
        }
        Ok(Self(input))
    }
}

impl JsonSchema for SearchNodesInput {
    fn schema_name() -> Cow<'static, str> {
        "SearchNodesInput".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = domain::SearchNodesInput::json_schema(generator);
        schema
            .ensure_object()
            .insert("type".to_owned(), "object".into());
        apply_search_contract_schema(generator);
        schema
    }
}

fn apply_search_contract_schema(generator: &mut SchemaGenerator) {
    let defs = generator.definitions_mut();
    if let Some(scope) = defs.get_mut("SearchScope").and_then(Value::as_object_mut)
        && let Some(variants) = scope.remove("anyOf")
    {
        scope.insert("oneOf".to_owned(), variants);
    }
    if let Some(query) = defs.get_mut("SearchQuery").and_then(Value::as_object_mut) {
        query.insert(
            "anyOf".to_owned(),
            json!([
                {"required": ["name"]},
                {"required": ["nodeTypes"]},
                {"required": ["text"]}
            ]),
        );
    }
    if let Some(types) = defs.get_mut("NodeTypeList").and_then(Value::as_object_mut) {
        types.entry("minItems".to_owned()).or_insert(1.into());
    }
}
