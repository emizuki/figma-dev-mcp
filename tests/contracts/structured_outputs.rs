//! Successful structured results match checked-in output schemas; errors match ToolError.

use figma_dev_mcp_protocol::TOOL_NAMES;
use figma_dev_mcp_protocol::error::ToolError;
use figma_dev_mcp_tools::tools_catalog;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn observation() -> Value {
    json!({
        "startedAt": "2026-08-16T10:00:00.000Z",
        "completedAt": "2026-08-16T10:00:00.001Z"
    })
}

fn minimal_node(id: &str, name: &str) -> Value {
    json!({
        "summary": {
            "id": id,
            "name": name,
            "nodeType": "FRAME",
            "visible": true
        },
        "data": {},
        "children": [],
        "childrenTruncated": false
    })
}

fn output_schema(name: &str) -> Value {
    let catalog = tools_catalog();
    let tool = catalog
        .tools
        .iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} must be cataloged"));
    Value::Object(tool.output_schema.as_ref().unwrap().as_ref().clone())
}

fn assert_valid_output(name: &str, value: &Value) {
    let schema = output_schema(name);
    let validator =
        jsonschema::validator_for(&schema).unwrap_or_else(|error| panic!("{name}: {error}"));
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "{name} result must match the checked-in output schema: {errors:?}\n{value}"
    );
}

fn assert_tool_error_schema(value: &Value) {
    let schema = serde_json::to_value(schemars::schema_for!(ToolError)).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("ToolError schema");
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "ToolError must match the stable schema: {errors:?}\n{value}"
    );
    let decoded: ToolError = serde_json::from_value(value.clone()).expect("ToolError decodes");
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["message"],
        value["message"]
    );
}

