use figma_dev_mcp_protocol::domain::{
    self, ItemResult, NodeId, ObservationWindow, RasterSide, ReturnedList, SvgSource, Truncation,
};
use figma_dev_mcp_protocol::limits::{MAX_RASTER_BASE64_BYTES, MAX_RASTER_DECODED_BYTES};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
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
        let object = schema.ensure_object();
        object.insert("type".to_owned(), "object".into());
        schema
    }
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
    Svg {
        node_id: NodeId,
        source: SvgSource,
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
        domain::ScreenshotAsset::Svg { node_id, source } => ScreenshotAsset::Svg {
            node_id: node_id.clone(),
            source: source.clone(),
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
