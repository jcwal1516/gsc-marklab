use std::path::PathBuf;

use marklab_cohort::{
    patient_nested_field_inference, NestedFieldRecord, NestedFieldResult, NestedFieldSpec,
    PreparedNestedFields,
};
use serde::{Deserialize, Serialize};

use super::{publication::publish_json, validate_input_file, CohortError};

#[derive(Debug, Deserialize)]
struct InputRow {
    patient_id: String,
    specimen_id: String,
    group: String,
    endpoint: String,
    value: f64,
}

pub(crate) struct RunArgs {
    pub input: PathBuf,
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub maximum_patients: usize,
    pub maximum_specimens: usize,
    pub maximum_endpoints: usize,
    pub maximum_permutation_endpoint_evaluations: u64,
    pub memory_budget_mib: usize,
    pub out: PathBuf,
}

pub(crate) fn run(args: RunArgs) -> Result<(), CohortError> {
    let records = read_records(&args.input)?;
    let memory_budget_bytes = args
        .memory_budget_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| CohortError::Input("memory budget overflowed".into()))?;
    let result = patient_nested_field_inference(
        &records,
        &NestedFieldSpec {
            group_a: args.group_a.clone(),
            group_b: args.group_b.clone(),
            permutations: args.permutations,
            seed: args.seed,
            alpha: args.alpha,
            maximum_patients: args.maximum_patients,
            maximum_specimens: args.maximum_specimens,
            maximum_endpoints: args.maximum_endpoints,
            maximum_permutation_endpoint_evaluations: args.maximum_permutation_endpoint_evaluations,
            memory_budget_bytes,
        },
    )?;
    let output =
        canonical_result_codec(PatientNestedFieldsOutput::from_result(args.input, result))?;
    publish_json(&args.out, &output)
}

fn canonical_result_codec(
    result: PatientNestedFieldsOutput,
) -> Result<PatientNestedFieldsOutput, CohortError> {
    let bytes = serde_json::to_vec(&result)?;
    serde_json::from_slice(&bytes).map_err(CohortError::Json)
}

