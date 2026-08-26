use marklab::{
    AnalysisEngine, BinaryMarkDeclaration, CacheStatus, DeclaredMarkedAnalysisNode,
    DeclaredScalarInputError, DeclaredScalarPatternInput, LocalScheduler, MarkTable,
    MeasurementStatus, MissingnessPolicy, NodeError, NodeId, NucleusAreaUm2MarkDeclaration,
    ProbabilityMarkDeclaration, ProbabilityThresholdComparator, ResultDocument, ScalarMarkColumn,
    ScalarMarkId, ScalarMarkModality, ScalarMarkUnit, ScalarMarkValueKind, SchedulerLimits,
    WorkflowError, WorkflowGraph,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

const ROW_LIMIT: usize = 4;

fn independent_binary(
    fixture: &mut Fixture,
    status: MeasurementStatus,
    content: &[u8],
) -> BinaryMarkDeclaration {
    let provenance = publish_record(
        fixture,
        content,
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata("mmr_loss", "MMR loss", status, "independent"),
    );
    BinaryMarkDeclaration::independent(
        ScalarMarkId::new("mmr_loss").expect("binary ID"),
        "MMR loss",
        status,
        provenance,
    )
    .expect("independent binary")
}

fn paired_declarations(
    fixture: &mut Fixture,
) -> (BinaryMarkDeclaration, ProbabilityMarkDeclaration) {
    paired_declarations_with_threshold(
        fixture,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    )
}

fn paired_declarations_with_threshold(
    fixture: &mut Fixture,
    comparator: ProbabilityThresholdComparator,
    threshold: f32,
) -> (BinaryMarkDeclaration, ProbabilityMarkDeclaration) {
    let status = MeasurementStatus::ImportedPrediction;
    let probability_provenance = publish_record(
        fixture,
        b"workflow-probability-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata("mmr_loss_probability", status),
    );
    let probability = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new("mmr_loss_probability").expect("probability ID"),
        status,
        probability_provenance,
    )
    .expect("probability declaration");
    let comparator_name = match comparator {
        ProbabilityThresholdComparator::GreaterThan => "greater_than",
        ProbabilityThresholdComparator::GreaterThanOrEqual => "greater_than_or_equal",
    };
    let threshold_provenance = publish_record(
        fixture,
        b"workflow-threshold-provenance",
        THRESHOLD_SCHEMA,
        1,
        None,
        vec![probability_provenance],
        threshold_metadata(
            "mmr_loss",
            probability.mark_id().as_str(),
            comparator_name,
            threshold,
        ),
    );
    let binary_provenance = publish_record(
        fixture,
        b"workflow-binary-provenance",
        MARK_SCHEMA,
        1,
        None,
        vec![probability_provenance, threshold_provenance],
        binary_metadata("mmr_loss", "MMR loss", status, "thresholded"),
    );
    let binary = BinaryMarkDeclaration::thresholded(
        ScalarMarkId::new("mmr_loss").expect("binary ID"),
        "MMR loss",
        status,
        binary_provenance,
        probability.mark_id().clone(),
        comparator,
        threshold,
        Some(threshold_provenance),
    )
    .expect("thresholded binary");
    (binary, probability)
}

fn input<'a>(
    project: &marklab::MarklabProject,
    pattern: &'a marklab::Pattern,
    cell_ids: &'a [marklab::CellId],
    slide_id: &marklab::SlideId,
    frame_id: &marklab::CoordinateFrameId,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
) -> DeclaredScalarPatternInput<'a> {
    DeclaredScalarPatternInput::new(
        project,
        pattern,
        cell_ids,
        slide_id.clone(),
        frame_id.clone(),
        binary,
        probability,
        ROW_LIMIT,
        cell_id_text_bytes(cell_ids),
    )
    .expect("declared input")
}

fn without_timings(mut result: marklab::MarkedPatternResult) -> marklab::MarkedPatternResult {
    result.timings.clear();
    result
}

