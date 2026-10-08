use super::*;

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
