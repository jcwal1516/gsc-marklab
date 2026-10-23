use std::collections::BTreeMap;

use marklab_cohort::{
    HierarchicalMaxTResult, InferenceAnalysisLevel, InferenceMultiplicity, InferenceNullFamily,
    InferencePermutationUnit, OrderedEndpointFamily, PatientEndpointVector,
};
use serde::{Deserialize, Serialize};

use crate::cli_error::CliError;

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

pub(crate) struct Input {
    pub(crate) patients: Vec<PatientEndpointVector>,
    pub(crate) families: Vec<OrderedEndpointFamily>,
}

type FamilyKey = (usize, String, String);
type PatientRows = (String, BTreeMap<FamilyKey, f64>);

pub(crate) fn read_input(reader: impl std::io::Read) -> Result<Input, CliError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(reader);
    if !reader.headers()?.iter().eq([
        "patient_id",
        "group",
        "family_order",
        "family",
        "endpoint",
        "value",
    ]) {
        return Err(CliError::Input(
            "CSV header must be exactly patient_id,group,family_order,family,endpoint,value".into(),
        ));
    }
    let mut family_names = BTreeMap::<usize, String>::new();
    let mut endpoint_keys = BTreeMap::<String, FamilyKey>::new();
    let mut patients = BTreeMap::<String, PatientRows>::new();
    for row in reader.deserialize::<CsvRow>() {
        let row = row?;
        if row.family_order == 0 {
            return Err(CliError::Input(
                "family_order must be a contiguous one-based integer".into(),
            ));
        }
        if family_names
            .insert(row.family_order, row.family.clone())
            .is_some_and(|existing| existing != row.family)
        {
            return Err(CliError::Input(format!(
                "family order {} has conflicting names",
                row.family_order
            )));
        }
        let key = (row.family_order, row.family.clone(), row.endpoint.clone());
        if endpoint_keys
            .insert(row.endpoint.clone(), key.clone())
            .is_some_and(|existing| existing != key)
        {
            return Err(CliError::Input(format!(
                "endpoint {:?} belongs to conflicting families",
                row.endpoint
            )));
        }
        let patient = patients
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), BTreeMap::new()));
        if patient.0 != row.group {
            return Err(CliError::Input(format!(
                "patient {} has conflicting group values",
                row.patient_id
            )));
        }
        if patient.1.insert(key, row.value).is_some() {
            return Err(CliError::Input(format!(
                "patient {} has a duplicate hierarchical endpoint",
                row.patient_id
            )));
        }
    }
    if family_names.keys().copied().ne(1..=family_names.len()) {
        return Err(CliError::Input(
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
                return Err(CliError::Input(format!(
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Output {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub(crate) design: Design,
    pub(crate) patients: PatientCounts,
    pub(crate) families: Vec<FamilyOutput>,
    pub(crate) opened_family_count: usize,
    pub(crate) alpha: f64,
    pub(crate) permutations: Permutations,
    pub(crate) claim_status: String,
}

impl Output {
    pub(crate) fn from_result(result: HierarchicalMaxTResult) -> Self {
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
        Self {
            format: "marklab.cohort_hierarchical_max_t".into(),
            version: 1,
            design: Design {
                analysis_level: "patient".into(),
                null_family: "population_independence".into(),
                permutation_unit: "patient_label".into(),
                multiplicity: "ordered_family_gatekeeping_max_t".into(),
                correction: result.correction.as_str().into(),
                opening_rule: "open the next family only when every endpoint in the current opened family is rejected".into(),
            },
            patients: PatientCounts {
                total: result.group_a_count + result.group_b_count,
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
                            effect_group_a_minus_group_b: endpoint.effect_group_a_minus_group_b,
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
                seed: result.seed,
            },
            claim_status: "strong_fwer_prespecified_serial_gatekeeping".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Design {
    pub(crate) analysis_level: String,
    pub(crate) null_family: String,
    pub(crate) permutation_unit: String,
    pub(crate) multiplicity: String,
    pub(crate) correction: String,
    pub(crate) opening_rule: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatientCounts {
    pub(crate) total: usize,
    pub(crate) group_a: usize,
    pub(crate) group_b: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FamilyOutput {
    pub(crate) order: usize,
    pub(crate) family: String,
    pub(crate) opened: bool,
    pub(crate) all_endpoints_rejected: bool,
    pub(crate) critical_value: f64,
    pub(crate) endpoints: Vec<EndpointOutput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EndpointOutput {
    pub(crate) endpoint: String,
    pub(crate) effect_group_a_minus_group_b: f64,
    pub(crate) studentized_statistic: f64,
    pub(crate) local_adjusted_p_value: f64,
    pub(crate) rejected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Permutations {
    pub(crate) requested: usize,
    pub(crate) seed: u64,
}