#[test]
fn declared_binary_engine_path_is_numerically_identical_to_legacy() {
    let mut fixture = fixture();
    let binary = independent_binary(
        &mut fixture,
        MeasurementStatus::Measured,
        b"engine-binary-provenance",
    );
    let input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary.clone(),
        None,
    );
    let config = analysis_config(false);
    let engine = AnalysisEngine::new(config).expect("engine");

    let legacy = engine
        .analyze_pattern_run(&fixture.pattern)
        .expect("legacy run");
    let declared = engine
        .analyze_declared_scalar_pattern(&input)
        .expect("declared run");

    assert_eq!(declared.actual_thread_count, legacy.actual_thread_count);
    assert_eq!(declared.scalar_identity.row_count(), fixture.cell_ids.len());
    assert_eq!(
        declared.scalar_identity.owning_slide_id(),
        &fixture.slide_id
    );
    assert_eq!(
        declared.scalar_identity.coordinate_frame_id(),
        &fixture.frame_id
    );
    assert_eq!(
        without_timings(declared.result),
        without_timings(legacy.result)
    );
    assert_eq!(declared.mark_use.binary_mark(), &binary);
    assert_eq!(declared.mark_use.probability_mark(), None);
    assert_eq!(
        declared.mark_use.structure_factor_value_kind(),
        ScalarMarkValueKind::Binary
    );
    assert_eq!(
        declared.mark_use.other_endpoint_value_kind(),
        ScalarMarkValueKind::Binary
    );
}

#[test]
fn declared_probability_routes_only_structure_factor_and_matches_legacy() {
    let mut fixture = fixture();
    fixture.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let (binary, probability) = paired_declarations(&mut fixture);
    let input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary.clone(),
        Some(probability.clone()),
    );
    let config = analysis_config(true);
    let engine = AnalysisEngine::new(config).expect("probability engine");

    let legacy = engine
        .analyze_pattern_run(&fixture.pattern)
        .expect("legacy probability run");
    let declared = engine
        .analyze_declared_scalar_pattern(&input)
        .expect("declared probability run");

    assert_eq!(
        without_timings(declared.result),
        without_timings(legacy.result)
    );
    assert_eq!(declared.mark_use.binary_mark(), &binary);
    assert_eq!(declared.mark_use.probability_mark(), Some(&probability));
    assert_eq!(
        declared.mark_use.structure_factor_value_kind(),
        ScalarMarkValueKind::Probability
    );
    assert_eq!(
        declared.mark_use.other_endpoint_value_kind(),
        ScalarMarkValueKind::Binary
    );
}

#[test]
fn declared_probability_workflow_is_constructed_from_one_typed_mark_table() {
    let mut fixture = fixture();
    fixture.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    fixture.pattern.nucleus_area_um2 = Some(vec![10.0, 11.0, 12.0, 13.0].into_boxed_slice());
    let (binary, probability) = paired_declarations(&mut fixture);
    let area_provenance = publish_record(
        &mut fixture,
        b"workflow-area-provenance",
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        nucleus_area_um2_metadata(MeasurementStatus::Measured),
    );
    let area = NucleusAreaUm2MarkDeclaration::new(MeasurementStatus::Measured, area_provenance)
        .expect("area declaration");
    let mark_table = MarkTable::new(
        fixture.cell_ids.clone(),
        vec![
            ScalarMarkColumn::binary(
                binary.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark.clone(),
            )
            .expect("binary column"),
            ScalarMarkColumn::probability(
                probability.clone(),
                ScalarMarkModality::Immunohistochemistry,
                ScalarMarkUnit::Unitless,
                MissingnessPolicy::NotPermitted,
                fixture.pattern.mark_prob.clone().expect("probabilities"),
            )
            .expect("probability column"),
            ScalarMarkColumn::continuous(
                area,
                ScalarMarkModality::Morphology,
                ScalarMarkUnit::SquareMicrometer,
                MissingnessPolicy::NotPermitted,
                fixture
                    .pattern
                    .nucleus_area_um2
                    .clone()
                    .expect("nucleus areas"),
            )
            .expect("continuous column"),
        ],
        ROW_LIMIT,
        cell_id_text_bytes(&fixture.cell_ids),
    )
    .expect("typed mark table");
    let input = DeclaredScalarPatternInput::from_mark_table(
        &fixture.project,
        &fixture.pattern,
        &mark_table,
        fixture.slide_id.clone(),
        fixture.frame_id.clone(),
    )
    .expect("declared table input");
    let config = analysis_config(true);
    let node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("declared-mark-table-analysis").expect("node ID"),
        &input,
        &config,
    )
    .expect("declared table node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");

    let executed = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &node, &fixture.store)
        .expect("table-backed declared workflow");
    let legacy = AnalysisEngine::new(config)
        .expect("legacy engine")
        .analyze_pattern_run(&fixture.pattern)
        .expect("legacy analysis");
    let legacy_round_trip = ResultDocument::from_json(
        &ResultDocument::marked(legacy.result)
            .to_json_pretty()
            .expect("legacy result 0.3"),
    )
    .expect("decode legacy result 0.3")
    .into_marked_pattern()
    .expect("legacy marked result");

    assert_eq!(executed.cache_status, CacheStatus::Miss);
    assert_eq!(
        without_timings(executed.output.result),
        without_timings(legacy_round_trip)
    );
    assert_eq!(executed.output.mark_use.binary_mark(), &binary);
    assert_eq!(
        executed.output.mark_use.probability_mark(),
        Some(&probability)
    );
}

