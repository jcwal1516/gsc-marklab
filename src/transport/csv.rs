use super::TransportCsvError;
use marklab_bayes::TransportMass;
use serde::Deserialize;
use std::collections::BTreeMap;
fn bounded(bytes: &[u8]) -> Result<(), TransportCsvError> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(TransportCsvError::Input(
            "transport CSV exceeds 16 MiB".into(),
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
struct SourceRow {
    source_id: String,
    mass: f64,
}

#[derive(Deserialize)]
struct TargetRow {
    target_id: String,
    mass: f64,
}

#[derive(Deserialize)]
struct CostRow {
    source_id: String,
    target_id: String,
    cost: f64,
}

pub(super) fn read_costs(
    bytes: &[u8],
    source: &[TransportMass],
    target: &[TransportMass],
) -> Result<Vec<f64>, TransportCsvError> {
    bounded(bytes)?;
    let mut costs = BTreeMap::new();
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader
        .headers()?
        .iter()
        .eq(["source_id", "target_id", "cost"])
    {
        return Err(TransportCsvError::Input(
            "transport cost header differs".into(),
        ));
    }
    for row in reader.deserialize::<CostRow>() {
        let row = row?;
        if costs.len() >= source.len() * target.len() {
            return Err(TransportCsvError::Input(
                "transport cost matrix exceeds declared dimensions".into(),
            ));
        }
        if costs
            .insert((row.source_id, row.target_id), row.cost)
            .is_some()
        {
            return Err(TransportCsvError::Input(
                "duplicate transport cost pair".into(),
            ));
        }
    }
    let matrix = source
        .iter()
        .flat_map(|source_row| {
            target.iter().map(|target_row| {
                costs
                    .get(&(source_row.id.clone(), target_row.id.clone()))
                    .copied()
                    .ok_or_else(|| {
                        TransportCsvError::Input("transport cost matrix is incomplete".into())
                    })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if costs.len() != matrix.len() {
        return Err(TransportCsvError::Input(
            "transport cost matrix contains undeclared identities".into(),
        ));
    }
    Ok(matrix)
}

pub(super) fn read_source(
    bytes: &[u8],
    maximum: usize,
) -> Result<Vec<TransportMass>, TransportCsvError> {
    bounded(bytes)?;
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader.headers()?.iter().eq(["source_id", "mass"]) {
        return Err(TransportCsvError::Input(
            "transport source header differs".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<SourceRow>() {
        if rows.len() >= maximum {
            return Err(TransportCsvError::Input(
                "transport source exceeds support limit".into(),
            ));
        }
        let row = row?;
        rows.push(TransportMass {
            id: row.source_id,
            mass: row.mass,
        });
    }
    Ok(rows)
}

pub(super) fn read_target(
    bytes: &[u8],
    maximum: usize,
) -> Result<Vec<TransportMass>, TransportCsvError> {
    bounded(bytes)?;
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_reader(bytes);
    if !reader.headers()?.iter().eq(["target_id", "mass"]) {
        return Err(TransportCsvError::Input(
            "transport target header differs".into(),
        ));
    }
    let mut rows = Vec::new();
    for row in reader.deserialize::<TargetRow>() {
        if rows.len() >= maximum {
            return Err(TransportCsvError::Input(
                "transport target exceeds support limit".into(),
            ));
        }
        let row = row?;
        rows.push(TransportMass {
            id: row.target_id,
            mass: row.mass,
        });
    }
    Ok(rows)
}
