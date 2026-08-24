use super::*;

#[test]
fn derived_graph_rejects_variant_binding_and_missing_record_drift() {
    let fixture = derived_fixture();
    assert_eq!(
        validate_graph_result(
            &fixture,
            &fixture._direct.provenance,
            fixture.provenance_artifact_id,
            &fixture.derivation,
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::UnsupportedDerivedRegionProvenanceVariant)
    );
    let arithmetic =
        MultiscaleEmbeddingDerivationContract::arithmetic_mean("fixed_order_f64.v1", BUDGET)
            .expect("arithmetic derivation");
    assert_eq!(
        validate_graph_result(
            &fixture,
            &fixture.provenance,
            fixture.provenance_artifact_id,
            &arithmetic,
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            }
        )
    );
    assert_eq!(
        validate_graph_result(
            &fixture,
            &fixture.provenance,
            fixture.provenance_artifact_id,
            &fixture.derivation,
            &ArtifactCatalog::new(),
            &fixture._direct.store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::MissingRecord {
            role: MultiscaleEmbeddingArtifactRole::Provenance,
        })
    );
}

#[test]
fn derived_graph_rejects_cross_fixture_table_and_support_receipts() {
    let fixture = derived_fixture();
    let other = derived_fixture_with_options(DerivedFixtureOptions {
        patch_format: PhysicalFormat::Parquet,
        link_format: PhysicalFormat::Parquet,
        ..DerivedFixtureOptions::default()
    });
    assert_ne!(
        fixture.source_patch_table.artifact_id(),
        other.source_patch_table.artifact_id()
    );
    assert_eq!(
        fixture.provenance.validate_derived_region_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_regions,
            &fixture.derivation,
            other.source_patch_table,
            fixture.region_support,
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            }
        )
    );

    let other_support = derived_fixture_with_options(DerivedFixtureOptions {
        direct_parquet_physical: true,
        link_format: PhysicalFormat::Parquet,
        ..DerivedFixtureOptions::default()
    });
    assert_ne!(
        fixture.region_support.artifact_id(),
        other_support.region_support.artifact_id()
    );
    assert_eq!(
        fixture.provenance.validate_derived_region_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_regions,
            &fixture.derivation,
            fixture.source_patch_table,
            other_support.region_support,
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            }
        )
    );
}

