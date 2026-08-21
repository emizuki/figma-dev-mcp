use figma_dev_mcp_protocol::domain::{
    self, ItemResult, NodeId, ObservationWindow, RasterSide, ReturnedList, SvgRejection, SvgSource,
    Truncation,
};
use figma_dev_mcp_protocol::limits::{MAX_RASTER_BASE64_BYTES, MAX_RASTER_DECODED_BYTES};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct GetScreenshotInput(domain::GetScreenshotInput);

impl GetScreenshotInput {
    pub(crate) fn into_protocol(self) -> domain::GetScreenshotInput {
        self.0
    }
}

impl JsonSchema for GetScreenshotInput {
    fn schema_name() -> Cow<'static, str> {
        "GetScreenshotInput".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = domain::GetScreenshotInput::json_schema(generator);
        flatten_format_variants(&mut schema);
        schema
    }
}

/// The domain input is an enum tagged by `format`, so schemars renders it as a
/// root `oneOf`. The Anthropic API rejects a top-level combinator and Claude
/// Code drops the tool entirely, so the variants are merged into one object
/// whose `format` is a plain string enum. This only widens what the schema
/// advertises: `domain::GetScreenshotInput` still deserializes per format and
/// rejects a field that belongs to a different one. The tool description
/// carries the pairing the schema no longer states.
fn flatten_format_variants(schema: &mut Schema) {
    let object = schema.ensure_object();
    let Some(Value::Array(variants)) = object.remove("oneOf") else {
        return;
    };
    let mut properties = Map::new();
    let mut formats = Vec::new();
    let mut required: Option<Vec<Value>> = None;
    for variant in &variants {
        let Some(variant) = variant.as_object() else {
            continue;
        };
        if let Some(fields) = variant.get("properties").and_then(Value::as_object) {
            for (name, field) in fields {
                if name == "format" {
                    if let Some(format) = field.get("const") {
                        formats.push(format.clone());
                    }
                    continue;
                }
                properties.entry(name.clone()).or_insert(field.clone());
            }
        }
        // Only what every variant demands can be required once they are merged.
        let names = variant
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        required = Some(match required {
            None => names,
            Some(shared) => shared
                .into_iter()
                .filter(|name| names.contains(name))
                .collect(),
        });
    }
    properties.insert(
        "format".to_owned(),
        json!({ "type": "string", "enum": formats }),
    );
    object.insert("type".to_owned(), "object".into());
    object.insert("additionalProperties".to_owned(), false.into());
    object.insert("properties".to_owned(), Value::Object(properties));
    object.insert(
        "required".to_owned(),
        Value::Array(required.unwrap_or_default()),
    );
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "format",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ScreenshotAsset {
    Png {
        node_id: NodeId,
        width: RasterSide,
        height: RasterSide,
        #[schemars(range(min = 0, max = 12_582_912))]
        decoded_bytes: u32,
        #[schemars(range(min = 0, max = 16_777_216))]
        base64_bytes: u32,
    },
    Jpeg {
        node_id: NodeId,
        width: RasterSide,
        height: RasterSide,
        #[schemars(range(min = 0, max = 12_582_912))]
        decoded_bytes: u32,
        #[schemars(range(min = 0, max = 16_777_216))]
        base64_bytes: u32,
    },
    /// The source is always returned. `safe` states the safety verdict rather
    /// than withholding the source, and `rejection` names the rule that fired,
    /// present exactly when `safe` is false.
    Svg {
        node_id: NodeId,
        source: SvgSource,
        safe: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rejection: Option<SvgRejection>,
    },
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

pub(crate) fn public_asset(asset: &domain::ScreenshotAsset) -> ScreenshotAsset {
    match asset {
        domain::ScreenshotAsset::Png {
            node_id,
            data_base64,
            width,
            height,
        } => ScreenshotAsset::Png {
            node_id: node_id.clone(),
            width: *width,
            height: *height,
            decoded_bytes: decoded_len(data_base64.as_str()),
            base64_bytes: u32::try_from(data_base64.as_str().len()).unwrap_or(u32::MAX),
        },
        domain::ScreenshotAsset::Jpeg {
            node_id,
            data_base64,
            width,
            height,
        } => ScreenshotAsset::Jpeg {
            node_id: node_id.clone(),
            width: *width,
            height: *height,
            decoded_bytes: decoded_len(data_base64.as_str()),
            base64_bytes: u32::try_from(data_base64.as_str().len()).unwrap_or(u32::MAX),
        },
        domain::ScreenshotAsset::Svg {
            node_id,
            source,
            safe,
            rejection,
        } => ScreenshotAsset::Svg {
            node_id: node_id.clone(),
            source: source.clone(),
            safe: *safe,
            rejection: rejection.clone(),
        },
    }
}

pub(crate) fn decoded_len(base64: &str) -> u32 {
    let padding = base64
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    u32::try_from((base64.len() * 3 / 4).saturating_sub(padding)).unwrap_or(u32::MAX)
}

const _: () = {
    assert!(MAX_RASTER_DECODED_BYTES == 12_582_912);
    assert!(MAX_RASTER_BASE64_BYTES == 16_777_216);
};
