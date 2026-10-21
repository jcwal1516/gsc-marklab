use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_patch_footprint_set_parquet(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, PatchFootprintEntity>(
        store,
        &PatchFootprintWriteContext {
            expected,
            context,
            footprints,
        },
        budgets,
    )
}

/// Count/hash, replay, and durably publish a fresh canonical parquet artifact.
pub fn publish_patch_overlap_graph_parquet(
    store: &LocalArtifactStore,
    expected: &ExpectedPatchSet,
    context: &PatchEmbeddingContext,
    footprints: &PatchFootprintSet,
    overlap: &PatchOverlapGraph,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<Parquet, PatchOverlapEntity>(
        store,
        &PatchOverlapWriteContext {
            expected,
            context,
            footprints,
            overlap,
        },
        budgets,
    )
}
