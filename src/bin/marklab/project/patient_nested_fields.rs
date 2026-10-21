use std::{borrow::Cow, path::PathBuf};

use marklab_cohort::{
    execute_patient_nested_fields, prepare_patient_nested_fields, NestedFieldSpec,
    PreparedNestedFields,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::PatientNestedFieldsProjectArgs;
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_RESULT_BYTES,
};
use crate::cohort::patient_nested_fields::{self, PatientNestedFieldsOutput};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.patient-nested-field-endpoints+csv;version=1";

pub(super) fn run(args: PatientNestedFieldsProjectArgs) -> Result<(), CliError> {
    let PatientNestedFieldsProjectArgs {
        project: project_path,
        input: input_path,
        group_a,
        group_b,
        permutations,
        seed,
        alpha,
        maximum_patients,
        maximum_specimens,
        maximum_endpoints,
        maximum_permutation_endpoint_evaluations,
        memory_budget_mib,
        out: output_path,
    } = args;
    const LABEL: &str = "patient-nested field source";
    let memory_budget_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("memory budget overflowed".into()))?;
    let specification = NestedFieldSpec {
        group_a,
        group_b,
        permutations,
        seed,
        alpha,
        maximum_patients,
        maximum_specimens,
        maximum_endpoints,
        maximum_permutation_endpoint_evaluations,
        memory_budget_bytes,
    };
    let source = Source::identify(input_path.clone(), INPUT_KIND)?;
    let records = patient_nested_fields::read_records(&input_path)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let prepared = prepare_patient_nested_fields(&records, &specification)
        .map_err(|error| CliError::Input(error.to_string()))?;
    source.ensure_unchanged(LABEL)?;
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let configuration_digest = ContentDigest::from_framed([
        b"marklab-patient-nested-fields-configuration-v1".as_slice(),
        input_path_identity.as_slice(),
        specification.group_a.as_bytes(),
        specification.group_b.as_bytes(),
        specification.permutations.to_string().as_bytes(),
        specification.seed.to_string().as_bytes(),
        specification.alpha.to_bits().to_be_bytes().as_slice(),
        specification.maximum_patients.to_string().as_bytes(),
        specification.maximum_specimens.to_string().as_bytes(),
        specification.maximum_endpoints.to_string().as_bytes(),
        specification
            .maximum_permutation_endpoint_evaluations
            .to_string()
            .as_bytes(),
        memory_budget_mib.to_string().as_bytes(),
    ]);
    let execution_policy = format!(
        "serial;equal-weight-specimens-inside-patients;fold-internal-z-score-nearest-centroid;whole-patient-step-down-max-t;permutations={};seed={};alpha_bits={};maximum_patients={};maximum_specimens={};maximum_endpoints={};maximum_permutation_endpoint_evaluations={};memory_budget_mib={memory_budget_mib}",
        specification.permutations,
        specification.seed,
        specification.alpha.to_bits(),
        specification.maximum_patients,
        specification.maximum_specimens,
        specification.maximum_endpoints,
        specification.maximum_permutation_endpoint_evaluations,
    );
    let contract = NodeContract {
        id: "patient-nested-fields",
        kind: "patient_nested_fields",
        version: 1,
        configuration_digest,
        execution_policy: Cow::Owned(execution_policy.into_bytes()),
        implementation_identity: "marklab-project-patient-nested-fields-node-v2",
        output_kind: "application/vnd.marklab.cohort-patient-nested-fields+json;version=2",
        codec: Codec::Json,
    };
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        (input_path, specification, prepared),
        |(input_path, specification, prepared)| {
            let result = execute_patient_nested_fields(prepared, specification)
                .map_err(NodeError::execution)?;
            Ok(PatientNestedFieldsOutput::from_result(
                input_path.clone(),
                result,
            ))
        },
        |(input_path, specification, prepared): &(
            PathBuf,
            NestedFieldSpec,
            PreparedNestedFields,
        ),
         output: &PatientNestedFieldsOutput| {
            if output.matches_request(input_path, prepared, specification) {
                Ok(())
            } else {
                Err(NodeError::decode(CliError::Input(
                    "decoded patient-nested field result differs from its cache-bound request"
                        .into(),
                )))
            }
        },
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_patient_nested_fields", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "patient-nested-fields",
        &output_path,
        &run.output,
        run.cache_status,
    )
}
