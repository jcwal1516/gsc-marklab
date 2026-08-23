#![cfg(feature = "parquet")]

use marklab::{
    publish_cell_patch_assignment_table_arrow, publish_cell_patch_assignment_table_parquet,
    publish_cell_patch_edge_table_arrow, publish_cell_patch_edge_table_parquet,
    publish_patch_footprint_set_arrow, publish_patch_footprint_set_parquet,
    validate_cell_patch_assignment_table_arrow_bytes,
    validate_cell_patch_assignment_table_arrow_from_store,
    validate_cell_patch_assignment_table_parquet_bytes,
    validate_cell_patch_assignment_table_parquet_from_store,
    validate_cell_patch_edge_table_arrow_bytes, validate_cell_patch_edge_table_arrow_from_store,
    validate_cell_patch_edge_table_parquet_bytes,
    validate_cell_patch_edge_table_parquet_from_store,
    verify_cell_patch_assignment_table_arrow_from_store,
    verify_cell_patch_assignment_table_parquet_from_store,
    verify_cell_patch_edge_table_arrow_from_store, verify_cell_patch_edge_table_parquet_from_store,
    write_cell_patch_assignment_table_arrow, write_cell_patch_assignment_table_parquet,
    write_cell_patch_edge_table_arrow, write_cell_patch_edge_table_parquet, ArtifactCatalog,
    ArtifactDraft, ArtifactKey, ArtifactLocator, ArtifactRecord, ArtifactRef, ArtifactSchema,
    ArtifactStoreError, CellId, CellPatchAnchor, CellPatchInputArtifactGraphError,
    CellPatchInputArtifactRole, CellPatchLink, CellPatchLinkBindings, CellPatchLinkProducer,
    CohortHierarchy, CoordinateFrame, CoordinateFrameId, CoordinateRegistry, CoordinateSpace,
    CoordinateUnit, EffectiveReceptiveField, EmbeddingColumnarBudgets, ExpectedCellSet,
    ExpectedPatchSet, FrameTransform, HierarchyId, HierarchyNode, ImageCoordinateConvention,
    LocalArtifactStore, MultiscaleColumnarError, PatchBoundaryPolicy, PatchEmbeddingContext,
    PatchFootprint, PatchFootprintSet, PatchId, PatientId, PositiveRational, ReplicationRole,
    SlideId, SpatialAxis, StoreId, TransformId, TransformMatrix, VerifiedCellPatchLinkArtifact,
    VerifiedReaderError,
};
use tempfile::TempDir;

const DOMAIN_BUDGET: usize = 8 * 1024 * 1024;

struct Fixture {
    _root: TempDir,
    store: LocalArtifactStore,
    catalog: ArtifactCatalog,
    hierarchy: CohortHierarchy,
    producer: CellPatchLinkProducer,
    producer_record: ArtifactRecord,
    expected_cells: ExpectedCellSet,
    expected_patches: ExpectedPatchSet,
    context: PatchEmbeddingContext,
    footprints: PatchFootprintSet,
    footprint_record: ArtifactRecord,
    link: CellPatchLink,
}

#[derive(Clone, Copy, Default)]
struct FixtureOptions {
    expected_cell_payload_drift: bool,
    expected_patch_payload_drift: bool,
    context_payload_drift: bool,
    producer_payload_drift: bool,
    producer_dependency_drift: bool,
    footprint_dependency_drift: bool,
    footprint_schema_drift: bool,
    footprint_parquet: bool,
    source_coordinates_catalog_only: bool,
    alias_source_coordinates_with_expected_cells: bool,
}

