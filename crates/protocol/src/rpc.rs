//! Multiplexed frontend-to-leader RPC and bounded length framing.

use crate::{
    domain::{BoundaryValueError, DisplayText, ProtocolVersion, RequestId, bounded_string_schema},
    error::ToolError,
    limits::{MAX_ENVELOPE_BYTES, MAX_IDENTIFIER_BYTES},
    wire::{BrokerCall, BrokerResult},
};

pub const FRONTEND_PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendHello {
    pub protocol_version: ProtocolVersion,
    pub frontend_id: RpcRequestId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FrontendHandshake {
    Ready,
    Rejected { error: ToolError },
}

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{DeserializeOwned, Error as _, MapAccess, Visitor},
};
use std::{
    borrow::Cow,
    fmt,
    io::{Read, Write},
};

/// Correlates a frontend request with the elected leader.
///
/// This is deliberately distinct from the plugin [`crate::domain::RequestId`], which the
/// leader creates only after routing and admission.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RpcRequestId(RequestId);

impl RpcRequestId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for RpcRequestId {
    type Error = BoundaryValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RequestId::try_from(value).map(Self)
    }
}

impl TryFrom<&str> for RpcRequestId {
    type Error = BoundaryValueError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        RequestId::try_from(value).map(Self)
    }
}

impl<'de> Deserialize<'de> for RpcRequestId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RequestId::deserialize(deserializer).map(Self)
    }
}

