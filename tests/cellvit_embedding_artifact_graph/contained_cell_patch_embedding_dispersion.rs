use std::mem::size_of;

use super::*;
use marklab::{
    contained_cell_patch_embedding_dispersion, contained_patch_binary_nucleus_area_contrast,
    declared_binary_group_nucleus_area_contrast, publish_cell_patch_assignment_table_arrow,
    publish_cell_patch_edge_table_arrow, publish_patch_footprint_set_arrow,
    verify_cell_patch_assignment_table_arrow_from_store,
    verify_cell_patch_edge_table_arrow_from_store, ArtifactDraft, BinaryMarkDeclaration,
    CellPatchAnchor, CellPatchContributor, CellPatchLink, CellPatchLinkBindings,
    CellPatchLinkProducer, ContainedCellPatchEmbeddingDispersionError,
    ContainedCellPatchEmbeddingDispersionStatus, ContainedPatchBinaryNucleusAreaContrastError,
    ContainedPatchBinaryNucleusAreaContrastStatus, DeclaredBinaryGroupNucleusAreaContrastError,
    DeclaredCellPatchAssignment, DeclaredScalarInputError, DeclaredScalarPatternInput,
    EffectiveReceptiveField, ExpectedPatchSet, MarklabProject, MeasurementStatus,
    NucleusAreaUm2MarkDeclaration, PatchEmbeddingContext, PatchFootprint, PatchFootprintSet,
    PatchId, Pattern, PatternMeta, ScalarMarkId, VerifiedCellPatchInputArtifactGraph,
    VerifiedCellPatchLinkArtifact,
};

const DIMENSION: u32 = 1_280;
const ASSIGNMENT_COUNT: usize = 3;
const EDGE_COUNT: usize = 4;
const COMPONENT_OPERATIONS: u64 = 3 * EDGE_COUNT as u64 * DIMENSION as u64;
const WORKING_BYTES: usize =
    EDGE_COUNT * size_of::<usize>() + DIMENSION as usize * size_of::<f64>();
const DOMAIN_BUDGET: usize = 8 * 1024 * 1024;

fn embedding_budgets() -> EmbeddingColumnarBudgets {
    EmbeddingColumnarBudgets::new(
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
        32 * 1024 * 1024,
    )
}

fn fixture_status(status: EmbeddingStatus) -> FixtureEmbeddingStatus {
    match status {
        EmbeddingStatus::Present => FixtureEmbeddingStatus::Present,
        EmbeddingStatus::MissingVector => FixtureEmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed => FixtureEmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected => FixtureEmbeddingStatus::QcRejected,
    }
}

fn verified_embedding(
    rows: Vec<(CellId, EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    let fixture = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        DIMENSION,
        rows.iter()
            .map(|(cell_id, status, _)| (cell_id.clone(), fixture_status(*status)))
            .collect(),
    );
    let domain_rows = rows
        .into_iter()
        .map(|(cell_id, status, vector)| match status {
            EmbeddingStatus::Present => {
                CellEmbeddingRow::present(cell_id, vector.expect("present vector"))
            }
            status => {
                assert!(vector.is_none());
                CellEmbeddingRow::non_present(cell_id, status).expect("non-present row")
            }
        })
        .collect();
    let graph = verified_graph(&fixture);
    let (table, bytes, embedding_record) =
        write_embedding_rows_arrow(&fixture, domain_rows, embedding_budgets());
    let embedding_receipt = verify_cell_embedding_table_arrow_bytes(
        &bytes,
        &embedding_record,
        &fixture.expected,
        &fixture.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("verified embedding table");
    let row_link_record = record_with_schema(&fixture, "marklab.cell_embedding_row_link");
    let row_link_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &fixture.store,
        row_link_record,
        &fixture.expected,
        &fixture.row_link,
        embedding_budgets(),
    )
    .expect("verified embedding row link");
    let artifact = CellEmbeddingArtifact::new(embedding_receipt, row_link_receipt, graph)
        .expect("verified embedding artifact");
    (fixture, table, artifact)
}

fn canonical_cells() -> Vec<CellId> {
    vec![cell("cell-a"), cell("cell-b"), cell("cell-c")]
}

fn available_rows() -> Vec<(CellId, EmbeddingStatus, Option<Vec<f32>>)> {
    canonical_cells()
        .into_iter()
        .zip([0.0, 2.0, 4.0])
        .map(|(cell_id, level)| (cell_id, EmbeddingStatus::Present, Some(level_vector(level))))
        .collect()
}

fn level_vector(level: f32) -> Vec<f32> {
    (0..DIMENSION)
        .map(|component| {
            if component.is_multiple_of(2) {
                level
            } else {
                0.0
            }
        })
        .collect()
}

struct LinkFixture {
    hierarchy: CohortHierarchy,
    expected_patches: ExpectedPatchSet,
    context: PatchEmbeddingContext,
    footprints: PatchFootprintSet,
    producer: CellPatchLinkProducer,
    producer_record: ArtifactRecord,
    catalog: ArtifactCatalog,
    bindings: CellPatchLinkBindings,
    link: CellPatchLink,
    graph: Option<VerifiedCellPatchInputArtifactGraph>,
    receipt: Option<VerifiedCellPatchLinkArtifact>,
}

impl LinkFixture {
    fn graph(&self) -> VerifiedCellPatchInputArtifactGraph {
        self.graph.expect("initialized graph")
    }

    fn receipt(&self) -> VerifiedCellPatchLinkArtifact {
        self.receipt.expect("initialized receipt")
    }
}