fn budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn draft_record(
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
) -> ArtifactRecord {
    ArtifactRecord::new(
        ArtifactRef::from_bytes(kind, bytes).expect("artifact content"),
        ArtifactSchema::new(schema, 1).expect("artifact schema"),
        None,
        dependencies,
        Default::default(),
        vec![ArtifactLocator::new(
            StoreId::new("fixture-source").expect("source store"),
            ArtifactKey::new(format!("fixtures/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("artifact draft")
}

fn publish_record(
    store: &LocalArtifactStore,
    schema: &str,
    kind: &str,
    bytes: &[u8],
    dependencies: Vec<marklab::ArtifactId>,
) -> ArtifactRecord {
    let draft = draft_record(schema, kind, bytes, dependencies);
    store
        .publish(&draft, |writer| writer.write_all(bytes))
        .expect("publish artifact")
        .into_record()
}

fn fixture() -> Fixture {
    fixture_with_options(FixtureOptions::default())
}

fn fixture_with_options(options: FixtureOptions) -> Fixture {
    let root = TempDir::new().expect("store root");
    let store = LocalArtifactStore::open(
        root.path(),
        StoreId::new("cell-patch-inputs").expect("store ID"),
    )
    .expect("store");
    let patient = HierarchyId::from(PatientId::new("input-patient").expect("patient ID"));
    let slide_id = SlideId::new("input-slide").expect("slide ID");
    let slide = HierarchyId::from(slide_id.clone());
    let cells = [
        CellId::new("cell-a").expect("cell ID"),
        CellId::new("cell-b").expect("cell ID"),
    ];
    let patches = [
        PatchId::new("patch-a").expect("patch ID"),
        PatchId::new("patch-b").expect("patch ID"),
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
    nodes.extend(cells.iter().cloned().map(|cell| {
        HierarchyNode::new(
            HierarchyId::from(cell),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(patches.iter().cloned().map(|patch| {
        HierarchyNode::new(
            HierarchyId::from(patch),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    let hierarchy = CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy");

    let expected_cells =
        ExpectedCellSet::new("all_cells.v1", cells.to_vec()).expect("expected cells");
    let mut expected_cell_bytes = expected_cells.to_bytes().expect("expected-cell bytes");
    if options.expected_cell_payload_drift {
        expected_cell_bytes[0] ^= 1;
    }
    let expected_cell_record = publish_record(
        &store,
        "marklab.cell_embedding_expected_cells",
        "application/vnd.marklab.embedding-expected-cells.v1",
        &expected_cell_bytes,
        Vec::new(),
    );
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_patches.v1",
        patches.to_vec(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let mut expected_patch_bytes = expected_patches
        .to_canonical_json()
        .expect("expected-patch JSON");
    if options.expected_patch_payload_drift {
        *expected_patch_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let expected_patch_record = publish_record(
        &store,
        "marklab.expected_patch_set",
        "application/vnd.marklab.expected-patch-set.v1+json",
        &expected_patch_bytes,
        Vec::new(),
    );

    let image_id = CoordinateFrameId::new("input-image-pixels").expect("image frame");
    let physical_id = CoordinateFrameId::new("input-slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("input-pixel-to-micrometer").expect("transform");
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
        image_id.clone(),
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
    let mut context_bytes = context.to_canonical_json().expect("context JSON");
    if options.context_payload_drift {
        *context_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let context_record = publish_record(
        &store,
        "marklab.patch_embedding_context",
        "application/vnd.marklab.patch-embedding-context.v1+json",
        &context_bytes,
        Vec::new(),
    );
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
    let published_footprint_record = if options.footprint_parquet {
        publish_patch_footprint_set_parquet(
            &store,
            &expected_patches,
            &context,
            &footprints,
            budgets(),
        )
        .expect("publish Parquet footprint")
        .into_record()
    } else {
        publish_patch_footprint_set_arrow(
            &store,
            &expected_patches,
            &context,
            &footprints,
            budgets(),
        )
        .expect("publish Arrow footprint")
        .into_record()
    };
    let footprint_record = if options.footprint_dependency_drift || options.footprint_schema_drift {
        let mut dependencies = published_footprint_record.dependencies().to_vec();
        if options.footprint_dependency_drift {
            dependencies.push(expected_cell_record.id());
        }
        ArtifactRecord::new(
            published_footprint_record.content().clone(),
            if options.footprint_schema_drift {
                ArtifactSchema::new("marklab.patch_footprint_table", 2)
                    .expect("drifted footprint schema")
            } else {
                published_footprint_record.schema().clone()
            },
            published_footprint_record.table().cloned(),
            dependencies,
            published_footprint_record.semantic_metadata().clone(),
            published_footprint_record.locations().to_vec(),
        )
        .expect("drifted footprint record")
    } else {
        published_footprint_record
    };

    let source_coordinate_draft = draft_record(
        "marklab.cell_anchor_source_coordinates",
        "application/vnd.marklab.cell-anchor-source-coordinates.v1+binary",
        b"opaque-private-coordinates",
        Vec::new(),
    );
    let source_coordinates = if options.source_coordinates_catalog_only {
        source_coordinate_draft
    } else {
        store
            .publish(&source_coordinate_draft, |writer| {
                writer.write_all(b"opaque-private-coordinates")
            })
            .expect("publish source coordinates")
            .into_record()
    };
    let run_config = publish_record(
        &store,
        "marklab.embedding_run_config",
        "application/json",
        b"run-config",
        Vec::new(),
    );
    let environment = publish_record(
        &store,
        "marklab.execution_environment",
        "application/json",
        b"environment",
        Vec::new(),
    );
    let converter = publish_record(
        &store,
        "marklab.converter_manifest",
        "application/json",
        b"converter",
        Vec::new(),
    );
    let producer = CellPatchLinkProducer::contained_shared(
        "1.0.0",
        if options.alias_source_coordinates_with_expected_cells {
            expected_cell_record.id()
        } else {
            source_coordinates.id()
        },
        run_config.id(),
        environment.id(),
        converter.id(),
        DOMAIN_BUDGET,
    )
    .expect("producer");
    let mut producer_bytes = producer.to_canonical_json().expect("producer JSON");
    if options.producer_payload_drift {
        *producer_bytes.last_mut().expect("JSON is nonempty") = b' ';
    }
    let mut producer_dependencies = producer.direct_dependencies().collect::<Vec<_>>();
    if options.producer_dependency_drift {
        producer_dependencies.push(expected_patch_record.id());
        producer_dependencies.sort_unstable();
    }
    let producer_record = publish_record(
        &store,
        "marklab.cell_patch_link_producer",
        "application/vnd.marklab.cell-patch-link-producer.v1+json",
        &producer_bytes,
        producer_dependencies,
    );
    let bindings = CellPatchLinkBindings::new(
        expected_cell_record.id(),
        footprint_record.id(),
        producer_record.id(),
        producer_record.content().digest(),
        image_id,
    );
    let link = CellPatchLink::derive_contained_shared(
        &hierarchy,
        &expected_cells,
        &expected_patches,
        &context,
        &footprints,
        &bindings,
        vec![
            CellPatchAnchor::new(cells[0].clone(), [10.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[1].clone(), [500.0, 10.0]).expect("anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("cell-patch link");
    let catalog = ArtifactCatalog::from_records(vec![
        expected_cell_record,
        expected_patch_record,
        context_record,
        footprint_record.clone(),
        source_coordinates,
        run_config,
        environment,
        converter,
        producer_record.clone(),
    ])
    .expect("catalog");
    Fixture {
        _root: root,
        store,
        catalog,
        hierarchy,
        producer,
        producer_record,
        expected_cells,
        expected_patches,
        context,
        footprints,
        footprint_record,
        link,
    }
}

fn alternate_link(fixture: &Fixture) -> CellPatchLink {
    let bindings = CellPatchLinkBindings::new(
        fixture.link.expected_cells_artifact_id(),
        fixture.link.patch_footprints_artifact_id(),
        fixture.link.producer_artifact_id(),
        fixture.link.producer_content_digest(),
        fixture.context.image_frame_id().clone(),
    );
    CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        &fixture.expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        &bindings,
        vec![
            CellPatchAnchor::new(fixture.expected_cells.cells()[0].clone(), [20.0, 10.0])
                .expect("alternate anchor"),
            CellPatchAnchor::new(fixture.expected_cells.cells()[1].clone(), [500.0, 10.0])
                .expect("outside anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("alternate link")
}

fn verified_graph(
    fixture: &Fixture,
    link: &CellPatchLink,
) -> marklab::VerifiedCellPatchInputArtifactGraph {
    fixture
        .producer
        .validate_cell_patch_input_artifact_graph(
            fixture.producer_record.id(),
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            link,
            &fixture.catalog,
            &fixture.store,
            budgets(),
        )
        .expect("verified cell-patch inputs")
}

#[test]
fn cell_patch_input_graph_is_vector_independent_and_physically_verifies_footprints() {
    for footprint_parquet in [false, true] {
        let fixture = fixture_with_options(FixtureOptions {
            footprint_parquet,
            ..FixtureOptions::default()
        });
        let verified = fixture
            .producer
            .validate_cell_patch_input_artifact_graph(
                fixture.producer_record.id(),
                &fixture.expected_cells,
                &fixture.expected_patches,
                &fixture.context,
                &fixture.footprints,
                &fixture.link,
                &fixture.catalog,
                &fixture.store,
                budgets(),
            )
            .expect("verified cell-patch inputs");
        assert_eq!(verified.assignment_count(), 2);
        assert_eq!(verified.edge_count(), 1);
        assert_eq!(verified.logical_digest(), fixture.link.logical_digest());
        assert_eq!(
            verified.producer_artifact_id(),
            fixture.producer_record.id()
        );
        let debug = format!("{verified:?}");
        assert!(!debug.contains("input-slide"));
        assert!(!debug.contains("opaque-private-coordinates"));
        assert!(!debug.contains(&fixture.footprint_record.id().to_string()));
    }
}

#[test]
fn cell_patch_physical_receipts_pair_all_arrow_and_parquet_half_combinations() {
    let fixture = fixture();
    let graph = fixture
        .producer
        .validate_cell_patch_input_artifact_graph(
            fixture.producer_record.id(),
            &fixture.expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            &fixture.link,
            &fixture.catalog,
            &fixture.store,
            budgets(),
        )
        .expect("verified cell-patch inputs");
    let assignment_arrow =
        publish_cell_patch_assignment_table_arrow(&fixture.store, &fixture.link, budgets())
            .expect("publish assignment Arrow")
            .into_record();
    let assignment_parquet =
        publish_cell_patch_assignment_table_parquet(&fixture.store, &fixture.link, budgets())
            .expect("publish assignment Parquet")
            .into_record();
    let edge_arrow = publish_cell_patch_edge_table_arrow(&fixture.store, &fixture.link, budgets())
        .expect("publish edge Arrow")
        .into_record();
    let edge_parquet =
        publish_cell_patch_edge_table_parquet(&fixture.store, &fixture.link, budgets())
            .expect("publish edge Parquet")
            .into_record();

    let assignment_receipts = [
        verify_cell_patch_assignment_table_arrow_from_store(
            &fixture.store,
            &assignment_arrow,
            &fixture.link,
            graph,
            budgets(),
        )
        .expect("verify assignment Arrow"),
        verify_cell_patch_assignment_table_parquet_from_store(
            &fixture.store,
            &assignment_parquet,
            &fixture.link,
            graph,
            budgets(),
        )
        .expect("verify assignment Parquet"),
    ];
    let edge_receipts = [
        verify_cell_patch_edge_table_arrow_from_store(
            &fixture.store,
            &edge_arrow,
            &fixture.link,
            graph,
            budgets(),
        )
        .expect("verify edge Arrow"),
        verify_cell_patch_edge_table_parquet_from_store(
            &fixture.store,
            &edge_parquet,
            &fixture.link,
            graph,
            budgets(),
        )
        .expect("verify edge Parquet"),
    ];

    for assignment in assignment_receipts {
        assert_eq!(
            assignment.row_count(),
            fixture.link.assignment_count() as u64
        );
        assert_eq!(assignment.logical_digest(), fixture.link.logical_digest());
        for edge in edge_receipts {
            let verified = VerifiedCellPatchLinkArtifact::from_verified_halves(assignment, edge)
                .expect("pair format-independent halves");
            assert_eq!(verified.assignment_artifact_id(), assignment.artifact_id());
            assert_eq!(verified.edge_artifact_id(), edge.artifact_id());
            assert_eq!(verified.logical_digest(), fixture.link.logical_digest());
            assert_eq!(verified.assignment_count(), 2);
            assert_eq!(verified.edge_count(), 1);
        }
    }
    let debug = format!("{:?} {:?}", assignment_receipts[0], edge_receipts[0]);
    assert!(!debug.contains("input-slide"));
    assert!(!debug.contains(&assignment_arrow.id().to_string()));
    assert!(!debug.contains(&edge_arrow.id().to_string()));
}

#[test]
fn cell_patch_receipts_reject_different_links_and_wrong_graph_tokens() {
    let fixture = fixture();
    let graph = verified_graph(&fixture, &fixture.link);
    let alternate = alternate_link(&fixture);
    let alternate_graph = verified_graph(&fixture, &alternate);
    assert_ne!(fixture.link.logical_digest(), alternate.logical_digest());

    let assignment =
        publish_cell_patch_assignment_table_arrow(&fixture.store, &fixture.link, budgets())
            .expect("publish assignment")
            .into_record();
    let edge = publish_cell_patch_edge_table_parquet(&fixture.store, &alternate, budgets())
        .expect("publish alternate edge")
        .into_record();
    let assignment_receipt = verify_cell_patch_assignment_table_arrow_from_store(
        &fixture.store,
        &assignment,
        &fixture.link,
        graph,
        budgets(),
    )
    .expect("verify assignment");
    let edge_receipt = verify_cell_patch_edge_table_parquet_from_store(
        &fixture.store,
        &edge,
        &alternate,
        alternate_graph,
        budgets(),
    )
    .expect("verify alternate edge");
    assert_eq!(
        VerifiedCellPatchLinkArtifact::from_verified_halves(assignment_receipt, edge_receipt),
        Err(MultiscaleColumnarError::ArtifactBindingMismatch)
    );
    assert!(matches!(
        verify_cell_patch_assignment_table_arrow_from_store(
            &fixture.store,
            &assignment,
            &fixture.link,
            alternate_graph,
            budgets(),
        ),
        Err(VerifiedReaderError::Callback(
            MultiscaleColumnarError::ArtifactBindingMismatch
        ))
    ));
}

#[test]
fn cell_patch_managed_integrity_precedes_decode_and_malformed_callbacks_match_borrowed() {
    #[derive(Clone, Copy)]
    enum Profile {
        AssignmentArrow,
        EdgeArrow,
        AssignmentParquet,
        EdgeParquet,
    }

    let fixture = fixture();
    for profile in [
        Profile::AssignmentArrow,
        Profile::EdgeArrow,
        Profile::AssignmentParquet,
        Profile::EdgeParquet,
    ] {
        let canonical_record = match profile {
            Profile::AssignmentArrow => {
                publish_cell_patch_assignment_table_arrow(&fixture.store, &fixture.link, budgets())
            }
            Profile::EdgeArrow => {
                publish_cell_patch_edge_table_arrow(&fixture.store, &fixture.link, budgets())
            }
            Profile::AssignmentParquet => publish_cell_patch_assignment_table_parquet(
                &fixture.store,
                &fixture.link,
                budgets(),
            ),
            Profile::EdgeParquet => {
                publish_cell_patch_edge_table_parquet(&fixture.store, &fixture.link, budgets())
            }
        }
        .expect("publish canonical profile")
        .into_record();
        let mut malformed = Vec::new();
        match profile {
            Profile::AssignmentArrow => {
                write_cell_patch_assignment_table_arrow(&mut malformed, &fixture.link, budgets())
            }
            Profile::EdgeArrow => {
                write_cell_patch_edge_table_arrow(&mut malformed, &fixture.link, budgets())
            }
            Profile::AssignmentParquet => {
                write_cell_patch_assignment_table_parquet(&mut malformed, &fixture.link, budgets())
            }
            Profile::EdgeParquet => {
                write_cell_patch_edge_table_parquet(&mut malformed, &fixture.link, budgets())
            }
        }
        .expect("write canonical profile");
        let needle = match profile {
            Profile::AssignmentArrow | Profile::AssignmentParquet => b"cell-a".as_slice(),
            Profile::EdgeArrow | Profile::EdgeParquet => b"patch-a".as_slice(),
        };
        let row_at = malformed
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("physical row value");
        malformed[row_at] ^= 0x20;
        let draft = ArtifactDraft::new(
            ArtifactRef::from_bytes(canonical_record.content().kind(), &malformed)
                .expect("malformed content"),
            canonical_record.schema().clone(),
            canonical_record.table().cloned(),
            canonical_record.dependencies().to_vec(),
            Default::default(),
        )
        .expect("malformed draft");
        let malformed_record = fixture
            .store
            .publish_new_send(&draft, |output| output.write_all(&malformed))
            .expect("publish digest-matching malformed profile")
            .into_record();

        let borrowed = match profile {
            Profile::AssignmentArrow => validate_cell_patch_assignment_table_arrow_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeArrow => validate_cell_patch_edge_table_arrow_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::AssignmentParquet => validate_cell_patch_assignment_table_parquet_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeParquet => validate_cell_patch_edge_table_parquet_bytes(
                &malformed,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
        }
        .expect_err("borrowed malformed profile");
        let managed = match profile {
            Profile::AssignmentArrow => validate_cell_patch_assignment_table_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeArrow => validate_cell_patch_edge_table_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::AssignmentParquet => validate_cell_patch_assignment_table_parquet_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeParquet => validate_cell_patch_edge_table_parquet_from_store(
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
        let integrity = match profile {
            Profile::AssignmentArrow => validate_cell_patch_assignment_table_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeArrow => validate_cell_patch_edge_table_arrow_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::AssignmentParquet => validate_cell_patch_assignment_table_parquet_from_store(
                &fixture.store,
                &malformed_record,
                &fixture.link,
                budgets(),
            )
            .map(|_| ()),
            Profile::EdgeParquet => validate_cell_patch_edge_table_parquet_from_store(
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
fn cell_patch_input_graph_rejects_payload_dependency_alias_and_mode_drift() {
    let payload = fixture_with_options(FixtureOptions {
        expected_cell_payload_drift: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        payload.producer.validate_cell_patch_input_artifact_graph(
            payload.producer_record.id(),
            &payload.expected_cells,
            &payload.expected_patches,
            &payload.context,
            &payload.footprints,
            &payload.link,
            &payload.catalog,
            &payload.store,
            budgets(),
        ),
        Err(CellPatchInputArtifactGraphError::PayloadIdentityMismatch {
            role: CellPatchInputArtifactRole::ExpectedCells,
        })
    ));

    let dependencies = fixture_with_options(FixtureOptions {
        producer_dependency_drift: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        dependencies
            .producer
            .validate_cell_patch_input_artifact_graph(
                dependencies.producer_record.id(),
                &dependencies.expected_cells,
                &dependencies.expected_patches,
                &dependencies.context,
                &dependencies.footprints,
                &dependencies.link,
                &dependencies.catalog,
                &dependencies.store,
                budgets(),
            ),
        Err(CellPatchInputArtifactGraphError::DependencyMismatch {
            role: CellPatchInputArtifactRole::Producer,
        })
    ));

    let alias = fixture_with_options(FixtureOptions {
        alias_source_coordinates_with_expected_cells: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        alias.producer.validate_cell_patch_input_artifact_graph(
            alias.producer_record.id(),
            &alias.expected_cells,
            &alias.expected_patches,
            &alias.context,
            &alias.footprints,
            &alias.link,
            &alias.catalog,
            &alias.store,
            budgets(),
        ),
        Err(CellPatchInputArtifactGraphError::RoleAlias)
    ));

    let mode = fixture();
    let interpolation = CellPatchLinkProducer::declared_weighted_interpolation(
        "bilinear_declared",
        "1.0.0",
        mode.producer.source_coordinates_artifact_id(),
        mode.producer.run_config_artifact_id(),
        mode.producer.environment_artifact_id(),
        mode.producer.converter_artifact_id(),
        DOMAIN_BUDGET,
    )
    .expect("interpolation producer");
    assert!(matches!(
        interpolation.validate_cell_patch_input_artifact_graph(
            mode.producer_record.id(),
            &mode.expected_cells,
            &mode.expected_patches,
            &mode.context,
            &mode.footprints,
            &mode.link,
            &mode.catalog,
            &mode.store,
            budgets(),
        ),
        Err(CellPatchInputArtifactGraphError::DomainBindingMismatch {
            role: CellPatchInputArtifactRole::Producer,
        })
    ));
}

#[test]
fn cell_patch_input_graph_rejects_each_canonical_payload_and_footprint_record_drift() {
    for (fixture, role) in [
        (
            fixture_with_options(FixtureOptions {
                expected_patch_payload_drift: true,
                ..FixtureOptions::default()
            }),
            CellPatchInputArtifactRole::ExpectedPatches,
        ),
        (
            fixture_with_options(FixtureOptions {
                context_payload_drift: true,
                ..FixtureOptions::default()
            }),
            CellPatchInputArtifactRole::PatchContext,
        ),
        (
            fixture_with_options(FixtureOptions {
                producer_payload_drift: true,
                ..FixtureOptions::default()
            }),
            CellPatchInputArtifactRole::Producer,
        ),
    ] {
        assert_eq!(
            fixture.producer.validate_cell_patch_input_artifact_graph(
                fixture.producer_record.id(),
                &fixture.expected_cells,
                &fixture.expected_patches,
                &fixture.context,
                &fixture.footprints,
                &fixture.link,
                &fixture.catalog,
                &fixture.store,
                budgets(),
            ),
            Err(CellPatchInputArtifactGraphError::PayloadIdentityMismatch { role })
        );
    }

    let dependencies = fixture_with_options(FixtureOptions {
        footprint_dependency_drift: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        dependencies
            .producer
            .validate_cell_patch_input_artifact_graph(
                dependencies.producer_record.id(),
                &dependencies.expected_cells,
                &dependencies.expected_patches,
                &dependencies.context,
                &dependencies.footprints,
                &dependencies.link,
                &dependencies.catalog,
                &dependencies.store,
                budgets(),
            ),
        Err(CellPatchInputArtifactGraphError::DependencyMismatch {
            role: CellPatchInputArtifactRole::PatchFootprints,
        })
    );

    let profile = fixture_with_options(FixtureOptions {
        footprint_schema_drift: true,
        ..FixtureOptions::default()
    });
    assert_eq!(
        profile.producer.validate_cell_patch_input_artifact_graph(
            profile.producer_record.id(),
            &profile.expected_cells,
            &profile.expected_patches,
            &profile.context,
            &profile.footprints,
            &profile.link,
            &profile.catalog,
            &profile.store,
            budgets(),
        ),
        Err(CellPatchInputArtifactGraphError::SchemaMismatch {
            role: CellPatchInputArtifactRole::PatchFootprints,
        })
    );
}

#[test]
fn cell_patch_input_graph_requires_managed_evidence_and_preserves_integrity_precedence() {
    let catalog_only = fixture_with_options(FixtureOptions {
        source_coordinates_catalog_only: true,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        catalog_only
            .producer
            .validate_cell_patch_input_artifact_graph(
                catalog_only.producer_record.id(),
                &catalog_only.expected_cells,
                &catalog_only.expected_patches,
                &catalog_only.context,
                &catalog_only.footprints,
                &catalog_only.link,
                &catalog_only.catalog,
                &catalog_only.store,
                budgets(),
            ),
        Err(CellPatchInputArtifactGraphError::Unavailable {
            role: CellPatchInputArtifactRole::SourceCoordinates,
            ..
        })
    ));

    let corrupt = fixture();
    let artifact = corrupt.footprint_record.id().to_string();
    let path = corrupt
        ._root
        .path()
        .join("objects/sha256")
        .join(&artifact[..2])
        .join(&artifact);
    std::fs::write(
        &path,
        vec![0_u8; corrupt.footprint_record.content().byte_len() as usize],
    )
    .expect("corrupt temporary managed footprint");
    let error = corrupt
        .producer
        .validate_cell_patch_input_artifact_graph(
            corrupt.producer_record.id(),
            &corrupt.expected_cells,
            &corrupt.expected_patches,
            &corrupt.context,
            &corrupt.footprints,
            &corrupt.link,
            &corrupt.catalog,
            &corrupt.store,
            budgets(),
        )
        .expect_err("corrupt footprint must fail");
    assert!(matches!(
        error,
        CellPatchInputArtifactGraphError::Unavailable {
            role: CellPatchInputArtifactRole::PatchFootprints,
            reason: marklab::ArtifactAvailabilityFailure::Integrity,
        }
    ));
    let rendered = error.to_string();
    assert!(!rendered.contains("opaque-private-coordinates"));
    assert!(!rendered.contains(&path.display().to_string()));
    assert!(!rendered.contains(&artifact));
}
