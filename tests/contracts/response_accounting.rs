//! Exact MCP/plugin response envelope accounting.

use figma_dev_mcp_protocol::domain::GetScreenshotResult;
use figma_dev_mcp_protocol::{
    error::{ErrorCode, ToolError},
    limits::{
        MAX_ENVELOPE_BYTES, MAX_IN_FLIGHT, MAX_QUEUE, MAX_RASTER_BASE64_BYTES, MAX_SVG_BYTES,
        MAX_TEXT_BYTES,
    },
};
use figma_dev_mcp_tools::{
    AccountedImage, EnvelopeContext, account_batch_images, account_call_tool_result,
    account_screenshot_result, serialize_mcp_envelope,
};
use rmcp::model::{CallToolResult, ContentBlock, ProtocolVersion, RequestId};
use serde_json::{Value, json};

fn modern_context() -> EnvelopeContext {
    EnvelopeContext {
        request_id: RequestId::Number(7),
        protocol_version: ProtocolVersion::V_2026_07_28,
    }
}

fn legacy_context() -> EnvelopeContext {
    EnvelopeContext {
        request_id: RequestId::Number(7),
        protocol_version: ProtocolVersion::V_2025_11_25,
    }
}

fn observation() -> Value {
    json!({"startedAt": "s", "completedAt": "e"})
}

#[test]
fn rust_and_typescript_share_the_same_resource_constants() {
    assert_eq!(MAX_IN_FLIGHT, 4);
    assert_eq!(MAX_QUEUE, 16);
    assert_eq!(MAX_TEXT_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_ENVELOPE_BYTES, 24 * 1024 * 1024);
}

#[test]
fn utf8_and_json_escaping_count_serialized_bytes() {
    let e_acute = "é";
    assert_eq!(e_acute.len(), 2);
    let quotes = "\"";
    let structured = json!({
        "accent": e_acute.repeat(1_000),
        "quoted": quotes.repeat(1_000),
    });
    let accounted = account_call_tool_result(
        CallToolResult::structured(structured.clone()),
        &modern_context(),
    )
    .expect("small structured result must fit");
    let structured_json = serde_json::to_vec(&structured).unwrap();
    assert!(
        accounted.text_bytes >= structured_json.len() * 2,
        "structuredContent plus compatibility text must both count: {}",
        accounted.text_bytes
    );
    assert!(
        accounted.text_bytes > 1_000 + 1_000,
        "UTF-8 and JSON escaping must expand past raw character counts"
    );
}

#[test]
fn modern_envelope_includes_result_type_and_legacy_omits_it() {
    let result = CallToolResult::structured(json!({"ok": true}));
    let modern = serialize_mcp_envelope(&result, &modern_context()).unwrap();
    let legacy = serialize_mcp_envelope(&result, &legacy_context()).unwrap();
    let modern_value: Value = serde_json::from_slice(&modern).unwrap();
    let legacy_value: Value = serde_json::from_slice(&legacy).unwrap();
    assert_eq!(modern_value["jsonrpc"], "2.0");
    assert_eq!(modern_value["id"], 7);
    assert_eq!(modern_value["result"]["resultType"], "complete");
    assert!(legacy_value["result"].get("resultType").is_none());
    assert!(modern.len() > legacy.len());
}

#[test]
fn eight_mib_ceiling_counts_structured_and_compatibility_text() {
    let payload = "a".repeat(MAX_TEXT_BYTES / 2 + 8);
    let structured = json!({"blob": payload});
    let error = account_call_tool_result(CallToolResult::structured(structured), &modern_context())
        .expect_err("duplicated JSON text must exceed 8 MiB");
    assert_eq!(error.code(), ErrorCode::LimitExceeded);
    assert!(!error.retryable());
}

#[test]
fn twenty_four_mib_envelope_uses_the_real_jsonrpc_wrapper() {
    let image = "B".repeat(MAX_ENVELOPE_BYTES);
    let mut result = CallToolResult::structured(json!({"nodeId": "1:2"}));
    result.content.push(ContentBlock::image(image, "image/png"));
    let error = account_call_tool_result(result, &modern_context())
        .expect_err("oversized wrapper must be rejected");
    assert_eq!(error.code(), ErrorCode::LimitExceeded);
}

#[test]
fn svg_source_is_counted_in_structured_text_and_image() {
    let source = "<svg xmlns='http://www.w3.org/2000/svg'><rect/></svg>";
    let structured = json!({
        "assets": [{
            "status": "success",
            "value": {"format": "svg", "nodeId": "1:2", "source": source, "safe": true}
        }],
        "truncated": false,
        "observation": observation()
    });
    let accounted = account_batch_images(
        &modern_context(),
        vec![AccountedImage {
            id: Some("1:2".into()),
            structured_item: structured["assets"][0]["value"].clone(),
            image_base64: base64_svg(source),
            mime_type: "image/svg+xml".into(),
        }],
        |items| {
            json!({
                "assets": items,
                "truncated": false,
                "observation": observation()
            })
        },
    )
    .expect("small SVG must fit");
    let envelope = serialize_mcp_envelope(&accounted.result, &modern_context()).unwrap();
    let envelope_text = String::from_utf8(envelope).unwrap();
    let occurrences = envelope_text.matches(source).count();
    assert!(
        occurrences >= 2,
        "SVG source must appear in structuredContent and compatibility text: {occurrences}"
    );
    assert!(
        accounted
            .result
            .content
            .iter()
            .any(|block| block.as_image().is_some()),
        "SVG must also be appended as image/svg+xml"
    );
}

