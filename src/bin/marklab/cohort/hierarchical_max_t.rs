use std::{collections::BTreeMap, path::PathBuf};

use marklab_cohort::{
    hierarchical_gatekeeping_max_t, HierarchicalMaxTResult, InferenceAnalysisLevel,
    InferenceMultiplicity, InferenceNullFamily, InferencePermutationUnit, MaxTCorrection,
    MaxTPermutationSpec, OrderedEndpointFamily, PatientEndpointVector,
};
use serde::{Deserialize, Serialize};

use super::{input::validate_input_file_with_message, publication::publish_json, CohortError};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvRow {
    patient_id: String,
    group: String,
    family_order: usize,
    family: String,
    endpoint: String,
    value: f64,
}

struct Input {
    patients: Vec<PatientEndpointVector>,
    families: Vec<OrderedEndpointFamily>,
}

#[derive(Debug, Serialize)]
struct Output {
    format: &'static str,
    version: u32,
    design: Design,
    patients: PatientCounts,
    families: Vec<FamilyOutput>,
    opened_family_count: usize,
    alpha: f64,
    permutations: Permutations,
    claim_status: &'static str,
}

#[derive(Debug, Serialize)]
struct Design {
    analysis_level: &'static str,
    null_family: &'static str,
    permutation_unit: &'static str,
    multiplicity: &'static str,
    correction: &'static str,
    opening_rule: &'static str,
}

#[derive(Debug, Serialize)]
struct PatientCounts {
    total: usize,
    group_a: usize,
    group_b: usize,
}

#[derive(Debug, Serialize)]
struct FamilyOutput {
    order: usize,
    family: String,
    opened: bool,
    all_endpoints_rejected: bool,
    critical_value: f64,
    endpoints: Vec<EndpointOutput>,
}

#[derive(Debug, Serialize)]
struct EndpointOutput {
    endpoint: String,
    effect_group_a_minus_group_b: f64,
    studentized_statistic: f64,
    local_adjusted_p_value: f64,
    rejected: bool,
}

#[derive(Debug, Serialize)]
struct Permutations {
    requested: usize,
    attempted: usize,
    completed: usize,
    seed: u64,
}

pub(super) struct RunArgs {
    pub input: PathBuf,
    pub group_a: String,
    pub group_b: String,
    pub permutations: usize,
    pub seed: u64,
    pub alpha: f64,
    pub step_down: bool,
    pub out: PathBuf,
}

pub(super) fn run(args: RunArgs) -> Result<(), CohortError> {
    let input = read_input(&args.input)?;
    let correction = if args.step_down {
        MaxTCorrection::StepDown
    } else {
        MaxTCorrection::SingleStep
    };
    let result = hierarchical_gatekeeping_max_t(
        &input.patients,
        &input.families,
        &MaxTPermutationSpec {
            group_a: args.group_a,
            group_b: args.group_b,
            permutations: args.permutations,
            seed: args.seed,
            alpha: args.alpha,
        },
        correction,
    )?;
    publish_json(&args.out, &Output::from_result(result))
}

impl Output {
    fn from_result(result: HierarchicalMaxTResult) -> Self {
        match result.inference_design.analysis_level() {
            InferenceAnalysisLevel::Patient => {}
            _ => unreachable!("hierarchical Max-T returned another analysis level"),
        }
        match result.inference_design.null_family() {
            InferenceNullFamily::PopulationIndependence => {}
            _ => unreachable!("hierarchical Max-T returned another null family"),
        }
        match result.inference_design.permutation_unit() {
            InferencePermutationUnit::PatientLabel => {}
            _ => unreachable!("hierarchical Max-T returned another permutation unit"),
        }
        match result.inference_design.multiplicity() {
            InferenceMultiplicity::OrderedFamilyGatekeepingMaxT => {}
            _ => unreachable!("hierarchical Max-T returned another multiplicity policy"),
        }
        let total = result.group_a_count + result.group_b_count;
        Self {
            format: "marklab.cohort_hierarchical_max_t",
            version: 1,
            design: Design {
                analysis_level: "patient",
                null_family: "population_independence",
                permutation_unit: "patient_label",
                multiplicity: "ordered_family_gatekeeping_max_t",
                correction: result.correction.as_str(),
                opening_rule: "open the next family only when every endpoint in the current opened family is rejected",
            },
            patients: PatientCounts {
                total,
                group_a: result.group_a_count,
                group_b: result.group_b_count,
            },
            families: result
                .families
                .into_iter()
                .map(|family| FamilyOutput {
                    order: family.order,
                    family: family.family,
                    opened: family.opened,
                    all_endpoints_rejected: family.all_endpoints_rejected,
                    critical_value: family.critical_value,
                    endpoints: family
                        .endpoints
                        .into_iter()
                        .map(|endpoint| EndpointOutput {
                            endpoint: endpoint.endpoint,
                            effect_group_a_minus_group_b: endpoint
                                .effect_group_a_minus_group_b,
                            studentized_statistic: endpoint.studentized_statistic,
                            local_adjusted_p_value: endpoint.local_adjusted_p_value,
                            rejected: endpoint.rejected,
                        })
                        .collect(),
                })
                .collect(),
            opened_family_count: result.opened_family_count,
            alpha: result.alpha,
            permutations: Permutations {
                requested: result.permutations_requested,
                attempted: result.permutations_attempted,
                completed: result.permutations_completed,
                seed: result.seed,
            },
            claim_status: "strong_fwer_prespecified_serial_gatekeeping",
        }
    }
}

