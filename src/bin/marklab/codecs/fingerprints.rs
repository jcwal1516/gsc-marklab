use std::collections::BTreeMap;

use marklab_cohort::{Fingerprint, PatientExchangeabilityBlock};
use serde::Deserialize;

use crate::cli_error::CliError;

#[derive(Debug, Deserialize)]
struct FingerprintCsvRecord {
    patient_id: String,
    group: String,
    feature: String,
    value: f64,
    #[serde(default)]
    block: Option<String>,
}

#[derive(Clone)]
pub(crate) struct FingerprintInput {
    pub(crate) fingerprints: Vec<Fingerprint>,
    pub(crate) blocks: Option<Vec<PatientExchangeabilityBlock>>,
}

pub(crate) fn read_fingerprints(input: impl std::io::Read) -> Result<FingerprintInput, CliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(input);
    let headers = reader.headers()?.clone();
    let blocked = headers
        .iter()
        .eq(["patient_id", "group", "feature", "value", "block"]);
    if !blocked
        && !headers
            .iter()
            .eq(["patient_id", "group", "feature", "value"])
    {
        return Err(CliError::Input(
            "CSV header must be exactly patient_id,group,feature,value or patient_id,group,feature,value,block"
                .into(),
        ));
    }
    let mut grouped = BTreeMap::<String, (String, Option<String>, BTreeMap<String, f64>)>::new();
    for decoded in reader.deserialize::<FingerprintCsvRecord>() {
        let row = decoded.map_err(|error| CliError::Input(error.to_string()))?;
        if blocked && row.block.is_none() {
            return Err(CliError::Input(format!(
                "patient {} is missing its fingerprint block",
                row.patient_id
            )));
        }
        let entry = grouped
            .entry(row.patient_id.clone())
            .or_insert_with(|| (row.group.clone(), row.block.clone(), BTreeMap::new()));
        if entry.0 != row.group {
            return Err(CliError::Input(format!(
                "patient {} has conflicting group labels",
                row.patient_id
            )));
        }
        if entry.1 != row.block {
            return Err(CliError::Input(format!(
                "patient {} has conflicting fingerprint blocks",
                row.patient_id
            )));
        }
        if entry.2.insert(row.feature.clone(), row.value).is_some() {
            return Err(CliError::Input(format!(
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
                    .map_err(|error| CliError::Input(error.to_string()))?,
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