fn publish_domain_record(
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
        BTreeMap::new(),
        vec![ArtifactLocator::new(
            StoreId::new("fixture-source").expect("source store"),
            ArtifactKey::new(format!("fixtures/s11/{schema}")).expect("source key"),
            None,
        )
        .expect("source locator")],
    )
    .expect("artifact record");
    store
        .publish(&draft, |writer| writer.write_all(bytes))
        .expect("publish S11 artifact")
        .into_record()
}

fn link_hierarchy(cells: &[CellId], patches: &[PatchId], slide_id: &SlideId) -> CohortHierarchy {
    let patient = HierarchyId::from(PatientId::new("s11-patient").expect("patient ID"));
    let slide = HierarchyId::from(slide_id.clone());
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
    nodes.extend(cells.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    nodes.extend(patches.iter().cloned().map(|patch_id| {
        HierarchyNode::new(
            HierarchyId::from(patch_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("S11 hierarchy")
}

fn patch_context(hierarchy: &CohortHierarchy, slide_id: SlideId) -> PatchEmbeddingContext {
    let image_id = CoordinateFrameId::new("s11-image-pixels").expect("image frame");
    let physical_id = CoordinateFrameId::new("s11-slide-micrometers").expect("physical frame");
    let transform_id = TransformId::new("s11-pixel-to-micrometer").expect("transform");
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
    .expect("coordinate registry");
    PatchEmbeddingContext::new(
        hierarchy,
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
    .expect("patch context")
}

fn derive_contained_link(
    fixture: &LinkFixture,
    expected_cells: &ExpectedCellSet,
    bindings: &CellPatchLinkBindings,
    anchors: Vec<CellPatchAnchor>,
) -> CellPatchLink {
    CellPatchLink::derive_contained_shared(
        &fixture.hierarchy,
        expected_cells,
        &fixture.expected_patches,
        &fixture.context,
        &fixture.footprints,
        bindings,
        anchors,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("contained cell-patch link")
}

fn verify_link_graph(
    embedding: &Fixture,
    fixture: &LinkFixture,
    expected_cells: &ExpectedCellSet,
    link: &CellPatchLink,
) -> VerifiedCellPatchInputArtifactGraph {
    fixture
        .producer
        .validate_cell_patch_input_artifact_graph(
            fixture.producer_record.id(),
            expected_cells,
            &fixture.expected_patches,
            &fixture.context,
            &fixture.footprints,
            link,
            &fixture.catalog,
            &embedding.store,
            embedding_budgets(),
        )
        .expect("verified cell-patch graph")
}

fn verify_link_receipt(
    embedding: &Fixture,
    link: &CellPatchLink,
    graph: VerifiedCellPatchInputArtifactGraph,
) -> VerifiedCellPatchLinkArtifact {
    let assignment_record =
        publish_cell_patch_assignment_table_arrow(&embedding.store, link, embedding_budgets())
            .expect("publish assignment table")
            .into_record();
    let edge_record =
        publish_cell_patch_edge_table_arrow(&embedding.store, link, embedding_budgets())
            .expect("publish edge table")
            .into_record();
    let assignment = verify_cell_patch_assignment_table_arrow_from_store(
        &embedding.store,
        &assignment_record,
        link,
        graph,
        embedding_budgets(),
    )
    .expect("verified assignment table");
    let edge = verify_cell_patch_edge_table_arrow_from_store(
        &embedding.store,
        &edge_record,
        link,
        graph,
        embedding_budgets(),
    )
    .expect("verified edge table");
    VerifiedCellPatchLinkArtifact::from_verified_halves(assignment, edge)
        .expect("paired cell-patch receipt")
}

fn link_fixture(embedding: &Fixture) -> LinkFixture {
    let cells = embedding.expected.cells();
    let patches = [
        PatchId::new("patch-a").expect("patch ID"),
        PatchId::new("patch-b").expect("patch ID"),
    ];
    let slide_id = SlideId::new("s11-slide").expect("slide ID");
    let hierarchy = link_hierarchy(cells, &patches, &slide_id);
    let expected_patches = ExpectedPatchSet::new(
        &hierarchy,
        slide_id.clone(),
        "all_patches.v1",
        patches.to_vec(),
        DOMAIN_BUDGET,
    )
    .expect("expected patches");
    let expected_patch_record = publish_domain_record(
        &embedding.store,
        "marklab.expected_patch_set",
        "application/vnd.marklab.expected-patch-set.v1+json",
        &expected_patches
            .to_canonical_json()
            .expect("expected-patch JSON"),
        Vec::new(),
    );
    let context = patch_context(&hierarchy, slide_id);
    let context_record = publish_domain_record(
        &embedding.store,
        "marklab.patch_embedding_context",
        "application/vnd.marklab.patch-embedding-context.v1+json",
        &context.to_canonical_json().expect("context JSON"),
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
    .expect("patch footprints");
    let footprint_record = publish_patch_footprint_set_arrow(
        &embedding.store,
        &expected_patches,
        &context,
        &footprints,
        embedding_budgets(),
    )
    .expect("publish footprints")
    .into_record();
    let source_coordinates = publish_domain_record(
        &embedding.store,
        "marklab.cell_anchor_source_coordinates",
        "application/vnd.marklab.cell-anchor-source-coordinates.v1+binary",
        b"opaque-s11-cell-anchors",
        Vec::new(),
    );
    let run_config = publish_domain_record(
        &embedding.store,
        "marklab.embedding_run_config",
        "application/json",
        b"s11-run-config",
        Vec::new(),
    );
    let environment = publish_domain_record(
        &embedding.store,
        "marklab.execution_environment",
        "application/json",
        b"s11-environment",
        Vec::new(),
    );
    let converter = publish_domain_record(
        &embedding.store,
        "marklab.converter_manifest",
        "application/json",
        b"s11-converter",
        Vec::new(),
    );
    let producer = CellPatchLinkProducer::contained_shared(
        "1.0.0",
        source_coordinates.id(),
        run_config.id(),
        environment.id(),
        converter.id(),
        DOMAIN_BUDGET,
    )
    .expect("cell-patch producer");
    let producer_record = publish_domain_record(
        &embedding.store,
        "marklab.cell_patch_link_producer",
        "application/vnd.marklab.cell-patch-link-producer.v1+json",
        &producer.to_canonical_json().expect("producer JSON"),
        producer.direct_dependencies().collect(),
    );
    let expected_record = record_with_schema(embedding, "marklab.cell_embedding_expected_cells");
    let bindings = CellPatchLinkBindings::new(
        expected_record.id(),
        footprint_record.id(),
        producer_record.id(),
        producer_record.content().digest(),
        context.image_frame_id().clone(),
    );
    let link = CellPatchLink::derive_contained_shared(
        &hierarchy,
        &embedding.expected,
        &expected_patches,
        &context,
        &footprints,
        &bindings,
        vec![
            CellPatchAnchor::new(cells[0].clone(), [10.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("contained link");
    assert_eq!(link.assignment_count(), ASSIGNMENT_COUNT);
    assert_eq!(link.edge_count(), EDGE_COUNT);
    let catalog = ArtifactCatalog::from_records(
        embedding
            .catalog
            .iter()
            .map(|(_id, record)| record.clone())
            .chain([
                expected_patch_record,
                context_record,
                footprint_record,
                source_coordinates,
                run_config,
                environment,
                converter,
                producer_record.clone(),
            ]),
    )
    .expect("combined catalog");
    let mut shell = LinkFixture {
        hierarchy,
        expected_patches,
        context,
        footprints,
        producer,
        producer_record,
        catalog,
        bindings,
        link,
        graph: None,
        receipt: None,
    };
    let graph = verify_link_graph(embedding, &shell, &embedding.expected, &shell.link);
    let receipt = verify_link_receipt(embedding, &shell.link, graph);
    shell.graph = Some(graph);
    shell.receipt = Some(receipt);
    shell
}

fn run(
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    links: &LinkFixture,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_component_operations: u64,
    maximum_working_bytes: usize,
) -> Result<
    marklab::ContainedCellPatchEmbeddingDispersion,
    ContainedCellPatchEmbeddingDispersionError,
> {
    contained_cell_patch_embedding_dispersion(
        table,
        artifact,
        &links.link,
        links.graph(),
        links.receipt(),
        maximum_assignments,
        maximum_edges,
        maximum_component_operations,
        maximum_working_bytes,
    )
}

fn run_exact(
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    links: &LinkFixture,
) -> marklab::ContainedCellPatchEmbeddingDispersion {
    run(
        table,
        artifact,
        links,
        ASSIGNMENT_COUNT,
        EDGE_COUNT,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("contained cell-patch dispersion")
}

#[test]
fn contained_cell_patch_embedding_dispersion_matches_overlapping_oracle_and_identities() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let result = run_exact(&table, artifact, &links);
    assert_eq!(
        result.status(),
        ContainedCellPatchEmbeddingDispersionStatus::Available
    );
    assert_eq!(result.assignment_count(), 3);
    assert_eq!(result.edge_count(), 4);
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(result.eligible_incidence_count(), 4);
    assert_eq!(result.excluded_incidence_count(), 0);
    assert_eq!(result.dimension(), DIMENSION);
    assert_eq!(result.mean_squared_component_dispersion(), Some(0.5));
    assert_eq!(result.embedding_qc_summary(), artifact.qc_summary());
    assert_eq!(
        result.embedding_table_logical_digest(),
        artifact.logical_digest()
    );
    assert_eq!(
        result.expected_cells_artifact_id(),
        artifact.expected_cells_artifact_id()
    );
    assert_eq!(
        result.expected_cells_logical_digest(),
        artifact.expected_cells_logical_digest()
    );
    assert_eq!(
        result.embedding_artifact_id(),
        artifact.embedding_artifact_id()
    );
    assert_eq!(
        result.row_link_artifact_id(),
        artifact.row_link_artifact_id()
    );
    assert_eq!(
        result.embedding_provenance_artifact_id(),
        artifact.provenance_artifact_id()
    );
    assert_eq!(
        result.cell_patch_logical_digest(),
        links.link.logical_digest()
    );
    assert_eq!(
        result.cell_patch_assignment_artifact_id(),
        links.receipt().assignment_artifact_id()
    );
    assert_eq!(
        result.cell_patch_edge_artifact_id(),
        links.receipt().edge_artifact_id()
    );
    assert_eq!(
        result.expected_patches_artifact_id(),
        links.link.expected_patches_artifact_id()
    );
    assert_eq!(
        result.patch_context_artifact_id(),
        links.link.patch_context_artifact_id()
    );
    assert_eq!(
        result.patch_footprints_artifact_id(),
        links.link.patch_footprints_artifact_id()
    );
    assert_eq!(
        result.cell_patch_producer_artifact_id(),
        links.link.producer_artifact_id()
    );
    let repeated = run_exact(&table, artifact, &links);
    assert_eq!(repeated, result);
    assert_eq!(
        repeated
            .mean_squared_component_dispersion()
            .expect("dispersion")
            .to_bits(),
        0.5_f64.to_bits()
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_changes_with_vectors_and_canonicalizes_zero() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let baseline = run_exact(&table, artifact, &links);

    let mut changed_rows = available_rows();
    changed_rows[2].2 = Some(level_vector(6.0));
    let (changed_embedding, changed_table, changed_artifact) = verified_embedding(changed_rows);
    let changed_links = link_fixture(&changed_embedding);
    let changed = run_exact(&changed_table, changed_artifact, &changed_links);
    assert_ne!(
        changed.mean_squared_component_dispersion(),
        baseline.mean_squared_component_dispersion()
    );
    assert_eq!(
        changed.cell_patch_logical_digest(),
        changed_links.link.logical_digest()
    );
    assert_ne!(
        changed.embedding_table_logical_digest(),
        baseline.embedding_table_logical_digest()
    );

    let zero_rows = canonical_cells()
        .into_iter()
        .map(|cell_id| {
            let values = (0..DIMENSION)
                .map(|component| {
                    if component.is_multiple_of(2) {
                        0.0
                    } else {
                        -0.0
                    }
                })
                .collect();
            (cell_id, EmbeddingStatus::Present, Some(values))
        })
        .collect();
    let (zero_embedding, zero_table, zero_artifact) = verified_embedding(zero_rows);
    let zero_links = link_fixture(&zero_embedding);
    let zero = run_exact(&zero_table, zero_artifact, &zero_links);
    let value = zero
        .mean_squared_component_dispersion()
        .expect("zero dispersion");
    assert_eq!(value.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn contained_cell_patch_embedding_dispersion_excludes_every_non_present_status() {
    for status in [
        EmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected,
    ] {
        let mut rows = available_rows();
        rows[0] = (rows[0].0.clone(), status, None);
        let (embedding, table, artifact) = verified_embedding(rows);
        let links = link_fixture(&embedding);
        let result = run_exact(&table, artifact, &links);
        assert_eq!(
            result.status(),
            ContainedCellPatchEmbeddingDispersionStatus::Available
        );
        assert_eq!(result.eligible_patch_count(), 1);
        assert_eq!(result.eligible_incidence_count(), 2);
        assert_eq!(result.excluded_incidence_count(), 2);
        assert_eq!(result.mean_squared_component_dispersion(), Some(0.5));
        let qc = result.embedding_qc_summary();
        assert_eq!(qc.present_count(), 2);
        match status {
            EmbeddingStatus::MissingVector => assert_eq!(qc.missing_vector_count(), 1),
            EmbeddingStatus::ExtractionFailed => assert_eq!(qc.extraction_failed_count(), 1),
            EmbeddingStatus::QcRejected => assert_eq!(qc.qc_rejected_count(), 1),
            EmbeddingStatus::Present => unreachable!(),
        }
    }

    let mut singleton_rows = available_rows();
    singleton_rows[1] = (
        singleton_rows[1].0.clone(),
        EmbeddingStatus::MissingVector,
        None,
    );
    let (embedding, table, artifact) = verified_embedding(singleton_rows);
    let links = link_fixture(&embedding);
    let result = run_exact(&table, artifact, &links);
    assert_eq!(
        result.status(),
        ContainedCellPatchEmbeddingDispersionStatus::InsufficientEligiblePatches
    );
    assert_eq!(result.eligible_patch_count(), 0);
    assert_eq!(result.eligible_incidence_count(), 0);
    assert_eq!(result.excluded_incidence_count(), 4);
    assert_eq!(result.mean_squared_component_dispersion(), None);
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_interpolation_before_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let cells = embedding.expected.cells();
    let patches = links.expected_patches.ids();
    let interpolation = CellPatchLink::from_declared_weighted_interpolation(
        &links.hierarchy,
        &embedding.expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[0].clone(), [10.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[0].clone(), 1, 1).expect("weight")],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
                vec![
                    CellPatchContributor::new(patches[0].clone(), 1, 2).expect("weight"),
                    CellPatchContributor::new(patches[1].clone(), 1, 2).expect("weight"),
                ],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[1].clone(), 1, 1).expect("weight")],
            ),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("interpolation link");
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &interpolation,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::UnsupportedAssignmentMode)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_artifact_and_expected_cell_drift() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let mut changed_rows = available_rows();
    changed_rows[0].2 = Some(level_vector(8.0));
    let (_other_embedding, _other_table, other_artifact) = verified_embedding(changed_rows);
    assert_eq!(
        run(&table, other_artifact, &links, 0, 0, 0, 0),
        Err(ContainedCellPatchEmbeddingDispersionError::EmbeddingArtifactBindingMismatch)
    );

    let drift_record = publish_domain_record(
        &embedding.store,
        "marklab.s11_expected_cell_binding_drift",
        "application/octet-stream",
        b"expected-cell-binding-drift",
        Vec::new(),
    );
    let id_drift_bindings = CellPatchLinkBindings::new(
        drift_record.id(),
        links.link.patch_footprints_artifact_id(),
        links.link.producer_artifact_id(),
        links.link.producer_content_digest(),
        links.context.image_frame_id().clone(),
    );
    let anchors = vec![
        CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0]).expect("anchor"),
        CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0]).expect("anchor"),
        CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0]).expect("anchor"),
    ];
    let id_drift = derive_contained_link(
        &links,
        &embedding.expected,
        &id_drift_bindings,
        anchors.clone(),
    );
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &id_drift,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );

    let logical_drift_expected =
        ExpectedCellSet::new("logical_drift.v1", embedding.expected.cells().to_vec())
            .expect("drifted expected cells");
    let logical_drift =
        derive_contained_link(&links, &logical_drift_expected, &links.bindings, anchors);
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &logical_drift,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_rejects_cell_graph_and_receipt_drift_before_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let alternate_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [20.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0])
                .expect("anchor"),
        ],
    );
    let alternate_graph =
        verify_link_graph(&embedding, &links, &embedding.expected, &alternate_link);
    let alternate_receipt = verify_link_receipt(&embedding, &alternate_link, alternate_graph);
    assert_ne!(alternate_link.logical_digest(), links.link.logical_digest());

    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &links.link,
            alternate_graph,
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::CellPatchGraphBindingMismatch)
    );
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &links.link,
            links.graph(),
            alternate_receipt,
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::CellPatchReceiptBindingMismatch)
    );

    let different_cells = vec![cell("cell-a"), cell("cell-b"), cell("cell-d")];
    let different_expected =
        ExpectedCellSet::new("all.v1", different_cells.clone()).expect("different expected cells");
    let different_hierarchy = link_hierarchy(
        &different_cells,
        links.expected_patches.ids(),
        links.context.owning_slide_id(),
    );
    let different_link = CellPatchLink::derive_contained_shared(
        &different_hierarchy,
        &different_expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            CellPatchAnchor::new(different_cells[0].clone(), [10.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("ordered CellId drift link");
    assert_eq!(
        contained_cell_patch_embedding_dispersion(
            &table,
            artifact,
            &different_link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(ContainedCellPatchEmbeddingDispersionError::ExpectedCellBindingMismatch)
    );
}

#[test]
fn contained_cell_patch_embedding_dispersion_enforces_exact_limits() {
    let (embedding, table, artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    run_exact(&table, artifact, &links);

    for (assignments, edges, operations, bytes, expected) in [
        (
            ASSIGNMENT_COUNT - 1,
            EDGE_COUNT,
            COMPONENT_OPERATIONS,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::AssignmentCountBudgetExceeded {
                required: ASSIGNMENT_COUNT,
                maximum: ASSIGNMENT_COUNT - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT - 1,
            COMPONENT_OPERATIONS,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::EdgeCountBudgetExceeded {
                required: EDGE_COUNT,
                maximum: EDGE_COUNT - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT,
            COMPONENT_OPERATIONS - 1,
            WORKING_BYTES,
            ContainedCellPatchEmbeddingDispersionError::ComponentOperationBudgetExceeded {
                required: COMPONENT_OPERATIONS,
                maximum: COMPONENT_OPERATIONS - 1,
            },
        ),
        (
            ASSIGNMENT_COUNT,
            EDGE_COUNT,
            COMPONENT_OPERATIONS,
            WORKING_BYTES - 1,
            ContainedCellPatchEmbeddingDispersionError::WorkingByteBudgetExceeded {
                required: WORKING_BYTES,
                maximum: WORKING_BYTES - 1,
            },
        ),
    ] {
        assert_eq!(
            run(
                &table,
                artifact,
                &links,
                assignments,
                edges,
                operations,
                bytes
            ),
            Err(expected)
        );
    }
}

const S13_UNEQUAL_EDGE_COUNT: usize = 5;
const S13_UNEQUAL_WORKING_BYTES: usize = S13_UNEQUAL_EDGE_COUNT * size_of::<usize>();

struct S13ScalarFixture {
    project: MarklabProject,
    pattern: Pattern,
    cell_ids: Vec<CellId>,
    slide_id: SlideId,
    frame_id: CoordinateFrameId,
    binary_mark: BinaryMarkDeclaration,
    nucleus_area_mark: NucleusAreaUm2MarkDeclaration,
}

fn s13_measurement_status_name(status: MeasurementStatus) -> &'static str {
    match status {
        MeasurementStatus::Measured => "measured",
        MeasurementStatus::ImportedPrediction => "imported_prediction",
        MeasurementStatus::MorphologyPrediction => "morphology_prediction",
        MeasurementStatus::DerivedSummary => "derived_summary",
    }
}

fn s13_binary_metadata() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), "mmr_loss".into()),
        ("mark_label".into(), "MMR loss".into()),
        (
            "measurement_status".into(),
            s13_measurement_status_name(MeasurementStatus::Measured).into(),
        ),
        ("origin".into(), "independent".into()),
        ("unit".into(), "unitless".into()),
        ("value_kind".into(), "binary".into()),
    ])
}

