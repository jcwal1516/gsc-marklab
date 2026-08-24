use marklab::{
    compare_declared_marked_prepost, compare_marked_prepost, AnalysisEngine, AnalysisSection,
    BinaryMarkDeclaration, BinaryMarkOrigin, CacheStatus, DeclaredMarkedAnalysisNode,
    DeclaredMarkedAnalysisResult, DeclaredMarkedPrePostError, DeclaredScalarPatternInput,
    LocalScheduler, MeasurementStatus, NodeId, ProbabilityMarkDeclaration,
    ProbabilityThresholdComparator, ResultDocument, ScalarMarkId, SchedulerLimits, WorkflowGraph,
};

#[path = "support/declared_scalar.rs"]
mod support;
use support::*;

const ROW_LIMIT: usize = 4;

fn independent_binary(
    fixture: &mut Fixture,
    content: &[u8],
    mark_id: &str,
    label: &str,
    status: MeasurementStatus,
) -> BinaryMarkDeclaration {
    let provenance = publish_record(
        fixture,
        content,
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        binary_metadata(mark_id, label, status, "independent"),
    );
    BinaryMarkDeclaration::independent(
        ScalarMarkId::new(mark_id).expect("binary mark ID"),
        label,
        status,
        provenance,
    )
    .expect("independent binary declaration")
}

#[allow(clippy::too_many_arguments)]
fn paired_declarations(
    fixture: &mut Fixture,
    prefix: &str,
    binary_mark_id: &str,
    label: &str,
    probability_mark_id: &str,
    status: MeasurementStatus,
    comparator: ProbabilityThresholdComparator,
    threshold: f32,
) -> (BinaryMarkDeclaration, ProbabilityMarkDeclaration) {
    let probability_content = format!("{prefix}-probability-provenance");
    let probability_provenance = publish_record(
        fixture,
        probability_content.as_bytes(),
        MARK_SCHEMA,
        1,
        None,
        Vec::new(),
        probability_metadata(probability_mark_id, status),
    );
    let probability = ProbabilityMarkDeclaration::new(
        ScalarMarkId::new(probability_mark_id).expect("probability mark ID"),
        status,
        probability_provenance,
    )
    .expect("probability declaration");
    let comparator_name = match comparator {
        ProbabilityThresholdComparator::GreaterThan => "greater_than",
        ProbabilityThresholdComparator::GreaterThanOrEqual => "greater_than_or_equal",
    };
    let threshold_content = format!("{prefix}-threshold-provenance");
    let threshold_provenance = publish_record(
        fixture,
        threshold_content.as_bytes(),
        THRESHOLD_SCHEMA,
        1,
        None,
        vec![probability_provenance],
        threshold_metadata(
            binary_mark_id,
            probability_mark_id,
            comparator_name,
            threshold,
        ),
    );
    let binary_content = format!("{prefix}-binary-provenance");
    let binary_provenance = publish_record(
        fixture,
        binary_content.as_bytes(),
        MARK_SCHEMA,
        1,
        None,
        vec![probability_provenance, threshold_provenance],
        binary_metadata(binary_mark_id, label, status, "thresholded"),
    );
    let binary = BinaryMarkDeclaration::thresholded(
        ScalarMarkId::new(binary_mark_id).expect("binary mark ID"),
        label,
        status,
        binary_provenance,
        probability.mark_id().clone(),
        comparator,
        threshold,
        Some(threshold_provenance),
    )
    .expect("thresholded binary declaration");
    (binary, probability)
}

#[allow(clippy::too_many_arguments)]
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
    .expect("declared scalar input")
}

