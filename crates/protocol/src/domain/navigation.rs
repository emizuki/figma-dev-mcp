//! Connection, navigation, search, and design-context contracts.

use super::{
    CapabilitySet, CompactNodeData, ConnectionId, DetailLevel, DisplayText, FileKey, FullNodeData,
    MinimalNodeDetails, NodeBatch, NodeForest, NodeId, NodeIdList, NodeSummary, NodeTypeList,
    ObservationWindow, PageId, QueryText, ReturnedList, Selector, Truncation,
    deserialize_optional_depth,
};
use crate::deferred::decode_raw;
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListFilesInput {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LiveFile {
    pub connection_id: ConnectionId,
    pub display_name: DisplayText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_key: Option<FileKey>,
    pub file_name: DisplayText,
    pub current_page: PageSummary,
    pub editor_type: DisplayText,
    pub capabilities: CapabilitySet,
    pub connected_at: DisplayText,
    pub last_seen_at: DisplayText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListFilesResult {
    pub files: ReturnedList<LiveFile>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetMetadataInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<FileKey>,
    pub name: DisplayText,
    pub editor_type: DisplayText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageSummary {
    pub id: PageId,
    pub name: DisplayText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetMetadataResult {
    pub file: FileMetadata,
    pub pages: ReturnedList<PageSummary>,
    pub current_page_id: PageId,
    pub plugin_version: DisplayText,
    pub capabilities: CapabilitySet,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetSelectionInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailLevel>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_depth",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(range(max = 6))]
    pub depth: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "detail",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GetSelectionResult {
    Minimal {
        nodes: NodeForest<MinimalNodeDetails>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Compact {
        nodes: NodeForest<CompactNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Full {
        nodes: NodeForest<FullNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetNodesInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    pub node_ids: NodeIdList,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailLevel>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_depth",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(range(max = 6))]
    pub depth: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "detail",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GetNodesResult {
    Minimal {
        items: NodeBatch<MinimalNodeDetails>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Compact {
        items: NodeBatch<CompactNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Full {
        items: NodeBatch<FullNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum SearchScope {
    Page(SearchPageScope),
    Node(SearchNodeScope),
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum SearchScopeField {
    PageId,
    NodeId,
}

impl<'de> Deserialize<'de> for SearchScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(SearchScopeVisitor)
    }
}

struct SearchScopeVisitor;

impl<'de> Visitor<'de> for SearchScopeVisitor {
    type Value = SearchScope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exactly one page or node search scope")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(field) = map.next_key::<SearchScopeField>()? else {
            return Err(A::Error::custom("search scope must contain one field"));
        };
        let scope = match field {
            SearchScopeField::PageId => SearchScope::Page(SearchPageScope {
                page_id: map.next_value()?,
            }),
            SearchScopeField::NodeId => SearchScope::Node(SearchNodeScope {
                node_id: map.next_value()?,
            }),
        };
        if map.next_key::<SearchScopeField>()?.is_some() {
            return Err(A::Error::custom(
                "search scope must contain exactly one field",
            ));
        }
        Ok(scope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchPageScope {
    pub page_id: PageId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchNodeScope {
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SearchMatchMode {
    Exact,
    Contains,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchTerm {
    pub value: QueryText,
    pub mode: SearchMatchMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_sensitive: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<SearchTerm>,
    #[serde(default, skip_serializing_if = "NodeTypeList::is_empty")]
    pub node_types: NodeTypeList,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<SearchTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchNodesInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    pub scope: SearchScope,
    pub query: SearchQuery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeMatch {
    pub node: NodeSummary,
    pub reasons: ReturnedList<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchNodesResult {
    pub matches: ReturnedList<NodeMatch>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetDesignContextInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_id: Option<ConnectionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<Selector>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_depth",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(range(max = 6))]
    pub depth: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailLevel>,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub dedupe_components: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "detail",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GetDesignContextResult {
    Minimal {
        roots: NodeForest<MinimalNodeDetails>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Compact {
        roots: NodeForest<CompactNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
    Full {
        roots: NodeForest<FullNodeData>,
        truncated: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        truncation: Option<Truncation>,
        observation: ObservationWindow,
    },
}

macro_rules! impl_detail_result_deserialize {
    ($result:ident, $visitor:ident, $field:ident, $payload_variant:ident, $payload_field:ident, $payload_json:literal) => {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "camelCase")]
        enum $field {
            Detail,
            $payload_variant,
            Truncated,
            Truncation,
            Observation,
        }

        impl<'de> Deserialize<'de> for $result {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_map($visitor)
            }
        }

        struct $visitor;

        impl<'de> Visitor<'de> for $visitor {
            type Value = $result;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a closed detail-discriminated result")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut detail = None;
                let mut payload = None;
                let mut truncated = None;
                let mut truncation = None;
                let mut observation = None;
                while let Some(field) = map.next_key::<$field>()? {
                    match field {
                        $field::Detail => {
                            if detail.is_some() {
                                return Err(A::Error::duplicate_field("detail"));
                            }
                            detail = Some(map.next_value::<DetailLevel>()?);
                        }
                        $field::$payload_variant => {
                            if payload.is_some() {
                                return Err(A::Error::duplicate_field($payload_json));
                            }
                            payload = Some(map.next_value::<Box<RawValue>>()?);
                        }
                        $field::Truncated => {
                            if truncated.is_some() {
                                return Err(A::Error::duplicate_field("truncated"));
                            }
                            truncated = Some(map.next_value::<bool>()?);
                        }
                        $field::Truncation => {
                            if truncation.is_some() {
                                return Err(A::Error::duplicate_field("truncation"));
                            }
                            truncation = Some(map.next_value::<Option<Truncation>>()?);
                        }
                        $field::Observation => {
                            if observation.is_some() {
                                return Err(A::Error::duplicate_field("observation"));
                            }
                            observation = Some(map.next_value::<ObservationWindow>()?);
                        }
                    }
                }
                let payload = payload.ok_or_else(|| A::Error::missing_field($payload_json))?;
                let truncated = truncated.ok_or_else(|| A::Error::missing_field("truncated"))?;
                let truncation = truncation.unwrap_or(None);
                let observation =
                    observation.ok_or_else(|| A::Error::missing_field("observation"))?;
                match detail.ok_or_else(|| A::Error::missing_field("detail"))? {
                    DetailLevel::Minimal => Ok($result::Minimal {
                        $payload_field: decode_raw(&payload)?,
                        truncated,
                        truncation,
                        observation,
                    }),
                    DetailLevel::Compact => Ok($result::Compact {
                        $payload_field: decode_raw(&payload)?,
                        truncated,
                        truncation,
                        observation,
                    }),
                    DetailLevel::Full => Ok($result::Full {
                        $payload_field: decode_raw(&payload)?,
                        truncated,
                        truncation,
                        observation,
                    }),
                }
            }
        }
    };
}

impl_detail_result_deserialize!(
    GetSelectionResult,
    GetSelectionResultVisitor,
    GetSelectionResultField,
    Nodes,
    nodes,
    "nodes"
);
impl_detail_result_deserialize!(
    GetNodesResult,
    GetNodesResultVisitor,
    GetNodesResultField,
    Items,
    items,
    "items"
);
impl_detail_result_deserialize!(
    GetDesignContextResult,
    GetDesignContextResultVisitor,
    GetDesignContextResultField,
    Roots,
    roots,
    "roots"
);
