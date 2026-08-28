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

#[test]
fn declared_binary_centroid_workflow_miss_hit_matches_direct_and_exact_codec() {
    let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let mark_table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                fixture.binary.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::vector_artifact_ref(
                VectorArtifactRefMarkDeclaration::new(
                    ScalarMarkId::new("cellvit_embedding").expect("vector mark ID"),
                    "CellViT embedding",
                    MeasurementStatus::MorphologyPrediction,
                )
                .expect("vector declaration"),
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::EmbeddingVector,
                MissingnessPolicy::NotPermitted,
                &fixture.table,
                fixture.artifact,
            )
            .expect("vector artifact reference"),
        ],
        fixture.cell_ids.len(),
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("typed vector MarkTable");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &mark_table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared typed vector input");
    let direct = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("direct centroid");
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut fixture.project,
        NodeId::new("declared-centroid-workflow").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES,
    })
    .expect("scheduler");

    let no_store = match scheduler.run_single(&mut fixture.project, &graph, &node) {
        Ok(_) => panic!("semantic store is required"),
        Err(error) => error,
    };
    assert!(matches!(
        no_store,
        WorkflowError::SemanticStoreRequired { .. }
    ));
    assert_eq!(fixture.project.successful_run_count(), 0);

    let miss = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &node,
            &fixture.embedding.store,
        )
        .expect("scheduler miss");
    assert_eq!(miss.cache_status, CacheStatus::Miss);
    assert_eq!(miss.output, direct);
    assert_eq!(miss.artifact.kind(), node.output_kind());
    let expected = expected_codec(&direct);
    assert_eq!(
        fixture
            .project
            .read_inline_verified(&miss.artifact)
            .expect("cached bytes"),
        expected
    );
    assert_eq!(
        node.encode_output(&direct).expect("encode").as_ref(),
        expected
    );

    let hit = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &node,
            &fixture.embedding.store,
        )
        .expect("scheduler hit");
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(hit.output, direct);
    assert_eq!(hit.artifact, miss.artifact);
    assert_eq!(hit.cache_key, miss.cache_key);
    assert_eq!(fixture.project.successful_run_count(), 1);
    assert_eq!(
        fixture.binary_record_id,
        direct.binary_mark().provenance_artifact_id()
    );
}

