//! Schema-safe tool and broker logs. Design content never appears here.

use std::time::Duration;

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

pub fn tool_result_log_code(is_error: Option<bool>, structured_code: Option<&str>) -> &'static str {
    if is_error != Some(true) {
        return "OK";
    }
    match structured_code {
        Some("NO_FIGMA_CONNECTION") => "NO_FIGMA_CONNECTION",
        Some("AMBIGUOUS_CONNECTION") => "AMBIGUOUS_CONNECTION",
        Some("CONNECTION_NOT_FOUND") => "CONNECTION_NOT_FOUND",
        Some("CONNECTION_LOST") => "CONNECTION_LOST",
        Some("PROTOCOL_MISMATCH") => "PROTOCOL_MISMATCH",
        Some("NODE_NOT_FOUND") => "NODE_NOT_FOUND",
        Some("PAGE_NOT_FOUND") => "PAGE_NOT_FOUND",
        Some("UNSUPPORTED_NODE") => "UNSUPPORTED_NODE",
        Some("EMPTY_NODE_BOUNDS") => "EMPTY_NODE_BOUNDS",
        Some("CAPABILITY_UNAVAILABLE") => "CAPABILITY_UNAVAILABLE",
        Some("UNSAFE_SVG") => "UNSAFE_SVG",
        Some("LIMIT_EXCEEDED") => "LIMIT_EXCEEDED",
        Some("TIMEOUT") => "TIMEOUT",
        Some("CANCELLED") => "CANCELLED",
        Some("INTERNAL_ERROR") => "INTERNAL_ERROR",
        _ => "LIMIT_EXCEEDED",
    }
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
    use super::tool_result_log_code;

    #[test]
    fn converted_error_results_are_not_logged_as_ok() {
        assert_eq!(
            tool_result_log_code(Some(true), Some("LIMIT_EXCEEDED")),
            "LIMIT_EXCEEDED"
        );
        assert_eq!(tool_result_log_code(Some(false), None), "OK");
        assert_eq!(tool_result_log_code(Some(true), None), "LIMIT_EXCEEDED");
    }
}
