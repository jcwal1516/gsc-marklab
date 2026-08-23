#[cfg(any(feature = "parquet", test))]
use std::io::{self, Read};

use marklab_data::CellId;
use marklab_project::ContentDigest;

use crate::{digest::FramedDigest, EmbeddingError};

const MAGIC: &[u8; 8] = b"ML-ECS\0\x01";
const DIGEST_DOMAIN: &[u8] = b"marklab-cell-embedding-expected-cells-v1";

/// Immutable canonical set of cells expected to have embedding rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedCellSet {
    selection_rule: String,
    cells: Box<[CellId]>,
    logical_digest: ContentDigest,
}

impl ExpectedCellSet {
    /// Validate a bounded rule and strictly increasing unique cell IDs.
    pub fn new(
        selection_rule: impl Into<String>,
        cells: Vec<CellId>,
    ) -> Result<Self, EmbeddingError> {
        let selection_rule = selection_rule.into();
        if !valid_selection_rule(&selection_rule) {
            return Err(EmbeddingError::InvalidSelectionRule);
        }
        if cells.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(EmbeddingError::NonCanonicalCellOrder);
        }
        let count = u64::try_from(cells.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let logical_digest = digest(&selection_rule, count, &cells);
        Ok(Self {
            selection_rule,
            cells: cells.into_boxed_slice(),
            logical_digest,
        })
    }

    /// Bounded token naming the explicit selection rule.
    pub fn selection_rule(&self) -> &str {
        &self.selection_rule
    }

    /// Strictly increasing expected cell IDs.
    pub fn cells(&self) -> &[CellId] {
        &self.cells
    }

    /// Domain-separated logical digest independent of artifact encoding.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    /// Encode the exact streamable expected-cell artifact bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EmbeddingError> {
        let required = self.encoded_byte_len()?;
        let count = u64::try_from(self.cells.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let rule_length =
            u16::try_from(self.selection_rule.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(required)
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: required,
            })?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes.extend_from_slice(&rule_length.to_be_bytes());
        bytes.extend_from_slice(self.selection_rule.as_bytes());
        for cell in &self.cells {
            let value = cell.as_str().as_bytes();
            let length = u16::try_from(value.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
            bytes.extend_from_slice(&length.to_be_bytes());
            bytes.extend_from_slice(value);
        }
        Ok(bytes)
    }

    pub(crate) fn encoded_byte_len(&self) -> Result<usize, EmbeddingError> {
        let mut required = MAGIC.len() + size_of::<u64>() + size_of::<u16>();
        required = required
            .checked_add(self.selection_rule.len())
            .ok_or(EmbeddingError::SizeOverflow)?;
        for cell in &self.cells {
            required = required
                .checked_add(size_of::<u16>())
                .and_then(|value| value.checked_add(cell.as_str().len()))
                .ok_or(EmbeddingError::SizeOverflow)?;
        }
        Ok(required)
    }

    #[cfg(any(feature = "parquet", test))]
    pub(crate) fn compare_canonical_reader<R: Read + ?Sized>(
        &self,
        reader: &mut R,
    ) -> Result<(), ExpectedCellReaderError> {
        let encoded_len = self
            .encoded_byte_len()
            .map_err(|_| ExpectedCellReaderError::Mismatch)?;
        let count =
            u64::try_from(self.cells.len()).map_err(|_| ExpectedCellReaderError::Mismatch)?;
        let rule_length = u16::try_from(self.selection_rule.len())
            .map_err(|_| ExpectedCellReaderError::Mismatch)?;
        let mut comparator = ExpectedCellReaderComparator {
            reader,
            emitted: 0,
            encoded_len,
            scratch: [0; 8_192],
        };
        comparator.compare(MAGIC)?;
        comparator.compare(&count.to_be_bytes())?;
        comparator.compare(&rule_length.to_be_bytes())?;
        comparator.compare(self.selection_rule.as_bytes())?;
        for cell in &self.cells {
            let value = cell.as_str().as_bytes();
            let length =
                u16::try_from(value.len()).map_err(|_| ExpectedCellReaderError::Mismatch)?;
            comparator.compare(&length.to_be_bytes())?;
            comparator.compare(value)?;
        }
        comparator.finish()
    }

