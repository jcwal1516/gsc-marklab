use std::{collections::BTreeMap, fs, path::Path};

use marklab_cohort::{
    Fingerprint, FunctionalCurve, PairedPatientEndpoint, PairedPatientEndpointVector,
    PatientEndpoint, PatientEndpointVector, PatientExchangeabilityBlock,
};
use serde::Deserialize;

use super::CohortError;

const MAXIMUM_INPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct CsvRecord {
    patient_id: String,
    group: String,
    endpoint: f64,
    #[serde(default)]
    block: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairedCsvRecord {
    patient_id: String,
    condition: String,
    endpoint: f64,
}

#[derive(Debug, Deserialize)]
struct PairedMaxTCsvRecord {
    patient_id: String,
    condition: String,
    endpoint: String,
    value: f64,
}

#[derive(Debug, Deserialize)]
struct FunctionalCsvRecord {
    patient_id: String,
    group: String,
    axis: f64,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

pub(super) struct FunctionalInput {
    pub(super) curves: Vec<FunctionalCurve>,
    pub(super) blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

#[derive(Debug, Deserialize)]
struct MaxTCsvRecord {
    patient_id: String,
    group: String,
    endpoint: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

pub(super) struct MaxTInput {
    pub(super) patients: Vec<PatientEndpointVector>,
    pub(super) blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

#[derive(Debug, Deserialize)]
struct FingerprintCsvRecord {
    patient_id: String,
    group: String,
    feature: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

pub(super) struct FingerprintInput {
    pub(super) fingerprints: Vec<Fingerprint>,
    pub(super) blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

pub(super) fn read_records(path: &Path) -> Result<Vec<PatientEndpoint>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let valid_headers = headers.iter().eq(["patient_id", "group", "endpoint"])
        || headers
            .iter()
            .eq(["patient_id", "group", "endpoint", "block"]);
    if !valid_headers {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,endpoint with optional trailing block"
                .into(),
        ));
    }
    reader
        .deserialize::<CsvRecord>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(PatientEndpoint {
                patient_id: row.patient_id,
                group: row.group,
                endpoint: row.endpoint,
                block: row.block.filter(|block| !block.is_empty()),
            })
        })
        .collect()
}

pub(super) fn read_paired_records(path: &Path) -> Result<Vec<PairedPatientEndpoint>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers.iter().eq(["patient_id", "condition", "endpoint"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,condition,endpoint".into(),
        ));
    }
    reader
        .deserialize::<PairedCsvRecord>()
        .map(|decoded| {
            let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
            Ok(PairedPatientEndpoint {
                patient_id: row.patient_id,
                condition: row.condition,
                endpoint: row.endpoint,
            })
        })
        .collect()
}

pub(super) fn read_paired_max_t_records(
    path: &Path,
) -> Result<Vec<PairedPatientEndpointVector>, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    if !headers
        .iter()
        .eq(["patient_id", "condition", "endpoint", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,condition,endpoint,value".into(),
        ));
    }
    let mut grouped = BTreeMap::<(String, String), BTreeMap<String, f64>>::new();
    for decoded in reader.deserialize::<PairedMaxTCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        let key = (row.patient_id.clone(), row.condition.clone());
        if grouped
            .entry(key)
            .or_default()
            .insert(row.endpoint.clone(), row.value)
            .is_some()
        {
            return Err(CohortError::Input(format!(
                "patient {} condition {} has duplicate endpoint {:?}",
                row.patient_id, row.condition, row.endpoint
            )));
        }
    }
    Ok(grouped
        .into_iter()
        .map(
            |((patient_id, condition), endpoints)| PairedPatientEndpointVector {
                patient_id,
                condition,
                values: endpoints.values().copied().collect(),
                endpoints: endpoints.into_keys().collect(),
            },
        )
        .collect())
}

