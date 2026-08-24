use super::*;

const EXACT_COMPONENT_OPERATIONS: u64 = 9;

fn finalization_budgets() -> EmbeddingFinalizationBudgets {
    EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, BUDGET as u64, BUDGET as u64)
}

fn finalize_candidate(
    fixture: &DerivedFixture,
    graph: VerifiedDerivedRegionEmbeddingArtifactGraph,
) -> marklab::DerivedRegionEmbeddingTableCandidate {
    finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        finalization_budgets(),
    )
    .expect("derived region candidate")
}

fn compute(
    fixture: &DerivedFixture,
    maximum_component_operations: u64,
) -> Result<marklab::PatchRegionEmbeddingDispersion, PatchRegionEmbeddingDispersionError> {
    let graph = validate_graph(fixture);
    let candidate = finalize_candidate(fixture, graph);
    patch_region_embedding_dispersion(
        &fixture.source_patch_table_value,
        &fixture.link,
        &candidate,
        graph,
        &fixture._direct.provenance,
        &fixture.provenance,
        maximum_component_operations,
    )
}

#[test]
fn region_dispersion_is_hand_computed_observable_and_identity_bound() {
    let fixture = derived_fixture();
    let result = compute(&fixture, EXACT_COMPONENT_OPERATIONS).expect("region dispersion");

    let region = [
        f32::from_bits(0x3faaaaab),
        f32::from_bits(0x40155555),
        f32::from_bits(0x40555555),
    ];
    let first_distance = [1.0_f32, 2.0, 3.0]
        .into_iter()
        .zip(region)
        .map(|(patch, region)| {
            let difference = f64::from(patch) - f64::from(region);
            difference * difference
        })
        .sum::<f64>();
    let second_distance = [2.0_f32, 3.0, 4.0]
        .into_iter()
        .zip(region)
        .map(|(patch, region)| {
            let difference = f64::from(patch) - f64::from(region);
            difference * difference
        })
        .sum::<f64>();
    let expected = (first_distance + 0.5 * second_distance) / 2.5;

    assert_eq!(
        result.status(),
        PatchRegionEmbeddingDispersionStatus::Available
    );
    assert_eq!(
        result.source_measurement_status(),
        MeasurementStatus::MorphologyPrediction
    );
    assert_eq!(
        result.region_measurement_status(),
        MeasurementStatus::DerivedSummary
    );
    assert_eq!(result.total_relation_count(), 3);
    assert_eq!(result.eligible_relation_count(), 3);
    assert_eq!(result.excluded_relation_count(), 0);
    assert_eq!(result.dimension(), 3);
    assert_eq!(
        result
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits(),
        expected.to_bits()
    );
    assert_eq!(
        result.source_table_artifact_id(),
        fixture.source_patch_table.artifact_id()
    );
    assert_eq!(
        result.source_table_logical_digest(),
        fixture.source_patch_table_value.logical_digest()
    );
    assert_eq!(
        result.source_provenance_artifact_id(),
        fixture.source_patch_table_value.provenance_artifact_id()
    );
    assert_eq!(
        result.source_provenance_logical_digest(),
        fixture._direct.provenance.logical_digest()
    );
    assert_eq!(
        result.link_artifact_id(),
        fixture.link_receipt.artifact_id()
    );
    assert_eq!(result.link_logical_digest(), fixture.link.logical_digest());
    let graph = validate_graph(&fixture);
    let candidate = finalize_candidate(&fixture, graph);
    assert_eq!(
        result.region_table_logical_digest(),
        candidate.logical_digest()
    );
    assert_eq!(
        result.region_provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(
        result.region_provenance_logical_digest(),
        fixture.provenance.logical_digest()
    );

    assert_eq!(
        result,
        compute(&fixture, EXACT_COMPONENT_OPERATIONS).expect("repeat")
    );
}

#[test]
fn region_dispersion_is_axis_sign_flip_invariant() {
    let original = derived_fixture();
    let sign_flipped = derived_fixture_with_options(DerivedFixtureOptions {
        source_vector_pattern: SourceVectorPattern::AxisSignFlip,
        ..DerivedFixtureOptions::default()
    });

    assert_eq!(
        compute(&original, EXACT_COMPONENT_OPERATIONS)
            .expect("original")
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits(),
        compute(&sign_flipped, EXACT_COMPONENT_OPERATIONS)
            .expect("sign-flipped")
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits()
    );
}

#[test]
fn region_dispersion_excludes_non_present_sources_but_keeps_present_zero_vectors() {
    let statuses = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 4,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..DerivedFixtureOptions::default()
    });
    let result = compute(&statuses, 15).expect("status-aware dispersion");
    assert_eq!(result.total_relation_count(), 5);
    assert_eq!(result.eligible_relation_count(), 1);
    assert_eq!(result.excluded_relation_count(), 4);
    assert_eq!(result.mean_squared_euclidean_distance(), Some(0.0));

    let zero = derived_fixture_with_options(DerivedFixtureOptions {
        source_vector_pattern: SourceVectorPattern::AllZero,
        ..DerivedFixtureOptions::default()
    });
    let result = compute(&zero, EXACT_COMPONENT_OPERATIONS).expect("zero vectors");
    assert_eq!(result.eligible_relation_count(), 3);
    assert_eq!(
        result
            .mean_squared_euclidean_distance()
            .expect("available")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn region_dispersion_without_contributors_is_typed_unavailable() {
    let fixture = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 0,
        ..DerivedFixtureOptions::default()
    });
    let result = compute(&fixture, 0).expect("typed unavailable");

    assert_eq!(
        result.status(),
        PatchRegionEmbeddingDispersionStatus::InsufficientContributors
    );
    assert_eq!(result.total_relation_count(), 0);
    assert_eq!(result.eligible_relation_count(), 0);
    assert_eq!(result.excluded_relation_count(), 0);
    assert_eq!(result.mean_squared_euclidean_distance(), None);
}

