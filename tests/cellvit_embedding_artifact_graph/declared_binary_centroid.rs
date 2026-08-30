use super::declared_embedding_support as embedding_support;
use super::*;
use marklab::{
    declared_binary_cell_embedding_centroid_discrepancy, BinaryMarkDeclaration,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus, DeclaredScalarInputError,
    DeclaredScalarPatternInput, MarkTable, MeasurementStatus, MissingnessPolicy,
    ProbabilityMarkDeclaration, ScalarMarkColumn, ScalarMarkId, ScalarMarkModality, ScalarMarkUnit,
    VectorArtifactRefMarkDeclaration,
};

const DIMENSION: u32 = 1_280;
const AVAILABLE_COMPONENT_OPERATIONS: u64 = 6_400;
const UNAVAILABLE_COMPONENT_OPERATIONS: u64 = 2_560;
const WORKING_BYTES: usize = 20_480;

fn verified_embedding(
    cell_ids: &[CellId],
    dimension: u32,
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    embedding_support::verified_embedding(cell_ids, dimension, rows)
}

fn binary_declaration(fixture: &mut declared_scalar_support::Fixture) -> BinaryMarkDeclaration {
    embedding_support::binary_declaration(fixture, b"declared-binary-centroid")
}

fn probability_declaration(
    fixture: &mut declared_scalar_support::Fixture,
) -> ProbabilityMarkDeclaration {
    embedding_support::probability_declaration(fixture, b"declared-probability-centroid")
}

fn declared_input<'a>(
    fixture: &'a declared_scalar_support::Fixture,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
    cell_ids: &'a [CellId],
) -> DeclaredScalarPatternInput<'a> {
    embedding_support::declared_input(fixture, binary, probability, cell_ids)
}

fn available_rows() -> Vec<(EmbeddingStatus, Option<Vec<f32>>)> {
    vec![
        (
            EmbeddingStatus::Present,
            Some(vec![0.0; DIMENSION as usize]),
        ),
        (EmbeddingStatus::Present, Some(alternating(2.0, 4.0))),
        (EmbeddingStatus::Present, Some(alternating(4.0, 8.0))),
        (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
    ]
}

fn alternating(even: f32, odd: f32) -> Vec<f32> {
    embedding_support::alternating(DIMENSION, even, odd)
}

fn vector_declaration() -> VectorArtifactRefMarkDeclaration {
    VectorArtifactRefMarkDeclaration::new(
        ScalarMarkId::new("cellvit_embedding").expect("vector mark ID"),
        "CellViT embedding",
        MeasurementStatus::MorphologyPrediction,
    )
    .expect("vector declaration")
}

#[test]
fn verified_cellvit_artifact_is_a_row_bound_typed_vector_mark_for_the_centroid_caller() {
    let mut declared_fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut declared_fixture);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, available_rows());
    let vector_mark_id = ScalarMarkId::new("cellvit_embedding").expect("vector mark ID");
    let vector = ScalarMarkColumn::vector_artifact_ref(
        vector_declaration(),
        ScalarMarkModality::Morphology,
        ScalarMarkUnit::EmbeddingVector,
        MissingnessPolicy::NotPermitted,
        &table,
        artifact,
    )
    .expect("row-bound vector artifact reference");
    let mark_table = MarkTable::new(
        declared_fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                declared_fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            vector,
        ],
        4,
        declared_scalar_support::cell_id_text_bytes(&declared_fixture.cell_ids),
    )
    .expect("typed vector mark table");
    assert_eq!(
        mark_table.vector_artifact_ref(&vector_mark_id),
        Some(artifact)
    );
    let input = DeclaredScalarPatternInput::from_mark_table(
        &declared_fixture.project,
        &declared_fixture.pattern,
        &mark_table,
        declared_fixture.slide_id.clone(),
        declared_fixture.frame_id.clone(),
    )
    .expect("declared vector input");
    let result = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("typed vector centroid discrepancy");
    assert_eq!(result.mean_squared_component_difference(), Some(20.0));
}

