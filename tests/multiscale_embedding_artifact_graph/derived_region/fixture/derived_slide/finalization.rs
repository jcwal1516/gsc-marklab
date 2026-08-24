use super::*;

fn vector_bits(candidate: &DerivedSlideEmbeddingTableCandidate) -> Vec<u32> {
    candidate
        .table()
        .row(0)
        .expect("slide row")
        .vector()
        .expect("present slide vector")
        .iter()
        .map(|value| value.to_bits())
        .collect()
}

#[test]
fn slide_finalizers_compute_distinct_fixed_order_means_for_both_source_levels() {
    let patches = slide_fixture(SlideSourceLevel::Patches);
    let patch_candidate = finalize_slide(&patches);
    assert_eq!(
        vector_bits(&patch_candidate),
        vec![0x3fc00000, 0x40200000, 0x40600000]
    );

    let regions = slide_fixture(SlideSourceLevel::Regions);
    let region_candidate = finalize_slide(&regions);
    assert_eq!(
        vector_bits(&region_candidate),
        vec![0x3fd55556, 0x402aaaaa, 0x406aaaaa]
    );
    assert_ne!(
        patch_candidate.logical_digest(),
        region_candidate.logical_digest()
    );

    let sensitive = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Patches,
        entity_count: 3,
        region_count: 1,
        output_dimension: 2,
        source_vector_pattern: SourceVectorPattern::ArithmeticSensitive,
        ..SlideFixtureOptions::default()
    });
    let sensitive_candidate = finalize_slide(&sensitive);
    assert_eq!(vector_bits(&sensitive_candidate), vec![0, 0x3eaaaaab]);
    assert_eq!(sensitive_candidate.qc_summary().all_zero_present_count(), 0);
}

#[test]
fn slide_finalizers_exclude_authorized_non_present_sources_and_require_one_present() {
    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture_with_options(SlideFixtureOptions {
            source_level,
            entity_count: 4,
            source_row_status_pattern: SourceRowStatusPattern::AllStatuses,
            ..SlideFixtureOptions::default()
        });
        let candidate = finalize_slide(&fixture);
        assert_eq!(
            vector_bits(&candidate),
            vec![0x3f800000, 0x40000000, 0x40400000]
        );
        assert_eq!(candidate.qc_summary().present_count(), 1);
        assert_eq!(candidate.qc_summary().missing_vector_count(), 0);
        assert_eq!(candidate.qc_summary().extraction_failed_count(), 0);
        assert_eq!(candidate.qc_summary().qc_rejected_count(), 0);
    }

    for options in [
        SlideFixtureOptions {
            source_level: SlideSourceLevel::Patches,
            entity_count: 0,
            ..SlideFixtureOptions::default()
        },
        SlideFixtureOptions {
            source_level: SlideSourceLevel::Regions,
            region_count: 0,
            ..SlideFixtureOptions::default()
        },
    ] {
        let fixture = slide_fixture_with_options(options);
        let candidate = finalize_slide(&fixture);
        let row = candidate.table().row(0).expect("missing slide row");
        assert_eq!(row.status(), EmbeddingStatus::MissingVector);
        assert_eq!(row.vector(), None);
        assert_eq!(candidate.qc_summary().missing_vector_count(), 1);
    }
}

