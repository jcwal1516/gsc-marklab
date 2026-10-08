use std::collections::BTreeSet;

use marklab::{
    declared_binary_cell_embedding_centroid_discrepancy, publish_cell_embedding_table_arrow,
    verify_cell_embedding_table_arrow_from_store, ArtifactDraft, ArtifactId, BinaryMarkDeclaration,
    CacheStatus, DeclaredBinaryCellEmbeddingCentroidDiscrepancy,
    DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus, DeclaredBinaryCellEmbeddingCentroidNode,
    DeclaredScalarPatternInput, LocalArtifactStore, LocalScheduler, MarkTable, MarklabProject,
    MeasurementStatus, MissingnessPolicy, NodeError, NodeId, NodeRun, Pattern, PatternMeta,
    ProbabilityMarkDeclaration, ProbabilityThresholdComparator, ProjectError, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, SchedulerLimits,
    VectorArtifactRefMarkDeclaration, WorkflowError, WorkflowGraph, WorkflowNode,
};

use super::*;

const DIMENSION: u32 = 1_280;
const COMPONENT_OPERATIONS: u64 = 6_400;
const WORKING_BYTES: usize = 20_480;
const CODEC_BYTES: usize = 18;

struct WorkflowFixture {
    embedding: Fixture,
    project: MarklabProject,
    pattern: Pattern,
    cell_ids: Vec<CellId>,
    slide_id: SlideId,
    frame_id: CoordinateFrameId,
    binary: BinaryMarkDeclaration,
    binary_record_id: marklab::ArtifactId,
    table: CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
}

fn alternating(even: f32, odd: f32) -> Vec<f32> {
    (0..DIMENSION)
        .map(|index| if index.is_multiple_of(2) { even } else { odd })
        .collect()
}

fn available_rows() -> Vec<(EmbeddingStatus, Option<Vec<f32>>)> {
    vec![
        (
            EmbeddingStatus::Present,
            Some(vec![0.0; DIMENSION as usize]),
        ),
        (EmbeddingStatus::Present, Some(alternating(2.0, 4.0))),
        (EmbeddingStatus::Present, Some(alternating(4.0, 8.0))),
        (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
    ]
}

fn fixture_status(status: EmbeddingStatus) -> FixtureEmbeddingStatus {
    match status {
        EmbeddingStatus::Present => FixtureEmbeddingStatus::Present,
        EmbeddingStatus::MissingVector => FixtureEmbeddingStatus::MissingVector,
        EmbeddingStatus::ExtractionFailed => FixtureEmbeddingStatus::ExtractionFailed,
        EmbeddingStatus::QcRejected => FixtureEmbeddingStatus::QcRejected,
    }
}

fn publish_managed_embedding(
    embedding: &Fixture,
    project: &mut MarklabProject,
    cell_ids: &[CellId],
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
) -> (CellEmbeddingTable, CellEmbeddingArtifact) {
    let domain_rows = cell_ids
        .iter()
        .cloned()
        .zip(rows)
        .map(|(cell_id, (status, vector))| match status {
            EmbeddingStatus::Present => {
                CellEmbeddingRow::present(cell_id, vector.expect("present vector"))
            }
            status => {
                assert!(vector.is_none());
                CellEmbeddingRow::non_present(cell_id, status).expect("non-present row")
            }
        })
        .collect();
    let expected_record = record_with_schema(embedding, "marklab.cell_embedding_expected_cells");
    let row_link_record = record_with_schema(embedding, "marklab.cell_embedding_row_link");
    let table = CellEmbeddingTable::from_rows(
        DIMENSION,
        &embedding.expected,
        expected_record.id(),
        embedding.provenance_artifact_id,
        embedding.row_link.logical_digest(),
        domain_rows,
        embedding_budgets().maximum_retained_bytes(),
    )
    .expect("embedding table");
    let bindings = CellEmbeddingTablePhysicalBindings::new(
        expected_record.id(),
        embedding.provenance_artifact_id,
        row_link_record.id(),
        embedding.row_link.logical_digest(),
        table.qc_summary().logical_digest(),
    )
    .expect("physical bindings");
    let embedding_record =
        publish_cell_embedding_table_arrow(&embedding.store, &table, bindings, embedding_budgets())
            .expect("publish managed embedding table")
            .into_record();
    let graph = verified_graph(embedding);
    let table_receipt = verify_cell_embedding_table_arrow_from_store(
        &embedding.store,
        &embedding_record,
        &embedding.expected,
        &embedding.row_link,
        graph,
        embedding_budgets(),
    )
    .expect("verified managed embedding table");
    let row_link_receipt = verify_cell_embedding_row_link_arrow_from_store(
        &embedding.store,
        row_link_record,
        &embedding.expected,
        &embedding.row_link,
        embedding_budgets(),
    )
    .expect("verified managed row link");
    let artifact = CellEmbeddingArtifact::new(table_receipt, row_link_receipt, graph)
        .expect("verified embedding artifact");
    project
        .register_artifact(embedding_record)
        .expect("register embedding table");
    (table, artifact)
}

fn publish_scalar_artifact(
    project: &mut MarklabProject,
    store: &LocalArtifactStore,
    bytes: &[u8],
    schema: &str,
    dependencies: Vec<ArtifactId>,
    metadata: BTreeMap<String, String>,
) -> ArtifactId {
    let draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", bytes).expect("scalar content"),
        ArtifactSchema::new(schema, 1).expect("scalar schema"),
        None,
        dependencies,
        metadata,
    )
    .expect("scalar draft");
    let record = store
        .publish_new_send(&draft, |writer| writer.write_all(bytes))
        .expect("publish scalar artifact")
        .into_record();
    let id = record.id();
    project
        .register_artifact(record)
        .expect("register scalar artifact");
    id
}

