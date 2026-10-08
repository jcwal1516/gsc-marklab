use std::io::{self, Read, Write};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CanonicalJsonReaderError {
    Mismatch,
    Read,
}

pub(super) fn compare_canonical_json_reader<T, R>(
    wire: &T,
    encoded_len: usize,
    hard_maximum: usize,
    reader: &mut R,
) -> Result<(), CanonicalJsonReaderError>
where
    T: Serialize,
    R: Read + ?Sized,
{
    if encoded_len > hard_maximum {
        return Err(CanonicalJsonReaderError::Mismatch);
    }
    let mut writer = ReaderComparingWriter {
        reader,
        emitted: 0,
        encoded_len,
        failure: None,
        scratch: [0; 8_192],
    };
    if serde_json::to_writer(&mut writer, wire).is_err() || writer.write_all(b"\n").is_err() {
        return Err(writer.failure.unwrap_or(CanonicalJsonReaderError::Mismatch));
    }
    writer.finish()
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

struct ReaderComparingWriter<'a, R: ?Sized> {
    reader: &'a mut R,
    emitted: usize,
    encoded_len: usize,
    failure: Option<CanonicalJsonReaderError>,
    scratch: [u8; 8_192],
}

impl<R: Read + ?Sized> ReaderComparingWriter<'_, R> {
    fn fail(&mut self, failure: CanonicalJsonReaderError) -> io::Error {
        self.failure = Some(failure);
        io::Error::other("canonical JSON comparison failed")
    }

    fn finish(mut self) -> Result<(), CanonicalJsonReaderError> {
        if self.emitted != self.encoded_len {
            return Err(CanonicalJsonReaderError::Mismatch);
        }
        loop {
            match self.reader.read(&mut self.scratch[..1]) {
                Ok(0) => return Ok(()),
                Ok(_) => return Err(CanonicalJsonReaderError::Mismatch),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Err(CanonicalJsonReaderError::Read),
            }
        }
    }
}

impl<R: Read + ?Sized> Write for ReaderComparingWriter<'_, R> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_emitted) = self.emitted.checked_add(bytes.len()) else {
            return Err(self.fail(CanonicalJsonReaderError::Mismatch));
        };
        if next_emitted > self.encoded_len {
            return Err(self.fail(CanonicalJsonReaderError::Mismatch));
        }
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(self.scratch.len());
            match self.reader.read_exact(&mut self.scratch[..chunk_len]) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    return Err(self.fail(CanonicalJsonReaderError::Mismatch));
                }
                Err(_) => return Err(self.fail(CanonicalJsonReaderError::Read)),
            }
            if self.scratch[..chunk_len] != remaining[..chunk_len] {
                return Err(self.fail(CanonicalJsonReaderError::Mismatch));
            }
            remaining = &remaining[chunk_len..];
        }
        self.emitted = next_emitted;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
    use std::io::{self, Cursor, Read};

    use super::{
        canonical_json_len, compare_canonical_json_reader, encode_canonical_json,
        matches_canonical_json, CanonicalJsonReaderError,
    };
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

    #[test]
    fn streaming_comparator_requires_the_exact_escaped_document_and_final_newline() {
        let wire = ["quoted-\"value", "back\\slash", "multibyte-β"];
        let length = canonical_json_len(&wire).expect("canonical length");
        let bytes = encode_canonical_json(&wire, length, length).expect("canonical bytes");

        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut Cursor::new(&bytes)),
            Ok(())
        );

        for truncated in [
            &bytes[..0],
            &bytes[..bytes.len() / 2],
            &bytes[..bytes.len() - 1],
        ] {
            assert_eq!(
                compare_canonical_json_reader(&wire, length, length, &mut Cursor::new(truncated),),
                Err(CanonicalJsonReaderError::Mismatch)
            );
        }

        let mut suffixed = bytes.clone();
        suffixed.push(b' ');
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut Cursor::new(suffixed)),
            Err(CanonicalJsonReaderError::Mismatch)
        );

        let mut drifted = bytes;
        drifted[2] ^= 1;
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut Cursor::new(drifted)),
            Err(CanonicalJsonReaderError::Mismatch)
        );
    }

    struct ObservedReader<'a> {
        bytes: &'a [u8],
        offset: usize,
        maximum_chunk: usize,
        largest_request: usize,
    }

    struct FailingReader<'a> {
        bytes: &'a [u8],
        offset: usize,
        fail_after: usize,
    }

    impl Read for FailingReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.offset >= self.fail_after {
                return Err(io::Error::other("injected read failure"));
            }
            let available = self.bytes.len().saturating_sub(self.offset);
            let before_failure = self.fail_after - self.offset;
            let count = available.min(before_failure).min(output.len());
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    impl Read for ObservedReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            self.largest_request = self.largest_request.max(output.len());
            let available = self.bytes.len().saturating_sub(self.offset);
            let count = available.min(output.len()).min(self.maximum_chunk);
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    #[test]
    fn streaming_comparator_bounds_reads_and_accepts_short_readers() {
        let wire = ["x".repeat(2 * 1024 * 1024)];
        let length = canonical_json_len(&wire).expect("large canonical length");
        let bytes = encode_canonical_json(&wire, length, length).expect("large canonical bytes");

        let mut bytewise = ObservedReader {
            bytes: &bytes,
            offset: 0,
            maximum_chunk: 1,
            largest_request: 0,
        };
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut bytewise),
            Ok(())
        );
        assert!(bytewise.largest_request <= 8_192);

        let mut chunked = ObservedReader {
            bytes: &bytes,
            offset: 0,
            maximum_chunk: usize::MAX,
            largest_request: 0,
        };
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut chunked),
            Ok(())
        );
        assert_eq!(chunked.largest_request, 8_192);
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length - 1, &mut Cursor::new(bytes),),
            Err(CanonicalJsonReaderError::Mismatch)
        );
    }

    #[test]
    fn streaming_comparator_redacts_reader_failures() {
        let wire = ["reader-failure"];
        let length = canonical_json_len(&wire).expect("canonical length");
        let bytes = encode_canonical_json(&wire, length, length).expect("canonical bytes");
        let mut reader = FailingReader {
            bytes: &bytes,
            offset: 0,
            fail_after: bytes.len() / 2,
        };
        assert_eq!(
            compare_canonical_json_reader(&wire, length, length, &mut reader),
            Err(CanonicalJsonReaderError::Read)
        );
    }
}
