//! Bounded raw JSON deferral for order-independent discriminators.

use crate::limits::MAX_ENVELOPE_BYTES;
use serde::Deserialize;
use serde_json::value::RawValue;

pub(crate) struct DeferredObject {
    fields: Vec<DeferredField>,
    encoded_bytes: usize,
}

struct DeferredField {
    name: &'static str,
    value: Box<RawValue>,
}

impl DeferredObject {
    pub(crate) fn new() -> Self {
        Self {
            fields: Vec::new(),
            encoded_bytes: 2,
        }
    }

    pub(crate) fn insert<E>(&mut self, name: &'static str, value: Box<RawValue>) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        if self.fields.iter().any(|field| field.name == name) {
            return Err(E::duplicate_field(name));
        }
        let syntax_bytes = name.len().saturating_add(4);
        let next = self
            .encoded_bytes
            .checked_add(syntax_bytes)
            .and_then(|bytes| bytes.checked_add(value.get().len()))
            .ok_or_else(|| E::custom("deferred JSON length overflow"))?;
        if next > MAX_ENVELOPE_BYTES {
            return Err(E::custom(format_args!(
                "deferred JSON exceeds {MAX_ENVELOPE_BYTES} bytes"
            )));
        }
        self.encoded_bytes = next;
        self.fields.push(DeferredField { name, value });
        Ok(())
    }

    pub(crate) fn decode<T, E>(self) -> Result<T, E>
    where
        T: for<'de> Deserialize<'de>,
        E: serde::de::Error,
    {
        let mut encoded = Vec::with_capacity(self.encoded_bytes);
        encoded.push(b'{');
        for (index, field) in self.fields.into_iter().enumerate() {
            if index != 0 {
                encoded.push(b',');
            }
            encoded.push(b'"');
            encoded.extend_from_slice(field.name.as_bytes());
            encoded.extend_from_slice(b"\":");
            encoded.extend_from_slice(field.value.get().as_bytes());
        }
        encoded.push(b'}');
        if encoded.len() > MAX_ENVELOPE_BYTES {
            return Err(E::custom(format_args!(
                "deferred JSON exceeds {MAX_ENVELOPE_BYTES} bytes"
            )));
        }
        serde_json::from_slice(&encoded).map_err(E::custom)
    }
}

pub(crate) fn decode_raw<T, E>(raw: &RawValue) -> Result<T, E>
where
    T: for<'de> Deserialize<'de>,
    E: serde::de::Error,
{
    if raw.get().len() > MAX_ENVELOPE_BYTES {
        return Err(E::custom(format_args!(
            "deferred JSON exceeds {MAX_ENVELOPE_BYTES} bytes"
        )));
    }
    serde_json::from_str(raw.get()).map_err(E::custom)
}
