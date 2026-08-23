use super::support::*;

fn valid_anchors() -> Vec<CellPatchAnchor> {
    vec![
        anchor("cell-a", [60.0, 10.0]),
        anchor("cell-b", [-0.0, 0.0]),
        anchor("cell-c", [224.0, 10.0]),
        anchor("cell-d", [900.0, 700.0]),
    ]
}

fn overallocated_anchors() -> Vec<CellPatchAnchor> {
    let mut anchors = Vec::with_capacity(16);
    anchors.extend(valid_anchors());
    anchors
}

#[test]
fn contained_shared_assigns_every_expected_cell_to_all_half_open_windows() {
    let fixture = fixture();
    let link = CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        valid_anchors(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .expect("contained shared link");

    assert_eq!(link.mode(), CellPatchAssignmentMode::ContainedShared);
    assert_eq!(link.assignment_count(), 4);
    assert_eq!(link.edge_count(), 4);
    assert_eq!(
        link.assignments()[0].status(),
        CellPatchAssignmentStatus::Assigned
    );
    assert_eq!(link.assignments()[0].edge_start(), 0);
    assert_eq!(link.assignments()[0].edge_count(), 2);
    assert_eq!(link.assignments()[1].edge_start(), 2);
    assert_eq!(link.assignments()[1].edge_count(), 1);
    assert_eq!(link.assignments()[1].anchor_px()[0].to_bits(), 0);
    assert_eq!(link.assignments()[2].edge_start(), 3);
    assert_eq!(link.assignments()[2].edge_count(), 1);
    assert_eq!(
        link.assignments()[3].status(),
        CellPatchAssignmentStatus::OutsideSampledSupport
    );
    assert_eq!(link.assignments()[3].edge_start(), 4);
    assert_eq!(link.assignments()[3].edge_count(), 0);

    let first = link.edges_for_assignment(0).expect("first edges");
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].patch_id(), &patch("patch-a"));
    assert_eq!(first[1].patch_id(), &patch("patch-b"));
    assert!(first.iter().all(|edge| edge.weight().is_none()));
    assert_eq!(
        link.edges_for_assignment(1).expect("left/top edge")[0].patch_id(),
        &patch("patch-a")
    );
    assert_eq!(
        link.edges_for_assignment(2).expect("right boundary edge")[0].patch_id(),
        &patch("patch-b")
    );
    assert!(link
        .edges_for_assignment(3)
        .expect("outside group")
        .is_empty());
    assert!(link.edges().windows(2).all(|pair| {
        (pair[0].assignment_row(), pair[0].patch_id())
            < (pair[1].assignment_row(), pair[1].patch_id())
    }));
    assert!(!format!("{link:?}").contains("cell-a"));
    assert!(!format!("{:?}", &link.assignments()[0]).contains("60"));
    assert!(!format!("{:?}", &link.edges()[0]).contains("patch-a"));
    assert_eq!(link.logical_digest(), reference_link_digest(&link));
    assert_eq!(
        link.logical_digest().to_string(),
        "ffadec571282725c30a94178358bbe885642c043948a3529d23c00125f3a1b73"
    );
}

#[test]
fn contained_shared_rejects_set_frame_anchor_and_budget_drift() {
    let fixture = fixture();
    let valid = valid_anchors();
    let mut missing = valid.clone();
    missing.pop();
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            missing,
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::CellPatchSetMismatch)
    ));

    let wrong_frame_bindings = CellPatchLinkBindings::new(
        fixture.bindings.expected_cells_artifact_id(),
        fixture.bindings.patch_footprints_artifact_id(),
        fixture.bindings.producer_artifact_id(),
        fixture.bindings.producer_content_digest(),
        CoordinateFrameId::new("other-image-frame").expect("wrong frame"),
    );
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &wrong_frame_bindings,
            valid.clone(),
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::CellPatchFrameMismatch)
    ));
    assert!(CellPatchAnchor::new(cell("cell-a"), [f64::NAN, 0.0]).is_err());

    let duplicate_bindings = CellPatchLinkBindings::new(
        fixture.footprints.expected_patches_artifact_id(),
        fixture.bindings.patch_footprints_artifact_id(),
        fixture.bindings.producer_artifact_id(),
        fixture.bindings.producer_content_digest(),
        fixture.bindings.anchor_frame_id().clone(),
    );
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &duplicate_bindings,
            valid.clone(),
            BUDGET,
            BUDGET,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::DuplicateCellPatchArtifactDependency)
    ));

    let measured = CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        valid.clone(),
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .expect("measured link");
    let input_bytes = valid.capacity() * size_of::<CellPatchAnchor>()
        + valid
            .iter()
            .map(|anchor| anchor.cell_id().as_str().len())
            .sum::<usize>();
    let retained = size_of::<CellPatchLink>()
        + measured.owning_slide_id().as_str().len()
        + measured.assignment_count() * size_of::<CellPatchAssignment>()
        + measured
            .assignments()
            .iter()
            .map(|assignment| assignment.cell_id().as_str().len())
            .sum::<usize>()
        + measured.edge_count() * size_of::<CellPatchEdge>()
        + measured
            .edges()
            .iter()
            .map(|edge| edge.patch_id().as_str().len())
            .sum::<usize>();
    let scratch = fixture.footprints.row_count() * size_of::<([i64; 2], usize)>();
    let working = input_bytes + scratch + retained;
    CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        valid.clone(),
        BUDGET,
        retained,
        working,
    )
    .expect("independently computed exact budgets");
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            valid.clone(),
            BUDGET,
            retained - 1,
            working,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { required, maximum })
            if required == retained && maximum == retained - 1
    ));
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            valid.clone(),
            BUDGET,
            retained,
            working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == working && maximum == working - 1
    ));
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            valid.clone(),
            BUDGET,
            0,
            BUDGET,
        ),
        Err(MultiscaleEmbeddingError::RetainedByteBudgetExceeded { .. })
    ));
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            valid,
            BUDGET,
            BUDGET,
            0,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { .. })
    ));

    let overallocated = overallocated_anchors();
    let overallocated_input = overallocated.capacity() * size_of::<CellPatchAnchor>()
        + overallocated
            .iter()
            .map(|anchor| anchor.cell_id().as_str().len())
            .sum::<usize>();
    let overallocated_working = overallocated_input + scratch + retained;
    CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &fixture.bindings,
        overallocated_anchors(),
        BUDGET,
        retained,
        overallocated_working,
    )
    .expect("caller anchor capacity is charged exactly");
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            overallocated_anchors(),
            BUDGET,
            retained,
            overallocated_working - 1,
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == overallocated_working && maximum == overallocated_working - 1
    ));
}