pub(crate) fn read_records(path: &PathBuf) -> Result<Vec<NestedFieldRecord>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !headers
        .iter()
        .eq(["patient_id", "specimen_id", "group", "endpoint", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,specimen_id,group,endpoint,value".into(),
        ));
    }
    reader
        .deserialize::<InputRow>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(NestedFieldRecord {
                patient_id: row.patient_id,
                specimen_id: row.specimen_id,
                group: row.group,
                endpoint: row.endpoint,
                value: row.value,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatientNestedFieldsOutput {
    format: String,
    version: u32,
    input: PathBuf,
    population_unit: String,
    specimen_unit: String,
    null: String,
    assumptions: [String; 3],
    failure_policy: String,
    patient_count: usize,
    specimen_count: usize,
    endpoint_count: usize,
    endpoints: Vec<String>,
    patient_endpoints: Vec<PatientOutput>,
    nested_specimen_stability: StabilityOutput,
    heldout: HeldoutOutput,
    population_inference: PopulationInferenceOutput,
    work: WorkOutput,
}

impl PatientNestedFieldsOutput {
    pub(crate) fn from_result(input: PathBuf, result: NestedFieldResult) -> Self {
        let population = result.population_inference;
        Self {
            format: "marklab.cohort_patient_nested_fields".into(),
            version: 1,
            input,
            population_unit: "patient".into(),
            specimen_unit: "specimen_nested_within_patient".into(),
            null: "whole_patient_group_labels_exchangeable_under_the_declared_design".into(),
            assumptions: [
                "patients_are_independent_population_units".into(),
                "specimens_are_equal_weight_nested_measurements".into(),
                "the_complete_endpoint_family_is_prespecified".into(),
            ],
            failure_policy: "reject_incomplete_nonfinite_degenerate_or_over_budget_inputs".into(),
            patient_count: result.patients.len(),
            specimen_count: result.specimen_count,
            endpoint_count: result.endpoints.len(),
            endpoints: result.endpoints,
            patient_endpoints: result
                .patients
                .into_iter()
                .map(|patient| PatientOutput {
                    patient_id: patient.patient_id,
                    group: patient.group,
                    specimen_count: patient.specimen_count,
                    values: patient.values,
                })
                .collect(),
            nested_specimen_stability: StabilityOutput {
                method: result.stability.method.into(),
                comparison_count: result.stability.comparison_count,
                median_rank_correlation: result.stability.median_rank_correlation,
                minimum_rank_correlation: result.stability.minimum_rank_correlation,
            },
            heldout: HeldoutOutput {
                split_unit: result.heldout.split_unit.into(),
                training_transform: result.heldout.training_transform.into(),
                classifier: result.heldout.classifier.into(),
                balanced_accuracy: result.heldout.balanced_accuracy,
                predictions: result
                    .heldout
                    .predictions
                    .into_iter()
                    .map(|prediction| HeldoutPredictionOutput {
                        patient_id: prediction.patient_id,
                        observed_group: prediction.observed_group,
                        predicted_group: prediction.predicted_group,
                        distance_to_group_a: prediction.distance_to_group_a,
                        distance_to_group_b: prediction.distance_to_group_b,
                    })
                    .collect(),
            },
            population_inference: PopulationInferenceOutput {
                permutation_unit: "whole_patient_label".into(),
                correction: population.correction.as_str().into(),
                group_a_count: population.group_a_count,
                group_b_count: population.group_b_count,
                critical_value: population.critical_value,
                alpha: population.alpha,
                permutations_requested: population.permutations_requested,
                permutations_attempted: population.permutations_attempted,
                permutations_completed: population.permutations_completed,
                seed: population.seed,
                endpoints: population
                    .endpoints
                    .into_iter()
                    .map(|endpoint| PopulationEndpointOutput {
                        endpoint: endpoint.endpoint,
                        effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
                        studentized_statistic: endpoint.studentized_statistic,
                        adjusted_p_value: endpoint.adjusted_p_value,
                    })
                    .collect(),
            },
            work: WorkOutput {
                retained_memory_bytes: result.retained_memory_bytes,
                permutation_endpoint_evaluations: result.permutation_endpoint_evaluations,
            },
        }
    }

    pub(crate) fn matches_request(
        &self,
        input: &PathBuf,
        prepared: &PreparedNestedFields,
        spec: &NestedFieldSpec,
    ) -> bool {
        self.format == "marklab.cohort_patient_nested_fields"
            && self.version == 1
            && &self.input == input
            && self.population_unit == "patient"
            && self.specimen_unit == "specimen_nested_within_patient"
            && self.patient_count == prepared.patient_count()
            && self.specimen_count == prepared.specimen_count()
            && self.endpoint_count == prepared.endpoint_count()
            && self.patient_endpoints.len() == prepared.patient_count()
            && self.nested_specimen_stability.comparison_count == prepared.specimen_count()
            && self.heldout.split_unit == "whole_patient"
            && self.heldout.training_transform == "fold_internal_z_score"
            && self.heldout.predictions.len() == prepared.patient_count()
            && self.population_inference.permutation_unit == "whole_patient_label"
            && self.population_inference.correction == "step_down_max_t"
            && self.population_inference.permutations_requested == spec.permutations
            && self.population_inference.permutations_attempted == spec.permutations
            && self.population_inference.permutations_completed == spec.permutations
            && self.population_inference.seed == spec.seed
            && self.population_inference.alpha.to_bits() == spec.alpha.to_bits()
            && self.population_inference.endpoints.len() == prepared.endpoint_count()
            && self.work.permutation_endpoint_evaluations
                == prepared.permutation_endpoint_evaluations()
            && self.work.retained_memory_bytes <= spec.memory_budget_bytes
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PatientOutput {
    patient_id: String,
    group: String,
    specimen_count: usize,
    values: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StabilityOutput {
    method: String,
    comparison_count: usize,
    median_rank_correlation: f64,
    minimum_rank_correlation: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HeldoutOutput {
    split_unit: String,
    training_transform: String,
    classifier: String,
    balanced_accuracy: f64,
    predictions: Vec<HeldoutPredictionOutput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HeldoutPredictionOutput {
    patient_id: String,
    observed_group: String,
    predicted_group: String,
    distance_to_group_a: f64,
    distance_to_group_b: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PopulationInferenceOutput {
    permutation_unit: String,
    correction: String,
    group_a_count: usize,
    group_b_count: usize,
    critical_value: f64,
    alpha: f64,
    permutations_requested: usize,
    permutations_attempted: usize,
    permutations_completed: usize,
    seed: u64,
    endpoints: Vec<PopulationEndpointOutput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PopulationEndpointOutput {
    endpoint: String,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    adjusted_p_value: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct WorkOutput {
    retained_memory_bytes: usize,
    permutation_endpoint_evaluations: u64,
}
