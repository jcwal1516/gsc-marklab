use std::str::FromStr;

use marklab::{
    patch_overlap_embedding_dispersion, publish_patch_embedding_table_arrow,
    read_patch_embedding_table_arrow_from_store, ArtifactId, ContentDigest,
    EmbeddingColumnarBudgets, EmbeddingStatus, ExpectedPatchSet, LocalArtifactStore,
    MeasurementStatus, MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingDerivationContract,
    MultiscaleEmbeddingExecutionProvenance, MultiscaleEmbeddingProvenance,
    MultiscaleEmbeddingProvenanceVariant, MultiscaleEmbeddingSupport, PatchEmbeddingRow,
    PatchEmbeddingTable, PatchEmbeddingTableReadBindings, PatchOverlapEmbeddingDispersionError,
    PatchOverlapEmbeddingDispersionStatus, PatchOverlapGraph, SlideId, StoreId,
};

#[allow(dead_code)]
#[path = "support/multiscale_columnar.rs"]
mod spatial_support;

const BUDGET: usize = 1 << 20;
const EXACT_COMPONENT_OPERATIONS: u64 = 6;

#[derive(Clone, Copy)]
enum RowValue {
    Present([f32; 2]),
    MissingVector,
    ExtractionFailed,
    QcRejected,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Drift {
    None,
    TableExpectedArtifact,
    TableProvenanceDigest,
    SupportOverlapDigest,
}

struct FlowFixture {
    expected: ExpectedPatchSet,
    table: PatchEmbeddingTable,
    support: MultiscaleEmbeddingSupport,
    provenance: MultiscaleEmbeddingProvenance,
    overlap: PatchOverlapGraph,
    overlap_artifact_id: ArtifactId,
}

fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn execution() -> MultiscaleEmbeddingExecutionProvenance {
    MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"dispersion-run-config"),
        artifact(b"dispersion-environment"),
        artifact(b"dispersion-converter"),
        "dispersion_converter",
        "1.0.0",
        BUDGET,
    )
    .expect("execution provenance")
}

fn direct_provenance(
    slide: &SlideId,
    support: &MultiscaleEmbeddingSupport,
    expected_artifact_id: ArtifactId,
    support_artifact_id: ArtifactId,
) -> MultiscaleEmbeddingProvenance {
    let model = MultiscaleDirectPatchModelProvenance::new(
        "patch_encoder",
        "1.0.0",
        "vit",
        artifact(b"dispersion-checkpoint"),
        ContentDigest::from_bytes(b"dispersion-checkpoint-content"),
        artifact(b"dispersion-source-snapshot"),
        artifact(b"dispersion-license"),
        "Apache-2.0",
        "doi:10.1000/dispersion-test",
        "encoder.output",
        1,
        BUDGET,
    )
    .expect("model provenance");
    let inputs = MultiscaleDirectPatchInputArtifacts::new(
        artifact(b"dispersion-normalization"),
        artifact(b"dispersion-source-entities"),
        artifact(b"dispersion-source-vectors"),
        expected_artifact_id,
        artifact(b"dispersion-identity-map"),
        artifact(b"dispersion-source-row-link"),
        support_artifact_id,
    );
    MultiscaleEmbeddingProvenance::direct_patch(
        slide.clone(),
        support,
        2,
        "mean_tokens",
        model,
        execution(),
        artifact(b"dispersion-preprocessing"),
        inputs,
        BUDGET,
    )
    .expect("direct patch provenance")
}