#[test]
fn config_label_and_probability_mode_mismatch_fail_before_analysis() {
    let mut fixture = fixture();
    let binary = independent_binary(
        &mut fixture,
        MeasurementStatus::Measured,
        b"mismatch-binary-provenance",
    );
    let input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary,
        None,
    );

    let mut wrong_label = analysis_config(false);
    wrong_label.analysis.mark_label = "different mark".into();
    let engine = AnalysisEngine::new(wrong_label).expect("wrong-label engine");
    assert!(matches!(
        engine.analyze_declared_scalar_pattern(&input),
        Err(DeclaredScalarInputError::ConfigurationMarkLabelMismatch)
    ));

    let engine = AnalysisEngine::new(analysis_config(true)).expect("probability engine");
    assert!(matches!(
        engine.analyze_declared_scalar_pattern(&input),
        Err(DeclaredScalarInputError::ConfigurationValueKindMismatch)
    ));
}

#[test]
fn declared_node_revalidates_the_target_project_before_cataloging_inputs() {
    let mut fixture = fixture();
    let binary = independent_binary(
        &mut fixture,
        MeasurementStatus::Measured,
        b"target-project-provenance",
    );
    let input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary,
        None,
    );
    let config = analysis_config(false);
    let mut unrelated_project = marklab::MarklabProject::new();

    let error = match DeclaredMarkedAnalysisNode::new(
        &mut unrelated_project,
        NodeId::new("unrelated-project-analysis").expect("node ID"),
        &input,
        &config,
    ) {
        Ok(_) => panic!("node construction must reject an unrelated project"),
        Err(error) => error,
    };
    assert!(matches!(error, NodeError::Input { .. }));
    assert_eq!(unrelated_project.artifact_count(), 0);
}

#[test]
fn scheduler_miss_hit_preserves_mark_use_and_exact_result_v03_codec() {
    let mut fixture = fixture();
    fixture.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let (binary, probability) = paired_declarations(&mut fixture);
    let input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary,
        Some(probability),
    );
    let config = analysis_config(true);
    let node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("declared-marked-analysis").expect("node ID"),
        &input,
        &config,
    )
    .expect("declared node");
    let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");

    let first = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &node, &fixture.store)
        .expect("scheduler miss");
    assert_eq!(first.cache_status, CacheStatus::Miss);
    assert_eq!(
        first.output.scalar_identity.row_count(),
        fixture.cell_ids.len()
    );
    assert_eq!(
        first.output.scalar_identity.coordinate_frame_id(),
        &fixture.frame_id
    );
    assert_eq!(first.output.mark_use.binary_mark(), input.binary_mark());
    assert_eq!(
        first.output.mark_use.probability_mark(),
        input.probability_mark()
    );
    assert_eq!(
        first.output.mark_use.structure_factor_value_kind(),
        ScalarMarkValueKind::Probability
    );
    assert_eq!(
        first.output.mark_use.other_endpoint_value_kind(),
        ScalarMarkValueKind::Binary
    );
    let expected_bytes = ResultDocument::marked(first.output.result.clone())
        .to_json_pretty()
        .expect("result 0.3")
        .into_bytes();
    assert_eq!(
        fixture
            .project
            .read_inline_verified(&first.artifact)
            .expect("cached result bytes"),
        expected_bytes
    );
    let decoded =
        ResultDocument::from_json(std::str::from_utf8(&expected_bytes).expect("UTF-8 result 0.3"))
            .expect("decode result 0.3")
            .into_marked_pattern()
            .expect("marked result");
    assert_eq!(decoded, first.output.result);

    let cached = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &node, &fixture.store)
        .expect("scheduler hit");
    assert_eq!(cached.cache_status, CacheStatus::Hit);
    assert_eq!(cached.output, first.output);
    assert_eq!(cached.artifact, first.artifact);
}

