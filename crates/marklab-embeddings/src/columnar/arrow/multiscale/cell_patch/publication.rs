use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_cell_patch_assignment_table_arrow(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, CellPatchAssignmentEntity>(store, &link, budgets)
}

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_cell_patch_edge_table_arrow(
    store: &LocalArtifactStore,
    link: &CellPatchLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, CellPatchEdgeEntity>(store, &link, budgets)
}