#[test]
fn vector_artifact_reference_rejects_row_qc_and_caller_identity_drift() {
    let mut declared_fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut declared_fixture);
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, available_rows());
    let mut changed_rows = available_rows();
    changed_rows[0] = (
        EmbeddingStatus::Present,
        Some(vec![1.0; DIMENSION as usize]),
    );
    let (_changed_fixture, changed_table, changed_artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, changed_rows);
    assert!(matches!(
        ScalarMarkColumn::vector_artifact_ref(
            vector_declaration(),
            ScalarMarkModality::Morphology,
            ScalarMarkUnit::EmbeddingVector,
            MissingnessPolicy::NotPermitted,
            &table,
            changed_artifact,
        ),
        Err(DeclaredScalarInputError::VectorArtifactBindingMismatch)
    ));
    let vector = ScalarMarkColumn::vector_artifact_ref(
        vector_declaration(),
        ScalarMarkModality::Morphology,
        ScalarMarkUnit::EmbeddingVector,
        MissingnessPolicy::NotPermitted,
        &table,
        artifact,
    )
    .expect("vector reference");
    assert!(matches!(
        MarkTable::new(
            declared_fixture.alternate_cell_ids.clone(),
            vec![
                ScalarMarkColumn::binary(
                    binary.clone(),
                    ScalarMarkModality::Immunohistochemistry,
                    ScalarMarkUnit::Unitless,
                    MissingnessPolicy::NotPermitted,
                    declared_fixture.pattern.mark.clone(),
                )
                .expect("binary column"),
                vector.clone(),
            ],
            4,
            declared_scalar_support::cell_id_text_bytes(&declared_fixture.alternate_cell_ids),
        ),
        Err(DeclaredScalarInputError::VectorArtifactCellIdentityMismatch)
    ));
    let mark_table = MarkTable::new(
        declared_fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                declared_fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            vector,
        ],
        4,
        declared_scalar_support::cell_id_text_bytes(&declared_fixture.cell_ids),
    )
    .expect("vector mark table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &declared_fixture.project,
        &declared_fixture.pattern,
        &mark_table,
        declared_fixture.slide_id.clone(),
        declared_fixture.frame_id.clone(),
    )
    .expect("declared vector input");
    assert!(matches!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &changed_table,
            changed_artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES,
        ),
        Err(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::VectorArtifactReferenceMismatch)
    ));

    let mut missing_rows = available_rows();
    missing_rows[0] = (EmbeddingStatus::MissingVector, None);
    let (_missing_fixture, missing_table, missing_artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, missing_rows);
    assert!(matches!(
        ScalarMarkColumn::vector_artifact_ref(
            vector_declaration(),
            ScalarMarkModality::Morphology,
            ScalarMarkUnit::EmbeddingVector,
            MissingnessPolicy::NotPermitted,
            &missing_table,
            missing_artifact,
        ),
        Err(DeclaredScalarInputError::VectorArtifactMissingnessMismatch)
    ));
    let nullable_vector = ScalarMarkColumn::vector_artifact_ref(
        vector_declaration(),
        ScalarMarkModality::Morphology,
        ScalarMarkUnit::EmbeddingVector,
        MissingnessPolicy::Allowed,
        &missing_table,
        missing_artifact,
    )
    .expect("explicitly nullable vector reference");
    let nullable_mark_table = MarkTable::new(
        declared_fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary,
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                declared_fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            nullable_vector,
        ],
        4,
        declared_scalar_support::cell_id_text_bytes(&declared_fixture.cell_ids),
    )
    .expect("nullable vector MarkTable");
    let nullable_input = DeclaredScalarPatternInput::from_mark_table(
        &declared_fixture.project,
        &declared_fixture.pattern,
        &nullable_mark_table,
        declared_fixture.slide_id.clone(),
        declared_fixture.frame_id.clone(),
    )
    .expect("nullable typed vector input");
    let nullable_result = declared_binary_cell_embedding_centroid_discrepancy(
        &nullable_input,
        &missing_table,
        missing_artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("status-aware vector result");
    assert_eq!(
        nullable_result.marked_counts().missing_vector_count()
            + nullable_result.unmarked_counts().missing_vector_count(),
        1
    );
}

