use super::*;

#[test]
fn nucleus_area_cross_covariance_matches_both_oracles_and_ignores_binary_rows() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());
    let mark_table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::probability(
                probability.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark_prob.clone().expect("probabilities"),
            )
            .expect("probability column"),
            ScalarMarkColumn::continuous(
                nucleus_area.clone(),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::SquareMicrometer,
                MissingnessPolicy::NotPermitted,
                fixture
                    .pattern
                    .nucleus_area_um2
                    .clone()
                    .expect("nucleus areas"),
            )
            .expect("nucleus-area column"),
            ScalarMarkColumn::vector_artifact_ref(
                VectorArtifactRefMarkDeclaration::new(
                    ScalarMarkId::new("cellvit_embedding").expect("vector mark ID"),
                    "CellViT embedding",
                    MeasurementStatus::MorphologyPrediction,
                )
                .expect("vector declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::EmbeddingVector,
                MissingnessPolicy::NotPermitted,
                &table,
                artifact,
            )
            .expect("vector reference"),
        ],
        4,
        declared_scalar_support::cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("typed vector MarkTable");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &mark_table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("typed vector input");

    let result = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &fixture.project,
        &input,
        nucleus_area.clone(),
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("nucleus-area cross-covariance energy");
    assert_eq!(
        result.status(),
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::Available
    );
    assert_eq!(result.present_row_count(), 4);
    assert_eq!(result.present_nucleus_area_mean_um2(), Some(15.0));
    assert_eq!(result.dimension(), DIMENSION);
    assert_eq!(result.cross_covariance_energy(), Some(62.5));
    assert_eq!(result.scalar_identity().row_count(), 4);
    assert_eq!(result.binary_mark(), &binary);
    assert_eq!(result.probability_mark(), Some(&probability));
    assert_eq!(result.nucleus_area_mark(), &nucleus_area);
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
    assert_eq!(result.embedding_qc_summary(), artifact.qc_summary());
    assert_eq!(result.table_logical_digest(), artifact.logical_digest());

    let mut changed_rows = oracle_rows();
    changed_rows[0] = (
        EmbeddingStatus::Present,
        Some(vec![1.0; DIMENSION as usize]),
    );
    let (_changed_fixture, changed_table, changed_artifact) =
        verified_embedding(&fixture.cell_ids, changed_rows);
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &changed_table,
            changed_artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::VectorArtifactReferenceMismatch)
    ));
    assert_eq!(nucleus_area.mark_id().as_str(), "nucleus_area_um2");
    assert_eq!(nucleus_area.label(), "Nucleus area");
    assert_eq!(
        nucleus_area.measurement_status(),
        MeasurementStatus::Measured
    );

    let repeated = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &fixture.project,
        &input,
        nucleus_area,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("repeat energy");
    assert_eq!(
        repeated
            .cross_covariance_energy()
            .expect("repeat value")
            .to_bits(),
        result.cross_covariance_energy().expect("value").to_bits()
    );
    assert_eq!(
        repeated
            .present_nucleus_area_mean_um2()
            .expect("repeat mean")
            .to_bits(),
        result
            .present_nucleus_area_mean_um2()
            .expect("mean")
            .to_bits()
    );
    assert_eq!(
        repeated.nucleus_area_values_logical_digest(),
        result.nucleus_area_values_logical_digest()
    );
    assert_eq!(repeated, result);

    let mut changed_fixture = declared_scalar_support::fixture();
    changed_fixture.pattern.nucleus_area_um2 =
        Some(vec![10.0, 10.0, 15.0, 20.0].into_boxed_slice());
    changed_fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let changed_binary = binary_declaration(&mut changed_fixture);
    let changed_probability = probability_declaration(&mut changed_fixture);
    let changed_nucleus_area = nucleus_area_declaration(&mut changed_fixture);
    let changed_input = declared_input(
        &changed_fixture,
        changed_binary,
        Some(changed_probability),
        &changed_fixture.cell_ids,
    );
    let changed = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &changed_fixture.project,
        &changed_input,
        changed_nucleus_area,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("changed-area energy");
    assert_ne!(
        changed.nucleus_area_values_logical_digest(),
        result.nucleus_area_values_logical_digest()
    );
    assert_eq!(changed.present_nucleus_area_mean_um2(), Some(13.75));
    assert_eq!(changed.cross_covariance_energy(), Some(45.3125));

    let mut swapped_fixture = declared_scalar_support::fixture();
    swapped_fixture.pattern.nucleus_area_um2 =
        Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    swapped_fixture.pattern.mark = vec![1, 0, 0, 1].into_boxed_slice();
    swapped_fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let swapped_binary = binary_declaration(&mut swapped_fixture);
    let swapped_probability = probability_declaration(&mut swapped_fixture);
    let swapped_nucleus_area = nucleus_area_declaration(&mut swapped_fixture);
    let swapped_input = declared_input(
        &swapped_fixture,
        swapped_binary,
        Some(swapped_probability),
        &swapped_fixture.cell_ids,
    );
    let swapped = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &swapped_fixture.project,
        &swapped_input,
        swapped_nucleus_area,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("binary-independent area energy");
    assert_eq!(swapped.scalar_identity(), result.scalar_identity());
    assert_eq!(
        swapped.nucleus_area_values_logical_digest(),
        result.nucleus_area_values_logical_digest()
    );
    assert_eq!(
        swapped.present_nucleus_area_mean_um2(),
        result.present_nucleus_area_mean_um2()
    );
    assert_eq!(
        swapped.cross_covariance_energy(),
        result.cross_covariance_energy()
    );
    assert_eq!(swapped, result);
}

#[test]
fn nucleus_area_cross_covariance_canonicalizes_equal_vectors_to_positive_zero() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 12.0, 18.0, 20.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None, &fixture.cell_ids);
    let equal = (EmbeddingStatus::Present, Some(alternating(1.0, -2.0)));
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![equal.clone(), equal.clone(), equal.clone(), equal],
    );

    let result = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &fixture.project,
        &input,
        nucleus_area,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("zero area covariance energy");
    assert_eq!(
        result
            .cross_covariance_energy()
            .expect("zero energy")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn nucleus_area_cross_covariance_excludes_every_non_present_status() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 20.0, 30.0, 40.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None, &fixture.cell_ids);
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![
            (EmbeddingStatus::Present, Some(alternating(2.0, 4.0))),
            (EmbeddingStatus::MissingVector, None),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
        ],
    );

    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &table,
            artifact,
            4,
            ONE_PRESENT_COMPONENT_OPERATIONS - 1,
            WORKING_BYTES,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: ONE_PRESENT_COMPONENT_OPERATIONS,
                maximum
            }
        ) if maximum == ONE_PRESENT_COMPONENT_OPERATIONS - 1
    ));
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &table,
            artifact,
            4,
            ONE_PRESENT_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum
            }
        ) if maximum == WORKING_BYTES - 1
    ));

    let result = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &fixture.project,
        &input,
        nucleus_area,
        &table,
        artifact,
        4,
        ONE_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("one present row");
    assert_eq!(
        result.status(),
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    );
    assert_eq!(result.present_row_count(), 1);
    assert_eq!(result.present_nucleus_area_mean_um2(), Some(10.0));
    assert_eq!(result.cross_covariance_energy(), None);
    assert_eq!(result.embedding_qc_summary().present_count(), 1);
    assert_eq!(result.embedding_qc_summary().missing_vector_count(), 1);
    assert_eq!(result.embedding_qc_summary().extraction_failed_count(), 1);
    assert_eq!(result.embedding_qc_summary().qc_rejected_count(), 1);
}
