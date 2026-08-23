pub(super) use std::{collections::BTreeMap, str::FromStr};

pub(super) use marklab::{
    ArtifactCatalog, ArtifactId, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef,
    ArtifactSchema, CohortHierarchy, ContentDigest, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, EffectiveReceptiveField, ExpectedPatchSet,
    FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention, LocalArtifactStore,
    MultiscaleArtifactBinding, MultiscaleDirectPatchInputArtifacts,
    MultiscaleDirectPatchModelProvenance, MultiscaleEmbeddingExecutionProvenance,
    MultiscaleEmbeddingProvenance, MultiscaleEmbeddingSupport, PatchBoundaryPolicy,
    PatchEmbeddingContext, PatchEmbeddingInputNormalization, PatchEmbeddingSourceRowLink,
    PatchEmbeddingSourceRowLinkEntry, PatchFootprint, PatchFootprintSet, PatchId, PatchIdentityMap,
    PatchIdentityMapEntry, PatchNormalizationDecimal, PatchOverlapGraph, PatchSourceEntityEntry,
    PatchSourceEntitySet, PatientId, PositiveRational, ReplicationRole, SlideId, SpatialAxis,
    StoreId, TableColumn, TableColumnType, TableFormat, TableManifest, TableScalarType,
    TransformId, TransformMatrix,
};
pub(super) use tempfile::TempDir;

pub(super) const BUDGET: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum SourceEntityPayload {
    Exact,
    SameLengthDrift,
    Truncated,
    Suffixed,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CanonicalPayloadRole {
    Provenance,
    InputNormalization,
    SourceEntities,
    ExpectedPatches,
    IdentityMap,
    SourceRowLink,
    PatchContext,
    PatchSupport,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DependencyShape {
    Leaf,
    IdentityMap,
    SourceRowLink,
    PatchFootprints,
    PatchOverlapGraph,
    PatchSupport,
    Provenance,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PhysicalArtifactRole {
    PatchFootprints,
    PatchOverlapGraph,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum PhysicalManifestMutation {
    ContentKind,
    MissingManifest,
    Format,
    Encoding,
    Columns,
    Nullability,
    PrimaryKey,
    RowCount,
}

#[derive(Clone, Copy)]
pub(super) struct FixtureOptions {
    pub(super) checkpoint_schema: &'static str,
    pub(super) checkpoint_schema_version: u32,
    pub(super) checkpoint_digest_drift: bool,
    pub(super) source_entity_kind: &'static str,
    pub(super) source_entity_payload: SourceEntityPayload,
    pub(super) canonical_payload_drift: Option<CanonicalPayloadRole>,
    pub(super) source_entity_table_manifest: bool,
    pub(super) source_entity_semantic_metadata: bool,
    pub(super) physical_manifest_mutation: Option<(PhysicalArtifactRole, PhysicalManifestMutation)>,
    pub(super) dependency_drift: Option<DependencyShape>,
    pub(super) license_catalog_only: bool,
    pub(super) source_entity_catalog_only: bool,
    pub(super) parquet_physical: bool,
    pub(super) entity_count: usize,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            checkpoint_schema: "marklab.model_checkpoint",
            checkpoint_schema_version: 1,
            checkpoint_digest_drift: false,
            source_entity_kind: "application/vnd.marklab.patch-source-entity-set.v1+json",
            source_entity_payload: SourceEntityPayload::Exact,
            canonical_payload_drift: None,
            source_entity_table_manifest: false,
            source_entity_semantic_metadata: false,
            physical_manifest_mutation: None,
            dependency_drift: None,
            license_catalog_only: false,
            source_entity_catalog_only: false,
            parquet_physical: false,
            entity_count: 2,
        }
    }
}

pub(super) struct Fixture {
    pub(super) _root: TempDir,
    pub(super) store: LocalArtifactStore,
    pub(super) catalog: ArtifactCatalog,
    pub(super) provenance: MultiscaleEmbeddingProvenance,
    pub(super) provenance_artifact_id: ArtifactId,
    pub(super) expected_patches: ExpectedPatchSet,
    pub(super) source_entities: PatchSourceEntitySet,
    pub(super) identity_map: PatchIdentityMap,
    pub(super) source_row_link: PatchEmbeddingSourceRowLink,
    pub(super) input_normalization: PatchEmbeddingInputNormalization,
    pub(super) context: PatchEmbeddingContext,
    pub(super) footprints: PatchFootprintSet,
    pub(super) overlap: PatchOverlapGraph,
    pub(super) support: MultiscaleEmbeddingSupport,
}

pub(super) fn patch(value: &str) -> PatchId {
    PatchId::new(value).expect("patch ID")
}

fn patch_at(index: usize) -> PatchId {
    patch(&format!("patch-{index:06}"))
}

fn source_at(index: usize) -> String {
    format!("private-source-{index:06}")
}

pub(super) fn artifact(label: &[u8]) -> ArtifactId {
    ArtifactId::from_str(&ContentDigest::from_bytes(label).to_string()).expect("artifact ID")
}

fn drift_canonical_payload(
    bytes: &mut [u8],
    selected: Option<CanonicalPayloadRole>,
    role: CanonicalPayloadRole,
) {
    if selected == Some(role) {
        bytes[0] ^= 1;
    }
}

fn physical_mutation(
    options: &FixtureOptions,
    role: PhysicalArtifactRole,
) -> Option<PhysicalManifestMutation> {
    options
        .physical_manifest_mutation
        .and_then(|(selected, mutation)| (selected == role).then_some(mutation))
}

mod domain;
mod fixture;
mod record;

pub(super) use domain::{context, footprint_manifest, hierarchy, overlap_manifest};
pub(super) use fixture::{fixture, fixture_with_options};
pub(super) use record::{draft_record, draft_record_with_version, publish_record};
