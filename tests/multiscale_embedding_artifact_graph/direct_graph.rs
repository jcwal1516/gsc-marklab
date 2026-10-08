use super::support::*;
use marklab::{
    ArtifactAvailabilityFailure, ArtifactCatalog, MultiscaleArtifactBinding,
    MultiscaleEmbeddingArtifactGraphError, MultiscaleEmbeddingArtifactRole,
    MultiscaleEmbeddingDerivationContract, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, PatchSourceEntitySet,
    VerifiedDirectPatchEmbeddingArtifactGraph,
};

fn validate(fixture: &Fixture) -> VerifiedDirectPatchEmbeddingArtifactGraph {
    validate_result(fixture).expect("valid direct-patch structural graph")
}

fn validate_result(
    fixture: &Fixture,
) -> Result<VerifiedDirectPatchEmbeddingArtifactGraph, MultiscaleEmbeddingArtifactGraphError> {
    fixture.provenance.validate_direct_patch_artifact_graph(
        fixture.provenance_artifact_id,
        &fixture.expected_patches,
        &fixture.source_entities,
        &fixture.identity_map,
        &fixture.source_row_link,
        &fixture.input_normalization,
        &fixture.context,
        &fixture.footprints,
        &fixture.overlap,
        &fixture.support,
        &fixture.catalog,
        &fixture.store,
    )
}

fn corrupt_source_entity(fixture: &Fixture) -> String {
    let source_id = fixture
        .identity_map
        .source_entities_artifact_id()
        .to_string();
    let managed_path = fixture
        ._root
        .path()
        .join("objects/sha256")
        .join(&source_id[..2])
        .join(&source_id);
    std::fs::write(&managed_path, b"corrupt-managed-source-entity-bytes")
        .expect("corrupt temporary managed fixture");
    source_id
}

#[test]
fn direct_patch_graph_proves_exact_structure_without_claiming_physical_decoding() {
    let fixture = fixture();
    let receipt = validate(&fixture);
    assert_eq!(
        receipt.provenance_artifact_id(),
        fixture.provenance_artifact_id
    );
    assert_eq!(receipt.provenance_dependency_count(), 14);
    assert_eq!(receipt.output_dimension(), 1_024);

    let debug = format!("{receipt:?}");
    assert!(!debug.contains(&fixture.provenance_artifact_id.to_string()));
    assert!(!debug.contains("private-source"));
    assert_eq!(
        MultiscaleEmbeddingArtifactRole::PatchFootprints.to_string(),
        "patch footprints"
    );
}

