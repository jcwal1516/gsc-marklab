use crate::codecs::hierarchical_max_t::{read_input, Input, Output};

use std::{borrow::Cow, collections::HashSet, io, path::Path};

use marklab_cohort::{
    hierarchical_gatekeeping_max_t, MaxTCorrection, MaxTPermutationSpec, OrderedEndpointFamily,
    PatientEndpointVector,
};
use marklab_project::ContentDigest;
use marklab_workflow::NodeError;

use super::cli_schema::{CohortHierarchicalMaxTLimits, CohortHierarchicalMaxTProjectArgs};
use super::{
    durable::{self, Codec, NodeContract, Source, SourceNode},
    CliError, MAXIMUM_INPUT_BYTES, MAXIMUM_RESULT_BYTES,
};

const INPUT_KIND: &str =
    "application/vnd.marklab.source.hierarchical-max-t-patient-endpoints-csv;version=1";
const OUTPUT_KIND: &str = "application/vnd.marklab.cohort-hierarchical-max-t+json;version=2";
const IMPLEMENTATION_IDENTITY: &str = "marklab-project-cohort-hierarchical-max-t-node-v2";
const FIXED_EVALUATION_CEILING: u64 = 100_000_000;

pub(super) fn run(args: CohortHierarchicalMaxTProjectArgs) -> Result<(), CliError> {
    let CohortHierarchicalMaxTProjectArgs {
        project: project_path,
        input: input_path,
        group_a,
        group_b,
        permutations,
        seed,
        alpha,
        step_down,
        limits,
        out: output_path,
    } = args;
    let CohortHierarchicalMaxTLimits {
        maximum_patients,
        maximum_families,
        maximum_endpoints,
        maximum_cells,
        maximum_permutation_endpoint_evaluations,
        memory_budget_mib,
    } = limits;
    const LABEL: &str = "hierarchical Max-T source";
    let memory_bytes = memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CliError::Input("--memory-budget-mib is too large".into()))?;
    if maximum_patients < 4
        || maximum_families < 2
        || maximum_endpoints < 2
        || maximum_cells == 0
        || maximum_permutation_endpoint_evaluations == 0
        || maximum_permutation_endpoint_evaluations > FIXED_EVALUATION_CEILING
        || memory_bytes == 0
    {
        return Err(CliError::Input(
            "hierarchical Max-T resource limits must be positive and remain within fixed ceilings"
                .into(),
        ));
    }
    let input_path_identity = serde_json::to_vec(&input_path).map_err(|error| {
        CliError::Input(format!("input path is not JSON-serializable: {error}"))
    })?;
    let input_bytes = read_bounded(&input_path, memory_bytes)?;
    let source = Source::from_bytes(input_path.clone(), INPUT_KIND, &input_bytes)?;
    let input = read_input(input_bytes.as_slice())?;
    let requirements = preflight(&input, &group_a, &group_b, permutations, alpha, limits)?;
    let retained = retained_bytes(input_bytes.len(), &input, &requirements, permutations)?;
    if retained > memory_bytes {
        return Err(CliError::Input(format!(
            "hierarchical Max-T retained-memory estimate {retained} exceeds budget {memory_bytes}"
        )));
    }
    let output_estimate = output_bytes_upper_bound(&input)?;
    if output_estimate > MAXIMUM_RESULT_BYTES {
        return Err(CliError::Input(format!(
            "hierarchical Max-T output estimate {output_estimate} exceeds durable result limit {MAXIMUM_RESULT_BYTES}"
        )));
    }
    let peak = retained
        .checked_add(output_estimate)
        .ok_or_else(|| CliError::Input("hierarchical Max-T peak memory overflowed".into()))?;
    if peak > memory_bytes {
        return Err(CliError::Input(format!(
            "hierarchical Max-T peak-memory estimate {peak} exceeds budget {memory_bytes}"
        )));
    }
    source.ensure_unchanged(LABEL)?;

    let correction = if step_down {
        MaxTCorrection::StepDown
    } else {
        MaxTCorrection::SingleStep
    };
    let analysis = Analysis {
        input_path_identity,
        patients: input.patients,
        families: input.families,
        spec: MaxTPermutationSpec {
            group_a,
            group_b,
            permutations,
            seed,
            alpha,
        },
        correction,
        requirements,
    };
    let contract = analysis.contract(
        maximum_patients,
        maximum_families,
        maximum_endpoints,
        maximum_cells,
        maximum_permutation_endpoint_evaluations,
        memory_budget_mib,
    );
    let node = SourceNode::new(
        contract,
        LABEL,
        vec![source],
        analysis,
        Analysis::execute,
        |analysis, output| analysis.validate(output).map_err(NodeError::decode),
    )?;
    let run = durable::execute_node(
        &project_path,
        &node,
        ("marklab.cohort_hierarchical_max_t", 1),
        MAXIMUM_RESULT_BYTES,
    )?;
    durable::publish(
        "cohort-hierarchical-max-t",
        &output_path,
        &run.output,
        run.cache_status,
    )
}

