//! Typed public-contract to broker-protocol mapping.

use crate::content::{EnvelopeContext, account_screenshot_result};
use crate::contracts::*;
use crate::names::ToolName;
use figma_dev_mcp_broker::{BrokerClient, OpenCall};
use figma_dev_mcp_protocol::{
    domain::ConnectionId,
    error::{ErrorCode, ToolError},
    wire::{BrokerCall, BrokerResult, Invocation, ReadOperation, ReadResult},
};
use rmcp::{
    model::{JsonObject, ProgressNotificationParam, ProgressToken},
    service::{Peer, RoleServer},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, from_value, to_value};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub struct DispatchContext<'a> {
    pub cancellation: &'a CancellationToken,
    pub progress_token: Option<ProgressToken>,
    pub peer: &'a Peer<RoleServer>,
    pub envelope: &'a EnvelopeContext,
}

pub enum DispatchOutput {
    Value(Value),
    Accounted(crate::content::AccountedCallToolResult),
}

#[derive(Debug)]
pub enum DispatchError {
    InvalidParams(String),
    Tool(ToolError),
}

fn parse<T: DeserializeOwned>(arguments: Option<JsonObject>) -> Result<T, DispatchError> {
    from_value(Value::Object(arguments.unwrap_or_default()))
        .map_err(|error| DispatchError::InvalidParams(error.to_string()))
}

async fn invoke(
    broker: &BrokerClient,
    connection_id: Option<ConnectionId>,
    operation: ReadOperation,
    context: &DispatchContext<'_>,
) -> Result<ReadResult, DispatchError> {
    let open = broker
        .open(BrokerCall::Invoke {
            connection_id,
            invocation: Box::new(Invocation { operation }),
        })
        .await
        .map_err(DispatchError::Tool)?;
    wait_for_result(broker, open, context).await
}

async fn wait_for_result(
    broker: &BrokerClient,
    mut open: OpenCall,
    context: &DispatchContext<'_>,
) -> Result<ReadResult, DispatchError> {
    let mut inactivity = Box::pin(sleep(open.inactivity_timeout));
    let total = sleep_until_deadline(open.total_deadline);
    tokio::pin!(total);
    loop {
        tokio::select! {
            biased;
            result = &mut open.result => {
                return map_broker_result(result);
            }
            progress = open.progress.recv() => {
                let Some(progress) = progress else {
                    continue;
                };
                inactivity = Box::pin(sleep(open.inactivity_timeout));
                let Some(token) = context.progress_token.clone() else {
                    continue;
                };
                let mut notification = ProgressNotificationParam::new(
                    token,
                    f64::from(progress.completed),
                )
                .with_message(progress.phase.as_str());
                if let Some(total) = progress.total {
                    notification = notification.with_total(f64::from(total));
                }
                let peer = context.peer.clone();
                tokio::spawn(async move {
                    let _ = peer.notify_progress(notification).await;
                });
            }
            _ = context.cancellation.cancelled() => {
                abort_open(broker, &open).await;
                return Err(DispatchError::Tool(ToolError::new(
                    ErrorCode::Cancelled,
                    false,
                )));
            }
            _ = &mut inactivity, if !open.inactivity_timeout.is_zero() => {
                abort_open(broker, &open).await;
                return Err(DispatchError::Tool(ToolError::new(ErrorCode::Timeout, true)));
            }
            _ = &mut total => {
                abort_open(broker, &open).await;
                return Err(DispatchError::Tool(ToolError::new(ErrorCode::Timeout, true)));
            }
        }
    }
}

fn sleep_until_deadline(deadline: tokio::time::Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(deadline)
}

async fn abort_open(broker: &BrokerClient, open: &OpenCall) {
    open.abort.cancel();
    if let (Some(connection_id), Some(request_id), BrokerClient::Local(local)) =
        (&open.connection_id, &open.request_id, broker)
    {
        let _ = local.cancel(connection_id, request_id).await;
    }
}

fn map_broker_result(
    result: Result<Result<BrokerResult, ToolError>, tokio::sync::oneshot::error::RecvError>,
) -> Result<ReadResult, DispatchError> {
    match result
        .map_err(|_| DispatchError::Tool(ToolError::new(ErrorCode::ConnectionLost, true)))?
        .map_err(DispatchError::Tool)?
    {
        BrokerResult::Invocation { result } => Ok(result),
        BrokerResult::Files { .. } => Err(DispatchError::Tool(ToolError::new(
            ErrorCode::InternalError,
            false,
        ))),
    }
}