#[test]
fn direct_patch_graph_rejects_each_structural_record_profile_layer() {
    let cases = [
        (
            FixtureOptions {
                checkpoint_schema: "marklab.wrong_checkpoint",
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::Checkpoint,
            },
        ),
        (
            FixtureOptions {
                checkpoint_schema_version: 2,
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::Checkpoint,
            },
        ),
        (
            FixtureOptions {
                source_entity_kind: "application/json",
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourceEntities,
            },
        ),
        (
            FixtureOptions {
                source_entity_table_manifest: true,
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::UnexpectedTableManifest {
                role: MultiscaleEmbeddingArtifactRole::SourceEntities,
            },
        ),
        (
            FixtureOptions {
                source_entity_semantic_metadata: true,
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourceEntities,
            },
        ),
        (
            FixtureOptions {
                checkpoint_digest_drift: true,
                ..FixtureOptions::default()
            },
            MultiscaleEmbeddingArtifactGraphError::CheckpointDigestMismatch,
        ),
    ];
    for (options, expected) in cases {
        let fixture = fixture_with_options(options);
        assert_eq!(validate_result(&fixture), Err(expected));
    }
}

#[test]
fn direct_patch_graph_rejects_every_dependency_shape() {
    for (dependency_drift, role) in [
        (
            DependencyShape::Leaf,
            MultiscaleEmbeddingArtifactRole::Preprocessing,
        ),
        (
            DependencyShape::IdentityMap,
            MultiscaleEmbeddingArtifactRole::IdentityMap,
        ),
        (
            DependencyShape::SourceRowLink,
            MultiscaleEmbeddingArtifactRole::SourceRowLink,
        ),
        (
            DependencyShape::PatchFootprints,
            MultiscaleEmbeddingArtifactRole::PatchFootprints,
        ),
        (
            DependencyShape::PatchOverlapGraph,
            MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
        ),
        (
            DependencyShape::PatchSupport,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
        ),
        (
            DependencyShape::Provenance,
            MultiscaleEmbeddingArtifactRole::Provenance,
        ),
    ] {
        let fixture = fixture_with_options(FixtureOptions {
            dependency_drift: Some(dependency_drift),
            ..FixtureOptions::default()
        });
        assert_eq!(
            validate_result(&fixture),
            Err(MultiscaleEmbeddingArtifactGraphError::DependencyMismatch { role })
        );
    }
}

#[test]
fn direct_patch_graph_streams_canonical_payloads_and_checks_domain_bindings() {
    for (canonical_payload_drift, role) in [
        (
            CanonicalPayloadRole::Provenance,
            MultiscaleEmbeddingArtifactRole::Provenance,
        ),
        (
            CanonicalPayloadRole::InputNormalization,
            MultiscaleEmbeddingArtifactRole::InputNormalization,
        ),
        (
            CanonicalPayloadRole::SourceEntities,
            MultiscaleEmbeddingArtifactRole::SourceEntities,
        ),
        (
            CanonicalPayloadRole::ExpectedPatches,
            MultiscaleEmbeddingArtifactRole::ExpectedPatches,
        ),
        (
            CanonicalPayloadRole::IdentityMap,
            MultiscaleEmbeddingArtifactRole::IdentityMap,
        ),
        (
            CanonicalPayloadRole::SourceRowLink,
            MultiscaleEmbeddingArtifactRole::SourceRowLink,
        ),
        (
            CanonicalPayloadRole::PatchContext,
            MultiscaleEmbeddingArtifactRole::PatchContext,
        ),
        (
            CanonicalPayloadRole::PatchSupport,
            MultiscaleEmbeddingArtifactRole::PatchSupport,
        ),
    ] {
        let payload_drift = fixture_with_options(FixtureOptions {
            canonical_payload_drift: Some(canonical_payload_drift),
            ..FixtureOptions::default()
        });
        assert_eq!(
            validate_result(&payload_drift),
            Err(MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch { role })
        );
    }

    for source_entity_payload in [
        SourceEntityPayload::Truncated,
        SourceEntityPayload::Suffixed,
    ] {
        let payload_drift = fixture_with_options(FixtureOptions {
            source_entity_payload,
            ..FixtureOptions::default()
        });
        assert_eq!(
            validate_result(&payload_drift),
            Err(
                MultiscaleEmbeddingArtifactGraphError::PayloadIdentityMismatch {
                    role: MultiscaleEmbeddingArtifactRole::SourceEntities,
                }
            )
        );
    }

    let fixture = fixture();
    let different_source = PatchSourceEntitySet::new(
        "different_profile.v1",
        fixture.source_entities.entries().to_vec(),
        BUDGET,
    )
    .expect("different source entities");
    let error = fixture
        .provenance
        .validate_direct_patch_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_patches,
            &different_source,
            &fixture.identity_map,
            &fixture.source_row_link,
            &fixture.input_normalization,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            &fixture.support,
            &fixture.catalog,
            &fixture.store,
        )
        .expect_err("logical binding drift must fail");
    assert_eq!(
        error,
        MultiscaleEmbeddingArtifactGraphError::DomainBindingMismatch {
            role: MultiscaleEmbeddingArtifactRole::IdentityMap,
        }
    );
}

#[test]
fn managed_integrity_precedes_payload_mismatch_and_errors_remain_private() {
    let fixture = fixture_with_options(FixtureOptions {
        source_entity_payload: SourceEntityPayload::SameLengthDrift,
        ..FixtureOptions::default()
    });
    let source_id = corrupt_source_entity(&fixture);
    let error = validate_result(&fixture).expect_err("managed corruption must fail");
    assert_eq!(
        error,
        MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::SourceEntities,
            reason: ArtifactAvailabilityFailure::Integrity,
        }
    );
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("private-source"));
    assert!(!rendered.contains("objects/sha256"));
    assert!(!rendered.contains(&source_id));
}

#[test]
fn managed_availability_precedes_truncated_and_suffixed_payload_mismatches() {
    let catalog_only = fixture_with_options(FixtureOptions {
        source_entity_payload: SourceEntityPayload::Truncated,
        source_entity_catalog_only: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        validate_result(&catalog_only),
        Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::SourceEntities,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        })
    );

    for source_entity_payload in [
        SourceEntityPayload::Truncated,
        SourceEntityPayload::Suffixed,
    ] {
        let fixture = fixture_with_options(FixtureOptions {
            source_entity_payload,
            ..FixtureOptions::default()
        });
        corrupt_source_entity(&fixture);
        assert_eq!(
            validate_result(&fixture),
            Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
                role: MultiscaleEmbeddingArtifactRole::SourceEntities,
                reason: ArtifactAvailabilityFailure::Integrity,
            })
        );
    }
}

