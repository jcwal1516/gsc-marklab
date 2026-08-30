use std::mem::size_of;

use super::declared_embedding_support as embedding_support;
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

fn verified_embedding(
    rows: Vec<(CellId, EmbeddingStatus, Option<Vec<f32>>)>,
) -> (Fixture, CellEmbeddingTable, CellEmbeddingArtifact) {
    let (cell_ids, rows) = rows
        .into_iter()
        .map(|(cell_id, status, vector)| (cell_id, (status, vector)))
        .unzip::<_, _, Vec<_>, Vec<_>>();
    embedding_support::verified_embedding(&cell_ids, DIMENSION, rows)
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

#[path = "contained_cell_patch_embedding_dispersion/embedding_dispersion.rs"]
mod embedding_dispersion;
#[path = "contained_cell_patch_embedding_dispersion/nucleus_area.rs"]
mod nucleus_area;