fn run_node(
    project: &mut MarklabProject,
    store: &LocalArtifactStore,
    node_id: &str,
    input: &DeclaredScalarPatternInput<'_>,
    table: &CellEmbeddingTable,
    artifact: CellEmbeddingArtifact,
    limits: (usize, u64, usize),
) -> NodeRun<DeclaredBinaryCellEmbeddingCentroidDiscrepancy> {
    let (maximum_rows, maximum_component_operations, maximum_working_bytes) = limits;
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        project,
        NodeId::new(node_id).expect("node ID"),
        input,
        table,
        artifact,
        maximum_rows,
        maximum_component_operations,
        maximum_working_bytes,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES,
    })
    .expect("scheduler")
    .run_single_with_store(project, &graph, &node, store)
    .expect("centroid run")
}

fn workflow_fixture(
    rows: Vec<(EmbeddingStatus, Option<Vec<f32>>)>,
    marks: Vec<u8>,
    project_inline_limit: usize,
) -> WorkflowFixture {
    let cell_ids = (0..rows.len())
        .map(|index| CellId::new(format!("cell-{index:04}")).expect("cell ID"))
        .collect::<Vec<_>>();
    let embedding = build_fixture_with_rows(
        "marklab.model_checkpoint",
        LicenseAvailability::Managed,
        DIMENSION,
        cell_ids
            .iter()
            .cloned()
            .zip(rows.iter().map(|(status, _)| fixture_status(*status)))
            .collect(),
    );
    let mut project = MarklabProject::with_inline_artifact_limit(project_inline_limit)
        .expect("project inline limit");
    let slide_id = SlideId::new("centroid-workflow-slide").expect("slide ID");
    let frame_id = CoordinateFrameId::new("centroid-workflow-frame").expect("frame ID");
    install_structure(&mut project, &cell_ids, &slide_id, &frame_id);
    register_catalog(&mut project, &embedding.catalog);

    let (table, artifact) = publish_managed_embedding(&embedding, &mut project, &cell_ids, rows);

    let binary_bytes = b"declared-centroid-workflow-binary";
    let binary_draft = ArtifactDraft::new(
        ArtifactRef::from_bytes("application/json", binary_bytes).expect("binary content"),
        ArtifactSchema::new(declared_scalar_support::MARK_SCHEMA, 1).expect("mark schema"),
        None,
        Vec::new(),
        declared_scalar_support::binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::Measured,
            "independent",
        ),
    )
    .expect("binary draft");
    let binary_record = embedding
        .store
        .publish_new_send(&binary_draft, |writer| writer.write_all(binary_bytes))
        .expect("publish binary provenance")
        .into_record();
    let binary_record_id = binary_record.id();
    project
        .register_artifact(binary_record)
        .expect("register binary provenance");
    let binary = BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("mark ID"),
        "MMR loss",
        MeasurementStatus::Measured,
        binary_record_id,
    )
    .expect("binary declaration");
    let mut pattern = Pattern::from_arrays(
        (0..marks.len()).map(|index| index as f64).collect(),
        vec![0.0; marks.len()],
        marks,
        PatternMeta {
            case_id: "centroid-workflow-case".into(),
            timepoint: "post".into(),
            protein: "MSH6".into(),
            slide_id: Some(slide_id.as_str().to_owned()),
            section_id: None,
            stain_batch: None,
            block_id: None,
            region_id: None,
        },
    )
    .expect("pattern");
    pattern.window.area_um2 = 40.0;
    pattern.window.analysis_effective_length_um = 4.0;
    pattern.window.d_nn_mean_um = 1.0;

    WorkflowFixture {
        embedding,
        project,
        pattern,
        cell_ids,
        slide_id,
        frame_id,
        binary,
        binary_record_id,
        table,
        artifact,
    }
}