fn derived_region_provenance(slide: &SlideId) -> MultiscaleEmbeddingProvenance {
    let support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        MultiscaleArtifactBinding::new(
            artifact(b"dispersion-derived-patch-support"),
            ContentDigest::from_bytes(b"dispersion-derived-patch-support"),
        ),
        MultiscaleArtifactBinding::new(
            artifact(b"dispersion-derived-region-link"),
            ContentDigest::from_bytes(b"dispersion-derived-region-link"),
        ),
        BUDGET,
    )
    .expect("region support");
    let derivation =
        MultiscaleEmbeddingDerivationContract::weighted_mean("dispersion_weighted.v1", BUDGET)
            .expect("weighted derivation");
    MultiscaleEmbeddingProvenance::derived_region(
        slide.clone(),
        &support,
        &derivation,
        2,
        execution(),
        artifact(b"dispersion-derived-source-table"),
        artifact(b"dispersion-derived-region-link-artifact"),
        artifact(b"dispersion-derived-expected-regions"),
        artifact(b"dispersion-derived-support"),
        artifact(b"dispersion-derived-contract"),
        BUDGET,
    )
    .expect("derived provenance")
}

fn default_rows() -> [RowValue; 4] {
    [
        RowValue::Present([0.0, 0.0]),
        RowValue::Present([3.0, 4.0]),
        RowValue::Present([3.0, 0.0]),
        RowValue::Present([0.0, 0.0]),
    ]
}