fn direct_output(
    fixture: &mut Fixture,
    timepoint: &str,
    binary: BinaryMarkDeclaration,
    probability: Option<ProbabilityMarkDeclaration>,
) -> DeclaredMarkedAnalysisResult {
    let mut pattern = fixture.pattern.clone();
    pattern.meta.timepoint = timepoint.into();
    if probability.is_some() {
        pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    }
    let mut config = analysis_config(probability.is_some());
    config.analysis.mark_label = binary.label().into();
    let declared = input(
        &fixture.project,
        &pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        binary,
        probability,
    );
    let run = AnalysisEngine::new(config)
        .expect("analysis engine")
        .analyze_declared_scalar_pattern(&declared)
        .expect("declared analysis");
    DeclaredMarkedAnalysisResult {
        result: run.result,
        mark_use: run.mark_use,
        scalar_identity: run.scalar_identity,
    }
}

#[test]
fn scheduler_outputs_compare_with_distinct_rows_frames_slides_and_evidence() {
    let mut fixture = fixture();
    let (pre_binary, pre_probability) = paired_declarations(
        &mut fixture,
        "pre",
        "mmr_loss",
        "MMR loss",
        "mmr_loss_probability",
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let (post_binary, post_probability) = paired_declarations(
        &mut fixture,
        "post",
        "mmr_loss",
        "MMR loss",
        "mmr_loss_probability",
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let mut pre_pattern = fixture.pattern.clone();
    pre_pattern.meta.timepoint = "pre".into();
    pre_pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let mut post_pattern = fixture.pattern.clone();
    post_pattern.meta.timepoint = "post".into();
    post_pattern.meta.slide_id = Some(fixture.alternate_slide_id.as_str().into());
    post_pattern.mark_prob = Some(vec![0.1, 0.8, 0.7, 0.2].into_boxed_slice());
    let pre_input = input(
        &fixture.project,
        &pre_pattern,
        &fixture.cell_ids,
        &fixture.slide_id,
        &fixture.frame_id,
        pre_binary,
        Some(pre_probability),
    );
    let post_input = input(
        &fixture.project,
        &post_pattern,
        &fixture.alternate_slide_cell_ids,
        &fixture.alternate_slide_id,
        &fixture.alternate_frame_id,
        post_binary,
        Some(post_probability),
    );
    let config = analysis_config(true);
    let pre_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("declared-pre").expect("pre node ID"),
        &pre_input,
        &config,
    )
    .expect("pre node");
    let post_node = DeclaredMarkedAnalysisNode::new(
        &mut fixture.project,
        NodeId::new("declared-post").expect("post node ID"),
        &post_input,
        &config,
    )
    .expect("post node");
    let graph = WorkflowGraph::new([pre_node.spec().clone(), post_node.spec().clone()])
        .expect("workflow graph");
    let scheduler = LocalScheduler::new(SchedulerLimits {
        max_inline_output_bytes: 16 * 1024 * 1024,
    })
    .expect("scheduler");
    let pre = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &pre_node, &fixture.store)
        .expect("pre output");
    let post = scheduler
        .run_single_with_store(&mut fixture.project, &graph, &post_node, &fixture.store)
        .expect("post output");
    assert_eq!(pre.cache_status, CacheStatus::Miss);
    assert_eq!(post.cache_status, CacheStatus::Miss);

    let legacy = compare_marked_prepost(&pre.output.result, &post.output.result);
    let comparison = compare_declared_marked_prepost(&pre.output, &post.output)
        .expect("declared pre/post comparison");
    assert_eq!(comparison.result, legacy);
    assert_eq!(comparison.pre_scalar_identity, pre.output.scalar_identity);
    assert_eq!(comparison.post_scalar_identity, post.output.scalar_identity);
    assert_eq!(comparison.pre_mark_use, pre.output.mark_use);
    assert_eq!(comparison.post_mark_use, post.output.mark_use);
    assert_eq!(comparison.pre_timepoint, "pre");
    assert_eq!(comparison.post_timepoint, "post");
    assert_ne!(
        comparison.pre_scalar_identity.cell_ids_logical_digest(),
        comparison.post_scalar_identity.cell_ids_logical_digest()
    );
    assert_ne!(
        comparison.pre_scalar_identity.owning_slide_id(),
        comparison.post_scalar_identity.owning_slide_id()
    );
    assert_ne!(
        comparison.pre_scalar_identity.coordinate_frame_id(),
        comparison.post_scalar_identity.coordinate_frame_id()
    );
    assert_ne!(
        comparison
            .pre_mark_use
            .binary_mark()
            .provenance_artifact_id(),
        comparison
            .post_mark_use
            .binary_mark()
            .provenance_artifact_id()
    );
    assert_ne!(
        comparison
            .pre_mark_use
            .probability_mark()
            .expect("pre probability")
            .provenance_artifact_id(),
        comparison
            .post_mark_use
            .probability_mark()
            .expect("post probability")
            .provenance_artifact_id()
    );
    let (pre_threshold, post_threshold) = match (
        comparison.pre_mark_use.binary_mark().origin(),
        comparison.post_mark_use.binary_mark().origin(),
    ) {
        (
            BinaryMarkOrigin::Thresholded {
                threshold_provenance_artifact_id: pre,
                ..
            },
            BinaryMarkOrigin::Thresholded {
                threshold_provenance_artifact_id: post,
                ..
            },
        ) => (pre, post),
        _ => panic!("thresholded declarations"),
    };
    assert_ne!(pre_threshold, post_threshold);

    let legacy_bytes = ResultDocument::marked_prepost(legacy)
        .to_json_pretty()
        .expect("legacy result bytes");
    let declared_bytes = ResultDocument::marked_prepost(comparison.result)
        .to_json_pretty()
        .expect("declared result bytes");
    assert_eq!(declared_bytes, legacy_bytes);
    assert!(!declared_bytes.contains("scalar_identity"));
    assert!(!declared_bytes.contains("mark_use"));
}

