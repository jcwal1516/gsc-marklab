use super::*;
use marklab::{
    slide_embedding_aggregation_path_discrepancy, MultiscaleEmbeddingProvenanceVariant,
    SlideEmbeddingAggregationPathDiscrepancy, SlideEmbeddingAggregationPathDiscrepancyError,
    SlideEmbeddingAggregationPathDiscrepancyStatus,
};

struct PathPair {
    patches: SlideFixture,
    patch_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    patch_candidate: DerivedSlideEmbeddingTableCandidate,
    regions: SlideFixture,
    region_graph: VerifiedDerivedSlideEmbeddingArtifactGraph,
    region_candidate: DerivedSlideEmbeddingTableCandidate,
}

fn path_pair(options: SlideFixtureOptions) -> PathPair {
    let patches = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Patches,
        ..options
    });
    let patch_graph = validate_slide_graph(&patches);
    let patch_candidate =
        finalize_slide_with_budgets(&patches, patch_graph, finalization_budgets())
            .expect("patch-sourced slide candidate");

    let regions = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Regions,
        ..options
    });
    let region_graph = validate_slide_graph(&regions);
    let region_candidate =
        finalize_slide_with_budgets(&regions, region_graph, finalization_budgets())
            .expect("region-sourced slide candidate");

    PathPair {
        patches,
        patch_graph,
        patch_candidate,
        regions,
        region_graph,
        region_candidate,
    }
}

fn discrepancy(
    pair: &PathPair,
    maximum_component_operations: u64,
) -> Result<SlideEmbeddingAggregationPathDiscrepancy, SlideEmbeddingAggregationPathDiscrepancyError>
{
    slide_embedding_aggregation_path_discrepancy(
        &pair.patch_candidate,
        pair.patch_graph,
        &pair.patches.provenance,
        &pair.region_candidate,
        pair.region_graph,
        &pair.regions.provenance,
        source_region_receipt(&pair.regions),
        maximum_component_operations,
    )
}

#[test]
fn slide_path_discrepancy_is_hand_computed_observable_and_identity_bound() {
    let pair = path_pair(SlideFixtureOptions {
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::PathDiscrepancy,
        ..SlideFixtureOptions::default()
    });

    let result = discrepancy(&pair, 2).expect("slide aggregation-path discrepancy");
    assert_eq!(
        result.status(),
        SlideEmbeddingAggregationPathDiscrepancyStatus::Available
    );
    assert_eq!(result.patch_path_status(), EmbeddingStatus::Present);
    assert_eq!(result.region_path_status(), EmbeddingStatus::Present);
    assert_eq!(
        result.patch_measurement_status(),
        MeasurementStatus::DerivedSummary
    );
    assert_eq!(
        result.region_measurement_status(),
        MeasurementStatus::DerivedSummary
    );
    assert_eq!(result.dimension(), 2);
    assert_eq!(result.mean_squared_component_difference(), Some(0.625));

    let region_receipt = source_region_receipt(&pair.regions);
    assert_eq!(
        result.expected_slides_artifact_id(),
        pair.patches.expected_slide_record.id()
    );
    assert_eq!(
        result.expected_slides_logical_digest(),
        pair.patches.expected_slides.logical_digest()
    );
    assert_eq!(
        result.source_patch_table_artifact_id(),
        region_receipt.source_patch_table_artifact_id()
    );
    assert_eq!(
        result.source_patch_table_logical_digest(),
        region_receipt.source_patch_table_logical_digest()
    );
    assert_eq!(
        result.region_table_artifact_id(),
        region_receipt.artifact_id()
    );
    assert_eq!(
        result.region_table_logical_digest(),
        region_receipt.logical_digest()
    );
    assert_eq!(
        result.patch_region_link_artifact_id(),
        region_receipt.patch_region_link_artifact_id()
    );
    assert_eq!(
        result.patch_region_link_logical_digest(),
        region_receipt.patch_region_link_logical_digest()
    );
    assert_eq!(
        result.patch_slide_support_artifact_id(),
        pair.patches.support_record.id()
    );
    assert_eq!(
        result.patch_slide_support_logical_digest(),
        pair.patches.support_value.logical_digest()
    );
    assert_eq!(
        result.region_slide_support_artifact_id(),
        pair.regions.support_record.id()
    );
    assert_eq!(
        result.region_slide_support_logical_digest(),
        pair.regions.support_value.logical_digest()
    );
    assert_eq!(
        result.patch_slide_candidate_logical_digest(),
        pair.patch_candidate.logical_digest()
    );
    assert_eq!(
        result.region_slide_candidate_logical_digest(),
        pair.region_candidate.logical_digest()
    );
    assert_eq!(
        result.patch_slide_provenance_artifact_id(),
        pair.patches.provenance_record.id()
    );
    assert_eq!(
        result.patch_slide_provenance_logical_digest(),
        pair.patches.provenance.logical_digest()
    );
    assert_eq!(
        result.region_slide_provenance_artifact_id(),
        pair.regions.provenance_record.id()
    );
    assert_eq!(
        result.region_slide_provenance_logical_digest(),
        pair.regions.provenance.logical_digest()
    );

    let repeated = discrepancy(&pair, 2).expect("repeat discrepancy");
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
        discrepancy(&pair, 1),
        Err(
            SlideEmbeddingAggregationPathDiscrepancyError::ComponentOperationBudgetExceeded {
                required: 2,
                maximum: 1,
            }
        )
    );
}

