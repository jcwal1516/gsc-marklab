use std::collections::BTreeMap;

use marklab::{
    ArtifactAvailabilityFailure, ArtifactCatalog, ArtifactKey, ArtifactLocator, ArtifactRecord,
    ArtifactRef, ArtifactSchema, CanonicalDecimal, CellEmbeddingArtifactRole,
    CellEmbeddingExecutionProvenance, CellEmbeddingInputArtifacts, CellEmbeddingModelProvenance,
    CellEmbeddingProvenance, CellEmbeddingRowLink, CellEmbeddingRowLinkEntry,
    CellEmbeddingTensorContract, CellId, CellIdentityMap, CellIdentityMapEntry, CohortHierarchy,
    CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace, CoordinateUnit,
    EmbeddingArtifactGraphError, EmbeddingSpatialContext, ExpectedCellSet, FrameTransform,
    HierarchyId, HierarchyNode, ImageCoordinateConvention, LocalArtifactStore, PatchBoundaryPolicy,
    PatientId, PositiveRational, ReplicationRole, SlideId, SpatialAxis, StoreId, TableColumn,
    TableColumnType, TableFormat, TableManifest, TableScalarType, TransformId, TransformMatrix,
};
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    store: LocalArtifactStore,
    catalog: ArtifactCatalog,
    provenance: CellEmbeddingProvenance,
    provenance_artifact_id: marklab::ArtifactId,
    expected: ExpectedCellSet,
    identity_map: CellIdentityMap,
    context: EmbeddingSpatialContext,
    row_link: CellEmbeddingRowLink,
}

#[derive(Clone, Copy)]
enum LicenseAvailability {
    Managed,
    CatalogOnly,
    NonManagedLocal,
}

fn cell(value: &str) -> CellId {
    CellId::new(value).expect("cell ID")
}