fn success_fixture(name: &str) -> Value {
    match name {
        "list_files" => json!({
            "files": [{
                "connectionId": "123e4567-e89b-42d3-a456-426614174000",
                "displayName": "Checkout flow",
                "fileName": "Checkout flow",
                "currentPage": {"id": "0:2", "name": "Checkout"},
                "editorType": "dev",
                "capabilities": {},
                "connectedAt": "2026-08-16T10:00:00.000Z",
                "lastSeenAt": "2026-08-16T10:00:00.001Z"
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_metadata" => json!({
            "file": {"name": "Checkout flow", "editorType": "dev"},
            "pages": [{"id": "0:1", "name": "Home"}, {"id": "0:2", "name": "Checkout"}],
            "currentPageId": "0:2",
            "pluginVersion": "0.1.0",
            "capabilities": {},
            "truncated": false,
            "observation": observation()
        }),
        "get_selection" => json!({
            "detail": "minimal",
            "nodes": [minimal_node("1:1", "Card")],
            "truncated": false,
            "observation": observation()
        }),
        "get_nodes" => json!({
            "detail": "minimal",
            "items": [
                {"status": "success", "value": minimal_node("1:1", "Card")},
                {"status": "error", "error": {
                    "code": "NODE_NOT_FOUND",
                    "message": "The requested node was not found.",
                    "retryable": false
                }}
            ],
            "truncated": false,
            "observation": observation()
        }),
        "search_nodes" => json!({
            "matches": [{
                "node": {
                    "id": "1:2",
                    "name": "Card",
                    "nodeType": "FRAME",
                    "visible": true
                },
                "reasons": ["name"]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_design_context" => json!({
            "detail": "minimal",
            "roots": [minimal_node("1:1", "Card")],
            "truncated": false,
            "observation": observation()
        }),
        "get_styles" => json!({
            "styles": [{
                "styleType": "paint",
                "id": "S:brand",
                "name": "Brand",
                "description": "Brand fill",
                "remote": false,
                "key": "paint-key",
                "paints": [{
                    "type": "solid",
                    "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
                    "opacity": 1.0
                }]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_variables" => json!({
            "collections": [{
                "id": "C:theme",
                "name": "Theme",
                "modes": [{"id": "M:default", "name": "Default"}],
                "variables": [{
                    "id": "V:bg",
                    "name": "bg",
                    "collectionId": "C:theme",
                    "scopes": ["ALL_FILLS"],
                    "values": [{
                        "modeId": "M:default",
                        "source": {"kind": "alias", "value": "V:leaf"},
                        "resolved": {
                            "kind": "color",
                            "value": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}
                        }
                    }],
                    "codeSyntax": [{"platform": "WEB", "code": "var(--bg)"}]
                }]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_components" => json!({
            "components": [{
                "id": "2:1",
                "name": "Button",
                "componentSetId": "2:0",
                "description": "Primary button",
                "documentation": [{
                    "uri": "https://docs.example/button",
                    "label": "Button"
                }],
                "variantProperties": [{"name": "Size", "value": "Small"}],
                "propertyDefinitions": [{
                    "name": "Size",
                    "defaultValue": {"kind": "variant", "value": "Small"},
                    "preferredValues": [
                        {"kind": "variant", "value": "Small"},
                        {"kind": "variant", "value": "Large"}
                    ]
                }]
            }],
            "instances": [{"instanceId": "4:1", "componentId": "2:1"}],
            "truncated": false,
            "observation": observation()
        }),
        "get_fonts" => json!({
            "fonts": [{
                "font": {"family": "Inter", "style": "Regular"},
                "availability": "unavailable",
                "nodeIds": ["5:1"]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_dev_mode_data" => json!({
            "items": [{
                "status": "success",
                "value": {
                    "nodeId": "4:1",
                    "description": "Primary card",
                    "descriptionMarkdown": "**Primary** card",
                    "annotations": [{
                        "id": "4:1:annotation:0",
                        "categoryId": "cat-note",
                        "text": "Match padding"
                    }],
                    "annotationCategories": [{"id": "cat-note", "label": "Note"}],
                    "documentation": [{
                        "name": "Guide",
                        "uri": "https://docs.example/card"
                    }],
                    "devResources": [{
                        "name": "Storybook",
                        "uri": "https://storybook.example/card"
                    }],
                    "ownerNodeId": "4:1",
                    "inheritedFromNodeId": "2:1"
                }
            }],
            "visitedNodes": 1,
            "truncated": false,
            "observation": observation()
        }),
        "get_reactions" => json!({
            "items": [{
                "status": "success",
                "value": {
                    "nodeId": "5:1",
                    "reactions": [{
                        "trigger": "click",
                        "action": {"type": "navigate", "destinationId": "5:9"},
                        "destinationAccessible": true
                    }]
                }
            }],
            "visitedNodes": 1,
            "truncated": false,
            "observation": observation()
        }),
        "get_motion" => json!({
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
                        "props": [{"name": "direction", "value": "right"}]
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
                                "value": {"type": "FLOAT", "value": 120.0},
                                "easing": {"type": "EASE_IN_BACK"}
                            }]
                        }]
                    }],
                    "manualKeyframeTracks": [{
                        "field": {"type": "indexedItem", "collection": "fills", "index": 0},
                        "id": "manual-1",
                        "baseValue": {"type": "unsupported", "tag": "MESH"},
                        "keyframes": []
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
            "observation": observation()
        }),
        "get_screenshot" => json!({
            "assets": [{
                "status": "success",
                "value": {
                    "format": "png",
                    "nodeId": "8:1",
                    "width": 1,
                    "height": 1,
                    "decodedBytes": 70,
                    "base64Bytes": 92
                }
            }],
            "truncated": false,
            "observation": observation()
        }),
        other => panic!("missing success fixture for {other}"),
    }
}

#[test]
fn every_tool_has_a_success_fixture_that_matches_its_output_schema() {
    assert_eq!(TOOL_NAMES.len(), 14);
    for name in TOOL_NAMES {
        let fixture = success_fixture(name);
        assert_valid_output(name, &fixture);
        assert!(
            fixture.get("durationMs").is_none(),
            "{name} must not use durationMs"
        );
        let rendered = fixture.to_string();
        assert!(
            !rendered.contains("durationMs"),
            "{name} fixture must not contain durationMs"
        );
        let result = CallToolResult::structured(fixture.clone());
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            fixture.to_string(),
            "{name} compatibility text must equal structured JSON"
        );
    }
}

#[test]
fn every_stable_tool_error_matches_the_tool_error_schema() {
    for (code, message, retryable) in [
        (
            "NO_FIGMA_CONNECTION",
            "No Figma connection is available.",
            false,
        ),
        (
            "AMBIGUOUS_CONNECTION",
            "More than one Figma connection matches the request.",
            false,
        ),
        (
            "CONNECTION_NOT_FOUND",
            "The requested Figma connection was not found.",
            false,
        ),
        ("CONNECTION_LOST", "The Figma connection was lost.", true),
        ("TIMEOUT", "The operation timed out.", true),
        ("CANCELLED", "The operation was cancelled.", false),
        (
            "LIMIT_EXCEEDED",
            "The operation exceeded a safety limit.",
            true,
        ),
        ("NODE_NOT_FOUND", "The requested node was not found.", false),
        (
            "CAPABILITY_UNAVAILABLE",
            "The required Figma capability is unavailable.",
            false,
        ),
    ] {
        let error = json!({
            "code": code,
            "message": message,
            "retryable": retryable
        });
        assert_tool_error_schema(&error);
        let result = CallToolResult::structured_error(error.clone());
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content[0].as_text().unwrap().text, error.to_string());
    }
}

#[test]
fn motion_success_fixture_uses_seconds_and_keyed_field_maps() {
    let motion = success_fixture("get_motion");
    assert_eq!(
        motion["items"][0]["value"]["animationStyles"][0]["duration"],
        0.4
    );
    assert_eq!(
        motion["items"][0]["value"]["animations"][0]["timelineDuration"],
        0.4
    );
    assert_eq!(motion["items"][0]["value"]["timelines"][0]["duration"], 0.4);
    assert_eq!(
        motion["items"][0]["value"]["animations"][0]["field"]["type"],
        "property"
    );
    assert_eq!(
        motion["items"][0]["value"]["manualKeyframeTracks"][0]["field"]["collection"],
        "fills"
    );
    assert!(motion.to_string().contains("\"duration\":0.4"));
    assert!(!motion.to_string().contains("durationMs"));
}