fn s13_nucleus_area_metadata() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mark_id".into(), "nucleus_area_um2".into()),
        ("mark_label".into(), "Nucleus area".into()),
        (
            "measurement_status".into(),
            s13_measurement_status_name(MeasurementStatus::Measured).into(),
        ),
        ("modality".into(), "morphology".into()),
        ("unit".into(), "square_micrometer".into()),
        ("value_kind".into(), "continuous".into()),
    ])
}

fn publish_s13_scalar_record(
    embedding: &Fixture,
    label: &[u8],
    metadata: BTreeMap<String, String>,
) -> ArtifactRecord {
    let draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", label).expect("scalar content"),
        ArtifactSchema::new("marklab.scalar_mark_provenance", 1).expect("scalar schema"),
        None,
        Vec::new(),
        metadata,
    )
    .expect("scalar artifact draft");
    embedding
        .store
        .publish_new_send(&draft, |writer| writer.write_all(label))
        .expect("publish scalar provenance")
        .into_record()
}

fn s13_scalar_hierarchy(cell_ids: &[CellId], slide_id: &SlideId) -> CohortHierarchy {
    let patient = HierarchyId::from(
        PatientId::new(format!("{}-patient", slide_id.as_str())).expect("patient ID"),
    );
    let slide = HierarchyId::from(slide_id.clone());
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
    nodes.extend(cell_ids.iter().cloned().map(|cell_id| {
        HierarchyNode::new(
            HierarchyId::from(cell_id),
            Some(slide.clone()),
            ReplicationRole::Structural,
        )
    }));
    CohortHierarchy::new(nodes, Vec::new()).expect("scalar hierarchy")
}

