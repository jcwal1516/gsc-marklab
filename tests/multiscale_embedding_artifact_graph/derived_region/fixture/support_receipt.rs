use super::*;

#[test]
fn region_support_receipt_rejects_variant_binding_alias_and_missing_record_drift() {
    let fixture = derived_fixture();
    assert_eq!(
        validate_region_support_result(
            &fixture,
            &fixture._direct.support,
            fixture.region_support_record.id(),
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            }
        )
    );

    let wrong_link = MultiscaleEmbeddingSupport::region_from_patches(
        fixture._direct.expected_patches.owning_slide_id().clone(),
        MultiscaleArtifactBinding::new(
            fixture.patch_support.artifact_id(),
            fixture.patch_support.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            fixture.link_receipt.artifact_id(),
            ContentDigest::from_bytes(b"wrong-link-logical-digest"),
        ),
        BUDGET,
    )
    .expect("wrong-link support");
    assert_eq!(
        validate_region_support_result(
            &fixture,
            &wrong_link,
            fixture.region_support_record.id(),
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            }
        )
    );

    assert_eq!(
        validate_region_support_result(
            &fixture,
            &fixture.region_support_value,
            fixture.patch_support.artifact_id(),
            &fixture.catalog,
            &fixture._direct.store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::RoleAlias)
    );
    assert_eq!(
        validate_region_support_result(
            &fixture,
            &fixture.region_support_value,
            fixture.region_support_record.id(),
            &ArtifactCatalog::new(),
            &fixture._direct.store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::MissingRecord {
            role: MultiscaleEmbeddingArtifactRole::RegionSupport,
        })
    );
}

#[test]
fn region_support_receipt_rejects_individually_valid_cross_lineage_receipts() {
    let patch_chain = derived_fixture();
    let link_chain = derived_fixture_with_options(DerivedFixtureOptions {
        direct_parquet_physical: true,
        link_format: PhysicalFormat::Parquet,
        ..DerivedFixtureOptions::default()
    });
    assert_eq!(
        patch_chain._direct.expected_patches.logical_digest(),
        link_chain._direct.expected_patches.logical_digest()
    );
    assert_ne!(
        patch_chain._direct.footprint_record.id(),
        link_chain._direct.footprint_record.id()
    );
    let support = MultiscaleEmbeddingSupport::region_from_patches(
        patch_chain
            ._direct
            .expected_patches
            .owning_slide_id()
            .clone(),
        MultiscaleArtifactBinding::new(
            patch_chain.patch_support.artifact_id(),
            patch_chain.patch_support.logical_digest(),
        ),
        MultiscaleArtifactBinding::new(
            link_chain.link_receipt.artifact_id(),
            link_chain.link_receipt.logical_digest(),
        ),
        BUDGET,
    )
    .expect("mixed support declaration");
    assert_eq!(
        support.verify_region_from_patches_artifact(
            artifact(b"mixed-region-support"),
            patch_chain.patch_support,
            &link_chain.link,
            link_chain.link_receipt,
            &patch_chain.catalog,
            &patch_chain._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            }
        )
    );
}

#[test]
fn region_support_receipt_rejects_every_record_profile_and_dependency_layer() {
    let fixture = derived_fixture();
    let bytes = fixture
        .region_support_value
        .to_canonical_json()
        .expect("region-support bytes");
    let dependencies: Vec<_> = fixture.region_support_value.direct_dependencies().collect();
    let mut metadata = BTreeMap::new();
    metadata.insert("private".to_owned(), "sentinel".to_owned());
    let cases = [
        (
            draft_record(
                "marklab.wrong_region_support",
                "application/vnd.marklab.multiscale-embedding-support.v1+json",
                &bytes,
                dependencies.clone(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_support",
                "application/json",
                &bytes,
                dependencies.clone(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_support",
                "application/vnd.marklab.multiscale-embedding-support.v1+json",
                &bytes,
                dependencies.clone(),
                fixture.source_patch_table_record.table().cloned(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::UnexpectedTableManifest {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_support",
                "application/vnd.marklab.multiscale-embedding-support.v1+json",
                &bytes,
                dependencies.clone(),
                None,
                metadata,
            ),
            MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        ),
        (
            draft_record(
                "marklab.multiscale_embedding_support",
                "application/vnd.marklab.multiscale-embedding-support.v1+json",
                &bytes,
                Vec::new(),
                None,
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::DependencyMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            },
        ),
    ];
    for (record, expected) in cases {
        let catalog = catalog_with(&fixture.catalog, record.clone());
        assert_eq!(
            validate_region_support_result(
                &fixture,
                &fixture.region_support_value,
                record.id(),
                &catalog,
                &fixture._direct.store,
            ),
            Err(expected)
        );
    }
}

#[test]
fn region_support_receipt_streams_payload_and_requires_managed_integrity() {
    let fixture = derived_fixture();
    let mut drift = fixture
        .region_support_value
        .to_canonical_json()
        .expect("region-support bytes");
    drift[0] ^= 1;
    let drift_record = publish_record(
        &fixture._direct.store,
        "marklab.multiscale_embedding_support",
        "application/vnd.marklab.multiscale-embedding-support.v1+json",
        &drift,
        fixture.region_support_value.direct_dependencies().collect(),
        None,
    );
    let catalog = catalog_with(&fixture.catalog, drift_record.clone());
    assert_eq!(
        validate_region_support_result(
            &fixture,
            &fixture.region_support_value,
            drift_record.id(),
            &catalog,
            &fixture._direct.store,
        ),
        Err(
            MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch {
                role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            }
        )
    );

    let empty_root = TempDir::new().expect("empty store root");
    let empty_store = LocalArtifactStore::open(
        empty_root.path(),
        StoreId::new("empty-derived").expect("empty store ID"),
    )
    .expect("empty store");
    assert_eq!(
        validate_region_support_result(
            &fixture,
            &fixture.region_support_value,
            fixture.region_support_record.id(),
            &fixture.catalog,
            &empty_store,
        ),
        Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        })
    );

    let artifact_id = corrupt_managed(&fixture._direct._root, fixture.region_support_record.id());
    let error = validate_region_support_result(
        &fixture,
        &fixture.region_support_value,
        fixture.region_support_record.id(),
        &fixture.catalog,
        &fixture._direct.store,
    )
    .expect_err("corrupt support must fail");
    assert_eq!(
        error,
        MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::RegionSupport,
            reason: ArtifactAvailabilityFailure::Integrity,
        }
    );
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("private-derived-region"));
    assert!(!rendered.contains("objects/sha256"));
    assert!(!rendered.contains(&artifact_id));
}

#[test]
fn region_support_receipt_requires_both_lower_receipts_to_remain_managed() {
    for role in [
        MultiscaleEmbeddingArtifactRole::PatchSupport,
        MultiscaleEmbeddingArtifactRole::PatchRegionLink,
    ] {
        let fixture = derived_fixture();
        let artifact_id = match role {
            MultiscaleEmbeddingArtifactRole::PatchSupport => fixture.patch_support.artifact_id(),
            MultiscaleEmbeddingArtifactRole::PatchRegionLink => fixture.link_receipt.artifact_id(),
            _ => unreachable!("closed test role"),
        };
        corrupt_managed(&fixture._direct._root, artifact_id);
        assert_eq!(
            validate_region_support_result(
                &fixture,
                &fixture.region_support_value,
                fixture.region_support_record.id(),
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
