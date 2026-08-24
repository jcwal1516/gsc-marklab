use super::*;

#[test]
fn slide_supports_and_graphs_close_both_exact_dependency_chains() {
    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture(source_level);
        assert_eq!(
            fixture.support.logical_digest(),
            fixture.support_value.logical_digest()
        );
        let graph = validate_slide_graph(&fixture);
        assert_eq!(
            graph.provenance_artifact_id(),
            fixture.provenance_record.id()
        );
        assert_eq!(graph.dependency_count(), 7);
        assert_eq!(graph.output_dimension(), 3);
        let debug = format!("{:?} {graph:?}", fixture.support);
        assert!(!debug.contains("derived-slide"));
        assert!(!debug.contains(&fixture.provenance_record.id().to_string()));
    }
}

#[test]
fn slide_supports_and_graphs_reject_cross_path_and_cross_lineage_receipts() {
    let patches = slide_fixture(SlideSourceLevel::Patches);
    let regions = slide_fixture(SlideSourceLevel::Regions);

    assert!(validate_slide_graph_result(
        &patches,
        &regions.provenance,
        regions.provenance_record.id(),
        &regions.catalog,
        &regions.lower._direct.store,
    )
    .is_err());
    assert!(validate_slide_graph_result(
        &regions,
        &patches.provenance,
        patches.provenance_record.id(),
        &patches.catalog,
        &patches.lower._direct.store,
    )
    .is_err());

    let other_patches = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Patches,
        entity_count: 3,
        ..SlideFixtureOptions::default()
    });
    assert!(patches
        .support_value
        .verify_slide_from_patches_artifact(
            patches.support_record.id(),
            &other_patches.lower.source_patch_table_value,
            patches.lower.patch_support,
            other_patches.lower.source_patch_table,
            &patches.catalog,
            &patches.lower._direct.store,
        )
        .is_err());

    let other_regions = slide_fixture_with_options(SlideFixtureOptions {
        source_level: SlideSourceLevel::Regions,
        region_count: 1,
        ..SlideFixtureOptions::default()
    });
    assert!(regions
        .support_value
        .verify_slide_from_regions_artifact(
            regions.support_record.id(),
            source_region_table(&other_regions),
            regions.lower.region_support,
            source_region_receipt(&other_regions),
            &regions.catalog,
            &regions.lower._direct.store,
        )
        .is_err());

    for fixture in [&patches, &regions] {
        let foreign_slide = SlideId::new("foreign-derived-slide").expect("foreign slide");
        let wrong_slide_support = match fixture.source_level {
            SlideSourceLevel::Patches => MultiscaleEmbeddingSupport::slide_from_patches(
                foreign_slide,
                MultiscaleArtifactBinding::new(
                    fixture.lower.patch_support.artifact_id(),
                    fixture.lower.patch_support.logical_digest(),
                ),
                MultiscaleArtifactBinding::new(
                    fixture.lower.source_patch_table_record.id(),
                    fixture.lower.source_patch_table_value.logical_digest(),
                ),
                BUDGET,
            ),
            SlideSourceLevel::Regions => MultiscaleEmbeddingSupport::slide_from_regions(
                foreign_slide,
                MultiscaleArtifactBinding::new(
                    fixture.lower.region_support.artifact_id(),
                    fixture.lower.region_support.logical_digest(),
                ),
                MultiscaleArtifactBinding::new(
                    source_table_record(fixture).id(),
                    source_region_candidate(fixture).logical_digest(),
                ),
                BUDGET,
            ),
        }
        .expect("foreign-slide support value");
        assert_eq!(
            verify_slide_support_result(
                fixture,
                &wrong_slide_support,
                fixture.support_record.id(),
                &fixture.catalog,
                &fixture.lower._direct.store,
            ),
            Err(
                MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                    role: MultiscaleEmbeddingArtifactRole::SlideSupport,
                }
            )
        );
    }
}