fn s13_scalar_fixture(
    embedding: &Fixture,
    slide_id: SlideId,
    marks: [u8; 3],
    areas: [f32; 3],
) -> S13ScalarFixture {
    let cell_ids = embedding.expected.cells().to_vec();
    let frame_id = CoordinateFrameId::new("s13-physical-xy").expect("frame ID");
    let hierarchy = s13_scalar_hierarchy(&cell_ids, &slide_id);
    let registry = CoordinateRegistry::new(
        vec![CoordinateFrame::new(
            frame_id.clone(),
            vec![SpatialAxis::X, SpatialAxis::Y],
            CoordinateUnit::Micrometer,
            CoordinateSpace::Physical,
        )
        .expect("physical frame")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("scalar coordinate registry");
    let mut project = MarklabProject::new();
    project
        .install_hierarchy(hierarchy)
        .expect("install scalar hierarchy");
    project
        .install_coordinate_registry(registry)
        .expect("install scalar registry");

    let binary_record = publish_s13_scalar_record(
        embedding,
        format!("s13-binary-{}", slide_id.as_str()).as_bytes(),
        s13_binary_metadata(),
    );
    let nucleus_record = publish_s13_scalar_record(
        embedding,
        format!("s13-nucleus-{}", slide_id.as_str()).as_bytes(),
        s13_nucleus_area_metadata(),
    );
    let binary_artifact_id = binary_record.id();
    let nucleus_artifact_id = nucleus_record.id();
    project
        .register_artifact(binary_record)
        .expect("register binary provenance");
    project
        .register_artifact(nucleus_record)
        .expect("register nucleus provenance");

    let binary_mark = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_artifact_id,
    )
    .expect("binary declaration");
    let nucleus_area_mark =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, nucleus_artifact_id)
            .expect("nucleus-area declaration");
    let mut pattern = Pattern::from_arrays(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 0.0, 0.0],
        marks.to_vec(),
        PatternMeta {
            case_id: "s13-case".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: Some(slide_id.as_str().to_owned()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("scalar pattern");
    pattern.nucleus_area_um2 = Some(areas.to_vec().into_boxed_slice());
    S13ScalarFixture {
        project,
        pattern,
        cell_ids,
        slide_id,
        frame_id,
        binary_mark,
        nucleus_area_mark,
    }
}

fn s13_input(fixture: &S13ScalarFixture) -> DeclaredScalarPatternInput<'_> {
    DeclaredScalarPatternInput::new(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary_mark.clone(),
        None,
        16 * 1024,
        fixture
            .cell_ids
            .iter()
            .map(|cell_id| cell_id.as_str().len())
            .sum(),
    )
    .expect("declared scalar input")
}

fn run_s13(
    scalar: &S13ScalarFixture,
    input: &DeclaredScalarPatternInput<'_>,
    links: &LinkFixture,
    maximum_rows: usize,
    maximum_assignments: usize,
    maximum_edges: usize,
    maximum_working_bytes: usize,
) -> Result<
    marklab::ContainedPatchBinaryNucleusAreaContrast,
    ContainedPatchBinaryNucleusAreaContrastError,
> {
    contained_patch_binary_nucleus_area_contrast(
        &scalar.project,
        input,
        scalar.nucleus_area_mark.clone(),
        &links.link,
        links.graph(),
        links.receipt(),
        maximum_rows,
        maximum_assignments,
        maximum_edges,
        maximum_working_bytes,
    )
}

fn run_s13_exact(
    scalar: &S13ScalarFixture,
    input: &DeclaredScalarPatternInput<'_>,
    links: &LinkFixture,
) -> marklab::ContainedPatchBinaryNucleusAreaContrast {
    let edge_count = links.link.edge_count();
    run_s13(
        scalar,
        input,
        links,
        3,
        links.link.assignment_count(),
        edge_count,
        edge_count * size_of::<usize>(),
    )
    .expect("contained-patch nucleus-area contrast")
}

#[test]
fn contained_patch_binary_nucleus_area_matches_equal_patch_oracle_and_identities() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let base_links = link_fixture(&embedding);
    let unequal_link = derive_contained_link(
        &base_links,
        &embedding.expected,
        &base_links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [210.0, 10.0])
                .expect("anchor"),
        ],
    );
    assert_eq!(unequal_link.edge_count(), S13_UNEQUAL_EDGE_COUNT);
    let graph = verify_link_graph(&embedding, &base_links, &embedding.expected, &unequal_link);
    let receipt = verify_link_receipt(&embedding, &unequal_link, graph);
    let links = LinkFixture {
        link: unequal_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..base_links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let whole = declared_binary_group_nucleus_area_contrast(
        &scalar.project,
        &input,
        scalar.nucleus_area_mark.clone(),
        3,
    )
    .expect("whole-input contrast");
    let result = run_s13_exact(&scalar, &input, &links);
    assert_eq!(
        result.status(),
        ContainedPatchBinaryNucleusAreaContrastStatus::Available
    );
    assert_eq!(result.assignment_count(), 3);
    assert_eq!(result.edge_count(), 5);
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(result.eligible_marked_incidence_count(), 2);
    assert_eq!(result.eligible_unmarked_incidence_count(), 3);
    assert_eq!(result.eligible_incidence_count(), 5);
    assert_eq!(result.excluded_incidence_count(), 0);
    let whole_input_difference = whole
        .marked_minus_unmarked_mean_nucleus_area_um2()
        .expect("whole-input difference");
    let patch_incidence_weighted_difference = (8.0_f64 * 3.0 + 6.0 * 2.0) / 5.0;
    assert_eq!(whole_input_difference, 8.0);
    assert_eq!(patch_incidence_weighted_difference, 7.2);
    assert_eq!(
        result.equal_patch_mean_marked_minus_unmarked_nucleus_area_um2(),
        Some(7.0)
    );
    assert_ne!(7.0_f64.to_bits(), whole_input_difference.to_bits());
    assert_ne!(
        7.0_f64.to_bits(),
        patch_incidence_weighted_difference.to_bits()
    );
    assert_ne!(
        whole_input_difference.to_bits(),
        patch_incidence_weighted_difference.to_bits()
    );
    assert_eq!(
        result.whole_input_contrast().paired_values_logical_digest(),
        whole.paired_values_logical_digest()
    );
    assert_eq!(
        result.whole_input_contrast().scalar_identity(),
        whole.scalar_identity()
    );
    assert_eq!(
        result.whole_input_contrast().binary_mark(),
        whole.binary_mark()
    );
    assert_eq!(
        result.whole_input_contrast().nucleus_area_mark(),
        whole.nucleus_area_mark()
    );
    assert_eq!(
        result.cell_patch_logical_digest(),
        links.link.logical_digest()
    );
    assert_eq!(
        result.expected_cells_artifact_id(),
        links.link.expected_cells_artifact_id()
    );
    assert_eq!(
        result.expected_patches_artifact_id(),
        links.link.expected_patches_artifact_id()
    );
    assert_eq!(
        result.patch_context_artifact_id(),
        links.link.patch_context_artifact_id()
    );
    assert_eq!(
        result.patch_footprints_artifact_id(),
        links.link.patch_footprints_artifact_id()
    );
    assert_eq!(
        result.cell_patch_producer_artifact_id(),
        links.link.producer_artifact_id()
    );
    assert_eq!(
        result.cell_patch_assignment_artifact_id(),
        links.receipt().assignment_artifact_id()
    );
    assert_eq!(
        result.cell_patch_edge_artifact_id(),
        links.receipt().edge_artifact_id()
    );

    let repeated = run_s13_exact(&scalar, &input, &links);
    assert_eq!(
        repeated
            .equal_patch_mean_marked_minus_unmarked_nucleus_area_um2()
            .expect("contrast")
            .to_bits(),
        7.0_f64.to_bits()
    );
    assert_eq!(
        repeated.whole_input_contrast(),
        result.whole_input_contrast()
    );
}

