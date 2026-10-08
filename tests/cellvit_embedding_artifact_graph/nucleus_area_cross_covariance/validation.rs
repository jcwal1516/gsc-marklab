use super::*;

#[test]
fn nucleus_area_cross_covariance_types_zero_one_and_constant_present_states() {
    let mut zero_fixture = declared_scalar_support::fixture();
    zero_fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 20.0, 30.0, 40.0].into_boxed_slice());
    let zero_binary = binary_declaration(&mut zero_fixture);
    let zero_nucleus_area = nucleus_area_declaration(&mut zero_fixture);
    let zero_input = declared_input(&zero_fixture, zero_binary, None, &zero_fixture.cell_ids);
    let (_zero_embedding_fixture, zero_table, zero_artifact) = verified_embedding(
        &zero_fixture.cell_ids,
        vec![
            (EmbeddingStatus::MissingVector, None),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
            (EmbeddingStatus::MissingVector, None),
        ],
    );
    let zero = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &zero_fixture.project,
        &zero_input,
        zero_nucleus_area,
        &zero_table,
        zero_artifact,
        4,
        ZERO_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("zero present rows");
    assert_eq!(
        zero.status(),
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    );
    assert_eq!(zero.present_row_count(), 0);
    assert_eq!(zero.present_nucleus_area_mean_um2(), None);
    assert_eq!(zero.cross_covariance_energy(), None);

    let mut one_fixture = declared_scalar_support::fixture();
    one_fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 20.0, 30.0, 40.0].into_boxed_slice());
    let one_binary = binary_declaration(&mut one_fixture);
    let one_nucleus_area = nucleus_area_declaration(&mut one_fixture);
    let one_input = declared_input(&one_fixture, one_binary, None, &one_fixture.cell_ids);
    let (_one_embedding_fixture, one_table, one_artifact) = verified_embedding(
        &one_fixture.cell_ids,
        vec![
            (EmbeddingStatus::MissingVector, None),
            (EmbeddingStatus::Present, Some(alternating(1.0, 2.0))),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
        ],
    );
    let one = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &one_fixture.project,
        &one_input,
        one_nucleus_area,
        &one_table,
        one_artifact,
        4,
        ONE_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("one present row");
    assert_eq!(
        one.status(),
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    );
    assert_eq!(one.present_row_count(), 1);
    assert_eq!(one.present_nucleus_area_mean_um2(), Some(20.0));
    assert_eq!(one.cross_covariance_energy(), None);

    let mut constant_fixture = declared_scalar_support::fixture();
    constant_fixture.pattern.nucleus_area_um2 =
        Some(vec![10.0, 10.0, 30.0, 40.0].into_boxed_slice());
    let constant_binary = binary_declaration(&mut constant_fixture);
    let constant_nucleus_area = nucleus_area_declaration(&mut constant_fixture);
    let constant_input = declared_input(
        &constant_fixture,
        constant_binary,
        None,
        &constant_fixture.cell_ids,
    );
    let (_constant_embedding_fixture, constant_table, constant_artifact) = verified_embedding(
        &constant_fixture.cell_ids,
        vec![
            (EmbeddingStatus::Present, Some(alternating(1.0, 2.0))),
            (EmbeddingStatus::Present, Some(alternating(3.0, 4.0))),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
        ],
    );
    let constant = declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &constant_fixture.project,
        &constant_input,
        constant_nucleus_area,
        &constant_table,
        constant_artifact,
        4,
        TWO_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("constant present nucleus areas");
    assert_eq!(
        constant.status(),
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus::NoNucleusAreaVariation
    );
    assert_eq!(constant.present_row_count(), 2);
    assert_eq!(constant.present_nucleus_area_mean_um2(), Some(10.0));
    assert_eq!(constant.cross_covariance_energy(), None);
}

#[test]
fn nucleus_area_cross_covariance_rejects_missing_length_and_invalid_values() {
    let mut missing_fixture = declared_scalar_support::fixture();
    let missing_binary = binary_declaration(&mut missing_fixture);
    let missing_nucleus_area = nucleus_area_declaration(&mut missing_fixture);
    let missing_input = declared_input(
        &missing_fixture,
        missing_binary,
        None,
        &missing_fixture.cell_ids,
    );
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&missing_fixture.cell_ids, oracle_rows());
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &missing_fixture.project,
            &missing_input,
            missing_nucleus_area,
            &table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::NucleusAreaColumnRequired)
    ));

    for (areas, observed) in [(vec![10.0, 20.0, 30.0], 3), (vec![10.0; 5], 5)] {
        let mut fixture = declared_scalar_support::fixture();
        fixture.pattern.nucleus_area_um2 = Some(areas.into_boxed_slice());
        let binary = binary_declaration(&mut fixture);
        let error = match DeclaredScalarPatternInput::new(
            &fixture.project,
            &fixture.pattern,
            &fixture.cell_ids,
            fixture.slide_id.clone(),
            fixture.frame_id.clone(),
            binary,
            None,
            16 * 1024,
            declared_scalar_support::cell_id_text_bytes(&fixture.cell_ids),
        ) {
            Err(error) => error,
            Ok(_) => panic!("declared input accepted an invalid compatibility-column length"),
        };
        assert!(matches!(
            error,
            DeclaredScalarInputError::PatternColumnLengthMismatch {
                column: "nucleus_area_um2",
                expected: 4,
                observed: actual,
            } if actual == observed
        ));
    }

    for invalid in [f32::NAN, f32::INFINITY, 0.0, -1.0, -0.0] {
        let mut fixture = declared_scalar_support::fixture();
        fixture.pattern.nucleus_area_um2 = Some(vec![10.0, invalid, 20.0, 30.0].into_boxed_slice());
        let binary = binary_declaration(&mut fixture);
        let nucleus_area = nucleus_area_declaration(&mut fixture);
        let input = declared_input(&fixture, binary, None, &fixture.cell_ids);
        assert!(matches!(
            declared_nucleus_area_cell_embedding_cross_covariance_energy(
                &fixture.project,
                &input,
                nucleus_area,
                &table,
                artifact,
                4,
                AVAILABLE_COMPONENT_OPERATIONS,
                WORKING_BYTES,
            ),
            Err(
                DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::InvalidNucleusAreaValue {
                    row: 1
                }
            )
        ));
    }
}

