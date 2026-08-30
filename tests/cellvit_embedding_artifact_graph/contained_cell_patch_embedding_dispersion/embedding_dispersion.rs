use super::*;

fn run(
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    links: &LinkFixture,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    marklab::ContainedCellPatchEmbeddingDispersion,
    ContainedCellPatchEmbeddingDispersionError,
> {
    contained_cell_patch_embedding_dispersion(
        table,
        artifact,
        &links.link,
        links.graph(),
        links.receipt(),
        maximum_assignments,
        maximum_edges,
        maximum_component_operations,
        maximum_working_bytes,
    )
}

fn run_exact(
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    links: &LinkFixture,
) -> marklab::ContainedCellPatchEmbeddingDispersion {
    run(
        table,
        artifact,
        links,
        ASSIGNMENT_COUNT,
        EDGE_COUNT,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("contained cell-patch dispersion")
}

#[test]
fn contained_cell_patch_embedding_dispersion_matches_overlapping_oracle_and_identities() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let result = run_exact(&table, artifact, &links);
    assert_eq!(
        result.status(),
        ContainedCellPatchEmbeddingDispersionStatus::Available
    );
    assert_eq!(result.assignment_count(), 3);
    assert_eq!(result.edge_count(), 4);
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(result.eligible_incidence_count(), 4);
    assert_eq!(result.excluded_incidence_count(), 0);
    assert_eq!(result.dimension(), DIMENSION);
    assert_eq!(result.mean_squared_component_dispersion(), Some(0.5));
    assert_eq!(result.embedding_qc_summary(), artifact.qc_summary());
    assert_eq!(
        result.embedding_table_logical_digest(),
        artifact.logical_digest()
    );
    assert_eq!(
        result.expected_cells_artifact_id(),
        artifact.expected_cells_artifact_id()
    );
    assert_eq!(
        result.expected_cells_logical_digest(),
        artifact.expected_cells_logical_digest()
    );
    assert_eq!(
        result.embedding_artifact_id(),
        artifact.embedding_artifact_id()
    );
    assert_eq!(
        result.row_link_artifact_id(),
        artifact.row_link_artifact_id()
    );
    assert_eq!(
        result.embedding_provenance_artifact_id(),
        artifact.provenance_artifact_id()
    );
    assert_eq!(
        result.cell_patch_logical_digest(),
        links.link.logical_digest()
    );
    assert_eq!(
        result.cell_patch_assignment_artifact_id(),
        links.receipt().assignment_artifact_id()
    );
    assert_eq!(
        result.cell_patch_edge_artifact_id(),
        links.receipt().edge_artifact_id()
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
    let repeated = run_exact(&table, artifact, &links);
    assert_eq!(repeated, result);
    assert_eq!(
        repeated
            .mean_squared_component_dispersion()
            .expect("dispersion")
            .to_bits(),
        0.5_f64.to_bits()
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_changes_with_vectors_and_canonicalizes_zero() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let baseline = run_exact(&table, artifact, &links);

    let mut changed_rows = available_rows();
    changed_rows[2].2 = Some(level_vector(6.0));
    let (changed_embedding, changed_table, changed_artifact) = verified_embedding(changed_rows);
    let changed_links = link_fixture(&changed_embedding);
    let changed = run_exact(&changed_table, changed_artifact, &changed_links);
    assert_ne!(
        changed.mean_squared_component_dispersion(),
        baseline.mean_squared_component_dispersion()
    );
    assert_eq!(
        changed.cell_patch_logical_digest(),
        changed_links.link.logical_digest()
    );
    assert_ne!(
        changed.embedding_table_logical_digest(),
        baseline.embedding_table_logical_digest()
    );

    let zero_rows = canonical_cells()
        .into_iter()
        .map(|cell_id| {
            let values = (0..DIMENSION)
                .map(|component| {
                    if component.is_multiple_of(2) {
                        0.0
                    } else {
                        -0.0
                    }
                })
                .collect();
            (cell_id, EmbeddingStatus::Present, Some(values))
        })
        .collect();
    let (zero_embedding, zero_table, zero_artifact) = verified_embedding(zero_rows);
    let zero_links = link_fixture(&zero_embedding);
    let zero = run_exact(&zero_table, zero_artifact, &zero_links);
    let value = zero
        .mean_squared_component_dispersion()
        .expect("zero dispersion");
    assert_eq!(value.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn contained_cell_patch_embedding_dispersion_excludes_every_non_present_status() {
    for status in [
        EmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected,
    ] {
        let mut rows = available_rows();
        rows[0] = (rows[0].0.clone(), status, None);
        let (embedding, table, artifact) = verified_embedding(rows);
        let links = link_fixture(&embedding);
        let result = run_exact(&table, artifact, &links);
        assert_eq!(
            result.status(),
            ContainedCellPatchEmbeddingDispersionStatus::Available
        );
        assert_eq!(result.eligible_patch_count(), 1);
        assert_eq!(result.eligible_incidence_count(), 2);
        assert_eq!(result.excluded_incidence_count(), 2);
        assert_eq!(result.mean_squared_component_dispersion(), Some(0.5));
        let qc = result.embedding_qc_summary();
        assert_eq!(qc.present_count(), 2);
        match status {
            EmbeddingStatus::MissingVector => assert_eq!(qc.missing_vector_count(), 1),
            EmbeddingStatus::ExtractionFailed => assert_eq!(qc.extraction_failed_count(), 1),
            EmbeddingStatus::QcRejected => assert_eq!(qc.qc_rejected_count(), 1),
            EmbeddingStatus::Present => unreachable!(),
        }
    }

    let mut singleton_rows = available_rows();
    singleton_rows[1] = (
        singleton_rows[1].0.clone(),
        EmbeddingStatus::MissingVector,
        None,
    );
    let (embedding, table, artifact) = verified_embedding(singleton_rows);
    let links = link_fixture(&embedding);
    let result = run_exact(&table, artifact, &links);
    assert_eq!(
        result.status(),
        ContainedCellPatchEmbeddingDispersionStatus::InsufficientEligiblePatches
    );
    assert_eq!(result.eligible_patch_count(), 0);
    assert_eq!(result.eligible_incidence_count(), 0);
    assert_eq!(result.excluded_incidence_count(), 4);
    assert_eq!(result.mean_squared_component_dispersion(), None);
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_interpolation_before_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
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
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &interpolation,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::UnsupportedAssignmentMode)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_artifact_and_expected_cell_drift() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let mut changed_rows = available_rows();
    changed_rows[0].2 = Some(level_vector(8.0));
    let (_other_embedding, _other_table, other_artifact) = verified_embedding(changed_rows);
    assert_eq!(
        run(&table, other_artifact, &links, 0, 0, 0, 0),
        Err(ContainedCellPatchEmbeddingDispersionError::EmbeddingArtifactBindingMismatch)
    );

    let drift_record = publish_domain_record(
        &embedding.store,
        "marklab.s11_expected_cell_binding_drift",
        "application/octet-stream",
        b"expected-cell-binding-drift",
        Vec::new(),
    );
    let id_drift_bindings = CellPatchLinkBindings::new(
        drift_record.id(),
        links.link.patch_footprints_artifact_id(),
        links.link.producer_artifact_id(),
        links.link.producer_content_digest(),
        links.context.image_frame_id().clone(),
    );
    let anchors = vec![
        CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0]).expect("anchor"),
        CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0]).expect("anchor"),
        CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0]).expect("anchor"),
    ];
    let id_drift = derive_contained_link(
        &links,
        &embedding.expected,
        &id_drift_bindings,
        anchors.clone(),
    );
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &id_drift,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );

    let logical_drift_expected =
        ExpectedCellSet::new("logical_drift.v1", embedding.expected.cells().to_vec())
            .expect("drifted expected cells");
    let logical_drift =
        derive_contained_link(&links, &logical_drift_expected, &links.bindings, anchors);
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &logical_drift,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_cell_graph_and_receipt_drift_before_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let alternate_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [20.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0])
                .expect("anchor"),
        ],
    );
    let alternate_graph =
        verify_link_graph(&embedding, &links, &embedding.expected, &alternate_link);
    let alternate_receipt = verify_link_receipt(&embedding, &alternate_link, alternate_graph);
    assert_ne!(alternate_link.logical_digest(), links.link.logical_digest());

    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &links.link,
            alternate_graph,
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::CellPatchGraphBindingMismatch)
    );
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &links.link,
            links.graph(),
            alternate_receipt,
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::CellPatchReceiptBindingMismatch)
    );

    let different_cells = vec![cell("cell-a"), cell("cell-b"), cell("cell-d")];
    let different_expected =
        ExpectedCellSet::new("all.v1", different_cells.clone()).expect("different expected cells");
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
    .expect("ordered CellId drift link");
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &different_link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_enforces_exact_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    run_exact(&table, artifact, &links);

    for (assignments, edges, operations, bytes, expected) in [
        (
            ASSIGNMENT_COUNT - 1,
            EDGE_COUNT,
            COMPONENT_OPERATIONS,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::AssignmentCountBudgetExceeded {
                required: ASSIGNMENT_COUNT,
                maximum: ASSIGNMENT_COUNT - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT - 1,
            COMPONENT_OPERATIONS,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::EdgeCountBudgetExceeded {
                required: EDGE_COUNT,
                maximum: EDGE_COUNT - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT,
            COMPONENT_OPERATIONS - 1,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: COMPONENT_OPERATIONS,
                maximum: COMPONENT_OPERATIONS - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT,
            COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
            ContainedCellPatchEmbeddingDispersionError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            },
        ),
    ] {
        assert_eq!(
            run(
                &table,
                artifact,
                &links,
                assignments,
                edges,
                operations,
                bytes
            ),
            Err(expected)
        );
    }
}