#[test]
fn derived_graph_rejects_provenance_profile_dependency_and_payload_drift() {
    let fixture = derived_fixture();
    let bytes = fixture
        .provenance
        .to_canonical_json()
        .expect("provenance bytes");
    let dependencies: Vec<_> = fixture.provenance.direct_dependencies().collect();
    let mut metadata = BTreeMap::new();
    metadata.insert("private".to_owned(), "sentinel".to_owned());
    let cases = [
        (
            draft_record(
                "marklab.wrong_multiscale_provenance",
                "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
                &bytes,
                dependencies.clone(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_provenance",
                "application/json",
                &bytes,
                dependencies.clone(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_provenance",
                "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
                &bytes,
                dependencies.clone(),
                fixture.source_patch_table_record.table().cloned(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::UnexpectedTableManifest {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_provenance",
                "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
                &bytes,
                dependencies.clone(),
                None,
                metadata,
            ),
            MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_provenance",
                "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
                &bytes,
                Vec::new(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::DependencyMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            },
        ),
    ];
    for (record, expected) in cases {
        let catalog = catalog_with(&fixture.catalog, record.clone());
        assert_eq!(
            validate_graph_result(
                &fixture,
                &fixture.provenance,
                record.id(),
                &fixture.derivation,
                &catalog,
                &fixture._direct.store,
            ),
            Err(expected)
        );
    }

    let mut drift = bytes;
    drift[0] ^= 1;
    let record = publish_record(
        &fixture._direct.store,
        "marklab.multiscale_embedding_provenance",
        "application/vnd.marklab.multiscale-embedding-provenance.v1+json",
        &drift,
        dependencies,
        None,
    );
    let catalog = catalog_with(&fixture.catalog, record.clone());
    assert_eq!(
        validate_graph_result(
            &fixture,
            &fixture.provenance,
            record.id(),
            &fixture.derivation,
            &catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: MultiscaleEmbeddingArtifactRole::Provenance,
            }
        )
    );
}

#[test]
fn derived_graph_rejects_leaf_dependency_drift_at_each_execution_role() {
    for role in [
        MultiscaleEmbeddingArtifactRole::RunConfig,
        MultiscaleEmbeddingArtifactRole::Environment,
        MultiscaleEmbeddingArtifactRole::Converter,
    ] {
        let fixture = derived_fixture();
        let (schema, bytes) = match role {
            MultiscaleEmbeddingArtifactRole::RunConfig => {
                ("marklab.embedding_run_config", b"drift-run".as_slice())
            }
            MultiscaleEmbeddingArtifactRole::Environment => (
                "marklab.execution_environment",
                b"drift-environment".as_slice(),
            ),
            MultiscaleEmbeddingArtifactRole::Converter => {
                ("marklab.converter_manifest", b"drift-converter".as_slice())
            }
            _ => unreachable!("closed test role"),
        };
        let role_record = draft_record(
            schema,
            "application/json",
            bytes,
            vec![fixture.provenance_record.id()],
            None,
            BTreeMap::new(),
        );
        let (run_config, environment, converter) = match role {
            MultiscaleEmbeddingArtifactRole::RunConfig => (
                role_record.id(),
                fixture.environment_record.id(),
                fixture.converter_record.id(),
            ),
            MultiscaleEmbeddingArtifactRole::Environment => (
                fixture.run_config_record.id(),
                role_record.id(),
                fixture.converter_record.id(),
            ),
            MultiscaleEmbeddingArtifactRole::Converter => (
                fixture.run_config_record.id(),
                fixture.environment_record.id(),
                role_record.id(),
            ),
            _ => unreachable!("closed test role"),
        };
        let (provenance, provenance_record) = rebuild_provenance(
            &fixture,
            run_config,
            environment,
            converter,
            fixture.derivation_record.id(),
        );
        let catalog = catalog_with(
            &catalog_with(&fixture.catalog, role_record),
            provenance_record.clone(),
        );
        assert_eq!(
            validate_graph_result(
                &fixture,
                &provenance,
                provenance_record.id(),
                &fixture.derivation,
                &catalog,
                &fixture._direct.store,
            ),
            Err(MultiscaleEmbeddingArtifactGraphError::DependencyMismatch { role })
        );
    }
}

#[test]
fn derived_graph_streams_and_profiles_the_derivation_contract() {
    let fixture = derived_fixture();
    let bytes = fixture
        .derivation
        .to_canonical_json()
        .expect("derivation bytes");
    let cases = [
        (
            draft_record(
                "marklab.wrong_multiscale_derivation",
                "application/vnd.marklab.multiscale-embedding-derivation.v1+json",
                &bytes,
                Vec::new(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::Derivation,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_derivation",
                "application/vnd.marklab.multiscale-embedding-derivation.v1+json",
                &bytes,
                vec![fixture.provenance_record.id()],
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::DependencyMismatch {
                role: MultiscaleEmbeddingArtifactRole::Derivation,
            },
        ),
    ];
    for (derivation_record, expected) in cases {
        let (provenance, provenance_record) = rebuild_provenance(
            &fixture,
            fixture.run_config_record.id(),
            fixture.environment_record.id(),
            fixture.converter_record.id(),
            derivation_record.id(),
        );
        let catalog = catalog_with(
            &catalog_with(&fixture.catalog, derivation_record),
            provenance_record.clone(),
        );
        assert_eq!(
            validate_graph_result(
                &fixture,
                &provenance,
                provenance_record.id(),
                &fixture.derivation,
                &catalog,
                &fixture._direct.store,
            ),
            Err(expected)
        );
    }

    let mut drift = bytes;
    drift[0] ^= 1;
    let derivation_record = publish_record(
        &fixture._direct.store,
        "marklab.multiscale_embedding_derivation",
        "application/vnd.marklab.multiscale-embedding-derivation.v1+json",
        &drift,
        Vec::new(),
        None,
    );
    let (provenance, provenance_record) = rebuild_provenance(
        &fixture,
        fixture.run_config_record.id(),
        fixture.environment_record.id(),
        fixture.converter_record.id(),
        derivation_record.id(),
    );
    let catalog = catalog_with(
        &catalog_with(&fixture.catalog, derivation_record),
        provenance_record.clone(),
    );
    assert_eq!(
        validate_graph_result(
            &fixture,
            &provenance,
            provenance_record.id(),
            &fixture.derivation,
            &catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: MultiscaleEmbeddingArtifactRole::Derivation,
            }
        )
    );
}

#[test]
fn derived_graph_requires_managed_integrity_and_redacts_failures() {
    let fixture = derived_fixture();
    let empty_root = TempDir::new().expect("empty store root");
    let empty_store = LocalArtifactStore::open(
        empty_root.path(),
        StoreId::new("empty-derived-graph").expect("empty store ID"),
    )
    .expect("empty store");
    assert_eq!(
        validate_graph_result(
            &fixture,
            &fixture.provenance,
            fixture.provenance_artifact_id,
            &fixture.derivation,
            &fixture.catalog,
            &empty_store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::Provenance,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        })
    );

    let source_id = corrupt_managed(
        &fixture._direct._root,
        fixture.source_patch_table_record.id(),
    );
    let error = validate_graph_result(
        &fixture,
        &fixture.provenance,
        fixture.provenance_artifact_id,
        &fixture.derivation,
        &fixture.catalog,
        &fixture._direct.store,
    )
    .expect_err("corrupt source patch table must fail");
    assert_eq!(
        error,
        MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            reason: ArtifactAvailabilityFailure::Integrity,
        }
    );
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("private-derived-region"));
    assert!(!rendered.contains("objects/sha256"));
    assert!(!rendered.contains(&source_id));
    assert!(!rendered.contains(&fixture.provenance_record.id().to_string()));
}

#[test]
fn derived_graph_checks_managed_integrity_for_every_role() {
    for role in [
        MultiscaleEmbeddingArtifactRole::Provenance,
        MultiscaleEmbeddingArtifactRole::ExpectedRegions,
        MultiscaleEmbeddingArtifactRole::Derivation,
        MultiscaleEmbeddingArtifactRole::RunConfig,
        MultiscaleEmbeddingArtifactRole::Environment,
        MultiscaleEmbeddingArtifactRole::Converter,
        MultiscaleEmbeddingArtifactRole::SourcePatchTable,
        MultiscaleEmbeddingArtifactRole::PatchRegionLink,
        MultiscaleEmbeddingArtifactRole::RegionSupport,
    ] {
        let fixture = derived_fixture();
        let artifact_id = match role {
            MultiscaleEmbeddingArtifactRole::Provenance => fixture.provenance_record.id(),
            MultiscaleEmbeddingArtifactRole::ExpectedRegions => fixture.expected_region_record.id(),
            MultiscaleEmbeddingArtifactRole::Derivation => fixture.derivation_record.id(),
            MultiscaleEmbeddingArtifactRole::RunConfig => fixture.run_config_record.id(),
            MultiscaleEmbeddingArtifactRole::Environment => fixture.environment_record.id(),
            MultiscaleEmbeddingArtifactRole::Converter => fixture.converter_record.id(),
            MultiscaleEmbeddingArtifactRole::SourcePatchTable => {
                fixture.source_patch_table_record.id()
            }
            MultiscaleEmbeddingArtifactRole::PatchRegionLink => fixture.link_receipt.artifact_id(),
            MultiscaleEmbeddingArtifactRole::RegionSupport => fixture.region_support_record.id(),
            _ => unreachable!("closed test role"),
        };
        corrupt_managed(&fixture._direct._root, artifact_id);
        assert_eq!(
            validate_graph_result(
                &fixture,
                &fixture.provenance,
                fixture.provenance_artifact_id,
                &fixture.derivation,
                &fixture.catalog,
                &fixture._direct.store,
            ),
            Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
                role,
                reason: ArtifactAvailabilityFailure::Integrity,
            })
        );
    }
}
