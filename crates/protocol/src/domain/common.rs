//! Shared typed values used by more than one read operation.

use crate::{
    deferred::decode_raw,
    error::ToolError,
    limits::{
        MAX_DEPTH, MAX_DISPLAY_TEXT_BYTES, MAX_IDENTIFIER_BYTES, MAX_INPUT_IDS, MAX_PAGE_IDS,
        MAX_QUERY_BYTES, MAX_RASTER_BASE64_BYTES, MAX_RETURNED_NODES, MAX_SEARCH_CURSOR_BYTES,
        MAX_SVG_BYTES,
    },
};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{borrow::Cow, fmt, marker::PhantomData};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoundaryValueError {
    #[error("{label} must not be empty")]
    Empty { label: &'static str },
    #[error("{label} length {actual} bytes exceeds {maximum}")]
    TooLong {
        label: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("{label} has {actual} items, exceeding {maximum}")]
    TooMany {
        label: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("node tree depth {actual} exceeds {maximum}")]
    TooDeep { actual: u8, maximum: u8 },
    #[error("{label} must be an RFC 4122 UUID")]
    MalformedUuid { label: &'static str },
}

fn validate_boundary_string(
    value: &str,
    maximum_bytes: usize,
    label: &'static str,
    allow_empty: bool,
) -> Result<(), BoundaryValueError> {
    if !allow_empty && value.is_empty() {
        return Err(BoundaryValueError::Empty { label });
    }
    if value.len() > maximum_bytes {
        return Err(BoundaryValueError::TooLong {
            label,
            actual: value.len(),
            maximum: maximum_bytes,
        });
    }
    Ok(())
}

pub(crate) fn bounded_string_schema(
    generator: &mut SchemaGenerator,
    maximum_bytes: usize,
    label: &'static str,
    allow_empty: bool,
) -> Schema {
    let mut schema = String::json_schema(generator);
    if let Some(object) = schema.as_object_mut() {
        object.insert("maxLength".to_owned(), maximum_bytes.into());
        object.insert("x-maxUtf8Bytes".to_owned(), maximum_bytes.into());
        if !allow_empty {
            object.insert("minLength".to_owned(), 1.into());
        }
        object.insert(
            "description".to_owned(),
            format!("{label}; bounded by UTF-8 encoded byte length").into(),
        );
    }
    schema
}

macro_rules! bounded_string_newtype {
    ($name:ident, $maximum:expr, $label:literal, $allow_empty:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl TryFrom<String> for $name {
            type Error = BoundaryValueError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                validate_boundary_string(&value, $maximum, $label, $allow_empty)?;
                Ok(Self(value))
            }
        }

        impl TryFrom<&str> for $name {
            type Error = BoundaryValueError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                validate_boundary_string(value, $maximum, $label, $allow_empty)?;
                Ok(Self(value.to_owned()))
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct BoundedStringVisitor;

                impl<'de> Visitor<'de> for BoundedStringVisitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        write!(
                            formatter,
                            "{} with at most {} UTF-8 bytes",
                            $label, $maximum
                        )
                    }

                    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::try_from(value).map_err(E::custom)
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::try_from(value).map_err(E::custom)
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        $name::try_from(value).map_err(E::custom)
                    }
                }

                deserializer.deserialize_str(BoundedStringVisitor)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                bounded_string_schema(generator, $maximum, $label, $allow_empty)
            }
        }
    };
}

bounded_string_newtype!(
    RequestId,
    MAX_IDENTIFIER_BYTES,
    "plugin request identifier",
    false
);
bounded_string_newtype!(NodeId, MAX_IDENTIFIER_BYTES, "node identifier", false);
bounded_string_newtype!(PageId, MAX_IDENTIFIER_BYTES, "page identifier", false);
bounded_string_newtype!(NodeTypeName, MAX_IDENTIFIER_BYTES, "node type name", false);
bounded_string_newtype!(FileKey, MAX_IDENTIFIER_BYTES, "file key", false);
bounded_string_newtype!(
    ProtocolVersion,
    MAX_IDENTIFIER_BYTES,
    "protocol version",
    false
);
bounded_string_newtype!(
    ItemIdentifier,
    MAX_IDENTIFIER_BYTES,
    "item identifier",
    false
);
bounded_string_newtype!(QueryText, MAX_QUERY_BYTES, "query text", true);
bounded_string_newtype!(
    SearchCursor,
    MAX_SEARCH_CURSOR_BYTES,
    "opaque search cursor",
    false
);
bounded_string_newtype!(DisplayText, MAX_DISPLAY_TEXT_BYTES, "display text", true);
bounded_string_newtype!(
    RasterBase64,
    MAX_RASTER_BASE64_BYTES,
    "raster base64 payload",
    true
);
bounded_string_newtype!(SvgSource, MAX_SVG_BYTES, "SVG source", true);
bounded_string_newtype!(
    SvgRejectionName,
    MAX_IDENTIFIER_BYTES,
    "SVG element or attribute local name",
    false
);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ConnectionId(String);

impl ConnectionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ConnectionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn is_canonical_rfc4122_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || !matches!(bytes.get(8), Some(b'-'))
        || !matches!(bytes.get(13), Some(b'-'))
        || !matches!(bytes.get(18), Some(b'-'))
        || !matches!(bytes.get(23), Some(b'-'))
    {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    matches!(bytes[14], b'1'..=b'8') && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
}

impl TryFrom<String> for ConnectionId {
    type Error = BoundaryValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_boundary_string(&value, MAX_IDENTIFIER_BYTES, "connection identifier", false)?;
        if !is_canonical_rfc4122_uuid(&value) {
            return Err(BoundaryValueError::MalformedUuid {
                label: "connection identifier",
            });
        }
        let uuid = Uuid::parse_str(&value).map_err(|_| BoundaryValueError::MalformedUuid {
            label: "connection identifier",
        })?;
        Ok(Self(uuid.hyphenated().to_string()))
    }
}

impl TryFrom<&str> for ConnectionId {
    type Error = BoundaryValueError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl<'de> Deserialize<'de> for ConnectionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ConnectionIdVisitor;

        impl<'de> Visitor<'de> for ConnectionIdVisitor {
            type Value = ConnectionId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an RFC 4122 UUID connection identifier")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ConnectionId::try_from(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ConnectionId::try_from(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ConnectionId::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ConnectionIdVisitor)
    }
}

impl JsonSchema for ConnectionId {
    fn schema_name() -> Cow<'static, str> {
        "ConnectionId".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema =
            bounded_string_schema(generator, 36, "RFC 4122 UUID connection identifier", false);
        if let Some(object) = schema.as_object_mut() {
            object.insert("format".to_owned(), "uuid".into());
            object.insert(
                "pattern".to_owned(),
                "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89aAbB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$".into(),
            );
        }
        schema
    }
}