#[test]
fn declared_binary_centroid_workflow_keys_exact_grouping_probability_evidence_and_embedding() {
    let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );
    let baseline = run_node(
        &mut fixture.project,
        &fixture.embedding.store,
        "centroid-key-binding",
        &input,
        &fixture.table,
        fixture.artifact,
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
    );

    let mut swapped_pattern = fixture.pattern.clone();
    swapped_pattern.mark = vec![1, 0, 0, 1].into_boxed_slice();
    let swapped_input = declared_input(
        &fixture.project,
        &swapped_pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );
    let swapped = run_node(
        &mut fixture.project,
        &fixture.embedding.store,
        "centroid-key-binding",
        &swapped_input,
        &fixture.table,
        fixture.artifact,
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
    );
    assert_eq!(swapped.cache_status, CacheStatus::Miss);
    assert_ne!(swapped.cache_key, baseline.cache_key);
    assert_ne!(
        swapped.output.binary_grouping_logical_digest(),
        baseline.output.binary_grouping_logical_digest()
    );
    assert_eq!(
        swapped.output.marked_counts(),
        baseline.output.marked_counts()
    );
    assert_eq!(
        swapped.output.unmarked_counts(),
        baseline.output.unmarked_counts()
    );
    assert_eq!(
        swapped.output.mean_squared_component_difference(),
        baseline.output.mean_squared_component_difference()
    );

    let probability_id = publish_scalar_artifact(
        &mut fixture.project,
        &fixture.embedding.store,
        b"centroid-workflow-probability",
        declared_scalar_support::MARK_SCHEMA,
        Vec::new(),
        declared_scalar_support::probability_metadata(
            "mmr_loss_probability",
            MeasurementStatus::ImportedPrediction,
        ),
    );
    let probability = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability mark ID"),
        MeasurementStatus::ImportedPrediction,
        probability_id,
    )
    .expect("probability declaration");
    let mut probability_pattern = fixture.pattern.clone();
    probability_pattern.mark_prob = Some(vec![0.1, 0.9, 0.9, 0.1].into_boxed_slice());
    let probability_input = DeclaredScalarPatternInput::new(
        &fixture.project,
        &probability_pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
        Some(probability.clone()),
        16 * 1024,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("probability input");
    let probability_run = run_node(
        &mut fixture.project,
        &fixture.embedding.store,
        "centroid-key-binding",
        &probability_input,
        &fixture.table,
        fixture.artifact,
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
    );
    assert_eq!(probability_run.cache_status, CacheStatus::Miss);
    assert_ne!(probability_run.cache_key, baseline.cache_key);
    assert_eq!(
        probability_run.output.mean_squared_component_difference(),
        baseline.output.mean_squared_component_difference()
    );
    assert_eq!(
        probability_run.output.marked_counts(),
        baseline.output.marked_counts()
    );
    assert_eq!(
        probability_run.output.probability_mark(),
        Some(&probability)
    );

    let threshold_evidence_id = publish_scalar_artifact(
        &mut fixture.project,
        &fixture.embedding.store,
        b"centroid-workflow-threshold-evidence",
        declared_scalar_support::THRESHOLD_SCHEMA,
        vec![probability_id],
        declared_scalar_support::threshold_metadata(
            "mmr_loss",
            "mmr_loss_probability",
            "greater_than_or_equal",
            0.5,
        ),
    );
    let thresholded_binary_id = publish_scalar_artifact(
        &mut fixture.project,
        &fixture.embedding.store,
        b"centroid-workflow-thresholded-binary",
        declared_scalar_support::MARK_SCHEMA,
        vec![probability_id, threshold_evidence_id],
        declared_scalar_support::binary_metadata(
            "mmr_loss",
            "MMR loss",
            MeasurementStatus::ImportedPrediction,
            "thresholded",
        ),
    );
    let thresholded_binary = BinaryMarkDeclaration::thresholded(
        ScalarMarkId::new("mmr_loss").expect("binary mark ID"),
        "MMR loss",
        MeasurementStatus::ImportedPrediction,
        thresholded_binary_id,
        probability.mark_id().clone(),
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
        Some(threshold_evidence_id),
    )
    .expect("thresholded binary declaration");
    let thresholded_input = DeclaredScalarPatternInput::new(
        &fixture.project,
        &probability_pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        thresholded_binary,
        Some(probability),
        16 * 1024,
        fixture.cell_ids.iter().map(|id| id.as_str().len()).sum(),
    )
    .expect("thresholded input");
    let thresholded = run_node(
        &mut fixture.project,
        &fixture.embedding.store,
        "centroid-key-binding",
        &thresholded_input,
        &fixture.table,
        fixture.artifact,
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
    );
    assert_eq!(thresholded.cache_status, CacheStatus::Miss);
    assert_ne!(thresholded.cache_key, probability_run.cache_key);
    assert_eq!(
        thresholded.output.mean_squared_component_difference(),
        baseline.output.mean_squared_component_difference()
    );

    let mut changed_rows = available_rows();
    changed_rows[0] = (EmbeddingStatus::Present, Some(alternating(0.5, 0.25)));
    let (changed_table, changed_artifact) = publish_managed_embedding(
        &fixture.embedding,
        &mut fixture.project,
        &fixture.cell_ids,
        changed_rows,
    );
    let changed_embedding = run_node(
        &mut fixture.project,
        &fixture.embedding.store,
        "centroid-key-binding",
        &input,
        &changed_table,
        changed_artifact,
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
    );
    assert_eq!(changed_embedding.cache_status, CacheStatus::Miss);
    assert_ne!(changed_embedding.cache_key, baseline.cache_key);
    assert_ne!(
        changed_embedding.output.embedding_artifact_id(),
        baseline.output.embedding_artifact_id()
    );
    assert_eq!(
        changed_embedding.output.binary_grouping_logical_digest(),
        baseline.output.binary_grouping_logical_digest()
    );
}