#[test]
fn contained_patch_binary_nucleus_area_canonicalizes_equal_patch_cancellation_to_positive_zero() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 30.0],
    );
    let input = s13_input(&scalar);
    let result = run_s13_exact(&scalar, &input, &links);
    assert_eq!(result.eligible_patch_count(), 2);
    assert_eq!(
        result
            .equal_patch_mean_marked_minus_unmarked_nucleus_area_um2()
            .expect("zero contrast")
            .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn contained_patch_binary_nucleus_area_types_global_and_patch_local_single_group_states() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    for marks in [[0, 0, 0], [1, 1, 1]] {
        let scalar = s13_scalar_fixture(
            &embedding,
            links.context.owning_slide_id().clone(),
            marks,
            [10.0, 20.0, 14.0],
        );
        let input = s13_input(&scalar);
        let result = run_s13_exact(&scalar, &input, &links);
        assert_eq!(
            result.status(),
            ContainedPatchBinaryNucleusAreaContrastStatus::InsufficientEligiblePatches
        );
        assert_eq!(result.eligible_patch_count(), 0);
        assert_eq!(result.eligible_incidence_count(), 0);
        assert_eq!(result.excluded_incidence_count(), 4);
        assert_eq!(
            result.equal_patch_mean_marked_minus_unmarked_nucleus_area_um2(),
            None
        );
    }

    let single_group_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [20.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [400.0, 10.0])
                .expect("anchor"),
        ],
    );
    let graph = verify_link_graph(&embedding, &links, &embedding.expected, &single_group_link);
    let receipt = verify_link_receipt(&embedding, &single_group_link, graph);
    let single_group_links = LinkFixture {
        link: single_group_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        single_group_links.context.owning_slide_id().clone(),
        [0, 0, 1],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let result = run_s13(
        &scalar,
        &input,
        &single_group_links,
        3,
        3,
        3,
        3 * size_of::<usize>(),
    )
    .expect("patch-local single groups");
    assert_eq!(
        result.whole_input_contrast().status(),
        marklab::DeclaredBinaryGroupNucleusAreaContrastStatus::Available
    );
    assert_eq!(
        result.status(),
        ContainedPatchBinaryNucleusAreaContrastStatus::InsufficientEligiblePatches
    );
    assert_eq!(result.represented_patch_count(), 2);
    assert_eq!(result.excluded_incidence_count(), 3);
}