#[test]
fn declared_binary_centroid_is_hand_computed_identity_bound_and_binary_routed() {
    let mut declared_fixture = declared_scalar_support::fixture();
    declared_fixture.pattern.mark_prob = Some(vec![0.99, 0.01, 0.02, 0.98].into_boxed_slice());
    let binary = binary_declaration(&mut declared_fixture);
    let probability = probability_declaration(&mut declared_fixture);
    let input = declared_input(
        &declared_fixture,
        binary.clone(),
        Some(probability.clone()),
        &declared_fixture.cell_ids,
    );
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, available_rows());

    let result = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("declared binary centroid discrepancy");
    assert_eq!(
        result.status(),
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available
    );
    assert_eq!(result.dimension(), DIMENSION);
    assert_eq!(result.mean_squared_component_difference(), Some(20.0));
    assert_eq!(result.marked_counts().row_count(), 2);
    assert_eq!(result.marked_counts().present_count(), 2);
    assert_eq!(result.unmarked_counts().row_count(), 2);
    assert_eq!(result.unmarked_counts().present_count(), 2);
    assert_eq!(result.binary_mark(), &binary);
    assert_eq!(result.probability_mark(), Some(&probability));
    assert_eq!(result.scalar_identity().row_count(), 4);
    assert_eq!(
        result.scalar_identity().owning_slide_id(),
        &declared_fixture.slide_id
    );
    assert_eq!(
        result.scalar_identity().coordinate_frame_id(),
        &declared_fixture.frame_id
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
    assert_eq!(result.embedding_qc_summary(), artifact.qc_summary());
    assert_eq!(result.table_logical_digest(), artifact.logical_digest());

    let repeated = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("repeat discrepancy");
    assert_eq!(
        repeated
            .mean_squared_component_difference()
            .expect("repeat value")
            .to_bits(),
        result
            .mean_squared_component_difference()
            .expect("value")
            .to_bits()
    );
    assert_eq!(
        repeated.binary_grouping_logical_digest(),
        result.binary_grouping_logical_digest()
    );

    let mut swapped_fixture = declared_scalar_support::fixture();
    swapped_fixture.pattern.mark = vec![1, 0, 0, 1].into_boxed_slice();
    swapped_fixture.pattern.mark_prob = Some(vec![0.99, 0.01, 0.02, 0.98].into_boxed_slice());
    let swapped_binary = binary_declaration(&mut swapped_fixture);
    let swapped_probability = probability_declaration(&mut swapped_fixture);
    assert_eq!(swapped_binary, binary);
    assert_eq!(swapped_probability, probability);
    let swapped_input = declared_input(
        &swapped_fixture,
        swapped_binary,
        Some(swapped_probability),
        &swapped_fixture.cell_ids,
    );
    let swapped = declared_binary_cell_embedding_centroid_discrepancy(
        &swapped_input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("swapped binary grouping");
    assert_eq!(swapped.scalar_identity(), result.scalar_identity());
    assert_eq!(swapped.marked_counts(), result.marked_counts());
    assert_eq!(swapped.unmarked_counts(), result.unmarked_counts());
    assert_eq!(
        swapped.mean_squared_component_difference(),
        result.mean_squared_component_difference()
    );
    assert_ne!(
        swapped.binary_grouping_logical_digest(),
        result.binary_grouping_logical_digest()
    );
}

#[test]
fn declared_binary_centroid_canonicalizes_equal_group_centroids_to_positive_zero() {
    let mut declared_fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut declared_fixture);
    let input = declared_input(&declared_fixture, binary, None, &declared_fixture.cell_ids);
    let equal = (EmbeddingStatus::Present, Some(alternating(1.0, -2.0)));
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &declared_fixture.cell_ids,
        DIMENSION,
        vec![equal.clone(), equal.clone(), equal.clone(), equal],
    );

    let result = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("equal centroid discrepancy");
    assert_eq!(
        result
            .mean_squared_component_difference()
            .expect("zero discrepancy")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn declared_binary_centroid_excludes_and_counts_every_non_present_status() {
    let mut declared_fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut declared_fixture);
    let input = declared_input(&declared_fixture, binary, None, &declared_fixture.cell_ids);
    let (_embedding_fixture, table, artifact) = verified_embedding(
        &declared_fixture.cell_ids,
        DIMENSION,
        vec![
            (
                EmbeddingStatus::Present,
                Some(vec![0.0; DIMENSION as usize]),
            ),
            (EmbeddingStatus::MissingVector, None),
            (EmbeddingStatus::ExtractionFailed, None),
            (EmbeddingStatus::QcRejected, None),
        ],
    );

    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &table,
            artifact,
            4,
            UNAVAILABLE_COMPONENT_OPERATIONS - 1,
            WORKING_BYTES,
        ),
        Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::ComponentOperationBudgetExceeded {
                required: UNAVAILABLE_COMPONENT_OPERATIONS,
                maximum: UNAVAILABLE_COMPONENT_OPERATIONS - 1,
            }
        )
    );
    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &table,
            artifact,
            4,
            UNAVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            }
        )
    );
    let result = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        UNAVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("typed insufficient groups");
    assert_eq!(
        result.status(),
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups
    );
    assert_eq!(result.mean_squared_component_difference(), None);
    assert_eq!(result.marked_counts().row_count(), 2);
    assert_eq!(result.marked_counts().present_count(), 0);
    assert_eq!(result.marked_counts().missing_vector_count(), 1);
    assert_eq!(result.marked_counts().extraction_failed_count(), 1);
    assert_eq!(result.marked_counts().qc_rejected_count(), 0);
    assert_eq!(result.unmarked_counts().row_count(), 2);
    assert_eq!(result.unmarked_counts().present_count(), 1);
    assert_eq!(result.unmarked_counts().missing_vector_count(), 0);
    assert_eq!(result.unmarked_counts().extraction_failed_count(), 0);
    assert_eq!(result.unmarked_counts().qc_rejected_count(), 1);
}

