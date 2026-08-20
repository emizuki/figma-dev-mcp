//! Stable, safe errors that may cross an MCP or plugin boundary.

use crate::{
    domain::{
        BoundaryValueError, DisplayText, ItemIdentifier, SvgRejectionName, bounded_string_schema,
    },
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
    PageNotFound,
    UnsupportedNode,
    CapabilityUnavailable,
    UnsafeSvg,
    InvalidCursor,
    LimitExceeded,
    Timeout,
    Cancelled,
    InternalError,
}

pub const fn canonical_message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::NoFigmaConnection => "No Figma connection is available.",
        ErrorCode::AmbiguousConnection => "More than one Figma connection matches the request.",
        ErrorCode::ConnectionNotFound => "The requested Figma connection was not found.",
        ErrorCode::ConnectionLost => "The Figma connection was lost.",
        ErrorCode::ProtocolMismatch => "The plugin protocol version is not supported.",
        ErrorCode::NodeNotFound => "The requested node was not found.",
        ErrorCode::PageNotFound => "The requested page was not found.",
        ErrorCode::UnsupportedNode => "The requested node type is not supported.",
        ErrorCode::CapabilityUnavailable => "The required Figma capability is unavailable.",
        ErrorCode::UnsafeSvg => "The SVG was rejected by the safety policy.",
        ErrorCode::InvalidCursor => "The search cursor is invalid or stale.",
        ErrorCode::LimitExceeded => "The operation exceeded a safety limit.",
        ErrorCode::Timeout => "The operation timed out.",
        ErrorCode::Cancelled => "The operation was cancelled.",
        ErrorCode::InternalError => "The operation failed.",
    }
}

// `UNSAFE_SVG` alone cannot be diagnosed from outside the plugin, so the rule
// that fired travels with the error.
//
// Modelled as a closed discriminant plus an optional name rather than as a
// tagged enum. An adjacently tagged enum would have forced a required `content`
// object on every name-carrying variant, but the plugin drops a name it cannot
// bound, so the name is optional for every rule. A struct closes under
// `deny_unknown_fields` exactly as adjacent tagging does, needs no hand-written
// `Visitor`, and keeps one shape on both ends of the wire.
/// Which safety rule rejected an SVG export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SvgRejectionKind {
    ParserError,
    UnsafeElement,
    UnsafeAttribute,
    UnsafeCss,
    UnsafeProcessingInstruction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SvgRejection {
    kind: SvgRejectionKind,
    // Never an attribute value: values carry design content.
    /// Local name of the offending element or attribute, where the rule has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<SvgRejectionName>,
}

impl SvgRejection {
    pub fn new(kind: SvgRejectionKind, name: Option<SvgRejectionName>) -> Self {
        Self { kind, name }
    }

    pub fn kind(&self) -> SvgRejectionKind {
        self.kind
    }

    pub fn name(&self) -> Option<&SvgRejectionName> {
        self.name.as_ref()
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
    svg_rejection: Option<SvgRejection>,
}

impl ToolError {
    pub fn new(code: ErrorCode, retryable: bool) -> Self {
        Self {
            code,
            retryable,
            items: None,
            svg_rejection: None,
        }
    }

    pub fn with_items(mut self, items: Vec<ItemError>) -> Result<Self, BoundaryValueError> {
        validate_item_count(items.len())?;
        self.items = Some(items);
        Ok(self)
    }

    pub fn with_svg_rejection(mut self, rejection: SvgRejection) -> Self {
        self.svg_rejection = Some(rejection);
        self
    }

    pub fn svg_rejection(&self) -> Option<&SvgRejection> {
        self.svg_rejection.as_ref()
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
        let mut state = serializer.serialize_struct("ToolError", 5)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", self.message())?;
        state.serialize_field("retryable", &self.retryable)?;
        if let Some(items) = &self.items {
            state.serialize_field("items", items)?;
        }
        if let Some(rejection) = &self.svg_rejection {
            state.serialize_field("svgRejection", rejection)?;
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
            #[serde(default)]
            svg_rejection: Option<SvgRejection>,
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
            svg_rejection: raw.svg_rejection,
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
            // A plugin-level failure envelope reports no per-asset SVG rule; the
            // rule only exists on the item results inside a screenshot payload.
            svg_rejection: None,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    svg_rejection: Option<SvgRejection>,
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