#[test]
fn publicly_constructed_runtime_mixes_fail_available_binding_checks() {
    let mut fixture = fixture();
    let pre_binary = independent_binary(
        &mut fixture,
        b"binding-pre",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let post_binary = independent_binary(
        &mut fixture,
        b"binding-post",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let alternate_binary = independent_binary(
        &mut fixture,
        b"binding-alternate",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let pre = direct_output(&mut fixture, "pre", pre_binary, None);
    let post = direct_output(&mut fixture, "post", post_binary, None);
    let alternate = direct_output(&mut fixture, "post", alternate_binary, None);

    let mut wrong_label = post.clone();
    wrong_label.result.mark_label = "forged label".into();
    assert_eq!(
        compare_declared_marked_prepost(&pre, &wrong_label),
        Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding)
    );

    let mut wrong_rows = post.clone();
    wrong_rows.result.n_cells += 1;
    assert_eq!(
        compare_declared_marked_prepost(&pre, &wrong_rows),
        Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding)
    );

    let mut mixed_mark_use = post.clone();
    mixed_mark_use.mark_use = alternate.mark_use.clone();
    assert_eq!(
        compare_declared_marked_prepost(&pre, &mixed_mark_use),
        Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding)
    );

    let mut mixed_identity = post;
    mixed_identity.scalar_identity = alternate.scalar_identity;
    assert_eq!(
        compare_declared_marked_prepost(&pre, &mixed_identity),
        Err(DeclaredMarkedPrePostError::InvalidPostRuntimeBinding)
    );
}

