#![cfg(feature = "parquet")]

use marklab::{
    publish_patch_footprint_set_arrow, publish_patch_footprint_set_parquet,
    publish_patch_region_link_arrow, publish_patch_region_link_parquet,
    validate_patch_region_link_arrow_bytes, validate_patch_region_link_arrow_from_store,
    validate_patch_region_link_parquet_bytes, validate_patch_region_link_parquet_from_store,
    verify_patch_region_link_arrow_from_store, verify_patch_region_link_parquet_from_store,
    write_patch_region_link_arrow, write_patch_region_link_parquet, ArtifactCatalog, ArtifactDraft,
    ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema, ArtifactStoreError,
    CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, EffectiveReceptiveField, EmbeddingColumnarBudgets, ExpectedPatchSet,
    ExpectedRegionSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    LocalArtifactStore, MultiscaleColumnarError, PatchBoundaryPolicy, PatchEmbeddingContext,
    PatchFootprint, PatchFootprintSet, PatchId, PatchRegionAssessment,
    PatchRegionAssessmentBindings, PatchRegionDeclaration, PatchRegionInputArtifactGraphError,
    PatchRegionInputArtifactRole, PatchRegionLink, PatientId, PositiveRational, RegionId,
    ReplicationRole, SlideId, SpatialAxis, StoreId, TableManifest, TransformId, TransformMatrix,
    VerifiedPatchRegionInputArtifactGraph, VerifiedReaderError,
};
use tempfile::TempDir;

const DOMAIN_BUDGET: usize = 8 * 1024 * 1024;

struct Fixture {
    _root: TempDir,
    store: LocalArtifactStore,
    catalog: ArtifactCatalog,
    assessment: PatchRegionAssessment,
    assessment_record: ArtifactRecord,
    expected_patches: ExpectedPatchSet,
    expected_regions: ExpectedRegionSet,
    context: PatchEmbeddingContext,
    footprints: PatchFootprintSet,
    link: PatchRegionLink,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    ExpectedPatches,
    ExpectedRegions,
    PatchContext,
    PatchFootprints,
    Converter,
    Assessment,
}

