use super::declared_embedding_support as embedding_support;
use super::*;
use marklab::{
    declared_nucleus_area_cell_embedding_cross_covariance_energy, BinaryMarkDeclaration,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceError,
    DeclaredNucleusAreaCellEmbeddingCrossCovarianceStatus, DeclaredScalarInputError,
    DeclaredScalarPatternInput, MarkTable, MarklabProject, MeasurementStatus, MissingnessPolicy,
    NucleusAreaUm2MarkDeclaration, ProbabilityMarkDeclaration, ScalarMarkColumn, ScalarMarkId,
    ScalarMarkModality, ScalarMarkUnit, VectorArtifactRefMarkDeclaration,
};

const DIMENSION: u32 = 1_280;
const AVAILABLE_COMPONENT_OPERATIONS: u64 = 12_800;
const ZERO_PRESENT_COMPONENT_OPERATIONS: u64 = 2_560;
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
    embedding_support::binary_declaration(fixture, b"declared-nucleus-area-covariance-binary")
}

fn probability_declaration(
    fixture: &mut declared_scalar_support::Fixture,
) -> ProbabilityMarkDeclaration {
    embedding_support::probability_declaration(
        fixture,
        b"declared-nucleus-area-covariance-probability",
    )
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

#[path = "nucleus_area_cross_covariance/oracles.rs"]
mod oracles;
#[path = "nucleus_area_cross_covariance/validation.rs"]
mod validation;
