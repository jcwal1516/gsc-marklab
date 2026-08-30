use super::*;

#[test]
fn contained_patch_binary_nucleus_area_matches_equal_patch_oracle_and_identities() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let base_links = link_fixture(&embedding);
    let unequal_link = derive_contained_link(
        &base_links,
        &embedding.expected,
        &base_links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [210.0, 10.0])
                .expect("anchor"),
        ],
    );
    assert_eq!(unequal_link.edge_count(), S13_UNEQUAL_EDGE_COUNT);
    let graph = verify_link_graph(&embedding, &base_links, &embedding.expected, &unequal_link);
    let receipt = verify_link_receipt(&embedding, &unequal_link, graph);
    let links = LinkFixture {
        link: unequal_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..base_links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let whole = declared_binary_group_nucleus_area_contrast(
        &scalar.project,
        &input,
        scalar.nucleus_area_mark.clone(),
        3,
    )
    .expect("whole-input contrast");
    let result = run_s13_exact(&scalar, &input, &links);
    assert_eq!(
        result.status(),
        ContainedPatchBinaryNucleusAreaContrastStatus::Available
    );
    assert_eq!(result.assignment_count(), 3);
    assert_eq!(result.edge_count(), 5);
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(result.eligible_marked_incidence_count(), 2);
    assert_eq!(result.eligible_unmarked_incidence_count(), 3);
    assert_eq!(result.eligible_incidence_count(), 5);
    assert_eq!(result.excluded_incidence_count(), 0);
    let whole_input_difference = whole
        .marked_minus_unmarked_mean_nucleus_area_um2()
        .expect("whole-input difference");
    let patch_incidence_weighted_difference = (8.0_f64 * 3.0 + 6.0 * 2.0) / 5.0;
    assert_eq!(whole_input_difference, 8.0);
    assert_eq!(patch_incidence_weighted_difference, 7.2);
    assert_eq!(
        result.equal_patch_mean_marked_minus_unmarked_nucleus_area_um2(),
        Some(7.0)
    );
    assert_ne!(7.0_f64.to_bits(), whole_input_difference.to_bits());
    assert_ne!(
        7.0_f64.to_bits(),
        patch_incidence_weighted_difference.to_bits()
    );
    assert_ne!(
        whole_input_difference.to_bits(),
        patch_incidence_weighted_difference.to_bits()
    );
    assert_eq!(
        result.whole_input_contrast().paired_values_logical_digest(),
        whole.paired_values_logical_digest()
    );
    assert_eq!(
        result.whole_input_contrast().scalar_identity(),
        whole.scalar_identity()
    );
    assert_eq!(
        result.whole_input_contrast().binary_mark(),
        whole.binary_mark()
    );
    assert_eq!(
        result.whole_input_contrast().nucleus_area_mark(),
        whole.nucleus_area_mark()
    );
    assert_eq!(
        result.cell_patch_logical_digest(),
        links.link.logical_digest()
    );
    assert_eq!(
        result.expected_cells_artifact_id(),
        links.link.expected_cells_artifact_id()
    );
    assert_eq!(
        result.expected_patches_artifact_id(),
        links.link.expected_patches_artifact_id()
    );
    assert_eq!(
        result.patch_context_artifact_id(),
        links.link.patch_context_artifact_id()
    );
    assert_eq!(
        result.patch_footprints_artifact_id(),
        links.link.patch_footprints_artifact_id()
    );
    assert_eq!(
        result.cell_patch_producer_artifact_id(),
        links.link.producer_artifact_id()
    );
    assert_eq!(
        result.cell_patch_assignment_artifact_id(),
        links.receipt().assignment_artifact_id()
    );
    assert_eq!(
        result.cell_patch_edge_artifact_id(),
        links.receipt().edge_artifact_id()
    );

    let repeated = run_s13_exact(&scalar, &input, &links);
    assert_eq!(
        repeated
            .equal_patch_mean_marked_minus_unmarked_nucleus_area_um2()
            .expect("contrast")
            .to_bits(),
        7.0_f64.to_bits()
    );
    assert_eq!(
        repeated.whole_input_contrast(),
        result.whole_input_contrast()
    );
}

#[test]
fn contained_patch_binary_nucleus_area_canonicalizes_equal_patch_cancellation_to_positive_zero() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 30.0],
    );
    let input = s13_input(&scalar);
    let result = run_s13_exact(&scalar, &input, &links);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(
        result
            .equal_patch_mean_marked_minus_unmarked_nucleus_area_um2()
            .expect("zero contrast")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn contained_patch_binary_nucleus_area_types_global_and_patch_local_single_group_states() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    for marks in [[0, 0, 0], [1, 1, 1]] {
        let scalar = s13_scalar_fixture(
            &embedding,
            links.context.owning_slide_id().clone(),
            marks,
            [10.0, 20.0, 14.0],
        );
        let input = s13_input(&scalar);
        let result = run_s13_exact(&scalar, &input, &links);
        assert_eq!(
            result.status(),
            ContainedPatchBinaryNucleusAreaContrastStatus::InsufficientEligiblePatches
        );
        assert_eq!(result.eligible_patch_count(), 0);
        assert_eq!(result.eligible_incidence_count(), 0);
        assert_eq!(result.excluded_incidence_count(), 4);
        assert_eq!(
            result.equal_patch_mean_marked_minus_unmarked_nucleus_area_um2(),
            None
        );
    }

    let single_group_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [20.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0])
                .expect("anchor"),
        ],
    );
    let graph = verify_link_graph(&embedding, &links, &embedding.expected, &single_group_link);
    let receipt = verify_link_receipt(&embedding, &single_group_link, graph);
    let single_group_links = LinkFixture {
        link: single_group_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        single_group_links.context.owning_slide_id().clone(),
        [0, 0, 1],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let result = run_s13(
        &scalar,
        &input,
        &single_group_links,
        3,
        3,
        3,
        3 * size_of::<usize>(),
    )
    .expect("patch-local single groups");
    assert_eq!(
        result.whole_input_contrast().status(),
        marklab::DeclaredBinaryGroupNucleusAreaContrastStatus::Available
    );
    assert_eq!(
        result.status(),
        ContainedPatchBinaryNucleusAreaContrastStatus::InsufficientEligiblePatches
    );
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.excluded_incidence_count(), 3);
}

