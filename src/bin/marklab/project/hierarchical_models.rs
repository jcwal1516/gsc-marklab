use super::durable;

use super::{
    bayes, cache_status_name, BetaBinomialGroupGenderRegressionProjectArgs,
    BetaBinomialGroupGenderRegressionWorkerResult,
    BetaBinomialGroupGenderSlideHierarchyProjectArgs,
    BetaBinomialGroupGenderSlideHierarchyWorkerResult, BetaBinomialGroupRegressionProjectArgs,
    BetaBinomialGroupRegressionWorkerResult, BetaBinomialHierarchyProjectArgs,
    BetaBinomialHierarchyWorkerResult, CliError, Codec, DirichletMultinomialGroupProjectArgs,
    DirichletMultinomialGroupWorkerResult, FusedGromovWassersteinProjectArgs,
    FusedGromovWassersteinWorkerResult, GriddedLgcpFitWorkerResult, GriddedLgcpModelArgs,
    GriddedLgcpProjectArgs, HierarchicalNormalProjectArgs, HierarchicalWorkerResult,
    HurdleBetaBinomialGroupProjectArgs, HurdleBetaBinomialGroupWorkerResult, NodeError,
    NonproportionalOrdinalGroupSiteProjectArgs, NonproportionalOrdinalGroupSiteWorkerResult,
    NormalMeanProjectArgs, OrdinalGroupProjectArgs, OrdinalGroupSiteHierarchyProjectArgs,
    OrdinalGroupSiteHierarchyWorkerResult, OrdinalGroupWorkerResult, Source, SourceNode,
    StaticBackendWorkflow, StudentTHierarchyProjectArgs, StudentTHierarchyWorkerResult,
    WorkerResult, MAXIMUM_RESULT_BYTES,
};

