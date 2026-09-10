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

/// The bytes one field contributes to the encoded object: `"name":value`,
/// plus the comma that precedes every field except the first.
///
/// `decode` writes those same bytes by hand; the test module asserts the two
/// agree, because this function alone cannot enforce it.
fn field_cost(name_len: usize, value_len: usize, is_first: bool) -> usize {
    let separator = usize::from(!is_first);
    name_len + 3 + value_len + separator
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
        let cost = field_cost(name.len(), value.get().len(), self.fields.is_empty());
        let next = self
            .encoded_bytes
            .checked_add(cost)
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
        debug_assert_eq!(
            encoded.len(),
            self.encoded_bytes,
            "insert's estimate drifted from decode's encoding"
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::value::RawValue;

    fn raw(json: &str) -> Box<RawValue> {
        RawValue::from_string(json.to_owned()).expect("valid JSON")
    }

    /// The estimate `insert` accumulates must equal the bytes `decode`
    /// actually produces. This is the whole guarantee: `field_cost` removes
    /// the duplicated arithmetic, but `decode` still writes its own byte
    /// sequence by hand, so only an equality check catches drift between them.
    #[test]
    fn the_running_estimate_equals_the_bytes_decode_produces() {
        const NAMES: [&str; 4] = ["a", "detail", "", "a_rather_longer_field_name"];
        for count in 0..=4usize {
            for value in ["1", "\"\"", "\"xyz\"", "{\"nested\":[1,2,3]}"] {
                let mut object = DeferredObject::new();
                // `.copied()` matters: `insert` takes `&'static str`, and
                // `.iter()` alone yields `&&'static str`.
                for name in NAMES.iter().copied().take(count) {
                    object
                        .insert::<serde_json::Error>(name, raw(value))
                        .expect("within the ceiling");
                }
                let estimate = object.encoded_bytes;
                let decoded: Box<RawValue> = object
                    .decode::<Box<RawValue>, serde_json::Error>()
                    .expect("decodes");
                assert_eq!(
                    estimate,
                    decoded.get().len(),
                    "estimate and actual disagree at {count} field(s) of value {value}"
                );
            }
        }
    }

    /// Every other enforcement of this ceiling uses `>`, so exactly
    /// `MAX_ENVELOPE_BYTES` is legal. Before the fix `insert` refused it.
    #[test]
    fn an_object_encoding_to_exactly_the_ceiling_is_accepted() {
        // {"a":"<pad>"} is 8 bytes of syntax: 2 braces, 3 for the quoted name,
        // 1 colon, 2 quotes around the value. Derived the same way through
        // field_cost: 2 + (1 + 3 + (pad + 2) + 0) = pad + 8.
        let padding = "x".repeat(MAX_ENVELOPE_BYTES - 8);
        let mut object = DeferredObject::new();
        object
            .insert::<serde_json::Error>("a", raw(&format!("\"{padding}\"")))
            .expect("exactly the ceiling is within the ceiling");
        let decoded: Box<RawValue> = object
            .decode::<Box<RawValue>, serde_json::Error>()
            .expect("decodes at the ceiling");
        assert_eq!(decoded.get().len(), MAX_ENVELOPE_BYTES);
    }

    /// `decode`'s ceiling check is unreachable through `insert`, which refuses
    /// first. It is not unreachable: a directly constructed accumulator gets
    /// there, and this pins that the guard still refuses when it does.
    #[test]
    fn decodes_ceiling_check_refuses_an_oversized_field_set() {
        let oversized = "x".repeat(MAX_ENVELOPE_BYTES + 2);
        let value = raw(&format!("\"{oversized}\""));
        // Self-consistent, so the standing `debug_assert_eq!` in `decode`
        // holds and only the ceiling check is what refuses.
        let encoded_bytes = 2 + field_cost("a".len(), value.get().len(), true);
        let object = DeferredObject {
            fields: vec![DeferredField { name: "a", value }],
            encoded_bytes,
        };
        let error = object
            .decode::<Box<RawValue>, serde_json::Error>()
            .expect_err("the guard must refuse an oversized field set");
        assert!(
            error.to_string().contains("deferred JSON exceeds"),
            "refused for the wrong reason: {error}"
        );
    }

    /// `decode_raw` is the third of this file's three ceiling guards, and the
    /// only one nothing reached: narrowing it left the suite green where the
    /// other two turn a test red. It is the guard on every read-operation
    /// input arriving from the wire (`wire.rs` dispatches all of them through
    /// here), so an unpinned ceiling here is an unpinned ceiling on the whole
    /// read surface.
    ///
    /// Both directions are pinned, because both are ways to get it wrong: a
    /// payload one byte over must be refused, and one of exactly
    /// `MAX_ENVELOPE_BYTES` must be accepted — every other enforcement of this
    /// constant uses `>`, so the limit is inclusive by the protocol's own
    /// convention.
    #[test]
    fn decode_raw_refuses_one_byte_over_the_ceiling_and_accepts_the_ceiling() {
        // {"a":"<pad>"} is 8 bytes of syntax: 2 braces, 3 for the quoted name,
        // 1 colon, 2 quotes around the value.
        let at_ceiling = raw(&format!(
            "{{\"a\":\"{}\"}}",
            "x".repeat(MAX_ENVELOPE_BYTES - 8)
        ));
        assert_eq!(at_ceiling.get().len(), MAX_ENVELOPE_BYTES);
        decode_raw::<Box<RawValue>, serde_json::Error>(&at_ceiling)
            .expect("exactly the ceiling is within the ceiling");

        let over = raw(&format!(
            "{{\"a\":\"{}\"}}",
            "x".repeat(MAX_ENVELOPE_BYTES - 7)
        ));
        assert_eq!(over.get().len(), MAX_ENVELOPE_BYTES + 1);
        let error = decode_raw::<Box<RawValue>, serde_json::Error>(&over)
            .expect_err("one byte over the ceiling must be refused");
        assert!(
            error.to_string().contains("deferred JSON exceeds"),
            "refused for the wrong reason: {error}"
        );
    }
}