fn build(rows: [RowValue; 4], drift: Drift) -> FlowFixture {
    let spatial = spatial_support::fixture(4);
    assert_eq!(spatial.overlap.edge_count(), 3);

    let support_artifact_id = artifact(b"dispersion-support");
    let overlap_artifact_id = artifact(b"dispersion-overlap");
    let overlap_digest = if drift == Drift::SupportOverlapDigest {
        ContentDigest::from_bytes(b"different-overlap-logical-digest")
    } else {
        spatial.overlap.logical_digest()
    };
    let support = MultiscaleEmbeddingSupport::patch(
        spatial.expected.owning_slide_id().clone(),
        MultiscaleArtifactBinding::new(
            spatial.context_artifact_id,
            spatial.context.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            spatial.overlap.patch_footprints_artifact_id(),
            spatial.footprints.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(overlap_artifact_id, overlap_digest),
        BUDGET,
    )
    .expect("patch support");
    let provenance = direct_provenance(
        spatial.expected.owning_slide_id(),
        &support,
        spatial.expected_artifact_id,
        support_artifact_id,
    );
    let table_expected_artifact_id = if drift == Drift::TableExpectedArtifact {
        artifact(b"different-expected-patches")
    } else {
        spatial.expected_artifact_id
    };
    let table_provenance_digest = if drift == Drift::TableProvenanceDigest {
        ContentDigest::from_bytes(b"different-provenance-logical-digest")
    } else {
        provenance.logical_digest()
    };
    let table_rows = spatial
        .expected
        .ids()
        .iter()
        .cloned()
        .zip(rows)
        .map(|(id, row)| match row {
            RowValue::Present(vector) => PatchEmbeddingRow::present(id, vector.to_vec()),
            RowValue::MissingVector => {
                PatchEmbeddingRow::non_present(id, EmbeddingStatus::MissingVector)
                    .expect("missing row")
            }
            RowValue::ExtractionFailed => {
                PatchEmbeddingRow::non_present(id, EmbeddingStatus::ExtractionFailed)
                    .expect("failed row")
            }
            RowValue::QcRejected => PatchEmbeddingRow::non_present(id, EmbeddingStatus::QcRejected)
                .expect("rejected row"),
        })
        .collect();
    let table = PatchEmbeddingTable::from_rows(
        2,
        &spatial.expected,
        table_expected_artifact_id,
        support_artifact_id,
        support.logical_digest(),
        artifact(b"dispersion-provenance"),
        table_provenance_digest,
        table_rows,
        BUDGET,
    )
    .expect("patch table");

    FlowFixture {
        expected: spatial.expected,
        table,
        support,
        provenance,
        overlap: spatial.overlap,
        overlap_artifact_id,
    }
}

#[test]
fn canonical_arrow_patch_table_materializes_into_the_dispersion_caller() {
    let fixture = build(default_rows(), Drift::None);
    let direct = compute(
        &fixture,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("direct dispersion");
    let root = tempfile::tempdir().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-materialization").expect("store ID"),
    )
    .expect("artifact store");
    let budgets = EmbeddingColumnarBudgets::new(BUDGET as u64, BUDGET, BUDGET, BUDGET as u64);
    let record = publish_patch_embedding_table_arrow(&store, &fixture.table, budgets)
        .expect("published Arrow table")
        .into_record();
    let bindings = PatchEmbeddingTableReadBindings::new(
        MultiscaleArtifactBinding::new(
            fixture.table.expected_entities_artifact_id(),
            fixture.expected.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            fixture.table.support_artifact_id(),
            fixture.support.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            fixture.table.provenance_artifact_id(),
            fixture.provenance.logical_digest(),
        ),
    )
    .expect("read bindings");
    let decoded = read_patch_embedding_table_arrow_from_store(
        &store,
        &record,
        &fixture.expected,
        bindings,
        budgets,
    )
    .expect("materialized Arrow table");
    let materialized = patch_overlap_embedding_dispersion(
        &decoded,
        &fixture.support,
        &fixture.overlap,
        &fixture.provenance,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("materialized dispersion");

    assert_eq!(decoded.logical_digest(), fixture.table.logical_digest());
    assert_eq!(materialized, direct);
}

#[test]
fn canonical_arrow_patch_materialization_rejects_a_false_support_identity() {
    let fixture = build(default_rows(), Drift::None);
    let root = tempfile::tempdir().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-binding-rejection").expect("store ID"),
    )
    .expect("artifact store");
    let budgets = EmbeddingColumnarBudgets::new(BUDGET as u64, BUDGET, BUDGET, BUDGET as u64);
    let record = publish_patch_embedding_table_arrow(&store, &fixture.table, budgets)
        .expect("published Arrow table")
        .into_record();
    let bindings = PatchEmbeddingTableReadBindings::new(
        MultiscaleArtifactBinding::new(
            fixture.table.expected_entities_artifact_id(),
            fixture.expected.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            fixture.table.support_artifact_id(),
            ContentDigest::from_bytes(b"false-support-logical-identity"),
        ),
        MultiscaleArtifactBinding::new(
            fixture.table.provenance_artifact_id(),
            fixture.provenance.logical_digest(),
        ),
    )
    .expect("distinct artifact bindings");

    assert!(read_patch_embedding_table_arrow_from_store(
        &store,
        &record,
        &fixture.expected,
        bindings,
        budgets,
    )
    .is_err());
}

fn compute(
    fixture: &FlowFixture,
    status: MeasurementStatus,
    maximum_component_operations: u64,
) -> Result<marklab::PatchOverlapEmbeddingDispersion, PatchOverlapEmbeddingDispersionError> {
    patch_overlap_embedding_dispersion(
        &fixture.table,
        &fixture.support,
        &fixture.overlap,
        &fixture.provenance,
        status,
        maximum_component_operations,
    )
}

#[test]
fn hand_computed_overlap_dispersion_is_observable_and_identity_bound() {
    let fixture = build(default_rows(), Drift::None);
    let result = compute(
        &fixture,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("bounded dispersion");

    assert_eq!(
        result.status(),
        PatchOverlapEmbeddingDispersionStatus::Available
    );
    assert_eq!(
        result.measurement_status(),
        MeasurementStatus::MorphologyPrediction
    );
    assert_eq!(result.total_edge_count(), 3);
    assert_eq!(result.eligible_edge_count(), 3);
    assert_eq!(result.excluded_edge_count(), 0);
    assert_eq!(result.dimension(), 2);
    assert_eq!(result.mean_squared_euclidean_distance(), Some(50.0 / 3.0));
    assert_eq!(
        result.table_logical_digest(),
        fixture.table.logical_digest()
    );
    assert_eq!(
        result.support_artifact_id(),
        fixture.table.support_artifact_id()
    );
    assert_eq!(
        result.support_logical_digest(),
        fixture.support.logical_digest()
    );
    assert_eq!(result.overlap_artifact_id(), fixture.overlap_artifact_id);
    assert_eq!(
        result.overlap_logical_digest(),
        fixture.overlap.logical_digest()
    );
    assert_eq!(
        result.provenance_artifact_id(),
        fixture.table.provenance_artifact_id()
    );
    assert_eq!(
        result.provenance_logical_digest(),
        fixture.provenance.logical_digest()
    );

    let repeated = compute(
        &fixture,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("repeated dispersion");
    assert_eq!(result, repeated);
}

#[test]
fn exact_signed_permutation_preserves_the_descriptive_value() {
    let original = build(default_rows(), Drift::None);
    let rotated = build(
        [
            RowValue::Present([-0.0, 0.0]),
            RowValue::Present([-4.0, 3.0]),
            RowValue::Present([-0.0, 3.0]),
            RowValue::Present([-0.0, 0.0]),
        ],
        Drift::None,
    );
    let original = compute(
        &original,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("original");
    let rotated = compute(
        &rotated,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("signed permutation");

    assert_eq!(
        original
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits(),
        rotated
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits()
    );
}

#[test]
fn extraction_validity_excludes_edges_but_present_zero_vectors_remain_eligible() {
    for non_present in [
        RowValue::MissingVector,
        RowValue::ExtractionFailed,
        RowValue::QcRejected,
    ] {
        let fixture = build(
            [
                RowValue::Present([0.0, 0.0]),
                non_present,
                RowValue::Present([3.0, 0.0]),
                RowValue::Present([0.0, 0.0]),
            ],
            Drift::None,
        );
        let result = compute(
            &fixture,
            MeasurementStatus::MorphologyPrediction,
            EXACT_COMPONENT_OPERATIONS,
        )
        .expect("status-aware dispersion");
        assert_eq!(result.eligible_edge_count(), 1);
        assert_eq!(result.excluded_edge_count(), 2);
        assert_eq!(result.mean_squared_euclidean_distance(), Some(9.0));
    }

    let all_zero = build([RowValue::Present([0.0, 0.0]); 4], Drift::None);
    let result = compute(
        &all_zero,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("all-zero present rows");
    assert_eq!(result.eligible_edge_count(), 3);
    assert_eq!(result.mean_squared_euclidean_distance(), Some(0.0));
    assert_eq!(
        result
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn no_eligible_pair_is_typed_unavailable_and_never_nan() {
    let fixture = build([RowValue::MissingVector; 4], Drift::None);
    let result = compute(
        &fixture,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("typed unavailable result");

    assert_eq!(
        result.status(),
        PatchOverlapEmbeddingDispersionStatus::InsufficientPairs
    );
    assert_eq!(result.eligible_edge_count(), 0);
    assert_eq!(result.excluded_edge_count(), 3);
    assert_eq!(result.mean_squared_euclidean_distance(), None);

    assert_eq!(
        compute(
            &fixture,
            MeasurementStatus::MorphologyPrediction,
            EXACT_COMPONENT_OPERATIONS - 1,
        ),
        Err(
            PatchOverlapEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: EXACT_COMPONENT_OPERATIONS,
                maximum: EXACT_COMPONENT_OPERATIONS - 1,
            }
        )
    );
}

#[test]
fn status_and_all_input_bindings_fail_before_arithmetic() {
    let exact = build(default_rows(), Drift::None);
    for observed in [
        MeasurementStatus::Measured,
        MeasurementStatus::ImportedPrediction,
        MeasurementStatus::DerivedSummary,
    ] {
        assert_eq!(
            compute(&exact, observed, 0),
            Err(
                PatchOverlapEmbeddingDispersionError::MeasurementStatusMismatch {
                    expected: MeasurementStatus::MorphologyPrediction,
                    observed,
                }
            )
        );
    }

    let derived = derived_region_provenance(exact.table.owning_slide_id());
    assert_eq!(
        patch_overlap_embedding_dispersion(
            &exact.table,
            &exact.support,
            &exact.overlap,
            &derived,
            MeasurementStatus::DerivedSummary,
            0,
        ),
        Err(
            PatchOverlapEmbeddingDispersionError::UnsupportedProvenanceVariant {
                observed: MultiscaleEmbeddingProvenanceVariant::DerivedRegion,
            }
        )
    );

    let expected_drift = build(default_rows(), Drift::TableExpectedArtifact);
    assert_eq!(
        compute(&expected_drift, MeasurementStatus::MorphologyPrediction, 0,),
        Err(PatchOverlapEmbeddingDispersionError::ExpectedPatchBindingMismatch)
    );

    let provenance_drift = build(default_rows(), Drift::TableProvenanceDigest);
    assert_eq!(
        compute(
            &provenance_drift,
            MeasurementStatus::MorphologyPrediction,
            0,
        ),
        Err(PatchOverlapEmbeddingDispersionError::ProvenanceBindingMismatch)
    );

    let wrong_support = MultiscaleEmbeddingSupport::patch(
        exact.table.owning_slide_id().clone(),
        MultiscaleArtifactBinding::new(
            artifact(b"wrong-context"),
            ContentDigest::from_bytes(b"wrong-context"),
        ),
        MultiscaleArtifactBinding::new(
            exact.overlap.patch_footprints_artifact_id(),
            exact.overlap.patch_footprints_logical_digest(),
        ),
        MultiscaleArtifactBinding::new(exact.overlap_artifact_id, exact.overlap.logical_digest()),
        BUDGET,
    )
    .expect("different support");
    assert_eq!(
        patch_overlap_embedding_dispersion(
            &exact.table,
            &wrong_support,
            &exact.overlap,
            &exact.provenance,
            MeasurementStatus::MorphologyPrediction,
            0,
        ),
        Err(PatchOverlapEmbeddingDispersionError::SupportBindingMismatch)
    );

    let overlap_drift = build(default_rows(), Drift::SupportOverlapDigest);
    assert_eq!(
        compute(&overlap_drift, MeasurementStatus::MorphologyPrediction, 0,),
        Err(PatchOverlapEmbeddingDispersionError::OverlapGraphBindingMismatch)
    );
}

#[test]
fn exact_work_cap_and_small_brute_force_oracle_agree() {
    let rows = default_rows();
    let fixture = build(rows, Drift::None);
    assert_eq!(
        compute(
            &fixture,
            MeasurementStatus::MorphologyPrediction,
            EXACT_COMPONENT_OPERATIONS - 1,
        ),
        Err(
            PatchOverlapEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: EXACT_COMPONENT_OPERATIONS,
                maximum: EXACT_COMPONENT_OPERATIONS - 1,
            }
        )
    );

    let observed = compute(
        &fixture,
        MeasurementStatus::MorphologyPrediction,
        EXACT_COMPONENT_OPERATIONS,
    )
    .expect("exact work cap")
    .mean_squared_euclidean_distance()
    .expect("available");
    let vectors = rows.map(|row| match row {
        RowValue::Present(vector) => Some(vector),
        RowValue::MissingVector | RowValue::ExtractionFailed | RowValue::QcRejected => None,
    });
    let expected_ids = [
        "patch-00000000",
        "patch-00000001",
        "patch-00000002",
        "patch-00000003",
    ];
    let mut total = 0.0_f64;
    let mut count = 0_u64;
    for edge in fixture.overlap.edges() {
        let left = expected_ids
            .iter()
            .position(|id| *id == edge.left_patch_id().as_str())
            .expect("known left patch");
        let right = expected_ids
            .iter()
            .position(|id| *id == edge.right_patch_id().as_str())
            .expect("known right patch");
        if let (Some(left), Some(right)) = (vectors[left], vectors[right]) {
            total += left
                .into_iter()
                .zip(right)
                .map(|(left, right)| {
                    let difference = f64::from(left) - f64::from(right);
                    difference * difference
                })
                .sum::<f64>();
            count += 1;
        }
    }
    assert_eq!(observed.to_bits(), (total / count as f64).to_bits());
}
