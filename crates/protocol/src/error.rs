//! Stable, safe errors that may cross an MCP or plugin boundary.

use crate::{
    domain::{BoundaryValueError, DisplayText, ItemIdentifier, bounded_string_schema},
    limits::{MAX_DISPLAY_TEXT_BYTES, MAX_INPUT_IDS},
};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, IgnoredAny, SeqAccess, Visitor},
    ser::SerializeStruct,
};
use std::{borrow::Cow, fmt, marker::PhantomData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NoFigmaConnection,
    AmbiguousConnection,
    ConnectionNotFound,
    ConnectionLost,
    ProtocolMismatch,
    NodeNotFound,
    /// The node is in the document but does not render, by its own `visible`
    /// or an ancestor's. Distinct from `NODE_NOT_FOUND`, which means the id
    /// resolved to nothing: here the id is correct and the caller would waste
    /// their time re-checking it. The server returns only what Figma draws, so
    /// a switched-off node is refused rather than returned with a flag.
    NodeNotVisible,
    PageNotFound,
    UnsupportedNode,
    /// The node puts no ink on the page, so there is no picture to return in
    /// any format. Distinct from `INTERNAL_ERROR`, which means the cause is
    /// unknown; here it is known. Judged on the host's own render bounds,
    /// measured after strokes and effects, so a zero-height rule or divider
    /// still renders and still succeeds.
    EmptyNodeBounds,
    CapabilityUnavailable,
    /// Reserved and no longer emitted: SVG safety reports a verdict on the
    /// asset instead of failing the item. The member stays because removing a
    /// member of a closed enum is itself a wire change.
    UnsafeSvg,
    InvalidCursor,
    LimitExceeded,
    Timeout,
    Cancelled,
    InternalError,
}

impl ErrorCode {
    /// Every member, in wire order.
    ///
    /// The enum has no iterator, so any test that wants to sweep the whole set
    /// has to write the members out. Written out once here and shared, a member
    /// added to the enum reaches every sweep at the same time — where a copy
    /// kept beside each test would leave the new member silently unswept while
    /// the test still passed over a shorter list. `tests/contracts` pins this
    /// against the set derived from the enum's own schema, so a member missing
    /// from it fails there rather than quietly narrowing what the sweeps cover.
    pub const ALL: [ErrorCode; 17] = [
        ErrorCode::NoFigmaConnection,
        ErrorCode::AmbiguousConnection,
        ErrorCode::ConnectionNotFound,
        ErrorCode::ConnectionLost,
        ErrorCode::ProtocolMismatch,
        ErrorCode::NodeNotFound,
        ErrorCode::NodeNotVisible,
        ErrorCode::PageNotFound,
        ErrorCode::UnsupportedNode,
        ErrorCode::EmptyNodeBounds,
        ErrorCode::CapabilityUnavailable,
        ErrorCode::UnsafeSvg,
        ErrorCode::InvalidCursor,
        ErrorCode::LimitExceeded,
        ErrorCode::Timeout,
        ErrorCode::Cancelled,
        ErrorCode::InternalError,
    ];
}

pub const fn canonical_message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::NoFigmaConnection => "No Figma connection is available.",
        ErrorCode::AmbiguousConnection => "More than one Figma connection matches the request.",
        ErrorCode::ConnectionNotFound => "The requested Figma connection was not found.",
        ErrorCode::ConnectionLost => "The Figma connection was lost.",
        ErrorCode::ProtocolMismatch => "The plugin protocol version is not supported.",
        ErrorCode::NodeNotFound => "The requested node was not found.",
        ErrorCode::NodeNotVisible => {
            "The requested node exists but is switched off, so it renders nothing."
        }
        ErrorCode::PageNotFound => "The requested page was not found.",
        ErrorCode::UnsupportedNode => "The requested node type is not supported.",
        ErrorCode::EmptyNodeBounds => "The requested node renders nothing.",
        ErrorCode::CapabilityUnavailable => "The required Figma capability is unavailable.",
        ErrorCode::UnsafeSvg => "The SVG was rejected by the safety policy.",
        ErrorCode::InvalidCursor => "The search cursor is invalid or stale.",
        ErrorCode::LimitExceeded => "The operation exceeded a safety limit.",
        ErrorCode::Timeout => "The operation timed out.",
        ErrorCode::Cancelled => "The operation was cancelled.",
        ErrorCode::InternalError => "The operation failed.",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemError {
    index: usize,
    id: Option<ItemIdentifier>,
    code: ErrorCode,
    retryable: bool,
}