#[test]
fn region_dispersion_rejects_all_binding_drift_before_the_work_budget() {
    let fixture = derived_fixture();
    let graph = validate_graph(&fixture);
    let candidate = finalize_candidate(&fixture, graph);
    let different_source = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 4,
        ..DerivedFixtureOptions::default()
    });
    let different_regions = derived_fixture_with_options(DerivedFixtureOptions {
        region_count: 1,
        ..DerivedFixtureOptions::default()
    });

    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &candidate,
            graph,
            &fixture.provenance,
            &fixture.provenance,
            0,
        ),
        Err(
            PatchRegionEmbeddingDispersionError::UnsupportedSourceProvenanceVariant {
                observed: marklab::MultiscaleEmbeddingProvenanceVariant::DerivedRegion,
            }
        )
    );
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &candidate,
            graph,
            &fixture._direct.provenance,
            &fixture._direct.provenance,
            0,
        ),
        Err(
            PatchRegionEmbeddingDispersionError::UnsupportedRegionProvenanceVariant {
                observed: marklab::MultiscaleEmbeddingProvenanceVariant::DirectPatch,
            }
        )
    );

    assert_eq!(
        patch_region_embedding_dispersion(
            &different_source.source_patch_table_value,
            &fixture.link,
            &candidate,
            graph,
            &fixture._direct.provenance,
            &fixture.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::SourceBindingMismatch)
    );
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &different_regions.link,
            &candidate,
            graph,
            &fixture._direct.provenance,
            &fixture.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::LinkBindingMismatch)
    );
    let other_graph = validate_graph(&different_regions);
    let other_candidate = finalize_candidate(&different_regions, other_graph);
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &other_candidate,
            graph,
            &fixture._direct.provenance,
            &fixture.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch)
    );
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &candidate,
            validate_graph(&different_source),
            &fixture._direct.provenance,
            &fixture.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::SourceBindingMismatch)
    );
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &candidate,
            graph,
            &different_source._direct.provenance,
            &fixture.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::SourceBindingMismatch)
    );
    assert_eq!(
        patch_region_embedding_dispersion(
            &fixture.source_patch_table_value,
            &fixture.link,
            &candidate,
            graph,
            &fixture._direct.provenance,
            &different_regions.provenance,
            0,
        ),
        Err(PatchRegionEmbeddingDispersionError::RegionCandidateBindingMismatch)
    );
}

#[test]
fn region_dispersion_enforces_the_exact_conservative_work_cap() {
    let fixture = derived_fixture();
    assert_eq!(
        compute(&fixture, EXACT_COMPONENT_OPERATIONS - 1),
        Err(
            PatchRegionEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: EXACT_COMPONENT_OPERATIONS,
                maximum: EXACT_COMPONENT_OPERATIONS - 1,
            }
        )
    );
    compute(&fixture, EXACT_COMPONENT_OPERATIONS).expect("exact operation cap");
}
