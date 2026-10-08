use std::{mem::size_of, str::FromStr};

use marklab_project::{ArtifactId, ContentDigest};
use serde::de::{Error as _, MapAccess};
use serde::Serializer;

use crate::multiscale::error::MultiscaleEmbeddingError;

pub(super) const MAX_SOURCE_RECORD_BYTES: usize = 1024 * 1024 * 1024;
pub(super) const MAX_SOURCE_ROWS: usize = 100_000_000;
pub(super) const MAX_NORMALIZATION_BYTES: usize = 64 * 1024;
pub(super) const MAX_RAW_SOURCE_JSON_STRING_BYTES: usize = 6 * 255;
pub(in crate::multiscale) const MAX_SMALL_RECORD_BYTES: usize = 256 * 1024;
pub(in crate::multiscale) const MAX_RAW_SMALL_JSON_STRING_BYTES: usize = 6 * 255;
const MAX_JSON_DEPTH: usize = 8;
const MAX_OBJECT_FIELDS: usize = 256;

pub(super) use crate::canonical_token::valid_token;

pub(super) fn validate_source_row_count(count: usize) -> Result<(), MultiscaleEmbeddingError> {
    if count > MAX_SOURCE_ROWS {
        return Err(MultiscaleEmbeddingError::RowCountExceeded {
            observed: count,
            maximum: MAX_SOURCE_ROWS,
        });
    }
    Ok(())
}

pub(super) fn valid_source_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub(super) fn valid_decimal(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 || !value.is_ascii() || value.starts_with('+') {
        return false;
    }
    let (negative, unsigned) = match value.strip_prefix('-') {
        Some(unsigned) => (true, unsigned),
        None => (false, value),
    };
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
    {
        return false;
    }
    if fraction.is_some_and(|fraction| {
        fraction.is_empty()
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.ends_with('0')
    }) {
        return false;
    }
    !(negative && unsigned == "0")
}

pub(in crate::multiscale) fn preflight_json_strings(
    bytes: &[u8],
    maximum_raw_string_bytes: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    preflight_json_strings_with_scratch(bytes, maximum_raw_string_bytes).map(|_| ())
}

pub(in crate::multiscale) fn preflight_json_strings_with_scratch(
    bytes: &[u8],
    maximum_raw_string_bytes: usize,
) -> Result<usize, MultiscaleEmbeddingError> {
    let mut index = 0_usize;
    let mut depth = 0_usize;
    let mut maximum_scratch_bytes = 0_usize;
    let mut containers = [0_u8; MAX_JSON_DEPTH];
    let mut field_counts = [0_usize; MAX_JSON_DEPTH];
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                let start = index;
                let mut escaped = false;
                let mut contains_escape = false;
                while index < bytes.len() {
                    let byte = bytes[index];
                    if !escaped && byte == b'"' {
                        break;
                    }
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                        contains_escape = true;
                    }
                    index += 1;
                    if index.saturating_sub(start) > maximum_raw_string_bytes {
                        return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                    }
                }
                if index == bytes.len() {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
                if contains_escape {
                    let raw_string_bytes = index.saturating_sub(start);
                    // serde_json appends into one reusable Vec<u8>. Rust's amortized Vec growth
                    // can retain twice the requested bytes, with an eight-byte minimum for u8.
                    let scratch_capacity_bound = raw_string_bytes
                        .checked_mul(2)
                        .ok_or(MultiscaleEmbeddingError::SizeOverflow)?
                        .max(8);
                    maximum_scratch_bytes = maximum_scratch_bytes.max(scratch_capacity_bound);
                }
            }
            b'{' | b'[' => {
                if depth == MAX_JSON_DEPTH {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
                containers[depth] = bytes[index];
                field_counts[depth] = 0;
                depth += 1;
            }
            b'}' => {
                if depth == 0 || containers[depth - 1] != b'{' {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
                depth -= 1;
            }
            b']' => {
                if depth == 0 || containers[depth - 1] != b'[' {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
                depth -= 1;
            }
            b':' => {
                if depth == 0 || containers[depth - 1] != b'{' {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
                field_counts[depth - 1] = field_counts[depth - 1]
                    .checked_add(1)
                    .ok_or(MultiscaleEmbeddingError::SizeOverflow)?;
                if field_counts[depth - 1] > MAX_OBJECT_FIELDS {
                    return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
                }
            }
            _ => {}
        }
        index += 1;
    }
    if depth == 0 {
        Ok(maximum_scratch_bytes)
    } else {
        Err(MultiscaleEmbeddingError::InvalidCanonicalJson)
    }
}

pub(in crate::multiscale) fn require_decoded(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::DecodedByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

pub(in crate::multiscale) fn require_retained(
    required: usize,
    maximum: usize,
) -> Result<(), MultiscaleEmbeddingError> {
    if required > maximum {
        return Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum });
    }
    Ok(())
}

pub(super) fn checked_slots<T>(count: usize) -> Result<usize, MultiscaleEmbeddingError> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

pub(super) fn try_vec_capacity<T>(capacity: usize) -> Result<Vec<T>, MultiscaleEmbeddingError> {
    let requested = checked_slots::<T>(capacity)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| MultiscaleEmbeddingError::AllocationFailed { requested })?;
    Ok(values)
}