fn bounded_list_schema<T: JsonSchema>(
    generator: &mut SchemaGenerator,
    maximum_items: usize,
    label: &'static str,
) -> Schema {
    let mut schema = Vec::<T>::json_schema(generator);
    if let Some(object) = schema.as_object_mut() {
        object.insert("maxItems".to_owned(), maximum_items.into());
        object.insert(
            "description".to_owned(),
            format!("{label}; decoder stops at item {}", maximum_items + 1).into(),
        );
    }
    schema
}

struct BoundedListVisitor<T> {
    maximum_items: usize,
    label: &'static str,
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for BoundedListVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} containing at most {} items",
            self.label, self.maximum_items
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(0).min(self.maximum_items);
        let mut values = Vec::with_capacity(capacity);
        while values.len() < self.maximum_items {
            match sequence.next_element()? {
                Some(value) => values.push(value),
                None => return Ok(values),
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format_args!(
                "{} count exceeds {}",
                self.label, self.maximum_items
            )));
        }
        Ok(values)
    }
}

fn deserialize_bounded_list<'de, D, T>(
    deserializer: D,
    maximum_items: usize,
    label: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(BoundedListVisitor {
        maximum_items,
        label,
        marker: PhantomData,
    })
}

macro_rules! bounded_list_newtype {
    ($name:ident, $item:ty, $maximum:expr, $label:literal) => {
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Vec<$item>);

        impl $name {
            pub fn as_slice(&self) -> &[$item] {
                &self.0
            }

            pub fn len(&self) -> usize {
                self.0.len()
            }

            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl TryFrom<Vec<$item>> for $name {
            type Error = BoundaryValueError;

            fn try_from(values: Vec<$item>) -> Result<Self, Self::Error> {
                if values.len() > $maximum {
                    return Err(BoundaryValueError::TooMany {
                        label: $label,
                        actual: values.len(),
                        maximum: $maximum,
                    });
                }
                Ok(Self(values))
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserialize_bounded_list(deserializer, $maximum, $label).map(Self)
            }
        }

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                bounded_list_schema::<$item>(generator, $maximum, $label)
            }
        }
    };
}

bounded_list_newtype!(NodeIdList, NodeId, MAX_INPUT_IDS, "node identifiers");
bounded_list_newtype!(PageIdList, PageId, MAX_PAGE_IDS, "page identifiers");
bounded_list_newtype!(NodeTypeList, NodeTypeName, MAX_INPUT_IDS, "node type names");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ReturnedList<T>(Vec<T>);

impl<T> Default for ReturnedList<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> ReturnedList<T> {
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> TryFrom<Vec<T>> for ReturnedList<T> {
    type Error = BoundaryValueError;

    fn try_from(values: Vec<T>) -> Result<Self, Self::Error> {
        if values.len() > MAX_RETURNED_NODES {
            return Err(BoundaryValueError::TooMany {
                label: "returned result collection",
                actual: values.len(),
                maximum: MAX_RETURNED_NODES,
            });
        }
        Ok(Self(values))
    }
}

impl<'de, T> Deserialize<'de> for ReturnedList<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_bounded_list(
            deserializer,
            MAX_RETURNED_NODES,
            "returned result collection",
        )
        .map(Self)
    }
}

impl<T> JsonSchema for ReturnedList<T>
where
    T: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("ReturnedList_of_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        bounded_list_schema::<T>(generator, MAX_RETURNED_NODES, "returned result collection")
    }
}

fn set_field_once<E, T>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: serde::de::Error,
{
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

pub(crate) fn deserialize_optional_depth<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<u8>::deserialize(deserializer)?;
    if value.is_some_and(|depth| depth > MAX_DEPTH) {
        return Err(D::Error::custom(format_args!("depth exceeds {MAX_DEPTH}")));
    }
    Ok(value)
}

pub(crate) fn deserialize_search_limit<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u16::deserialize(deserializer)?;
    if value == 0 || usize::from(value) > MAX_RETURNED_NODES {
        return Err(D::Error::custom(format_args!(
            "limit must be between 1 and {MAX_RETURNED_NODES}"
        )));
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionFlag;

impl Serialize for SelectionFlag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for SelectionFlag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Ok(Self)
        } else {
            Err(D::Error::custom("selection must be the literal true"))
        }
    }
}

