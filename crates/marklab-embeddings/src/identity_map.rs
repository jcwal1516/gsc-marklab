use std::collections::BTreeSet;

use marklab_data::CellId;
use marklab_project::{ArtifactId, ContentDigest};

use crate::{digest::FramedDigest, expected::BinaryInput, EmbeddingError, ExpectedCellSet};

const MAGIC: &[u8; 8] = b"ML-CIM\0\x01";
const DIGEST_DOMAIN: &[u8] = b"marklab-cell-embedding-identity-map-v1";

/// One explicit source-local identifier to canonical `CellId` mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellIdentityMapEntry {
    source_cell_id: String,
    cell_id: CellId,
}

impl CellIdentityMapEntry {
    /// Validate one bounded, control-free source-local identifier.
    pub fn new(source_cell_id: impl Into<String>, cell_id: CellId) -> Result<Self, EmbeddingError> {
        let source_cell_id = source_cell_id.into();
        if source_cell_id.is_empty()
            || source_cell_id.len() > 256
            || source_cell_id.chars().any(char::is_control)
        {
            return Err(EmbeddingError::InvalidSourceCellId);
        }
        Ok(Self {
            source_cell_id,
            cell_id,
        })
    }

    /// Source-local identifier, never inferred to be canonical identity.
    pub fn source_cell_id(&self) -> &str {
        &self.source_cell_id
    }

    /// Explicit canonical target.
    pub fn cell_id(&self) -> &CellId {
        &self.cell_id
    }
}

/// Immutable one-to-one mapping from source-local identifiers to expected cells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CellIdentityMap {
    source_cells_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    expected_cells_logical_digest: ContentDigest,
    entries: Box<[CellIdentityMapEntry]>,
    logical_digest: ContentDigest,
}

impl CellIdentityMap {
    /// Validate source ordering, uniqueness, and exact expected-cell range.
    pub fn new(
        source_cells_artifact_id: ArtifactId,
        expected_cells_artifact_id: ArtifactId,
        expected: &ExpectedCellSet,
        entries: Vec<CellIdentityMapEntry>,
    ) -> Result<Self, EmbeddingError> {
        if entries
            .windows(2)
            .any(|pair| pair[0].source_cell_id >= pair[1].source_cell_id)
        {
            return Err(EmbeddingError::NonCanonicalSourceOrder);
        }
        let targets = entries
            .iter()
            .map(|entry| entry.cell_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_targets = expected.cells().iter().cloned().collect::<BTreeSet<_>>();
        if targets.len() != entries.len() || targets != expected_targets {
            return Err(EmbeddingError::IdentityMapRangeMismatch);
        }
        let count = u64::try_from(entries.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let logical_digest = digest(
            source_cells_artifact_id,
            expected_cells_artifact_id,
            count,
            &entries,
        );
        Ok(Self {
            source_cells_artifact_id,
            expected_cells_artifact_id,
            expected_cells_logical_digest: expected.logical_digest(),
            entries: entries.into_boxed_slice(),
            logical_digest,
        })
    }

    /// Source-cell artifact whose local identifiers form this map's domain.
    pub fn source_cells_artifact_id(&self) -> ArtifactId {
        self.source_cells_artifact_id
    }

    /// Expected-cell artifact whose cells form this map's range.
    pub fn expected_cells_artifact_id(&self) -> ArtifactId {
        self.expected_cells_artifact_id
    }

    /// Logical expected-set identity validated when this map was constructed.
    pub fn expected_cells_logical_digest(&self) -> ContentDigest {
        self.expected_cells_logical_digest
    }

    /// Entries sorted by source-local identifier bytes.
    pub fn entries(&self) -> &[CellIdentityMapEntry] {
        &self.entries
    }

    /// Domain-separated digest binding dependencies and ordered pairs.
    pub fn logical_digest(&self) -> ContentDigest {
        self.logical_digest
    }

    /// Encode the exact streamable identity-map artifact bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EmbeddingError> {
        let required = self.encoded_byte_len()?;
        let count = u64::try_from(self.entries.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(required)
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: required,
            })?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&count.to_be_bytes());
        for entry in &self.entries {
            push_text(&mut bytes, &entry.source_cell_id)?;
            push_text(&mut bytes, entry.cell_id.as_str())?;
        }
        Ok(bytes)
    }

    pub(crate) fn encoded_byte_len(&self) -> Result<usize, EmbeddingError> {
        let mut required = MAGIC.len() + size_of::<u64>();
        for entry in &self.entries {
            required = required
                .checked_add(size_of::<u16>() * 2)
                .and_then(|value| value.checked_add(entry.source_cell_id.len()))
                .and_then(|value| value.checked_add(entry.cell_id.as_str().len()))
                .ok_or(EmbeddingError::SizeOverflow)?;
        }
        Ok(required)
    }

    /// Decode exact bytes and rebind their logical identity to explicit dependencies.
    pub fn from_bytes(
        bytes: &[u8],
        maximum_bytes: usize,
        source_cells_artifact_id: ArtifactId,
        expected_cells_artifact_id: ArtifactId,
        expected: &ExpectedCellSet,
    ) -> Result<Self, EmbeddingError> {
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
        let count = usize::try_from(input.u64()?).map_err(|_| EmbeddingError::SizeOverflow)?;
        let minimum_lengths = count
            .checked_mul(size_of::<u16>() * 2)
            .ok_or(EmbeddingError::SizeOverflow)?;
        if minimum_lengths > input.remaining() {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| EmbeddingError::AllocationFailed {
                requested: count.saturating_mul(size_of::<CellIdentityMapEntry>()),
            })?;
        for _ in 0..count {
            let source_length = usize::from(input.u16()?);
            let source = input.text(source_length)?.to_owned();
            let cell_length = usize::from(input.u16()?);
            let cell = CellId::new(input.text(cell_length)?)
                .map_err(|_| EmbeddingError::InvalidBinaryEncoding)?;
            entries.push(CellIdentityMapEntry::new(source, cell)?);
        }
        if input.remaining() != 0 {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        let decoded = Self::new(
            source_cells_artifact_id,
            expected_cells_artifact_id,
            expected,
            entries,
        )?;
        if decoded.to_bytes()?.as_slice() != bytes {
            return Err(EmbeddingError::InvalidBinaryEncoding);
        }
        Ok(decoded)
    }
}

fn push_text(bytes: &mut Vec<u8>, value: &str) -> Result<(), EmbeddingError> {
    let length = u16::try_from(value.len()).map_err(|_| EmbeddingError::SizeOverflow)?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn digest(
    source_cells_artifact_id: ArtifactId,
    expected_cells_artifact_id: ArtifactId,
    count: u64,
    entries: &[CellIdentityMapEntry],
) -> ContentDigest {
    let mut digest = FramedDigest::new();
    digest.field(DIGEST_DOMAIN);
    digest.field(source_cells_artifact_id.digest().as_bytes());
    digest.field(expected_cells_artifact_id.digest().as_bytes());
    digest.field(&count.to_be_bytes());
    for entry in entries {
        digest.field(entry.source_cell_id.as_bytes());
        digest.field(entry.cell_id.as_str().as_bytes());
    }
    digest.finish()
}
