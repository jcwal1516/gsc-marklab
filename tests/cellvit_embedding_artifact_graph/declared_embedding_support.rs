use super::*;
use marklab::{
    BinaryMarkDeclaration, DeclaredScalarPatternInput, MeasurementStatus,
    ProbabilityMarkDeclaration, ScalarMarkId,
};

pub(super) fn fixture_status(status: EmbeddingStatus) -> FixtureEmbeddingStatus {
    match status {
        EmbeddingStatus::Present => FixtureEmbeddingStatus::Present,
        EmbeddingStatus::MissingVector => FixtureEmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed => FixtureEmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected => FixtureEmbeddingStatus::QcRejected,
    }
}

pub(super) fn verified_embedding(
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

pub(super) fn binary_declaration(
    fixture: &mut declared_scalar_support::Fixture,
    provenance_payload: &[u8],
) -> BinaryMarkDeclaration {
    let provenance_artifact_id = declared_scalar_support::publish_record(
        fixture,
        provenance_payload,
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

pub(super) fn probability_declaration(
    fixture: &mut declared_scalar_support::Fixture,
    provenance_payload: &[u8],
) -> ProbabilityMarkDeclaration {
    let provenance_artifact_id = declared_scalar_support::publish_record(
        fixture,
        provenance_payload,
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

pub(super) fn declared_input<'a>(
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

pub(super) fn alternating(dimension: u32, even: f32, odd: f32) -> Vec<f32> {
    (0..dimension)
        .map(|index| if index.is_multiple_of(2) { even } else { odd })
        .collect()
}