fn install_structure(
    project: &mut MarklabProject,
    cell_ids: &[CellId],
    slide_id: &SlideId,
    frame_id: &CoordinateFrameId,
) {
    let patient = HierarchyId::from(PatientId::new("centroid-patient").expect("patient ID"));
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
    project
        .install_hierarchy(CohortHierarchy::new(nodes, Vec::new()).expect("hierarchy"))
        .expect("install hierarchy");
    let frame = CoordinateFrame::new(
        frame_id.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .expect("frame");
    project
        .install_coordinate_registry(
            CoordinateRegistry::new(vec![frame], Vec::new(), Vec::new(), Vec::new())
                .expect("registry"),
        )
        .expect("install registry");
}

fn register_catalog(project: &mut MarklabProject, catalog: &ArtifactCatalog) {
    let mut pending = catalog
        .iter()
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>();
    while !pending.is_empty() {
        let index = pending
            .iter()
            .position(|record| {
                record
                    .dependencies()
                    .iter()
                    .all(|id| project.artifact_record(*id).is_some())
            })
            .expect("catalog must contain a dependency order");
        project
            .register_artifact(pending.remove(index))
            .expect("register embedding catalog record");
    }
}

fn declared_input<'a>(
    project: &MarklabProject,
    pattern: &'a Pattern,
    cell_ids: &'a [CellId],
    slide_id: SlideId,
    frame_id: CoordinateFrameId,
    binary: BinaryMarkDeclaration,
) -> DeclaredScalarPatternInput<'a> {
    DeclaredScalarPatternInput::new(
        project,
        pattern,
        cell_ids,
        slide_id,
        frame_id,
        binary,
        None,
        16 * 1024,
        cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("declared input")
}

fn expected_codec(result: &DeclaredBinaryCellEmbeddingCentroidDiscrepancy) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(CODEC_BYTES);
    bytes.extend_from_slice(b"MLCBCENT");
    bytes.push(1);
    let (status, value) = match result.status() {
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::Available => (
            1,
            result
                .mean_squared_component_difference()
                .expect("available value"),
        ),
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups => (0, 0.0),
    };
    bytes.push(status);
    bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    assert_eq!(bytes.len(), CODEC_BYTES);
    bytes
}

fn raw_codec(status: u8, value_bits: u64) -> Vec<u8> {
    let mut bytes = b"MLCBCENT".to_vec();
    bytes.push(1);
    bytes.push(status);
    bytes.extend_from_slice(&value_bits.to_be_bytes());
    assert_eq!(bytes.len(), CODEC_BYTES);
    bytes
}

fn assert_decode_rejected(node: &DeclaredBinaryCellEmbeddingCentroidNode<'_, '_>, bytes: &[u8]) {
    assert!(matches!(
        node.decode_output(bytes),
        Err(NodeError::Decoding { .. })
    ));
}

#[path = "declared_binary_centroid_workflow/execution.rs"]
mod execution;
#[path = "declared_binary_centroid_workflow/validation.rs"]
mod validation;
