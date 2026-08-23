use std::io::{self, Write};

use serde::Serialize;

use super::error::MultiscaleEmbeddingError;

pub(super) fn canonical_json_len<T: Serialize>(
    wire: &T,
) -> Result<usize, MultiscaleEmbeddingError> {
    let mut writer = CountingWriter { length: 0 };
    serde_json::to_writer(&mut writer, wire)
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    writer
        .length
        .checked_add(1)
        .ok_or(MultiscaleEmbeddingError::SizeOverflow)
}

pub(super) fn encode_canonical_json<T: Serialize>(
    wire: &T,
    encoded_len: usize,
    hard_maximum: usize,
) -> Result<Vec<u8>, MultiscaleEmbeddingError> {
    if encoded_len > hard_maximum {
        return Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
            observed: encoded_len,
            maximum: hard_maximum,
        });
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(encoded_len).map_err(|_| {
        MultiscaleEmbeddingError::AllocationFailed {
            requested: encoded_len,
        }
    })?;
    serde_json::to_writer(&mut bytes, wire)
        .map_err(|_| MultiscaleEmbeddingError::InvalidCanonicalJson)?;
    bytes.push(b'\n');
    if bytes.len() != encoded_len {
        return Err(MultiscaleEmbeddingError::InvalidCanonicalJson);
    }
    Ok(bytes)
}

pub(super) fn matches_canonical_json<T: Serialize>(wire: &T, expected: &[u8]) -> bool {
    let mut writer = ComparingWriter {
        expected,
        offset: 0,
        matches: true,
    };
    if serde_json::to_writer(&mut writer, wire).is_err() || writer.write_all(b"\n").is_err() {
        return false;
    }
    writer.matches && writer.offset == expected.len()
}

struct CountingWriter {
    length: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("canonical JSON length overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ComparingWriter<'a> {
    expected: &'a [u8],
    offset: usize,
    matches: bool,
}

impl Write for ComparingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self.offset.checked_add(bytes.len());
        let observed = end.and_then(|end| self.expected.get(self.offset..end));
        if observed != Some(bytes) {
            self.matches = false;
        }
        self.offset = end.unwrap_or(usize::MAX);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_json_len, encode_canonical_json, matches_canonical_json};
    use crate::multiscale::error::MultiscaleEmbeddingError;

    #[test]
    fn canonical_encoder_sizes_and_rejects_before_reserving() {
        let wire = ["a", "b"];
        let length = canonical_json_len(&wire).expect("canonical length");
        let bytes = encode_canonical_json(&wire, length, length).expect("exact hard limit");
        assert!(matches_canonical_json(&wire, &bytes));
        assert!(matches!(
            encode_canonical_json(&wire, length, length - 1),
            Err(MultiscaleEmbeddingError::EncodedByteBudgetExceeded {
                observed,
                maximum,
            }) if observed == length && maximum == length - 1
        ));
    }
}