#[test]
fn contained_patch_binary_nucleus_area_preserves_s12_failure_precedence() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [f32::NAN, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &MarklabProject::new(),
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
                    DeclaredScalarInputError::CoordinateRegistryMissing
                )
            )
        )
    ));
    assert!(matches!(
        run_s13(&scalar, &input, &links, 2, 0, 0, 0),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::RowCountBudgetExceeded {
                    required: 3,
                    maximum: 2,
                }
            )
        )
    ));
    assert!(matches!(
        run_s13(&scalar, &input, &links, 3, 0, 0, 0),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::InvalidNucleusAreaValue { row: 0 }
            )
        )
    ));

    let absent_record = publish_s13_scalar_record(
        &embedding,
        b"s13-absent-nucleus-provenance",
        s13_nucleus_area_metadata(),
    );
    let absent_nucleus =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, absent_record.id())
            .expect("absent nucleus declaration");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            absent_nucleus,
            &links.link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
                    DeclaredScalarInputError::ProvenanceRecordMissing { artifact }
                )
            )
        ) if artifact == absent_record.id()
    ));
}

#[test]
fn contained_patch_binary_nucleus_area_rejects_mode_slide_cell_graph_and_receipt_drift() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let cells = embedding.expected.cells();
    let patches = links.expected_patches.ids();
    let interpolation = CellPatchLink::from_declared_weighted_interpolation(
        &links.hierarchy,
        &embedding.expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[0].clone(), [10.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[0].clone(), 1, 1).expect("weight")],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
                vec![
                    CellPatchContributor::new(patches[0].clone(), 1, 2).expect("weight"),
                    CellPatchContributor::new(patches[1].clone(), 1, 2).expect("weight"),
                ],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[1].clone(), 1, 1).expect("weight")],
            ),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("interpolation link");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &interpolation,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::UnsupportedAssignmentMode)
    ));

    let foreign_scalar = s13_scalar_fixture(
        &embedding,
        SlideId::new("s13-foreign-slide").expect("foreign slide"),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let foreign_input = s13_input(&foreign_scalar);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &foreign_scalar.project,
            &foreign_input,
            foreign_scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::OwningSlideBindingMismatch)
    ));

    let different_cells = vec![cell("cell-a"), cell("cell-b"), cell("cell-d")];
    let different_expected =
        ExpectedCellSet::new("all.v1", different_cells.clone()).expect("different cells");
    let different_hierarchy = link_hierarchy(
        &different_cells,
        links.expected_patches.ids(),
        links.context.owning_slide_id(),
    );
    let different_link = CellPatchLink::derive_contained_shared(
        &different_hierarchy,
        &different_expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            CellPatchAnchor::new(different_cells[0].clone(), [10.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("different-cell link");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &different_link,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch { row: 2 })
    ));

    let alternate_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(cells[0].clone(), [20.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
    );
    let alternate_graph =
        verify_link_graph(&embedding, &links, &embedding.expected, &alternate_link);
    let alternate_receipt = verify_link_receipt(&embedding, &alternate_link, alternate_graph);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            alternate_graph,
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchGraphBindingMismatch)
    ));
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            alternate_receipt,
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchReceiptBindingMismatch)
    ));
}

#[test]
fn contained_patch_binary_nucleus_area_enforces_exact_limits() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let base_links = link_fixture(&embedding);
    let unequal_link = derive_contained_link(
        &base_links,
        &embedding.expected,
        &base_links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [210.0, 10.0])
                .expect("anchor"),
        ],
    );
    let graph = verify_link_graph(&embedding, &base_links, &embedding.expected, &unequal_link);
    let receipt = verify_link_receipt(&embedding, &unequal_link, graph);
    let links = LinkFixture {
        link: unequal_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..base_links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    run_s13_exact(&scalar, &input, &links);
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            2,
            S13_UNEQUAL_EDGE_COUNT,
            S13_UNEQUAL_WORKING_BYTES,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::AssignmentCountBudgetExceeded {
                required: 3,
                maximum: 2,
            }
        )
    ));
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            3,
            S13_UNEQUAL_EDGE_COUNT - 1,
            S13_UNEQUAL_WORKING_BYTES,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::EdgeCountBudgetExceeded {
                required: S13_UNEQUAL_EDGE_COUNT,
                maximum,
            }
        ) if maximum == S13_UNEQUAL_EDGE_COUNT - 1
    ));
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            3,
            S13_UNEQUAL_EDGE_COUNT,
            S13_UNEQUAL_WORKING_BYTES - 1,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WorkingByteBudgetExceeded {
                required: S13_UNEQUAL_WORKING_BYTES,
                maximum,
            }
        ) if maximum == S13_UNEQUAL_WORKING_BYTES - 1
    ));
}
