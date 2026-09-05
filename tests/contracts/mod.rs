//! Contract tests.

mod allocation;
mod prompts_catalog;
mod resources_catalog;
mod response_accounting;
mod structured_outputs;
mod tools_catalog;

use figma_dev_mcp_broker::PLUGIN_PROTOCOL_VERSION;
use figma_dev_mcp_protocol::{
    domain::{
        AxisAlign, ComponentValue, ConnectionId, CornerRadiusValue, DesignNode,
        GetDesignContextResult, GetDevModeDataResult, GetMotionResult, GetNodesResult,
        GetReactionsResult, GetSelectionResult, InstanceValue, ItemIdentifier, LayoutValue,
        LetterSpacingValue, LineHeightValue, MinimalNodeDetails, NodeForest, NodeId, NodeTypeList,
        NodeTypeName, NodesSelector, PageId, PagesSelector, PaintValue, RasterScale,
        ReactionAction, RequestId, ReturnedList, ScreenshotAsset, SearchNodesInput, Selector,
        StrokeAlign, StrokeValue, StyleKind, StyleReference, SvgRejectionKind, TextStyle,
    },
    error::{ErrorCode, ItemError, PluginFailure, ToolError, canonical_message},
    limits::{
        HEARTBEAT_SECS, IDLE_GRACE_SECS, INACTIVITY_TIMEOUT_SECS, MAX_DEPTH,
        MAX_DISPLAY_TEXT_BYTES, MAX_ENVELOPE_BYTES, MAX_IDENTIFIER_BYTES, MAX_IN_FLIGHT,
        MAX_INPUT_IDS, MAX_PAGE_IDS, MAX_QUERY_BYTES, MAX_QUEUE, MAX_RASTER_BASE64_BYTES,
        MAX_RASTER_DECODED_BYTES, MAX_RASTER_PIXELS, MAX_RASTER_SIDE, MAX_RETURNED_NODES,
        MAX_SVG_BYTES, MAX_TEXT_BYTES, MAX_VISITED_NODES, STALE_SESSION_SECS, TOTAL_TIMEOUT_SECS,
    },
    rpc::{FrontendToLeader, LeaderToFrontend, RpcRequestId, decode_frame, encode_frame},
    wire::{
        BrokerCall, BrokerToPlugin, Hello, PluginToBroker, ReadOperation, ReadResult, SelectionFlag,
    },
};
use serde::de::{
    IntoDeserializer,
    value::{Error as ValueError, MapDeserializer, SeqDeserializer, StrDeserializer},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::io::Cursor;
use std::{cell::Cell, rc::Rc};

const FIXTURES: [&str; 5] = [
    include_str!("fixtures/hello.json"),
    include_str!("fixtures/get_metadata_request.json"),
    include_str!("fixtures/get_metadata_response.json"),
    include_str!("fixtures/error.json"),
    include_str!("fixtures/cancel.json"),
];

fn assert_round_trip<T>(fixture: &str)
where
    T: DeserializeOwned + Serialize,
{
    let expected: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
    let decoded: T = serde_json::from_value(expected.clone()).expect("fixture decodes");
    let actual = serde_json::to_value(decoded).expect("fixture re-encodes");
    assert_eq!(actual, expected);
}

#[test]
fn checked_in_wire_fixtures_round_trip_without_shape_drift() {
    assert_eq!(FIXTURES.len(), 5);
    assert_round_trip::<PluginToBroker>(FIXTURES[0]);
    assert_round_trip::<BrokerToPlugin>(FIXTURES[1]);
    assert_round_trip::<PluginToBroker>(FIXTURES[2]);
    assert_round_trip::<PluginToBroker>(FIXTURES[3]);
    assert_round_trip::<BrokerToPlugin>(FIXTURES[4]);
}

#[test]
fn hello_carries_explicit_session_identity_and_fresh_display_metadata() {
    let hello: Value = serde_json::from_str(FIXTURES[0]).unwrap();
    assert_eq!(
        hello["connectionId"],
        "123e4567-e89b-42d3-a456-426614174000"
    );
    assert_eq!(hello["displayName"], "Quynh's Figma");
    assert_eq!(hello["currentPage"]["id"], "0:1");
    assert_eq!(hello["currentPage"]["name"], "Checkout flow");
    assert_eq!(hello["pluginVersion"], "0.1.0");

    let schema = serde_json::to_value(schemars::schema_for!(Hello))
        .unwrap()
        .to_string();
    for field in ["connectionId", "currentPage", "pluginVersion"] {
        assert!(schema.contains(field), "Hello schema is missing {field}");
    }
}

#[test]
fn wire_message_tags_are_closed_camel_case_and_exact() {
    let plugin_tags = ["hello", "progress", "response", "error", "pong"];
    let broker_tags = ["request", "cancel", "ping"];
    assert_eq!(plugin_tags.len() + broker_tags.len(), 8);

    for tag in plugin_tags {
        let value = match tag {
            "hello" => serde_json::from_str::<Value>(FIXTURES[0]).unwrap(),
            "response" => serde_json::from_str::<Value>(FIXTURES[2]).unwrap(),
            "error" => serde_json::from_str::<Value>(FIXTURES[3]).unwrap(),
            "progress" => json!({
                "type": "progress", "requestId": "plugin-1", "completed": 1,
                "total": 2, "message": "Reading nodes"
            }),
            "pong" => json!({"type": "pong", "nonce": 7}),
            _ => unreachable!(),
        };
        let decoded: PluginToBroker = serde_json::from_value(value).unwrap();
        assert_eq!(plugin_to_broker_tag(&decoded), tag);
        assert_eq!(serde_json::to_value(decoded).unwrap()["type"], tag);
    }

    for tag in broker_tags {
        let value = match tag {
            "request" => serde_json::from_str::<Value>(FIXTURES[1]).unwrap(),
            "cancel" => serde_json::from_str::<Value>(FIXTURES[4]).unwrap(),
            "ping" => json!({"type": "ping", "nonce": 7}),
            _ => unreachable!(),
        };
        let decoded: BrokerToPlugin = serde_json::from_value(value).unwrap();
        assert_eq!(broker_to_plugin_tag(&decoded), tag);
        assert_eq!(serde_json::to_value(decoded).unwrap()["type"], tag);
    }

    assert!(serde_json::from_value::<PluginToBroker>(json!({"type": "unknown"})).is_err());
    assert!(serde_json::from_value::<BrokerToPlugin>(json!({"type": "unknown"})).is_err());
    let mut hello: Value = serde_json::from_str(FIXTURES[0]).unwrap();
    hello["unexpected"] = json!(true);
    assert!(serde_json::from_value::<PluginToBroker>(hello).is_err());
}

#[test]
fn read_operation_and_result_tags_are_closed_and_exact() {
    let operation_tags = [
        "get_metadata",
        "get_selection",
        "get_nodes",
        "search_nodes",
        "get_design_context",
        "get_styles",
        "get_variables",
        "get_components",
        "get_fonts",
        "get_dev_mode_data",
        "get_reactions",
        "get_motion",
        "get_screenshot",
    ];
    assert_eq!(operation_tags.len(), 13);

    let inputs = [
        json!({}),
        json!({}),
        json!({"nodeIds": []}),
        json!({"scope": {"pageId": "0:1"}, "query": "Card", "match": "contains", "limit": 50}),
        json!({}),
        json!({}),
        json!({}),
        json!({}),
        json!({}),
        json!({}),
        json!({}),
        json!({}),
        json!({"format": "png", "selector": {"nodeId": "1:2"}}),
    ];
    for (tag, input) in operation_tags.into_iter().zip(inputs) {
        let operation: ReadOperation = serde_json::from_value(json!({
            "operation": tag,
            "input": input
        }))
        .unwrap_or_else(|error| panic!("{tag} must decode: {error}"));
        assert_eq!(read_operation_name(&operation), tag);
        assert_eq!(serde_json::to_value(operation).unwrap()["operation"], tag);
    }

    let request: BrokerToPlugin = serde_json::from_str(FIXTURES[1]).unwrap();
    let response: PluginToBroker = serde_json::from_str(FIXTURES[2]).unwrap();
    assert!(matches!(
        request,
        BrokerToPlugin::Request(ref request)
            if matches!(request.operation, ReadOperation::GetMetadata(_))
    ));
    assert!(matches!(
        response,
        PluginToBroker::Response(ref response)
            if matches!(response.result, ReadResult::GetMetadata(_))
    ));

    let response: PluginToBroker = serde_json::from_str(FIXTURES[2]).unwrap();
    let PluginToBroker::Response(response) = response else {
        panic!("fixture must be a response");
    };
    assert_eq!(read_result_name(&response.result), "get_metadata");
    let result_schema = serde_json::to_value(schemars::schema_for!(ReadResult)).unwrap();
    let result_schema = result_schema.to_string();
    for tag in operation_tags {
        assert!(
            result_schema.contains(tag),
            "result schema is missing {tag}"
        );
    }
    assert!(
        !result_schema.contains("get_css"),
        "removed get_css result must not remain in the schema"
    );
    assert!(
        !result_schema.contains("get_tokens"),
        "removed get_tokens result must not remain in the schema"
    );

    assert!(
        serde_json::from_value::<ReadOperation>(json!({
            "operation": "get_css",
            "input": {}
        }))
        .is_err(),
        "removed get_css wire operation must be rejected"
    );

    assert!(
        serde_json::from_value::<ReadOperation>(json!({
            "operation": "get_tokens",
            "input": {}
        }))
        .is_err(),
        "removed get_tokens wire operation must be rejected"
    );

    let mut unknown: Value = serde_json::from_str(FIXTURES[1]).unwrap();
    unknown["operation"] = json!({"operation": "run_arbitrary_code", "input": {}});
    assert!(serde_json::from_value::<BrokerToPlugin>(unknown).is_err());
}

fn read_operation_name(operation: &ReadOperation) -> &'static str {
    match operation {
        ReadOperation::GetMetadata(_) => "get_metadata",
        ReadOperation::GetSelection(_) => "get_selection",
        ReadOperation::GetNodes(_) => "get_nodes",
        ReadOperation::SearchNodes(_) => "search_nodes",
        ReadOperation::GetDesignContext(_) => "get_design_context",
        ReadOperation::GetStyles(_) => "get_styles",
        ReadOperation::GetVariables(_) => "get_variables",
        ReadOperation::GetComponents(_) => "get_components",
        ReadOperation::GetFonts(_) => "get_fonts",
        ReadOperation::GetDevModeData(_) => "get_dev_mode_data",
        ReadOperation::GetReactions(_) => "get_reactions",
        ReadOperation::GetMotion(_) => "get_motion",
        ReadOperation::GetScreenshot(_) => "get_screenshot",
    }
}

fn read_result_name(result: &ReadResult) -> &'static str {
    match result {
        ReadResult::GetMetadata(_) => "get_metadata",
        ReadResult::GetSelection(_) => "get_selection",
        ReadResult::GetNodes(_) => "get_nodes",
        ReadResult::SearchNodes(_) => "search_nodes",
        ReadResult::GetDesignContext(_) => "get_design_context",
        ReadResult::GetStyles(_) => "get_styles",
        ReadResult::GetVariables(_) => "get_variables",
        ReadResult::GetComponents(_) => "get_components",
        ReadResult::GetFonts(_) => "get_fonts",
        ReadResult::GetDevModeData(_) => "get_dev_mode_data",
        ReadResult::GetReactions(_) => "get_reactions",
        ReadResult::GetMotion(_) => "get_motion",
        ReadResult::GetScreenshot(_) => "get_screenshot",
    }
}

/// The wire order of the stable error codes. Mirrored by `ERROR_CODES` in
/// `plugin/src/shared/protocol.ts`; the mirror test below checks that this list
/// is still the set the enum declares, so a member added to `ErrorCode` and not
/// added here fails rather than going unnoticed.
pub const ERROR_CODES: [ErrorCode; 16] = [
    ErrorCode::NoFigmaConnection,
    ErrorCode::AmbiguousConnection,
    ErrorCode::ConnectionNotFound,
    ErrorCode::ConnectionLost,
    ErrorCode::ProtocolMismatch,
    ErrorCode::NodeNotFound,
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

#[test]
fn stable_error_codes_are_exact_and_screaming_snake_case() {
    let encoded: Vec<String> = ERROR_CODES
        .iter()
        .map(|code| {
            let expected = error_code_tag(*code);
            assert_eq!(serde_json::to_value(code).unwrap(), expected);
            expected.to_owned()
        })
        .collect();
    assert_eq!(
        encoded,
        [
            "NO_FIGMA_CONNECTION",
            "AMBIGUOUS_CONNECTION",
            "CONNECTION_NOT_FOUND",
            "CONNECTION_LOST",
            "PROTOCOL_MISMATCH",
            "NODE_NOT_FOUND",
            "PAGE_NOT_FOUND",
            "UNSUPPORTED_NODE",
            "EMPTY_NODE_BOUNDS",
            "CAPABILITY_UNAVAILABLE",
            "UNSAFE_SVG",
            "INVALID_CURSOR",
            "LIMIT_EXCEEDED",
            "TIMEOUT",
            "CANCELLED",
            "INTERNAL_ERROR",
        ]
    );
    assert!(serde_json::from_str::<ErrorCode>("\"NOT_A_REAL_CODE\"").is_err());

    let error: ToolError = serde_json::from_str(
        r#"{"code":"TIMEOUT","message":"The operation timed out.","retryable":true}"#,
    )
    .unwrap();
    let encoded = serde_json::to_value(error).unwrap();
    assert!(encoded.get("source").is_none());
    assert!(encoded.get("pluginPayload").is_none());
}

/// Exhaustive by construction: a new rule fails to compile here until it is
/// given a wire tag, which is the same tag the plugin must mirror.
pub fn svg_rejection_kind_tag(kind: SvgRejectionKind) -> &'static str {
    match kind {
        SvgRejectionKind::ParserError => "parserError",
        SvgRejectionKind::UnsafeElement => "unsafeElement",
        SvgRejectionKind::UnsafeAttribute => "unsafeAttribute",
        SvgRejectionKind::UnsafeCss => "unsafeCss",
        SvgRejectionKind::UnsafeProcessingInstruction => "unsafeProcessingInstruction",
    }
}

