use crate::contracts::public_asset;
use figma_dev_mcp_protocol::{
    domain::{self, ItemResult},
    error::{ErrorCode, ToolError},
    limits::{MAX_ENVELOPE_BYTES, MAX_RASTER_BASE64_BYTES, MAX_SVG_BYTES, MAX_TEXT_BYTES},
};
use rmcp::model::{
    CallToolResult, ContentBlock, JsonRpcResponse, JsonRpcVersion2_0, ProtocolVersion, RequestId,
    ServerResult,
};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct EnvelopeContext {
    pub request_id: RequestId,
    pub protocol_version: ProtocolVersion,
}

impl EnvelopeContext {
    pub fn includes_result_type(&self) -> bool {
        self.protocol_version.as_str() >= ProtocolVersion::V_2026_07_28.as_str()
    }
}

#[derive(Debug, Clone)]
pub struct AccountedCallToolResult {
    pub result: CallToolResult,
    pub text_bytes: usize,
    pub envelope_bytes: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone)]
pub struct AccountedImage {
    pub id: Option<String>,
    pub structured_item: Value,
    pub image_base64: String,
    pub mime_type: String,
}

pub(crate) fn structured(value: Value) -> CallToolResult {
    CallToolResult::structured(value)
}

pub(crate) fn structured_error(value: Value) -> CallToolResult {
    CallToolResult::structured_error(value)
}

pub fn structured_with_image(
    value: Value,
    base64_data: impl Into<String>,
    mime_type: impl Into<String>,
    context: &EnvelopeContext,
) -> Result<CallToolResult, ToolError> {
    let mut result = structured(value);
    result
        .content
        .push(ContentBlock::image(base64_data, mime_type));
    account_call_tool_result(result, context).map(|accounted| accounted.result)
}

pub fn account_screenshot_result(
    context: &EnvelopeContext,
    result: domain::GetScreenshotResult,
) -> Result<AccountedCallToolResult, ToolError> {
    let truncated = result.truncated;
    let truncation = result.truncation.clone();
    let observation = result.observation.clone();
    let mut pending = Vec::with_capacity(result.assets.len());
    for item in result.assets.as_slice() {
        match item {
            ItemResult::Error { error } => pending.push(ScreenshotPending::Error(
                json!({ "status": "error", "error": error }),
            )),
            ItemResult::Success { value } => {
                pending.push(ScreenshotPending::Image(screenshot_image(value)?));
            }
        }
    }
    account_mixed_images(context, pending, move |items| {
        let mut wrapped = json!({
            "assets": items,
            "truncated": truncated,
            "observation": observation,
        });
        if let Some(truncation) = truncation.clone() {
            wrapped["truncation"] = serde_json::to_value(truncation)
                .unwrap_or_else(|_| json!({ "reason": "byteLimit" }));
        }
        wrapped
    })
}

enum ScreenshotPending {
    Image(AccountedImage),
    Error(Value),
}

fn screenshot_image(asset: &domain::ScreenshotAsset) -> Result<AccountedImage, ToolError> {
    let structured_item = serde_json::to_value(public_asset(asset))
        .map_err(|_| ToolError::new(ErrorCode::InternalError, false))?;
    match asset {
        domain::ScreenshotAsset::Png {
            node_id,
            data_base64,
            ..
        } => Ok(AccountedImage {
            id: Some(node_id.to_string()),
            structured_item,
            image_base64: data_base64.as_str().to_owned(),
            mime_type: "image/png".into(),
        }),
        domain::ScreenshotAsset::Jpeg {
            node_id,
            data_base64,
            ..
        } => Ok(AccountedImage {
            id: Some(node_id.to_string()),
            structured_item,
            image_base64: data_base64.as_str().to_owned(),
            mime_type: "image/jpeg".into(),
        }),
        domain::ScreenshotAsset::Svg { node_id, source } => Ok(AccountedImage {
            id: Some(node_id.to_string()),
            structured_item,
            image_base64: encode_base64(source.as_str().as_bytes()),
            mime_type: "image/svg+xml".into(),
        }),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let remaining = bytes.len() - index;
        let b0 = bytes[index];
        let b1 = if remaining > 1 { bytes[index + 1] } else { 0 };
        let b2 = if remaining > 2 { bytes[index + 2] } else { 0 };
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if remaining > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if remaining > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        index += 3;
    }
    out
}