#[test]
fn declared_binary_centroid_rejects_binding_drift_and_enforces_exact_limits() {
    let mut declared_fixture = declared_scalar_support::fixture();
    let binary = binary_declaration(&mut declared_fixture);
    let input = declared_input(
        &declared_fixture,
        binary.clone(),
        None,
        &declared_fixture.cell_ids,
    );
    let (_embedding_fixture, table, artifact) =
        verified_embedding(&declared_fixture.cell_ids, DIMENSION, available_rows());

    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(&input, &table, artifact, 3, 0, 0,),
        Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::RowCountBudgetExceeded {
                required: 4,
                maximum: 3,
            }
        )
    );
    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS - 1,
            0,
        ),
        Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::ComponentOperationBudgetExceeded {
                required: AVAILABLE_COMPONENT_OPERATIONS,
                maximum: AVAILABLE_COMPONENT_OPERATIONS - 1,
            }
        )
    );
    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &table,
            artifact,
            4,
            AVAILABLE_COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
        ),
        Err(
            DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            }
        )
    );

    let (_foreign_fixture, foreign_table, _foreign_artifact) = verified_embedding(
        &declared_fixture.cell_ids,
        DIMENSION,
        vec![
            (
                EmbeddingStatus::Present,
                Some(vec![9.0; DIMENSION as usize]),
            ),
            (EmbeddingStatus::Present, Some(alternating(2.0, 4.0))),
            (EmbeddingStatus::Present, Some(alternating(4.0, 8.0))),
            (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
        ],
    );
    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &input,
            &foreign_table,
            artifact,
            0,
            0,
            0,
        ),
        Err(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::EmbeddingArtifactBindingMismatch)
    );

    let alternate_input = declared_input(
        &declared_fixture,
        binary,
        None,
        &declared_fixture.alternate_cell_ids,
    );
    assert_eq!(
        declared_binary_cell_embedding_centroid_discrepancy(
            &alternate_input,
            &table,
            artifact,
            4,
            0,
            0,
        ),
        Err(DeclaredBinaryCellEmbeddingCentroidDiscrepancyError::CellIdBindingMismatch { row: 0 })
    );

    declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &table,
        artifact,
        4,
        AVAILABLE_COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("exact resource edges");
}
