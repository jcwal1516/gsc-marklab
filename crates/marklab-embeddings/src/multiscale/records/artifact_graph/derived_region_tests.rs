use std::{collections::BTreeMap, str::FromStr};

use marklab_project::{
    ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema,
    ContentDigest, StoreId, TableManifest,
};

use super::{require_patch_region_profile, require_patch_table_profile};
use crate::multiscale::{
    physical::{
        content_kind, table_manifest, MatrixPhysicalProfile, SpatialArtifactRole,
        SpatialPhysicalEncoding,
    },
    records::artifact_graph::{
        record::require_dependencies, MultiscaleEmbeddingArtifactGraphError,
        MultiscaleEmbeddingArtifactRole,
    },
};

const ROW_COUNT: u64 = 7;
const DIMENSION: u32 = 3;

fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn record(
    schema: &str,
    kind: &str,
    table: Option<TableManifest>,
    dependencies: Vec<ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, b"derived-physical-profile").expect("content"),
        ArtifactSchema::new(schema, 1).expect("schema"),
        table,
        dependencies,
        semantic_metadata,
        vec![ArtifactLocator::new(
            StoreId::new("derived-profile-test").expect("store ID"),
            ArtifactKey::new("fixtures/derived-profile").expect("artifact key"),
            None,
        )
        .expect("locator")],
    )
    .expect("record")
}

fn patch_record(
    encoding: SpatialPhysicalEncoding,
    row_count: u64,
    dimension: u32,
) -> ArtifactRecord {
    let profile = MatrixPhysicalProfile::Patch;
    record(
        profile.schema_id(),
        profile.content_kind(encoding),
        Some(
            profile
                .table_manifest(encoding, row_count, dimension)
                .expect("patch manifest"),
        ),
        Vec::new(),
        BTreeMap::new(),
    )
}

fn link_record(encoding: SpatialPhysicalEncoding, row_count: u64) -> ArtifactRecord {
    record(
        "marklab.patch_region_link",
        content_kind(SpatialArtifactRole::PatchRegion, encoding),
        Some(
            table_manifest(SpatialArtifactRole::PatchRegion, encoding, row_count)
                .expect("link manifest"),
        ),
        Vec::new(),
        BTreeMap::new(),
    )
}

#[test]
fn derived_physical_profiles_accept_both_exact_formats() {
    for encoding in [
        SpatialPhysicalEncoding::Arrow,
        SpatialPhysicalEncoding::Parquet,
    ] {
        assert_eq!(
            require_patch_table_profile(
                &patch_record(encoding, ROW_COUNT, DIMENSION),
                ROW_COUNT,
                DIMENSION,
            ),
            Ok(())
        );
        assert_eq!(
            require_patch_region_profile(&link_record(encoding, ROW_COUNT), ROW_COUNT),
            Ok(())
        );
    }
}