#[test]
fn nucleus_area_cross_covariance_revalidates_target_project_and_exact_provenance() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None, &fixture.cell_ids);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());

    let empty_project = MarklabProject::new();
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &empty_project,
            &input,
            nucleus_area,
            &table,
            artifact,
            0,
            0,
            0,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ScalarInput(
                DeclaredScalarInputError::CoordinateRegistryMissing
            )
        )
    ));

    let mut missing_fixture = declared_scalar_support::fixture();
    missing_fixture.pattern.nucleus_area_um2 =
        Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    let missing_binary = binary_declaration(&mut missing_fixture);
    let mut foreign_fixture = declared_scalar_support::fixture();
    let absent_id = declared_scalar_support::publish_record(
        &mut foreign_fixture,
        b"absent-nucleus-area-provenance",
        declared_scalar_support::MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        declared_scalar_support::nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let missing_nucleus_area =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, absent_id)
            .expect("missing-record declaration");
    let missing_input = declared_input(
        &missing_fixture,
        missing_binary,
        None,
        &missing_fixture.cell_ids,
    );
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &missing_fixture.project,
            &missing_input,
            missing_nucleus_area,
            &table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ScalarInput(
            DeclaredScalarInputError::ProvenanceRecordMissing { artifact: missing }
        )) if missing == absent_id
    ));

    let mut drift_fixture = declared_scalar_support::fixture();
    drift_fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    let drift_binary = binary_declaration(&mut drift_fixture);
    let mut metadata =
        declared_scalar_support::nucleus_area_um2_metadata(MeasurementStatus::Measured);
    metadata.insert("unit".into(), "square_millimeter".into());
    let drift_id = declared_scalar_support::publish_record(
        &mut drift_fixture,
        b"drifted-nucleus-area-provenance",
        declared_scalar_support::MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        metadata,
    );
    let drift_nucleus_area =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, drift_id)
            .expect("drifted declaration");
    let drift_input = declared_input(&drift_fixture, drift_binary, None, &drift_fixture.cell_ids);
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &drift_fixture.project,
            &drift_input,
            drift_nucleus_area,
            &table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ScalarInput(
            DeclaredScalarInputError::ProvenanceProfileMismatch { artifact: drifted }
        )) if drifted == drift_id
    ));

    assert!(matches!(
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::DerivedSummary, drift_id),
        Err(DeclaredScalarInputError::UnsupportedPerCellMeasurementStatus)
    ));
}

#[test]
fn nucleus_area_cross_covariance_enforces_binding_precedence_and_exact_limits() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(&fixture, binary.clone(), None, &fixture.cell_ids);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());

    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &table,
            artifact,
            3,
            0,
            0,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3
            }
        )
    ));
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS - 1,
            0,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: AVAILABLE_COMPONENT_OPERATIONS,
                maximum
            }
        ) if maximum == AVAILABLE_COMPONENT_OPERATIONS - 1
    ));
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum
            }
        ) if maximum == WORKING_BYTES - 1
    ));

    let (_foreign_fixture, foreign_table, _foreign_artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![
            (EmbeddingStatus::Present, Some(alternating(9.0, 9.0))),
            (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
            (EmbeddingStatus::Present, Some(alternating(4.0, 2.0))),
            (EmbeddingStatus::Present, Some(alternating(6.0, 2.0))),
        ],
    );
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &input,
            nucleus_area.clone(),
            &foreign_table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch)
    ));

    let alternate_input = declared_input(&fixture, binary, None, &fixture.alternate_cell_ids);
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &alternate_input,
            nucleus_area.clone(),
            &table,
            artifact,
            3,
            0,
            0,
        ),
        Err(
            DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3
            }
        )
    ));
    assert!(matches!(
        declared_nucleus_area_cell_embedding_cross_covariance_energy(
            &fixture.project,
            &alternate_input,
            nucleus_area.clone(),
            &table,
            artifact,
            4,
            0,
            0,
        ),
        Err(DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::CellIdBindingMismatch { row: 0 })
    ));

    declared_nucleus_area_cell_embedding_cross_covariance_energy(
        &fixture.project,
        &input,
        nucleus_area,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("exact resource edges");

    let _typed_overflow = DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::SizeOverflow;
    let _typed_allocation =
        DeclaredNucleusAreaCellEmbeddingCrossCovarianceError::AllocationFailed {
            requested: WORKING_BYTES,
        };
}
