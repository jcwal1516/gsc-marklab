use marklab_bayes::NutsSamplingSpec;
use marklab_workflow::NodeError;

use super::{
    bayes,
    durable::{self, Codec, Source, SourceNode},
    ArbitraryWindowIppSpatialPpcProjectArgs, CliError, StaticBackendWorkflow, MAXIMUM_RESULT_BYTES,
};

pub(super) fn run(arguments: ArbitraryWindowIppSpatialPpcProjectArgs) -> Result<(), CliError> {
    const LABEL: &str = "arbitrary-window IPP spatial PPC source";
    let backend = StaticBackendWorkflow::PymcArbitraryWindowIppSpatialPpc.descriptor();
    let sources = Source::identify_all(
        [
            arguments.events,
            arguments.event_membership,
            arguments.quadrature,
            arguments.window,
        ],
        backend.input_kinds,
    )?;
    let prepared = bayes::prepare_arbitrary_window_ipp_spatial_ppc(bayes::SpatialPpcParameters {
        events: sources[0].path().to_owned(),
        event_membership: sources[1].path().to_owned(),
        quadrature: sources[2].path().to_owned(),
        window: sources[3].path().to_owned(),
        intercept_prior_mean: arguments.intercept_prior_mean,
        intercept_prior_sd: arguments.intercept_prior_sd,
        coefficient_prior_mean: arguments.coefficient_prior_mean,
        coefficient_prior_sd: arguments.coefficient_prior_sd,
        sampling: NutsSamplingSpec {
            chains: arguments.chains,
            tune_per_chain: arguments.tune,
            draws_per_chain: arguments.draws,
            target_accept: arguments.target_accept,
            seed: arguments.seed,
        },
        prediction_seed: arguments.prediction_seed,
        neighbor_radius_um: arguments.neighbor_radius_um,
        maximum_events: arguments.maximum_events,
        maximum_quadrature_nodes: arguments.maximum_quadrature_nodes,
        maximum_neighbor_pairs: arguments.maximum_neighbor_pairs,
        maximum_draw_node_work: arguments.maximum_draw_node_work,
        timeout_seconds: arguments.timeout_seconds,
    })?;
    Source::ensure_all_unchanged(&sources, LABEL)?;
    let contract = backend.contract(
        prepared.backend_contract(),
        prepared.request_bytes(),
        Codec::ExactFloatJson,
    )?;
    let node = SourceNode::new(
        contract,
        LABEL,
        sources,
        prepared,
        |prepared| {
            bayes::execute_arbitrary_window_ipp_spatial_ppc(prepared).map_err(NodeError::execution)
        },
        |prepared, output: &bayes::SpatialPpcResult| {
            bayes::validate_arbitrary_window_ipp_spatial_ppc(prepared, output)
                .map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &arguments.project,
        &node,
        backend.result_schema(),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "arbitrary-window-ipp-spatial-ppc",
        &arguments.out,
        &run.output,
        run.cache_status,
    )
}
