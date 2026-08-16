//! Closed plugin WebSocket messages and operation/result unions.

use crate::{
    deferred::{DeferredObject, decode_raw},
    domain::*,
    error::PluginFailure,
};
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::fmt;

pub use crate::domain::SelectionFlag;

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum PluginToBroker {
    Hello(Hello),
    Progress(Progress),
    Response(Response),
    Error(WireError),
    Pong(Pong),
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum BrokerToPlugin {
    Request(Request),
    Cancel(Cancel),
    Ping(Ping),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum PluginMessageTag {
    Hello,
    Progress,
    Response,
    Error,
    Pong,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum BrokerMessageTag {
    Request,
    Cancel,
    Ping,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum MessageField {
    Type,
    ProtocolVersion,
    ConnectionId,
    DisplayName,
    FileKey,
    FileName,
    CurrentPage,
    EditorType,
    PluginVersion,
    Capabilities,
    RequestId,
    Completed,
    Total,
    Message,
    Result,
    Error,
    Nonce,
    DeadlineMs,
    Target,
    Operation,
}

impl<'de> Deserialize<'de> for PluginToBroker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PluginMessageVisitor)
    }
}

struct PluginMessageVisitor;

impl<'de> Visitor<'de> for PluginMessageVisitor {
    type Value = PluginToBroker;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed plugin-to-broker message")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut object = DeferredObject::new();
        while let Some(field) = map.next_key::<MessageField>()? {
            match field {
                MessageField::Type => {
                    if tag.is_some() {
                        return Err(A::Error::duplicate_field("type"));
                    }
                    tag = Some(map.next_value::<PluginMessageTag>()?);
                }
                field => {
                    let name = message_field_name(&field);
                    object.insert::<A::Error>(name, map.next_value::<Box<RawValue>>()?)?;
                }
            }
        }
        match tag.ok_or_else(|| A::Error::missing_field("type"))? {
            PluginMessageTag::Hello => object.decode().map(PluginToBroker::Hello),
            PluginMessageTag::Progress => object.decode().map(PluginToBroker::Progress),
            PluginMessageTag::Response => object.decode().map(PluginToBroker::Response),
            PluginMessageTag::Error => object.decode().map(PluginToBroker::Error),
            PluginMessageTag::Pong => object.decode().map(PluginToBroker::Pong),
        }
    }
}

impl<'de> Deserialize<'de> for BrokerToPlugin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(BrokerMessageVisitor)
    }
}

struct BrokerMessageVisitor;

impl<'de> Visitor<'de> for BrokerMessageVisitor {
    type Value = BrokerToPlugin;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed broker-to-plugin message")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut object = DeferredObject::new();
        while let Some(field) = map.next_key::<MessageField>()? {
            match field {
                MessageField::Type => {
                    if tag.is_some() {
                        return Err(A::Error::duplicate_field("type"));
                    }
                    tag = Some(map.next_value::<BrokerMessageTag>()?);
                }
                field => {
                    let name = message_field_name(&field);
                    object.insert::<A::Error>(name, map.next_value::<Box<RawValue>>()?)?;
                }
            }
        }
        match tag.ok_or_else(|| A::Error::missing_field("type"))? {
            BrokerMessageTag::Request => object.decode().map(BrokerToPlugin::Request),
            BrokerMessageTag::Cancel => object.decode().map(BrokerToPlugin::Cancel),
            BrokerMessageTag::Ping => object.decode().map(BrokerToPlugin::Ping),
        }
    }
}

