use std::borrow::Cow;

use marklab_cohort::NoninferiorityDirection;
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::{
    CohortEquivalenceProjectArgs, CohortNoninferiorityProjectArgs, ProjectNoninferiorityDirection,
};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::cohort::{equivalence, noninferiority};

const INPUT_KIND: &str = "application/vnd.marklab.source.patient-effects+csv;version=1";
const LABEL: &str = "patient-effect source";

pub(super) fn run_equivalence(args: CohortEquivalenceProjectArgs) -> Result<(), CliError> {
    let CohortEquivalenceProjectArgs {
        project: project_path,
        input: input_path,
        lower_margin,
        upper_margin,
        alpha,
        margin_rationale,
        out: output_path,
    } = args;
    let source = Source::identify(input_path.clone(), INPUT_KIND)?;
    let prepared = equivalence::prepare(
        input_path,
        lower_margin,
        upper_margin,
        alpha,
        margin_rationale,
    )
    .map_err(|error| CliError::Input(error.to_string()))?;
    source.ensure_unchanged(LABEL)?;
    let configuration = serde_json::to_vec(&(
        prepared.spec.lower_margin,
        prepared.spec.upper_margin,
        prepared.spec.alpha,
        &prepared.spec.margin_rationale,
    ))?;
    let contract = NodeContract {
        id: "cohort-equivalence",
        kind: "patient_tost_equivalence",
        version: 1,
        configuration_digest: ContentDigest::from_bytes(&configuration),
        execution_policy: Cow::Borrowed(
            b"patient-unit-one-sample-student-t-tost-declared-margins-v1",
        ),
        implementation_identity: "marklab-project-cohort-equivalence-v1",
        output_kind: "application/vnd.marklab.cohort-equivalence+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| equivalence::execute(prepared).map_err(NodeError::execution),
        |prepared, output| {
            equivalence::validate_result(prepared, output)
                .map_err(|error| NodeError::decode(std::io::Error::other(error)))
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_equivalence", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-equivalence",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

pub(super) fn run_noninferiority(args: CohortNoninferiorityProjectArgs) -> Result<(), CliError> {
    let CohortNoninferiorityProjectArgs {
        project: project_path,
        input: input_path,
        direction,
        margin,
        alpha,
        margin_rationale,
        out: output_path,
    } = args;
    let direction = match direction {
        ProjectNoninferiorityDirection::HigherIsBetter => NoninferiorityDirection::HigherIsBetter,
        ProjectNoninferiorityDirection::LowerIsBetter => NoninferiorityDirection::LowerIsBetter,
    };
    let source = Source::identify(input_path.clone(), INPUT_KIND)?;
    let prepared = noninferiority::prepare(input_path, direction, margin, alpha, margin_rationale)
        .map_err(|error| CliError::Input(error.to_string()))?;
    source.ensure_unchanged(LABEL)?;
    let direction = match prepared.spec.direction {
        NoninferiorityDirection::HigherIsBetter => "higher_is_better",
        NoninferiorityDirection::LowerIsBetter => "lower_is_better",
    };
    let configuration = serde_json::to_vec(&(
        direction,
        prepared.spec.margin,
        prepared.spec.alpha,
        &prepared.spec.margin_rationale,
    ))?;
    let contract = NodeContract {
        id: "cohort-noninferiority",
        kind: "patient_student_t_noninferiority",
        version: 1,
        configuration_digest: ContentDigest::from_bytes(&configuration),
        execution_policy: Cow::Borrowed(
            b"patient-unit-one-sample-student-t-noninferiority-declared-margin-v1",
        ),
        implementation_identity: "marklab-project-cohort-noninferiority-v1",
        output_kind: "application/vnd.marklab.cohort-noninferiority+json;version=1",
        codec: Codec::ExactFloatJson,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        prepared,
        |prepared| noninferiority::execute(prepared).map_err(NodeError::execution),
        |prepared, output| {
            noninferiority::validate_result(prepared, output)
                .map_err(|error| NodeError::decode(std::io::Error::other(error)))
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_noninferiority", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-noninferiority",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