#[test]
fn derived_slide_graph_rejects_decoded_provenance_for_a_foreign_slide() {
    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture(source_level);
        let foreign_slide = SlideId::new("foreign-derived-slide").expect("foreign slide");
        let foreign_expected = expected_slides_with_rule(&foreign_slide, "derived_slide.v1");
        let foreign_expected_bytes = foreign_expected
            .to_canonical_json()
            .expect("foreign expected-slide bytes");
        let foreign_expected_record = publish_record(
            &fixture.lower._direct.store,
            "marklab.expected_slide_set",
            "application/vnd.marklab.expected-slide-set.v1+json",
            &foreign_expected_bytes,
            Vec::new(),
            None,
        );

        let original_slide = fixture.expected_slides.owning_slide_id().as_str();
        let original_expected_id = fixture.expected_slide_record.id().to_string();
        let foreign_expected_id = foreign_expected_record.id().to_string();
        let provenance_json = String::from_utf8(
            fixture
                .provenance
                .to_canonical_json()
                .expect("provenance bytes"),
        )
        .expect("canonical provenance is UTF-8")
        .replace(original_slide, foreign_slide.as_str())
        .replace(&original_expected_id, &foreign_expected_id);
        let foreign_provenance = MultiscaleEmbeddingProvenance::from_canonical_json(
            provenance_json.as_bytes(),
            BUDGET,
            BUDGET,
            BUDGET,
        )
        .expect("decoded foreign-slide provenance");
        let foreign_provenance_record = publish_record(
            &fixture.lower._direct.store,
            "marklab.multiscale_embedding_provenance",
            "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
            provenance_json.as_bytes(),
            foreign_provenance.direct_dependencies().collect(),
            None,
        );
        let catalog = catalog_with(
            &catalog_with(&fixture.catalog, foreign_expected_record),
            foreign_provenance_record.clone(),
        );

        let result = match source_level {
            SlideSourceLevel::Patches => foreign_provenance
                .validate_derived_slide_from_patches_artifact_graph(
                    foreign_provenance_record.id(),
                    &foreign_expected,
                    &fixture.derivation,
                    fixture.support,
                    &catalog,
                    &fixture.lower._direct.store,
                ),
            SlideSourceLevel::Regions => foreign_provenance
                .validate_derived_slide_from_regions_artifact_graph(
                    foreign_provenance_record.id(),
                    &foreign_expected,
                    &fixture.derivation,
                    fixture.support,
                    &catalog,
                    &fixture.lower._direct.store,
                ),
        };
        assert_eq!(
            result,
            Err(
                MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                    role: MultiscaleEmbeddingArtifactRole::Provenance,
                }
            )
        );
    }
}

#[test]
fn slide_supports_and_graphs_reject_direct_record_and_managed_drift() {
    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture(source_level);
        assert_eq!(
            validate_slide_graph_result(
                &fixture,
                &fixture.provenance,
                fixture.provenance_record.id(),
                &ArtifactCatalog::new(),
                &fixture.lower._direct.store,
            ),
            Err(MultiscaleEmbeddingArtifactGraphError::MissingRecord {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            })
        );

        let mut drift = fixture
            .provenance
            .to_canonical_json()
            .expect("provenance bytes");
        drift[0] ^= 1;
        let drift_record = publish_record(
            &fixture.lower._direct.store,
            "marklab.multiscale_embedding_provenance",
            "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
            &drift,
            fixture.provenance.direct_dependencies().collect(),
            None,
        );
        let catalog = catalog_with(&fixture.catalog, drift_record.clone());
        assert_eq!(
            validate_slide_graph_result(
                &fixture,
                &fixture.provenance,
                drift_record.id(),
                &catalog,
                &fixture.lower._direct.store,
            ),
            Err(
                MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch {
                    role: MultiscaleEmbeddingArtifactRole::Provenance,
                }
            )
        );

        let support_bytes = fixture
            .support_value
            .to_canonical_json()
            .expect("support bytes");
        let support_record = draft_record(
            "marklab.multiscale_embedding_support",
            "application/vnd.marklab.multiscale-embedding-support.v1+json",
            &support_bytes,
            Vec::new(),
            None,
            BTreeMap::new(),
        );
        let catalog = catalog_with(&fixture.catalog, support_record.clone());
        assert_eq!(
            verify_slide_support_result(
                &fixture,
                &fixture.support_value,
                support_record.id(),
                &catalog,
                &fixture.lower._direct.store,
            ),
            Err(MultiscaleEmbeddingArtifactGraphError::DependencyMismatch {
                role: MultiscaleEmbeddingArtifactRole::SlideSupport,
            })
        );
    }

    for source_level in [SlideSourceLevel::Patches, SlideSourceLevel::Regions] {
        let fixture = slide_fixture(source_level);
        let artifact_id = source_table_record(&fixture).id();
        corrupt_managed(&fixture.lower._direct._root, artifact_id);
        let error = validate_slide_graph_result(
            &fixture,
            &fixture.provenance,
            fixture.provenance_record.id(),
            &fixture.catalog,
            &fixture.lower._direct.store,
        )
        .expect_err("corrupt source table must fail");
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("objects/sha256"));
        assert!(!rendered.contains(&artifact_id.to_string()));
    }
}