#[derive(Clone, Copy, Default)]
struct FixtureOptions {
    footprint_parquet: bool,
    schema_drift: Option<Role>,
    kind_drift: Option<Role>,
    dependency_drift: Option<Role>,
    payload_drift: Option<Role>,
    semantic_metadata_drift: Option<Role>,
    omit: Option<Role>,
    alternate_relation: bool,
    assessment_table_drift: bool,
    footprint_manifest_drift: bool,
}

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
) -> ArtifactRecord {
    let draft = ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("artifact content"),
        ArtifactSchema::new(schema, 1).expect("artifact schema"),
        None,
        dependencies,
        Default::default(),
        vec![ArtifactLocator::new(
            StoreId::new("patch-region-source").expect("source store"),
            ArtifactKey::new(format!("fixtures/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("artifact draft");
    store
        .publish(&draft, |writer| writer.write_all(bytes))
        .expect("publish artifact")
        .into_record()
}

fn rebuild_record(
    record: &ArtifactRecord,
    schema_version: u32,
    dependencies: Vec<marklab::ArtifactId>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        record.content().clone(),
        ArtifactSchema::new(record.schema().id(), schema_version).expect("rebuilt schema"),
        record.table().cloned(),
        dependencies,
        record.semantic_metadata().clone(),
        record.locations().to_vec(),
    )
    .expect("rebuilt record")
}

fn rebuild_profile(
    record: &ArtifactRecord,
    content_kind: &str,
    table: Option<TableManifest>,
    semantic_metadata: std::collections::BTreeMap<String, String>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::new(
            content_kind,
            record.content().digest(),
            record.content().byte_len(),
        )
        .expect("rebuilt content"),
        record.schema().clone(),
        table,
        record.dependencies().to_vec(),
        semantic_metadata,
        record.locations().to_vec(),
    )
    .expect("rebuilt profile")
}

fn apply_common_profile_drift(
    record: ArtifactRecord,
    role: Role,
    options: FixtureOptions,
) -> ArtifactRecord {
    let kind = if options.kind_drift == Some(role) {
        "application/vnd.marklab.profile-drift.v1"
    } else {
        record.content().kind()
    };
    let metadata = if options.semantic_metadata_drift == Some(role) {
        std::collections::BTreeMap::from([("forbidden".to_owned(), "value".to_owned())])
    } else {
        record.semantic_metadata().clone()
    };
    if kind != record.content().kind() || &metadata != record.semantic_metadata() {
        rebuild_profile(&record, kind, record.table().cloned(), metadata)
    } else {
        record
    }
}

fn fixture(footprint_parquet: bool) -> Fixture {
    fixture_with_options(FixtureOptions {
        footprint_parquet,
        ..FixtureOptions::default()
    })
}

fn fixture_with_options(options: FixtureOptions) -> Fixture {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("patch-region-inputs").expect("store ID"),
    )
    .expect("store");
    let patient = HierarchyId::from(PatientId::new("region-patient").expect("patient ID"));
    let slide_id = SlideId::new("region-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let patches = [
        PatchId::new("patch-a").expect("patch ID"),
        PatchId::new("patch-b").expect("patch ID"),
    ];
    let regions = [
        RegionId::new("region-a").expect("region ID"),
        RegionId::new("region-b").expect("region ID"),
    ];
    let mut nodes = vec![
        HierarchyNode::new(patient.clone(), None, ReplicationRole::BiologicalUnit),
        HierarchyNode::new(
            slide.clone(),
            None,
            ReplicationRole::TechnicalReplicate {
                biological_source: patient,
            },
        ),
    ];
    nodes.extend(patches.iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(regions.iter().cloned().map(|region| {
        HierarchyNode::new(
            HierarchyId::from(region),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");

    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_patches.v1",
        patches.to_vec(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let expected_patch_bytes = expected_patches
        .to_canonical_json()
        .expect("expected patch bytes");
    let mut expected_patch_bytes = expected_patch_bytes;
    if options.payload_drift == Some(Role::ExpectedPatches) {
        *expected_patch_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let mut expected_patch_record = publish_record(
        &store,
        "marklab.expected_patch_set",
        "application/vnd.marklab.expected-patch-set.v1+json",
        &expected_patch_bytes,
        Vec::new(),
    );
    let expected_regions = ExpectedRegionSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_regions.v1",
        regions.to_vec(),
        DOMAIN_BUDGET,
    )
    .expect("expected regions");
    let expected_region_bytes = expected_regions
        .to_canonical_json()
        .expect("expected region bytes");
    let mut expected_region_bytes = expected_region_bytes;
    if options.payload_drift == Some(Role::ExpectedRegions) {
        *expected_region_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let mut expected_region_record = publish_record(
        &store,
        "marklab.expected_region_set",
        "application/vnd.marklab.expected-region-set.v1+json",
        &expected_region_bytes,
        Vec::new(),
    );

    let image_id = CoordinateFrameId::new("region-image-pixels").expect("image frame");
    let physical_id = CoordinateFrameId::new("region-slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("region-pixel-to-micrometer").expect("transform");
    let image = CoordinateFrame::new(
        image_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Pixel,
        CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
    )
    .expect("image frame");
    let physical = CoordinateFrame::new(
        physical_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("physical frame");
    let transform = FrameTransform::new(
        transform_id.clone(),
        image_id.clone(),
        physical_id.clone(),
        TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("matrix"),
        None,
    );
    let registry = CoordinateRegistry::new(
        vec![image, physical],
        Vec::new(),
        vec![transform],
        Vec::new(),
    )
    .expect("registry");
    let context = PatchEmbeddingContext::new(
        &hierarchy,
        &registry,
        slide_id,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("x scale"),
        PositiveRational::new(1, 2).expect("y scale"),
        [640, 256],
        [224, 224],
        [192, 192],
        [32, 32],
        EffectiveReceptiveField::FullInput,
        PatchBoundaryPolicy::FullyContainedOnly,
        DOMAIN_BUDGET,
    )
    .expect("context");
    let mut context_bytes = context.to_canonical_json().expect("context bytes");
    if options.payload_drift == Some(Role::PatchContext) {
        *context_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let mut context_record = publish_record(
        &store,
        "marklab.patch_embedding_context",
        "application/vnd.marklab.patch-embedding-context.v1+json",
        &context_bytes,
        Vec::new(),
    );
    if options.schema_drift == Some(Role::ExpectedPatches) {
        expected_patch_record = rebuild_record(&expected_patch_record, 2, Vec::new());
    } else if options.dependency_drift == Some(Role::ExpectedPatches) {
        expected_patch_record =
            rebuild_record(&expected_patch_record, 1, vec![expected_region_record.id()]);
    }
    if options.schema_drift == Some(Role::ExpectedRegions) {
        expected_region_record = rebuild_record(&expected_region_record, 2, Vec::new());
    } else if options.dependency_drift == Some(Role::ExpectedRegions) {
        expected_region_record =
            rebuild_record(&expected_region_record, 1, vec![context_record.id()]);
    }
    if options.schema_drift == Some(Role::PatchContext) {
        context_record = rebuild_record(&context_record, 2, Vec::new());
    } else if options.dependency_drift == Some(Role::PatchContext) {
        context_record = rebuild_record(&context_record, 1, vec![expected_patch_record.id()]);
    }
    expected_patch_record =
        apply_common_profile_drift(expected_patch_record, Role::ExpectedPatches, options);
    expected_region_record =
        apply_common_profile_drift(expected_region_record, Role::ExpectedRegions, options);
    context_record = apply_common_profile_drift(context_record, Role::PatchContext, options);
    let footprints = PatchFootprintSet::new(
        &hierarchy,
        &expected_patches,
        expected_patch_record.id(),
        &context,
        context_record.id(),
        vec![
            PatchFootprint::new(patches[0].clone(), [0, 0]),
            PatchFootprint::new(patches[1].clone(), [192, 0]),
        ],
        DOMAIN_BUDGET,
    )
    .expect("footprints");
    let mut footprint_record = if options.footprint_parquet {
        publish_patch_footprint_set_parquet(
            &store,
            &expected_patches,
            &context,
            &footprints,
            budgets(),
        )
        .expect("publish Parquet footprints")
        .into_record()
    } else {
        publish_patch_footprint_set_arrow(
            &store,
            &expected_patches,
            &context,
            &footprints,
            budgets(),
        )
        .expect("publish Arrow footprints")
        .into_record()
    };
    let mut converter_record = publish_record(
        &store,
        "marklab.converter_manifest",
        "application/json",
        b"patch-region-converter",
        Vec::new(),
    );
    if options.schema_drift == Some(Role::Converter) {
        converter_record = rebuild_record(&converter_record, 2, Vec::new());
    } else if options.dependency_drift == Some(Role::Converter) {
        converter_record = rebuild_record(&converter_record, 1, vec![expected_patch_record.id()]);
    }
    if options.schema_drift == Some(Role::PatchFootprints) {
        footprint_record = rebuild_record(
            &footprint_record,
            2,
            footprint_record.dependencies().to_vec(),
        );
    } else if options.dependency_drift == Some(Role::PatchFootprints) {
        let mut dependencies = footprint_record.dependencies().to_vec();
        dependencies.push(converter_record.id());
        dependencies.sort_unstable();
        footprint_record = rebuild_record(&footprint_record, 1, dependencies);
    }
    if options.footprint_manifest_drift {
        let table = footprint_record.table().expect("footprint manifest");
        let drifted = TableManifest::new(
            table.format(),
            table.encoding_version(),
            table.row_count() + 1,
            table.columns().to_vec(),
            table.primary_key().to_vec(),
        )
        .expect("drifted footprint manifest");
        footprint_record = rebuild_profile(
            &footprint_record,
            footprint_record.content().kind(),
            Some(drifted),
            footprint_record.semantic_metadata().clone(),
        );
    }
    converter_record = apply_common_profile_drift(converter_record, Role::Converter, options);
    footprint_record = apply_common_profile_drift(footprint_record, Role::PatchFootprints, options);
    let bindings = PatchRegionAssessmentBindings::new(
        expected_region_record.id(),
        footprint_record.id(),
        converter_record.id(),
        converter_record.content().digest(),
    );
    let assessment = PatchRegionAssessment::new(
        &expected_patches,
        &expected_regions,
        &context,
        &footprints,
        &bindings,
        vec![
            PatchRegionDeclaration::fully_contained(patches[0].clone(), regions[0].clone()),
            PatchRegionDeclaration::partial_overlap(
                patches[0].clone(),
                regions[1].clone(),
                1,
                if options.alternate_relation { 3 } else { 4 },
            )
            .expect("partial overlap"),
            PatchRegionDeclaration::partial_overlap(patches[1].clone(), regions[0].clone(), 1, 2)
                .expect("half overlap"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("assessment");
    let mut assessment_bytes = assessment.to_canonical_json().expect("assessment bytes");
    if options.payload_drift == Some(Role::Assessment) {
        *assessment_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let mut assessment_record = publish_record(
        &store,
        "marklab.patch_region_assessment",
        "application/vnd.marklab.patch-region-assessment.v1+json",
        &assessment_bytes,
        assessment.direct_dependencies().collect(),
    );
    if options.schema_drift == Some(Role::Assessment) {
        assessment_record = rebuild_record(
            &assessment_record,
            2,
            assessment_record.dependencies().to_vec(),
        );
    } else if options.dependency_drift == Some(Role::Assessment) {
        assessment_record = rebuild_record(&assessment_record, 1, Vec::new());
    }
    if options.assessment_table_drift {
        assessment_record = rebuild_profile(
            &assessment_record,
            assessment_record.content().kind(),
            footprint_record.table().cloned(),
            assessment_record.semantic_metadata().clone(),
        );
    }
    assessment_record = apply_common_profile_drift(assessment_record, Role::Assessment, options);
    let link = PatchRegionLink::from_exhaustive_assessment(
        &assessment,
        assessment_record.id(),
        assessment_record.content().digest(),
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("link");
    let mut records = vec![
        (Role::ExpectedPatches, expected_patch_record),
        (Role::ExpectedRegions, expected_region_record),
        (Role::PatchContext, context_record),
        (Role::PatchFootprints, footprint_record),
        (Role::Converter, converter_record),
        (Role::Assessment, assessment_record.clone()),
    ];
    records.retain(|(role, _)| options.omit != Some(*role));
    let catalog = ArtifactCatalog::from_records(records.into_iter().map(|(_, record)| record))
        .expect("catalog");
    Fixture {
        _root: root,
        store,
        catalog,
        assessment,
        assessment_record,
        expected_patches,
        expected_regions,
        context,
        footprints,
        link,
    }
}

fn verified_graph(fixture: &Fixture) -> VerifiedPatchRegionInputArtifactGraph {
    fixture
        .assessment
        .validate_patch_region_input_artifact_graph(
            fixture.assessment_record.id(),
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.link,
            &fixture.catalog,
            &fixture.store,
            budgets(),
        )
        .expect("verified patch-region inputs")
}

fn public_role(role: Role) -> PatchRegionInputArtifactRole {
    match role {
        Role::ExpectedPatches => PatchRegionInputArtifactRole::ExpectedPatches,
        Role::ExpectedRegions => PatchRegionInputArtifactRole::ExpectedRegions,
        Role::PatchContext => PatchRegionInputArtifactRole::PatchContext,
        Role::PatchFootprints => PatchRegionInputArtifactRole::PatchFootprints,
        Role::Converter => PatchRegionInputArtifactRole::Converter,
        Role::Assessment => PatchRegionInputArtifactRole::Assessment,
    }
}

fn graph_result(
    fixture: &Fixture,
) -> Result<VerifiedPatchRegionInputArtifactGraph, PatchRegionInputArtifactGraphError> {
    fixture
        .assessment
        .validate_patch_region_input_artifact_graph(
            fixture.assessment_record.id(),
            &fixture.expected_patches,
            &fixture.expected_regions,
            &fixture.context,
            &fixture.footprints,
            &fixture.link,
            &fixture.catalog,
            &fixture.store,
            budgets(),
        )
}

#[test]
fn patch_region_input_graph_is_vector_independent_and_physically_verifies_footprints() {
    for footprint_parquet in [false, true] {
        let fixture = fixture(footprint_parquet);
        let verified = fixture
            .assessment
            .validate_patch_region_input_artifact_graph(
                fixture.assessment_record.id(),
                &fixture.expected_patches,
                &fixture.expected_regions,
                &fixture.context,
                &fixture.footprints,
                &fixture.link,
                &fixture.catalog,
                &fixture.store,
                budgets(),
            )
            .expect("verified patch-region inputs");
        assert_eq!(verified.assessed_pair_count(), 4);
        assert_eq!(verified.nonzero_relation_count(), 3);
        assert_eq!(verified.logical_digest(), fixture.link.logical_digest());
        assert_eq!(
            verified.assessment_artifact_id(),
            fixture.assessment_record.id()
        );
        let debug = format!("{verified:?}");
        assert!(!debug.contains("region-slide"));
        assert!(!debug.contains(&fixture.assessment_record.id().to_string()));
    }
}

#[test]
fn patch_region_input_graph_rejects_every_role_schema_and_dependency_drift() {
    for role in [
        Role::ExpectedPatches,
        Role::ExpectedRegions,
        Role::PatchContext,
        Role::PatchFootprints,
        Role::Converter,
        Role::Assessment,
    ] {
        let fixture = fixture_with_options(FixtureOptions {
            schema_drift: Some(role),
            ..FixtureOptions::default()
        });
        assert_eq!(
            graph_result(&fixture),
            Err(PatchRegionInputArtifactGraphError::SchemaMismatch {
                role: public_role(role),
            }),
            "schema role {role:?}"
        );

        let fixture = fixture_with_options(FixtureOptions {
            dependency_drift: Some(role),
            ..FixtureOptions::default()
        });
        assert_eq!(
            graph_result(&fixture),
            Err(PatchRegionInputArtifactGraphError::DependencyMismatch {
                role: public_role(role),
            }),
            "dependency role {role:?}"
        );
    }
}

#[test]
fn patch_region_input_graph_rejects_each_profile_branch() {
    for role in [
        Role::ExpectedPatches,
        Role::ExpectedRegions,
        Role::PatchContext,
        Role::PatchFootprints,
        Role::Converter,
        Role::Assessment,
    ] {
        let fixture = fixture_with_options(FixtureOptions {
            kind_drift: Some(role),
            ..FixtureOptions::default()
        });
        assert_eq!(
            graph_result(&fixture),
            Err(PatchRegionInputArtifactGraphError::ContentKindMismatch {
                role: public_role(role),
            }),
            "content-kind role {role:?}"
        );
    }

    for role in [Role::ExpectedPatches, Role::PatchFootprints] {
        let fixture = fixture_with_options(FixtureOptions {
            semantic_metadata_drift: Some(role),
            ..FixtureOptions::default()
        });
        assert_eq!(
            graph_result(&fixture),
            Err(
                PatchRegionInputArtifactGraphError::SemanticMetadataMismatch {
                    role: public_role(role),
                },
            ),
            "semantic-metadata role {role:?}"
        );
    }

    let unexpected_table = fixture_with_options(FixtureOptions {
        assessment_table_drift: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        graph_result(&unexpected_table),
        Err(
            PatchRegionInputArtifactGraphError::UnexpectedTableManifest {
                role: PatchRegionInputArtifactRole::Assessment,
            }
        )
    );

    let wrong_footprint_manifest = fixture_with_options(FixtureOptions {
        footprint_manifest_drift: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        graph_result(&wrong_footprint_manifest),
        Err(PatchRegionInputArtifactGraphError::TableManifestMismatch)
    );
}

#[test]
fn patch_region_input_graph_streams_every_canonical_payload_and_requires_assessment() {
    for role in [
        Role::ExpectedPatches,
        Role::ExpectedRegions,
        Role::PatchContext,
        Role::Assessment,
    ] {
        let fixture = fixture_with_options(FixtureOptions {
            payload_drift: Some(role),
            ..FixtureOptions::default()
        });
        assert_eq!(
            graph_result(&fixture),
            Err(
                PatchRegionInputArtifactGraphError::PayloadIdentityMismatch {
                    role: public_role(role),
                }
            ),
            "payload role {role:?}"
        );
    }

    let fixture = fixture_with_options(FixtureOptions {
        omit: Some(Role::Assessment),
        ..FixtureOptions::default()
    });
    assert_eq!(
        graph_result(&fixture),
        Err(PatchRegionInputArtifactGraphError::MissingRecord {
            role: PatchRegionInputArtifactRole::Assessment,
        })
    );
}

#[test]
fn patch_region_physical_receipts_require_the_exact_input_graph_in_both_formats() {
    let fixture = fixture(false);
    let graph = verified_graph(&fixture);
    let arrow = publish_patch_region_link_arrow(&fixture.store, &fixture.link, budgets())
        .expect("publish Arrow link")
        .into_record();
    let parquet = publish_patch_region_link_parquet(&fixture.store, &fixture.link, budgets())
        .expect("publish Parquet link")
        .into_record();
    let arrow_receipt = verify_patch_region_link_arrow_from_store(
        &fixture.store,
        &arrow,
        &fixture.link,
        graph,
        budgets(),
    )
    .expect("verify Arrow link");
    let parquet_receipt = verify_patch_region_link_parquet_from_store(
        &fixture.store,
        &parquet,
        &fixture.link,
        graph,
        budgets(),
    )
    .expect("verify Parquet link");
    for receipt in [arrow_receipt, parquet_receipt] {
        assert_eq!(receipt.logical_digest(), fixture.link.logical_digest());
        assert_eq!(receipt.assessed_pair_count(), 4);
        assert_eq!(receipt.row_count(), 3);
    }
    assert_eq!(arrow_receipt.artifact_id(), arrow.id());
    assert_eq!(parquet_receipt.artifact_id(), parquet.id());
    let debug = format!("{arrow_receipt:?} {parquet_receipt:?}");
    assert!(!debug.contains("region-slide"));
    assert!(!debug.contains(&arrow.id().to_string()));
    assert!(!debug.contains(&parquet.id().to_string()));
}

#[test]
fn patch_region_receipt_rejects_a_graph_for_a_different_logical_link() {
    let fixture = fixture(false);
    let alternate = fixture_with_options(FixtureOptions {
        alternate_relation: true,
        ..FixtureOptions::default()
    });
    let alternate_graph = verified_graph(&alternate);
    assert_ne!(
        fixture.link.logical_digest(),
        alternate.link.logical_digest()
    );
    let record = publish_patch_region_link_arrow(&fixture.store, &fixture.link, budgets())
        .expect("publish Arrow link")
        .into_record();
    assert!(matches!(
        verify_patch_region_link_arrow_from_store(
            &fixture.store,
            &record,
            &fixture.link,
            alternate_graph,
            budgets(),
        ),
        Err(VerifiedReaderError::Callback(
            MultiscaleColumnarError::ArtifactBindingMismatch,
        ))
    ));
}

#[test]
fn patch_region_managed_integrity_precedes_decode_and_callbacks_match_borrowed() {
    #[derive(Clone, Copy)]
    enum Format {
        Arrow,
        Parquet,
    }

    for format in [Format::Arrow, Format::Parquet] {
        let fixture = fixture(false);
        let canonical = match format {
            Format::Arrow => {
                publish_patch_region_link_arrow(&fixture.store, &fixture.link, budgets())
            }
            Format::Parquet => {
                publish_patch_region_link_parquet(&fixture.store, &fixture.link, budgets())
            }
        }
        .expect("publish canonical profile")
        .into_record();
        let mut malformed = Vec::new();
        match format {
            Format::Arrow => {
                write_patch_region_link_arrow(&mut malformed, &fixture.link, budgets())
            }
            Format::Parquet => {
                write_patch_region_link_parquet(&mut malformed, &fixture.link, budgets())
            }
        }
        .expect("write canonical profile");
        let row_at = malformed
            .windows(b"patch-a".len())
            .position(|window| window == b"patch-a")
            .expect("physical row value");
        malformed[row_at] ^= 0x20;
        let draft = ArtifactDraft::new(
            ArtifactRef::from_bytes(canonical.content().kind(), &malformed)
                .expect("malformed content"),
            canonical.schema().clone(),
            canonical.table().cloned(),
            canonical.dependencies().to_vec(),
            Default::default(),
        )
        .expect("malformed draft");
        let malformed_record = fixture
            .store
            .publish_new_send(&draft, |output| output.write_all(&malformed))
            .expect("publish digest-matching malformed profile")
            .into_record();
        let borrowed = match format {
            Format::Arrow => validate_patch_region_link_arrow_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_region_link_parquet_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
        }
        .expect_err("borrowed malformed profile");
        let managed = match format {
            Format::Arrow => validate_patch_region_link_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_region_link_parquet_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
        };
        match managed {
            Err(VerifiedReaderError::Callback(error)) => assert_eq!(error, borrowed),
            observed => panic!("expected matching managed callback error, got {observed:?}"),
        }

        let path = fixture
            ._root
            .path()
            .join(malformed_record.locations()[0].key().as_str());
        let mut corrupt = malformed.clone();
        corrupt[0] ^= 1;
        std::fs::write(&path, corrupt).expect("corrupt managed profile");
        let integrity = match format {
            Format::Arrow => validate_patch_region_link_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Format::Parquet => validate_patch_region_link_parquet_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
        };
        assert!(matches!(
            integrity,
            Err(VerifiedReaderError::Store(
                ArtifactStoreError::ContentIntegrity { .. }
            ))
        ));
    }
}

#[test]
fn patch_region_input_graph_preserves_managed_footprint_integrity_precedence() {
    let fixture = fixture(false);
    let footprint = fixture.link.patch_footprints_artifact_id().to_string();
    let path = fixture
        ._root
        .path()
        .join("objects/sha256")
        .join(&footprint[..2])
        .join(&footprint);
    std::fs::write(
        &path,
        vec![
            0_u8;
            fixture
                .catalog
                .get(fixture.link.patch_footprints_artifact_id())
                .expect("footprint record")
                .content()
                .byte_len() as usize
        ],
    )
    .expect("corrupt managed footprint");
    let error = graph_result(&fixture).expect_err("corrupt footprint must fail");
    assert_eq!(
        error,
        PatchRegionInputArtifactGraphError::Unavailable {
            role: PatchRegionInputArtifactRole::PatchFootprints,
            reason: marklab::ArtifactAvailabilityFailure::Integrity,
        }
    );
    let rendered = error.to_string();
    assert!(!rendered.contains("region-slide"));
    assert!(!rendered.contains(&path.display().to_string()));
    assert!(!rendered.contains(&footprint));
}