#[test]
fn direct_patch_graph_accepts_both_structural_physical_manifest_families() {
    let fixture = fixture_with_options(FixtureOptions {
        parquet_physical: true,
        ..FixtureOptions::default()
    });
    validate(&fixture);
}

#[test]
fn direct_patch_graph_rejects_every_physical_manifest_field_in_both_formats() {
    for parquet_physical in [false, true] {
        for (physical_role, role) in [
            (
                PhysicalArtifactRole::PatchFootprints,
                MultiscaleEmbeddingArtifactRole::PatchFootprints,
            ),
            (
                PhysicalArtifactRole::PatchOverlapGraph,
                MultiscaleEmbeddingArtifactRole::PatchOverlapGraph,
            ),
        ] {
            for mutation in [
                PhysicalManifestMutation::ContentKind,
                PhysicalManifestMutation::MissingManifest,
                PhysicalManifestMutation::Format,
                PhysicalManifestMutation::Encoding,
                PhysicalManifestMutation::Columns,
                PhysicalManifestMutation::Nullability,
                PhysicalManifestMutation::PrimaryKey,
                PhysicalManifestMutation::RowCount,
            ] {
                let fixture = fixture_with_options(FixtureOptions {
                    physical_manifest_mutation: Some((physical_role, mutation)),
                    parquet_physical,
                    ..FixtureOptions::default()
                });
                let expected = if mutation == PhysicalManifestMutation::ContentKind {
                    MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch { role }
                } else {
                    MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch { role }
                };
                assert_eq!(validate_result(&fixture), Err(expected));
            }
        }
    }
}

#[test]
fn catalog_only_records_are_not_managed_availability_evidence() {
    let fixture = fixture_with_options(FixtureOptions {
        license_catalog_only: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        validate_result(&fixture),
        Err(MultiscaleEmbeddingArtifactGraphError::Unavailable {
            role: MultiscaleEmbeddingArtifactRole::LicenseRecord,
            reason: ArtifactAvailabilityFailure::LocatorMissing,
        })
    );
}

#[test]
fn direct_patch_entry_point_rejects_other_variants_and_missing_catalog_records() {
    let fixture = fixture();
    let empty_catalog = ArtifactCatalog::new();
    let missing = fixture
        .provenance
        .validate_direct_patch_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_patches,
            &fixture.source_entities,
            &fixture.identity_map,
            &fixture.source_row_link,
            &fixture.input_normalization,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            &fixture.support,
            &empty_catalog,
            &fixture.store,
        )
        .expect_err("empty catalog must fail");
    assert_eq!(
        missing,
        MultiscaleEmbeddingArtifactGraphError::MissingRecord {
            role: MultiscaleEmbeddingArtifactRole::Provenance,
        }
    );

    let slide = fixture.provenance.owning_slide_id().clone();
    let region_support = MultiscaleEmbeddingSupport::region_from_patches(
        slide.clone(),
        MultiscaleArtifactBinding::new(
            artifact(b"derived-patch-support"),
            ContentDigest::from_bytes(b"derived-patch-support"),
        ),
        MultiscaleArtifactBinding::new(
            artifact(b"derived-patch-region-link"),
            ContentDigest::from_bytes(b"derived-patch-region-link"),
        ),
        BUDGET,
    )
    .expect("region support");
    let derivation =
        MultiscaleEmbeddingDerivationContract::weighted_mean("fraction_order.v1", BUDGET)
            .expect("derivation");
    let execution = MultiscaleEmbeddingExecutionProvenance::new(
        artifact(b"derived-run"),
        artifact(b"derived-environment"),
        artifact(b"derived-converter"),
        "derived_converter",
        "1.0.0",
        BUDGET,
    )
    .expect("derived execution");
    let derived = MultiscaleEmbeddingProvenance::derived_region(
        slide,
        &region_support,
        &derivation,
        1_024,
        execution,
        artifact(b"derived-source-patch-table"),
        artifact(b"derived-patch-region-link-provenance"),
        artifact(b"derived-expected-regions"),
        artifact(b"derived-region-support-record"),
        artifact(b"derived-derivation-record"),
        BUDGET,
    )
    .expect("derived provenance");
    let error = derived
        .validate_direct_patch_artifact_graph(
            fixture.provenance_artifact_id,
            &fixture.expected_patches,
            &fixture.source_entities,
            &fixture.identity_map,
            &fixture.source_row_link,
            &fixture.input_normalization,
            &fixture.context,
            &fixture.footprints,
            &fixture.overlap,
            &fixture.support,
            &fixture.catalog,
            &fixture.store,
        )
        .expect_err("derived provenance must use a derived validator");
    assert_eq!(
        error,
        MultiscaleEmbeddingArtifactGraphError::UnsupportedProvenanceVariant
    );
}