    /// Decode exact bytes after enforcing the caller's encoded-byte budget.
    pub fn from_bytes(bytes: &[u8], maximum_bytes: usize) -> Result<Self, EmbeddingError> {
        if bytes.len() > maximum_bytes {
            return Err(EmbeddingError::EncodedByteBudgetExceeded {
                observed: bytes.len(),
                maximum: maximum_bytes,
            });
        }
        let mut input = BinaryInput::new(bytes);
        if input.take(MAGIC.len())? != MAGIC {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        let count = input.u64()?;
        let count = usize::try_from(count).map_err(|_| EmbeddingError::SizeOverflow)?;
        let rule_length = usize::from(input.u16()?);
        let selection_rule = input.text(rule_length)?.to_owned();
        let minimum_lengths = count
            .checked_mul(size_of::<u16>())
            .ok_or(EmbeddingError::SizeOverflow)?;
        if minimum_lengths > input.remaining() {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(count)
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: count.saturating_mul(size_of::<CellId>()),
            })?;
        for _ in 0..count {
            let length = usize::from(input.u16()?);
            let value = input.text(length)?;
            cells.push(CellId::new(value).map_err(|_| EmbeddingError::InvalidBinaryEncoding)?);
        }
        if input.remaining() != 0 {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        let decoded = Self::new(selection_rule, cells)?;
        if decoded.to_bytes()?.as_slice() != bytes {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        Ok(decoded)
    }
}

#[cfg(any(feature = "parquet", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedCellReaderError {
    Mismatch,
    Read,
}

#[cfg(any(feature = "parquet", test))]
struct ExpectedCellReaderComparator<'a, R: ?Sized> {
    reader: &'a mut R,
    emitted: usize,
    encoded_len: usize,
    scratch: [u8; 8_192],
}

#[cfg(any(feature = "parquet", test))]
impl<R: Read + ?Sized> ExpectedCellReaderComparator<'_, R> {
    fn compare(&mut self, expected: &[u8]) -> Result<(), ExpectedCellReaderError> {
        let next = self
            .emitted
            .checked_add(expected.len())
            .ok_or(ExpectedCellReaderError::Mismatch)?;
        if next > self.encoded_len {
            return Err(ExpectedCellReaderError::Mismatch);
        }
        let mut remaining = expected;
        while !remaining.is_empty() {
            let length = remaining.len().min(self.scratch.len());
            match self.reader.read_exact(&mut self.scratch[..length]) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                    return Err(ExpectedCellReaderError::Mismatch);
                }
                Err(_) => return Err(ExpectedCellReaderError::Read),
            }
            if self.scratch[..length] != remaining[..length] {
                return Err(ExpectedCellReaderError::Mismatch);
            }
            remaining = &remaining[length..];
        }
        self.emitted = next;
        Ok(())
    }

    fn finish(mut self) -> Result<(), ExpectedCellReaderError> {
        if self.emitted != self.encoded_len {
            return Err(ExpectedCellReaderError::Mismatch);
        }
        loop {
            match self.reader.read(&mut self.scratch[..1]) {
                Ok(0) => return Ok(()),
                Ok(_) => return Err(ExpectedCellReaderError::Mismatch),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Err(ExpectedCellReaderError::Read),
            }
        }
    }
}

pub(crate) struct BinaryInput<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BinaryInput<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8], EmbeddingError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(EmbeddingError::SizeOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(EmbeddingError::InvalidBinaryEncoding)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, EmbeddingError> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| EmbeddingError::InvalidBinaryEncoding)?;
        Ok(u16::from_be_bytes(bytes))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, EmbeddingError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| EmbeddingError::InvalidBinaryEncoding)?;
        Ok(u64::from_be_bytes(bytes))
    }

    pub(crate) fn text(&mut self, length: usize) -> Result<&'a str, EmbeddingError> {
        std::str::from_utf8(self.take(length)?).map_err(|_| EmbeddingError::InvalidBinaryEncoding)
    }
}