fn draft_record(
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("artifact content"),
        ArtifactSchema::new(schema, 1).expect("artifact schema"),
        table,
        dependencies,
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new(format!("fixtures/privacy-sentinel/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("artifact record")
}

fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    store
        .publish(
            &draft_record(schema, kind, bytes, dependencies, table),
            |writer| writer.write_all(bytes),
        )
        .expect("publish fixture")
        .into_record()
}

fn row_link_manifest(row_count: u64) -> TableManifest {
    TableManifest::new(
        TableFormat::ArrowIpcFile,
        "marklab.arrow-ipc.embedding-row-link.v1",
        row_count,
        vec![
            TableColumn::new(
                "cell_id",
                TableColumnType::Scalar(TableScalarType::Utf8),
                false,
            )
            .expect("cell column"),
            TableColumn::new(
                "source_cell_row",
                TableColumnType::Scalar(TableScalarType::U64),
                false,
            )
            .expect("source row column"),
            TableColumn::new(
                "source_embedding_row",
                TableColumnType::Scalar(TableScalarType::U64),
                true,
            )
            .expect("embedding row column"),
        ],
        vec!["cell_id".to_owned()],
    )
    .expect("row-link manifest")
}

fn context() -> EmbeddingSpatialContext {
    let image_id = CoordinateFrameId::new("image-pixels").expect("image frame");
    let physical_id = CoordinateFrameId::new("slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("pixel-to-micrometer").expect("transform");
    let registry = CoordinateRegistry::new(
        vec![
            CoordinateFrame::new(
                image_id.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Pixel,
                CoordinateSpace::Image(ImageCoordinateConvention::PixelCenterAtInteger),
            )
            .expect("image frame"),
            CoordinateFrame::new(
                physical_id.clone(),
                vec![SpatialAxis::X, SpatialAxis::Y],
                CoordinateUnit::Micrometer,
                CoordinateSpace::Physical,
            )
            .expect("physical frame"),
        ],
        Vec::new(),
        vec![FrameTransform::new(
            transform_id.clone(),
            image_id.clone(),
            physical_id.clone(),
            TransformMatrix::affine_2d([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]).expect("matrix"),
            None,
        )],
        Vec::new(),
    )
    .expect("registry");
    EmbeddingSpatialContext::new(
        &registry,
        image_id,
        physical_id,
        transform_id,
        PositiveRational::new(1, 2).expect("mpp x"),
        PositiveRational::new(1, 2).expect("mpp y"),
        [1_024, 1_024],
        [64, 64],
        [16, 16],
        PatchBoundaryPolicy::FullyContainedOnly,
    )
    .expect("context")
}

fn hierarchy(expected: &ExpectedCellSet) -> CohortHierarchy {
    let patient = HierarchyId::from(PatientId::new("patient").expect("patient"));
    let slide = HierarchyId::from(SlideId::new("slide").expect("slide"));
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
    nodes.extend(expected.cells().iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy")
}

fn build_fixture(checkpoint_schema: &str, license_availability: LicenseAvailability) -> Fixture {
    let root = TempDir::new().expect("temporary store");
    let store =
        LocalArtifactStore::open(root.path(), StoreId::new("local").expect("local store ID"))
            .expect("local store");
    let checkpoint = publish_record(
        &store,
        checkpoint_schema,
        "application/octet-stream",
        b"checkpoint",
        Vec::new(),
        None,
    );
    let source_snapshot = publish_record(
        &store,
        "marklab.source_snapshot",
        "application/octet-stream",
        b"source-snapshot",
        Vec::new(),
        None,
    );
    let license_draft = draft_record(
        "marklab.license_record",
        "text/plain",
        b"license",
        Vec::new(),
        None,
    );
    let license = match license_availability {
        LicenseAvailability::Managed => store
            .publish(&license_draft, |writer| writer.write_all(b"license"))
            .expect("publish license")
            .into_record(),
        LicenseAvailability::CatalogOnly => license_draft,
        LicenseAvailability::NonManagedLocal => {
            let key = "incoming/privacy-sentinel-license";
            std::fs::create_dir_all(root.path().join("incoming"))
                .expect("create non-managed local parent");
            std::fs::write(root.path().join(key), b"license")
                .expect("write non-managed local replica");
            ArtifactRecord::new(
                license_draft.content().clone(),
                license_draft.schema().clone(),
                None,
                Vec::new(),
                BTreeMap::new(),
                vec![ArtifactLocator::new(
                    StoreId::new("local").expect("local store ID"),
                    ArtifactKey::new(key).expect("non-managed local key"),
                    None,
                )
                .expect("non-managed local locator")],
            )
            .expect("non-managed local record")
        }
    };
    let preprocessing = publish_record(
        &store,
        "marklab.embedding_preprocessing",
        "application/json",
        b"preprocessing",
        Vec::new(),
        None,
    );
    let run_config = publish_record(
        &store,
        "marklab.embedding_run_config",
        "application/json",
        b"run-config",
        Vec::new(),
        None,
    );
    let environment = publish_record(
        &store,
        "marklab.execution_environment",
        "application/json",
        b"environment",
        Vec::new(),
        None,
    );
    let converter = publish_record(
        &store,
        "marklab.converter_manifest",
        "application/json",
        b"converter",
        Vec::new(),
        None,
    );
    let source_cells = publish_record(
        &store,
        "marklab.cell_embedding_source_cells",
        "text/csv;profile=marklab-cellvit-he-bundle-v1",
        b"source-cells",
        Vec::new(),
        None,
    );
    let source_vectors = publish_record(
        &store,
        "marklab.cell_embedding_source_npy",
        "application/x-npy;profile=marklab-cellvit-he-f4-v1",
        b"source-vectors",
        Vec::new(),
        None,
    );

    let expected = ExpectedCellSet::new("all.v1", vec![cell("cell-a")]).expect("expected");
    let expected_bytes = expected.to_bytes().expect("expected bytes");
    let expected_record = publish_record(
        &store,
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected_bytes,
        Vec::new(),
        None,
    );
    let identity_map = CellIdentityMap::new(
        source_cells.id(),
        expected_record.id(),
        &expected,
        vec![CellIdentityMapEntry::new("source-a", cell("cell-a")).expect("identity")],
    )
    .expect("identity map");
    let identity_bytes = identity_map.to_bytes().expect("identity bytes");
    let identity_record = publish_record(
        &store,
        "marklab.cell_embedding_identity_map",
        "application/vnd.marklab.embedding-identity-map.v1",
        &identity_bytes,
        vec![source_cells.id(), expected_record.id()],
        None,
    );
    let context = context();
    let context_bytes = context.to_canonical_json().expect("context bytes");
    let context_record = publish_record(
        &store,
        "marklab.cell_embedding_spatial_context",
        "application/vnd.marklab.embedding-spatial-context.v1+json",
        &context_bytes,
        Vec::new(),
        None,
    );
    let hierarchy = hierarchy(&expected);
    let row_link = CellEmbeddingRowLink::new(
        source_cells.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        converter.id(),
        &expected,
        &hierarchy,
        vec![CellEmbeddingRowLinkEntry::present(cell("cell-a"), 0, 0)],
        4_096,
    )
    .expect("row link");
    let row_link_record = publish_record(
        &store,
        "marklab.cell_embedding_row_link",
        "application/vnd.marklab.embedding-row-link.v1+arrow",
        b"validated-row-link-physical-bytes",
        row_link.direct_dependencies().to_vec(),
        Some(row_link_manifest(1)),
    );

    let model = CellEmbeddingModelProvenance::new(
        "cellvit_sam_h",
        "1.0",
        "sam_h",
        checkpoint.id(),
        checkpoint.content().digest(),
        source_snapshot.id(),
        license.id(),
        "Apache-2.0",
        "doi:10.0000-example",
        "z4",
        32,
    )
    .expect("model provenance");
    let decimal = |value| CanonicalDecimal::new(value).expect("decimal");
    let tensor = CellEmbeddingTensorContract::rgb_he_raw(
        1_280,
        [decimal("0.485"), decimal("0.456"), decimal("0.406")],
        [decimal("0.229"), decimal("0.224"), decimal("0.225")],
    )
    .expect("tensor");
    let execution = CellEmbeddingExecutionProvenance::new(
        preprocessing.id(),
        run_config.id(),
        environment.id(),
        converter.id(),
        "marklab_cellvit_converter",
        "1.0",
        "cellvit_he_bundle",
        "1.0",
    )
    .expect("execution");
    let inputs = CellEmbeddingInputArtifacts::new(
        source_cells.id(),
        source_vectors.id(),
        expected_record.id(),
        identity_record.id(),
        context_record.id(),
        row_link_record.id(),
    );
    let provenance =
        CellEmbeddingProvenance::new(model, tensor, execution, inputs).expect("provenance");
    let provenance_bytes = provenance.to_canonical_json().expect("provenance bytes");
    let provenance_record = publish_record(
        &store,
        "marklab.cell_embedding_provenance",
        "application/vnd.marklab.embedding-provenance.v1+json",
        &provenance_bytes,
        provenance.direct_dependencies().to_vec(),
        None,
    );
    let provenance_artifact_id = provenance_record.id();
    let catalog = ArtifactCatalog::from_records([
        checkpoint,
        source_snapshot,
        license,
        preprocessing,
        run_config,
        environment,
        converter,
        source_cells,
        source_vectors,
        expected_record,
        identity_record,
        context_record,
        row_link_record,
        provenance_record,
    ])
    .expect("artifact catalog");
    Fixture {
        _root: root,
        store,
        catalog,
        provenance,
        provenance_artifact_id,
        expected,
        identity_map,
        context,
        row_link,
    }
}

fn replace_record(
    catalog: &ArtifactCatalog,
    replaced: marklab::ArtifactId,
    replacement: ArtifactRecord,
) -> ArtifactCatalog {
    ArtifactCatalog::from_records(
        catalog
            .iter()
            .filter(|(id, _record)| **id != replaced)
            .map(|(_id, record)| record.clone())
            .chain(std::iter::once(replacement)),
    )
    .expect("replacement catalog")
}

fn provenance_record(
    fixture: &Fixture,
    kind: &str,
    dependencies: Vec<marklab::ArtifactId>,
    semantic_metadata: BTreeMap<String, String>,
    table: Option<TableManifest>,
) -> ArtifactRecord {
    let bytes = fixture
        .provenance
        .to_canonical_json()
        .expect("provenance bytes");
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, &bytes).expect("provenance content"),
        ArtifactSchema::new("marklab.cell_embedding_provenance", 1).expect("provenance schema"),
        table,
        dependencies,
        semantic_metadata,
        vec![ArtifactLocator::new(
            StoreId::new("source").expect("source store"),
            ArtifactKey::new("fixtures/privacy-sentinel/provenance-drift").expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("provenance record")
}

fn record_with_schema<'a>(fixture: &'a Fixture, schema: &str) -> &'a ArtifactRecord {
    fixture
        .catalog
        .iter()
        .find_map(|(_id, record)| (record.schema().id() == schema).then_some(record))
        .expect("record with schema")
}

fn managed_path(root: &TempDir, record: &ArtifactRecord) -> std::path::PathBuf {
    let id = record.id().to_string();
    root.path().join("objects/sha256").join(&id[..2]).join(id)
}

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