fn read_bounded(path: &Path, memory_bytes: usize) -> Result<Vec<u8>, CliError> {
    let maximum = (MAXIMUM_INPUT_BYTES as usize).min(memory_bytes);
    crate::input_file::read_regular_file(
        path,
        maximum as u64,
        &format!(
            "hierarchical Max-T input must be a regular file within {maximum} bytes: {}",
            path.display()
        ),
    )
}

#[derive(Clone, Copy)]
struct Requirements {
    patient_count: usize,
    family_count: usize,
    endpoint_count: usize,
    group_a_count: usize,
    group_b_count: usize,
    cells: u64,
    evaluations: u64,
}

fn preflight(
    input: &Input,
    group_a: &str,
    group_b: &str,
    permutations: usize,
    alpha: f64,
    limits: CohortHierarchicalMaxTLimits,
) -> Result<Requirements, CliError> {
    let CohortHierarchicalMaxTLimits {
        maximum_patients,
        maximum_families,
        maximum_endpoints,
        maximum_cells,
        maximum_permutation_endpoint_evaluations: maximum_evaluations,
        ..
    } = limits;
    if group_a.is_empty()
        || group_b.is_empty()
        || group_a.trim() != group_a
        || group_b.trim() != group_b
        || group_a == group_b
    {
        return Err(CliError::Input(
            "hierarchical Max-T group labels must be distinct exact nonempty values".into(),
        ));
    }
    if permutations == 0 || permutations > 1_000_000 {
        return Err(CliError::Input(
            "hierarchical Max-T permutations must be between 1 and 1000000".into(),
        ));
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(CliError::Input(
            "hierarchical Max-T alpha must be finite and strictly between zero and one".into(),
        ));
    }
    if input.patients.len() < 4 || input.patients.len() > maximum_patients {
        return Err(CliError::Input(format!(
            "patient count is outside 4..=maximum_patients {maximum_patients}"
        )));
    }
    if input.families.len() < 2 || input.families.len() > maximum_families {
        return Err(CliError::Input(format!(
            "family count is outside 2..=maximum_families {maximum_families}"
        )));
    }
    let endpoints = &input.patients[0].endpoints;
    if endpoints.len() < 2
        || endpoints.len() > maximum_endpoints
        || endpoints.iter().any(|endpoint| endpoint.trim().is_empty())
        || endpoints.iter().collect::<HashSet<_>>().len() != endpoints.len()
    {
        return Err(CliError::Input(format!(
            "endpoint count or identities exceed maximum_endpoints {maximum_endpoints}"
        )));
    }
    let mut family_names = HashSet::new();
    if input.families.iter().any(|family| {
        family.family.is_empty()
            || family.family.trim() != family.family
            || !family_names.insert(family.family.as_str())
            || family.endpoints.is_empty()
    }) {
        return Err(CliError::Input(
            "ordered endpoint families require exact unique names and nonempty members".into(),
        ));
    }
    let flattened = input
        .families
        .iter()
        .flat_map(|family| family.endpoints.iter())
        .collect::<Vec<_>>();
    if flattened.iter().copied().ne(endpoints.iter()) {
        return Err(CliError::Input(
            "ordered endpoint families must exactly partition the complete endpoint vector".into(),
        ));
    }
    let mut patient_ids = HashSet::new();
    let mut group_a_count = 0;
    let mut group_b_count = 0;
    for patient in &input.patients {
        if patient.patient_id.is_empty()
            || patient.patient_id.trim() != patient.patient_id
            || !patient_ids.insert(patient.patient_id.as_str())
            || patient.endpoints != *endpoints
            || patient.values.len() != endpoints.len()
            || patient.values.iter().any(|value| !value.is_finite())
        {
            return Err(CliError::Input(
                "hierarchical Max-T requires unique exact patients and complete finite endpoint vectors"
                    .into(),
            ));
        }
        if patient.group == group_a {
            group_a_count += 1;
        } else if patient.group == group_b {
            group_b_count += 1;
        } else {
            return Err(CliError::Input(format!(
                "patient {} has undeclared group {:?}",
                patient.patient_id, patient.group
            )));
        }
    }
    if group_a_count < 2 || group_b_count < 2 {
        return Err(CliError::Input(
            "each hierarchical Max-T group requires at least two patients".into(),
        ));
    }
    let cells = (input.patients.len() as u64)
        .checked_mul(endpoints.len() as u64)
        .ok_or_else(|| CliError::Input("hierarchical Max-T cell count overflowed".into()))?;
    if cells > maximum_cells {
        return Err(CliError::Input(format!(
            "{cells} patient-endpoint cells exceed maximum_cells {maximum_cells}"
        )));
    }
    let evaluations = cells
        .checked_mul(permutations as u64 + 1)
        .ok_or_else(|| CliError::Input("hierarchical Max-T work overflowed".into()))?;
    if evaluations > maximum_evaluations {
        return Err(CliError::Input(format!(
            "{evaluations} patient-endpoint-permutation evaluations exceed maximum_permutation_endpoint_evaluations {maximum_evaluations}"
        )));
    }
    Ok(Requirements {
        patient_count: input.patients.len(),
        family_count: input.families.len(),
        endpoint_count: endpoints.len(),
        group_a_count,
        group_b_count,
        cells,
        evaluations,
    })
}

