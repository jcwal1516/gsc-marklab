use super::*;

fn finalization_budgets() -> EmbeddingFinalizationBudgets {
    EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, BUDGET as u64, BUDGET as u64)
}

#[test]
fn weighted_region_finalization_recomputes_declared_fractions_in_canonical_order() {
    let fixture = derived_fixture();
    let graph = validate_graph(&fixture);
    let candidate = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        finalization_budgets(),
    )
    .expect("derived region candidate");

    assert_eq!(candidate.row_count(), 2);
    assert_eq!(candidate.dimension(), 3);
    let first = candidate.table().row(0).expect("first region");
    assert_eq!(first.status(), EmbeddingStatus::Present);
    assert_eq!(
        first
            .vector()
            .expect("first vector")
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        vec![0x3faaaaab, 0x40155555, 0x40555555]
    );
    let second = candidate.table().row(1).expect("second region");
    assert_eq!(second.status(), EmbeddingStatus::Present);
    assert_eq!(second.vector(), Some([2.0, 3.0, 4.0].as_slice()));
}

#[test]
fn region_finalization_freezes_patch_order_and_positive_zero_output() {
    let fixture = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 3,
        region_count: 1,
        output_dimension: 1,
        source_vector_pattern: SourceVectorPattern::CancellationSensitive,
        ..DerivedFixtureOptions::default()
    });
    let candidate = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        validate_graph(&fixture),
        finalization_budgets(),
    )
    .expect("order-sensitive finalization");

    let vector = candidate
        .table()
        .row(0)
        .expect("region")
        .vector()
        .expect("present vector");
    assert_eq!(vector[0].to_bits(), 0);
    assert_eq!(candidate.qc_summary().all_zero_present_count(), 1);
}

#[test]
fn region_finalization_excludes_every_non_present_source_status() {
    let fixture = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 4,
        source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
        ..DerivedFixtureOptions::default()
    });
    let candidate = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        validate_graph(&fixture),
        finalization_budgets(),
    )
    .expect("status-aware finalization");
    let first = candidate.table().row(0).expect("first region");
    assert_eq!(first.status(), EmbeddingStatus::Present);
    assert_eq!(first.vector(), Some([1.0, 2.0, 3.0].as_slice()));
    let second = candidate.table().row(1).expect("second region");
    assert_eq!(second.status(), EmbeddingStatus::MissingVector);
    assert_eq!(second.vector(), None);
    assert_eq!(candidate.qc_summary().present_count(), 1);
    assert_eq!(candidate.qc_summary().missing_vector_count(), 1);
    assert_eq!(candidate.qc_summary().extraction_failed_count(), 0);
    assert_eq!(candidate.qc_summary().qc_rejected_count(), 0);
}

#[test]
fn region_finalization_covers_empty_sources_regions_and_dimension_boundaries() {
    let empty_source = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 0,
        ..DerivedFixtureOptions::default()
    });
    let candidate = finalize_region_embedding_table_from_patches(
        &empty_source.expected_regions,
        &empty_source.source_patch_table_value,
        &empty_source.link,
        validate_graph(&empty_source),
        finalization_budgets(),
    )
    .expect("empty-source finalization");
    assert_eq!(candidate.qc_summary().missing_vector_count(), 2);

    let empty_regions = derived_fixture_with_options(DerivedFixtureOptions {
        region_count: 0,
        ..DerivedFixtureOptions::default()
    });
    let candidate = finalize_region_embedding_table_from_patches(
        &empty_regions.expected_regions,
        &empty_regions.source_patch_table_value,
        &empty_regions.link,
        validate_graph(&empty_regions),
        finalization_budgets(),
    )
    .expect("empty-region finalization");
    assert_eq!(candidate.row_count(), 0);

    for output_dimension in [1, 65_536] {
        let fixture = derived_fixture_with_options(DerivedFixtureOptions {
            entity_count: 1,
            region_count: 1,
            output_dimension,
            ..DerivedFixtureOptions::default()
        });
        let candidate = finalize_region_embedding_table_from_patches(
            &fixture.expected_regions,
            &fixture.source_patch_table_value,
            &fixture.link,
            validate_graph(&fixture),
            EmbeddingFinalizationBudgets::new(
                BUDGET,
                BUDGET,
                BUDGET as u64,
                u64::from(output_dimension),
            ),
        )
        .expect("dimension-boundary finalization");
        assert_eq!(candidate.dimension(), output_dimension);
        assert_eq!(
            candidate.table().row(0).expect("region").status(),
            EmbeddingStatus::Present
        );
    }
}