fn account_mixed_images(
    context: &EnvelopeContext,
    items: Vec<ScreenshotPending>,
    wrap: impl Fn(Vec<Value>) -> Value,
) -> Result<AccountedCallToolResult, ToolError> {
    let mut kept = Vec::with_capacity(items.len());
    let mut images: Vec<(String, String)> = Vec::new();
    let mut overflow = false;
    for item in items {
        match item {
            ScreenshotPending::Error(error) if !overflow => {
                kept.push(error);
            }
            ScreenshotPending::Error(error) => kept.push(error),
            ScreenshotPending::Image(item) => {
                if overflow {
                    kept.push(item_error(item.id.as_deref()));
                    continue;
                }
                if !screenshot_item_allowed(&item) {
                    kept.push(item_error(item.id.as_deref()));
                    continue;
                }
                let mut tentative_items = kept.clone();
                tentative_items.push(item_success(item.structured_item.clone()));
                let mut tentative = structured(wrap(tentative_items.clone()));
                for (data, mime) in &images {
                    tentative
                        .content
                        .push(ContentBlock::image(data.clone(), mime.clone()));
                }
                tentative.content.push(ContentBlock::image(
                    item.image_base64.clone(),
                    item.mime_type.clone(),
                ));
                match account_screenshot_call_tool_result(tentative, context) {
                    Ok(_) => {
                        kept = tentative_items;
                        images.push((item.image_base64, item.mime_type));
                    }
                    Err(_) => {
                        kept.push(item_error(item.id.as_deref()));
                        overflow = true;
                    }
                }
            }
        }
    }
    let mut result = structured(wrap(kept));
    for (data, mime) in images {
        result.content.push(ContentBlock::image(data, mime));
    }
    account_screenshot_call_tool_result(result, context)
}

fn screenshot_item_allowed(item: &AccountedImage) -> bool {
    if item.mime_type == "image/svg+xml" {
        let source_len = item
            .structured_item
            .get("source")
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or(item.image_base64.len());
        return source_len <= MAX_SVG_BYTES;
    }
    item.image_base64.len() <= MAX_RASTER_BASE64_BYTES
}

fn duplicated_svg_source_bytes(result: &CallToolResult) -> usize {
    let Some(value) = result.structured_content.as_ref() else {
        return 0;
    };
    let Some(assets) = value.get("assets").and_then(Value::as_array) else {
        return 0;
    };
    assets
        .iter()
        .filter_map(|asset| {
            asset
                .get("value")
                .and_then(|value| value.get("source"))
                .and_then(Value::as_str)
                .map(str::len)
        })
        .sum()
}

fn account_screenshot_call_tool_result(
    result: CallToolResult,
    context: &EnvelopeContext,
) -> Result<AccountedCallToolResult, ToolError> {
    let text_bytes =
        non_image_text_bytes(&result).saturating_sub(duplicated_svg_source_bytes(&result));
    if text_bytes > MAX_TEXT_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    let envelope = serialize_mcp_envelope(&result, context)?;
    if envelope.len() > MAX_ENVELOPE_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    Ok(AccountedCallToolResult {
        item_count: item_count(&result),
        result,
        text_bytes,
        envelope_bytes: envelope.len(),
    })
}

pub fn serialize_mcp_envelope(
    result: &CallToolResult,
    context: &EnvelopeContext,
) -> Result<Vec<u8>, ToolError> {
    let mut server_result = ServerResult::CallToolResult(result.clone());
    if !context.includes_result_type() {
        server_result.strip_result_type_for_legacy_peer();
    }
    let response = JsonRpcResponse {
        jsonrpc: JsonRpcVersion2_0,
        id: context.request_id.clone(),
        result: server_result,
    };
    serde_json::to_vec(&response).map_err(|_| ToolError::new(ErrorCode::InternalError, false))
}

