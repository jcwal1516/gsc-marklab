use std::{collections::BTreeMap, path::Path};

use marklab::spatial::{ArbitraryWindowIppEvent, ArbitraryWindowIppQuadratureNode};
use marklab_bayes::json_sha256;
use serde::{Deserialize, Serialize};

use super::CliError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MembershipRow {
    event_id: String,
    quadrature_node_id: String,
}

pub(super) struct PreparedEventMembership {
    pub(super) digest: String,
    pub(super) observed_node_counts: Vec<u64>,
}

pub(super) fn prepare(
    path: &Path,
    events: &[ArbitraryWindowIppEvent],
    quadrature: &[ArbitraryWindowIppQuadratureNode],
) -> Result<PreparedEventMembership, CliError> {
    let mut reader = csv::ReaderBuilder::new().flexible(false).from_path(path)?;
    let rows = reader
        .deserialize()
        .collect::<Result<Vec<MembershipRow>, _>>()?;
    if rows.len() != events.len() || rows.len() > 100_000 {
        return Err(CliError::Input(
            "exact-window event membership must contain every event once within 100000 rows".into(),
        ));
    }
    let node_indices = quadrature
        .iter()
        .enumerate()
        .map(|(index, node)| (node.node_id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut counts = vec![0_u64; node_indices.len()];
    for (event, row) in events.iter().zip(&rows) {
        if event.event_id != row.event_id {
            return Err(CliError::Input(
                "exact-window membership event identities differ from canonical events".into(),
            ));
        }
        let index = node_indices
            .get(row.quadrature_node_id.as_str())
            .ok_or_else(|| {
                CliError::Input(format!(
                    "exact-window event {} references absent quadrature node {}",
                    row.event_id, row.quadrature_node_id
                ))
            })?;
        counts[*index] += 1;
    }
    Ok(PreparedEventMembership {
        digest: json_sha256(&rows)?,
        observed_node_counts: counts,
    })
}

pub(super) fn physical_neighbor_pairs(
    quadrature: &[ArbitraryWindowIppQuadratureNode],
    radius_um: f64,
) -> Result<Vec<[usize; 2]>, CliError> {
    if !radius_um.is_finite() || radius_um <= 0.0 {
        return Err(CliError::Input(
            "exact-window physical neighbor radius must be positive and finite".into(),
        ));
    }
    let radius_squared = radius_um * radius_um;
    let mut pairs = Vec::new();
    for left in 0..quadrature.len() {
        for right in (left + 1)..quadrature.len() {
            let dx = quadrature[left].x_um - quadrature[right].x_um;
            let dy = quadrature[left].y_um - quadrature[right].y_um;
            let distance_squared = dx.mul_add(dx, dy * dy);
            if !distance_squared.is_finite() {
                return Err(CliError::Input(
                    "exact-window physical neighbor distance is non-finite".into(),
                ));
            }
            if distance_squared <= radius_squared {
                pairs.push([left, right]);
            }
        }
    }
    Ok(pairs)
}