#[test]
fn declared_binary_centroid_workflow_keys_every_scientific_limit() {
    let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );
    let limits = [
        (4, COMPONENT_OPERATIONS, WORKING_BYTES),
        (5, COMPONENT_OPERATIONS, WORKING_BYTES),
        (4, COMPONENT_OPERATIONS + 1, WORKING_BYTES),
        (4, COMPONENT_OPERATIONS, WORKING_BYTES + 1),
    ];
    let runs = limits.map(|(rows, operations, working)| {
        run_node(
            &mut fixture.project,
            &fixture.embedding.store,
            "centroid-limit-key",
            &input,
            &fixture.table,
            fixture.artifact,
            (rows, operations, working),
        )
    });
    let baseline_key = runs[0].cache_key;
    let baseline_output = &runs[0].output;
    for run in &runs[1..] {
        assert_eq!(run.cache_status, CacheStatus::Miss);
        assert_ne!(run.cache_key, baseline_key);
        assert_eq!(&run.output, baseline_output);
    }
    let distinct = runs
        .iter()
        .map(|run| run.cache_key)
        .collect::<BTreeSet<_>>();
    assert_eq!(distinct.len(), runs.len());

    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut fixture.project,
        NodeId::new("centroid-limit-key").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let scheduler_changed = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES + 1,
    })
    .expect("changed scheduler")
    .run_single_with_store(
        &mut fixture.project,
        &graph,
        &node,
        &fixture.embedding.store,
    )
    .expect("changed scheduler run");
    assert_eq!(scheduler_changed.cache_status, CacheStatus::Miss);
    assert_ne!(scheduler_changed.cache_key, baseline_key);
    assert_eq!(&scheduler_changed.output, baseline_output);
}

#[test]
fn declared_binary_centroid_constructor_failures_register_no_new_references() {
    let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );

    let mut wrong_project = MarklabProject::new();
    let before = (
        wrong_project.artifact_count(),
        wrong_project.inline_artifact_count(),
        wrong_project.successful_run_count(),
    );
    let error = match DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut wrong_project,
        NodeId::new("wrong-target-project").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    ) {
        Ok(_) => panic!("wrong target project must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, NodeError::Input { .. }));
    assert_eq!(
        (
            wrong_project.artifact_count(),
            wrong_project.inline_artifact_count(),
            wrong_project.successful_run_count(),
        ),
        before
    );

    let mut missing_catalog = MarklabProject::new();
    install_structure(
        &mut missing_catalog,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
    );
    missing_catalog
        .register_artifact(
            fixture
                .project
                .artifact_record(fixture.binary_record_id)
                .expect("binary record")
                .clone(),
        )
        .expect("register binary record only");
    let before = (
        missing_catalog.artifact_count(),
        missing_catalog.inline_artifact_count(),
        missing_catalog.successful_run_count(),
    );
    let error = match DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut missing_catalog,
        NodeId::new("missing-target-catalog").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    ) {
        Ok(_) => panic!("missing target catalog must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, NodeError::Input { .. }));
    assert_eq!(
        (
            missing_catalog.artifact_count(),
            missing_catalog.inline_artifact_count(),
            missing_catalog.successful_run_count(),
        ),
        before
    );

    let before = (
        fixture.project.artifact_count(),
        fixture.project.inline_artifact_count(),
        fixture.project.successful_run_count(),
    );
    let error = match DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut fixture.project,
        NodeId::new("binding-budget-failure").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        3,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    ) {
        Ok(_) => panic!("binding budget failure must reject construction"),
        Err(error) => error,
    };
    assert!(matches!(error, NodeError::Input { .. }));
    assert_eq!(
        (
            fixture.project.artifact_count(),
            fixture.project.inline_artifact_count(),
            fixture.project.successful_run_count(),
        ),
        before
    );
}

#[test]
fn declared_binary_centroid_semantic_inputs_are_verified_before_cache_or_execution() {
    for role in 0..4 {
        for remove in [false, true] {
            for populate_cache in [false, true] {
                let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
                let input = declared_input(
                    &fixture.project,
                    &fixture.pattern,
                    &fixture.cell_ids,
                    fixture.slide_id.clone(),
                    fixture.frame_id.clone(),
                    fixture.binary.clone(),
                );
                let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
                    &mut fixture.project,
                    NodeId::new(format!(
                        "semantic-integrity-{role}-{remove}-{populate_cache}"
                    ))
                    .expect("node ID"),
                    &input,
                    &fixture.table,
                    fixture.artifact,
                    4,
                    COMPONENT_OPERATIONS,
                    WORKING_BYTES,
                )
                .expect("centroid node");
                let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
                let scheduler = LocalScheduler::new(SchedulerLimits {
                    max_inline_output_bytes: CODEC_BYTES,
                })
                .expect("scheduler");
                if populate_cache {
                    let first = scheduler
                        .run_single_with_store(
                            &mut fixture.project,
                            &graph,
                            &node,
                            &fixture.embedding.store,
                        )
                        .expect("populate cache");
                    assert_eq!(first.cache_status, CacheStatus::Miss);
                }
                let artifact_id = [
                    fixture.binary_record_id,
                    fixture.artifact.embedding_artifact_id(),
                    fixture.artifact.row_link_artifact_id(),
                    fixture.artifact.provenance_artifact_id(),
                ][role];
                let record = fixture
                    .project
                    .artifact_record(artifact_id)
                    .expect("semantic record")
                    .clone();
                let path = managed_path(&fixture.embedding._root, &record);
                if remove {
                    std::fs::remove_file(path).expect("remove managed semantic input");
                } else {
                    std::fs::write(path, b"corrupt-centroid-semantic-input")
                        .expect("corrupt managed semantic input");
                }
                let before = (
                    fixture.project.artifact_count(),
                    fixture.project.inline_artifact_count(),
                    fixture.project.successful_run_count(),
                );

                let error = match scheduler.run_single_with_store(
                    &mut fixture.project,
                    &graph,
                    &node,
                    &fixture.embedding.store,
                ) {
                    Ok(_) => panic!("unavailable semantic input must fail"),
                    Err(error) => error,
                };
                assert!(matches!(
                    error,
                    WorkflowError::SemanticInputIntegrity { artifact, .. }
                        if artifact == artifact_id
                ));
                assert_eq!(
                    (
                        fixture.project.artifact_count(),
                        fixture.project.inline_artifact_count(),
                        fixture.project.successful_run_count(),
                    ),
                    before
                );
            }
        }
    }
}