impl ItemError {
    pub fn new(index: usize, id: Option<ItemIdentifier>, code: ErrorCode, retryable: bool) -> Self {
        Self {
            index,
            id,
            code,
            retryable,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> Option<&ItemIdentifier> {
        self.id.as_ref()
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &'static str {
        canonical_message(self.code)
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

impl Serialize for ItemError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ItemError", 5)?;
        state.serialize_field("index", &self.index)?;
        if let Some(id) = &self.id {
            state.serialize_field("id", id)?;
        }
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", self.message())?;
        state.serialize_field("retryable", &self.retryable)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for ItemError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RawItemError {
            index: usize,
            #[serde(default)]
            id: Option<ItemIdentifier>,
            code: ErrorCode,
            message: DisplayText,
            retryable: bool,
        }

        let raw = RawItemError::deserialize(deserializer)?;
        if raw.message.as_str() != canonical_message(raw.code) {
            return Err(D::Error::custom(
                "error message does not match its stable code",
            ));
        }
        Ok(Self::new(raw.index, raw.id, raw.code, raw.retryable))
    }
}

impl JsonSchema for ItemError {
    fn schema_name() -> Cow<'static, str> {
        "ItemError".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        ItemErrorSchema::json_schema(generator)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    code: ErrorCode,
    retryable: bool,
    items: Option<Vec<ItemError>>,
}

impl ToolError {
    pub fn new(code: ErrorCode, retryable: bool) -> Self {
        Self {
            code,
            retryable,
            items: None,
        }
    }

    pub fn with_items(mut self, items: Vec<ItemError>) -> Result<Self, BoundaryValueError> {
        validate_item_count(items.len())?;
        self.items = Some(items);
        Ok(self)
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &'static str {
        canonical_message(self.code)
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }

    pub fn items(&self) -> Option<&[ItemError]> {
        self.items.as_deref()
    }
}

impl Serialize for ToolError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ToolError", 4)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", self.message())?;
        state.serialize_field("retryable", &self.retryable)?;
        if let Some(items) = &self.items {
            state.serialize_field("items", items)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for ToolError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RawToolError {
            code: ErrorCode,
            message: DisplayText,
            retryable: bool,
            #[serde(default, deserialize_with = "deserialize_optional_error_items")]
            items: Option<Vec<ItemError>>,
        }

        let raw = RawToolError::deserialize(deserializer)?;
        if raw.message.as_str() != canonical_message(raw.code) {
            return Err(D::Error::custom(
                "error message does not match its stable code",
            ));
        }
        Ok(Self {
            code: raw.code,
            retryable: raw.retryable,
            items: raw.items,
        })
    }
}

impl JsonSchema for ToolError {
    fn schema_name() -> Cow<'static, str> {
        "ToolError".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        ToolErrorSchema::json_schema(generator)
    }
}

/// Message-free, plugin-originated failure facts.
///
/// The broker converts this type into [`ToolError`] and supplies the canonical
/// public message for the stable code. Plugin diagnostics never cross this
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginItemFailure {
    index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<ItemIdentifier>,
    code: ErrorCode,
    retryable: bool,
}

