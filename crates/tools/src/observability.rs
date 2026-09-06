//! Schema-safe tool and broker logs. Design content never appears here.

use std::time::Duration;

use figma_dev_mcp_protocol::error::ErrorCode;
use rmcp::model::RequestId;

#[derive(Debug, Clone)]
pub struct ToolObservation {
    pub request_id: RequestId,
    pub tool_name: &'static str,
    pub connection_id: Option<String>,
    pub duration: Duration,
    pub item_count: usize,
    pub text_bytes: usize,
    pub envelope_bytes: usize,
    pub error_code: &'static str,
}

pub fn log_tool_completion(observation: &ToolObservation) {
    tracing::info!(
        request_id = %observation.request_id,
        tool_name = observation.tool_name,
        connection_id = observation.connection_id.as_deref().unwrap_or("-"),
        duration_ms = observation.duration.as_millis() as u64,
        item_count = observation.item_count,
        text_bytes = observation.text_bytes,
        envelope_bytes = observation.envelope_bytes,
        error_code = observation.error_code,
        "tool completed"
    );
}

/// The operator log's own copy of the wire tag, kept in a `&'static str`
/// rather than the borrowed JSON string so `tool_result_log_code` can return
/// it without an allocation. Exhaustive by construction and deliberately not
/// wildcarded: a new `ErrorCode` member fails to compile here until it is
/// given a tag, rather than silently falling through to a wrong label the
/// way string matching once did. Update this together with `ErrorCode`
/// itself, `error_code_tag` in `tests/contracts/mod.rs`, and `error_code_name`
/// in `service.rs` — all four enumerate the same closed set by hand because
/// the enum has no `strum`-style iterator.
const fn known_error_code_tag(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::NoFigmaConnection => "NO_FIGMA_CONNECTION",
        ErrorCode::AmbiguousConnection => "AMBIGUOUS_CONNECTION",
        ErrorCode::ConnectionNotFound => "CONNECTION_NOT_FOUND",
        ErrorCode::ConnectionLost => "CONNECTION_LOST",
        ErrorCode::ProtocolMismatch => "PROTOCOL_MISMATCH",
        ErrorCode::NodeNotFound => "NODE_NOT_FOUND",
        ErrorCode::NodeNotVisible => "NODE_NOT_VISIBLE",
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

pub fn tool_result_log_code(is_error: Option<bool>, structured_code: Option<&str>) -> &'static str {
    if is_error != Some(true) {
        return "OK";
    }
    // Three cases, not two. A failure that carries one of this protocol's own
    // codes logs as that code, via the exhaustive match below. The other two
    // are genuinely different failures from each other — `structured_code`
    // being `None` means the failure never reached structured content at all
    // (for example a schema rejection at the MCP layer), while `Some` but
    // unparseable means a "code" field was present but held something this
    // protocol never declared — but neither is a safety-limit breach, so
    // neither should log as `LIMIT_EXCEEDED`: that would tell an operator a
    // specific, actionable, and wrong thing. Both get their own tag instead,
    // `UNKNOWN`, which is honest about not knowing rather than confidently
    // wrong about knowing something false.
    match structured_code.and_then(parse_error_code) {
        Some(code) => known_error_code_tag(code),
        None => "UNKNOWN",
    }
}

fn parse_error_code(code: &str) -> Option<ErrorCode> {
    serde_json::from_value(serde_json::Value::String(code.to_owned())).ok()
}

pub fn log_debug_queue(
    connection_id: Option<&str>,
    in_flight: usize,
    queue_depth: usize,
    inactivity_timeout_secs: u64,
    total_timeout_secs: u64,
) {
    tracing::debug!(
        connection_id = connection_id.unwrap_or("-"),
        in_flight,
        queue_depth,
        inactivity_timeout_secs,
        total_timeout_secs,
        "resource control"
    );
}

#[cfg(test)]
mod tests {
    use super::{ErrorCode, tool_result_log_code};

    #[test]
    fn converted_error_results_are_not_logged_as_ok() {
        assert_eq!(
            tool_result_log_code(Some(true), Some("LIMIT_EXCEEDED")),
            "LIMIT_EXCEEDED"
        );
        assert_eq!(tool_result_log_code(Some(false), None), "OK");
    }

    /// A failure with no code at all, and a failure whose code is not one of
    /// this protocol's own, are both `UNKNOWN` rather than the safety-limit
    /// tag they used to share. Neither is a limit breach, and mislabelling
    /// one as one is the exact bug this whole function exists to stop.
    #[test]
    fn a_failure_without_a_recognizable_code_is_unknown_not_a_limit_breach() {
        assert_eq!(tool_result_log_code(Some(true), None), "UNKNOWN");
        assert_eq!(
            tool_result_log_code(Some(true), Some("NOT_A_REAL_CODE")),
            "UNKNOWN"
        );
    }

    /// Every code the protocol declares must log as itself, not as
    /// `LIMIT_EXCEEDED` by accident. The expected tag comes from the enum's
    /// own `Serialize` impl — the actual wire spelling — rather than from a
    /// second hand-copied string, so this catches a typo in the log arm the
    /// same way it would catch a missing one. A member missing its arm in
    /// `known_error_code_tag` fails to compile before this test can even run,
    /// which is the stronger half of the guarantee; this test additionally
    /// pins that every arm answers with the *right* tag, and stands as the
    /// regression test for `NODE_NOT_VISIBLE` and `INVALID_CURSOR`, which
    /// both fell through to `LIMIT_EXCEEDED` before this fix.
    #[test]
    fn every_protocol_error_code_logs_as_itself() {
        for code in [
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
        ] {
            let wire_tag = match serde_json::to_value(code).expect("ErrorCode always serializes") {
                serde_json::Value::String(tag) => tag,
                other => panic!("ErrorCode serialized to a non-string: {other:?}"),
            };
            assert_eq!(
                tool_result_log_code(Some(true), Some(&wire_tag)),
                wire_tag,
                "log arm for {code:?} must echo its own wire tag, not fall through to a default"
            );
        }
    }
}
