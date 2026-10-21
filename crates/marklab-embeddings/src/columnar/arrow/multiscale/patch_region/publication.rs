use crate::columnar::driver::*;
use crate::*;
use marklab_project::{ArtifactPublication, LocalArtifactStore};

/// Count/hash, replay, and durably publish a fresh canonical arrow artifact.
pub fn publish_patch_region_link_arrow(
    store: &LocalArtifactStore,
    link: &PatchRegionLink,
    budgets: EmbeddingColumnarBudgets,
) -> Result<ArtifactPublication, ColumnarPublicationError> {
    publish::<ArrowIpc, PatchRegionEntity>(store, &link, budgets)
}