#[test]
fn contained_patch_binary_nucleus_area_preserves_s12_failure_precedence() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [f32::NAN, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &MarklabProject::new(),
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
                    DeclaredScalarInputError::CoordinateRegistryMissing
                )
            )
        )
    ));
    assert!(matches!(
        run_s13(&scalar, &input, &links, 2, 0, 0, 0),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::RowCountBudgetExceeded {
                    required: 3,
                    maximum: 2,
                }
            )
        )
    ));
    assert!(matches!(
        run_s13(&scalar, &input, &links, 3, 0, 0, 0),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::InvalidNucleusAreaValue { row: 0 }
            )
        )
    ));

    let absent_record = publish_s13_scalar_record(
        &embedding,
        b"s13-absent-nucleus-provenance",
        s13_nucleus_area_metadata(),
    );
    let absent_nucleus =
        NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, absent_record.id())
            .expect("absent nucleus declaration");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            absent_nucleus,
            &links.link,
            links.graph(),
            links.receipt(),
            0,
            0,
            0,
            0,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WholeInputContrast(
                DeclaredBinaryGroupNucleusAreaContrastError::ScalarInput(
                    DeclaredScalarInputError::ProvenanceRecordMissing { artifact }
                )
            )
        ) if artifact == absent_record.id()
    ));
}