#[test]
fn batch_images_preserve_successes_and_never_slice_on_overflow() {
    let small = "AAAA";
    let huge = "C".repeat(MAX_RASTER_BASE64_BYTES.min(MAX_ENVELOPE_BYTES / 2));
    let accounted = account_batch_images(
        &modern_context(),
        vec![
            AccountedImage {
                id: Some("1:1".into()),
                structured_item: json!({"format": "png", "nodeId": "1:1", "dataBase64": small, "width": 1, "height": 1}),
                image_base64: small.into(),
                mime_type: "image/png".into(),
            },
            AccountedImage {
                id: Some("1:2".into()),
                structured_item: json!({"format": "png", "nodeId": "1:2", "dataBase64": huge, "width": 1, "height": 1}),
                image_base64: huge.clone(),
                mime_type: "image/png".into(),
            },
            AccountedImage {
                id: Some("1:3".into()),
                structured_item: json!({"format": "png", "nodeId": "1:3", "dataBase64": small, "width": 1, "height": 1}),
                image_base64: small.into(),
                mime_type: "image/png".into(),
            },
        ],
        |items| {
            json!({
                "assets": items,
                "truncated": false,
                "observation": observation()
            })
        },
    )
    .expect("partial batch must still return a result");

    let assets = accounted.result.structured_content.as_ref().unwrap()["assets"]
        .as_array()
        .unwrap();
    assert_eq!(assets[0]["status"], "success");
    assert_eq!(assets[1]["status"], "error");
    assert_eq!(assets[1]["error"]["code"], "LIMIT_EXCEEDED");
    assert_eq!(assets[2]["status"], "error");
    assert_eq!(assets[2]["error"]["code"], "LIMIT_EXCEEDED");
    assert_eq!(
        accounted
            .result
            .content
            .iter()
            .filter(|block| block.as_image().is_some())
            .count(),
        1
    );
    let image = accounted
        .result
        .content
        .iter()
        .find_map(ContentBlock::as_image);
    assert_eq!(image.unwrap().data, small);
    assert!(!image.unwrap().data.contains(&huge[..32.min(huge.len())]) || huge.starts_with(small));
}

#[test]
fn per_item_ceilings_reject_complete_assets_without_slicing() {
    let oversized_raster = "D".repeat(MAX_RASTER_BASE64_BYTES + 1);
    let oversized_svg = "e".repeat(MAX_SVG_BYTES + 1);
    for (mime, payload) in [
        ("image/png", oversized_raster.as_str()),
        ("image/svg+xml", oversized_svg.as_str()),
    ] {
        let accounted = account_batch_images(
            &modern_context(),
            vec![AccountedImage {
                id: Some("1:9".into()),
                structured_item: json!({"nodeId": "1:9"}),
                image_base64: payload.to_owned(),
                mime_type: mime.into(),
            }],
            |items| json!({"assets": items, "truncated": false, "observation": observation()}),
        )
        .expect("item-level overflow is still a tool result");
        let asset = &accounted.result.structured_content.as_ref().unwrap()["assets"][0];
        assert_eq!(asset["status"], "error");
        assert_eq!(asset["error"]["code"], "LIMIT_EXCEEDED");
        assert!(
            accounted
                .result
                .content
                .iter()
                .all(|block| block.as_image().is_none())
        );
    }
}

#[test]
fn max_svg_source_survives_preview_base64_and_is_charged_once() {
    let prefix = "<svg xmlns='http://www.w3.org/2000/svg'>";
    let suffix = "</svg>";
    let source = format!(
        "{prefix}{}{suffix}",
        "a".repeat(MAX_SVG_BYTES - prefix.len() - suffix.len())
    );
    assert_eq!(source.len(), MAX_SVG_BYTES);
    let preview = base64_svg(&source);
    assert!(
        preview.len() > MAX_SVG_BYTES,
        "preview must expand past the source ceiling so the old check would fail"
    );

    let result: GetScreenshotResult = serde_json::from_value(json!({
        "assets": [{
            "status": "success",
            "value": {"format": "svg", "nodeId": "1:2", "source": source, "safe": true}
        }],
        "truncated": false,
        "observation": observation()
    }))
    .expect("max SVG source must be a legal wire asset");
    let accounted = account_screenshot_result(&modern_context(), result)
        .expect("a 4 MiB SVG source must remain a success");

    let asset = &accounted.result.structured_content.as_ref().unwrap()["assets"][0];
    assert_eq!(asset["status"], "success");
    assert_eq!(asset["value"]["source"], source);
    assert!(accounted.text_bytes <= MAX_TEXT_BYTES);
    assert!(
        accounted.text_bytes >= MAX_SVG_BYTES,
        "unique source bytes must still count once: {}",
        accounted.text_bytes
    );
    let image = accounted
        .result
        .content
        .iter()
        .find_map(ContentBlock::as_image)
        .expect("preview image");
    assert_eq!(image.mime_type, "image/svg+xml");
    assert_eq!(image.data, preview);
    let envelope = serialize_mcp_envelope(&accounted.result, &modern_context()).unwrap();
    let envelope_text = String::from_utf8(envelope).unwrap();
    assert!(
        envelope_text.matches(source.as_str()).count() >= 2,
        "source must still appear in structured content and compatibility text"
    );
}

#[test]
fn plugin_and_rpc_frames_use_the_same_24_mib_check() {
    let huge = json!({"blob": "F".repeat(MAX_ENVELOPE_BYTES + 1)});
    let error = figma_dev_mcp_tools::reject_oversize_frame(&huge)
        .expect_err("oversize plugin/RPC frame must be rejected before enqueue");
    assert_eq!(error.code(), ErrorCode::LimitExceeded);
    let _ok: ToolError;
    figma_dev_mcp_tools::reject_oversize_frame(&json!({"ok": true}))
        .expect("small frame must be accepted");
}

fn base64_svg(source: &str) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = source.as_bytes();
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