#[test]
fn declared_identity_changes_cache_key_without_changing_legacy_numerics() {
    let mut fixture = fixture();
    let measured = independent_binary(
        &mut fixture,
        MeasurementStatus::Measured,
        b"cache-measured-provenance",
    );
    let alternate_provenance = independent_binary(
        &mut fixture,
        MeasurementStatus::Measured,
        b"cache-alternate-provenance",
    );
    let predicted = independent_binary(
        &mut fixture,
        MeasurementStatus::MorphologyPrediction,
        b"cache-predicted-provenance",
    );
    let measured_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        measured.clone(),
        None,
    );
    let alternate_cells_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.alternate_cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        measured.clone(),
        None,
    );
    let alternate_frame_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.alternate_frame_id,
        measured,
        None,
    );
    let alternate_provenance_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        alternate_provenance,
        None,
    );
    let predicted_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        predicted,
        None,
    );
    let config = analysis_config(false);
    let node_id = NodeId::new("declared-cache-analysis").expect("node ID");
    let measured_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        node_id.clone(),
        &measured_input,
        &config,
    )
    .expect("measured node");
    let alternate_cells_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        node_id.clone(),
        &alternate_cells_input,
        &config,
    )
    .expect("alternate cells node");
    let alternate_frame_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        node_id.clone(),
        &alternate_frame_input,
        &config,
    )
    .expect("alternate frame node");
    let alternate_provenance_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        node_id.clone(),
        &alternate_provenance_input,
        &config,
    )
    .expect("alternate provenance node");
    let predicted_node =
        DeclaredMarkedAnalysisNode::new(&mut fixture.project, node_id, &predicted_input, &config)
            .expect("predicted node");
    let graph = WorkflowGraph::new([measured_node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");

    let measured_run = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &measured_node, &fixture.store)
        .expect("measured run");
    let alternate_cells_run = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &alternate_cells_node,
            &fixture.store,
        )
        .expect("alternate cells run");
    let alternate_frame_run = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &alternate_frame_node,
            &fixture.store,
        )
        .expect("alternate frame run");
    let alternate_provenance_run = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &alternate_provenance_node,
            &fixture.store,
        )
        .expect("alternate provenance run");
    let predicted_run = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &predicted_node,
            &fixture.store,
        )
        .expect("predicted run");
    for run in [
        &measured_run,
        &alternate_cells_run,
        &alternate_frame_run,
        &alternate_provenance_run,
        &predicted_run,
    ] {
        assert_eq!(run.cache_status, CacheStatus::Miss);
    }
    assert_ne!(measured_run.cache_key, alternate_cells_run.cache_key);
    assert_ne!(measured_run.cache_key, alternate_frame_run.cache_key);
    assert_ne!(measured_run.cache_key, alternate_provenance_run.cache_key);
    assert_ne!(measured_run.cache_key, predicted_run.cache_key);
    assert_ne!(
        measured_run
            .output
            .scalar_identity
            .cell_ids_logical_digest(),
        alternate_cells_run
            .output
            .scalar_identity
            .cell_ids_logical_digest()
    );
    assert_eq!(
        measured_run
            .output
            .scalar_identity
            .cell_ids_logical_digest(),
        alternate_frame_run
            .output
            .scalar_identity
            .cell_ids_logical_digest()
    );
    assert_ne!(
        measured_run.output.scalar_identity.coordinate_frame_id(),
        alternate_frame_run
            .output
            .scalar_identity
            .coordinate_frame_id()
    );
    for run in [
        &alternate_cells_run,
        &alternate_frame_run,
        &alternate_provenance_run,
        &predicted_run,
    ] {
        assert_ne!(
            measured_run
                .output
                .scalar_identity
                .declared_input_logical_digest(),
            run.output.scalar_identity.declared_input_logical_digest()
        );
    }
    let measured_result = without_timings(measured_run.output.result);
    for result in [
        alternate_cells_run.output.result,
        alternate_frame_run.output.result,
        alternate_provenance_run.output.result,
        predicted_run.output.result,
    ] {
        assert_eq!(without_timings(result), measured_result);
    }
}