fn retained_bytes(
    source_bytes: usize,
    input: &Input,
    requirements: &Requirements,
    permutations: usize,
) -> Result<usize, CliError> {
    let cell_bytes = usize::try_from(requirements.cells)
        .ok()
        .and_then(|cells| cells.checked_mul(3 * std::mem::size_of::<f64>()))
        .ok_or_else(|| CliError::Input("hierarchical Max-T cell memory overflowed".into()))?;
    source_bytes
        .checked_add(cell_bytes)
        .and_then(|value| value.checked_add(requirements.patient_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(requirements.endpoint_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(requirements.family_count.saturating_mul(1024)))
        .and_then(|value| value.checked_add(permutations.saturating_mul(16)))
        .and_then(|value| {
            value.checked_add(
                input
                    .families
                    .iter()
                    .map(|family| family.endpoints.len())
                    .max()
                    .unwrap_or(0)
                    .saturating_mul(requirements.patient_count)
                    .saturating_mul(32),
            )
        })
        .ok_or_else(|| CliError::Input("hierarchical Max-T memory estimate overflowed".into()))
}

fn output_bytes_upper_bound(input: &Input) -> Result<usize, CliError> {
    let family_name_bytes = input
        .families
        .iter()
        .try_fold(0_usize, |total, family| {
            total.checked_add(family.family.len())
        })
        .ok_or_else(|| CliError::Input("family identity size overflowed".into()))?;
    let endpoint_name_bytes = input
        .families
        .iter()
        .try_fold(0_usize, |total, family| {
            family
                .endpoints
                .iter()
                .try_fold(total, |total, endpoint| total.checked_add(endpoint.len()))
        })
        .ok_or_else(|| CliError::Input("endpoint identity size overflowed".into()))?;
    (32 * 1024_usize)
        .checked_add(input.families.len().saturating_mul(512))
        .and_then(|value| value.checked_add(family_name_bytes))
        .and_then(|value| {
            value.checked_add(
                input
                    .families
                    .iter()
                    .map(|family| family.endpoints.len())
                    .sum::<usize>()
                    .saturating_mul(384),
            )
        })
        .and_then(|value| value.checked_add(endpoint_name_bytes))
        .ok_or_else(|| CliError::Input("hierarchical Max-T output estimate overflowed".into()))
}

struct Analysis {
    input_path_identity: Vec<u8>,
    patients: Vec<PatientEndpointVector>,
    families: Vec<OrderedEndpointFamily>,
    spec: MaxTPermutationSpec,
    correction: MaxTCorrection,
    requirements: Requirements,
}

impl Analysis {
    fn contract(
        &self,
        maximum_patients: usize,
        maximum_families: usize,
        maximum_endpoints: usize,
        maximum_cells: u64,
        maximum_evaluations: u64,
        memory_budget_mib: usize,
    ) -> NodeContract {
        let configuration_digest = ContentDigest::from_framed([
            b"marklab-cohort-hierarchical-max-t-configuration-v1".as_slice(),
            self.input_path_identity.as_slice(),
            self.spec.group_a.as_bytes(),
            self.spec.group_b.as_bytes(),
            self.spec.permutations.to_string().as_bytes(),
            self.spec.seed.to_string().as_bytes(),
            self.spec.alpha.to_bits().to_string().as_bytes(),
            correction_name(self.correction).as_bytes(),
            maximum_patients.to_string().as_bytes(),
            maximum_families.to_string().as_bytes(),
            maximum_endpoints.to_string().as_bytes(),
            maximum_cells.to_string().as_bytes(),
            maximum_evaluations.to_string().as_bytes(),
            memory_budget_mib.to_string().as_bytes(),
        ]);
        let execution_policy = format!(
            "serial;patient-label-permutation;ordered-family-gatekeeping-max-t;correction={};maximum_patients={maximum_patients};maximum_families={maximum_families};maximum_endpoints={maximum_endpoints};maximum_cells={maximum_cells};maximum_permutation_endpoint_evaluations={maximum_evaluations};memory_budget_mib={memory_budget_mib}",
            correction_name(self.correction)
        )
        .into_bytes();
        NodeContract {
            id: "cohort-hierarchical-max-t",
            kind: "cohort_hierarchical_max_t",
            version: 1,
            configuration_digest,
            execution_policy: Cow::Owned(execution_policy),
            implementation_identity: IMPLEMENTATION_IDENTITY,
            output_kind: OUTPUT_KIND,
            codec: Codec::ExactFloatJson,
        }
    }

    fn execute(&self) -> Result<Output, NodeError> {
        let result = hierarchical_gatekeeping_max_t(
            &self.patients,
            &self.families,
            &self.spec,
            self.correction,
        )
        .map_err(NodeError::execution)?;
        Ok(Output::from_result(result))
    }

    fn validate(&self, output: &Output) -> io::Result<()> {
        let requirements = &self.requirements;
        if output.format != "marklab.cohort_hierarchical_max_t"
            || output.version != 1
            || output.design.analysis_level != "patient"
            || output.design.null_family != "population_independence"
            || output.design.permutation_unit != "patient_label"
            || output.design.multiplicity != "ordered_family_gatekeeping_max_t"
            || output.design.correction != correction_name(self.correction)
            || output.design.opening_rule
                != "open the next family only when every endpoint in the current opened family is rejected"
            || output.patients.total != requirements.patient_count
            || output.patients.group_a != requirements.group_a_count
            || output.patients.group_b != requirements.group_b_count
            || output.families.len() != requirements.family_count
            || output.opened_family_count
                != output.families.iter().filter(|family| family.opened).count()
            || output.alpha.to_bits() != self.spec.alpha.to_bits()
            || output.permutations.requested != self.spec.permutations
            || output.permutations.seed != self.spec.seed
            || output.claim_status != "strong_fwer_prespecified_serial_gatekeeping"
            || requirements.evaluations > FIXED_EVALUATION_CEILING
        {
            return Err(invalid("decoded hierarchical Max-T identity differs"));
        }
        let minimum_p = 1.0 / (self.spec.permutations + 1) as f64;
        let mut should_open = true;
        for (order, (family, expected)) in output.families.iter().zip(&self.families).enumerate() {
            if family.order != order
                || family.family != expected.family
                || family.opened != should_open
                || !family.critical_value.is_finite()
                || family.critical_value < 0.0
                || family.endpoints.len() != expected.endpoints.len()
            {
                return Err(invalid("decoded hierarchical Max-T family differs"));
            }
            for (endpoint, expected_name) in family.endpoints.iter().zip(&expected.endpoints) {
                let lattice = endpoint.local_adjusted_p_value * (self.spec.permutations + 1) as f64;
                if &endpoint.endpoint != expected_name
                    || !endpoint.effect_group_a_minus_group_b.is_finite()
                    || !endpoint.studentized_statistic.is_finite()
                    || !(minimum_p..=1.0).contains(&endpoint.local_adjusted_p_value)
                    || (lattice - lattice.round()).abs() > 1e-9
                    || endpoint.rejected
                        != (family.opened && endpoint.local_adjusted_p_value <= self.spec.alpha)
                {
                    return Err(invalid("decoded hierarchical Max-T endpoint differs"));
                }
            }
            let all_rejected =
                family.opened && family.endpoints.iter().all(|endpoint| endpoint.rejected);
            if family.all_endpoints_rejected != all_rejected {
                return Err(invalid("decoded hierarchical Max-T gate state differs"));
            }
            should_open = all_rejected;
        }
        Ok(())
    }
}

fn correction_name(value: MaxTCorrection) -> &'static str {
    value.as_str()
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