pub const SVG_REJECTION_KINDS: [SvgRejectionKind; 5] = [
    SvgRejectionKind::ParserError,
    SvgRejectionKind::UnsafeElement,
    SvgRejectionKind::UnsafeAttribute,
    SvgRejectionKind::UnsafeCss,
    SvgRejectionKind::UnsafeProcessingInstruction,
];

const VERDICT_SOURCE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";

fn unsafe_svg_asset(rejection: Value) -> Value {
    json!({
        "format": "svg",
        "nodeId": "1:2",
        "source": VERDICT_SOURCE,
        "safe": false,
        "rejection": rejection,
    })
}

#[test]
fn every_svg_rejection_rule_round_trips_named_and_unnamed() {
    for kind in SVG_REJECTION_KINDS {
        let tag = svg_rejection_kind_tag(kind);
        for name in [None, Some("href")] {
            let mut rejection = json!({ "kind": tag });
            if let Some(name) = name {
                rejection["name"] = json!(name);
            }
            let encoded = unsafe_svg_asset(rejection);
            let asset: ScreenshotAsset = serde_json::from_value(encoded.clone()).unwrap();
            let ScreenshotAsset::Svg {
                source,
                safe,
                rejection,
                ..
            } = &asset
            else {
                panic!("{tag} must decode as an SVG asset");
            };
            assert!(!safe, "{tag} must decode as an unsafe verdict");
            assert_eq!(
                source.as_str(),
                VERDICT_SOURCE,
                "{tag} must return the source it judged"
            );
            let decoded = rejection.as_ref().expect("rule survives decoding");
            assert_eq!(decoded.kind(), kind);
            assert_eq!(
                decoded.name().map(|value| value.as_str()),
                name,
                "{tag} must keep its offender name exactly as sent"
            );
            assert_eq!(
                serde_json::to_value(&asset).unwrap(),
                encoded,
                "{tag} must re-encode byte for byte"
            );
        }
    }

    let safe_asset = json!({
        "format": "svg", "nodeId": "1:2", "source": VERDICT_SOURCE, "safe": true
    });
    let asset: ScreenshotAsset = serde_json::from_value(safe_asset.clone()).unwrap();
    let ScreenshotAsset::Svg {
        safe, rejection, ..
    } = &asset
    else {
        panic!("a safe verdict must decode as an SVG asset");
    };
    assert!(safe);
    assert!(rejection.is_none());
    assert_eq!(
        serde_json::to_value(&asset).unwrap(),
        safe_asset,
        "an absent rule must not materialise as null"
    );
}

#[test]
fn an_svg_verdict_is_stated_and_matches_its_rule() {
    // `safe` is required: an absent boolean reads the same as `false`, and the
    // caller has to be able to rely on the verdict having been stated.
    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "svg", "nodeId": "1:2", "source": VERDICT_SOURCE
        }))
        .is_err(),
        "an SVG asset must state its verdict"
    );
    // The verdict and its reason are one fact, so neither half may travel alone.
    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "svg", "nodeId": "1:2", "source": VERDICT_SOURCE, "safe": false
        }))
        .is_err(),
        "an unsafe verdict must name the rule that fired"
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "svg", "nodeId": "1:2", "source": VERDICT_SOURCE, "safe": true,
            "rejection": { "kind": "unsafeElement", "name": "script" }
        }))
        .is_err(),
        "a safe verdict cannot carry a rule"
    );
    // A verdict belongs to SVG alone; a raster asset was never judged.
    for format in ["png", "jpeg"] {
        assert!(
            serde_json::from_value::<ScreenshotAsset>(json!({
                "format": format, "nodeId": "1:2", "dataBase64": "AA==",
                "width": 1, "height": 1, "safe": true
            }))
            .is_err(),
            "{format} asset must not carry a safety verdict"
        );
    }
}

#[test]
fn svg_rejection_rules_stay_closed() {
    // An unknown rule must be refused, not ignored: a variant only one end
    // knows about would drop the session rather than one request.
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(
            json!({ "kind": "unsafeFont" })
        ))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(
            json!({ "kind": "parsererror" })
        ))
        .is_err()
    );
    assert!(serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(json!({}))).is_err());
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(
            json!({ "kind": "unsafeElement", "value": "fill:#ff0000" })
        ))
        .is_err(),
        "the rule carries names, never values"
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(
            json!({ "kind": "unsafeElement", "name": "a".repeat(MAX_IDENTIFIER_BYTES + 1) })
        ))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(json!({
            "kind": "unsafeElement", "name": ""
        })))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(unsafe_svg_asset(json!("unsafeElement")))
            .is_err(),
        "the rule is an object, not a bare tag"
    );
    // `deny_unknown_fields` must still refuse an unknown field on the asset
    // itself, not only inside the rule it now carries.
    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "svg", "nodeId": "1:2", "source": VERDICT_SOURCE, "safe": true,
            "verdict": "safe"
        }))
        .is_err(),
        "an unknown field on the asset must be refused"
    );
}

#[test]
fn the_plugin_mirrors_every_svg_rejection_rule() {
    // A rule present on one end and absent on the other drops the whole broker
    // session, not one request, so the two ends are pinned to each other in
    // BOTH directions: a plugin-only sixth kind would otherwise go uncaught.
    let plugin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate has a workspace parent")
        .join("plugin/src/shared");
    let results = std::fs::read_to_string(plugin.join("results.ts")).unwrap();
    let validation = std::fs::read_to_string(plugin.join("result-validation.ts")).unwrap();

    let mut rust: Vec<String> = SVG_REJECTION_KINDS
        .iter()
        .map(|kind| svg_rejection_kind_tag(*kind).to_owned())
        .collect();
    rust.sort();

    let declared = quoted_tags(&results, "export type SvgRejectionKind =", "\n\n");
    let accepted = quoted_tags(
        &validation,
        "const SVG_REJECTION_KINDS: readonly SvgRejectionKind[] = [",
        "]",
    );
    assert_eq!(
        declared, rust,
        "plugin results.ts declares a different set of SVG rejection rules than Rust"
    );
    assert_eq!(
        accepted, rust,
        "plugin result-validation.ts accepts a different set of SVG rejection rules than Rust"
    );
    assert!(
        results.contains("rejection?: SvgRejection") && validation.contains("\"rejection\""),
        "plugin must carry the rule on its SVG asset"
    );
    assert!(
        !results.contains("svgRejection") && !validation.contains("svgRejection"),
        "the rule no longer travels on the tool error"
    );
}

/// The sorted, deduplicated double-quoted strings in the plugin source between
/// `start` and the first `end` after it. Reading the plugin's own lists is what
/// makes the mirror bidirectional: a kind only the plugin knows shows up here.
fn quoted_tags(source: &str, start: &str, end: &str) -> Vec<String> {
    let begin = source
        .find(start)
        .unwrap_or_else(|| panic!("plugin source must contain {start}"))
        + start.len();
    let rest = &source[begin..];
    let stop = rest
        .find(end)
        .unwrap_or_else(|| panic!("plugin source must terminate {start}"));
    sorted_quoted(&rest[..stop])
}