#[test]
fn semantic_mismatches_are_typed_before_legacy_comparison() {
    let mut fixture = fixture();
    let baseline_binary = independent_binary(
        &mut fixture,
        b"baseline",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let baseline = direct_output(&mut fixture, "pre", baseline_binary, None);

    let different_id = independent_binary(
        &mut fixture,
        b"different-id",
        "other_mark",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let different_id = direct_output(&mut fixture, "post", different_id, None);
    assert_eq!(
        compare_declared_marked_prepost(&baseline, &different_id),
        Err(DeclaredMarkedPrePostError::BinaryMarkIdMismatch)
    );

    let different_label = independent_binary(
        &mut fixture,
        b"different-label",
        "mmr_loss",
        "Other label",
        MeasurementStatus::Measured,
    );
    let different_label = direct_output(&mut fixture, "post", different_label, None);
    assert_eq!(
        compare_declared_marked_prepost(&baseline, &different_label),
        Err(DeclaredMarkedPrePostError::BinaryMarkLabelMismatch)
    );

    let predicted = independent_binary(
        &mut fixture,
        b"predicted",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::MorphologyPrediction,
    );
    let predicted = direct_output(&mut fixture, "post", predicted, None);
    assert_eq!(
        compare_declared_marked_prepost(&baseline, &predicted),
        Err(DeclaredMarkedPrePostError::MeasurementStatusMismatch)
    );

    let (routed_binary, routed_probability) = paired_declarations(
        &mut fixture,
        "routed",
        "mmr_loss",
        "MMR loss",
        "mmr_loss_probability",
        MeasurementStatus::Measured,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let routed = direct_output(
        &mut fixture,
        "post",
        routed_binary,
        Some(routed_probability),
    );
    assert_eq!(
        compare_declared_marked_prepost(&baseline, &routed),
        Err(DeclaredMarkedPrePostError::EndpointRoutingMismatch)
    );

    let (probability_binary, probability) = paired_declarations(
        &mut fixture,
        "probability-baseline",
        "mmr_loss",
        "MMR loss",
        "mmr_loss_probability",
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let probability_baseline =
        direct_output(&mut fixture, "pre", probability_binary, Some(probability));
    let (different_probability_binary, different_probability) = paired_declarations(
        &mut fixture,
        "different-probability",
        "mmr_loss",
        "MMR loss",
        "other_probability",
        MeasurementStatus::ImportedPrediction,
        ProbabilityThresholdComparator::GreaterThanOrEqual,
        0.5,
    );
    let different_probability = direct_output(
        &mut fixture,
        "post",
        different_probability_binary,
        Some(different_probability),
    );
    assert_eq!(
        compare_declared_marked_prepost(&probability_baseline, &different_probability),
        Err(DeclaredMarkedPrePostError::ProbabilityMarkIdMismatch)
    );

    for (prefix, comparator, threshold) in [
        (
            "different-comparator",
            ProbabilityThresholdComparator::GreaterThan,
            0.5,
        ),
        (
            "different-threshold",
            ProbabilityThresholdComparator::GreaterThanOrEqual,
            0.6,
        ),
    ] {
        let (binary, probability) = paired_declarations(
            &mut fixture,
            prefix,
            "mmr_loss",
            "MMR loss",
            "mmr_loss_probability",
            MeasurementStatus::ImportedPrediction,
            comparator,
            threshold,
        );
        let output = direct_output(&mut fixture, "post", binary, Some(probability));
        assert_eq!(
            compare_declared_marked_prepost(&probability_baseline, &output),
            Err(DeclaredMarkedPrePostError::BinaryOriginMismatch)
        );
    }
}

#[test]
fn legacy_flags_and_typed_unavailability_are_preserved_exactly() {
    let mut fixture = fixture();
    let pre_binary = independent_binary(
        &mut fixture,
        b"flags-pre",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let post_binary = independent_binary(
        &mut fixture,
        b"flags-post",
        "mmr_loss",
        "MMR loss",
        MeasurementStatus::Measured,
    );
    let mut pre = direct_output(&mut fixture, "pre", pre_binary, None);
    let mut post = direct_output(&mut fixture, "post", post_binary, None);
    post.result.case_id = "different-case".into();
    post.result.protein = "different-protein".into();
    pre.result.spectrum = AnalysisSection::Disabled;
    post.result.spectrum = AnalysisSection::NotApplicable;

    let legacy = compare_marked_prepost(&pre.result, &post.result);
    let declared = compare_declared_marked_prepost(&pre, &post).expect("declared comparison");
    assert_eq!(declared.result, legacy);
    assert!(!declared.result.status_flags.is_empty());
    assert!(matches!(
        declared.result.delta_xi_um,
        AnalysisSection::InsufficientData { .. }
    ));
}
