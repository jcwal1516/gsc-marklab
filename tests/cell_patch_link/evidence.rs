use super::support::*;

fn valid_anchors() -> Vec<CellPatchAnchor> {
    vec![
        anchor("cell-a", [60.0, 10.0]),
        anchor("cell-b", [0.0, 0.0]),
        anchor("cell-c", [224.0, 10.0]),
        anchor("cell-d", [900.0, 700.0]),
    ]
}

#[test]
fn contained_shared_covers_empty_dense_hierarchy_and_support_drift() {
    let fixture = fixture();
    let empty_cells = ExpectedCellSet::new("empty_cells.v1", Vec::new()).expect("empty cells");
    let empty = CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &empty_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        Vec::new(),
        0,
        BUDGET,
        BUDGET,
    )
    .expect("empty cell link");
    assert_eq!(empty.assignment_count(), 0);
    assert_eq!(empty.edge_count(), 0);

    let dense = fixture_with_footprints(
        PatchBoundaryPolicy::FullyContainedOnly,
        [[0, 0], [0, 0], [0, 0]],
    );
    let dense_link = CellPatchLink::derive_contained_shared(
        &dense.hierarchy,
        &dense.expected_cells,
        &dense.expected_patches,
        &dense.context,
        &dense.footprints,
        &dense.bindings,
        vec![
            anchor("cell-a", [0.0, 0.0]),
            anchor("cell-b", [10.0, 10.0]),
            anchor("cell-c", [20.0, 20.0]),
            anchor("cell-d", [30.0, 30.0]),
        ],
        12,
        BUDGET,
        BUDGET,
    )
    .expect("dense shared link");
    assert_eq!(dense_link.edge_count(), 12);
    assert!(dense_link
        .assignments()
        .iter()
        .all(|assignment| assignment.edge_count() == 3));

    let missing_cell = ExpectedCellSet::new("missing_cell.v1", vec![cell("cell-z")])
        .expect("missing hierarchy cell set");
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &missing_cell,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            vec![anchor("cell-z", [0.0, 0.0])],
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::CellPatchHierarchyMismatch { row: 0 })
    ));

    let drifted_expected = ExpectedPatchSet::new(
        &fixture.hierarchy,
        fixture.context.owning_slide_id().clone(),
        "different_patch_selection.v1",
        fixture.expected_patches.ids().to_vec(),
        BUDGET,
    )
    .expect("drifted expected patches");
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &drifted_expected,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            valid_anchors(),
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::CellPatchInputMismatch)
    ));

    let reflected =
        fixture_with_footprints(PatchBoundaryPolicy::Reflect, [[0, 0], [50, 0], [300, 0]]);
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &reflected.context,
            &fixture.footprints,
            &fixture.bindings,
            valid_anchors(),
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::CellPatchInputMismatch)
    ));
}
