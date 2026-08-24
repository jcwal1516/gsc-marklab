use super::*;
use marklab::{
    declared_binary_cell_embedding_centroid_discrepancy, BinaryMarkDeclaration,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyError,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus, DeclaredScalarPatternInput,
    MeasurementStatus, ProbabilityMarkDeclaration, ScalarMarkId,
};

#[allow(dead_code)]
#[path = "../support/declared_scalar.rs"]
mod declared_scalar_support;

const DIMENSION: u32 = 1_280;
const AVAILABLE_COMPONENT_OPERATIONS: u64 = 6_400;
const UNAVAILABLE_COMPONENT_OPERATIONS: u64 = 2_560;
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
    dimension: u32,
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    assert_eq!(cell_ids.len(), rows.len());
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        dimension,
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
        b"declared-binary-centroid",
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
        b"declared-probability-centroid",
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
    (0..DIMENSION)
        .map(|index| if index % 2 == 0 { even } else { odd })
        .collect()
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