pub(super) fn run_normal_mean(args: NormalMeanProjectArgs) -> Result<(), CliError> {
    let NormalMeanProjectArgs {
        project: project_path,
        input: input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        nuts,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    let backend = StaticBackendWorkflow::PymcNormalMean.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_normal_mean(
        input_path,
        prior_mean,
        prior_sd,
        known_sigma,
        sampling,
        timeout_seconds,
    )?;
    source.ensure_unchanged("normal-mean input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "normal-mean input",
        vec![source],
        prepared,
        |prepared| bayes::execute_normal_mean(prepared).map_err(NodeError::execution),
        |prepared, output: &WorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("normal-mean", &output_path, &fit, run.cache_status)
}

pub(super) fn run_hierarchical_normal(args: HierarchicalNormalProjectArgs) -> Result<(), CliError> {
    let HierarchicalNormalProjectArgs {
        model,
        project: project_path,
        input: input_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcHierarchicalNormal.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_hierarchical(input_path, model)?;
    source.ensure_unchanged("hierarchical-normal input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "hierarchical-normal input",
        vec![source],
        prepared,
        |prepared| bayes::execute_hierarchical(prepared).map_err(NodeError::execution),
        |prepared, output: &HierarchicalWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("hierarchical-normal", &output_path, &fit, run.cache_status)
}

pub(super) fn run_beta_binomial_hierarchy(
    args: BetaBinomialHierarchyProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_hierarchy(model)?;
    source.ensure_unchanged("beta-binomial-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-hierarchy input",
        vec![source],
        prepared,
        |prepared| bayes::execute_beta_binomial_hierarchy(prepared).map_err(NodeError::execution),
        |prepared, output: &BetaBinomialHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_beta_binomial_group_regression(
    args: BetaBinomialGroupRegressionProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupRegressionProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupRegression.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_regression(model)?;
    source.ensure_unchanged("beta-binomial-group-regression input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-regression input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_regression(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupRegressionWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-regression",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_dirichlet_multinomial_group(
    args: DirichletMultinomialGroupProjectArgs,
) -> Result<(), CliError> {
    let DirichletMultinomialGroupProjectArgs {
        project: project_path,
        model,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcDirichletMultinomialGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_dirichlet_multinomial_group(model)?;
    source.ensure_unchanged("dirichlet-multinomial-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "dirichlet-multinomial-group input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_dirichlet_multinomial_group(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &DirichletMultinomialGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "dirichlet-multinomial-group",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_ordinal_group(args: OrdinalGroupProjectArgs) -> Result<(), CliError> {
    let OrdinalGroupProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcOrdinalGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group(model)?;
    source.ensure_unchanged("ordinal-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "ordinal-group input",
        vec![source],
        prepared,
        |prepared| bayes::execute_ordinal_group(prepared).map_err(NodeError::execution),
        |prepared, output: &OrdinalGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("ordinal-group", &output_path, &fit, run.cache_status)
}

pub(super) fn run_ordinal_group_site_hierarchy(
    args: OrdinalGroupSiteHierarchyProjectArgs,
) -> Result<(), CliError> {
    let OrdinalGroupSiteHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcOrdinalGroupSiteHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_ordinal_group_site_hierarchy(model)?;
    source.ensure_unchanged("ordinal-group-site-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "ordinal-group-site-hierarchy input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_ordinal_group_site_hierarchy(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &OrdinalGroupSiteHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "ordinal-group-site-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_nonproportional_ordinal_group_site(
    args: NonproportionalOrdinalGroupSiteProjectArgs,
) -> Result<(), CliError> {
    let NonproportionalOrdinalGroupSiteProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcNonproportionalOrdinalGroupSite.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_nonproportional_ordinal_group_site(model)?;
    source.ensure_unchanged("nonproportional ordinal input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "nonproportional ordinal input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_nonproportional_ordinal_group_site(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &NonproportionalOrdinalGroupSiteWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "nonproportional-ordinal-group-site",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_hurdle_beta_binomial_group(
    args: HurdleBetaBinomialGroupProjectArgs,
) -> Result<(), CliError> {
    let HurdleBetaBinomialGroupProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcHurdleBetaBinomialGroup.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_hurdle_beta_binomial_group(model)?;
    source.ensure_unchanged("hurdle-beta-binomial-group input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "hurdle-beta-binomial-group input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_hurdle_beta_binomial_group(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &HurdleBetaBinomialGroupWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "hurdle-beta-binomial-group",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_beta_binomial_group_gender_regression(
    args: BetaBinomialGroupGenderRegressionProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderRegressionProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderRegression.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_regression(model)?;
    source.ensure_unchanged("beta-binomial-group-gender-regression input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-gender-regression input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_gender_regression(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupGenderRegressionWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-gender-regression",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_beta_binomial_group_gender_slide_hierarchy(
    args: BetaBinomialGroupGenderSlideHierarchyProjectArgs,
) -> Result<(), CliError> {
    let BetaBinomialGroupGenderSlideHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcBetaBinomialGroupGenderSlideHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_beta_binomial_group_gender_slide_hierarchy(model)?;
    source.ensure_unchanged("beta-binomial-group-gender-slide-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "beta-binomial-group-gender-slide-hierarchy input",
        vec![source],
        prepared,
        |prepared| {
            bayes::execute_beta_binomial_group_gender_slide_hierarchy(prepared)
                .map_err(NodeError::execution)
        },
        |prepared, output: &BetaBinomialGroupGenderSlideHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish(
        "beta-binomial-group-gender-slide-hierarchy",
        &output_path,
        &fit,
        run.cache_status,
    )
}

pub(super) fn run_student_t_hierarchy(args: StudentTHierarchyProjectArgs) -> Result<(), CliError> {
    let StudentTHierarchyProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PymcStudentTHierarchy.descriptor();
    let source = Source::identify(model.input.clone(), backend.input_kinds[0])?;
    let prepared = bayes::prepare_student_t_hierarchy(model)?;
    source.ensure_unchanged("student-t-hierarchy input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        "student-t-hierarchy input",
        vec![source],
        prepared,
        |prepared| bayes::execute_student_t_hierarchy(prepared).map_err(NodeError::execution),
        |prepared, output: &StudentTHierarchyWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_result(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("student-t-hierarchy", &output_path, &fit, run.cache_status)
}

pub(super) fn run_gridded_lgcp(args: GriddedLgcpProjectArgs) -> Result<(), CliError> {
    let GriddedLgcpProjectArgs {
        model,
        project: project_path,
        out: output_path,
    } = args;
    const LABEL: &str = "gridded-LGCP input";
    let backend = StaticBackendWorkflow::PymcGriddedLgcp.descriptor();
    let sources = Source::identify_all(
        [model.events.clone(), model.grid.clone()],
        backend.input_kinds,
    )?;
    let prepared = bayes::prepare_gridded_lgcp(
        None,
        GriddedLgcpModelArgs {
            events: sources[0].path().to_owned(),
            grid: sources[1].path().to_owned(),
            ..model
        },
    )?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::Json,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| bayes::execute_gridded_lgcp(prepared).map_err(NodeError::execution),
        |prepared, output: &GriddedLgcpFitWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    let prepared = node.payload();
    let fit = run
        .output
        .into_fit(prepared.request.clone(), prepared.input_identity.clone());
    durable::publish("gridded-lgcp", &output_path, &fit, run.cache_status)
}

pub(super) fn run_fused_gromov_wasserstein(
    args: FusedGromovWassersteinProjectArgs,
) -> Result<(), CliError> {
    let FusedGromovWassersteinProjectArgs {
        project: project_path,
        input: input_path,
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
        out: output_path,
    } = args;
    let backend = StaticBackendWorkflow::PotFusedGromovWasserstein.descriptor();
    let source = Source::identify(input_path.clone(), backend.input_kinds[0])?;
    let prepared = bayes::fused_gromov::prepare(crate::bayes::fused_gromov::Parameters {
        input: input_path,
        alpha,
        epsilon,
        feature_scale,
        structure_scale,
        tolerance,
        maximum_iterations,
        timeout_seconds,
    })?;
    source.ensure_unchanged("FGW input")?;
    let contract = backend.contract(
        &prepared.request.backend,
        &prepared.request_bytes,
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        "FGW input",
        vec![source],
        prepared,
        |prepared| bayes::fused_gromov::execute(prepared).map_err(NodeError::execution),
        |prepared, output: &FusedGromovWassersteinWorkerResult| {
            output
                .validate(&prepared.request, &prepared.request_sha256)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    bayes::fused_gromov::publish_result(&output_path, node.payload(), run.output)?;
    eprintln!(
        "project fused-gromov-wasserstein cache_status={}",
        cache_status_name(run.cache_status)
    );
    Ok(())
}