#[test]
fn source_patch_profile_rejects_each_manifest_layer() {
    let profile = MatrixPhysicalProfile::Patch;
    let arrow_manifest = profile
        .table_manifest(SpatialPhysicalEncoding::Arrow, ROW_COUNT, DIMENSION)
        .expect("Arrow patch manifest");
    let mut metadata = BTreeMap::new();
    metadata.insert("private".to_owned(), "sentinel".to_owned());
    let cases = [
        (
            record(
                "marklab.wrong_patch_embedding_table",
                profile.content_kind(SpatialPhysicalEncoding::Arrow),
                Some(arrow_manifest.clone()),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            record(
                profile.schema_id(),
                "application/octet-stream",
                Some(arrow_manifest.clone()),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            record(
                profile.schema_id(),
                profile.content_kind(SpatialPhysicalEncoding::Arrow),
                Some(arrow_manifest.clone()),
                Vec::new(),
                metadata,
            ),
            MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            record(
                profile.schema_id(),
                profile.content_kind(SpatialPhysicalEncoding::Arrow),
                None,
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            patch_record(SpatialPhysicalEncoding::Arrow, ROW_COUNT + 1, DIMENSION),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            patch_record(SpatialPhysicalEncoding::Arrow, ROW_COUNT, DIMENSION + 1),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
        (
            record(
                profile.schema_id(),
                profile.content_kind(SpatialPhysicalEncoding::Arrow),
                Some(
                    profile
                        .table_manifest(SpatialPhysicalEncoding::Parquet, ROW_COUNT, DIMENSION)
                        .expect("Parquet patch manifest"),
                ),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            },
        ),
    ];
    for (record, expected) in cases {
        assert_eq!(
            require_patch_table_profile(&record, ROW_COUNT, DIMENSION),
            Err(expected)
        );
    }
}

#[test]
fn patch_region_profile_rejects_each_manifest_layer() {
    let arrow_kind = content_kind(
        SpatialArtifactRole::PatchRegion,
        SpatialPhysicalEncoding::Arrow,
    );
    let arrow_manifest = table_manifest(
        SpatialArtifactRole::PatchRegion,
        SpatialPhysicalEncoding::Arrow,
        ROW_COUNT,
    )
    .expect("Arrow link manifest");
    let mut metadata = BTreeMap::new();
    metadata.insert("private".to_owned(), "sentinel".to_owned());
    let cases = [
        (
            record(
                "marklab.wrong_patch_region_link",
                arrow_kind,
                Some(arrow_manifest.clone()),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::SchemaMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
        (
            record(
                "marklab.patch_region_link",
                "application/octet-stream",
                Some(arrow_manifest.clone()),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::ContentKindMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
        (
            record(
                "marklab.patch_region_link",
                arrow_kind,
                Some(arrow_manifest.clone()),
                Vec::new(),
                metadata,
            ),
            MultiscaleEmbeddingArtifactGraphError::SemanticMetadataMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
        (
            record(
                "marklab.patch_region_link",
                arrow_kind,
                None,
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
        (
            link_record(SpatialPhysicalEncoding::Arrow, ROW_COUNT + 1),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
        (
            record(
                "marklab.patch_region_link",
                arrow_kind,
                Some(
                    table_manifest(
                        SpatialArtifactRole::PatchRegion,
                        SpatialPhysicalEncoding::Parquet,
                        ROW_COUNT,
                    )
                    .expect("Parquet link manifest"),
                ),
                Vec::new(),
                BTreeMap::new(),
            ),
            MultiscaleEmbeddingArtifactGraphError::TableManifestMismatch {
                role: MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            },
        ),
    ];
    for (record, expected) in cases {
        assert_eq!(
            require_patch_region_profile(&record, ROW_COUNT),
            Err(expected)
        );
    }
}

#[test]
fn derived_physical_roles_require_their_exact_dependency_sets() {
    let expected = [artifact(b"expected-a"), artifact(b"expected-b")];
    let wrong = vec![artifact(b"wrong-dependency")];
    for (role, record) in [
        (
            MultiscaleEmbeddingArtifactRole::SourcePatchTable,
            record(
                MatrixPhysicalProfile::Patch.schema_id(),
                MatrixPhysicalProfile::Patch.content_kind(SpatialPhysicalEncoding::Arrow),
                Some(
                    MatrixPhysicalProfile::Patch
                        .table_manifest(SpatialPhysicalEncoding::Arrow, ROW_COUNT, DIMENSION)
                        .expect("patch manifest"),
                ),
                wrong.clone(),
                BTreeMap::new(),
            ),
        ),
        (
            MultiscaleEmbeddingArtifactRole::PatchRegionLink,
            record(
                "marklab.patch_region_link",
                content_kind(
                    SpatialArtifactRole::PatchRegion,
                    SpatialPhysicalEncoding::Arrow,
                ),
                Some(
                    table_manifest(
                        SpatialArtifactRole::PatchRegion,
                        SpatialPhysicalEncoding::Arrow,
                        ROW_COUNT,
                    )
                    .expect("link manifest"),
                ),
                wrong,
                BTreeMap::new(),
            ),
        ),
    ] {
        assert_eq!(
            require_dependencies(&record, role, &expected),
            Err(MultiscaleEmbeddingArtifactGraphError::DependencyMismatch { role })
        );
    }
}
