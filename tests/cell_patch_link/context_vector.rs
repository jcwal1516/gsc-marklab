use super::support::*;
use marklab_embeddings::{
    cell_patch_context, patch_dependency_weighting, PatchDependencyWeight, PatchEmbeddingRow,
    PatchEmbeddingTable, PatchOverlapGraph,
};

fn contributor(patch_id: &str, numerator: u64, denominator: u64) -> CellPatchContributor {
    CellPatchContributor::new(patch(patch_id), numerator, denominator).unwrap()
}

#[test]
fn weighted_context_uses_overlap_groups_for_effective_patch_count() {
    let fixture = fixture();
    let link = CellPatchLink::from_declared_weighted_interpolation(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        vec![
            DeclaredCellPatchAssignment::new(
                anchor("cell-a", [60.0, 10.0]),
                vec![
                    contributor("patch-a", 2, 4),
                    contributor("patch-b", 1, 4),
                    contributor("patch-c", 1, 4),
                ],
            ),
            DeclaredCellPatchAssignment::new(anchor("cell-b", [0.0, 0.0]), Vec::new()),
            DeclaredCellPatchAssignment::new(anchor("cell-c", [224.0, 10.0]), Vec::new()),
            DeclaredCellPatchAssignment::new(anchor("cell-d", [900.0, 700.0]), Vec::new()),
        ],
        BUDGET,
        BUDGET,
    )
    .unwrap();
    let overlap = PatchOverlapGraph::derive(
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        fixture.bindings.patch_footprints_artifact_id(),
        BUDGET,
        BUDGET,
    )
    .unwrap();
    let support_id = artifact(b"context-support");
    let support_digest = ContentDigest::from_bytes(b"context-support-digest");
    let provenance_id = artifact(b"context-provenance");
    let provenance_digest = ContentDigest::from_bytes(b"context-provenance-digest");
    let table = PatchEmbeddingTable::from_rows(
        2,
        &fixture.expected_patches,
        link.expected_patches_artifact_id(),
        support_id,
        support_digest,
        provenance_id,
        provenance_digest,
        vec![
            PatchEmbeddingRow::present(patch("patch-a"), vec![0.0, 0.0]),
            PatchEmbeddingRow::present(patch("patch-b"), vec![4.0, 0.0]),
            PatchEmbeddingRow::present(patch("patch-c"), vec![0.0, 8.0]),
        ],
        BUDGET,
    )
    .unwrap();

    let result = cell_patch_context(&cell("cell-a"), &table, &link, &overlap, 6).unwrap();
    assert_eq!(result.context_vector(), &[1.0, 2.0]);
    assert_eq!(result.linked_patch_count(), 3);
    assert_eq!(result.dependency_group_count(), 2);
    assert!((result.effective_independent_patch_count() - 1.6).abs() < 1e-12);
    assert_eq!(result.component_operations(), 6);
    assert_eq!(result.link_logical_digest(), link.logical_digest());
    assert_eq!(result.overlap_logical_digest(), overlap.logical_digest());

    let dependency = patch_dependency_weighting(
        &overlap,
        vec![
            PatchDependencyWeight::new(patch("patch-a"), 0.5).unwrap(),
            PatchDependencyWeight::new(patch("patch-b"), 0.25).unwrap(),
            PatchDependencyWeight::new(patch("patch-c"), 0.25).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(dependency.patch_count(), 3);
    assert_eq!(dependency.dependency_group_count(), 2);
    assert!((dependency.effective_independent_patch_count() - 1.6).abs() < 1e-12);
}