#[test]
fn threshold_semantics_change_cache_identity_without_changing_legacy_numerics() {
    let mut fixture = fixture();
    fixture.pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let (inclusive_binary, inclusive_probability) = paired_declarations_with_threshold(
        &mut fixture,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let (strict_binary, strict_probability) = paired_declarations_with_threshold(
        &mut fixture,
        ProbabilityThresholdComparator::GreaterThan,
        0.6,
    );
    let inclusive_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        inclusive_binary,
        Some(inclusive_probability),
    );
    let strict_input = input(
        &fixture.project,
        &fixture.pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        strict_binary,
        Some(strict_probability),
    );
    let config = analysis_config(true);
    let node_id = NodeId::new("threshold-cache-analysis").expect("node ID");
    let inclusive_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        node_id.clone(),
        &inclusive_input,
        &config,
    )
    .expect("inclusive node");
    let strict_node =
        DeclaredMarkedAnalysisNode::new(&mut fixture.project, node_id, &strict_input, &config)
            .expect("strict node");
    let graph = WorkflowGraph::new([inclusive_node.spec().clone()]).expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");

    let inclusive_run = scheduler
        .run_single_with_store(
            &mut fixture.project,
            &graph,
            &inclusive_node,
            &fixture.store,
        )
        .expect("inclusive run");
    let strict_run = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &strict_node, &fixture.store)
        .expect("strict run");
    assert_eq!(inclusive_run.cache_status, CacheStatus::Miss);
    assert_eq!(strict_run.cache_status, CacheStatus::Miss);
    assert_ne!(inclusive_run.cache_key, strict_run.cache_key);
    assert_ne!(
        inclusive_run
            .output
            .scalar_identity
            .declared_input_logical_digest(),
        strict_run
            .output
            .scalar_identity
            .declared_input_logical_digest()
    );
    assert_eq!(
        without_timings(inclusive_run.output.result),
        without_timings(strict_run.output.result)
    );
}

#[test]
fn missing_or_corrupt_semantic_provenance_prevents_execution_and_success_commit() {
    for remove in [true, false] {
        let mut fixture = fixture();
        let binary = independent_binary(
            &mut fixture,
            MeasurementStatus::Measured,
            b"unavailable-binary-provenance",
        );
        let binary_provenance = binary.provenance_artifact_id();
        let input = input(
            &fixture.project,
            &fixture.pattern,
            &fixture.cell_ids,
            &fixture.slide_id,
            &fixture.frame_id,
            binary,
            None,
        );
        let config = analysis_config(false);
        let node_id = NodeId::new(if remove {
            "missing-declared-analysis"
        } else {
            "corrupt-declared-analysis"
        })
        .expect("node ID");
        let node = DeclaredMarkedAnalysisNode::new(&mut fixture.project, node_id, &input, &config)
            .expect("declared node");
        let graph = WorkflowGraph::new([node.spec().clone()]).expect("workflow graph");
        let scheduler = LocalScheduler::new(SchedulerLimits {
            max_inline_output_bytes: 16 * 1024 * 1024,
        })
        .expect("scheduler");
        let path = managed_path(fixture.root.path(), binary_provenance);
        if remove {
            std::fs::remove_file(path).expect("remove managed provenance");
        } else {
            std::fs::write(path, b"corrupt-provenance-bytes").expect("corrupt managed provenance");
        }

        let error = match scheduler.run_single_with_store(
            &mut fixture.project,
            &graph,
            &node,
            &fixture.store,
        ) {
            Ok(_) => panic!("unavailable semantic input must prevent execution"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            WorkflowError::SemanticInputIntegrity { artifact, .. }
                if artifact == binary_provenance
        ));
        assert_eq!(fixture.project.successful_run_count(), 0);
    }
}
