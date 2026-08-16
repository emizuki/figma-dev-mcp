//! Developer-handoff contracts for styles, variables, components, and fonts.

use super::{
    ComponentPropertyValue, ConnectionId, NodeId, NodeIdList, ObservationWindow, ReturnedList,
    Selector, StyleValue, Truncation, VariableValue,
};
use crate::error::ErrorCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! scoped_input {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub connection_id: Option<ConnectionId>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub selector: Option<Selector>,
        }
    };
}

scoped_input!(GetFontsInput);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StyleSource {
    Local,
    Referenced,
    #[default]
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetStylesInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
    #[serde(default)]
    pub source: StyleSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetStylesResult {
    pub styles: ReturnedList<StyleValue>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetVariablesInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
    #[serde(default)]
    pub resolve_aliases: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableModeError {
    pub code: ErrorCode,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableModeValue {
    pub mode_id: String,
    pub source: VariableValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<VariableValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<VariableModeError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableDefinition {
    pub id: String,
    pub name: String,
    pub collection_id: String,
    pub scopes: ReturnedList<String>,
    pub values: ReturnedList<VariableModeValue>,
    pub code_syntax: ReturnedList<CodeSyntax>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableMode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableCollection {
    pub id: String,
    pub name: String,
    pub modes: ReturnedList<VariableMode>,
    pub variables: ReturnedList<VariableDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetVariablesResult {
    pub collections: ReturnedList<VariableCollection>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetComponentsInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentationReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentPropertyDefinition {
    pub name: String,
    pub default_value: ComponentPropertyValue,
    #[serde(default, skip_serializing_if = "ReturnedList::is_empty")]
    pub preferred_values: ReturnedList<ComponentPropertyValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentDefinition {
    pub id: NodeId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_set_id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub documentation: ReturnedList<DocumentationReference>,
    pub variant_properties: ReturnedList<NamedVariantProperty>,
    pub property_definitions: ReturnedList<ComponentPropertyDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedVariantProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceRelationship {
    pub instance_id: NodeId,
    pub component_id: NodeId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetComponentsResult {
    pub components: ReturnedList<ComponentDefinition>,
    pub instances: ReturnedList<InstanceRelationship>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontName {
    pub family: String,
    pub style: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum FontAvailability {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontUsage {
    pub font: FontName,
    pub availability: FontAvailability,
    pub node_ids: NodeIdList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetFontsResult {
    pub fonts: ReturnedList<FontUsage>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodeSyntax {
    pub platform: String,
    pub code: String,
}