impl JsonSchema for SelectionFlag {
    fn schema_name() -> Cow<'static, str> {
        "SelectionFlag".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = bool::json_schema(generator);
        if let Some(object) = schema.as_object_mut() {
            object.insert("const".to_owned(), true.into());
        }
        schema
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DetailLevel {
    Minimal,
    Compact,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TruncationReason {
    DepthLimit,
    NodeLimit,
    ByteLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Truncation {
    pub reason: TruncationReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_depth: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visited_nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoded_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationWindow {
    pub started_at: DisplayText,
    pub completed_at: DisplayText,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct CapabilitySet {
    pub annotations: bool,
    pub dev_resources: bool,
    pub motion: bool,
    pub svg_string_export: bool,
    pub variable_code_syntax: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Selector {
    Selection(SelectionSelector),
    Page(PageSelector),
    Pages(PagesSelector),
    Node(NodeSelector),
    Nodes(NodesSelector),
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum SelectorField {
    Selection,
    PageId,
    PageIds,
    NodeId,
    NodeIds,
}

impl<'de> Deserialize<'de> for Selector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(SelectorVisitor)
    }
}

struct SelectorVisitor;

impl<'de> Visitor<'de> for SelectorVisitor {
    type Value = Selector;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exactly one closed selector field")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut selector = None;
        while let Some(field) = map.next_key::<SelectorField>()? {
            if selector.is_some() {
                return Err(A::Error::custom("selector must contain exactly one field"));
            }
            selector = Some(match field {
                SelectorField::Selection => Selector::Selection(SelectionSelector {
                    selection: map.next_value()?,
                }),
                SelectorField::PageId => Selector::Page(PageSelector {
                    page_id: map.next_value()?,
                }),
                SelectorField::PageIds => Selector::Pages(PagesSelector {
                    page_ids: map.next_value()?,
                }),
                SelectorField::NodeId => Selector::Node(NodeSelector {
                    node_id: map.next_value()?,
                }),
                SelectorField::NodeIds => Selector::Nodes(NodesSelector {
                    node_ids: map.next_value()?,
                }),
            });
        }
        selector.ok_or_else(|| A::Error::custom("selector must contain exactly one field"))
    }
}

impl JsonSchema for Selector {
    fn schema_name() -> Cow<'static, str> {
        "Selector".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        #[allow(dead_code)]
        #[derive(JsonSchema)]
        #[serde(untagged)]
        enum SelectorSchema {
            Selection(SelectionSelector),
            Page(PageSelector),
            Pages(PagesSelector),
            Node(NodeSelector),
            Nodes(NodesSelector),
        }

        let mut schema = SelectorSchema::json_schema(generator);
        if let Some(object) = schema.as_object_mut()
            && let Some(variants) = object.remove("anyOf")
        {
            object.insert("oneOf".to_owned(), variants);
        }
        schema
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionSelector {
    pub selection: SelectionFlag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageSelector {
    pub page_id: PageId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PagesSelector {
    pub page_ids: PageIdList,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeSelector {
    pub node_id: NodeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodesSelector {
    pub node_ids: NodeIdList,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradientStop {
    pub position: f64,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ImageScaleMode {
    Fill,
    Fit,
    Crop,
    Tile,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PaintValue {
    Solid {
        color: Color,
        opacity: f64,
    },
    LinearGradient {
        stops: ReturnedList<GradientStop>,
        gradient_transform: Transform2D,
        opacity: f64,
    },
    RadialGradient {
        stops: ReturnedList<GradientStop>,
        gradient_transform: Transform2D,
        opacity: f64,
    },
    AngularGradient {
        stops: ReturnedList<GradientStop>,
        gradient_transform: Transform2D,
        opacity: f64,
    },
    DiamondGradient {
        stops: ReturnedList<GradientStop>,
        gradient_transform: Transform2D,
        opacity: f64,
    },
    Image {
        image_ref: String,
        scale_mode: ImageScaleMode,
        opacity: f64,
    },
    Mixed,
    Unsupported {
        figma_type: String,
    },
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum PaintTag {
    Solid,
    LinearGradient,
    RadialGradient,
    AngularGradient,
    DiamondGradient,
    Image,
    Mixed,
    Unsupported,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum PaintField {
    Type,
    Color,
    Opacity,
    Stops,
    GradientTransform,
    ImageRef,
    ScaleMode,
    FigmaType,
}

impl<'de> Deserialize<'de> for PaintValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PaintVisitor)
    }
}

struct PaintVisitor;

impl<'de> Visitor<'de> for PaintVisitor {
    type Value = PaintValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed paint value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut color = None;
        let mut opacity = None;
        let mut stops = None;
        let mut gradient_transform = None;
        let mut image_ref = None;
        let mut scale_mode = None;
        let mut figma_type = None;
        while let Some(field) = map.next_key::<PaintField>()? {
            match field {
                PaintField::Type => {
                    set_field_once(&mut tag, map.next_value::<PaintTag>()?, "type")?
                }
                PaintField::Color => {
                    set_field_once(&mut color, map.next_value::<Color>()?, "color")?
                }
                PaintField::Opacity => {
                    set_field_once(&mut opacity, map.next_value::<f64>()?, "opacity")?
                }
                PaintField::Stops => set_field_once(
                    &mut stops,
                    map.next_value::<ReturnedList<GradientStop>>()?,
                    "stops",
                )?,
                PaintField::GradientTransform => set_field_once(
                    &mut gradient_transform,
                    map.next_value::<Transform2D>()?,
                    "gradientTransform",
                )?,
                PaintField::ImageRef => {
                    set_field_once(&mut image_ref, map.next_value::<String>()?, "imageRef")?
                }
                PaintField::ScaleMode => set_field_once(
                    &mut scale_mode,
                    map.next_value::<ImageScaleMode>()?,
                    "scaleMode",
                )?,
                PaintField::FigmaType => {
                    set_field_once(&mut figma_type, map.next_value::<String>()?, "figmaType")?
                }
            }
        }
        let tag = tag.ok_or_else(|| A::Error::missing_field("type"))?;
        match tag {
            PaintTag::Solid => {
                if stops.is_some()
                    || gradient_transform.is_some()
                    || image_ref.is_some()
                    || scale_mode.is_some()
                    || figma_type.is_some()
                {
                    return Err(A::Error::custom("solid paint contains variant-only fields"));
                }
                Ok(PaintValue::Solid {
                    color: color.ok_or_else(|| A::Error::missing_field("color"))?,
                    opacity: opacity.ok_or_else(|| A::Error::missing_field("opacity"))?,
                })
            }
            PaintTag::LinearGradient
            | PaintTag::RadialGradient
            | PaintTag::AngularGradient
            | PaintTag::DiamondGradient => {
                if color.is_some()
                    || image_ref.is_some()
                    || scale_mode.is_some()
                    || figma_type.is_some()
                {
                    return Err(A::Error::custom(
                        "gradient paint contains variant-only fields",
                    ));
                }
                let stops = stops.ok_or_else(|| A::Error::missing_field("stops"))?;
                let gradient_transform = gradient_transform
                    .ok_or_else(|| A::Error::missing_field("gradientTransform"))?;
                let opacity = opacity.ok_or_else(|| A::Error::missing_field("opacity"))?;
                // Name every gradient tag. A wildcard here compiles just as well
                // and is how a future fifth gradient type silently decodes as a
                // diamond: adding it to the outer pattern above would be enough to
                // build. `unreachable!` is only for the non-gradient tags the outer
                // match already excluded.
                Ok(match tag {
                    PaintTag::LinearGradient => PaintValue::LinearGradient {
                        stops,
                        gradient_transform,
                        opacity,
                    },
                    PaintTag::RadialGradient => PaintValue::RadialGradient {
                        stops,
                        gradient_transform,
                        opacity,
                    },
                    PaintTag::AngularGradient => PaintValue::AngularGradient {
                        stops,
                        gradient_transform,
                        opacity,
                    },
                    PaintTag::DiamondGradient => PaintValue::DiamondGradient {
                        stops,
                        gradient_transform,
                        opacity,
                    },
                    _ => unreachable!("outer match narrowed tag to gradients"),
                })
            }
            PaintTag::Image => {
                if color.is_some()
                    || stops.is_some()
                    || gradient_transform.is_some()
                    || figma_type.is_some()
                {
                    return Err(A::Error::custom("image paint contains variant-only fields"));
                }
                Ok(PaintValue::Image {
                    image_ref: image_ref.ok_or_else(|| A::Error::missing_field("imageRef"))?,
                    scale_mode: scale_mode.ok_or_else(|| A::Error::missing_field("scaleMode"))?,
                    opacity: opacity.ok_or_else(|| A::Error::missing_field("opacity"))?,
                })
            }
            PaintTag::Mixed => {
                if color.is_some()
                    || opacity.is_some()
                    || stops.is_some()
                    || gradient_transform.is_some()
                    || image_ref.is_some()
                    || scale_mode.is_some()
                    || figma_type.is_some()
                {
                    return Err(A::Error::custom(
                        "mixed paint cannot contain payload fields",
                    ));
                }
                Ok(PaintValue::Mixed)
            }
            PaintTag::Unsupported => {
                if color.is_some()
                    || opacity.is_some()
                    || stops.is_some()
                    || gradient_transform.is_some()
                    || image_ref.is_some()
                    || scale_mode.is_some()
                {
                    return Err(A::Error::custom(
                        "unsupported paint cannot contain payload fields",
                    ));
                }
                Ok(PaintValue::Unsupported {
                    figma_type: figma_type.ok_or_else(|| A::Error::missing_field("figmaType"))?,
                })
            }
        }
    }
}

/// Figma blend mode, mapped to CSS `mix-blend-mode` where an equivalent exists.
///
/// `Normal` and `PassThrough` are the Figma defaults and are never emitted; they
/// stay in the enum so the schema describes the full domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    PassThrough,
    Normal,
    Darken,
    Multiply,
    LinearBurn,
    ColorBurn,
    Lighten,
    Screen,
    LinearDodge,
    ColorDodge,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EffectValue {
    DropShadow {
        color: Color,
        offset_x: f64,
        offset_y: f64,
        radius: f64,
        spread: f64,
    },
    InnerShadow {
        color: Color,
        offset_x: f64,
        offset_y: f64,
        radius: f64,
        spread: f64,
    },
    LayerBlur {
        radius: f64,
    },
    BackgroundBlur {
        radius: f64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum EffectTag {
    DropShadow,
    InnerShadow,
    LayerBlur,
    BackgroundBlur,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum EffectField {
    Type,
    Color,
    OffsetX,
    OffsetY,
    Radius,
    Spread,
}

impl<'de> Deserialize<'de> for EffectValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(EffectVisitor)
    }
}

struct EffectVisitor;

impl<'de> Visitor<'de> for EffectVisitor {
    type Value = EffectValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed effect value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut color = None;
        let mut offset_x = None;
        let mut offset_y = None;
        let mut radius = None;
        let mut spread = None;
        while let Some(field) = map.next_key::<EffectField>()? {
            match field {
                EffectField::Type => {
                    set_field_once(&mut tag, map.next_value::<EffectTag>()?, "type")?
                }
                EffectField::Color => {
                    set_field_once(&mut color, map.next_value::<Color>()?, "color")?
                }
                EffectField::OffsetX => {
                    set_field_once(&mut offset_x, map.next_value::<f64>()?, "offsetX")?
                }
                EffectField::OffsetY => {
                    set_field_once(&mut offset_y, map.next_value::<f64>()?, "offsetY")?
                }
                EffectField::Radius => {
                    set_field_once(&mut radius, map.next_value::<f64>()?, "radius")?
                }
                EffectField::Spread => {
                    set_field_once(&mut spread, map.next_value::<f64>()?, "spread")?
                }
            }
        }
        let radius = radius.ok_or_else(|| A::Error::missing_field("radius"))?;
        match tag.ok_or_else(|| A::Error::missing_field("type"))? {
            EffectTag::DropShadow => Ok(EffectValue::DropShadow {
                color: color.ok_or_else(|| A::Error::missing_field("color"))?,
                offset_x: offset_x.ok_or_else(|| A::Error::missing_field("offsetX"))?,
                offset_y: offset_y.ok_or_else(|| A::Error::missing_field("offsetY"))?,
                radius,
                spread: spread.ok_or_else(|| A::Error::missing_field("spread"))?,
            }),
            EffectTag::InnerShadow => Ok(EffectValue::InnerShadow {
                color: color.ok_or_else(|| A::Error::missing_field("color"))?,
                offset_x: offset_x.ok_or_else(|| A::Error::missing_field("offsetX"))?,
                offset_y: offset_y.ok_or_else(|| A::Error::missing_field("offsetY"))?,
                radius,
                spread: spread.ok_or_else(|| A::Error::missing_field("spread"))?,
            }),
            EffectTag::LayerBlur => {
                reject_shadow_fields::<A::Error>(color, offset_x, offset_y, spread)?;
                Ok(EffectValue::LayerBlur { radius })
            }
            EffectTag::BackgroundBlur => {
                reject_shadow_fields::<A::Error>(color, offset_x, offset_y, spread)?;
                Ok(EffectValue::BackgroundBlur { radius })
            }
        }
    }
}

fn reject_shadow_fields<E>(
    color: Option<Color>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
    spread: Option<f64>,
) -> Result<(), E>
where
    E: serde::de::Error,
{
    if color.is_some() || offset_x.is_some() || offset_y.is_some() || spread.is_some() {
        return Err(E::custom("blur effect cannot contain shadow-only fields"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StrokeAlign {
    Inside,
    Outside,
    Center,
}

/// Border paint, width, alignment, and dash pattern.
///
/// `FullNodeData::paints` carries fills only; stroke colours live here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrokeValue {
    pub paints: ReturnedList<PaintValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<StrokeAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash_pattern: Option<ReturnedList<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CornerRadiusValue {
    Uniform {
        radius: f64,
    },
    PerCorner {
        top_left: f64,
        top_right: f64,
        bottom_right: f64,
        bottom_left: f64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum CornerRadiusTag {
    Uniform,
    PerCorner,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum CornerRadiusField {
    Kind,
    Radius,
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl<'de> Deserialize<'de> for CornerRadiusValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CornerRadiusVisitor)
    }
}

struct CornerRadiusVisitor;

impl<'de> Visitor<'de> for CornerRadiusVisitor {
    type Value = CornerRadiusValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed corner radius value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut radius = None;
        let mut top_left = None;
        let mut top_right = None;
        let mut bottom_right = None;
        let mut bottom_left = None;
        while let Some(field) = map.next_key::<CornerRadiusField>()? {
            match field {
                CornerRadiusField::Kind => {
                    set_field_once(&mut tag, map.next_value::<CornerRadiusTag>()?, "kind")?
                }
                CornerRadiusField::Radius => {
                    set_field_once(&mut radius, map.next_value::<f64>()?, "radius")?
                }
                CornerRadiusField::TopLeft => {
                    set_field_once(&mut top_left, map.next_value::<f64>()?, "topLeft")?
                }
                CornerRadiusField::TopRight => {
                    set_field_once(&mut top_right, map.next_value::<f64>()?, "topRight")?
                }
                CornerRadiusField::BottomRight => {
                    set_field_once(&mut bottom_right, map.next_value::<f64>()?, "bottomRight")?
                }
                CornerRadiusField::BottomLeft => {
                    set_field_once(&mut bottom_left, map.next_value::<f64>()?, "bottomLeft")?
                }
            }
        }
        match tag.ok_or_else(|| A::Error::missing_field("kind"))? {
            CornerRadiusTag::Uniform => {
                if top_left.is_some()
                    || top_right.is_some()
                    || bottom_right.is_some()
                    || bottom_left.is_some()
                {
                    return Err(A::Error::custom(
                        "uniform corner radius contains variant-only fields",
                    ));
                }
                Ok(CornerRadiusValue::Uniform {
                    radius: radius.ok_or_else(|| A::Error::missing_field("radius"))?,
                })
            }
            CornerRadiusTag::PerCorner => {
                if radius.is_some() {
                    return Err(A::Error::custom(
                        "per-corner radius contains variant-only fields",
                    ));
                }
                Ok(CornerRadiusValue::PerCorner {
                    top_left: top_left.ok_or_else(|| A::Error::missing_field("topLeft"))?,
                    top_right: top_right.ok_or_else(|| A::Error::missing_field("topRight"))?,
                    bottom_right: bottom_right
                        .ok_or_else(|| A::Error::missing_field("bottomRight"))?,
                    bottom_left: bottom_left
                        .ok_or_else(|| A::Error::missing_field("bottomLeft"))?,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LayoutMode {
    None,
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LayoutSizing {
    Fixed,
    Hug,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AxisAlign {
    Min,
    Center,
    Max,
    SpaceBetween,
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutValue {
    pub mode: LayoutMode,
    pub primary_sizing: LayoutSizing,
    pub counter_sizing: LayoutSizing,
    pub gap: f64,
    pub padding_top: f64,
    pub padding_right: f64,
    pub padding_bottom: f64,
    pub padding_left: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_align: Option<AxisAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_align: Option<AxisAlign>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter_axis_spacing: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "unit",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum LineHeightValue {
    Pixels(f64),
    Percent(f64),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "unit",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum LetterSpacingValue {
    Pixels(f64),
    Percent(f64),
}

/// `None` is the Figma default for textDecoration and is never emitted; it stays in
/// the enum so the schema describes the full domain (absence means "default").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TextDecoration {
    None,
    Underline,
    Strikethrough,
}

/// `Left` is the Figma default for textAlignHorizontal and is never emitted; it
/// stays in the enum so the schema describes the full domain (absence means
/// "default").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TextAlignHorizontal {
    Left,
    Center,
    Right,
    Justified,
}

/// `Top` is the Figma default for textAlignVertical and is never emitted; it stays
/// in the enum so the schema describes the full domain (absence means "default").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TextAlignVertical {
    Top,
    Center,
    Bottom,
}

/// `None` is the Figma default for textAutoResize and is never emitted; it stays in
/// the enum so the schema describes the full domain (absence means "default").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TextAutoResize {
    None,
    WidthAndHeight,
    Height,
    Truncate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextStyle {
    pub font_family: String,
    pub font_style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<LineHeightValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<LetterSpacingValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_decoration: Option<TextDecoration>,
    pub paints: ReturnedList<PaintValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyledTextRange {
    pub start: usize,
    pub end: usize,
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextValue {
    pub characters: String,
    pub default_style: TextStyle,
    pub styled_ranges: ReturnedList<StyledTextRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_horizontal: Option<TextAlignHorizontal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_vertical: Option<TextAlignVertical>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<TextAutoResize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextSummary {
    pub character_count: usize,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum ComponentPropertyValue {
    Text(String),
    Boolean(bool),
    InstanceSwap(String),
    Variant(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ComponentPropertyTag {
    Text,
    Boolean,
    InstanceSwap,
    Variant,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum AdjacentValueField {
    Kind,
    Value,
}

impl<'de> Deserialize<'de> for ComponentPropertyValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ComponentPropertyVisitor)
    }
}

struct ComponentPropertyVisitor;

impl<'de> Visitor<'de> for ComponentPropertyVisitor {
    type Value = ComponentPropertyValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed component property value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut value = None;
        while let Some(field) = map.next_key::<AdjacentValueField>()? {
            match field {
                AdjacentValueField::Kind => {
                    set_field_once(&mut tag, map.next_value::<ComponentPropertyTag>()?, "kind")?
                }
                AdjacentValueField::Value => {
                    set_field_once(&mut value, map.next_value::<Box<RawValue>>()?, "value")?
                }
            }
        }
        let value = value.ok_or_else(|| A::Error::missing_field("value"))?;
        match tag.ok_or_else(|| A::Error::missing_field("kind"))? {
            ComponentPropertyTag::Text => decode_raw(&value).map(ComponentPropertyValue::Text),
            ComponentPropertyTag::Boolean => {
                decode_raw(&value).map(ComponentPropertyValue::Boolean)
            }
            ComponentPropertyTag::InstanceSwap => {
                decode_raw(&value).map(ComponentPropertyValue::InstanceSwap)
            }
            ComponentPropertyTag::Variant => {
                decode_raw(&value).map(ComponentPropertyValue::Variant)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamedComponentProperty {
    pub name: String,
    pub value: ComponentPropertyValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentValue {
    pub component_id: NodeId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component_set_id: Option<NodeId>,
    pub properties: ReturnedList<NamedComponentProperty>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum VariableValue {
    Boolean(bool),
    Float(f64),
    String(String),
    Color(Color),
    Alias(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum VariableValueTag {
    Boolean,
    Float,
    String,
    Color,
    Alias,
}

impl<'de> Deserialize<'de> for VariableValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(VariableValueVisitor)
    }
}

struct VariableValueVisitor;

impl<'de> Visitor<'de> for VariableValueVisitor {
    type Value = VariableValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed variable value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut value = None;
        while let Some(field) = map.next_key::<AdjacentValueField>()? {
            match field {
                AdjacentValueField::Kind => {
                    set_field_once(&mut tag, map.next_value::<VariableValueTag>()?, "kind")?
                }
                AdjacentValueField::Value => {
                    set_field_once(&mut value, map.next_value::<Box<RawValue>>()?, "value")?
                }
            }
        }
        let value = value.ok_or_else(|| A::Error::missing_field("value"))?;
        match tag.ok_or_else(|| A::Error::missing_field("kind"))? {
            VariableValueTag::Boolean => decode_raw(&value).map(VariableValue::Boolean),
            VariableValueTag::Float => decode_raw(&value).map(VariableValue::Float),
            VariableValueTag::String => decode_raw(&value).map(VariableValue::String),
            VariableValueTag::Color => decode_raw(&value).map(VariableValue::Color),
            VariableValueTag::Alias => decode_raw(&value).map(VariableValue::Alias),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StyleKind {
    Paint,
    Stroke,
    Text,
    Effect,
    Grid,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "styleType",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StyleValue {
    Paint {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        paints: ReturnedList<PaintValue>,
    },
    Text {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        text: TextValue,
    },
    Effect {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        effects: ReturnedList<EffectValue>,
    },
    Grid {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        pattern: String,
        size: f64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum StyleValueTag {
    Paint,
    Text,
    Effect,
    Grid,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum StyleValueField {
    StyleType,
    Id,
    Name,
    Description,
    Remote,
    Key,
    Paints,
    Text,
    Effects,
    Pattern,
    Size,
}

impl<'de> Deserialize<'de> for StyleValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(StyleValueVisitor)
    }
}

struct StyleValueVisitor;

impl<'de> Visitor<'de> for StyleValueVisitor {
    type Value = StyleValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed style value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut id = None;
        let mut name = None;
        let mut description = None;
        let mut remote = None;
        let mut key = None;
        let mut paints = None;
        let mut text = None;
        let mut effects = None;
        let mut pattern = None;
        let mut size = None;
        while let Some(field) = map.next_key::<StyleValueField>()? {
            match field {
                StyleValueField::StyleType => {
                    set_field_once(&mut tag, map.next_value::<StyleValueTag>()?, "styleType")?
                }
                StyleValueField::Id => set_field_once(&mut id, map.next_value::<String>()?, "id")?,
                StyleValueField::Name => {
                    set_field_once(&mut name, map.next_value::<String>()?, "name")?
                }
                StyleValueField::Description => {
                    set_field_once(&mut description, map.next_value::<String>()?, "description")?
                }
                StyleValueField::Remote => {
                    set_field_once(&mut remote, map.next_value::<bool>()?, "remote")?
                }
                StyleValueField::Key => {
                    set_field_once(&mut key, map.next_value::<String>()?, "key")?
                }
                StyleValueField::Paints => set_field_once(
                    &mut paints,
                    map.next_value::<ReturnedList<PaintValue>>()?,
                    "paints",
                )?,
                StyleValueField::Text => {
                    set_field_once(&mut text, map.next_value::<TextValue>()?, "text")?
                }
                StyleValueField::Effects => set_field_once(
                    &mut effects,
                    map.next_value::<ReturnedList<EffectValue>>()?,
                    "effects",
                )?,
                StyleValueField::Pattern => {
                    set_field_once(&mut pattern, map.next_value::<String>()?, "pattern")?
                }
                StyleValueField::Size => {
                    set_field_once(&mut size, map.next_value::<f64>()?, "size")?
                }
            }
        }
        let id = id.ok_or_else(|| A::Error::missing_field("id"))?;
        let name = name.ok_or_else(|| A::Error::missing_field("name"))?;
        match tag.ok_or_else(|| A::Error::missing_field("styleType"))? {
            StyleValueTag::Paint => {
                if text.is_some() || effects.is_some() || pattern.is_some() || size.is_some() {
                    return Err(A::Error::custom("paint style contains variant-only fields"));
                }
                Ok(StyleValue::Paint {
                    id,
                    name,
                    description,
                    remote,
                    key,
                    paints: paints.ok_or_else(|| A::Error::missing_field("paints"))?,
                })
            }
            StyleValueTag::Text => {
                if paints.is_some() || effects.is_some() || pattern.is_some() || size.is_some() {
                    return Err(A::Error::custom("text style contains variant-only fields"));
                }
                Ok(StyleValue::Text {
                    id,
                    name,
                    description,
                    remote,
                    key,
                    text: text.ok_or_else(|| A::Error::missing_field("text"))?,
                })
            }
            StyleValueTag::Effect => {
                if paints.is_some() || text.is_some() || pattern.is_some() || size.is_some() {
                    return Err(A::Error::custom(
                        "effect style contains variant-only fields",
                    ));
                }
                Ok(StyleValue::Effect {
                    id,
                    name,
                    description,
                    remote,
                    key,
                    effects: effects.ok_or_else(|| A::Error::missing_field("effects"))?,
                })
            }
            StyleValueTag::Grid => {
                if paints.is_some() || text.is_some() || effects.is_some() {
                    return Err(A::Error::custom("grid style contains variant-only fields"));
                }
                Ok(StyleValue::Grid {
                    id,
                    name,
                    description,
                    remote,
                    key,
                    pattern: pattern.ok_or_else(|| A::Error::missing_field("pattern"))?,
                    size: size.ok_or_else(|| A::Error::missing_field("size"))?,
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transform2D {
    pub m00: f64,
    pub m01: f64,
    pub m02: f64,
    pub m10: f64,
    pub m11: f64,
    pub m12: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometryValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    pub rotation: f64,
    pub opacity: f64,
    pub transform: Transform2D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintAxis {
    Min,
    Center,
    Max,
    Stretch,
    Scale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutConstraints {
    pub horizontal: ConstraintAxis,
    pub vertical: ConstraintAxis,
}

/// A style applied to a node.
///
/// `name` is absent when the style could not be resolved (unreachable remote
/// style, missing id, or an exhausted resolve budget). It is never an empty
/// string and never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleReference {
    pub id: String,
    pub kind: StyleKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariableReference {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceValue {
    pub component_id: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_set_id: Option<NodeId>,
    pub properties: ReturnedList<NamedComponentProperty>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeSummary {
    pub id: NodeId,
    pub name: DisplayText,
    pub node_type: NodeTypeName,
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<NodeId>,
    #[serde(default, skip_serializing_if = "NodeIdList::is_empty")]
    pub child_ids: NodeIdList,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesignNode<D> {
    pub summary: NodeSummary,
    pub data: D,
    pub children: Vec<DesignNode<D>>,
    pub children_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children_truncation: Option<Truncation>,
}

struct NodeDecodeBudget {
    remaining_nodes: usize,
}

struct DesignNodeSeed<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    depth: u8,
    marker: PhantomData<D>,
}

impl<'de, D> DeserializeSeed<'de> for DesignNodeSeed<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = DesignNode<D>;

    fn deserialize<DeserializerType>(
        self,
        deserializer: DeserializerType,
    ) -> Result<Self::Value, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        if self.depth > MAX_DEPTH {
            return Err(DeserializerType::Error::custom(format_args!(
                "node tree depth {} exceeds {MAX_DEPTH}",
                self.depth
            )));
        }
        if self.budget.remaining_nodes == 0 {
            return Err(DeserializerType::Error::custom(format_args!(
                "returned node count exceeds {MAX_RETURNED_NODES}"
            )));
        }
        self.budget.remaining_nodes -= 1;
        deserializer.deserialize_map(DesignNodeVisitor {
            budget: self.budget,
            depth: self.depth,
            marker: PhantomData,
        })
    }
}

struct DesignNodeVisitor<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    depth: u8,
    marker: PhantomData<D>,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum DesignNodeField {
    Summary,
    Data,
    Children,
    ChildrenTruncated,
    ChildrenTruncation,
}

impl<'de, D> Visitor<'de> for DesignNodeVisitor<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = DesignNode<D>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed, depth-bounded design node")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut summary = None;
        let mut data = None;
        let mut children = None;
        let mut children_truncated = None;
        let mut children_truncation = None;

        while let Some(field) = map.next_key::<DesignNodeField>()? {
            match field {
                DesignNodeField::Summary => {
                    if summary.is_some() {
                        return Err(A::Error::duplicate_field("summary"));
                    }
                    summary = Some(map.next_value()?);
                }
                DesignNodeField::Data => {
                    if data.is_some() {
                        return Err(A::Error::duplicate_field("data"));
                    }
                    data = Some(map.next_value()?);
                }
                DesignNodeField::Children => {
                    if children.is_some() {
                        return Err(A::Error::duplicate_field("children"));
                    }
                    children = Some(map.next_value_seed(DesignNodeListSeed {
                        budget: self.budget,
                        depth: self.depth.saturating_add(1),
                        maximum_items: MAX_RETURNED_NODES,
                        marker: PhantomData,
                    })?);
                }
                DesignNodeField::ChildrenTruncated => {
                    if children_truncated.is_some() {
                        return Err(A::Error::duplicate_field("childrenTruncated"));
                    }
                    children_truncated = Some(map.next_value()?);
                }
                DesignNodeField::ChildrenTruncation => {
                    if children_truncation.is_some() {
                        return Err(A::Error::duplicate_field("childrenTruncation"));
                    }
                    children_truncation = Some(map.next_value()?);
                }
            }
        }

        Ok(DesignNode {
            summary: summary.ok_or_else(|| A::Error::missing_field("summary"))?,
            data: data.ok_or_else(|| A::Error::missing_field("data"))?,
            children: children.ok_or_else(|| A::Error::missing_field("children"))?,
            children_truncated: children_truncated
                .ok_or_else(|| A::Error::missing_field("childrenTruncated"))?,
            children_truncation: children_truncation.unwrap_or(None),
        })
    }
}

struct DesignNodeListSeed<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    depth: u8,
    maximum_items: usize,
    marker: PhantomData<D>,
}

impl<'de, D> DeserializeSeed<'de> for DesignNodeListSeed<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = Vec<DesignNode<D>>;

    fn deserialize<DeserializerType>(
        self,
        deserializer: DeserializerType,
    ) -> Result<Self::Value, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DesignNodeListVisitor {
            budget: self.budget,
            depth: self.depth,
            maximum_items: self.maximum_items,
            marker: PhantomData,
        })
    }
}

struct DesignNodeListVisitor<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    depth: u8,
    maximum_items: usize,
    marker: PhantomData<D>,
}

impl<'de, D> Visitor<'de> for DesignNodeListVisitor<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = Vec<DesignNode<D>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a globally node-budgeted design node list")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence
            .size_hint()
            .unwrap_or(0)
            .min(self.maximum_items)
            .min(self.budget.remaining_nodes);
        let mut nodes = Vec::with_capacity(capacity);
        while nodes.len() < self.maximum_items {
            let next = sequence.next_element_seed(DesignNodeSeed {
                budget: self.budget,
                depth: self.depth,
                marker: PhantomData,
            })?;
            match next {
                Some(node) => nodes.push(node),
                None => return Ok(nodes),
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format_args!(
                "design node list exceeds {} items",
                self.maximum_items
            )));
        }
        Ok(nodes)
    }
}

impl<'de, D> Deserialize<'de> for DesignNode<D>
where
    D: Deserialize<'de>,
{
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let mut budget = NodeDecodeBudget {
            remaining_nodes: MAX_RETURNED_NODES,
        };
        DesignNodeSeed {
            budget: &mut budget,
            depth: 0,
            marker: PhantomData,
        }
        .deserialize(deserializer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NodeForest<D>(Vec<DesignNode<D>>);

impl<D> NodeForest<D> {
    pub fn as_slice(&self) -> &[DesignNode<D>] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D> TryFrom<Vec<DesignNode<D>>> for NodeForest<D> {
    type Error = BoundaryValueError;

    fn try_from(nodes: Vec<DesignNode<D>>) -> Result<Self, Self::Error> {
        if nodes.len() > MAX_RETURNED_NODES {
            return Err(BoundaryValueError::TooMany {
                label: "returned design node roots",
                actual: nodes.len(),
                maximum: MAX_RETURNED_NODES,
            });
        }
        validate_node_forest(&nodes)?;
        Ok(Self(nodes))
    }
}

impl<'de, D> Deserialize<'de> for NodeForest<D>
where
    D: Deserialize<'de>,
{
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let mut budget = NodeDecodeBudget {
            remaining_nodes: MAX_RETURNED_NODES,
        };
        DesignNodeListSeed {
            budget: &mut budget,
            depth: 0,
            maximum_items: MAX_RETURNED_NODES,
            marker: PhantomData,
        }
        .deserialize(deserializer)
        .map(Self)
    }
}

impl<D> JsonSchema for NodeForest<D>
where
    D: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("NodeForest_of_{}", D::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        recursive_collection_schema::<DesignNode<D>>(generator, MAX_RETURNED_NODES)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NodeBatch<D>(Vec<ItemResult<DesignNode<D>>>);

impl<D> NodeBatch<D> {
    pub fn as_slice(&self) -> &[ItemResult<DesignNode<D>>] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<D> TryFrom<Vec<ItemResult<DesignNode<D>>>> for NodeBatch<D> {
    type Error = BoundaryValueError;

    fn try_from(items: Vec<ItemResult<DesignNode<D>>>) -> Result<Self, Self::Error> {
        if items.len() > MAX_INPUT_IDS {
            return Err(BoundaryValueError::TooMany {
                label: "node results",
                actual: items.len(),
                maximum: MAX_INPUT_IDS,
            });
        }
        validate_node_roots(items.iter().filter_map(|item| match item {
            ItemResult::Success { value } => Some(value),
            ItemResult::Error { .. } => None,
        }))?;
        Ok(Self(items))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ItemResultStatus {
    Success,
    Error,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ItemResultField {
    Status,
    Value,
    Error,
}

struct ItemResultSeed<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    marker: PhantomData<D>,
}

impl<'de, D> DeserializeSeed<'de> for ItemResultSeed<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = ItemResult<DesignNode<D>>;

    fn deserialize<DeserializerType>(
        self,
        deserializer: DeserializerType,
    ) -> Result<Self::Value, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_map(ItemResultVisitor {
            budget: self.budget,
            marker: PhantomData,
        })
    }
}

struct ItemResultVisitor<'a, D> {
    budget: &'a mut NodeDecodeBudget,
    marker: PhantomData<D>,
}

impl<'de, D> Visitor<'de> for ItemResultVisitor<'_, D>
where
    D: Deserialize<'de>,
{
    type Value = ItemResult<DesignNode<D>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed success or error node result")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut status = None;
        let mut value = None;
        let mut error = None;
        while let Some(field) = map.next_key::<ItemResultField>()? {
            match field {
                ItemResultField::Status => {
                    if status.is_some() {
                        return Err(A::Error::duplicate_field("status"));
                    }
                    status = Some(map.next_value()?);
                }
                ItemResultField::Value => {
                    if value.is_some() {
                        return Err(A::Error::duplicate_field("value"));
                    }
                    value = Some(map.next_value_seed(DesignNodeSeed {
                        budget: self.budget,
                        depth: 0,
                        marker: PhantomData,
                    })?);
                }
                ItemResultField::Error => {
                    if error.is_some() {
                        return Err(A::Error::duplicate_field("error"));
                    }
                    error = Some(map.next_value()?);
                }
            }
        }

        match status.ok_or_else(|| A::Error::missing_field("status"))? {
            ItemResultStatus::Success => {
                if error.is_some() {
                    return Err(A::Error::custom("success node result cannot contain error"));
                }
                Ok(ItemResult::Success {
                    value: value.ok_or_else(|| A::Error::missing_field("value"))?,
                })
            }
            ItemResultStatus::Error => {
                if value.is_some() {
                    return Err(A::Error::custom("error node result cannot contain value"));
                }
                Ok(ItemResult::Error {
                    error: error.ok_or_else(|| A::Error::missing_field("error"))?,
                })
            }
        }
    }
}

struct NodeBatchVisitor<D>(PhantomData<D>);

impl<'de, D> Visitor<'de> for NodeBatchVisitor<D>
where
    D: Deserialize<'de>,
{
    type Value = NodeBatch<D>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {MAX_INPUT_IDS} ordered node results")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(0).min(MAX_INPUT_IDS);
        let mut items = Vec::with_capacity(capacity);
        let mut budget = NodeDecodeBudget {
            remaining_nodes: MAX_RETURNED_NODES,
        };
        while items.len() < MAX_INPUT_IDS {
            let next = sequence.next_element_seed(ItemResultSeed {
                budget: &mut budget,
                marker: PhantomData,
            })?;
            match next {
                Some(item) => items.push(item),
                None => return Ok(NodeBatch(items)),
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format_args!(
                "node result count exceeds {MAX_INPUT_IDS}"
            )));
        }
        Ok(NodeBatch(items))
    }
}

impl<'de, D> Deserialize<'de> for NodeBatch<D>
where
    D: Deserialize<'de>,
{
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_seq(NodeBatchVisitor(PhantomData))
    }
}

impl<D> JsonSchema for NodeBatch<D>
where
    D: JsonSchema,
{
    fn schema_name() -> Cow<'static, str> {
        format!("NodeBatch_of_{}", D::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        recursive_collection_schema::<ItemResult<DesignNode<D>>>(generator, MAX_INPUT_IDS)
    }
}

fn recursive_collection_schema<T: JsonSchema>(
    generator: &mut SchemaGenerator,
    maximum_items: usize,
) -> Schema {
    let mut schema = Vec::<T>::json_schema(generator);
    if let Some(object) = schema.as_object_mut() {
        object.insert("maxItems".to_owned(), maximum_items.into());
        object.insert("x-maxDepth".to_owned(), MAX_DEPTH.into());
        object.insert("x-maxReturnedNodes".to_owned(), MAX_RETURNED_NODES.into());
        object.insert(
            "description".to_owned(),
            "Recursive design-node payload with a shared decoder budget".into(),
        );
    }
    schema
}

fn validate_node_forest<D>(nodes: &[DesignNode<D>]) -> Result<(), BoundaryValueError> {
    validate_node_roots(nodes.iter())
}

fn validate_node_roots<'a, D>(
    roots: impl Iterator<Item = &'a DesignNode<D>>,
) -> Result<(), BoundaryValueError>
where
    D: 'a,
{
    let mut count = 0_usize;
    for root in roots {
        validate_node(root, 0, &mut count)?;
    }
    Ok(())
}

fn validate_node<D>(
    node: &DesignNode<D>,
    depth: u8,
    count: &mut usize,
) -> Result<(), BoundaryValueError> {
    if depth > MAX_DEPTH {
        return Err(BoundaryValueError::TooDeep {
            actual: depth,
            maximum: MAX_DEPTH,
        });
    }
    *count += 1;
    if *count > MAX_RETURNED_NODES {
        return Err(BoundaryValueError::TooMany {
            label: "returned design nodes",
            actual: *count,
            maximum: MAX_RETURNED_NODES,
        });
    }
    if node.children.len() > MAX_RETURNED_NODES - *count {
        return Err(BoundaryValueError::TooMany {
            label: "returned design nodes",
            actual: (*count).saturating_add(node.children.len()),
            maximum: MAX_RETURNED_NODES,
        });
    }
    for child in &node.children {
        validate_node(child, depth.saturating_add(1), count)?;
    }
    Ok(())
}

/// Closed detail marker for a recursive minimal node.
///
/// Identity, visibility, hierarchy references, and basic bounds live in the
/// node's [`NodeSummary`]. The empty detail object prevents compact/full-only
/// fields from appearing in a minimal result while retaining one recursive
/// tree representation for all detail levels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MinimalNodeDetails {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactNodeData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<LayoutConstraints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_layout: Option<LayoutValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<ComponentValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<InstanceValue>,
    pub style_references: ReturnedList<StyleReference>,
    pub variable_references: ReturnedList<VariableReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FullNodeData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<GeometryValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<LayoutConstraints>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_layout: Option<LayoutValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextValue>,
    pub paints: ReturnedList<PaintValue>,
    pub effects: ReturnedList<EffectValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<ComponentValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<InstanceValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strokes: Option<StrokeValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<CornerRadiusValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_smoothing: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clips_content: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_mode: Option<BlendMode>,
    pub style_references: ReturnedList<StyleReference>,
    pub variable_references: ReturnedList<VariableReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase", deny_unknown_fields)]
pub enum ItemResult<T> {
    Success { value: T },
    Error { error: ToolError },
}

struct PlainItemResultVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for PlainItemResultVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = ItemResult<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed success or error item result")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut status = None;
        let mut value = None;
        let mut error = None;
        while let Some(field) = map.next_key::<ItemResultField>()? {
            match field {
                ItemResultField::Status => {
                    if status.is_some() {
                        return Err(A::Error::duplicate_field("status"));
                    }
                    status = Some(map.next_value::<ItemResultStatus>()?);
                }
                ItemResultField::Value => {
                    if value.is_some() {
                        return Err(A::Error::duplicate_field("value"));
                    }
                    value = Some(map.next_value::<T>()?);
                }
                ItemResultField::Error => {
                    if error.is_some() {
                        return Err(A::Error::duplicate_field("error"));
                    }
                    error = Some(map.next_value::<ToolError>()?);
                }
            }
        }
        match status.ok_or_else(|| A::Error::missing_field("status"))? {
            ItemResultStatus::Success => {
                if error.is_some() {
                    return Err(A::Error::custom("success item cannot contain error"));
                }
                Ok(ItemResult::Success {
                    value: value.ok_or_else(|| A::Error::missing_field("value"))?,
                })
            }
            ItemResultStatus::Error => {
                if value.is_some() {
                    return Err(A::Error::custom("error item cannot contain value"));
                }
                Ok(ItemResult::Error {
                    error: error.ok_or_else(|| A::Error::missing_field("error"))?,
                })
            }
        }
    }
}

impl<'de, T> Deserialize<'de> for ItemResult<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PlainItemResultVisitor(PhantomData))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedItems<T> {
    pub items: ReturnedList<T>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}

#[cfg(test)]
mod tests {
    use super::{BlendMode, TextAutoResize, TextDecoration, TextValue};
    use serde_json::json;

    #[test]
    fn blend_mode_round_trips_and_stays_closed() {
        for value in ["passThrough", "multiply", "softLight", "luminosity"] {
            let parsed: BlendMode = serde_json::from_value(json!(value)).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), json!(value));
        }
        assert!(
            serde_json::from_value::<BlendMode>(json!("plusLighter")).is_err(),
            "blend mode must stay closed"
        );
    }

    #[test]
    fn text_properties_round_trip_and_stay_closed() {
        let value = json!({
            "characters": "Save",
            "defaultStyle": {
                "fontFamily": "Inter",
                "fontStyle": "Light",
                "fontWeight": 300.0,
                "textDecoration": "underline",
                "paints": []
            },
            "styledRanges": [],
            "alignHorizontal": "justified",
            "alignVertical": "center",
            "autoResize": "widthAndHeight"
        });
        let parsed: TextValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);

        assert!(
            serde_json::from_value::<TextAutoResize>(json!("width")).is_err(),
            "auto resize must stay closed"
        );
        assert!(
            serde_json::from_value::<TextDecoration>(json!("overline")).is_err(),
            "text decoration must stay closed"
        );
    }
}