#[test]
fn slide_path_discrepancy_canonicalizes_equal_vectors_to_positive_zero() {
    let pair = path_pair(SlideFixtureOptions {
        entity_count: 3,
        region_count: 2,
        output_dimension: 2,
        ..SlideFixtureOptions::default()
    });

    let result = discrepancy(&pair, 2).expect("equal path discrepancy");
    assert_eq!(
        result.status(),
        SlideEmbeddingAggregationPathDiscrepancyStatus::Available
    );
    assert_eq!(
        result
            .mean_squared_component_difference()
            .expect("zero discrepancy")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn slide_path_discrepancy_missing_region_path_is_typed_and_still_bounded() {
    let pair = path_pair(SlideFixtureOptions {
        region_count: 0,
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::PathDiscrepancy,
        ..SlideFixtureOptions::default()
    });

    assert_eq!(
        discrepancy(&pair, 1),
        Err(
            SlideEmbeddingAggregationPathDiscrepancyError::ComponentOperationBudgetExceeded {
                required: 2,
                maximum: 1,
            }
        )
    );
    let result = discrepancy(&pair, 2).expect("typed unavailable discrepancy");
    assert_eq!(
        result.status(),
        SlideEmbeddingAggregationPathDiscrepancyStatus::InsufficientComparablePaths
    );
    assert_eq!(result.patch_path_status(), EmbeddingStatus::Present);
    assert_eq!(result.region_path_status(), EmbeddingStatus::MissingVector);
    assert_eq!(result.mean_squared_component_difference(), None);
}

#[test]
fn slide_path_discrepancy_rejects_variants_and_candidate_drift_before_budget() {
    let pair = path_pair(SlideFixtureOptions {
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::PathDiscrepancy,
        ..SlideFixtureOptions::default()
    });

    assert_eq!(
        slide_embedding_aggregation_path_discrepancy(
            &pair.patch_candidate,
            pair.patch_graph,
            &pair.regions.provenance,
            &pair.region_candidate,
            pair.region_graph,
            &pair.regions.provenance,
            source_region_receipt(&pair.regions),
            0,
        ),
        Err(
            SlideEmbeddingAggregationPathDiscrepancyError::UnsupportedPatchProvenanceVariant {
                observed: MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromRegions,
            }
        )
    );
    assert_eq!(
        slide_embedding_aggregation_path_discrepancy(
            &pair.patch_candidate,
            pair.patch_graph,
            &pair.patches.provenance,
            &pair.region_candidate,
            pair.region_graph,
            &pair.patches.provenance,
            source_region_receipt(&pair.regions),
            0,
        ),
        Err(
            SlideEmbeddingAggregationPathDiscrepancyError::UnsupportedRegionProvenanceVariant {
                observed: MultiscaleEmbeddingProvenanceVariant::DerivedSlideFromPatches,
            }
        )
    );

    let drift = path_pair(SlideFixtureOptions {
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::AllZero,
        ..SlideFixtureOptions::default()
    });
    assert_eq!(
        slide_embedding_aggregation_path_discrepancy(
            &pair.patch_candidate,
            drift.patch_graph,
            &drift.patches.provenance,
            &pair.region_candidate,
            pair.region_graph,
            &pair.regions.provenance,
            source_region_receipt(&pair.regions),
            0,
        ),
        Err(SlideEmbeddingAggregationPathDiscrepancyError::PatchPathBindingMismatch)
    );

    let different_dimension = path_pair(SlideFixtureOptions {
        output_dimension: 3,
        ..SlideFixtureOptions::default()
    });
    assert_eq!(
        slide_embedding_aggregation_path_discrepancy(
            &pair.patch_candidate,
            pair.patch_graph,
            &pair.patches.provenance,
            &different_dimension.region_candidate,
            different_dimension.region_graph,
            &different_dimension.regions.provenance,
            source_region_receipt(&different_dimension.regions),
            0,
        ),
        Err(SlideEmbeddingAggregationPathDiscrepancyError::CommonContextMismatch)
    );
}

#[test]
fn slide_path_discrepancy_rejects_same_slide_from_different_patch_lineage() {
    let pair = path_pair(SlideFixtureOptions {
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::PathDiscrepancy,
        ..SlideFixtureOptions::default()
    });
    let foreign = path_pair(SlideFixtureOptions {
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::AllZero,
        ..SlideFixtureOptions::default()
    });

    assert_eq!(
        slide_embedding_aggregation_path_discrepancy(
            &pair.patch_candidate,
            pair.patch_graph,
            &pair.patches.provenance,
            &foreign.region_candidate,
            foreign.region_graph,
            &foreign.regions.provenance,
            source_region_receipt(&foreign.regions),
            0,
        ),
        Err(SlideEmbeddingAggregationPathDiscrepancyError::SourceLineageMismatch)
    );
}