#[test]
fn contained_patch_binary_nucleus_area_rejects_mode_slide_cell_graph_and_receipt_drift() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let links = link_fixture(&embedding);
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    let cells = embedding.expected.cells();
    let patches = links.expected_patches.ids();
    let interpolation = CellPatchLink::from_declared_weighted_interpolation(
        &links.hierarchy,
        &embedding.expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[0].clone(), [10.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[0].clone(), 1, 1).expect("weight")],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
                vec![
                    CellPatchContributor::new(patches[0].clone(), 1, 2).expect("weight"),
                    CellPatchContributor::new(patches[1].clone(), 1, 2).expect("weight"),
                ],
            ),
            DeclaredCellPatchAssignment::new(
                CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
                vec![CellPatchContributor::new(patches[1].clone(), 1, 1).expect("weight")],
            ),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("interpolation link");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &interpolation,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::UnsupportedAssignmentMode)
    ));

    let foreign_scalar = s13_scalar_fixture(
        &embedding,
        SlideId::new("s13-foreign-slide").expect("foreign slide"),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let foreign_input = s13_input(&foreign_scalar);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &foreign_scalar.project,
            &foreign_input,
            foreign_scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::OwningSlideBindingMismatch)
    ));

    let different_cells = vec![cell("cell-a"), cell("cell-b"), cell("cell-d")];
    let different_expected =
        ExpectedCellSet::new("all.v1", different_cells.clone()).expect("different cells");
    let different_hierarchy = link_hierarchy(
        &different_cells,
        links.expected_patches.ids(),
        links.context.owning_slide_id(),
    );
    let different_link = CellPatchLink::derive_contained_shared(
        &different_hierarchy,
        &different_expected,
        &links.expected_patches,
        &links.context,
        &links.footprints,
        &links.bindings,
        vec![
            CellPatchAnchor::new(different_cells[0].clone(), [10.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(different_cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
        DOMAIN_BUDGET,
    )
    .expect("different-cell link");
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &different_link,
            links.graph(),
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellIdBindingMismatch { row: 2 })
    ));

    let alternate_link = derive_contained_link(
        &links,
        &embedding.expected,
        &links.bindings,
        vec![
            CellPatchAnchor::new(cells[0].clone(), [20.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[1].clone(), [200.0, 10.0]).expect("anchor"),
            CellPatchAnchor::new(cells[2].clone(), [400.0, 10.0]).expect("anchor"),
        ],
    );
    let alternate_graph =
        verify_link_graph(&embedding, &links, &embedding.expected, &alternate_link);
    let alternate_receipt = verify_link_receipt(&embedding, &alternate_link, alternate_graph);
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            alternate_graph,
            links.receipt(),
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchGraphBindingMismatch)
    ));
    assert!(matches!(
        contained_patch_binary_nucleus_area_contrast(
            &scalar.project,
            &input,
            scalar.nucleus_area_mark.clone(),
            &links.link,
            links.graph(),
            alternate_receipt,
            3,
            0,
            0,
            0,
        ),
        Err(ContainedPatchBinaryNucleusAreaContrastError::CellPatchReceiptBindingMismatch)
    ));
}