impl PluginItemFailure {
    pub fn new(index: usize, id: Option<ItemIdentifier>, code: ErrorCode, retryable: bool) -> Self {
        Self {
            index,
            id,
            code,
            retryable,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> Option<&ItemIdentifier> {
        self.id.as_ref()
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginFailure {
    code: ErrorCode,
    retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    items: Option<Vec<PluginItemFailure>>,
}

impl PluginFailure {
    pub fn new(code: ErrorCode, retryable: bool) -> Self {
        Self {
            code,
            retryable,
            items: None,
        }
    }

    pub fn with_items(mut self, items: Vec<PluginItemFailure>) -> Result<Self, BoundaryValueError> {
        validate_item_count(items.len())?;
        self.items = Some(items);
        Ok(self)
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn retryable(&self) -> bool {
        self.retryable
    }

    pub fn items(&self) -> Option<&[PluginItemFailure]> {
        self.items.as_deref()
    }
}

impl<'de> Deserialize<'de> for PluginFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RawPluginFailure {
            code: ErrorCode,
            retryable: bool,
            #[serde(default, deserialize_with = "deserialize_optional_plugin_items")]
            items: Option<Vec<PluginItemFailure>>,
        }

        let raw = RawPluginFailure::deserialize(deserializer)?;
        Ok(Self {
            code: raw.code,
            retryable: raw.retryable,
            items: raw.items,
        })
    }
}

impl From<PluginFailure> for ToolError {
    fn from(failure: PluginFailure) -> Self {
        let items = failure.items.map(|items| {
            items
                .into_iter()
                .map(|item| ItemError::new(item.index, item.id, item.code, item.retryable))
                .collect()
        });
        Self {
            code: failure.code,
            retryable: failure.retryable,
            items,
        }
    }
}

fn validate_item_count(count: usize) -> Result<(), BoundaryValueError> {
    if count > MAX_INPUT_IDS {
        return Err(BoundaryValueError::TooMany {
            label: "error items",
            actual: count,
            maximum: MAX_INPUT_IDS,
        });
    }
    Ok(())
}

struct ErrorItemListVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for ErrorItemListVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {MAX_INPUT_IDS} error items")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(0).min(MAX_INPUT_IDS);
        let mut items = Vec::with_capacity(capacity);
        while items.len() < MAX_INPUT_IDS {
            match sequence.next_element()? {
                Some(item) => items.push(item),
                None => return Ok(items),
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format_args!(
                "error item count exceeds {MAX_INPUT_IDS}"
            )));
        }
        Ok(items)
    }
}

fn deserialize_optional_error_items<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<ItemError>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_bounded_items(deserializer)
}

fn deserialize_optional_plugin_items<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<PluginItemFailure>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_bounded_items(deserializer)
}

fn deserialize_optional_bounded_items<'de, D, T>(
    deserializer: D,
) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OptionalVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for OptionalVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Option<Vec<T>>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a null or bounded error item list")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
        where
            D2: Deserializer<'de>,
        {
            deserializer
                .deserialize_seq(ErrorItemListVisitor(PhantomData))
                .map(Some)
        }
    }

    deserializer.deserialize_option(OptionalVisitor(PhantomData))
}

#[derive(JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code)]
struct ItemErrorSchema {
    index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<ItemIdentifier>,
    code: ErrorCode,
    message: CanonicalMessageSchema,
    retryable: bool,
}

#[derive(JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(dead_code)]
struct ToolErrorSchema {
    code: ErrorCode,
    message: CanonicalMessageSchema,
    retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    items: Option<Vec<ItemError>>,
}

struct CanonicalMessageSchema;

impl JsonSchema for CanonicalMessageSchema {
    fn schema_name() -> Cow<'static, str> {
        "CanonicalErrorMessage".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = bounded_string_schema(
            generator,
            MAX_DISPLAY_TEXT_BYTES,
            "code-owned canonical error message",
            false,
        );
        if let Some(object) = schema.as_object_mut() {
            object.insert("readOnly".to_owned(), true.into());
        }
        schema
    }
}

#[cfg(test)]
mod error_code_all_tests {
    use super::ErrorCode;

    /// `ALL`'s completeness is pinned in `tests/contracts` against the set the
    /// enum's schema declares, but that check lives in another crate and reads
    /// the plugin source tree off disk. This one needs neither, so `cargo test
    /// -p figma-dev-mcp-protocol` on its own still catches the mistake the
    /// length check cannot see: a member written twice while another is
    /// missing, which keeps the array 17 long and compiles.
    #[test]
    fn all_lists_each_member_once() {
        let mut tags: Vec<String> = ErrorCode::ALL
            .iter()
            .map(|code| serde_json::to_string(code).expect("ErrorCode serializes"))
            .collect();
        let listed = tags.len();
        tags.sort();
        tags.dedup();
        assert_eq!(
            tags.len(),
            listed,
            "ErrorCode::ALL repeats a member, so some other member is missing from every sweep"
        );
    }
}
