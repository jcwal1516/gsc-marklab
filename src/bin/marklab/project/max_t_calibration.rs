use std::borrow::Cow;

use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::MaxTCalibrationProjectArgs;
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::cohort::max_t_calibration::{self, MaxTCalibrationOutput};

const INPUT_KIND: &str = "application/vnd.marklab.source.max-t-calibration+csv;version=1";

pub(super) fn run(args: MaxTCalibrationProjectArgs) -> Result<(), CliError> {
    let MaxTCalibrationProjectArgs {
        project: project_path,
        input: input_path,
        group_a_count,
        family_sizes,
        alpha,
        maximum_assignments,
        maximum_assignment_endpoint_evaluations,
        memory_budget_mib,
        out: output_path,
    } = args;
    const LABEL: &str = "Max-T calibration source";
    let prepared = max_t_calibration::prepare(
        input_path.clone(),
        group_a_count,
        family_sizes,
        alpha,
        maximum_assignments,
        maximum_assignment_endpoint_evaluations,
        memory_budget_mib,
    )
    .map_err(|error| CliError::Input(error.to_string()))?;
    let source = Source::from_bytes(input_path, INPUT_KIND, &prepared.input_bytes)?;
    source.ensure_unchanged(LABEL)?;
    let path_identity = serde_json::to_vec(&prepared.input).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let configuration = serde_json::to_vec(&prepared.configuration)?;
    let execution_policy = format!(
        "serial;exhaustive-fixed-size-whole-patient-assignments;single-step;step-down;ordered-gatekeeping-step-down;group_a_count={};family_sizes={:?};alpha_bits={};maximum_assignments={};maximum_assignment_endpoint_evaluations={};memory_budget_mib={memory_budget_mib}",
        prepared.configuration.group_a_count,
        prepared.configuration.family_sizes,
        prepared.configuration.alpha.to_bits(),
        prepared.configuration.maximum_assignments,
        prepared.configuration.maximum_assignment_endpoint_evaluations,
    );
    let contract = NodeContract {
        id: "max-t-calibration",
        kind: "max_t_calibration",
        version: 1,
        configuration_digest: ContentDigest::from_framed([
            b"marklab-max-t-calibration-configuration-v1".as_slice(),
            path_identity.as_slice(),
            configuration.as_slice(),
        ]),
        execution_policy: Cow::Owned(execution_policy.into_bytes()),
        implementation_identity: "marklab-project-max-t-calibration-node-v1",
        output_kind: "application/vnd.marklab.cohort-max-t-calibration+json;version=1",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| max_t_calibration::execute(prepared).map_err(NodeError::execution),
        |prepared, output: &MaxTCalibrationOutput| {
            if output.validates(prepared) {
                Ok(())
            } else {
                Err(NodeError::decode(CliError::Input(
                    "decoded Max-T calibration result differs from its cache-bound request".into(),
                )))
            }
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_max_t_calibration", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "max-t-calibration",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