fn valid_selection_rule(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b':' | b'+' | b'-' => index != 0,
            _ => false,
        })
}

fn digest(selection_rule: &str, count: u64, cells: &[CellId]) -> ContentDigest {
    let mut digest = FramedDigest::new();
    digest.field(DIGEST_DOMAIN);
    digest.field(selection_rule.as_bytes());
    digest.field(&count.to_be_bytes());
    for cell in cells {
        digest.field(cell.as_str().as_bytes());
    }
    digest.finish()
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use super::{ExpectedCellReaderError, ExpectedCellSet};
    use marklab_data::CellId;

    struct ChunkedReader<'a> {
        bytes: &'a [u8],
        offset: usize,
        maximum_chunk: usize,
    }

    impl Read for ChunkedReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let remaining = &self.bytes[self.offset..];
            let length = remaining.len().min(output.len()).min(self.maximum_chunk);
            output[..length].copy_from_slice(&remaining[..length]);
            self.offset += length;
            Ok(length)
        }
    }

    struct FailingReader<'a> {
        bytes: &'a [u8],
        offset: usize,
        fail_at: usize,
    }

    impl Read for FailingReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.offset >= self.fail_at {
                return Err(io::Error::other("injected expected-cell read failure"));
            }
            let remaining = &self.bytes[self.offset..];
            let length = remaining
                .len()
                .min(output.len())
                .min(self.fail_at - self.offset);
            output[..length].copy_from_slice(&remaining[..length]);
            self.offset += length;
            Ok(length)
        }
    }

    fn expected_cells(count: usize) -> ExpectedCellSet {
        ExpectedCellSet::new(
            "all-segmented-cells",
            (0..count)
                .map(|index| CellId::new(format!("cell-{index:06}")))
                .collect::<Result<Vec<_>, _>>()
                .expect("fixture cell IDs are valid"),
        )
        .expect("fixture expected-cell set is canonical")
    }

    #[test]
    fn canonical_reader_comparison_streams_high_cardinality_input() {
        let expected = expected_cells(4_096);
        let bytes = expected.to_bytes().expect("fixture encodes");

        for maximum_chunk in [1, 3, 8_192] {
            let mut reader = ChunkedReader {
                bytes: &bytes,
                offset: 0,
                maximum_chunk,
            };
            assert_eq!(expected.compare_canonical_reader(&mut reader), Ok(()));
            assert_eq!(reader.offset, bytes.len());
        }
    }

    #[test]
    fn canonical_reader_comparison_rejects_truncation_suffix_and_drift() {
        let expected = expected_cells(3);
        let bytes = expected.to_bytes().expect("fixture encodes");

        for truncated_at in [0, bytes.len() / 2, bytes.len() - 1] {
            let mut reader = &bytes[..truncated_at];
            assert_eq!(
                expected.compare_canonical_reader(&mut reader),
                Err(ExpectedCellReaderError::Mismatch)
            );
        }

        let mut suffixed = bytes.clone();
        suffixed.push(0);
        assert_eq!(
            expected.compare_canonical_reader(&mut suffixed.as_slice()),
            Err(ExpectedCellReaderError::Mismatch)
        );

        let mut drifted = bytes.clone();
        let final_byte = drifted.last_mut().expect("fixture is nonempty");
        *final_byte ^= 1;
        assert_eq!(
            expected.compare_canonical_reader(&mut drifted.as_slice()),
            Err(ExpectedCellReaderError::Mismatch)
        );
    }

    #[test]
    fn canonical_reader_comparison_distinguishes_io_failure_from_mismatch() {
        let expected = expected_cells(3);
        let bytes = expected.to_bytes().expect("fixture encodes");

        for fail_at in [0, bytes.len() / 2, bytes.len()] {
            let mut reader = FailingReader {
                bytes: &bytes,
                offset: 0,
                fail_at,
            };
            assert_eq!(
                expected.compare_canonical_reader(&mut reader),
                Err(ExpectedCellReaderError::Read)
            );
        }
    }
}
