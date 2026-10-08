use super::declared_embedding_support as embedding_support;
use super::*;
use marklab::{
    declared_probability_cell_embedding_cross_covariance_energy, BinaryMarkDeclaration,
    DeclaredProbabilityCellEmbeddingCrossCovarianceError,
    DeclaredProbabilityCellEmbeddingCrossCovarianceStatus, DeclaredScalarPatternInput, MarkTable,
    MeasurementStatus, MissingnessPolicy, ProbabilityMarkDeclaration, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, VectorArtifactRefMarkDeclaration,
};

const DIMENSION: u32 = 1_280;
const AVAILABLE_COMPONENT_OPERATIONS: u64 = 12_800;
const ONE_PRESENT_COMPONENT_OPERATIONS: u64 = 5_120;
const TWO_PRESENT_COMPONENT_OPERATIONS: u64 = 7_680;
const WORKING_BYTES: usize = 20_480;

fn verified_embedding(
    cell_ids: &[CellId],
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    embedding_support::verified_embedding(cell_ids, DIMENSION, rows)
}

fn binary_declaration(fixture: &mut declared_scalar_support::Fixture) -> BinaryMarkDeclaration {
    embedding_support::binary_declaration(fixture, b"declared-probability-covariance-binary")
}

fn probability_declaration(
    fixture: &mut declared_scalar_support::Fixture,
) -> ProbabilityMarkDeclaration {
    embedding_support::probability_declaration(fixture, b"declared-probability-covariance")
}

fn declared_input<'a>(
    fixture: &'a declared_scalar_support::Fixture,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
    cell_ids: &'a [CellId],
) -> DeclaredScalarPatternInput<'a> {
    embedding_support::declared_input(fixture, binary, probability, cell_ids)
}

fn alternating(even: f32, odd: f32) -> Vec<f32> {
    embedding_support::alternating(DIMENSION, even, odd)
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
fn probability_cross_covariance_matches_oracle_and_binds_probability_values() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.mark_prob = Some(vec![0.0, 0.0, 1.0, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
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

    let result = declared_probability_cell_embedding_cross_covariance_energy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("probability cross-covariance energy");
    assert_eq!(
        result.status(),
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::Available
    );
    assert_eq!(result.present_row_count(), 4);
    assert_eq!(result.present_probability_mean(), Some(0.5));
    assert_eq!(result.dimension(), DIMENSION);
    assert_eq!(result.cross_covariance_energy(), Some(0.625));
    assert_eq!(result.binary_mark(), &binary);
    assert_eq!(result.probability_mark(), &probability);
    assert_eq!(result.scalar_identity().row_count(), 4);
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
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &changed_table,
            changed_artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES,
        ),
        Err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::VectorArtifactReferenceMismatch)
    ));

    let repeated = declared_probability_cell_embedding_cross_covariance_energy(
        &input,
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
        repeated.probability_values_logical_digest(),
        result.probability_values_logical_digest()
    );

    let mut swapped_binary_fixture = declared_scalar_support::fixture();
    swapped_binary_fixture.pattern.mark = vec![1, 0, 0, 1].into_boxed_slice();
    swapped_binary_fixture.pattern.mark_prob = Some(vec![0.0, 0.0, 1.0, 1.0].into_boxed_slice());
    let swapped_binary = binary_declaration(&mut swapped_binary_fixture);
    let swapped_probability = probability_declaration(&mut swapped_binary_fixture);
    assert_eq!(swapped_binary, binary);
    assert_eq!(swapped_probability, probability);
    let swapped_input = declared_input(
        &swapped_binary_fixture,
        swapped_binary,
        Some(swapped_probability),
        &swapped_binary_fixture.cell_ids,
    );
    let swapped = declared_probability_cell_embedding_cross_covariance_energy(
        &swapped_input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("binary-independent probability energy");
    assert_eq!(swapped.scalar_identity(), result.scalar_identity());
    assert_eq!(
        swapped.probability_values_logical_digest(),
        result.probability_values_logical_digest()
    );
    assert_eq!(
        swapped.cross_covariance_energy(),
        result.cross_covariance_energy()
    );

    let mut changed_probability_fixture = declared_scalar_support::fixture();
    changed_probability_fixture.pattern.mark_prob =
        Some(vec![0.0, 0.0, 0.5, 1.0].into_boxed_slice());
    let changed_binary = binary_declaration(&mut changed_probability_fixture);
    let changed_probability = probability_declaration(&mut changed_probability_fixture);
    let changed_input = declared_input(
        &changed_probability_fixture,
        changed_binary,
        Some(changed_probability),
        &changed_probability_fixture.cell_ids,
    );
    let changed = declared_probability_cell_embedding_cross_covariance_energy(
        &changed_input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("changed probability energy");
    assert_eq!(changed.scalar_identity(), result.scalar_identity());
    assert_ne!(
        changed.probability_values_logical_digest(),
        result.probability_values_logical_digest()
    );
    assert_eq!(changed.present_probability_mean(), Some(0.375));
    assert_eq!(changed.cross_covariance_energy(), Some(0.453125));
}

#[test]
fn probability_cross_covariance_canonicalizes_independent_equal_vectors_to_positive_zero() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, Some(probability), &fixture.cell_ids);
    let equal = (EmbeddingStatus::Present, Some(alternating(1.0, -2.0)));
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![equal.clone(), equal.clone(), equal.clone(), equal],
    );

    let result = declared_probability_cell_embedding_cross_covariance_energy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("zero cross-covariance energy");
    assert_eq!(
        result
            .cross_covariance_energy()
            .expect("zero value")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn probability_cross_covariance_excludes_statuses_and_types_unavailable_inputs() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.mark_prob = Some(vec![0.0, 0.25, 0.75, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, Some(probability), &fixture.cell_ids);
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![
            (EmbeddingStatus::Present, Some(alternating(2.0, 4.0))),
            (EmbeddingStatus::MissingVector, None),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
        ],
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &table,
            artifact,
            4,
            ONE_PRESENT_COMPONENT_OPERATIONS - 1,
            WORKING_BYTES,
        ),
        Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: ONE_PRESENT_COMPONENT_OPERATIONS,
                maximum: ONE_PRESENT_COMPONENT_OPERATIONS - 1,
            }
        )
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &table,
            artifact,
            4,
            ONE_PRESENT_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            }
        )
    );
    let insufficient = declared_probability_cell_embedding_cross_covariance_energy(
        &input,
        &table,
        artifact,
        4,
        ONE_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("insufficient present rows");
    assert_eq!(
        insufficient.status(),
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::InsufficientPresentRows
    );
    assert_eq!(insufficient.present_row_count(), 1);
    assert_eq!(insufficient.present_probability_mean(), Some(0.0));
    assert_eq!(insufficient.cross_covariance_energy(), None);
    assert_eq!(insufficient.embedding_qc_summary().present_count(), 1);
    assert_eq!(
        insufficient.embedding_qc_summary().missing_vector_count(),
        1
    );
    assert_eq!(
        insufficient
            .embedding_qc_summary()
            .extraction_failed_count(),
        1
    );
    assert_eq!(insufficient.embedding_qc_summary().qc_rejected_count(), 1);

    let mut constant_fixture = declared_scalar_support::fixture();
    constant_fixture.pattern.mark_prob = Some(vec![0.5, 0.5, 0.75, 1.0].into_boxed_slice());
    let constant_binary = binary_declaration(&mut constant_fixture);
    let constant_probability = probability_declaration(&mut constant_fixture);
    let constant_input = declared_input(
        &constant_fixture,
        constant_binary,
        Some(constant_probability),
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
    let constant = declared_probability_cell_embedding_cross_covariance_energy(
        &constant_input,
        &constant_table,
        constant_artifact,
        4,
        TWO_PRESENT_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("constant probability unavailability");
    assert_eq!(
        constant.status(),
        DeclaredProbabilityCellEmbeddingCrossCovarianceStatus::NoProbabilityVariation
    );
    assert_eq!(constant.present_row_count(), 2);
    assert_eq!(constant.present_probability_mean(), Some(0.5));
    assert_eq!(constant.cross_covariance_energy(), None);
}

#[test]
fn probability_cross_covariance_requires_the_declared_probability_modality() {
    let mut fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut fixture);
    let input = declared_input(&fixture, binary, None, &fixture.cell_ids);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());

    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input, &table, artifact, 0, 0, 0,
        ),
        Err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::ProbabilityMarkRequired)
    );
}