impl JsonSchema for RpcRequestId {
    fn schema_name() -> Cow<'static, str> {
        "RpcRequestId".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        bounded_string_schema(
            generator,
            MAX_IDENTIFIER_BYTES,
            "frontend RPC request identifier",
            false,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FrontendToLeader {
    Request {
        rpc_request_id: RpcRequestId,
        call: Box<BrokerCall>,
    },
    Cancel {
        rpc_request_id: RpcRequestId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcProgress {
    pub completed: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<DisplayText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LeaderToFrontend {
    Progress {
        rpc_request_id: RpcRequestId,
        progress: RpcProgress,
    },
    Response {
        rpc_request_id: RpcRequestId,
        result: BrokerResult,
    },
    Error {
        rpc_request_id: RpcRequestId,
        error: ToolError,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum FrontendMessageTag {
    Request,
    Cancel,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum FrontendMessageField {
    Type,
    RpcRequestId,
    Call,
}

impl<'de> Deserialize<'de> for FrontendToLeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(FrontendMessageVisitor)
    }
}

struct FrontendMessageVisitor;

impl<'de> Visitor<'de> for FrontendMessageVisitor {
    type Value = FrontendToLeader;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed frontend-to-leader message")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut rpc_request_id = None;
        let mut call = None;
        while let Some(field) = map.next_key::<FrontendMessageField>()? {
            match field {
                FrontendMessageField::Type => {
                    if tag
                        .replace(map.next_value::<FrontendMessageTag>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("type"));
                    }
                }
                FrontendMessageField::RpcRequestId => {
                    if rpc_request_id
                        .replace(map.next_value::<RpcRequestId>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("rpcRequestId"));
                    }
                }
                FrontendMessageField::Call => {
                    if call.replace(map.next_value::<Box<BrokerCall>>()?).is_some() {
                        return Err(A::Error::duplicate_field("call"));
                    }
                }
            }
        }
        let rpc_request_id =
            rpc_request_id.ok_or_else(|| A::Error::missing_field("rpcRequestId"))?;
        match tag.ok_or_else(|| A::Error::missing_field("type"))? {
            FrontendMessageTag::Request => Ok(FrontendToLeader::Request {
                rpc_request_id,
                call: call.ok_or_else(|| A::Error::missing_field("call"))?,
            }),
            FrontendMessageTag::Cancel => {
                if call.is_some() {
                    return Err(A::Error::custom("cancel message cannot contain call"));
                }
                Ok(FrontendToLeader::Cancel { rpc_request_id })
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum LeaderMessageTag {
    Progress,
    Response,
    Error,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "camelCase")]
enum LeaderMessageField {
    Type,
    RpcRequestId,
    Progress,
    Result,
    Error,
}

impl<'de> Deserialize<'de> for LeaderToFrontend {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(LeaderMessageVisitor)
    }
}

struct LeaderMessageVisitor;

impl<'de> Visitor<'de> for LeaderMessageVisitor {
    type Value = LeaderToFrontend;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a closed leader-to-frontend message")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut tag = None;
        let mut rpc_request_id = None;
        let mut progress = None;
        let mut result = None;
        let mut error = None;
        while let Some(field) = map.next_key::<LeaderMessageField>()? {
            match field {
                LeaderMessageField::Type => {
                    if tag.replace(map.next_value::<LeaderMessageTag>()?).is_some() {
                        return Err(A::Error::duplicate_field("type"));
                    }
                }
                LeaderMessageField::RpcRequestId => {
                    if rpc_request_id
                        .replace(map.next_value::<RpcRequestId>()?)
                        .is_some()
                    {
                        return Err(A::Error::duplicate_field("rpcRequestId"));
                    }
                }
                LeaderMessageField::Progress => {
                    if progress.replace(map.next_value::<RpcProgress>()?).is_some() {
                        return Err(A::Error::duplicate_field("progress"));
                    }
                }
                LeaderMessageField::Result => {
                    if result.replace(map.next_value::<BrokerResult>()?).is_some() {
                        return Err(A::Error::duplicate_field("result"));
                    }
                }
                LeaderMessageField::Error => {
                    if error.replace(map.next_value::<ToolError>()?).is_some() {
                        return Err(A::Error::duplicate_field("error"));
                    }
                }
            }
        }
        let rpc_request_id =
            rpc_request_id.ok_or_else(|| A::Error::missing_field("rpcRequestId"))?;
        match tag.ok_or_else(|| A::Error::missing_field("type"))? {
            LeaderMessageTag::Progress => {
                reject_leader_fields::<A::Error>(&result, &error, "progress")?;
                Ok(LeaderToFrontend::Progress {
                    rpc_request_id,
                    progress: progress.ok_or_else(|| A::Error::missing_field("progress"))?,
                })
            }
            LeaderMessageTag::Response => {
                if progress.is_some() || error.is_some() {
                    return Err(A::Error::custom(
                        "response message contains variant-only fields",
                    ));
                }
                Ok(LeaderToFrontend::Response {
                    rpc_request_id,
                    result: result.ok_or_else(|| A::Error::missing_field("result"))?,
                })
            }
            LeaderMessageTag::Error => {
                if progress.is_some() || result.is_some() {
                    return Err(A::Error::custom(
                        "error message contains variant-only fields",
                    ));
                }
                Ok(LeaderToFrontend::Error {
                    rpc_request_id,
                    error: error.ok_or_else(|| A::Error::missing_field("error"))?,
                })
            }
        }
    }
}

fn reject_leader_fields<E>(
    result: &Option<BrokerResult>,
    error: &Option<ToolError>,
    variant: &'static str,
) -> Result<(), E>
where
    E: serde::de::Error,
{
    if result.is_some() || error.is_some() {
        return Err(E::custom(format_args!(
            "{variant} message contains variant-only fields"
        )));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame length {actual} exceeds {maximum} bytes")]
    TooLarge { actual: usize, maximum: usize },
    #[error("frame length {0} cannot be represented by the 4-byte prefix")]
    LengthOverflow(usize),
    #[error("frame contains {actual} body bytes but declares {declared}")]
    LengthMismatch { declared: usize, actual: usize },
    #[error("frame I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn encode_frame<T>(value: &T) -> Result<Vec<u8>, FrameError>
where
    T: Serialize,
{
    encode_frame_with_maximum(value, MAX_ENVELOPE_BYTES)
}

fn encode_frame_with_maximum<T>(value: &T, maximum: usize) -> Result<Vec<u8>, FrameError>
where
    T: Serialize,
{
    let mut writer = CappedWriter::new(maximum);
    if let Err(error) = serde_json::to_writer(&mut writer, value) {
        if writer.exceeded() {
            return Err(FrameError::TooLarge {
                actual: maximum.saturating_add(1),
                maximum,
            });
        }
        return Err(FrameError::Json(error));
    }
    let body = writer.into_inner();
    let length = u32::try_from(body.len()).map_err(|_| FrameError::LengthOverflow(body.len()))?;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

struct CappedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl CappedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(maximum.min(8 * 1024)),
            maximum,
            exceeded: false,
        }
    }

    fn exceeded(&self) -> bool {
        self.exceeded
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes.len());
        if buffer.len() > remaining {
            self.exceeded = true;
            return Err(std::io::Error::other(
                "serialized frame exceeds its byte limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn decode_frame<T>(frame: &[u8]) -> Result<T, FrameError>
where
    T: DeserializeOwned,
{
    if frame.len() < 4 {
        return Err(FrameError::LengthMismatch {
            declared: 4,
            actual: frame.len(),
        });
    }
    let declared = u32::from_be_bytes(frame[..4].try_into().expect("four-byte prefix")) as usize;
    validate_body_length(declared)?;
    let body = &frame[4..];
    if body.len() != declared {
        return Err(FrameError::LengthMismatch {
            declared,
            actual: body.len(),
        });
    }
    Ok(serde_json::from_slice(body)?)
}

pub fn read_frame<R, T>(reader: &mut R) -> Result<T, FrameError>
where
    R: Read,
    T: DeserializeOwned,
{
    let mut prefix = [0_u8; 4];
    reader.read_exact(&mut prefix)?;
    let declared = u32::from_be_bytes(prefix) as usize;
    validate_body_length(declared)?;

    // The size is rejected above, before allocating this body buffer.
    let mut body = vec![0_u8; declared];
    reader.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}

pub fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), FrameError>
where
    W: Write,
    T: Serialize,
{
    writer.write_all(&encode_frame(value)?)?;
    Ok(())
}

fn validate_body_length(length: usize) -> Result<(), FrameError> {
    if length > MAX_ENVELOPE_BYTES {
        return Err(FrameError::TooLarge {
            actual: length,
            maximum: MAX_ENVELOPE_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CappedWriter, FrameError, encode_frame_with_maximum};
    use serde::{Serialize, Serializer, ser::SerializeSeq};
    use std::{cell::Cell, io::Write, rc::Rc};

    struct CountingSequence {
        serialized: Rc<Cell<usize>>,
        total: usize,
    }

    impl Serialize for CountingSequence {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut sequence = serializer.serialize_seq(Some(self.total))?;
            for _ in 0..self.total {
                self.serialized.set(self.serialized.get() + 1);
                sequence.serialize_element("12345678")?;
            }
            sequence.end()
        }
    }

    #[test]
    fn streaming_encoder_aborts_at_cap_without_serializing_the_remaining_items() {
        let serialized = Rc::new(Cell::new(0));
        let error = encode_frame_with_maximum(
            &CountingSequence {
                serialized: Rc::clone(&serialized),
                total: 1_000,
            },
            32,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            FrameError::TooLarge {
                actual: 33,
                maximum: 32
            }
        ));
        assert!(serialized.get() < 1_000);

        let mut writer = CappedWriter::new(8);
        writer.write_all(b"12345678").unwrap();
        assert!(writer.write_all(b"9").is_err());
        assert_eq!(writer.bytes.len(), 8);
        assert!(writer.exceeded());
    }
}
