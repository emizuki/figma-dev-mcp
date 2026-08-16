//! MCP server handler.

use crate::{
    CACHE_TTL_MS, EnvelopeContext, account_call_tool_result,
    content::{structured, structured_error},
    dispatch::{DispatchContext, DispatchError, DispatchOutput, execute},
    names::ToolName,
    observability::{ToolObservation, log_tool_completion, tool_result_log_code},
    tools_catalog,
};
use figma_dev_mcp_broker::BrokerClient;
use figma_dev_mcp_protocol::error::ToolError;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, DiscoverResult, ErrorCode,
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
};
use std::borrow::Cow;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct McpService {
    broker: BrokerClient,
}

impl McpService {
    pub fn new(broker: impl Into<BrokerClient>) -> Self {
        Self {
            broker: broker.into(),
        }
    }
}

impl ServerHandler for McpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new(
            "figma-dev-mcp",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions("Read-only Figma Dev Mode inspection through a local plugin connection.")
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&[ProtocolVersion::V_2025_11_25, ProtocolVersion::V_2026_07_28])
    }

    async fn discover(
        &self,
        _context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, McpError> {
        Ok(DiscoverResult::from_server_info(
            self.supported_protocol_versions().into_owned(),
            self.get_info(),
        )
        .with_ttl_ms(CACHE_TTL_MS)
        .with_cache_scope(CacheScope::Public))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(tools_catalog())
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools_catalog()
            .tools
            .into_iter()
            .find(|tool| tool.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let name = request.name;
        let tool_name = ToolName::try_from(name.as_ref()).map_err(|_| {
            McpError::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("unknown tool: {name}"),
                None,
            )
        })?;
        crate::observability::log_debug_queue(
            request
                .arguments
                .as_ref()
                .and_then(|arguments| arguments.get("connectionId"))
                .and_then(serde_json::Value::as_str),
            figma_dev_mcp_protocol::limits::MAX_IN_FLIGHT,
            figma_dev_mcp_protocol::limits::MAX_QUEUE,
            figma_dev_mcp_protocol::limits::INACTIVITY_TIMEOUT_SECS,
            figma_dev_mcp_protocol::limits::TOTAL_TIMEOUT_SECS,
        );
        let started = Instant::now();
        let connection_id = request.arguments.as_ref().and_then(|arguments| {
            arguments
                .get("connectionId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
        let envelope = EnvelopeContext {
            request_id: context.id.clone(),
            protocol_version: context
                .protocol_version()
                .unwrap_or(ProtocolVersion::V_2025_11_25),
        };
        let dispatch = DispatchContext {
            cancellation: &context.ct,
            progress_token: context.meta.get_progress_token(),
            peer: &context.peer,
            envelope: &envelope,
        };
        let outcome = execute(&self.broker, tool_name, request.arguments, &dispatch).await;
        let duration = started.elapsed();
        match outcome {
            Ok(output) => {
                let accounted = match output {
                    DispatchOutput::Accounted(accounted) => Ok(accounted),
                    DispatchOutput::Value(value) => {
                        let result = structured(value);
                        account_call_tool_result(result, &envelope).or_else(|error| {
                            let value = serde_json::to_value(&error).map_err(|_| error.clone())?;
                            account_call_tool_result(structured_error(value), &envelope)
                        })
                    }
                };
                match accounted {
                    Ok(accounted) => {
                        let structured_code = accounted
                            .result
                            .structured_content
                            .as_ref()
                            .and_then(|value| value.get("code"))
                            .and_then(serde_json::Value::as_str);
                        log_tool_completion(&ToolObservation {
                            request_id: context.id.clone(),
                            tool_name: tool_name.as_str(),
                            connection_id,
                            duration,
                            item_count: accounted.item_count,
                            text_bytes: accounted.text_bytes,
                            envelope_bytes: accounted.envelope_bytes,
                            error_code: tool_result_log_code(
                                accounted.result.is_error,
                                structured_code,
                            ),
                        });
                        Ok(accounted.result.into())
                    }
                    Err(error) => tool_error_response(
                        error,
                        &envelope,
                        context.id.clone(),
                        tool_name.as_str(),
                        connection_id,
                        duration,
                    ),
                }
            }
            Err(DispatchError::InvalidParams(message)) => {
                Err(McpError::invalid_params(message, None))
            }
            Err(DispatchError::Tool(error)) => tool_error_response(
                error,
                &envelope,
                context.id.clone(),
                tool_name.as_str(),
                connection_id,
                duration,
            ),
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(figma_dev_mcp_prompts::prompts_catalog())
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        figma_dev_mcp_prompts::get_prompt_result(&request.name)
            .map(GetPromptResult::into)
            .ok_or_else(|| {
                McpError::invalid_params(format!("prompt '{}' not found", request.name), None)
            })
    }
}

fn tool_error_response(
    error: ToolError,
    envelope: &EnvelopeContext,
    request_id: rmcp::model::RequestId,
    tool_name: &'static str,
    connection_id: Option<String>,
    duration: std::time::Duration,
) -> Result<CallToolResponse, McpError> {
    let code = error_code_name(error.code());
    let value = serde_json::to_value(&error)
        .map_err(|_| McpError::internal_error("failed to serialize tool error", None))?;
    let result = structured_error(value);
    let accounted = account_call_tool_result(result.clone(), envelope).unwrap_or(
        crate::content::AccountedCallToolResult {
            text_bytes: 0,
            envelope_bytes: 0,
            item_count: 0,
            result,
        },
    );
    log_tool_completion(&ToolObservation {
        request_id,
        tool_name,
        connection_id,
        duration,
        item_count: accounted.item_count,
        text_bytes: accounted.text_bytes,
        envelope_bytes: accounted.envelope_bytes,
        error_code: code,
    });
    Ok(accounted.result.into())
}

fn error_code_name(code: figma_dev_mcp_protocol::error::ErrorCode) -> &'static str {
    match code {
        figma_dev_mcp_protocol::error::ErrorCode::NoFigmaConnection => "NO_FIGMA_CONNECTION",
        figma_dev_mcp_protocol::error::ErrorCode::AmbiguousConnection => "AMBIGUOUS_CONNECTION",
        figma_dev_mcp_protocol::error::ErrorCode::ConnectionNotFound => "CONNECTION_NOT_FOUND",
        figma_dev_mcp_protocol::error::ErrorCode::ConnectionLost => "CONNECTION_LOST",
        figma_dev_mcp_protocol::error::ErrorCode::ProtocolMismatch => "PROTOCOL_MISMATCH",
        figma_dev_mcp_protocol::error::ErrorCode::NodeNotFound => "NODE_NOT_FOUND",
        figma_dev_mcp_protocol::error::ErrorCode::PageNotFound => "PAGE_NOT_FOUND",
        figma_dev_mcp_protocol::error::ErrorCode::UnsupportedNode => "UNSUPPORTED_NODE",
        figma_dev_mcp_protocol::error::ErrorCode::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
        figma_dev_mcp_protocol::error::ErrorCode::UnsafeSvg => "UNSAFE_SVG",
        figma_dev_mcp_protocol::error::ErrorCode::LimitExceeded => "LIMIT_EXCEEDED",
        figma_dev_mcp_protocol::error::ErrorCode::Timeout => "TIMEOUT",
        figma_dev_mcp_protocol::error::ErrorCode::Cancelled => "CANCELLED",
        figma_dev_mcp_protocol::error::ErrorCode::InternalError => "INTERNAL_ERROR",
    }
}
