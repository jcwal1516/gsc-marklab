use marklab_project::{ArtifactReadSeek, ContentDigest};

use super::{
    EmbeddingNpyMatrix, EmbeddingNpySummary, NpyVersion, SourceBundleBudgets, SourceBundleError,
    CELLVIT_DIMENSION,
};

/// Compatibility reader for the fixed 1,280-component CellViT source profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellVitNpySummary(pub(super) EmbeddingNpySummary);

impl CellVitNpySummary {
    /// Validate a bounded CellViT reader, retaining no vector payload.
    pub fn from_reader(
        reader: &mut dyn ArtifactReadSeek,
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        super::visit_reader(reader, budgets, &mut |_row, _column, _value| Ok(()))
    }

    /// Accepted NPY wire version.
    pub fn version(self) -> NpyVersion {
        self.0.version()
    }
    /// Exact source row count.
    pub fn row_count(self) -> u64 {
        self.0.row_count()
    }
    /// Fixed CellViT vector width.
    pub fn dimension(self) -> u32 {
        self.0.dimension()
    }
    /// Rows with finite source vectors.
    pub fn present_count(self) -> u64 {
        self.0.present_count()
    }
    /// Exact encoded source length.
    pub fn encoded_byte_len(self) -> u64 {
        self.0.encoded_byte_len()
    }
    /// Existing content identity for the exact source bytes.
    pub fn content_digest(self) -> ContentDigest {
        self.0.content_digest()
    }
}

/// Borrowed compatibility reader for the fixed 1,280-component CellViT profile.
#[derive(Clone, Copy, Debug)]
pub struct CellVitNpyMatrix<'a>(EmbeddingNpyMatrix<'a>);

impl<'a> CellVitNpyMatrix<'a> {
    /// Validate the original CellViT profile with unchanged resource and finite-value checks.
    pub fn from_bytes(
        bytes: &'a [u8],
        budgets: SourceBundleBudgets,
    ) -> Result<Self, SourceBundleError> {
        EmbeddingNpyMatrix::from_bytes_with_dimension(bytes, budgets, Some(CELLVIT_DIMENSION))
            .map(Self)
    }
    /// Exact source row count.
    pub fn row_count(self) -> u64 {
        self.0.row_count()
    }
    /// Fixed CellViT vector width.
    pub fn dimension(self) -> u32 {
        self.0.dimension()
    }
    /// Aggregate validated source facts.
    pub fn summary(self) -> CellVitNpySummary {
        CellVitNpySummary(self.0.summary())
    }
    /// Decode one finite component without an aligned typed cast.
    pub fn value(self, row: u64, column: u32) -> Result<f32, SourceBundleError> {
        self.0.value(row, column)
    }
}
