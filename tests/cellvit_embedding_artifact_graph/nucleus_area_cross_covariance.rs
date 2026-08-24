use super::*;
use marklab::{
    declared_nucleus_area_cell_embedding_cross_covariance_energy, BinaryMarkDeclaration,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus, DeclaredScalarInputError,
    DeclaredScalarPatternInput, MarklabProject, MeasurementStatus, NucleusAreaUm2MarkDeclaration,
    ProbabilityMarkDeclaration, ScalarMarkId,
};

const DIMENSION: u32 = 1_280;
const AVAILABLE_COMPONENT_OPERATIONS: u64 = 12_800;
const ZERO_PRESENT_COMPONENT_OPERATIONS: u64 = 2_560;
const ONE_PRESENT_COMPONENT_OPERATIONS: u64 = 5_120;
const TWO_PRESENT_COMPONENT_OPERATIONS: u64 = 7_680;
const WORKING_BYTES: usize = 20_480;

fn fixture_status(status: EmbeddingStatus) -> FixtureEmbeddingStatus {
    match status {
        EmbeddingStatus::Present => FixtureEmbeddingStatus::Present,
        EmbeddingStatus::MissingVector => FixtureEmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed => FixtureEmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected => FixtureEmbeddingStatus::QcRejected,
    }
}

fn verified_embedding(
    cell_ids: &[CellId],
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    assert_eq!(cell_ids.len(), rows.len());
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        DIMENSION,
        cell_ids
            .iter()
            .cloned()
            .zip(rows.iter().map(|(status, _)| fixture_status(*status)))
            .collect(),
    );
    let domain_rows = cell_ids
        .iter()
        .cloned()
        .zip(rows)
        .map(|(cell_id, (status, vector))| match status {
            EmbeddingStatus::Present => {
                CellEmbeddingRow::present(cell_id, vector.expect("present vector"))
            }
            status => {
                assert!(vector.is_none());
                CellEmbeddingRow::non_present(cell_id, status).expect("non-present row")
            }
        })
        .collect();
    let verified = verified_graph(&fixture);
    let (table, bytes, embedding_record) =
        write_embedding_rows_arrow(&fixture, domain_rows, embedding_budgets());
    let embedding_receipt = verify_cell_embedding_table_arrow_bytes(
        &bytes,
        &embedding_record,
        &fixture.expected,
        &fixture.row_link,
        verified,
        embedding_budgets(),
    )
    .expect("verified embedding table");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let row_link_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &fixture.store,
        row_link_record,
        &fixture.expected,
        &fixture.row_link,
        embedding_budgets(),
    )
    .expect("verified row link");
    let artifact = CellEmbeddingArtifact::new(embedding_receipt, row_link_receipt, verified)
        .expect("verified cell-embedding artifact");
    (fixture, table, artifact)
}

fn binary_declaration(fixture: &mut declared_scalar_support::Fixture) -> BinaryMarkDeclaration {
    let provenance_artifact_id = declared_scalar_support::publish_record(
        fixture,
        b"declared-nucleus-area-covariance-binary",
        declared_scalar_support::MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        declared_scalar_support::binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    );
    BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        provenance_artifact_id,
    )
    .expect("binary declaration")
}

fn probability_declaration(
    fixture: &mut declared_scalar_support::Fixture,
) -> ProbabilityMarkDeclaration {
    let provenance_artifact_id = declared_scalar_support::publish_record(
        fixture,
        b"declared-nucleus-area-covariance-probability",
        declared_scalar_support::MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        declared_scalar_support::probability_metadata(
            "mmr_loss_probability",
            MeasurementStatus::ImportedPrediction,
        ),
    );
    ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability mark ID"),
        MeasurementStatus::ImportedPrediction,
        provenance_artifact_id,
    )
    .expect("probability declaration")
}

fn nucleus_area_declaration(
    fixture: &mut declared_scalar_support::Fixture,
) -> NucleusAreaUm2MarkDeclaration {
    let provenance_artifact_id = declared_scalar_support::publish_record(
        fixture,
        b"declared-nucleus-area-covariance",
        declared_scalar_support::MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        declared_scalar_support::nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, provenance_artifact_id)
        .expect("nucleus-area declaration")
}

fn declared_input<'a>(
    fixture: &'a declared_scalar_support::Fixture,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
    cell_ids: &'a [CellId],
) -> DeclaredScalarPatternInput<'a> {
    DeclaredScalarPatternInput::new(
        &fixture.project,
        &fixture.pattern,
        cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        binary,
        probability,
        16 * 1024,
        declared_scalar_support::cell_id_text_bytes(cell_ids),
    )
    .expect("declared scalar input")
}

fn alternating(even: f32, odd: f32) -> Vec<f32> {
    (0..DIMENSION)
        .map(|index| if index % 2 == 0 { even } else { odd })
        .collect()
}

fn oracle_rows() -> Vec<(EmbeddingStatus, Option<Vec<f32>>)> {
    vec![
        (EmbeddingStatus::Present, Some(alternating(0.0, 0.0))),
        (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
        (EmbeddingStatus::Present, Some(alternating(4.0, 2.0))),
        (EmbeddingStatus::Present, Some(alternating(6.0, 2.0))),
    ]
}

#[test]
fn nucleus_area_cross_covariance_matches_both_oracles_and_ignores_binary_rows() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 10.0, 20.0, 20.0].into_boxed_slice());
    fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
    let nucleus_area = nucleus_area_declaration(&mut fixture);
    let input = declared_input(
        &fixture,
        binary.clone(),
        Some(probability.clone()),
        &fixture.cell_ids,
    );
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());

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