pub fn reject_oversize_frame<T: Serialize>(value: &T) -> Result<usize, ToolError> {
    let encoded =
        serde_json::to_vec(value).map_err(|_| ToolError::new(ErrorCode::InternalError, false))?;
    if encoded.len() > MAX_ENVELOPE_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    Ok(encoded.len())
}

pub fn account_call_tool_result(
    result: CallToolResult,
    context: &EnvelopeContext,
) -> Result<AccountedCallToolResult, ToolError> {
    let text_bytes = non_image_text_bytes(&result);
    if text_bytes > MAX_TEXT_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    let envelope = serialize_mcp_envelope(&result, context)?;
    if envelope.len() > MAX_ENVELOPE_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    Ok(AccountedCallToolResult {
        item_count: item_count(&result),
        result,
        text_bytes,
        envelope_bytes: envelope.len(),
    })
}

pub fn account_batch_images(
    context: &EnvelopeContext,
    items: Vec<AccountedImage>,
    wrap: impl Fn(Vec<Value>) -> Value,
) -> Result<AccountedCallToolResult, ToolError> {
    let mut kept = Vec::with_capacity(items.len());
    let mut images: Vec<(String, String)> = Vec::new();
    let mut overflow = false;
    for item in items {
        if overflow {
            kept.push(item_error(item.id.as_deref()));
            continue;
        }
        if !per_item_allowed(&item) {
            kept.push(item_error(item.id.as_deref()));
            continue;
        }
        let mut tentative_items = kept.clone();
        tentative_items.push(item_success(item.structured_item.clone()));
        let mut tentative = structured(wrap(tentative_items.clone()));
        for (data, mime) in &images {
            tentative
                .content
                .push(ContentBlock::image(data.clone(), mime.clone()));
        }
        tentative.content.push(ContentBlock::image(
            item.image_base64.clone(),
            item.mime_type.clone(),
        ));
        match account_call_tool_result(tentative, context) {
            Ok(_) => {
                kept = tentative_items;
                images.push((item.image_base64, item.mime_type));
            }
            Err(_) => {
                kept.push(item_error(item.id.as_deref()));
                overflow = true;
            }
        }
    }
    let mut result = structured(wrap(kept));
    for (data, mime) in images {
        result.content.push(ContentBlock::image(data, mime));
    }
    account_call_tool_result(result, context)
}

fn per_item_allowed(item: &AccountedImage) -> bool {
    if item.mime_type == "image/svg+xml" {
        return item.image_base64.len() <= MAX_SVG_BYTES;
    }
    item.image_base64.len() <= MAX_RASTER_BASE64_BYTES
}

fn item_success(value: Value) -> Value {
    json!({ "status": "success", "value": value })
}

fn item_error(id: Option<&str>) -> Value {
    let mut error = json!({
        "status": "error",
        "error": {
            "code": "LIMIT_EXCEEDED",
            "message": "The operation exceeded a safety limit.",
            "retryable": false
        }
    });
    if let Some(id) = id {
        error["error"]["id"] = json!(id);
    }
    error
}

fn non_image_text_bytes(result: &CallToolResult) -> usize {
    let structured = result
        .structured_content
        .as_ref()
        .and_then(|value| serde_json::to_vec(value).ok())
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    let content = result
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text(text) => text.text.len(),
            ContentBlock::Image(_) | ContentBlock::Audio(_) => 0,
            other => serde_json::to_vec(other)
                .map(|bytes| bytes.len())
                .unwrap_or(0),
        })
        .sum::<usize>();
    structured + content
}

fn item_count(result: &CallToolResult) -> usize {
    let Some(value) = result.structured_content.as_ref() else {
        return result.content.len().max(1);
    };
    for key in [
        "items", "assets", "matches", "nodes", "roots", "files", "pages",
    ] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            return items.len();
        }
    }
    1
}
