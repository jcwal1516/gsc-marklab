use super::*;

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