pub(super) fn read_functional_curves(path: &Path) -> Result<FunctionalInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "axis", "value", "block"]);
    if !blocked && !headers.iter().eq(["patient_id", "group", "axis", "value"]) {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,axis,value or patient_id,group,axis,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, Vec<(f64, f64)>)>::new();
    for decoded in reader.deserialize::<FunctionalCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if !row.axis.is_finite() || !row.value.is_finite() {
            return Err(CohortError::Input(
                "functional CSV axis and value fields must be finite".into(),
            ));
        }
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its functional block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), Vec::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting functional blocks",
                row.patient_id
            )));
        }
        entry.2.push((row.axis, row.value));
    }
    let mut curves = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, mut points)) in grouped {
        points.sort_by(|left, right| left.0.total_cmp(&right.0));
        if points.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err(CohortError::Input(format!(
                "patient {patient_id} has duplicate or non-increasing axis rows"
            )));
        }
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        let (axis, values): (Vec<_>, Vec<_>) = points.into_iter().unzip();
        curves.push(FunctionalCurve {
            patient_id,
            group,
            axis,
            values,
        });
    }
    Ok(FunctionalInput { curves, blocks })
}

pub(super) fn read_max_t_patients(path: &Path) -> Result<MaxTInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "endpoint", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "endpoint", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,endpoint,value or patient_id,group,endpoint,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<MaxTCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its Max-T block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting Max-T blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.endpoint.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate endpoint {:?}",
                row.patient_id, row.endpoint
            )));
        }
    }
    let mut patients = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, endpoints)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        patients.push(PatientEndpointVector {
            patient_id,
            group,
            values: endpoints.values().copied().collect(),
            endpoints: endpoints.into_keys().collect(),
        });
    }
    Ok(MaxTInput { patients, blocks })
}

pub(super) fn read_fingerprints(path: &Path) -> Result<FingerprintInput, CohortError> {
    validate_input_file(path)?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_path(path)
        .map_err(|error| CohortError::Input(error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| CohortError::Input(error.to_string()))?
        .clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "feature", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "feature", "value"])
    {
        return Err(CohortError::Input(
            "CSV header must be exactly patient_id,group,feature,value or patient_id,group,feature,value,block".into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<FingerprintCsvRecord>() {
        let row = decoded.map_err(|error| CohortError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(CohortError::Input(format!(
                "patient {} is missing its fingerprint block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CohortError::Input(format!(
                "patient {} has conflicting fingerprint blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.feature.clone(), row.value).is_some() {
            return Err(CohortError::Input(format!(
                "patient {} has duplicate feature {:?}",
                row.patient_id, row.feature
            )));
        }
    }
    let mut fingerprints = Vec::with_capacity(grouped.len());
    let mut blocks = blocked.then(|| Vec::with_capacity(grouped.len()));
    for (patient_id, (group, block, features)) in grouped {
        if let Some(assignments) = &mut blocks {
            assignments.push(
                PatientExchangeabilityBlock::new(patient_id.clone(), block.unwrap_or_default())
                    .map_err(|error| CohortError::Input(error.to_string()))?,
            );
        }
        fingerprints.push(Fingerprint {
            patient_id,
            group,
            values: features.values().copied().collect(),
            features: features.into_keys().collect(),
        });
    }
    Ok(FingerprintInput {
        fingerprints,
        blocks,
    })
}

pub(super) fn validate_input_file(path: &Path) -> Result<(), CohortError> {
    validate_input_metadata(path, None)
}

pub(super) fn validate_input_file_with_message(
    path: &Path,
    invalid_message: &'static str,
) -> Result<(), CohortError> {
    validate_input_metadata(path, Some(invalid_message))
}

fn validate_input_metadata(
    path: &Path,
    combined_invalid_message: Option<&'static str>,
) -> Result<(), CohortError> {
    let metadata = fs::metadata(path).map_err(|source| CohortError::Output {
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(CohortError::Input(match combined_invalid_message {
            Some(message) => message.into(),
            None => format!("input must be a regular file: {}", path.display()),
        }));
    }
    if metadata.len() > MAXIMUM_INPUT_BYTES {
        return Err(CohortError::Input(match combined_invalid_message {
            Some(message) => message.into(),
            None => format!("input exceeds the {MAXIMUM_INPUT_BYTES}-byte limit"),
        }));
    }
    Ok(())
}