#[test]
fn declared_binary_centroid_codec_rejects_every_noncanonical_available_payload() {
    let mut fixture = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );
    let direct = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("direct centroid");
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut fixture.project,
        NodeId::new("strict-available-codec").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let valid = expected_codec(&direct);
    assert_eq!(node.decode_output(&valid).expect("valid decode"), direct);

    assert_decode_rejected(&node, &valid[..CODEC_BYTES - 1]);
    let mut too_long = valid.clone();
    too_long.push(0);
    assert_decode_rejected(&node, &too_long);
    let mut wrong_magic = valid.clone();
    wrong_magic[0] ^= 1;
    assert_decode_rejected(&node, &wrong_magic);
    let mut wrong_version = valid.clone();
    wrong_version[8] = 2;
    assert_decode_rejected(&node, &wrong_version);
    let mut wrong_tag = valid.clone();
    wrong_tag[9] = 2;
    assert_decode_rejected(&node, &wrong_tag);
    assert_decode_rejected(&node, &raw_codec(0, 0.0_f64.to_bits()));
    for bits in [
        f64::NAN.to_bits(),
        f64::INFINITY.to_bits(),
        (-1.0_f64).to_bits(),
        (-0.0_f64).to_bits(),
    ] {
        assert_decode_rejected(&node, &raw_codec(1, bits));
    }
}