/// The sorted, deduplicated double-quoted strings in one slice of plugin source.
fn sorted_quoted(segment: &str) -> Vec<String> {
    let mut tags: Vec<String> = segment
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// The two halves of the plugin's `ERROR_CODES` declaration: the tuple type
/// that gives `ErrorCode` its members, and the value list the validator checks
/// an incoming code against. The plugin writes the sixteen strings out twice
/// and nothing else pins the two copies to each other.
fn plugin_error_code_lists(source: &str) -> (Vec<String>, Vec<String>) {
    const START: &str = "export const ERROR_CODES: readonly [";
    const SPLIT: &str = "] = [";
    let begin = source
        .find(START)
        .expect("plugin protocol.ts must declare ERROR_CODES")
        + START.len();
    let rest = &source[begin..];
    let split = rest
        .find(SPLIT)
        .expect("the ERROR_CODES tuple type must terminate");
    let values = &rest[split + SPLIT.len()..];
    let end = values
        .find("\n]")
        .expect("the ERROR_CODES value list must terminate");
    (sorted_quoted(&rest[..split]), sorted_quoted(&values[..end]))
}

/// The `CODE: "message"` pairs of a `Record<ErrorCode, string>` literal, sorted.
/// Pairs rather than two sets: a message swapped between two codes leaves both
/// sets identical and is still a dropped session.
fn plugin_message_map(source: &str, start: &str) -> Vec<(String, String)> {
    let begin = source
        .find(start)
        .unwrap_or_else(|| panic!("plugin source must contain {start}"))
        + start.len();
    let rest = &source[begin..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("plugin source must terminate {start}"));
    let mut pairs: Vec<(String, String)> = rest[..end]
        .lines()
        .filter_map(|line| {
            let (code, tail) = line.split_once(':')?;
            let message = tail.split('"').nth(1)?;
            Some((code.trim().to_owned(), message.to_owned()))
        })
        .collect();
    pairs.sort();
    pairs
}

/// Every `ErrorCode` tag, read out of the derived schema rather than a list
/// written by hand. A member added to the enum turns up here whether or not
/// anyone remembers the copies, which is what makes the mirror below catch a
/// Rust-only member instead of silently comparing two stale lists.
fn schema_error_codes() -> Vec<String> {
    let schema = serde_json::to_value(schemars::schema_for!(ErrorCode)).unwrap();
    let mut tags = Vec::new();
    collect_schema_string_tags(&schema, &mut tags);
    tags.sort();
    tags.dedup();
    assert!(
        !tags.is_empty(),
        "the ErrorCode schema must enumerate its members"
    );
    tags
}

/// Schemars splits a unit enum into a bare `enum` for the members with no doc
/// comment and a separate `const` for each one that has one, so both shapes
/// have to be gathered or documenting a member would silently hide it.
fn collect_schema_string_tags(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                match (key.as_str(), child) {
                    ("const", Value::String(tag)) => out.push(tag.clone()),
                    ("enum", Value::Array(items)) => {
                        out.extend(items.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                    _ => collect_schema_string_tags(child, out),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_schema_string_tags(item, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_plugin_mirrors_every_error_code_and_its_canonical_message() {
    // A code present on one end and absent on the other is not a failed
    // request: the decoder refuses the frame and the whole broker session
    // drops. So the two ends are pinned in BOTH directions. The Rust side
    // comes from the derived schema, so a Rust-only seventeenth member is
    // caught; the plugin side comes from the plugin's own lists, so a
    // plugin-only one is caught too.
    let plugin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate has a workspace parent")
        .join("plugin/src");
    let protocol = std::fs::read_to_string(plugin.join("shared/protocol.ts")).unwrap();
    let validation = std::fs::read_to_string(plugin.join("shared/result-validation.ts")).unwrap();
    let render = std::fs::read_to_string(plugin.join("read/render.ts")).unwrap();

    let rust = schema_error_codes();
    let mut listed: Vec<String> = ERROR_CODES
        .iter()
        .map(|code| error_code_tag(*code).to_owned())
        .collect();
    listed.sort();
    assert_eq!(
        listed, rust,
        "ERROR_CODES in this file is not the set the enum declares"
    );

    let (declared, accepted) = plugin_error_code_lists(&protocol);
    assert_eq!(
        declared, rust,
        "plugin protocol.ts declares a different set of error codes than Rust"
    );
    assert_eq!(
        accepted, rust,
        "plugin protocol.ts's ERROR_CODES value list differs from its own tuple type"
    );

    // The message is code-owned, and the Rust decoder refuses any frame whose
    // message is not the canonical one for its code. A drifted string is
    // therefore also a dropped session, not a cosmetic difference. Three lists
    // hold these strings and nothing else pins them to each other.
    let mut expected: Vec<(String, String)> = ERROR_CODES
        .iter()
        .map(|code| {
            (
                error_code_tag(*code).to_owned(),
                canonical_message(*code).to_owned(),
            )
        })
        .collect();
    expected.sort();
    assert_eq!(
        plugin_message_map(
            &validation,
            "const CANONICAL_MESSAGES: Record<ErrorCode, string> = {"
        ),
        expected,
        "plugin result-validation.ts carries different canonical messages than Rust"
    );
    assert_eq!(
        plugin_message_map(&render, "const MESSAGES: Record<ErrorCode, string> = {"),
        expected,
        "plugin read/render.ts carries different canonical messages than Rust"
    );
}

#[test]
fn an_unsafe_screenshot_asset_carries_its_rule_through_the_result() {
    let payload = json!({
        "assets": [{ "status": "success", "value": unsafe_svg_asset(json!({
            "kind": "unsafeAttribute", "name": "href"
        }))}],
        "truncated": false,
        "observation": {
            "startedAt": "2026-08-19T00:00:00.000Z",
            "completedAt": "2026-08-19T00:00:01.000Z"
        }
    });
    let result: figma_dev_mcp_protocol::domain::GetScreenshotResult =
        serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(serde_json::to_value(&result).unwrap(), payload);
}

#[test]
fn a_lone_surrogate_in_svg_source_is_refused_by_the_decoder() {
    // Once SVG safety stopped withholding the source, the plugin's lone-surrogate
    // guard became the only thing keeping a JSON-unencodable string off the wire.
    // This pins the consequence that makes that guard load-bearing rather than
    // merely tidy: serde_json rejects a lone surrogate while PARSING, so the whole
    // frame fails to decode — the broker session drops, not one asset.
    let payload = |escape: &str| {
        format!(
            r#"{{"assets":[{{"status":"success","value":{{"format":"svg","nodeId":"1:2","source":"<svg/>{escape}","safe":true}}}}],"truncated":false,"observation":{{"startedAt":"2026-08-19T00:00:00.000Z","completedAt":"2026-08-19T00:00:01.000Z"}}}}"#
        )
    };

    // Control first, and it must be a surrogate PAIR written as an escape rather
    // than a literal character, so it exercises the same escape-decoding path the
    // lone surrogate takes. Without this the assertion below could pass because
    // the fixture is malformed some other way, proving nothing about surrogates.
    serde_json::from_str::<figma_dev_mcp_protocol::domain::GetScreenshotResult>(&payload(
        r"\ud83d\ude00",
    ))
    .expect("a surrogate pair escape decodes");

    let error = serde_json::from_str::<figma_dev_mcp_protocol::domain::GetScreenshotResult>(
        &payload(r"\ud800"),
    )
    .expect_err("a lone surrogate must not decode");
    assert!(
        error.is_syntax(),
        "expected a JSON syntax error from the lone surrogate, got: {error}"
    );
}

#[test]
fn the_tool_error_no_longer_carries_an_svg_rule() {
    // The field moved to the asset. Left on the error it would sit in the
    // schema of four tools that have no SVG in them at all.
    assert!(
        serde_json::from_value::<ToolError>(json!({
            "code": "UNSAFE_SVG",
            "message": "The SVG was rejected by the safety policy.",
            "retryable": false,
            "svgRejection": { "kind": "unsafeElement", "name": "script" }
        }))
        .is_err(),
        "an svgRejection on a tool error must be refused, not ignored"
    );
}

#[test]
fn the_empty_bounds_code_round_trips_and_owns_its_message() {
    // A code one end knows and the other does not is not a failed request: the
    // decoder refuses the frame and the whole broker session drops. This pins
    // the wire tag the plugin has to mirror exactly.
    let wire = r#"{"code":"EMPTY_NODE_BOUNDS","message":"The requested node renders nothing.","retryable":false}"#;
    let error: ToolError = serde_json::from_str(wire).unwrap();
    assert_eq!(error.code(), ErrorCode::EmptyNodeBounds);
    assert_eq!(serde_json::to_string(&error).unwrap(), wire);

    let item: ItemError = serde_json::from_str(
        r#"{"index":0,"code":"EMPTY_NODE_BOUNDS","message":"The requested node renders nothing.","retryable":false}"#,
    )
    .unwrap();
    assert_eq!(item.code(), ErrorCode::EmptyNodeBounds);

    // The message is code-owned: a sender that supplies its own is refused
    // rather than quietly relaying prose we did not write.
    assert!(
        serde_json::from_value::<ToolError>(json!({
            "code": "EMPTY_NODE_BOUNDS",
            "message": "The node is empty.",
            "retryable": false
        }))
        .is_err(),
        "a non-canonical message for EMPTY_NODE_BOUNDS must be refused"
    );
    assert!(
        serde_json::from_str::<ErrorCode>("\"EMPTY_BOUNDS\"").is_err(),
        "a near-miss spelling must not decode"
    );
}

#[test]
fn fixed_limits_match_the_reviewed_ceiling() {
    assert_eq!(MAX_DEPTH, 6);
    assert_eq!(MAX_INPUT_IDS, 2_000);
    assert_eq!(MAX_PAGE_IDS, 100);
    assert_eq!(MAX_IDENTIFIER_BYTES, 256);
    assert_eq!(MAX_QUERY_BYTES, 1_024);
    assert_eq!(MAX_DISPLAY_TEXT_BYTES, 1_024);
    assert_eq!(MAX_VISITED_NODES, 10_000);
    assert_eq!(MAX_RETURNED_NODES, 2_000);
    assert_eq!(MAX_TEXT_BYTES, 8 * 1024 * 1024);
    assert_eq!(MAX_ENVELOPE_BYTES, 24 * 1024 * 1024);
    assert_eq!(MAX_RASTER_SIDE, 4_096);
    assert_eq!(MAX_RASTER_PIXELS, 16_000_000);
    assert_eq!(MAX_RASTER_DECODED_BYTES, 12 * 1024 * 1024);
    assert_eq!(MAX_RASTER_BASE64_BYTES, 16 * 1024 * 1024);
    assert_eq!(MAX_SVG_BYTES, 4 * 1024 * 1024);
    assert_eq!(MAX_IN_FLIGHT, 4);
    assert_eq!(MAX_QUEUE, 16);
    assert_eq!(INACTIVITY_TIMEOUT_SECS, 15);
    assert_eq!(TOTAL_TIMEOUT_SECS, 120);
    assert_eq!(HEARTBEAT_SECS, 5);
    assert_eq!(STALE_SESSION_SECS, 20);
    assert_eq!(IDLE_GRACE_SECS, 30);
}

#[test]
fn boundary_decoders_reject_oversized_inputs_before_dispatch() {
    let oversized_identifier = "x".repeat(MAX_IDENTIFIER_BYTES + 1);
    let oversized_display = "x".repeat(MAX_DISPLAY_TEXT_BYTES + 1);
    let oversized_query = "x".repeat(MAX_QUERY_BYTES + 1);
    let too_many_nodes: Vec<String> = (0..=MAX_INPUT_IDS).map(|i| format!("1:{i}")).collect();
    let too_many_pages: Vec<String> = (0..=MAX_PAGE_IDS).map(|i| format!("2:{i}")).collect();

    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": oversized_identifier, "deadlineMs": 100,
            "target": {}, "operation": {"operation": "get_metadata", "input": {}}
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<PluginToBroker>(json!({
            "type": "hello", "protocolVersion": "1", "displayName": oversized_display,
            "fileName": "Example", "editorType": "dev", "capabilities": {}
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
            "target": {}, "operation": {"operation": "search_nodes", "input": {
                "scope": {"pageId": "0:1"}, "query": oversized_query, "match": "contains", "limit": 50
            }}
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
            "target": {}, "operation": {"operation": "get_nodes", "input": {
                "nodeIds": too_many_nodes
            }}
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
            "target": {}, "operation": {"operation": "get_components", "input": {
                "selector": {"pageIds": too_many_pages}
            }}
        }))
        .is_err()
    );

    assert!(serde_json::from_str::<SelectionFlag>("true").is_ok());
    assert!(serde_json::from_str::<SelectionFlag>("false").is_err());
}

#[test]
fn screenshot_schema_and_decoder_exclude_raster_scale_from_svg() {
    let invalid = json!({
        "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
        "target": {}, "operation": {"operation": "get_screenshot", "input": {
            "format": "svg", "selector": {"nodeId": "1:2"}, "scale": 2.0
        }}
    });
    assert!(serde_json::from_value::<BrokerToPlugin>(invalid).is_err());
    let schema = schemars::schema_for!(figma_dev_mcp_protocol::domain::GetScreenshotInput);
    let schema_json = serde_json::to_value(schema).unwrap().to_string();
    assert!(schema_json.contains("oneOf"));
    assert!(schema_json.contains("svg"));
}

#[test]
fn screenshot_assets_enforce_wire_byte_and_raster_dimension_limits() {
    let raster = |width, height| {
        json!({
            "format": "png", "nodeId": "1:2", "dataBase64": "AA==",
            "width": width, "height": height
        })
    };

    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "png", "nodeId": "1:2",
            "dataBase64": "A".repeat(MAX_RASTER_BASE64_BYTES + 1),
            "width": 1, "height": 1
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ScreenshotAsset>(json!({
            "format": "svg", "nodeId": "1:2", "source": "x".repeat(MAX_SVG_BYTES + 1),
            "safe": true
        }))
        .is_err()
    );
    assert!(serde_json::from_value::<ScreenshotAsset>(raster(MAX_RASTER_SIDE + 1, 1)).is_err());
    assert!(serde_json::from_value::<ScreenshotAsset>(raster(4_001, 4_000)).is_err());
    assert!(serde_json::from_value::<ScreenshotAsset>(raster(4_000, 4_000)).is_ok());

    let schema = serde_json::to_value(schemars::schema_for!(ScreenshotAsset))
        .unwrap()
        .to_string();
    for expected in [
        format!("\"maxLength\":{MAX_RASTER_BASE64_BYTES}"),
        format!("\"x-maxUtf8Bytes\":{MAX_RASTER_BASE64_BYTES}"),
        format!("\"maxLength\":{MAX_SVG_BYTES}"),
        format!("\"x-maxUtf8Bytes\":{MAX_SVG_BYTES}"),
        format!("\"maximum\":{MAX_RASTER_SIDE}"),
        format!("\"x-maxRasterSide\":{MAX_RASTER_SIDE}"),
        format!("\"x-maxRasterPixels\":{MAX_RASTER_PIXELS}"),
    ] {
        assert!(
            schema.contains(&expected),
            "screenshot schema is missing {expected}"
        );
    }
}

#[test]
fn selector_schema_is_one_of_and_selection_is_literal_true() {
    let schema = serde_json::to_value(schemars::schema_for!(Selector)).unwrap();
    assert!(schema.get("oneOf").is_some());
    assert!(schema.get("anyOf").is_none());
    let schema_text = schema.to_string();
    assert!(schema_text.contains("\"const\":true"));

    assert!(serde_json::from_value::<Selector>(json!({"selection": true})).is_ok());
    assert!(serde_json::from_value::<Selector>(json!({"selection": false})).is_err());
    assert!(
        serde_json::from_value::<Selector>(json!({
            "pageId": "0:1", "nodeId": "1:2"
        }))
        .is_err()
    );
}

#[test]
fn rpc_frames_are_length_prefixed_and_reject_oversize_before_body_read() {
    assert_ne!(
        std::any::TypeId::of::<RequestId>(),
        std::any::TypeId::of::<RpcRequestId>(),
        "plugin and frontend correlation IDs must remain distinct types"
    );
    let request: FrontendToLeader = serde_json::from_value(json!({
        "type": "cancel", "rpcRequestId": "rpc-1"
    }))
    .unwrap();
    let encoded = encode_frame(&request).unwrap();
    let declared = u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize;
    assert_eq!(declared, encoded.len() - 4);
    assert_eq!(decode_frame::<FrontendToLeader>(&encoded).unwrap(), request);

    let mut oversized = Cursor::new(((MAX_ENVELOPE_BYTES as u32) + 1).to_be_bytes());
    let error =
        figma_dev_mcp_protocol::rpc::read_frame::<_, LeaderToFrontend>(&mut oversized).unwrap_err();
    assert_eq!(oversized.position(), 4);
    assert!(error.to_string().contains("exceeds"));
}

#[test]
fn unknown_fields_are_rejected_at_nested_boundaries() {
    let mut request: Value = serde_json::from_str(FIXTURES[1]).unwrap();
    request["operation"]["input"]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<BrokerToPlugin>(request).is_err());

    let rpc = json!({"type": "cancel", "rpcRequestId": "rpc-1", "method": "anything"});
    assert!(serde_json::from_value::<FrontendToLeader>(rpc).is_err());

    assert!(
        serde_json::from_value::<ToolError>(json!({
            "code": "INTERNAL_ERROR",
            "message": "The operation failed.",
            "retryable": false,
            "pluginPayload": {"stack": "not allowed"}
        }))
        .is_err()
    );
}

#[test]
fn nested_enum_fields_are_camel_case_and_input_bounds_are_schema_backed() {
    let paint: PaintValue = serde_json::from_value(json!({
        "type": "image", "imageRef": "image-1", "scaleMode": "fill", "opacity": 1.0
    }))
    .unwrap();
    let paint = serde_json::to_value(paint).unwrap();
    assert_eq!(paint["imageRef"], "image-1");
    assert!(paint.get("image_ref").is_none());

    let action: ReactionAction = serde_json::from_value(json!({
        "type": "navigate", "destinationId": "2:3"
    }))
    .unwrap();
    let action = serde_json::to_value(action).unwrap();
    assert_eq!(action["destinationId"], "2:3");
    assert!(action.get("destination_id").is_none());

    let asset: ScreenshotAsset = serde_json::from_value(json!({
        "format": "png", "nodeId": "1:2", "dataBase64": "AA==",
        "width": 1, "height": 1
    }))
    .unwrap();
    let asset = serde_json::to_value(asset).unwrap();
    assert_eq!(asset["nodeId"], "1:2");
    assert!(asset.get("node_id").is_none());

    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
            "target": {}, "operation": {"operation": "get_selection", "input": {
                "depth": 7
            }}
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<BrokerToPlugin>(json!({
            "type": "request", "requestId": "plugin-1", "deadlineMs": 100,
            "target": {}, "operation": {"operation": "get_screenshot", "input": {
                "format": "png", "selector": {"nodeId": "1:2"}, "scale": 4.01
            }}
        }))
        .is_err()
    );

    let schema = serde_json::to_value(schemars::schema_for!(ReadOperation))
        .unwrap()
        .to_string();
    assert!(schema.contains("\"maxItems\":2000"));
    assert!(schema.contains("\"maxItems\":100"));
    assert!(schema.contains("\"maxLength\":256"));
    assert!(schema.contains("\"maxLength\":1024"));
}

#[test]
fn frontend_invocations_cannot_supply_plugin_request_ids() {
    let invocation = json!({
        "call": "invoke",
        "connectionId": "123e4567-e89b-42d3-a456-426614174000",
        "invocation": {
            "operation": {"operation": "get_metadata", "input": {}}
        }
    });
    let call: BrokerCall = serde_json::from_value(invocation.clone()).unwrap();
    assert_eq!(broker_call_tag(&call), "invoke");

    let frontend: FrontendToLeader = serde_json::from_value(json!({
        "type": "request",
        "rpcRequestId": "rpc-1",
        "call": invocation
    }))
    .unwrap();
    assert_eq!(frontend_to_leader_tag(&frontend), "request");

    let frontend_schema = serde_json::to_value(schemars::schema_for!(FrontendToLeader))
        .unwrap()
        .to_string();
    assert!(!frontend_schema.contains("\"requestId\""));

    let smuggled_plugin_request = json!({
        "call": "invoke",
        "connectionId": "123e4567-e89b-42d3-a456-426614174000",
        "request": {
            "requestId": "attacker-chosen-plugin-id",
            "deadlineMs": 120000,
            "target": {},
            "operation": {"operation": "get_metadata", "input": {}}
        }
    });
    assert!(serde_json::from_value::<BrokerCall>(smuggled_plugin_request).is_err());
}

#[test]
fn detail_results_are_discriminated_and_rich_nodes_are_schema_explicit() {
    let summary = node_summary_fixture();
    let minimal_node = minimal_node_fixture();
    let compact_node = compact_node_fixture();
    let full_node = full_node_fixture();

    for result in [
        json!({
            "detail": "minimal", "nodes": [minimal_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }),
        json!({
            "detail": "compact", "nodes": [compact_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }),
        json!({
            "detail": "full", "nodes": [full_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }),
    ] {
        serde_json::from_value::<GetSelectionResult>(result).unwrap();
    }

    for result in [
        json!({
            "detail": "minimal",
            "items": [{"status": "success", "value": minimal_node.clone()}],
            "truncated": false, "observation": observation_fixture()
        }),
        json!({
            "detail": "compact",
            "items": [{"status": "success", "value": compact_node.clone()}],
            "truncated": false, "observation": observation_fixture()
        }),
        json!({
            "detail": "full",
            "items": [{"status": "success", "value": full_node.clone()}],
            "truncated": false, "observation": observation_fixture()
        }),
    ] {
        serde_json::from_value::<GetNodesResult>(result).unwrap();
    }

    for result in [
        json!({
            "detail": "minimal", "roots": [minimal_node], "truncated": false,
            "observation": observation_fixture()
        }),
        json!({
            "detail": "compact", "roots": [compact_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }),
        json!({
            "detail": "full", "roots": [full_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }),
    ] {
        serde_json::from_value::<GetDesignContextResult>(result).unwrap();
    }

    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "minimal", "nodes": [summary.clone()], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "compact", "nodes": [summary.clone()], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "minimal", "nodes": [compact_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "compact", "nodes": [full_node.clone()], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "full", "nodes": [compact_node], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "full", "nodes": [full_node], "truncated": false,
            "observation": observation_fixture(), "unknown": true
        }))
        .is_err()
    );

    let schema = serde_json::to_value(schemars::schema_for!(GetSelectionResult))
        .unwrap()
        .to_string();
    for required in [
        "minimal",
        "compact",
        "full",
        "geometry",
        "constraints",
        "autoLayout",
        "styledRanges",
        "component",
        "instance",
        "styleReferences",
        "variableReferences",
        "childrenTruncated",
    ] {
        assert!(
            schema.contains(required),
            "detail schema is missing {required}"
        );
    }
}

#[test]
fn minimal_results_preserve_recursive_depth_and_reject_flat_summaries() {
    let minimal = minimal_node_fixture();

    let selection: GetSelectionResult = serde_json::from_value(json!({
        "detail": "minimal", "nodes": [minimal.clone()], "truncated": false,
        "observation": observation_fixture()
    }))
    .unwrap();
    let selection = serde_json::to_value(selection).unwrap();
    assert_eq!(selection["nodes"][0]["summary"]["id"], "1:2");
    assert_eq!(selection["nodes"][0]["children"][0]["summary"]["id"], "1:3");
    assert_eq!(
        selection["nodes"][0]["children"][0]["childrenTruncation"]["reason"],
        "depthLimit"
    );

    let nodes: GetNodesResult = serde_json::from_value(json!({
        "detail": "minimal",
        "items": [
            {"status": "success", "value": minimal.clone()},
            {"status": "error", "error": {
                "code": "NODE_NOT_FOUND", "message": "The requested node was not found.",
                "retryable": false
            }}
        ],
        "truncated": false, "observation": observation_fixture()
    }))
    .unwrap();
    let nodes = serde_json::to_value(nodes).unwrap();
    assert_eq!(nodes["items"][0]["value"]["summary"]["id"], "1:2");
    assert_eq!(nodes["items"][1]["status"], "error");

    let context: GetDesignContextResult = serde_json::from_value(json!({
        "detail": "minimal", "roots": [minimal], "truncated": false,
        "observation": observation_fixture()
    }))
    .unwrap();
    let context = serde_json::to_value(context).unwrap();
    assert_eq!(context["roots"][0]["children"][0]["summary"]["id"], "1:3");

    let schema = serde_json::to_value(schemars::schema_for!(GetSelectionResult))
        .unwrap()
        .to_string();
    assert!(schema.contains("MinimalNodeDetails"));
    assert!(schema.contains("childrenTruncation"));

    let flat = node_summary_fixture();
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "minimal", "nodes": [flat.clone()], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetNodesResult>(json!({
            "detail": "minimal",
            "items": [{"status": "success", "value": flat.clone()}],
            "truncated": false, "observation": observation_fixture()
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<GetDesignContextResult>(json!({
            "detail": "minimal", "roots": [flat], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );

    let mut mixed = minimal_node_fixture();
    mixed["data"]["geometry"] = json!({
        "bounds": null, "rotation": 0.0, "opacity": 1.0,
        "transform": {
            "m00": 1.0, "m01": 0.0, "m02": 0.0,
            "m10": 0.0, "m11": 1.0, "m12": 0.0
        }
    });
    assert!(
        serde_json::from_value::<GetSelectionResult>(json!({
            "detail": "minimal", "nodes": [mixed], "truncated": false,
            "observation": observation_fixture()
        }))
        .is_err()
    );
}

#[test]
fn search_node_types_are_count_and_utf8_byte_bounded() {
    let too_many = vec!["FRAME"; MAX_INPUT_IDS + 1];
    assert!(serde_json::from_value::<NodeTypeList>(json!(too_many)).is_err());

    let over_bytes_but_not_chars = "é".repeat((MAX_IDENTIFIER_BYTES / 2) + 1);
    assert!(over_bytes_but_not_chars.chars().count() < MAX_IDENTIFIER_BYTES);
    assert!(over_bytes_but_not_chars.len() > MAX_IDENTIFIER_BYTES);
    assert!(serde_json::from_value::<NodeTypeList>(json!([over_bytes_but_not_chars])).is_err());

    let schema = serde_json::to_value(schemars::schema_for!(NodeTypeList))
        .unwrap()
        .to_string();
    assert!(schema.contains("\"maxItems\":2000"));
    assert!(schema.contains("\"maxLength\":256"));
}

#[test]
fn broker_and_rpc_enums_are_exhaustive_and_shape_locked() {
    let list_files: BrokerCall = serde_json::from_value(json!({"call": "listFiles"})).unwrap();
    assert_eq!(broker_call_tag(&list_files), "listFiles");

    let invocation: BrokerCall = serde_json::from_value(json!({
        "call": "invoke", "connectionId": "123e4567-e89b-42d3-a456-426614174000", "invocation": {
            "operation": {"operation": "get_metadata", "input": {}}
        }
    }))
    .unwrap();
    assert_eq!(broker_call_tag(&invocation), "invoke");

    let frontend_request: FrontendToLeader = serde_json::from_value(json!({
        "type": "request", "rpcRequestId": "rpc-1", "call": {"call": "listFiles"}
    }))
    .unwrap();
    assert_eq!(frontend_to_leader_tag(&frontend_request), "request");
    let frontend_cancel: FrontendToLeader = serde_json::from_value(json!({
        "type": "cancel", "rpcRequestId": "rpc-1"
    }))
    .unwrap();
    assert_eq!(frontend_to_leader_tag(&frontend_cancel), "cancel");

    let progress: LeaderToFrontend = serde_json::from_value(json!({
        "type": "progress", "rpcRequestId": "rpc-1", "progress": {"completed": 1}
    }))
    .unwrap();
    assert_eq!(leader_to_frontend_tag(&progress), "progress");
    let response: LeaderToFrontend = serde_json::from_value(json!({
        "type": "response", "rpcRequestId": "rpc-1", "result": {
            "kind": "files", "result": {
                "files": [], "truncated": false, "observation": observation_fixture()
            }
        }
    }))
    .unwrap();
    assert_eq!(leader_to_frontend_tag(&response), "response");
    let error: LeaderToFrontend = serde_json::from_value(json!({
        "type": "error", "rpcRequestId": "rpc-1", "error": {
            "code": "CANCELLED", "message": "The operation was cancelled.",
            "retryable": false
        }
    }))
    .unwrap();
    assert_eq!(leader_to_frontend_tag(&error), "error");

    for invalid in [
        json!({"call": "unknown"}),
        json!({"call": "listFiles", "requestId": "not-allowed"}),
    ] {
        assert!(serde_json::from_value::<BrokerCall>(invalid).is_err());
    }
    assert!(
        serde_json::from_value::<FrontendToLeader>(json!({
            "type": "unknown", "rpcRequestId": "rpc-1"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<LeaderToFrontend>(json!({
            "type": "unknown", "rpcRequestId": "rpc-1"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<LeaderToFrontend>(json!({
            "type": "progress", "rpcRequestId": "rpc-1", "progress": {"completed": 1},
            "unexpected": true
        }))
        .is_err()
    );
}

#[test]
fn bounded_collection_decoders_stop_at_max_plus_one_and_frame_encoding_is_stream_capped() {
    let (nodes, consumed) =
        deserialize_counted_sequence::<NodesSelector>("nodeIds", MAX_INPUT_IDS + 5, "1:2");
    assert!(nodes.is_err());
    assert_eq!(consumed, MAX_INPUT_IDS + 1);

    let (pages, consumed) =
        deserialize_counted_sequence::<PagesSelector>("pageIds", MAX_PAGE_IDS + 5, "0:1");
    assert!(pages.is_err());
    assert_eq!(consumed, MAX_PAGE_IDS + 1);

    let (returned, consumed) = deserialize_counted_returned_list(MAX_RETURNED_NODES + 5, "item");
    assert!(returned.is_err());
    assert_eq!(consumed, MAX_RETURNED_NODES + 1);

    let rpc_source = include_str!("../../crates/protocol/src/rpc.rs");
    assert!(!rpc_source.contains("serde_json::to_vec"));
    assert!(rpc_source.contains("serde_json::to_writer"));
    assert!(rpc_source.contains("CappedWriter"));
}

#[test]
fn nested_returned_value_collections_reject_more_than_the_result_limit() {
    let gradient_stop = json!({
        "position": 0.0,
        "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}
    });
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "linearGradient",
            "stops": vec![gradient_stop; MAX_RETURNED_NODES + 1],
            "gradientTransform": {"m00": 1.0, "m01": 0.0, "m02": 0.0,
                                  "m10": 0.0, "m11": 1.0, "m12": 0.0},
            "opacity": 1.0
        }))
        .is_err()
    );
}

#[test]
fn gradient_and_image_paints_carry_opacity_and_direction() {
    let gradient: PaintValue = serde_json::from_value(json!({
        "type": "linearGradient",
        "stops": [{"position": 0.0, "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}],
        "gradientTransform": {"m00": 0.0, "m01": 1.0, "m02": 0.0,
                              "m10": -1.0, "m11": 0.0, "m12": 1.0},
        "opacity": 0.4
    }))
    .unwrap();
    let encoded = serde_json::to_value(gradient).unwrap();
    assert_eq!(encoded["opacity"], 0.4);
    assert_eq!(encoded["gradientTransform"]["m01"], 1.0);

    let image: PaintValue = serde_json::from_value(json!({
        "type": "image", "imageRef": "image-1", "scaleMode": "fill", "opacity": 0.25
    }))
    .unwrap();
    assert_eq!(serde_json::to_value(image).unwrap()["opacity"], 0.25);
}

#[test]
fn angular_and_diamond_gradients_are_first_class_paints() {
    let transform = json!({"m00": 1.0, "m01": 0.0, "m02": 0.0,
                           "m10": 0.0, "m11": 1.0, "m12": 0.0});
    let stops = json!([{"position": 0.0, "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}}]);

    for tag in ["angularGradient", "diamondGradient"] {
        let paint: PaintValue = serde_json::from_value(json!({
            "type": tag, "stops": stops, "gradientTransform": transform, "opacity": 1.0
        }))
        .unwrap();
        assert_eq!(serde_json::to_value(paint).unwrap()["type"], tag);
    }
}

#[test]
fn a_gradient_without_its_direction_or_opacity_is_rejected() {
    let stops = json!([{"position": 0.0, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}]);
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "linearGradient", "stops": stops, "opacity": 1.0
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "image", "imageRef": "i", "scaleMode": "fill"
        }))
        .is_err()
    );
}

#[test]
fn a_gradient_transform_belongs_only_to_gradients() {
    let transform = json!({"m00": 1.0, "m01": 0.0, "m02": 0.0,
                           "m10": 0.0, "m11": 1.0, "m12": 0.0});
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "solid",
            "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0},
            "opacity": 1.0,
            "gradientTransform": transform
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "image",
            "imageRef": "i",
            "scaleMode": "fill",
            "opacity": 1.0,
            "gradientTransform": transform
        }))
        .is_err()
    );
}

#[test]
fn every_read_result_family_rejects_an_oversized_top_level_collection() {
    let observation = observation_fixture();
    let minimal_node = detail_node_fixture("minimal");
    let node_summary = node_summary_fixture();
    let cases = [
        (
            "get_metadata",
            json!({
                "file": {"key": "file", "name": "File", "editorType": "dev"},
                "pages": oversized_items(json!({"id": "0:1", "name": "Page"})),
                "currentPageId": "0:1", "capabilities": {}, "truncated": false,
                "observation": observation.clone()
            }),
        ),
        (
            "get_selection",
            json!({
                "detail": "minimal", "nodes": oversized_items(minimal_node.clone()),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_nodes",
            json!({
                "detail": "minimal",
                "items": oversized_items(json!({
                    "status": "success", "value": minimal_node.clone()
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "search_nodes",
            json!({
                "matches": oversized_items(json!({
                    "node": node_summary.clone(), "reasons": []
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_design_context",
            json!({
                "detail": "minimal", "roots": oversized_items(minimal_node.clone()),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_styles",
            json!({
                "styles": oversized_items(json!({
                    "styleType": "grid", "id": "style", "name": "Layout",
                    "pattern": "grid", "size": 8.0
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_variables",
            json!({
                "collections": oversized_items(json!({
                    "id": "collection", "name": "Theme", "modes": [], "variables": []
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_components",
            json!({
                "components": oversized_items(json!({
                    "id": "2:1", "name": "Button", "documentation": [],
                    "variantProperties": [], "propertyDefinitions": []
                })),
                "instances": [], "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_fonts",
            json!({
                "fonts": oversized_items(json!({
                    "font": {"family": "Inter", "style": "Regular"},
                    "availability": "available", "nodeIds": []
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_dev_mode_data",
            json!({
                "items": oversized_items(json!({
                    "status": "success",
                    "value": {
                        "nodeId": "4:1", "annotations": [], "annotationCategories": [],
                        "documentation": [], "devResources": []
                    }
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_reactions",
            json!({
                "items": oversized_items(json!({
                    "status": "success", "value": {"nodeId": "5:1", "reactions": []}
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_motion",
            json!({
                "items": oversized_items(json!({
                    "status": "success",
                    "value": {
                        "nodeId": "6:1", "animationStyles": [], "animations": [],
                        "manualKeyframeTracks": [], "timelines": []
                    }
                })),
                "truncated": false, "observation": observation.clone()
            }),
        ),
        (
            "get_screenshot",
            json!({
                "assets": oversized_items(json!({
                    "status": "success",
                    "value": {"format": "svg", "nodeId": "7:1", "source": "<svg/>", "safe": true}
                })),
                "truncated": false, "observation": observation
            }),
        ),
    ];

    assert_eq!(cases.len(), 13);
    for (operation, result) in cases {
        assert!(
            serde_json::from_value::<ReadResult>(json!({
                "operation": operation,
                "result": result
            }))
            .is_err(),
            "{operation} must reject a collection with more than {MAX_RETURNED_NODES} items"
        );
    }
}

#[test]
fn public_scalar_boundaries_reject_invalid_inbound_and_outbound_values() {
    assert!(serde_json::from_str::<ConnectionId>("\"\"").is_err());
    assert!(serde_json::from_str::<ConnectionId>("\"connection-1\"").is_err());
    assert!(serde_json::from_str::<RequestId>("\"\"").is_err());
    assert!(serde_json::from_str::<RpcRequestId>("\"\"").is_err());
    assert!(serde_json::from_str::<NodeTypeName>("\"\"").is_err());
    assert!(serde_json::from_value::<Selector>(json!({"nodeId": ""})).is_err());
    assert!(serde_json::from_value::<Selector>(json!({"pageId": ""})).is_err());

    assert!(ConnectionId::try_from("").is_err());
    assert!(ConnectionId::try_from("connection-1").is_err());
    assert!(RequestId::try_from("").is_err());
    assert!(RpcRequestId::try_from("").is_err());
    assert!(NodeId::try_from("").is_err());
    assert!(PageId::try_from("").is_err());
    assert!(NodeTypeName::try_from("").is_err());
    assert!(RasterScale::try_from(f64::NAN).is_err());
    assert!(RasterScale::try_from(0.24).is_err());
    assert!(RasterScale::try_from(4.01).is_err());

    let connection = ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap();
    let request = RequestId::try_from("plugin-1").unwrap();
    let rpc_request = RpcRequestId::try_from("rpc-1").unwrap();
    let node = NodeId::try_from("1:2").unwrap();
    let page = PageId::try_from("0:1").unwrap();
    let node_type = NodeTypeName::try_from("FRAME").unwrap();
    let scale = RasterScale::try_from(4.0).unwrap();
    assert_eq!(connection.as_str(), "123e4567-e89b-42d3-a456-426614174000");
    assert_eq!(rpc_request.as_str(), "rpc-1");
    assert_eq!(scale.value(), 4.0);
    assert_eq!(
        serde_json::to_value(connection).unwrap(),
        "123e4567-e89b-42d3-a456-426614174000"
    );
    assert_eq!(serde_json::to_value(request).unwrap(), "plugin-1");
    assert_eq!(serde_json::to_value(rpc_request).unwrap(), "rpc-1");
    assert_eq!(serde_json::to_value(node).unwrap(), "1:2");
    assert_eq!(serde_json::to_value(page).unwrap(), "0:1");
    assert_eq!(serde_json::to_value(node_type).unwrap(), "FRAME");
    assert_eq!(serde_json::to_value(scale).unwrap(), 4.0);
}

#[test]
fn recursive_results_enforce_depth_and_global_returned_node_budgets() {
    for detail in ["minimal", "compact", "full"] {
        for branch in ["selection", "nodes", "context"] {
            let valid = result_with_node(branch, detail, nested_detail_node(detail, MAX_DEPTH));
            assert!(
                decode_detail_result(branch, valid).is_ok(),
                "{branch}/{detail} must accept depth {MAX_DEPTH}"
            );

            let too_deep =
                result_with_node(branch, detail, nested_detail_node(detail, MAX_DEPTH + 1));
            assert!(
                decode_detail_result(branch, too_deep).is_err(),
                "{branch}/{detail} must reject depth {}",
                MAX_DEPTH + 1
            );
        }
    }

    for (branch, detail) in [
        ("selection", "minimal"),
        ("nodes", "compact"),
        ("context", "full"),
    ] {
        let boundary_nodes: Vec<Value> = (0..MAX_RETURNED_NODES)
            .map(|_| detail_node_fixture(detail))
            .collect();
        let boundary_result = result_with_nodes(branch, detail, boundary_nodes);
        assert!(
            decode_detail_result(branch, boundary_result).is_ok(),
            "{branch}/{detail} must accept exactly {MAX_RETURNED_NODES} returned nodes"
        );

        let nodes: Vec<Value> = (0..=MAX_RETURNED_NODES)
            .map(|_| detail_node_fixture(detail))
            .collect();
        let result = result_with_nodes(branch, detail, nodes);
        assert!(
            decode_detail_result(branch, result).is_err(),
            "{branch}/{detail} must reject {} returned nodes",
            MAX_RETURNED_NODES + 1
        );
    }

    let schema = serde_json::to_value(schemars::schema_for!(GetSelectionResult))
        .unwrap()
        .to_string();
    assert!(schema.contains("\"x-maxDepth\":6"));
    assert!(schema.contains("\"x-maxReturnedNodes\":2000"));
}

#[test]
fn schemas_and_decoders_agree_on_utf8_byte_limits() {
    let id_schema = serde_json::to_value(schemars::schema_for!(ConnectionId))
        .unwrap()
        .to_string();
    assert!(id_schema.contains("\"x-maxUtf8Bytes\":36"));
    assert!(id_schema.contains("\"format\":\"uuid\""));
    assert!(id_schema.contains("\"pattern\":\"^[0-9a-fA-F]{8}-"));

    let query_schema = serde_json::to_value(schemars::schema_for!(SearchNodesInput))
        .unwrap()
        .to_string();
    assert!(query_schema.contains("\"x-maxUtf8Bytes\":1024"));
    assert!(
        serde_json::from_value::<SearchNodesInput>(json!({
            "scope": {"pageId": "0:1"}, "query": "Pay", "match": "contains", "limit": 50
        }))
        .is_ok()
    );
    assert!(
        serde_json::from_value::<SearchNodesInput>(json!({
            "scope": {"pageId": "0:1"},
            "types": ["FRAME"]
        }))
        .is_ok()
    );
    let hello_schema =
        serde_json::to_value(schemars::schema_for!(figma_dev_mcp_protocol::wire::Hello))
            .unwrap()
            .to_string();
    assert!(hello_schema.contains("\"x-maxUtf8Bytes\":1024"));

    let error_schema = serde_json::to_value(schemars::schema_for!(ToolError))
        .unwrap()
        .to_string();
    assert!(error_schema.contains("\"x-maxUtf8Bytes\":1024"));

    let multibyte_identifier = "é".repeat(129);
    assert_eq!(multibyte_identifier.chars().count(), 129);
    assert!(multibyte_identifier.len() > MAX_IDENTIFIER_BYTES);
    assert!(serde_json::from_value::<ConnectionId>(json!(multibyte_identifier)).is_err());

    let multibyte_query = "é".repeat(513);
    assert_eq!(multibyte_query.chars().count(), 513);
    assert!(multibyte_query.len() > MAX_QUERY_BYTES);
    assert!(
        serde_json::from_value::<SearchNodesInput>(json!({
            "scope": {"pageId": "0:1"}, "query": multibyte_query, "match": "contains", "limit": 50
        }))
        .is_err()
    );

    let multibyte_display = "é".repeat(513);
    assert!(
        serde_json::from_value::<PluginToBroker>(json!({
            "type": "hello", "protocolVersion": "1", "displayName": multibyte_display,
            "fileName": "Example", "editorType": "dev", "capabilities": {}
        }))
        .is_err()
    );
}

#[test]
fn plugin_failures_cannot_supply_public_or_diagnostic_messages() {
    assert!(
        serde_json::from_value::<PluginToBroker>(json!({
            "type": "error", "requestId": "plugin-1", "error": {
                "code": "INTERNAL_ERROR",
                "message": "token=secret stack=/private/path",
                "retryable": false
            }
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<ToolError>(json!({
            "code": "TIMEOUT", "message": "attacker controlled", "retryable": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ToolError>(json!({
            "code": "NODE_NOT_FOUND", "message": "The operation timed out.",
            "retryable": false
        }))
        .is_err()
    );

    let item = ItemError::new(
        0,
        Some(ItemIdentifier::try_from("1:2").unwrap()),
        ErrorCode::NodeNotFound,
        false,
    );
    let error = ToolError::new(ErrorCode::NodeNotFound, false)
        .with_items(vec![item])
        .unwrap();
    assert_eq!(
        serde_json::to_value(error).unwrap(),
        json!({
            "code": "NODE_NOT_FOUND",
            "message": "The requested node was not found.",
            "retryable": false,
            "items": [{
                "index": 0,
                "id": "1:2",
                "code": "NODE_NOT_FOUND",
                "message": "The requested node was not found.",
                "retryable": false
            }]
        })
    );

    let plugin_failure_schema = serde_json::to_value(schemars::schema_for!(PluginFailure))
        .unwrap()
        .to_string();
    assert!(!plugin_failure_schema.contains("\"message\""));
    assert!(plugin_failure_schema.contains("\"maxItems\":2000"));
}

#[test]
fn actual_wire_discriminators_accept_tag_last_and_reject_duplicate_or_unknown_fields() {
    let hello = r#"{
        "protocolVersion":"1","connectionId":"123e4567-e89b-42d3-a456-426614174000",
        "displayName":"Plugin","fileName":"Example",
        "currentPage":{"id":"0:1","name":"Page 1"},
        "editorType":"dev","pluginVersion":"0.1.0","capabilities":{},"type":"hello"
    }"#;
    assert!(serde_json::from_str::<PluginToBroker>(hello).is_ok());

    let request = r#"{
        "requestId":"plugin-1","deadlineMs":100,"target":{},
        "operation":{"input":{"nodeIds":["1:2"]},"operation":"get_nodes"},
        "type":"request"
    }"#;
    assert!(serde_json::from_str::<BrokerToPlugin>(request).is_ok());

    let response = r#"{
        "requestId":"plugin-1","result":{
            "result":{"nodes":[],"truncated":false,
                "observation":{"startedAt":"s","completedAt":"e"},"detail":"minimal"},
            "operation":"get_selection"
        },"type":"response"
    }"#;
    assert!(serde_json::from_str::<PluginToBroker>(response).is_ok());

    for duplicate in [
        r#"{"type":"pong","nonce":1,"type":"pong"}"#,
        r#"{"type":"request","requestId":"p","deadlineMs":1,"target":{},"operation":{"operation":"get_metadata","input":{},"operation":"get_metadata"}}"#,
        r#"{"type":"response","requestId":"p","result":{"operation":"get_selection","result":{"detail":"minimal","nodes":[],"truncated":false,"observation":{"startedAt":"s","completedAt":"e"},"detail":"minimal"}}}"#,
    ] {
        assert!(serde_json::from_str::<PluginToBroker>(duplicate).is_err());
        assert!(serde_json::from_str::<BrokerToPlugin>(duplicate).is_err());
    }

    assert!(
        serde_json::from_str::<PluginToBroker>(
            r#"{"requestId":"p","result":{"result":{},"operation":"get_metadata","unknown":true},"type":"response"}"#
        )
        .is_err()
    );
}

#[test]
fn outbound_node_collections_reject_wide_roots_and_children_without_auxiliary_growth() {
    let leaf: DesignNode<MinimalNodeDetails> =
        serde_json::from_value(detail_node_fixture("minimal")).unwrap();

    assert!(NodeForest::try_from(vec![leaf.clone(); MAX_RETURNED_NODES]).is_ok());
    assert!(NodeForest::try_from(vec![leaf.clone(); MAX_RETURNED_NODES + 1]).is_err());

    let mut exact_parent = leaf.clone();
    exact_parent.children = vec![leaf.clone(); MAX_RETURNED_NODES - 1];
    assert!(NodeForest::try_from(vec![exact_parent]).is_ok());

    let mut too_wide_parent = leaf.clone();
    too_wide_parent.children = vec![leaf; MAX_RETURNED_NODES + 1];
    assert!(NodeForest::try_from(vec![too_wide_parent]).is_err());
}

fn deserialize_counted_sequence<T>(
    field: &'static str,
    total: usize,
    item: &'static str,
) -> (Result<T, ValueError>, usize)
where
    T: DeserializeOwned,
{
    let consumed = Rc::new(Cell::new(0));
    let consumed_by_iterator = Rc::clone(&consumed);
    let values = (0..total).map(move |_| {
        consumed_by_iterator.set(consumed_by_iterator.get() + 1);
        let value: StrDeserializer<'static, ValueError> = item.into_deserializer();
        value
    });
    let sequence = SeqDeserializer::<_, ValueError>::new(values);
    let key: StrDeserializer<'static, ValueError> = field.into_deserializer();
    let map = MapDeserializer::<_, ValueError>::new(std::iter::once((key, sequence)));
    (T::deserialize(map), consumed.get())
}

fn deserialize_counted_returned_list(
    total: usize,
    item: &'static str,
) -> (Result<ReturnedList<String>, ValueError>, usize) {
    let consumed = Rc::new(Cell::new(0));
    let consumed_by_iterator = Rc::clone(&consumed);
    let values = (0..total).map(move |_| {
        consumed_by_iterator.set(consumed_by_iterator.get() + 1);
        let value: StrDeserializer<'static, ValueError> = item.into_deserializer();
        value
    });
    let sequence = SeqDeserializer::<_, ValueError>::new(values);
    (ReturnedList::deserialize(sequence), consumed.get())
}

fn oversized_items(item: Value) -> Vec<Value> {
    vec![item; MAX_RETURNED_NODES + 1]
}

fn detail_node_fixture(detail: &str) -> Value {
    match detail {
        "minimal" => json!({
            "summary": node_summary_fixture(), "data": {}, "children": [],
            "childrenTruncated": false
        }),
        "compact" => compact_node_fixture(),
        "full" => full_node_fixture(),
        _ => panic!("unsupported detail fixture {detail}"),
    }
}

fn nested_detail_node(detail: &str, depth: u8) -> Value {
    let mut node = detail_node_fixture(detail);
    for _ in 0..depth {
        let mut parent = detail_node_fixture(detail);
        parent["children"] = json!([node]);
        node = parent;
    }
    node
}

fn result_with_node(branch: &str, detail: &str, node: Value) -> Value {
    result_with_nodes(branch, detail, vec![node])
}

fn result_with_nodes(branch: &str, detail: &str, nodes: Vec<Value>) -> Value {
    match branch {
        "selection" => json!({
            "detail": detail, "nodes": nodes, "truncated": false,
            "observation": observation_fixture()
        }),
        "nodes" => json!({
            "detail": detail,
            "items": nodes.into_iter().map(|value| json!({
                "status": "success", "value": value
            })).collect::<Vec<_>>(),
            "truncated": false, "observation": observation_fixture()
        }),
        "context" => json!({
            "detail": detail, "roots": nodes, "truncated": false,
            "observation": observation_fixture()
        }),
        _ => panic!("unsupported result branch {branch}"),
    }
}

fn decode_detail_result(branch: &str, value: Value) -> Result<Value, serde_json::Error> {
    match branch {
        "selection" => {
            serde_json::from_value::<GetSelectionResult>(value).and_then(serde_json::to_value)
        }
        "nodes" => serde_json::from_value::<GetNodesResult>(value).and_then(serde_json::to_value),
        "context" => {
            serde_json::from_value::<GetDesignContextResult>(value).and_then(serde_json::to_value)
        }
        _ => panic!("unsupported result branch {branch}"),
    }
}

fn plugin_to_broker_tag(message: &PluginToBroker) -> &'static str {
    match message {
        PluginToBroker::Hello(_) => "hello",
        PluginToBroker::Progress(_) => "progress",
        PluginToBroker::Response(_) => "response",
        PluginToBroker::Error(_) => "error",
        PluginToBroker::Pong(_) => "pong",
    }
}

fn broker_to_plugin_tag(message: &BrokerToPlugin) -> &'static str {
    match message {
        BrokerToPlugin::Request(_) => "request",
        BrokerToPlugin::Cancel(_) => "cancel",
        BrokerToPlugin::Ping(_) => "ping",
    }
}

fn error_code_tag(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::NoFigmaConnection => "NO_FIGMA_CONNECTION",
        ErrorCode::AmbiguousConnection => "AMBIGUOUS_CONNECTION",
        ErrorCode::ConnectionNotFound => "CONNECTION_NOT_FOUND",
        ErrorCode::ConnectionLost => "CONNECTION_LOST",
        ErrorCode::ProtocolMismatch => "PROTOCOL_MISMATCH",
        ErrorCode::NodeNotFound => "NODE_NOT_FOUND",
        ErrorCode::PageNotFound => "PAGE_NOT_FOUND",
        ErrorCode::UnsupportedNode => "UNSUPPORTED_NODE",
        ErrorCode::EmptyNodeBounds => "EMPTY_NODE_BOUNDS",
        ErrorCode::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
        ErrorCode::UnsafeSvg => "UNSAFE_SVG",
        ErrorCode::InvalidCursor => "INVALID_CURSOR",
        ErrorCode::LimitExceeded => "LIMIT_EXCEEDED",
        ErrorCode::Timeout => "TIMEOUT",
        ErrorCode::Cancelled => "CANCELLED",
        ErrorCode::InternalError => "INTERNAL_ERROR",
    }
}

fn broker_call_tag(call: &BrokerCall) -> &'static str {
    match call {
        BrokerCall::ListFiles {} => "listFiles",
        BrokerCall::Invoke { .. } => "invoke",
    }
}

fn frontend_to_leader_tag(message: &FrontendToLeader) -> &'static str {
    match message {
        FrontendToLeader::Request { .. } => "request",
        FrontendToLeader::Cancel { .. } => "cancel",
    }
}

fn leader_to_frontend_tag(message: &LeaderToFrontend) -> &'static str {
    match message {
        LeaderToFrontend::Progress { .. } => "progress",
        LeaderToFrontend::Response { .. } => "response",
        LeaderToFrontend::Error { .. } => "error",
    }
}

fn observation_fixture() -> Value {
    json!({
        "startedAt": "2026-08-15T10:00:00Z",
        "completedAt": "2026-08-15T10:00:01Z"
    })
}

fn node_summary_fixture() -> Value {
    json!({
        "id": "1:2", "name": "Card", "nodeType": "FRAME", "visible": true,
        "parentId": "0:1", "childIds": ["1:3"],
        "bounds": {"x": 0.0, "y": 0.0, "width": 320.0, "height": 200.0}
    })
}

fn compact_node_fixture() -> Value {
    json!({
        "summary": node_summary_fixture(),
        "data": {
            "geometry": {
                "bounds": {"x": 0.0, "y": 0.0, "width": 320.0, "height": 200.0},
                "rotation": 0.0, "opacity": 1.0,
                "transform": {
                    "m00": 1.0, "m01": 0.0, "m02": 0.0,
                    "m10": 0.0, "m11": 1.0, "m12": 0.0
                }
            },
            "constraints": {"horizontal": "stretch", "vertical": "min"},
            "autoLayout": null,
            "text": {"characterCount": 4, "preview": "Card"},
            "component": null,
            "instance": null,
            "styleReferences": [{"id": "S:1", "kind": "paint"}],
            "variableReferences": [{"id": "V:1", "name": "surface/background"}]
        },
        "children": [], "childrenTruncated": false
    })
}

fn minimal_node_fixture() -> Value {
    let mut child_summary = node_summary_fixture();
    child_summary["id"] = json!("1:3");
    child_summary["name"] = json!("Label");
    child_summary["nodeType"] = json!("TEXT");
    child_summary["parentId"] = json!("1:2");
    child_summary["childIds"] = json!([]);

    json!({
        "summary": node_summary_fixture(),
        "data": {},
        "children": [{
            "summary": child_summary,
            "data": {},
            "children": [],
            "childrenTruncated": true,
            "childrenTruncation": {
                "reason": "depthLimit", "appliedDepth": 1
            }
        }],
        "childrenTruncated": false
    })
}

fn full_node_fixture() -> Value {
    json!({
        "summary": node_summary_fixture(),
        "data": {
            "geometry": {
                "bounds": {"x": 0.0, "y": 0.0, "width": 320.0, "height": 200.0},
                "rotation": 0.0, "opacity": 1.0,
                "transform": {
                    "m00": 1.0, "m01": 0.0, "m02": 0.0,
                    "m10": 0.0, "m11": 1.0, "m12": 0.0
                }
            },
            "constraints": {"horizontal": "stretch", "vertical": "min"},
            "autoLayout": {
                "mode": "horizontal", "primarySizing": "hug", "counterSizing": "fixed",
                "gap": 8.0, "paddingTop": 4.0, "paddingRight": 12.0,
                "paddingBottom": 4.0, "paddingLeft": 12.0,
                "primaryAlign": "spaceBetween", "counterAlign": "center",
                "wrap": true, "counterAxisSpacing": 6.0
            },
            "text": {
                "characters": "Card", "defaultStyle": {
                    "fontFamily": "Inter", "fontStyle": "Regular", "fontSize": 16.0,
                    "lineHeight": {"unit": "pixels", "value": 24.0},
                    "letterSpacing": {"unit": "pixels", "value": 0.0},
                    "fontWeight": 400.0, "paints": []
                },
                "styledRanges": [{
                    "start": 0, "end": 4, "style": {
                        "fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 16.0,
                        "lineHeight": {"unit": "pixels", "value": 24.0},
                        "letterSpacing": {"unit": "pixels", "value": 0.0},
                        "fontWeight": 400.0, "paints": []
                    }
                }],
                "alignHorizontal": "center", "autoResize": "height"
            },
            "paints": [{
                "type": "solid",
                "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0},
                "opacity": 1.0
            }],
            "effects": [],
            "strokes": {
                "paints": [{
                    "type": "solid",
                    "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0},
                    "opacity": 1.0
                }],
                "weight": 1.0, "align": "inside"
            },
            "cornerRadius": {"kind": "uniform", "radius": 8.0},
            "cornerSmoothing": 0.6,
            "clipsContent": true,
            "blendMode": "multiply",
            "component": null,
            "instance": {
                "componentId": "C:1", "componentSetId": "CS:1", "properties": []
            },
            "styleReferences": [
                {"id": "S:1", "kind": "paint", "name": "Primary/500"},
                {"id": "S:2", "kind": "stroke"}
            ],
            "variableReferences": [{"id": "V:1", "name": "surface/background"}]
        },
        "children": [], "childrenTruncated": false
    })
}

/// Clones `full_node_fixture()` and replaces its `data.instance` with a populated
/// (non-empty `properties`) instance value. Does not modify `full_node_fixture()` itself.
fn full_node_fixture_with_populated_instance_properties() -> Value {
    let mut node = full_node_fixture();
    node["data"]["instance"] = json!({
        "componentId": "C:1",
        "componentSetId": "CS:1",
        "properties": [
            {"name": "ButtonText#0:1", "value": {"kind": "text", "value": "Save"}},
            {"name": "Size", "value": {"kind": "variant", "value": "Large"}}
        ]
    });
    node
}

#[test]
fn full_node_results_carry_populated_instance_properties_past_the_serializer() {
    let node = full_node_fixture_with_populated_instance_properties();

    for branch in ["selection", "nodes", "context"] {
        let wrapped = result_with_node(branch, "full", node.clone());
        let decoded =
            decode_detail_result(branch, wrapped).expect("populated instance properties decode");
        let properties = match branch {
            "selection" => &decoded["nodes"][0]["data"]["instance"]["properties"],
            "nodes" => &decoded["items"][0]["value"]["data"]["instance"]["properties"],
            "context" => &decoded["roots"][0]["data"]["instance"]["properties"],
            _ => unreachable!(),
        };
        assert_eq!(
            *properties,
            json!([
                {"name": "ButtonText#0:1", "value": {"kind": "text", "value": "Save"}},
                {"name": "Size", "value": {"kind": "variant", "value": "Large"}}
            ]),
            "{branch} must round-trip populated instance properties unchanged"
        );
    }

    let schema = serde_json::to_value(schemars::schema_for!(GetSelectionResult))
        .unwrap()
        .to_string();
    assert!(schema.contains("NamedComponentProperty"));
    assert!(schema.contains("\"instance\""));
}

#[test]
fn component_property_values_round_trip_and_reject_unknown_kinds() {
    let value = json!({
        "componentId": "C:1",
        "componentSetId": "CS:1",
        "properties": [
            {"name": "ButtonText#0:1", "value": {"kind": "text", "value": "Save"}},
            {"name": "IconSwap#0:2", "value": {"kind": "instanceSwap", "value": "9:9"}},
            {"name": "IconVisible#0:0", "value": {"kind": "boolean", "value": false}},
            {"name": "Size", "value": {"kind": "variant", "value": "Large"}}
        ]
    });

    let parsed: ComponentValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);

    // InstanceValue is the type this feature actually populates; keep it in lockstep
    // with ComponentValue even though the two are structurally identical today.
    let parsed: InstanceValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);

    assert!(
        serde_json::from_value::<ComponentValue>(json!({
            "componentId": "C:1",
            "properties": [
                {"name": "Slot#0:3", "value": {"kind": "slot", "value": ""}}
            ]
        }))
        .is_err(),
        "component property union must stay closed"
    );
    assert!(
        serde_json::from_value::<InstanceValue>(json!({
            "componentId": "C:1",
            "properties": [
                {"name": "Slot#0:3", "value": {"kind": "slot", "value": ""}}
            ]
        }))
        .is_err(),
        "instance property union must stay closed"
    );
}

fn motion_observation() -> Value {
    json!({
        "startedAt": "2026-08-16T10:00:00.000Z",
        "completedAt": "2026-08-16T10:00:00.001Z"
    })
}

#[test]
fn amended_motion_contract_uses_seconds_keyed_maps_and_distinct_style_types() {
    let result = json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "6:1",
                "animationStyles": [{
                    "id": "applied-1",
                    "styleId": "S:fade",
                    "name": "Fade in",
                    "duration": 0.4,
                    "timelineOffset": 0.1,
                    "props": [
                        {"name": "direction", "value": "right"},
                        {"name": "distance", "value": 120.0},
                        {"name": "enabled", "value": true},
                        {"name": "easing", "value": {"type": "EASE_OUT"}}
                    ]
                }],
                "animations": [{
                    "field": {"type": "property", "name": "TRANSLATION_X"},
                    "baseValue": {"type": "FLOAT", "value": 0.0},
                    "timelineDuration": 0.4,
                    "tracks": [{
                        "id": "track-1",
                        "keyframeOperation": "SET",
                        "keyframes": [{
                            "id": "kf-1",
                            "timelinePosition": 0.4,
                            "value": {"type": "CIRCLE", "value": {"x": 1.0, "y": 2.0, "radius": 3.0}},
                            "easing": {"type": "EASE_IN_BACK"}
                        }]
                    }]
                }],
                "manualKeyframeTracks": [{
                    "field": {"type": "indexedItem", "collection": "fills", "index": 0},
                    "id": "manual-1",
                    "baseValue": {"type": "unsupported", "tag": "MESH"},
                    "keyframes": [{
                        "id": "kf-2",
                        "timelinePosition": 0.2,
                        "value": {"type": "COLOR_POINT", "value": {
                            "x": 1.0, "y": 2.0,
                            "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}
                        }},
                        "easing": {
                            "type": "CUSTOM_CUBIC_BEZIER",
                            "easingFunctionCubicBezier": {
                                "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4
                            }
                        }
                    }]
                }],
                "timelines": [{"id": "tl-1", "duration": 0.4}]
            }
        }],
        "availableStyles": [{
            "styleId": "S:fade",
            "name": "Fade in",
            "description": "Catalog fade",
            "props": [{"name": "direction", "value": "string"}]
        }],
        "visitedNodes": 1,
        "truncated": false,
        "observation": motion_observation()
    });
    let decoded: GetMotionResult =
        serde_json::from_value(result.clone()).expect("amended motion result must decode");
    let encoded = serde_json::to_value(decoded).expect("amended motion result must re-encode");
    assert_eq!(
        encoded["items"][0]["value"]["animationStyles"][0]["duration"],
        0.4
    );
    assert_eq!(
        encoded["items"][0]["value"]["animations"][0]["timelineDuration"],
        0.4
    );
    assert_eq!(
        encoded["items"][0]["value"]["manualKeyframeTracks"][0]["keyframes"][0]["timelinePosition"],
        0.2
    );
    assert_eq!(
        encoded["items"][0]["value"]["timelines"][0]["duration"],
        0.4
    );
    assert!(
        encoded["items"][0]["value"]["animationStyles"][0]
            .get("durationMs")
            .is_none()
    );
    assert!(encoded["availableStyles"][0].get("duration").is_none());
    assert_eq!(encoded["availableStyles"][0]["styleId"], "S:fade");
    assert_eq!(
        encoded["items"][0]["value"]["manualKeyframeTracks"][0]["baseValue"],
        json!({"type": "unsupported", "tag": "MESH"})
    );

    for easing in [
        "LINEAR",
        "EASE_IN",
        "EASE_OUT",
        "EASE_IN_AND_OUT",
        "EASE_IN_BACK",
        "EASE_OUT_BACK",
        "EASE_IN_AND_OUT_BACK",
        "CUSTOM_CUBIC_BEZIER",
        "GENTLE",
        "QUICK",
        "BOUNCY",
        "SLOW",
        "CUSTOM_SPRING",
        "HOLD",
        "VARIABLE_ALIAS",
    ] {
        let mut fixture = result.clone();
        fixture["items"][0]["value"]["animations"][0]["tracks"][0]["keyframes"][0]["easing"] =
            if easing == "VARIABLE_ALIAS" {
                json!({"type": easing, "id": "V:ease"})
            } else {
                json!({"type": easing})
            };
        serde_json::from_value::<GetMotionResult>(fixture)
            .unwrap_or_else(|error| panic!("{easing} must decode: {error}"));
    }

    for value in [
        json!({"type": "FLOAT", "value": 1.0}),
        json!({"type": "COLOR", "value": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}}),
        json!({"type": "TEXT_DATA", "value": "hello"}),
        json!({"type": "VECTOR", "value": {"x": 1.0, "y": 2.0}}),
        json!({"type": "BOOL", "value": true}),
        json!({"type": "CIRCLE", "value": {"x": 1.0, "y": 2.0, "radius": 3.0}}),
        json!({"type": "LINE", "value": {"x": 0.0, "y": 0.0, "x2": 1.0, "y2": 1.0}}),
        json!({"type": "CIRCLE_POINT", "value": {"x": 1.0, "y": 2.0, "radius": 3.0, "angle": 0.5}}),
        json!({"type": "COLOR_POINT", "value": {
            "x": 1.0, "y": 2.0, "color": {"r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0}
        }}),
        json!({"type": "unsupported", "tag": "MESH"}),
    ] {
        let mut fixture = result.clone();
        fixture["items"][0]["value"]["animations"][0]["baseValue"] = value.clone();
        serde_json::from_value::<GetMotionResult>(fixture)
            .unwrap_or_else(|error| panic!("{value} must decode: {error}"));
    }

    assert!(
        serde_json::from_value::<GetMotionResult>(json!({
            "items": [{
                "status": "success",
                "value": {
                    "nodeId": "6:1",
                    "animationStyles": [{
                        "id": "a",
                        "name": "Fade",
                        "durationMs": 400,
                        "easing": {"kind": "linear"}
                    }],
                    "animations": [{
                        "name": "TRANSLATION_X",
                        "durationMs": 400,
                        "delayMs": 0,
                        "easing": {"kind": "easeIn"}
                    }],
                    "manualKeyframeTracks": [],
                    "timelines": [{
                        "name": "tl",
                        "startsAtMs": 0,
                        "durationMs": 400
                    }]
                }
            }],
            "visitedNodes": 1,
            "truncated": false,
            "observation": motion_observation()
        }))
        .is_err(),
        "Task 2 millisecond motion types must be rejected"
    );

    let schema = serde_json::to_value(schemars::schema_for!(GetMotionResult))
        .unwrap()
        .to_string();
    for forbidden in ["durationMs", "delayMs", "startsAtMs", "EasingKind"] {
        assert!(
            !schema.contains(forbidden),
            "motion schema still contains superseded {forbidden}"
        );
    }
    for required in [
        "AppliedAnimationStyle",
        "AvailableAnimationStyle",
        "timelineDuration",
        "timelinePosition",
        "timelineOffset",
        "EASE_IN_BACK",
        "CUSTOM_SPRING",
        "COLOR_POINT",
        "unsupported",
    ] {
        assert!(
            schema.contains(required),
            "motion schema is missing {required}"
        );
    }
}

#[test]
fn reaction_overlay_settings_are_closed_optional_and_camel_case() {
    let result = json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "5:1",
                "reactions": [{
                    "trigger": "hover",
                    "action": {"type": "openOverlay", "destinationId": "5:8"},
                    "transitionId": "MOVE_IN",
                    "destinationAccessible": true,
                    "overlay": {
                        "relativePosition": {"x": 8.0, "y": 12.0},
                        "positionType": "bottomRight",
                        "background": {
                            "type": "solidColor",
                            "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.4}
                        },
                        "backgroundInteraction": "closeOnClickOutside"
                    }
                }]
            }
        }],
        "visitedNodes": 1,
        "truncated": false,
        "observation": motion_observation()
    });
    let decoded: GetReactionsResult =
        serde_json::from_value(result).expect("overlay reaction must decode");
    let encoded = serde_json::to_value(decoded).expect("overlay reaction must re-encode");
    assert_eq!(
        encoded["items"][0]["value"]["reactions"][0]["overlay"]["relativePosition"],
        json!({"x": 8.0, "y": 12.0})
    );
    assert_eq!(
        encoded["items"][0]["value"]["reactions"][0]["overlay"]["positionType"],
        "bottomRight"
    );
    assert_eq!(
        encoded["items"][0]["value"]["reactions"][0]["overlay"]["background"]["type"],
        "solidColor"
    );
    assert!(
        encoded["items"][0]["value"]["reactions"][0]["overlay"]
            .get("overlay_relative_position")
            .is_none()
    );

    assert!(
        serde_json::from_value::<GetReactionsResult>(json!({
            "items": [{
                "status": "success",
                "value": {
                    "nodeId": "5:1",
                    "reactions": [{
                        "trigger": "hover",
                        "action": {"type": "openOverlay", "destinationId": "5:8"},
                        "destinationAccessible": true,
                        "overlay": {"leftover": true}
                    }]
                }
            }],
            "visitedNodes": 1,
            "truncated": false,
            "observation": motion_observation()
        }))
        .is_err(),
        "overlay must reject unknown fields"
    );

    let without = json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "5:1",
                "reactions": [{
                    "trigger": "click",
                    "action": {"type": "back"},
                    "destinationAccessible": true
                }]
            }
        }],
        "visitedNodes": 1,
        "truncated": false,
        "observation": motion_observation()
    });
    let decoded: GetReactionsResult =
        serde_json::from_value(without).expect("overlay-free reaction must still decode");
    let encoded = serde_json::to_value(decoded).unwrap();
    assert!(
        encoded["items"][0]["value"]["reactions"][0]
            .get("overlay")
            .is_none()
    );
}

#[test]
fn text_style_units_round_trip_and_reject_unknown_units() {
    let value = json!({
        "fontFamily": "Inter",
        "fontStyle": "Medium",
        "fontSize": 14.0,
        "lineHeight": {"unit": "percent", "value": 150.0},
        "letterSpacing": {"unit": "pixels", "value": 0.5},
        "paints": []
    });
    let parsed: TextStyle = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);

    let auto = json!({
        "fontFamily": "Inter",
        "fontStyle": "Medium",
        "lineHeight": {"unit": "auto"},
        "paints": []
    });
    let parsed: TextStyle = serde_json::from_value(auto.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), auto);

    assert!(
        serde_json::from_value::<LineHeightValue>(json!({"unit": "em", "value": 2.0})).is_err(),
        "line height unit must stay closed"
    );
    assert!(
        serde_json::from_value::<LetterSpacingValue>(json!({"unit": "auto"})).is_err(),
        "letter spacing has no auto variant"
    );
}

#[test]
fn layout_alignment_round_trips_and_rejects_unknown_values() {
    let value = json!({
        "mode": "horizontal",
        "primarySizing": "hug",
        "counterSizing": "fixed",
        "gap": 8.0,
        "paddingTop": 4.0,
        "paddingRight": 12.0,
        "paddingBottom": 4.0,
        "paddingLeft": 12.0,
        "primaryAlign": "spaceBetween",
        "counterAlign": "baseline",
        "wrap": true,
        "counterAxisSpacing": 6.0
    });
    let parsed: LayoutValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);

    let bare = json!({
        "mode": "vertical",
        "primarySizing": "fixed",
        "counterSizing": "fixed",
        "gap": 0.0,
        "paddingTop": 0.0,
        "paddingRight": 0.0,
        "paddingBottom": 0.0,
        "paddingLeft": 0.0
    });
    let parsed: LayoutValue = serde_json::from_value(bare.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), bare);

    assert!(
        serde_json::from_value::<AxisAlign>(json!("spaceAround")).is_err(),
        "axis alignment must stay closed"
    );
}

#[test]
fn stroke_value_round_trips_and_rejects_unknown_fields() {
    let value = json!({
        "paints": [{
            "type": "solid",
            "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0},
            "opacity": 1.0
        }],
        "weight": 2.0,
        "align": "inside",
        "dashPattern": [4.0, 2.0]
    });
    let parsed: StrokeValue = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);

    let bare = json!({"paints": [{"type": "mixed"}]});
    let parsed: StrokeValue = serde_json::from_value(bare.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), bare);

    assert!(
        serde_json::from_value::<StrokeValue>(json!({"paints": [], "style": "dashed"})).is_err(),
        "stroke value must stay closed"
    );
    assert!(
        serde_json::from_value::<StrokeAlign>(json!("baseline")).is_err(),
        "stroke alignment must stay closed"
    );
}

#[test]
fn corner_radius_round_trips_and_stays_closed() {
    let uniform = json!({"kind": "uniform", "radius": 8.0});
    let parsed: CornerRadiusValue = serde_json::from_value(uniform.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), uniform);

    let per_corner = json!({
        "kind": "perCorner",
        "topLeft": 8.0, "topRight": 0.0, "bottomRight": 4.0, "bottomLeft": 0.0
    });
    let parsed: CornerRadiusValue = serde_json::from_value(per_corner.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), per_corner);

    assert!(
        serde_json::from_value::<CornerRadiusValue>(json!({"kind": "mixed"})).is_err(),
        "corner radius tag must stay closed"
    );
    assert!(
        serde_json::from_value::<CornerRadiusValue>(
            json!({"kind": "uniform", "radius": 8.0, "topLeft": 8.0})
        )
        .is_err(),
        "uniform radius must reject per-corner fields"
    );
    assert!(
        serde_json::from_value::<CornerRadiusValue>(
            json!({"kind": "uniform", "radius": 8.0, "smoothing": 0.6})
        )
        .is_err(),
        "corner radius must reject unknown fields"
    );
}

#[test]
fn style_reference_carries_optional_name_and_stroke_kind() {
    let named = json!({"id": "S:1", "kind": "stroke", "name": "Border/Default"});
    let parsed: StyleReference = serde_json::from_value(named.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), named);

    let bare = json!({"id": "S:1", "kind": "paint"});
    let parsed: StyleReference = serde_json::from_value(bare.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), bare);

    assert!(
        serde_json::from_value::<StyleKind>(json!("variable")).is_err(),
        "style kind must stay closed"
    );
    assert!(
        serde_json::from_value::<StyleReference>(
            json!({"id": "S:1", "kind": "paint", "label": "x"})
        )
        .is_err(),
        "style reference must stay closed"
    );
}

fn visited_nodes_observation() -> Value {
    json!({
        "startedAt": "2026-08-19T10:00:00.000Z",
        "completedAt": "2026-08-19T10:00:00.001Z"
    })
}

fn sample_dev_mode_result() -> GetDevModeDataResult {
    serde_json::from_value(json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "4:1",
                "annotations": [{"id": "4:1:annotation:0", "text": "Match padding"}],
                "annotationCategories": [],
                "documentation": [],
                "devResources": []
            }
        }],
        "visitedNodes": 3,
        "truncated": false,
        "observation": visited_nodes_observation()
    }))
    .expect("dev mode sample must decode")
}

fn sample_reactions_result() -> GetReactionsResult {
    serde_json::from_value(json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "5:1",
                "reactions": [{
                    "trigger": "click",
                    "action": {"type": "back"},
                    "destinationAccessible": true
                }]
            }
        }],
        "visitedNodes": 3,
        "truncated": false,
        "observation": visited_nodes_observation()
    }))
    .expect("reactions sample must decode")
}

fn sample_motion_result() -> GetMotionResult {
    serde_json::from_value(json!({
        "items": [{
            "status": "success",
            "value": {
                "nodeId": "6:1",
                "animationStyles": [{
                    "id": "applied-1",
                    "styleId": "S:fade",
                    "name": "Fade in"
                }],
                "animations": [],
                "manualKeyframeTracks": [],
                "timelines": [{"id": "tl-1", "duration": 0.4}]
            }
        }],
        "visitedNodes": 3,
        "truncated": false,
        "observation": visited_nodes_observation()
    }))
    .expect("motion sample must decode")
}

#[test]
fn get_dev_mode_data_result_round_trips_visited_nodes() {
    let value = sample_dev_mode_result();
    let json = serde_json::to_value(&value).expect("serialize");
    assert_eq!(json["visitedNodes"], json!(3));
    let decoded: GetDevModeDataResult = serde_json::from_value(json).expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn get_dev_mode_data_result_requires_visited_nodes() {
    let mut json = serde_json::to_value(sample_dev_mode_result()).expect("serialize");
    json.as_object_mut().unwrap().remove("visitedNodes");
    assert!(
        serde_json::from_value::<GetDevModeDataResult>(json).is_err(),
        "visitedNodes must be required: an absent count reads as zero nodes walked"
    );
}

#[test]
fn get_dev_mode_data_result_still_rejects_unknown_fields() {
    let mut json = serde_json::to_value(sample_dev_mode_result()).expect("serialize");
    json.as_object_mut()
        .unwrap()
        .insert("bogus".into(), json!(1));
    assert!(serde_json::from_value::<GetDevModeDataResult>(json).is_err());
}

#[test]
fn get_reactions_result_round_trips_visited_nodes() {
    let value = sample_reactions_result();
    let json = serde_json::to_value(&value).expect("serialize");
    assert_eq!(json["visitedNodes"], json!(3));
    let decoded: GetReactionsResult = serde_json::from_value(json).expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn get_reactions_result_requires_visited_nodes() {
    let mut json = serde_json::to_value(sample_reactions_result()).expect("serialize");
    json.as_object_mut().unwrap().remove("visitedNodes");
    assert!(
        serde_json::from_value::<GetReactionsResult>(json).is_err(),
        "visitedNodes must be required: an absent count reads as zero nodes walked"
    );
}

#[test]
fn get_reactions_result_still_rejects_unknown_fields() {
    let mut json = serde_json::to_value(sample_reactions_result()).expect("serialize");
    json.as_object_mut()
        .unwrap()
        .insert("bogus".into(), json!(1));
    assert!(serde_json::from_value::<GetReactionsResult>(json).is_err());
}

#[test]
fn get_motion_result_round_trips_visited_nodes() {
    let value = sample_motion_result();
    let json = serde_json::to_value(&value).expect("serialize");
    assert_eq!(json["visitedNodes"], json!(3));
    let decoded: GetMotionResult = serde_json::from_value(json).expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn get_motion_result_requires_visited_nodes() {
    let mut json = serde_json::to_value(sample_motion_result()).expect("serialize");
    json.as_object_mut().unwrap().remove("visitedNodes");
    assert!(
        serde_json::from_value::<GetMotionResult>(json).is_err(),
        "visitedNodes must be required: an absent count reads as zero nodes walked"
    );
}

#[test]
fn get_motion_result_still_rejects_unknown_fields() {
    let mut json = serde_json::to_value(sample_motion_result()).expect("serialize");
    json.as_object_mut()
        .unwrap()
        .insert("bogus".into(), json!(1));
    assert!(serde_json::from_value::<GetMotionResult>(json).is_err());
}

#[test]
fn visited_nodes_reaches_every_per_node_output_schema() {
    for schema in [
        serde_json::to_value(schemars::schema_for!(GetDevModeDataResult)).unwrap(),
        serde_json::to_value(schemars::schema_for!(GetReactionsResult)).unwrap(),
        serde_json::to_value(schemars::schema_for!(GetMotionResult)).unwrap(),
    ] {
        assert!(
            schema["properties"]["visitedNodes"].is_object(),
            "visitedNodes must be published on the output schema"
        );
        let required = schema["required"].as_array().expect("required list");
        assert!(
            required.iter().any(|name| name == "visitedNodes"),
            "visitedNodes must be required in the output schema"
        );
    }
}

/// The three snapshots of the MCP surface: the published tool list and the
/// input and output schemas. They are not the plugin wire format. The two
/// overlap heavily — the output schemas are generated from the same result
/// types the plugin sends back over its socket — which is what makes this a
/// useful tripwire for a wire-shape change that arrives through a tool's
/// payload. It is not a complete one: the socket envelope in
/// `crates/protocol/src/wire.rs` (`Hello`, `Request`, `Response`, `Progress`,
/// `Cancel`, `Ping`/`Pong`) has no representation here, so a breaking change to
/// one of those passes this test untouched. Widening the fingerprint to cover
/// the envelope is deliberately deferred; until then that case is caught by the
/// version constants and the `hello` fixture pinned below, not by this hash.
///
/// Included rather than read at run time so the fingerprint cannot depend on a
/// working directory, and so cargo rebuilds this test when one of them changes.
const WIRE_SNAPSHOTS: [&str; 3] = [
    include_str!("snapshots/tools.json"),
    include_str!("snapshots/input-schemas.json"),
    include_str!("snapshots/output-schemas.json"),
];

/// The fingerprint of `WIRE_SNAPSHOTS` at the current wire version, over
/// LF-normalised bytes so it does not depend on the checkout's line endings.
const EXPECTED_WIRE_FINGERPRINT: &str = "0x2f2ace5975f55d91";

/// FNV-1a, 64-bit, over the three snapshots in order, separated by a byte that
/// cannot occur in UTF-8 so moving text between two files still changes it.
///
/// Carriage returns are dropped before hashing. `include_str!` hands back
/// whatever bytes are on disk, so a checkout that materialises these files with
/// CRLF endings would otherwise fingerprint differently from the same content
/// with LF and fail this test with a message pointing at a version decision the
/// reader does not have to make. Filtering here keeps the answer a property of
/// the content rather than of the checkout, without a `.gitattributes` that
/// every contributor would have to already have applied.
///
/// Not `DefaultHasher`: that one is explicitly unstable across Rust releases,
/// so a pinned constant would break on a toolchain upgrade rather than on a
/// real change. Not a cryptographic digest either: this detects change, it does
/// not resist forgery, and it is not worth a dependency.
fn wire_snapshot_fingerprint() -> String {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    const CARRIAGE_RETURN: u8 = b'\r';
    let mut hash = OFFSET_BASIS;
    for snapshot in WIRE_SNAPSHOTS {
        for byte in snapshot
            .as_bytes()
            .iter()
            .copied()
            .filter(|byte| *byte != CARRIAGE_RETURN)
            .chain([0xff])
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    format!("{hash:#018x}")
}

/// The first double-quoted value on the line declaring `key`.
fn extract_string_literal(source: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    source
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))?
        .split('"')
        .nth(1)
        .map(str::to_owned)
}

fn plugin_src() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate has a workspace parent")
        .join("plugin/src")
}

#[test]
fn both_ends_declare_the_same_wire_version() {
    // Two constants in two languages drift. When they do, the broker refuses
    // every connection at the hello, so this is the whole product down rather
    // than one request failing.
    let hello = std::fs::read_to_string(plugin_src().join("ui/hello.ts")).unwrap();
    let declared = extract_string_literal(&hello, "protocolVersion")
        .expect("plugin hello declares protocolVersion");
    assert_eq!(
        declared, PLUGIN_PROTOCOL_VERSION,
        "the plugin announces {declared} and the broker expects {PLUGIN_PROTOCOL_VERSION}; \
         a mismatch here refuses every connection"
    );

    // The checked-in hello is the frame both ends are read against elsewhere in
    // this file and in the plugin's own round-trip test, so it goes stale the
    // same way and is pinned with them.
    let fixture: Value = serde_json::from_str(FIXTURES[0]).unwrap();
    assert_eq!(
        fixture["protocolVersion"], PLUGIN_PROTOCOL_VERSION,
        "fixtures/hello.json announces a version no live plugin would send"
    );
}

#[test]
fn a_wire_snapshot_change_must_be_a_deliberate_version_decision() {
    let fingerprint = wire_snapshot_fingerprint();
    assert_eq!(
        fingerprint, EXPECTED_WIRE_FINGERPRINT,
        "\nThe MCP tool and schema snapshots changed.\n\
         If this is a breaking change to a shape the plugin also sends over its \
         socket, bump PLUGIN_PROTOCOL_VERSION and plugin/src/ui/hello.ts \
         together, then update EXPECTED_WIRE_FINGERPRINT.\n\
         If it is only a description or documentation edit, update \
         EXPECTED_WIRE_FINGERPRINT alone.\n"
    );
}

#[test]
fn an_unsupported_paint_names_its_figma_type() {
    let paint: PaintValue = serde_json::from_value(json!({
        "type": "unsupported", "figmaType": "VIDEO"
    }))
    .unwrap();
    let encoded = serde_json::to_value(paint).unwrap();
    assert_eq!(encoded["figmaType"], "VIDEO");

    assert!(serde_json::from_value::<PaintValue>(json!({"type": "unsupported"})).is_err());
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "unsupported", "figmaType": "VIDEO", "opacity": 1.0
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<PaintValue>(json!({
            "type": "solid",
            "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0},
            "opacity": 1.0,
            "figmaType": "VIDEO"
        }))
        .is_err()
    );
}