#[test]
fn slide_finalizers_reject_binding_drift_before_resource_limits() {
    let zero = EmbeddingFinalizationBudgets::new(0, 0, 0, 0);
    let patches = slide_fixture(SlideSourceLevel::Patches);
    let other_patches = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Patches,
        entity_count: 3,
        ..SlideFixtureOptions::default()
    });
    let graph = validate_slide_graph(&patches);
    let different_expected = expected_slides_with_rule(
        patches.expected_slides.owning_slide_id(),
        "different_slide_selection.v1",
    );
    assert_eq!(
        finalize_slide_embedding_table_from_patches(
            &different_expected,
            &patches.lower.source_patch_table_value,
            graph,
            zero,
        )
        .expect_err("mixed expected slide must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );
    assert_eq!(
        finalize_slide_embedding_table_from_patches(
            &patches.expected_slides,
            &other_patches.lower.source_patch_table_value,
            graph,
            zero,
        )
        .expect_err("mixed patch source must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );

    let regions = slide_fixture(SlideSourceLevel::Regions);
    let other_regions = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Regions,
        region_count: 1,
        ..SlideFixtureOptions::default()
    });
    let graph = validate_slide_graph(&regions);
    let different_expected = expected_slides_with_rule(
        regions.expected_slides.owning_slide_id(),
        "different_slide_selection.v1",
    );
    assert_eq!(
        finalize_slide_embedding_table_from_regions(
            &different_expected,
            source_region_table(&regions),
            graph,
            zero,
        )
        .expect_err("mixed expected slide must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );
    assert_eq!(
        finalize_slide_embedding_table_from_regions(
            &regions.expected_slides,
            source_region_table(&other_regions),
            graph,
            zero,
        )
        .expect_err("mixed region source must fail"),
        MultiscaleEmbeddingError::DerivedEmbeddingFinalizationBindingMismatch
    );
}

#[test]
fn slide_finalizers_enforce_exact_resource_edges_for_both_paths() {
    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture(source_level);
        let graph = validate_slide_graph(&fixture);
        assert_eq!(
            finalize_slide_with_budgets(
                &fixture,
                graph,
                EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, 1, BUDGET as u64),
            )
            .expect_err("contributor budget"),
            MultiscaleEmbeddingError::DerivedEmbeddingContributorBudgetExceeded {
                required: 2,
                maximum: 1,
            }
        );
        assert_eq!(
            finalize_slide_with_budgets(
                &fixture,
                graph,
                EmbeddingFinalizationBudgets::new(BUDGET, BUDGET, 2, 5),
            )
            .expect_err("component budget"),
            MultiscaleEmbeddingError::DerivedEmbeddingComponentBudgetExceeded {
                required: 6,
                maximum: 5,
            }
        );

        let retained = match finalize_slide_with_budgets(
            &fixture,
            graph,
            EmbeddingFinalizationBudgets::new(0, usize::MAX, 2, 6),
        )
        .expect_err("retained budget")
        {
            MultiscaleEmbeddingError::RetainedByteBudgetExceeded {
                required,
                maximum: 0,
            } => required,
            error => panic!("unexpected retained error: {error:?}"),
        };
        finalize_slide_with_budgets(
            &fixture,
            graph,
            EmbeddingFinalizationBudgets::new(retained, usize::MAX, 2, 6),
        )
        .expect("exact retained budget");

        let mut working = 0_usize;
        loop {
            match finalize_slide_with_budgets(
                &fixture,
                graph,
                EmbeddingFinalizationBudgets::new(retained, working, 2, 6),
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
            finalize_slide_with_budgets(
                &fixture,
                graph,
                EmbeddingFinalizationBudgets::new(retained, working - 1, 2, 6),
            ),
            Err(MultiscaleEmbeddingError::WorkingByteBudgetExceeded { required, maximum })
                if required == working && maximum == working - 1
        ));
    }
}

#[test]
fn slide_finalizers_cover_empty_and_dimension_boundaries() {
    for options in [
        SlideFixtureOptions {
            source_level: SlideSourceLevel::Patches,
            entity_count: 1,
            region_count: 1,
            output_dimension: 1,
            ..SlideFixtureOptions::default()
        },
        SlideFixtureOptions {
            source_level: SlideSourceLevel::Regions,
            entity_count: 1,
            region_count: 1,
            output_dimension: 65_536,
            ..SlideFixtureOptions::default()
        },
    ] {
        let fixture = slide_fixture_with_options(options);
        let candidate = finalize_slide_with_budgets(
            &fixture,
            validate_slide_graph(&fixture),
            EmbeddingFinalizationBudgets::new(
                BUDGET,
                BUDGET,
                1,
                u64::from(options.output_dimension),
            ),
        )
        .expect("dimension-boundary slide");
        assert_eq!(candidate.row_count(), 1);
        assert_eq!(candidate.dimension(), options.output_dimension);
    }
}