fn result_value<O: serde::Serialize>(value: O) -> Result<Value, DispatchError> {
    to_value(value)
        .map_err(|_| DispatchError::Tool(ToolError::new(ErrorCode::InternalError, false)))
}

macro_rules! read_branch {
    ($args:expr, $broker:expr, $ctx:expr, $input:ty, $operation:ident, $result:ident, $output:ty) => {{
        let public: $input = parse($args)?;
        let mut input = public.into_protocol();
        let connection_id = input.connection_id.take();
        let result = invoke(
            $broker,
            connection_id,
            ReadOperation::$operation(input),
            $ctx,
        )
        .await?;
        match result {
            ReadResult::$result(value) => {
                result_value(<$output>::from_protocol(value)).map(DispatchOutput::Value)
            }
            _ => Err(DispatchError::Tool(ToolError::new(
                ErrorCode::InternalError,
                false,
            ))),
        }
    }};
}

pub async fn execute(
    broker: &BrokerClient,
    name: ToolName,
    arguments: Option<JsonObject>,
    context: &DispatchContext<'_>,
) -> Result<DispatchOutput, DispatchError> {
    match name {
        ToolName::ListFiles => {
            let _: ListFilesInput = parse(arguments)?;
            let result = broker
                .call(BrokerCall::ListFiles {}, context.cancellation)
                .await
                .map_err(DispatchError::Tool)?;
            match result {
                BrokerResult::Files { result } => {
                    result_value(ListFilesResult::from_protocol(result)).map(DispatchOutput::Value)
                }
                BrokerResult::Invocation { .. } => Err(DispatchError::Tool(ToolError::new(
                    ErrorCode::InternalError,
                    false,
                ))),
            }
        }
        ToolName::GetMetadata => read_branch!(
            arguments,
            broker,
            context,
            GetMetadataInput,
            GetMetadata,
            GetMetadata,
            GetMetadataResult
        ),
        ToolName::GetSelection => read_branch!(
            arguments,
            broker,
            context,
            GetSelectionInput,
            GetSelection,
            GetSelection,
            GetSelectionResult
        ),
        ToolName::GetNodes => read_branch!(
            arguments,
            broker,
            context,
            GetNodesInput,
            GetNodes,
            GetNodes,
            GetNodesResult
        ),
        ToolName::SearchNodes => read_branch!(
            arguments,
            broker,
            context,
            SearchNodesInput,
            SearchNodes,
            SearchNodes,
            SearchNodesResult
        ),
        ToolName::GetDesignContext => read_branch!(
            arguments,
            broker,
            context,
            GetDesignContextInput,
            GetDesignContext,
            GetDesignContext,
            GetDesignContextResult
        ),
        ToolName::GetStyles => read_branch!(
            arguments,
            broker,
            context,
            GetStylesInput,
            GetStyles,
            GetStyles,
            GetStylesResult
        ),
        ToolName::GetVariables => read_branch!(
            arguments,
            broker,
            context,
            GetVariablesInput,
            GetVariables,
            GetVariables,
            GetVariablesResult
        ),
        ToolName::GetComponents => read_branch!(
            arguments,
            broker,
            context,
            GetComponentsInput,
            GetComponents,
            GetComponents,
            GetComponentsResult
        ),
        ToolName::GetFonts => read_branch!(
            arguments,
            broker,
            context,
            GetFontsInput,
            GetFonts,
            GetFonts,
            GetFontsResult
        ),
        ToolName::GetDevModeData => read_branch!(
            arguments,
            broker,
            context,
            GetDevModeDataInput,
            GetDevModeData,
            GetDevModeData,
            GetDevModeDataResult
        ),
        ToolName::GetReactions => read_branch!(
            arguments,
            broker,
            context,
            GetReactionsInput,
            GetReactions,
            GetReactions,
            GetReactionsResult
        ),
        ToolName::GetMotion => read_branch!(
            arguments,
            broker,
            context,
            GetMotionInput,
            GetMotion,
            GetMotion,
            GetMotionResult
        ),
        ToolName::GetScreenshot => {
            let public: GetScreenshotInput = parse(arguments)?;
            let input = public.into_protocol();
            let result = invoke(
                broker,
                input.connection_id().cloned(),
                ReadOperation::GetScreenshot(input),
                context,
            )
            .await?;
            match result {
                ReadResult::GetScreenshot(value) => {
                    account_screenshot_result(context.envelope, value)
                        .map(DispatchOutput::Accounted)
                        .map_err(DispatchError::Tool)
                }
                _ => Err(DispatchError::Tool(ToolError::new(
                    ErrorCode::InternalError,
                    false,
                ))),
            }
        }
    }
}