#[test]
fn contained_patch_binary_nucleus_area_enforces_exact_limits() {
    let (embedding, _table, _artifact) = verified_embedding(available_rows());
    let base_links = link_fixture(&embedding);
    let unequal_link = derive_contained_link(
        &base_links,
        &embedding.expected,
        &base_links.bindings,
        vec![
            CellPatchAnchor::new(embedding.expected.cells()[0].clone(), [10.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[1].clone(), [200.0, 10.0])
                .expect("anchor"),
            CellPatchAnchor::new(embedding.expected.cells()[2].clone(), [210.0, 10.0])
                .expect("anchor"),
        ],
    );
    let graph = verify_link_graph(&embedding, &base_links, &embedding.expected, &unequal_link);
    let receipt = verify_link_receipt(&embedding, &unequal_link, graph);
    let links = LinkFixture {
        link: unequal_link,
        graph: Some(graph),
        receipt: Some(receipt),
        ..base_links
    };
    let scalar = s13_scalar_fixture(
        &embedding,
        links.context.owning_slide_id().clone(),
        [0, 1, 0],
        [10.0, 20.0, 14.0],
    );
    let input = s13_input(&scalar);
    run_s13_exact(&scalar, &input, &links);
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            2,
            S13_UNEQUAL_EDGE_COUNT,
            S13_UNEQUAL_WORKING_BYTES,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::AssignmentCountBudgetExceeded {
                required: 3,
                maximum: 2,
            }
        )
    ));
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            3,
            S13_UNEQUAL_EDGE_COUNT - 1,
            S13_UNEQUAL_WORKING_BYTES,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::EdgeCountBudgetExceeded {
                required: S13_UNEQUAL_EDGE_COUNT,
                maximum,
            }
        ) if maximum == S13_UNEQUAL_EDGE_COUNT - 1
    ));
    assert!(matches!(
        run_s13(
            &scalar,
            &input,
            &links,
            3,
            3,
            S13_UNEQUAL_EDGE_COUNT,
            S13_UNEQUAL_WORKING_BYTES - 1,
        ),
        Err(
            ContainedPatchBinaryNucleusAreaContrastError::WorkingByteBudgetExceeded {
                required: S13_UNEQUAL_WORKING_BYTES,
                maximum,
            }
        ) if maximum == S13_UNEQUAL_WORKING_BYTES - 1
    ));
}