#[test]
fn region_finalization_rejects_mixed_graph_inputs_before_resource_limits() {
    let fixture = derived_fixture();
    let different_source = derived_fixture_with_options(DerivedFixtureOptions {
        entity_count: 4,
        ..DerivedFixtureOptions::default()
    });
    let zero = EmbeddingFinalizationBudgets::new(0, 0, 0, 0);
    assert_eq!(
        finalize_region_embedding_table_from_patches(
            &fixture.expected_regions,
            &different_source.source_patch_table_value,
            &fixture.link,
            validate_graph(&fixture),
            zero,
        )
        .expect_err("mixed source must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );

    let different_link = derived_fixture_with_options(DerivedFixtureOptions {
        direct_parquet_physical: true,
        ..DerivedFixtureOptions::default()
    });
    assert_eq!(
        finalize_region_embedding_table_from_patches(
            &fixture.expected_regions,
            &fixture.source_patch_table_value,
            &different_link.link,
            validate_graph(&fixture),
            zero,
        )
        .expect_err("mixed link must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );

    let different_regions = derived_fixture_with_options(DerivedFixtureOptions {
        region_count: 1,
        ..DerivedFixtureOptions::default()
    });
    assert_eq!(
        finalize_region_embedding_table_from_patches(
            &different_regions.expected_regions,
            &fixture.source_patch_table_value,
            &fixture.link,
            validate_graph(&fixture),
            zero,
        )
        .expect_err("mixed expected regions must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );
}

#[test]
fn region_finalization_enforces_work_and_memory_budgets_at_exact_edges() {
    let fixture = derived_fixture();
    let graph = validate_graph(&fixture);
    let error = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, 2, BUDGET as u64),
    )
    .expect_err("contributor budget");
    assert_eq!(
        error,
        MultiscaleEmbeddingError::DerivedEmbeddingContributorBudgetExceeded {
            required: 3,
            maximum: 2,
        }
    );
    let error = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, 3, 8),
    )
    .expect_err("component budget");
    assert_eq!(
        error,
        MultiscaleEmbeddingError::DerivedEmbeddingComponentBudgetExceeded {
            required: 9,
            maximum: 8,
        }
    );

    let retained = match finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        EmbeddingFinalizationBudgets::new(0, usize::MAX, 3, 9),
    )
    .expect_err("retained budget")
    {
        MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
            required,
            maximum: 0,
        } => required,
        error => panic!("unexpected retained error: {error:?}"),
    };
    finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        graph,
        EmbeddingFinalizationBudgets::new(retained, usize::MAX, 3, 9),
    )
    .expect("exact retained budget");

    let mut working = 0_usize;
    loop {
        match finalize_region_embedding_table_from_patches(
            &fixture.expected_regions,
            &fixture.source_patch_table_value,
            &fixture.link,
            graph,
            EmbeddingFinalizationBudgets::new(retained, working, 3, 9),
        ) {
            Ok(_) => break,
            Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum }) => {
                assert_eq!(maximum, working);
                assert!(required > working);
                working = required;
            }
            Err(error) => panic!("unexpected working error: {error:?}"),
        }
    }
    assert!(working > 0);
    assert!(matches!(
        finalize_region_embedding_table_from_patches(
            &fixture.expected_regions,
            &fixture.source_patch_table_value,
            &fixture.link,
            graph,
            EmbeddingFinalizationBudgets::new(retained, working - 1, 3, 9),
        ),
        Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
            if required == working && maximum == working - 1
    ));
}

#[test]
fn region_candidate_debug_is_aggregate_only() {
    let fixture = derived_fixture();
    let candidate = finalize_region_embedding_table_from_patches(
        &fixture.expected_regions,
        &fixture.source_patch_table_value,
        &fixture.link,
        validate_graph(&fixture),
        finalization_budgets(),
    )
    .expect("candidate");
    let debug = format!("{candidate:?}");
    assert!(!debug.contains(&fixture.provenance_artifact_id.to_string()));
    assert!(!debug.contains("derived-region-"));
    assert_eq!(
        candidate.logical_digest(),
        candidate.table().logical_digest()
    );
}