type FamilyKey = (usize, String, String);
type PatientRows = (String, BTreeMap<FamilyKey, f64>);

fn read_input(path: &std::path::Path) -> Result<Input, CohortError> {
    validate_input_file_with_message(
        path,
        "hierarchical Max-T input must be a regular file within 16 MiB",
    )?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    if !reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .iter()
        .eq([
            "patient_id",
            "group",
            "family_order",
            "family",
            "endpoint",
            "value",
        ])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,family_order,family,endpoint,value".into(),
        ));
    }
    let mut family_names = BTreeMap::<usize, String>::new();
    let mut endpoint_keys = BTreeMap::<String, FamilyKey>::new();
    let mut patients = BTreeMap::<String, PatientRows>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row.map_err(|error| CohortError::Input(error.to_string()))?;
        if row.family_order == 0 {
            return Err(CohortError::Input(
                "family_order must be a contiguous one-based integer".into(),
            ));
        }
        if family_names
            .insert(row.family_order, row.family.clone())
            .is_some_and(|existing| existing != row.family)
        {
            return Err(CohortError::Input(format!(
                "family order {} has conflicting names",
                row.family_order
            )));
        }
        let key = (row.family_order, row.family.clone(), row.endpoint.clone());
        if endpoint_keys
            .insert(row.endpoint.clone(), key.clone())
            .is_some_and(|existing| existing != key)
        {
            return Err(CohortError::Input(format!(
                "endpoint {:?} belongs to conflicting families",
                row.endpoint
            )));
        }
        let patient = patients
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), BTreeMap::new()));
        if patient.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group values",
                row.patient_id
            )));
        }
        if patient.1.insert(key, row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has a duplicate hierarchical endpoint",
                row.patient_id
            )));
        }
    }
    if family_names.keys().copied().ne(1..=family_names.len()) {
        return Err(CohortError::Input(
            "family_order must be contiguous from one".into(),
        ));
    }
    let mut families = Vec::with_capacity(family_names.len());
    for (order, family) in family_names {
        let endpoints = endpoint_keys
            .values()
            .filter(|(endpoint_order, endpoint_family, _)| {
                *endpoint_order == order && endpoint_family == &family
            })
            .map(|(_, _, endpoint)| endpoint.clone())
            .collect::<Vec<_>>();
        families.push(OrderedEndpointFamily { family, endpoints });
    }
    let expected_keys = families
        .iter()
        .enumerate()
        .flat_map(|(order, family)| {
            family
                .endpoints
                .iter()
                .map(move |endpoint| (order + 1, family.family.clone(), endpoint.clone()))
        })
        .collect::<Vec<_>>();
    let patients = patients
        .into_iter()
        .map(|(patient_id, (group, values))| {
            if values.keys().ne(expected_keys.iter()) {
                return Err(CohortError::Input(format!(
                    "patient {patient_id} does not have the exact complete ordered endpoint families"
                )));
            }
            Ok(PatientEndpointVector {
                patient_id,
                group,
                endpoints: expected_keys
                    .iter()
                    .map(|(_, _, endpoint)| endpoint.clone())
                    .collect(),
                values: values.into_values().collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Input { patients, families })
}