#[test]
fn contained_index_matches_brute_force_for_fractional_and_negative_anchors() {
    let fixture = fixture();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..64 {
        let anchors = fixture
            .expected_cells
            .cells()
            .iter()
            .map(|cell_id| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let x = ((state >> 16) % 1_401) as f64 - 200.25;
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let y = ((state >> 16) % 1_001) as f64 - 100.75;
                CellPatchAnchor::new(cell_id.clone(), [x, y]).expect("finite generated anchor")
            })
            .collect::<Vec<_>>();
        let link = CellPatchLink::derive_contained_shared(
            &fixture.hierarchy,
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.bindings,
            anchors.clone(),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .expect("indexed generated link");
        for (row, anchor) in anchors.iter().enumerate() {
            let point = anchor.anchor_px().map(f64::floor);
            let oracle = fixture
                .footprints
                .footprints()
                .iter()
                .filter(|footprint| {
                    let origin = footprint.origin_px();
                    (0..2).all(|axis| {
                        origin[axis] as f64 <= point[axis]
                            && point[axis] < (origin[axis] + 224) as f64
                    })
                })
                .map(|footprint| footprint.patch_id().as_str())
                .collect::<Vec<_>>();
            let indexed = link
                .edges_for_assignment(row)
                .expect("generated edge range")
                .iter()
                .map(|edge| edge.patch_id().as_str())
                .collect::<Vec<_>>();
            assert_eq!(indexed, oracle);
        }
    }

    let reflected =
        fixture_with_footprints(PatchBoundaryPolicy::Reflect, [[-223, 0], [0, 0], [300, 0]]);
    let link = CellPatchLink::derive_contained_shared(
        &reflected.hierarchy,
        &reflected.expected_cells,
        &reflected.expected_patches,
        &reflected.context,
        &reflected.footprints,
        &reflected.bindings,
        vec![
            anchor("cell-a", [-0.5, 0.0]),
            anchor("cell-b", [0.0, 0.0]),
            anchor("cell-c", [223.999, 223.999]),
            anchor("cell-d", [224.0, 0.0]),
        ],
        BUDGET,
        BUDGET,
        BUDGET,
    )
    .expect("negative-origin contained link");
    assert_eq!(link.edges_for_assignment(0).expect("row 0").len(), 1);
    assert_eq!(link.edges_for_assignment(1).expect("row 1").len(), 2);
    assert_eq!(link.edges_for_assignment(2).expect("row 2").len(), 1);
    assert!(link.edges_for_assignment(3).expect("row 3").is_empty());

    let degenerate = fixture_with_footprints(
        PatchBoundaryPolicy::FullyContainedOnly,
        [[0, 0], [0, 0], [0, 0]],
    );
    let link = CellPatchLink::derive_contained_shared(
        &degenerate.hierarchy,
        &degenerate.expected_cells,
        &degenerate.expected_patches,
        &degenerate.context,
        &degenerate.footprints,
        &degenerate.bindings,
        vec![
            anchor("cell-a", [447.0, 0.0]),
            anchor("cell-b", [447.0, 10.0]),
            anchor("cell-c", [447.0, 20.0]),
            anchor("cell-d", [447.0, 30.0]),
        ],
        12,
        BUDGET,
        BUDGET,
    )
    .expect("degenerate candidate-heavy zero-output link");
    assert_eq!(link.edge_count(), 0);
    assert!(link
        .assignments()
        .iter()
        .all(|assignment| assignment.status() == CellPatchAssignmentStatus::OutsideSampledSupport));
    assert!(matches!(
        CellPatchLink::derive_contained_shared(
            &degenerate.hierarchy,
            &degenerate.expected_cells,
            &degenerate.expected_patches,
            &degenerate.context,
            &degenerate.footprints,
            &degenerate.bindings,
            vec![
                anchor("cell-a", [447.0, 0.0]),
                anchor("cell-b", [447.0, 10.0]),
                anchor("cell-c", [447.0, 20.0]),
                anchor("cell-d", [447.0, 30.0]),
            ],
            11,
            BUDGET,
            BUDGET,
        ),
        Err(
            MultiscaleEmbeddingError::CellPatchCandidateCheckBudgetExceeded {
                required: 12,
                maximum: 11,
            }
        )
    ));
}