pub(in crate::multiscale) fn serialize_artifact<S: Serializer>(
    value: &ArtifactId,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serialize_hex(value.digest().as_bytes(), serializer)
}

pub(in crate::multiscale) fn serialize_digest<S: Serializer>(
    value: &ContentDigest,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serialize_hex(value.as_bytes(), serializer)
}

fn serialize_hex<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = [0_u8; 64];
    for (index, byte) in bytes.iter().copied().enumerate() {
        encoded[index * 2] = HEX[usize::from(byte >> 4)];
        encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    let text = std::str::from_utf8(&encoded).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(text)
}

pub(in crate::multiscale) fn expect_key<'de, A: MapAccess<'de>>(
    map: &mut A,
    expected: &str,
) -> Result<(), A::Error> {
    let observed = map
        .next_key::<&'de str>()?
        .ok_or_else(|| A::Error::custom("missing field"))?;
    if observed != expected {
        return Err(A::Error::custom("field order mismatch"));
    }
    Ok(())
}

pub(in crate::multiscale) fn expect_header<'de, A: MapAccess<'de>>(
    map: &mut A,
    expected_format: &str,
    expected_version: u32,
) -> Result<(), A::Error> {
    expect_key(map, "format")?;
    if map.next_value::<&str>()? != expected_format {
        return Err(A::Error::custom("wrong format"));
    }
    expect_key(map, "version")?;
    if map.next_value::<u32>()? != expected_version {
        return Err(A::Error::custom("wrong version"));
    }
    Ok(())
}

pub(in crate::multiscale) fn require_hex<E: serde::de::Error>(value: &str) -> Result<(), E> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(E::custom("invalid digest"));
    }
    Ok(())
}

pub(in crate::multiscale) fn parse_artifact<E: serde::de::Error>(
    value: &str,
) -> Result<ArtifactId, E> {
    ArtifactId::from_str(value).map_err(|_| E::custom("invalid artifact ID"))
}

pub(in crate::multiscale) fn parse_digest<E: serde::de::Error>(
    value: &str,
) -> Result<ContentDigest, E> {
    ContentDigest::from_str(value).map_err(|_| E::custom("invalid digest"))
}

#[cfg(test)]
mod tests {
    use super::{
        preflight_json_strings, try_vec_capacity, validate_source_row_count,
        MAX_RAW_SOURCE_JSON_STRING_BYTES, MAX_SOURCE_ROWS,
    };
    use crate::multiscale::error::MultiscaleEmbeddingError;

    #[test]
    fn source_record_row_limit_rejects_the_first_excess_row() {
        assert_eq!(validate_source_row_count(MAX_SOURCE_ROWS), Ok(()));
        assert_eq!(
            validate_source_row_count(MAX_SOURCE_ROWS + 1),
            Err(MultiscaleEmbeddingError::RowCountExceeded {
                observed: MAX_SOURCE_ROWS + 1,
                maximum: MAX_SOURCE_ROWS,
            })
        );
    }

    #[test]
    fn structural_preflight_enforces_depth_fields_and_balanced_containers() {
        assert!(preflight_json_strings(
            br#"[[[[[[[["allowed"]]]]]]]]"#,
            MAX_RAW_SOURCE_JSON_STRING_BYTES,
        )
        .is_ok());
        assert!(preflight_json_strings(
            br#"[[[[[[[[["too-deep"]]]]]]]]]"#,
            MAX_RAW_SOURCE_JSON_STRING_BYTES,
        )
        .is_err());
        assert!(preflight_json_strings(b"{]", MAX_RAW_SOURCE_JSON_STRING_BYTES).is_err());

        let mut too_many_fields = String::from("{");
        for index in 0..257 {
            if index > 0 {
                too_many_fields.push(',');
            }
            too_many_fields.push_str("\"a\":0");
        }
        too_many_fields.push('}');
        assert!(preflight_json_strings(
            too_many_fields.as_bytes(),
            MAX_RAW_SOURCE_JSON_STRING_BYTES,
        )
        .is_err());
    }

    #[test]
    fn raw_string_preflight_accepts_the_exact_limit_and_rejects_first_excess() {
        let exact = format!("\"{}\"", "x".repeat(MAX_RAW_SOURCE_JSON_STRING_BYTES));
        assert!(
            preflight_json_strings(exact.as_bytes(), MAX_RAW_SOURCE_JSON_STRING_BYTES,).is_ok()
        );
        let excess = format!("\"{}\"", "x".repeat(MAX_RAW_SOURCE_JSON_STRING_BYTES + 1));
        assert!(
            preflight_json_strings(excess.as_bytes(), MAX_RAW_SOURCE_JSON_STRING_BYTES,).is_err()
        );
        assert!(
            preflight_json_strings(br#""escaped\\\"quote""#, MAX_RAW_SOURCE_JSON_STRING_BYTES,)
                .is_ok()
        );
        assert!(
            preflight_json_strings(br#""unterminated"#, MAX_RAW_SOURCE_JSON_STRING_BYTES,).is_err()
        );
    }

    #[test]
    fn exact_capacity_failure_remains_a_typed_allocation_error() {
        assert_eq!(
            try_vec_capacity::<u8>(usize::MAX),
            Err(MultiscaleEmbeddingError::AllocationFailed {
                requested: usize::MAX,
            })
        );
    }
}
