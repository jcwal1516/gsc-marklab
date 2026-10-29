use super::durable;

use super::{
    bayes, ArbitraryWindowIppFitProjectArgs, ArbitraryWindowIppFitResult,
    ArbitraryWindowIppLikelihoodResult, ArbitraryWindowIppProjectArgs,
    ArbitraryWindowLgcpFitProjectArgs, CliError, Codec, ContentDigest, Cow, InProject,
    NodeContract, NodeError, ReplicatedArbitraryWindowLgcpFitProjectArgs,
    ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs, Source, SourceNode,
    StaticBackendWorkflow, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

pub(super) fn run_arbitrary_window_ipp(
    args: ArbitraryWindowIppProjectArgs,
) -> Result<(), CliError> {
    let ArbitraryWindowIppProjectArgs {
        project: project_path,
        events: events_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept,
        coefficient,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_work,
        maximum_retained_bytes,
        out: output_path,
    } = args;
    const LABEL: &str = "arbitrary-window IPP source";
    let sources = Source::identify_all(
        [events_path, quadrature_path, window_path],
        &[
            "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
            "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
            "application/vnd.marklab.source.observation-window+geojson;version=1",
        ],
    )?;
    let prepared = bayes::prepare_arbitrary_window_ipp(
        sources[0].path().to_owned(),
        sources[1].path().to_owned(),
        sources[2].path().to_owned(),
        intercept,
        coefficient,
        marklab::spatial::ArbitraryWindowIppLimits {
            maximum_events,
            maximum_quadrature_nodes,
            maximum_work,
            maximum_retained_bytes,
        },
    )?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "intercept": intercept,
        "coefficient": coefficient,
        "maximum_events": maximum_events,
        "maximum_quadrature_nodes": maximum_quadrature_nodes,
        "maximum_work": maximum_work,
        "maximum_retained_bytes": maximum_retained_bytes,
    }))?;
    let contract = NodeContract {
        id: "arbitrary-window-ipp-likelihood",
        kind: "fixed_point_process_likelihood",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-project-arbitrary-window-ipp-likelihood-node-v1".as_slice(),
            configuration_bytes.as_slice(),
        ]),
        execution_policy: Cow::Borrowed(b"native-safe-rust-weighted-arbitrary-window-ipp-v1"),
        implementation_identity: "marklab-project-arbitrary-window-ipp-likelihood-node-v1",
        output_kind:
            "application/vnd.marklab.arbitrary-window-ipp-likelihood-result+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_ipp(prepared).map_err(NodeError::execution),
        |prepared, output: &ArbitraryWindowIppLikelihoodResult| {
            output
                .validate_for_spec(&prepared.window, &prepared.spec)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.arbitrary_window_ipp_likelihood_result", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "arbitrary-window-ipp-likelihood",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_arbitrary_window_ipp_fit(
    args: ArbitraryWindowIppFitProjectArgs,
) -> Result<(), CliError> {
    let ArbitraryWindowIppFitProjectArgs {
        project: project_path,
        events: events_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        nuts,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "arbitrary-window IPP fit source";
    let backend = StaticBackendWorkflow::PymcArbitraryWindowIpp.descriptor();
    let sources = Source::identify_all(
        [events_path, quadrature_path, window_path],
        backend.input_kinds,
    )?;
    let prepared =
        bayes::prepare_arbitrary_window_ipp_fit(crate::bayes::ArbitraryWindowIppFitParameters {
            events_path: sources[0].path().to_owned(),
            quadrature_path: sources[1].path().to_owned(),
            window_path: sources[2].path().to_owned(),
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            sampling,
            maximum_events,
            maximum_quadrature_nodes,
            maximum_draw_node_work,
            timeout_seconds,
        })?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_ipp_fit(prepared).map_err(NodeError::execution),
        |prepared, output: &ArbitraryWindowIppFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "fit-arbitrary-window-ipp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_arbitrary_window_lgcp(
    args: ArbitraryWindowLgcpFitProjectArgs,
) -> Result<(), CliError> {
    let ArbitraryWindowLgcpFitProjectArgs {
        project: project_path,
        events: events_path,
        event_membership: membership_path,
        quadrature: quadrature_path,
        window: window_path,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_mean,
        coefficient_prior_sd,
        field_amplitude,
        field_length_scale_um,
        jitter,
        nuts,
        maximum_events,
        maximum_quadrature_nodes,
        maximum_draw_node_work,
        prediction_replicates,
        prediction_seed,
        maximum_predictive_points,
        neighbor_radius_um,
        maximum_neighbor_pairs,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "arbitrary-window LGCP source";
    let backend = StaticBackendWorkflow::PymcArbitraryWindowLgcp.descriptor();
    let sources = Source::identify_all(
        [events_path, membership_path, quadrature_path, window_path],
        backend.input_kinds,
    )?;
    let prepared =
        bayes::prepare_arbitrary_window_lgcp_fit(crate::bayes::ArbitraryWindowLgcpFitArgs {
            events: sources[0].path().to_owned(),
            event_membership: sources[1].path().to_owned(),
            quadrature: sources[2].path().to_owned(),
            window: sources[3].path().to_owned(),
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_mean,
            coefficient_prior_sd,
            field_amplitude,
            field_length_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_events,
            maximum_quadrature_nodes,
            maximum_draw_node_work,
            prediction_replicates,
            prediction_seed,
            maximum_predictive_points,
            neighbor_radius_um,
            maximum_neighbor_pairs,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        })?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_arbitrary_window_lgcp_fit(prepared).map_err(NodeError::execution),
        |prepared, output: &bayes::ArbitraryWindowLgcpFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "arbitrary-window-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_replicated_arbitrary_window_lgcp(
    args: ReplicatedArbitraryWindowLgcpFitProjectArgs,
) -> Result<(), CliError> {
    let ReplicatedArbitraryWindowLgcpFitProjectArgs {
        project: project_path,
        input: input_path,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude,
        field_length_scale_um,
        jitter,
        nuts,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "replicated LGCP source";
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcp.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_fit(
        crate::bayes::ReplicatedArbitraryWindowLgcpFitArgs {
            input: input_path,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            covariate_effect_prior_sd,
            patient_sd_prior_scale,
            pattern_sd_prior_scale,
            field_amplitude,
            field_length_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients,
            maximum_patterns,
            maximum_nodes_per_pattern,
            maximum_total_nodes,
            maximum_total_events,
            maximum_draw_node_work,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_lgcp_fit(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowLgcpFitResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_replicated_arbitrary_window_lgcp_inferred_kernel(
    args: ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs,
) -> Result<(), CliError> {
    let ReplicatedArbitraryWindowLgcpInferredKernelProjectArgs {
        project: project_path,
        input: input_path,
        reference_group,
        comparison_group,
        intercept_prior_mean,
        intercept_prior_sd,
        group_effect_prior_sd,
        covariate_effect_prior_sd,
        patient_sd_prior_scale,
        pattern_sd_prior_scale,
        field_amplitude_prior_scale,
        field_length_scale_prior_scale_um,
        jitter,
        nuts,
        maximum_patients,
        maximum_patterns,
        maximum_nodes_per_pattern,
        maximum_total_nodes,
        maximum_total_events,
        maximum_draw_node_work,
        maximum_kernel_cube_work,
        maximum_tree_depth,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "inferred-kernel replicated LGCP source";
    let backend =
        StaticBackendWorkflow::PymcReplicatedArbitraryWindowLgcpInferredKernel.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_replicated_arbitrary_window_lgcp_inferred_kernel(
        crate::bayes::ReplicatedArbitraryWindowLgcpInferredKernelArgs {
            input: input_path,
            reference_group,
            comparison_group,
            intercept_prior_mean,
            intercept_prior_sd,
            group_effect_prior_sd,
            covariate_effect_prior_sd,
            patient_sd_prior_scale,
            pattern_sd_prior_scale,
            field_amplitude_prior_scale,
            field_length_scale_prior_scale_um,
            jitter,
            chains: sampling.chains,
            tune: sampling.tune_per_chain,
            draws: sampling.draws_per_chain,
            target_accept: sampling.target_accept,
            seed: sampling.seed,
            maximum_patients,
            maximum_patterns,
            maximum_nodes_per_pattern,
            maximum_total_nodes,
            maximum_total_events,
            maximum_draw_node_work,
            maximum_kernel_cube_work,
            maximum_tree_depth,
            timeout_seconds,
            out: std::path::PathBuf::new(),
        },
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_lgcp_inferred_kernel(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowLgcpInferredKernelResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-lgcp-inferred-kernel",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp(
    args: InProject<bayes::ReplicatedArbitraryWindowMultitypeLgcpArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated multitype LGCP source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcp.descriptor();
    let source = Source::identify(arguments.input.clone(), backend.input_kinds[0])?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared = bayes::prepare_replicated_arbitrary_window_multitype_lgcp(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_multitype_lgcp(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowMultitypeLgcpResult| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-multitype-lgcp",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_conditional_multitype_mark(
    args: InProject<bayes::ConditionalMultitypeMarkArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "conditional multitype source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcConditionalMultitypeMark.descriptor();
    let input_bytes = crate::input_file::read_regular_file(
        &arguments.input,
        MAXIMUM_INPUT_BYTES,
        "conditional multitype source exceeds input ceiling",
    )?;
    let source = Source::from_bytes(
        arguments.input.clone(),
        backend.input_kinds[0],
        &input_bytes,
    )?;
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::conditional_multitype_mark_backend_contract()?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "reference_type": &arguments.reference_type,
        "radius_um": arguments.radius_um,
        "intercept_prior_sd": arguments.intercept_prior_sd,
        "interaction_prior_sd": arguments.interaction_prior_sd,
        "chains": arguments.chains,
        "tune": arguments.tune,
        "draws": arguments.draws,
        "target_accept": arguments.target_accept,
        "seed": arguments.seed,
        "maximum_points": arguments.maximum_points,
        "maximum_types": arguments.maximum_types,
        "maximum_neighbor_visits": arguments.maximum_neighbor_visits,
        "maximum_edges": arguments.maximum_edges,
        "maximum_draw_parameter_work": arguments.maximum_draw_parameter_work,
        "maximum_working_bytes": arguments.maximum_working_bytes,
        "maximum_tree_depth": arguments.maximum_tree_depth,
        "timeout_seconds": arguments.timeout_seconds,
    }))?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &request_backend,
        &configuration_bytes,
        Codec::ExactFloatJson,
    )?;
    let output_path = std::mem::take(&mut arguments.out);
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        (arguments, input_sha256, request_backend),
        |(arguments, _, _)| {
            bayes::execute_conditional_multitype_mark(arguments.clone())
                .map_err(NodeError::execution)
        },
        |(arguments, input_sha256, request_backend),
         output: &bayes::ConditionalMultitypeMarkResult| {
            output
                .validate_for(
                    input_sha256,
                    &arguments.reference_type,
                    arguments.radius_um,
                    request_backend,
                )
                .map_err(bayes::protocol_error)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "conditional-multitype-mark",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(
    args: InProject<bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated multitype inferred-kernel source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel
        .descriptor();
    let source = Source::identify(arguments.input.clone(), backend.input_kinds[0])?;
    let output_path = std::mem::take(&mut arguments.out);
    let prepared =
        bayes::prepare_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(arguments)?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        prepared.backend(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_replicated_arbitrary_window_multitype_lgcp_inferred_kernel(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &bayes::ReplicatedArbitraryWindowMultitypeLgcpInferredKernelOutput| {
            prepared.validate_result(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-arbitrary-window-multitype-lgcp-inferred-kernel",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_replicated_conditional_multitype_mark(
    args: InProject<bayes::ReplicatedConditionalMultitypeMarkArgs>,
) -> Result<(), CliError> {
    const LABEL: &str = "replicated conditional-mark source";
    let InProject {
        project: project_path,
        command: mut arguments,
    } = args;
    let backend = StaticBackendWorkflow::PymcReplicatedConditionalMultitypeMark.descriptor();
    let input_bytes = crate::input_file::read_regular_file(
        &arguments.input,
        MAXIMUM_INPUT_BYTES,
        "replicated conditional-mark source exceeds durable input ceiling",
    )?;
    let source = Source::from_bytes(
        arguments.input.clone(),
        backend.input_kinds[0],
        &input_bytes,
    )?;
    let input_sha256 = marklab_bayes::sha256_hex(&input_bytes);
    let request_backend = bayes::replicated_conditional_multitype_mark_backend_contract()?;
    let configuration_bytes = serde_json::to_vec(&serde_json::json!({
        "reference_group": &arguments.reference_group,
        "reference_type": &arguments.reference_type,
        "radius_um": arguments.radius_um,
        "intercept_prior_sd": arguments.intercept_prior_sd,
        "interaction_prior_sd": arguments.interaction_prior_sd,
        "group_effect_prior_sd": arguments.group_effect_prior_sd,
        "patient_sd_prior_scale": arguments.patient_sd_prior_scale,
        "pattern_sd_prior_scale": arguments.pattern_sd_prior_scale,
        "chains": arguments.chains,
        "tune": arguments.tune,
        "draws": arguments.draws,
        "target_accept": arguments.target_accept,
        "seed": arguments.seed,
        "maximum_patients": arguments.maximum_patients,
        "maximum_patterns": arguments.maximum_patterns,
        "maximum_points": arguments.maximum_points,
        "maximum_types": arguments.maximum_types,
        "maximum_neighbor_visits": arguments.maximum_neighbor_visits,
        "maximum_edges": arguments.maximum_edges,
        "maximum_draw_parameter_work": arguments.maximum_draw_parameter_work,
        "maximum_working_bytes": arguments.maximum_working_bytes,
        "maximum_tree_depth": arguments.maximum_tree_depth,
        "timeout_seconds": arguments.timeout_seconds,
    }))?;
    source.ensure_unchanged(LABEL)?;
    let contract = backend.contract(
        &request_backend,
        &configuration_bytes,
        Codec::ExactFloatJson,
    )?;
    let output_path = std::mem::take(&mut arguments.out);
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        (arguments, input_sha256, request_backend),
        |(arguments, _, _)| {
            bayes::execute_replicated_conditional_multitype_mark(arguments.clone())
                .map_err(NodeError::execution)
        },
        |(arguments, input_sha256, request_backend),
         output: &bayes::ReplicatedConditionalMultitypeMarkResult| {
            output
                .validate_for(
                    input_sha256,
                    &arguments.reference_group,
                    &arguments.reference_type,
                    arguments.radius_um,
                    request_backend,
                )
                .map_err(bayes::protocol_error)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "replicated-conditional-multitype-mark",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
