use super::*;

#[test]
fn artifact_graph_requires_exact_records_dependencies_payloads_and_store_bytes() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let verified = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect("verified artifact graph");
    assert_eq!(
        verified.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(verified.dependency_count(), 13);
}

#[test]
fn artifact_graph_rejects_wrong_schema_and_catalog_only_availability() {
    let wrong_schema = build_fixture("marklab.wrong_checkpoint", LicenseAvailability::Managed);
    assert!(matches!(
        wrong_schema.provenance.validate_artifact_graph(
            wrong_schema.provenance_artifact_id,
            &wrong_schema.expected,
            &wrong_schema.identity_map,
            &wrong_schema.context,
            &wrong_schema.row_link,
            &wrong_schema.catalog,
            &wrong_schema.store,
        ),
        Err(EmbeddingArtifactGraphError::SchemaMismatch {
            role: CellEmbeddingArtifactRole::Checkpoint
        })
    ));

    let catalog_only = build_fixture("marklab.model_checkpoint", LicenseAvailability::CatalogOnly);
    let error = catalog_only
        .provenance
        .validate_artifact_graph(
            catalog_only.provenance_artifact_id,
            &catalog_only.expected,
            &catalog_only.identity_map,
            &catalog_only.context,
            &catalog_only.row_link,
            &catalog_only.catalog,
            &catalog_only.store,
        )
        .expect_err("catalog-only record must not be available");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel"));

    let non_managed = build_fixture(
        "marklab.model_checkpoint",
        LicenseAvailability::NonManagedLocal,
    );
    let error = non_managed
        .provenance
        .validate_artifact_graph(
            non_managed.provenance_artifact_id,
            &non_managed.expected,
            &non_managed.identity_map,
            &non_managed.context,
            &non_managed.row_link,
            &non_managed.catalog,
            &non_managed.store,
        )
        .expect_err("non-managed local locator must not satisfy graph verification");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel-license"));
}

#[test]
fn artifact_graph_rejects_row_link_values_from_a_different_expected_set() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let other_expected =
        ExpectedCellSet::new("other.v1", vec![cell("cell-b")]).expect("other expected set");
    let forged_link = CellEmbeddingRowLink::new(
        fixture.row_link.source_cells_artifact_id(),
        fixture.row_link.source_vectors_artifact_id(),
        fixture.row_link.expected_cells_artifact_id(),
        fixture.row_link.identity_map_artifact_id(),
        fixture.row_link.converter_artifact_id(),
        &other_expected,
        &hierarchy(&other_expected),
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-b"), 0, 0)],
        4_096,
    )
    .expect("internally valid forged link");
    assert!(matches!(
        fixture.provenance.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &forged_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::LinkageMismatch)
    ));
}

#[test]
fn artifact_graph_rejects_provenance_record_structure_before_store_access() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let cases = [
        (
            provenance_record(
                &fixture,
                "application/json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::new(),
                None,
            ),
            EmbeddingArtifactGraphError::ContentKindMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::from([("extra".to_owned(), "value".to_owned())]),
                None,
            ),
            EmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies()[1..].to_vec(),
                BTreeMap::new(),
                None,
            ),
            EmbeddingArtifactGraphError::DependencyMismatch {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            provenance_record(
                &fixture,
                "application/vnd.marklab.embedding-provenance.v1+json",
                fixture.provenance.direct_dependencies().to_vec(),
                BTreeMap::new(),
                Some(row_link_manifest(1)),
            ),
            EmbeddingArtifactGraphError::UnexpectedTableManifest {
                role: CellEmbeddingArtifactRole::Provenance,
            },
        ),
    ];
    for (record, expected_error) in cases {
        let id = record.id();
        let catalog = replace_record(&fixture.catalog, fixture.provenance_artifact_id, record);
        let error = fixture
            .provenance
            .validate_artifact_graph(
                id,
                &fixture.expected,
                &fixture.identity_map,
                &fixture.context,
                &fixture.row_link,
                &catalog,
                &fixture.store,
            )
            .expect_err("structural drift must fail");
        assert_eq!(error, expected_error);
        assert!(!error.to_string().contains("privacy-sentinel"));
    }
}

#[test]
fn artifact_graph_rejects_canonical_payload_mismatches() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let encoded = fixture
        .provenance
        .to_canonical_json()
        .expect("provenance bytes");
    let changed = String::from_utf8(encoded)
        .expect("UTF-8 provenance")
        .replace("\"model_version\":\"1.0\"", "\"model_version\":\"1.1\"");
    let changed = CellEmbeddingProvenance::from_canonical_json(changed.as_bytes())
        .expect("changed canonical provenance");
    assert!(matches!(
        changed.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::PayloadIdentityMismatch {
            role: CellEmbeddingArtifactRole::Provenance,
        })
    ));

    let changed_map =
        CellIdentityMap::new(
            fixture.identity_map.source_cells_artifact_id(),
            fixture.identity_map.expected_cells_artifact_id(),
            &fixture.expected,
            vec![CellIdentityMapEntry::new("changed-source", cell("cell-a"))
                .expect("changed identity")],
        )
        .expect("changed identity map");
    assert!(matches!(
        fixture.provenance.validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &changed_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        ),
        Err(EmbeddingArtifactGraphError::PayloadIdentityMismatch {
            role: CellEmbeddingArtifactRole::IdentityMap,
        })
    ));
}

#[test]
fn artifact_graph_maps_corrupted_managed_bytes_to_redacted_integrity() {
    let fixture = build_fixture("marklab.model_checkpoint", LicenseAvailability::Managed);
    let license = record_with_schema(&fixture, "marklab.license_record");
    std::fs::write(
        managed_path(&fixture._root, license),
        b"corrupted privacy-sentinel",
    )
    .expect("corrupt managed fixture");
    let error = fixture
        .provenance
        .validate_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected,
            &fixture.identity_map,
            &fixture.context,
            &fixture.row_link,
            &fixture.catalog,
            &fixture.store,
        )
        .expect_err("corrupted managed bytes must fail");
    assert!(matches!(
        error,
        EmbeddingArtifactGraphError::Unavailable {
            role: CellEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::Integrity,
        }
    ));
    assert!(!error.to_string().contains("privacy-sentinel"));
}
