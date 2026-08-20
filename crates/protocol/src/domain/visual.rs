//! Raster and SVG contracts.

use super::{
    ConnectionId, ItemResult, NodeId, NodeSelector, NodesSelector, ObservationWindow, RasterBase64,
    ReturnedList, SelectionSelector, SvgRejectionName, SvgSource, Truncation,
};
use crate::limits::{MAX_RASTER_PIXELS, MAX_RASTER_SIDE};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use std::{borrow::Cow, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ScreenshotSelector {
    Selection(SelectionSelector),
    Node(NodeSelector),
    Nodes(NodesSelector),
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ScreenshotSelectorField {
    Selection,
    NodeId,
    NodeIds,
}

impl<'de> Deserialize<'de> for ScreenshotSelector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ScreenshotSelectorVisitor)
    }
}

struct ScreenshotSelectorVisitor;

impl<'de> Visitor<'de> for ScreenshotSelectorVisitor {
    type Value = ScreenshotSelector;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("exactly one screenshot selector field")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(field) = map.next_key::<ScreenshotSelectorField>()? else {
            return Err(A::Error::custom(
                "screenshot selector must contain one field",
            ));
        };
        let selector = match field {
            ScreenshotSelectorField::Selection => {
                ScreenshotSelector::Selection(SelectionSelector {
                    selection: map.next_value()?,
                })
            }
            ScreenshotSelectorField::NodeId => ScreenshotSelector::Node(NodeSelector {
                node_id: map.next_value()?,
            }),
            ScreenshotSelectorField::NodeIds => ScreenshotSelector::Nodes(NodesSelector {
                node_ids: map.next_value()?,
            }),
        };
        if map.next_key::<ScreenshotSelectorField>()?.is_some() {
            return Err(A::Error::custom(
                "screenshot selector must contain exactly one field",
            ));
        }
        Ok(selector)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct RasterScale(#[schemars(range(min = 0.25, max = 4.0))] f64);

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("scale must be finite and between 0.25 and 4.0")]
pub struct RasterScaleError;

impl RasterScale {
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for RasterScale {
    type Error = RasterScaleError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || !(0.25..=4.0).contains(&value) {
            return Err(RasterScaleError);
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for RasterScale {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "format",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GetScreenshotInput {
    Png {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connection_id: Option<ConnectionId>,
        selector: ScreenshotSelector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<RasterScale>,
    },
    Jpeg {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connection_id: Option<ConnectionId>,
        selector: ScreenshotSelector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<RasterScale>,
    },
    Svg {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connection_id: Option<ConnectionId>,
        selector: ScreenshotSelector,
        #[serde(default = "default_true")]
        svg_outline_text: bool,
        #[serde(default)]
        svg_id_attribute: bool,
        #[serde(default = "default_true")]
        svg_simplify_stroke: bool,
    },
}

impl GetScreenshotInput {
    pub fn connection_id(&self) -> Option<&ConnectionId> {
        match self {
            Self::Png { connection_id, .. }
            | Self::Jpeg { connection_id, .. }
            | Self::Svg { connection_id, .. } => connection_id.as_ref(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ScreenshotFormatTag {
    Png,
    Jpeg,
    Svg,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ScreenshotInputField {
    Format,
    ConnectionId,
    Selector,
    Scale,
    SvgOutlineText,
    SvgIdAttribute,
    SvgSimplifyStroke,
}

impl<'de> Deserialize<'de> for GetScreenshotInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(GetScreenshotInputVisitor)
    }
}

struct GetScreenshotInputVisitor;

impl<'de> Visitor<'de> for GetScreenshotInputVisitor {
    type Value = GetScreenshotInput;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed screenshot input")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut format = None;
        let mut connection_id = None;
        let mut selector = None;
        let mut scale = None;
        let mut svg_outline_text = None;
        let mut svg_id_attribute = None;
        let mut svg_simplify_stroke = None;
        while let Some(field) = map.next_key::<ScreenshotInputField>()? {
            match field {
                ScreenshotInputField::Format => set_once(
                    &mut format,
                    map.next_value::<ScreenshotFormatTag>()?,
                    "format",
                )?,
                ScreenshotInputField::ConnectionId => set_once(
                    &mut connection_id,
                    map.next_value::<Option<ConnectionId>>()?,
                    "connectionId",
                )?,
                ScreenshotInputField::Selector => set_once(
                    &mut selector,
                    map.next_value::<ScreenshotSelector>()?,
                    "selector",
                )?,
                ScreenshotInputField::Scale => set_once(
                    &mut scale,
                    map.next_value::<Option<RasterScale>>()?,
                    "scale",
                )?,
                ScreenshotInputField::SvgOutlineText => set_once(
                    &mut svg_outline_text,
                    map.next_value::<bool>()?,
                    "svgOutlineText",
                )?,
                ScreenshotInputField::SvgIdAttribute => set_once(
                    &mut svg_id_attribute,
                    map.next_value::<bool>()?,
                    "svgIdAttribute",
                )?,
                ScreenshotInputField::SvgSimplifyStroke => set_once(
                    &mut svg_simplify_stroke,
                    map.next_value::<bool>()?,
                    "svgSimplifyStroke",
                )?,
            }
        }
        let selector = selector.ok_or_else(|| A::Error::missing_field("selector"))?;
        match format.ok_or_else(|| A::Error::missing_field("format"))? {
            ScreenshotFormatTag::Png => {
                reject_svg_fields::<A::Error>(
                    svg_outline_text,
                    svg_id_attribute,
                    svg_simplify_stroke,
                )?;
                Ok(GetScreenshotInput::Png {
                    connection_id: connection_id.unwrap_or(None),
                    selector,
                    scale: scale.unwrap_or(None),
                })
            }
            ScreenshotFormatTag::Jpeg => {
                reject_svg_fields::<A::Error>(
                    svg_outline_text,
                    svg_id_attribute,
                    svg_simplify_stroke,
                )?;
                Ok(GetScreenshotInput::Jpeg {
                    connection_id: connection_id.unwrap_or(None),
                    selector,
                    scale: scale.unwrap_or(None),
                })
            }
            ScreenshotFormatTag::Svg => {
                if scale.is_some() {
                    return Err(A::Error::custom("SVG input cannot contain scale"));
                }
                Ok(GetScreenshotInput::Svg {
                    connection_id: connection_id.unwrap_or(None),
                    selector,
                    svg_outline_text: svg_outline_text.unwrap_or_else(default_true),
                    svg_id_attribute: svg_id_attribute.unwrap_or(false),
                    svg_simplify_stroke: svg_simplify_stroke.unwrap_or_else(default_true),
                })
            }
        }
    }
}

fn set_once<E, T>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: serde::de::Error,
{
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

fn reject_svg_fields<E>(
    outline: Option<bool>,
    id_attribute: Option<bool>,
    simplify_stroke: Option<bool>,
) -> Result<(), E>
where
    E: serde::de::Error,
{
    if outline.is_some() || id_attribute.is_some() || simplify_stroke.is_some() {
        return Err(E::custom("raster input cannot contain SVG options"));
    }
    Ok(())
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RasterSide(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("raster side must not exceed {MAX_RASTER_SIDE}")]
pub struct RasterSideError;

impl RasterSide {
    pub fn value(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for RasterSide {
    type Error = RasterSideError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value > MAX_RASTER_SIDE {
            return Err(RasterSideError);
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for RasterSide {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

impl JsonSchema for RasterSide {
    fn schema_name() -> Cow<'static, str> {
        "RasterSide".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = u32::json_schema(generator);
        if let Some(object) = schema.as_object_mut() {
            object.insert("maximum".to_owned(), MAX_RASTER_SIDE.into());
            object.insert("x-maxRasterSide".to_owned(), MAX_RASTER_SIDE.into());
            object.insert("x-maxRasterPixels".to_owned(), MAX_RASTER_PIXELS.into());
            object.insert(
                "description".to_owned(),
                "Raster dimension; width times height is bounded by x-maxRasterPixels".into(),
            );
        }
        schema
    }
}

fn validate_raster_pixels<E>(width: RasterSide, height: RasterSide) -> Result<(), E>
where
    E: serde::de::Error,
{
    let pixels = u64::from(width.value()) * u64::from(height.value());
    if pixels > MAX_RASTER_PIXELS {
        return Err(E::custom(format_args!(
            "raster pixels {pixels} exceed {MAX_RASTER_PIXELS}"
        )));
    }
    Ok(())
}

// Modelled as a closed discriminant plus an optional name rather than as a
// tagged enum. An adjacently tagged enum would have forced a required `content`
// object on every name-carrying variant, but the plugin drops a name it cannot
// bound, so the name is optional for every rule. A struct closes under
// `deny_unknown_fields` exactly as adjacent tagging does, needs no hand-written
// `Visitor`, and keeps one shape on both ends of the wire.
/// Which safety rule judged an SVG export unsafe.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(
    tag = "format",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScreenshotAsset {
    Png {
        node_id: NodeId,
        data_base64: RasterBase64,
        width: RasterSide,
        height: RasterSide,
    },
    Jpeg {
        node_id: NodeId,
        data_base64: RasterBase64,
        width: RasterSide,
        height: RasterSide,
    },
    /// The source is always present. Safety declares a verdict on it rather
    /// than withholding it, so `safe` is required — an absent boolean reads the
    /// same as `false`, and the caller has to be able to rely on the verdict
    /// having been stated. `rejection` is present exactly when `safe` is false.
    Svg {
        node_id: NodeId,
        source: SvgSource,
        safe: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rejection: Option<SvgRejection>,
    },
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ScreenshotAssetField {
    Format,
    NodeId,
    DataBase64,
    Width,
    Height,
    Source,
    Safe,
    Rejection,
}

impl<'de> Deserialize<'de> for ScreenshotAsset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ScreenshotAssetVisitor)
    }
}

struct ScreenshotAssetVisitor;

impl<'de> Visitor<'de> for ScreenshotAssetVisitor {
    type Value = ScreenshotAsset;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed screenshot asset")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut format = None;
        let mut node_id = None;
        let mut data_base64 = None;
        let mut width = None;
        let mut height = None;
        let mut source = None;
        let mut safe = None;
        let mut rejection = None;
        while let Some(field) = map.next_key::<ScreenshotAssetField>()? {
            match field {
                ScreenshotAssetField::Format => set_once(
                    &mut format,
                    map.next_value::<ScreenshotFormatTag>()?,
                    "format",
                )?,
                ScreenshotAssetField::NodeId => {
                    set_once(&mut node_id, map.next_value::<NodeId>()?, "nodeId")?
                }
                ScreenshotAssetField::DataBase64 => set_once(
                    &mut data_base64,
                    map.next_value::<RasterBase64>()?,
                    "dataBase64",
                )?,
                ScreenshotAssetField::Width => {
                    set_once(&mut width, map.next_value::<RasterSide>()?, "width")?
                }
                ScreenshotAssetField::Height => {
                    set_once(&mut height, map.next_value::<RasterSide>()?, "height")?
                }
                ScreenshotAssetField::Source => {
                    set_once(&mut source, map.next_value::<SvgSource>()?, "source")?
                }
                ScreenshotAssetField::Safe => {
                    set_once(&mut safe, map.next_value::<bool>()?, "safe")?
                }
                ScreenshotAssetField::Rejection => set_once(
                    &mut rejection,
                    map.next_value::<SvgRejection>()?,
                    "rejection",
                )?,
            }
        }
        let node_id = node_id.ok_or_else(|| A::Error::missing_field("nodeId"))?;
        match format.ok_or_else(|| A::Error::missing_field("format"))? {
            ScreenshotFormatTag::Png => {
                if source.is_some() || safe.is_some() || rejection.is_some() {
                    return Err(A::Error::custom(
                        "PNG asset cannot contain source or a safety verdict",
                    ));
                }
                let width = width.ok_or_else(|| A::Error::missing_field("width"))?;
                let height = height.ok_or_else(|| A::Error::missing_field("height"))?;
                validate_raster_pixels::<A::Error>(width, height)?;
                Ok(ScreenshotAsset::Png {
                    node_id,
                    data_base64: data_base64
                        .ok_or_else(|| A::Error::missing_field("dataBase64"))?,
                    width,
                    height,
                })
            }
            ScreenshotFormatTag::Jpeg => {
                if source.is_some() || safe.is_some() || rejection.is_some() {
                    return Err(A::Error::custom(
                        "JPEG asset cannot contain source or a safety verdict",
                    ));
                }
                let width = width.ok_or_else(|| A::Error::missing_field("width"))?;
                let height = height.ok_or_else(|| A::Error::missing_field("height"))?;
                validate_raster_pixels::<A::Error>(width, height)?;
                Ok(ScreenshotAsset::Jpeg {
                    node_id,
                    data_base64: data_base64
                        .ok_or_else(|| A::Error::missing_field("dataBase64"))?,
                    width,
                    height,
                })
            }
            ScreenshotFormatTag::Svg => {
                if data_base64.is_some() || width.is_some() || height.is_some() {
                    return Err(A::Error::custom(
                        "SVG asset cannot contain raster data or dimensions",
                    ));
                }
                let safe = safe.ok_or_else(|| A::Error::missing_field("safe"))?;
                // The verdict and its reason are one fact. A safe asset
                // carrying a rule, or an unsafe one carrying none, is a shape
                // neither end can act on.
                if safe == rejection.is_some() {
                    return Err(A::Error::custom(
                        "rejection is present exactly when safe is false",
                    ));
                }
                Ok(ScreenshotAsset::Svg {
                    node_id,
                    source: source.ok_or_else(|| A::Error::missing_field("source"))?,
                    safe,
                    rejection,
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetScreenshotResult {
    pub assets: ReturnedList<ItemResult<ScreenshotAsset>>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    pub observation: ObservationWindow,
}