fn message_field_name(field: &MessageField) -> &'static str {
    match field {
        MessageField::Type => "type",
        MessageField::ProtocolVersion => "protocolVersion",
        MessageField::ConnectionId => "connectionId",
        MessageField::DisplayName => "displayName",
        MessageField::FileKey => "fileKey",
        MessageField::FileName => "fileName",
        MessageField::CurrentPage => "currentPage",
        MessageField::EditorType => "editorType",
        MessageField::PluginVersion => "pluginVersion",
        MessageField::Capabilities => "capabilities",
        MessageField::RequestId => "requestId",
        MessageField::Completed => "completed",
        MessageField::Total => "total",
        MessageField::Message => "message",
        MessageField::Result => "result",
        MessageField::Error => "error",
        MessageField::Nonce => "nonce",
        MessageField::DeadlineMs => "deadlineMs",
        MessageField::Target => "target",
        MessageField::Operation => "operation",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    pub protocol_version: ProtocolVersion,
    pub connection_id: ConnectionId,
    pub display_name: DisplayText,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_key: Option<FileKey>,
    pub file_name: DisplayText,
    pub current_page: PageSummary,
    pub editor_type: DisplayText,
    pub plugin_version: DisplayText,
    pub capabilities: CapabilitySet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Progress {
    pub request_id: RequestId,
    pub completed: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<DisplayText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub request_id: RequestId,
    pub result: ReadResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireError {
    pub request_id: RequestId,
    pub error: PluginFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pong {
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub request_id: RequestId,
    pub deadline_ms: u64,
    pub target: RequestTarget,
    pub operation: ReadOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_key: Option<FileKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub request_id: RequestId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ping {
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "operation",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReadOperation {
    GetMetadata(GetMetadataInput),
    GetSelection(GetSelectionInput),
    GetNodes(GetNodesInput),
    SearchNodes(SearchNodesInput),
    GetDesignContext(GetDesignContextInput),
    GetStyles(GetStylesInput),
    GetVariables(GetVariablesInput),
    GetComponents(GetComponentsInput),
    GetFonts(GetFontsInput),
    GetDevModeData(GetDevModeDataInput),
    GetReactions(GetReactionsInput),
    GetMotion(GetMotionInput),
    GetScreenshot(GetScreenshotInput),
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "operation",
    content = "result",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ReadResult {
    GetMetadata(GetMetadataResult),
    GetSelection(GetSelectionResult),
    GetNodes(GetNodesResult),
    SearchNodes(SearchNodesResult),
    GetDesignContext(GetDesignContextResult),
    GetStyles(GetStylesResult),
    GetVariables(GetVariablesResult),
    GetComponents(GetComponentsResult),
    GetFonts(GetFontsResult),
    GetDevModeData(GetDevModeDataResult),
    GetReactions(GetReactionsResult),
    GetMotion(GetMotionResult),
    GetScreenshot(GetScreenshotResult),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReadTag {
    GetMetadata,
    GetSelection,
    GetNodes,
    SearchNodes,
    GetDesignContext,
    GetStyles,
    GetVariables,
    GetComponents,
    GetFonts,
    GetDevModeData,
    GetReactions,
    GetMotion,
    GetScreenshot,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum OperationField {
    Operation,
    Input,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum ResultField {
    Operation,
    Result,
}

impl<'de> Deserialize<'de> for ReadOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ReadOperationVisitor)
    }
}

struct ReadOperationVisitor;

impl<'de> Visitor<'de> for ReadOperationVisitor {
    type Value = ReadOperation;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed read operation")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut input = None;
        while let Some(field) = map.next_key::<OperationField>()? {
            match field {
                OperationField::Operation => {
                    if tag.is_some() {
                        return Err(A::Error::duplicate_field("operation"));
                    }
                    tag = Some(map.next_value::<ReadTag>()?);
                }
                OperationField::Input => {
                    if input.is_some() {
                        return Err(A::Error::duplicate_field("input"));
                    }
                    input = Some(map.next_value::<Box<RawValue>>()?);
                }
            }
        }
        let input = input.ok_or_else(|| A::Error::missing_field("input"))?;
        match tag.ok_or_else(|| A::Error::missing_field("operation"))? {
            ReadTag::GetMetadata => decode_raw(&input).map(ReadOperation::GetMetadata),
            ReadTag::GetSelection => decode_raw(&input).map(ReadOperation::GetSelection),
            ReadTag::GetNodes => decode_raw(&input).map(ReadOperation::GetNodes),
            ReadTag::SearchNodes => decode_raw(&input).map(ReadOperation::SearchNodes),
            ReadTag::GetDesignContext => decode_raw(&input).map(ReadOperation::GetDesignContext),
            ReadTag::GetStyles => decode_raw(&input).map(ReadOperation::GetStyles),
            ReadTag::GetVariables => decode_raw(&input).map(ReadOperation::GetVariables),
            ReadTag::GetComponents => decode_raw(&input).map(ReadOperation::GetComponents),
            ReadTag::GetFonts => decode_raw(&input).map(ReadOperation::GetFonts),
            ReadTag::GetDevModeData => decode_raw(&input).map(ReadOperation::GetDevModeData),
            ReadTag::GetReactions => decode_raw(&input).map(ReadOperation::GetReactions),
            ReadTag::GetMotion => decode_raw(&input).map(ReadOperation::GetMotion),
            ReadTag::GetScreenshot => decode_raw(&input).map(ReadOperation::GetScreenshot),
        }
    }
}

impl<'de> Deserialize<'de> for ReadResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ReadResultVisitor)
    }
}

struct ReadResultVisitor;

impl<'de> Visitor<'de> for ReadResultVisitor {
    type Value = ReadResult;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed read result")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut result = None;
        while let Some(field) = map.next_key::<ResultField>()? {
            match field {
                ResultField::Operation => {
                    if tag.is_some() {
                        return Err(A::Error::duplicate_field("operation"));
                    }
                    tag = Some(map.next_value::<ReadTag>()?);
                }
                ResultField::Result => {
                    if result.is_some() {
                        return Err(A::Error::duplicate_field("result"));
                    }
                    result = Some(map.next_value::<Box<RawValue>>()?);
                }
            }
        }
        let result = result.ok_or_else(|| A::Error::missing_field("result"))?;
        match tag.ok_or_else(|| A::Error::missing_field("operation"))? {
            ReadTag::GetMetadata => decode_raw(&result).map(ReadResult::GetMetadata),
            ReadTag::GetSelection => decode_raw(&result).map(ReadResult::GetSelection),
            ReadTag::GetNodes => decode_raw(&result).map(ReadResult::GetNodes),
            ReadTag::SearchNodes => decode_raw(&result).map(ReadResult::SearchNodes),
            ReadTag::GetDesignContext => decode_raw(&result).map(ReadResult::GetDesignContext),
            ReadTag::GetStyles => decode_raw(&result).map(ReadResult::GetStyles),
            ReadTag::GetVariables => decode_raw(&result).map(ReadResult::GetVariables),
            ReadTag::GetComponents => decode_raw(&result).map(ReadResult::GetComponents),
            ReadTag::GetFonts => decode_raw(&result).map(ReadResult::GetFonts),
            ReadTag::GetDevModeData => decode_raw(&result).map(ReadResult::GetDevModeData),
            ReadTag::GetReactions => decode_raw(&result).map(ReadResult::GetReactions),
            ReadTag::GetMotion => decode_raw(&result).map(ReadResult::GetMotion),
            ReadTag::GetScreenshot => decode_raw(&result).map(ReadResult::GetScreenshot),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invocation {
    pub operation: ReadOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "call",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BrokerCall {
    ListFiles {},
    Invoke {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connection_id: Option<ConnectionId>,
        invocation: Box<Invocation>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BrokerResult {
    Files { result: ListFilesResult },
    Invocation { result: ReadResult },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum BrokerCallTag {
    ListFiles,
    Invoke,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum BrokerCallField {
    Call,
    ConnectionId,
    Invocation,
}

impl<'de> Deserialize<'de> for BrokerCall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(BrokerCallVisitor)
    }
}

struct BrokerCallVisitor;

impl<'de> Visitor<'de> for BrokerCallVisitor {
    type Value = BrokerCall;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed broker call")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut connection_id = None;
        let mut invocation = None;
        while let Some(field) = map.next_key::<BrokerCallField>()? {
            match field {
                BrokerCallField::Call => {
                    if tag.replace(map.next_value::<BrokerCallTag>()?).is_some() {
                        return Err(A::Error::duplicate_field("call"));
                    }
                }
                BrokerCallField::ConnectionId => {
                    if connection_id
                        .replace(map.next_value::<ConnectionId>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("connectionId"));
                    }
                }
                BrokerCallField::Invocation => {
                    if invocation
                        .replace(map.next_value::<Box<Invocation>>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("invocation"));
                    }
                }
            }
        }
        match tag.ok_or_else(|| A::Error::missing_field("call"))? {
            BrokerCallTag::ListFiles => {
                if connection_id.is_some() || invocation.is_some() {
                    return Err(A::Error::custom(
                        "listFiles call cannot contain invoke fields",
                    ));
                }
                Ok(BrokerCall::ListFiles {})
            }
            BrokerCallTag::Invoke => Ok(BrokerCall::Invoke {
                connection_id,
                invocation: invocation.ok_or_else(|| A::Error::missing_field("invocation"))?,
            }),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum BrokerResultTag {
    Files,
    Invocation,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum BrokerResultField {
    Kind,
    Result,
}

impl<'de> Deserialize<'de> for BrokerResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(BrokerResultVisitor)
    }
}

struct BrokerResultVisitor;

impl<'de> Visitor<'de> for BrokerResultVisitor {
    type Value = BrokerResult;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed broker result")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut result = None;
        while let Some(field) = map.next_key::<BrokerResultField>()? {
            match field {
                BrokerResultField::Kind => {
                    if tag.replace(map.next_value::<BrokerResultTag>()?).is_some() {
                        return Err(A::Error::duplicate_field("kind"));
                    }
                }
                BrokerResultField::Result => {
                    if result.replace(map.next_value::<Box<RawValue>>()?).is_some() {
                        return Err(A::Error::duplicate_field("result"));
                    }
                }
            }
        }
        let result = result.ok_or_else(|| A::Error::missing_field("result"))?;
        match tag.ok_or_else(|| A::Error::missing_field("kind"))? {
            BrokerResultTag::Files => {
                decode_raw(&result).map(|result| BrokerResult::Files { result })
            }
            BrokerResultTag::Invocation => {
                decode_raw(&result).map(|result| BrokerResult::Invocation { result })
            }
        }
    }
}
