use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::SpatialVaryingCoefficientProjectArgs;
use super::{
    bayes::spatial_varying_coefficient,
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str = "application/vnd.marklab.source.spatial-varying-coefficient-csv;version=1";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-spatial-varying-coefficient-pymc-node-v1";

pub(super) fn run(args: SpatialVaryingCoefficientProjectArgs) -> Result<(), CliError> {
    let SpatialVaryingCoefficientProjectArgs {
        project: project_path,
        input: input_path,
        global_predictor_name,
        spatial_predictor_name,
        intercept_prior_mean,
        intercept_prior_sd,
        coefficient_prior_sd,
        amplitude_prior_sd,
        length_scale_prior_sd_um,
        known_noise_sd,
        jitter,
        nuts,
        timeout_seconds,
        out: output_path,
    } = args;
    let sampling = nuts.spec();
    const LABEL: &str = "spatial coefficient input";
    let source = Source::identify(input_path.clone(), INPUT_KIND)?;
    let input_path_identity = input_path.to_string_lossy().into_owned().into_bytes();
    let prepared = spatial_varying_coefficient::prepare(
        crate::bayes::spatial_varying_coefficient::Parameters {
            input_path,
            global_predictor_name,
            spatial_predictor_name,
            intercept_prior_mean,
            intercept_prior_sd,
            coefficient_prior_sd,
            amplitude_prior_sd,
            length_scale_prior_sd_um,
            known_noise_sd,
            jitter,
            sampling,
            timeout_seconds,
        },
    )?;
    source.ensure_unchanged(LABEL)?;
    let contract = NodeContract {
        id: "spatial-varying-coefficient",
        kind: "pymc_spatial_varying_coefficient_fit",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            IMPLEMENTATION_IDENTITY.as_bytes(),
            input_path_identity.as_slice(),
            prepared.request_bytes(),
        ]),
        execution_policy: Cow::Borrowed(b"pinned-pymc-spatial-varying-coefficient-v1"),
        implementation_identity: IMPLEMENTATION_IDENTITY,
        output_kind:
            "application/vnd.marklab.bayesian-spatial-varying-coefficient-fit+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| {
            let output =
                spatial_varying_coefficient::execute(prepared).map_err(NodeError::execution)?;
            serde_json::to_value(output).map_err(NodeError::encoding)
        },
        |prepared, output: &serde_json::Value| {
            prepared.validate_output(output).map_err(NodeError::decode)
        },
    )?;
    let run = durable::execute_python_node(
        &project_path,
        &node,
        ("marklab.bayesian_spatial_varying_coefficient_fit", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "spatial-varying-coefficient",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
