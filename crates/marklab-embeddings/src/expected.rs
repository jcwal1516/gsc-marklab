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