#[test]
fn probability_cross_covariance_rejects_binding_drift_and_enforces_exact_limits() {
    let mut fixture = declared_scalar_support::fixture();
    fixture.pattern.mark_prob = Some(vec![0.0, 0.0, 1.0, 1.0].into_boxed_slice());
    let binary = binary_declaration(&mut fixture);
    let probability = probability_declaration(&mut fixture);
    let input = declared_input(
        &fixture,
        binary.clone(),
        Some(probability.clone()),
        &fixture.cell_ids,
    );
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&fixture.cell_ids, oracle_rows());

    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input, &table, artifact, 3, 0, 0,
        ),
        Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3,
            }
        )
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS - 1,
            0,
        ),
        Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::ComponentOperationBudgetExceeded {
                required: AVAILABLE_COMPONENT_OPERATIONS,
                maximum: AVAILABLE_COMPONENT_OPERATIONS - 1,
            }
        )
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredProbabilityCellEmbeddingCrossCovarianceError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            }
        )
    );

    let (_foreign_fixture, foreign_table, _foreign_artifact) = verified_embedding(
        &fixture.cell_ids,
        vec![
            (EmbeddingStatus::Present, Some(alternating(9.0, 9.0))),
            (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
            (EmbeddingStatus::Present, Some(alternating(4.0, 2.0))),
            (EmbeddingStatus::Present, Some(alternating(6.0, 2.0))),
        ],
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &input,
            &foreign_table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::EmbeddingArtifactBindingMismatch)
    );

    let alternate_input = declared_input(
        &fixture,
        binary,
        Some(probability),
        &fixture.alternate_cell_ids,
    );
    assert_eq!(
        declared_probability_cell_embedding_cross_covariance_energy(
            &alternate_input,
            &table,
            artifact,
            4,
            0,
            0,
        ),
        Err(DeclaredProbabilityCellEmbeddingCrossCovarianceError::CellIdBindingMismatch { row: 0 })
    );

    declared_probability_cell_embedding_cross_covariance_energy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("exact resource edges");
}