#[test]
fn declared_binary_centroid_insufficient_groups_round_trip_and_reject_filler_or_status_drift() {
    let rows = vec![
        (
            EmbeddingStatus::Present,
            Some(vec![0.0; DIMENSION as usize]),
        ),
        (EmbeddingStatus::MissingVector, None),
        (EmbeddingStatus::ExtractionFailed, None),
        (EmbeddingStatus::Present, Some(alternating(2.0, 0.0))),
    ];
    let mut fixture = workflow_fixture(rows, vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
        fixture.binary.clone(),
    );
    let direct = declared_binary_cell_embedding_centroid_discrepancy(
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("insufficient direct centroid");
    assert_eq!(
        direct.status(),
        DeclaredBinaryCellEmbeddingCentroidDiscrepancyStatus::InsufficientGroups
    );
    assert_eq!(direct.mean_squared_component_difference(), None);
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut fixture.project,
        NodeId::new("insufficient-centroid-codec").expect("node ID"),
        &input,
        &fixture.table,
        fixture.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES,
    })
    .expect("scheduler");
    let miss = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &node,
            &fixture.embedding.store,
        )
        .expect("insufficient miss");
    let hit = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &node,
            &fixture.embedding.store,
        )
        .expect("insufficient hit");
    assert_eq!(miss.cache_status, CacheStatus::Miss);
    assert_eq!(hit.cache_status, CacheStatus::Hit);
    assert_eq!(miss.output, direct);
    assert_eq!(hit.output, direct);
    assert_eq!(
        fixture
            .project
            .read_inline_verified(&miss.artifact)
            .expect("insufficient cached bytes"),
        raw_codec(0, 0.0_f64.to_bits())
    );
    assert_decode_rejected(&node, &raw_codec(0, 1.0_f64.to_bits()));
    assert_decode_rejected(&node, &raw_codec(1, 0.0_f64.to_bits()));

    let mut available = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let available_input = declared_input(
        &available.project,
        &available.pattern,
        &available.cell_ids,
        available.slide_id.clone(),
        available.frame_id.clone(),
        available.binary.clone(),
    );
    let available_node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut available.project,
        NodeId::new("available-binding-codec").expect("node ID"),
        &available_input,
        &available.table,
        available.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("available centroid node");
    assert!(matches!(
        available_node.encode_output(&direct),
        Err(NodeError::Encoding { .. })
    ));
}

#[test]
fn declared_binary_centroid_exact_output_limits_pass_and_one_short_fail_atomically() {
    let mut scheduler_short = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES);
    let input = declared_input(
        &scheduler_short.project,
        &scheduler_short.pattern,
        &scheduler_short.cell_ids,
        scheduler_short.slide_id.clone(),
        scheduler_short.frame_id.clone(),
        scheduler_short.binary.clone(),
    );
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut scheduler_short.project,
        NodeId::new("scheduler-one-short").expect("node ID"),
        &input,
        &scheduler_short.table,
        scheduler_short.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let before = (
        scheduler_short.project.artifact_count(),
        scheduler_short.project.inline_artifact_count(),
        scheduler_short.project.successful_run_count(),
    );
    let error = match LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES - 1,
    })
    .expect("scheduler")
    .run_single_with_store(
        &mut scheduler_short.project,
        &graph,
        &node,
        &scheduler_short.embedding.store,
    ) {
        Ok(_) => panic!("one-short scheduler limit must fail"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        WorkflowError::InlineOutputTooLarge {
            encoded_bytes: CODEC_BYTES,
            limit_bytes,
            ..
        } if limit_bytes == CODEC_BYTES - 1
    ));
    assert_eq!(
        (
            scheduler_short.project.artifact_count(),
            scheduler_short.project.inline_artifact_count(),
            scheduler_short.project.successful_run_count(),
        ),
        before
    );

    let mut project_short = workflow_fixture(available_rows(), vec![0, 1, 1, 0], CODEC_BYTES - 1);
    let input = declared_input(
        &project_short.project,
        &project_short.pattern,
        &project_short.cell_ids,
        project_short.slide_id.clone(),
        project_short.frame_id.clone(),
        project_short.binary.clone(),
    );
    let node = DeclaredBinaryCellEmbeddingCentroidNode::new(
        &mut project_short.project,
        NodeId::new("project-one-short").expect("node ID"),
        &input,
        &project_short.table,
        project_short.artifact,
        4,
        COMPONENT_OPERATIONS,
        WORKING_BYTES,
    )
    .expect("centroid node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let before = (
        project_short.project.artifact_count(),
        project_short.project.inline_artifact_count(),
        project_short.project.successful_run_count(),
    );
    let error = match LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: CODEC_BYTES,
    })
    .expect("scheduler")
    .run_single_with_store(
        &mut project_short.project,
        &graph,
        &node,
        &project_short.embedding.store,
    ) {
        Ok(_) => panic!("one-short project limit must fail"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        WorkflowError::Project(ProjectError::InlineArtifactTooLarge {
            encoded_bytes: CODEC_BYTES,
            limit_bytes,
        }) if limit_bytes == CODEC_BYTES - 1
    ));
    assert_eq!(
        (
            project_short.project.artifact_count(),
            project_short.project.inline_artifact_count(),
            project_short.project.successful_run_count(),
        ),
        before
    );
}
