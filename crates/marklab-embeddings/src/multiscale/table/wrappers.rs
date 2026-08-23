use std::fmt;

use marklab_data::{PatchId, RegionId, SlideId};
use marklab_project::{ArtifactId, ContentDigest};

use crate::EmbeddingStatus;

use super::{MatrixBlock, MatrixCore, MatrixView, MultiscaleEmbeddingQcSummary, OwnedRow};
use crate::multiscale::{
    entity::{EmbeddingEntityKind, EntitySpec},
    error::MultiscaleEmbeddingError,
    expected::{ExpectedPatchSet, ExpectedRegionSet, ExpectedSlideSet},
};

macro_rules! define_typed_table {
    (
        $row:ident,
        $view:ident,
        $block:ident,
        $table:ident,
        $expected:ty,
        $id:ty,
        $id_method:ident,
        $row_docs:literal,
        $view_docs:literal,
        $table_docs:literal
    ) => {
        #[doc = $row_docs]
        #[derive(Clone, PartialEq)]
        pub struct $row {
            id: $id,
            status: EmbeddingStatus,
            vector: Option<Vec<f32>>,
        }

        impl fmt::Debug for $row {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($row))
                    .field("status", &self.status)
                    .finish_non_exhaustive()
            }
        }

        impl $row {
            /// Construct a present row; table construction checks dimension and finiteness.
            pub fn present(id: $id, vector: Vec<f32>) -> Self {
                Self {
                    id,
                    status: EmbeddingStatus::Present,
                    vector: Some(vector),
                }
            }

            /// Construct an explicitly non-present row without a usable vector.
            pub fn non_present(
                id: $id,
                status: EmbeddingStatus,
            ) -> Result<Self, MultiscaleEmbeddingError> {
                if status == EmbeddingStatus::Present {
                    return Err(MultiscaleEmbeddingError::StatusVectorMismatch);
                }
                Ok(Self {
                    id,
                    status,
                    vector: None,
                })
            }
        }

        impl OwnedRow<$id> for $row {
            fn id(&self) -> &$id {
                &self.id
            }

            fn vector_capacity(&self) -> usize {
                self.vector.as_ref().map_or(0, Vec::capacity)
            }

            fn into_parts(self) -> ($id, EmbeddingStatus, Option<Vec<f32>>) {
                (self.id, self.status, self.vector)
            }
        }

        #[doc = $view_docs]
        #[derive(Clone, Copy)]
        pub struct $view<'a>(MatrixView<'a, $id>);

        impl fmt::Debug for $view<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($view))
                    .field("status", &self.0.status)
                    .finish_non_exhaustive()
            }
        }

        impl<'a> $view<'a> {
            /// Typed canonical identity for this row.
            pub fn $id_method(self) -> &'a $id {
                self.0.id
            }

            /// Closed extraction-validity state.
            pub fn status(self) -> EmbeddingStatus {
                self.0.status
            }

            /// Present vector, or `None` for every non-present state.
            pub fn vector(self) -> Option<&'a [f32]> {
                self.0.vector
            }
        }

        /// Borrowed contiguous typed row block with status-aware vector access.
        #[derive(Clone, Copy)]
        pub struct $block<'a>(MatrixBlock<'a, $id>);

        impl fmt::Debug for $block<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($block))
                    .field("row_count", &self.0.ids.len())
                    .field("dimension", &self.0.dimension)
                    .finish()
            }
        }

        impl<'a> $block<'a> {
            /// Number of rows in this block.
            pub fn row_count(self) -> usize {
                self.0.row_count()
            }

            /// Fixed vector dimension shared by every row.
            pub fn dimension(self) -> u32 {
                self.0.dimension
            }

            /// Return one status-aware row relative to this block.
            pub fn row(self, index: usize) -> Result<$view<'a>, MultiscaleEmbeddingError> {
                self.0.row(index).map($view)
            }

            /// Iterate every status-aware row in canonical order.
            pub fn rows(self) -> impl ExactSizeIterator<Item = $view<'a>> + 'a {
                self.0.rows().map($view)
            }
        }

        #[doc = $table_docs]
        pub struct $table(MatrixCore<$id>);

        impl fmt::Debug for $table {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($table))
                    .field(&self.0)
                    .finish()
            }
        }

        impl $table {
            /// Validate exact expected rows and materialize one checked contiguous matrix.
            #[allow(clippy::too_many_arguments)]
            pub fn from_rows(
                dimension: u32,
                expected: &$expected,
                expected_entities_artifact_id: ArtifactId,
                support_artifact_id: ArtifactId,
                support_logical_digest: ContentDigest,
                provenance_artifact_id: ArtifactId,
                provenance_logical_digest: ContentDigest,
                rows: Vec<$row>,
                maximum_retained_bytes: usize,
            ) -> Result<Self, MultiscaleEmbeddingError> {
                MatrixCore::from_rows(
                    dimension,
                    expected.owning_slide_id(),
                    expected.ids(),
                    expected_entities_artifact_id,
                    expected.logical_digest(),
                    support_artifact_id,
                    support_logical_digest,
                    provenance_artifact_id,
                    provenance_logical_digest,
                    rows,
                    maximum_retained_bytes,
                )
                .map(Self)
            }

            /// Typed entity family for this table.
            pub fn entity_kind(&self) -> EmbeddingEntityKind {
                <$id as EntitySpec>::KIND
            }

            /// Owning slide shared by every row.
            pub fn owning_slide_id(&self) -> &SlideId {
                &self.0.owning_slide_id
            }

            /// Number of canonical rows.
            pub fn row_count(&self) -> usize {
                self.0.ids.len()
            }

            /// Fixed vector dimension.
            pub fn dimension(&self) -> u32 {
                self.0.dimension
            }

            /// Status-aware row view that hides every non-present filler.
            pub fn row(&self, index: usize) -> Result<$view<'_>, MultiscaleEmbeddingError> {
                self.0.row(index).map($view)
            }

            /// Borrow a checked contiguous row block without exposing non-present fillers.
            pub fn block(
                &self,
                start: usize,
                row_count: usize,
            ) -> Result<$block<'_>, MultiscaleEmbeddingError> {
                self.0.block(start, row_count).map($block)
            }

            /// Recompute factual QC and logical identity across bounded borrowed blocks.
            pub fn scan_qc(
                &self,
                maximum_block_rows: usize,
            ) -> Result<MultiscaleEmbeddingQcSummary, MultiscaleEmbeddingError> {
                self.0.scan_qc(maximum_block_rows)
            }

            /// Factual counts, shape, entity kind, zero-vector count, and logical identity.
            pub fn qc_summary(&self) -> MultiscaleEmbeddingQcSummary {
                self.0.qc_summary
            }

            /// Format-independent logical identity.
            pub fn logical_digest(&self) -> ContentDigest {
                self.0.qc_summary.logical_digest
            }

            /// Exact expected-set artifact identity bound by this table.
            pub fn expected_entities_artifact_id(&self) -> ArtifactId {
                self.0.expected_entities_artifact_id
            }

            /// Exact expected-set logical identity bound by this table.
            pub fn expected_entities_logical_digest(&self) -> ContentDigest {
                self.0.expected_entities_logical_digest
            }

            /// Exact support artifact identity bound by this table.
            pub fn support_artifact_id(&self) -> ArtifactId {
                self.0.support_artifact_id
            }

            /// Exact support logical identity bound by this table.
            pub fn support_logical_digest(&self) -> ContentDigest {
                self.0.support_logical_digest
            }

            /// Exact provenance artifact identity bound by this table.
            pub fn provenance_artifact_id(&self) -> ArtifactId {
                self.0.provenance_artifact_id
            }

            /// Exact provenance logical identity bound by this table.
            pub fn provenance_logical_digest(&self) -> ContentDigest {
                self.0.provenance_logical_digest
            }
        }
    };
}

define_typed_table!(
    PatchEmbeddingRow,
    PatchEmbeddingView,
    PatchEmbeddingBlock,
    PatchEmbeddingTable,
    ExpectedPatchSet,
    PatchId,
    patch_id,
    "Owned construction row for a canonical patch-embedding table.",
    "Borrowed status-aware patch-embedding row.",
    "Canonical row-major patch-embedding table."
);
define_typed_table!(
    RegionEmbeddingRow,
    RegionEmbeddingView,
    RegionEmbeddingBlock,
    RegionEmbeddingTable,
    ExpectedRegionSet,
    RegionId,
    region_id,
    "Owned construction row for a canonical region-embedding table.",
    "Borrowed status-aware region-embedding row.",
    "Canonical row-major region-embedding table."
);
define_typed_table!(
    SlideEmbeddingRow,
    SlideEmbeddingView,
    SlideEmbeddingBlock,
    SlideEmbeddingTable,
    ExpectedSlideSet,
    SlideId,
    slide_id,
    "Owned construction row for a canonical slide-embedding table.",
    "Borrowed status-aware slide-embedding row.",
    "Canonical singleton-row slide-embedding table."
);
